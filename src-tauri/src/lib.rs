mod commands;
mod error;
mod model;
mod state;
mod windows;

pub use error::AppError;
pub use state::AppState;

use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager,
};

fn request_show(app: &tauri::AppHandle) {
    windows::show_hub(app);
    let _ = app.emit("qingjian://show-requested", ());
}

fn build_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let show = MenuItem::with_id(app, "show", "显示轻笺", true, None::<&str>)?;
    let new_note = MenuItem::with_id(app, "new", "新建便签", true, Some("Ctrl+N"))?;
    let settings = MenuItem::with_id(app, "settings", "设置与开机自启…", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "退出轻笺", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &new_note, &settings, &separator, &quit])?;

    let tray = TrayIconBuilder::new()
        .tooltip("轻笺")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => request_show(app),
            "new" => {
                windows::show_hub(app);
                let _ = app.emit("qingjian://new-note", ());
            }
            "settings" => {
                windows::show_hub(app);
                let _ = app.emit("qingjian://settings", ());
            }
            "quit" => {
                let _ = app.emit("qingjian://quit-requested", ());
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                request_show(tray.app_handle());
            }
        });
    let tray = if let Some(icon) = app.default_window_icon() {
        tray.icon(icon.clone())
    } else {
        tray
    };
    tray.build(app)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            request_show(app);
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--hidden"]),
        ))
        .setup(|app| {
            let data_dir = app.path().app_local_data_dir()?;
            let state = AppState::initialize(data_dir.clone())?;
            app.manage(state);
            build_tray(app)?;
            if let Some(window) = app.get_webview_window(windows::HUB_LABEL) {
                windows::install_window_appearance(&window);
                windows::position_hub_on_first_launch(&window, &data_dir);
            }
            if std::env::args().any(|argument| argument == "--hidden") {
                if let Some(window) = app.get_webview_window(windows::HUB_LABEL) {
                    let _ = window.hide();
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_notes,
            commands::get_note,
            commands::create_note,
            commands::save_note,
            commands::delete_note,
            commands::restore_note,
            commands::permanently_delete_note,
            commands::empty_trash,
            commands::set_note_archived,
            commands::set_note_pinned,
            commands::list_folders,
            commands::create_folder,
            commands::rename_folder,
            commands::delete_folder,
            commands::move_note,
            commands::reorder_note,
            commands::get_workspace_state,
            commands::save_workspace_state,
            commands::get_settings,
            commands::save_settings,
            commands::list_system_fonts,
            commands::save_attachment,
            commands::export_html,
            commands::export_pdf,
            commands::export_backup,
            commands::restore_backup,
            commands::quit_app,
            windows::show_hub_window,
            windows::toggle_hub_window,
            windows::open_note_in_editor,
            windows::close_editor_window,
        ])
        .run(tauri::generate_context!())
        .expect("轻笺启动失败");
}
