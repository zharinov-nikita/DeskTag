#![windows_subsystem = "windows"]

mod badge;
mod desktop;
mod edit;
mod icon;
mod label;

use std::time::{Duration, Instant};
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Console::{AttachConsole, ATTACH_PARENT_PROCESS};
use windows::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE,
};

fn main() -> anyhow::Result<()> {
    if let Some(pos) = std::env::args().position(|a| a == "--gen-icon") {
        unsafe {
            let _ = AttachConsole(ATTACH_PARENT_PROCESS);
        }
        let path = std::env::args()
            .nth(pos + 1)
            .unwrap_or_else(|| "assets/desktag.ico".to_string());
        match icon::write_ico(&path, "D") {
            Ok(()) => {
                println!("wrote {path}");
                return Ok(());
            }
            Err(e) => {
                eprintln!("FAILED: {e:?}");
                std::process::exit(1);
            }
        }
    }

    if std::env::args().any(|a| a == "--once") {
        unsafe {
            let _ = AttachConsole(ATTACH_PARENT_PROCESS);
        }
        match desktop::current_index_and_name() {
            Ok((index, name)) => println!("{}", label::format_label(index, &name)),
            Err(e) => {
                eprintln!("FAILED: {e:?}");
                std::process::exit(1);
            }
        }
        return Ok(());
    }

    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
    run_daemon()
}

fn run_daemon() -> anyhow::Result<()> {
    let initial = desktop::current_label().unwrap_or_else(|_| "Desktop ?".to_string());
    let hwnd = badge::create(&initial)?;
    pin_with_retry(hwnd);
    // Must follow pinning: the pin registers an app view that re-adds the badge
    // to Alt-Tab, and this drops it back out via WS_EX_TOOLWINDOW.
    badge::hide_from_alt_tab(hwnd);
    badge::install_tray(hwnd);
    match desktop::start_listener(hwnd) {
        Ok(guard) => {
            let _listener = guard; // keep alive
            run_message_loop();
        }
        Err(e) => {
            eprintln!("warning: listener failed, using fallback poll: {e:?}");
            badge::start_fallback_poll(hwnd);
            run_message_loop();
        }
    }
    Ok(())
}

/// Pin the badge to all virtual desktops. The shell only exposes an application
/// view for the window a short moment after it is shown, and `winvd` treats
/// "no view yet" (`WindowNotFound`) as terminal, so retry briefly while pumping
/// messages until the view exists.
fn pin_with_retry(hwnd: HWND) {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if desktop::pin_to_all_desktops(hwnd).is_ok() {
            return;
        }
        unsafe {
            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        if Instant::now() >= deadline {
            eprintln!("warning: pin failed, badge limited to one desktop");
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn run_message_loop() {
    unsafe {
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
