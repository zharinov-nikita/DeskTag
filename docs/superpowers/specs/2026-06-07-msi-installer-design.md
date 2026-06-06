# MSI Installer for DeskTag — Design

**Date:** 2026-06-07
**Status:** Approved (brainstorming)

## Goal

Ship a Windows `.msi` installer for DeskTag so users can install it like a
normal program: into `Program Files`, with Start-menu shortcut, an
"Add/Remove Programs" entry, and automatic start at logon. The installer is
built and published by GitHub Actions when a release tag is pushed.

## Decisions (from brainstorming)

| Question | Decision |
|----------|----------|
| Installer format | **MSI** via `cargo-wix` (WiX Toolset v3) |
| Autostart at logon | **Yes** — `HKLM` `Run` registry value |
| Install scope | **Per-machine** (`Program Files`, requires UAC/admin) |
| Build | **GitHub Actions CI** on tag push |
| Tag / release | New tag **`v1.0.1`**; CI creates the release, attaches `.exe` + `.msi` |
| Launch after install | **Yes** — finish-page checkbox launches `desktag.exe` |
| Product / shortcut name | **DeskTag** |

## Architecture

Three artifacts are added to the repo:

1. **`wix/main.wxs`** — the WiX source, generated once via `cargo wix init`
   then hand-customized. Committed so the GUIDs and behavior are stable across
   builds.
2. **`.github/workflows/release.yml`** — builds the release `.exe`, runs
   `cargo wix`, and publishes the GitHub Release with both assets.
3. **`Cargo.toml`** version bump `1.0.0` → `1.0.1` (cargo-wix reads the MSI
   `ProductVersion` from here).

`/target` stays git-ignored; no build artifacts are committed.

### MSI behavior (per-machine)

- Installs `desktag.exe` into `C:\Program Files\DeskTag\`. UAC elevation
  required (`InstallScope=perMachine`).
- **Autostart:** a `RegistryValue` under
  `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Run` named `DeskTag` pointing
  at the installed `desktag.exe`. The daemon then starts for every user at
  logon (an accepted consequence of per-machine scope).
- **Start-menu shortcut** `DeskTag`, using `assets/desktag.ico`.
- **Add/Remove Programs** entry with the same icon → standard uninstall.

### Critical WiX details (correctness)

These are required or the installer breaks in normal use:

- **Close the running daemon on install / upgrade / uninstall.** DeskTag is a
  long-running single-instance daemon; while it runs, `desktag.exe` is locked,
  so an in-place upgrade would otherwise demand a reboot. Use
  `util:CloseApplication` (WiX `WixUtilExtension`) to terminate the
  `desktag.exe` process before files are replaced/removed.
- **Launch after install.** A custom action (finish-page checkbox) starts
  `desktag.exe` at the end of a fresh install so the badge appears without
  requiring a logoff/logon cycle. Autostart covers subsequent logons.
- **Stable `UpgradeCode` GUID** in `main.wxs` so future versions install as a
  *major upgrade* over the old one (old removed, new installed) rather than
  stacking duplicate entries.

### Versioning

`cargo-wix` derives the MSI `ProductVersion` from `Cargo.toml`. The release
flow is: bump `Cargo.toml` to `1.0.1` → commit → tag `v1.0.1` → push tag. The
tag and the embedded MSI version stay in sync.

### GitHub Actions workflow

`.github/workflows/release.yml`:

- **Trigger:** push of a tag matching `v*`.
- **Runner:** `windows-latest`.
- **Permissions:** `contents: write` (built-in `GITHUB_TOKEN`; no extra
  secrets).
- **Steps:**
  1. `actions/checkout`
  2. Install Rust stable toolchain.
  3. Ensure WiX Toolset v3 is available (install if the runner image lacks it)
     and `cargo install cargo-wix`.
  4. `cargo build --release`
  5. `cargo wix` → produces `target/wix/desktag-1.0.1-x86_64.msi`.
  6. Publish the release for the tag and attach **`desktag.exe`** +
     **`.msi`** via `softprops/action-gh-release`.

## Testing

- **Local:** install WiX v3 (winget) + `cargo-wix`; run `cargo wix` to produce
  the `.msi`; install it and verify: lands in `Program Files`, Start-menu
  shortcut works, daemon launches after install, badge survives logoff/logon
  (autostart), and uninstall removes everything (including the `Run` value)
  without a reboot while the daemon is running.
- **CI:** push `v1.0.1`; confirm the workflow is green and the release carries
  both `desktag.exe` and the `.msi`.

## Files changed / added

- `wix/main.wxs` (new)
- `.github/workflows/release.yml` (new)
- `Cargo.toml` (version `1.0.0` → `1.0.1`)

## Out of scope (YAGNI)

- Per-user MSI variant.
- WinGet manifest / package submission.
- Code signing of the `.exe`/`.msi` (no certificate available; SmartScreen
  warning on first run is accepted).
- Local-build release path (CI is the publish path).
