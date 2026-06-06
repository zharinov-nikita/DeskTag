mod badge;
mod desktop;
mod label;

use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, TranslateMessage, MSG,
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
    let _hwnd = badge::create(&initial)?;
    run_message_loop();
    Ok(())
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
