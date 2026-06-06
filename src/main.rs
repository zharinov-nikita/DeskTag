mod badge;
mod desktop;
mod label;

use std::time::{Duration, Instant};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE,
};

fn main() -> anyhow::Result<()> {
    if std::env::args().any(|a| a == "--once") {
        match desktop::current_index_and_name() {
            Ok((index, name)) => println!("{}", label::format_label(index, &name)),
            Err(e) => {
                eprintln!("FAILED: {e:?}");
                std::process::exit(1);
            }
        }
        return Ok(());
    }
    run_daemon()
}

fn run_daemon() -> anyhow::Result<()> {
    let initial = desktop::current_label().unwrap_or_else(|_| "Desktop ?".to_string());
    let hwnd = badge::create(&initial)?;
    pin_with_retry(hwnd);
    badge::install_tray(hwnd);
    let _listener = desktop::start_listener(hwnd)
        .map_err(|e| {
            eprintln!("warning: event listener failed, label will not auto-update: {e:?}");
            e
        })
        .ok();
    run_message_loop();
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
