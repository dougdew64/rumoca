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

### Broken: Debug button bridge breakpoint not hit

The Debug button (matching/tarjan/reduction animation views) completes the
full handshake — the extension arms the breakpoint and writes the ack, HRW
spawns the algorithm thread — but LLDB does not stop at the breakpoint.
The algorithm runs to completion and the `on_complete` callback removes it.

**Diagnosis so far:**

1. Extension wasn't installed (fixed: `npm install && npm run build` in
   `hrw/vscode-extension/`, since `out/` is gitignored)
2. `std::fs::canonicalize` on Windows produces `\\?\C:\...` extended-length
   paths (fixed in commit 0ffb28d8: `strip_windows_prefix` in `bridge.rs`)
3. Despite the prefix fix, the breakpoint is still not hit. The extension
   log shows `Armed: live_trace.rs:120` then `Removed: live_trace.rs:120`
   (no pause in between).

**Next diagnostic steps:**

- Add the full path to the extension's log output (`handleAdd` in
  `vscode-extension/src/extension.ts`) to verify the path the extension
  passes to `vscode.debug.addBreakpoints`
- Check whether the breakpoint appears in VS Code's Breakpoints panel
  when the Debug button is clicked (it should appear briefly)
- Manually set a breakpoint on `live_trace_breakpoint` in `live_trace.rs:120`
  and verify LLDB hits it (isolates whether the issue is path-matching vs
  dynamic breakpoint addition)
- Check CodeLLDB's LLDB output for breakpoint resolution messages
- The `wait_for_debugger` 500ms sleep may be insufficient on Windows;
  try increasing it

**Workaround:** manually set a breakpoint on `live_trace_breakpoint`
(`crates/rumoca-phase-structural/src/live_trace.rs:120`) before clicking
Debug, or use Recompile with a pre-set breakpoint.

### Platform-specific code

- **`hrw/src/worker.rs`:** `OutputCapture` uses three `#[cfg(unix)]`/`#[cfg(windows)]`
  platform primitives (`create_pipe`, `file_from_raw_fd`, `write_to_fd` test helper)
- **`hrw/src/bridge.rs`:** `strip_windows_prefix` (`#[cfg(windows)]`) strips
  `\\?\` from canonicalized paths
- **`hrw/Cargo.toml`:** `libc` is unconditional (needed for both platforms)
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
8. Add to `.vscode/launch.json`: `"env": {"_NO_DEBUG_HEAP": "1"}`
9. Restart VS Code to pick up extension + PATH changes
