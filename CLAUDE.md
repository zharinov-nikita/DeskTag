# DeskTag

Always-on-top badge showing the current Windows 11 virtual desktop name on
every desktop. Rust + Win32.

## Commands

```bash
cargo build --release   # -> target/release/desktag.exe
cargo run -- --once     # print current desktop label and exit (dev one-shot)
cargo run -- --gen-icon assets/desktag.ico  # regenerate the exe icon asset
cargo test              # unit tests: label + edit + (Windows) icon raster/.ico
cargo clippy            # lint
```

Run the built binary: `desktag.exe` (daemon; quit via tray icon) or
`desktag.exe --once`.

## Architecture

Single binary, six modules under `src/`:

- `main.rs` — entry. `--once` prints the label and exits; `--gen-icon [path]`
  writes the `.ico` asset and exits; otherwise sets
  per-monitor DPI awareness and runs the daemon. The daemon sequence is
  load-bearing (see Gotchas): create badge -> `pin_with_retry` ->
  `hide_from_alt_tab` -> `install_tray` -> `start_listener` -> message loop.
- `badge.rs` — the Win32 window: layered, always-on-top pill. GDI paint,
  rounded-rect region, uniform alpha, tray icon, `wndproc`, and the inline
  rename mode (double-click to edit the desktop name). Owns UI-thread state
  (`thread_local!` LABEL + EDIT buffer + caret). Receives clicks (no
  `WS_EX_TRANSPARENT`) but never activates (`WS_EX_NOACTIVATE`).
- `desktop.rs` — thin wrapper over the `winvd` crate: read current desktop
  index+name, rename the current desktop, pin to all desktops, spawn the
  event-listener thread.
- `icon.rs` — pure-ish GDI rasteriser of the badge pill. `rasterize` ->
  RGBA; `make_tray_hicon` (live tray icon, rebuilt on desktop change);
  `write_ico` (multi-size .ico for the embedded exe resource).
- `label.rs` — pure `format_label(index, name)`. OS-independent; unit-tested
  alongside `edit.rs`.
- `edit.rs` — pure `EditState` (buffer, caret, `anchor`-based selection) for
  inline rename mode: char/word caret movement, range selection (Shift/Ctrl),
  insert/delete/replace. OS-independent; unit-tested.

`build.rs` embeds `assets/desktag.ico` (regenerate with `--gen-icon`) as the
exe icon resource via `winresource`.

Desktop change -> listener thread posts `WM_APP_DESKTOP_CHANGED` -> `wndproc`
re-reads the label, repaints, and rebuilds the tray icon.

## Gotchas

- **Window setup order is load-bearing.** Create the badge as a plain owned
  `WS_POPUP` window *without* `WS_EX_TOOLWINDOW`, THEN pin, THEN add
  `WS_EX_TOOLWINDOW` (`hide_from_alt_tab`). `winvd::pin_window` needs the shell
  *application view*, which the shell grants only to non-tool, non-noactivate
  windows. Pinning re-adds the badge to Alt-Tab; the late tool-window style
  drops it back out while the pinned view survives. Reordering breaks either
  pinning or Alt-Tab hiding. `WS_EX_NOACTIVATE` is added in the same late step
  (`hide_from_alt_tab`) for the same reason — a noactivate window is denied the
  app view, so it must come after the pin.
- **Rename mode toggles input styles.** The badge drops `WS_EX_TRANSPARENT` and
  adds `CS_DBLCLKS` so a double-click can start an inline rename; the post-pin
  `WS_EX_NOACTIVATE` keeps ordinary clicks from stealing focus. `enter_edit`
  forces keyboard focus via `SetForegroundWindow` + `SetFocus` (verified to work
  despite `WS_EX_NOACTIVATE`). Display vs. rename text both flow through
  `WM_PAINT`/`measure` via `with_display_text`; `TIMER_CARET` blinks the caret.
- **`pin_with_retry`.** The shell grants the app view a moment *after* the
  window is shown, and `winvd` reports the gap as terminal `WindowNotFound`.
  Retry up to 3s while pumping messages.
- **`HWND` is not `Send`.** The listener thread receives it as a raw `isize`
  and rebuilds the `HWND` inside the thread.
- **Timers.** `TIMER_TOPMOST` (2s) re-asserts `HWND_TOPMOST` (Z-order gets
  stolen). `TIMER_POLL` (750ms) is a *fallback* only, started when the event
  listener fails to start.
- **`COLORREF` is `0x00BBGGRR`**, not RGB.
- **GDI writes alpha=0.** Classic GDI drawing leaves the DIB alpha byte at 0.
  `icon.rs::rasterize` fixes it in one pass: pixels inside the rounded-rect
  region get alpha 255, outside 0. The tray HICON also carries a 1-bpp AND
  mask from that alpha as a belt-and-braces transparency.
- **Tray icon ownership.** `CURRENT_TRAY_ICON` holds the one owned HICON;
  `DestroyIcon` runs on every replace and on teardown. The stock fallback
  (`IDI_APPLICATION`) is shared and is never destroyed (cell stays null).
- **Naming:** the folder/README say "DeskTag" but the crate and binary are
  `desktag` (lowercase); the window class is `DeskTagBadgeClass`.

## Environment

- Windows 11 24H2 (build 26100.2605+). `winvd` binds undocumented
  virtual-desktop COM interfaces that shift between Windows builds; the `0.0.x`
  pin in `Cargo.lock` is build-sensitive.
- Rust stable. No toolchain pin, no CI, no fmt/clippy config.

## Reference

- `docs/superpowers/specs/`, `docs/superpowers/plans/` — badge design and
  implementation notes.
- `.agents/skills/` — vendored Rust skills (best-practices, testing, patterns,
  async-patterns).
