# DeskTag

[![Latest release](https://img.shields.io/github/v/release/zharinov-nikita/DeskTag)](https://github.com/zharinov-nikita/DeskTag/releases/latest)
[![License: MIT](https://img.shields.io/github/license/zharinov-nikita/DeskTag)](LICENSE)
![Platform: Windows 11](https://img.shields.io/badge/platform-Windows%2011-0078D4)

> Always-on-top badge that shows the name of your current Windows 11 virtual
> desktop — on every desktop.

Windows 11 lets you name virtual desktops, but never shows that name unless you
open the Win+Tab view. DeskTag puts a small always-on-top pill on screen that
displays the current desktop's number and name, and updates the moment you
switch desktops. Build it once, leave it running.

<p align="center">
  <img src="assets/screenshots/demo.gif" alt="DeskTag badge in action: moving it, renaming the desktop, and switching desktops" width="760">
</p>

## Features

- **Always visible.** A compact pill stays on top across every virtual desktop.
- **Live updates.** Reacts instantly when you switch desktops (Win+Ctrl+←/→).
- **Inline rename.** Double-click the badge to rename the current desktop in
  place — no Win+Tab needed.
- **Nine positions + drag.** Pick an anchor from the tray menu or drag the
  badge anywhere; the chosen spot is remembered between runs.
- **Light/dark theme.** Follows the Windows system theme automatically.
- **Live tray icon.** Mirrors the current desktop; right-click for the menu.
- **Tiny & native.** A single ~680 KB binary — pure Rust + Win32, no runtime
  dependencies.

## Screenshots

| Dark theme | Light theme | Tray menu |
| :--------: | :---------: | :-------: |
| ![DeskTag badge on a dark-themed desktop](assets/screenshots/badge.png) | ![DeskTag badge on a light-themed desktop](assets/screenshots/light-theme.png) | ![Tray menu with the Position submenu open](assets/screenshots/tray-menu.png) |

The badge follows the Windows light/dark theme automatically. Double-click it to
rename the current desktop; right-click the tray icon to reposition it or quit.

## Positioning

Place the badge at any of nine anchor points from the tray **Position** submenu,
or drag it anywhere — the spot is remembered between runs.

| Top left | Top right | Top center |
| :------: | :-------: | :--------: |
| ![Badge anchored top-left](assets/screenshots/position-top-left.png) | ![Badge anchored top-right](assets/screenshots/position-top-right.png) | ![Badge anchored top-center](assets/screenshots/position-top-center.png) |

## Install

### Download (recommended)

Grab the latest build from the
[Releases page](https://github.com/zharinov-nikita/DeskTag/releases/latest):

- **`desktag-x.y.z-x86_64.msi`** — installer; adds a Start-menu shortcut and can
  enable autostart.
- **`desktag.exe`** — portable single file; just run it.

### Build from source

Requires the stable Rust toolchain and Windows 11 (24H2, build 26100.2605+).

    cargo build --release

The binary lands at `target/release/desktag.exe`.

## Usage

- Run `desktag.exe` — the badge appears and keeps running in the background.
- **Switch desktops** — the badge follows and updates the label.
- **Double-click** the badge to rename the current desktop inline.
- **Right-click** the tray icon for the **Position** submenu and **Quit**.
- **Drag** the badge to place it anywhere on screen.
- `desktag.exe --once` — print the current desktop label and exit (handy for
  scripts).

## Autostart

The MSI installer can enable autostart during setup. For the portable `.exe`:
press Win+R, type `shell:startup`, and drop a shortcut to `desktag.exe` there.

## Requirements

- Windows 11, 24H2 (build 26100.2605) or newer.
- Rust toolchain (stable) — only for building from source.

> **Note:** DeskTag binds undocumented virtual-desktop COM interfaces that can
> shift between Windows builds. If a Windows update breaks it, please open an
> issue.

## License

[MIT](LICENSE) © Nikita Zharinov
