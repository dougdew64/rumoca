# HRW Architecture

How the observatory works, for a reader who knows Rust and egui basics but hasn't
read the HRW source. Written as onboarding material and as documentation for an
upstream PR to the Rumoca repository.

---

## 1. What HRW is

HRW is a desktop application for **studying** the Rumoca Modelica compiler. It
compiles Modelica specimens through Rumoca's pipeline (Parse → Resolve →
Instantiate → Typecheck → Flatten → Structural Analysis → Index Reduction →
Initialization → Events → Solve Lowering → Simulation) and renders each phase's
intermediate representation (IR) in an interactive inspector. It also runs
simulations and plots the resulting trajectories.

HRW lives inside a fork of the Rumoca workspace (`hrw/` directory) as a normal
Cargo workspace member. This lets it depend on Rumoca crates via path deps
(`../crates/rumoca-*`), which enables instrumenting Rumoca internals — the public
API exposes phase *results* but not the algorithms' *process*.


## 2. Crate structure

```
hrw/
├── src/
│   ├── lib.rs          # Module registration (the library crate)
│   ├── main.rs         # Binary entry point (eframe launcher + WSLg workaround)
│   ├── app.rs          # The eframe::App — UI layout, tab bar, panel routing
│   ├── worker.rs       # Background-thread compilation & simulation
│   ├── bridge.rs       # Claude Code integration — JSON focus-file emitter
│   ├── colors.rs       # Shared color constants (theme-aware palette)
│   ├── expr_format.rs  # Modelica expression pretty-printer (precedence-aware)
│   ├── tree.rs         # Generic serde-value tree inspector widget
│   ├── canvas.rs       # Reusable pan/zoom scaffold for custom-painted views
│   ├── spyplot.rs      # BLT (Block Lower Triangular) spy-plot painter
│   ├── incidence_view.rs  # Incidence matrix (equation × unknown) painter
│   ├── matching_anim.rs   # Animated matching stepper (augmenting-path replay)
│   ├── tarjan_anim.rs     # Animated Tarjan SCC stepper (BLT discovery replay)
│   ├── reduction_view.rs  # Index reduction process summary panel
│   ├── equation_sheet.rs     # Readable equation sheet from the flat DAE
│   ├── identifier_index.rs  # Cross-stage identifier index (source → flat names)
│   ├── log_view.rs          # Timestamped compilation/simulation log panel
│   └── field_help.rs        # Build-time-embedded IR field documentation
├── specimens/          # Modelica source files (the inputs)
├── examples/
│   ├── gen_trace.rs    # Headless: writes a specimen's durable compilation trace
│   └── gen_field_help.rs  # Extracts doc comments from rumoca-ir-ast
├── docs/               # Documentation (this file, charter, ideas, etc.)
└── vendor/msl/         # Gitignored: the staged reference MSL 4.1.0
```

**Why a library + binary split?** Rust binaries can't be depended on by other
targets. All modules live in the library crate (`lib.rs`), so both the GUI binary
(`main.rs`) and headless tools (`gen_trace`, `gen_field_help`) share one
implementation of the compilation pipeline.


## 3. The big picture: data flow

```
                         ┌─────────────────────────────────┐
                         │         Worker thread            │
                         │                                  │
     Compile(path) ──────►  WorkerState::compile()          │
                         │    parse → resolve → instantiate │
                         │    → typecheck → flatten →       │
                         │    structural → index_reduce →   │
                         │    initialization → events →     │
                         │    solve_lowering                 │
                         │         │                        │
                         │    ◄────┘ streams CompileProgress│
     FromWorker::Log  ◄──┤         + final Compiled         │
     FromWorker::Progress│                                  │
     FromWorker::Compiled│  WorkerState::simulate()         │
                         │    compile → lower → integrate   │
                         │         │                        │
     FromWorker::Simulated◄────────┘                        │
                         └─────────────────────────────────┘
                                        ▲
                                        │ mpsc channels
                                        ▼
                         ┌─────────────────────────────────┐
                         │          UI thread (App)         │
                         │                                  │
                         │  drain_worker()  ◄── poll rx     │
                         │       │                          │
                         │       ▼ store in Stage fields    │
                         │                                  │
                         │  ui() ──► tab bar (StageKind)    │
                         │          center panel:           │
                         │            tree / spy-plot /     │
                         │            incidence / reduction │
                         │            / eq sheet / plot /   │
                         │            log                   │
                         │                                  │
                         │  click ──► bridge::emit_focus()  │
                         │            writes focus.json     │
                         └─────────────────────────────────┘
```


## 4. The worker thread

**File:** `worker.rs` (~3200 lines, the largest module)

### Why a separate thread?

egui calls `update()` every frame at ~60 fps. Each call must return in under
~16 ms. Compiling a Modelica model takes hundreds of milliseconds; simulation
can take seconds. So all compiler and solver work runs on a dedicated **worker
thread**, communicating with the UI via `std::sync::mpsc` channels — message
passing with no shared mutable state.

### Channel protocol

Two channels, one in each direction:

| Direction | Type | Purpose |
|-----------|------|---------|
| UI → Worker | `Sender<ToWorker>` | Commands: Compile, Simulate, SetLibraries, OpenDef, SetTracing |
| Worker → UI | `Sender<FromWorker>` | Results: Log, CompileProgress, Compiled, Simulated, DefOpened, LibrariesLoaded |

The UI polls `rx.try_recv()` each frame (non-blocking) inside `App::drain_worker()`.
The worker calls `ctx.request_repaint()` after sending a result, so the UI wakes
up even if the user isn't interacting.

### The `Worker` struct

```rust
pub struct Worker {
    tx: Sender<ToWorker>,       // Send commands to the worker
    rx: Receiver<FromWorker>,   // Receive results from the worker
}
```

Owned by `App`. The actual compilation state (`Session`, loaded libraries, etc.)
lives in `WorkerState` on the worker thread — the UI never touches it.

### Progressive streaming

Compilation doesn't wait until all 10 stages finish before reporting. All stages
are grouped in a `StageBundle` struct. After each stage completes, the worker sends
a clone of the bundle via `FromWorker::CompileProgress`; the UI assigns it directly
to `App.stages` so tabs update in real time:

```
parse completes    → send CompileProgress { parse: Some(...), resolve: None, ... }
resolve completes  → send CompileProgress { parse: Some(...), resolve: Some(...), ... }
...
solve_lowering     → send Compiled { all stages + def_index }
```

This lets the UI update the tab bar and tree inspector progressively — the user
sees each stage appear while later stages are still computing.

### The Rumoca Session

The worker owns a persistent `rumoca_compile::Session` — Rumoca's incremental
compilation workspace (the same type the LSP server uses). Library dependencies
(the MSL — ~15,000 files) are loaded once as `DurableExternal` source roots.
Thereafter, each specimen compile re-resolves incrementally (~0.3s) rather than
re-parsing the entire library. On **recompile** (same specimen, same source),
the worker calls `session.remove_document()` before `update_document()` to
bypass the session's content-comparison cache — without this, unchanged source
text short-circuits and phase code never re-executes (armed breakpoints would
not fire).

### The compilation pipeline

`WorkerState::compile()` drives the 10-stage pipeline:

1. **Parse** — `rumoca_phase_parse::parse()` → AST
2. **Resolve** — `session.resolve()` → resolved tree (names bound to definitions)
3. **Instantiate** — `rumoca_phase_instantiate::instantiate_model()` → `InstanceOverlay`
4. **Typecheck** — `rumoca_phase_typecheck::typecheck_instanced()` → types assigned
5. **Flatten** — `session.compile_model_strict_reachable_with_recovery()` → flat model
   (this is an opaque Rumoca entry point that runs phases 5–9 internally)
6. **Structural** — `build_structural_report()` on the raw (pre-reduction) DAE
7. **Index reduction** — replicate Rumoca's reduction funnel on a clone of the DAE
8. **Initialization** — `build_ic_plan()` + determinacy analysis
9. **Events** — read hybrid partitions from the DAE's public fields
10. **Solve lowering** — `lower_dae_to_solve_model()` → `SolveModel`

Phases 5–10 depend on Rumoca's `PhaseResult`, which is either `Success(CompileResult)`
(carrying the DAE + flat IR) or `Failed { phase, error }`. When a phase fails, later
stages are skipped and show informational notes ("not reached — Flatten failed").

Phases 5–10 use the `run_stage!` macro to avoid repeating the 7-line log/time/extract/
emit pattern for each stage. The macro captures `log`, `drain_traces`, `bundle`, `emit`,
and `path` from the enclosing scope.

Stage-extraction functions (`structural_stage`, `index_reduction_stage`, etc.) share
fallback handling via `not_reached_stage()` — a helper that returns placeholder stages
for `Failed`/`NeedsInner`/`None` result variants, eliminating duplicated match arms.
`unwrap_success()` extracts the `&CompilationResult` from a `PhaseResult::Success`,
replacing inline `match` arms that were duplicated across five stage functions.

`StageKind::ALL` is a const array of all 11 pipeline stages in order, used for
exhaustive iteration and test assertions.

Both `compile()` and `simulate()` use `make_log(&t0, emit)` to build their timing-aware
log closures from a shared helper, avoiding the identical closure pattern.

### Simulation

`WorkerState::simulate()` is a separate pipeline: compile → solve-lower →
`simulate_solve_model()`. It re-compiles rather than reusing the compilation state
because `SolveModel` borrows from `CompileResult`, and the borrow semantics don't
allow storing the intermediate. The result is `SimData` — a plain struct with
`times`, `names`, `data[var][t]`, and `solver_steps` (per-step diagnostics from
the BDF integrator: time `t`, step size `h`, and BDF order `k` at each internal
solver step). `SimData` carries no Rumoca types into the UI.

The simulation pane renders two linked plots when solver diagnostics are available:
a **trajectory plot** (state variables vs time) and a **solver diagnostics plot**
(step size `h(t)` and BDF order `k(t)` vs time). The two plots share a synchronized
time axis via `egui_plot::Plot::link_axis`, so panning/zooming one updates the other.
The diagnostics panel only appears for models solved with the BDF integrator (stiff
models); RK45-solved models show only the trajectory plot.

### Output capture (`OutputCapture`)

Some Rumoca phases emit diagnostics via `println!`/`eprintln!` rather than structured
errors. `OutputCapture` intercepts these at the file-descriptor level and forwards
them as log entries so they appear in HRW's log pane.

**The problem it solves.** On Unix, stdout/stderr are file descriptors (fd 1 and fd 2).
`OutputCapture::start()` creates two `pipe()` pairs and uses `dup2()` to redirect
fd 1 and fd 2 into the write ends of those pipes. Any `println!` or `eprintln!` in
Rumoca library code now writes into the pipes instead of the terminal. After each
Rumoca API call, `drain()` reads the accumulated bytes and the `drain_output` closure
forwards each line as a `LogLevel::Stdout` or `LogLevel::Stderr` log entry.

**The deadlock hazard.** Linux pipes buffer 65,536 bytes. The `drain()` call is
*post-hoc* — it runs after a Rumoca API call returns. If a single API call writes
more than 64 KB to stdout/stderr, the `write()` syscall blocks (pipe full), the
API call never returns, and `drain()` never runs — a classic deadlock. The worker
thread hangs silently with no error signal.

**The fix: concurrent reader threads.** Rather than draining post-hoc, `start()`
spawns two lightweight reader threads (one per pipe) that continuously read into
shared `Arc<Mutex<Vec<u8>>>` buffers:

```
                              ┌─────────────────────────────────┐
                              │  Worker thread                  │
                              │                                 │
                              │  Rumoca API call                │
                              │    └─ println!("...")           │
                              │         └─ write(fd 1, ...)  ───┼──┐
                              │                                 │  │ pipe
                              │  drain() ─── lock(stdout_buf)   │  │
                              │    └─ take accumulated bytes    │  │
                              └─────────────────────────────────┘  │
                                                                   │
                              ┌─────────────────────────────────┐  │
                              │  Reader thread (stdout)         │  │
                              │                                 │  │
                              │  loop {                      ◄──┼──┘
                              │    read(pipe_read_end, 4096)    │
                              │    lock(stdout_buf).extend(...) │
                              │  }                              │
                              └─────────────────────────────────┘
```

The write side of the pipe stays in normal **blocking** mode — the reader thread
keeps the pipe buffer from ever filling, so `write()` never blocks, and
`println!`/`eprintln!` never see `EAGAIN` or panic.

**Lifecycle:**
1. `start()` — flush existing stdout/stderr, create pipes, `dup()`-save originals,
   `dup2()` the write ends onto fd 1/2, close the original write ends (only fd 1/2
   are writers now), spawn two reader threads.
2. `drain()` — flush stdout/stderr (push any buffered `BufWriter` data into the pipe),
   lock each `Mutex<Vec<u8>>`, `mem::take` the accumulated bytes, return as strings.
   Called after each Rumoca API call.
3. `Drop` — flush, `dup2()` the saved originals back onto fd 1/2 (restoring normal
   output), close the saved fds. This closes the pipe write ends, so the reader
   threads see EOF and exit. `join()` both threads to ensure clean shutdown.

**Platform scope:** `OutputCapture` is `#[cfg(unix)]` — the `pipe()`/`dup2()` calls
are Unix-specific. On other platforms, `start()` returns `None` and `drain_output` is
a no-op. The `#[allow(unused)]` annotation on the `capture` parameter suppresses the
resulting dead-code warning.

### JSON serialization strategy

Each stage serializes only the **user model's** IR (a few KB), not the whole resolved
aggregate (~430 MB with the full MSL). The interchange format is `serde_json::Value`
(a generic JSON tree) because not all Rumoca IR types implement `Serialize`, and JSON
lets the generic tree inspector render any stage without knowing its Rust type.


## 5. The UI shell

**File:** `app.rs` (~3200 lines)

### Immediate-mode UI

egui is **immediate-mode**: every frame, `App::ui()` rebuilds the entire UI from
scratch — buttons, labels, panels, trees, plots, everything. There is no retained
widget tree that persists between frames. All durable state lives in the `App`
struct's fields. A click is detected the same frame the button is drawn:

```rust
if ui.button("Run").clicked() {
    self.worker.send(ToWorker::Simulate { ... });
}
```

### The `App` struct

`App` holds all application state, organized into 16 field groups:

1. **Worker** — the `Worker` handle (send/receive channels)
2. **Library config** — MSL source-root paths, load status
3. **Specimen list** — directory path, file list, purpose hints
4. **Compilation results** — a `StageBundle` (all 10 pipeline stages in one struct), model name, def_index
5. **Navigation** — the "go to definition" stack for browsing library classes
6. **Bridge** — Claude Code capture state (monotonic `ask_seq` counter)
7. **View toggles** — UI mode (Tour/Specimen/Debug), Settings, Help, About window visibility
8. **Field help** — the embedded doc-comment lookup table (delivered as tree node tooltips)
9. **Custom views** — sub-view selectors and pan/zoom cameras for spy-plot, incidence, matching, and Tarjan views
10. **Log** — timestamped compilation/simulation log entries
11. **Simulation** — `SimData`, plot flags, sim-in-progress state
12. **Cached layout** — `cached_specimen_width` avoids per-frame `layout_no_wrap`
13. **Cached views** — `cached_spy_plot`, `cached_incidence`, `cached_reduction`,
    `cached_equation_sheet`, `cached_matching_anim`, `cached_tarjan_anim`
    (`Option<Option<T>>` — outer = cache state, inner = parse result) avoid per-frame
    re-parsing of structural report JSON; invalidated on `Compiled` and when switching
    between Structural/IndexReduction (tracked by `cached_report_stage`)
14. **Markdown rendering** — `egui_commonmark` cache and per-specimen narrative cache
15. **Pending stage** — deferred stage switch for `hrw://load/Specimen/Stage` links
16. **Live debug spawn** — deferred algorithm thread spawn with breakpoint ack handshake

### Panel layout and UI modes

HRW has three UI modes (`UiMode` enum), selectable from the **View** menu.
All three share the same app state — the mode controls the left panel content:

| Mode       | Left panel                              | Right (center) panel |
|------------|----------------------------------------|----------------------|
| **Tour**   | End-to-end tour (rendered markdown)     | Stage tabs           |
| **Specimen** | Specimen list (top ⅓) + narrative (bottom ⅔) | Stage tabs   |
| **Debug**  | Hidden (VS Code alongside)             | Stage tabs           |

**Tour mode** (fullscreen, no VS Code):
```
┌───────────────────────┬──────────────────────────────────┐
│  Tour guide           │  [Specimen ▾] Log│Parse│...│Sim   │
│  (rendered markdown)  ├──────────────────────────────────┤
│                       │  Stage views                     │
│                       │                                  │
└───────────────────────┴──────────────────────────────────┘
```

**Specimen mode** (fullscreen, default):
```
┌───────────────────────┬──────────────────────────────────┐
│  Specimens            │  [Specimen ▾] Log│Parse│...│Sim   │
│  BouncingBall         ├──────────────────────────────────┤
│  Drivetrain  ◄───┐    │  Stage views                     │
│  ...              │    │                                  │
│───────────────────│────│                                  │
│  Narrative        │    │                                  │
│  (rendered md)    │    │                                  │
└───────────────────────┴──────────────────────────────────┘
```

**Debug mode** (HRW on right half, VS Code on left):
```
┌──────────────────────────────────────────────────────────┐
│  [Specimen ▾] Log│Parse│Resolve│...│Simulation  [▶Play]   │
├──────────────────────────────────────────────────────────┤
│  Stage views (full width)                                 │
│                                                          │
└──────────────────────────────────────────────────────────┘
```

Panels are added in **top → bottom → left → center** order. In egui,
each panel claims space from what remains, so order determines layout.
The `CentralPanel` automatically fills whatever space the left panel
doesn't claim.

A **specimen-switcher dropdown** (combo box) is embedded in the stage
tab bar header, visible only in Debug mode — where the specimen list is
hidden, this is the only way to switch specimens.

### Navigation links (`hrw://`)

Tour and narrative markdown can contain `hrw://` links that trigger in-app
navigation when clicked. The link scheme:

- `hrw://load/<Specimen>` — load and compile a specimen by name
- `hrw://stage/<Stage>` — switch to a stage tab (PascalCase slug)
- `hrw://load/<Specimen>/<Stage>` — load a specimen and switch to a stage

Link handling uses `egui_commonmark`'s `add_link_hook` / `get_link_hook` API.
Before rendering markdown, `register_hrw_hooks` registers all `hrw://` URLs
found in the text as link hooks. After rendering, `drain_hrw_hooks` checks
which hooks were clicked and returns the first triggered `HrwLink` action.
The dispatch code then calls `open()`, sets the stage, or both.

For `LoadAndSwitch` links, the stage switch is deferred via `pending_stage`
because `open()` starts an async compilation — the stage can only be applied
after the `Compiled` message arrives in `drain_worker`.

### Field help (tooltips)

Generic field help (doc-comment text from `field_help.json`) is delivered as
**hover tooltips** on tree nodes. Hovering any IR field in the tree inspector
shows its doc comment instantly — no panel navigation needed. For deeper,
specimen-specific explanations, the Claude bridge capture + "explain" chat
shortcut remains available.

### Tab bar mechanics

Each stage tab uses `selectable_label` (not `selectable_value`) because tabs
must be conditionally suppressed during compilation — a stage that hasn't been
reached yet is disabled. The tab label is color-coded:

- **Green** — stage completed successfully
- **Red** — stage failed (the phase returned an error)
- **Neutral** — not yet reached or not yet compiled

When compilation finishes, the tab auto-selects the furthest successful stage.

### The play button

The `▶` button in the tab bar runs a simulation **without** switching the viewed
stage. This lets the user watch the log while simulation runs in the background.
It dispatches `ToWorker::Simulate` and sets `simulating = true`.


## 6. The generic tree inspector

**File:** `tree.rs` (~360 lines)

The charter mandates **one generic serde-value tree inspector** for all pipeline
stages — not per-stage bespoke widgets. This is implemented as a recursive
function that renders any `serde_json::Value`:

- **Objects** → collapsible nodes (keys as labels)
- **Arrays** → indexed child lists
- **Scalars** → colored leaf values (strings green, numbers blue, booleans orange)

### Path accumulation

As the recursive walk descends, each level pushes a path segment (`Seg::Key("name")`
or `Seg::Index(3)`) onto a `Vec<Seg>`. When the user clicks a leaf, the full path
is the node's address from the stage root (e.g., `components → inertia → type_def_id`).
This path is captured into the bridge focus file.

### Cross-stage diff highlighting

The `prev` parameter carries the previous stage's IR at the same JSON path. When a
leaf value differs from `prev` (e.g., a `def_id` going from `null` to a real integer
between Parse and Resolve), it is painted **green**. This makes it visually obvious
what each compiler phase changed — the "what did Resolve actually do?" question
answered at a glance.

### DefId resolution

The tree recognizes fields named `def_id`, `type_def_id`, and `base_def_id` and
annotates them inline with the resolved class name (e.g., `type_def_id: 27579 →
model Modelica.Mechanics.Rotational.Components.Inertia`). Right-clicking offers
"Go to definition," which pushes a `NavEntry` onto the navigation stack and renders
the target class in the same generic tree.


## 7. Custom-painted views

Three views use egui's low-level `Painter` API instead of the generic tree.

### Canvas scaffold (`canvas.rs`, ~410 lines)

A reusable pan/zoom camera shared by the spy-plot and incidence views. It maintains
a persistent transform (offset + zoom) and handles:

- **Pan** — drag to scroll
- **Zoom** — scroll wheel, zooming about the pointer position (sensitivity controlled
  by `SCROLL_ZOOM_SENSITIVITY`, clamped to `MIN_ZOOM`..`MAX_ZOOM`)
- **Fit to content** — on first draw, automatically fits the data bounds into the
  available screen space (with `FIT_MARGIN` breathing room)

The canvas maps between two coordinate spaces:

- **World space** — logical coordinates (e.g., cell `(3, 7)` in the matrix)
- **Screen space** — pixel coordinates on the egui widget

`View::to_screen(world_pos)` and `View::to_world(screen_pos)` convert between them.
`View::cell_rect(col, row)` maps a grid cell, `View::hovered_cell(response, n_cols,
n_rows)` resolves the hover pointer to a cell index (shared by spy-plot and incidence),
and `View::draw_grid(painter, n_cols, n_rows, color)` draws grid lines with a built-in
zoom guard.

### BLT spy-plot (`spyplot.rs`, ~380 lines)

Visualizes the Block Lower Triangular (BLT) decomposition of the structural
analysis. Each diagonal block is a group of equations that must be solved together:

- **Scalar blocks** (size 1) — drawn as single cells; these can be solved
  independently by forward substitution
- **Coupled blocks** (size > 1) — drawn as outlined rectangles; these represent
  algebraic loops that require an iterative solver (Newton)

Blocks are laid out consecutively along the diagonal. Colors distinguish block types
(blue for scalar, orange for coupled with tearing). Hover shows the block's equations
and tearing report; click captures the block into the bridge.

### Incidence matrix (`incidence_view.rs`, ~740 lines)

Visualizes the equation × unknown adjacency matrix — the bipartite graph that
maximum matching runs on. Equations are rows, unknowns are columns; a filled cell
means the equation references that unknown. The matrix uses sparse row storage with
sorted column indices for O(log n) hit testing via `binary_search`.

Level-of-detail rendering: grid lines appear at zoom ≥ 6, axis labels (with -45°
rotation for column headers) at zoom ≥ 16. Hover shows crosshair bands highlighting
the row and column; click captures the equation's incidence data.

Row labels use pretty-printed equation text (e.g. `der(w) - tau / J`) instead of
opaque index labels (`f_x[0]`). The text comes from the expression pretty-printer
(`expr_format.rs`) and is carried in the JSON as `equation_text` alongside the
identifier-key `equation`. Tooltips show both — the readable text prominently, the
index label in small print below. The `f_x[N]` identifiers are retained for
matching/BLT overlay lookups, which cross-reference by name.

### Algorithm animation steppers

Two animated views replay structural analysis algorithms frame by frame, built
from trace data recorded by instrumented variants of the Rumoca phase functions.
Both support two animation modes:

1. **Recorded** (default): pre-computed frames from `from_incidence` — standard
   play/pause/step/reset controls with a speed slider.
2. **Live debug**: frames arrive from a shared `LiveTrace<Frame>` buffer as a
   separate algorithm thread runs. Clicking the "Debug" button automatically
   arms a breakpoint on `live_trace_breakpoint` via the HRW Debugger Bridge
   extension, then spawns the algorithm thread — no manual breakpoint setup
   needed. After each frame push, a 20ms delay lets the UI render, then the
   breakpoint fires. The user steps through the algorithm with Continue (F5);
   the button reappears after the session finishes for re-runs.

   **Ack handshake**: HRW does not spawn the algorithm thread immediately
   after writing the breakpoint request — the extension needs time to process
   it and register the breakpoint with LLDB. Instead, HRW polls for an
   acknowledgment file (`breakpoint-ack.json`) that the extension writes after
   processing. The thread is spawned only after the ack arrives (or after a
   3-second timeout if the extension isn't running). This guarantees the
   breakpoint is active before the first frame is pushed.

   **Breakpoint cleanup**: when the algorithm finishes, the thread's
   `on_complete` callback removes the `live_trace_breakpoint` breakpoint via
   the bridge's `action: "remove"` protocol *before the thread exits*. This
   prevents LLDB from delivering SIGSTOP/SIGCHLD when the thread terminates
   with the breakpoint still armed. A UI-side `live_just_finished` check acts
   as a safety-net fallback.

The `LiveTrace<F>` type (in `rumoca-phase-structural/src/live_trace.rs`) wraps an
`Arc<Mutex<Vec<F>>>` shared between the algorithm producer and UI consumer. The
traced algorithms (`maximum_matching_with_trace`, `tarjan_scc_with_trace`) accept
an optional `&LiveTrace<Frame>` — when present, each frame is pushed to both the
local vec (returned in the result) and the shared buffer (read by the UI).
Live mode uses `LiveTrace::new().with_frame_delay(20ms)`, which adds a sleep
after each push (so the UI thread can render before the debugger pauses all
threads) and calls `live_trace_breakpoint` — a dedicated `#[inline(never)]`
function that the debugger resolves unambiguously. This is the upstreamable
observability API.

**Why re-running the algorithm (clicking Debug again) is safe — a Rust ownership lesson.**
The Debug button spawns a new algorithm thread that re-runs matching or Tarjan.
This is safe because the algorithm runs on private copies of the data and writes
only to its own `LiveTrace` buffer — no shared mutable state is touched. Several
Rust ownership mechanisms work together to guarantee this at compile time:

- **Immutable borrow `&IncidenceMatrix`**: `start_live` takes `&IncidenceMatrix`,
  not `&mut`. The compiler guarantees the algorithm thread cannot modify the
  shared incidence data. In C++, this would be a `const&` convention that the
  programmer promises to honor — Rust enforces it.

- **Move semantics on `thread::spawn`**: the `move` closure forces the thread to
  *own* everything it uses. The `eq_vars`, `n_eq`, `n_var` are moved into the
  closure — they're private copies, not shared references. If you tried to
  capture a reference to stack-local data, the compiler would reject it because
  `thread::spawn` requires a `'static` closure. This makes dangling pointers
  across threads a compile-time error.

- **`Send` and `Sync` traits**: checked automatically by the compiler.
  `Arc<Mutex<Vec<MatchingFrame>>>` is `Send` (safe to transfer to another
  thread) because `Mutex<Vec<MatchingFrame>>` is `Sync` (safe to share). If
  `MatchingFrame` contained a `Rc` or a raw pointer, the compiler would refuse
  to let it cross the thread boundary. You never have to reason about whether
  `LiveTrace` is thread-safe — the compiler verifies it structurally from its
  fields.

- **RAII and `Drop`**: when clicking Debug replaces the old animation
  (`self.cached_matching_anim = Some(Some(new_anim))`), the old
  `MatchingAnimation` is dropped automatically. Its `Arc` clone is released; if
  the old algorithm thread is still running, it holds the other `Arc` clone and
  finishes normally — the `Arc` reference count manages the lifetime without
  manual cleanup.

The key insight: **the fact that this code compiles is itself the proof that
re-running the algorithm is safe.** If any of these invariants were violated —
mutable aliasing, dangling references, non-thread-safe types crossing
boundaries — you'd get a compiler error, not a runtime bug. In C++ or Java, the
equivalent code would work identically, but the safety guarantees would be
conventions enforced by code review, not by the type system.

**Matching animation** (`matching_anim.rs`, ~630 lines): replays Kuhn's
augmenting-path algorithm on the incidence matrix. Each frame highlights the
current equation, explored edges, found/failed paths, and confirmed matches
with step-by-step descriptions using readable equation text.

**Tarjan SCC animation** (`tarjan_anim.rs`, ~610 lines): replays Tarjan's
strongly connected component algorithm on the dependency graph (derived from
the matching result). Nodes are colored by DFS state (on stack, in discovered
SCC) and edges are classified as tree/back edges.

### Index reduction summary (`reduction_view.rs`, ~580 lines)

A scrollable panel (not a canvas) summarizing what the Pantelides / dummy-derivative
funnel did: which states were demoted, which equations were differentiated, which
variables were eliminated. Renders as sections: summary → funnel steps → demoted
states → differentiated equations → trivial eliminations. Color-coded: green for
successful steps, red for stopped, neutral for no-ops.


### Equation sheet (`equation_sheet.rs`, ~600 lines)

A readable view of the flat DAE as math, replacing the raw JSON tree for the
Flatten stage. Built from the typed `Dae` in the worker thread (where the typed IR
lives), using the precedence-aware `expr_format` pretty-printer. Equations are
grouped by origin category (component, connection, flow conservation, binding,
event) with counts and descriptions. Below the equations, a striped grid shows the
variable classification: name, kind (state/algebraic/parameter/...), start value,
and unit. The Flatten tab gains three sub-tabs: "Equations" (the sheet),
"Source Map" (bidirectional source ↔ equation traceability), and "Tree" (the
generic serde-value inspector). Clicking an equation highlights its row in the
incidence matrix and auto-switches to the Structural / Incidence view (cross-link
via `App::highlighted_eq_row`).

**Source-to-equation traceability** (#28): when the specimen source is available,
each equation is linked to its originating source line(s) via a three-layer
matching strategy:

1. **Span-based** (`SourceId` + byte offset): direct specimen equations whose
   `span` points to the specimen file (not an MSL library file).
2. **Origin-based** (connect matching): connection/flow equations are matched to
   their `connect()` statements by parsing the origin string and comparing
   connector paths against `scan_connect_statements()` results.
3. **Text search**: component equations matched to declaration lines, bindings
   matched to variable declarations.

An equation can map to **multiple source lines** when it arises from the
interaction of several `connect()` statements. Modelica `connect` creates
equivalence classes: when `connect(A, B)` and `connect(C, A)` share a node, the
compiler generates a transitive equality `B.phi = C.phi` and a multi-connector
flow sum `A.tau + B.tau + C.tau = 0` — both are consequences of *both* connect
statements, and the source map links them to both lines. Direct equalities
(both connectors from one connect) map to a single line.

The "Source Map" sub-tab shows a split pane: Modelica source on the left (with
category color bars in the gutter for lines that produced equations) and the flat
equations on the right. Clicking a source line filters equations to those it
produced; clicking an equation highlights its source line(s). Category colors
(component=blue, connection=orange, flow=red, binding=green, event=purple)
visually distinguish equation origins in both panes.

Data model: `EquationSheet` struct with `FormattedEquation` entries (each carrying
a `source_lines: Vec<u32>`), `ClassifiedVariable` entries, and `SourceLine`
entries (reverse mapping from source lines to equation indices), built by
`equation_sheet::build(&dae, source_info)`.


## 8. The Claude bridge

**File:** `bridge.rs` (~1150 lines)

### Architecture: thin emitter, thick reasoner

When the user clicks a node in the tree or a custom view, the app writes a JSON
**focus file** describing what the user is looking at. The app carries **no
reasoning** — it is a pure context emitter. The actual explanation happens in the
Claude Code session, which reads the focus file along with the Modelica source, the
staged IR files, and the compiler-phase documentation.

### The `Ask` struct and `AskRequest` enum

The `Ask` struct aggregates all context needed to write one focus file. It borrows
everything (lifetime `'a`) to avoid cloning large IR trees. The `request` field is
an `AskRequest` enum (`Explain`, `DebugWhereSet`) — type-safe, not stringly typed.
The `stage` field is `Option<StageKind>` — `None` for navigated library definitions
that don't correspond to a pipeline stage.

### The file protocol

Two categories of files, both in `.hrw-bridge/` (gitignored):

1. **`focus.json`** — written on each capture. Contains:
   - `seq` — monotonic counter (each capture increments it)
   - `request` — what the user wants: `"explain"` or `"debug-where-set"` (from `AskRequest`)
   - `kind` — `"node"`, `"stage"`, or `"specimen"`
   - `model` — the Modelica model name
   - `stage` — which pipeline stage the capture came from
   - `node_path` — the JSON-path address of the clicked node
   - `node_value` — the node's IR subtree
   - `provenance` — source location (Modelica line) found by span-ascent
   - `cross_stage` — the same node in the previous stage, with a diff
   - `def_resolutions` — DefId → human-readable name mapping

2. **`stages/<name>.json`** — one file per pipeline stage's full IR, rewritten
   each compile. Claude can diff any two stages by reading two files.

### Span-ascent (source provenance)

Rumoca IR nodes carry source provenance (`location` or `span` fields with byte
offsets into the Modelica source). But leaf nodes usually have no provenance of
their own — the nearest `location`/`span` lives on an ancestor. So the bridge
walks **up** the serde tree from the clicked node to the root, looking for the
tightest enclosing provenance. Once found, the byte range is sliced from the
Modelica source file, expanded to whole lines for context.

### Chat shortcuts

The bridge supports two bare-keyword shortcuts in the Claude Code chat:

- **`explain`** — Claude reads `focus.json` and explains the captured node
- **`debug`** — Claude reads `focus.json`, finds the Rumoca source line where the
  captured item is processed, and writes a breakpoint request to
  `.hrw-bridge/breakpoint-request.json` with a **conditional** breakpoint keyed on
  the captured item's identity (e.g. `def_id.0 == 85`). The HRW Debugger Bridge
  extension (see below) picks up the request and arms it on the running debug
  session. The user then right-clicks the specimen → **Recompile** to trigger the
  compiler and hit the breakpoint.

### The HRW Debugger Bridge VS Code extension

**Directory:** `vscode-extension/`

A standalone VS Code extension (`hrw-debugger-bridge`) that bridges the
file-based `.hrw-bridge/` protocol to VS Code's debug API. It activates on
startup, finds the `.hrw-bridge/` directory, and watches for
`breakpoint-request.json` files.

**Protocol:** Claude writes a JSON file:

```json
{
  "version": 1,
  "specimen": "ProportionalLoop",
  "breakpoints": [
    {
      "path": "/absolute/path/to/registration.rs",
      "line": 22,
      "condition": "def_id.0 == 85"
    }
  ]
}
```

The extension reads it, calls `vscode.debug.addBreakpoints()`, shows a status bar
indicator with the count, and deletes the file.

**Accumulation:** breakpoints accumulate across requests for the same specimen.
When the `specimen` field changes, all previously armed breakpoints are cleared
before adding the new ones. The status bar item doubles as a manual clear button.

**Why a separate extension?** The upstream Rumoca VS Code extension
(`packages/vscode/`) is maintained by CogniPilot. Modifying it on the `hrw`
branch would create rebase conflicts on every upstream sync. A separate extension
keeps HRW-specific features isolated, preserves the upstream contribution path,
and has its own release cycle.

### Recompile

The specimen list's right-click context menu offers a **Recompile** action (in
addition to the existing **Capture**). It re-runs the full compilation pipeline on
the currently selected specimen, which is necessary after arming a breakpoint —
the first compilation already completed before the breakpoint was set.

**Session cache bypass:** Rumoca's `Session::update_document()` compares the
source text and short-circuits when it's unchanged, returning cached resolution
results without re-executing phase code. The worker calls
`session.remove_document()` before `update_document()` to force a fresh
compilation that hits armed breakpoints.


## 9. Supporting modules

### Field help (`field_help.rs`, ~60 lines)

A two-tier help system:

- **Fast tier (this module):** The `///` doc comments that Rumoca's authors wrote
  on IR fields, extracted at build time into `field_help.json` and embedded via
  `include_str!`. Keyed by field name, shown as hover tooltips on tree nodes.
  No AI, no latency.

- **Specific tier (the bridge):** "Why did THIS particular field get this value?" —
  requires Claude to reason about the specimen, the IR, and the phase code.

### Expression pretty-printer (`expr_format.rs`, ~550 lines)

Renders `rumoca_core::Expression` trees as readable Modelica-like text — e.g.
`der(w) - tau / J` instead of `f_x[0]`. The printer is **precedence-aware**: it
tracks operator binding strength (Or < And < relational < Add/Sub < Mul/Div < Exp)
and only inserts parentheses when the child's precedence is lower than the parent's,
or when same-precedence operators require disambiguation (e.g. `a - (b - c)` but not
`a - b - c`). Exponentiation is right-associative; all other binary operators are
left-associative.

Entry points:
- `format_expr(expr)` — render a single expression
- `format_equation(eq)` — render a DAE equation as `lhs = rhs` or `0 = rhs`
- `format_equation_short(eq)` — label form: `lhs = rhs` or just the rhs for residuals
- `equation_labels(equations)` — batch-format a slice of DAE equations into label strings

The module handles all 14 `Expression` variants (Binary, Unary, VarRef, BuiltinCall,
FunctionCall, Literal, If, Array, Tuple, Range, ArrayComprehension, Index, FieldAccess,
Empty) and delegates to existing `Display` impls for `Reference`, `OpBinary`, `OpUnary`,
`Literal`, and `BuiltinFunction::name()`.

Used by:
- **`worker.rs`** — `incidence_to_json` formats each equation's text and carries it
  in the JSON alongside the identifier key
- **`incidence_view.rs`** — axis labels and tooltips show equation text
- **`matching_anim.rs`** / **`tarjan_anim.rs`** — step descriptions use equation text

### Colors (`colors.rs`, ~110 lines)

Shared color constants used across multiple view modules. Contains:
- `OK_GREEN` — the fixed dark-mode success green, used in canvas painters
- `ok_color(dark_mode: bool)` — theme-aware variant (brighter on dark, darker on light)
- `stage_start_color(dark_mode: bool)` — theme-aware stage-start marker (log view)
- `WARN_AMBER` — fixed warning color (log view)
- `INCIDENCE_CELL` / `INCIDENCE_HOVER` — incidence matrix colors
- `COUPLED_STROKE` / `coupled_fill()` — BLT coupled-block colors (fill is semi-transparent)
- `GRID_ALPHA` — grid line alpha multiplier for canvas views

Centralizes what was previously inline `Color32::from_rgb(...)` and `if dark_mode`
blocks duplicated across `app.rs`, `spyplot.rs`, `incidence_view.rs`, `reduction_view.rs`,
`log_view.rs`, and `tree.rs`.

### Log view (`log_view.rs`, ~190 lines)

Renders timestamped compilation and simulation log entries in a scrollable panel
with stick-to-bottom behavior. Each entry is color-coded by level (green for Info,
yellow for Warn, red for Error) and prefixed with a relative timestamp. The log
view is the proof that the in-workspace migration was worthwhile — per-phase
timing and tracing detail were impossible when phases 5–9 came from one opaque
Rumoca API call.


## 10. The instrumentation surface

HRW depends on these Rumoca crates (all via path deps on `../crates/`):

| Crate | What HRW uses |
|-------|---------------|
| `rumoca-core` | `Expression`, `OpBinary`, `Subscript`, etc. — IR primitives for the expression pretty-printer |
| `rumoca-phase-parse` | `parse()` — AST |
| `rumoca-compile` | `Session`, `SessionConfig`, `PhaseResult`, source-root loading |
| `rumoca-ir-ast` | IR types (indirectly, via serde serialization) |
| `rumoca-phase-instantiate` | `instantiate_model()` — InstanceOverlay |
| `rumoca-phase-typecheck` | `typecheck_instanced()` — type assignment |
| `rumoca-phase-structural` | `build_structural_report()`, `build_incidence()`, `build_ic_plan()`, `dae_prepare::*`, matching/tarjan trace types, `LiveTrace<F>` |
| `rumoca-ir-dae` | `Dae`, `Equation` types for the index-reduction funnel and expression formatting |
| `rumoca-phase-solve` | `lower_dae_to_solve_model()` — SolveModel |
| `rumoca-sim` | `simulate_solve_model()` — simulation runner |

**Instrumentation discipline:** any modification to the Rumoca crates must be:

- **Additive and observation-only** — semantics-preserving, so HRW stays faithful
  to real Rumoca and rebases on upstream stay clean.
- **Upstreamable** — shaped as a general observability/tracing API, kept separable
  from `hrw/` so an upstream PR is a clean cherry-pick of Rumoca-only changes.

When Rumoca upstream changes an API, the breakage shows up in these imports and
their call sites. The regression test suite (231 tests) guards against silent
regressions during a rebase.


## 11. Build and run

```sh
# Build and run the observatory
cargo run -p hrw

# Run the test suite
cargo test -p hrw

# Regenerate field help after a Rumoca pin bump
cargo run -p hrw --example gen_field_help

# Generate a specimen's durable compilation trace
cargo run -p hrw --example gen_trace -- BouncingBall
```

The `-p hrw` flag scopes to the HRW workspace member, avoiding a full workspace
build (which would pull in all Rumoca crates, including platform-specific ones
like `libudev-sys` that may not be needed).


## 12. Key design decisions explained

### Why `serde_json::Value` everywhere?

Not all Rumoca IR types implement `Serialize` (or consistently). By converting
each phase's output to a generic JSON value, the tree inspector, bridge, and
trace generator all work with one uniform type. Adding a new pipeline stage to the
UI requires zero changes to the tree inspector — just serialize the IR and hand it
over.

### Why does simulation re-compile?

`SolveModel` borrows from `CompileResult`. Rust's borrow checker prevents storing
both the `CompileResult` and the `SolveModel` that references it simultaneously in
a way that survives across the channel send. So `simulate()` runs its own fresh
compilation. This is fast (~0.3s) because the MSL is already loaded.

### Why replicate the index-reduction funnel?

Rumoca's `prepare_dae_for_structural_analysis` (the full reduction funnel) is
`pub(super)` — not reachable from outside `rumoca-sim`. But the individual steps
(`dae_prepare::demote_exact_alias_component_states`, etc.) are all public. HRW
calls them in the same order. This is guarded by a test that asserts the funnel
still works; if an upstream change reorders or renames the steps, the test fails
loudly. This is a candidate for upstream visibility widening.

### Why the "thin emitter, thick reasoner" bridge?

The app has no language model and can't answer "why" questions. Rather than
building a static lookup table of pre-computed answers (which guesses what will be
asked), the bridge writes a context-rich focus file that an AI assistant can read
and reason over. This keeps the app simple and makes every question answerable,
not just anticipated ones.

### Why immediate-mode (egui) instead of retained-mode?

The charter chose egui for: native builds only (no WASM), Rust-native (no FFI),
fast iteration (layout is code, not XML), and seamless integration with the
CodeLLDB debugger (Rust-aware value formatters make IR legible while
single-stepping a phase). The trade-off is that the entire UI is rebuilt every
frame — but for an inspector app with modest widget counts, this is well within
the 16 ms budget.

### Why a standalone Rust/egui app — and why it works so well

HRW benefits from a fortunate convergence of three design choices that reinforce
each other: Rumoca's compiler architecture, Rust's ownership system, and egui's
immediate-mode rendering. Understanding how they fit together explains why the
observatory is implementable at all, and why the alternatives (e.g. a VS Code
extension) would be significantly harder.

**Rumoca's strict phase boundaries.** Rumoca's pipeline is a chain of pure
functions: each phase takes an IR data structure, produces the next one, and has
no side effects on shared state. The IR crates (`rumoca-ir-ast`, `rumoca-ir-flat`,
etc.) are pure data — no behavior, no mutation methods, no hidden invariants.
This means HRW can freely snapshot any phase's output, render it, re-render it,
diff it against a previous version, or hand it to a background thread, all without
worrying about stale references or concurrent mutation. The strict phase
boundaries also make instrumentation safe: adding observation hooks inside a phase
can't corrupt downstream phases, because phases communicate only through their
IR outputs.

**Rust's ownership and concurrency guarantees.** The live algorithm stepping
feature is a good example. A `LiveTrace<F>` shared buffer lets an algorithm
thread push frames while the UI thread polls them — classic producer/consumer
concurrency. In most languages this would require careful manual synchronization
and defensive programming against data races. In Rust, the compiler enforces it:

- The algorithm thread receives its inputs as *copies* (`move` closure) or
  immutable borrows (`&IncidenceMatrix`), so it cannot corrupt the source data.
- The `LiveTrace` buffer is `Arc<Mutex<Vec<F>>>` — the `Arc` makes sharing
  explicit, the `Mutex` makes access exclusive, and `Send + Sync` trait bounds
  are checked at compile time. A data race is a compile error, not a runtime bug.
- When the algorithm thread finishes, Rust's RAII/`Drop` cleans up automatically.
  No leaked threads, no forgotten locks.
- Re-running an algorithm is safe because `start_live` copies all data into the
  new thread — there is no shared mutable state to reset.

This is why the live stepping feature was implementable in a single session rather
than requiring weeks of careful concurrent programming. The language eliminated
the entire class of bugs that would otherwise dominate the effort.

**egui's immediate-mode rendering.** Custom-painted views (incidence matrices,
spy plots, matching animations, Tarjan graph visualizations) are straightforward
because egui's `Painter` API is just "draw a rectangle here, draw text there" —
there's no retained scene graph to synchronize with changing data. When a new
algorithm frame arrives in the `LiveTrace` buffer, the UI simply reads it and
paints it; there's no "update the widget" step, no virtual DOM diffing, no
callback registration. This directness is what makes the three-tier animation
architecture (static snapshot → recorded replay → live-stepped execution) work:
all three tiers use the same painting code, just pointed at different data
sources.

**Why HRW is a native app, not a VS Code extension.** Rebuilding the observatory
as a VS Code webview would lose type safety (custom views in HTML/JS/Canvas2D),
the direct Rumoca library link (compilation would need IPC), and the live algorithm
stepping architecture (spawning algorithm threads directly). The one real gap — live
breakpoint arming on an already-running session — is now bridged by a lightweight
**companion** VS Code extension (`vscode-extension/`) that watches
`.hrw-bridge/breakpoint-request.json` and calls `vscode.debug.addBreakpoints()`.
The two communicate through a file-based protocol with an ack handshake: HRW
writes a request, the extension processes it and writes an ack, HRW reads the
ack before proceeding. This ensures breakpoints are registered before algorithm
threads start, and removed before algorithm threads exit.

**The side-by-side workflow** (HRW on one half of the screen, VS Code on the
other) gives the best of both worlds: HRW's rich visual rendering and direct
compiler access alongside VS Code's debugger and editor. The two tools
communicate through the file-based bridge (focus files for Claude chat,
breakpoint requests for the debugger extension) and the shared debugger
(breakpoints on instrumented Rumoca code). This loose coupling is a feature —
each tool does what it's best at, and neither constrains the other.
