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
│   ├── reduction_anim.rs  # Animated index-reduction stepper (step-by-step replay)
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

**File:** `worker.rs` (~3950 lines, the largest module)

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

**File:** `app.rs` (~3850 lines)

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

`App` holds all application state, organized into 15 field groups:

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
12. **Cached views** — `cached_spy_plot`, `cached_incidence`, `cached_reduction`,
    `cached_equation_sheet`, `cached_matching_anim`, `cached_tarjan_anim`
    (`Option<Option<T>>` — outer = cache state, inner = parse result) avoid per-frame
    re-parsing of structural report JSON; invalidated on `Compiled` and when switching
    between Structural/IndexReduction (tracked by `cached_report_stage`)
13. **Markdown rendering** — `egui_commonmark` cache and per-specimen narrative cache
14. **Pending stage** — deferred stage switch for `hrw://load/Specimen/Stage` links
15. **Live debug spawn** — deferred algorithm thread spawn with breakpoint ack handshake

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

### BLT spy-plot (`spyplot.rs`, ~450 lines)

Visualizes the Block Lower Triangular (BLT) decomposition of the structural
analysis. Each diagonal block is a group of equations that must be solved together:

- **Scalar blocks** (size 1) — drawn as single cells; these can be solved
  independently by forward substitution
- **Coupled blocks** (size > 1) — drawn as outlined rectangles; these represent
  algebraic loops that require an iterative solver (Newton)

Blocks are laid out consecutively along the diagonal. Colors distinguish block types
(blue for scalar, orange for coupled with tearing). Hover shows the block's equations
and tearing report; click captures the block into the bridge.

### Incidence matrix (`incidence_view.rs`, ~700 lines)

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

Three animated views replay structural analysis algorithms frame by frame, built
from trace data recorded by instrumented variants of the Rumoca phase functions.
All three support two animation modes:

1. **Recorded** (default): pre-computed frames from `from_incidence` (matching,
   tarjan) or `from_frames` (reduction) — standard play/pause/step/reset
   controls with a speed slider.
2. **Live debug**: frames arrive from a shared `LiveTrace<Frame>` buffer as a
   separate algorithm thread runs. Clicking the "Debug" button automatically
   arms a breakpoint on `live_trace_breakpoint` via the HRW Debugger Bridge
   extension, then spawns the algorithm thread — no manual breakpoint setup
   needed. After each frame push, a 20ms delay lets the UI render, then the
   breakpoint fires. The user steps through the algorithm with Continue (F5).

   All animation controls sit on **one row**, built by `animation_controls`:

   ```text
   [Play/Pause] [Reset] [Back] [Step] | Frame n/m | Speed: [====] | [Debug]  Live (done)
   ```

   Playback first, then the frame/speed group, then a divider, then the live
   debug controls. The Debug button is rendered by `animation_controls` rather
   than by the caller so the row stays together; it cannot arm a session itself
   (that needs the bridge, the model name, and `pending_live_debug`), so it
   returns its click and `app.rs` calls `App::start_live_debug`. The handshake
   polling moved to `App::live_debug_poll`, which renders nothing.

   **Controls are enabled and disabled, never shown and hidden.** A control that
   vanishes gives no clue that the action exists or why it is unavailable, and
   the row reflows under the pointer as it goes. `LiveState` (in `lib.rs`) drives
   this for both the Debug button and the playback row, and has four states
   rather than the two booleans it replaced:

   | State | Meaning | Debug | Play / Reset / Back / Step / Speed |
   |-------|---------|-------|-----------------------------------|
   | `Idle` | Recorded playback | enabled | enabled |
   | `Arming` | Debug clicked; handshake in flight | disabled | disabled |
   | `Running` | Debugger owns the cursor | disabled | disabled |
   | `Finished` | Session over; frames are now recorded | enabled (re-run) | enabled |

   `Arming` is the state the booleans could not express. The breakpoint
   handshake takes several frames, and throughout them the view still holds the
   *recorded* animation — so `is_live()` is false and the controls stayed live
   right after the click. The animations cannot detect it themselves; `arming`
   is passed in from `App::is_arming`, which checks whether `pending_live_debug`
   names this view's algorithm.

   `Finished` matters because `live_rx` is never cleared once `start_live` sets
   it — `is_live()` stays true for the animation's lifetime. Gating on that
   alone left playback and the Debug button dead for good after a single
   session. Disabled controls carry hover text explaining why.

   **Ack handshake**: HRW does not spawn the algorithm thread immediately
   after writing the breakpoint request — the extension needs time to process
   it and register the breakpoint with LLDB. Instead, HRW polls for an
   acknowledgment file (`breakpoint-ack.json`) that the extension writes after
   processing. The thread is spawned only after the ack arrives (or after a
   3-second timeout if the extension isn't running). This guarantees the
   breakpoint is registered before the thread starts.

   **Startup gate**: the ack handshake guarantees that
   `vscode.debug.addBreakpoints()` has been *called*, but that VS Code API is
   asynchronous — LLDB may not have finished installing the breakpoint by the
   time HRW sees the ack and spawns the thread. To close this gap, every
   `start_live` thread calls `LiveTrace::wait_for_debugger()` as its very
   first action, *before* any algorithm work begins. This sleeps 500ms (giving
   LLDB time to finish breakpoint installation), then calls
   `live_trace_breakpoint(usize::MAX)` with a sentinel index, so the debugger
   pauses the thread at a point where zero frames have been pushed. The user's
   first Continue (F5) releases the gate and the algorithm starts from step
   zero — no steps are missed.

   **Breakpoint pre-warm** (`App::tick_prewarm`, `Prewarm` in `app.rs`): the
   *first* breakpoint requested in a given source file costs far more than
   later ones, because the debugger must resolve that file to a compilation
   unit and load its line table. For `hrw.exe` — a 21 MB `.text` section with a
   correspondingly large PDB — that cold resolution takes well over a second,
   comfortably longer than the 500 ms startup gate. The observable symptom was
   sharp: the first Debug click of a session missed its breakpoint entirely and
   the algorithm ran to completion, while the second click and every one after
   worked. Rather than lengthen the wait — a guess paid on every live debug
   start forever — HRW arms and immediately removes the anchor once on the first
   UI frame, moving the cold resolution off the critical path. Nothing is left
   armed; the debugger keeps the line table cached regardless. The arm/remove
   pair must be sequenced through the ack (not issued back-to-back) because both
   requests write the *same* file, so an immediate remove would overwrite the
   arm before the extension ever read it. If a Debug click arrives mid-pre-warm,
   the pre-warm abandons rather than consuming the ack that click is waiting for.

   The four layers — pre-warm, ack handshake, startup sleep, sentinel breakpoint
   call — are all necessary: the pre-warm removes the cold-resolution cost; the
   ack prevents spawning before the request is processed; the sleep covers the
   asynchronous breakpoint installation; the sentinel call provides the actual
   pause point.

   See [Live trace debugging on Windows](#live-trace-debugging-on-windows) for
   the environment this depends on — it is not self-contained in the code.

   **Breakpoint cleanup**: when the algorithm finishes, the thread's
   `on_complete` callback removes the `live_trace_breakpoint` breakpoint via
   the bridge's `action: "remove"` protocol *before the thread exits*. This
   prevents LLDB from delivering SIGSTOP/SIGCHLD when the thread terminates
   with the breakpoint still armed. A UI-side `live_just_finished` check acts
   as a safety-net fallback.

The `LiveTrace<F>` type (in `rumoca-phase-structural/src/live_trace.rs`) is the
producer half of an `mpsc` channel. `LiveTrace::new()` returns
`(LiveTrace<F>, mpsc::Receiver<F>)` — the producer moves into the algorithm
thread, the receiver stays in the animation struct. The producer stores a bare
`mpsc::Sender<F>` (no `Mutex` wrapper — `send()` takes `&self`) and the UI
reads from the `Receiver` via `try_iter()`. The animation structs use
`Arc<AtomicBool>` for the `live_done` flag instead of `Arc<Mutex<bool>>`.
This design eliminates all explicit locks from the live debug path — the only
synchronization is atomics and the `mpsc` channel's internal state.

The traced algorithms (`maximum_matching_with_trace`, `tarjan_scc_with_trace`,
`reduce_constrained_dummy_derivatives_with_trace`,
`index_reduce_missing_state_derivatives_with_trace`) accept an optional
`&LiveTrace<Frame>` — when present, each frame is pushed to both the local vec
(returned in the result) and the channel (drained by the UI).
Live mode uses `LiveTrace::new()` + `.with_frame_delay(20ms)`, which adds a
sleep after each push (so the UI thread can render before the debugger pauses
all threads) and calls `live_trace_breakpoint` — a dedicated `#[inline(never)]`
function that the debugger resolves unambiguously. This is the upstreamable
observability API.

### Live trace debugging on Windows

Live trace is the only one of the three animation tiers (recorded snapshot →
recorded replay → live trace) that runs the *real* algorithm under the *real*
debugger. That makes it uniquely sensitive to the toolchain: it depends on the
compiled binary, the linker, the debug adapter, the GPU backend, and the build
profile all cooperating. Most of that is invisible from the source, so it is
written down here.

Everything below was diagnosed on 2026-07-27 while porting from WSL2 to native
Windows. The setup lives in `.vscode/launch.json`, `.vscode/tasks.json`, and the
workspace `Cargo.toml`; `hrw/README.md` is the step-by-step version for a fresh
clone.

#### 1. The anchor must never compile to an empty body

This is the subtle one, and it silently breaks everything downstream.

`live_trace_breakpoint` originally stored to a static and did nothing else:

```rust
static LAST_FRAME_INDEX: AtomicUsize = AtomicUsize::new(0);

#[inline(never)]
pub fn live_trace_breakpoint(frame_index: usize) {
    LAST_FRAME_INDEX.store(frame_index, Ordering::Relaxed);
}
```

Nothing ever *read* `LAST_FRAME_INDEX`. A write-only static is dead state, so at
`opt-level = 1` LLVM is free to dead-store-eliminate the store. With its only
statement gone, the function becomes a bare `ret` — and the MSVC linker's
identical COMDAT folding (`/OPT:ICF`, on by default) merges byte-identical
functions, collapsing the anchor onto *every other empty function in the
binary*, including eframe's `App::raw_input_hook`.

`#[inline(never)]` does not prevent this. It keeps the *function* from being
inlined; it says nothing about whether the *body* survives.

The consequences were baffling until the mechanism was understood. A breakpoint
on the anchor resolved to a shared address reached from eframe's per-frame
render loop, so the debugger paused during startup, in an unrelated crate,
reporting "Paused on breakpoint" — which was literally true. `image lookup` gave
the diagnosis outright:

```
(before) 0x1400027a0  <hashbrown::raw::RawTableInner>::drop_elements::<…>  at epi.rs:273
(after)  0x141441380  rumoca_phase_structural::live_trace::live_trace_breakpoint  at live_trace.rs:152
```

Three unrelated symbols sharing one address is exactly what folding produces.
Note also that breakpoints in the `hrw` package itself always worked — those are
substantial functions with nothing to fold into — which is why the fault looked
like a path-resolution bug in path-dependency crates for some time.

The fix is two independent defenses, both in `live_trace.rs`:

```rust
pub fn last_frame_index() -> usize { LAST_FRAME_INDEX.load(Ordering::Acquire) }

#[inline(never)]
pub fn live_trace_breakpoint(frame_index: usize) {
    LAST_FRAME_INDEX.store(frame_index, Ordering::Release);
    std::hint::black_box(LAST_FRAME_INDEX.load(Ordering::Acquire));
}
```

`last_frame_index` gives the store a genuine consumer, so it is no longer dead;
`black_box` makes the round-trip opaque, so it cannot be reasoned away even in
principle. `breakpoint_anchor_store_is_observable` guards the property. **Do not
"simplify" this function** — an empty body reintroduces the bug, and the failure
presents as a breakpoint in someone else's crate rather than as anything
recognizable.

#### 2. Optimization level

The workspace sets `[profile.dev] opt-level = 1` (upstream Rumoca's choice,
for parser throughput), which applies to every crate. At that level LLVM drops
line-table entries and reports locals as `<optimized out>` — so even once a
breakpoint binds, `frame_index` is unreadable and stepping teaches nothing.

`[profile.dev.package.<crate>] opt-level = 0` overrides it per crate. Note this
*lowers* opt-level for debuggability, opposite in purpose to the four overrides
above it, which *raise* it for speed. Two crates carry it today:
`rumoca-phase-structural` (the live-trace anchor) and `rumoca-phase-dae` (DAE
construction — `pre()` lowering, when-equation lowering, event structure).

**Extend this to any crate the moment you first try to set a breakpoint in it**,
not after losing time to the failure — because the failure is quiet. A
breakpoint on a line with no line-table entry still **binds**, so VS Code shows
an ordinary verified breakpoint that simply never fires. Nothing on screen
distinguishes it from a line that was never executed, which sends you looking
for the wrong bug: whether the code runs, whether the session is attached,
whether the call path is what you thought. `rumoca-phase-dae` was added
2026-07-28 after exactly that detour, tracing where `__pre__.overSpeed` is
created.

#### 3. The GPU backend and long pauses

A debugger freezes **every** thread, including the egui UI thread, and live
trace pauses are long by design — the whole point is to sit and study. A D3D12
device does not reliably survive that. On resume the next paint fails and
egui-wgpu panics on the main thread, killing HRW with exit code 101:

```
egui-wgpu-0.35.0/src/renderer.rs:981
Failed to create staging buffer for index data.
Index count: 8508. Required index buffer size: 34032.
Actual size 480024 and capacity: 480024 (bytes)
```

The reported buffer is ~14x larger than required, so this is device loss, not a
sizing bug. Because the panic is on the main thread, the process exits rather
than losing a worker — which is why the symptom was "visuals froze, then HRW
died", not a stack trace in HRW code.

The launch configs set `WGPU_BACKEND=gl`, which resolves it. The scope is
deliberate: the hazard exists only under a debugger, so normal `cargo run -p hrw`
keeps the faster default backend. Note that rust-analyzer's "Debug" CodeLens
builds its own configuration and will **not** pick this up — launch live trace
sessions from the launch-configuration dropdown.

Panic output goes to the debuggee's stderr, which under CodeLLDB is the
integrated terminal, **not** the Debug Console. Looking in the wrong pane makes
these crashes appear silent.

#### 4. All-threads stepping

VS Code's F10/F11 step the *selected thread*. That is wrong for live trace: the
visuals are painted by the UI thread, so stepping only the algorithm thread
leaves the animation stale, and live trace degrades into replay with extra
steps. The render window is the `sleep(frame_delay)` in `LiveTrace::push` —
Continue (F5) crosses it with every thread running, which is why Continue
updates the animation.

The lldb launch config defines aliases, typed in the Debug Console:

| Alias | Command |
|-------|---------|
| `ns`  | `thread step-over -m all-threads` |
| `si`  | `thread step-in -m all-threads` |
| `so`  | `thread step-out -m all-threads` |

These previously lived in a user-level `~/.lldbinit` on the Linux machine and
were lost in the platform move; they are in version control now. LLDB also
offers `while-stepping`, which runs other threads only during single-stepping
portions — `all-threads` is the mode that gives the UI thread real wall-clock
time. **Status: implemented but not yet verified end to end.**

#### 5. Debug adapter

Two launch configurations exist, and both work now that the anchor is fixed:

- **CodeLLDB** (`type: lldb`, extension `vadimcn.vscode-lldb`) — the primary.
  Has Rust-aware formatters and the thread-run-mode control the aliases need.
  Cargo integration is built in, so no separate build task.
- **cppvsdbg** (extension `ms-vscode.cpptools`) — Microsoft's debugger, added
  while CodeLLDB was suspected of misreading PDB. It reads PDB natively and
  proved the fault was in the *binary*, not either reader. No Cargo integration,
  hence the explicit `program` path and the `preLaunchTask` in `tasks.json`. It
  is worth keeping as a cross-check: it reports moved breakpoints honestly,
  where CodeLLDB silently kept a stale entry.

Note that debuggers skip a function's prologue, so a breakpoint requested on a
`pub fn` signature line resolves to the first statement line (`exact_match = 0`
in LLDB's `breakpoint list`). That is correct behavior, not a fault — but it
used to produce a phantom duplicate: the bridge asked for the signature line
while the debugger placed the breakpoint one line lower, so a bridge-armed
breakpoint and a hand-set one at the same place looked like two locations.

Two changes remove it. `bridge::find_live_trace_line` now targets the anchor's
first body *statement* — located structurally (signature → opening brace →
first non-blank, non-comment line) rather than by a hard-coded offset, so it
survives edits to the anchor. And the extension's `isDuplicate` checks all of
`vscode.debug.breakpoints` rather than only the ones it armed, so a breakpoint
you set by hand suppresses the bridge's. That is also safer on the way out:
`handleRemove` only removes breakpoints the extension added, so a hand-set
breakpoint survives the end of the live session.

#### 6. Diagnostic commands

In the CodeLLDB Debug Console:

| Command | What it answers |
|---------|-----------------|
| `image lookup -r -n live_trace_breakpoint` | Is the anchor at its own address with its own file:line, or folded onto another function? |
| `breakpoint list` | Did the breakpoint resolve, and to which address and line? |
| `thread list` | Which thread is stopped, and what is `frame_index`? |
| `help thread step-out` | Confirms which run-mode flags a step command accepts |

#### 5. Rumoca's compile cache — why a phase breakpoint fires only once

`CompiledSourceRoot` holds `compile_cache: Mutex<IndexMap<String, PhaseResult>>`,
keyed by model name. `Session::compile_model_strict_reachable_with_recovery`
consults it, so **the second and later compiles of the same model return a
cached result and the phases never run**.

HRW does remove and re-add the document before each compile (`worker.rs`, "so
the session treats it as new"), which defeats *document* caching — but not this
one.

The failure is deeply confusing because it is **selective**. Breakpoints in
`rumoca-phase-structural` keep firing on every reselect, because HRW calls
`build_structural_report` **itself**, on the returned DAE, outside the cached
call. So one phase crate stops and another does not, with identical build flags
— which reads exactly like a debug-info or adapter defect and sends you to §1
and §2. It cost four rounds of misdiagnosis on 2026-07-28, including a wrongly
blamed `opt-level` and a wrongly blamed PDB reader.

**How to tell in one step:** load a model this process has **not** compiled yet.
A first-ever compile cannot be a cache hit, so the phase breakpoints fire. If
they do, nothing is wrong with the debugger.

`Session::compile_model_strict_reachable_uncached_with_recovery` is the public
escape hatch; HRW does not currently use it (see `DECISIONS.md`, 2026-07-28).

#### 7. Failure signatures

| Symptom | Cause |
|---------|-------|
| Pause in an unrelated crate during startup, "Paused on breakpoint" | Anchor folded onto another empty function (§1) |
| Breakpoint never verifies; no gutter dot while the session is live | Same — the adapter cannot bind it meaningfully |
| Locals show `<optimized out>` | `opt-level` above 0 for the crate (§2) |
| Breakpoint looks verified but never fires, and the code definitely runs | Line-table entry dropped at `opt-level` above 0 — add the crate to §2 |
| Breakpoint in a *compiler phase* never fires, but one in `rumoca-phase-structural` does | Rumoca's **compile cache** (§5) — the phase only runs on the first compile of that model per process |
| Breakpoint in HRW's own code will not bind at all | The `hrw` package is still at `opt-level = 1`; no override (§2) |
| Visuals freeze, then exit code 101, nothing in the Debug Console | GPU device loss after a long pause (§3); look in the terminal |
| First Debug click misses, second works | Cold line-table resolution — fixed by the pre-warm |
| Stepping works but the animation does not advance | Single-thread stepping; use `ns`/`si`/`so` (§4) |

**Why re-running the algorithm (clicking Debug again) is safe — a Rust ownership lesson.**
The Debug button spawns a new algorithm thread that re-runs the algorithm.
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
  `LiveTrace<MatchingFrame>` is `Send` because its `Sender<F>` and
  `Arc<AtomicUsize>` are both `Send`. The `Receiver<F>` held by the UI is
  also `Send`. If `MatchingFrame` contained a `Rc` or a raw pointer, the
  compiler would refuse to let it cross the thread boundary. You never have to
  reason about whether `LiveTrace` is thread-safe — the compiler verifies it
  structurally from its fields.

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

**Matching animation** (`matching_anim.rs`, ~550 lines): replays Kuhn's
augmenting-path algorithm on the incidence matrix. Each frame highlights the
current equation, explored edges, found/failed paths, and confirmed matches
with step-by-step descriptions using readable equation text.

**Tarjan SCC animation** (`tarjan_anim.rs`, ~550 lines): replays Tarjan's
strongly connected component algorithm on the dependency graph (derived from
the matching result). Nodes are colored by DFS state (on stack, in discovered
SCC) and edges are classified as tree/back edges.

**Reduction animation** (`reduction_anim.rs`, ~300 lines): replays the
constrained-dummy-derivative and missing-state-derivative index reduction
algorithms. Each frame shows the current step (begin state search, differentiate
constraint, demote state, round complete) with before/after equation text and a
running table of demoted states. Live mode receives the raw `Dae` (cloned from the
worker's compilation result and stored in `App::cached_dae`) and spawns a thread
running both reduction passes with a shared `LiveTrace<IndexReductionFrame>`.

### Index reduction summary (`reduction_view.rs`, ~650 lines)

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

### Crash and diagnostic log (`diagnostics.rs`, ~370 lines)

**The problem:** when HRW dies, the evidence dies with it. HRW is a *windowed*
application — a Rust panic prints to stderr, and launched from the VS Code
debugger or from Explorer there is frequently no stderr anyone reads. This has
cost real diagnostic time twice: a panic on clicking an identifier in the
specimen source view (2026-07-28), and an `exit code 101` from egui-wgpu's
staging buffer during a long debugger pause. Both were eventually solved, but
only because each happened to be *re-creatable*. A crash in the paint path, in a
drag, or one depending on window or GPU state would have left nothing but a
description of what was clicked.

**Who it is for:** Claude, not the user. These files are not error reports and
are not tuned for readability or brevity — they exist so a reasoner can diagnose
a failure it did not witness. Same principle as the bridge's `focus.json`; see
`DECISIONS.md` (2026-07-28).

**Why the backtrace is the less useful half.** A message and backtrace say
*where* the process died; they rarely say *why the app was there*. Reconstructing
that from a sentence describing what the user clicked is the expensive part —
and every field of it already lives in `App`. `App::diagnostic_snapshot` carries
the specimen, model, stage tab, detail view, navigation stack, the assembled
noun (what is pointed at, what is followed, the sequence counters, the last
emission error), the live-trace arming state, which animation is on screen and
at which frame, which stage IRs exist, and a few counts. The field list is
literally the 2026-07-28 debugging session's findings turned into code.

**Two files, because there are two kinds of death:**

| File | Written | Contents |
|------|---------|----------|
| `crash-<utc>.json` | from the panic hook | panic message, location, thread, backtrace, snapshot, actions, log tail, build |
| `session.json` | on every recorded user action | everything above except the panic |
| `previous-session.json` | at startup, by rotation | the prior run's `session.json` — the file to read after a hard death |

`session.json` exists for the deaths that run **no hook at all** — a stack
overflow, a driver `SIGSEGV`, a hard kill. It survives them because it was
already on disk. Rust's `exit code 101` *is* a panic, so the egui-wgpu class of
failure is covered by the hook.

**Startup rotates it to `previous-session.json`.** Without that, the file
defeats its own purpose: the natural response to a hookless death is to launch
HRW again, and the restart records `"HRW started"` — rewriting the file and
erasing the evidence of the death before anyone reads it. **So after a hard
death, `previous-session.json` describes it and `session.json` describes the
restart.** One generation is kept rather than a timestamped archive, because the
interesting file is always the run that just died and a pile of session files
would bury the crash files that matter.

`Help ▸ Write diagnostic snapshot` produces the same content on demand, for a
session that is misbehaving without dying. A wrong-looking view needs identical
evidence to a crash, and writes none of its own.

**The action ring buffer is the part that makes a file actionable.** A crash's
cause is usually *the action before last*, not the state after. State alone is a
still photograph; the buffer is a reproduction script — *selected
MotorWithBrake, switched to Resolve, followed `overSpeed`* can be replayed, and
a final state cannot. Actions are recorded at four choke points (specimen open,
stage-tab click, point-at, follow-change) rather than at every UI site, because
those four are the state changes that reach the compiler and the bridge.

**Design constraints, for anyone editing it:**

- **A panic hook cannot borrow `App`** — it is `'static + Send + Sync` and runs
  on whichever thread panicked. So `ui()` pushes a snapshot into a global each
  frame and the hook reads that global. The snapshot is rebuilt every frame,
  not throttled: its entire value is describing state *at the instant of the
  crash*, and a stale one misdirects exactly when it matters.
- **The hook must never panic**, or the process aborts and the file is lost.
  Every step is fallible-and-ignored: `try_lock` rather than `lock` (a panic
  while the app holds the lock would otherwise deadlock against itself —
  `std::sync::Mutex` is not reentrant), poisoned locks are recovered, I/O errors
  discarded.
- **The previous hook still runs**, so stderr keeps its normal message.
- **Log entries are mirrored on arrival**, once per `FromWorker::Log`, never by
  cloning the log view's `Vec` into a per-frame snapshot.
- **`examples/crash_probe.rs` verifies the panic path.** It cannot be a unit
  test: the test harness installs its own panic hook and catches the unwind, so
  a real process-killing panic is unobservable from inside `cargo test`.

**Build identity** comes from `build.rs`, which already stamped the workspace
git rev and now also stamps `HRW_GIT_DIRTY`. Without the dirty flag the rev is
actively misleading mid-session — it names a commit whose code is not what ran.

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

### Colors (`colors.rs`, ~150 lines)

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
their call sites. The regression test suite (270 tests) guards against silent
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
- The `LiveTrace` uses a channel (`Sender<F>` on the producer,
  `Receiver<F>` on the consumer) — no explicit locks, only atomics and
  the channel's internal synchronization. `Send` trait bounds are checked
  at compile time. A data race is a compile error, not a runtime bug.
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
