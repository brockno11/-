use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tag {
    pub id: String,
    pub name: String,
    pub color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
    pub id: String,
    pub name: String,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteSummary {
    pub id: String,
    pub title: String,
    pub preview: String,
    pub created_at: String,
    pub updated_at: String,
    pub pinned: bool,
    pub archived: bool,
    pub trashed_at: Option<String>,
    pub folder_id: Option<String>,
    pub sort_order: i64,
    pub tags: Vec<Tag>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    #[serde(flatten)]
    pub summary: NoteSummary,
    pub content: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveNoteInput {
    pub id: String,
    pub title: String,
    pub content: Value,
    pub plain_text: String,
    pub pinned: bool,
    pub archived: bool,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AppSettings {
    pub hotkey: String,
    pub start_on_boot: bool,
    pub theme: String,
    pub default_font_family: String,
    pub default_font_size: u16,
    pub sidebar_collapsed: bool,
    #[serde(alias = "alwaysOnTop")]
    pub default_note_always_on_top: bool,
    #[serde(alias = "defaultNoteOpacity")]
    pub default_note_background_opacity: u8,
    pub default_note_text_opacity: u8,
    pub restore_note_windows_on_boot: bool,
    pub check_updates_on_startup: bool,
    pub window_layout: String,
    pub hotkey_window_scope: String,
    pub hotkey_action: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            hotkey: "Ctrl+Alt+N".into(),
            start_on_boot: false,
            theme: "system".into(),
            default_font_family: "Microsoft YaHei UI".into(),
            default_font_size: 16,
            sidebar_collapsed: false,
            default_note_always_on_top: false,
            default_note_background_opacity: 100,
            default_note_text_opacity: 100,
            restore_note_windows_on_boot: true,
            check_updates_on_startup: true,
            window_layout: "integrated".into(),
            hotkey_window_scope: "editor".into(),
            hotkey_action: "last-note".into(),
        }
    }
}

impl AppSettings {
    pub fn migrate_legacy(mut self) -> Self {
        if self.hotkey_action == "hub" {
            self.hotkey_action = "last-note".into();
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::AppSettings;

    #[test]
    fn hotkey_defaults_to_the_last_note() {
        assert_eq!(AppSettings::default().hotkey_action, "last-note");
        assert!(AppSettings::default().check_updates_on_startup);
    }

    #[test]
    fn legacy_hub_hotkey_is_migrated() {
        let mut settings = AppSettings::default();
        settings.hotkey_action = "hub".into();
        assert_eq!(settings.migrate_legacy().hotkey_action, "last-note");
    }

    #[test]
    fn legacy_opacity_becomes_background_only() {
        let settings: AppSettings = serde_json::from_str(r#"{"defaultNoteOpacity":70}"#).unwrap();
        assert_eq!(settings.default_note_background_opacity, 70);
        assert_eq!(settings.default_note_text_opacity, 100);
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceState {
    #[serde(default)]
    pub open_note_ids: Vec<String>,
    pub active_note_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteWindowState {
    pub note_id: String,
    pub mode: String,
    pub visible: bool,
    pub always_on_top: bool,
    pub background_opacity: u8,
    pub text_opacity: u8,
    pub locked: bool,
    pub dock_edge: Option<String>,
    pub dock_offset: f64,
    pub monitor_key: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteWindowGeometry {
    pub note_id: String,
    pub mode: String,
    pub x_ratio: f64,
    pub y_ratio: f64,
    pub width_dip: f64,
    pub height_dip: f64,
    pub work_area_x: i32,
    pub work_area_y: i32,
    pub work_area_width: u32,
    pub work_area_height: u32,
    pub scale_factor: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentResult {
    pub id: String,
    pub asset_path: String,
    pub file_path: String,
    pub mime: String,
    pub width: u32,
    pub height: u32,
    pub size_bytes: u64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupManifest {
    pub format: String,
    pub schema_version: u32,
    pub app_version: String,
    pub created_at: String,
    #[serde(default)]
    pub files: BTreeMap<String, String>,
}
