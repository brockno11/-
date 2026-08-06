use rusqlite::{Connection, OptionalExtension};
use std::{
    fs,
    path::PathBuf,
    sync::Mutex,
};

use crate::AppError;

pub struct AppState {
    pub db: Mutex<Connection>,
    pub data_dir: PathBuf,
    pub assets_dir: PathBuf,
}

impl AppState {
    pub fn initialize(data_dir: PathBuf) -> Result<Self, AppError> {
        let assets_dir = data_dir.join("assets");
        fs::create_dir_all(&assets_dir)?;
        fs::create_dir_all(data_dir.join("backups"))?;
        let db_path = data_dir.join("qingjian.db");
        let connection = Connection::open(db_path)?;
        migrate(&connection)?;
        Ok(Self {
            db: Mutex::new(connection),
            data_dir,
            assets_dir,
        })
    }
}

fn has_column(db: &Connection, table: &str, column: &str) -> Result<bool, AppError> {
    let mut statement = db.prepare(&format!("PRAGMA table_info({table})"))?;
    let names = statement
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(names.iter().any(|name| name == column))
}

pub(crate) fn migrate(db: &Connection) -> Result<(), AppError> {
    db.execute_batch(
        r#"
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;
        PRAGMA foreign_keys = ON;
        PRAGMA busy_timeout = 3000;

        CREATE TABLE IF NOT EXISTS notes (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            content_json TEXT NOT NULL,
            plain_text TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            pinned INTEGER NOT NULL DEFAULT 0,
            archived INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_notes_updated ON notes(pinned DESC, updated_at DESC);
        CREATE INDEX IF NOT EXISTS idx_notes_archived ON notes(archived);

        CREATE TABLE IF NOT EXISTS tags (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL UNIQUE COLLATE NOCASE,
            color TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS note_tags (
            note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
            tag_id TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
            PRIMARY KEY(note_id, tag_id)
        );
        CREATE TABLE IF NOT EXISTS attachments (
            id TEXT PRIMARY KEY,
            sha256 TEXT NOT NULL UNIQUE,
            asset_path TEXT NOT NULL,
            mime TEXT NOT NULL,
            width INTEGER NOT NULL,
            height INTEGER NOT NULL,
            size_bytes INTEGER NOT NULL,
            created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS settings (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            json TEXT NOT NULL
        );
        "#,
    )?;

    let version: u32 = db.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version < 2 {
        if !has_column(db, "notes", "folder_id")? {
            db.execute_batch("ALTER TABLE notes ADD COLUMN folder_id TEXT;")?;
        }
        if !has_column(db, "notes", "sort_order")? {
            db.execute_batch(
                "ALTER TABLE notes ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0;",
            )?;
        }
        db.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS folders (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE COLLATE NOCASE,
                sort_order INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_folders_order
                ON folders(sort_order, name COLLATE NOCASE);
            CREATE INDEX IF NOT EXISTS idx_notes_folder_order
                ON notes(folder_id, sort_order, updated_at DESC);
            CREATE TABLE IF NOT EXISTS workspace_state (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                json TEXT NOT NULL
            );
            INSERT OR IGNORE INTO workspace_state(id, json)
                VALUES(1, '{"openNoteIds":[],"activeNoteId":null}');
            UPDATE notes SET sort_order = rowid WHERE sort_order = 0;
            PRAGMA user_version = 2;
            "#,
        )?;
    }
    if version < 3 {
        if !has_column(db, "notes", "trashed_at")? {
            db.execute_batch("ALTER TABLE notes ADD COLUMN trashed_at TEXT;")?;
        }
        db.execute_batch(
            r#"
            CREATE INDEX IF NOT EXISTS idx_notes_trashed
                ON notes(trashed_at);
            PRAGMA user_version = 3;
            "#,
        )?;
    }
    if version < 4 {
        db.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS note_window_state (
                note_id TEXT PRIMARY KEY REFERENCES notes(id) ON DELETE CASCADE,
                mode TEXT NOT NULL DEFAULT 'card',
                visible INTEGER NOT NULL DEFAULT 0,
                always_on_top INTEGER NOT NULL DEFAULT 0,
                opacity INTEGER NOT NULL DEFAULT 100,
                locked INTEGER NOT NULL DEFAULT 0,
                dock_edge TEXT,
                dock_offset REAL NOT NULL DEFAULT 0,
                monitor_key TEXT,
                updated_at TEXT NOT NULL DEFAULT ''
            );
            CREATE TABLE IF NOT EXISTS note_window_geometry (
                note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
                mode TEXT NOT NULL,
                x_ratio REAL NOT NULL DEFAULT 0.5,
                y_ratio REAL NOT NULL DEFAULT 0.5,
                width_dip REAL NOT NULL,
                height_dip REAL NOT NULL,
                work_area_x INTEGER NOT NULL DEFAULT 0,
                work_area_y INTEGER NOT NULL DEFAULT 0,
                work_area_width INTEGER NOT NULL DEFAULT 0,
                work_area_height INTEGER NOT NULL DEFAULT 0,
                scale_factor REAL NOT NULL DEFAULT 1,
                PRIMARY KEY(note_id, mode)
            );
            CREATE INDEX IF NOT EXISTS idx_note_window_visible
                ON note_window_state(visible, updated_at DESC);
            PRAGMA user_version = 4;
            "#,
        )?;

        let workspace_json: Option<String> = db
            .query_row("SELECT json FROM workspace_state WHERE id=1", [], |row| row.get(0))
            .optional()?;
        let active_note_id = workspace_json
            .and_then(|json| serde_json::from_str::<serde_json::Value>(&json).ok())
            .and_then(|value| value.get("activeNoteId").and_then(|id| id.as_str()).map(str::to_owned));
        if let Some(note_id) = active_note_id {
            let exists: bool = db.query_row(
                "SELECT EXISTS(SELECT 1 FROM notes WHERE id=?1 AND archived=0 AND trashed_at IS NULL)",
                [&note_id],
                |row| row.get(0),
            )?;
            if exists {
                let always_on_top = db
                    .query_row("SELECT json FROM settings WHERE id=1", [], |row| row.get::<_, String>(0))
                    .optional()?
                    .and_then(|json| serde_json::from_str::<serde_json::Value>(&json).ok())
                    .and_then(|value| value.get("alwaysOnTop").and_then(|value| value.as_bool()))
                    .unwrap_or(false);
                db.execute(
                    "INSERT OR IGNORE INTO note_window_state(
                        note_id,mode,visible,always_on_top,opacity,locked,dock_offset,updated_at
                     ) VALUES(?1,'editor',1,?2,100,0,0,datetime('now'))",
                    rusqlite::params![note_id, always_on_top as i32],
                )?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_v1_database_without_losing_notes() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(
            r#"
            CREATE TABLE notes (
                id TEXT PRIMARY KEY, title TEXT NOT NULL, content_json TEXT NOT NULL,
                plain_text TEXT NOT NULL DEFAULT '', created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL, pinned INTEGER NOT NULL DEFAULT 0,
                archived INTEGER NOT NULL DEFAULT 0
            );
            INSERT INTO notes VALUES(
                'one','旧便签','{"type":"doc"}','内容','now','now',0,0
            );
            PRAGMA user_version = 1;
            "#,
        )
        .unwrap();
        migrate(&db).unwrap();
        let version: u32 = db.query_row("PRAGMA user_version", [], |row| row.get(0)).unwrap();
        let title: String = db
            .query_row("SELECT title FROM notes WHERE id='one'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, 4);
        assert_eq!(title, "旧便签");
        assert!(has_column(&db, "notes", "folder_id").unwrap());
        assert!(has_column(&db, "notes", "sort_order").unwrap());
        assert!(has_column(&db, "notes", "trashed_at").unwrap());
        let folder_count: i64 = db
            .query_row("SELECT COUNT(*) FROM folders", [], |row| row.get(0))
            .unwrap();
        let workspace_json: String = db
            .query_row("SELECT json FROM workspace_state WHERE id=1", [], |row| row.get(0))
            .unwrap();
        assert_eq!(folder_count, 0);
        assert!(workspace_json.contains("openNoteIds"));
    }

    #[test]
    fn fresh_database_starts_at_schema_four() {
        let db = Connection::open_in_memory().unwrap();
        migrate(&db).unwrap();
        let version: u32 = db.query_row("PRAGMA user_version", [], |row| row.get(0)).unwrap();
        let tables: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type='table' AND name IN (
                    'folders','notes','workspace_state','note_window_state','note_window_geometry'
                 )",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(version, 4);
        assert_eq!(tables, 5);
    }

    #[test]
    fn v3_migration_only_restores_the_active_note_window() {
        let db = Connection::open_in_memory().unwrap();
        migrate(&db).unwrap();
        db.execute_batch(
            "INSERT INTO notes(id,title,content_json,plain_text,created_at,updated_at,pinned,archived,sort_order)
             VALUES('active','A','{}','','now','now',0,0,1),
                   ('other','B','{}','','now','now',0,0,2);
             UPDATE workspace_state SET json='{\"openNoteIds\":[\"active\",\"other\"],\"activeNoteId\":\"active\"}';
             DELETE FROM note_window_state;
             DELETE FROM note_window_geometry;
             PRAGMA user_version=3;"
        ).unwrap();
        migrate(&db).unwrap();
        let rows: Vec<(String, String, bool)> = db
            .prepare("SELECT note_id,mode,visible FROM note_window_state")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(rows, vec![("active".into(), "editor".into(), true)]);
    }

    #[test]
    fn deleting_a_note_cascades_its_window_state_and_geometry() {
        let db = Connection::open_in_memory().unwrap();
        migrate(&db).unwrap();
        db.execute_batch(
            "INSERT INTO notes(id,title,content_json,plain_text,created_at,updated_at,pinned,archived,sort_order)
             VALUES('n','N','{}','','now','now',0,0,1);
             INSERT INTO note_window_state(note_id,mode,visible,updated_at) VALUES('n','card',1,'now');
             INSERT INTO note_window_geometry(note_id,mode,width_dip,height_dip) VALUES('n','card',360,280);
             DELETE FROM notes WHERE id='n';"
        ).unwrap();
        let states: i64 = db.query_row("SELECT COUNT(*) FROM note_window_state", [], |row| row.get(0)).unwrap();
        let geometries: i64 = db.query_row("SELECT COUNT(*) FROM note_window_geometry", [], |row| row.get(0)).unwrap();
        assert_eq!((states, geometries), (0, 0));
    }
}
