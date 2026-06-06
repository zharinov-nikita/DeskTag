# Pre-commit Hook via cargo-husky — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Block any commit that would introduce unformatted code, clippy warnings, or failing tests, before the commit lands.

**Architecture:** Add `cargo-husky` as a dev-dependency in user-hook mode. A versioned POSIX-sh script at `.cargo-husky/hooks/pre-commit` runs `cargo fmt --check`, `cargo clippy -D warnings`, then `cargo test`. cargo-husky's build script copies the script into `.git/hooks/` when dev-dependencies compile (i.e. on `cargo test`).

**Tech Stack:** Rust, cargo, cargo-husky 1.x (dev-dependency), git hooks, POSIX sh (Git for Windows MSYS).

**Spec:** `docs/superpowers/specs/2026-06-07-pre-commit-cargo-husky-design.md`

---

## Pre-flight

This plan runs on a feature branch, not `main`. Before Task 1, ensure an isolated
workspace exists (worktree or branch) — e.g. `git checkout -b feature/pre-commit-cargo-husky`.

**Versioned files** (get committed): `Cargo.toml`, `Cargo.lock`,
`.cargo-husky/hooks/pre-commit`, `CLAUDE.md`.
**Not versioned** (`.gitignore` ignores `superpowers/`): the spec and this plan.

## File Structure

- `Cargo.toml` — add `[dev-dependencies.cargo-husky]` (the only manifest change).
- `Cargo.lock` — auto-updated by cargo when cargo-husky resolves; commit it.
- `.cargo-husky/hooks/pre-commit` — NEW. The hook script (single responsibility:
  run the three checks fail-fast). Source of truth, versioned.
- `.git/hooks/pre-commit` — generated copy (NOT versioned, NOT edited by hand).
- `CLAUDE.md` — docs: note the hook + activation step.

---

## Task 1: Add cargo-husky dev-dependency

**Files:**
- Modify: `Cargo.toml` (append a new section after the existing `[build-dependencies]`)

- [ ] **Step 1: Add the dev-dependency block**

Append to `Cargo.toml`:

```toml
[dev-dependencies.cargo-husky]
version = "1"
default-features = false
features = ["user-hooks"]
```

`default-features = false` disables cargo-husky's defaults (`prepush-hook`,
`run-cargo-test`, `run-for-all`) so ONLY the user hook installs. `user-hooks`
tells cargo-husky to copy `.cargo-husky/hooks/*` into `.git/hooks/`.

- [ ] **Step 2: Verify the manifest resolves**

Run: `cargo metadata --format-version 1 --no-deps`
Expected: exits 0, prints JSON (no manifest/parse error). This confirms the new
section is syntactically valid before we build anything heavy.

---

## Task 2: Create the hook script

**Files:**
- Create: `.cargo-husky/hooks/pre-commit`

- [ ] **Step 1: Write the script**

Create `.cargo-husky/hooks/pre-commit` with EXACTLY this content (LF line
endings, no trailing CRLF — MSYS `sh` needs LF):

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

- [ ] **Step 2: Verify the file is LF-terminated**

Run: `git diff --no-index --stat /dev/null .cargo-husky/hooks/pre-commit` (or open
in an editor showing line endings).
Expected: no `^M`/CRLF. If CRLF crept in, re-save as LF. A CRLF shebang makes
MSYS sh fail with `/bin/sh^M: bad interpreter`.

---

## Task 3: Activate the hook and confirm it installs

cargo-husky installs the hook from its build script, which runs when
dev-dependencies compile — that is on `cargo test`. The first `cargo test` here
also compiles `winvd`/`windows` and is SLOW; that is expected, one-time.

- [ ] **Step 1: Pre-clean the tree so our own feature commit will pass the hook**

The feature commit in Task 4 will itself trigger the hook, so the tree must be
clean first. Run each and fix any reported issue before continuing:

Run: `cargo fmt --all`
Run: `cargo clippy --all-targets -- -D warnings`
Expected: clippy exits 0. If it reports warnings on existing code, fix them now
(or the feature commit in Task 4 will be blocked).

- [ ] **Step 2: Run cargo test to install the hook**

Run: `cargo test`
Expected: builds (slow first time), tests PASS, exits 0.

- [ ] **Step 3: Confirm the hook landed in .git/hooks**

Run: `cat .git/hooks/pre-commit`
Expected: shows our script body (cargo-husky prepends a generated marker comment
line such as `# This hook was set by cargo-husky ...`, followed by our
`cargo fmt`/`clippy`/`test` lines). If the file is absent, cargo-husky did not
run — confirm `default-features = false` + `features = ["user-hooks"]` and that
`cargo test` actually recompiled (try `cargo clean -p cargo-husky` then
`cargo test`).

---

## Task 4: Commit the feature (positive acceptance — clean tree passes the hook)

This commit is the positive acceptance test: a clean tree must commit
successfully WITH the hook active.

**Files:**
- Add: `Cargo.toml`, `Cargo.lock`, `.cargo-husky/hooks/pre-commit`

- [ ] **Step 1: Stage the versioned files**

Run: `git add Cargo.toml Cargo.lock .cargo-husky/hooks/pre-commit`

- [ ] **Step 2: Commit (the hook runs now)**

```bash
git commit -m "feat: pre-commit hook (fmt, clippy -D warnings, test) via cargo-husky"
```

Expected: hook prints `[pre-commit] cargo fmt --check`, `[pre-commit] cargo
clippy -D warnings`, `[pre-commit] cargo test`, all pass, commit is created.
This proves acceptance test #4 (clean code → commit succeeds).

---

## Task 5: Negative acceptance tests (each broken state blocks the commit)

Each sub-test: introduce one defect on disk, stage, attempt an empty-message
throwaway commit, confirm the hook BLOCKS at the expected stage, then revert
fully. The hook checks the working tree on disk, so the defect just needs to
exist on disk; staging is only needed so `git commit` has something to start.

- [ ] **Step 1: fmt failure blocks (acceptance #1)**

Append a badly-formatted (but valid) line to `src/label.rs`:

```rust
pub fn _fmt_probe(){let _x=1;}
```

Run:
```bash
git add src/label.rs
git commit -m "probe: should be blocked by fmt"
```
Expected: FAILS at `[pre-commit] cargo fmt --check` (non-zero), NO commit created.

Revert:
```bash
git reset HEAD src/label.rs
git checkout -- src/label.rs
```
Confirm reverted: `git status --short` shows clean for `src/label.rs`.

- [ ] **Step 2: clippy failure blocks (acceptance #2)**

Append a properly-formatted function with a clippy lint to `src/label.rs`
(fmt must pass so the hook reaches the clippy stage). `v.len() == 0` triggers
`clippy::len_zero`, an error under `-D warnings`:

```rust
pub fn _clippy_probe(v: &[u8]) -> bool {
    v.len() == 0
}
```

Run:
```bash
git add src/label.rs
git commit -m "probe: should be blocked by clippy"
```
Expected: fmt PASSES, then FAILS at `[pre-commit] cargo clippy -D warnings` with
a `len_zero` / `is_empty()` error, NO commit created.

Revert:
```bash
git reset HEAD src/label.rs
git checkout -- src/label.rs
```

- [ ] **Step 3: test failure blocks (acceptance #3)**

Append a failing test to `src/label.rs` (fmt + clippy pass; the test fails):

```rust
#[test]
fn _probe_failing() {
    assert_eq!(1, 2);
}
```

Run:
```bash
git add src/label.rs
git commit -m "probe: should be blocked by test"
```
Expected: fmt + clippy PASS, then FAILS at `[pre-commit] cargo test` with
`_probe_failing` failing, NO commit created.

Revert:
```bash
git reset HEAD src/label.rs
git checkout -- src/label.rs
```

- [ ] **Step 4: --no-verify bypasses (acceptance #5)**

Re-add the fmt defect from Step 1 (`pub fn _fmt_probe(){let _x=1;}`) to
`src/label.rs`, then:

```bash
git add src/label.rs
git commit --no-verify -m "probe: bypass"
```
Expected: NO hook output, commit IS created (bypass works).

Revert the throwaway commit AND the defect:
```bash
git reset --hard HEAD~1
```
Confirm: `git log --oneline -1` shows the Task 4 feature commit (not "probe:
bypass"), and `git status --short` is clean.

---

## Task 6: Document in CLAUDE.md

**Files:**
- Modify: `CLAUDE.md` (the `## Commands` section and the `## Gotchas` section)

- [ ] **Step 1: Add a Commands note**

In `CLAUDE.md`, immediately AFTER the closing ``` of the `## Commands` code
block, add:

```markdown
A git **pre-commit** hook (via `cargo-husky`, dev-dependency) blocks commits on
`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, or `cargo test`
failures. It self-installs into `.git/hooks/` the first time you run `cargo test`.
Bypass deliberately with `git commit --no-verify`.
```

- [ ] **Step 2: Add a Gotchas bullet**

In `CLAUDE.md`, append to the `## Gotchas` list:

```markdown
- **Pre-commit hook installs via `cargo test`.** `cargo-husky` (user-hook mode,
  `default-features = false`, `features = ["user-hooks"]`) copies
  `.cargo-husky/hooks/pre-commit` into `.git/hooks/` from its build script, which
  runs only when dev-dependencies compile — i.e. on `cargo test`, NOT plain
  `cargo build`. The script must keep LF line endings (MSYS `sh`). Fresh clones
  get the hook on their first `cargo test`.
```

- [ ] **Step 3: Commit the docs**

```bash
git add CLAUDE.md
git commit -m "docs: document pre-commit hook in CLAUDE.md"
```

Expected: hook runs (tree still clean) and passes; commit created.

---

## Done criteria

- `.git/hooks/pre-commit` exists and contains the three checks.
- Clean tree commits succeed (Task 4); fmt/clippy/test defects each block at the
  right stage (Task 5 Steps 1-3); `--no-verify` bypasses (Step 4).
- `Cargo.toml`, `Cargo.lock`, `.cargo-husky/hooks/pre-commit`, `CLAUDE.md`
  committed on the feature branch. No `probe:` commits remain in history.
