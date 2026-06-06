# Configurable Badge Position Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let the user place the DeskTag pill via nine tray-menu anchor presets or a freeform mouse drag, persisted across restarts.

**Architecture:** Pure, unit-tested geometry/serialization in a new `position.rs` (mirrors `label.rs`/`edit.rs`); thin file IO in `config.rs`; Win32 wiring in `badge.rs` (tray submenu, drag capture, `resize_and_position` rewrite, display-change handlers). No new dependencies.

**Tech Stack:** Rust 2021, `windows` crate 0.58 (Win32), GDI/USER32. Tests run cross-platform via `cargo test`.

Reference spec: `docs/superpowers/specs/2026-06-07-badge-position-design.md`.

---

## File Structure

- **Create `src/position.rs`** — pure types (`Anchor`, `Position`, `Rect`), `parse`/`format`, `anchor_origin`, `clamp`. OS-independent, unit-tested.
- **Create `src/config.rs`** — `%APPDATA%\DeskTag\config` path, `load()`/`save()`. Thin IO over `position`.
- **Modify `src/main.rs`** — add `mod position;` (Task 1) and `mod config;` (Task 4).
- **Modify `src/badge.rs`** — `POSITION` state, rewrite `resize_and_position`, work-area/virtual-bounds helpers, tray Position submenu, drag handlers, display-change handlers.
- **Modify `CLAUDE.md`** — document the two new modules and the position/config gotchas.

Baseline: `cargo test` is green at **24 tests** before starting.

---

## Task 1: `position.rs` — model + parse/format

**Files:**
- Create: `src/position.rs`
- Modify: `src/main.rs:3-7` (module list)

- [ ] **Step 1: Create `src/position.rs` with types, stubbed `parse`/`format`, and tests**

```rust
//! Pure, OS-independent badge-position model: anchor presets, custom
//! coordinates, on-disk (de)serialization, and geometry to turn a position
//! into a top-left origin. Unit-tested with no Win32 dependency, like
//! `label.rs` and `edit.rs`.

/// A plain rectangle (left, top, right, bottom). OS-independent — NOT the Win32
/// `RECT` — so this module builds and tests on any platform.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Rect {
    pub fn new(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        Rect { left, top, right, bottom }
    }
    fn width(&self) -> i32 {
        self.right - self.left
    }
    fn height(&self) -> i32 {
        self.bottom - self.top
    }
}

/// Nine standard anchor positions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anchor {
    TopLeft,
    TopCenter,
    TopRight,
    MidLeft,
    Center,
    MidRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

impl Anchor {
    /// All nine in reading order — used to build the tray submenu.
    pub const ALL: [Anchor; 9] = [
        Anchor::TopLeft,
        Anchor::TopCenter,
        Anchor::TopRight,
        Anchor::MidLeft,
        Anchor::Center,
        Anchor::MidRight,
        Anchor::BottomLeft,
        Anchor::BottomCenter,
        Anchor::BottomRight,
    ];

    /// Stable token written to the config file.
    pub fn token(self) -> &'static str {
        match self {
            Anchor::TopLeft => "top-left",
            Anchor::TopCenter => "top-center",
            Anchor::TopRight => "top-right",
            Anchor::MidLeft => "mid-left",
            Anchor::Center => "center",
            Anchor::MidRight => "mid-right",
            Anchor::BottomLeft => "bottom-left",
            Anchor::BottomCenter => "bottom-center",
            Anchor::BottomRight => "bottom-right",
        }
    }

    fn from_token(s: &str) -> Option<Anchor> {
        Anchor::ALL.into_iter().find(|a| a.token() == s)
    }

    /// Human label for the tray menu.
    pub fn label(self) -> &'static str {
        match self {
            Anchor::TopLeft => "Top left",
            Anchor::TopCenter => "Top center",
            Anchor::TopRight => "Top right",
            Anchor::MidLeft => "Middle left",
            Anchor::Center => "Center",
            Anchor::MidRight => "Middle right",
            Anchor::BottomLeft => "Bottom left",
            Anchor::BottomCenter => "Bottom center",
            Anchor::BottomRight => "Bottom right",
        }
    }
}

/// Where the badge sits: a named anchor, or an absolute custom point on the
/// virtual screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Position {
    Anchor(Anchor),
    Custom { x: i32, y: i32 },
}

impl Default for Position {
    fn default() -> Self {
        Position::Anchor(Anchor::TopCenter)
    }
}

/// Serialize to the `key=value` config body.
pub fn format(_pos: &Position) -> String {
    String::new() // stub
}

/// Parse the config body. Any missing/garbage field falls back to the default.
pub fn parse(_s: &str) -> Position {
    Position::default() // stub
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_top_center() {
        assert_eq!(Position::default(), Position::Anchor(Anchor::TopCenter));
    }

    #[test]
    fn roundtrip_all_anchors() {
        for a in Anchor::ALL {
            let p = Position::Anchor(a);
            assert_eq!(parse(&format(&p)), p, "anchor {a:?}");
        }
    }

    #[test]
    fn roundtrip_custom() {
        let p = Position::Custom { x: 1820, y: 12 };
        assert_eq!(parse(&format(&p)), p);
    }

    #[test]
    fn roundtrip_custom_negative() {
        let p = Position::Custom { x: -100, y: -5 };
        assert_eq!(parse(&format(&p)), p);
    }

    #[test]
    fn parse_empty_is_default() {
        assert_eq!(parse(""), Position::default());
    }

    #[test]
    fn parse_garbage_is_default() {
        assert_eq!(parse("hello world\n???"), Position::default());
    }

    #[test]
    fn parse_unknown_anchor_is_default() {
        assert_eq!(
            parse("mode=anchor\nanchor=middle-of-nowhere"),
            Position::default()
        );
    }

    #[test]
    fn parse_custom_missing_coord_is_default() {
        assert_eq!(parse("mode=custom\nx=10"), Position::default());
    }

    #[test]
    fn parse_custom_nonnumeric_is_default() {
        assert_eq!(parse("mode=custom\nx=abc\ny=def"), Position::default());
    }
}
```

Add `mod position;` to `src/main.rs` so the module is compiled. The module block (lines 3-7) becomes:

```rust
mod badge;
mod desktop;
mod edit;
mod icon;
mod label;
mod position;
```

- [ ] **Step 2: Run tests, verify the round-trips fail**

Run: `cargo test roundtrip`
Expected: FAIL — `roundtrip_all_anchors` / `roundtrip_custom` panic (stub `format` returns `""`, `parse` returns default). `default_is_top_center` and the `parse_*_is_default` tests PASS.

- [ ] **Step 3: Implement `format` and `parse`**

Replace the two stub functions:

```rust
/// Serialize to the `key=value` config body.
pub fn format(pos: &Position) -> String {
    match pos {
        Position::Anchor(a) => format!("mode=anchor\nanchor={}\n", a.token()),
        Position::Custom { x, y } => format!("mode=custom\nx={x}\ny={y}\n"),
    }
}

/// Parse the config body. Any missing/garbage field falls back to the default.
pub fn parse(s: &str) -> Position {
    let mut mode = None;
    let mut anchor = None;
    let mut x = None;
    let mut y = None;
    for line in s.lines() {
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        match k.trim() {
            "mode" => mode = Some(v.trim().to_string()),
            "anchor" => anchor = Anchor::from_token(v.trim()),
            "x" => x = v.trim().parse::<i32>().ok(),
            "y" => y = v.trim().parse::<i32>().ok(),
            _ => {}
        }
    }
    match mode.as_deref() {
        Some("anchor") => anchor.map_or_else(Position::default, Position::Anchor),
        Some("custom") => match (x, y) {
            (Some(x), Some(y)) => Position::Custom { x, y },
            _ => Position::default(),
        },
        _ => Position::default(),
    }
}
```

- [ ] **Step 4: Run tests, verify all pass**

Run: `cargo test position::`
Expected: PASS — all 9 `position::tests::*` green.

- [ ] **Step 5: Commit**

```bash
git add src/position.rs src/main.rs
git commit -m "feat: position model with parse/format"
```

---

## Task 2: `position.rs` — `anchor_origin`

**Files:**
- Modify: `src/position.rs`

- [ ] **Step 1: Add a stubbed `anchor_origin` and its tests**

Add after `parse` (before the `tests` module):

```rust
/// Top-left origin for `anchor` inside `work`, for a pill of `size`
/// (width, height), inset by `margin` at the edges.
pub fn anchor_origin(anchor: Anchor, work: Rect, size: (i32, i32), margin: i32) -> (i32, i32) {
    let _ = (anchor, work, size, margin);
    (0, 0) // stub
}
```

Add inside `mod tests`:

```rust
    fn work() -> Rect {
        Rect::new(0, 0, 1000, 600)
    }
    const SIZE: (i32, i32) = (100, 40);
    const M: i32 = 10;

    #[test]
    fn anchor_corners() {
        assert_eq!(anchor_origin(Anchor::TopLeft, work(), SIZE, M), (10, 10));
        assert_eq!(anchor_origin(Anchor::TopRight, work(), SIZE, M), (890, 10));
        assert_eq!(anchor_origin(Anchor::BottomLeft, work(), SIZE, M), (10, 550));
        assert_eq!(anchor_origin(Anchor::BottomRight, work(), SIZE, M), (890, 550));
    }

    #[test]
    fn anchor_edges_and_center() {
        assert_eq!(anchor_origin(Anchor::TopCenter, work(), SIZE, M), (450, 10));
        assert_eq!(anchor_origin(Anchor::BottomCenter, work(), SIZE, M), (450, 550));
        assert_eq!(anchor_origin(Anchor::MidLeft, work(), SIZE, M), (10, 280));
        assert_eq!(anchor_origin(Anchor::MidRight, work(), SIZE, M), (890, 280));
        assert_eq!(anchor_origin(Anchor::Center, work(), SIZE, M), (450, 280));
    }

    #[test]
    fn anchor_respects_work_offset() {
        // Work area not at the origin (taskbar inset / secondary monitor).
        let w = Rect::new(100, 50, 1100, 650);
        assert_eq!(anchor_origin(Anchor::TopLeft, w, SIZE, M), (110, 60));
        assert_eq!(anchor_origin(Anchor::BottomRight, w, SIZE, M), (990, 600));
    }
```

- [ ] **Step 2: Run tests, verify they fail**

Run: `cargo test anchor_`
Expected: FAIL — stub returns `(0, 0)`, asserts mismatch.

- [ ] **Step 3: Implement `anchor_origin`**

Replace the stub body:

```rust
pub fn anchor_origin(anchor: Anchor, work: Rect, size: (i32, i32), margin: i32) -> (i32, i32) {
    let (w, h) = size;
    let left = work.left + margin;
    let right = work.right - w - margin;
    let cx = work.left + (work.width() - w) / 2;
    let top = work.top + margin;
    let bottom = work.bottom - h - margin;
    let cy = work.top + (work.height() - h) / 2;
    match anchor {
        Anchor::TopLeft => (left, top),
        Anchor::TopCenter => (cx, top),
        Anchor::TopRight => (right, top),
        Anchor::MidLeft => (left, cy),
        Anchor::Center => (cx, cy),
        Anchor::MidRight => (right, cy),
        Anchor::BottomLeft => (left, bottom),
        Anchor::BottomCenter => (cx, bottom),
        Anchor::BottomRight => (right, bottom),
    }
}
```

- [ ] **Step 4: Run tests, verify they pass**

Run: `cargo test anchor_`
Expected: PASS — `anchor_corners`, `anchor_edges_and_center`, `anchor_respects_work_offset` green.

- [ ] **Step 5: Commit**

```bash
git add src/position.rs
git commit -m "feat: anchor_origin geometry for nine presets"
```

---

## Task 3: `position.rs` — `clamp`

**Files:**
- Modify: `src/position.rs`

- [ ] **Step 1: Add a stubbed `clamp` and its tests**

Add after `anchor_origin`:

```rust
/// Push `pos` so a pill of `size` stays fully inside `bounds`. If the pill is
/// larger than `bounds` on an axis, pin to the top/left edge of that axis.
pub fn clamp(pos: (i32, i32), bounds: Rect, size: (i32, i32)) -> (i32, i32) {
    let _ = (bounds, size);
    pos // stub
}
```

Add inside `mod tests`:

```rust
    #[test]
    fn clamp_inside_unchanged() {
        assert_eq!(clamp((500, 300), work(), SIZE), (500, 300));
    }

    #[test]
    fn clamp_pushes_in() {
        assert_eq!(clamp((-50, -50), work(), SIZE), (0, 0));
        assert_eq!(clamp((5000, 5000), work(), SIZE), (900, 560));
    }

    #[test]
    fn clamp_respects_bounds_offset() {
        let b = Rect::new(100, 50, 1100, 650);
        assert_eq!(clamp((0, 0), b, SIZE), (100, 50));
    }

    #[test]
    fn clamp_pill_larger_than_bounds_pins_topleft() {
        let tiny = Rect::new(0, 0, 50, 20);
        assert_eq!(clamp((10, 10), tiny, (100, 40)), (0, 0));
    }
```

- [ ] **Step 2: Run tests, verify they fail**

Run: `cargo test clamp_`
Expected: FAIL — stub returns `pos`; `clamp_pushes_in`, `clamp_respects_bounds_offset`, `clamp_pill_larger_than_bounds_pins_topleft` mismatch. `clamp_inside_unchanged` PASS.

- [ ] **Step 3: Implement `clamp`**

Replace the stub body:

```rust
pub fn clamp(pos: (i32, i32), bounds: Rect, size: (i32, i32)) -> (i32, i32) {
    let (x, y) = pos;
    let (w, h) = size;
    // .max(edge) handles a pill wider/taller than bounds: max < min would panic
    // in i32::clamp, so floor the upper bound at the near edge.
    let max_x = (bounds.right - w).max(bounds.left);
    let max_y = (bounds.bottom - h).max(bounds.top);
    (x.clamp(bounds.left, max_x), y.clamp(bounds.top, max_y))
}
```

- [ ] **Step 4: Run tests, verify they pass**

Run: `cargo test position::`
Expected: PASS — every `position::tests::*` green (16 tests in this module now).

- [ ] **Step 5: Commit**

```bash
git add src/position.rs
git commit -m "feat: clamp custom position into visible bounds"
```

---

## Task 4: `config.rs` — load/save

**Files:**
- Create: `src/config.rs`
- Modify: `src/main.rs` (module list)

- [ ] **Step 1: Create `src/config.rs` with stubbed `load_from`/`save_to` and tests**

```rust
//! Persist the chosen badge position to `%APPDATA%\DeskTag\config`.
//! Thin IO over `position::{parse, format}`. Never panics.

use crate::position::{self, Position};
use std::path::PathBuf;

/// `%APPDATA%\DeskTag\config`, or `None` if `APPDATA` is unset.
fn config_path() -> Option<PathBuf> {
    let appdata = std::env::var_os("APPDATA")?;
    let mut p = PathBuf::from(appdata);
    p.push("DeskTag");
    p.push("config");
    Some(p)
}

/// Load the saved position. Missing file / unreadable / malformed → default.
pub fn load() -> Position {
    load_from(config_path())
}

fn load_from(path: Option<PathBuf>) -> Position {
    let _ = &path;
    Position::default() // stub
}

/// Save the position. Best-effort: errors are logged, never propagated.
pub fn save(pos: &Position) {
    if let Err(e) = save_to(config_path(), pos) {
        eprintln!("config save failed: {e}");
    }
}

fn save_to(path: Option<PathBuf>, pos: &Position) -> std::io::Result<()> {
    let _ = (&path, pos);
    Ok(()) // stub
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::position::Position;

    #[test]
    fn save_load_roundtrip_via_temp() {
        let dir = std::env::temp_dir().join(format!("desktag-cfg-{}", std::process::id()));
        let path = dir.join("config");
        let _ = std::fs::remove_dir_all(&dir);
        let p = Position::Custom { x: 42, y: 99 };
        save_to(Some(path.clone()), &p).expect("save");
        assert_eq!(load_from(Some(path)), p);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_missing_file_is_default() {
        let path = std::env::temp_dir().join("desktag-cfg-nonexistent/config");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
        assert_eq!(load_from(Some(path)), Position::default());
    }

    #[test]
    fn load_none_path_is_default() {
        assert_eq!(load_from(None), Position::default());
    }
}
```

Add `mod config;` to `src/main.rs`; the module list becomes:

```rust
mod badge;
mod config;
mod desktop;
mod edit;
mod icon;
mod label;
mod position;
```

- [ ] **Step 2: Run tests, verify the round-trip fails**

Run: `cargo test config::`
Expected: FAIL — `save_load_roundtrip_via_temp` mismatches (stub `load_from` returns default, not the saved custom). The two `*_default` tests PASS.

- [ ] **Step 3: Implement `load_from` and `save_to`**

Replace both stub bodies:

```rust
fn load_from(path: Option<PathBuf>) -> Position {
    let Some(path) = path else {
        return Position::default();
    };
    match std::fs::read_to_string(&path) {
        Ok(s) => position::parse(&s),
        Err(_) => Position::default(),
    }
}
```

```rust
fn save_to(path: Option<PathBuf>, pos: &Position) -> std::io::Result<()> {
    let path = path
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "APPDATA not set"))?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&path, position::format(pos))
}
```

- [ ] **Step 4: Run tests, verify they pass**

Run: `cargo test config::`
Expected: PASS — all 3 `config::tests::*` green.

- [ ] **Step 5: Commit**

```bash
git add src/config.rs src/main.rs
git commit -m "feat: persist badge position to %APPDATA%/DeskTag/config"
```

---

## Task 5: `badge.rs` — `POSITION` state + reposition rewrite

No unit test (Win32 surface). Verification = compile clean + existing 24 tests still green + clippy + documented manual smoke.

**Files:**
- Modify: `src/badge.rs`

- [ ] **Step 1: Add imports and the `POSITION` thread-local**

In the `use windows::core::...` line, add `PCWSTR`:

```rust
use windows::core::{w, PCWSTR};
```

Add a module-use for the new pure module near the other `crate::` references (top of file, after the existing `use` block):

```rust
use crate::position::{self, Anchor, Position};
```

In the `thread_local!` block (currently `badge.rs:38-46`) add:

```rust
    // Where the badge sits; loaded from config on create, updated by the tray
    // Position submenu and by drag-to-move.
    static POSITION: RefCell<Position> = RefCell::new(Position::default());
```

- [ ] **Step 2: Load the saved position in `create`**

In `create` (`badge.rs:49-50`), right after `LABEL.with(|l| *l.borrow_mut() = initial.to_string());`, add:

```rust
    POSITION.with(|p| *p.borrow_mut() = crate::config::load());
```

- [ ] **Step 3: Add work-area / virtual-bounds helpers and rewrite `resize_and_position`**

Replace the whole `resize_and_position` function (`badge.rs:255-265`) with:

```rust
/// Primary-monitor work area (taskbar excluded). Falls back to the full primary
/// screen if the query fails or returns an empty rect.
unsafe fn primary_work_area() -> position::Rect {
    let mut rc = RECT::default();
    let ok = SystemParametersInfoW(
        SPI_GETWORKAREA,
        0,
        Some(&mut rc as *mut RECT as *mut core::ffi::c_void),
        SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
    )
    .is_ok();
    if ok && rc.right > rc.left && rc.bottom > rc.top {
        position::Rect::new(rc.left, rc.top, rc.right, rc.bottom)
    } else {
        position::Rect::new(0, 0, GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN))
    }
}

/// Bounding rect of the whole virtual screen (all monitors), for clamping a
/// custom position. Falls back to the primary screen on failure.
unsafe fn virtual_bounds() -> position::Rect {
    let x = GetSystemMetrics(SM_XVIRTUALSCREEN);
    let y = GetSystemMetrics(SM_YVIRTUALSCREEN);
    let w = GetSystemMetrics(SM_CXVIRTUALSCREEN);
    let h = GetSystemMetrics(SM_CYVIRTUALSCREEN);
    if w > 0 && h > 0 {
        position::Rect::new(x, y, x + w, y + h)
    } else {
        position::Rect::new(0, 0, GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN))
    }
}

unsafe fn resize_and_position(hwnd: HWND) {
    let (w, h) = measure(hwnd);
    let (x, y) = match POSITION.with(|p| *p.borrow()) {
        Position::Anchor(a) => {
            anchor_origin_dpi(hwnd, a, w, h)
        }
        Position::Custom { x, y } => position::clamp((x, y), virtual_bounds(), (w, h)),
    };
    let _ = SetWindowPos(hwnd, HWND_TOPMOST, x, y, w, h, SWP_NOACTIVATE);
    let radius = scale(hwnd, 16);
    let rgn = CreateRoundRectRgn(0, 0, w + 1, h + 1, radius, radius);
    // The window takes ownership of the region; do not delete it here.
    let _ = SetWindowRgn(hwnd, rgn, BOOL(1));
}

/// Anchor origin against the primary work area, with a DPI-scaled 8px margin.
unsafe fn anchor_origin_dpi(hwnd: HWND, a: Anchor, w: i32, h: i32) -> (i32, i32) {
    position::anchor_origin(a, primary_work_area(), (w, h), scale(hwnd, 8))
}
```

- [ ] **Step 4: Build, lint, and run the existing test suite**

Run: `cargo build`
Expected: compiles, no errors.

Run: `cargo clippy`
Expected: no new warnings.

Run: `cargo test`
Expected: PASS — still **27 tests** (24 existing + 0 here; position/config tests from Tasks 1-4 included). 0 failures.

- [ ] **Step 5: Manual smoke (Windows)**

Run: `cargo run` — badge appears top-center (default, unchanged). Quit via tray.

- [ ] **Step 6: Commit**

```bash
git add src/badge.rs
git commit -m "feat: drive badge placement from POSITION + work area"
```

---

## Task 6: `badge.rs` — tray Position submenu

**Files:**
- Modify: `src/badge.rs`

- [ ] **Step 1: Add the menu-id base constant and a UTF-16 helper**

Near the other menu constants (`badge.rs:27`, `const MENU_QUIT: usize = 1001;`) add:

```rust
const MENU_POS_BASE: usize = 2000; // 2000..2008 -> Anchor::ALL
```

Add a small helper near `with_display_text` (or anywhere at module scope):

```rust
/// Null-terminated UTF-16 for a dynamic menu label.
fn to_w(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}
```

- [ ] **Step 2: Build the Position submenu in `show_tray_menu`**

Replace `show_tray_menu` (`badge.rs:352-361`) with:

```rust
unsafe fn show_tray_menu(hwnd: HWND) {
    let menu = CreatePopupMenu().unwrap_or_default();
    let submenu = CreatePopupMenu().unwrap_or_default();

    let current = POSITION.with(|p| *p.borrow());
    for (i, a) in Anchor::ALL.iter().enumerate() {
        let mut flags = MF_STRING;
        if matches!(current, Position::Anchor(c) if c == *a) {
            flags |= MF_CHECKED;
        }
        let label = to_w(a.label());
        let _ = AppendMenuW(submenu, flags, MENU_POS_BASE + i, PCWSTR(label.as_ptr()));
    }

    // Popup item owns the submenu; DestroyMenu(menu) frees both.
    let _ = AppendMenuW(menu, MF_POPUP, submenu.0 as usize, w!("Position"));
    let _ = AppendMenuW(menu, MF_STRING, MENU_QUIT, w!("Quit"));

    let mut pt = windows::Win32::Foundation::POINT::default();
    let _ = GetCursorPos(&mut pt);
    // Required so the menu closes when focus is lost.
    let _ = SetForegroundWindow(hwnd);
    let _ = TrackPopupMenu(menu, TPM_RIGHTBUTTON, pt.x, pt.y, 0, hwnd, None);
    let _ = DestroyMenu(menu);
}
```

- [ ] **Step 3: Handle the new command ids in `WM_COMMAND`**

Replace the `WM_COMMAND` arm (`badge.rs:529-535`) with:

```rust
            WM_COMMAND => {
                let id = wparam.0 & 0xFFFF;
                if id == MENU_QUIT {
                    remove_tray(hwnd);
                    PostQuitMessage(0);
                } else if (MENU_POS_BASE..MENU_POS_BASE + Anchor::ALL.len()).contains(&id) {
                    let pos = Position::Anchor(Anchor::ALL[id - MENU_POS_BASE]);
                    POSITION.with(|p| *p.borrow_mut() = pos);
                    crate::config::save(&pos);
                    resize_and_position(hwnd);
                }
                LRESULT(0)
            }
```

- [ ] **Step 4: Build, lint, test**

Run: `cargo build`
Expected: compiles clean.

Run: `cargo clippy`
Expected: no new warnings.

Run: `cargo test`
Expected: PASS — 27 tests, 0 failures.

- [ ] **Step 5: Manual smoke (Windows)**

Run: `cargo run`. Right-click tray → **Position** → pick each of the nine; the pill jumps to that corner/edge/center; the active one shows a check mark. Pick "Top left", quit, relaunch → badge starts at top-left (persisted).

- [ ] **Step 6: Commit**

```bash
git add src/badge.rs
git commit -m "feat: tray Position submenu with nine anchor presets"
```

---

## Task 7: `badge.rs` — drag to move (custom position)

**Files:**
- Modify: `src/badge.rs`

- [ ] **Step 1: Add the drag-state type and thread-local**

Above the `thread_local!` block add:

```rust
/// Tracks a left-button drag of the badge. `dragging` flips true once the
/// cursor crosses the system drag threshold, so a plain click / double-click
/// (which barely moves) never turns into a move.
#[derive(Clone, Copy)]
struct DragState {
    start_cx: i32,
    start_cy: i32,
    origin_x: i32,
    origin_y: i32,
    dragging: bool,
}
```

Inside `thread_local!` add:

```rust
    static DRAG: Cell<Option<DragState>> = const { Cell::new(None) };
```

- [ ] **Step 2: Start the drag on `WM_LBUTTONDOWN`**

Add a new arm to `wndproc` (next to `WM_LBUTTONDBLCLK`, `badge.rs:563-566`):

```rust
            WM_LBUTTONDOWN => {
                // Drag only in display mode; while renaming, clicks are for text.
                if !is_editing() {
                    let mut pt = windows::Win32::Foundation::POINT::default();
                    let _ = GetCursorPos(&mut pt);
                    let mut rc = RECT::default();
                    let _ = GetWindowRect(hwnd, &mut rc);
                    DRAG.with(|d| {
                        d.set(Some(DragState {
                            start_cx: pt.x,
                            start_cy: pt.y,
                            origin_x: rc.left,
                            origin_y: rc.top,
                            dragging: false,
                        }))
                    });
                    SetCapture(hwnd);
                }
                LRESULT(0)
            }
```

- [ ] **Step 3: Follow the cursor on `WM_MOUSEMOVE`**

Add:

```rust
            WM_MOUSEMOVE => {
                DRAG.with(|d| {
                    if let Some(mut st) = d.get() {
                        let mut pt = windows::Win32::Foundation::POINT::default();
                        let _ = GetCursorPos(&mut pt);
                        let dx = pt.x - st.start_cx;
                        let dy = pt.y - st.start_cy;
                        if !st.dragging
                            && (dx.abs() >= GetSystemMetrics(SM_CXDRAG)
                                || dy.abs() >= GetSystemMetrics(SM_CYDRAG))
                        {
                            st.dragging = true;
                        }
                        if st.dragging {
                            let _ = SetWindowPos(
                                hwnd,
                                HWND_TOPMOST,
                                st.origin_x + dx,
                                st.origin_y + dy,
                                0,
                                0,
                                SWP_NOSIZE | SWP_NOACTIVATE,
                            );
                        }
                        d.set(Some(st));
                    }
                });
                LRESULT(0)
            }
```

- [ ] **Step 4: Finish the drag on `WM_LBUTTONUP` and `WM_CAPTURECHANGED`**

Add:

```rust
            WM_LBUTTONUP => {
                if let Some(st) = DRAG.with(|d| d.take()) {
                    let _ = ReleaseCapture();
                    if st.dragging {
                        let mut rc = RECT::default();
                        let _ = GetWindowRect(hwnd, &mut rc);
                        let pos = Position::Custom { x: rc.left, y: rc.top };
                        POSITION.with(|p| *p.borrow_mut() = pos);
                        crate::config::save(&pos);
                    }
                }
                LRESULT(0)
            }
            WM_CAPTURECHANGED => {
                // Capture yanked away (e.g. menu/alt-tab) — drop the drag.
                DRAG.with(|d| d.set(None));
                LRESULT(0)
            }
```

- [ ] **Step 5: Build, lint, test**

Run: `cargo build`
Expected: compiles clean.

Run: `cargo clippy`
Expected: no new warnings.

Run: `cargo test`
Expected: PASS — 27 tests, 0 failures.

- [ ] **Step 6: Manual smoke (Windows)**

Run: `cargo run`. Drag the pill to an arbitrary spot — it follows the cursor and stays where dropped. Double-click still enters rename (no accidental move). Quit, relaunch → badge restores at the dragged spot. Tiny click without moving does nothing.

- [ ] **Step 7: Commit**

```bash
git add src/badge.rs
git commit -m "feat: drag badge to a custom position"
```

---

## Task 8: `badge.rs` — react to display changes

**Files:**
- Modify: `src/badge.rs`

- [ ] **Step 1: Add `WM_DISPLAYCHANGE` and `WM_SETTINGCHANGE` arms**

Add to `wndproc` (e.g. after the existing `WM_DPICHANGED` arm, `badge.rs:559-562`):

```rust
            WM_DISPLAYCHANGE => {
                // Resolution / monitor add/remove: re-anchor or re-clamp.
                resize_and_position(hwnd);
                LRESULT(0)
            }
            WM_SETTINGCHANGE => {
                // Only react to work-area changes (taskbar moved/resized), not
                // every system-setting broadcast.
                if wparam.0 as u32 == SPI_SETWORKAREA.0 {
                    resize_and_position(hwnd);
                }
                LRESULT(0)
            }
```

- [ ] **Step 2: Build, lint, test**

Run: `cargo build`
Expected: compiles clean.

Run: `cargo clippy`
Expected: no new warnings.

Run: `cargo test`
Expected: PASS — 27 tests, 0 failures.

- [ ] **Step 3: Manual smoke (Windows)**

Run: `cargo run`. Set an anchor (e.g. Bottom right). Change display resolution (or dock/undock a monitor): the badge re-snaps to the new bottom-right and never lands off-screen. With a custom position near an edge, lowering resolution re-clamps it back into view.

- [ ] **Step 4: Commit**

```bash
git add src/badge.rs
git commit -m "feat: reposition badge on display/work-area changes"
```

---

## Task 9: Docs — update `CLAUDE.md`

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: Document the new modules**

In the Architecture module list, after the `label.rs` bullet, add:

```markdown
- `position.rs` — pure badge-position model: `Anchor` (9 presets), `Position`
  (anchor | custom x/y), `parse`/`format` for the config file, `anchor_origin`
  (origin inside a work-area rect) and `clamp` (keep custom in view).
  OS-independent; unit-tested alongside `label.rs`/`edit.rs`.
- `config.rs` — load/save the chosen `Position` to
  `%APPDATA%\DeskTag\config` (key=value). Best-effort IO; missing/garbage →
  default (top-center).
```

In the `badge.rs` bullet, note the new responsibilities (append to its description): tray **Position** submenu, drag-to-move, and `resize_and_position` driven by `POSITION` against the primary work area.

- [ ] **Step 2: Add Gotchas entries**

Add to the Gotchas list:

```markdown
- **Position is config-driven.** `resize_and_position` reads the `POSITION`
  thread-local (loaded by `config::load` in `create`). Anchors use the primary
  *work area* (`SPI_GETWORKAREA`) with a `scale(8)` margin; custom is absolute
  virtual-screen coords, `clamp`ed into `SM_*VIRTUALSCREEN` on apply. Re-applied
  on `WM_DISPLAYCHANGE` and on `WM_SETTINGCHANGE`/`SPI_SETWORKAREA`.
- **Drag vs. double-click.** `WM_LBUTTONDOWN` arms a drag (capture + start
  point); it only becomes a move once the cursor passes `SM_CXDRAG`/`SM_CYDRAG`,
  so a double-click's tiny jitter still reaches `WM_LBUTTONDBLCLK` (rename).
  Drag is disabled while renaming. `WM_CAPTURECHANGED` cancels a drag.
```

- [ ] **Step 3: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: document position.rs, config.rs, and drag/anchor gotchas"
```

---

## Self-Review

**Spec coverage:**
- Nine presets via tray → Task 6. ✓
- Drag custom → Task 7. ✓
- Persist + restore → Task 4 (IO) + Task 5 Step 2 (load) + Tasks 6/7 (save). ✓
- Anchors on primary work area, custom absolute + clamp → Task 5. ✓
- Display-change handling (`WM_DISPLAYCHANGE`, `WM_SETTINGCHANGE`/`SPI_SETWORKAREA`, existing `WM_DPICHANGED`) → Task 8. ✓
- Default `TopCenter` / backward compat → `Position::default` (Task 1), loaded in Task 5. ✓
- Pure types + tests like `label.rs`/`edit.rs` → Tasks 1-3. ✓
- No new dependencies → confirmed; all Win32 symbols are under the already-enabled `Win32_UI_WindowsAndMessaging` / `Win32_Graphics_Gdi` features. ✓
- Deferred (screen-share WDA, per-monitor anchors) → not in any task, as intended. ✓

**Placeholder scan:** No unresolved-marker text anywhere; every code step shows full code. The stubs in Tasks 1-4 are intentional TDD red-state and get replaced in the same task's Step 3.

**Type consistency:** `Position` / `Anchor` / `Rect` signatures identical across tasks. `anchor_origin(anchor, work, size, margin)`, `clamp(pos, bounds, size)`, `parse(&str)->Position`, `format(&Position)->String`, `load()/save(&Position)` used consistently. `MENU_POS_BASE`, `DragState` fields, `POSITION`/`DRAG` thread-locals consistent. Test count math: 24 baseline + 9 (Task1) + ... pure/config tests counted as 27 total at Task 5 (16 position + 3 config + 5 label + 3 icon — verify actual baseline split at runtime; the green/0-failures check is the real gate, not the exact integer).

> Note on the "27 tests" figure: the baseline is 24. Tasks 1-4 add the `position`/`config` unit tests. The exact post-Task-4 total depends on the baseline split; treat **"0 failed"** as the gate. If a step's printed total differs from 27, that is not a failure — only non-zero `failed` is.
