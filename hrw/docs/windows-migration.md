# Windows Migration — Status and Handoff

Migrated HRW development from WSL2 to native Windows (2026-07-26/27).
This document captures migration status and open issues for continuity
across the platform change.

## Migration status

### Working

- **Build:** `cargo build -p hrw` succeeds on Windows (MSVC toolchain)
- **Tests:** all 270 tests pass with `cargo test -p hrw -- --test-threads=1`
  (parallel execution hits the same bridge filesystem races as WSL2)
- **App:** runs, UI faster and sharper than WSL2
- **Debugger:** CodeLLDB on Windows uses Windows Debug API (no ptrace),
  no step-over/continue deadlocks (the WSL2 ptrace deadlock is gone)
- **Debug heap:** `_NO_DEBUG_HEAP=1` in `.vscode/launch.json` prevents
  the MSVC debug heap from destroying performance under the debugger
- **VS Code extension:** `hrw-debugger-bridge` installed, activating,
  watching `.hrw-bridge/` for breakpoint requests
- **MSL vendor:** manually copied from WSL2 (gitignored, not in clone)

### RESOLVED (2026-07-27): Debug button bridge breakpoint not hit

**The bridge was never broken.** The breakpoint anchor was being folded onto
other functions by the linker, so breakpoints on it resolved to an unrelated
address. Full explanation, with the failure chain and the diagnostic commands,
is in [`architecture.md` § Live trace debugging on Windows](architecture.md#live-trace-debugging-on-windows).
Setup instructions for a fresh clone are in [`../README.md`](../README.md).

Summary of the chain: `LAST_FRAME_INDEX` was written by `live_trace_breakpoint`
and read nowhere, so at `[profile.dev] opt-level = 1` the store was
dead-store-eliminated; the function became a bare `ret`; the MSVC linker's
`/OPT:ICF` merged it with every other empty function in the binary (notably
eframe's `App::raw_input_hook`). A breakpoint on the anchor therefore fired from
eframe's render loop, reported — correctly — as "Paused on breakpoint" at
`epi.rs:273`. `#[inline(never)]` does not prevent this; it protects the function,
not the body.

**Hypotheses pursued and discarded along the way** (recorded so they are not
re-tried): the `\\?\` path prefix; ack-handshake timing; CodeLLDB's PDB reader
(cppvsdbg reproduced it identically, which is what proved the fault was in the
binary); source-file identity confusion; a `FunctionBreakpoint` rewrite (dead on
arrival — symbol resolution pointed at the folded address too).

**Fixes applied:**

1. `live_trace_breakpoint` given a body that survives optimization — a real
   reader (`last_frame_index`) plus `black_box`. Regression test:
   `breakpoint_anchor_store_is_observable`.
2. `[profile.dev.package.rumoca-phase-structural] opt-level = 0` — restores
   dense line tables and readable locals (`frame_index` had been `<optimized out>`).
3. `WGPU_BACKEND=gl` in both launch configs — a D3D12 device does not survive
   the long pauses live trace depends on; the loss surfaced as an `egui-wgpu`
   staging-buffer panic on the main thread and exit code 101.
4. Breakpoint pre-warm (`App::tick_prewarm`) — the first breakpoint in a source
   file costs a cold line-table load, longer than the 500 ms startup gate, so the
   first Debug click of a session missed. HRW now arms and removes the anchor
   once at startup, moving that cost off the critical path.
5. All-threads stepping aliases (`ns`/`si`/`so`) committed to `.vscode/launch.json`.
   These previously lived in an untracked `~/.lldbinit` on the Linux machine.
   **Not yet verified end to end.**

### Note: `.vscode/` is gitignored at the repo root

`.gitignore:18` excludes `.vscode/`, so `launch.json`, `tasks.json`, and
`settings.json` there were invisible to git and would not survive a clone. They
are now tracked via `git add -f` — the same way `hrw/.vscode/` is tracked. This
avoids modifying upstream's `.gitignore`, which keeps the rebase workflow clean.
**Any new file under a `.vscode/` directory needs `git add -f`.**

### Platform-specific code

- **`hrw/src/worker.rs`:** `OutputCapture` uses three `#[cfg(unix)]`/`#[cfg(windows)]`
  platform primitives (`create_pipe`, `file_from_raw_fd`, `write_to_fd` test helper)
- **`hrw/src/bridge.rs`:** `strip_windows_prefix` (`#[cfg(windows)]`) strips
  `\\?\` from canonicalized paths
- **`hrw/Cargo.toml`:** `libc` is unconditional (needed for both platforms)
- **`launch.json` (both `.vscode/` and `hrw/.vscode/`):** carry `"env": {"_NO_DEBUG_HEAP": "1"}`
  on the "Debug HRW Observatory" config. The WSL2-era `preRunCommands` entry
  (`process handle SIGCHLD -s false -n false`) was **removed** — SIGCHLD is a
  UNIX signal with no meaning on a Windows target, and a failing
  `preRunCommands` entry aborts a CodeLLDB launch. The two files are duplicates
  serving different opened-folder choices (repo root vs `hrw/`); keep them in sync.
- **Workspace `Cargo.toml`:** per-crate opt-level overrides are back to the
  original four only (parse=3, compile=2, parol_runtime=3, scnr2=3) — the
  pipeline crate overrides were reverted because `_NO_DEBUG_HEAP=1` was the
  real fix

### WSL2-specific issues that no longer apply on Windows

- **LLDB deadlock on step-over/continue:** caused by ptrace on WSL2; Windows
  uses Windows Debug API, no deadlocks observed
- **Debug heap performance:** `_NO_DEBUG_HEAP=1` in launch.json eliminates the
  MSVC debug heap that makes debugger sessions 100x slower

## Setup checklist for a fresh Windows clone

> **Superseded by [`../README.md`](../README.md)**, which is maintained as the
> setup guide. The list below is kept as the historical migration record.

1. Install Rust (MSVC toolchain): `winget install Rustlang.Rustup`
2. Clone and checkout: `git clone ... && git checkout hrw`
3. Copy MSL vendor directory (gitignored):
   `Copy-Item -Recurse "\\wsl$\Ubuntu-24.04\home\dougdew\dev\rumoca\hrw\vendor" hrw\vendor`
   (or from any machine that has it)
4. Build: `cargo build -p hrw`
5. Test: `cargo test -p hrw -- --test-threads=1`
6. Build VS Code extension:
   `cd hrw\vscode-extension && npm install && npm run build`
7. Install extension: `code --install-extension hrw\vscode-extension`
8. Restart VS Code to pick up extension + PATH changes

`_NO_DEBUG_HEAP=1` is already committed in both `launch.json` files — no manual
step needed.
