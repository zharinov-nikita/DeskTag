# DeskTag — Theme-Aware Badge (Design)

- **Date:** 2026-06-07
- **Status:** Approved (brainstorming complete → ready for implementation plan)
- **Branch:** `worktree-theme-aware-badge`

## Summary

Make the badge pill **and** the tray icon follow the Windows light/dark
*system* theme, and switch **live** when the user changes the theme — no
restart. Today both are a fixed dark pill with light text.

## Motivation

The pill colors are hardcoded in two places (`badge.rs` and `icon.rs`) as a
single dark palette. On a light desktop theme the dark pill is fine, but the
look is fixed and ignores the user's chosen theme. A near-white pill on a
bright wallpaper would also lose its edges, so theme support needs an edge
treatment too.

## Requirements (decided during brainstorming)

1. **Adapt to light/dark only.** Two fixed palettes, flipped by theme. Accent
   color is explicitly out of scope (see Non-Goals).
2. **Theme signal:** `SystemUsesLightTheme` (the system/taskbar mode), because
   the badge behaves like a system overlay (a peer of the taskbar), not an app
   window.
3. **Palettes** (`COLORREF` is `0x00BBGGRR`; the grays are symmetric so byte
   order is moot):

   | Theme | bg (pill) | text | border |
   |-------|-----------|------|--------|
   | Dark (current) | `0x202020` | `0xF0F0F0` | `0x505050` |
   | Light (new)    | `0xF0F0F0` | `0x202020` | `0xC8C8C8` |

   `ALPHA = 220` unchanged for both.
4. **Hairline 1px border** on the rounded pill, in the per-theme `border` tone,
   so the pill reads on a matching-luminance wallpaper (light pill on light
   wallpaper / dark on dark). Applies to the badge window **and** the tray icon.
5. **Live update** when the theme changes, independent of the winvd listener.

## Non-Goals

- **Accent color.** A future, separate feature could use the Windows accent
  color for the pill. Not in this change.
- No new configuration/UI for choosing colors. Palettes are compile-time
  constants (the border tones are easy to tune later).
- The embedded EXE icon asset (`assets/desktag.ico`) is **not** re-themed — it
  stays the baked Dark pill (see `write_ico` below). No asset churn.

## Architecture

A new module `theme.rs` is the single source of truth for the palette. Flow:

```
theme::detect() --reads registry--> Theme (Light|Dark)
        Palette::for_theme(Theme) --pure--> Palette { bg, text, border }
                                |
        badge.rs (UI thread)    |  holds CURRENT_THEME (thread_local)
          WM_PAINT  -----------> uses Palette for fill / text / border
          install_tray / update_tray_icon --passes Palette--> icon::rasterize
                                |
        wndproc WM_SETTINGCHANGE("ImmersiveColorSet")
          -> re-detect -> if changed: store + InvalidateRect + update tray
```

The daemon startup sequence in `main.rs` is **unchanged** (it is load-bearing,
per CLAUDE.md gotchas). Only a `mod theme;` line is added.

## Components

### `src/theme.rs` (new)

```rust
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Theme { Light, Dark }

#[derive(Clone, Copy)]
pub struct Palette {
    pub bg: COLORREF,
    pub text: COLORREF,
    pub border: COLORREF,
}

impl Palette {
    /// Pure mapping Theme -> colors. Unit-testable, OS-independent in spirit.
    pub fn for_theme(t: Theme) -> Palette { /* table above */ }
}

/// Read HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize
/// value `SystemUsesLightTheme` (REG_DWORD): 1 -> Light, 0 -> Dark.
/// On any error or missing value -> Dark (preserve current look, no panic).
pub fn detect() -> Theme { /* RegGetValueW + RRF_RT_REG_DWORD */ }
```

Registry read uses the already-present `windows` crate (`RegGetValueW`); **no
new dependency**.

### `src/badge.rs` (modified)

- Remove constants `BG_COLOR` / `TEXT_COLOR`. Keep `ALPHA`.
- Add `thread_local! { static CURRENT_THEME: Cell<Theme> }`, initialized in
  `create()` from `theme::detect()`.
- `WM_PAINT`: build `let p = Palette::for_theme(CURRENT_THEME.get());`
  - fill with `p.bg`,
  - draw text with `p.text`,
  - draw the **border** with `p.border` via `FrameRgn` over a rounded region
    matching the window (`CreateRoundRectRgn` with the same radius). `FrameRgn`
    paints *inside* the region edge, so the 1px (DPI-scaled) line is not clipped
    away by the window region. Region recreated locally and deleted in-paint.
- New `WM_SETTINGCHANGE` arm: if `lParam` points to the wide string
  `"ImmersiveColorSet"`, call `theme::detect()`; if it differs from
  `CURRENT_THEME`, store it, `InvalidateRect`, and `update_tray_icon`. Other
  `WM_SETTINGCHANGE` payloads are ignored (cheap string compare, no repaint).
- `install_tray` / `update_tray_icon` derive the current `Palette` and pass it
  to `icon::make_tray_hicon`.

### `src/icon.rs` (modified)

- Remove constants `BG` / `TEXT`.
- `rasterize(text, size, palette: &Palette)` — fill `palette.bg`, text
  `palette.text`, and stroke the same hairline `palette.border` so the tray
  icon is legible on a light **or** dark taskbar.
- `make_tray_hicon(text, size, palette: &Palette)` — thread `palette` through.
- `write_ico(path, text)` (used by `--gen-icon` for the embedded resource):
  rasterize with a **fixed** `Palette::for_theme(Theme::Dark)`, so the committed
  `assets/desktag.ico` is byte-identical to today. The embedded icon is static;
  only the live tray icon is themed.
- Update existing tests to pass a `Palette` (e.g. the Dark palette).

### `src/main.rs` (modified)

- Add `mod theme;`. Nothing else.

## Data Flow

1. **Startup:** `badge::create` → `theme::detect()` → store `CURRENT_THEME`.
   First `WM_PAINT` and `install_tray` render with that palette.
2. **Theme toggled:** shell broadcasts `WM_SETTINGCHANGE` with
   `lParam = "ImmersiveColorSet"` to all top-level windows. The badge is a
   top-level owned `WS_POPUP`, so it receives it. `wndproc` re-detects and, on
   change, repaints the pill and rebuilds the tray icon. This path is
   independent of the winvd event listener, so it works even in fallback-poll
   mode.
3. **Desktop changed (existing):** `WM_APP_DESKTOP_CHANGED` → `apply_label` +
   `update_tray_icon`, which now rasterizes with the current palette.

## Error Handling

- `theme::detect()` registry open/query failure or missing value → `Theme::Dark`
  (no panic; preserves the current appearance).
- `WM_SETTINGCHANGE` with a non-`"ImmersiveColorSet"` payload → ignored.
- Border draw failure (GDI) → non-fatal, consistent with the existing
  best-effort GDI paint.
- No new failure modes introduced into the load-bearing daemon sequence.

## Testing

- **`theme.rs`** — unit test `Palette::for_theme` for both variants, asserting
  exact `bg` / `text` / `border` values (pure, like `label.rs`).
- **`theme::detect()`** — `#[cfg(windows)]` smoke test: returns a `Theme`
  without panicking.
- **`icon.rs`** — existing `rasterize` / `make_tray_hicon` tests updated to pass
  a `Palette`; palette-agnostic invariants still hold (opaque pill body,
  transparent corners; valid `.ico` header).
- **Manual** — run the daemon; Settings → Personalization → Colors → switch
  Light/Dark; observe the pill and tray icon flip live; verify the border is
  visible on a same-luminance wallpaper.

## File Changes Summary

| File | Change |
|------|--------|
| `src/theme.rs` | **new** — `Theme`, `Palette`, `for_theme` (pure), `detect` (registry) |
| `src/badge.rs` | drop color consts; `CURRENT_THEME` thread_local; themed `WM_PAINT` + border; `WM_SETTINGCHANGE` handler; pass palette to tray |
| `src/icon.rs` | drop color consts; `rasterize`/`make_tray_hicon` take `&Palette`; border stroke; `write_ico` pins Dark |
| `src/main.rs` | add `mod theme;` |
| `.gitignore` | anchor `superpowers/` → `/superpowers/` so `docs/superpowers/` is trackable |

No new crate dependencies. `assets/desktag.ico` unchanged.
