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

fn request_quit(app: &tauri::AppHandle) {
    let _ = app.emit("qingjian://quit-requested", ());
    let fallback = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(4));
        fallback.exit(0);
    });
}

fn request_show(app: &tauri::AppHandle) {
    windows::show_hub(app);
    let _ = app.emit("qingjian://show-requested", ());
}

#[cfg(target_os = "windows")]
fn watch_instance_signal(app: &tauri::AppHandle, raw_signal: isize) {
    if raw_signal == 0 { return }
    let app = app.clone();
    std::thread::spawn(move || {
        use ::windows::Win32::{
            Foundation::{HANDLE, WAIT_OBJECT_0},
            System::Threading::{WaitForSingleObject, INFINITE},
        };
        let signal = HANDLE(raw_signal as *mut core::ffi::c_void);
        loop {
            if unsafe { WaitForSingleObject(signal, INFINITE) } != WAIT_OBJECT_0 { break }
            let scheduler = app.clone();
            let to_show = app.clone();
            // Queueing may report a transient event-loop error while the window
            // is being recreated. Keep the signal watcher alive so later
            // desktop/tray launches can always recover the workbench.
            let _ = scheduler.run_on_main_thread(move || request_show(&to_show));
        }
    });
}

fn build_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let show = MenuItem::with_id(app, "show", "显示轻笺工作台", true, None::<&str>)?;
    let new_note = MenuItem::with_id(app, "new", "新建便签", true, Some("Ctrl+N"))?;
    let show_notes = MenuItem::with_id(app, "show_notes", "显示全部便签  Ctrl+Alt+H", true, None::<&str>)?;
    let hide_notes = MenuItem::with_id(app, "hide_notes", "隐藏全部便签  Ctrl+Alt+H", true, None::<&str>)?;
    let unlock_notes = MenuItem::with_id(app, "unlock_notes", "解锁全部便签  Ctrl+Alt+L", true, None::<&str>)?;
    let restore_visibility = MenuItem::with_id(app, "restore_visibility", "恢复便签可见度  Ctrl+Alt+O", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "设置与开机自启", true, None::<&str>)?;
    let check_update = MenuItem::with_id(app, "check_update", "检查更新", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "退出轻笺", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[
        &show, &new_note, &show_notes, &hide_notes, &unlock_notes, &restore_visibility,
        &settings, &check_update, &separator, &quit,
    ])?;

    let tray = TrayIconBuilder::new()
        .tooltip("轻笺")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => request_show(app),
            "new" => { let _ = app.emit("qingjian://new-note", ()); }
            "show_notes" => windows::show_all(app),
            "hide_notes" => windows::hide_all(app),
            "unlock_notes" => windows::unlock_all(app),
            "restore_visibility" => windows::restore_visibility(app),
            "settings" => {
                windows::show_hub(app);
                let _ = app.emit("qingjian://settings", ());
            }
            "check_update" => {
                windows::show_hub(app);
                let _ = app.emit("qingjian://check-update", ());
            }
            "quit" => request_quit(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event { request_show(tray.app_handle()); }
        });
    let tray = if let Some(icon) = app.default_window_icon() { tray.icon(icon.clone()) } else { tray };
    tray.build(app)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run(instance_signal: Option<isize>) {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_window_state::Builder::default()
            .with_state_flags(
                tauri_plugin_window_state::StateFlags::POSITION
                    | tauri_plugin_window_state::StateFlags::SIZE,
            )
            .with_filter(|label| label == windows::HUB_LABEL)
            .build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--hidden"]),
        ))
        .on_window_event(|window, event| {
            if window.label() == windows::HUB_LABEL {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .setup(move |app| {
            let data_dir = app.path().app_local_data_dir()?;
            let state = AppState::initialize(data_dir.clone())?;
            app.manage(state);
            build_tray(app)?;
            #[cfg(target_os = "windows")]
            if let Some(signal) = instance_signal { watch_instance_signal(app.handle(), signal); }
            if let Some(window) = app.get_webview_window(windows::HUB_LABEL) {
                windows::install_window_appearance(&window);
                windows::position_hub_on_first_launch(&window, &data_dir);
            }
            let on_boot = std::env::args().any(|argument| argument == "--hidden");
            if on_boot {
                if let Some(window) = app.get_webview_window(windows::HUB_LABEL) { let _ = window.hide(); }
            } else {
                windows::show_hub(app.handle());
            }
            windows::restore_visible_note_windows(app.handle(), on_boot);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_notes,
            commands::get_note,
            commands::create_note,
            commands::save_note,
            commands::delete_note,
            commands::restore_note,
            commands::batch_note_action,
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
            commands::export_markdown,
            commands::export_pdf,
            commands::export_backup,
            commands::restore_backup,
            commands::quit_app,
            windows::show_hub_window,
            windows::toggle_hub_window,
            windows::open_note_window,
            windows::note_window_ready,
            windows::get_note_window_state,
            windows::list_visible_note_windows,
            windows::save_note_window_geometry,
            windows::set_note_window_mode,
            windows::set_note_window_always_on_top,
            windows::set_note_window_opacity,
            windows::set_note_window_locked,
            windows::hide_note_window,
            windows::finalize_note_window_move,
            windows::collapse_docked_note_window,
            windows::expand_docked_note_window,
            windows::unlock_all_note_windows,
            windows::restore_all_note_visibility,
            windows::toggle_all_note_windows,
        ])
        .run(tauri::generate_context!())
        .expect("轻笺启动失败");
}
