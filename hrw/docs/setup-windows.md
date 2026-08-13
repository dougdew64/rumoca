# Setting up HRW on a fresh Windows machine

**Purpose:** every step from a bare Windows box to a running HRW, including the live-trace
debugger setup.
**Status:** procedure. Verified on Windows 11 with the MSVC toolchain.
**Read when:** setting up a new machine, or when something that used to work stops. Steps 1-5
get a running app; **steps 6-8 are needed only for live trace debugging**, which is the
feature with real environmental requirements.

*Split out of `../README.md` on 2026-08-01. It was 208 of that file's 223 lines, which left no
room for the README to say what HRW is.*

**Why each precaution exists** is in [`architecture.md`](architecture.md); this page is the
*what to type*. The failure-signature table for live trace is in
[`architecture.md` § Live trace debugging on Windows](architecture.md#live-trace-debugging-on-windows)
— read that before changing anything in this area.

---

## 1. Rust

```powershell
winget install Rustlang.Rustup
```

The workspace pins its toolchain in `rust-toolchain.toml` (nightly, plus a
`wasm32-unknown-unknown` target inherited from upstream Rumoca — HRW itself is native-only).
Rustup installs the pinned toolchain automatically on first build; no manual `rustup default`
needed.

You need the **MSVC** toolchain (the rustup default on Windows), which implies the Visual
Studio Build Tools with the C++ workload. **If linking fails with a missing `link.exe`, that
is what to install.**

## 2. Clone

```powershell
git clone https://github.com/dougdew64/rumoca.git
cd rumoca
git checkout hrw
```

**Line endings are handled for you, but check if tests fail oddly.**
[`../.gitattributes`](../.gitattributes) pins `eol=lf` for everything under `hrw/`, because
Git for Windows ships `core.autocrlf=true` in its **system** config
(`C:/Program Files/Git/etc/gitconfig`) and a CRLF checkout breaks the tests that read
repository files as exact text. If a clone predates that file, the symptom is two failures —
`tour_catalogue_is_current` and `app_does_not_regrow_its_field_count`, the second of which
**reports a false reason** (*"the App struct must be closed by a `}` at column 0"*, when the
struct is fine and the line is `}\r`). The repair:

```powershell
git config core.autocrlf false
git rm --cached -r -q .
git reset --hard
```

*Encountered 2026-08-07 on a second Windows machine; the clone was otherwise perfect.*

## 3. Stage the Modelica Standard Library (required — not in the clone)

`hrw/vendor/` is gitignored, so **a fresh clone has no MSL and specimens will fail to
compile.** HRW expects reference **MSL 4.1.0** at `hrw/vendor/msl/`, laid out exactly as the
upstream release:

```
hrw/vendor/msl/
├── Modelica 4.1.0/
├── ModelicaServices 4.1.0/
├── ModelicaReference 4.1.0/
├── Complex.mo
├── ObsoleteModelica4.mo
├── LICENSE
└── README.md
```

Get it either by copying `hrw/vendor/` from a machine that already has it, or by downloading
the v4.1.0 release from
[modelica/ModelicaStandardLibrary](https://github.com/modelica/ModelicaStandardLibrary/releases)
and arranging it as above. **The directory names include the version and are matched
literally** — see the library list in [`../src/app.rs`](../src/app.rs).

## 4. Build and test

```powershell
cargo build -p hrw

# Between edits — 431 tests, about 8 seconds.
cargo test -p hrw --lib -- --test-threads=1

# Before committing — all 491, about 2 minutes.
cargo test -p hrw --lib --features slow-tests -- --test-threads=1

# Covers the binary, which `cargo test` does NOT build.
cargo clippy -p hrw --all-targets
```

*(Counts and timings measured 2026-08-01, after `ideas.md` #48 memoised compiled specimens: the full run was 375s before it.)*

**Two commands, because 59 tests hold nearly all the runtime.** Measured 2026-07-29: 49 of
402 tests took 180 of the suite's 183 seconds, nearly all of them compiling a specimen against
the MSL. They are gated behind the `slow-tests` feature so the loop between edits stays fast;
`cargo test` lists them as ignored *with a reason*, so **a skipped test never looks like a
passing one**. Run the full command before every commit — the fast suite deliberately does not
cover compilation.

Parallelism would not help: those tests all acquire a global `Mutex<WorkerState>` (Rumoca's
`Session` is not thread-safe) and serialize whatever `--test-threads` says. The fix that would
shorten the full run is memoizing compiled specimens (`ideas.md` #48).

**`cargo test` does not build the binary**, and that gap is not theoretical — on 2026-07-31 a
misplaced `#[cfg(test)]` let two helpers compile into `--bin hrw` referencing test-only
imports. Every test passed; the debugger launch failed. `cargo clippy --all-targets` covers
the bin — **check its exit code, not its output**; the same breakage survived a clippy run
piped to `grep -c "^warning: "`, which counts warnings and ignores a compile error.

**`--test-threads=1` is required.** Two independent causes, both of which reproduce on a clean
checkout:

1. Tests that exercise the Claude bridge and the breakpoint pre-warm share single files under
   `.hrw-bridge/`, so in parallel they race each other.
2. `worker::tests::output_capture_handles_large_write_without_deadlock` redirects
   **process-global** stdout, so any concurrently-running test that writes to stdout steals
   bytes from it.

Without the flag the suite does not merely fail — **it can also hang**, which looks like a
broken build rather than a test-isolation problem.

## 5. Run

```powershell
cargo run -p hrw
```

Add `--half` to open at half screen width, for working side by side with VS Code.

---

# Live trace debugging (steps 6-8)

The third and most demanding animation tier: the real algorithm runs on a worker thread,
pauses in the debugger at each step, and the animation advances in lockstep. Because it runs
the real binary under a real debugger, **it depends on the toolchain in ways that are
invisible from the source.**

## 6. VS Code extensions

| Extension | Id | Why |
|-----------|-----|-----|
| C/C++ | `ms-vscode.cpptools` | **Required.** Provides the `cppvsdbg` adapter — the working debug config on windows-msvc. Install the base extension only, *not* the Extension Pack. |
| rust-analyzer | `rust-lang.rust-analyzer` | Language support |
| CodeLLDB | `vadimcn.vscode-lldb` | *Optional.* The alternative adapter, with Rust-aware formatters. Retained; see the caveat below. |

**Use the `Debug HRW Observatory (cppvsdbg)` launch config.** Verified 2026-07-28: breakpoints
in the path-dep `crates/rumoca-*` bind and fire, and live-trace stepping advances the animation
with plain **F10** — no Debug Console aliases needed. That last point was a surprise:
all-threads stepping was assumed to be a CodeLLDB feature, when in fact **LLDB defaults to
stepping one thread and must be told otherwise**, while the Visual Studio debugger already runs
all threads on a step.

If you use CodeLLDB instead, type `ns` / `si` / `so` in the Debug Console rather than pressing
F10/F11, or the animation will not advance. A note in `launch.json` records CodeLLDB
mis-binding breakpoints in path-dep crates; **that note is unverified** — it predates the
discovery of Rumoca's compile cache, which produces a near-identical symptom (see
[`architecture.md`](architecture.md) §§ 4-5).

## 7. The HRW Debugger Bridge extension

HRW arms its own breakpoints by writing request files that a small VS Code extension picks up.
Its `out/` directory is gitignored, so **it must be built after cloning** or the Debug button
does nothing:

**It needs Node.js, which nothing else in this project does** — so a machine that runs HRW
perfectly can still stop dead here with `npm: command not found`. Install it first, then use a
**new shell** so `npm` is on `PATH`:

```powershell
winget install OpenJS.NodeJS.LTS
```

```powershell
cd hrw\vscode-extension
npm install
npm run build
npm test
```

```powershell
New-Item -ItemType Junction -Path "$env:USERPROFILE\.vscode\extensions\dougdew64.hrw-debugger-bridge-0.1.0" -Target "$PWD"
```

Reload VS Code afterwards, and confirm with `code --list-extensions` — it should list
`dougdew64.hrw-debugger-bridge`. The extension logs to the "HRW Bridge" output channel; a
working arm shows `Armed: live_trace.rs:<line>`.

> **This step said `code --install-extension hrw\vscode-extension` until 2026-08-07**, and that
> command does not work: `--install-extension` takes a **marketplace ID or a `.vsix`**, and
> VS Code 1.126.0 answers *"Extension 'hrw\vscode-extension' not found"*. The junction installs
> the folder directly, and has the further advantage that `npm run build` then updates the
> installed extension in place — no reinstall after an edit, only a window reload. Junctions
> need no administrator rights; symlinks do, unless Developer Mode is on.
>
> *Both the Node.js prerequisite and the broken install command were found the first time this
> page was followed on a genuinely bare machine.*

## 8. Claude Code's permission allowlist — per machine, and it does NOT travel

**`.claude/` is gitignored by upstream Rumoca** (not by us), so a permission allowlist cannot be
committed. On a fresh clone every Bash call prompts for approval, and Doug reported that as real
friction while walking tours: *"the high latency of your answers… seems to be caused by you asking
for my approval to perform tasks."* **This section is the durable record, since the file itself
cannot be** — the same reason `working-with-doug.md` exists.

Create `.claude/settings.json` at the **repository root**:

```json
{
  "permissions": {
    "allow": [
      "Bash(cargo test -p hrw --lib *)",
      "Bash(cargo clippy -p hrw --all-targets)",
      "Bash(rustfmt --edition 2024 --check *)"
    ]
  }
}
```

**Why only these three.** Everything else Claude runs often is either already auto-allowed by
Claude Code (`grep`, `sed -n`, `cat`, `echo`, `git status`, `git diff`) or genuinely mutating and
*should* keep asking — `git push`, `git commit`, and `cargo run --example gen_*`, which rewrites
`architecture.md` and `CATALOGUE.md`.

**The one judgement call, recorded so it can be revisited rather than rediscovered:** `cargo test`
executes code, and the general rule is not to allowlist patterns permitting arbitrary execution. It
is scoped to `-p hrw --lib` — this crate's own library tests, which are built and run constantly
anyway — because it was **40 of 418 observed calls**, the single largest source of prompts. Narrow
it or drop it if that trade stops being worth it.

## 9. Launch

Open **the repository root** as the VS Code folder (not `hrw/`), and launch **"Debug HRW
Observatory"** from the launch-configuration dropdown.

> **Launch from the dropdown, not from rust-analyzer's "Debug" CodeLens.** The CodeLens builds
> its own configuration and ignores `launch.json`, so it gets neither the GPU backend override
> nor the stepping aliases — live trace will crash or misbehave under it.

`.vscode/launch.json` carries settings that are not optional:

| Setting | Why |
|---------|-----|
| `WGPU_BACKEND=gl` | A D3D12 device does not survive the long pauses live trace depends on. Without this, HRW dies with exit code 101 and an `egui-wgpu` staging-buffer panic. |
| `_NO_DEBUG_HEAP=1` | Windows switches the CRT to the validating debug heap under a debugger, making sessions ~100x slower. |
| `RUST_BACKTRACE=1` | Panic output goes to the debuggee's stderr — the **integrated terminal**, not the Debug Console. |
| `initCommands` aliases | All-threads stepping (below). |

Note these files are force-added: the repo root's `.gitignore` excludes `.vscode/`, so new
files there need `git add -f` to be tracked.

## Using it

1. Launch under the debugger, load a specimen, open the Structural Analysis or Index Reduction
   tab.
2. Open a **Matching**, **Tarjan**, or **Reduction** animation view and click **Debug**.
3. Execution stops at the anchor with `frame_index = 18446744073709551615` (`usize::MAX`) —
   the startup gate, before any algorithm work.
4. **Continue (F5)** advances one algorithm step per press, and the animation follows.
5. To step through Rumoca's code *and* keep the animation live under CodeLLDB, type these in
   the **Debug Console** rather than pressing F10/F11:

   | Alias | Equivalent |
   |-------|-----------|
   | `ns` | `thread step-over -m all-threads` |
   | `si` | `thread step-in -m all-threads` |
   | `so` | `thread step-out -m all-threads` |

   VS Code's step buttons move only the selected thread, which freezes the UI thread and
   leaves the animation stale.

## If something goes wrong

| Symptom | Cause |
|---------|-------|
| Debug button does nothing; "HRW Bridge" channel silent | Extension not built or not installed (step 7) |
| Breakpoint lands in an unrelated crate during startup | The anchor has been folded onto another empty function — see [`architecture.md`](architecture.md) §1. Do not simplify `live_trace_breakpoint`. |
| Locals show `<optimized out>` | The crate under study needs `opt-level = 0` in the workspace `Cargo.toml` |
| Visuals freeze, HRW exits with code 101 | GPU device loss — confirm `WGPU_BACKEND=gl` is in effect and that you launched from the dropdown |
| Specimens fail to compile | MSL not staged (step 3) |
| Tests fail intermittently, or hang | Missing `--test-threads=1` |
| A feature behaves as it did *before* a change you know landed | **The running HRW is holding `hrw.exe`, so the last build never relinked.** See below. |

### `Access is denied. (os error 5)` — a stale binary that looks current

**Windows locks a running executable.** With HRW open, `cargo build -p hrw` compiles
everything, fails at the *link* step with `Access is denied. (os error 5)`, and leaves the
**previous `hrw.exe` in place**. The library and the whole test suite still build, so
`cargo test` passes against code the running app does not contain.

**This is the trap:** the failure is one line at the end of a long build, and everything else
about the run looks healthy.

It cost a troubleshooting cycle on 2026-08-03 — a tour-link click that "did nothing" and could
not be reproduced by three headless tests. Restarting HRW resolved it, and the bug was never
found in the code, which is the signature of this rather than of a defect.

**So when a symptom does not match the source, check the binary before reading the code:**

```powershell
Get-Item target/debug/hrw.exe | Select-Object LastWriteTime   # older than your edit?
Get-Process hrw | Select-Object Id, StartTime                 # started before that build?
```

Close HRW, rebuild, confirm the timestamp moved, and only then start diagnosing.

## Before a long run: stop rust-analyzer

Command Palette → **"rust-analyzer: Stop server"**. It holds ~5.7 GB here, and this workspace
is near its worst case: 173k lines of our own code against **989 dependency packages and
642 MB of third-party source**. **Do not kill the process** — VS Code treats that as a crash
and restarts it within seconds. See [`long-runs.md`](long-runs.md).
