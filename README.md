# DeskTag

Always-on-top badge showing the name of your current Windows 11 virtual desktop,
visible on every desktop. Build it once, leave it running.

## Requirements

- Windows 11, 24H2 (build 26100.2605) or newer.
- Rust toolchain (stable).

## Build

    cargo build --release

The binary is `target/release/desktag.exe`.

## Run

- `desktag.exe` — start the badge (background; quit via the tray icon).
- `desktag.exe --once` — print the current desktop label and exit.

Rename desktops in the native Win+Tab view; the badge reflects the name.

## Autostart (optional)

Press Win+R, type `shell:startup`, and drop a shortcut to `desktag.exe` there.
