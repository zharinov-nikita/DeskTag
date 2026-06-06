# Configurable badge position — design

Date: 2026-06-07

## Goal

Let the user place the DeskTag pill anywhere on screen. Three ways, all
requested:

- **Popular positions** — nine anchor presets (corners, edge centers, screen
  center), chosen from the tray menu.
- **Any place** / **fully custom** — drag the pill with the mouse to a freeform
  location.

The chosen position persists across restarts.

## Scope

**In scope**

- Nine anchor presets via the tray context menu.
- Drag-to-move for a freeform custom position.
- Persist the choice to a config file; restore on launch.
- Anchors computed against the **primary monitor work area**; custom stored as
  absolute virtual-screen coordinates so it can land on any monitor.
- React to display-configuration changes at runtime (`WM_DISPLAYCHANGE`,
  `WM_SETTINGCHANGE` work-area, existing `WM_DPICHANGED`): re-apply the anchor
  to the new geometry; re-clamp a custom position back into the visible area.

**Out of scope / deferred**

- Hiding the badge from screen capture during screen share
  (`SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)`) — deferred to a later
  iteration.
- Per-monitor anchors (anchor to a specific non-primary monitor). v1 anchors are
  primary-only; a custom drag covers the "badge on another monitor" need.
- AppBar / space reservation, taskbar embedding, click-through — explicitly
  rejected during brainstorming.

## Data model

Pure types, OS-independent:

```rust
enum Anchor {
    TopLeft, TopCenter, TopRight,
    MidLeft, Center, MidRight,
    BottomLeft, BottomCenter, BottomRight,
}

enum Position {
    Anchor(Anchor),
    Custom { x: i32, y: i32 },
}
```

Default = `Position::Anchor(Anchor::TopCenter)`. This reproduces today's
hardcoded top-center placement, so an existing user with no config file sees no
change.

## Modules

Follows the project's existing split of pure, unit-tested logic (`label.rs`,
`edit.rs`) from the Win32 surface (`badge.rs`).

### `position.rs` (new — pure, OS-independent, unit-tested)

- `parse(&str) -> Position` and `format(&Position) -> String` — the on-disk
  serialization (key=value, see Persistence). Robust: missing/garbage input
  falls back to the default.
- `anchor_origin(work: Rect, size: (i32, i32), margin: i32) -> (i32, i32)` —
  top-left coordinate for a given anchor inside a work-area rect, for a pill of
  `size`, inset by `margin` at the edges.
- `clamp(pos: (i32, i32), bounds: Rect, size: (i32, i32)) -> (i32, i32)` — push
  a point back so a pill of `size` stays fully inside `bounds`.

`Rect` here is a plain pure struct (left/top/right/bottom `i32`), not the Win32
`RECT`, to keep the module OS-independent and testable on any platform.

### `config.rs` (new — thin IO)

- Resolves the config path: `%APPDATA%\DeskTag\config` via
  `std::env::var("APPDATA")`.
- `load() -> Position` — read file, delegate to `position::parse`; any IO error
  or missing file yields the default.
- `save(&Position)` — create the `DeskTag` dir if needed, write
  `position::format(...)`. Best-effort: on error, log to stderr and continue
  (never panic, never block the UI).

### `badge.rs` (changes)

- New `thread_local! POSITION: RefCell<Position>`, loaded via `config::load()`
  in `create()`.
- Rewrite `resize_and_position`:
  - measure pill `(w, h)` as today;
  - if `Anchor(a)`: get the primary work area
    (`SystemParametersInfoW(SPI_GETWORKAREA, ...)`), compute
    `position::anchor_origin(work, (w,h), scale(8))`;
  - if `Custom{x,y}`: `position::clamp((x,y), virtual_bounds, (w,h))` where
    `virtual_bounds` comes from `SM_XVIRTUALSCREEN / SM_YVIRTUALSCREEN /
    SM_CXVIRTUALSCREEN / SM_CYVIRTUALSCREEN`;
  - `SetWindowPos` to the result; rebuild the rounded-rect region as today.
- Tray menu: add a **Position** submenu with nine items (one per anchor), a
  check mark on the active anchor. Selecting one sets `POSITION = Anchor(a)`,
  calls `config::save`, and repositions.
- Drag handling (display mode only; disabled while renaming):
  - `WM_LBUTTONDOWN`: `SetCapture`, record the cursor start and the pill origin.
  - `WM_MOUSEMOVE` while captured: once movement exceeds the system drag
    threshold (`SM_CXDRAG` / `SM_CYDRAG`), enter dragging and move the window to
    follow the cursor (`SetWindowPos` with `SWP_NOSIZE`; no region rebuild —
    size is unchanged).
  - `WM_LBUTTONUP`: `ReleaseCapture`; if a drag happened, set
    `POSITION = Custom{...}` from the final origin and `config::save`.
  - Double-click (rename) is unaffected: `WM_LBUTTONDBLCLK` is a distinct
    message, and the drag threshold absorbs the small jitter of a double-click's
    first press.
- New message handlers:
  - `WM_DISPLAYCHANGE` → `resize_and_position`.
  - `WM_SETTINGCHANGE` (work-area changes) → `resize_and_position`.

No change to the load-bearing window setup order (create → pin → hide_from_alt_tab
→ tray → listener). Drag/position work happens entirely after setup.

## Persistence

Location: `%APPDATA%\DeskTag\config`.

Format — plain `key=value` lines:

```
mode=anchor
anchor=bottom-right
```

or

```
mode=custom
x=1820
y=12
```

Anchor tokens: `top-left top-center top-right mid-left center mid-right
bottom-left bottom-center bottom-right`.

`load()` is called once in `create()`. `save()` is called on a preset selection
and at the end of a drag.

## Geometry details

- **Anchors** use the **primary work area** (`SPI_GETWORKAREA`, taskbar
  excluded) so bottom anchors sit above the taskbar and top anchors below any
  top-docked appbar. Edge margin = `scale(8)` (matches today's 8px top inset),
  DPI-scaled.
- **Custom** stores absolute virtual-screen coordinates, so a drag can place the
  pill on any monitor. On restore it is clamped into the virtual-screen bounds,
  so a disconnected/rearranged monitor never strands the badge off-screen.
- **Display changes** re-run `resize_and_position`: an anchor recomputes against
  the new work area (snaps to the new corner); a custom position re-clamps.

## Error handling

- Config read failure / missing file / malformed content → default position
  (`TopCenter`). Never panics.
- Config write failure → log to stderr, keep running with the in-memory
  position.
- All Win32 metric/work-area queries fall back gracefully (e.g. a zero/empty
  work area must not divide-by-zero or place the pill at a nonsensical spot).

## Testing

`position.rs` (cross-platform unit tests, like `label.rs` / `edit.rs`):

- `parse`/`format` round-trip for every anchor and for a custom position.
- `parse` robustness: empty string, missing keys, unknown anchor token, garbage,
  non-numeric coordinates → default.
- `anchor_origin` for all nine anchors against a sample work rect + pill size +
  margin, asserting the expected top-left.
- `clamp`: points outside each edge are pushed in; an inside point is unchanged;
  a pill larger than bounds degrades sanely.

`config.rs`:

- `save` then `load` round-trip through a temp directory
  (`std::env::temp_dir`), asserting the recovered `Position`.

Manual smoke (Windows, documented, not automated): pick each preset from the
tray; drag the pill; restart and confirm the position is restored; change
resolution / disconnect a monitor and confirm the badge stays visible.

## Dependencies

None added. `SystemParametersInfoW`, `GetSystemMetrics`, `SetCapture` /
`ReleaseCapture` / `GetCapture` are already in the `Win32_UI_WindowsAndMessaging`
feature. No `serde`, no registry feature.

## Backward compatibility

No config file → default `TopCenter` → identical to current behavior. The
feature is purely additive.
