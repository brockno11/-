use std::fs;

use rusqlite::{params, OptionalExtension};
use tauri::{
    window::Color, AppHandle, Emitter, Manager, PhysicalPosition, State, WebviewUrl,
    WebviewWindow, WebviewWindowBuilder, WindowEvent,
};

use crate::{model::WorkspaceState, AppError, AppState};

pub const HUB_LABEL: &str = "hub";
pub const EDITOR_LABEL: &str = "editor";
const EDITOR_WIDTH: f64 = 844.0;
const EDITOR_HEIGHT: f64 = 644.0;
const WINDOW_GAP: f64 = 12.0;
const WEBVIEW_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection \
--disable-gpu --disable-gpu-compositing --disable-crash-reporter --disable-background-networking \
--disable-component-update --disable-default-apps --disable-sync --no-first-run --renderer-process-limit=1";
#[cfg(target_os = "windows")]
fn update_window_appearance(window: &WebviewWindow) {
    use std::{ffi::c_void, mem::size_of};
    use windows::Win32::{
        Foundation::HWND,
        Graphics::Dwm::{
            DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_COLOR_NONE,
            DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND,
        },
    };

    let Ok(raw_hwnd) = window.hwnd() else {
        return;
    };
    let hwnd = HWND(raw_hwnd.0);
    // The outer native window stays square and transparent so it does not clip
    // the CSS shadow. The visible content keeps its rounded CSS corners.
    let corner_preference = DWMWCP_DONOTROUND;
    let border_color = DWMWA_COLOR_NONE;

    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            (&corner_preference as *const _) as *const c_void,
            size_of_val(&corner_preference) as u32,
        );
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_BORDER_COLOR,
            (&border_color as *const _) as *const c_void,
            size_of::<u32>() as u32,
        );
    }
}

#[cfg(not(target_os = "windows"))]
fn update_window_appearance(_window: &WebviewWindow) {}

pub fn install_window_appearance(window: &WebviewWindow) {
    let _ = window.set_background_color(Some(Color(0, 0, 0, 0)));
    update_window_appearance(window);
    let watched_window = window.clone();
    window.on_window_event(move |event| {
        if matches!(
            event,
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. }
        ) {
            update_window_appearance(&watched_window);
        }
    });
}

pub fn position_hub_on_first_launch(hub: &WebviewWindow, data_dir: &std::path::Path) {
    let marker = data_dir.join(".hub-positioned-v3");
    if marker.exists() {
        return;
    }
    let positioned = (|| {
        let monitor = hub.current_monitor().ok()??;
        let scale = monitor.scale_factor();
        let monitor_position = monitor.position();
        let monitor_size = monitor.size();
        let size = hub.outer_size().ok()?;
        let margin = (20.0 * scale).round() as i32;
        let x = monitor_position.x + monitor_size.width as i32 - size.width as i32 - margin;
        let y = monitor_position.y
            + ((monitor_size.height as i32 - size.height as i32) / 2).max(margin);
        hub.set_position(PhysicalPosition::new(x, y)).ok()?;
        Some(())
    })()
    .is_some();
    if positioned {
        let _ = fs::write(marker, b"positioned");
    }
}

fn show_window(window: &WebviewWindow, focus: bool) -> Result<(), AppError> {
    window.show().map_err(|error| AppError::Other(error.to_string()))?;
    window
        .unminimize()
        .map_err(|error| AppError::Other(error.to_string()))?;
    if focus {
        window
            .set_focus()
            .map_err(|error| AppError::Other(error.to_string()))?;
    }
    Ok(())
}

fn editor_position(hub: &WebviewWindow) -> Option<(f64, f64)> {
    let scale = hub.scale_factor().ok()?;
    let hub_position = hub.outer_position().ok()?;
    let monitor = hub.current_monitor().ok()??;
    let monitor_position = monitor.position();
    let monitor_size = monitor.size();
    let editor_width = EDITOR_WIDTH * scale;
    let editor_height = EDITOR_HEIGHT * scale;
    let gap = WINDOW_GAP * scale;
    let margin = 20.0 * scale;
    let monitor_left = f64::from(monitor_position.x);
    let monitor_top = f64::from(monitor_position.y);
    let monitor_right = monitor_left + f64::from(monitor_size.width);
    let monitor_bottom = monitor_top + f64::from(monitor_size.height);
    let hub_left = f64::from(hub_position.x);
    let left = hub_left - editor_width - gap;
    let x = if left >= monitor_left + margin {
        left
    } else {
        (hub_left + gap).min(monitor_right - editor_width - margin)
    };
    let y = f64::from(hub_position.y)
        .max(monitor_top + margin)
        .min((monitor_bottom - editor_height - margin).max(monitor_top + margin));
    Some((x / scale, y / scale))
}

fn ensure_editor(app: &AppHandle, always_on_top: bool) -> Result<WebviewWindow, AppError> {
    if let Some(window) = app.get_webview_window(EDITOR_LABEL) {
        window
            .set_always_on_top(always_on_top)
            .map_err(|error| AppError::Other(error.to_string()))?;
        show_window(&window, true)?;
        return Ok(window);
    }
    let hub = app
        .get_webview_window(HUB_LABEL)
        .ok_or_else(|| AppError::Other("找不到核心工作台".into()))?;
    let mut builder = WebviewWindowBuilder::new(
        app,
        EDITOR_LABEL,
        WebviewUrl::App("index.html".into()),
    )
    .title("轻笺编辑器")
    .inner_size(EDITOR_WIDTH, EDITOR_HEIGHT)
    .min_inner_size(584.0, 444.0)
    .decorations(false)
    .resizable(true)
    .transparent(true)
    .shadow(false)
    .background_color(Color(0, 0, 0, 0))
    .always_on_top(always_on_top)
    .visible(false)
    .additional_browser_args(WEBVIEW_ARGS);
    if let Some((x, y)) = editor_position(&hub) {
        builder = builder.position(x, y);
    } else {
        builder = builder.center();
    }
    let window = builder
        .build()
        .map_err(|error| AppError::Other(format!("创建编辑器窗口失败：{error}")))?;
    install_window_appearance(&window);
    show_window(&window, true)?;
    Ok(window)
}

fn read_workspace(state: &AppState) -> Result<WorkspaceState, AppError> {
    let db = state
        .db
        .lock()
        .map_err(|_| AppError::Other("数据库连接不可用".into()))?;
    let json: Option<String> = db
        .query_row("SELECT json FROM workspace_state WHERE id=1", [], |row| row.get(0))
        .optional()?;
    Ok(json
        .and_then(|value| serde_json::from_str(&value).ok())
        .unwrap_or_default())
}

fn write_workspace(state: &AppState, workspace: &WorkspaceState) -> Result<(), AppError> {
    let db = state
        .db
        .lock()
        .map_err(|_| AppError::Other("数据库连接不可用".into()))?;
    db.execute(
        "INSERT INTO workspace_state(id,json) VALUES(1,?1)
         ON CONFLICT(id) DO UPDATE SET json=excluded.json",
        params![serde_json::to_string(workspace)?],
    )?;
    Ok(())
}

#[tauri::command]
pub async fn show_hub_window(app: AppHandle) -> Result<(), AppError> {
    let hub = app
        .get_webview_window(HUB_LABEL)
        .ok_or_else(|| AppError::Other("找不到核心工作台".into()))?;
    show_window(&hub, true)
}

#[tauri::command]
pub async fn toggle_hub_window(app: AppHandle) -> Result<bool, AppError> {
    let hub = app
        .get_webview_window(HUB_LABEL)
        .ok_or_else(|| AppError::Other("找不到核心工作台".into()))?;
    let visible = hub.is_visible().map_err(|error| AppError::Other(error.to_string()))?;
    let focused = hub.is_focused().map_err(|error| AppError::Other(error.to_string()))?;
    if visible && focused {
        hub.hide().map_err(|error| AppError::Other(error.to_string()))?;
        return Ok(false);
    }
    show_window(&hub, true)?;
    Ok(true)
}

#[tauri::command]
pub async fn open_note_in_editor(
    app: AppHandle,
    state: State<'_, AppState>,
    note_id: String,
    always_on_top: bool,
) -> Result<(), AppError> {
    {
        let db = state
            .db
            .lock()
            .map_err(|_| AppError::Other("数据库连接不可用".into()))?;
        let exists: bool = db.query_row(
            "SELECT EXISTS(SELECT 1 FROM notes WHERE id=?1 AND trashed_at IS NULL)",
            [&note_id],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(AppError::Invalid("便签不存在".into()));
        }
    }
    let mut workspace = read_workspace(&state)?;
    if !workspace.open_note_ids.contains(&note_id) {
        workspace.open_note_ids.push(note_id.clone());
    }
    workspace.active_note_id = Some(note_id.clone());
    write_workspace(&state, &workspace)?;
    ensure_editor(&app, always_on_top)?;
    let _ = app.emit_to(EDITOR_LABEL, "qingjian://open-note", note_id);
    Ok(())
}

#[tauri::command]
pub async fn close_editor_window(app: AppHandle) -> Result<(), AppError> {
    if let Some(editor) = app.get_webview_window(EDITOR_LABEL) {
        editor
            .destroy()
            .map_err(|error| AppError::Other(error.to_string()))?;
    }
    Ok(())
}

pub fn show_hub(app: &AppHandle) {
    if let Some(hub) = app.get_webview_window(HUB_LABEL) {
        let _ = show_window(&hub, true);
    }
}
