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
│   ├── log_view.rs     # Timestamped compilation/simulation log panel
│   └── field_help.rs   # Build-time-embedded IR field documentation
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
                         │            / plot / log          │
                         │          right panel:            │
                         │            field help / specimen │
                         │            info / sim controls   │
                         │                                  │
                         │  click ──► bridge::emit_focus()  │
                         │            writes focus.json     │
                         └─────────────────────────────────┘
```


## 4. The worker thread

**File:** `worker.rs` (~3000 lines, the largest module)

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
Thereafter, each specimen recompile re-resolves incrementally (~0.3s) rather than
re-parsing the entire library.

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
`times`, `names`, and `data[var][t]` that carries no Rumoca types into the UI.

### JSON serialization strategy

Each stage serializes only the **user model's** IR (a few KB), not the whole resolved
aggregate (~430 MB with the full MSL). The interchange format is `serde_json::Value`
(a generic JSON tree) because not all Rumoca IR types implement `Serialize`, and JSON
lets the generic tree inspector render any stage without knowing its Rust type.


## 5. The UI shell

**File:** `app.rs` (~2000 lines)

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

`App` holds all application state, organized into 13 field groups:

1. **Worker** — the `Worker` handle (send/receive channels)
2. **Library config** — MSL source-root paths, load status
3. **Specimen list** — directory path, file list, purpose hints
4. **Compilation results** — a `StageBundle` (all 10 pipeline stages in one struct), model name, def_index
5. **Navigation** — the "go to definition" stack for browsing library classes
6. **Bridge** — Claude Code capture state (monotonic `ask_seq` counter)
7. **View toggles** — Settings, Help, About window visibility
8. **Field help** — the embedded doc-comment lookup table
9. **Custom views** — pan/zoom cameras for spy-plot and incidence views
10. **Log** — timestamped compilation/simulation log entries
11. **Simulation** — `SimData`, plot flags, sim-in-progress state
12. **Cached path checks** — `narrative_exists` avoids per-frame `Path::exists()`
13. **Cached layout** — `cached_specimen_width` avoids per-frame `layout_no_wrap`
14. **Cached views** — `cached_spy_plot`, `cached_incidence`, `cached_reduction`
    (`Option<Option<T>>` — outer = cache state, inner = parse result) avoid per-frame
    re-parsing of structural report JSON; invalidated on `Compiled`

### Panel layout

```
┌──────────────────────────────────────────────────────────────┐
│                        Top panel                             │
│  Tab bar: Parse│Resolve│...│Simulation  [▶Play] [⚙Settings]  │
├──────────┬───────────────────────────────┬───────────────────┤
│  Left    │      Center panel             │    Right panel    │
│  panel   │                               │                   │
│          │  Tree inspector / Spy-plot /   │  Specimen info /  │
│ Specimen │  Incidence / Reduction /       │  Field help /     │
│ list     │  Simulation plot /            │  Simulation       │
│          │  Log                           │  controls         │
│          │                               │                   │
├──────────┴───────────────────────────────┴───────────────────┤
│                       Bottom panel                           │
│  Status line: compiling… / bridge status / error messages     │
└──────────────────────────────────────────────────────────────┘
```

Panels are added in **top → bottom → left → right → center** order. In egui,
each panel claims space from what remains, so order determines layout.

**Panel visibility toggles.** Both side panels can be hidden via the **View**
menu (checkboxes for "Specimens panel" and "Help panel"). When hidden, the
`CentralPanel` reclaims the space — useful during live debug sessions where the
animation view benefits from full width. The bools `show_left_panel` /
`show_right_panel` (both default to `true`) gate whether the `Panel::left` /
`Panel::right` calls run at all; egui's `CentralPanel` automatically fills
whatever space the side panels don't claim.

### Right panel routing

The right panel has three modes, selected by a state machine:

1. **Specimen info** — shown when the user hasn't clicked any stage tab yet
   (`!stage_clicked && nav.is_empty()`). Displays the specimen's name, model
   name, purpose, and a link to read the specimen narrative.

2. **Simulation controls** — shown when the Simulation tab is active and no
   navigation is open. Displays plot-control hints and the run button.

3. **Field help** — shown after the user clicks any stage tab. Shows the
   clicked field's doc-comment and a link to the relevant compiler-phase chapter.

The `stage_clicked` flag transitions from mode 1 to mode 3 — once the user
clicks a stage tab, they've moved from "browsing the specimen" to "inspecting
the IR," and the right panel shifts accordingly.

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

**File:** `tree.rs` (~350 lines)

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

### Canvas scaffold (`canvas.rs`, ~380 lines)

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

### BLT spy-plot (`spyplot.rs`, ~350 lines)

Visualizes the Block Lower Triangular (BLT) decomposition of the structural
analysis. Each diagonal block is a group of equations that must be solved together:

- **Scalar blocks** (size 1) — drawn as single cells; these can be solved
  independently by forward substitution
- **Coupled blocks** (size > 1) — drawn as outlined rectangles; these represent
  algebraic loops that require an iterative solver (Newton)

Blocks are laid out consecutively along the diagonal. Colors distinguish block types
(blue for scalar, orange for coupled with tearing). Hover shows the block's equations
and tearing report; click captures the block into the bridge.

### Incidence matrix (`incidence_view.rs`, ~400 lines)

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
   separate algorithm thread runs. The user sets a breakpoint on
   `live_trace_breakpoint` in the VS Code debugger and steps through the
   algorithm code — after each frame push, a 20ms delay lets the UI render,
   then the breakpoint fires. Started via the "Debug" button in the UI; the
   button reappears after the session finishes for re-runs.

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

**Matching animation** (`matching_anim.rs`, ~600 lines): replays Kuhn's
augmenting-path algorithm on the incidence matrix. Each frame highlights the
current equation, explored edges, found/failed paths, and confirmed matches
with step-by-step descriptions using readable equation text.

**Tarjan SCC animation** (`tarjan_anim.rs`, ~530 lines): replays Tarjan's
strongly connected component algorithm on the dependency graph (derived from
the matching result). Nodes are colored by DFS state (on stack, in discovered
SCC) and edges are classified as tree/back edges.

### Index reduction summary (`reduction_view.rs`, ~500 lines)

A scrollable panel (not a canvas) summarizing what the Pantelides / dummy-derivative
funnel did: which states were demoted, which equations were differentiated, which
variables were eliminated. Renders as sections: summary → funnel steps → demoted
states → differentiated equations → trivial eliminations. Color-coded: green for
successful steps, red for stopped, neutral for no-ops.


## 8. The Claude bridge

**File:** `bridge.rs` (~800 lines)

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
- **`arm it`** — Claude finds the Rumoca source line where the captured field is
  set and writes a breakpoint into `.vscode/launch.json`


## 9. Supporting modules

### Field help (`field_help.rs`, ~160 lines)

A two-tier help system:

- **Fast tier (this module):** The `///` doc comments that Rumoca's authors wrote
  on IR fields, extracted at build time into `field_help.json` and embedded via
  `include_str!`. Keyed by field name, shown instantly in the right panel on click.
  No AI, no latency.

- **Specific tier (the bridge):** "Why did THIS particular field get this value?" —
  requires Claude to reason about the specimen, the IR, and the phase code.

The module also maps each stage to its `docs/compiler-phases` chapter, providing
the "Read: Phase N" button in the right panel.

### Expression pretty-printer (`expr_format.rs`, ~250 lines)

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

### Colors (`colors.rs`, ~90 lines)

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

### Log view (`log_view.rs`, ~130 lines)

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
their call sites. The regression test suite (149 tests) guards against silent
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
