// The Tauri dev terminal still carries logs; the GUI process itself should
// never open a second black console window on Windows.
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

#[cfg(target_os = "windows")]
fn claim_primary_instance() -> Option<isize> {
    use std::sync::OnceLock;
    use windows::{
        core::w,
        Win32::{
            Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS},
            System::Threading::{CreateEventW, CreateMutexW, SetEvent},
        },
    };

    static PRIMARY_MUTEX: OnceLock<isize> = OnceLock::new();
    let Ok(mutex) = (unsafe {
        CreateMutexW(None, true, w!("Local\\QingjianDesktopPrimaryInstanceV1"))
    }) else {
        // If Windows cannot create the guard, keep the app usable instead of
        // turning an optional lifecycle safeguard into a startup failure.
        return Some(0);
    };
    let already_running = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
    let signal = unsafe {
        CreateEventW(None, false, false, w!("Local\\QingjianDesktopShowPrimaryV1"))
    };
    if !already_running {
        let _ = PRIMARY_MUTEX.set(mutex.0 as isize);
        return Some(signal.map(|handle| handle.0 as isize).unwrap_or(0));
    }

    // An auto-reset event keeps even a very early launch signal pending until
    // the primary Tauri event loop is ready to restore its own window.
    if let Ok(signal) = signal {
        unsafe {
            let _ = SetEvent(signal);
            let _ = CloseHandle(signal);
        }
    }
    unsafe { let _ = CloseHandle(mutex); }
    None
}

fn main() {
    #[cfg(target_os = "windows")]
    {
        let Some(instance_signal) = claim_primary_instance() else { return };
        qingjian_lib::run(Some(instance_signal));
    }
    #[cfg(not(target_os = "windows"))]
    qingjian_lib::run(None)
}
