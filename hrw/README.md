# HRW Observatory

An egui application for **studying** the Rumoca Modelica compiler — a pipeline observatory that
makes each compiler phase's IR inspectable, animates the structural-analysis algorithms, and lets
you step through Rumoca's own code in a debugger while the animation follows along.

HRW is a workspace member of [`dougdew64/rumoca`](https://github.com/dougdew64/rumoca) (a fork of
`CogniPilot/rumoca`) on the **`hrw` branch**, depending on the compiler crates via path deps
(`../crates/rumoca-*`).

- **What it is and why** — [`docs/CHARTER.md`](docs/CHARTER.md), [`docs/vision.md`](docs/vision.md)
- **How it is built** — [`docs/architecture.md`](docs/architecture.md)
- **Design decisions** — [`DECISIONS.md`](DECISIONS.md)
- **Working agreements for Claude** — [`CLAUDE.md`](CLAUDE.md)

---

## Setting up on a fresh Windows machine

Everything here has been verified on Windows 11 with the MSVC toolchain. Steps 1–5 get you a
running app; steps 6–8 are needed only for **live trace debugging**, which is the feature with real
environmental requirements.

### 1. Rust

```powershell
winget install Rustlang.Rustup
```

The workspace pins its toolchain in `rust-toolchain.toml` (nightly, plus a `wasm32-unknown-unknown`
target inherited from upstream Rumoca — HRW itself is native-only). Rustup installs the pinned
toolchain automatically on first build; no manual `rustup default` needed.

You need the **MSVC** toolchain (the rustup default on Windows), which implies the Visual Studio
Build Tools with the C++ workload. If linking fails with a missing `link.exe`, that is what to
install.

### 2. Clone

```powershell
git clone https://github.com/dougdew64/rumoca.git
cd rumoca
git checkout hrw
```

### 3. Stage the Modelica Standard Library (required — not in the clone)

`hrw/vendor/` is gitignored, so **a fresh clone has no MSL and specimens will fail to compile.**
HRW expects reference **MSL 4.1.0** at `hrw/vendor/msl/`, laid out exactly as the upstream release:

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

Get it either by copying `hrw/vendor/` from a machine that already has it, or by downloading the
v4.1.0 release from [modelica/ModelicaStandardLibrary](https://github.com/modelica/ModelicaStandardLibrary/releases)
and arranging it as above. The directory names include the version and are matched literally — see
the library list in [`src/app.rs`](src/app.rs).

### 4. Build and test

```powershell
cargo build -p hrw
cargo test -p hrw -- --test-threads=1
```

**`--test-threads=1` is required.** Several tests exercise the Claude bridge and the breakpoint
pre-warm, which share single files under `.hrw-bridge/`; in parallel they race each other.

### 5. Run

```powershell
cargo run -p hrw
```

Add `--half` to open at half screen width, for working side by side with VS Code.

---

## Live trace debugging

The third and most demanding animation tier: the real algorithm runs on a worker thread, pauses in
the debugger at each step, and the animation advances in lockstep. Because it runs the real binary
under a real debugger, it depends on the toolchain in ways that are invisible from the source.

**[`docs/architecture.md` § Live trace debugging on Windows](docs/architecture.md#live-trace-debugging-on-windows)
is the authoritative reference** — it explains *why* each piece below exists, and carries a failure-
signature table. Read it before changing anything in this area.

### 6. VS Code extensions

| Extension | Id | Why |
|-----------|-----|-----|
| C/C++ | `ms-vscode.cpptools` | **Required.** Provides the `cppvsdbg` adapter — the working debug config on windows-msvc. Install the base extension only, *not* the Extension Pack. |
| rust-analyzer | `rust-lang.rust-analyzer` | Language support |
| CodeLLDB | `vadimcn.vscode-lldb` | *Optional.* The alternative adapter, with Rust-aware formatters. Retained; see the caveat below. |

**Use the `Debug HRW Observatory (cppvsdbg)` launch config.** Verified 2026-07-28: breakpoints in
the path-dep `crates/rumoca-*` bind and fire, and live-trace stepping advances the animation with
plain **F10** — no Debug Console aliases needed. That last point was a surprise: all-threads
stepping was assumed to be a CodeLLDB feature, when in fact **LLDB defaults to stepping one thread
and must be told otherwise**, while the Visual Studio debugger already runs all threads on a step.

If you use CodeLLDB instead, type `ns` / `si` / `so` in the Debug Console rather than pressing
F10/F11, or the animation will not advance. A note in `launch.json` records CodeLLDB mis-binding
breakpoints in path-dep crates; that note is **unverified** — it predates the discovery of Rumoca's
compile cache, which produces a near-identical symptom (see `docs/architecture.md` §§ 4–5).

### 7. The HRW Debugger Bridge extension

HRW arms its own breakpoints by writing request files that a small VS Code extension picks up. Its
`out/` directory is gitignored, so **it must be built after cloning** or the Debug button does
nothing:

```powershell
cd hrw\vscode-extension
npm install
npm run build
cd ..\..
code --install-extension hrw\vscode-extension
```

Reload VS Code afterwards. The extension logs to the "HRW Bridge" output channel — a working arm
shows `Armed: live_trace.rs:<line>`.

### 8. Launch

Open **the repository root** as the VS Code folder (not `hrw/`), and launch **"Debug HRW
Observatory"** from the launch-configuration dropdown.

> **Launch from the dropdown, not from rust-analyzer's "Debug" CodeLens.** The CodeLens builds its
> own configuration and ignores `launch.json`, so it gets neither the GPU backend override nor the
> stepping aliases — live trace will crash or misbehave under it.

`.vscode/launch.json` carries settings that are not optional. Each is explained in the architecture
doc; briefly:

| Setting | Why |
|---------|-----|
| `WGPU_BACKEND=gl` | A D3D12 device does not survive the long pauses live trace depends on. Without this, HRW dies with exit code 101 and an `egui-wgpu` staging-buffer panic. |
| `_NO_DEBUG_HEAP=1` | Windows switches the CRT to the validating debug heap under a debugger, making sessions ~100x slower. |
| `RUST_BACKTRACE=1` | Panic output goes to the debuggee's stderr — the **integrated terminal**, not the Debug Console. |
| `initCommands` aliases | All-threads stepping (below). |

Note these files are force-added: the repo root's `.gitignore` excludes `.vscode/`, so new files
there need `git add -f` to be tracked.

### Using it

1. Launch under the debugger, load a specimen, open the Structural Analysis or Index Reduction tab.
2. Open a **Matching**, **Tarjan**, or **Reduction** animation view and click **Debug**.
3. Execution stops at the anchor with `frame_index = 18446744073709551615` (`usize::MAX`) — the
   startup gate, before any algorithm work.
4. **Continue (F5)** advances one algorithm step per press, and the animation follows.
5. To step through Rumoca's code *and* keep the animation live, type these in the **Debug Console**
   rather than pressing F10/F11:

   | Alias | Equivalent |
   |-------|-----------|
   | `ns` | `thread step-over -m all-threads` |
   | `si` | `thread step-in -m all-threads` |
   | `so` | `thread step-out -m all-threads` |

   VS Code's step buttons move only the selected thread, which freezes the UI thread and leaves the
   animation stale.

### If something goes wrong

| Symptom | Cause |
|---------|-------|
| Debug button does nothing; "HRW Bridge" channel silent | Extension not built or not installed (step 7) |
| Breakpoint lands in an unrelated crate during startup | The anchor has been folded onto another empty function — see architecture doc §1. Do not simplify `live_trace_breakpoint`. |
| Locals show `<optimized out>` | The crate under study needs `opt-level = 0` in the workspace `Cargo.toml` |
| Visuals freeze, HRW exits with code 101 | GPU device loss — confirm `WGPU_BACKEND=gl` is in effect and that you launched from the dropdown |
| Specimens fail to compile | MSL not staged (step 3) |
| Tests fail intermittently | Missing `--test-threads=1` |

---

## Layout

```
hrw/
├── src/               # The application
├── specimens/         # Modelica models, authored in Wolfram System Modeler
├── vendor/msl/        # Gitignored — staged MSL 4.1.0 (step 3)
├── vscode-extension/  # The HRW Debugger Bridge (out/ gitignored — step 7)
├── docs/              # Charter, architecture, compiler phases, specimen notebook
└── .hrw-bridge/       # Gitignored — runtime scratch for the Claude/debugger bridge
```

Build, run, and test from the **workspace root** with `-p hrw`, or from `hrw/` directly.
