# Theme-Aware Badge Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the badge pill and tray icon follow the Windows light/dark system theme and switch live when the user changes the theme.

**Architecture:** A new `theme.rs` module is the single source of truth: `Palette::for_theme` (pure) maps a detected `Theme` to colors, and `detect()` reads the registry. `badge.rs` holds the current theme in UI-thread state, paints the pill + a hairline border with the palette, and re-detects on `WM_SETTINGCHANGE("ImmersiveColorSet")`. `icon.rs` takes the palette as a parameter so the live tray icon themes too; the embedded `.ico` asset stays the fixed Dark pill.

**Tech Stack:** Rust (edition 2021), `windows` crate 0.58 (Win32 GDI, Registry), `winvd`. Tests run via `cargo test` (inline `#[cfg(test)]` modules, the existing convention).

---

## Spec

Design: `docs/superpowers/specs/2026-06-07-theme-aware-badge-design.md`.

## File Structure

| File | Responsibility |
|------|----------------|
| `src/theme.rs` (**new**) | `Theme` enum, `Palette` struct, pure `for_theme`, registry `detect`. The only place colors are defined. |
| `src/badge.rs` (modify) | Hold current `Theme`; themed `WM_PAINT` + hairline border; `WM_SETTINGCHANGE` live update; pass palette to the tray. |
| `src/icon.rs` (modify) | `rasterize`/`make_tray_hicon` take `&Palette` (+ a `border` flag); `write_ico` pins the Dark palette with no border so the asset is unchanged. |
| `src/main.rs` (modify) | `mod theme;`. |
| `Cargo.toml` (modify) | Add the `Win32_System_Registry` feature. |

## Key signatures (locked — keep consistent across tasks)

- `theme::Theme` — `enum { Light, Dark }`
- `theme::Palette` — `struct { bg: COLORREF, text: COLORREF, border: COLORREF }`
- `theme::Palette::for_theme(theme: Theme) -> Palette`
- `theme::detect() -> Theme`
- `icon::rasterize(text: &str, size: u32, palette: &Palette, border: bool) -> Result<Vec<u8>>`
- `icon::make_tray_hicon(text: &str, size: u32, palette: &Palette) -> Result<HICON>` (calls `rasterize(.., true)`)
- `icon::write_ico(path: &str, text: &str) -> Result<()>` (Dark palette, `border = false`)
- `badge::current_palette() -> Palette` (private helper)

---

## Task 1: `theme.rs` module (palette + registry detect)

**Files:**
- Modify: `Cargo.toml:14-24` (add registry feature)
- Create: `src/theme.rs`
- Modify: `src/main.rs:3-6` (add `mod theme;`)

- [ ] **Step 1: Add the registry feature to `Cargo.toml`**

In `Cargo.toml`, add `"Win32_System_Registry"` to the `windows` features list so the final block reads:

```toml
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
    "Win32_System_Registry",
]
```

- [ ] **Step 2: Create `src/theme.rs` with types, tests, and `todo!()` bodies**

```rust
//! System theme (light/dark) detection and the per-theme color palette.
//! `Palette::for_theme` is a pure mapping (unit-tested); `detect` reads the
//! Windows registry. This module is the single source of truth for the badge
//! and tray-icon colors.

use windows::Win32::Foundation::COLORREF;

/// The Windows system-UI theme we render against.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Theme {
    Light,
    Dark,
}

/// Colors for one theme. `COLORREF` is `0x00BBGGRR`; the grays here are
/// symmetric, so byte order does not matter.
#[derive(Clone, Copy)]
pub struct Palette {
    pub bg: COLORREF,
    pub text: COLORREF,
    pub border: COLORREF,
}

impl Palette {
    /// Pure `Theme` -> colors mapping.
    pub fn for_theme(theme: Theme) -> Palette {
        todo!()
    }
}

/// Read `HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize`
/// value `SystemUsesLightTheme` (REG_DWORD): 1 => Light, 0 => Dark. On any
/// error or a missing value => Dark (preserve the current look; never panic).
pub fn detect() -> Theme {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_palette_matches_legacy_colors() {
        let p = Palette::for_theme(Theme::Dark);
        assert_eq!(p.bg.0, 0x0020_2020);
        assert_eq!(p.text.0, 0x00F0_F0F0);
        assert_eq!(p.border.0, 0x0050_5050);
    }

    #[test]
    fn light_palette_is_inverted() {
        let p = Palette::for_theme(Theme::Light);
        assert_eq!(p.bg.0, 0x00F0_F0F0);
        assert_eq!(p.text.0, 0x0020_2020);
        assert_eq!(p.border.0, 0x00C8_C8C8);
    }

    #[test]
    #[cfg(windows)]
    fn detect_returns_a_theme_without_panicking() {
        assert!(matches!(detect(), Theme::Light | Theme::Dark));
    }
}
```

Then add the module declaration to `src/main.rs` alongside the other `mod` lines (after `mod badge;`):

```rust
mod badge;
mod desktop;
mod icon;
mod label;
mod theme;
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test theme`
Expected: compiles, then the three `theme::tests` FAIL with a panic `not yet implemented` (from `todo!()`).

- [ ] **Step 4: Implement `for_theme` and `detect`**

Replace the `for_theme` body:

```rust
    pub fn for_theme(theme: Theme) -> Palette {
        match theme {
            Theme::Dark => Palette {
                bg: COLORREF(0x0020_2020),
                text: COLORREF(0x00F0_F0F0),
                border: COLORREF(0x0050_5050),
            },
            Theme::Light => Palette {
                bg: COLORREF(0x00F0_F0F0),
                text: COLORREF(0x0020_2020),
                border: COLORREF(0x00C8_C8C8),
            },
        }
    }
```

Replace the `detect` body:

```rust
pub fn detect() -> Theme {
    use windows::core::w;
    use windows::Win32::Foundation::ERROR_SUCCESS;
    use windows::Win32::System::Registry::{
        RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD,
    };

    let mut value: u32 = 0;
    let mut size = std::mem::size_of::<u32>() as u32;
    // SAFETY: `value`/`size` are valid for a 4-byte DWORD read; RRF_RT_REG_DWORD
    // restricts the value type so nothing else is written.
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            w!("SystemUsesLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut value as *mut u32 as *mut core::ffi::c_void),
            Some(&mut size),
        )
    };
    if status == ERROR_SUCCESS && value == 1 {
        Theme::Light
    } else {
        Theme::Dark
    }
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test theme`
Expected: 3 passed (`dark_palette_matches_legacy_colors`, `light_palette_is_inverted`, `detect_returns_a_theme_without_panicking`).

Run: `cargo test`
Expected: 8 passed total (the 5 existing + 3 new). Note: a transient `dead_code` warning for `detect`/`for_theme` is expected until Task 2 wires them into the daemon — that is fine, it is a warning, not an error.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml src/theme.rs src/main.rs
git commit -m "feat(theme): add theme module with palette and registry detect" -m "Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: Theme the pill window + hairline border (`badge.rs`)

**Files:**
- Modify: `src/badge.rs` (imports, thread_local, `create`, `WM_PAINT`, new `current_palette` helper; remove `BG_COLOR`/`TEXT_COLOR`)

This task themes the badge window. The tray icon still uses its own colors until Task 3 — that is an acceptable intermediate state (the build stays green).

- [ ] **Step 1: Import the theme types**

At the top of `src/badge.rs`, after the existing `use` lines, add:

```rust
use crate::theme::{Palette, Theme};
```

- [ ] **Step 2: Remove the hardcoded color constants**

Delete these two lines (around `src/badge.rs:28-29`); keep `ALPHA`:

```rust
const BG_COLOR: COLORREF = COLORREF(0x0020_2020); // dark gray
const TEXT_COLOR: COLORREF = COLORREF(0x00F0_F0F0); // near white
```

The comment line `// COLORREF is 0x00BBGGRR.` directly above may stay (it still documents `ALPHA`/region colors).

- [ ] **Step 3: Add current-theme UI-thread state**

Extend the existing `thread_local!` block (around `src/badge.rs:32-36`) to add `CURRENT_THEME`:

```rust
thread_local! {
    static LABEL: RefCell<String> = RefCell::new(String::from("Desktop ?"));
    /// The tray HICON we currently own (null = none / using a shared fallback).
    static CURRENT_TRAY_ICON: Cell<HICON> = const { Cell::new(HICON(std::ptr::null_mut())) };
    /// The system theme we last painted with. Updated on WM_SETTINGCHANGE.
    static CURRENT_THEME: Cell<Theme> = const { Cell::new(Theme::Dark) };
}
```

- [ ] **Step 4: Detect the theme in `create`**

In `create`, immediately after the existing line that sets the initial label
(`LABEL.with(|l| *l.borrow_mut() = initial.to_string());`), add:

```rust
    CURRENT_THEME.with(|t| t.set(crate::theme::detect()));
```

- [ ] **Step 5: Add the `current_palette` helper**

Add this private helper (e.g. directly below the `scale` function):

```rust
/// The palette for the theme we currently believe is active (UI thread only).
fn current_palette() -> Palette {
    Palette::for_theme(CURRENT_THEME.with(|t| t.get()))
}
```

- [ ] **Step 6: Repaint with the palette and draw the border**

Replace the entire `WM_PAINT` arm in `wndproc` with:

```rust
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut rc = RECT::default();
                let _ = GetClientRect(hwnd, &mut rc);

                let p = current_palette();

                let brush = CreateSolidBrush(p.bg);
                FillRect(hdc, &rc, brush);
                let _ = DeleteObject(brush);

                let font = make_font(hwnd);
                let old = SelectObject(hdc, font);
                SetBkMode(hdc, TRANSPARENT);
                SetTextColor(hdc, p.text);
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

                // Hairline border: frame the rounded region from the inside so
                // the (DPI-scaled) 1px line is not clipped by the window region.
                let radius = scale(hwnd, 16);
                let rgn = CreateRoundRectRgn(0, 0, rc.right + 1, rc.bottom + 1, radius, radius);
                let border_brush = CreateSolidBrush(p.border);
                let t = scale(hwnd, 1).max(1);
                let _ = FrameRgn(hdc, rgn, border_brush, t, t);
                let _ = DeleteObject(border_brush);
                let _ = DeleteObject(rgn);

                let _ = EndPaint(hwnd, &ps);
                LRESULT(0)
            }
```

(The `radius` here matches `resize_and_position`, which builds the window region with `scale(hwnd, 16)`.)

- [ ] **Step 7: Build and run the existing tests**

Run: `cargo build`
Expected: compiles cleanly (the Task 1 `dead_code` warning is now gone — `detect`/`for_theme` are used).

Run: `cargo test`
Expected: 8 passed (unchanged; this task has no new unit test — the change is GDI paint).

- [ ] **Step 8: Manual smoke (optional but recommended)**

Run: `cargo run` (in dark system theme), confirm the pill looks unchanged (dark) and now has a faint border. Quit via the tray's Quit. Full live-switch verification is Task 5.

- [ ] **Step 9: Commit**

```bash
git add src/badge.rs
git commit -m "feat(badge): theme the pill window and add a hairline border" -m "Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: Theme the tray icon palette + border (`icon.rs` + badge call sites)

**Files:**
- Modify: `src/icon.rs` (imports, `rasterize`, `make_tray_hicon`, `encode_ico`, `bmp_icon_image`, `write_ico`, tests; remove `BG`/`TEXT`)
- Modify: `src/badge.rs` (`install_tray`, `update_tray_icon` call sites)

`icon.rs` and its `badge.rs` callers change together so the build stays green.

- [ ] **Step 1: Write the failing test (border changes pixels)**

In the `#[cfg(test)] mod tests` of `src/icon.rs`, update the imports line and the existing tests, and add a border test. Replace the whole test module with:

```rust
#[cfg(test)]
#[cfg(windows)]
mod tests {
    use super::*;
    use crate::theme::{Palette, Theme};

    #[test]
    fn rasterize_pill_has_size_and_alpha() {
        let size = 32u32;
        let palette = Palette::for_theme(Theme::Dark);
        let rgba = rasterize("1", size, &palette, true).unwrap();
        assert_eq!(rgba.len(), (size * size * 4) as usize);
        // Pill body is opaque somewhere, corners are transparent somewhere.
        assert!(rgba.chunks(4).any(|p| p[3] == 255), "expected opaque pixels");
        assert!(rgba.chunks(4).any(|p| p[3] == 0), "expected transparent corners");
    }

    #[test]
    fn border_changes_pixels() {
        let size = 32u32;
        let palette = Palette::for_theme(Theme::Dark);
        let plain = rasterize("1", size, &palette, false).unwrap();
        let bordered = rasterize("1", size, &palette, true).unwrap();
        assert_ne!(plain, bordered, "border flag should change the raster");
    }

    #[test]
    fn make_tray_hicon_returns_icon() {
        let palette = Palette::for_theme(Theme::Light);
        let icon = make_tray_hicon("2", 32, &palette).unwrap();
        assert!(!icon.is_invalid());
        unsafe {
            let _ = windows::Win32::UI::WindowsAndMessaging::DestroyIcon(icon);
        }
    }

    #[test]
    fn encode_ico_has_valid_header() {
        let sizes = [16u32, 32, 48, 256];
        let palette = Palette::for_theme(Theme::Dark);
        let bytes = encode_ico("D", &sizes, &palette).unwrap();
        // ICONDIR magic: reserved=0, type=1 (icon).
        assert_eq!(&bytes[0..4], &[0, 0, 1, 0]);
        // Entry count.
        assert_eq!(u16::from_le_bytes([bytes[4], bytes[5]]), sizes.len() as u16);
        // First entry width byte (offset 6) = 16.
        assert_eq!(bytes[6], 16);
        // 256 is encoded as 0 in the last entry's width byte.
        let last = 6 + 16 * (sizes.len() - 1);
        assert_eq!(bytes[last], 0);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test icon`
Expected: FAIL — does not compile, because `rasterize`/`encode_ico` do not yet take a `&Palette`/`border` argument and `crate::theme` is not imported in `icon.rs`.

- [ ] **Step 3: Import the theme types in `icon.rs`**

At the top of `src/icon.rs`, after the existing `use` lines, add:

```rust
use crate::theme::{Palette, Theme};
```

- [ ] **Step 4: Remove the hardcoded color constants**

Delete these lines (around `src/icon.rs:11-13`):

```rust
// Pill palette — keep in sync with badge.rs (window pill uses the same colors).
const BG: COLORREF = COLORREF(0x0020_2020); // 0x00BBGGRR, dark gray
const TEXT: COLORREF = COLORREF(0x00F0_F0F0); // near white
```

- [ ] **Step 5: Take the palette + border flag in `rasterize`**

Change the `rasterize` signature and use the palette. Replace the signature line and the fill/text color lines, and add the border block before the pixel readback.

Signature:

```rust
pub fn rasterize(text: &str, size: u32, palette: &Palette, border: bool) -> Result<Vec<u8>> {
```

Fill brush (was `CreateSolidBrush(BG)`):

```rust
        let brush = CreateSolidBrush(palette.bg);
```

Text color (was `SetTextColor(hdc, TEXT);`):

```rust
        SetTextColor(hdc, palette.text);
```

Then, immediately after the `DrawTextW(...)` block and its `SelectObject(hdc, old_font); let _ = DeleteObject(font);` cleanup, and BEFORE the `// Read pixels` readback, insert the border draw:

```rust
        // Optional hairline border, same rounded shape. Drawn before the alpha
        // pass so border pixels (inside the region) get alpha 255. Skipped for
        // the embedded .ico so that asset stays byte-identical.
        if border {
            let border_brush = CreateSolidBrush(palette.border);
            let brgn = CreateRoundRectRgn(0, 0, s + 1, s + 1, radius, radius);
            let _ = FrameRgn(hdc, brgn, border_brush, 1, 1);
            let _ = DeleteObject(brgn);
            let _ = DeleteObject(border_brush);
        }
```

(`s` and `radius` are the existing locals in `rasterize`.)

- [ ] **Step 6: Thread the palette through `make_tray_hicon`, `encode_ico`, `bmp_icon_image`, `write_ico`**

`make_tray_hicon` — new signature + bordered raster call:

```rust
pub fn make_tray_hicon(text: &str, size: u32, palette: &Palette) -> Result<HICON> {
    let rgba = rasterize(text, size, palette, true)?;
```

(rest of `make_tray_hicon` is unchanged)

`write_ico` — pin the Dark palette (embedded asset is theme-agnostic):

```rust
/// Encode `text` as a multi-size .ico (16/32/48/256, BMP entries) and write `path`.
pub fn write_ico(path: &str, text: &str) -> Result<()> {
    // The embedded EXE icon is static; bake it in the Dark palette with no
    // border so the committed asset never changes with the user's theme.
    let palette = Palette::for_theme(Theme::Dark);
    let bytes = encode_ico(text, &[16, 32, 48, 256], &palette)?;
    std::fs::write(path, bytes).map_err(|e| anyhow!("write {path}: {e}"))
}
```

`encode_ico` — accept and forward the palette:

```rust
fn encode_ico(text: &str, sizes: &[u32], palette: &Palette) -> Result<Vec<u8>> {
    let images: Vec<Vec<u8>> = sizes
        .iter()
        .map(|&sz| bmp_icon_image(text, sz, palette))
        .collect::<Result<_>>()?;
```

(rest of `encode_ico` unchanged)

`bmp_icon_image` — accept the palette and rasterize with `border = false` (the embedded asset has no border):

```rust
fn bmp_icon_image(text: &str, size: u32, palette: &Palette) -> Result<Vec<u8>> {
    let rgba = rasterize(text, size, palette, false)?; // top-down RGBA
```

(rest of `bmp_icon_image` unchanged)

- [ ] **Step 7: Update the `badge.rs` tray call sites to pass the palette**

In `src/badge.rs`, `install_tray`, replace the `make_tray_hicon` call:

```rust
        let size = tray_icon_size();
        let palette = current_palette();
        let hicon = match crate::icon::make_tray_hicon(&current_desktop_number(), size, &palette) {
```

In `src/badge.rs`, `update_tray_icon`, replace the `make_tray_hicon` call:

```rust
        let size = tray_icon_size();
        let palette = current_palette();
        let new = match crate::icon::make_tray_hicon(&current_desktop_number(), size, &palette) {
```

- [ ] **Step 8: Run the tests to verify they pass**

Run: `cargo test icon`
Expected: 4 passed (`rasterize_pill_has_size_and_alpha`, `border_changes_pixels`, `make_tray_hicon_returns_icon`, `encode_ico_has_valid_header`).

Run: `cargo test`
Expected: 9 passed total (5 original + 3 theme + 1 new `border_changes_pixels`; the two changed icon tests still pass).

- [ ] **Step 9: Verify the embedded asset is unchanged**

Run: `cargo run -- --gen-icon assets/desktag.ico`
Then: `git diff --stat assets/desktag.ico`
Expected: **no diff** (empty output) — `write_ico` pins Dark with `border = false`, so the regenerated bytes match the committed asset. If a diff appears, the border flag or palette pinning is wrong — fix before committing.

- [ ] **Step 10: Commit**

```bash
git add src/icon.rs src/badge.rs
git commit -m "feat(icon): theme the tray icon palette and add a border flag" -m "Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: Live theme switch on `WM_SETTINGCHANGE` (`badge.rs`)

**Files:**
- Modify: `src/badge.rs` (new `WM_SETTINGCHANGE` arm in `wndproc`; new `lparam_is_immersive_color_set` helper)

- [ ] **Step 1: Add the lParam string-match helper**

Add this private helper near `current_palette` in `src/badge.rs`:

```rust
/// True when a WM_SETTINGCHANGE lParam points to the wide string
/// "ImmersiveColorSet" — the signal Windows broadcasts on a light/dark or
/// accent change. lParam can be null for other setting changes.
unsafe fn lparam_is_immersive_color_set(lparam: LPARAM) -> bool {
    if lparam.0 == 0 {
        return false;
    }
    let ptr = lparam.0 as *const u16;
    let mut len = 0usize;
    // "ImmersiveColorSet" is 17 chars; cap the scan well above that.
    while len <= 64 && *ptr.add(len) != 0 {
        len += 1;
    }
    let s = String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len));
    s == "ImmersiveColorSet"
}
```

- [ ] **Step 2: Add the `WM_SETTINGCHANGE` arm to `wndproc`**

In `wndproc`'s `match msg`, add a new arm (e.g. directly after the `WM_APP_DESKTOP_CHANGED` arm):

```rust
            WM_SETTINGCHANGE => {
                if lparam_is_immersive_color_set(lparam) {
                    let new_theme = crate::theme::detect();
                    let changed = CURRENT_THEME.with(|t| {
                        if t.get() != new_theme {
                            t.set(new_theme);
                            true
                        } else {
                            false
                        }
                    });
                    if changed {
                        let _ = InvalidateRect(hwnd, None, BOOL(1));
                        update_tray_icon(hwnd);
                    }
                }
                LRESULT(0)
            }
```

- [ ] **Step 3: Build and run the tests**

Run: `cargo build`
Expected: compiles cleanly.

Run: `cargo test`
Expected: 9 passed (unchanged; the handler is GDI/message-driven and verified manually).

- [ ] **Step 4: Manual live-switch verification**

Run: `cargo run`. Open Settings → Personalization → Colors. Switch "Choose your mode" between Light and Dark (or set Windows mode). Confirm:
- the pill flips palette live (no restart) — light pill + dark text in Light, dark pill + light text in Dark;
- the border remains visible in both;
- the tray icon (desktop number) flips palette to match;
- switching virtual desktops still updates the number and keeps the current theme.

Quit via the tray's Quit.

- [ ] **Step 5: Commit**

```bash
git add src/badge.rs
git commit -m "feat(badge): switch theme live on WM_SETTINGCHANGE" -m "Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Task 5: Final verification

**Files:** none (verification; commit only if clippy fixes are needed)

- [ ] **Step 1: Full test run**

Run: `cargo test`
Expected: 9 passed, 0 failed.

- [ ] **Step 2: Lint**

Run: `cargo clippy`
Expected: no warnings. If clippy flags anything in the new/changed code, fix it inline, re-run until clean, then:

```bash
git add -A
git commit -m "chore(theme): satisfy clippy for theme-aware badge" -m "Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

(Skip this commit if clippy was already clean.)

- [ ] **Step 3: Release build**

Run: `cargo build --release`
Expected: builds `target/release/desktag.exe` with no errors.

- [ ] **Step 4: Asset integrity re-check**

Run: `git status --short assets/`
Expected: clean (no modified `assets/desktag.ico`). This confirms the embedded icon asset was not changed by the feature.

- [ ] **Step 5: Confirm against the spec**

Re-read `docs/superpowers/specs/2026-06-07-theme-aware-badge-design.md` and confirm each requirement is implemented: light/dark flip, `SystemUsesLightTheme` signal, exact palettes, hairline border (window + tray), live update via `WM_SETTINGCHANGE`, Dark default on read failure, asset unchanged, no new dependencies (only a new feature flag).

---

## Self-Review (completed by plan author)

**Spec coverage:**
- Light/dark only → Task 1 `for_theme` two palettes. ✓
- `SystemUsesLightTheme` signal → Task 1 `detect`. ✓
- Exact palettes (dark/light bg/text/border) → Task 1 + tests assert exact values. ✓
- Hairline border, window + tray → Task 2 (`FrameRgn` in `WM_PAINT`) + Task 3 (`border` flag in `rasterize`, tray = true). ✓
- Live update independent of listener → Task 4 `WM_SETTINGCHANGE` (broadcast to top-level windows, not tied to winvd). ✓
- Default Dark on read failure → Task 1 `detect` else-branch + test. ✓
- Asset unchanged → Task 3 `write_ico` pins Dark + `border = false`; Task 3 Step 9 and Task 5 Step 4 verify via `git diff`. ✓
- No new crate deps → only the `Win32_System_Registry` feature added (Task 1 Step 1). ✓
- `mod theme;` → Task 1 Step 2. ✓

**Placeholder scan:** No "TBD"/"TODO" left as work items. The `todo!()` in Task 1 Step 2 is an intentional TDD red state, replaced in Step 4. All code steps show complete code. ✓

**Type consistency:** `rasterize(text, size, &Palette, bool)`, `make_tray_hicon(text, size, &Palette)`, `encode_ico(text, sizes, &Palette)`, `bmp_icon_image(text, size, &Palette)`, `write_ico(path, text)`, `current_palette() -> Palette`, `Palette { bg, text, border }`, `Theme { Light, Dark }` — used identically across Tasks 1–4. Tray call sites (Task 3 Step 7) match the `make_tray_hicon` signature (Task 3 Step 6). ✓
