# Virtual Desktop Badge — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a lean Windows background widget — an always-on-top "pill" badge that is visible on every virtual desktop and shows the name of the desktop you are currently on, updating instantly when you switch.

**Architecture:** One Rust binary, one long-lived process. A `winvd` wrapper reads the current native virtual desktop and listens for desktop events on its own thread; a hand-rolled Win32 layered, click-through, top-most tool-window paints the label with GDI and is pinned to all desktops via `winvd::pin_window`. Desktop events are marshalled to the UI thread with `PostMessageW`, which re-queries the label and repaints. A raw `Shell_NotifyIcon` tray icon provides Quit.

**Tech Stack:** Rust (edition 2021) · `winvd` 0.0.49 (native virtual-desktop COM access; needs Windows 11 ≥ 24H2 26100.2605 — target machine is build 26200) · `windows` 0.58 (Win32: window, layered, GDI, HiDPI, tray) · `anyhow`.

---

## How to verify in this project

This is OS/GUI systems code. Only pure logic is unit-testable:

- **Pure logic (`label.rs`)** → real TDD: write a failing `#[test]`, run it red, implement, run it green, commit.
- **Win32 / `winvd` code** → the "test" is **build + run + observe**. Each such task gives an exact `cargo` command and the exact expected on-screen / stdout result. Treat "observe expected result" as the passing gate before committing.

**windows-rs 0.58 note:** all code below is complete and written against `windows` 0.58. The crate is strict about argument wrapper types. If the compiler rejects a call, the fix is mechanical — wrap a bool arg as `BOOL(1)`/`BOOL(0)`, wrap/unwrap an `Option<HWND>` parameter, or add a missing `*` import. These are type-fixups, not design gaps. Do not change behavior.

**Commits:** every commit message in this plan is shown without the trailer for brevity. Append this repo's trailer to each:

```
Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
```

**Risk #0 — de-risked first.** Task 2 is a spike that proves `winvd` works on build 26200 before any UI is built. If it fails, stop and switch to Plan B (the `VirtualDesktopAccessor` DLL or direct COM) before continuing.

---

## File structure

| File | Responsibility |
|------|----------------|
| `Cargo.toml` | Package + pinned dependencies (`winvd` 0.0.49, `windows` 0.58, `anyhow`). |
| `src/label.rs` | Pure formatting: `(index, name) -> badge string`. The only unit-tested module. |
| `src/desktop.rs` | Thin `winvd` wrapper: read current desktop → formatted label; start the event listener; pin a window. |
| `src/badge.rs` | Win32 layered click-through top-most tool-window; GDI paint; rounded-rect shape; tray icon; window procedure. |
| `src/main.rs` | Entry point: `--once` spike mode vs. daemon mode; DPI setup; wiring; message loop. |

---

## Task 1: Project scaffold and dependencies

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs` (minimal stub, replaced in later tasks)

- [ ] **Step 1: Create the Cargo project in place**

The repo already exists (git initialized, `.gitignore` present). Initialize a binary crate without overwriting git:

Run: `cargo init --name fast-blazing-virtual-desktop --vcs none`
Expected: creates `Cargo.toml` and `src/main.rs`. (`--vcs none` keeps the existing git repo.)

- [ ] **Step 2: Write `Cargo.toml`**

Replace the generated `Cargo.toml` with:

```toml
[package]
name = "fast-blazing-virtual-desktop"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "fbvd"
path = "src/main.rs"

[dependencies]
winvd = "0.0.49"
anyhow = "1"

[dependencies.windows]
version = "0.58"
features = [
    "Win32_Foundation",
    "Win32_UI_WindowsAndMessaging",
    "Win32_UI_HiDpi",
    "Win32_UI_Shell",
    "Win32_Graphics_Gdi",
    "Win32_System_LibraryLoader",
    "Win32_System_Console",
]
```

Note: `windows` is pinned to `0.58` so its `HWND` type is identical to the one `winvd` 0.0.49 expects in `pin_window`. Do not bump it independently.

- [ ] **Step 3: Minimal `src/main.rs`**

```rust
fn main() {
    println!("fbvd: starting");
}
```

- [ ] **Step 4: Build to fetch and verify dependencies**

Run: `cargo build`
Expected: `winvd`, `windows`, `anyhow` download and compile; build succeeds. If `winvd` 0.0.49 fails to resolve, run `cargo update -p winvd` and re-check the version.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock src/main.rs
git commit -m "chore: scaffold crate with winvd + windows deps"
```

---

## Task 2: winvd spike (`--once`) — Risk #0 gate

Prove `winvd` reads the current desktop on build 26200 before building any UI.

**Files:**
- Create: `src/desktop.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Write `src/desktop.rs` (spike version)**

```rust
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
```

- [ ] **Step 2: Wire `--once` into `src/main.rs`**

```rust
mod desktop;

fn main() -> anyhow::Result<()> {
    if std::env::args().any(|a| a == "--once") {
        match desktop::current_index_and_name() {
            Ok((index, name)) => {
                println!("ok: index={index} name={name:?}");
            }
            Err(e) => {
                eprintln!("FAILED: {e:?}");
                std::process::exit(1);
            }
        }
        return Ok(());
    }
    println!("fbvd: no mode (try --once)");
    Ok(())
}
```

- [ ] **Step 3: Run the spike — THE GATE**

Run: `cargo run -- --once`
Expected: prints e.g. `ok: index=0 name=""` (or `name="auth"` if the current desktop is named).

**Decision gate:**
- Prints `ok: ...` → Risk #0 retired. Continue.
- Prints `FAILED: ...` → **stop**. `winvd` 0.0.49 does not work on this build. Switch to Plan B before continuing:
  - Try `cargo update -p winvd` for a newer point release.
  - Else integrate `VirtualDesktopAccessor.dll` (same repo, `dll/`) via `libloading`, or call the COM interfaces directly. The rest of this plan stays the same; only `src/desktop.rs` changes its backend.

- [ ] **Step 4: Sanity-check live updates by hand**

Create a second desktop (Win+Ctrl+D), switch between them (Win+Ctrl+Left/Right), run `cargo run -- --once` on each. Confirm `index` changes. Rename a desktop in Win+Tab, run again, confirm `name` reflects it.

- [ ] **Step 5: Commit**

```bash
git add src/desktop.rs src/main.rs
git commit -m "feat: winvd --once spike proving desktop read on build 26200"
```

---

## Task 3: Label formatting (TDD)

**Files:**
- Create: `src/label.rs`
- Test: in `src/label.rs` (`#[cfg(test)] mod tests`)
- Modify: `src/main.rs` (add `mod label;`)

- [ ] **Step 1: Write the failing tests**

Create `src/label.rs`:

```rust
//! Pure formatting of the badge text. OS-independent; the only unit-tested unit.

#[cfg(test)]
mod tests {
    use super::format_label;

    #[test]
    fn unnamed_desktop_shows_number() {
        assert_eq!(format_label(0, ""), "Desktop 1");
        assert_eq!(format_label(3, "   "), "Desktop 4");
    }

    #[test]
    fn named_desktop_shows_number_and_name() {
        assert_eq!(format_label(0, "auth"), "1 · auth");
        assert_eq!(format_label(2, "  ui  "), "3 · ui");
    }
}
```

Add `mod label;` to the top of `src/main.rs` (below `mod desktop;`) so the test target compiles the module.

- [ ] **Step 2: Run tests — verify they fail to compile (red)**

Run: `cargo test --lib label`
Expected: FAIL — `cannot find function 'format_label' in this scope`.

- [ ] **Step 3: Implement `format_label`**

Add to the top of `src/label.rs` (above the `tests` module):

```rust
/// Build the badge text from a 0-based desktop index and its (possibly empty) name.
///
/// - Empty/whitespace name -> `"Desktop {n}"` where `n = index + 1`.
/// - Otherwise            -> `"{n} · {name}"` (trimmed).
pub fn format_label(index0: u32, name: &str) -> String {
    let n = index0 + 1;
    let name = name.trim();
    if name.is_empty() {
        format!("Desktop {n}")
    } else {
        format!("{n} · {name}")
    }
}
```

- [ ] **Step 4: Run tests — green**

Run: `cargo test --lib label`
Expected: PASS — both tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/label.rs src/main.rs
git commit -m "feat: desktop label formatting with tests"
```

---

## Task 4: Fold formatting into the desktop wrapper

**Files:**
- Modify: `src/desktop.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Add `current_label` to `src/desktop.rs`**

Append to `src/desktop.rs`:

```rust
use crate::label::format_label;

/// Read the current desktop and return its formatted badge label.
pub fn current_label() -> Result<String> {
    let (index, name) = current_index_and_name()?;
    Ok(format_label(index, &name))
}
```

- [ ] **Step 2: Make `--once` print the formatted label**

In `src/main.rs`, replace the `Ok((index, name)) => { ... }` arm with:

```rust
            Ok((index, name)) => {
                println!("{}", label::format_label(index, &name));
            }
```

- [ ] **Step 3: Build and run**

Run: `cargo run -- --once`
Expected: prints the formatted label, e.g. `Desktop 1` or `2 · auth`.

- [ ] **Step 4: Commit**

```bash
git add src/desktop.rs src/main.rs
git commit -m "feat: current_label combining winvd read and formatting"
```

---

## Task 5: The badge window (layered, click-through, top-most)

Show a static pill with the current label. No events yet.

**Files:**
- Create: `src/badge.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Write `src/badge.rs`**

```rust
//! Win32 layered, click-through, always-on-top "pill" badge.
//! Painted with GDI; shaped with a rounded-rect region; uniform alpha.

use anyhow::{anyhow, Result};
use std::cell::RefCell;
use windows::core::w;
use windows::Win32::Foundation::{BOOL, COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::*;

/// Posted (from the listener thread) when the current desktop or its name changes.
pub const WM_APP_DESKTOP_CHANGED: u32 = WM_APP + 1;

// COLORREF is 0x00BBGGRR.
const BG_COLOR: COLORREF = COLORREF(0x0020_2020); // dark gray
const TEXT_COLOR: COLORREF = COLORREF(0x00F0_F0F0); // near white
const ALPHA: u8 = 220;

thread_local! {
    static LABEL: RefCell<String> = RefCell::new(String::from("Desktop ?"));
}

/// Create and show the badge window. Returns its HWND.
pub fn create(initial: &str) -> Result<HWND> {
    LABEL.with(|l| *l.borrow_mut() = initial.to_string());
    unsafe {
        let hinstance = GetModuleHandleW(None).map_err(|e| anyhow!("GetModuleHandleW: {e:?}"))?;
        let class_name = w!("FbvdBadgeClass");

        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(wndproc),
            hInstance: hinstance.into(),
            lpszClassName: class_name,
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            ..Default::default()
        };
        if RegisterClassExW(&wc) == 0 {
            return Err(anyhow!("RegisterClassExW failed"));
        }

        let ex_style = WS_EX_LAYERED
            | WS_EX_TRANSPARENT
            | WS_EX_TOPMOST
            | WS_EX_TOOLWINDOW
            | WS_EX_NOACTIVATE;

        let hwnd = CreateWindowExW(
            ex_style,
            class_name,
            w!("fbvd"),
            WS_POPUP,
            0,
            0,
            10,
            10,
            None,
            None,
            hinstance.into(),
            None,
        )
        .map_err(|e| anyhow!("CreateWindowExW: {e:?}"))?;

        // Uniform alpha over the region-clipped window.
        SetLayeredWindowAttributes(hwnd, COLORREF(0), ALPHA, LWA_ALPHA)
            .map_err(|e| anyhow!("SetLayeredWindowAttributes: {e:?}"))?;

        resize_and_position(hwnd);
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        Ok(hwnd)
    }
}

/// Re-assert HWND_TOPMOST (Z-order can be stolen; call on a timer).
pub fn reassert_topmost(hwnd: HWND) {
    unsafe {
        let _ = SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }
}

/// Replace the label, resize/reposition the pill, and repaint. UI thread only.
pub fn apply_label(hwnd: HWND, text: &str) {
    LABEL.with(|l| *l.borrow_mut() = text.to_string());
    unsafe {
        resize_and_position(hwnd);
        let _ = InvalidateRect(hwnd, None, BOOL(1));
    }
}

fn scale(hwnd: HWND, v: i32) -> i32 {
    let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
    v * dpi as i32 / 96
}

unsafe fn make_font(hwnd: HWND) -> HFONT {
    CreateFontW(
        -scale(hwnd, 15),
        0,
        0,
        0,
        FW_SEMIBOLD.0 as i32,
        0,
        0,
        0,
        DEFAULT_CHARSET.0 as u32,
        0,
        0,
        CLEARTYPE_QUALITY.0 as u32,
        0,
        w!("Segoe UI"),
    )
}

unsafe fn measure(hwnd: HWND) -> (i32, i32) {
    let pad_x = scale(hwnd, 14);
    let pad_y = scale(hwnd, 7);
    let hdc = GetDC(hwnd);
    let font = make_font(hwnd);
    let old = SelectObject(hdc, font);
    let mut size = SIZE::default();
    LABEL.with(|l| {
        let text: Vec<u16> = l.borrow().encode_utf16().collect();
        let _ = GetTextExtentPoint32W(hdc, &text, &mut size);
    });
    SelectObject(hdc, old);
    let _ = DeleteObject(font);
    ReleaseDC(hwnd, hdc);
    (size.cx + pad_x * 2, size.cy + pad_y * 2)
}

unsafe fn resize_and_position(hwnd: HWND) {
    let (w, h) = measure(hwnd);
    let screen_w = GetSystemMetrics(SM_CXSCREEN);
    let x = (screen_w - w) / 2;
    let y = scale(hwnd, 8);
    let _ = SetWindowPos(hwnd, HWND_TOPMOST, x, y, w, h, SWP_NOACTIVATE);
    let radius = scale(hwnd, 16);
    let rgn = CreateRoundRectRgn(0, 0, w + 1, h + 1, radius, radius);
    // The window takes ownership of the region; do not delete it here.
    let _ = SetWindowRgn(hwnd, rgn, BOOL(1));
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut rc = RECT::default();
                let _ = GetClientRect(hwnd, &mut rc);

                let brush = CreateSolidBrush(BG_COLOR);
                FillRect(hdc, &rc, brush);
                let _ = DeleteObject(brush);

                let font = make_font(hwnd);
                let old = SelectObject(hdc, font);
                SetBkMode(hdc, TRANSPARENT);
                SetTextColor(hdc, TEXT_COLOR);
                LABEL.with(|l| {
                    let mut text: Vec<u16> = l.borrow().encode_utf16().collect();
                    let _ = DrawTextW(
                        hdc,
                        &mut text,
                        &mut rc,
                        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
                    );
                });
                SelectObject(hdc, old);
                let _ = DeleteObject(font);

                let _ = EndPaint(hwnd, &ps);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}
```

- [ ] **Step 2: Daemon skeleton in `src/main.rs`**

Replace `src/main.rs` with:

```rust
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
```

- [ ] **Step 3: Build, then run the daemon**

Run: `cargo run`
Expected: a small dark rounded pill appears at the top-center of the primary monitor showing the current desktop label (e.g. `Desktop 1`). A console window is also present — that is expected until Task 9. Clicks pass through the pill to whatever is behind it.

- [ ] **Step 4: Observe — manual checklist**

- Pill is visible, top-center, text centered and crisp.
- Clicking the pill clicks the window behind it (click-through).
- The pill does NOT appear in the taskbar or Alt-Tab.

Stop the daemon with Ctrl+C in the console.

- [ ] **Step 5: Commit**

```bash
git add src/badge.rs src/main.rs
git commit -m "feat: layered click-through top-most badge window"
```

---

## Task 6: Pin the badge to all desktops

Without this the pill only shows on the desktop it was created on.

**Files:**
- Modify: `src/desktop.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Add a pin helper to `src/desktop.rs`**

Append:

```rust
use windows::Win32::Foundation::HWND;

/// Pin a window so it appears on every virtual desktop. Best-effort.
pub fn pin_to_all_desktops(hwnd: HWND) -> Result<()> {
    winvd::pin_window(hwnd).map_err(|e| anyhow!("pin_window: {e:?}"))
}
```

- [ ] **Step 2: Call it in `run_daemon`**

In `src/main.rs`, in `run_daemon`, replace:

```rust
    let _hwnd = badge::create(&initial)?;
```

with:

```rust
    let hwnd = badge::create(&initial)?;
    if let Err(e) = desktop::pin_to_all_desktops(hwnd) {
        eprintln!("warning: pin failed, badge limited to one desktop: {e:?}");
    }
```

- [ ] **Step 3: Build, run, observe**

Run: `cargo run`
Expected: the pill is visible. Switch desktops (Win+Ctrl+Left/Right). The pill stays on screen on every desktop. (Its text is still stale — that is Task 7.)

- [ ] **Step 4: Commit**

```bash
git add src/desktop.rs src/main.rs
git commit -m "feat: pin badge to all virtual desktops"
```

---

## Task 7: Live updates — listen for desktop events

**Files:**
- Modify: `src/desktop.rs`
- Modify: `src/badge.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Add a listener that posts to the UI thread (`src/desktop.rs`)**

Append:

```rust
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
```

- [ ] **Step 2: Handle the message in `src/badge.rs`**

In `wndproc`, add an arm above the `_ =>` fallback:

```rust
            WM_APP_DESKTOP_CHANGED => {
                if let Ok(text) = crate::desktop::current_label() {
                    apply_label(hwnd, &text);
                }
                LRESULT(0)
            }
```

- [ ] **Step 3: Start the listener in `run_daemon` (`src/main.rs`)**

After the pin block, before `run_message_loop();`, add:

```rust
    let _listener = desktop::start_listener(hwnd)
        .map_err(|e| {
            eprintln!("warning: event listener failed, label will not auto-update: {e:?}");
            e
        })
        .ok();
```

(`_listener` holds the guard alive until `run_daemon` returns.)

- [ ] **Step 4: Build, run, observe — the core behavior**

Run: `cargo run`
Expected: switch desktops (Win+Ctrl+Left/Right) → the pill's text changes within a fraction of a second to the new desktop's label. Rename the current desktop in Win+Tab → the pill text updates to `N · <newname>`. Create/remove desktops → the number updates.

- [ ] **Step 5: Commit**

```bash
git add src/desktop.rs src/badge.rs src/main.rs
git commit -m "feat: live badge updates from winvd desktop events"
```

---

## Task 8: Tray icon with Quit

Give the daemon a clean way to exit. Raw `Shell_NotifyIcon` + popup menu, handled inside our existing message loop.

**Files:**
- Modify: `src/badge.rs`

- [ ] **Step 1: Add tray constants and imports to `src/badge.rs`**

At the top, extend imports and constants:

```rust
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
};
```

Below `WM_APP_DESKTOP_CHANGED`:

```rust
/// Posted by the shell when the tray icon is interacted with.
const WM_APP_TRAY: u32 = WM_APP + 2;
const TRAY_UID: u32 = 1;
const MENU_QUIT: usize = 1001;
```

- [ ] **Step 2: Add the tray-install function**

```rust
/// Install a tray icon (stock app icon) whose context menu offers Quit.
pub fn install_tray(hwnd: HWND) {
    unsafe {
        let hicon = LoadIconW(None, IDI_APPLICATION).unwrap_or_default();
        let mut nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: TRAY_UID,
            uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
            uCallbackMessage: WM_APP_TRAY,
            hIcon: hicon,
            ..Default::default()
        };
        // szTip: copy "fbvd" into the fixed [u16; 128] buffer.
        for (i, c) in "fbvd".encode_utf16().enumerate() {
            nid.szTip[i] = c;
        }
        let _ = Shell_NotifyIconW(NIM_ADD, &nid);
    }
}

unsafe fn remove_tray(hwnd: HWND) {
    let nid = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: TRAY_UID,
        ..Default::default()
    };
    let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
}

unsafe fn show_tray_menu(hwnd: HWND) {
    let menu = CreatePopupMenu().unwrap_or_default();
    let _ = AppendMenuW(menu, MF_STRING, MENU_QUIT, w!("Quit"));
    let mut pt = windows::Win32::Foundation::POINT::default();
    let _ = GetCursorPos(&mut pt);
    // Required so the menu closes when focus is lost.
    let _ = SetForegroundWindow(hwnd);
    let _ = TrackPopupMenu(
        menu,
        TPM_RIGHTBUTTON,
        pt.x,
        pt.y,
        0,
        hwnd,
        None,
    );
    let _ = DestroyMenu(menu);
}
```

- [ ] **Step 3: Handle tray messages in `wndproc`**

Add these arms above the `_ =>` fallback:

```rust
            WM_APP_TRAY => {
                // lParam low word holds the mouse event.
                let event = (lparam.0 as u32) & 0xFFFF;
                if event == WM_RBUTTONUP || event == WM_LBUTTONUP {
                    show_tray_menu(hwnd);
                }
                LRESULT(0)
            }
            WM_COMMAND => {
                if (wparam.0 & 0xFFFF) == MENU_QUIT {
                    remove_tray(hwnd);
                    PostQuitMessage(0);
                }
                LRESULT(0)
            }
            WM_DESTROY => {
                remove_tray(hwnd);
                PostQuitMessage(0);
                LRESULT(0)
            }
```

- [ ] **Step 4: Call `install_tray` from `run_daemon` (`src/main.rs`)**

After the pin block (before starting the listener), add:

```rust
    badge::install_tray(hwnd);
```

- [ ] **Step 5: Build, run, observe**

Run: `cargo run`
Expected: a tray icon appears. Right-click it → a "Quit" menu item. Click Quit → the pill disappears, the tray icon disappears, and the process exits cleanly (the console returns to a prompt).

- [ ] **Step 6: Commit**

```bash
git add src/badge.rs src/main.rs
git commit -m "feat: tray icon with Quit"
```

---

## Task 9: Resilience and polish

DPI awareness, top-most re-assert, a fallback poll if the listener died, and hiding the console window.

**Files:**
- Modify: `src/badge.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Timers and DPI handling in `src/badge.rs`**

Add constants below `MENU_QUIT`:

```rust
const TIMER_TOPMOST: usize = 1;
const TIMER_POLL: usize = 2;
```

At the end of `create`, before `Ok(hwnd)`, start the top-most re-assert timer:

```rust
        SetTimer(hwnd, TIMER_TOPMOST, 2000, None);
```

Add a public helper to start the fallback poll (used only when the listener failed):

```rust
/// Start a 750ms poll that re-reads the label. Use only as a fallback when the
/// event listener could not start.
pub fn start_fallback_poll(hwnd: HWND) {
    unsafe {
        SetTimer(hwnd, TIMER_POLL, 750, None);
    }
}
```

Add these arms to `wndproc` above the `_ =>` fallback:

```rust
            WM_TIMER => {
                match wparam.0 {
                    TIMER_TOPMOST => reassert_topmost(hwnd),
                    TIMER_POLL => {
                        if let Ok(text) = crate::desktop::current_label() {
                            // apply_label is cheap and idempotent; only repaints on change of size.
                            apply_label(hwnd, &text);
                        }
                    }
                    _ => {}
                }
                LRESULT(0)
            }
            WM_DPICHANGED => {
                resize_and_position(hwnd);
                LRESULT(0)
            }
```

`SetTimer`, `WM_TIMER`, `WM_DPICHANGED` come from the `WindowsAndMessaging::*` import already in scope.

- [ ] **Step 2: Process-wide DPI awareness + console hiding (`src/main.rs`)**

Add at the very top of the file (line 1):

```rust
#![windows_subsystem = "windows"]
```

This removes the console window for the daemon. To keep `--once` usable from a terminal, attach to the parent console in that mode.

Replace `main` with:

```rust
use windows::Win32::System::Console::{AttachConsole, ATTACH_PARENT_PROCESS};
use windows::Win32::UI::HiDpi::{
    SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};

fn main() -> anyhow::Result<()> {
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
```

- [ ] **Step 3: Wire the fallback poll when the listener fails (`src/main.rs`)**

Replace the listener block from Task 7 with:

```rust
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
```

Remove the now-duplicated trailing `run_message_loop(); Ok(())` at the end of `run_daemon` so the loop runs exactly once via the match above.

- [ ] **Step 4: Build in release, run, observe**

Run: `cargo run --release`
Expected: no console window appears; the pill shows and updates on desktop switches; the tray Quit still works. Leave it running and switch to a maximized app, then back — the pill remains on top (top-most re-assert).

Run: `cargo run --release -- --once` from the terminal
Expected: the current label still prints to the terminal (console attach works).

- [ ] **Step 5: Final manual acceptance checklist**

- Pill visible on every virtual desktop.
- Updates within ~1s on switch and on rename.
- Click-through; absent from taskbar/Alt-Tab.
- Survives switching to/from full-screen-ish maximized windows (stays on top).
- Tray → Quit exits cleanly, removing the pill and tray icon.

- [ ] **Step 6: Commit**

```bash
git add src/badge.rs src/main.rs
git commit -m "feat: DPI awareness, top-most re-assert, fallback poll, hidden console"
```

---

## Task 10: README and autostart note

**Files:**
- Create: `README.md`

- [ ] **Step 1: Write `README.md`**

```markdown
# fast-blazing-virtual-desktop (fbvd)

Always-on-top badge showing the name of your current Windows 11 virtual desktop,
visible on every desktop. Build it once, leave it running.

## Requirements

- Windows 11, 24H2 (build 26100.2605) or newer.
- Rust toolchain (stable).

## Build

    cargo build --release

The binary is `target/release/fbvd.exe`.

## Run

- `fbvd.exe` — start the badge (background; quit via the tray icon).
- `fbvd.exe --once` — print the current desktop label and exit.

Rename desktops in the native Win+Tab view; the badge reflects the name.

## Autostart (optional)

Press Win+R, type `shell:startup`, and drop a shortcut to `fbvd.exe` there.
```

- [ ] **Step 2: Commit**

```bash
git add README.md
git commit -m "docs: add README with build, run, autostart notes"
```

---

## Self-Review

**Spec coverage** (against `docs/superpowers/specs/2026-06-06-virtual-desktop-badge-design.md`):

- §2 path A / `winvd` wrapper → Tasks 2, 4, 6, 7.
- §4 architecture (watcher / badge window / tray; thread model with `PostMessage`) → Tasks 5, 7, 8.
- §6 data flow (start→read→paint→pin; switch/rename/create→repaint; Quit) → Tasks 5–8.
- §7 label format (`N · name` / `Desktop N`), top-center, click-through, tool-window, constants-not-config → Tasks 3, 5.
- §8 error handling: winvd-unsupported gate → Task 2; pin failure degrade → Task 6; listener-death fallback poll → Task 9; DPI → Task 9; top-most re-assert → Task 9.
- §9 testing: label unit tests → Task 3; `--once` spike → Task 2; manual checklist → Tasks 5, 9.
- §10 deps → Task 1. **Deviation:** spec listed the `tray-icon` crate; this plan uses raw `Shell_NotifyIcon` instead (Task 8) because we already own a Win32 message loop, so the raw call is self-contained and avoids pumping `tray-icon`'s global event channels. One fewer dependency; same behavior. **Deviation:** spec said Direct2D/DirectWrite; this plan renders text with GDI on the layered window — fewer moving parts for a one-line badge, identical visible result. DirectWrite remains a v2 option.
- §11 implementation order (de-risk first) → Task ordering matches.

No gaps: every spec requirement maps to a task.

**Red-flag scan (TBD / TODO / incomplete code):** none found. Every code step contains complete code; every run step has an exact command and expected result. The only "fix it" guidance (windows-rs argument wrappers) is a mechanical type-fixup with the complete code already present, not a deferred design decision.

**Type consistency:** `format_label(index0: u32, name: &str) -> String` used identically in Tasks 3, 4. `current_index_and_name`, `current_label`, `pin_to_all_desktops`, `start_listener` signatures in `desktop.rs` match their call sites in `main.rs`. `WM_APP_DESKTOP_CHANGED` defined in `badge.rs`, imported in `desktop.rs`. `apply_label(hwnd, &str)` used by both `wndproc` arms and the poll. Tray constants (`WM_APP_TRAY`, `TRAY_UID`, `MENU_QUIT`) consistent across install/remove/menu/command.
