# Pre-commit hook via cargo-husky — Design

**Date:** 2026-06-07
**Status:** Approved (design)

## Goal

Block a commit when the working tree would introduce broken code, so problems
are caught before the commit lands (and therefore before any push). "Broken"
means any of: unformatted code, clippy lints, or failing tests.

## Mechanism

Use the `cargo-husky` crate in **user-hook** mode. It is a dev-dependency whose
build script copies hook scripts from `.cargo-husky/hooks/` into `.git/hooks/`
when the dev-dependencies are compiled. This keeps the hook versioned in git and
self-installing for every clone — no manual `git config` step, no external
runtime (Python, Node).

User-hook mode is chosen over cargo-husky's feature-based mode because the
feature-based clippy hook does **not** pass `-D warnings`; warnings would not
block the commit, leaving a hole under the stated goal. A user-authored script
gives full control over flags, order, and output.

## Files

### `Cargo.toml`

Add a dev-dependency:

```toml
[dev-dependencies.cargo-husky]
version = "1"
default-features = false
features = ["user-hooks"]
```

`default-features = false` disables cargo-husky's default `prepush-hook` +
`run-cargo-test` behavior, so only the user hook is installed.

### `.cargo-husky/hooks/pre-commit` (new, versioned)

POSIX-sh script, cheap→expensive ordering, fail-fast:

```sh
#!/bin/sh
set -e
echo "[pre-commit] cargo fmt --check"
cargo fmt --all -- --check
echo "[pre-commit] cargo clippy -D warnings"
cargo clippy --all-targets -- -D warnings
echo "[pre-commit] cargo test"
cargo test
```

- `set -e` aborts on the first non-zero stage.
- `fmt --all -- --check` is near-instant and runs first, so a formatting miss
  fails without paying for a build.
- `clippy --all-targets -- -D warnings` promotes every warning to an error.
- `cargo test` compiles and runs the unit tests last.

## Installation / activation

The hook is installed by cargo-husky's build script, which runs when the
dev-dependencies are compiled — i.e. on **`cargo test`** (not plain
`cargo build`, since cargo-husky is a dev-dependency). Activation step after this
change: run `cargo test` once. Future clones get the hook automatically the first
time anyone runs `cargo test`.

## Behavior / error handling

- Any failing stage → non-zero exit → commit aborted; the echoed stage name
  shows which check failed.
- Deliberate bypass: `git commit --no-verify`.
- Requires the `rustfmt` and `clippy` rustup components (present in a standard
  stable toolchain).

## Platform (Windows)

Git for Windows runs hooks through its bundled MSYS `sh`, so the `#!/bin/sh`
shebang works and `cargo` is found on PATH. The script is plain POSIX sh — no
PowerShell, no bashisms.

## Scope / non-goals

- `cargo build --release` is intentionally **excluded** — the release build of
  `windows`/`winvd` is slow and belongs in CI / a pre-push hook, not on every
  commit.
- No CI changes (project has no CI today).

## Acceptance tests (for the implementation plan)

1. Unformatted code staged → commit blocked at the fmt stage.
2. Code with a clippy warning → commit blocked at the clippy stage.
3. A failing unit test → commit blocked at the test stage.
4. Clean code → commit succeeds.
5. `git commit --no-verify` bypasses all checks.

## Documentation updates

- `CLAUDE.md` → **Commands**: note the pre-commit hook and the one-time
  `cargo test` activation step.
- `CLAUDE.md` → **Gotchas**: note that the hook installs via cargo-husky's build
  script and is triggered by `cargo test`, not `cargo build`.
