use rusqlite::Connection;
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
        assert_eq!(version, 3);
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
    fn fresh_database_starts_at_schema_three() {
        let db = Connection::open_in_memory().unwrap();
        migrate(&db).unwrap();
        let version: u32 = db.query_row("PRAGMA user_version", [], |row| row.get(0)).unwrap();
        let tables: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type='table' AND name IN ('folders','notes','workspace_state')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(version, 3);
        assert_eq!(tables, 3);
    }
}
