use std::fs;

use chrono::Utc;
use rusqlite::{params, OptionalExtension};
use tauri::{
    window::Color, AppHandle, Manager, LogicalSize, PhysicalPosition, PhysicalSize, State,
    Emitter, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent,
};

use crate::{
    model::{AppSettings, NoteWindowGeometry, NoteWindowState},
    AppError, AppState,
};

pub const HUB_LABEL: &str = "hub";
const NOTE_PREFIX: &str = "note-";
const DOCK_DISTANCE_DIP: f64 = 12.0;
const DOCK_STRIP_DIP: f64 = 12.0;
const MIN_VISIBLE_DIP: f64 = 32.0;
const WEBVIEW_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection \
--disable-gpu --disable-gpu-compositing --disable-crash-reporter --disable-background-networking \
--disable-component-update --disable-default-apps --disable-sync --no-first-run --renderer-process-limit=1";

fn mode_spec(mode: &str) -> Result<(f64, f64, f64, f64, Option<(f64, f64)>), AppError> {
    match mode {
        "capsule" => Ok((336.0, 76.0, 260.0, 72.0, Some((560.0, 104.0)))),
        "card" => Ok((360.0, 280.0, 280.0, 160.0, None)),
        "editor" => Ok((844.0, 644.0, 584.0, 444.0, None)),
        _ => Err(AppError::Invalid("便签窗口形态无效".into())),
    }
}

fn validate_note_id(note_id: &str) -> Result<(), AppError> {
    uuid::Uuid::parse_str(note_id)
        .map(|_| ())
        .map_err(|_| AppError::Invalid("便签编号无效".into()))
}

fn validated_opacity(layer: &str, opacity: u8) -> Result<(&'static str, u8), AppError> {
    match layer {
        "background" => Ok(("opacity", opacity.min(100))),
        "text" => Ok(("text_opacity", opacity.clamp(20, 100))),
        _ => Err(AppError::Invalid("透明度层类型无效".into())),
    }
}

fn note_label(note_id: &str) -> String {
    format!("{NOTE_PREFIX}{note_id}")
}

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
    let Ok(raw_hwnd) = window.hwnd() else { return };
    let hwnd = HWND(raw_hwnd.0);
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
        if matches!(event, WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. }) {
            update_window_appearance(&watched_window);
        }
    });
}

pub fn position_hub_on_first_launch(hub: &WebviewWindow, data_dir: &std::path::Path) {
    let marker = data_dir.join(".hub-positioned-v3");
    if marker.exists() { return }
    let positioned = (|| {
        let monitor = hub.current_monitor().ok()??;
        let scale = monitor.scale_factor();
        let work = monitor.work_area();
        let size = hub.outer_size().ok()?;
        let margin = (20.0 * scale).round() as i32;
        let x = work.position.x + work.size.width as i32 - size.width as i32 - margin;
        let y = work.position.y + ((work.size.height as i32 - size.height as i32) / 2).max(margin);
        hub.set_position(PhysicalPosition::new(x, y)).ok()?;
        Some(())
    })().is_some();
    if positioned { let _ = fs::write(marker, b"positioned"); }
}

fn show_window(window: &WebviewWindow, focus: bool) -> Result<(), AppError> {
    window.show().map_err(|error| AppError::Other(error.to_string()))?;
    window.unminimize().map_err(|error| AppError::Other(error.to_string()))?;
    restore_native_window(window, focus);
    if focus { window.set_focus().map_err(|error| AppError::Other(error.to_string()))?; }
    Ok(())
}

#[cfg(target_os = "windows")]
fn restore_native_window(window: &WebviewWindow, focus: bool) {
    use windows::Win32::{
        Foundation::HWND,
        UI::WindowsAndMessaging::{SetForegroundWindow, ShowWindow, SW_RESTORE},
    };
    let Ok(raw_hwnd) = window.hwnd() else { return };
    let hwnd = HWND(raw_hwnd.0);
    unsafe {
        let _ = ShowWindow(hwnd, SW_RESTORE);
        if focus { let _ = SetForegroundWindow(hwnd); }
    }
}

#[cfg(not(target_os = "windows"))]
fn restore_native_window(_window: &WebviewWindow, _focus: bool) {}

fn read_settings(state: &AppState) -> Result<AppSettings, AppError> {
    let db = state.db.lock().map_err(|_| AppError::Other("数据库连接不可用".into()))?;
    let json: Option<String> = db.query_row("SELECT json FROM settings WHERE id=1", [], |row| row.get(0)).optional()?;
    Ok(json.and_then(|value| serde_json::from_str::<AppSettings>(&value).ok()).unwrap_or_default().migrate_legacy())
}

fn read_window_state(state: &AppState, note_id: &str) -> Result<Option<NoteWindowState>, AppError> {
    let db = state.db.lock().map_err(|_| AppError::Other("数据库连接不可用".into()))?;
    db.query_row(
        "SELECT note_id,mode,visible,always_on_top,opacity,text_opacity,locked,dock_edge,dock_offset,monitor_key,updated_at
         FROM note_window_state WHERE note_id=?1",
        [note_id],
        |row| Ok(NoteWindowState {
            note_id: row.get(0)?, mode: row.get(1)?, visible: row.get(2)?,
            always_on_top: row.get(3)?, background_opacity: row.get::<_, i64>(4)? as u8,
            text_opacity: row.get::<_, i64>(5)? as u8, locked: row.get(6)?,
            dock_edge: row.get(7)?, dock_offset: row.get(8)?, monitor_key: row.get(9)?,
            updated_at: row.get(10)?,
        }),
    ).optional().map_err(Into::into)
}

fn ensure_window_state(state: &AppState, note_id: &str, preferred_mode: Option<&str>) -> Result<NoteWindowState, AppError> {
    validate_note_id(note_id)?;
    if let Some(mode) = preferred_mode { mode_spec(mode)?; }
    let settings = read_settings(state)?;
    let db = state.db.lock().map_err(|_| AppError::Other("数据库连接不可用".into()))?;
    let exists: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM notes WHERE id=?1 AND archived=0 AND trashed_at IS NULL)",
        [note_id], |row| row.get(0),
    )?;
    if !exists { return Err(AppError::Invalid("便签不存在、已归档或已删除".into())); }
    let mode = preferred_mode.unwrap_or("card");
    db.execute(
        "INSERT INTO note_window_state(
            note_id,mode,visible,always_on_top,opacity,text_opacity,locked,dock_offset,updated_at
         ) VALUES(?1,?2,1,?3,?4,?5,0,0,?6)
         ON CONFLICT(note_id) DO UPDATE SET
            mode=CASE WHEN ?7 THEN excluded.mode ELSE note_window_state.mode END,
            visible=1,updated_at=excluded.updated_at",
        params![note_id, mode, settings.default_note_always_on_top as i32,
            settings.default_note_background_opacity.min(100),
            settings.default_note_text_opacity.clamp(20, 100), Utc::now().to_rfc3339(),
            preferred_mode.is_some()],
    )?;
    drop(db);
    read_window_state(state, note_id)?.ok_or_else(|| AppError::Other("无法建立便签窗口状态".into()))
}

fn read_geometry(state: &AppState, note_id: &str, mode: &str) -> Result<Option<NoteWindowGeometry>, AppError> {
    let db = state.db.lock().map_err(|_| AppError::Other("数据库连接不可用".into()))?;
    db.query_row(
        "SELECT note_id,mode,x_ratio,y_ratio,width_dip,height_dip,work_area_x,work_area_y,
                work_area_width,work_area_height,scale_factor
         FROM note_window_geometry WHERE note_id=?1 AND mode=?2",
        params![note_id, mode],
        |row| Ok(NoteWindowGeometry {
            note_id: row.get(0)?, mode: row.get(1)?, x_ratio: row.get(2)?, y_ratio: row.get(3)?,
            width_dip: row.get(4)?, height_dip: row.get(5)?, work_area_x: row.get(6)?,
            work_area_y: row.get(7)?, work_area_width: row.get(8)?, work_area_height: row.get(9)?,
            scale_factor: row.get(10)?,
        }),
    ).optional().map_err(Into::into)
}

fn monitor_key(monitor: &tauri::Monitor) -> String {
    let work = monitor.work_area();
    format!("{}|{}|{}|{}|{}", monitor.name().map(String::as_str).unwrap_or("monitor"),
        work.position.x, work.position.y, work.size.width, work.size.height)
}

fn select_monitor(window: &WebviewWindow, key: Option<&str>) -> Option<tauri::Monitor> {
    let monitors = window.available_monitors().ok()?;
    if let Some(key) = key {
        let wanted_name = key.split('|').next().unwrap_or_default();
        if let Some(found) = monitors.iter().find(|monitor| monitor_key(monitor) == key) { return Some(found.clone()); }
        if let Some(found) = monitors.iter().find(|monitor| monitor.name().map(String::as_str) == Some(wanted_name)) { return Some(found.clone()); }
    }
    window.current_monitor().ok().flatten()
        .or_else(|| window.primary_monitor().ok().flatten())
        .or_else(|| monitors.into_iter().next())
}

fn apply_mode_constraints(window: &WebviewWindow, mode: &str) -> Result<(), AppError> {
    let (_, _, min_w, min_h, max) = mode_spec(mode)?;
    window.set_min_size(None::<LogicalSize<f64>>).map_err(|error| AppError::Other(error.to_string()))?;
    window.set_max_size(None::<LogicalSize<f64>>).map_err(|error| AppError::Other(error.to_string()))?;
    window.set_min_size(Some(LogicalSize::new(min_w, min_h))).map_err(|error| AppError::Other(error.to_string()))?;
    if let Some((max_w, max_h)) = max {
        window.set_max_size(Some(LogicalSize::new(max_w, max_h))).map_err(|error| AppError::Other(error.to_string()))?;
    }
    Ok(())
}

fn restore_geometry(window: &WebviewWindow, state: &AppState, note_state: &NoteWindowState) -> Result<(), AppError> {
    let (default_w, default_h, min_w, min_h, max) = mode_spec(&note_state.mode)?;
    let geometry = read_geometry(state, &note_state.note_id, &note_state.mode)?;
    let monitor = select_monitor(window, note_state.monitor_key.as_deref())
        .ok_or_else(|| AppError::Other("找不到可用显示器".into()))?;
    let scale = monitor.scale_factor();
    let work = monitor.work_area();
    let (mut width, mut height) = geometry.as_ref()
        .map(|value| (value.width_dip, value.height_dip)).unwrap_or((default_w, default_h));
    width = width.max(min_w); height = height.max(min_h);
    if let Some((max_w, max_h)) = max { width = width.min(max_w); height = height.min(max_h); }
    let physical_w = width * scale;
    let physical_h = height * scale;
    let x_ratio = geometry.as_ref().map(|value| value.x_ratio.clamp(0.0, 1.0)).unwrap_or(0.5);
    let y_ratio = geometry.as_ref().map(|value| value.y_ratio.clamp(0.0, 1.0)).unwrap_or(0.35);
    let x_span = (work.size.width as f64 - physical_w).max(0.0);
    let y_span = (work.size.height as f64 - physical_h).max(0.0);
    let mut x = work.position.x as f64 + x_span * x_ratio;
    let mut y = work.position.y as f64 + y_span * y_ratio;
    let min_visible = MIN_VISIBLE_DIP * scale;
    x = x.max(work.position.x as f64 - physical_w + min_visible)
        .min(work.position.x as f64 + work.size.width as f64 - min_visible);
    y = y.max(work.position.y as f64 - physical_h + min_visible)
        .min(work.position.y as f64 + work.size.height as f64 - min_visible);
    window.set_size(LogicalSize::new(width, height)).map_err(|error| AppError::Other(error.to_string()))?;
    window.set_position(PhysicalPosition::new(x.round() as i32, y.round() as i32)).map_err(|error| AppError::Other(error.to_string()))?;
    apply_mode_constraints(window, &note_state.mode)
}

fn save_geometry(window: &WebviewWindow, state: &AppState, note_id: &str, mode: &str) -> Result<(), AppError> {
    let monitor = window.current_monitor().map_err(|error| AppError::Other(error.to_string()))?
        .or_else(|| window.primary_monitor().ok().flatten())
        .ok_or_else(|| AppError::Other("找不到当前显示器".into()))?;
    let scale = monitor.scale_factor();
    let work = monitor.work_area();
    let position = window.outer_position().map_err(|error| AppError::Other(error.to_string()))?;
    let size = window.inner_size().map_err(|error| AppError::Other(error.to_string()))?;
    if size.width as f64 <= DOCK_STRIP_DIP * scale + 2.0 || size.height as f64 <= DOCK_STRIP_DIP * scale + 2.0 { return Ok(()); }
    let x_span = (work.size.width as f64 - size.width as f64).max(1.0);
    let y_span = (work.size.height as f64 - size.height as f64).max(1.0);
    let x_ratio = ((position.x - work.position.x) as f64 / x_span).clamp(0.0, 1.0);
    let y_ratio = ((position.y - work.position.y) as f64 / y_span).clamp(0.0, 1.0);
    let key = monitor_key(&monitor);
    let db = state.db.lock().map_err(|_| AppError::Other("数据库连接不可用".into()))?;
    db.execute(
        "INSERT INTO note_window_geometry(
            note_id,mode,x_ratio,y_ratio,width_dip,height_dip,work_area_x,work_area_y,
            work_area_width,work_area_height,scale_factor
         ) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)
         ON CONFLICT(note_id,mode) DO UPDATE SET
            x_ratio=excluded.x_ratio,y_ratio=excluded.y_ratio,width_dip=excluded.width_dip,
            height_dip=excluded.height_dip,work_area_x=excluded.work_area_x,
            work_area_y=excluded.work_area_y,work_area_width=excluded.work_area_width,
            work_area_height=excluded.work_area_height,scale_factor=excluded.scale_factor",
        params![note_id, mode, x_ratio, y_ratio, size.width as f64 / scale, size.height as f64 / scale,
            work.position.x, work.position.y, work.size.width, work.size.height, scale],
    )?;
    db.execute("UPDATE note_window_state SET monitor_key=?2,updated_at=?3 WHERE note_id=?1",
        params![note_id, key, Utc::now().to_rfc3339()])?;
    Ok(())
}

fn build_note_window(app: &AppHandle, state: &AppState, note_state: &NoteWindowState) -> Result<WebviewWindow, AppError> {
    let label = note_label(&note_state.note_id);
    if let Some(window) = app.get_webview_window(&label) { return Ok(window); }
    let (width, height, min_w, min_h, max) = mode_spec(&note_state.mode)?;
    let url = format!("index.html?noteId={}&window={label}", note_state.note_id);
    let mut builder = WebviewWindowBuilder::new(app, &label, WebviewUrl::App(url.into()))
        .title("轻笺")
        .inner_size(width, height)
        .min_inner_size(min_w, min_h)
        .decorations(false).resizable(true).transparent(true).shadow(false)
        .background_color(Color(0, 0, 0, 0)).always_on_top(note_state.always_on_top)
        .visible(false).additional_browser_args(WEBVIEW_ARGS).center();
    if let Some((max_w, max_h)) = max { builder = builder.max_inner_size(max_w, max_h); }
    let window = builder.build().map_err(|error| AppError::Other(format!("创建便签窗口失败：{error}")))?;
    install_window_appearance(&window);
    restore_geometry(&window, state, note_state)?;
    let watched = window.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let _ = watched.emit("qingjian://hide-requested", ());
        }
    });
    Ok(window)
}

fn set_lock_runtime(window: &WebviewWindow, locked: bool) -> Result<(), AppError> {
    window.set_ignore_cursor_events(locked).map_err(|error| AppError::Other(error.to_string()))?;
    window.set_focusable(!locked).map_err(|error| AppError::Other(error.to_string()))?;
    Ok(())
}

fn mark_visible(state: &AppState, note_id: &str, visible: bool) -> Result<(), AppError> {
    let db = state.db.lock().map_err(|_| AppError::Other("数据库连接不可用".into()))?;
    db.execute("UPDATE note_window_state SET visible=?2,updated_at=?3 WHERE note_id=?1",
        params![note_id, visible as i32, Utc::now().to_rfc3339()])?;
    Ok(())
}

#[tauri::command]
pub async fn show_hub_window(app: AppHandle) -> Result<(), AppError> {
    let hub = ensure_hub_window(&app)?;
    show_window(&hub, true)
}

#[tauri::command]
pub async fn toggle_hub_window(app: AppHandle) -> Result<bool, AppError> {
    let hub = ensure_hub_window(&app)?;
    let visible = hub.is_visible().map_err(|error| AppError::Other(error.to_string()))?;
    let focused = hub.is_focused().map_err(|error| AppError::Other(error.to_string()))?;
    if visible && focused { hub.hide().map_err(|error| AppError::Other(error.to_string()))?; return Ok(false); }
    show_window(&hub, true)?;
    Ok(true)
}

#[tauri::command]
pub async fn open_note_window(
    app: AppHandle,
    state: State<'_, AppState>,
    note_id: String,
    preferred_mode: Option<String>,
) -> Result<NoteWindowState, AppError> {
    let note_state = ensure_window_state(&state, &note_id, preferred_mode.as_deref())?;
    let already_open = app.get_webview_window(&note_label(&note_id)).is_some();
    let window = build_note_window(&app, &state, &note_state)?;
    if preferred_mode.is_some() { restore_geometry(&window, &state, &note_state)?; }
    set_lock_runtime(&window, note_state.locked)?;
    if already_open {
        let _ = window.emit("qingjian://window-state-changed", note_state.clone());
        show_window(&window, !note_state.locked)?;
    }
    let _ = app.emit("qingjian://note-windows-changed", ());
    Ok(note_state)
}

#[tauri::command]
pub async fn note_window_ready(
    app: AppHandle,
    state: State<'_, AppState>,
    note_id: String,
) -> Result<NoteWindowState, AppError> {
    let note_state = read_window_state(&state, &note_id)?.ok_or_else(|| AppError::Invalid("便签窗口状态不存在".into()))?;
    let window = app.get_webview_window(&note_label(&note_id)).ok_or_else(|| AppError::Other("便签窗口尚未创建".into()))?;
    set_lock_runtime(&window, note_state.locked)?;
    if note_state.visible { show_window(&window, !note_state.locked)?; }
    Ok(note_state)
}

#[tauri::command]
pub fn get_note_window_state(state: State<'_, AppState>, note_id: String) -> Result<NoteWindowState, AppError> {
    read_window_state(&state, &note_id)?.ok_or_else(|| AppError::Invalid("便签窗口状态不存在".into()))
}

#[tauri::command]
pub fn list_visible_note_windows(state: State<'_, AppState>) -> Result<Vec<NoteWindowState>, AppError> {
    let db = state.db.lock().map_err(|_| AppError::Other("数据库连接不可用".into()))?;
    let mut statement = db.prepare(
        "SELECT note_id,mode,visible,always_on_top,opacity,text_opacity,locked,dock_edge,dock_offset,monitor_key,updated_at
         FROM note_window_state WHERE visible=1 ORDER BY updated_at DESC")?;
    let rows = statement.query_map([], |row| Ok(NoteWindowState {
        note_id: row.get(0)?, mode: row.get(1)?, visible: row.get(2)?, always_on_top: row.get(3)?,
        background_opacity: row.get::<_, i64>(4)? as u8,
        text_opacity: row.get::<_, i64>(5)? as u8, locked: row.get(6)?, dock_edge: row.get(7)?,
        dock_offset: row.get(8)?, monitor_key: row.get(9)?, updated_at: row.get(10)?,
    }))?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

#[tauri::command]
pub fn save_note_window_geometry(app: AppHandle, state: State<'_, AppState>, note_id: String) -> Result<(), AppError> {
    let note_state = read_window_state(&state, &note_id)?.ok_or_else(|| AppError::Invalid("便签窗口状态不存在".into()))?;
    let window = app.get_webview_window(&note_label(&note_id)).ok_or_else(|| AppError::Other("便签窗口尚未创建".into()))?;
    save_geometry(&window, &state, &note_id, &note_state.mode)
}

#[tauri::command]
pub fn set_note_window_mode(
    app: AppHandle, state: State<'_, AppState>, note_id: String, mode: String,
) -> Result<NoteWindowState, AppError> {
    mode_spec(&mode)?;
    let mut current = read_window_state(&state, &note_id)?.ok_or_else(|| AppError::Invalid("便签窗口状态不存在".into()))?;
    let window = app.get_webview_window(&note_label(&note_id)).ok_or_else(|| AppError::Other("便签窗口尚未创建".into()))?;
    save_geometry(&window, &state, &note_id, &current.mode)?;
    let old_position = window.outer_position().map_err(|error| AppError::Other(error.to_string()))?;
    let old_size = window.outer_size().map_err(|error| AppError::Other(error.to_string()))?;
    let center = (old_position.x as f64 + old_size.width as f64 / 2.0, old_position.y as f64 + old_size.height as f64 / 2.0);
    let db = state.db.lock().map_err(|_| AppError::Other("数据库连接不可用".into()))?;
    db.execute("UPDATE note_window_state SET mode=?2,dock_edge=NULL,updated_at=?3 WHERE note_id=?1",
        params![note_id, mode, Utc::now().to_rfc3339()])?;
    drop(db);
    current.mode = mode.clone(); current.dock_edge = None;
    restore_geometry(&window, &state, &current)?;
    let size = window.outer_size().map_err(|error| AppError::Other(error.to_string()))?;
    let monitor = window.monitor_from_point(center.0, center.1).ok().flatten()
        .or_else(|| window.current_monitor().ok().flatten());
    if let Some(monitor) = monitor {
        let work = monitor.work_area();
        let x = (center.0 - size.width as f64 / 2.0)
            .max(work.position.x as f64).min((work.position.x + work.size.width as i32 - size.width as i32) as f64);
        let y = (center.1 - size.height as f64 / 2.0)
            .max(work.position.y as f64).min((work.position.y + work.size.height as i32 - size.height as i32) as f64);
        window.set_position(PhysicalPosition::new(x.round() as i32, y.round() as i32))
            .map_err(|error| AppError::Other(error.to_string()))?;
    }
    Ok(current)
}

#[tauri::command]
pub fn set_note_window_always_on_top(
    app: AppHandle, state: State<'_, AppState>, note_id: String, always_on_top: bool,
) -> Result<NoteWindowState, AppError> {
    let window = app.get_webview_window(&note_label(&note_id)).ok_or_else(|| AppError::Other("便签窗口尚未创建".into()))?;
    window.set_always_on_top(always_on_top).map_err(|error| AppError::Other(error.to_string()))?;
    let db = state.db.lock().map_err(|_| AppError::Other("数据库连接不可用".into()))?;
    db.execute("UPDATE note_window_state SET always_on_top=?2,updated_at=?3 WHERE note_id=?1",
        params![note_id, always_on_top as i32, Utc::now().to_rfc3339()])?;
    drop(db);
    get_note_window_state(state, note_id)
}

#[tauri::command]
pub fn set_note_window_opacity(
    app: AppHandle, state: State<'_, AppState>, note_id: String, layer: String, opacity: u8,
) -> Result<NoteWindowState, AppError> {
    let (column, opacity) = validated_opacity(&layer, opacity)?;
    let db = state.db.lock().map_err(|_| AppError::Other("数据库连接不可用".into()))?;
    db.execute(&format!("UPDATE note_window_state SET {column}=?2,updated_at=?3 WHERE note_id=?1"),
        params![note_id, opacity, Utc::now().to_rfc3339()])?;
    drop(db);
    let result = get_note_window_state(state, note_id.clone())?;
    if let Some(window) = app.get_webview_window(&note_label(&note_id)) {
        let _ = window.emit("qingjian://window-state-changed", result.clone());
    }
    Ok(result)
}

#[tauri::command]
pub fn set_note_window_locked(
    app: AppHandle, state: State<'_, AppState>, note_id: String, locked: bool,
) -> Result<NoteWindowState, AppError> {
    let window = app.get_webview_window(&note_label(&note_id)).ok_or_else(|| AppError::Other("便签窗口尚未创建".into()))?;
    set_lock_runtime(&window, locked)?;
    let db = state.db.lock().map_err(|_| AppError::Other("数据库连接不可用".into()))?;
    db.execute("UPDATE note_window_state SET locked=?2,updated_at=?3 WHERE note_id=?1",
        params![note_id, locked as i32, Utc::now().to_rfc3339()])?;
    drop(db);
    get_note_window_state(state, note_id)
}

#[tauri::command]
pub fn hide_note_window(app: AppHandle, state: State<'_, AppState>, note_id: String) -> Result<(), AppError> {
    if let Some(window) = app.get_webview_window(&note_label(&note_id)) {
        window.hide().map_err(|error| AppError::Other(error.to_string()))?;
    }
    mark_visible(&state, &note_id, false)?;
    let _ = app.emit("qingjian://note-windows-changed", ());
    Ok(())
}

pub fn hide_and_destroy_note_window(app: &AppHandle, note_id: &str) {
    if let Some(window) = app.get_webview_window(&note_label(note_id)) { let _ = window.destroy(); }
    let state = app.state::<AppState>();
    let _ = mark_visible(&state, note_id, false);
}

#[tauri::command]
pub fn finalize_note_window_move(app: AppHandle, state: State<'_, AppState>, note_id: String) -> Result<NoteWindowState, AppError> {
    let mut note_state = read_window_state(&state, &note_id)?.ok_or_else(|| AppError::Invalid("便签窗口状态不存在".into()))?;
    let window = app.get_webview_window(&note_label(&note_id)).ok_or_else(|| AppError::Other("便签窗口尚未创建".into()))?;
    save_geometry(&window, &state, &note_id, &note_state.mode)?;
    let monitor = window.current_monitor().map_err(|error| AppError::Other(error.to_string()))?
        .ok_or_else(|| AppError::Other("找不到当前显示器".into()))?;
    let scale = monitor.scale_factor();
    let threshold = if note_state.dock_edge.is_some() { 24.0 * scale } else { DOCK_DISTANCE_DIP * scale };
    let work = monitor.work_area();
    let pos = window.outer_position().map_err(|error| AppError::Other(error.to_string()))?;
    let size = window.outer_size().map_err(|error| AppError::Other(error.to_string()))?;
    let distances = [
        ("left", (pos.x - work.position.x).abs() as f64),
        ("right", (work.position.x + work.size.width as i32 - pos.x - size.width as i32).abs() as f64),
        ("top", (pos.y - work.position.y).abs() as f64),
        ("bottom", (work.position.y + work.size.height as i32 - pos.y - size.height as i32).abs() as f64),
    ];
    let edge = distances.iter().min_by(|a, b| a.1.total_cmp(&b.1))
        .filter(|(_, distance)| *distance <= threshold).map(|(edge, _)| (*edge).to_string());
    let mut x = pos.x; let mut y = pos.y;
    if let Some(edge) = edge.as_deref() {
        match edge {
            "left" => x = work.position.x,
            "right" => x = work.position.x + work.size.width as i32 - size.width as i32,
            "top" => y = work.position.y,
            "bottom" => y = work.position.y + work.size.height as i32 - size.height as i32,
            _ => {}
        }
        window.set_position(PhysicalPosition::new(x, y)).map_err(|error| AppError::Other(error.to_string()))?;
    }
    let offset = if matches!(edge.as_deref(), Some("left" | "right")) {
        (y - work.position.y) as f64 / scale
    } else { (x - work.position.x) as f64 / scale };
    let db = state.db.lock().map_err(|_| AppError::Other("数据库连接不可用".into()))?;
    db.execute("UPDATE note_window_state SET dock_edge=?2,dock_offset=?3,monitor_key=?4,updated_at=?5 WHERE note_id=?1",
        params![note_id, edge, offset.max(0.0), monitor_key(&monitor), Utc::now().to_rfc3339()])?;
    drop(db);
    note_state.dock_edge = edge; note_state.dock_offset = offset.max(0.0);
    Ok(note_state)
}

#[tauri::command]
pub fn collapse_docked_note_window(app: AppHandle, state: State<'_, AppState>, note_id: String) -> Result<(), AppError> {
    if !read_settings(&state)?.auto_hide_at_screen_edge { return Ok(()); }
    let note_state = read_window_state(&state, &note_id)?.ok_or_else(|| AppError::Invalid("便签窗口状态不存在".into()))?;
    let Some(edge) = note_state.dock_edge.as_deref() else { return Ok(()) };
    if note_state.locked { return Ok(()); }
    let window = app.get_webview_window(&note_label(&note_id)).ok_or_else(|| AppError::Other("便签窗口尚未创建".into()))?;
    save_geometry(&window, &state, &note_id, &note_state.mode)?;
    let monitor = select_monitor(&window, note_state.monitor_key.as_deref()).ok_or_else(|| AppError::Other("找不到停靠显示器".into()))?;
    let work = monitor.work_area(); let scale = monitor.scale_factor();
    let current = window.outer_size().map_err(|error| AppError::Other(error.to_string()))?;
    let strip = (DOCK_STRIP_DIP * scale).round() as u32;
    window.set_min_size(None::<LogicalSize<f64>>).map_err(|error| AppError::Other(error.to_string()))?;
    window.set_max_size(None::<LogicalSize<f64>>).map_err(|error| AppError::Other(error.to_string()))?;
    let (width, height, x, y) = match edge {
        "left" => (strip, current.height, work.position.x, work.position.y + (note_state.dock_offset * scale) as i32),
        "right" => (strip, current.height, work.position.x + work.size.width as i32 - strip as i32, work.position.y + (note_state.dock_offset * scale) as i32),
        "top" => (current.width, strip, work.position.x + (note_state.dock_offset * scale) as i32, work.position.y),
        "bottom" => (current.width, strip, work.position.x + (note_state.dock_offset * scale) as i32, work.position.y + work.size.height as i32 - strip as i32),
        _ => return Ok(()),
    };
    window.set_size(PhysicalSize::new(width, height)).map_err(|error| AppError::Other(error.to_string()))?;
    window.set_position(PhysicalPosition::new(x, y)).map_err(|error| AppError::Other(error.to_string()))?;
    Ok(())
}

#[tauri::command]
pub fn expand_docked_note_window(app: AppHandle, state: State<'_, AppState>, note_id: String) -> Result<(), AppError> {
    let note_state = read_window_state(&state, &note_id)?.ok_or_else(|| AppError::Invalid("便签窗口状态不存在".into()))?;
    if note_state.dock_edge.is_none() { return Ok(()); }
    let window = app.get_webview_window(&note_label(&note_id)).ok_or_else(|| AppError::Other("便签窗口尚未创建".into()))?;
    restore_geometry(&window, &state, &note_state)
}

pub fn show_all(app: &AppHandle) {
    let state = app.state::<AppState>();
    let ids = list_window_ids(&state, false, true).unwrap_or_default();
    for id in ids {
        if let Ok(note_state) = ensure_window_state(&state, &id, None) {
            let already_open = app.get_webview_window(&note_label(&id)).is_some();
            if let Ok(window) = build_note_window(app, &state, &note_state) {
                let _ = set_lock_runtime(&window, note_state.locked);
                if already_open { let _ = show_window(&window, false); }
            }
        }
    }
    let _ = app.emit("qingjian://note-windows-changed", ());
}

pub fn hide_all(app: &AppHandle) {
    let state = app.state::<AppState>();
    if let Ok(ids) = list_window_ids(&state, false, false) {
        for id in ids {
            if let Some(window) = app.get_webview_window(&note_label(&id)) { let _ = window.hide(); }
            let _ = mark_visible(&state, &id, false);
        }
    }
    let _ = app.emit("qingjian://note-windows-changed", ());
}

#[tauri::command]
pub fn toggle_all_note_windows(app: AppHandle) -> bool {
    let state = app.state::<AppState>();
    let has_visible_notes = list_window_ids(&state, false, false)
        .map(|ids| !ids.is_empty())
        .unwrap_or(false);
    if has_visible_notes {
        hide_all(&app);
        false
    } else {
        show_all(&app);
        true
    }
}

pub fn unlock_all(app: &AppHandle) {
    let state = app.state::<AppState>();
    if let Ok(ids) = list_window_ids(&state, true, false) {
        for id in ids {
            if let Some(window) = app.get_webview_window(&note_label(&id)) {
                let _ = set_lock_runtime(&window, false); let _ = show_window(&window, false);
            }
        }
        if let Ok(db) = state.db.lock() { let _ = db.execute("UPDATE note_window_state SET locked=0 WHERE locked=1", []); }
    }
    let _ = app.emit("qingjian://note-windows-changed", ());
}

#[tauri::command]
pub fn unlock_all_note_windows(app: AppHandle) {
    unlock_all(&app);
}

pub fn restore_visibility(app: &AppHandle) {
    let state = app.state::<AppState>();
    if let Ok(db) = state.db.lock() {
        let _ = db.execute(
            "UPDATE note_window_state SET opacity=85,text_opacity=100,updated_at=?1",
            [Utc::now().to_rfc3339()],
        );
    }
    if let Ok(ids) = list_window_ids(&state, false, false) {
        for id in ids {
            if let (Some(window), Ok(Some(note_state))) = (
                app.get_webview_window(&note_label(&id)), read_window_state(&state, &id),
            ) {
                let _ = window.emit("qingjian://window-state-changed", note_state);
            }
        }
    }
    let _ = app.emit("qingjian://note-windows-changed", ());
}

#[tauri::command]
pub fn restore_all_note_visibility(app: AppHandle) {
    restore_visibility(&app);
}

fn list_window_ids(state: &AppState, locked_only: bool, include_hidden: bool) -> Result<Vec<String>, AppError> {
    let db = state.db.lock().map_err(|_| AppError::Other("数据库连接不可用".into()))?;
    let sql = if locked_only {
        "SELECT note_id FROM note_window_state WHERE visible=1 AND locked=1"
    } else if include_hidden {
        "SELECT s.note_id FROM note_window_state s JOIN notes n ON n.id=s.note_id
         WHERE n.archived=0 AND n.trashed_at IS NULL"
    } else { "SELECT note_id FROM note_window_state WHERE visible=1" };
    let mut statement = db.prepare(sql)?;
    let ids = statement.query_map([], |row| row.get(0))?.collect::<Result<Vec<String>, _>>()?;
    Ok(ids)
}

pub fn restore_visible_note_windows(app: &AppHandle, on_boot: bool) {
    let state = app.state::<AppState>();
    if on_boot && read_settings(&state).map(|settings| !settings.restore_note_windows_on_boot).unwrap_or(false) { return; }
    let ids = list_window_ids(&state, false, false).unwrap_or_default();
    for id in ids {
        if let Ok(Some(note_state)) = read_window_state(&state, &id) {
            let _ = build_note_window(app, &state, &note_state);
        }
    }
}

fn ensure_hub_window(app: &AppHandle) -> Result<WebviewWindow, AppError> {
    if let Some(hub) = app.get_webview_window(HUB_LABEL) {
        if hub.hwnd().is_ok() { return Ok(hub) }
        let _ = hub.destroy();
    }
    let hub = WebviewWindowBuilder::new(app, HUB_LABEL, WebviewUrl::App("index.html".into()))
        .title("轻笺工作台")
        .inner_size(404.0, 704.0)
        .min_inner_size(344.0, 504.0)
        .decorations(false)
        .resizable(true)
        .transparent(true)
        .shadow(false)
        .background_color(Color(0, 0, 0, 0))
        .visible(false)
        .additional_browser_args(WEBVIEW_ARGS)
        .center()
        .build()
        .map_err(|error| AppError::Other(format!("重建工作台失败：{error}")))?;
    install_window_appearance(&hub);
    Ok(hub)
}

pub fn show_hub(app: &AppHandle) {
    if let Ok(hub) = ensure_hub_window(app) {
        if show_window(&hub, true).is_ok() { return }
        let _ = hub.destroy();
    }
    if let Ok(rebuilt) = ensure_hub_window(app) { let _ = show_window(&rebuilt, true); }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_modes_enforce_the_product_size_contract() {
        assert_eq!(mode_spec("capsule").unwrap(), (336.0, 76.0, 260.0, 72.0, Some((560.0, 104.0))));
        assert_eq!(mode_spec("card").unwrap(), (360.0, 280.0, 280.0, 160.0, None));
        assert_eq!(mode_spec("editor").unwrap(), (844.0, 644.0, 584.0, 444.0, None));
        assert!(mode_spec("invalid").is_err());
    }

    #[test]
    fn dynamic_note_labels_are_stable_and_unique_per_note() {
        let id = "550e8400-e29b-41d4-a716-446655440000";
        assert_eq!(note_label(id), "note-550e8400-e29b-41d4-a716-446655440000");
        assert!(validate_note_id(id).is_ok());
        assert!(validate_note_id("../editor").is_err());
    }

    #[test]
    fn opacity_layers_use_independent_safety_limits() {
        assert_eq!(validated_opacity("background", 0).unwrap(), ("opacity", 0));
        assert_eq!(validated_opacity("background", 200).unwrap(), ("opacity", 100));
        assert_eq!(validated_opacity("text", 0).unwrap(), ("text_opacity", 20));
        assert_eq!(validated_opacity("text", 200).unwrap(), ("text_opacity", 100));
        assert!(validated_opacity("window", 80).is_err());
    }
}
