//! Thin wrapper over winvd. (Spike: read current desktop, raw.)

use anyhow::{anyhow, Result};

/// Read the current virtual desktop: 0-based index and its (possibly empty) name.
pub fn current_index_and_name() -> Result<(u32, String)> {
    let desktop =
        winvd::get_current_desktop().map_err(|e| anyhow!("get_current_desktop: {e:?}"))?;
    let index = desktop
        .get_index()
        .map_err(|e| anyhow!("get_index: {e:?}"))?;
    let name = desktop.get_name().unwrap_or_default();
    Ok((index, name))
}

use crate::label::format_label;

/// Read the current desktop and return its formatted badge label.
pub fn current_label() -> Result<String> {
    let (index, name) = current_index_and_name()?;
    Ok(format_label(index, &name))
}

/// The current desktop's raw name (may be empty), without label formatting.
pub fn current_name() -> Result<String> {
    let (_index, name) = current_index_and_name()?;
    Ok(name)
}

/// Rename the current virtual desktop. An empty string clears the name; the
/// badge then shows "Desktop N" via `format_label`.
pub fn rename_current(name: &str) -> Result<()> {
    let desktop =
        winvd::get_current_desktop().map_err(|e| anyhow!("get_current_desktop: {e:?}"))?;
    desktop
        .set_name(name)
        .map_err(|e| anyhow!("set_name: {e:?}"))
}

use windows::Win32::Foundation::HWND;

/// Pin a window so it appears on every virtual desktop. Best-effort.
pub fn pin_to_all_desktops(hwnd: HWND) -> Result<()> {
    winvd::pin_window(hwnd).map_err(|e| anyhow!("pin_window: {e:?}"))
}

use crate::badge::WM_APP_DESKTOP_CHANGED;
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::PostMessageW;

/// Start the winvd event listener. On any desktop-structure change it posts
/// `WM_APP_DESKTOP_CHANGED` to `hwnd`. The returned thread guard must be kept
/// alive for the lifetime of the program.
pub fn start_listener(hwnd: HWND) -> Result<winvd::DesktopEventThread> {
    use winvd::DesktopEvent;

    let (tx, rx) = std::sync::mpsc::channel::<DesktopEvent>();
    let guard = winvd::listen_desktop_events(tx)
        .map_err(|e| anyhow!("listen_desktop_events: {e:?}"))?;

    // HWND is not Send; pass the raw pointer as isize and rebuild it in the thread.
    let hwnd_raw = hwnd.0 as isize;
    std::thread::spawn(move || {
        for ev in rx {
            let relevant = matches!(
                ev,
                DesktopEvent::DesktopChanged { .. }
                    | DesktopEvent::DesktopNameChanged(..)
                    | DesktopEvent::DesktopCreated(..)
                    | DesktopEvent::DesktopDestroyed { .. }
                    | DesktopEvent::DesktopMoved { .. }
            );
            if relevant {
                unsafe {
                    let hwnd = HWND(hwnd_raw as *mut core::ffi::c_void);
                    let _ = PostMessageW(hwnd, WM_APP_DESKTOP_CHANGED, WPARAM(0), LPARAM(0));
                }
            }
        }
    });
    Ok(guard)
}
