use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
};

use base64::{engine::general_purpose::STANDARD, Engine};
use chrono::Utc;
use font_kit::source::SystemSource;
use image::{codecs::jpeg::JpegEncoder, DynamicImage, GenericImageView, ImageFormat};
use regex::Regex;
use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};
use tauri::{Manager, State};
use tempfile::tempdir;
use uuid::Uuid;
use walkdir::WalkDir;
use zip::{write::FileOptions, CompressionMethod, ZipArchive, ZipWriter};

use crate::{
    model::{
        AppSettings, AttachmentResult, BackupManifest, Folder, Note, NoteSummary, SaveNoteInput,
        Tag, WorkspaceState,
    },
    AppError, AppState,
};

fn lock_db(state: &AppState) -> Result<std::sync::MutexGuard<'_, Connection>, AppError> {
    state
        .db
        .lock()
        .map_err(|_| AppError::Other("数据库连接不可用".into()))
}

fn tags_for(db: &Connection, note_id: &str) -> Result<Vec<Tag>, AppError> {
    let mut statement = db.prepare(
        "SELECT t.id, t.name, t.color
         FROM tags t JOIN note_tags nt ON nt.tag_id = t.id
         WHERE nt.note_id = ?1 ORDER BY t.name COLLATE NOCASE",
    )?;
    let rows = statement.query_map([note_id], |row| {
        Ok(Tag {
            id: row.get(0)?,
            name: row.get(1)?,
            color: row.get(2)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

#[tauri::command]
pub fn list_notes(
    state: State<'_, AppState>,
    query: String,
    include_archived: bool,
    include_trashed: bool,
) -> Result<Vec<NoteSummary>, AppError> {
    let db = lock_db(&state)?;
    let pattern = format!("%{}%", query.trim().replace('%', "\\%").replace('_', "\\_"));
    let mut statement = db.prepare(
        "SELECT n.id, n.title, n.plain_text, n.created_at, n.updated_at, n.pinned, n.archived,
                n.trashed_at, n.folder_id, n.sort_order
         FROM notes n
         WHERE (?1 = 1 OR n.archived = 0)
           AND (?2 = 1 OR n.trashed_at IS NULL)
           AND (
             ?3 = '%%'
             OR n.title LIKE ?3 ESCAPE '\\'
             OR n.plain_text LIKE ?3 ESCAPE '\\'
             OR EXISTS (
               SELECT 1 FROM note_tags nt JOIN tags t ON t.id = nt.tag_id
               WHERE nt.note_id = n.id AND t.name LIKE ?3 ESCAPE '\\'
             )
           )
         ORDER BY n.pinned DESC, n.folder_id, n.sort_order, n.updated_at DESC",
    )?;
    let base = statement
        .query_map(params![include_archived as i32, include_trashed as i32, pattern], |row| {
            let plain: String = row.get(2)?;
            Ok(NoteSummary {
                id: row.get(0)?,
                title: row.get(1)?,
                preview: plain.chars().take(160).collect(),
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
                pinned: row.get::<_, i64>(5)? != 0,
                archived: row.get::<_, i64>(6)? != 0,
                trashed_at: row.get(7)?,
                folder_id: row.get(8)?,
                sort_order: row.get(9)?,
                tags: vec![],
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    base.into_iter()
        .map(|mut note| {
            note.tags = tags_for(&db, &note.id)?;
            Ok(note)
        })
        .collect()
}

#[tauri::command]
pub fn get_note(state: State<'_, AppState>, id: String) -> Result<Note, AppError> {
    let db = lock_db(&state)?;
    let (summary, content_json): (NoteSummary, String) = db
        .query_row(
            "SELECT id, title, plain_text, created_at, updated_at, pinned, archived,
                    trashed_at, folder_id, sort_order, content_json
             FROM notes WHERE id = ?1",
            [&id],
            |row| {
                let plain: String = row.get(2)?;
                Ok((
                    NoteSummary {
                        id: row.get(0)?,
                        title: row.get(1)?,
                        preview: plain.chars().take(160).collect(),
                        created_at: row.get(3)?,
                        updated_at: row.get(4)?,
                        pinned: row.get::<_, i64>(5)? != 0,
                        archived: row.get::<_, i64>(6)? != 0,
                        trashed_at: row.get(7)?,
                        folder_id: row.get(8)?,
                        sort_order: row.get(9)?,
                        tags: vec![],
                    },
                    row.get(10)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| AppError::Invalid("便签不存在".into()))?;
    let mut summary = summary;
    summary.tags = tags_for(&db, &id)?;
    Ok(Note {
        summary,
        content: serde_json::from_str(&content_json)?,
    })
}

#[tauri::command]
pub fn create_note(
    state: State<'_, AppState>,
    folder_id: Option<String>,
) -> Result<Note, AppError> {
    let db = lock_db(&state)?;
    if let Some(folder) = folder_id.as_deref() {
        let exists: bool = db.query_row(
            "SELECT EXISTS(SELECT 1 FROM folders WHERE id=?1)",
            [folder],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(AppError::Invalid("目标文件夹不存在".into()));
        }
    }
    let id = Uuid::new_v4().to_string();
    let timestamp = Utc::now().to_rfc3339();
    let sort_order: i64 = db.query_row(
        "SELECT COALESCE(MAX(sort_order), 0) + 1
         FROM notes WHERE folder_id IS ?1",
        [folder_id.as_deref()],
        |row| row.get(0),
    )?;
    let content = serde_json::json!({
        "type": "doc",
        "content": [{ "type": "paragraph" }]
    });
    db.execute(
        "INSERT INTO notes
         (id, title, content_json, plain_text, created_at, updated_at, pinned, archived,
          folder_id, sort_order)
         VALUES (?1, '新便签', ?2, '', ?3, ?3, 0, 0, ?4, ?5)",
        params![id, content.to_string(), timestamp, folder_id, sort_order],
    )?;
    Ok(Note {
        summary: NoteSummary {
            id,
            title: "新便签".into(),
            preview: String::new(),
            created_at: timestamp.clone(),
            updated_at: timestamp,
            pinned: false,
            archived: false,
            trashed_at: None,
            folder_id,
            sort_order,
            tags: vec![],
        },
        content,
    })
}

fn tag_color(index: usize) -> &'static str {
    const COLORS: [&str; 8] = [
        "#E5B83B", "#77A66C", "#689BC3", "#AF82C0",
        "#D47A65", "#C6945B", "#6DADA5", "#9B998F",
    ];
    COLORS[index % COLORS.len()]
}

#[tauri::command]
pub fn save_note(state: State<'_, AppState>, input: SaveNoteInput) -> Result<Note, AppError> {
    let mut db = lock_db(&state)?;
    let transaction = db.transaction()?;
    let timestamp = Utc::now().to_rfc3339();
    let derived_title: String = input.plain_text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(|line| line.chars().take(80).collect())
        .unwrap_or_else(|| "新便签".into());
    let changed = transaction.execute(
        "UPDATE notes SET title=?2, content_json=?3, plain_text=?4,
         updated_at=?5, pinned=?6, archived=?7 WHERE id=?1",
        params![
            input.id,
            derived_title,
            input.content.to_string(),
            input.plain_text,
            timestamp,
            input.pinned as i32,
            input.archived as i32,
        ],
    )?;
    if changed == 0 {
        return Err(AppError::Invalid("要保存的便签不存在".into()));
    }
    transaction.execute("DELETE FROM note_tags WHERE note_id=?1", [&input.id])?;
    for (index, name) in input
        .tags
        .iter()
        .map(|name| name.trim())
        .filter(|name| !name.is_empty())
        .take(12)
        .enumerate()
    {
        let existing: Option<String> = transaction
            .query_row(
                "SELECT id FROM tags WHERE name=?1 COLLATE NOCASE",
                [name],
                |row| row.get(0),
            )
            .optional()?;
        let tag_id = existing.unwrap_or_else(|| Uuid::new_v4().to_string());
        transaction.execute(
            "INSERT OR IGNORE INTO tags(id,name,color) VALUES (?1,?2,?3)",
            params![tag_id, name, tag_color(index)],
        )?;
        transaction.execute(
            "INSERT OR IGNORE INTO note_tags(note_id,tag_id) VALUES (?1,?2)",
            params![input.id, tag_id],
        )?;
    }
    transaction.commit()?;
    drop(db);
    get_note(state, input.id)
}

#[tauri::command]
pub fn delete_note(app: tauri::AppHandle, state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    let db = lock_db(&state)?;
    let timestamp = Utc::now().to_rfc3339();
    let changed = db.execute(
        "UPDATE notes SET trashed_at=?2, pinned=0 WHERE id=?1 AND trashed_at IS NULL",
        params![id, timestamp],
    )?;
    if changed == 0 {
        return Err(AppError::Invalid("便签不存在或已在回收站".into()));
    }
    remove_note_from_workspace(&db, &id)?;
    drop(db);
    crate::windows::hide_and_destroy_note_window(&app, &id);
    Ok(())
}

fn remove_note_from_workspace(db: &Connection, id: &str) -> Result<(), AppError> {
    let json: Option<String> = db
        .query_row("SELECT json FROM workspace_state WHERE id=1", [], |row| row.get(0))
        .optional()?;
    let Some(json) = json else { return Ok(()) };
    let mut workspace: WorkspaceState = serde_json::from_str(&json).unwrap_or_default();
    workspace.open_note_ids.retain(|note_id| note_id != id);
    if workspace.active_note_id.as_deref() == Some(id) {
        workspace.active_note_id = workspace.open_note_ids.last().cloned();
    }
    db.execute(
        "UPDATE workspace_state SET json=?1 WHERE id=1",
        [serde_json::to_string(&workspace)?],
    )?;
    Ok(())
}

#[tauri::command]
pub fn restore_note(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    let db = lock_db(&state)?;
    let changed = db.execute(
        "UPDATE notes SET trashed_at=NULL WHERE id=?1 AND trashed_at IS NOT NULL",
        [&id],
    )?;
    if changed == 0 {
        return Err(AppError::Invalid("回收站中找不到该便签".into()));
    }
    Ok(())
}

#[tauri::command]
pub fn permanently_delete_note(app: tauri::AppHandle, state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    let db = lock_db(&state)?;
    let changed = db.execute(
        "DELETE FROM notes WHERE id=?1 AND trashed_at IS NOT NULL",
        [&id],
    )?;
    if changed == 0 {
        return Err(AppError::Invalid("只能永久删除回收站中的便签".into()));
    }
    remove_note_from_workspace(&db, &id)?;
    drop(db);
    crate::windows::hide_and_destroy_note_window(&app, &id);
    Ok(())
}

#[tauri::command]
pub fn empty_trash(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<(), AppError> {
    let db = lock_db(&state)?;
    let ids = db.prepare("SELECT id FROM notes WHERE trashed_at IS NOT NULL")?
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    db.execute("DELETE FROM notes WHERE trashed_at IS NOT NULL", [])?;
    drop(db);
    for id in ids { crate::windows::hide_and_destroy_note_window(&app, &id); }
    Ok(())
}

#[tauri::command]
pub fn set_note_archived(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    id: String,
    archived: bool,
) -> Result<(), AppError> {
    let db = lock_db(&state)?;
    let changed = db.execute(
        "UPDATE notes SET archived=?2, updated_at=?3
         WHERE id=?1 AND trashed_at IS NULL",
        params![id, archived as i32, Utc::now().to_rfc3339()],
    )?;
    if changed == 0 {
        return Err(AppError::Invalid("便签不存在".into()));
    }
    drop(db);
    if archived { crate::windows::hide_and_destroy_note_window(&app, &id); }
    Ok(())
}

#[tauri::command]
pub fn set_note_pinned(
    state: State<'_, AppState>,
    id: String,
    pinned: bool,
) -> Result<(), AppError> {
    let db = lock_db(&state)?;
    let changed = db.execute(
        "UPDATE notes SET pinned=?2, updated_at=?3
         WHERE id=?1 AND trashed_at IS NULL",
        params![id, pinned as i32, Utc::now().to_rfc3339()],
    )?;
    if changed == 0 {
        return Err(AppError::Invalid("便签不存在".into()));
    }
    Ok(())
}

#[tauri::command]
pub fn list_folders(state: State<'_, AppState>) -> Result<Vec<Folder>, AppError> {
    let db = lock_db(&state)?;
    let mut statement = db.prepare(
        "SELECT id, name, sort_order, created_at, updated_at
         FROM folders ORDER BY sort_order, name COLLATE NOCASE",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(Folder {
            id: row.get(0)?,
            name: row.get(1)?,
            sort_order: row.get(2)?,
            created_at: row.get(3)?,
            updated_at: row.get(4)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn validate_folder_name(name: &str) -> Result<String, AppError> {
    let value = name.trim();
    let length = value.chars().count();
    if !(1..=40).contains(&length) {
        return Err(AppError::Invalid("文件夹名称需要 1–40 个字符".into()));
    }
    Ok(value.into())
}

#[tauri::command]
pub fn create_folder(state: State<'_, AppState>, name: String) -> Result<Folder, AppError> {
    let name = validate_folder_name(&name)?;
    let db = lock_db(&state)?;
    let duplicate: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM folders WHERE name=?1 COLLATE NOCASE)",
        [&name],
        |row| row.get(0),
    )?;
    if duplicate {
        return Err(AppError::Invalid("已经存在同名文件夹".into()));
    }
    let id = Uuid::new_v4().to_string();
    let timestamp = Utc::now().to_rfc3339();
    let sort_order: i64 =
        db.query_row("SELECT COALESCE(MAX(sort_order), 0) + 1 FROM folders", [], |row| row.get(0))?;
    db.execute(
        "INSERT INTO folders(id,name,sort_order,created_at,updated_at)
         VALUES(?1,?2,?3,?4,?4)",
        params![id, name, sort_order, timestamp],
    )?;
    Ok(Folder {
        id,
        name,
        sort_order,
        created_at: timestamp.clone(),
        updated_at: timestamp,
    })
}

#[tauri::command]
pub fn rename_folder(
    state: State<'_, AppState>,
    id: String,
    name: String,
) -> Result<Folder, AppError> {
    let name = validate_folder_name(&name)?;
    let db = lock_db(&state)?;
    let duplicate: bool = db.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM folders WHERE name=?1 COLLATE NOCASE AND id<>?2
         )",
        params![name, id],
        |row| row.get(0),
    )?;
    if duplicate {
        return Err(AppError::Invalid("已经存在同名文件夹".into()));
    }
    let timestamp = Utc::now().to_rfc3339();
    let changed = db.execute(
        "UPDATE folders SET name=?2,updated_at=?3 WHERE id=?1",
        params![id, name, timestamp],
    )?;
    if changed == 0 {
        return Err(AppError::Invalid("文件夹不存在".into()));
    }
    db.query_row(
        "SELECT id,name,sort_order,created_at,updated_at FROM folders WHERE id=?1",
        [&id],
        |row| {
            Ok(Folder {
                id: row.get(0)?,
                name: row.get(1)?,
                sort_order: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        },
    )
    .map_err(Into::into)
}

#[tauri::command]
pub fn delete_folder(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    let mut db = lock_db(&state)?;
    let transaction = db.transaction()?;
    transaction.execute(
        "UPDATE notes SET folder_id=NULL WHERE folder_id=?1",
        [&id],
    )?;
    let changed = transaction.execute("DELETE FROM folders WHERE id=?1", [&id])?;
    if changed == 0 {
        return Err(AppError::Invalid("文件夹不存在".into()));
    }
    let mut statement = transaction.prepare(
        "SELECT id FROM notes WHERE folder_id IS NULL ORDER BY sort_order, updated_at DESC",
    )?;
    let ids = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    drop(statement);
    normalize_folder_order(&transaction, None, &ids)?;
    transaction.commit()?;
    Ok(())
}

fn normalize_folder_order(
    tx: &rusqlite::Transaction<'_>,
    folder_id: Option<&str>,
    ordered_ids: &[String],
) -> Result<(), AppError> {
    for (index, note_id) in ordered_ids.iter().enumerate() {
        tx.execute(
            "UPDATE notes SET folder_id=?2, sort_order=?3 WHERE id=?1",
            params![note_id, folder_id, index as i64 + 1],
        )?;
    }
    Ok(())
}

#[tauri::command]
pub fn move_note(
    state: State<'_, AppState>,
    note_id: String,
    folder_id: Option<String>,
    before_note_id: Option<String>,
) -> Result<(), AppError> {
    let mut db = lock_db(&state)?;
    let transaction = db.transaction()?;
    let source_folder_id: Option<String> = transaction
        .query_row(
            "SELECT folder_id FROM notes WHERE id=?1",
            [&note_id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| AppError::Invalid("便签不存在".into()))?;
    if let Some(folder) = folder_id.as_deref() {
        let exists: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM folders WHERE id=?1)",
            [folder],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(AppError::Invalid("目标文件夹不存在".into()));
        }
    }
    let mut statement = transaction.prepare(
        "SELECT id FROM notes
         WHERE folder_id IS ?1 AND id<>?2
         ORDER BY sort_order, updated_at DESC",
    )?;
    let mut ids = statement
        .query_map(params![folder_id.as_deref(), note_id], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    drop(statement);
    let index = before_note_id
        .as_ref()
        .and_then(|before| ids.iter().position(|id| id == before))
        .unwrap_or(ids.len());
    ids.insert(index, note_id);
    normalize_folder_order(&transaction, folder_id.as_deref(), &ids)?;
    if source_folder_id != folder_id {
        let mut source_statement = transaction.prepare(
            "SELECT id FROM notes WHERE folder_id IS ?1 ORDER BY sort_order, updated_at DESC",
        )?;
        let source_ids = source_statement
            .query_map([source_folder_id.as_deref()], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        drop(source_statement);
        normalize_folder_order(&transaction, source_folder_id.as_deref(), &source_ids)?;
    }
    transaction.commit()?;
    Ok(())
}

#[tauri::command]
pub fn reorder_note(
    state: State<'_, AppState>,
    note_id: String,
    before_note_id: Option<String>,
) -> Result<(), AppError> {
    let folder_id: Option<String> = {
        let db = lock_db(&state)?;
        db.query_row(
            "SELECT folder_id FROM notes WHERE id=?1",
            [&note_id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| AppError::Invalid("便签不存在".into()))?
    };
    move_note(state, note_id, folder_id, before_note_id)
}

#[tauri::command]
pub fn get_workspace_state(state: State<'_, AppState>) -> Result<WorkspaceState, AppError> {
    let db = lock_db(&state)?;
    let json: Option<String> = db
        .query_row("SELECT json FROM workspace_state WHERE id=1", [], |row| row.get(0))
        .optional()?;
    Ok(json
        .and_then(|value| serde_json::from_str(&value).ok())
        .unwrap_or_default())
}

#[tauri::command]
pub fn save_workspace_state(
    state: State<'_, AppState>,
    workspace: WorkspaceState,
) -> Result<WorkspaceState, AppError> {
    let db = lock_db(&state)?;
    let mut open_note_ids = Vec::new();
    for id in workspace.open_note_ids {
        if open_note_ids.contains(&id) {
            continue;
        }
        let exists: bool = db.query_row(
            "SELECT EXISTS(SELECT 1 FROM notes WHERE id=?1)",
            [&id],
            |row| row.get(0),
        )?;
        if exists {
            open_note_ids.push(id);
        }
    }
    let active_note_id = workspace
        .active_note_id
        .filter(|id| open_note_ids.contains(id))
        .or_else(|| open_note_ids.first().cloned());
    let normalized = WorkspaceState {
        open_note_ids,
        active_note_id,
    };
    db.execute(
        "INSERT INTO workspace_state(id,json) VALUES(1,?1)
         ON CONFLICT(id) DO UPDATE SET json=excluded.json",
        [serde_json::to_string(&normalized)?],
    )?;
    Ok(normalized)
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Result<AppSettings, AppError> {
    let db = lock_db(&state)?;
    let value: Option<String> = db
        .query_row("SELECT json FROM settings WHERE id=1", [], |row| row.get(0))
        .optional()?;
    match value {
        Some(json) => {
            let defaults = serde_json::to_value(AppSettings::default())?;
            let mut stored: serde_json::Value = serde_json::from_str(&json)?;
            if let serde_json::Value::Object(values) = &mut stored {
                if !values.contains_key("defaultNoteAlwaysOnTop") {
                    if let Some(value) = values.remove("alwaysOnTop") {
                        values.insert("defaultNoteAlwaysOnTop".into(), value);
                    }
                } else {
                    values.remove("alwaysOnTop");
                }
                if !values.contains_key("defaultNoteBackgroundOpacity") {
                    if let Some(value) = values.remove("defaultNoteOpacity") {
                        values.insert("defaultNoteBackgroundOpacity".into(), value);
                    }
                } else {
                    values.remove("defaultNoteOpacity");
                }
            }
            let merged = match (defaults, stored) {
                (serde_json::Value::Object(mut base), serde_json::Value::Object(stored)) => {
                    base.extend(stored);
                    serde_json::Value::Object(base)
                }
                (base, _) => base,
            };
            Ok(serde_json::from_value::<AppSettings>(merged)?.migrate_legacy())
        }
        None => Ok(AppSettings::default()),
    }
}

#[tauri::command]
pub fn save_settings(
    state: State<'_, AppState>,
    settings: AppSettings,
) -> Result<AppSettings, AppError> {
    if !(9..=72).contains(&settings.default_font_size) {
        return Err(AppError::Invalid("默认字号必须在 9 到 72 之间".into()));
    }
    if settings.default_note_background_opacity > 100 {
        return Err(AppError::Invalid("默认背景透明度必须在 0% 到 100% 之间".into()));
    }
    if !(20..=100).contains(&settings.default_note_text_opacity) {
        return Err(AppError::Invalid("默认文字透明度必须在 20% 到 100% 之间".into()));
    }
    if settings.hotkey.trim().is_empty() || !settings.hotkey.contains('+') {
        return Err(AppError::Invalid("全局快捷键必须是组合键".into()));
    }
    if !matches!(settings.window_layout.as_str(), "integrated" | "separate") {
        return Err(AppError::Invalid("窗口布局设置无效".into()));
    }
    if !matches!(settings.hotkey_window_scope.as_str(), "editor" | "both") {
        return Err(AppError::Invalid("快捷键唤起范围无效".into()));
    }
    if !matches!(settings.hotkey_action.as_str(), "last-note" | "new-note") {
        return Err(AppError::Invalid("快捷键动作无效".into()));
    }
    let db = lock_db(&state)?;
    db.execute(
        "INSERT INTO settings(id,json) VALUES(1,?1)
         ON CONFLICT(id) DO UPDATE SET json=excluded.json",
        [serde_json::to_string(&settings)?],
    )?;
    Ok(settings)
}

#[tauri::command]
pub fn list_system_fonts() -> Result<Vec<String>, AppError> {
    let mut families = SystemSource::new()
        .all_families()
        .map_err(|error| AppError::Other(format!("读取系统字体失败：{error}")))?;
    families.sort_by_key(|a| a.to_lowercase());
    families.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    Ok(families)
}

fn encode_image(image: &DynamicImage, use_jpeg: bool) -> Result<(Vec<u8>, &'static str), AppError> {
    let mut output = Vec::new();
    if use_jpeg {
        let rgb = image.to_rgb8();
        JpegEncoder::new_with_quality(&mut output, 90)
            .encode(&rgb, rgb.width(), rgb.height(), image::ExtendedColorType::Rgb8)
            .map_err(|error| AppError::Other(format!("图片编码失败：{error}")))?;
        Ok((output, "image/jpeg"))
    } else {
        image
            .write_to(&mut Cursor::new(&mut output), ImageFormat::Png)
            .map_err(|error| AppError::Other(format!("图片编码失败：{error}")))?;
        Ok((output, "image/png"))
    }
}

#[tauri::command]
pub fn save_attachment(
    state: State<'_, AppState>,
    data_base64: String,
    mime: String,
) -> Result<AttachmentResult, AppError> {
    let encoded = data_base64
        .split_once(',')
        .map(|(_, value)| value)
        .unwrap_or(data_base64.as_str());
    let bytes = STANDARD
        .decode(encoded)
        .map_err(|_| AppError::Invalid("剪贴板图片数据无效".into()))?;
    if bytes.len() > 80 * 1024 * 1024 {
        return Err(AppError::Invalid("单张图片不能超过 80MB".into()));
    }
    let decoded = image::load_from_memory(&bytes)
        .map_err(|error| AppError::Invalid(format!("无法读取图片：{error}")))?;
    let (width, height) = decoded.dimensions();
    let image = if width > 4096 || height > 4096 {
        decoded.thumbnail(4096, 4096)
    } else {
        decoded
    };
    let use_jpeg = mime.eq_ignore_ascii_case("image/jpeg") || mime.eq_ignore_ascii_case("image/jpg");
    let (output, saved_mime) = encode_image(&image, use_jpeg)?;
    let digest = format!("{:x}", Sha256::digest(&output));
    let extension = if use_jpeg { "jpg" } else { "png" };
    let asset_path = format!("{digest}.{extension}");
    let file_path = state.assets_dir.join(&asset_path);
    if !file_path.exists() {
        fs::write(&file_path, &output)?;
    }
    let db = lock_db(&state)?;
    let existing: Option<String> = db
        .query_row(
            "SELECT id FROM attachments WHERE sha256=?1",
            [&digest],
            |row| row.get(0),
        )
        .optional()?;
    let id = existing.unwrap_or_else(|| Uuid::new_v4().to_string());
    db.execute(
        "INSERT OR IGNORE INTO attachments
         (id,sha256,asset_path,mime,width,height,size_bytes,created_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        params![
            id,
            digest,
            asset_path,
            saved_mime,
            image.width(),
            image.height(),
            output.len() as u64,
            Utc::now().to_rfc3339(),
        ],
    )?;
    Ok(AttachmentResult {
        id,
        asset_path,
        file_path: file_path.to_string_lossy().into_owned(),
        mime: saved_mime.into(),
        width: image.width(),
        height: image.height(),
        size_bytes: output.len() as u64,
    })
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[tauri::command]
pub fn export_html(
    state: State<'_, AppState>,
    note_id: String,
    destination: String,
    html: String,
) -> Result<(), AppError> {
    let destination = PathBuf::from(destination);
    let parent = destination
        .parent()
        .ok_or_else(|| AppError::Invalid("导出路径无效".into()))?;
    fs::create_dir_all(parent)?;
    let stem = destination
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("轻笺");
    let resource_name = format!("{stem}_assets");
    let resource_dir = parent.join(&resource_name);
    fs::create_dir_all(&resource_dir)?;

    let asset_regex = Regex::new(r#"data-asset-path="([^"]+)""#)
        .map_err(|error| AppError::Other(error.to_string()))?;
    let src_regex = Regex::new(r#"src="[^"]*""#)
        .map_err(|error| AppError::Other(error.to_string()))?;
    let mut rendered = html.clone();
    for captures in asset_regex.captures_iter(&html) {
        let Some(asset) = captures.get(1).map(|value| value.as_str()) else { continue };
        let Some(file_name) = Path::new(asset).file_name() else { continue };
        let source = state.assets_dir.join(file_name);
        if source.is_file() {
            fs::copy(&source, resource_dir.join(file_name))?;
            let asset_marker = format!(r#"data-asset-path="{asset}""#);
            if let Some(index) = rendered.find(&asset_marker) {
                let tag_start = rendered[..index].rfind("<img").unwrap_or(index);
                let tag_end = rendered[index..]
                    .find('>')
                    .map(|offset| index + offset + 1)
                    .unwrap_or(index);
                if tag_end > tag_start {
                    let original = &rendered[tag_start..tag_end];
                    let replacement_src = format!(
                        r#"src="{}/{}""#,
                        resource_name,
                        file_name.to_string_lossy()
                    );
                    let replaced = if src_regex.is_match(original) {
                        src_regex.replace(original, replacement_src).into_owned()
                    } else {
                        original.replacen("<img", &format!("<img {replacement_src}"), 1)
                    };
                    rendered.replace_range(tag_start..tag_end, &replaced);
                }
            }
        }
    }

    let title = {
        let db = lock_db(&state)?;
        db.query_row("SELECT title FROM notes WHERE id=?1", [note_id], |row| row.get::<_, String>(0))
            .optional()?
            .unwrap_or_else(|| "轻笺".into())
    };
    let document = format!(
        r#"<!doctype html>
<html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width">
<title>{}</title><style>
body{{max-width:820px;margin:48px auto;padding:0 28px;color:#2c2b28;background:#fffefa;font:16px/1.7 "Microsoft YaHei UI",sans-serif}}
body::after{{content:"";display:block;clear:both}}h1{{font-size:30px}}
img{{display:block;max-width:100%;height:auto;border-radius:8px}}
img:not([data-layout]),img[data-layout="block-center"]{{clear:both;margin:14px auto}}
img[data-layout="block-left"]{{clear:both;margin:14px auto 14px 0}}
img[data-layout="block-right"]{{clear:both;margin:14px 0 14px auto}}
img[data-layout="wrap-left"]{{float:left;clear:left;margin:7px 18px 10px 0}}
img[data-layout="wrap-right"]{{float:right;clear:right;margin:7px 0 10px 18px}}
h1,h2,h3,table,blockquote,pre{{clear:both}}
table{{border-collapse:collapse;width:100%;margin:14px 0}}td,th{{border:1px solid #cfc8ba;padding:7px 9px;text-align:left}}th{{background:#f5f1e7}}
blockquote{{margin-left:0;padding-left:14px;border-left:3px solid #f1d887;color:#736e65}}
@media print{{body{{margin:0;max-width:none}}}}
</style></head><body>{}</body></html>"#,
        escape_html(&title),
        rendered
    );
    fs::write(destination, document)?;
    Ok(())
}

#[tauri::command]
pub fn export_markdown(
    state: State<'_, AppState>,
    destination: String,
    markdown: String,
) -> Result<(), AppError> {
    let destination = PathBuf::from(destination);
    let parent = destination
        .parent()
        .ok_or_else(|| AppError::Invalid("导出路径无效".into()))?;
    fs::create_dir_all(parent)?;
    let stem = destination
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("轻笺");
    let resource_name = format!("{stem}_assets");
    let resource_dir = parent.join(&resource_name);
    let asset_regex = Regex::new(r"qingjian-asset:([^\s\)>\]]+)")
        .map_err(|error| AppError::Other(error.to_string()))?;
    let mut rendered = markdown.clone();
    let mut created_resource_dir = false;

    for captures in asset_regex.captures_iter(&markdown) {
        let Some(marker) = captures.get(0).map(|value| value.as_str()) else { continue };
        let Some(asset) = captures.get(1).map(|value| value.as_str()) else { continue };
        let Some(file_name) = Path::new(asset).file_name() else { continue };
        let source = state.assets_dir.join(file_name);
        if !source.is_file() { continue }
        if !created_resource_dir {
            fs::create_dir_all(&resource_dir)?;
            created_resource_dir = true;
        }
        fs::copy(&source, resource_dir.join(file_name))?;
        rendered = rendered.replace(
            marker,
            &format!("{resource_name}/{}", file_name.to_string_lossy()),
        );
    }

    fs::write(destination, rendered)?;
    Ok(())
}

#[cfg(windows)]
#[tauri::command]
pub async fn export_pdf(
    app: tauri::AppHandle,
    note_id: String,
    destination: String,
) -> Result<(), AppError> {
    use std::{sync::mpsc, time::Duration};
    use webview2_com::{
        Microsoft::Web::WebView2::Win32::{
            ICoreWebView2Environment6, ICoreWebView2_7,
        },
        PrintToPdfCompletedHandler,
    };
    use windows::core::{HSTRING, Interface};

    let webview = app
        .get_webview_window(&format!("note-{note_id}"))
        .ok_or_else(|| AppError::Other("找不到当前便签窗口".into()))?;
    let (sender, receiver) = mpsc::channel::<Result<(), String>>();
    webview
        .with_webview(move |platform| {
            let error_sender = sender.clone();
            let result = (|| -> windows::core::Result<()> {
                let core = unsafe { platform.controller().CoreWebView2()? };
                let core7: ICoreWebView2_7 = core.cast()?;
                let environment6: ICoreWebView2Environment6 = platform.environment().cast()?;
                let print_settings = unsafe { environment6.CreatePrintSettings()? };
                let output_path = HSTRING::from(destination);
                let completed = PrintToPdfCompletedHandler::create(Box::new(
                    move |status, success| {
                        let outcome = status
                            .map_err(|error| error.to_string())
                            .and_then(|_| {
                                if success {
                                    Ok(())
                                } else {
                                    Err("WebView2 未能生成 PDF".into())
                                }
                            });
                        let _ = sender.send(outcome);
                        Ok(())
                    },
                ));
                unsafe {
                    core7.PrintToPdf(&output_path, &print_settings, &completed)?;
                }
                Ok(())
            })();
            if let Err(error) = result {
                let _ = error_sender.send(Err(error.to_string()));
            }
        })
        .map_err(|error| AppError::Other(format!("启动 PDF 导出失败：{error}")))?;

    tauri::async_runtime::spawn_blocking(move || {
        receiver
            .recv_timeout(Duration::from_secs(60))
            .map_err(|_| AppError::Other("PDF 导出超时".into()))?
            .map_err(AppError::Other)
    })
    .await
    .map_err(|error| AppError::Other(format!("PDF 导出任务失败：{error}")))?
}

fn write_backup(state: &State<'_, AppState>, destination: &Path) -> Result<(), AppError> {
    {
        let db = lock_db(state)?;
        db.execute_batch("PRAGMA wal_checkpoint(FULL);")?;
    }
    let database_path = state.data_dir.join("qingjian.db");
    let mut files = BTreeMap::new();
    files.insert("qingjian.db".into(), hash_file(&database_path)?);
    let assets = WalkDir::new(&state.assets_dir)
        .min_depth(1)
        .max_depth(1)
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| AppError::Other(error.to_string()))?;
    for entry in &assets {
        if entry.file_type().is_file() {
            files.insert(
                format!("assets/{}", entry.file_name().to_string_lossy()),
                hash_file(entry.path())?,
            );
        }
    }
    let manifest = BackupManifest {
        format: "qingjian-backup".into(),
        schema_version: 5,
        app_version: env!("CARGO_PKG_VERSION").into(),
        created_at: Utc::now().to_rfc3339(),
        files,
    };
    let file = File::create(destination)?;
    let mut zip = ZipWriter::new(file);
    let options = FileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o644);
    zip.start_file("manifest.json", options)?;
    zip.write_all(serde_json::to_string_pretty(&manifest)?.as_bytes())?;
    zip.start_file("qingjian.db", options)?;
    let mut database = File::open(&database_path)?;
    std::io::copy(&mut database, &mut zip)?;
    for entry in assets {
        if entry.file_type().is_file() {
            let name = format!("assets/{}", entry.file_name().to_string_lossy());
            zip.start_file(name, options)?;
            let mut asset = File::open(entry.path())?;
            std::io::copy(&mut asset, &mut zip)?;
        }
    }
    zip.finish()?;
    Ok(())
}

fn hash_file(path: &Path) -> Result<String, AppError> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

#[tauri::command]
pub fn export_backup(
    state: State<'_, AppState>,
    destination: String,
) -> Result<(), AppError> {
    write_backup(&state, Path::new(&destination))
}

#[tauri::command]
pub fn restore_backup(
    state: State<'_, AppState>,
    source: String,
    replace: bool,
) -> Result<(), AppError> {
    let temp = tempdir()?;
    let file = File::open(source)?;
    let mut archive = ZipArchive::new(file)?;
    let manifest: BackupManifest = {
        let mut entry = archive.by_name("manifest.json")?;
        let mut json = String::new();
        entry.read_to_string(&mut json)?;
        serde_json::from_str(&json)?
    };
    if manifest.format != "qingjian-backup" || manifest.schema_version > 5 {
        return Err(AppError::Invalid("该备份版本不受当前轻笺支持".into()));
    }
    let incoming_path = temp.path().join("incoming.db");
    {
        let mut entry = archive.by_name("qingjian.db")?;
        let mut output = File::create(&incoming_path)?;
        std::io::copy(&mut entry, &mut output)?;
    }
    if let Some(expected) = manifest.files.get("qingjian.db") {
        if &hash_file(&incoming_path)? != expected {
            return Err(AppError::Invalid("备份数据库校验失败，文件可能已损坏".into()));
        }
    }
    let incoming = Connection::open(&incoming_path)?;
    let version: u32 = incoming.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version > 5 {
        return Err(AppError::Invalid("备份数据库版本过新".into()));
    }
    drop(incoming);

    if replace {
        let safety = state
            .data_dir
            .join("backups")
            .join(format!("恢复前-{}.qingjian-backup", Utc::now().format("%Y%m%d-%H%M%S")));
        write_backup(&state, &safety)?;
    }

    {
        let mut db = lock_db(&state)?;
        db.execute("ATTACH DATABASE ?1 AS incoming", [incoming_path.to_string_lossy().as_ref()])?;
        let tx = db.transaction()?;
        if replace {
            tx.execute_batch(
                "DELETE FROM note_tags; DELETE FROM tags; DELETE FROM notes;
                 DELETE FROM folders; DELETE FROM attachments; DELETE FROM settings;
                 DELETE FROM workspace_state; DELETE FROM note_window_geometry;
                 DELETE FROM note_window_state;",
            )?;
        }
        let insert_mode = if replace { "OR REPLACE" } else { "OR IGNORE" };
        if version >= 3 {
            tx.execute_batch(&format!(
                "INSERT {insert_mode} INTO folders(id,name,sort_order,created_at,updated_at)
                    SELECT id,name,sort_order,created_at,updated_at FROM incoming.folders;
                 INSERT {insert_mode} INTO notes(
                    id,title,content_json,plain_text,created_at,updated_at,pinned,archived,
                    folder_id,sort_order,trashed_at
                 )
                    SELECT id,title,content_json,plain_text,created_at,updated_at,pinned,archived,
                           folder_id,sort_order,trashed_at FROM incoming.notes;"
            ))?;
        } else if version >= 2 {
            tx.execute_batch(&format!(
                "INSERT {insert_mode} INTO folders(id,name,sort_order,created_at,updated_at)
                    SELECT id,name,sort_order,created_at,updated_at FROM incoming.folders;
                 INSERT {insert_mode} INTO notes(
                    id,title,content_json,plain_text,created_at,updated_at,pinned,archived,
                    folder_id,sort_order,trashed_at
                 )
                    SELECT id,title,content_json,plain_text,created_at,updated_at,pinned,archived,
                           folder_id,sort_order,NULL FROM incoming.notes;"
            ))?;
        } else {
            tx.execute_batch(&format!(
                "INSERT {insert_mode} INTO notes(
                    id,title,content_json,plain_text,created_at,updated_at,pinned,archived,
                    folder_id,sort_order,trashed_at
                 )
                    SELECT id,title,content_json,plain_text,created_at,updated_at,pinned,archived,
                           NULL,rowid,NULL FROM incoming.notes;"
            ))?;
        }
        tx.execute_batch(&format!(
            "INSERT {insert_mode} INTO tags(id,name,color)
                SELECT id,name,color FROM incoming.tags;
             INSERT {insert_mode} INTO note_tags(note_id,tag_id)
                SELECT note_id,tag_id FROM incoming.note_tags;
             INSERT {insert_mode} INTO attachments(
                id,sha256,asset_path,mime,width,height,size_bytes,created_at
             )
                SELECT id,sha256,asset_path,mime,width,height,size_bytes,created_at
                FROM incoming.attachments;"
        ))?;
        if version >= 5 {
            tx.execute_batch(&format!(
                "INSERT {insert_mode} INTO note_window_state(
                    note_id,mode,visible,always_on_top,opacity,text_opacity,locked,dock_edge,dock_offset,monitor_key,updated_at
                 ) SELECT note_id,mode,visible,always_on_top,opacity,text_opacity,locked,dock_edge,dock_offset,monitor_key,updated_at
                   FROM incoming.note_window_state;
                 INSERT {insert_mode} INTO note_window_geometry(
                    note_id,mode,x_ratio,y_ratio,width_dip,height_dip,work_area_x,work_area_y,
                    work_area_width,work_area_height,scale_factor
                 ) SELECT note_id,mode,x_ratio,y_ratio,width_dip,height_dip,work_area_x,work_area_y,
                          work_area_width,work_area_height,scale_factor
                   FROM incoming.note_window_geometry;"
            ))?;
        } else if version >= 4 {
            tx.execute_batch(&format!(
                "INSERT {insert_mode} INTO note_window_state(
                    note_id,mode,visible,always_on_top,opacity,text_opacity,locked,dock_edge,dock_offset,monitor_key,updated_at
                 ) SELECT note_id,mode,visible,always_on_top,opacity,100,locked,dock_edge,dock_offset,monitor_key,updated_at
                   FROM incoming.note_window_state;
                 INSERT {insert_mode} INTO note_window_geometry(
                    note_id,mode,x_ratio,y_ratio,width_dip,height_dip,work_area_x,work_area_y,
                    work_area_width,work_area_height,scale_factor
                 ) SELECT note_id,mode,x_ratio,y_ratio,width_dip,height_dip,work_area_x,work_area_y,
                          work_area_width,work_area_height,scale_factor
                   FROM incoming.note_window_geometry;"
            ))?;
        }
        if replace {
            tx.execute_batch("INSERT OR REPLACE INTO settings SELECT * FROM incoming.settings;")?;
            if version >= 2 {
                tx.execute_batch(
                    "INSERT OR REPLACE INTO workspace_state
                     SELECT * FROM incoming.workspace_state;",
                )?;
            }
        }
        tx.commit()?;
        db.execute_batch("DETACH DATABASE incoming;")?;
        crate::state::migrate_legacy_note_titles(&db)?;
    }

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        let name = entry.name().to_owned();
        if !name.starts_with("assets/") || entry.is_dir() {
            continue;
        }
        let Some(file_name) = Path::new(&name).file_name() else { continue };
        let destination = state.assets_dir.join(file_name);
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes)?;
        if let Some(expected) = manifest.files.get(&name) {
            let actual = format!("{:x}", Sha256::digest(&bytes));
            if &actual != expected {
                return Err(AppError::Invalid(format!("备份资源校验失败：{name}")));
            }
        }
        if !destination.exists() {
            fs::write(destination, bytes)?;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn quit_app(app: tauri::AppHandle) {
    app.exit(0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_escape_handles_unsafe_characters() {
        assert_eq!(escape_html("<轻笺 & \"test\">"), "&lt;轻笺 &amp; &quot;test&quot;&gt;");
    }

    #[test]
    fn image_encoder_produces_png() {
        let image = DynamicImage::new_rgba8(16, 16);
        let (bytes, mime) = encode_image(&image, false).unwrap();
        assert_eq!(mime, "image/png");
        assert!(bytes.starts_with(&[137, 80, 78, 71]));
    }

    #[test]
    fn folder_order_is_renumbered_without_gaps() {
        let mut db = Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE notes(
                id TEXT PRIMARY KEY, folder_id TEXT, sort_order INTEGER NOT NULL
             );
             INSERT INTO notes VALUES('a','folder',50),('b','folder',900),('c','folder',4);",
        )
        .unwrap();
        let tx = db.transaction().unwrap();
        normalize_folder_order(
            &tx,
            Some("folder"),
            &["b".into(), "a".into(), "c".into()],
        )
        .unwrap();
        tx.commit().unwrap();
        let ordered = db
            .prepare("SELECT id,sort_order FROM notes ORDER BY sort_order")
            .unwrap()
            .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(
            ordered,
            vec![("b".into(), 1), ("a".into(), 2), ("c".into(), 3)]
        );
    }
}
