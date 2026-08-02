//! The observatory shell — the top-level egui application.
//!
//! ## Immediate-mode UI in a nutshell
//!
//! egui is an **immediate-mode** GUI: every frame, the framework calls our
//! [`App::ui()`] method, and we rebuild the entire UI from scratch — buttons,
//! labels, panels, trees, plots, everything. There is no retained widget tree
//! that persists between frames; instead, all durable state lives in the [`App`]
//! struct itself (stage results, navigation stack, view flags, etc.), and the UI
//! code reads/writes those fields each frame to decide what to show and how to
//! react to clicks. This means:
//!
//! - **Layout is code, not data.** You won't find XML/HTML templates — the UI
//!   is specified by Rust function calls (`ui.label(…)`, `ui.button(…)`, etc.)
//!   executed every frame.
//! - **State mutations happen in-line.** A button click is detected the same
//!   frame the button is drawn; a `if button.clicked() { self.flag = true; }`
//!   both renders the button and handles the event, in one pass.
//! - **The App struct IS the model.** Any value that must survive across frames
//!   (the selected specimen, compilation results, which panel is open) must be
//!   a field on `App`. Local variables inside `ui()` are gone after the frame.
//!
//! Eframe shell (charter §4.2.1, §4.4): a file picker over the specimen
//! directory, a library-path (source-root) configuration for dependency
//! resolution, and the generic serde-value tree inspector showing each stage's
//! IR for the selected model.

use std::path::{Path, PathBuf};

use eframe::egui;

use std::collections::{BTreeMap, HashMap, HashSet};
use serde_json::{json, Value};

// The bridge module handles communication with Claude Code (the AI assistant
// running in a terminal alongside this app): we write JSON "focus" files that
// Claude reads to understand what the user is looking at.
use crate::bridge::{self, Ask, Focus, Seg};
use crate::equation_sheet;
use crate::diagnostics;
use crate::identifier_index;
// Canvas provides a pan/zoom camera for custom-painted views (spy-plot,
// incidence matrix). It tracks the transform and handles drag/scroll input.
use crate::canvas::Canvas;
use crate::playback::Animated;
use crate::LiveState;
use crate::field_help;
use crate::incidence_view;
use crate::matching_anim;
use crate::alias_anim;
use crate::connection_anim;
use crate::ic_plan_anim;
use crate::tarjan_anim;
use crate::tearing_anim;
use crate::log_view;
use crate::pre_lowering_anim;
use crate::reduction_anim;
use crate::reduction_view;
use crate::spyplot;
use crate::tree;
// The worker module runs compilation and simulation on a background thread so
// the UI never blocks. Communication is via channels: we send `ToWorker`
// commands and receive `FromWorker` results. `Stage` holds one pipeline stage's
// output (its serde_json::Value IR + optional error note).
use crate::worker::{
    discontinuity_segments, DefInfo, DefKind, FromWorker, LogEntry, SimData, Stage, StageBundle, StageKind,
    ToWorker, Worker,
};

/// Initial UI zoom (fonts + spacing) — readable on a hi-dpi display. Adjustable
/// live via Settings (or Ctrl +/−); egui's `zoom_factor` is the idiomatic knob.
const DEFAULT_ZOOM: f32 = 2.0;

/// How often tour mode stats `.hrw-bridge/tour.md`. A quarter second is well
/// under human notice and keeps filesystem work out of the paint path.
const TOUR_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(250);

/// How often the scratch specimen directory is re-listed. Slower than the tour poll:
/// a specimen appearing a second late is imperceptible, and a rescan re-reads every
/// specimen's `// purpose:` line.
/// How many paints a pending frame seek keeps trying for before giving up.
///
/// Two would do — the target view needs one paint to build its animation — but a small
/// margin costs nothing and covers a view that defers construction one frame further.
const SEEK_ATTEMPTS: u8 = 5;

const SCRATCH_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(1000);

/// Default specimen directory: `specimens/` next to this crate's manifest.
const DEFAULT_SPECIMEN_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/specimens");

/// Default library source roots: the staged reference MSL 4.1.0, one path per
/// line. Editable at runtime.
const DEFAULT_LIBRARIES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/vendor/msl/Modelica 4.1.0\n",
    env!("CARGO_MANIFEST_DIR"),
    "/vendor/msl/ModelicaServices 4.1.0\n",
    env!("CARGO_MANIFEST_DIR"),
    "/vendor/msl/Complex.mo",
);

// ---- Layout constants ----
// Named fractions for panel sizes and splits, replacing inline magic numbers.

/// Fraction of available width used by the left panel (tour text or specimen
/// list) in Tour and Specimen modes.
const LEFT_PANEL_WIDTH_FRACTION: f32 = 0.4;

/// Fraction of the left panel's height reserved for the specimen file list
/// (the top third). The remaining two-thirds show source or purpose note.
const SPECIMEN_LIST_HEIGHT_FRACTION: f32 = 1.0 / 3.0;

/// Fraction of available width given to the source column in the
/// Flatten → SourceMap split view.
const SOURCE_MAP_SPLIT_FRACTION: f32 = 0.45;

/// Fraction of available height used by the trajectory plot when solver
/// diagnostics are shown below it on the Simulation tab.
const TRAJECTORY_PLOT_HEIGHT_FRACTION: f32 = 0.65;

/// How to render the Structural / Index-reduction stages: the custom BLT
/// spy-plot, the incidence matrix, the reduction process
/// summary (Index reduction only), or the generic serde tree.
///
/// On the Index Reduction tab, comparative views (SpyPlot, Incidence) render
/// in a Before/After split; Summary, Animate, and Tree are full-width.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StructuralView {
    Summary,
    SpyPlot,
    Incidence,
    MatchingAnim,
    TarjanAnim,
    /// Replay of tearing an algebraic loop open. Shares this enum (rather than
    /// getting a stage of its own) because tearing is part of what the
    /// Structural stage reports — its output is already in `blocks`.
    TearingAnim,
    /// Reveal of the alias eliminations. Index Reduction only -- that is the
    /// stage whose report carries them.
    AliasAnim,
    Animate,
    Tree,
}

impl StructuralView {
    /// Every variant, so the noun/verb parity test can iterate without naming them.
/// **Add new variants here** — that is what makes the omission loud instead of silent.
    #[cfg(test)]
    const ALL: &'static [StructuralView] = &[
        StructuralView::Summary,
        StructuralView::SpyPlot,
        StructuralView::Incidence,
        StructuralView::MatchingAnim,
        StructuralView::TarjanAnim,
        StructuralView::TearingAnim,
        StructuralView::AliasAnim,
        StructuralView::Animate,
        StructuralView::Tree,
    ];
}

/// Sub-tab selector for the Initialization stage: the IR tree, or a walk of the
/// initial-condition solve plan.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum InitView {
    #[default]
    Tree,
    IcPlan,
}

impl InitView {
    /// Every variant — see `StructuralView::ALL`.
    #[cfg(test)]
    const ALL: &'static [InitView] = &[
        InitView::Tree,
        InitView::IcPlan,
    ];
}

fn init_view_name(v: InitView) -> &'static str {
    match v {
        InitView::Tree => "Tree",
        InitView::IcPlan => "IcPlan",
    }
}

/// Sub-tab selector for the Flatten stage: readable equation sheet, source
/// traceability map, or the generic serde tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FlattenView {
    Equations,
    SourceMap,
    /// Replay of connection expansion (MLS §9) — where most of a flat model's
    /// equations come from.
    Connections,
    Tree,
}

impl FlattenView {
    /// Every variant — see `StructuralView::ALL`.
    #[cfg(test)]
    const ALL: &'static [FlattenView] = &[
        FlattenView::Equations,
        FlattenView::SourceMap,
        FlattenView::Connections,
        FlattenView::Tree,
    ];
}

/// The general rule for how context gets assembled — the empty bar's hover.
///
/// The visible line names one gesture that works *right now*; this names the
/// rule behind all of them, which holds regardless of what is on screen. Both
/// are needed: the state-specific hint gets you moving, and the rule explains
/// why the same click does different things in different places — the thing
/// Doug found genuinely confusing when he first met it.
const EMPTY_CONTEXT_RULE: &str = "Context is assembled from two things: one node you POINT AT, \
and one identifier you FOLLOW. Which one a left-click gives you depends on what the view shows. \
Where things appear as names \u{2014} the specimen source, the variable grid \u{2014} left-click \
follows them. Where the view shows IR nodes \u{2014} trees, stage tabs, incidence rows \u{2014} \
left-click points at them, and right-click offers Follow for names the model knows. Hover \
anything clickable and it will say which.";

/// Sub-tab selector for the Events stage: the IR tree, or a replay of `pre()`
/// lowering — where the `__pre__.x` slots the Events IR references get made.
///
/// Events hosts that replay even though the pass belongs to DAE construction:
/// the slots exist *because* of `when` equations, and this is the stage that
/// shows them. A separate `StageKind` would have to be wired into every
/// per-stage system (tabs, diffs, stage files, capture, notebook) to say
/// something that belongs beside what is already here.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum EventsView {
    #[default]
    Tree,
    PreLowering,
}

impl EventsView {
    /// Every variant — see `StructuralView::ALL`.
    #[cfg(test)]
    const ALL: &'static [EventsView] = &[
        EventsView::Tree,
        EventsView::PreLowering,
    ];
}

fn events_view_name(v: EventsView) -> &'static str {
    match v {
        EventsView::Tree => "Tree",
        EventsView::PreLowering => "PreLowering",
    }
}

/// Sub-view names for the capture's `view` section.
///
/// Written out rather than derived from `Debug` because these strings are read
/// by Claude and appear in `docs/context-assembly.md`; a `#[derive(Debug)]`
/// rename would silently change the emitted vocabulary. The enums themselves
/// stay display-only.
fn structural_view_name(v: StructuralView) -> &'static str {
    match v {
        StructuralView::Summary => "Summary",
        StructuralView::SpyPlot => "SpyPlot",
        StructuralView::Incidence => "Incidence",
        StructuralView::MatchingAnim => "MatchingAnim",
        StructuralView::TarjanAnim => "TarjanAnim",
        StructuralView::TearingAnim => "TearingAnim",
        StructuralView::AliasAnim => "AliasAnim",
        StructuralView::Animate => "Animate",
        StructuralView::Tree => "Tree",
    }
}

fn flatten_view_name(v: FlattenView) -> &'static str {
    match v {
        FlattenView::Equations => "EquationSheet",
        FlattenView::SourceMap => "SourceMap",
        FlattenView::Connections => "Connections",
        FlattenView::Tree => "Tree",
    }
}

/// What the bottom two-thirds of the Specimen mode LHS shows.
///
/// `Debug` so the crash log can name it — the derived variant name is exactly
/// the right thing to record, and hand-writing a second mapping would let the
/// two drift.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum SpecimenDetail {
    /// The specimen's Modelica source text.
    #[default]
    Source,
    /// The specimen's purpose note from
    /// `docs/specimen-notebook/<Model>/purpose.md`. Renamed from `narrative.md`
    /// 2026-07-29 when the stage-by-stage prose was retired — a file called
    /// `narrative.md` containing no narrative is the kind of stale signal that
    /// retirement was meant to remove. See `docs/ideas.md` #42.
    /// The specimen's purpose note (`purpose.md`). Was `Narrative` until
    /// 2026-07-29; the stage-by-stage prose it named is retired.
    Purpose,
}

/// One level of "go to definition" navigation: a class extracted from the
/// resolved tree, shown in the same generic tree the specimen stages use.
///
/// Navigation forms a stack: clicking "Go to definition" pushes a `NavEntry`,
/// "Back" pops one, and "Specimen" clears the stack entirely. Each entry
/// carries its own `def_index` so the tree inspector can resolve DefIds
/// (numeric cross-references) to human-readable class names within that class.
struct NavEntry {
    name: String,
    /// The serde_json representation of this class's IR — the same format every
    /// stage uses, so the generic tree inspector renders it without any special
    /// logic.
    value: serde_json::Value,
    /// Maps numeric DefIds (compiler-internal identifiers) to their resolved
    /// class names, enabling the tree view to show "type_def_id: 27579 ->
    /// model Resistor" rather than a bare number.
    def_index: BTreeMap<u64, DefInfo>,
}

/// Which left-panel content is active. Determines both what occupies the LHS
/// of the window and whether the LHS is visible at all.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UiMode {
    /// Guided tour: LHS shows the tour document, RHS shows stage tabs.
    #[default]
    Tour,
    /// Specimen exploration: LHS shows specimen list + purpose note, RHS shows stage tabs.
    Specimen,
    /// Debugger-assisted: LHS hidden, stage tabs fill the window. VS Code alongside.
    Debug,
}

/// The entire application state. In immediate-mode UI, this struct IS the
/// application — every frame, `ui()` reads and writes these fields to decide
/// what to render and how to react. Fields are grouped by concern:
///
/// 1. **Worker** — the background thread that runs the compiler and simulator.
/// 2. **Library config** — MSL source roots the compiler needs for `import`.
/// 3. **Specimen list** — the directory of `.mo` files the user picks from.
/// 4. **Compilation results** — one `Stage` per pipeline phase, plus selection state.
/// 5. **Navigation** — the "go to definition" stack for drilling into classes.
/// 6. **Bridge** — communication with Claude Code (the AI assistant in the terminal).
/// 7. **View toggles** — Settings/Help/About windows.
/// 8. **Field help** — generic documentation for IR fields (the fast, no-AI tier).
/// 9. **Custom views** — spy-plot/incidence cameras for the Structural stages.
/// 10. **Log** — the compilation event log.
/// 11. **Simulation** — on-demand model execution and plotting.
pub struct App {
    // ---- 1. Worker thread ----
    // Compilation and simulation run off the UI thread so the app stays
    // responsive. We send commands (`ToWorker`) and poll results (`FromWorker`)
    // each frame via `drain_worker()`.
    worker: Worker,

    // ---- 2. Library configuration ----
    // Modelica models import classes from external libraries (mainly MSL).
    // These fields hold the library source-root paths the user has configured
    // and the status of the last library-load operation.
    libraries_text: String,
    library_status: String,
    libraries_busy: bool,

    // ---- 3. Specimen directory + file list ----
    // The left panel shows a list of `.mo` specimen files from this directory.
    specimen_dir: String,
    files: Vec<PathBuf>,
    // Per-specimen one-line purpose hint (the `// purpose:` comment), scanned at
    // rescan so the specimen list reads as an index of what each one teaches.
    specimen_purposes: HashMap<PathBuf, String>,
    /// Which entries of `files` came from the gitignored scratch directory rather
    /// than the curated corpus (ideas #42). Held as a set rather than a flag on the
    /// path so `files` stays a plain `Vec<PathBuf>` that everything else can use.
    scratch_specimens: HashSet<PathBuf>,
    /// True when [`Self::selected`] names a **library model** rather than a file
    /// on disk. See [`Self::open_library_model`] for why the two share a field.
    selected_is_library: bool,
    /// The MSL corpus, read from the survey once on first use.
    ///
    /// **The survey is the corpus definition** — the same rule `fidelity_msl`
    /// follows. Re-enumerating models from the session here would be a second
    /// definition of "which models exist", and the two would drift the moment
    /// MSL moved.
    corpus: Option<Vec<crate::survey::SurveyRow>>,
    /// Filter text for the list. **Narrows every source**, so a curated specimen
    /// and a corpus model are found the same way.
    list_filter: String,
    /// A scratch specimen appeared since the list was last drawn.
    ///
    /// Holds the HRW section open, which is otherwise collapsed at startup. A
    /// scratch specimen is written *for the question being asked right now*, so
    /// it is the one file that must not need a click to be seen. Sticky for the
    /// session rather than one frame: it marks "this session has scratch work",
    /// and a section that sprang open and shut again would read as a glitch.
    scratch_arrived: bool,
    /// Scratch specimens skipped because a curated specimen already owns the name.
    /// Reported rather than silently resolved: loading the wrong model would have
    /// Claude reason confidently about source Doug is not looking at.
    shadowed_specimens: Vec<String>,
    scan_error: Option<String>,

    // ---- 4. Current selection + compilation results ----
    // When the user clicks a specimen, `selected` records the path and
    // `compiling` goes true. The worker streams `CompileProgress` messages
    // (partial results) and finally `Compiled` (all stages done). Each `Stage`
    // holds the JSON IR + an optional note/error for that pipeline phase.
    selected: Option<PathBuf>,
    compiling: bool,
    /// The Modelica model name extracted from the parsed file (e.g.
    /// "BouncingBall"). `None` until parsing succeeds.
    model: Option<String>,
    /// All ten pipeline-stage results. Each stage holds a `Stage` (an optional
    /// JSON IR tree + optional note/error). Populated progressively during
    /// compilation via `FromWorker::CompileProgress` and finalized by
    /// `FromWorker::Compiled`.
    stages: StageBundle,
    /// Which stage tab is currently selected (determines what the center panel
    /// shows). Updated when the user clicks a tab or after compilation finishes
    /// (auto-selects the furthest successful stage).
    stage: StageKind,
    // Resolved identity of every DefId referenced in the current model's IR.
    // Shared across all stages; populated by the final `Compiled` message.
    def_index: BTreeMap<u64, DefInfo>,

    // ---- 5. "Go to definition" navigation stack ----
    // Empty means we're showing the specimen's own stages; non-empty means we
    // navigated into a library class (e.g. Resistor's resolved definition).
    // Renders as a breadcrumb trail: model > class1 > class2.
    nav: Vec<NavEntry>,
    nav_loading: Option<String>,
    nav_error: Option<String>,

    // ---- 6. Claude bridge ----
    // The "capture" system writes JSON files that Claude Code reads to
    // understand what the user is looking at. `context_seq` is a monotonic counter
    // so each capture gets a unique ID. Confirmation lives in the Context Bar,
    // not the status bar — see `notice`.
    context_seq: u64,
    /// A transient one-line notice for the status bar.
    ///
    /// **No longer the bridge's confirmation channel.** Renamed from
    /// `bridge_status` when the Context Bar took that job: the bar names the
    /// point persistently, so a second, staler description of the same thing
    /// could only disagree with it. What is left is genuinely transient and
    /// belongs nowhere else — "specimen not found", "diagnostic written to …",
    /// a stage-file write failure.
    notice: Option<String>,

    // ---- 7. Panels and windows toggled from the menu bar ----
    ui_mode: UiMode,
    specimen_detail: SpecimenDetail,
    // egui's `Window::open(&mut bool)` pattern: the bool controls visibility,
    // and the window's close button flips it back to false.
    show_settings: bool,
    show_help: bool,
    show_about: bool,

    // ---- 8. Generic field help ----
    // `field_help` is a compile-time HashMap<field_name, explanation> loaded
    // from a generated help table, delivered as hover tooltips on tree nodes.
    field_help: HashMap<String, String>,

    // ---- 9. Custom views for Structural/Index-reduction stages ----
    // These stages have a spy-plot and incidence-matrix visualization in
    // addition to the generic JSON tree. Each custom view has its own `Canvas`
    // (pan/zoom camera state).
    flatten_view: FlattenView,
    events_view: EventsView,
    init_view: InitView,
    highlighted_eq_row: Option<usize>,
    highlighted_source_line: Option<u32>,
    structural_view: StructuralView,
    spy_canvas: Canvas,
    incidence_canvas: Canvas,

    // ---- 10. Compilation log ----
    // Timestamped events streamed from the worker thread (phase start/end,
    // timing, diagnostics). `viewing_log` is true when the Log button (left of
    // the stage tabs) is active; auto-selected when a specimen is opened so
    // the user sees compilation progress before any stage IR is ready.
    log_entries: Vec<LogEntry>,
    viewing_log: bool,
    tracing_enabled: bool,

    // ---- 11. On-demand simulation ----
    // Simulation is NOT a compiler stage — it's triggered by the user pressing
    // "Run". `simulation` is an always-empty `Stage` placeholder so that
    // `current_stage()` can return a `&Stage` for all `StageKind` variants
    // (the Simulation view is actually rendered by `simulation_pane()`, not
    // the generic tree inspector). `sim_data` holds the time-series output
    // from the solver once the run completes.
    simulation: Stage,
    sim_data: Option<SimData>,
    sim_running: bool,
    sim_error: Option<String>,
    sim_t_end: f64,

    // ---- 12. Cached structural views ----
    // Avoids re-parsing `from_report` JSON every frame. Invalidated when
    // `stages` changes (in `drain_worker` on `Compiled`) or when the
    // active stage switches between Structural and IndexReduction (each
    // has its own report data). Outer Option is the cache state (None =
    // not yet computed); inner Option is the parse result (None = report
    // had no data for this view).
    cached_report_stage: Option<StageKind>,
    cached_spy_plot: Option<Option<spyplot::Plot>>,
    cached_incidence: Option<Option<incidence_view::IncidenceMatrix>>,
    cached_reduction: Option<Option<reduction_view::ReductionView>>,
    cached_equation_sheet: Option<equation_sheet::EquationSheet>,
    identifier_index: Option<identifier_index::IdentifierIndex>,
    tracked_identifier: Option<String>,
    cached_matching_anim: Option<Option<matching_anim::MatchingAnimation>>,
    matching_anim_canvas: Canvas,
    cached_tarjan_anim: Option<Option<tarjan_anim::TarjanAnimation>>,
    tarjan_anim_canvas: Canvas,
    index_reduction_frames: Vec<rumoca_phase_structural::dae_prepare::IndexReductionFrame>,
    // Idea #40: replay of `pre()` lowering, on the Events stage. The frames are
    // recorded by re-running DAE construction over the flat model, since the
    // pass runs inside construction and the finished DAE has nothing left to
    // replay. `cached_flat` is kept for the live-debug variant.
    pre_lowering_frames: Vec<rumoca_phase_dae::PreLoweringFrame>,
    /// Connection-expansion replay frames (MLS §9), recorded at compile time.
    connection_frames: Vec<rumoca_phase_flatten::connections::trace::ConnectionFrame>,
    cached_flat: Option<rumoca_ir_flat::Model>,
    cached_pre_lowering_anim: Option<Option<pre_lowering_anim::PreLoweringAnimation>>,
    /// Tearing replay for the current stage tab. `Some(None)` means "computed,
    /// and this model has no algebraic loop" — distinct from `None`, "not yet
    /// computed". Rebuilt on stage change because Structural and Index
    /// Reduction tear *different* DAEs (raw vs reduced).
    cached_tearing_anim: Option<Option<tearing_anim::TearingAnimation>>,
    /// Alias-elimination reveal, built from the Index Reduction report.
    cached_alias_anim: Option<Option<alias_anim::AliasAnimation>>,
    /// Initial-condition plan walk, built from the Initialization report.
    cached_ic_plan_anim: Option<Option<ic_plan_anim::IcPlanAnimation>>,
    /// Connection-expansion replay, built from the recorded frames.
    cached_connection_anim: Option<Option<connection_anim::ConnectionAnimation>>,
    cached_reduction_anim: Option<Option<reduction_anim::ReductionAnimation>>,
    cached_dae: Option<rumoca_ir_dae::Dae>,
    // "Before" views for the Index Reduction split — parsed from the `"before"`
    // sub-object of the index reduction report JSON (the raw, pre-reduction DAE).
    cached_before_incidence: Option<Option<incidence_view::IncidenceMatrix>>,
    before_incidence_canvas: Canvas,

    // ---- 13. Markdown rendering ----
    // Caches parsed markdown for `egui_commonmark`. Shared across tour and
    // purpose-note rendering so heading IDs and image state persist across frames.
    commonmark_cache: egui_commonmark::CommonMarkCache,
    /// Source lines the compiler blamed, as `(1-based line, why)`.
    ///
    /// Populated **only when the model is genuinely ill-posed** — see
    /// [`Self::compute_problem_lines`]. Recomputed once per compile, never per frame.
    problem_lines: Vec<(u32, String)>,
    /// A line the source view should scroll to, set by an `hrw://source/<line>` link.
    /// Consumed on the frame it is honoured, like `jump_target`, so it cannot re-scroll
    /// every frame and pin the view.
    source_scroll_target: Option<u32>,
    /// The ad hoc tour from `.hrw-bridge/tour.md` (ideas #42), with the mtime it
    /// was read at, so a tour Claude rewrites mid-conversation is picked up
    /// without restarting HRW. `None` means no tour has been written — the
    /// common case, not an error.
    cached_tour: Option<(String, std::time::SystemTime)>,
    /// Tours available to pick: the ad hoc one when it exists, then the fixtures.
    /// Rebuilt by the poll, so a fixture Claude adds mid-session appears without a
    /// restart — the same reason the scratch specimen list is polled.
    tours: Vec<TourSource>,
    /// Which of `tours` is showing. Defaults to the ad hoc tour when one exists,
    /// because that is the answer to the question just asked.
    selected_tour: Option<TourSource>,
    /// When the tour file was last polled. Stat-ing once per frame would be
    /// cheap but puts filesystem work in the paint path, which the debugging
    /// conventions rule out; a few polls a second is indistinguishable to a
    /// reader and keeps the render path clean.
    tour_polled_at: Option<std::time::Instant>,
    /// A pending camera aim from `hrw://…/equation/<n>`, consumed by whichever canvas
    /// view paints next. `None` means no link asked for one.
    ///
    /// Held on the app rather than pushed straight into a `Canvas` because the target
    /// is an *equation index*, and turning that into a world position needs the view's
    /// own layout — which only exists at paint time.
    aim_at_equation: Option<usize>,
    /// A pending frame seek from `hrw://…/frame/<n>`: the target frame and how many
    /// more paints to keep trying for.
    ///
    /// **The budget is not decoration.** The animation for a newly-selected sub-view is
    /// not built until that view paints, so the first attempt after navigation always
    /// misses. Retrying unboundedly would be worse: a seek aimed at a view that *never*
    /// has an animation (Stop 6 of the frame-seeking fixture) would sit armed until the
    /// reader wandered into an animated view, and then fire there — a link taking effect
    /// somewhere it was never pointed at.
    seek_frame: Option<(usize, u8)>,
    /// When the scratch specimen directory was last polled, so a specimen Claude
    /// writes mid-conversation appears without restarting HRW — the same reason
    /// `tour.md` is polled.
    scratch_polled_at: Option<std::time::Instant>,
    // Specimen purpose notes, loaded on demand from
    // docs/specimen-notebook/<Model>/purpose.md.
    cached_purpose_notes: HashMap<PathBuf, Option<String>>,
    // The selected specimen's Modelica source text, loaded on demand.
    cached_source: Option<String>,
    /// Document URI of the file a **library model** was read from.
    ///
    /// Shown above the source, because the pane holds a whole package file that
    /// declares many classes — without it, a reader who asked for `Resistor`
    /// and sees `Basic.mo` has no way to tell the pane is showing the right thing.
    library_source_uri: Option<String>,
    /// Why the declaring file could not be read, if it could not.
    ///
    /// Kept separate from the text so the pane distinguishes *"unreadable"* from
    /// *"nothing selected"*. Both would otherwise render the same blank.
    library_source_error: Option<String>,
    /// Where the source pane is scrolled to, as of the last frame that drew it.
    ///
    /// Recorded so the horizontal offset is **checkable**. Layout is otherwise
    /// Doug's to judge -- the accessibility tree carries no geometry -- but this
    /// one number is what went wrong when a programmatic scroll centred long
    /// lines and pushed their opening characters off-screen.
    source_scroll_offset: egui::Vec2,
    // Every variable name in the compiled model — ground truth for which tree
    // leaves name something trackable. Rebuilt per compile alongside the
    // equation sheet, which is where the full classification lives.
    known_variables: Option<HashSet<String>>,
    // Variable name -> the class that declares it, when that is not the
    // specimen. See `build_declaring_classes`.
    declaring_classes: HashMap<String, String>,
    // "Reveal identifiers" — expand every tree path leading to a variable.
    expand_trackable: bool,
    // The last deliberate capture — the *point*. Retained (it used to be
    // written and forgotten) because the Context Bar must state it, and
    // because re-emitting when following changes has to preserve it.
    pointed_at: Option<PointedAt>,
    // Set when the last emission failed. The bar says so rather than claiming
    // context Claude does not have.
    point_error: Option<String>,
    // Mentions and stage count for the followed identifier, computed at
    // emission time. Never per frame — the sweep walks every stage IR.
    tracking_summary: Option<(usize, usize)>,
    // Bumped whenever the followed identifier changes. Emitted alongside the
    // focus `seq` so Claude can tell which of the two was acted on last —
    // "pointed at this, then went following" and the reverse are different
    // questions. See docs/context-assembly.md.
    track_seq: u64,
    // Which tracked identifier the source view has already scrolled to.
    //
    // Reverse tracking (idea #37) scrolls the source to a variable's
    // declaration when it becomes tracked from a downstream view. Scrolling
    // must happen on *change* only — repeating it every frame while the same
    // identifier stays tracked would pin the view and make the scrollbar
    // unusable. Comparing against this field is what makes it a one-shot.
    scrolled_source_for: Option<String>,
    // ---- Jump to the followed identifier ----
    // Where the followed name appears in the CURRENT stage's IR, so the Context
    // Bar can cycle through the matches. Comes from `bridge::mention_paths` —
    // the same walk that produces `tracking.paths` in the emitted context, so
    // the tree cannot highlight one set of nodes while Claude is told about
    // another.
    //
    // Cached because the walk visits the whole stage IR and lexes every
    // code-bearing string in it; doing that per frame is what `tracking_summary`
    // already avoids. `jump_matches_key` records what the cache was built for,
    // so a stage switch or a change of followed name rebuilds it and nothing
    // else does.
    jump_matches: Vec<Vec<Seg>>,
    jump_matches_key: Option<(StageKind, String)>,
    jump_index: usize,
    // Set for ONE frame when the user asks to jump. See `TreeOptions::jump_to`
    // for why it must not persist.
    jump_target: Option<Vec<Seg>>,
    /// The row a node link pointed at, kept **after** the scroll so it stays marked.
    ///
    /// Separate from `jump_target`, which lasts exactly one frame: a highlight that
    /// lived and died with it would flash for 16ms and tell nobody anything. This
    /// persists until Doug does something else — clicks a row, follows a link, loads a
    /// specimen — because the question it answers ("which row did that link mean?")
    /// stays open until he moves on.
    jump_highlight: Option<Vec<Seg>>,

    // Per-line token classification for `cached_source`, built on demand.
    // Tokenizing is cheap but not free, and the source cannot change while it
    // is displayed — so it is built once per specimen, not once per frame.
    // Cleared wherever `cached_source` is, or the two would disagree.
    cached_highlight: Option<crate::source_view::SourceHighlight>,

    // ---- 14. Pending stage from hrw:// link ----
    // When an hrw://load/Specimen/Stage link fires, the stage can't be applied
    // immediately — compilation is async and drain_worker will auto-select the
    // last successful stage. This field defers the stage switch until the
    // Compiled message arrives.
    pending_stage: Option<StageKind>,
    /// Sub-view requested by an `hrw://load/…/<Stage>/<SubView>` link, applied
    /// alongside `pending_stage` once the compile lands. Separate from the stage
    /// because it must be applied *after* the default-sub-view logic, which would
    /// otherwise overwrite it.
    pending_sub_view: Option<SubView>,

    // ---- 15. Deferred live debug spawn (ack handshake) ----
    // When the Debug button is clicked, `arm_live_trace_breakpoint` writes a
    // breakpoint request to `.hrw-bridge/breakpoint-request.json`. The algorithm
    // thread is NOT spawned immediately — the VS Code extension must process the
    // file and register the breakpoint with LLDB first. Each UI frame, we poll
    // `bridge::check_breakpoint_ack()` for the ack file the extension writes
    // after arming. The thread launches once the ack arrives (or after a
    // 3-second timeout if the extension isn't running). The `Instant` records
    // the request time for the timeout; the enum says which algorithm to spawn.
    pending_live_debug: Option<(std::time::Instant, PendingLiveDebug)>,
    // True while a `live_trace_breakpoint` is armed by the Debug button.
    // Cleared in two places: (1) the algorithm thread's `on_complete` callback
    // removes the breakpoint via the bridge *before* the thread exits (the
    // primary path — prevents SIGSTOP/SIGCHLD from LLDB on thread termination);
    // (2) the UI's `live_just_finished` check acts as a safety-net fallback.
    live_breakpoint_armed: bool,

    // ---- 16. Breakpoint pre-warm ----
    // See `Prewarm` and `tick_prewarm`. Runs once, early, so the debugger's
    // first (slow) resolution of live_trace.rs does not happen on the critical
    // path of the first Debug click.
    prewarm: Prewarm,
}

/// The last deliberate capture, retained so the Context Bar can state it and so
/// re-emission preserves it.
///
/// `stage` is the stage the capture was **made** in, not the one currently on
/// screen. They diverge as soon as the user switches tabs, and the bar must
/// report the former — anything else describes context Claude does not have.
#[derive(Clone)]
struct PointedAt {
    /// Stamp from the shared context counter — comparable against
    /// `track_seq`, which is stamped from the same source.
    seq: u64,
    /// Human-readable description, exactly as emitted.
    target: String,
    /// Which of the three capture shapes this was.
    ///
    /// **All three must be recorded, not just `Node`.** Only node captures were
    /// retained at first, so clicking a stage tab — which emits a *stage*
    /// capture — rewrote `focus.json` while the bar went on displaying the
    /// previous node. The bar and the file disagreed, which is precisely the
    /// drift its governing rule forbids.
    kind: PointKind,
    stage: StageKind,
    request: bridge::AskRequest,
}

/// What a capture pointed at, kept so the focus can be rebuilt when the
/// followed identifier changes.
#[derive(Clone)]
enum PointKind {
    /// A specific IR node, addressed from the stage root.
    Node(Vec<Seg>),
    /// A whole stage's IR.
    Stage,
    /// The specimen as a whole.
    Specimen,
}

/// `PartialEq` so "is the pending session this view's?" is `==`.
///
/// It used to be a hand-written list of matching pairs — `(Matching, Matching) |
/// (Tarjan, Tarjan) | (Reduction, Reduction)` — in two places. Adding a fourth
/// variant compiled cleanly and silently never matched, so the Debug button did
/// nothing at all: no error, no arming badge, no session. Derived equality
/// cannot go stale that way.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum PendingLiveDebug {
    Matching,
    Tarjan,
    Reduction,
    /// Idea #40. Unlike the other three it replays a phase of *DAE
    /// construction*, so it re-runs from the flat model rather than the DAE.
    PreLowering,
    /// Tearing. Re-runs from the DAE, like Reduction, because tearing needs the
    /// BLT blocks and those are rebuilt from the incidence each time.
    Tearing,
}

impl PendingLiveDebug {
    /// Every variant, so a test can check the arming machinery handles all of
    /// them without naming them. Add new variants here.
    ///
    /// Test-only: nothing in the app iterates the variants, because each view
    /// names its own. That is exactly why the omission this guards against was
    /// silent.
    #[cfg(test)]
    const ALL: &'static [PendingLiveDebug] = &[
        PendingLiveDebug::Matching,
        PendingLiveDebug::Tarjan,
        PendingLiveDebug::Reduction,
        PendingLiveDebug::PreLowering,
        PendingLiveDebug::Tearing,
    ];
}

enum LiveDebugAction {
    None,
    SpawnLive,
}

/// State of the one-shot breakpoint pre-warm.
///
/// ## The problem this solves
///
/// The first time a debugger is asked for a breakpoint in a given source file,
/// it must resolve that file to a compilation unit and load its line table.
/// For `hrw.exe` — a 21 MB `.text` section with a correspondingly large PDB —
/// that first resolution takes well over a second. Every later breakpoint in
/// the same file is fast, because the module is already warm.
///
/// That cold cost lands in exactly the wrong place. The Debug button's
/// handshake gives the debugger only ~500 ms (`LiveTrace::wait_for_debugger`)
/// between the extension's ack and the algorithm thread reaching the anchor,
/// and the ack means "VS Code has been told", not "the debugger has installed
/// it". So the *first* Debug click of a session missed its breakpoint and the
/// algorithm ran to completion; the second click worked, and every one after.
///
/// Rather than lengthen the wait — a guess that would be paid on every live
/// debug start forever — this moves the cold resolution off the critical path:
/// arm the anchor once at startup, wait for the ack, then remove it. Nothing is
/// left armed, but the debugger has loaded the line table by the time the user
/// clicks Debug, so the existing 500 ms is ample.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Prewarm {
    /// Nothing sent yet. The arm request goes out on the first UI frame.
    NotStarted,
    /// Arm request written; waiting for the extension's ack before removing it.
    /// The `Instant` bounds the wait for the case where no extension is running
    /// (HRW launched outside VS Code, or the bridge extension disabled).
    Awaiting(std::time::Instant),
    /// Finished — either warmed, timed out, or deliberately abandoned.
    Done,
}

impl App {
    /// Advance the one-shot breakpoint pre-warm. Called once per UI frame.
    ///
    /// See [`Prewarm`] for why this exists. The sequence is arm → await ack →
    /// remove, and it must be a sequence rather than an arm/remove pair: both
    /// requests are written to the *same* file
    /// (`.hrw-bridge/breakpoint-request.json`), so removing before the extension
    /// has read the arm request would simply overwrite it.
    fn tick_prewarm(&mut self, ctx: &egui::Context) {
        match self.prewarm {
            Prewarm::NotStarted => {
                // No specimen is loaded yet, so pass no model name — this arms
                // the anchor purely to force line-table resolution.
                if bridge::arm_live_trace_breakpoint(None).is_ok() {
                    self.prewarm = Prewarm::Awaiting(std::time::Instant::now());
                    ctx.request_repaint();
                } else {
                    // No bridge directory (or it is not writable). Live debug
                    // will not work either way; nothing to warm.
                    self.prewarm = Prewarm::Done;
                }
            }
            Prewarm::Awaiting(started) => {
                // A Debug click owns the handshake from here on. Abandon rather
                // than consume the ack it is waiting for — `check_breakpoint_ack`
                // deletes the file it reads, so polling here would make the
                // Debug click miss its own ack and fall back to its 3s timeout.
                if self.pending_live_debug.is_some() || self.live_breakpoint_armed {
                    self.prewarm = Prewarm::Done;
                    return;
                }
                let acked = bridge::check_breakpoint_ack();
                let timed_out = started.elapsed() >= std::time::Duration::from_secs(3);
                if acked || timed_out {
                    // Remove it again: pre-warming must not leave a breakpoint
                    // armed. Resolution stays cached in the debugger regardless.
                    let _ = bridge::remove_live_trace_breakpoint();
                    self.prewarm = Prewarm::Done;
                } else {
                    ctx.request_repaint();
                }
            }
            Prewarm::Done => {}
        }
    }

    /// Three-phase live-debug lifecycle shared by Matching, Tarjan, and Reduction views.
    ///
    /// Returns `SpawnLive` when the ack handshake completes and the caller
    /// should spawn the algorithm thread.
    /// Whether this view has the data its algorithm needs — gates the Debug button.
    fn has_live_debug_data(&self, variant: PendingLiveDebug) -> bool {
        match variant {
            PendingLiveDebug::Reduction | PendingLiveDebug::Tearing => self.cached_dae.is_some(),
            // The flat model, not the DAE: `pre()` lowering runs inside DAE
            // construction, so the DAE is already past it.
            PendingLiveDebug::PreLowering => self.cached_flat.is_some(),
            _ => matches!(&self.cached_incidence, Some(Some(_))),
        }
    }

    /// Arm a live debug session.
    ///
    /// Called when the Debug button reports a click. That button is rendered by
    /// `animation_controls`, so it sits on the same row as the playback
    /// controls; it cannot arm the session itself because that needs app state
    /// (the bridge, the model name, `pending_live_debug`), so it returns the
    /// click and the caller lands here.
    fn start_live_debug(&mut self, variant: PendingLiveDebug) {
        let _ = bridge::arm_live_trace_breakpoint(self.model.as_deref());
        self.pending_live_debug = Some((std::time::Instant::now(), variant));
    }

    /// Advance the live-debug handshake. Renders nothing.
    ///
    /// Returns `SpawnLive` on the frame the ack lands (or the timeout expires),
    /// at which point the caller should start the algorithm thread.
    fn live_debug_poll(
        &mut self,
        ctx: &egui::Context,
        live: LiveState,
        variant: PendingLiveDebug,
    ) -> LiveDebugAction {
        // Safety net: an armed breakpoint with no live session in flight has
        // nothing left to stop for, so release it.
        if self.live_breakpoint_armed && !live.is_busy() {
            let _ = bridge::remove_live_trace_breakpoint();
            self.live_breakpoint_armed = false;
        }

        if let Some((armed_at, v)) = self.pending_live_debug
            && v == variant {
                let acked = bridge::check_breakpoint_ack();
                let timed_out = armed_at.elapsed() >= std::time::Duration::from_secs(3);
                if acked || timed_out {
                    self.pending_live_debug = None;
                    self.live_breakpoint_armed = true;
                    return LiveDebugAction::SpawnLive;
                }
                // No status text here — the control row already shows the
                // "Arming…" badge via `LiveState::badge`.
                ctx.request_repaint();
            }
        LiveDebugAction::None
    }

    /// Whether a live debug session is being armed for this particular view.
    ///
    /// The animations cannot work this out themselves: during the handshake the
    /// view still holds the *recorded* animation, so its own `is_live()` is
    /// false. Without this, the controls stayed enabled for the several frames
    /// between the Debug click and the algorithm thread spawning.
    fn is_arming(&self, variant: PendingLiveDebug) -> bool {
        self.pending_live_debug.is_some_and(|(_, v)| v == variant)
    }

    /// Create the application. Called once by eframe at startup.
    ///
    /// Three things happen here that are worth understanding:
    ///
    /// 1. **Font fallback trick**: egui has two font families (Proportional and
    ///    Monospace), each with its own list of fonts. Some glyphs (like arrows
    ///    and ) only exist in the monospace font (Hack), so proportional labels
    ///    would show blank squares ("tofu") for them. The fix: add every loaded
    ///    font as a fallback for BOTH families, so egui searches all fonts when
    ///    a glyph is missing from the primary.
    ///
    /// 2. **Zoom factor**: rather than changing individual font sizes, we scale
    ///    the entire UI (fonts + widget spacing) via `set_zoom_factor`. This
    ///    keeps the Settings slider and Ctrl+/- keyboard shortcuts working
    ///    consistently.
    ///
    /// 3. **Worker spawn**: the background thread is created here with a clone
    ///    of the egui context. The worker uses that context to call
    ///    `request_repaint()` after sending results, which wakes the UI thread
    ///    to process them — without this, egui would only repaint on user input
    ///    and results could sit unseen in the channel.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Before anything else: install the panic hook, so a crash during
        // startup still writes a file. Everything below this line — font
        // loading, the worker spawn — can fail, and did once.
        diagnostics::init();
        diagnostics::record_action("session", "HRW started");

        // Make every bundled font a fallback for BOTH families, so a glyph that
        // lives in only one (e.g. the → and ← arrows are in Hack/monospace but
        // not Ubuntu-Light/proportional) still renders in any label — otherwise
        // arrows show as tofu squares in proportional text.
        let mut fonts = egui::FontDefinitions::default();
        let all: Vec<String> = fonts.font_data.keys().cloned().collect();
        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            let list = fonts.families.entry(family).or_default();
            for name in &all {
                if !list.contains(name) {
                    list.push(name.clone());
                }
            }
        }
        cc.egui_ctx.set_fonts(fonts);

        // Scale the whole UI (fonts + spacing) via egui's zoom, not by mutating
        // individual text styles — so the Settings slider and Ctrl +/− both work.
        cc.egui_ctx.set_zoom_factor(DEFAULT_ZOOM);

        // Spawn the worker thread. It gets a clone of the egui context so it
        // can wake the UI when results are ready (see `drain_worker`).
        let worker = Worker::spawn(cc.egui_ctx.clone());
        let mut app = App {
            worker,
            libraries_text: DEFAULT_LIBRARIES.to_owned(),
            library_status: String::new(),
            libraries_busy: false,
            specimen_dir: DEFAULT_SPECIMEN_DIR.to_owned(),
            files: Vec::new(),
            specimen_purposes: HashMap::new(),
            scratch_specimens: HashSet::new(),
            selected_is_library: false,
            corpus: None,
            list_filter: String::new(),
            scratch_arrived: false,
            shadowed_specimens: Vec::new(),
            scan_error: None,
            selected: None,
            compiling: false,
            model: None,
            stages: StageBundle::default(),
            stage: StageKind::Resolve,
            def_index: BTreeMap::new(),
            nav: Vec::new(),
            nav_loading: None,
            nav_error: None,
            context_seq: 0,
            notice: None,
            ui_mode: UiMode::Tour,
            specimen_detail: SpecimenDetail::default(),
            show_settings: false,
            show_help: false,
            show_about: false,
            field_help: field_help::load(),
            flatten_view: FlattenView::Equations,
            events_view: EventsView::default(),
            init_view: InitView::default(),
            highlighted_eq_row: None,
            highlighted_source_line: None,
            structural_view: StructuralView::SpyPlot,
            spy_canvas: Canvas::default().with_fit_vertical_bias(0.15),
            incidence_canvas: Canvas::default().with_fit_vertical_bias(0.15),
            log_entries: Vec::new(),
            viewing_log: false,

            tracing_enabled: false,
            simulation: Stage::default(),
            sim_data: None,
            sim_running: false,
            sim_error: None,
            sim_t_end: 2.0,
            cached_report_stage: None,
            cached_spy_plot: None,
            cached_incidence: None,
            cached_reduction: None,
            cached_equation_sheet: None,
            identifier_index: None,
            tracked_identifier: None,
            cached_matching_anim: None,
            matching_anim_canvas: Canvas::default().with_fit_vertical_bias(0.15),
            cached_tarjan_anim: None,
            tarjan_anim_canvas: Canvas::default().with_fit_vertical_bias(0.15),
            index_reduction_frames: Vec::new(),
            pre_lowering_frames: Vec::new(),
            connection_frames: Vec::new(),
            cached_flat: None,
            cached_pre_lowering_anim: None,
            cached_tearing_anim: None,
            cached_alias_anim: None,
            cached_ic_plan_anim: None,
            cached_connection_anim: None,
            cached_reduction_anim: None,
            cached_dae: None,
            cached_before_incidence: None,
            before_incidence_canvas: Canvas::default().with_fit_vertical_bias(0.15),
            commonmark_cache: egui_commonmark::CommonMarkCache::default(),
            problem_lines: Vec::new(),
            source_scroll_target: None,
            cached_tour: None,
            tours: Vec::new(),
            selected_tour: None,
            tour_polled_at: None,
            aim_at_equation: None,
            seek_frame: None,
            scratch_polled_at: None,
            cached_purpose_notes: HashMap::new(),
            cached_source: None,
            library_source_uri: None,
            library_source_error: None,
            source_scroll_offset: egui::Vec2::ZERO,
            cached_highlight: None,
            known_variables: None,
            declaring_classes: HashMap::new(),
            expand_trackable: false,
            pointed_at: None,
            point_error: None,
            tracking_summary: None,
            track_seq: 0,
            jump_matches: Vec::new(),
            jump_matches_key: None,
            jump_index: 0,
            jump_target: None,
            jump_highlight: None,
            scrolled_source_for: None,
            pending_stage: None,
            pending_sub_view: None,
            pending_live_debug: None,
            live_breakpoint_armed: false,
            prewarm: Prewarm::NotStarted,
        };
        // Scan the specimen directory and pre-load libraries at startup so the
        // Resolve phase works immediately when the user selects a specimen
        // (Resolve needs the MSL classes to resolve `import` references).
        app.rescan();
        app.load_libraries();
        app
    }

    /// Parse the multi-line library text field into a list of paths, one per
    /// non-empty line. The text field is editable in the Settings window.
    fn parse_library_paths(&self) -> Vec<PathBuf> {
        self.libraries_text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(PathBuf::from)
            .collect()
    }

    /// Send the current library paths to the worker thread for loading.
    /// Library loading is async — the result arrives as `FromWorker::Libraries`.
    fn load_libraries(&mut self) {
        let roots = self.parse_library_paths();
        self.libraries_busy = true;
        self.library_status = format!("loading {} source root(s)…", roots.len());
        self.worker.send(ToWorker::SetLibraries(roots));
    }

    /// Re-read the specimen directory, rebuilding the file list and scanning
    /// each `.mo` file for its `// purpose:` comment. Called at startup and
    /// when the user changes the directory in Settings.
    fn rescan(&mut self) {
        self.files.clear();
        self.scratch_specimens.clear();
        self.shadowed_specimens.clear();
        self.scan_error = None;
        match std::fs::read_dir(&self.specimen_dir) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|e| e.to_str()) == Some("mo") {
                        self.files.push(path);
                    }
                }
                self.files.sort();
            }
            Err(e) => self.scan_error = Some(format!("{}: {e}", self.specimen_dir)),
        }

        // Scratch specimens Claude wrote to answer a question (ideas #42). Appended
        // after the curated corpus and **never allowed to shadow it**: a name
        // collision would silently load a different model than the one Doug named,
        // and Claude would then reason confidently about source Doug is not looking
        // at. That is the "makes Claude guess" failure, so the collision is reported
        // and the scratch file is skipped rather than winning or losing quietly.
        let curated: HashSet<String> = self
            .files
            .iter()
            .filter_map(|p| p.file_name().and_then(|n| n.to_str()).map(str::to_owned))
            .collect();
        let mut scratch = Vec::new();
        for path in bridge::scratch_specimens() {
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if curated.contains(name) {
                self.shadowed_specimens.push(name.to_owned());
                continue;
            }
            self.scratch_specimens.insert(path.clone());
            scratch.push(path);
        }
        // **Scratch first, matching the tour list** (Doug, 2026-07-29). The ephemeral,
        // just-written thing is the one most likely to be wanted next — a probe exists
        // because a question is open right now. Appending it after 18 curated specimens
        // buried the common case, which is the same mistake the tour picker avoids by
        // putting Claude's answer at the top.
        //
        // Safe to reorder because a scratch name colliding with a curated one is
        // skipped above, so `files` never holds two entries with the same file name and
        // `find_specimen`'s first-match cannot become ambiguous.
        scratch.extend(std::mem::take(&mut self.files));
        self.files = scratch;
        // Scan each specimen's `// purpose:` hint (cheap; no compile), so the list
        // can show what each one demonstrates.
        self.specimen_purposes = self
            .files
            .iter()
            .filter_map(|p| read_purpose(p).map(|hint| (p.clone(), hint)))
            .collect();
    }

    /// Find a specimen by model name (e.g. "BouncingBall" → `specimens/BouncingBall.mo`).
    fn find_specimen(&self, name: &str) -> Option<PathBuf> {
        let with_ext = format!("{name}.mo");
        self.files.iter().find(|p| {
            p.file_name().and_then(|f| f.to_str()) == Some(with_ext.as_str())
        }).cloned()
    }

    /// Open (compile) a specimen. Called when the user clicks a file in the
    /// left panel or follows an `hrw://load/` link.
    ///
    /// **Why everything resets:** each specimen is an independent compilation.
    /// Leftover stage data from the previous specimen would be confusing (e.g.
    /// showing BouncingBall's events tab while compiling Drivetrain). So we
    /// clear every stage, the navigation stack, the simulation results, and the
    /// field-help selection before sending the new compile command.
    ///
    /// **Why `viewing_log` starts `true`:** compilation takes a moment, and
    /// showing the log view lets the user watch progress (phase start/end
    /// messages, timing) instead of staring at "compiling..." text. Once
    /// compilation finishes and the user clicks a stage tab, `viewing_log`
    /// flips to false.
    fn open(&mut self, path: PathBuf) {
        // Reselecting the SAME specimen keeps the assembled context.
        //
        // Recompiling used to wipe the point and the follow unconditionally,
        // which made one workflow impossible: assembling context, asking for
        // breakpoints, then recompiling to hit them — because the recompile
        // destroyed the very context that motivated the breakpoints. Doug hit
        // this the first time the breakpoints actually worked.
        //
        // Clearing is right when the specimen *changes*: a key-path addresses
        // one model's IR and means nothing in another's. For a reselect the IR
        // is normally identical, so the point still resolves and the followed
        // name still exists. "Normally" is not "always" — the file may have been
        // edited between loads — so the retained point is *validated* against
        // the new IR when it arrives, not assumed to survive.
        let same_specimen = self.selected.as_deref() == Some(path.as_path());

        // Mark as compiling — this disables stage tab highlighting and shows a
        // spinner.
        self.compiling = true;
        self.clear_specimen_state(same_specimen);
        // Start with the log view so the user sees compilation progress.
        self.log_entries.clear();
        self.viewing_log = true;
        // Send the compile command to the worker thread. Results will arrive
        // asynchronously via `FromWorker::CompileProgress` and `FromWorker::Compiled`.
        // Recorded before the state changes, so the ring buffer reads in the
        // order the user acted. See `diagnostics.rs`.
        self.jump_highlight = None;
        diagnostics::record_action("specimen", path.display().to_string());
        self.worker.send(ToWorker::Compile(path.clone()));
        self.selected = Some(path);
        // Cleared here, not only set in `open_library_model`: a specimen opened
        // after a corpus model would otherwise still look like one, and the
        // source view would refuse to read a file that is right there.
        self.selected_is_library = false;
    }

    /// Open a model from a **loaded library** by qualified name — the corpus
    /// counterpart of [`Self::open`].
    ///
    /// # Why the selection is a `PathBuf` holding a name
    ///
    /// `selected` identifies *what is loaded* and is compared for equality in
    /// three staleness checks, which discard results for a model the user has
    /// already navigated away from. A library model has no file of its own — it
    /// lives inside a package file that may declare many classes, so the file
    /// path would not identify it — and **the worker already established the
    /// convention**: `compile_target` puts the qualified name in the result's
    /// `path` field for a library target. Following it here keeps one identity
    /// scheme and leaves the staleness checks working unchanged.
    ///
    /// `selected_is_library` records which kind it is, rather than sniffing the
    /// string for dots. A model *file* could contain a dot; a flag cannot lie.
    pub(crate) fn open_library_model(&mut self, qualified: &str) {
        let id = PathBuf::from(qualified);
        let same = self.selected.as_deref() == Some(id.as_path());
        self.compiling = true;
        self.clear_specimen_state(same);
        self.log_entries.clear();
        self.viewing_log = true;
        self.jump_highlight = None;
        diagnostics::record_action("corpus-model", qualified.to_owned());
        self.worker.send(ToWorker::CompileLibraryModel(qualified.to_owned()));
        self.selected = Some(id);
        self.selected_is_library = true;
    }

    /// The corpus rows, read from the survey on first use.
    ///
    /// Returns an empty slice when the survey has not been generated — a fresh
    /// clone has no `docs/reports/msl-survey.csv` until someone runs the survey,
    /// and the list should degrade to curated specimens rather than error.
    fn corpus_rows(&mut self) -> &[crate::survey::SurveyRow] {
        self.corpus.get_or_insert_with(|| {
            let path = concat!(env!("CARGO_MANIFEST_DIR"), "/docs/reports/msl-survey.csv");
            std::fs::read_to_string(path)
                .map(|t| crate::survey::parse_csv(&t))
                .unwrap_or_default()
        })
    }

    /// Clear everything that belonged to the previously loaded specimen.
    ///
    /// Shared by [`Self::open`] and by switching tours, because both need "forget the
    /// last model" and **two copies of a thirty-field reset would drift**. That is not
    /// hypothetical: on 2026-07-30 clearing the jump highlight in one of two capture
    /// paths and not the other produced a real bug within the hour.
    ///
    /// `keep_context` is true when the *same* specimen is being reloaded: a key-path
    /// addresses one model's IR and means nothing in another's, but on a reselect the IR
    /// is normally identical, so the point still resolves. It is validated against the
    /// new IR on arrival rather than assumed to survive.
    fn clear_specimen_state(&mut self, keep_context: bool) {
        // Clear all previous results. Every field that could hold stale data
        // from the last specimen is reset to its default.
        self.model = None;
        self.stages = StageBundle::default();
        self.sim_data = None;
        self.sim_error = None;
        self.sim_running = false;
        self.def_index = BTreeMap::new();
        self.cached_equation_sheet = None;
        self.known_variables = None;
        self.declaring_classes.clear();
        // A point addresses IR that no longer exists once the stages are
        // replaced — but only when the specimen changed. See `same_specimen`.
        if !keep_context {
            self.pointed_at = None;
            self.tracked_identifier = None;
        }
        self.point_error = None;
        self.tracking_summary = None;
        self.identifier_index = None;
        // Whatever the followed name matched belonged to the old IR, so the
        // jump list must be rebuilt rather than reused.
        self.jump_matches = Vec::new();
        self.jump_matches_key = None;
        self.jump_index = 0;
        self.jump_target = None;
        self.scrolled_source_for = None;
        self.cached_source = None;
        // Cleared *with* the text it labels, or the header would name the previous
        // model's library file over an empty pane for the whole compile. The two
        // are set together in the `Compiled` handler for the same reason.
        self.library_source_uri = None;
        self.library_source_error = None;
        self.cached_highlight = None;
        self.highlighted_eq_row = None;
        self.highlighted_source_line = None;
        self.nav.clear();
        self.nav_loading = None;
        self.nav_error = None;
        // Clean up any in-flight or active live debug session. Without this,
        // switching specimens while arming or running leaves the breakpoint
        // armed on the old specimen and the polling state dangling.
        if self.live_breakpoint_armed {
            let _ = bridge::remove_live_trace_breakpoint();
        }
        self.live_breakpoint_armed = false;
        self.pending_live_debug = None;
        // Clear any stale pending_stage — a plain LoadSpecimen link should not
        // carry over a stage override from a previous LoadAndSwitch click.
        // LoadAndSwitch calls open() first, then sets pending_stage.
        self.pending_stage = None;
        self.pending_sub_view = None;
    }

    /// Fetch a class by qualified name for navigation (async; pushed on arrival).
    /// The worker resolves the name to a class definition and returns it as a
    /// `FromWorker::DefTree` message; `drain_worker` pushes it onto `self.nav`.
    fn navigate_to(&mut self, name: String) {
        self.nav_loading = Some(name.clone());
        self.nav_error = None;
        self.worker.send(ToWorker::OpenDef(name));
    }

    /// Drain all pending messages from the worker thread's channel.
    ///
    /// This is the **channel drain pattern**: the worker sends results via an
    /// `mpsc` channel, and we poll it each frame with `try_recv()` in a loop.
    /// `try_recv` is non-blocking — it returns `Ok(msg)` if a message is
    /// waiting, or `Err` if the channel is empty. The `while` loop processes
    /// ALL pending messages in one frame (the worker may have sent several
    /// between repaints), then falls through so the UI can render.
    ///
    /// Two compilation message types work together:
    /// - **`CompileProgress`**: sent after each pipeline phase completes. It
    ///   carries partial results so the tab colors update incrementally (you
    ///   see Parse go green, then Resolve, etc.) while compilation continues.
    ///   We must NOT touch `compiling` or `stage` here — the pipeline isn't done.
    /// - **`Compiled`**: sent once when the full pipeline finishes. This is
    ///   where we clear `compiling`, set `stage` to the furthest clean result,
    ///   publish all IRs to the bridge, and fit the custom-view cameras.
    ///
    /// Both messages carry the specimen `path` so we can detect stale results:
    /// if the user switched specimens while compilation was running, the old
    /// results arrive for a path that no longer matches `self.selected`, and
    /// we skip them with `continue`.
    fn drain_worker(&mut self) {
        while let Ok(msg) = self.worker.rx.try_recv() {
            match msg {
                FromWorker::Libraries(result) => {
                    self.libraries_busy = false;
                    self.library_status = match result {
                        Ok(n) => format!("loaded {n} library document(s)"),
                        Err(e) => format!("library load failed — {e}"),
                    };
                }
                FromWorker::Log(entry) => {
                    // Mirrored into the crash buffer here rather than snapshotted
                    // per frame: entries arrive one at a time, and cloning the
                    // whole log 60 times a second to carry it would not be
                    // affordable. See `diagnostics.rs`.
                    diagnostics::record_log(entry.level.label(), &entry.message);
                    self.log_entries.push(entry);
                }
                FromWorker::CompileProgress { path, stages } => {
                    // A partial result mid-compile: apply the stages known so far so
                    // the tab colours advance in step with the pipeline. Compilation
                    // is still running, so DON'T touch `compiling`, `stage`,
                    // `def_index`, or the bridge — the final `Compiled` owns those.
                    if self.selected.as_deref() != Some(path.as_path()) {
                        continue; // stale (specimen switched)
                    }
                    self.stages = stages;
                }
                FromWorker::Compiled {
                    path, model, stages, def_index, equation_sheet,
                    identifier_index, index_reduction_frames, dae,
                    pre_lowering_frames, connection_frames, flat,
                    library_source,
                } => {
                    if self.selected.as_deref() != Some(path.as_path()) {
                        continue; // stale result
                    }
                    self.compiling = false;
                    self.model = model;
                    self.stages = stages;
                    self.def_index = def_index;
                    // Ground truth for what the tree may offer to track. Built
                    // from the equation sheet's classification, which lists
                    // every variable in the compiled model — including
                    // library-origin ones, which the identifier index omits
                    // because they have no specimen source line.
                    self.known_variables = equation_sheet.as_ref().map(|s| {
                        s.variables.iter().map(|v| v.name.clone()).collect()
                    });
                    self.declaring_classes = Self::build_declaring_classes(
                        &self.stages, &self.def_index, equation_sheet.as_ref(),
                    );
                    self.cached_equation_sheet = equation_sheet;
                    self.identifier_index = identifier_index;
                    self.index_reduction_frames = index_reduction_frames;
                    self.pre_lowering_frames = pre_lowering_frames;
                    self.connection_frames = connection_frames;
                    self.cached_flat = flat;
                    self.cached_pre_lowering_anim = None;
                    self.cached_dae = dae;
                    self.cached_report_stage = None;
                    self.cached_spy_plot = None;
                    self.cached_incidence = None;
                    self.cached_reduction = None;
                    self.cached_matching_anim = None;
                    self.cached_tarjan_anim = None;
                    self.cached_tearing_anim = None;
                    self.cached_alias_anim = None;
                    self.cached_ic_plan_anim = None;
                    self.cached_connection_anim = None;
                    self.cached_reduction_anim = None;
                    self.cached_before_incidence = None;
                    // **A library model's source, seeded into the same cache a
                    // specimen fills from disk.** Everything downstream —
                    // highlighting, clickable identifiers, blamed lines, the
                    // scroll-to-declaration — then works without knowing
                    // where the text came from, which is the point: Doug asked
                    // for the MSL source view to be *as functional as* a
                    // specimen's, not for a second reduced one.
                    if let Some(lib) = library_source {
                        // A read failure is *reported*, never rendered as blank.
                        match lib.text {
                            Ok(text) => {
                                self.library_source_error = None;
                                self.cached_source = Some(text);
                            }
                            Err(why) => {
                                self.library_source_error = Some(why);
                                self.cached_source = None;
                            }
                        }
                        // Landing at line 1 of a package file shows a header and
                        // nothing asked for; `Resistor` starts 1,498 lines into
                        // `Basic.mo`. The line is the compiler's, not a search.
                        self.source_scroll_target = lib.decl_line;
                        self.library_source_uri = Some(lib.uri);
                        self.cached_highlight = None;
                    } else {
                        self.library_source_uri = None;
                        self.library_source_error = None;
                    }
                    if self.live_breakpoint_armed {
                        let _ = bridge::remove_live_trace_breakpoint();
                    }
                    self.live_breakpoint_armed = false;
                    self.pending_live_debug = None;
                    // A point retained across a reselect must still address
                    // something. Validated rather than assumed: the source may
                    // have been edited between loads, and a bar naming a node
                    // that no longer exists — over an emitted `subtree: null` —
                    // is exactly the confident lie this design forbids.
                    self.revalidate_point_against_new_ir();
                    self.spy_canvas.request_fit();
                    self.incidence_canvas.request_fit();
                    self.matching_anim_canvas.request_fit();
                    self.tarjan_anim_canvas.request_fit();
                    self.before_incidence_canvas.request_fit();
                    // Land on the furthest stage that completed cleanly,
                    // unless a pending_stage was requested via an hrw://load/…/Stage link.
                    // Always update `self.stage` so the correct stage is ready when the
                    // user clicks away from Log, but don't force `viewing_log = false` —
                    // if the user is watching the log, let them stay there.
                    if let Some(pending) = self.pending_stage.take() {
                        self.stage = pending;
                        self.viewing_log = false;
                    } else {
                        self.stage = self.last_successful_stage();
                    }
                    // Which source lines the compiler blamed. Once per compile.
                    self.compute_problem_lines();
                    // Publish every stage's full IR so Claude can diff any pair.
                    if let Err(e) = bridge::write_stages(&self.stages.as_stage_pairs()) {
                        self.notify(format!("write_stages failed: {e}"));
                    }
                    // **Close the loop on the load.** Without this the trail ended at
                    // "specimen sent to the worker", so the app block stayed frozen at
                    // `compiling: true, model: null` — accurate for the last *action*,
                    // and increasingly wrong about *now* the longer Doug paused. He
                    // caught exactly that by reloading a tour and asking what the trail
                    // said an hour later.
                    //
                    // Says where the pipeline got to, so a failure is legible without
                    // opening the stage files: the failing stage is the diagnostic.
                    diagnostics::record_action("compiled", self.compile_outcome());
                }
                FromWorker::Simulated { path, result } => {
                    if self.selected.as_deref() != Some(path.as_path()) {
                        continue; // stale result for a different specimen
                    }
                    self.sim_running = false;
                    // Same gap as the compile: without this the trail ends at "started
                    // simulating" and the app block stays frozen mid-run.
                    match result {
                        Ok(data) => {
                            diagnostics::record_action(
                                "simulated",
                                format!(
                                    "{}: {} variables, {} points",
                                    self.model.as_deref().unwrap_or("<unnamed>"),
                                    data.names.len(),
                                    data.times.len(),
                                ),
                            );
                            self.sim_data = Some(data);
                            self.sim_error = None;
                        }
                        Err(e) => {
                            // The failure text itself, because a simulation that will
                            // not run is exactly the case Doug would be asking about.
                            diagnostics::record_action(
                                "simulated",
                                format!(
                                    "{}: FAILED \u{2014} {e}",
                                    self.model.as_deref().unwrap_or("<unnamed>"),
                                ),
                            );
                            self.sim_data = None;
                            self.sim_error = Some(e);
                        }
                    }
                }
                FromWorker::DefTree { name, result } => {
                    self.nav_loading = None;
                    match result {
                        Ok((value, def_index)) => {
                            self.nav.push(NavEntry { name, value, def_index });
                            self.nav_error = None;
                        }
                        Err(e) => self.nav_error = Some(format!("open “{name}” failed: {e}")),
                    }
                }
            }
        }
    }

    /// Look up the `Stage` for the currently selected tab. Delegates to
    /// `StageBundle::get()` for the ten real stages; Simulation returns the
    /// always-empty placeholder (the Simulation view is the plot pane, rendered
    /// specially — not the generic tree inspector).
    fn current_stage(&self) -> &Stage {
        match self.stage {
            StageKind::Simulation => &self.simulation,
            other => self.stages.get(other),
        }
    }

    /// The previous stage's IR, aligned to the *current* stage's tree root, for
    /// the "changed by this stage" green highlight. `None` for Parse (no
    /// previous). Where structures differ (e.g. Resolve's class tree vs the
    /// Instantiate overlay) few paths align, so little highlights — as intended.
    fn previous_stage_value(&self) -> Option<&serde_json::Value> {
        match self.stage {
            StageKind::Parse => None,
            // Parse's root is the StoredDefinition; the class is under classes.<model>.
            StageKind::Resolve => self
                .stages
                .parse
                .value
                .as_ref()?
                .get("classes")?
                .get(self.model.as_deref()?),
            StageKind::Instantiate => self.stages.resolve.value.as_ref(),
            StageKind::Typecheck => self.stages.instantiate.value.as_ref(),
            StageKind::Flatten => self.stages.typecheck.value.as_ref(),
            // The structural report is a different shape from the flat model —
            // no path-aligned previous, so nothing to highlight.
            StageKind::Structural => None,
            // Diff the reduced report against the raw one: for an already-index-1
            // model they're identical (nothing highlights); for a reduced
            // high-index model the raw report is absent (it was singular).
            StageKind::IndexReduction => self.stages.structural.value.as_ref(),
            // The IC plan is its own shape (a solve sequence) — no path-aligned prior.
            StageKind::Initialization => None,
            // The event partitions are their own shape — no path-aligned prior.
            StageKind::Events => None,
            // The SolveModel is a different shape from the DAE — no path-aligned prior.
            StageKind::SolveLowering => None,
            // Simulation is a plot, not IR — no tree, no prior.
            StageKind::Simulation => None,
        }
    }

    /// The furthest-along stage that produced clean IR (value present, no error
    /// note) — where the tabs should land after a compile. Falls back to Parse.
    fn last_successful_stage(&self) -> StageKind {
        let ok = |s: &Stage| s.value.is_some() && !s.note_is_error();
        if ok(&self.stages.solve_lowering) {
            StageKind::SolveLowering
        } else if ok(&self.stages.events) {
            StageKind::Events
        } else if ok(&self.stages.initialization) {
            StageKind::Initialization
        } else if ok(&self.stages.index_reduction) {
            StageKind::IndexReduction
        } else if ok(&self.stages.structural) {
            StageKind::Structural
        } else if ok(&self.stages.flatten) {
            StageKind::Flatten
        } else if ok(&self.stages.typecheck) {
            StageKind::Typecheck
        } else if ok(&self.stages.instantiate) {
            StageKind::Instantiate
        } else if ok(&self.stages.resolve) {
            StageKind::Resolve
        } else {
            StageKind::Parse
        }
    }

    fn library_strings(&self) -> Vec<String> {
        self.parse_library_paths()
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect()
    }

    /// Emit a bridge focus file describing what the user wants to ask about, and
    /// record a one-line status. The reasoning happens in the Claude Code chat.
    ///
    /// **What "capture" means:** when the user clicks a tree node, a stage tab,
    /// or a specimen, HRW writes a JSON file (the "focus file") to a known
    /// location on disk. Claude Code (running in a terminal alongside the GUI)
    /// watches that location. The focus file contains: what was clicked (a
    /// key-path like `["equations", 3, "lhs"]`), which stage, the full IR of
    /// the current and adjacent stages, and the def_index for resolving
    /// cross-references. Claude reads this to understand the user's question
    /// context without the user having to copy-paste IR.
    ///
    /// Build an `Ask` from the current specimen state (shared fields).
    fn base_ask<'a>(
        &'a self,
        seq: u64,
        request: bridge::AskRequest,
        focus: Focus<'a>,
        stage_values: &'a [(&'a str, Option<&'a Value>)],
    ) -> Ask<'a> {
        Ask {
            seq,
            request,
            specimen: self.selected.as_deref(),
            model: self.model.as_deref(),
            stage: Some(self.stage),
            libraries: self.library_strings(),
            def_index: &self.def_index,
            parse_value: self.stages.parse.value.as_ref(),
            resolve_value: self.stages.resolve.value.as_ref(),
            focus,
            tracking: self.tracking_context(stage_values),
            view: self.view_context(),
            failure: self.failure_context(),
        }
    }

    /// The first pipeline stage that failed, for the capture (ideas #45 step 3).
    ///
    /// **First, not current.** A failure cascades — every later stage reports "not
    /// reached" — so the earliest error is the cause and the rest are consequences.
    /// Naming whichever stage Doug happens to be looking at would often name a
    /// consequence, which is exactly the wrong thing to hand a question like "why
    /// doesn't this work?".
    ///
    /// Returns `None` for a clean compile, so the section is absent rather than
    /// present-and-empty: a `pipeline_failure` key that exists always would make
    /// "nothing failed" indistinguishable from "the field was not populated".
    fn failure_context(&self) -> Option<bridge::PipelineFailure<'_>> {
        // `COMPILATION`, not `ALL`: `ALL` ends with `Simulation`, which is a tab rather
        // than a compilation stage, and `StageBundle::get()` panics on it.
        let first =
            StageKind::COMPILATION.iter().copied().find(|&k| self.stages.get(k).note_is_error())?;
        let stage = self.stages.get(first);
        let after = StageKind::COMPILATION
            .iter()
            .copied()
            .skip_while(|&k| k != first)
            .skip(1)
            .filter(|&k| self.stages.get(k).value.is_none())
            .map(StageKind::name)
            .collect();
        Some(bridge::PipelineFailure {
            stage: first.name(),
            note: stage.note.as_deref().unwrap_or(""),
            error: stage.value.as_ref().and_then(|v| v.get("error")),
            not_reached: after,
        })
    }

    /// The ambient half of the emitted context — what is being followed.
    ///
    /// Always included when something is tracked, whatever the user pointed at.
    /// Following is not a mode you enter to ask a question; it is standing
    /// context that any question is asked *within*. See
    /// `docs/context-assembly.md`.
    fn tracking_context<'a>(
        &'a self,
        stage_values: &'a [(&'a str, Option<&'a Value>)],
    ) -> Option<bridge::Tracking<'a>> {
        let name = self.tracked_identifier.as_deref()?;
        Some(bridge::Tracking {
            seq: self.track_seq,
            name,
            declared_line: self
                .identifier_index
                .as_ref()
                .and_then(|idx| idx.variables.get(name))
                .map(|v| v.source_line),
            declaring_class: self.declaring_classes.get(name).map(String::as_str),
            stage_values,
        })
    }

    /// Emit a stage or specimen capture, and record it as the point.
    ///
    /// Recording matters as much as emitting: this path used to write the file
    /// without updating `pointed_at`, so a stage-tab click replaced the emitted
    /// context while the Context Bar carried on showing the previous node.
    fn emit_focus(&mut self, focus: Focus) {
        // `Nothing` is not a capture. It is what the Context Bar emits *after*
        // the point is cleared, which goes through `emit_context`. Guarded
        // rather than asserted: a UI path must not abort the app to report a
        // programming error.
        if matches!(focus, Focus::Nothing) {
            return;
        }
        let seq = self.next_seq();
        // A point of Doug's own supersedes the link's — the highlight answers "which
        // row did that link mean?", and he has just answered a different question.
        self.jump_highlight = None;
        diagnostics::record_action("point-at", format!("in {}", self.stage.name()));
        // Name what was captured, so the status confirms the *right* thing was
        // written — not just that some focus was.
        let target = match &focus {
            Focus::Node { key_path, .. } => bridge::describe_path(key_path),
            Focus::Stage => format!("stage “{}”", self.stage.name()),
            Focus::Specimen => format!(
                "specimen “{}”",
                self.selected
                    .as_deref()
                    .and_then(|p| p.file_name())
                    .and_then(|n| n.to_str())
                    .unwrap_or("?"),
            ),
            // Excluded by the guard above. `return` rather than a panic keeps
            // that safe even if a future caller forgets.
            Focus::Nothing => return,
        };
        let stage_values = self.stages.as_stage_pairs();
        let kind = match &focus {
            Focus::Node { key_path, .. } => PointKind::Node(key_path.clone()),
            Focus::Stage => PointKind::Stage,
            Focus::Specimen => PointKind::Specimen,
            Focus::Nothing => return,
        };
        let ask = self.base_ask(seq, bridge::AskRequest::Explain, focus, &stage_values);
        let result = bridge::write(&ask);
        self.pointed_at = Some(PointedAt {
            seq,
            target: target.clone(),
            kind,
            stage: self.stage,
            request: bridge::AskRequest::Explain,
        });
        self.point_error = result.as_ref().err().map(std::string::ToString::to_string);
        // `None` on success: the Context Bar already names the point, and it
        // does so persistently. See `status_line`.
        self.notice = status_line(seq, &target, "explain", result);
    }

    /// What HRW is showing, for the capture.
    ///
    /// One builder for all four `Ask` construction sites, so the emitted view
    /// can never depend on which path produced the capture — a point made by
    /// clicking a node and the same point re-emitted when the followed
    /// identifier changes must describe the same screen.
    ///
    /// Overlaps `diagnostic_snapshot` deliberately rather than sharing with it.
    /// The two answer different questions — *what was on screen when this
    /// context was assembled* versus *what was the app doing when it died* —
    /// and merging them would tie the capture's shape to the crash log's.
    fn view_context(&self) -> bridge::View<'_> {
        bridge::View {
            ui_mode: match self.ui_mode {
                UiMode::Tour => "Tour",
                UiMode::Specimen => "Specimen",
                UiMode::Debug => "Debug",
            },
            stage_view: match self.stage {
                StageKind::Structural | StageKind::IndexReduction => {
                    Some(structural_view_name(self.structural_view))
                }
                StageKind::Flatten => Some(flatten_view_name(self.flatten_view)),
                StageKind::Events => Some(events_view_name(self.events_view)),
                StageKind::Initialization => Some(init_view_name(self.init_view)),
                _ => None,
            },
            specimen_detail: (self.ui_mode == UiMode::Specimen).then_some({
                match self.specimen_detail {
                    SpecimenDetail::Source => "Source",
                    SpecimenDetail::Purpose => "Purpose",
                }
            }),
            viewing_log: self.viewing_log,
            animation: self.animation_view(),
        }
    }

    /// The on-screen animation's position, if the current stage is showing one.
    ///
    /// Reported only for the *current* stage tab: the caches hold several
    /// animations at once, and naming a stale one would say the user was
    /// looking at something they were not.
    /// The animation the current stage tab is showing, if any.
    ///
    /// One place that answers "which animation is on screen?" — the capture and
    /// the crash log both used to carry their own copy of this match, so a new
    /// view (idea #40's `pre()` lowering) would have needed adding in three
    /// places and would have been forgotten in one.
    ///
    /// Reported only for the *current* tab: the caches hold several animations
    /// at once, and naming a stale one would say the user was looking at
    /// something they were not.
    fn on_screen_animation(&self) -> Option<&dyn Animated> {
        match self.stage {
            StageKind::Structural => match self.structural_view {
                StructuralView::MatchingAnim => {
                    Some(self.cached_matching_anim.as_ref()?.as_ref()?)
                }
                StructuralView::TarjanAnim => Some(self.cached_tarjan_anim.as_ref()?.as_ref()?),
                StructuralView::TearingAnim => Some(self.cached_tearing_anim.as_ref()?.as_ref()?),
                StructuralView::AliasAnim => Some(self.cached_alias_anim.as_ref()?.as_ref()?),
                _ => None,
            },
            StageKind::IndexReduction => Some(self.cached_reduction_anim.as_ref()?.as_ref()?),
            StageKind::Events if self.events_view == EventsView::PreLowering => {
                Some(self.cached_pre_lowering_anim.as_ref()?.as_ref()?)
            }
            StageKind::Initialization if self.init_view == InitView::IcPlan => {
                Some(self.cached_ic_plan_anim.as_ref()?.as_ref()?)
            }
            StageKind::Flatten if self.flatten_view == FlattenView::Connections => {
                Some(self.cached_connection_anim.as_ref()?.as_ref()?)
            }
            _ => None,
        }
    }


    /// One line saying how far the pipeline got, for the action trail.
    ///
    /// Reports the **first failing stage** when there is one, because that is the
    /// diagnostic — everything after it is "not reached" and says nothing. On success
    /// it reports the furthest stage reached, which is how a partial pipeline (a model
    /// that compiles but will not lower) is told apart from a complete one.
    fn compile_outcome(&self) -> String {
        let model = self.model.as_deref().unwrap_or("<unnamed>");
        for kind in StageKind::COMPILATION {
            let stage = self.stages.get(*kind);
            if stage.note_is_error() {
                return format!("{model}: FAILED at {}", kind.name());
            }
        }
        format!("{model}: reached {}", self.last_successful_stage().name())
    }

    /// Whether a structural sub-view has a tab right now.
    ///
    /// **One predicate, used by the tab bar and by the link guard**, so a link cannot
    /// select a view that has no tab. Doug hit exactly that: the cross-platform tour
    /// linked to `Structural/Summary`, which exists only when a model is *singular* —
    /// `ProportionalLoop` is not, so the link selected a view with no tab and the panel
    /// rendered the singular summary for a non-singular model.
    ///
    /// Availability depends on the *model*, not just the stage, which is why
    /// `SubView::from_slug` cannot catch it: that validates a slug against a stage, and
    /// this is a question about what the compile produced.
    fn structural_view_available(&self, v: StructuralView) -> bool {
        let is_index_reduction = self.stage == StageKind::IndexReduction;
        let is_singular = self
            .stages
            .get(self.stage)
            .note
            .as_deref()
            .is_some_and(|n| n.contains("singular"));
        match v {
            // Summary is the singular-system explanation, plus Index Reduction's report.
            StructuralView::Summary => is_index_reduction || is_singular,
            StructuralView::Animate => {
                is_index_reduction && !self.index_reduction_frames.is_empty()
            }
            StructuralView::AliasAnim => is_index_reduction && self.has_alias_eliminations(),
            // These need a complete matching to mean anything.
            StructuralView::SpyPlot | StructuralView::TarjanAnim | StructuralView::TearingAnim => {
                !is_singular || is_index_reduction
            }
            // Always available: the incidence pattern, the matching *search* (whose
            // failure is the point on a singular system), and the raw tree.
            StructuralView::Incidence | StructuralView::MatchingAnim | StructuralView::Tree => true,
        }
    }

    /// Show a notice **and record it**.
    ///
    /// Every notice is something HRW is telling Doug went wrong — an unresolvable node
    /// path, a frame that does not exist, a specimen not found. Those are exactly the
    /// events Claude needs when Doug reports "there seem to be several bugs", and until
    /// 2026-07-30 they existed only on screen for a few seconds.
    ///
    /// One function so the shown text and the recorded text cannot drift apart.
    fn notify(&mut self, message: impl Into<String>) {
        let message = message.into();
        diagnostics::record_action("notice", message.clone());
        self.notice = Some(message);
    }

    /// Act on a followed `hrw://` link.
    ///
    /// Extracted from `ui` 2026-07-29 so the ordering rules below can be tested — they
    /// are subtle, and one of them was already wrong.
    ///
    /// **Every sub-view request goes through `pending_sub_view`, never
    /// `apply_sub_view`.** The centre panel resets the sub-view whenever a report stage
    /// is entered (forcing `Summary` for Index Reduction, `SpyPlot` for a non-singular
    /// Structural), and that reset runs *after* this. Applying a sub-view here would be
    /// silently overwritten, and the symptom is the one Doug hit: the link appears to do
    /// nothing the first time and works the second, because the second click no longer
    /// changes the stage and so skips the reset.
    ///
    /// `LoadAndSwitch` already did this correctly and its comment said why; the three
    /// sibling verbs did not, which is the whole bug.
    fn dispatch_hrw_link(&mut self, action: HrwLink) {
        // **Record every followed link.** Doug clicking a tour stop is the single most
        // informative thing that happens in a session, and it was invisible: when he
        // reported bugs in a fixture tour, `session.json` showed the specimen load and
        // nothing after. Now the trail names each stop in order, so a bug report can
        // start from what was actually clicked rather than from a reconstruction.
        //
        // Deliberately **not** written to `focus.json`. That file is the noun Doug
        // *assembles*; overwriting it on every click would destroy what he is pointing
        // at and break the composition primitives. This is a different question —
        // "what did I do", not "what should you look at".
        diagnostics::record_action("tour-link", action.describe());

        // A link that needs a specimen, with none loaded, is refused rather than
        // half-applied. Setting `pending_stage` here and returning would be worse than
        // doing nothing: it would linger and fire when a specimen arrived later, sending
        // the reader somewhere no link had pointed — the same trap the frame-seek budget
        // exists to close.
        if action.requires_specimen() && self.selected.is_none() {
            self.notify(
                "no specimen loaded \u{2014} this stop needs one. Start at the tour's first \
                 stop, which loads it.",
            );
            return;
        }
        match action {
            HrwLink::LoadSpecimen(name) => {
                // **One verb, three sources** — deliberately not a second verb.
                //
                // The corpus list shows curated specimens, scratch probes and the
                // 2,626 MSL models in one widget (`docs/ideas.md` #52), so from a
                // tour's point of view they are all just models. A separate
                // `hrw://model/` verb would split one gesture in two and need
                // merging later, which is the mistake Test mode was.
                //
                // Files first: a curated specimen and a library model could in
                // principle share a name, and the repo's own copy should win.
                if let Some(path) = self.find_specimen(&name) {
                    self.open(path);
                } else if name.contains('.') {
                    // A qualified name — ask the library. The worker reports
                    // clearly if no such model is loaded, so nothing is guessed
                    // at here.
                    self.open_library_model(&name);
                } else {
                    self.notify(format!(
                        "not found: {name} - no specimen by that name, and it is not a \
                         qualified model name (those contain dots)"
                    ));
                }
            }
            HrwLink::ShowSource(line) => {
                // The source lives in Specimen mode's Source detail, so getting there
                // means setting both — a tour should not have to tell Doug which mode
                // to be in.
                self.ui_mode = UiMode::Specimen;
                self.specimen_detail = SpecimenDetail::Source;
                self.viewing_log = false;
                self.source_scroll_target = line;
            }
            HrwLink::SwitchStage(kind, sub) => {
                self.stage = kind;
                self.viewing_log = false;
                self.pending_sub_view = sub;
            }
            HrwLink::LoadAndSwitch(name, kind, sub) => {
                if let Some(path) = self.find_specimen(&name) {
                    self.open(path);
                    self.pending_stage = Some(kind);
                    self.pending_sub_view = sub;
                } else {
                    self.notify(format!("specimen not found: {name}"));
                }
            }
            HrwLink::AimAtEquation(kind, sub, equation) => {
                self.stage = kind;
                self.viewing_log = false;
                self.pending_sub_view = Some(sub);
                // Deferred: turning an equation index into a world position needs the
                // view's own layout, which exists only at paint time.
                self.aim_at_equation = Some(equation);
            }
            HrwLink::PointAtNode(kind, sub, path) => {
                self.stage = kind;
                self.viewing_log = false;
                self.pending_sub_view = Some(sub);
                // `jump_target` already forces ancestors open and scrolls, and is
                // consumed on the frame it is honoured — the same one-shot discipline
                // as the camera aim and the frame seek.
                self.jump_target = Some(path.clone());
                self.jump_highlight = Some(path);
            }
            HrwLink::OpenNotebook(name) => match bridge::resolve_notebook(&name) {
                Some(path) => {
                    if let Err(e) = open_with_os(&path) {
                        self.notify(format!("could not open {name}: {e}"));
                    }
                }
                None => self.notify(format!(
                    "no notebook {name} \u{2014} the link names one that is not here",
                )),
            },
            HrwLink::OpenInSystemModeler(name) => match self.find_specimen(&name) {
                Some(path) => {
                    if let Err(e) = open_with_os(&path) {
                        self.notify(format!("could not open {name} in System Modeler: {e}"));
                    }
                }
                None => self.notify(format!("specimen not found: {name}")),
            },
            HrwLink::Follow(name) => {
                // Following is independent of what is pointed at: a stop may set one,
                // the other, or both. So this deliberately does not touch the stage.
                self.set_tracked_identifier(name);
            }
            HrwLink::SeekFrame(kind, sub, frame) => {
                self.stage = kind;
                self.viewing_log = false;
                self.pending_sub_view = Some(sub);
                // Deferred for the same reason, plus one more: the animation for this
                // sub-view may not be built until it paints.
                self.seek_frame = Some((frame, SEEK_ATTEMPTS));
            }
        }
    }

    /// The on-screen animation, mutably — the seeking twin of
    /// [`Self::on_screen_animation`].
    ///
    /// Two functions rather than one generic over mutability because the immutable one
    /// serves the **capture**, and a capture must never move what it describes. Keeping
    /// them separate makes that a type-level fact rather than a convention.
    ///
    /// The duplicated match is deliberate and small; if a third caller appears, the
    /// sub-view-to-animation mapping should become a table instead.
    fn on_screen_animation_mut(&mut self) -> Option<&mut dyn Animated> {
        match self.stage {
            StageKind::Structural => match self.structural_view {
                StructuralView::MatchingAnim => {
                    Some(self.cached_matching_anim.as_mut()?.as_mut()?)
                }
                StructuralView::TarjanAnim => Some(self.cached_tarjan_anim.as_mut()?.as_mut()?),
                StructuralView::TearingAnim => Some(self.cached_tearing_anim.as_mut()?.as_mut()?),
                StructuralView::AliasAnim => Some(self.cached_alias_anim.as_mut()?.as_mut()?),
                _ => None,
            },
            StageKind::IndexReduction => Some(self.cached_reduction_anim.as_mut()?.as_mut()?),
            StageKind::Events if self.events_view == EventsView::PreLowering => {
                Some(self.cached_pre_lowering_anim.as_mut()?.as_mut()?)
            }
            StageKind::Initialization if self.init_view == InitView::IcPlan => {
                Some(self.cached_ic_plan_anim.as_mut()?.as_mut()?)
            }
            StageKind::Flatten if self.flatten_view == FlattenView::Connections => {
                Some(self.cached_connection_anim.as_mut()?.as_mut()?)
            }
            _ => None,
        }
    }

    /// Apply a pending `hrw://…/frame/<n>` seek, if the on-screen animation can take it.
    ///
    /// Called from the paint path *after* the animation views have had a chance to
    /// build themselves, which is why it is deferred rather than applied at link
    /// dispatch.
    fn apply_pending_seek(&mut self) {
        let Some((target, attempts)) = self.seek_frame else {
            return;
        };
        // Probe first: `on_screen_animation_mut` borrows `self`, so the pending flag
        // has to be settled before the borrow starts.
        if self.on_screen_animation().is_none() {
            // Not built yet — or this view never has one. Spend an attempt rather than
            // waiting forever; see `seek_frame` on why an unbounded retry is a bug.
            // `saturating_sub` then test for zero, so exactly `SEEK_ATTEMPTS` paints
            // are spent. `checked_sub` needed one *extra* call to clear, because it
            // only returns `None` when already at zero — an off-by-one my own expiry
            // test caught.
            let left = attempts.saturating_sub(1);
            self.seek_frame = (left > 0).then_some((target, left));
            return;
        }
        self.seek_frame = None;
        let ok = self.on_screen_animation_mut().is_some_and(|a| a.seek(target));
        if !ok {
            let (_, total) = self.on_screen_animation().map_or((0, 0), Animated::position);
            // Report the number Doug typed, not the internal cursor: the link is
            // 1-based to match the on-screen counter, so quoting `target` raw would be
            // off by one — the very bug this change fixes.
            self.notify(format!(
                "no frame {} in this replay \u{2014} it has {total}",
                target + 1,
            ));
        }
    }

    fn animation_view(&self) -> Option<bridge::AnimationView<'static>> {
        let anim = self.on_screen_animation()?;
        let (frame, frame_count) = anim.position();
        Some(bridge::AnimationView {
            which: anim.which(),
            frame,
            frame_count,
            live_state: anim.live_state(false).name(),
            // What the user is looking at, not merely where they are.
            frame_context: anim.current_frame_context(),
        })
    }

    /// The application state a crash file should carry.
    ///
    /// Built every frame and handed to [`crate::diagnostics::set_snapshot`], so
    /// the panic hook — which cannot borrow `App` — still has it. Every field
    /// here is one I had to reconstruct from Doug's description of what he
    /// clicked while diagnosing the 2026-07-28 crash; the list is that session's
    /// findings turned into code rather than a guess at what might be useful.
    ///
    /// Deliberately *not* an interpretation. It reports what the state is, not
    /// what it means — the same rule the bridge follows (`DECISIONS.md`,
    /// 2026-07-28). `stages` lists which IRs exist rather than judging the
    /// compile good or bad, because "Flatten produced a note but no value" is a
    /// fact and "compilation partly failed" is a conclusion.
    fn diagnostic_snapshot(&self) -> Value {
        let anim = self.animation_diagnostic();
        json!({
            "specimen": self.selected.as_ref().map(|p| p.display().to_string()),
            "model": self.model,
            "ui_mode": format!("{:?}", self.ui_mode),
            "specimen_detail": format!("{:?}", self.specimen_detail),
            "stage_tab": self.stage.name(),
            "viewing_log": self.viewing_log,
            "compiling": self.compiling,
            // Non-empty means the view is showing a library class, not the
            // specimen — which changes what every other field refers to.
            "navigation": self.nav.iter().map(|n| n.name.clone()).collect::<Vec<_>>(),
            "context": {
                "seq": self.context_seq,
                "pointing_at": self.pointed_at.as_ref().map(|p| json!({
                    "seq": p.seq,
                    "target": p.target,
                    "kind": match &p.kind {
                        PointKind::Node(path) => format!("node {}", bridge::describe_path(path)),
                        PointKind::Stage => "stage".to_owned(),
                        PointKind::Specimen => "specimen".to_owned(),
                    },
                    "stage": p.stage.name(),
                    "request": format!("{:?}", p.request),
                })),
                "following": self.tracked_identifier.as_ref().map(|name| json!({
                    "identifier": name,
                    "seq": self.track_seq,
                    "mentions": self.tracking_summary.map(|(m, _)| m),
                    "stages_with_mentions": self.tracking_summary.map(|(_, s)| s),
                })),
                "last_emission_error": self.point_error,
                "status_line": self.notice,
            },
            "animation": anim,
            "live_trace": {
                "breakpoint_armed": self.live_breakpoint_armed,
                "awaiting_ack": self.pending_live_debug.is_some(),
            },
            "simulation": {
                "running": self.sim_running,
                "error": self.sim_error,
                "has_data": self.sim_data.is_some(),
                "t_end": self.sim_t_end,
            },
            "stages": self.stages.as_stage_pairs().iter()
                .map(|(name, value)| json!({ "stage": name, "has_ir": value.is_some() }))
                .collect::<Vec<_>>(),
            "counts": {
                "log_entries": self.log_entries.len(),
                "specimen_files": self.files.len(),
                "resolved_def_ids": self.def_index.len(),
                "known_variables": self.known_variables.as_ref().map(HashSet::len),
            },
        })
    }

    /// Which animation is on screen and where its cursor stands.
    ///
    /// Reported only for the animation belonging to the *current* stage tab:
    /// the caches can hold several at once, and listing a stale one would
    /// suggest the user was looking at something they were not.
    fn animation_diagnostic(&self) -> Value {
        let Some(anim) = self.on_screen_animation() else { return Value::Null };
        let (frame, frame_count) = anim.position();
        json!({
            "which": anim.which(),
            "frame": frame,
            "frame_count": frame_count,
            "live_state": anim.live_state(false).name(),
            // A crash mid-animation is one of the harder ones to reproduce, so
            // the file carries what was being shown, not just the index.
            "showing": anim.current_frame_context(),
        })
    }

    /// The next stamp from the **shared** context counter.
    ///
    /// One counter for both halves, so `seq` and `tracking.seq` are directly
    /// comparable and "which did the user touch last?" has an answer. Two
    /// independent counters looked comparable and were not: after twelve
    /// captures and one follow they read 12 and 1, which says nothing about
    /// recency — and a reader trusting the instructions would conclude the
    /// wrong thing. Found on the first real `explain`.
    fn next_seq(&mut self) -> u64 {
        self.context_seq += 1;
        self.context_seq
    }

    /// Capture the node the user acted on — scoped to the navigated class when
    /// navigating, else to the current specimen stage (with cross-stage diff).
    /// `request` is "explain" (Ask Claude) or "debug-where-set" (the debugger).
    ///
    /// The two code paths handle different contexts:
    /// - **Navigated class** (nav stack non-empty): we're viewing a library
    ///   class reached via "Go to definition". No Parse stage exists here
    ///   (it's not a specimen), so no cross-stage diff is possible.
    /// - **Specimen stage** (nav stack empty): we're viewing the specimen's own
    ///   IR. The capture includes the Parse and Resolve values so Claude can
    ///   diff across stages (e.g. "what did Typecheck change vs Instantiate?").
    fn emit_node_focus(&mut self, key_path: Vec<Seg>, request: bridge::AskRequest) {
        // Same rule as `emit_focus`: a point of Doug's own supersedes the link's mark.
        // There are **two** capture paths — this one for nodes, `emit_focus` for stage
        // and specimen — and clearing in only one is exactly the omission the test for
        // this caught.
        self.jump_highlight = None;
        let seq = self.next_seq();
        let target = bridge::describe_path(&key_path);
        let request_str = request.as_str();

        let status = if let Some(entry) = self.nav.last() {
            let ask = Ask {
                seq,
                request,
                specimen: None,
                model: Some(&entry.name),
                stage: None,
                libraries: self.library_strings(),
                def_index: &entry.def_index,
                parse_value: None,
                resolve_value: None,
                focus: Focus::Node { key_path, stage_value: &entry.value },
                // A navigated library class is outside the specimen pipeline,
                // so there are no stages to sweep for the followed identifier.
                tracking: None,
                view: self.view_context(),
                failure: self.failure_context(),
            };
            status_line(seq, &target, request_str, bridge::write(&ask))
        } else {
            let stage_value = self.current_stage().value.clone();
            match &stage_value {
                Some(value) => {
                    let focus = Focus::Node { key_path: key_path.clone(), stage_value: value };
                    let stage_values = self.stages.as_stage_pairs();
                    let ask = self.base_ask(seq, request, focus, &stage_values);
                    let result = bridge::write(&ask);
                    // Retained so the Context Bar can state it, and so a later
                    // change of what is followed re-emits without losing it.
                    self.pointed_at = Some(PointedAt {
                        seq,
                        target: target.clone(),
                        kind: PointKind::Node(key_path),
                        stage: self.stage,
                        request,
                    });
                    self.point_error = result.as_ref().err().map(std::string::ToString::to_string);
                    status_line(seq, &target, request_str, result)
                }
                None => Some("(no IR for this stage to point at)".to_owned()),
            }
        };
        self.notice = status;
    }

    fn start_simulation(&mut self) {
        if let (Some(path), Some(model)) = (self.selected.clone(), self.model.clone()) {
            self.sim_running = true;
            self.sim_data = None;
            self.sim_error = None;
            self.worker.send(ToWorker::Simulate { path, model, t_end: self.sim_t_end });
        }
    }

    /// The Simulation view — a Run control + an `egui_plot` pane of the
    /// state trajectories. Running the model is on-demand (not a compile stage):
    /// Run dispatches `ToWorker::Simulate` to the worker thread, and the plot
    /// appears when `FromWorker::Simulated` lands (see `drain_worker`).
    fn simulation_pane(&mut self, ui: &mut egui::Ui) {
        use egui_plot::{Corner, Legend, Line, Plot, PlotPoints};

        let mut run = false;
        ui.horizontal(|ui| {
            run = ui
                .add_enabled(self.selected.is_some() && !self.sim_running, egui::Button::new("▶ Run"))
                .on_hover_text("Compile → lower → integrate, then plot the trajectories.")
                .clicked();
            ui.add(egui::Slider::new(&mut self.sim_t_end, 0.1..=20.0).step_by(0.1).text("stop time"));
            if self.sim_running {
                ui.spinner();
                ui.weak("simulating…");
            }
        });
        if let Some(e) = &self.sim_error {
            egui::ScrollArea::horizontal().id_salt("sim_err").show(ui, |ui| {
                ui.colored_label(ui.visuals().error_fg_color, egui::RichText::new(e).monospace());
            });
        }
        if run {
            self.start_simulation();
        }
        ui.separator();
        match &self.sim_data {
            Some(data) => {
                let has_diagnostics = !data.solver_steps.is_empty();
                let link_group = ui.id().with("sim_time_axis");

                let mut trajectory_plot = Plot::new("sim_plot")
                    .legend(Legend::default().position(Corner::LeftTop))
                    .x_axis_label("time");
                if has_diagnostics {
                    trajectory_plot = trajectory_plot
                        .link_axis(link_group, [true, false])
                        .link_cursor(link_group, [true, false])
                        .height(ui.available_height() * TRAJECTORY_PLOT_HEIGHT_FRACTION);
                }
                let tracked = self.tracked_identifier.as_deref();
                trajectory_plot.show(ui, |plot_ui| {
                    for (i, (name, series)) in data.names.iter().zip(&data.data).enumerate() {
                        let segments = if data.has_discontinuities {
                            discontinuity_segments(series)
                        } else {
                            std::iter::once(0..series.len()).collect()
                        };
                        let is_tracked = tracked == Some(name.as_str());
                        let color = if is_tracked {
                            crate::colors::TRACKED_GOLD
                        } else {
                            series_color(i)
                        };
                        let width = if is_tracked { 3.0 } else { 1.0 };
                        for seg in segments {
                            let pts: PlotPoints = data.times[seg.clone()]
                                .iter()
                                .zip(&series[seg])
                                .map(|(&t, &y)| [t, y])
                                .collect();
                            plot_ui.line(Line::new(name.clone(), pts).color(color).width(width));
                        }
                    }
                });

                if has_diagnostics {
                    ui.separator();
                    ui.strong("Solver diagnostics");

                    Plot::new("solver_diagnostics")
                        .legend(Legend::default().position(Corner::LeftTop))
                        .link_axis(link_group, [true, false])
                        .link_cursor(link_group, [true, false])
                        .x_axis_label("time")
                        .y_axis_label("step size h  /  BDF order k")
                        .show(ui, |plot_ui| {
                            let h_pts: PlotPoints = data.solver_steps.iter()
                                .map(|s| [s.t, s.h])
                                .collect();
                            plot_ui.line(
                                Line::new("step size h", h_pts)
                                    .color(crate::colors::SOLVER_STEP_SIZE),
                            );

                            let order_pts: PlotPoints = data.solver_steps.iter()
                                .map(|s| [s.t, s.order as f64])
                                .collect();
                            plot_ui.line(
                                Line::new("BDF order k", order_pts)
                                    .color(crate::colors::SOLVER_BDF_ORDER),
                            );
                        });
                }
            }
            None if !self.sim_running => {
                ui.weak("Press ▶ Run to simulate this specimen and plot its state trajectories.");
            }
            None => {}
        }
    }

}

/// A notice for the status bar after a bridge write — **only when something
/// needs saying**.
///
/// ## Why success is silent now
///
/// The status bar used to confirm every capture ("captured equations.3.lhs —
/// now ask me about it in the chat"). The Context Bar makes that redundant, and
/// worse than redundant: the bar states the point *persistently and by name*,
/// while the status line stated it once and then went stale, so the two could
/// disagree about what Claude had. Two places claiming to describe the same
/// thing is exactly the failure this design keeps hitting — a transient
/// confirmation is the weaker of the two, so it goes.
///
/// A failure still returns text, because a failure is precisely the case the
/// Context Bar cannot show on its own: it renders the point either way, and
/// silence would leave it describing context that was never written. (The bar
/// *also* flags it via `point_error`; this is the second, transient channel for
/// the moment it happens.)
///
/// `debug-where-set` still speaks on success, because it asks the user to do
/// something next — say "debug" in the chat — and an instruction is not a
/// confirmation.
fn status_line(
    seq: u64,
    target: &str,
    request: &str,
    result: std::io::Result<std::path::PathBuf>,
) -> Option<String> {
    match result {
        Err(e) => Some(format!("\u{26a0} not emitted \u{2014} {e}")),
        Ok(_) if request == "debug-where-set" => Some(format!(
            "🐞 pointing at  {target}  for the debugger \u{2014} say \u{201c}debug\u{201d} \
             in chat and I'll set the breakpoint  (context #{seq})"
        )),
        Ok(_) => None,
    }
}

// The `eframe::App` trait is what eframe calls each frame. `ui()` is the
// single entry point: it receives a `&mut egui::Ui` and must build the
// entire window's contents from scratch. This is the heart of the
// immediate-mode pattern — no persistent widget tree, just code that runs
// every frame.
impl App {
    fn floating_windows(&mut self, ui: &mut egui::Ui) {
        egui::Window::new("Using HRW")
            .open(&mut self.show_help)
            .collapsible(false)
            .default_width(440.0)
            .show(ui.ctx(), |ui| {
                ui.strong("Inspect");
                ui.label("Pick a specimen (left), choose Parse/Resolve, and expand the IR tree.");
                ui.add_space(6.0);
                ui.strong("Assemble context");
                ui.label(
                    "Two ways to build the subject of a question, and the Context Bar shows \
                     both. Point at one node \u{2014} left-click it, or right-click for more \
                     actions. Follow one identifier \u{2014} right-click a variable name and \
                     choose Follow, and HRW reports where it appears in every stage, and \
                     where it does not.",
                );
                ui.add_space(2.0);
                ui.label(
                    "Then ask in the Claude Code chat \u{2014} Claude reads what you assembled. \
                     Shortcut: just type \u{201c}explain\u{201d}, with no need to phrase a question.",
                );
                ui.add_space(6.0);
                ui.strong("Diff stages");
                ui.label(
                    "Every point publishes all stages\u{2019} full IR, so Claude can compare any \
                     two on request. Point at anything, then ask in the chat — e.g. \u{201c}what did \
                     Typecheck change vs Instantiate?\u{201d} (the resolved type_ids) or \u{201c}diff Parse and \
                     Resolve here\u{201d} (def_ids filled in) — and Claude reads the two stages and reports \
                     the differences. (A node captured on Parse/Resolve also carries its own \
                     before/after inline, so \u{201c}explain\u{201d} alone shows what Resolve changed.)",
                );
                ui.add_space(6.0);
                ui.strong("Structural (spy-plot)");
                ui.label(
                    "On the Structural stage, the BLT block structure is drawn as a spy-plot: \
                     diagonal blocks in evaluation order — scalar solves are single cells, coupled \
                     algebraic loops are boxes. Drag to pan, scroll to zoom, hover a block to see its \
                     equations/unknowns/tearing, and click it to capture it for \u{201c}explain\u{201d}. Toggle to \
                     Tree for the raw report.",
                );
                ui.add_space(6.0);
                ui.strong("Navigate");
                ui.label(
                    "Some fields hold a DefId that resolves to a class — the tree shows it inline \
                     (e.g. \u{201c}type_def_id: 27579 → model …\u{201d}). Right-click that field and choose \
                     \u{201c}Go to …\u{201d} to open that class\u{2019}s own IR. Use Back to step up one level, or \
                     Specimen to return to the top.",
                );
                ui.add_space(6.0);
                ui.strong("Debugger");
                ui.label(
                    "Right-click a field and choose 🐞 \u{201c}Show this being set\u{201d}, then tell Claude \
                     \u{201c}debug\u{201d} in the chat. Claude sets a breakpoint on the running debug session; \
                     right-click the specimen and choose Recompile to hit it.",
                );
            });
        egui::Window::new("About HRW Observatory")
            .open(&mut self.show_about)
            .collapsible(false)
            .resizable(false)
            .show(ui.ctx(), |ui| {
                ui.strong("HRW Observatory");
                ui.label("An egui instrument for studying the Rumoca Modelica compiler pipeline.");
                ui.separator();
                ui.label(format!("HRW v{}", env!("CARGO_PKG_VERSION")));
                ui.label(format!(
                    "Built against Rumoca {} · git {}",
                    env!("HRW_RUMOCA_VERSION"),
                    env!("HRW_RUMOCA_REV"),
                ));
                ui.label("Rumoca is linked as a library; compilation runs on a worker thread.");
            });
        // Settings uses the "deferred action" pattern: the `.open()` borrow
        // prevents calling `self.load_libraries()` inside the closure, so we
        // collect intent and act after the closure.
        let mut load_libraries = false;
        let mut rescan_specimens = false;
        egui::Window::new("Settings")
            .open(&mut self.show_settings)
            .collapsible(false)
            .default_width(560.0)
            .show(ui.ctx(), |ui| {
                ui.strong("Display");
                let mut zoom = ui.ctx().zoom_factor();
                if ui
                    .add(egui::Slider::new(&mut zoom, 0.75..=3.0).step_by(0.05).text("Font / UI scale"))
                    .changed()
                {
                    ui.ctx().set_zoom_factor(zoom);
                }
                ui.separator();
                ui.strong("Specimen directory");
                ui.horizontal(|ui| {
                    let changed = ui.add(
                        egui::TextEdit::singleline(&mut self.specimen_dir)
                            .desired_width(f32::INFINITY)
                            .font(egui::TextStyle::Monospace),
                    ).changed();
                    if ui.button("⟳").on_hover_text("Rescan directory").clicked() || changed {
                        rescan_specimens = true;
                    }
                });
                ui.separator();
                ui.strong("Library source roots");
                ui.label("One package directory (or single .mo) per line:");
                ui.add(
                    egui::TextEdit::multiline(&mut self.libraries_text)
                        .desired_rows(4)
                        .desired_width(f32::INFINITY)
                        .font(egui::TextStyle::Monospace),
                );
                ui.horizontal(|ui| {
                    if ui.add_enabled(!self.libraries_busy, egui::Button::new("Load libraries")).clicked() {
                        load_libraries = true;
                    }
                    if self.libraries_busy {
                        ui.spinner();
                    }
                    ui.weak(&self.library_status);
                });
            });
        if load_libraries {
            self.load_libraries();
        }
        if rescan_specimens {
            self.rescan();
        }
    }

    fn equation_sheet_ui(&mut self, ui: &mut egui::Ui) {
        let Some(sheet) = &self.cached_equation_sheet else {
            ui.weak("(no equation sheet)");
            return;
        };

        let has_incidence = self.cached_incidence.as_ref().is_some_and(|c| c.is_some())
            || self.stages.get(StageKind::Structural).value.is_some();

        let mut clicked_row = None;
        let mut clicked_variable: Option<String> = None;

        egui::ScrollArea::both()
            .id_salt("equation_sheet")
            .auto_shrink(false)
            .show(ui, |ui| {
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(format!(
                        "{} continuous equations   |   {} states, {} algebraics, {} parameters",
                        sheet.n_equations, sheet.n_states, sheet.n_algebraics, sheet.n_parameters,
                    ))
                    .strong(),
                );
                if sheet.n_constants > 0 || sheet.n_discrete > 0 || sheet.n_inputs > 0 || sheet.n_outputs > 0 {
                    let mut extras = Vec::new();
                    if sheet.n_constants > 0 { extras.push(format!("{} constants", sheet.n_constants)); }
                    if sheet.n_discrete > 0 { extras.push(format!("{} discrete", sheet.n_discrete)); }
                    if sheet.n_inputs > 0 { extras.push(format!("{} inputs", sheet.n_inputs)); }
                    if sheet.n_outputs > 0 { extras.push(format!("{} outputs", sheet.n_outputs)); }
                    ui.weak(extras.join(", "));
                }

                if has_incidence {
                    ui.weak("Click an equation to highlight it in the incidence matrix.");
                }

                ui.add_space(8.0);

                let tracked = self.tracked_identifier.as_deref();
                for (cat, eqs) in &sheet.groups {
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new(format!("{} ({})", cat.label(), eqs.len())).strong());
                    ui.weak(cat.description());
                    ui.add_space(2.0);
                    // Equations are Modelica-shaped text, so they get the same
                    // syntax colouring as the specimen source view. The tracked
                    // identifier is highlighted per token rather than by tinting
                    // the whole row — `eq.text.contains(t)` used to match
                    // `height` when tracking `h`, and then shade the entire
                    // equation rather than the mention within it.
                    let modelica = crate::source_view::ModelicaText::new(ui)
                        .tracked(tracked.map(|t| (t, crate::colors::TRACKED_FILL_MEDIUM)));
                    for eq in eqs {
                        let selected = self.highlighted_eq_row == Some(eq.index);
                        let text = modelica.job(&eq.text);
                        if has_incidence {
                            let resp = ui.selectable_label(selected, text);
                            if resp.clicked() {
                                clicked_row = Some(if selected { None } else { Some(eq.index) });
                            }
                            resp.on_hover_text(format!("f_x[{}] — {}", eq.index, &eq.origin));
                        } else {
                            ui.horizontal(|ui| { ui.label(text); });
                        }
                    }
                }

                ui.add_space(12.0);
                ui.separator();
                ui.add_space(4.0);
                ui.label(egui::RichText::new("Variable classification").strong());
                ui.add_space(2.0);

                egui::Grid::new("var_grid")
                    .striped(true)
                    .num_columns(4)
                    .spacing([12.0, 2.0])
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new("Name").strong());
                        ui.label(egui::RichText::new("Kind").strong());
                        ui.label(egui::RichText::new("Start").strong());
                        ui.label(egui::RichText::new("Unit").strong());
                        ui.end_row();

                        for v in &sheet.variables {
                            let is_tracked = tracked == Some(v.name.as_str());
                            let mut name_rt = egui::RichText::new(&v.name).monospace();
                            if is_tracked {
                                name_rt = name_rt
                                    .strong()
                                    .background_color(crate::colors::TRACKED_FILL_MEDIUM);
                            }
                            // Reverse tracking (#37): clicking a variable here
                            // tracks it, and the source view scrolls to its
                            // declaration. Clicking the tracked one again clears
                            // it, matching the source view's toggle behaviour.
                            let resp = ui.add(
                                egui::Label::new(name_rt).sense(egui::Sense::click()),
                            );
                            if resp.clicked() {
                                clicked_variable = Some(v.name.clone());
                            }
                            // One vocabulary with every other follow surface, via
                            // the shared helper. Two hand-written variants of
                            // the same sentence drift, and this one still said
                            // "track" after the rename.
                            resp.on_hover_text(crate::follow_hover(&v.name, is_tracked));
                            ui.label(v.kind);
                            ui.label(v.start.as_deref().unwrap_or("—"));
                            ui.label(v.unit.as_deref().unwrap_or(""));
                            ui.end_row();
                        }
                    });
            });

        if let Some(new_val) = clicked_row {
            self.highlighted_eq_row = new_val;
            if new_val.is_some() {
                self.stage = StageKind::Structural;
                self.structural_view = StructuralView::Incidence;
            }
        }
        if let Some(name) = clicked_variable {
            self.set_tracked_identifier(name);
            // The whole point of reverse tracking is *seeing* the declaration,
            // and the specimen source view only renders in Specimen mode — in
            // Tour or Debug mode the click sets tracking and appears to do
            // nothing. So reveal the source, the same way clicking an equation
            // already navigates to the incidence matrix.
            if self.tracked_identifier.is_some() {
                self.ui_mode = UiMode::Specimen;
                self.specimen_detail = SpecimenDetail::Source;
            }
        }
    }

    /// For each variable, the class that declares it — when that is not the
    /// specimen.
    ///
    /// A flattened name like `src.V` has no declaration in the specimen source:
    /// `src` is a component of the model, and `V` is a parameter of *its* type,
    /// `Modelica.Electrical.Analog.Sources.ConstantVoltage`. Without this, "where
    /// did this come from?" had no answer for such names — most of a real model,
    /// since most components are library instances.
    ///
    /// ## What this resolves, and what it does not
    ///
    /// Only the **first** path segment is resolved, against the model's own
    /// components in the Resolve IR. For `src.V` that is exact: `V` really is
    /// declared in `src`'s type. For a deeper name like `gear.flange_a.tau` it
    /// yields `gear`'s type, which *contains* the declaration rather than being
    /// it — resolving further would mean walking into library class IRs that are
    /// only loaded on demand. The UI therefore says "component `src` is a …"
    /// rather than "`V` is declared in …", which is true in both cases.
    fn build_declaring_classes(
        stages: &StageBundle,
        def_index: &BTreeMap<u64, DefInfo>,
        sheet: Option<&equation_sheet::EquationSheet>,
    ) -> HashMap<String, String> {
        let mut out = HashMap::new();
        let (Some(resolve), Some(sheet)) = (stages.get(StageKind::Resolve).value.as_ref(), sheet)
        else {
            return out;
        };
        let Some(components) = resolve.get("components").and_then(|c| c.as_object()) else {
            return out;
        };
        for var in &sheet.variables {
            let Some((head, _)) = var.name.split_once('.') else { continue };
            let class = components
                .get(head)
                .and_then(|c| c.get("type_def_id"))
                .and_then(serde_json::Value::as_u64)
                .and_then(|id| def_index.get(&id))
                .filter(|info| matches!(info.kind, DefKind::Class))
                .map(|info| info.name.clone());
            if let Some(class) = class {
                out.insert(var.name.clone(), class);
            }
        }
        out
    }

    /// The Matching animation view (Structural Analysis tab).
    ///
    /// Extracted from `ui` during the 2026-07-28 sweep. This and its two
    /// siblings below are near-identical — same six-step live-debug sequence,
    /// differing only in the `PendingLiveDebug` variant and which cached
    /// animation field they touch. Sitting adjacent makes that obvious; the
    /// actual de-duplication needs a trait over the three animation types and
    /// is logged in `docs/tech-debt.md`, deliberately not attempted here since
    /// Phase 7 will rework these views anyway.
    fn matching_anim_ui(&mut self, ui: &mut egui::Ui, ir_split: bool) {
    if self.cached_incidence.is_none() {
        self.cached_incidence = Some(
            self.stages.get(self.stage).value.as_ref()
                .and_then(incidence_view::IncidenceMatrix::from_report)
        );
    }
    let arming = self.is_arming(PendingLiveDebug::Matching);
    let live = self.cached_matching_anim.as_ref()
        .and_then(|o| o.as_ref())
        .map_or(
            if arming { LiveState::Arming } else { LiveState::Idle },
            |a| a.live_state(arming),
        );
    let debug_enabled = self.has_live_debug_data(PendingLiveDebug::Matching)
        && !live.is_busy();
    let mut debug_clicked = false;
    let action = self.live_debug_poll(
        ui.ctx(), live, PendingLiveDebug::Matching,
    );
    if matches!(action, LiveDebugAction::SpawnLive)
        && let Some(Some(mat)) = &self.cached_incidence {
            let live = matching_anim::MatchingAnimation::start_live(mat, || {
                let _ = bridge::remove_live_trace_breakpoint();
            });
            if live.is_none() {
                let _ = bridge::remove_live_trace_breakpoint();
                self.live_breakpoint_armed = false;
            }
            self.cached_matching_anim = Some(live);
            self.matching_anim_canvas.request_fit();
        }
    if self.cached_matching_anim.is_none() {
        let inc = self.cached_incidence.as_ref().unwrap();
        self.cached_matching_anim = Some(
            inc.as_ref().map(matching_anim::MatchingAnimation::from_incidence)
        );
    }
    if let Some(Some(anim)) = &mut self.cached_matching_anim {
        if ir_split {
            ui.label(egui::RichText::new("Before (raw DAE)")
                .strong().color(crate::colors::ANIM_FAIL));
            ui.weak("Matching animation unavailable (structurally singular \u{2014} only a partial matching exists)");
            ui.add_space(12.0);
            ui.label(egui::RichText::new("After (reduced)")
                .strong().color(crate::colors::ANIM_PATH_FOUND));
        }
        debug_clicked = anim.ui(
            ui, &mut self.matching_anim_canvas,
            self.tracked_identifier.as_deref(), arming, debug_enabled,
        );
    } else {
        ui.weak("(no incidence data for matching animation)");
    }
    if debug_clicked {
        self.start_live_debug(PendingLiveDebug::Matching);
    }
    }

    /// The BLT / Tarjan animation view. See [`Self::matching_anim_ui`].
    fn tarjan_anim_ui(&mut self, ui: &mut egui::Ui, ir_split: bool) {
    if self.cached_incidence.is_none() {
        self.cached_incidence = Some(
            self.stages.get(self.stage).value.as_ref()
                .and_then(incidence_view::IncidenceMatrix::from_report)
        );
    }
    let arming = self.is_arming(PendingLiveDebug::Tarjan);
    let live = self.cached_tarjan_anim.as_ref()
        .and_then(|o| o.as_ref())
        .map_or(
            if arming { LiveState::Arming } else { LiveState::Idle },
            |a| a.live_state(arming),
        );
    let debug_enabled = self.has_live_debug_data(PendingLiveDebug::Tarjan)
        && !live.is_busy();
    let mut debug_clicked = false;
    let action = self.live_debug_poll(
        ui.ctx(), live, PendingLiveDebug::Tarjan,
    );
    if matches!(action, LiveDebugAction::SpawnLive)
        && let Some(Some(mat)) = &self.cached_incidence {
            let live = tarjan_anim::TarjanAnimation::start_live(mat, || {
                let _ = bridge::remove_live_trace_breakpoint();
            });
            if live.is_none() {
                let _ = bridge::remove_live_trace_breakpoint();
                self.live_breakpoint_armed = false;
            }
            self.cached_tarjan_anim = Some(live);
            self.tarjan_anim_canvas.request_fit();
        }
    if self.cached_tarjan_anim.is_none() {
        let inc = self.cached_incidence.as_ref().unwrap();
        self.cached_tarjan_anim = Some(
            inc.as_ref().and_then(tarjan_anim::TarjanAnimation::from_incidence)
        );
    }
    // Consume a pending camera aim from `hrw://…/equation/<n>`. Taken here rather than
    // at link-dispatch time because turning an equation index into a world position
    // needs this view's own layout, which exists only at paint time.
    if let Some(target) = self.aim_at_equation
        && let Some(Some(anim)) = &self.cached_tarjan_anim
    {
        self.aim_at_equation = None;
        if !anim.aim_at_equation(&mut self.tarjan_anim_canvas, target) {
            self.notify(format!(
                "no equation {target} in this model \u{2014} the link names one that is not here",
            ));
        }
    }
    if let Some(Some(anim)) = &mut self.cached_tarjan_anim {
        if ir_split {
            ui.label(egui::RichText::new("Before (raw DAE)")
                .strong().color(crate::colors::ANIM_FAIL));
            ui.weak("BLT animation unavailable (structurally singular \u{2014} no full matching for block decomposition)");
            ui.add_space(12.0);
            ui.label(egui::RichText::new("After (reduced)")
                .strong().color(crate::colors::ANIM_PATH_FOUND));
        }
        debug_clicked = anim.ui(
            ui, &mut self.tarjan_anim_canvas,
            self.tracked_identifier.as_deref(), arming, debug_enabled,
        );
    } else {
        ui.weak("(no dependency graph for BLT animation)");
    }
    if debug_clicked {
        self.start_live_debug(PendingLiveDebug::Tarjan);
    }
    }

    /// The index-reduction animation view. See [`Self::matching_anim_ui`].
    fn reduction_anim_ui(&mut self, ui: &mut egui::Ui) {
    let arming = self.is_arming(PendingLiveDebug::Reduction);
    let live = self.cached_reduction_anim.as_ref()
        .and_then(|o| o.as_ref())
        .map_or(
            if arming { LiveState::Arming } else { LiveState::Idle },
            |a| a.live_state(arming),
        );
    let debug_enabled = self.has_live_debug_data(PendingLiveDebug::Reduction)
        && !live.is_busy();
    let mut debug_clicked = false;
    let action = self.live_debug_poll(
        ui.ctx(), live, PendingLiveDebug::Reduction,
    );
    if matches!(action, LiveDebugAction::SpawnLive)
        && let Some(dae) = &self.cached_dae {
            let live = reduction_anim::ReductionAnimation::start_live(
                dae.clone(), || {
                    let _ = bridge::remove_live_trace_breakpoint();
                },
            );
            if live.is_none() {
                let _ = bridge::remove_live_trace_breakpoint();
                self.live_breakpoint_armed = false;
            }
            self.cached_reduction_anim = Some(live);
        }
    if self.cached_reduction_anim.is_none() {
        let frames = &self.index_reduction_frames;
        self.cached_reduction_anim = Some(if frames.is_empty() {
            None
        } else {
            Some(reduction_anim::ReductionAnimation::from_frames(frames.clone()))
        });
    }
    if let Some(Some(anim)) = &mut self.cached_reduction_anim {
        debug_clicked = egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| anim.ui(ui, arming, debug_enabled))
            .inner;
    } else {
        ui.weak("(no index-reduction trace for this model)");
    }
    if debug_clicked {
        self.start_live_debug(PendingLiveDebug::Reduction);
    }
    }

    /// The tearing replay, on the Structural and Index Reduction stages.
    ///
    /// Unlike the other animated views this one is not built from the stage's
    /// JSON report: tearing works in each coupled block's own 0..n index space,
    /// and the report has already translated back to names. So the view walks
    /// the DAE again (`tearing_anim::walk_blocks`) and re-runs the algorithm
    /// with an observer attached.
    ///
    /// Which DAE depends on the tab. The Structural tab tears the raw DAE; the
    /// Index Reduction tab tears the *reduced* one, because that is the system
    /// its report describes -- and for a high-index model the raw DAE has no
    /// full matching, hence no blocks, hence nothing to tear.
    fn tearing_anim_ui(&mut self, ui: &mut egui::Ui) {
        let arming = self.is_arming(PendingLiveDebug::Tearing);
        let live = self
            .cached_tearing_anim
            .as_ref()
            .and_then(|o| o.as_ref())
            .map_or(
                if arming { LiveState::Arming } else { LiveState::Idle },
                |a| a.live_state(arming),
            );
        let debug_enabled = self.has_live_debug_data(PendingLiveDebug::Tearing) && !live.is_busy();
        let mut debug_clicked = false;
        let action = self.live_debug_poll(ui.ctx(), live, PendingLiveDebug::Tearing);
        if matches!(action, LiveDebugAction::SpawnLive)
            && let Some(dae) = self.tearing_dae()
        {
            let live = tearing_anim::TearingAnimation::start_live(dae, || {
                let _ = bridge::remove_live_trace_breakpoint();
            });
            if live.is_none() {
                let _ = bridge::remove_live_trace_breakpoint();
                self.live_breakpoint_armed = false;
            }
            self.cached_tearing_anim = Some(live);
        }
        if self.cached_tearing_anim.is_none() {
            self.cached_tearing_anim =
                Some(self.tearing_dae().map(|dae| tearing_anim::TearingAnimation::record(&dae)));
        }
        if let Some(Some(anim)) = &mut self.cached_tearing_anim {
            debug_clicked = egui::ScrollArea::vertical()
                .auto_shrink(false)
                .show(ui, |ui| anim.ui(ui, arming, debug_enabled))
                .inner;
        } else {
            ui.weak("(no DAE available for tearing)");
        }
        if debug_clicked {
            self.start_live_debug(PendingLiveDebug::Tearing);
        }
    }

    /// The DAE the current stage tab should tear -- see [`Self::tearing_anim_ui`].
    ///
    /// Index reduction is re-run here rather than carried from the worker
    /// because it is a pure function of the DAE, and re-running it keeps the
    /// two tabs from having to agree about a second cached artifact.
    fn tearing_dae(&self) -> Option<rumoca_ir_dae::Dae> {
        let dae = self.cached_dae.as_ref()?;
        if self.stage == StageKind::IndexReduction {
            let mut reduced = dae.clone();
            crate::worker::index_reduce_in_place(&mut reduced);
            Some(reduced)
        } else {
            Some(dae.clone())
        }
    }

    /// The connection-expansion replay, on the Flatten stage.
    ///
    /// Recorded only — see `connection_anim`'s module note on why there is no
    /// Debug button yet (re-running flatten needs the resolved ClassTree, which
    /// contains the whole MSL).
    fn connection_anim_ui(&mut self, ui: &mut egui::Ui) {
        if self.cached_connection_anim.is_none() {
            let frames = &self.connection_frames;
            self.cached_connection_anim = Some(if frames.is_empty() {
                None
            } else {
                Some(connection_anim::ConnectionAnimation::from_frames(frames.clone()))
            });
        }
        if let Some(Some(anim)) = &mut self.cached_connection_anim {
            egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| anim.ui(ui));
        } else {
            ui.weak("(no connections in this model)");
        }
    }

    /// The alias-elimination reveal, on the Index Reduction stage.
    ///
    /// Simpler than the replay views: no live-debug lifecycle, because this
    /// phase has no search to trace (see `alias_anim`'s module note).
    fn alias_anim_ui(&mut self, ui: &mut egui::Ui) {
        if self.cached_alias_anim.is_none() {
            self.cached_alias_anim = Some(
                self.stages
                    .get(self.stage)
                    .value
                    .as_ref()
                    .and_then(alias_anim::AliasAnimation::from_report),
            );
        }
        if let Some(Some(anim)) = &mut self.cached_alias_anim {
            egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| anim.ui(ui));
        } else {
            ui.weak("(no alias eliminations in this report)");
        }
    }

    /// Whether the current report has alias eliminations worth a tab.
    ///
    /// Read straight from the stage JSON rather than the cache: the tab bar is
    /// drawn before the view is, so the cache may not exist yet.
    fn has_alias_eliminations(&self) -> bool {
        self.stages
            .get(self.stage)
            .value
            .as_ref()
            .and_then(|v| v.get("reduction")?.get("eliminations")?.as_array())
            .is_some_and(|a| !a.is_empty())
    }

    /// The initial-condition plan walk, on the Initialization stage.
    fn ic_plan_anim_ui(&mut self, ui: &mut egui::Ui) {
        if self.cached_ic_plan_anim.is_none() {
            self.cached_ic_plan_anim = Some(
                self.stages
                    .initialization
                    .value
                    .as_ref()
                    .and_then(ic_plan_anim::IcPlanAnimation::from_report),
            );
        }
        if let Some(Some(anim)) = &mut self.cached_ic_plan_anim {
            egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| anim.ui(ui));
        } else {
            ui.weak("(no initial-condition plan in this report)");
        }
    }

    /// Whether the initialization report has a plan worth a tab. See
    /// [`Self::has_alias_eliminations`] on why this reads the JSON directly.
    fn has_ic_plan(&self) -> bool {
        self.stages
            .initialization
            .value
            .as_ref()
            .and_then(|v| v.get("blocks")?.as_array())
            .is_some_and(|a| !a.is_empty())
    }

    /// The `pre()`-lowering replay (idea #40), on the Events stage.
    ///
    /// Mirrors `reduction_anim_ui` beat for beat — the six-step live-debug
    /// sequence is the same for every animated view. The one difference is what
    /// gets re-run for a live session: the flat model rather than the DAE,
    /// because this pass happens *inside* DAE construction and the DAE HRW holds
    /// is already past it.
    fn pre_lowering_anim_ui(&mut self, ui: &mut egui::Ui) {
        let arming = self.is_arming(PendingLiveDebug::PreLowering);
        let live = self
            .cached_pre_lowering_anim
            .as_ref()
            .and_then(|o| o.as_ref())
            .map_or(
                if arming { LiveState::Arming } else { LiveState::Idle },
                |a| a.live_state(arming),
            );
        let debug_enabled =
            self.has_live_debug_data(PendingLiveDebug::PreLowering) && !live.is_busy();
        let mut debug_clicked = false;
        let action = self.live_debug_poll(ui.ctx(), live, PendingLiveDebug::PreLowering);
        if matches!(action, LiveDebugAction::SpawnLive)
            && let Some(flat) = &self.cached_flat
        {
            let live = pre_lowering_anim::PreLoweringAnimation::start_live(flat.clone(), || {
                let _ = bridge::remove_live_trace_breakpoint();
            });
            if live.is_none() {
                let _ = bridge::remove_live_trace_breakpoint();
                self.live_breakpoint_armed = false;
            }
            self.cached_pre_lowering_anim = Some(live);
        }
        if self.cached_pre_lowering_anim.is_none() {
            let frames = &self.pre_lowering_frames;
            self.cached_pre_lowering_anim = Some(if frames.is_empty() {
                None
            } else {
                Some(pre_lowering_anim::PreLoweringAnimation::from_frames(frames.clone()))
            });
        }
        if let Some(Some(anim)) = &mut self.cached_pre_lowering_anim {
            debug_clicked = egui::ScrollArea::vertical()
                .auto_shrink(false)
                .show(ui, |ui| anim.ui(ui, arming, debug_enabled))
                .inner;
        } else {
            ui.weak("(no pre() lowering in this model)");
        }
        if debug_clicked {
            self.start_live_debug(PendingLiveDebug::PreLowering);
        }
    }

    /// The central panel: stage tab bar, status banners, sub-tab bars, and the
    /// per-stage view dispatch. The bulk of the frame.
    ///
    /// Extracted from [`Self::ui`] on 2026-07-29. This was the payoff of
    /// `FrameIntent`: the body records what the user asked for into `intent`
    /// and the caller acts on it after the panel closures end, so the whole
    /// block moves with a single out-parameter.
    ///
    /// The early `return` near the top (no specimen selected) exits this method,
    /// the panel closure then ends normally, and `ui`'s deferred-action block
    /// still runs — the same order as when this was a closure body.
    fn central_panel_ui(&mut self, ui: &mut egui::Ui, intent: &mut FrameIntent) {
        if self.nav.is_empty() {
            // ---- Specimen stage view ----
            // No specimen yet → no stages to show, so don't render the tab
            // row (a highlighted tab before any compile is misleading).
            // In Debug mode the specimen list is hidden, so show the
            // dropdown here so the user can pick their first specimen.
            if self.selected.is_none() {
                if self.ui_mode == UiMode::Debug {
                    let combo = egui::ComboBox::from_id_salt("specimen_switcher")
                        .selected_text("(none)")
                        .width(120.0);
                    let mut switch_to = None;
                    combo.show_ui(ui, |ui| {
                        for path in &self.files {
                            let name = path.file_stem().and_then(|n| n.to_str()).unwrap_or("?");
                            if ui.selectable_label(false, name).clicked() {
                                switch_to = Some(path.clone());
                            }
                        }
                    });
                    if let Some(path) = switch_to {
                        self.open(path);
                    }
                } else if self.ui_mode == UiMode::Tour {
                    // **Tour mode has no specimen list to select from**, so telling Doug
                    // to select one is advice he cannot take — the same species as the
                    // Purpose tab telling him to select a specimen he had just selected.
                    // In Tour mode the specimen arrives from a stop.
                    ui.weak("Walk a tour \u{2014} its first stop loads a specimen.");
                    ui.weak("Pick one from the list on the left.");
                } else {
                    ui.weak("Select a specimen to compile.");
                }
                return;
            }
            // `horizontal_wrapped` lays widgets left-to-right, wrapping to a
            // second line when they don't fit. With 11 stage tabs, a narrow
            // window needs this (vs `horizontal` which would clip).
            ui.horizontal_wrapped(|ui| {
                // ---- Stage tab bar ----
                //
                // WHY `selectable_label` INSTEAD OF `selectable_value`:
                //
                // egui has two selection widgets:
                // - `selectable_value(&mut val, variant, text)` — ALWAYS highlights
                //   when `val == variant`. Good for radio-button groups.
                // - `selectable_label(is_selected, text)` — highlights when the
                //   bool is true. You control the condition explicitly.
                //
                // We use `selectable_label` here because we need to SUPPRESS
                // highlighting while compiling: when a fresh specimen is loading,
                // no tab should appear selected (the previous specimen's stage
                // would be misleading). The `stage_selected` bool below gates
                // this: it's false while compiling or while viewing the log, so
                // no tab highlights. `selectable_value` can't express this
                // conditional because it always highlights the current value.
                //
                // THE `stage_tab_clicked` PATTERN:
                //
                // Each stage tab checks `.clicked()` and sets the same
                // `stage_tab_clicked` flag. After the tab row, a single block
                // acts on that flag to turn off `viewing_log` and emit a stage
                // capture for the bridge. This avoids duplicating that logic
                // in every tab's click handler.
                //
                // TAB COLORING:
                //
                // Each tab label is colored via `tab_label()`:
                // - Red if the stage errored (so you see pipeline failures at a glance)
                // - Green if the stage produced IR (success)
                // - Default color if not yet reached or still compiling
                // Specimen switcher — a compact dropdown showing the
                // Specimen switcher dropdown — only in Debug mode, where
                // the specimen list is hidden.
                if self.ui_mode == UiMode::Debug {
                    let current_name = self.selected.as_ref()
                        .and_then(|p| p.file_stem())
                        .and_then(|n| n.to_str())
                        .unwrap_or("(none)");
                    let combo = egui::ComboBox::from_id_salt("specimen_switcher")
                        .selected_text(current_name)
                        .width(120.0);
                    let mut switch_to = None;
                    combo.show_ui(ui, |ui| {
                        for path in &self.files {
                            let name = path.file_stem().and_then(|n| n.to_str()).unwrap_or("?");
                            let is_selected = self.selected.as_deref() == Some(path.as_path());
                            if ui.selectable_label(is_selected, name).clicked() {
                                switch_to = Some(path.clone());
                            }
                        }
                    });
                    if let Some(path) = switch_to {
                        self.open(path);
                    }
                    ui.separator();
                }

                if ui.selectable_label(self.viewing_log, "Log").clicked() {
                    self.viewing_log = true;
                }
                ui.separator();
                // ---- Play button (inline simulation trigger) ----
                //
                // This button starts a simulation WITHOUT switching to the
                // Simulation tab. The user can be viewing the Structural
                // spy-plot or the Log and press play — the sim runs in the
                // background and the UI stays on the current view. This is
                // useful for watching log messages during simulation or
                // studying the IR while a run completes.
                //
                // `add_enabled` is like `add` (places a widget) but
                // greys it out when the bool is false. The button is only
                // active when: not compiling, not already simulating, a
                // model was parsed, and solve_lowering succeeded (the
                // simulator needs the SolveModel IR).
                let can_sim = !self.compiling
                    && !self.sim_running
                    && self.model.is_some()
                    && self.stages.solve_lowering.value.is_some();
                if ui
                    .add_enabled(can_sim, egui::Button::new("▶"))
                    .on_hover_text("Run simulation (stays on the current view)")
                    .on_disabled_hover_text("Compile a specimen first")
                    .clicked()
                {
                    self.start_simulation();
                }
                if self.sim_running {
                    ui.spinner();
                }
                ui.separator();
                let err = ui.visuals().error_fg_color;
                let ok = crate::colors::ok_color(ui.visuals().dark_mode);
                // While a freshly-selected specimen is still compiling, NO tab is
                // highlighted — the previous specimen's stage must not appear selected
                // over an empty/loading one. The highlight returns once results land
                // (`self.stage` = the furthest clean stage). Hence `selectable_label`
                // with an explicit `stage_selected && …` bool, not `selectable_value`
                // (which would always highlight the current stage).
                //
                // Selecting an IR stage tab ALSO captures that stage for the chat (no
                // separate 🔎 button) — so its context is ready the instant you view
                // it; the capture fires once below. Simulation is excluded: it's a
                // run/plot action, not an IR capture.
                let stage_selected = !self.compiling && !self.viewing_log;
                let mut stage_tab_clicked = false;
                let tabs: &[(StageKind, &str, &Stage, Option<&str>)] = &[
                    (StageKind::Parse, "Parse", &self.stages.parse, None),
                    (StageKind::Resolve, "Resolve", &self.stages.resolve, None),
                    (StageKind::Instantiate, "Instantiate", &self.stages.instantiate, None),
                    (StageKind::Typecheck, "Typecheck", &self.stages.typecheck, Some(
                        "The model-scoped instanced typecheck: it types the instantiated \
                         overlay (fills in type_ids, evaluates dimensions), so it runs AFTER \
                         Instantiate — not in Rumoca's nominal phase-3 slot. HRW can't use the \
                         pre-instantiation whole-tree typecheck; it fails on the full MSL.",
                    )),
                    (StageKind::Flatten, "Flatten", &self.stages.flatten, None),
                    (StageKind::Structural, "Structural", &self.stages.structural, Some(
                        "Structural analysis of the RAW DAE (Rumoca phase 7): maximum matching \
                         (equation↔unknown), BLT blocks (size>1 = algebraic loop), and tearing. \
                         A high-index system (rigid constraints) reports SINGULAR here — see the \
                         Index reduction tab for the reduced, solvable form. BLT spy-plot (drag \
                         to pan, scroll to zoom, click a block to capture) or the raw report tree.",
                    )),
                    (StageKind::IndexReduction, "Index reduction", &self.stages.index_reduction, Some(
                        "Structural analysis of the DAE AFTER index reduction (Pantelides / \
                         dummy derivatives): the funnel differentiates constraints and demotes states \
                         so a high-index singular system becomes matchable. For an already-index-1 \
                         model this equals Structural. Same BLT spy-plot / tree.",
                    )),
                    (StageKind::Initialization, "Initialization", &self.stages.initialization, Some(
                        "The consistent-initial-condition solve plan (build_ic_plan): the \
                         ordered blocks that compute a valid state at t=0 — direct symbolic solves, \
                         scalar Newton, torn/coupled loops — plus the relaxation hint (equations \
                         dropped / unknowns pinned) when the initial subsystem is singular, and a \
                         determinacy check that flags an OVER-determined init (more explicit initial \
                         conditions than states — conflicting/redundant ICs).",
                    )),
                    (StageKind::Events, "Events", &self.stages.events, Some(
                        "The DAE's hybrid / event structure: the conditions (relations that \
                         trigger events), the discrete updates lowered from `when` clauses (f_z real, \
                         f_m valued), and the event partition (zero-crossing root conditions + scheduled \
                         time events). A smooth (continuous) model shows none.",
                    )),
                    (StageKind::SolveLowering, "Solve lowering", &self.stages.solve_lowering, Some(
                        "The DAE lowered to a SolveModel (phase 8): the solvable form the \
                         simulator runs — residual programs, variable layout, mass matrix, Jacobian \
                         sparsity. This is the compile step just before simulation.",
                    )),
                ];
                for &(kind, label, stage, hover) in tabs {
                    let mut resp = ui.selectable_label(
                        stage_selected && self.stage == kind,
                        tab_label(label, stage, ok, err),
                    );
                    // A tab click is a point-at too — at the stage as a
                    // whole. Appended to the tab's own explanation rather
                    // than replacing it: what the stage *is* matters more
                    // than what clicking does, and this is the row where a
                    // reader is most likely to be learning the pipeline.
                    let tip = match hover {
                        Some(t) => format!("{t}\n\n{}", crate::POINT_AT_HOVER),
                        None => crate::POINT_AT_HOVER.to_owned(),
                    };
                    resp = resp.on_hover_text(tip);
                    if resp.clicked() {
                        diagnostics::record_action("stage-tab", kind.name());
                        self.stage = kind;
                        stage_tab_clicked = true;
                    }
                }
                // Simulation is a run/plot action, not an IR capture — no stage_tab_clicked.
                ui.separator();
                let sim_label = {
                    let text = egui::RichText::new("Simulation");
                    if self.sim_error.is_some() {
                        text.color(err)
                    } else if self.sim_data.is_some() {
                        text.color(ok)
                    } else {
                        text
                    }
                };
                if ui.selectable_label(stage_selected && self.stage == StageKind::Simulation, sim_label)
                    .on_hover_text(
                        "Run the model (phase 9): compile → lower to a SolveModel → integrate \
                         (Auto: BDF for stiff, RK45 otherwise), then plot the state trajectories. Runs \
                         on the worker thread, so the UI stays live.",
                    )
                    .clicked()
                {
                    self.stage = StageKind::Simulation;
                    self.viewing_log = false;
                }
                if stage_tab_clicked {
                    self.viewing_log = false;
                    if self.selected.is_some() {
                        intent.want_stage_ask = true;
                    }
                }
                // The compiled-model name is deliberately NOT shown here — the
                // stage tab row is short on horizontal space, and the same
                // identity is already visible in the specimen list and the
                // tree breadcrumb. `self.model` itself is still maintained;
                // it feeds the Claude bridge focus file, live-debug arming,
                // capture gating, and the purpose-note lookup.
                if self.compiling {
                    ui.spinner();
                }
                if let Some(n) = &self.nav_loading {
                    ui.weak(format!("opening {n}…"));
                    ui.spinner();
                }
            });

            if let Some(err) = &self.nav_error {
                ui.colored_label(ui.visuals().error_fg_color, err);
            }
            ui.separator();

            self.context_bar_ui(ui);

            if self.viewing_log {
                if log_view::ui(ui, &self.log_entries, &mut self.tracing_enabled) {
                    self.worker.send(ToWorker::SetTracing(self.tracing_enabled));
                }
            } else if self.stage == StageKind::Simulation {
                self.simulation_pane(ui);
            } else {
            // Stage note (in its own scope so its borrow of `self` ends
            // before the value section, which may borrow `self` mutably for
            // the spy-plot canvas).
            {
                let stage = self.current_stage();
                // Stages with structured error data show their own summary
                // below; Structural/IndexReduction with singular/index-1
                // notes show a status banner. Skip the generic note for both.
                let has_error_summary = stage.note_is_error()
                    && stage.value.as_ref().and_then(|v| v.get("error")).is_some();
                let has_custom_banner = matches!(
                    self.stage, StageKind::Structural | StageKind::IndexReduction
                ) && stage.note.as_deref().is_some_and(|n| n.contains("singular") || n.contains("index-1"));
                if let Some(note) = &stage.note
                    && !has_custom_banner && !has_error_summary {
                        let color = if stage.note_is_error() {
                            ui.visuals().error_fg_color
                        } else {
                            ui.visuals().weak_text_color()
                        };
                        egui::ScrollArea::horizontal().id_salt("note").show(ui, |ui| {
                            ui.colored_label(color, egui::RichText::new(note).monospace());
                        });
                        ui.separator();
                    }
            }

            // The Flatten stage offers an equation sheet alongside the tree.
            let flatten_ready =
                self.stage == StageKind::Flatten && self.cached_equation_sheet.is_some();
            let has_source_map = flatten_ready
                && self.cached_equation_sheet.as_ref().is_some_and(|s| !s.source_lines.is_empty());
            if flatten_ready {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.flatten_view, FlattenView::Equations, "Equations");
                    if has_source_map {
                        ui.selectable_value(&mut self.flatten_view, FlattenView::SourceMap, "Source Map");
                    }
                    // Connection expansion, only when the model has any --
                    // a hand-written model shows no empty tab.
                    if !self.connection_frames.is_empty() {
                        ui.selectable_value(&mut self.flatten_view, FlattenView::Connections, "Connections \u{25b6}")
                            .on_hover_text(
                                "Watch connect() statements become equations. A potential set \
                                 of n variables yields n-1 equalities; a flow set of the same \
                                 n yields one sum-to-zero equation (Kirchhoff).",
                            );
                    }
                    ui.selectable_value(&mut self.flatten_view, FlattenView::Tree, "Tree");
                });
                ui.separator();
            }

            // The Events stage offers a replay of `pre()` lowering beside the
            // tree — only when there is a trace to replay, so smooth models
            // never show an empty tab.
            let events_ready =
                self.stage == StageKind::Events && !self.pre_lowering_frames.is_empty();
            if events_ready {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.events_view, EventsView::Tree, "Tree");
                    ui.selectable_value(
                        &mut self.events_view,
                        EventsView::PreLowering,
                        "pre() lowering \u{25b6}",
                    )
                    .on_hover_text(
                        "Replay where the __pre__ parameter slots are manufactured. They \
                         appear in no source file: a `when` equation needs a value to hold \
                         when no branch fires, and a DAE cannot say \u{201c}unchanged\u{201d}.",
                    );
                });
                ui.separator();
            }

            // The Initialization stage offers a walk of the initial-condition
            // solve plan beside the tree -- only when there is a plan, so a
            // model whose initialization failed never shows an empty tab.
            let init_ready = self.stage == StageKind::Initialization && self.has_ic_plan();
            if init_ready {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.init_view, InitView::Tree, "Tree");
                    ui.selectable_value(&mut self.init_view, InitView::IcPlan, "IC plan \u{25b6}")
                        .on_hover_text(
                            "Walk the plan for computing a consistent state at t=0. Mostly \
                             plain assignment; the few blocks that iterate are where \
                             initialization fails when it fails.",
                        );
                });
                ui.separator();
            }

            // The report stages (Structural + Index reduction) offer a custom
            // BLT spy-plot alongside the generic tree; every other stage is
            // tree-only.
            let report_stage =
                matches!(self.stage, StageKind::Structural | StageKind::IndexReduction);
            let report_ready = report_stage && self.current_stage().value.is_some();
            if report_ready {
                // Set when a link names a sub-view this model has no tab for. Collected
                // here and posted after the borrows end, as `FrameIntent` does.
                let mut bad_sub_view: Option<String> = None;
                // Invalidate caches when switching between Structural
                // and IndexReduction — each has different report data.
                if self.cached_report_stage != Some(self.stage) {
                    self.cached_spy_plot = None;
                    self.cached_incidence = None;
                    self.cached_reduction = None;
                    self.cached_matching_anim = None;
                    self.cached_tarjan_anim = None;
                    self.cached_tearing_anim = None;
                    self.cached_alias_anim = None;
                    self.cached_ic_plan_anim = None;
                    self.cached_connection_anim = None;
                    self.cached_reduction_anim = None;
                    self.cached_before_incidence = None;
                    // Default sub-view: Summary for IndexReduction and
                    // singular Structural; SpyPlot otherwise.
                    let is_singular = self.stages.get(self.stage).note.as_deref()
                        .is_some_and(|n| n.contains("singular"));
                    if self.stage == StageKind::IndexReduction || is_singular {
                        self.structural_view = StructuralView::Summary;
                    } else if matches!(self.structural_view,
                        StructuralView::Summary | StructuralView::Animate)
                    {
                        self.structural_view = StructuralView::SpyPlot;
                    }
                    self.cached_report_stage = Some(self.stage);
                }
                // A sub-view requested by an hrw:// link is applied *here*, after
                // the default-sub-view logic above, precisely because that logic
                // would otherwise overwrite it: it forces Summary whenever a
                // report stage is entered singular. A link saying "show me the
                // matching animation" has to win over the default saying "show
                // the summary first".
                if let Some(sub) = self.pending_sub_view.take() {
                    // Refuse a sub-view this model does not have a tab for, rather than
                    // selecting it and rendering something misleading — the same rule as
                    // aiming at an equation that is not there. The link named a real
                    // slug; whether it is *available* depends on what the compile
                    // produced, which only this point knows.
                    let available = match sub {
                        SubView::Structural(v) => self.structural_view_available(v),
                        _ => true,
                    };
                    if available {
                        self.apply_sub_view(Some(sub));
                    } else {
                        let msg = format!(
                            "{} has no {} view for this model \u{2014} the link names one \
                             that is not here",
                            self.stage.name(),
                            sub.slug(),
                        );
                        bad_sub_view = Some(msg);
                    }
                }
                // Only now is the sub-view settled, so only now does looking up "the
                // on-screen animation" mean the one the link named. Applying this
                // before the block above would seek whichever animation happened to be
                // showing beforehand.
                self.apply_pending_seek();
                if let Some(msg) = bad_sub_view.take() {
                    self.notify(msg);
                }
                let is_index_reduction = self.stage == StageKind::IndexReduction;
                let note = self.stages.get(self.stage).note.as_deref().unwrap_or("");
                let is_singular = note.contains("singular");

                // Status banner
                if is_index_reduction {
                    if is_singular {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Singular").color(crate::colors::ANIM_FAIL).strong());
                            ui.weak("\u{2014} raw DAE was structurally singular; index reduction performed");
                        });
                    } else {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Index-1").color(crate::colors::ANIM_PATH_FOUND).strong());
                            ui.weak("\u{2014} already non-singular; reduction funnel is a no-op");
                        });
                    }
                    ui.add_space(2.0);
                } else if is_singular {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Singular").color(crate::colors::ANIM_FAIL).strong());
                        ui.weak("\u{2014} structurally singular; no perfect matching exists (see Index Reduction)");
                    });
                    ui.add_space(2.0);
                }

                // Sub-tab bar
                ui.horizontal(|ui| {
                    // Availability comes from `structural_view_available`, the same
                    // predicate the link guard uses — a tab that exists and a link that
                    // is honoured must not be able to disagree.
                    if self.structural_view_available(StructuralView::Summary) {
                        ui.selectable_value(&mut self.structural_view, StructuralView::Summary, "Summary");
                        ui.separator();
                    }
                    if self.structural_view_available(StructuralView::Animate) {
                        ui.selectable_value(&mut self.structural_view, StructuralView::Animate, "Reduction \u{25b6}");
                    }
                    // Alias elimination is reported by this stage only, and
                    // only when something was actually eliminated -- a model
                    // with no aliases must not show an empty tab.
                    if self.structural_view_available(StructuralView::AliasAnim) {
                        ui.selectable_value(&mut self.structural_view, StructuralView::AliasAnim, "Aliases \u{25b6}")
                            .on_hover_text(
                                "Watch variables be substituted away. Every connection \
                                 equation `a = b` lets one of the two be deleted, which is \
                                 why the solved system is far smaller than the equation \
                                 count suggests.",
                            );
                    }
                    // Spy-plot, Matching, BLT require a full matching —
                    // hide them when the Structural stage is singular.
                    if self.structural_view_available(StructuralView::SpyPlot) {
                        ui.selectable_value(&mut self.structural_view, StructuralView::SpyPlot, "Spy-plot");
                    }
                    ui.selectable_value(&mut self.structural_view, StructuralView::Incidence, "Incidence");
                    // Matching is shown *even when singular* — that is the whole
                    // point of it. The other three below need a complete matching
                    // before they mean anything; this one is a replay of the
                    // *search*, and the search failing is the most instructive
                    // thing on a singular stage. It was hidden here until
                    // 2026-07-29, when writing a tour to answer "what does a rank
                    // deficiency of 1 mean?" ran straight into its absence
                    // (ideas #44). Nothing else was needed: the trace already
                    // emits `MatchingStep::EquationFailed` and the view already
                    // paints the failed row red. The feature was built, then
                    // gated out of reach.
                    ui.selectable_value(&mut self.structural_view, StructuralView::MatchingAnim, "Matching \u{25b6}")
                        .on_hover_text(if is_singular && !is_index_reduction {
                            "Watch the augmenting-path search run out. The equation it \
                             gives up on is the rank deficiency."
                        } else {
                            "Replay the augmenting-path search that pairs each equation \
                             with one unknown."
                        });
                    if !is_singular || is_index_reduction {
                        ui.selectable_value(&mut self.structural_view, StructuralView::TarjanAnim, "BLT \u{25b6}");
                        // Tearing operates on the coupled blocks BLT finds,
                        // so it needs the same full matching those two do.
                        ui.selectable_value(&mut self.structural_view, StructuralView::TearingAnim, "Tearing \u{25b6}");
                    }
                    ui.selectable_value(&mut self.structural_view, StructuralView::Tree, "Tree");
                });
                ui.separator();
            }

            // Whether the Index Reduction tab shows a Before/After split for
            // comparative views. True when index reduction was actually needed
            // (the note mentions "singular").
            let ir_split = report_ready
                && self.stage == StageKind::IndexReduction
                && self.stages.get(self.stage).note.as_deref()
                    .is_some_and(|n| n.contains("singular"));

            if report_ready && self.structural_view == StructuralView::SpyPlot {
                if ir_split {
                    // No spy-plot for the Before pane (needs full matching),
                    // show only the After pane.
                    ui.label(egui::RichText::new("Before (raw DAE)")
                        .strong().color(crate::colors::ANIM_FAIL));
                    ui.weak("Spy-plot unavailable (structurally singular \u{2014} no BLT decomposition)");
                    ui.add_space(12.0);
                    ui.label(egui::RichText::new("After (reduced)")
                        .strong().color(crate::colors::ANIM_PATH_FOUND));
                }
                let cached = self.cached_spy_plot.get_or_insert_with(|| {
                    self.stages.get(self.stage).value.as_ref().and_then(spyplot::Plot::from_report)
                });
                if let Some(plot) = cached {
                    ui.weak(plot.caption());
                    plot.ui(ui, &mut self.spy_canvas, &mut intent.canvas_capture, self.tracked_identifier.as_deref());
                } else {
                    ui.weak("(the structural report has no BLT blocks to plot)");
                }
            } else if report_ready && self.structural_view == StructuralView::Incidence {
                if ir_split {
                    // Before/After split for incidence matrices.
                    let before_cached = self.cached_before_incidence.get_or_insert_with(|| {
                        self.stages.get(self.stage).value.as_ref()
                            .and_then(|v| v.get("before"))
                            .and_then(incidence_view::IncidenceMatrix::from_report)
                    });
                    let after_cached = self.cached_incidence.get_or_insert_with(|| {
                        self.stages.get(self.stage).value.as_ref()
                            .and_then(incidence_view::IncidenceMatrix::from_report)
                    });
                    ui.columns(2, |cols| {
                        // Before pane
                        cols[0].label(egui::RichText::new("Before (raw DAE)")
                            .strong().color(crate::colors::ANIM_FAIL));
                        if let Some(mat) = before_cached {
                            cols[0].weak(mat.caption());
                            mat.ui(
                                &mut cols[0], &mut self.before_incidence_canvas,
                                &mut intent.canvas_capture, self.highlighted_eq_row, None,
                            );
                        } else {
                            cols[0].weak("(no before incidence data)");
                        }
                        // After pane
                        cols[1].label(egui::RichText::new("After (reduced)")
                            .strong().color(crate::colors::ANIM_PATH_FOUND));
                        if let Some(mat) = after_cached {
                            cols[1].weak(mat.caption());
                            let tracked_col = self.tracked_identifier.as_deref()
                                .and_then(|name| mat.column_index(name));
                            mat.ui(
                                &mut cols[1], &mut self.incidence_canvas,
                                &mut intent.canvas_capture, self.highlighted_eq_row, tracked_col,
                            );
                        } else {
                            cols[1].weak("(no after incidence data)");
                        }
                    });
                } else {
                    let cached = self.cached_incidence.get_or_insert_with(|| {
                        self.stages.get(self.stage).value.as_ref()
                            .and_then(incidence_view::IncidenceMatrix::from_report)
                    });
                    if let Some(mat) = cached {
                        ui.weak(mat.caption());
                        let tracked_col = self.tracked_identifier.as_deref()
                            .and_then(|name| mat.column_index(name));
                        mat.ui(ui, &mut self.incidence_canvas, &mut intent.canvas_capture, self.highlighted_eq_row, tracked_col);
                    } else {
                        ui.weak("(no incidence data in this report)");
                    }
                }
            } else if report_ready && self.structural_view == StructuralView::MatchingAnim {
                self.matching_anim_ui(ui, ir_split);
            } else if report_ready && self.structural_view == StructuralView::TarjanAnim {
                self.tarjan_anim_ui(ui, ir_split);
            } else if report_ready && self.structural_view == StructuralView::TearingAnim {
                self.tearing_anim_ui(ui);
            } else if report_ready && self.structural_view == StructuralView::AliasAnim {
                self.alias_anim_ui(ui);
            } else if report_ready && self.structural_view == StructuralView::Summary {
                if self.stage == StageKind::Structural {
                    Self::structural_singular_summary(ui, &self.stages.structural);
                } else {
                    let cached = self.cached_reduction.get_or_insert_with(|| {
                        self.stages.get(self.stage).value.as_ref().and_then(reduction_view::ReductionView::from_report)
                    });
                    if let Some(view) = cached {
                        view.ui(ui, self.tracked_identifier.as_deref());
                    } else {
                        ui.weak("(no reduction data in this report)");
                    }
                }
            } else if self.structural_view == StructuralView::Animate {
                self.reduction_anim_ui(ui);
            } else if events_ready && self.events_view == EventsView::PreLowering {
                self.pre_lowering_anim_ui(ui);
            } else if init_ready && self.init_view == InitView::IcPlan {
                self.ic_plan_anim_ui(ui);
            } else if flatten_ready && self.flatten_view == FlattenView::Equations {
                self.equation_sheet_ui(ui);
            } else if flatten_ready && self.flatten_view == FlattenView::SourceMap {
                self.source_map_ui(ui);
            } else if flatten_ready && self.flatten_view == FlattenView::Connections {
                self.connection_anim_ui(ui);
            } else {
                // Set when a `hrw://…/node/<path>` link names a path this stage does not
                // have. Collected while `stage` is borrowed and acted on after, the same
                // pattern as `FrameIntent`.
                let mut bad_jump: Option<String> = None;
                let stage = self.current_stage();
                let has_error_data = stage.note_is_error()
                    && stage.value.as_ref().and_then(|v| v.get("error")).is_some();
                if has_error_data {
                    let error = stage.value.as_ref().unwrap().get("error").unwrap().clone();
                    egui::ScrollArea::vertical().id_salt("error_summary").auto_shrink(false).show(ui, |ui| {
                        Self::generic_error_summary(ui, &error, self.stage);
                    });
                } else {
                    match &stage.value {
                        Some(value) => {
                            let label = self.model.as_deref().unwrap_or("model");
                            let prev = self.previous_stage_value();
                            // Finding trackable names in a large IR means
                            // opening nodes at random. This opens exactly
                            // the paths that lead to one.
                            ui.horizontal(|ui| {
                                ui.checkbox(
                                    &mut intent.expand_trackable,
                                    "Reveal identifiers",
                                ).on_hover_text(
                                    "Expand every path leading to a variable of \
                                     this model. Underlined values can be \
                                     right-clicked to track.",
                                );
                                if let Some(n) = self.known_variables.as_ref().map(HashSet::len) {
                                    ui.weak(format!("({n} in this model)"));
                                }
                            });
                            // A node link that does not resolve must SAY so. The tree
                            // otherwise expands as far as it can and stops, which looks
                            // like "it opened something" rather than "that path is
                            // wrong" — the silent partial failure the aim and seek verbs
                            // deliberately avoid.
                            let jump_to = match &self.jump_target {
                                Some(t) => match resolve_jump_target(value, t) {
                                    Ok(()) => Some(t.clone()),
                                    Err(msg) => {
                                        bad_jump = Some(msg);
                                        None
                                    }
                                },
                                None => None,
                            };
                            let opts = tree::TreeOptions {
                                tracked: self.tracked_identifier.as_deref(),
                                known_variables: self.known_variables.as_ref(),
                                declaring_classes: Some(&self.declaring_classes),
                                expand_trackable: intent.expand_trackable,
                                jump_to: jump_to.as_deref(),
                                highlight: self.jump_highlight.as_deref(),
                            };
                            egui::ScrollArea::both().id_salt("tree").auto_shrink(false).show(ui, |ui| {
                                tree::tree_ui(ui, label, value, prev, &mut intent.tree, &self.def_index, &self.field_help, opts);
                            });
                        }
                        None if stage.note.is_none() => {
                            ui.weak(if self.compiling { "compiling…" } else { "(no output for this stage)" });
                        }
                        None => {}
                    }
                }
                // The stage borrow ends here, so the notices can finally be posted.
                if let Some(msg) = bad_jump {
                    self.jump_target = None;
                    self.notify(msg);
                }
            }
            } // end: non-Simulation stage rendering
        } else {
            // ---- Navigation view (a class reached via "Go to definition") ----
            ui.horizontal(|ui| {
                if ui.button("Specimen").on_hover_text("Return to the specimen stages (top of navigation)").clicked() {
                    intent.go_home = true;
                }
                if ui.button("← Back").clicked() {
                    intent.go_back = true;
                }
                ui.separator();
                let mut crumb = self.model.clone().unwrap_or_else(|| "model".to_owned());
                for e in &self.nav {
                    crumb.push_str("  ▸  ");
                    crumb.push_str(&e.name);
                }
                ui.label(egui::RichText::new(crumb).monospace().strong());
                if let Some(n) = &self.nav_loading {
                    ui.weak(format!("opening {n}…"));
                    ui.spinner();
                }
            });
            if let Some(err) = &self.nav_error {
                ui.colored_label(ui.visuals().error_fg_color, err);
            }
            ui.separator();

            let entry = self.nav.last().unwrap();
            egui::ScrollArea::both().id_salt("nav_tree").auto_shrink(false).show(ui, |ui| {
                tree::tree_ui(ui, &entry.name, &entry.value, None, &mut intent.tree, &entry.def_index, &self.field_help,
                    tree::TreeOptions {
                        tracked: self.tracked_identifier.as_deref(),
                        known_variables: self.known_variables.as_ref(),
                        declaring_classes: Some(&self.declaring_classes),
                        expand_trackable: intent.expand_trackable,
                        // A navigated library class is a different IR, so a
                        // jump target addressed into the stage tree would
                        // land on an unrelated node or nothing at all. The
                        // highlight is suppressed for the same reason.
                        jump_to: None,
                        highlight: None,
                    });
            });
        }
    }

    /// Work out which source lines the compiler blamed, from the stage errors.
    ///
    /// **Only fills in when the model is genuinely ill-posed**, meaning the Structural
    /// stage failed *and* index reduction could not rescue it. That condition is the
    /// whole point rather than caution: `MotorWithBrake` is structurally singular too,
    /// with unmatched unknowns and source lines to match, and it is a perfectly good
    /// model — high-index, not broken. Index reduction demotes a state and it solves.
    /// Painting its `connect()` line as a problem would teach something false.
    ///
    /// `CapacitorLoop` is the other case: states 1 → 1, nothing demoted, still
    /// singular. Nothing downstream can save it, so the blame is real.
    ///
    /// Reads the **index reduction** payload rather than the structural one, because
    /// that is the stage whose failure means "unrescuable". Both name the same unknown
    /// here, but the reduced system is the one actually stuck.
    fn compute_problem_lines(&mut self) {
        self.problem_lines.clear();
        let structural_failed =
            self.stages.structural.value.as_ref().is_some_and(|v| v.get("error").is_some());
        if !structural_failed {
            return;
        }
        // Index reduction succeeding means high-index at worst, never ill-posed.
        let Some(err) =
            self.stages.index_reduction.value.as_ref().and_then(|v| v.get("error"))
        else {
            return;
        };
        let Some(locs) = err.get("unmatched_unknown_locations").and_then(|v| v.as_array())
        else {
            return;
        };
        for entry in locs {
            // No source provenance means a manufactured or solver-vector variable.
            // There is no line to blame, and inventing one would be worse than silence.
            let Some(line) = entry
                .get("location")
                .and_then(|l| l.get("line"))
                .and_then(serde_json::Value::as_u64)
            else {
                continue;
            };
            let unknown =
                entry.get("unknown").and_then(serde_json::Value::as_str).unwrap_or("?");
            self.problem_lines.push((
                line as u32,
                format!(
                    "No equation determines `{unknown}`, and index reduction could not fix \
                     it \u{2014} this model is ill-posed, not merely high-index. See the \
                     Structural and Index Reduction tabs."
                ),
            ));
        }
    }

    /// Point the current stage's sub-view selector at `sub`, if one was requested.
    ///
    /// Each stage keeps its own selector enum, so this is a small dispatch rather
    /// than one assignment. A `SubView` can only have been built by
    /// `SubView::from_slug` against a stage, so a mismatch here would mean the link
    /// named one stage and the app is on another — possible if the user clicked away
    /// mid-compile. Ignoring it is right: better to leave the view where the user put
    /// it than to yank it somewhere the link no longer describes.
    fn apply_sub_view(&mut self, sub: Option<SubView>) {
        match (sub, self.stage) {
            (Some(SubView::Structural(v)), StageKind::Structural | StageKind::IndexReduction) => {
                self.structural_view = v;
            }
            (Some(SubView::Flatten(v)), StageKind::Flatten) => self.flatten_view = v,
            (Some(SubView::Events(v)), StageKind::Events) => self.events_view = v,
            (Some(SubView::Init(v)), StageKind::Initialization) => self.init_view = v,
            _ => {}
        }
    }

    /// Re-read `.hrw-bridge/tour.md` if it changed since the last read.
    ///
    /// Polled rather than watched: a `stat` every [`TOUR_POLL_INTERVAL`] is
    /// simpler than a filesystem watcher, has no platform quirks, and a tour
    /// appearing a quarter-second late is imperceptible. Re-reads only when the
    /// mtime differs, so an unchanged tour costs one `stat` per poll and no
    /// markdown re-parse.
    fn poll_tour_file(&mut self) {
        let due = self
            .tour_polled_at
            .is_none_or(|last| last.elapsed() >= TOUR_POLL_INTERVAL);
        if !due {
            return;
        }
        self.tour_polled_at = Some(std::time::Instant::now());

        // --- Rebuild the pick list ---
        //
        // Ad hoc first when it exists: it is the answer to the question just asked,
        // and burying it under the fixtures would make the common case the awkward one.
        let mut tours = Vec::new();
        if std::path::Path::new(bridge::TOUR_FILE).exists() {
            tours.push(TourSource::AdHoc);
        }
        tours.extend(bridge::fixture_tours().into_iter().map(TourSource::Fixture));
        let list_changed = tours != self.tours;
        self.tours = tours;

        // A selection that no longer exists (the ad hoc tour was deleted, a fixture
        // renamed) must not leave stale text on screen attributed to a live file.
        if self.selected_tour.as_ref().is_some_and(|t| !self.tours.contains(t)) {
            self.selected_tour = None;
            self.cached_tour = None;
        }
        // Default to the ad hoc tour when one appears and nothing is chosen.
        if self.selected_tour.is_none() && self.tours.contains(&TourSource::AdHoc) {
            self.select_tour(TourSource::AdHoc);
        }

        // --- Re-read the selected tour if it changed on disk ---
        let Some(selected) = self.selected_tour.clone() else {
            self.cached_tour = None;
            return;
        };
        let path = selected.path();
        let mtime = std::fs::metadata(&path).ok().and_then(|m| m.modified().ok());
        match mtime {
            Some(mtime) => {
                let unchanged =
                    self.cached_tour.as_ref().is_some_and(|(_, seen)| *seen == mtime);
                if !unchanged || list_changed {
                    self.cached_tour =
                        std::fs::read_to_string(&path).ok().map(|text| (text, mtime));
                }
            }
            // The file vanished between listing and reading. Drop the text rather
            // than keep a stale copy: for an ad hoc tour, absence is the normal state.
            None => {
                self.cached_tour = None;
                self.selected_tour = None;
            }
        }
    }

    /// Switch the Tour panel to `source`, discarding the previous text.
    ///
    /// Clears `cached_tour` rather than letting the poll notice: without this the old
    /// tour stays on screen until the next mtime comparison, and a reader who just
    /// clicked a different tour would see the previous one for up to a poll interval.
    fn select_tour(&mut self, source: TourSource) {
        // **Switching tours re-initialises the right-hand side.** A tour is a
        // self-contained sequence starting from its own first stop, which normally
        // loads a specimen. Leaving the previous tour's model on screen invites reading
        // the new tour's stops against the old tour's state — and worse, makes Stop 1
        // look as though it has already been done.
        //
        // Only on an actual change: re-clicking the tour already showing should not
        // throw away a specimen the reader is partway through.
        if self.selected_tour.as_ref() != Some(&source) {
            self.clear_specimen_state(false);
            self.selected = None;
            self.stage = StageKind::Parse;
            self.viewing_log = false;
            self.compiling = false;
        }
        self.selected_tour = Some(source);
        self.cached_tour = None;
    }

    /// Re-scan when the set of scratch specimens changes.
    ///
    /// Cheaper than it looks: a `read_dir` of a directory that is usually empty, at
    /// most once per [`SCRATCH_POLL_INTERVAL`]. A full `rescan()` only runs when the
    /// *set of paths* actually differs, so the per-file `// purpose:` reads are not
    /// repeated for an unchanged directory.
    fn poll_scratch_specimens(&mut self) {
        let due = self
            .scratch_polled_at
            .is_none_or(|last| last.elapsed() >= SCRATCH_POLL_INTERVAL);
        if !due {
            return;
        }
        self.scratch_polled_at = Some(std::time::Instant::now());

        let found: HashSet<PathBuf> = bridge::scratch_specimens().into_iter().collect();
        // Compare against what was *accepted* plus what was shadowed, so a scratch
        // file appearing under a curated name still triggers a rescan and gets
        // reported rather than being invisible until the next restart.
        let known = self.scratch_specimens.len() + self.shadowed_specimens.len();
        if found.len() != known || !found.iter().all(|p| self.scratch_specimens.contains(p)) {
            // Only an *arrival* opens the section. A scratch specimen being
            // deleted is not a reason to show the reader anything.
            if found.len() > known {
                self.scratch_arrived = true;
            }
            self.rescan();
        }
    }

    /// What tour mode shows when Claude has not written a tour.
    ///
    /// Deliberately **not** `end_to_end_tour.md`, which used to be compiled in
    /// here with `include_str!`. That document's prose was retired 2026-07-29
    /// (ideas #42) — it described a 7x7 incidence matrix on a tab that shows 48
    /// equations — so keeping it as the default would put the exact stale
    /// content this change exists to remove back on screen.
    fn no_tour_ui(ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("No tour right now.").strong());
        ui.add_space(6.0);
        ui.label(
            "Tour mode shows a tour Claude wrote for a question you asked \u{2014} a \
             sequence of places to look, with links that drive HRW to each one.",
        );
        ui.add_space(6.0);
        ui.weak(
            "Ask Claude for one. Answers come as text first; a tour is for the ones \
             where a sequence of places beats a paragraph.",
        );
        ui.add_space(10.0);
        ui.weak("Fixture tours \u{2014} tests with expected outcomes \u{2014} can be picked above \
                 when any exist.");
        ui.weak(format!("Claude writes an ad hoc tour to {}", bridge::TOUR_FILE));
        ui.weak("It appears here within a moment, and a rewrite is picked up live.");
    }

    /// The top menu bar (File, View, Help).
    ///
    /// Extracted from `ui` during the 2026-07-28 sweep — self-contained, and
    /// `ui` had grown past the point where a reader could see its shape.
    fn menu_bar_ui(&mut self, ui: &mut egui::Ui) {
egui::Panel::top("bar").show(ui, |ui| {
    // `MenuBar` creates a horizontal bar with dropdown menus.
    egui::MenuBar::new().ui(ui, |ui| {
        ui.menu_button("File", |ui| {
            if ui.button("Rescan specimens").clicked() {
                self.rescan();
                ui.close();
            }
            ui.separator();
            if ui.button("Settings…").clicked() {
                self.show_settings = true;
                ui.close();
            }
            ui.separator();
            if ui.button("Quit").clicked() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });
        ui.menu_button("View", |ui| {
            if ui.selectable_label(self.ui_mode == UiMode::Tour, "Tour").clicked() {
                self.ui_mode = UiMode::Tour;
                ui.close();
            }
            if ui.selectable_label(self.ui_mode == UiMode::Specimen, "Specimen").clicked() {
                self.ui_mode = UiMode::Specimen;
                ui.close();
            }
            if ui.selectable_label(self.ui_mode == UiMode::Debug, "Debug").clicked() {
                self.ui_mode = UiMode::Debug;
                ui.close();
            }
        });
        ui.menu_button("Help", |ui| {
            if ui.button("Using HRW…").clicked() {
                self.show_help = true;
                ui.close();
            }
            if ui.button("About HRW…").clicked() {
                self.show_about = true;
                ui.close();
            }
            ui.separator();
            // For problems that do NOT kill the app. A crash writes its own
            // file; a wrong-looking view or a hang writes nothing, and the
            // evidence needed to diagnose it is identical. The path goes into
            // the status bar because a file nobody can find is a file nobody
            // sends.
            if ui
                .button("Write diagnostic snapshot")
                .on_hover_text(
                    "Write the current app state, recent actions, and log tail to \
                     .hrw-bridge/diagnostics/ for Claude to read. Use when something \
                     looks wrong but HRW has not crashed — crashes write their own file.",
                )
                .clicked()
            {
                let msg = match diagnostics::write_on_demand() {
                    Ok(path) => format!("diagnostic written: {}", path.display()),
                    Err(e) => format!("diagnostic FAILED: {e}"),
                };
                self.notify(msg);
                ui.close();
            }
        });
    });
});
    }

    /// The specimen's Modelica source: syntax-highlighted, with clickable
    /// identifiers, and scrolled to whatever is being followed.
    ///
    /// Extracted from `ui` during the 2026-07-28 sweep. It is the one place
    /// three separate mechanisms have to agree about a line — the lexer's
    /// tokens, `IdentifierIndex`'s clickable spans, and `source_view::segments`
    /// merging them — and that is much easier to keep straight in its own
    /// function than buried a thousand lines into a panel closure.
    fn specimen_source_ui(&mut self, ui: &mut egui::Ui) {
        // **Which file this is.** A library model's source is a whole package
        // file declaring dozens of classes, so a reader who asked for `Resistor`
        // and is looking at `Basic.mo` needs to be told. A specimen needs no such
        // line: its file is the thing selected, and the name is already on screen.
        if let Some(uri) = self.library_source_uri.clone() {
            let file = std::path::Path::new(&uri)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(uri.as_str())
                .to_owned();
            let model = self.model.clone().unwrap_or_default();
            ui.horizontal(|ui| {
                ui.weak(file.clone())
                    .on_hover_text(format!("{uri}\n\nthe library file declaring {model}"));
                ui.weak("\u{00b7}");
                ui.weak(format!("declares {model} among others"));
            });
            ui.separator();
        }
        // **A read failure is reported, never rendered as blank.** Blank would be
        // indistinguishable from the refusal this replaced, and from an empty file.
        if let Some(why) = self.library_source_error.clone() {
            ui.label(
                egui::RichText::new(format!("cannot show this file\n\n{why}"))
                    .color(crate::colors::ANIM_FAIL),
            );
            return;
        }
        // A library model's text is already in the cache, seeded by the worker
        // from the document it compiled — `get_or_insert_with` therefore does
        // not run, and `selected` (a qualified name, not a path) is never read as
        // one. A specimen still reads from its own file so live edits show.
        //
        // **This pane used to refuse library models outright**, claiming there was
        // "no single source file to show". That was untrue: the worker reads that
        // very file out of the session in order to compile it. Doug, 2026-08-01:
        // *"The modelica source view for an MSL model should be just as functional
        // as for an HRW specimen."*
        let source = self.selected.as_ref().map(|path| {
            self.cached_source.get_or_insert_with(|| {
                std::fs::read_to_string(path).unwrap_or_default()
            }).as_str()
        });
        let mut clicked_id: Option<String> = None;
        match source {
            Some(text) if !text.is_empty() => {
                let scroll_out = egui::ScrollArea::both()
                    .id_salt("specimen_source")
                    .auto_shrink(false)
                    .show(ui, |ui| {
                    let tracked = self.tracked_identifier.as_deref();
                    let dark = ui.visuals().dark_mode;
                    // Reverse tracking: when the tracked
                    // identifier changes — typically from a click
                    // in a downstream view — bring its
                    // declaration into view. Gated on *change*,
                    // not on the value: scrolling every frame
                    // while an identifier stays tracked would peg
                    // the view and fight the scrollbar.
                    let scroll_to = (self.tracked_identifier
                        != self.scrolled_source_for)
                        .then(|| {
                            self.tracked_identifier.as_deref().and_then(|name| {
                                self.identifier_index.as_ref()
                                    .and_then(|idx| idx.variables.get(name))
                                    .map(|v| v.source_line)
                            })
                        })
                        .flatten();
                    if scroll_to.is_some() || self.tracked_identifier.is_none() {
                        self.scrolled_source_for = self.tracked_identifier.clone();
                    }
                    // A link-driven scroll, taken once so it cannot re-scroll every
                    // frame and pin the view — the same discipline as `jump_target`.
                    let source_scroll_to = self.source_scroll_target.take();
                    // Tokenized once per specimen, not per frame.
                    let highlight = self.cached_highlight.get_or_insert_with(
                        || crate::source_view::SourceHighlight::new(text)
                    );
                    for (i, line) in text.lines().enumerate() {
                        let line_1 = (i + 1) as u32;
                        // Why this line was blamed, if it was. `problem_lines` is only
                        // non-empty for a model index reduction could not rescue, so a
                        // high-index model like MotorWithBrake is never marked.
                        let blamed = self
                            .problem_lines
                            .iter()
                            .find(|(l, _)| *l == line_1)
                            .map(|(_, why)| why.as_str());
                        let line_tokens = highlight.line(i);
                        let spans = self.identifier_index.as_ref()
                            .map(|idx| idx.clickable_spans(line_1, line, line_tokens))
                            .unwrap_or_default();
                        // One pass produces both colour and click
                        // targets, so the two cannot disagree about
                        // where a run of text begins and ends.
                        let segments = crate::source_view::segments(
                            line, line_tokens, &spans,
                        );
                        let row = ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 0.0;
                            // A blamed line's number is coloured rather than
                            // gutter-marked: a marker glyph would widen this column and
                            // shift every line, and a layout regression is precisely the
                            // class of defect Claude cannot see.
                            let mut num = egui::RichText::new(format!("{:>4} ", line_1))
                                .monospace();
                            num = if blamed.is_some() {
                                num.color(crate::colors::ANIM_FAIL).strong()
                            } else {
                                num.weak()
                            };
                            ui.label(num);
                            for seg in &segments {
                                match seg.link {
                                    Some(name) => {
                                        // Clickable identifiers keep their own
                                        // colours, which outrank syntax colour —
                                        // interactivity has to win visually.
                                        let color = if tracked == Some(name) {
                                            crate::colors::TRACKED_GOLD
                                        } else {
                                            crate::colors::CLICKABLE_IDENT
                                        };
                                        let label = egui::Label::new(
                                            egui::RichText::new(seg.text)
                                                .monospace()
                                                .color(color)
                                                .underline()
                                        ).sense(egui::Sense::click());
                                        // Say which verb the click carries,
                                        // before it fires. This used to hover
                                        // with the bare name, confirming what
                                        // was under the cursor and nothing
                                        // about what clicking would do.
                                        let hover =
                                            crate::follow_hover(name, tracked == Some(name));
                                        if ui.add(label).on_hover_text(hover).clicked() {
                                            clicked_id = Some(name.to_owned());
                                        }
                                    }
                                    None => {
                                        let mut rt = egui::RichText::new(seg.text)
                                            .monospace();
                                        if let Some(c) = crate::colors::syntax_color(
                                            seg.kind, dark,
                                        ) {
                                            rt = rt.color(c);
                                        }
                                        ui.label(rt);
                                    }
                                }
                            }
                        });
                        if let Some(why) = blamed {
                            // Painted *over* the row at low alpha rather than behind it.
                            // A `Frame` fill would be cleaner-looking but adds margins,
                            // and any layout shift here is a rendered defect Claude has
                            // no way to notice. An overpaint cannot move anything.
                            ui.painter().rect_filled(
                                row.response.rect,
                                egui::CornerRadius::ZERO,
                                crate::colors::ANIM_FAIL.gamma_multiply(0.18),
                            );
                            row.response.clone().on_hover_text(why);
                        }
                        if scroll_to == Some(line_1) || source_scroll_to == Some(line_1) {
                            // **Scroll to the line's START, not to the line.**
                            //
                            // This is a `ScrollArea::both`, and `scroll_to_me`
                            // aligns on *both* axes. Centring a row horizontally
                            // centres a line that may be 200 characters wide, so
                            // its opening characters — the indentation, the
                            // keyword, the declared name — end up off the left
                            // edge. Doug, 2026-08-01: *"The text in the modelica
                            // source view is positioned too far to the left. The
                            // left-most characters in many source lines are being
                            // cut off."*
                            //
                            // Latent before MSL models: a specimen's lines are
                            // short and shallowly indented, so centring one moved
                            // the view barely at all. A library file is nested
                            // several packages deep with long signatures, and the
                            // scroll now fires on every library load to reach the
                            // declaration line.
                            //
                            // Collapsing the target to a sliver at the row's left
                            // edge fixes both axes at once: vertically it still
                            // centres the line, and horizontally the offset that
                            // would centre a sliver at the content's left margin
                            // is negative, so egui clamps it to 0 — the start of
                            // the line, which is where reading begins.
                            let mut target = row.response.rect;
                            target.max.x = target.min.x + 1.0;
                            ui.scroll_to_rect(target, Some(egui::Align::Center));
                        }
                    }
                });
                // **Observation hook, not decoration.** `ScrollArea` keeps its
                // offset in `Memory` under an id derived from the parent `Ui`,
                // which a test cannot reconstruct; the returned state is the only
                // honest way to read it. One `Vec2` per frame buys a headless
                // guard on the one layout property that has bitten -- the
                // horizontal offset drifting off the left margin.
                self.source_scroll_offset = scroll_out.state.offset;
            }
            _ => {
                ui.weak("Select a specimen to view its source.");
            }
        }
        if let Some(name) = clicked_id {
            // Shared with every other entry point, so clicking a name here
            // toggles exactly as following from a tree or the equation sheet
            // does. This used to be a private copy of the same logic.
            self.set_tracked_identifier(name);
        }
    }

    /// Re-emit the focus file from whatever context is currently assembled.
    ///
    /// The single write path, so the file always reflects **both** halves at
    /// once: the point retained in `pointed_at` and the thread in
    /// `tracked_identifier`. Called when either changes.
    ///
    /// This is what stops ambient following from destroying deliberate
    /// pointing. Following used to emit nothing at all; the naive fix — writing
    /// a fresh focus on every track change — would have overwritten the node
    /// the user meant to ask about. Retaining the point and re-emitting both
    /// keeps them independent, which is the property `docs/context-assembly.md`
    /// asks for.
    fn emit_context(&mut self) {
        let stage_values = self.stages.as_stage_pairs();
        // Summarised now (on a click), never per frame — it walks every stage.
        // Computed before the borrow below, since `tracking` borrows self.
        let summary = self
            .tracking_context(&stage_values)
            .as_ref()
            .map(bridge::summarize_tracking);
        self.tracking_summary = summary;

        let tracking = self.tracking_context(&stage_values);
        let Some(point) = self.pointed_at.clone() else {
            // Following with nothing pointed at is a normal state; emit the
            // thread on its own rather than withholding context.
            let ask = Ask {
                seq: self.context_seq,
                // NOT `Focus::Stage`. See `Focus::Nothing`.
                request: bridge::AskRequest::Explain,
                specimen: self.selected.as_deref(),
                model: self.model.as_deref(),
                stage: Some(self.stage),
                libraries: self.library_strings(),
                def_index: &self.def_index,
                parse_value: self.stages.parse.value.as_ref(),
                resolve_value: self.stages.resolve.value.as_ref(),
                focus: Focus::Nothing,
                tracking,
                view: self.view_context(),
                failure: self.failure_context(),
            };
            self.point_error = bridge::write(&ask).err().map(|e| e.to_string());
            return;
        };

        // Rebuild whichever shape was captured. Handling only `Node` here would
        // silently drop stage and specimen captures on the next follow-change,
        // reintroducing the disagreement between bar and file.
        let stage_value = self.stages.get(point.stage).value.clone();
        let focus = match (&point.kind, &stage_value) {
            (PointKind::Node(key_path), Some(value)) => {
                Focus::Node { key_path: key_path.clone(), stage_value: value }
            }
            // The stage's IR is not available, so the node cannot be described.
            // Skip re-emission rather than dropping the point: the file still
            // holds what Claude has, and the bar still describes that file, so
            // the two stay in agreement. Discarding the capture here would
            // destroy deliberate context over a transient condition — a
            // recompile clears the point through `reset`, which is the place
            // that knows the old IR is genuinely gone.
            (PointKind::Node(_), None) => return,
            (PointKind::Stage, _) => Focus::Stage,
            (PointKind::Specimen, _) => Focus::Specimen,
        };
        let ask = Ask {
            seq: point.seq,
            request: point.request,
            specimen: self.selected.as_deref(),
            model: self.model.as_deref(),
            // The stage the capture was MADE in, not the one now on screen.
            // A bar reading "Structural" for a point captured in Flatten would
            // be describing context Claude does not have.
            stage: Some(point.stage),
            libraries: self.library_strings(),
            def_index: &self.def_index,
            parse_value: self.stages.parse.value.as_ref(),
            resolve_value: self.stages.resolve.value.as_ref(),
            focus,
            tracking,
            view: self.view_context(),
            failure: self.failure_context(),
        };
        self.point_error = bridge::write(&ask).err().map(|e| e.to_string());
    }

    /// The Context Bar: what Claude can see right now.
    ///
    /// ## The rule this obeys
    ///
    /// **It renders what will be emitted — nothing more, nothing less.** If it
    /// showed context Claude does not receive, or omitted context Claude does,
    /// questions would be calibrated against a fiction. Built as a view of the
    /// payload, it cannot drift, because there is nothing to drift from.
    ///
    /// Hence three rows and no fourth: *pointing at* and *following* are the two
    /// shapes of assembled context, and *always* is the standing context —
    /// stage IRs, the DefId table, the libraries — that the old UI never
    /// mentioned at all, leaving the user to underestimate what a question had
    /// behind it.
    ///
    /// Controls here are only those that **change** what is emitted. Navigation
    /// is not context, so the declaring class is a link rather than a button.
    /// See `docs/context-assembly.md`.
    /// Drop a retained point if the recompiled IR no longer contains it.
    ///
    /// Runs once per compile, after the new stages land. Only `PointKind::Node`
    /// can dangle — a stage or specimen point names something that exists by
    /// construction.
    ///
    /// The **follow is deliberately not validated**: it is a name, not an
    /// address, and a name matching nothing is already reported honestly as
    /// `mentions: 0` in every stage. Dropping it would discard a deliberate
    /// choice in order to answer a question the emitted context answers better.
    ///
    /// Re-emits when anything survived, because the focus file still describes
    /// the *previous* compile's IR until it is rewritten — same node, different
    /// values — and a stale subtree is worse than an absent one.
    fn revalidate_point_against_new_ir(&mut self) {
        let dangling = match &self.pointed_at {
            Some(point) => match &point.kind {
                PointKind::Node(key_path) => !self
                    .stages
                    .get(point.stage)
                    .value
                    .as_ref()
                    .is_some_and(|value| bridge::node_exists(value, key_path)),
                PointKind::Stage | PointKind::Specimen => false,
            },
            None => false,
        };
        if dangling {
            let target = self.pointed_at.take().map(|p| p.target).unwrap_or_default();
            // Said out loud. Silently dropping it would leave the user believing
            // Claude still has a node that has vanished from the bar.
            self.notify(format!(
                "point dropped \u{2014} {target} no longer exists in the recompiled IR",
            ));
            diagnostics::record_action("point-dropped", target);
        }
        if self.pointed_at.is_some() || self.tracked_identifier.is_some() {
            self.emit_context();
        }
    }

    /// Rebuild the jump match list if the stage or the followed name changed.
    ///
    /// Cheap on the common path — a tuple comparison — because the walk itself
    /// is not: it visits every node of the stage IR and lexes every code-bearing
    /// string. Same discipline as `tracking_summary`, which is computed on a
    /// click and never per frame.
    fn refresh_jump_matches(&mut self) {
        let key = self
            .tracked_identifier
            .as_ref()
            .map(|name| (self.stage, name.clone()));
        if key == self.jump_matches_key {
            return;
        }
        self.jump_matches = match (&key, self.current_stage().value.as_ref()) {
            (Some((_, name)), Some(value)) => bridge::mention_paths(value, name),
            _ => Vec::new(),
        };
        // Restart the cycle: an index carried over from another stage would
        // point at a match that no longer exists, and "3 of 4" would be a lie
        // about a different list.
        self.jump_index = 0;
        self.jump_matches_key = key;
    }

    /// Advance to the next match and ask the tree to scroll to it.
    ///
    /// Wraps around rather than stopping at the end — with a handful of matches,
    /// a dead button at the last one is a worse surprise than returning to the
    /// first.
    fn jump_to_next_match(&mut self, forward: bool) {
        if self.jump_matches.is_empty() {
            return;
        }
        let n = self.jump_matches.len();
        self.jump_index = if forward {
            (self.jump_index + 1) % n
        } else {
            (self.jump_index + n - 1) % n
        };
        self.jump_target = Some(self.jump_matches[self.jump_index].clone());
        // The matches live in a stage's IR, so the tree has to be on screen for
        // the jump to land. Leaving the log showing would make the button look
        // broken.
        self.viewing_log = false;
        diagnostics::record_action(
            "jump",
            format!(
                "match {} of {n} for {} in {}",
                self.jump_index + 1,
                self.tracked_identifier.as_deref().unwrap_or("?"),
                self.stage.name(),
            ),
        );
    }

    /// How to leave the empty context, given what is **actually on screen**.
    ///
    /// The first version of this line was generic — "left-click a node to point
    /// at it, or right-click a variable name to follow it" — and Doug met it
    /// immediately after loading a specimen, where it was wrong twice over: the
    /// log view was showing, so there was no tree and no node to left-click;
    /// and the only clickable things in sight were source identifiers, which
    /// are **left**-click-to-follow, not right-click.
    ///
    /// A hint that names an unavailable gesture is worse than no hint. It is
    /// also the same defect as everything else this bar exists to prevent — a
    /// confident statement that does not match the state — so it is built from
    /// the state rather than written once.
    ///
    /// Each branch is a fact about what is rendered right now:
    /// - Source identifiers are underlined and clickable only in Specimen mode
    ///   showing Source, and only once a compile has produced the identifier
    ///   index (it is `None` while compilation is still running).
    /// - With the log showing there is no IR view at all, so the way forward is
    ///   a stage tab, not a node.
    fn empty_context_hint(&self) -> String {
        let mut ways: Vec<&str> = Vec::new();
        if self.ui_mode == UiMode::Specimen
            && self.specimen_detail == SpecimenDetail::Source
            && self.identifier_index.is_some()
        {
            ways.push("left-click an underlined identifier in the source to follow it");
        }
        ways.push(if self.viewing_log {
            "open a stage tab to inspect its IR"
        } else {
            "left-click a node to point at it"
        });
        // **Not "nothing assembled".** The background is always assembled and
        // always emitted, so that wording made the bar contradict `focus.json`
        // for every reader who had not yet clicked anything -- the majority
        // state. What is missing is a *selection*, which is what this now says.
        format!("\u{2014} {}", ways.join(", or "))
    }

    /// The **background**: specimen and stage, always context, always shown.
    ///
    /// `docs/context-assembly.md`: *"Specimen and stage are always context, so
    /// they are always shown."* One renderer for both branches of
    /// [`Self::context_bar_ui`], because the two drifted apart the moment there
    /// were two of them -- the empty-state branch returned before ever reaching
    /// the background, so the bar showed *no* context at all in the state a
    /// reader is in most of the time.
    ///
    /// Both halves went unrendered in different ways: the stage was **never**
    /// drawn (since `b2732393`, the commit that created the bar), and the
    /// specimen was drawn only once something was pointed at. Found 2026-08-01
    /// by Doug, who counted three kinds of context and saw two.
    fn background_ui(&self, ui: &mut egui::Ui) {
        match (&self.model, self.selected.is_some()) {
            (Some(model), _) => {
                ui.weak(format!("\u{00b7} {model} \u{00b7} {}", self.stage.name()));
            }
            // Mid-compile, or a compile that yielded no model name: still name
            // the stage rather than showing a bare "Context".
            (None, true) => {
                ui.weak(format!("\u{00b7} {}", self.stage.name()));
            }
            (None, false) => {}
        }
    }

    fn context_bar_ui(&mut self, ui: &mut egui::Ui) {
        let has_point = self.pointed_at.is_some();
        let has_thread = self.tracked_identifier.is_some();
        if !has_point && !has_thread {
            // Say the empty state rather than vanishing.
            //
            // Hiding made "nothing is assembled" indistinguishable from "the bar
            // is not rendering", which is absence by *implication* — the thing
            // this design eliminates everywhere else (`mentions: 0`,
            // `kind: "none"`, "not declared in this specimen"). It is also the
            // state a user is in just before asking a question that quietly has
            // nothing behind it.
            //
            // Only once a specimen is loaded: before that there is genuinely
            // nothing to say, and the status bar already carries the opening
            // hint.
            if self.selected.is_some() {
                let hint = self.empty_context_hint();
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Context").strong());
                    self.background_ui(ui);
                    ui.weak(hint).on_hover_text(EMPTY_CONTEXT_RULE);
                });
                ui.separator();
            }
            return;
        }

        // The match list has to be current before the row that reports it.
        self.refresh_jump_matches();

        let mut clear_thread = false;
        let mut clear_point = false;
        let mut jump_forward = false;
        let mut jump_back = false;
        let mut go_to_class: Option<String> = None;

        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Context").strong());
            self.background_ui(ui);
            if let Some(point) = &self.pointed_at {
                // Worth saying only when it **differs** from the background
                // stage; otherwise it repeats the line above as if it were a
                // second, independent fact.
                if point.stage != self.stage {
                    ui.weak(format!("\u{00b7} pointed at in {}", point.stage.name()));
                }
            }
            // An emission failure must be stated here, not swallowed. Otherwise
            // the bar claims context Claude does not have — it would still be
            // holding the *previous* focus — which is the confident lie this
            // whole design exists to prevent.
            if let Some(err) = &self.point_error {
                ui.colored_label(
                    ui.visuals().error_fg_color,
                    format!("\u{26a0} not emitted \u{2014} {err}"),
                );
            }
        });

        if let Some(point) = &self.pointed_at {
            let request = point.request.as_str();
            let target = point.target.clone();
            ui.horizontal(|ui| {
                ui.weak("   Pointing at  ");
                ui.label(egui::RichText::new(&target).monospace());
                ui.weak(format!("({request})"));
                // Symmetric with Following. Without it the point could only be
                // *replaced*, never removed — so "explain only what I am
                // following" was unaskable, and the sole escape was reloading
                // the specimen, which recompiles and discards everything.
                if ui
                    .small_button("\u{00d7}")
                    .on_hover_text(
                        "Stop pointing at this \u{2014} leaves only what you are \
                         following in the context Claude has",
                    )
                    .clicked()
                {
                    clear_point = true;
                }
            });
        }

        if let Some(name) = self.tracked_identifier.clone() {
            ui.horizontal(|ui| {
                ui.weak("   Following    ");
                ui.label(
                    egui::RichText::new(&name)
                        .monospace()
                        .color(crate::colors::TRACKED_GOLD),
                );
                // A synthesized name is checked FIRST, because it also carries a
                // source line — inherited from the variable it shadows — and
                // reporting that as "declared at line 41" sends the reader to a
                // declaration of a *different* variable. The emitted context had
                // the same defect; the two must agree, and both must be honest.
                //
                // Recognition uses Rumoca's own inverse, never a string match:
                // `generated_names.rs` owns the convention and says consumers
                // must not spell it out themselves.
                match rumoca_core::pre_slot_base(&name) {
                    Some(base) => {
                        ui.weak(format!("\u{2014} generated: pre({base})"))
                            .on_hover_text(
                                "Synthesized by DAE pre-lowering, not declared anywhere. \
                                 A `when` equation needs a value to hold when no branch \
                                 fires, and a DAE has no way to say \u{201c}unchanged\u{201d} \
                                 \u{2014} so the previous value gets a variable of its own.",
                            );
                    }
                    None => match self
                        .identifier_index
                        .as_ref()
                        .and_then(|idx| idx.variables.get(&name))
                        .map(|v| v.source_line)
                    {
                        Some(line) => {
                            ui.weak(format!("\u{2014} declared at line {line}"));
                        }
                    None => match self.declaring_classes.get(&name) {
                        Some(class) => {
                            ui.weak("\u{2014} in");
                            if ui
                                .link(class)
                                .on_hover_text(format!(
                                    "Open {class} \u{2014} the type of the component this \
                                     variable belongs to. Use Back to return here.",
                                ))
                                .clicked()
                            {
                                go_to_class = Some(class.clone());
                            }
                        }
                        None => {
                            ui.weak("\u{2014} not declared in this specimen")
                                .on_hover_text(
                                    "Neither the specimen nor a component type declares \
                                     this name, so a compiler phase created it. Ask \
                                     Claude to trace where it came from.",
                                );
                            }
                        },
                    },
                }
                // What the question will actually have behind it.
                if let Some((mentions, stages)) = self.tracking_summary {
                    ui.weak(format!(
                        "\u{00b7} {mentions} mention{} across {stages} stage{}",
                        if mentions == 1 { "" } else { "s" },
                        if stages == 1 { "" } else { "s" },
                    ));
                }
                // Jump to where it lives in THIS stage.
                //
                // Replaces hunting for it by eye. "Reveal identifiers" tried to
                // solve this by expanding every path that leads to *any*
                // trackable name — which surfaces N nodes to reveal one, making
                // the haystack bigger. Here the target is already known: the
                // user said which identifier they are following, so the app
                // should not also make them find it.
                let n = self.jump_matches.len();
                if n == 0 {
                    // Meaningful, not a failure — the same information as
                    // `mentions: 0` in the emitted context. A variable absent
                    // from Parse but present in Flatten is showing you the
                    // flattening boundary.
                    ui.weak(format!("\u{00b7} not in {}", self.stage.name()));
                } else {
                    ui.weak(format!(
                        "\u{00b7} {} of {n} in {}",
                        self.jump_index + 1,
                        self.stage.name(),
                    ));
                    if ui
                        .small_button("\u{2190}")
                        .on_hover_text("Previous occurrence in this stage")
                        .clicked()
                    {
                        jump_back = true;
                    }
                    if ui
                        .small_button("\u{2192}")
                        .on_hover_text(
                            "Scroll the tree to where this identifier appears in this \
                             stage, opening whatever is collapsed above it",
                        )
                        .clicked()
                    {
                        jump_forward = true;
                    }
                }
                if ui.small_button("\u{00d7}").on_hover_text("Stop following").clicked() {
                    clear_thread = true;
                }
            });
        }

        // Standing context — true for the whole session, and never previously
        // stated anywhere. Without it the user underestimates what Claude can
        // already see without doing anything.
        ui.horizontal(|ui| {
            ui.weak("   Always       ");
            let stage_count = self.stages.as_stage_pairs()
                .iter().filter(|(_, v)| v.is_some()).count();
            ui.weak(format!(
                "{stage_count} stage IRs \u{00b7} {} DefIds",
                self.def_index.len(),
            ))
            .on_hover_text(
                "Every pipeline stage's full IR is on disk under .hrw-bridge/stages/, \
                 and the DefId table resolves numeric ids to names. Claude reads these \
                 without you pointing at anything.",
            );
        });
        ui.separator();

        if jump_forward || jump_back {
            self.jump_to_next_match(jump_forward);
        }
        if clear_point {
            self.pointed_at = None;
            // A stale failure would otherwise keep warning about an emission
            // for a point that no longer exists.
            self.point_error = None;
            // Clearing is a context change like any other, so it advances the
            // shared counter and re-emits. Emitting matters more here than
            // anywhere: the file still holds the old node until it is rewritten,
            // and a bar showing no point over a file holding one is exactly the
            // disagreement this design exists to prevent.
            self.context_seq = self.next_seq();
            self.emit_context();
        }
        if clear_thread {
            self.tracked_identifier = None;
            self.track_seq = self.next_seq();
            self.emit_context();
        }
        if let Some(class) = go_to_class {
            self.navigate_to(class);
        }
    }

    /// Toggle the tracked identifier — the single entry point for tracking,
    /// whichever view the click came from.
    ///
    /// Clicking the already-tracked name clears it, so every view untracks the
    /// same way the source view does. Derivative mentions like `der(h)` are
    /// reduced to the base variable, since that is what `IdentifierIndex` and
    /// the source declaration are keyed by (idea #37's "wrinkle").
    fn set_tracked_identifier(&mut self, name: String) {
        let name = crate::identifier_index::strip_der(&name).to_owned();
        // The action most likely to be the last one before a crash: it walks
        // every stage's IR and lexes every code-bearing string in it, which is
        // exactly how the 2026-07-28 em-dash panic was reached.
        diagnostics::record_action(
            "follow",
            if self.tracked_identifier.as_deref() == Some(name.as_str()) {
                format!("stop following {name}")
            } else {
                format!("follow {name} (in {})", self.stage.name())
            },
        );
        if self.tracked_identifier.as_deref() == Some(name.as_str()) {
            self.tracked_identifier = None;
        } else {
            self.tracked_identifier = Some(name);
        }
        // Recency, not identity: the counter advances on *any* change,
        // including clearing, so the emitted context can say which half of it
        // the user touched most recently.
        self.track_seq = self.next_seq();
        // Following is context, so changing it changes what Claude has. Emit
        // now rather than waiting for the next capture, or the Context Bar
        // would show a thread that had never been sent.
        self.emit_context();
    }

    fn source_map_ui(&mut self, ui: &mut egui::Ui) {
        let Some(sheet) = &self.cached_equation_sheet else {
            ui.weak("(no equation sheet)");
            return;
        };
        if sheet.source_lines.is_empty() {
            ui.weak("(no source mapping available)");
            return;
        }

        let highlighted_line = self.highlighted_source_line;
        let highlighted_eq = self.highlighted_eq_row;
        let tracked = self.tracked_identifier.as_deref();
        let tracked_line = self.tracked_identifier.as_deref()
            .and_then(|name| self.identifier_index.as_ref()
                .and_then(|idx| idx.variables.get(name))
                .map(|v| v.source_line));

        // Collect equation indices associated with the highlighted source line.
        let line_eq_indices: Vec<usize> = highlighted_line
            .and_then(|ln| sheet.source_lines.get(ln as usize - 1))
            .map(|sl| sl.equation_indices.clone())
            .unwrap_or_default();

        // Collect source lines associated with the highlighted equation.
        let eq_source_lines: Vec<u32> = if let Some(eq_idx) = highlighted_eq {
            sheet.groups.iter()
                .flat_map(|(_, eqs)| eqs)
                .find(|eq| eq.index == eq_idx)
                .map(|eq| eq.source_lines.clone())
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        let mut clicked_line = None;
        let mut clicked_eq = None;

        let avail = ui.available_size();
        let left_width = (avail.x * SOURCE_MAP_SPLIT_FRACTION).max(200.0);

        // Use StripBuilder-style layout: a left child_ui for source, a
        // separator, then the remaining space for equations. Both children
        // get the full available height.
        let full_rect = ui.available_rect_before_wrap();
        let left_rect = egui::Rect::from_min_size(
            full_rect.min,
            egui::vec2(left_width, full_rect.height()),
        );
        let sep_x = left_rect.max.x;
        let right_rect = egui::Rect::from_min_max(
            egui::pos2(sep_x + 6.0, full_rect.min.y),
            full_rect.max,
        );

        // ---- Left pane: source code ----
        let mut left_ui = ui.new_child(egui::UiBuilder::new().max_rect(left_rect));
        left_ui.label(egui::RichText::new("Modelica source").strong());
        left_ui.weak("Click a line to see which equations it produced.");
        left_ui.add_space(4.0);

        egui::ScrollArea::both()
            .id_salt("source_map_source")
            .auto_shrink(false)
            .show(&mut left_ui, |ui| {
                for sl in &sheet.source_lines {
                    let is_selected = highlighted_line == Some(sl.line_number);
                    let is_eq_linked = eq_source_lines.contains(&sl.line_number);
                    let is_tracked = tracked_line == Some(sl.line_number);
                    let has_equations = !sl.equation_indices.is_empty();

                    // Foreground = syntax, background = relationship. The line
                    // number is not Modelica, so it is appended plainly rather
                    // than being run through the lexer.
                    let background = if is_tracked {
                        Some(crate::colors::TRACKED_FILL_MEDIUM)
                    } else if is_eq_linked {
                        Some(crate::colors::SOURCE_MAP_LINK)
                    } else {
                        None
                    };
                    let modelica = crate::source_view::ModelicaText::new(ui)
                        .background(background);
                    let mut job = egui::text::LayoutJob::default();
                    modelica.append_plain(
                        &mut job,
                        &format!("{:>4} ", sl.line_number),
                        ui.visuals().weak_text_color(),
                    );
                    modelica.append(&mut job, &sl.text);
                    let text = job;

                    if has_equations {
                        if let Some(cat) = sl.category {
                            let color = cat.color().gamma_multiply(0.7);
                            let bar_rect = ui.horizontal(|ui| {
                                let resp = ui.selectable_label(is_selected, text);
                                if resp.clicked() {
                                    clicked_line = Some(if is_selected {
                                        None
                                    } else {
                                        Some(sl.line_number)
                                    });
                                }
                                resp.rect
                            }).inner;
                            let painter = ui.painter();
                            let bar = egui::Rect::from_min_size(
                                bar_rect.left_top(),
                                egui::vec2(3.0, bar_rect.height()),
                            );
                            painter.rect_filled(bar, egui::CornerRadius::ZERO, color);
                        } else {
                            let resp = ui.selectable_label(is_selected, text);
                            if resp.clicked() {
                                clicked_line = Some(if is_selected {
                                    None
                                } else {
                                    Some(sl.line_number)
                                });
                            }
                        }
                    } else {
                        ui.label(text);
                    }
                }
            });

        // Separator line between the two panes.
        ui.painter().vline(
            sep_x + 2.0,
            full_rect.y_range(),
            ui.visuals().widgets.noninteractive.bg_stroke,
        );

        // ---- Right pane: equations linked to the selected source line ----
        let mut right_ui = ui.new_child(egui::UiBuilder::new().max_rect(right_rect));
        right_ui.label(egui::RichText::new("Flat equations").strong());
        if let Some(ln) = highlighted_line {
            let count = line_eq_indices.len();
            if count > 0 {
                right_ui.weak(format!(
                    "{count} equation{} from line {ln}",
                    if count == 1 { "" } else { "s" },
                ));
            } else {
                right_ui.weak(format!("Line {ln} — no equations"));
            }
        } else {
            right_ui.weak("Click a source line to see its equations.");
        }
        right_ui.add_space(4.0);

        egui::ScrollArea::both()
            .id_salt("source_map_equations")
            .auto_shrink(false)
            .show(&mut right_ui, |ui| {
                for (cat, eqs) in &sheet.groups {
                    let visible_eqs: Vec<_> = if line_eq_indices.is_empty() {
                        eqs.iter().collect()
                    } else {
                        eqs.iter()
                            .filter(|eq| line_eq_indices.contains(&eq.index))
                            .collect()
                    };
                    if visible_eqs.is_empty() {
                        continue;
                    }

                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(format!("{} ({})", cat.label(), visible_eqs.len()))
                            .strong()
                            .color(cat.color()),
                    );
                    ui.add_space(2.0);

                    for eq in visible_eqs {
                        let is_selected = highlighted_eq == Some(eq.index);

                        // No line-linked cue here, deliberately. This list is
                        // already *filtered* to the selected line's equations
                        // (see `visible_eqs` above), so such a cue would be true
                        // of every visible row and false of none — it cannot
                        // mark a subset when the subset is the whole list. The
                        // filter, plus the "N equations from line X" header, is
                        // the signal. The source-lines column is different: it
                        // is unfiltered, so its highlight does pick out a subset.
                        //
                        // Only per-equation facts get colour: the tracked
                        // identifier, and selectable_label's own selection state.
                        let text = crate::source_view::ModelicaText::new(ui)
                            .tracked(tracked.map(|t| (t, crate::colors::TRACKED_FILL_MEDIUM)))
                            .job(&eq.text);

                        let resp = ui.selectable_label(is_selected, text);
                        if resp.clicked() {
                            clicked_eq = Some(if is_selected {
                                None
                            } else {
                                Some(eq.index)
                            });
                        }
                        if eq.source_lines.is_empty() {
                            resp.on_hover_text(format!(
                                "f_x[{}] — {} (library)",
                                eq.index, &eq.origin,
                            ));
                        } else if eq.source_lines.len() == 1 {
                            resp.on_hover_text(format!(
                                "f_x[{}] — {} (line {})",
                                eq.index, &eq.origin, eq.source_lines[0],
                            ));
                        } else {
                            let lines_str: Vec<String> = eq.source_lines.iter()
                                .map(|ln| ln.to_string())
                                .collect();
                            resp.on_hover_text(format!(
                                "f_x[{}] — {} (lines {})",
                                eq.index, &eq.origin, lines_str.join(", "),
                            ));
                        }
                    }
                }
            });

        // Consume the full rect so the parent layout knows this space is used.
        ui.allocate_rect(full_rect, egui::Sense::hover());

        if let Some(new_val) = clicked_line {
            self.highlighted_source_line = new_val;
            self.highlighted_eq_row = None;
        }
        if let Some(new_val) = clicked_eq {
            self.highlighted_eq_row = new_val;
            if let Some(eq_idx) = new_val {
                let sheet = self.cached_equation_sheet.as_ref().unwrap();
                let line = sheet.groups.iter()
                    .flat_map(|(_, eqs)| eqs)
                    .find(|eq| eq.index == eq_idx)
                    .and_then(|eq| eq.source_lines.first().copied());
                self.highlighted_source_line = line;
            }
        }
    }

    /// Render a structured summary for a singular Structural stage.
    fn structural_singular_summary(ui: &mut egui::Ui, stage: &crate::worker::Stage) {
        let error_json = stage.value.as_ref().and_then(|v| v.get("error"));
        let Some(err) = error_json else {
            ui.weak("(no structural error details)");
            return;
        };
        Self::generic_error_summary(ui, err, StageKind::Structural);
    }

    /// Render a structured error summary for any stage with error data.
    fn generic_error_summary(
        ui: &mut egui::Ui,
        error: &serde_json::Value,
        stage: StageKind,
    ) {
        let kind = error.get("kind").and_then(|k| k.as_str()).unwrap_or("error");
        let message = error.get("message").and_then(|m| m.as_str()).unwrap_or("(unknown error)");

        let heading = match (kind, stage) {
            ("singular", StageKind::Structural) => "Structural singularity".to_owned(),
            ("singular", StageKind::Initialization) => "Initialization singularity".to_owned(),
            ("singular", StageKind::IndexReduction) => "Still singular after index reduction".to_owned(),
            ("singular", _) => "Structural singularity".to_owned(),
            _ => format!("{} error", stage.name()),
        };

        ui.heading(heading);
        ui.add_space(4.0);

        // For singular errors the grid below tells the story — skip the raw
        // error string which is verbose and redundant.
        if kind != "singular" {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.label(egui::RichText::new(message).color(ui.visuals().error_fg_color));
            });
        }

        // Error code (e.g. EI001 from instantiate)
        if let Some(code) = error.get("error_code").and_then(|c| c.as_str()) {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.strong("Error code");
                ui.monospace(code);
            });
        }

        // Detail text (a clearer restatement of the error)
        if let Some(detail) = error.get("detail").and_then(|d| d.as_str()) {
            ui.add_space(4.0);
            ui.label(detail);
        }

        // Mass matrix details (solve lowering)
        if kind == "mass_matrix" {
            ui.add_space(8.0);
            egui::Grid::new("mass_matrix_grid").num_columns(2).spacing([12.0, 4.0]).show(ui, |ui| {
                if let Some(name) = error.get("state_name").and_then(|n| n.as_str()) {
                    ui.strong("State variable");
                    ui.monospace(name);
                    ui.end_row();
                }
                if let Some(row) = error.get("row").and_then(|r| r.as_u64()) {
                    ui.strong("Matrix row");
                    ui.label(format!("{row}"));
                    ui.end_row();
                }
                if let Some(reason) = error.get("reason").and_then(|r| r.as_str()) {
                    ui.strong("Reason");
                    ui.label(reason);
                    ui.end_row();
                }
            });
        }

        // Evaluation context (solve lowering)
        if kind == "evaluation"
            && let Some(ctx) = error.get("context").and_then(|c| c.as_str()) {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.strong("Context");
                    ui.label(ctx);
                });
            }

        // Diagnostics list (flatten / typecheck)
        if let Some(diags) = error.get("diagnostics").and_then(|d| d.as_array())
            && !diags.is_empty() {
                ui.add_space(8.0);
                ui.strong(format!("Diagnostics ({})", diags.len()));
                for d in diags {
                    let severity = d.get("severity").and_then(|s| s.as_str()).unwrap_or("Error");
                    let code = d.get("code").and_then(|c| c.as_str());
                    let msg = d.get("message").and_then(|m| m.as_str()).unwrap_or("");
                    ui.horizontal(|ui| {
                        let sev_color = if severity.contains("Error") {
                            ui.visuals().error_fg_color
                        } else {
                            ui.visuals().warn_fg_color
                        };
                        ui.label(egui::RichText::new(severity).color(sev_color).strong());
                        if let Some(c) = code {
                            ui.monospace(format!("[{c}]"));
                        }
                        ui.label(msg);
                    });
                    if let Some(notes) = d.get("notes").and_then(|n| n.as_array()) {
                        for note in notes {
                            if let Some(text) = note.as_str() {
                                ui.horizontal(|ui| {
                                    ui.add_space(16.0);
                                    ui.weak(format!("note: {text}"));
                                });
                            }
                        }
                    }
                }
            }

        // Singularity details (structural/initialization errors)
        if kind == "singular"
            && let (Some(n_eq), Some(n_unk), Some(n_matched), Some(deficiency)) = (
                error["n_equations"].as_u64(),
                error["n_unknowns"].as_u64(),
                error["n_matched"].as_u64(),
                error["rank_deficiency"].as_u64(),
            ) {
                ui.add_space(8.0);
                egui::Grid::new("singular_grid").num_columns(2).spacing([12.0, 4.0]).show(ui, |ui| {
                    ui.strong("Equations");
                    ui.label(format!("{n_eq}"));
                    ui.end_row();
                    ui.strong("Unknowns");
                    ui.label(format!("{n_unk}"));
                    ui.end_row();
                    ui.strong("Matched");
                    ui.label(format!("{n_matched}"));
                    ui.end_row();
                    ui.strong("Rank deficiency");
                    ui.label(egui::RichText::new(format!("{deficiency}"))
                        .color(crate::colors::ANIM_FAIL).strong());
                    ui.end_row();
                });

                if let Some(eqs) = error["unmatched_equations"].as_array()
                    && !eqs.is_empty() {
                        ui.add_space(4.0);
                        ui.strong("Unmatched equations");
                        for eq in eqs {
                            if let Some(name) = eq.as_str() {
                                ui.label(format!("  {name}"));
                            }
                        }
                    }
                if let Some(unks) = error["unmatched_unknowns"].as_array()
                    && !unks.is_empty() {
                        ui.add_space(4.0);
                        ui.strong("Unmatched unknowns");
                        for unk in unks {
                            if let Some(name) = unk.as_str() {
                                ui.label(format!("  {name}"));
                            }
                        }
                    }
            }

        // Determinacy summary (initialization stage)
        if let Some(det) = error.get("determinacy") {
            ui.add_space(8.0);
            ui.strong("Initial condition determinacy");
            ui.add_space(2.0);
            egui::Grid::new("determinacy_grid").num_columns(2).spacing([12.0, 4.0]).show(ui, |ui| {
                if let Some(n) = det["states"].as_u64() {
                    ui.label("States");
                    ui.label(format!("{n}"));
                    ui.end_row();
                }
                if let Some(n) = det["initial_equations"].as_u64() {
                    ui.label("Initial equations");
                    ui.label(format!("{n}"));
                    ui.end_row();
                }
                if let Some(n) = det["fixed_start_states"].as_u64() {
                    ui.label("Fixed start states");
                    ui.label(format!("{n}"));
                    ui.end_row();
                }
                if let Some(n) = det["explicit_initial_conditions"].as_u64() {
                    ui.label("Explicit initial conditions");
                    ui.label(format!("{n}"));
                    ui.end_row();
                }
                if let Some(v) = det.get("verdict").and_then(|v| v.as_str()) {
                    ui.label("Verdict");
                    ui.label(v);
                    ui.end_row();
                }
            });
        }

        // Guidance
        if let Some(guidance) = error.get("guidance").and_then(|g| g.as_str()) {
            ui.add_space(12.0);
            ui.weak(guidance);
        }
    }
}

impl eframe::App for App {
    /// The main frame function. Called ~60 times/second (or on repaint
    /// request). Every panel, button, label, and tree node is built here.
    ///
    /// The overall layout is:
    /// - **Top**: menu bar (File, Help)
    /// - **Bottom**: status bar (bridge capture feedback)
    /// - **Left panel**: specimen file list
    /// - **Right panel**: field help / specimen info / simulation help
    /// - **Center**: the main content area (stage tabs + IR tree/spy-plot/simulation)
    ///
    /// egui panels claim space in the order they're added: top/bottom first,
    /// then left/right, and `CentralPanel` fills whatever remains. This is
    /// why the panels appear in this specific order in the code.
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.frame_ui(ui);
    }
}

impl App {
    /// The whole frame, minus the `eframe::Frame` the trait requires and this app
    /// never used.
    ///
    /// **Split out 2026-08-01 so the UI can be driven headlessly.**
    /// `eframe::Frame` cannot be constructed outside eframe, so a test harness
    /// could not call [`eframe::App::ui`] at all — one unused parameter was the
    /// only thing standing between ~12,000 lines of UI and an automated test.
    /// The parameter was already `_frame`; nothing is lost by not passing it.
    ///
    /// Everything below is unchanged and runs in the same order. See
    /// `docs/verification-plan.md` item 2.
    pub(crate) fn frame_ui(&mut self, ui: &mut egui::Ui) {
        // First thing every frame: check for results from the worker thread.
        self.drain_worker();

        // Publish the state a crash file would need. After `drain_worker`, so a
        // crash later in this frame reports the state the frame actually
        // rendered rather than the previous one's. See `diagnostics.rs`.
        diagnostics::set_snapshot(self.diagnostic_snapshot());

        // One-shot: force the debugger to resolve live_trace.rs early, so the
        // first Debug click does not pay for it. No-op after the first few
        // frames. See `Prewarm`.
        self.tick_prewarm(ui.ctx());

        // ---- Top menu bar ----
        self.menu_bar_ui(ui);

        self.floating_windows(ui);

        // ---- Bottom status bar ----
        // Added BEFORE the side panels so it spans the full window width (egui
        // panels claim space in the order they're created — a bottom panel added
        // after left/right panels would only fill the remaining center).
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.add_space(1.0);
            // Success is silent. The Context Bar states what Claude has; this
            // line is for things that happen and then are over.
            match &self.notice {
                // **Not `weak`.** A notice is something that just happened — often a
                // refusal — and rendering it in the same de-emphasised grey as the idle
                // hint made it read as background chrome. Doug clicked a link that was
                // correctly refused, with the reason on screen, and reported that
                // nothing happened (2026-07-30).
                Some(s) => ui.label(s),
                None => ui.weak(
                    "Left-click a tree node to point at it, then ask about it in the chat                      (right-click to follow an identifier, or for more actions).",
                ),
            };
            ui.add_space(1.0);
        });

        // ---- Left panel: content depends on UI mode ----
        // Tour and Specimen modes show a left panel (tour text or specimen list
        // + purpose). Debug mode hides it so the stage tabs fill HRW's window
        // (VS Code occupies the left half of the screen).
        let mut hrw_link_action: Option<HrwLink> = None;

        if self.ui_mode == UiMode::Tour {
            self.poll_tour_file();
            let tour_text = self.cached_tour.as_ref().map(|(t, _)| t.clone());
            let tour_links = tour_text.as_deref().map(extract_hrw_links).unwrap_or_default();
            register_hrw_hooks(&mut self.commonmark_cache, &tour_links);
            let panel_width = ui.available_width() * LEFT_PANEL_WIDTH_FRACTION;
            let mut switch_to: Option<TourSource> = None;
            egui::Panel::left("tour_panel")
                .exact_size(panel_width)
                .show(ui, |ui| {
                    // --- Top third: the tour list, laid out like the specimen list ---
                    //
                    // A vertical list rather than a wrapped bar: Doug, 2026-07-29 —
                    // "there are going to be too many fixture tours to fit into a bar."
                    // One fixture per capability still accumulates, and a bar degrades
                    // silently as it fills, which is the wrong failure mode for a list
                    // meant to be browsed.
                    let panel_height = ui.available_height();
                    let list_height = panel_height * SPECIMEN_LIST_HEIGHT_FRACTION;
                    ui.allocate_ui_with_layout(
                        egui::vec2(ui.available_width(), list_height),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            section_header(ui, "Tours");
                            ui.add_space(4.0);
                            if self.tours.is_empty() {
                                ui.weak("(no tours yet)");
                                return;
                            }
                            egui::ScrollArea::vertical().id_salt("tour_list").show(ui, |ui| {
                                for source in &self.tours {
                                    let selected = self.selected_tour.as_ref() == Some(source);
                                    let resp = ui.selectable_label(selected, source.label());
                                    let resp = match source {
                                        TourSource::AdHoc => resp.on_hover_text(
                                            "Written by Claude to answer your last question. \
                                             Ephemeral: regenerated, never stored.",
                                        ),
                                        TourSource::Fixture(p) => resp.on_hover_text(format!(
                                            "Fixture tour \u{2014} a test with expected \
                                             outcomes, kept and versioned.\n{}",
                                            p.display(),
                                        )),
                                    };
                                    if resp.clicked() {
                                        switch_to = Some(source.clone());
                                    }
                                }
                            });
                        },
                    );
                    ui.separator();

                                        egui::ScrollArea::vertical()
                        .id_salt("tour")
                        .show(ui, |ui| {
                        set_markdown_text_sizes(ui);
                        match &tour_text {
                            Some(text) => {
                                egui_commonmark::CommonMarkViewer::new()
                                    .show(ui, &mut self.commonmark_cache, text);
                            }
                            None => Self::no_tour_ui(ui),
                        }
                    });
                });
            if let Some(source) = switch_to {
                self.select_tour(source);
                // Re-read now rather than waiting up to a poll interval: a click that
                // appears to do nothing for a quarter second reads as a broken button.
                self.tour_polled_at = None;
                self.poll_tour_file();
            }
            hrw_link_action = drain_hrw_hooks(&mut self.commonmark_cache, &tour_links);
        }
        if self.ui_mode == UiMode::Specimen {
        let panel_width = ui.available_width() * LEFT_PANEL_WIDTH_FRACTION;
        egui::Panel::left("specimen_panel")
            .exact_size(panel_width)
            .show(ui, |ui| {
                let panel_height = ui.available_height();
                let list_height = panel_height * SPECIMEN_LIST_HEIGHT_FRACTION;

                // -- Top third: specimen list --
                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), list_height),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        section_header(ui, "Models");
                        ui.add_space(4.0);
                        // The filter is a PREREQUISITE, not an enhancement:
                        // 18 curated files need none, 2,644 rows do.
                        ui.horizontal(|ui| {
                            ui.label("\u{1f50d}");
                            ui.add(
                                egui::TextEdit::singleline(&mut self.list_filter)
                                    .hint_text("filter by name or outcome")
                                    .desired_width(f32::INFINITY),
                            );
                        });
                        ui.add_space(4.0);

                        // **Report, but do not return** — this guard predates the
                        // corpus and used to guard too much. An empty or unscanned
                        // `specimens/` would take the 2,626 MSL models down with it,
                        // leaving the whole corpus unreachable because a *different*
                        // source was empty. Found 2026-08-01 by the headless test,
                        // which runs with no specimens scanned.
                        if let Some(err) = &self.scan_error {
                            ui.colored_label(ui.visuals().error_fg_color, err);
                        } else if self.files.is_empty() {
                            ui.weak("(no .mo specimens found)");
                        }

                        self.poll_scratch_specimens();
                        // A scratch name that collides with a curated one is skipped,
                        // and said so out loud — silently loading a different model
                        // than the one named is the failure this guards against.
                        if !self.shadowed_specimens.is_empty() {
                            ui.colored_label(
                                crate::colors::ANIM_FAIL,
                                format!(
                                    "\u{26a0} ignored scratch specimen(s) shadowing curated names: {}",
                                    self.shadowed_specimens.join(", "),
                                ),
                            );
                        }
                        egui::ScrollArea::vertical()
                            .id_salt("specimen_list")
                            .show(ui, |ui| {
                            let mut to_open = None;
                            let mut recompile = None;
                            let mut capture_specimen = false;
                            // ---- HRW specimens: curated `specimens/` + scratch ----
                            //
                            // **Collapsed at startup, with MSL expanded** (Doug,
                            // 2026-08-01) -- the reverse of the first arrangement.
                            // The corpus is now the surface most sessions browse, and
                            // 18 curated files are the ones already known by name.
                            //
                            // Counted before the header is drawn, because the header
                            // has to say how many are inside it while it is shut. A
                            // collapsed section with no count is a section a reader has
                            // to open to learn whether opening it was worth it.
                            let hrw_filter = self.list_filter.trim().to_lowercase();
                            let hrw_hits = self
                                .files
                                .iter()
                                .filter(|p| {
                                    hrw_filter.is_empty()
                                        || p.file_name()
                                            .and_then(|n| n.to_str())
                                            .is_some_and(|n| n.to_lowercase().contains(&hrw_filter))
                                })
                                .count();
                            let hrw_header = if hrw_filter.is_empty() {
                                format!("HRW specimens \u{2014} {}", self.files.len())
                            } else {
                                format!(
                                    "HRW specimens \u{2014} {hrw_hits} of {}",
                                    self.files.len(),
                                )
                            };
                            // **Forced open when a scratch specimen just arrived.**
                            // Claude writes those mid-conversation to answer the
                            // question being asked, and HRW lists them within a second
                            // without a restart. Left shut, the one file written *for
                            // the current question* would be the one file not on
                            // screen. Same rule as the filter: open when there is a
                            // reason to look.
                            let hrw_open = if !hrw_filter.is_empty() || self.scratch_arrived {
                                Some(true)
                            } else {
                                None
                            };
                            egui::CollapsingHeader::new(hrw_header)
                                .id_salt("hrw_specimen_list")
                                .default_open(false)
                                .open(hrw_open)
                                .show(ui, |ui| {
                            if hrw_hits == 0 {
                                ui.weak("no match");
                            }
                            for path in &self.files {
                                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("<?>");
                                // One box narrows every source: a curated specimen
                                // and a corpus model are found the same way.
                                if !self.list_filter.trim().is_empty()
                                    && !name.to_lowercase()
                                        .contains(&self.list_filter.trim().to_lowercase())
                                {
                                    continue;
                                }
                                let selected = self.selected.as_deref() == Some(path.as_path());
                                let can_capture = selected && !self.compiling && self.model.is_some();
                                let can_recompile = selected && !self.compiling;
                                let purpose = self.specimen_purposes.get(path);
                                // Scratch specimens are marked: "a probe Claude wrote for
                                // one question" and "part of the curated corpus" carry very
                                // different weight, and the list is where that shows.
                                let is_scratch = self.scratch_specimens.contains(path);
                                let label = if is_scratch {
                                    egui::RichText::new(format!("\u{270e} {name}"))
                                        .color(crate::colors::ANIM_EXPLORE)
                                } else {
                                    egui::RichText::new(name)
                                };
                                let mut resp = ui.selectable_label(selected, label);
                                if is_scratch {
                                    resp = resp.on_hover_text(purpose.map(String::as_str).unwrap_or(
                                        "Scratch specimen \u{2014} written by Claude to answer a \
                                         question. Ephemeral: it lives in the gitignored bridge \
                                         directory and is not part of the curated corpus.",
                                    ));
                                } else if let Some(hint) = purpose {
                                    resp = resp.on_hover_text(hint);
                                }
                                resp.context_menu(|ui| {
                                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                                    let btn = ui.add_enabled(can_recompile, egui::Button::new("🔄 Recompile"));
                                    let btn = if can_recompile {
                                        btn.on_hover_text(
                                            "Re-run the compiler on this specimen (e.g. to hit an armed breakpoint).",
                                        )
                                    } else {
                                        btn.on_disabled_hover_text(
                                            "Left-click to load this specimen first.",
                                        )
                                    };
                                    if btn.clicked() {
                                        recompile = Some(path.clone());
                                        ui.close();
                                    }
                                    ui.separator();
                                    let btn = ui.add_enabled(can_capture, egui::Button::new("🎯 Point at"));
                                    let btn = if can_capture {
                                        btn.on_hover_text(
                                            "Make the whole specimen the subject of your next question, then ask in the chat.",
                                        )
                                    } else {
                                        btn.on_disabled_hover_text(
                                            "Left-click to load & compile this specimen first, then point at it.",
                                        )
                                    };
                                    if btn.clicked() {
                                        capture_specimen = true;
                                        ui.close();
                                    }
                                });
                                if resp.clicked() {
                                    to_open = Some(path.clone());
                                }
                            }
                                });
                            // ---- The corpus: the 2,626 MSL models ----
                            //
                            // **Expanded at startup** (Doug, 2026-08-01). The comment
                            // that stood here said the section was "shown only while
                            // filtering" -- true of the first version, false of the
                            // code beneath it since the same day, and left behind. A
                            // comment that describes a design the code abandoned is
                            // worse than none: it is read as intent.
                            let filter = self.list_filter.trim().to_owned();
                            let mut open_model: Option<String> = None;
                            {
                                let rows = self.corpus_rows();
                                let total = rows.len();
                                let hits: Vec<(String, String)> = rows
                                    .iter()
                                    .filter(|r| crate::survey::matches_filter(r, &filter))
                                    .map(|r| (r.name.clone(), r.outcome.clone()))
                                    .collect();
                                if total > 0 {
                                    ui.add_space(6.0);
                                    ui.separator();
                                    // **Always visible, collapsed by default.**
                                    //
                                    // The first version rendered this section only
                                    // while filtering, so an unfiltered list showed no
                                    // sign the corpus existed at all — Doug started HRW
                                    // and reported the MSL examples "not showing", which
                                    // was exactly right from where he sat. **An absence
                                    // you cannot see is indistinguishable from a feature
                                    // that was never built**, and the headless test had
                                    // asserted the hidden behaviour, so it encoded the
                                    // defect as a requirement.
                                    //
                                    // **Open at startup, with HRW specimens shut.**
                                    // The earlier worry -- 2,626 rows burying 18
                                    // curated files -- is answered by giving the 18
                                    // their own header rather than by hiding the 2,626.
                                    // Only `MAX_LISTED` rows render, so an open corpus
                                    // costs a bounded amount of screen either way.
                                    let header = if filter.is_empty() {
                                        format!("MSL corpus \u{2014} {total} models")
                                    } else {
                                        format!("MSL corpus \u{2014} {} of {total}", hits.len())
                                    };
                                    egui::CollapsingHeader::new(header)
                                        .id_salt("corpus_list")
                                        .default_open(true)
                                        .open(if filter.is_empty() { None } else { Some(true) })
                                        .show(ui, |ui| {
                                            if hits.is_empty() {
                                                ui.weak("no match");
                                            }
                                    for (name, outcome) in hits.iter().take(crate::survey::MAX_LISTED)
                                    {
                                        let is_sel = self
                                            .selected
                                            .as_ref()
                                            .map(|p| p.as_os_str() == name.as_str())
                                            .unwrap_or(false);
                                        // The leaf name reads; the qualified name is
                                        // 60 characters and would wrap every row.
                                        let leaf = name.rsplit('.').next().unwrap_or(name);
                                        let label = egui::RichText::new(format!("\u{1f4e6} {leaf}"))
                                            .color(crate::colors::INCIDENCE_CELL);
                                        let resp = ui
                                            .selectable_label(is_sel, label)
                                            .on_hover_text(format!("{name}\n\noutcome: {outcome}"));
                                        if resp.clicked() {
                                            open_model = Some(name.clone());
                                        }
                                    }
                                    // **No silent caps.** A truncated list that does not
                                    // say so reads as "that is all there is".
                                    if hits.len() > crate::survey::MAX_LISTED {
                                        ui.weak(format!(
                                            "\u{2026} and {} more \u{2014} narrow the filter",
                                            hits.len() - crate::survey::MAX_LISTED,
                                        ));
                                    }
                                        });
                                }
                            }
                            if let Some(name) = open_model {
                                self.open_library_model(&name);
                            } else if let Some(path) = recompile {
                                self.open(path);
                            } else if let Some(path) = to_open {
                                if self.selected.as_ref() == Some(&path) {
                                    self.viewing_log = false;
                                } else {
                                    self.open(path);
                                }
                            }
                            if capture_specimen {
                                self.emit_focus(Focus::Specimen);
                            }
                        });
                    },
                );

                ui.add_space(10.0);
                section_header_toggle(
                    ui,
                    &mut self.specimen_detail,
                    &[
                        (SpecimenDetail::Source, "Source"),
                        (SpecimenDetail::Purpose, "Purpose"),
                    ],
                );
                ui.add_space(4.0);

                // -- Bottom two-thirds: source or purpose --
                match self.specimen_detail {
                    SpecimenDetail::Source => self.specimen_source_ui(ui),
                    SpecimenDetail::Purpose => {
                        let model_name = self.model.as_deref();
                        let purpose = model_name.and_then(|name| {
                            let key = PathBuf::from(name);
                            let cached = self.cached_purpose_notes.entry(key).or_insert_with(|| {
                                let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
                                let path = manifest
                                    .join("docs/specimen-notebook")
                                    .join(name)
                                    .join("purpose.md");
                                std::fs::read_to_string(path).ok()
                            });
                            cached.as_deref()
                        });

                        match purpose {
                            Some(text) => {
                                let purpose_links = extract_hrw_links(text);
                                register_hrw_hooks(&mut self.commonmark_cache, &purpose_links);
                                egui::ScrollArea::vertical()
                                    .id_salt("purpose")
                                    .show(ui, |ui| {
                                    set_markdown_text_sizes(ui);
                                    egui_commonmark::CommonMarkViewer::new()
                                        .show(ui, &mut self.commonmark_cache, text);
                                });
                                if hrw_link_action.is_none() {
                                    hrw_link_action =
                                        drain_hrw_hooks(&mut self.commonmark_cache, &purpose_links);
                                }
                            }
                            None => {
                                for line in
                                    purpose_placeholder(model_name, self.selected.as_deref())
                                {
                                    ui.weak(line);
                                }
                            }
                        }
                    }
                }
            });
        }

        // ---- Dispatch hrw:// link actions ----
        if let Some(action) = hrw_link_action {
            self.dispatch_hrw_link(action);
        }

        // ---- Center panel: stage tabs + main content ----
        //
        // More "deferred action" variables: the CentralPanel closure borrows
        // `self` through the panel, so we can't call methods like
        // `emit_node_focus` inside it. Instead, the closure sets these flags
        // and the actual method calls happen after the closure, below.
        // Everything the panels want done, collected in one place. See
        // `FrameIntent` for why this is a struct rather than seven locals.
        //
        // `hrw_link_action` is deliberately *not* in here: the left panel
        // collects it and it is acted on immediately above, before the central
        // panel renders, so it is not deferred state.
        let mut intent = FrameIntent {
            expand_trackable: self.expand_trackable,
            ..FrameIntent::default()
        };

        // `CentralPanel` fills all remaining space after top/bottom/left/right
        // panels have claimed theirs. This is where the main content lives.
        // The central panel's body is `central_panel_ui`. It was extracted
        // 2026-07-29 once `FrameIntent` made it possible: it needs only
        // `&mut self` and one `&mut FrameIntent`, where before it would have
        // required seven transposable out-parameters.
        egui::CentralPanel::default().show(ui, |ui| {
            self.central_panel_ui(ui, &mut intent);
        });

        // ---- Deferred actions ----
        //
        // All the flags/options collected during the CentralPanel closure are
        // now acted on. The panel closure has ended, so `self` is no longer
        // borrowed and we can call methods freely. This is the payoff of the
        // "collect intent, act later" pattern used throughout this function.
        let FrameIntent {
            tree: tree_actions,
            expand_trackable,
            canvas_capture,
            want_stage_ask,
            go_back,
            go_home,
        } = intent;

        if go_home {
            self.nav.clear();
        } else if go_back {
            self.nav.pop();
        }
        self.expand_trackable = expand_trackable;
        // The tree is the only producer of these three; earlier revisions
        // declared parallel locals for other views to write into, but nothing
        // ever did, so `nav_to.or(tree_actions.nav_to)` was always the latter.
        let debug_ask = tree_actions.debug;
        let mut node_ask = tree_actions.capture;
        if let Some(name) = tree_actions.nav_to {
            self.navigate_to(name);
        }
        // Reverse tracking from any stage (idea #37). Reveals the source, the
        // same as tracking from the equation sheet — the point is seeing where
        // the identifier came from.
        if let Some(name) = tree_actions.track {
            self.set_tracked_identifier(name);
            if self.tracked_identifier.is_some() {
                self.ui_mode = UiMode::Specimen;
                self.specimen_detail = SpecimenDetail::Source;
            }
        }
        // A spy-plot block click is treated identically to a tree-node click
        // for capture purposes.
        if canvas_capture.is_some() {
            node_ask = canvas_capture;
        }
        // Priority: debugger capture > explain capture > stage capture.
        // Only one bridge write per frame.
        if let Some(key_path) = debug_ask {
            self.emit_node_focus(key_path, bridge::AskRequest::DebugWhereSet);
        } else if let Some(key_path) = node_ask {
            self.emit_node_focus(key_path, bridge::AskRequest::Explain);
        } else if want_stage_ask {
            self.emit_focus(Focus::Stage);
        }

        // The jump lasts exactly one frame. It has now been rendered: the target
        // row called `scroll_to_me`, and forcing its ancestors open *stored*
        // that state in egui, so they stay open on their own from here. Holding
        // it any longer would re-scroll every frame — pinning the view the way
        // the source view's reverse-tracking scroll did before it was gated on
        // change — and would keep those headers forced open, which is the
        // "Reveal identifiers" complaint all over again.
        self.jump_target = None;

        // ---- End of frame: publish diagnostics ----
        //
        // Refresh the snapshot and only then write `session.json`, so its `app` block
        // describes the state the frame's actions *produced*. The top-of-frame snapshot
        // is still set (a crash mid-frame reports where the user was), but it is the
        // wrong thing to pair with an action that just fired: on 2026-07-30 that
        // combination made Claude report three phantom bugs from correct values.
        //
        // `flush_session` is a no-op unless an action was recorded, so an idle frame
        // costs one bool check.
        diagnostics::set_snapshot(self.diagnostic_snapshot());
        diagnostics::flush_session();
    }
}

/// Read a specimen's one-line purpose hint — the first `// purpose:` comment in
/// the file (the phenomenon it's authored to exercise). Scanned without compiling
/// so every file in the list gets a hint, even one that fails to compile. `None`
/// if the convention is absent.
///
/// The convention: each specimen `.mo` file contains a comment like:
/// ```text
/// // purpose: high-index, structurally singular DAE
/// ```
/// This is scanned at **rescan time** (when the specimen directory is read),
/// NOT at compile time. This is deliberate — it means every specimen in the
/// list gets its purpose hint shown in the tooltip even if the specimen fails
/// to compile. The purpose comment describes the *compiler feature* the
/// specimen exercises, which is different from the Modelica `annotation`
/// description string (which describes the *model* itself).
fn read_purpose(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    text.lines().find_map(|line| {
        line.trim_start()
            .strip_prefix("// purpose:")
            .map(str::trim)
            .filter(|hint| !hint.is_empty())
            .map(str::to_owned)
    })
}

/// A blue-tinted header bar for left-panel sections (Tour, Specimens, Purpose).
/// Uses a navy background with light-blue text in dark mode, matching the RHS
/// stage-tab palette for visual consistency.
struct SectionStyle {
    active_color: egui::Color32,
    inactive_color: egui::Color32,
    frame: egui::Frame,
}

fn section_style(ui: &egui::Ui) -> SectionStyle {
    let dark = ui.visuals().dark_mode;
    let bg = if dark {
        egui::Color32::from_rgb(0x1A, 0x2A, 0x40)
    } else {
        egui::Color32::from_rgb(0xD8, 0xE8, 0xF8)
    };
    let active_color = if dark {
        egui::Color32::from_rgb(0x8A, 0xC4, 0xFF)
    } else {
        egui::Color32::from_rgb(0x0A, 0x5C, 0xC4)
    };
    let inactive_color = if dark {
        egui::Color32::from_rgb(0x50, 0x70, 0x90)
    } else {
        egui::Color32::from_rgb(0x60, 0x90, 0xC0)
    };
    let h_margin = ui.spacing().item_spacing.x;
    let frame = egui::Frame::new()
        .fill(bg)
        .inner_margin(egui::Margin::symmetric(6, 4))
        .outer_margin(egui::Margin { left: -h_margin as i8, right: -h_margin as i8, top: 2, bottom: 0 });
    SectionStyle { active_color, inactive_color, frame }
}

fn section_header(ui: &mut egui::Ui, title: &str) {
    let style = section_style(ui);
    style.frame.show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.label(egui::RichText::new(title).strong().size(13.0).color(style.active_color));
    });
}

/// A section header bar with clickable toggle options (e.g. "Source | Purpose").
/// The active option is shown in bright text; inactive options are dimmed and clickable.
fn section_header_toggle<T: PartialEq + Copy>(
    ui: &mut egui::Ui,
    current: &mut T,
    options: &[(T, &str)],
) {
    let style = section_style(ui);
    style.frame.show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            for (i, (value, label)) in options.iter().enumerate() {
                if i > 0 {
                    ui.label(egui::RichText::new("|").size(13.0).color(style.inactive_color));
                }
                let is_active = *current == *value;
                let color = if is_active { style.active_color } else { style.inactive_color };
                let text = if is_active {
                    egui::RichText::new(*label).strong().size(13.0).color(color)
                } else {
                    egui::RichText::new(*label).size(13.0).color(color)
                };
                if ui.add(egui::Label::new(text).sense(egui::Sense::click())).clicked() {
                    *current = *value;
                }
            }
        });
    });
}

/// Everything the frame's panels want done, acted on after they close.
///
/// egui panel bodies borrow `self`, so a click cannot mutate app state where it
/// happens — it records what the user asked for and [`App::ui`] acts once the
/// borrows end. Before this struct existed those were seven separate locals,
/// which is what blocked extracting any panel body into a method: each
/// extraction would have needed all eight threaded through as transposable
/// out-parameters. One `&mut FrameIntent` instead.
///
/// Same pattern as `tree::TreeActions`, which bundled the tree's out-parameters
/// for the same reason; that type is nested here rather than flattened, because
/// the tree owns which of its actions exist.
#[derive(Default)]
struct FrameIntent {
    /// What the IR tree wants: capture, debug-capture, navigate, track.
    tree: tree::TreeActions,
    /// Copied out of `self` because the stage-tree block holds an immutable
    /// borrow of `self` (via `current_stage`); written back at the end.
    expand_trackable: bool,
    /// A spy-plot or incidence block was clicked — treated identically to a
    /// tree-node click for capture purposes.
    canvas_capture: Option<Vec<Seg>>,
    /// A stage tab was clicked, so the capture should describe the stage.
    want_stage_ask: bool,
    /// Navigation "Back".
    go_back: bool,
    /// Navigation "Specimen" — clears the whole nav stack.
    go_home: bool,
}

/// A sub-view within a stage, addressable by an `hrw://` link.
///
/// **The slugs are exactly the names the capture emits** (`structural_view_name`,
/// `flatten_view_name`, `events_view_name`, `init_view_name`), which is #42's design
/// principle applied: `hrw://` should express any noun `focus.json` can describe, so
/// the two directions share one vocabulary rather than inventing a second.
///
/// Added 2026-07-29 to close a tour hole. Links reached a stage tab and no further,
/// so every animation and custom view — all of them one level below a stage — had to
/// be handed off in prose ("same tab → now click **Incidence**"). The first tour had
/// two working links and four such hand-offs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SubView {
    Structural(StructuralView),
    Flatten(FlattenView),
    Events(EventsView),
    Init(InitView),
}

impl SubView {
    /// The slug this sub-view is written as in an `hrw://` link.
    ///
    /// **The canonical inverse of [`Self::from_slug`]**, and it dispatches to the very
    /// functions the *capture* uses (`structural_view_name` and friends) rather than
    /// repeating the strings. That is what makes "the link vocabulary equals the capture
    /// vocabulary" true by construction instead of by assertion.
    ///
    /// Added 2026-07-30. `from_slug` had existed without an inverse — the same gap that
    /// let the stage vocabulary drift into four hand-written copies, one of which
    /// disagreed with the other three.
    fn slug(&self) -> &'static str {
        match self {
            Self::Structural(v) => structural_view_name(*v),
            Self::Flatten(v) => flatten_view_name(*v),
            Self::Events(v) => events_view_name(*v),
            Self::Init(v) => init_view_name(*v),
        }
    }

    /// Resolve a slug against the stage it appears under.
    ///
    /// Stage-scoped rather than global because the same slug means different things
    /// in different stages — `Tree` exists under four of them — and because a link
    /// naming a sub-view the stage does not have should fail to parse rather than
    /// navigate somewhere surprising.
    fn from_slug(stage: StageKind, slug: &str) -> Option<Self> {
        Some(match stage {
            StageKind::Structural | StageKind::IndexReduction => {
                Self::Structural(match slug {
                    "Summary" => StructuralView::Summary,
                    "SpyPlot" => StructuralView::SpyPlot,
                    "Incidence" => StructuralView::Incidence,
                    "MatchingAnim" => StructuralView::MatchingAnim,
                    "TarjanAnim" => StructuralView::TarjanAnim,
                    "TearingAnim" => StructuralView::TearingAnim,
                    "AliasAnim" => StructuralView::AliasAnim,
                    "Animate" => StructuralView::Animate,
                    "Tree" => StructuralView::Tree,
                    _ => return None,
                })
            }
            StageKind::Flatten => Self::Flatten(match slug {
                "EquationSheet" => FlattenView::Equations,
                "SourceMap" => FlattenView::SourceMap,
                "Connections" => FlattenView::Connections,
                "Tree" => FlattenView::Tree,
                _ => return None,
            }),
            StageKind::Events => Self::Events(match slug {
                "PreLowering" => EventsView::PreLowering,
                "Tree" => EventsView::Tree,
                _ => return None,
            }),
            StageKind::Initialization => Self::Init(match slug {
                "IcPlan" => InitView::IcPlan,
                "Tree" => InitView::Tree,
                _ => return None,
            }),
            // Stages with no sub-views: a link naming one is malformed.
            _ => return None,
        })
    }
}

/// Which tour the Tour panel is showing.
///
/// Two kinds, with different lifetimes and different jobs:
///
/// - **`AdHoc`** — `.hrw-bridge/tour.md`, written by Claude to answer the question just
///   asked. Gitignored, regenerated, ephemeral by construction.
/// - **`Fixture`** — a file in `docs/fixture-tours/`, kept and versioned because it is a
///   *test* with a pass/fail criterion rather than an explanation that would rot.
///
/// Keeping them in one list (rather than one panel each) is deliberate: from the
/// reader's side both are "a sequence of stops to walk", and the distinction is about
/// where the file lives, not about how it is used.
#[derive(Clone, Debug, PartialEq, Eq)]
enum TourSource {
    AdHoc,
    Fixture(PathBuf),
}

impl TourSource {
    fn path(&self) -> PathBuf {
        match self {
            Self::AdHoc => PathBuf::from(bridge::TOUR_FILE),
            Self::Fixture(p) => p.clone(),
        }
    }

    /// Label for the picker. The ad hoc tour is named by what it *is* rather than by
    /// its filename, which is an implementation detail nobody should have to know.
    fn label(&self) -> String {
        match self {
            Self::AdHoc => "\u{2728} Claude's answer".to_owned(),
            Self::Fixture(p) => p
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("fixture")
                .to_owned(),
        }
    }
}

/// Navigation action parsed from an `hrw://` URI in tour or narrative markdown.
#[derive(Debug, PartialEq, Eq)]
enum HrwLink {
    /// `hrw://load/<Specimen>` — load and compile a specimen by name.
    LoadSpecimen(String),
    /// `hrw://stage/<Stage>[/<SubView>]` — switch to a stage tab, optionally to a
    /// sub-view within it (specimen already loaded).
    SwitchStage(StageKind, Option<SubView>),
    /// `hrw://load/<Specimen>/<Stage>[/<SubView>]` — load a specimen, then switch.
    LoadAndSwitch(String, StageKind, Option<SubView>),
    /// `hrw://source[/<line>]` — show the Modelica source, optionally scrolled to a
    /// 1-based line.
    ///
    /// Added 2026-07-29 to close a tour hole: two tours had to *quote* a source line
    /// ("reported at line 9, `connect(src.n, gnd.p);`") because nothing could point at
    /// one. Quoting is a prose workaround, which is the quiet-hole species that
    /// accumulates unnoticed — see the tour-holes table in `docs/tech-debt.md`.
    ShowSource(Option<u32>),
    /// `hrw://stage/<Stage>/<SubView>/equation/<n>` — go to a canvas view and **aim
    /// the camera** at equation `n`, so a stop can say "watch this equation" and put it
    /// in front of the reader.
    ///
    /// **0-based, unlike `frame`** — deliberately. Each verb matches how *its* thing is
    /// displayed: equations appear as `f_x[46]` counting from zero, frames appear as
    /// "Frame 3/11" counting from one. Making the two verbs uniform would force one of
    /// them to disagree with the screen. Agreeing with the display is the rule; agreeing
    /// with each other is not.
    ///
    /// **The noun is `equation`, not `node`.** Tarjan's view draws equations as graph
    /// nodes, so "node" was the tempting word — but the matrix views index the same
    /// thing as a row, and the IR calls it `f_x[n]`. A verb shared by four views needs
    /// the vocabulary they share, or the capture and the link drift apart exactly as
    /// the stage slug did.
    ///
    /// Added 2026-07-29. Until then a tour could name a *view* but not a place inside
    /// it, which is where a canvas view's content actually lives: `ideas.md` #42 listed
    /// camera aiming as the biggest missing capability for canvas-backed stops.
    ///
    /// Only meaningful for the canvas-backed views. A text or grid view has no camera,
    /// so the link still navigates and the aim is simply ignored rather than failing
    /// the stop — a tour degrading to "the right view, not aimed" beats one that
    /// silently does nothing.
    AimAtEquation(StageKind, SubView, usize),
    /// `hrw://stage/<Stage>/<SubView>/frame/<n>` — go to an animated view and **stop on
    /// frame `n`**, so a tour can point at the moment a decision is made rather than at
    /// the view containing it.
    ///
    /// The moment is where a replay's content lives: "the algorithm gives up here",
    /// "this is the tear that cascades". Naming only the view leaves the reader to find
    /// it by scrubbing.
    ///
    /// **Recorded playback only.** A live session has no meaningful frame count until
    /// it ends — that was a real bug Doug caught — so a frame index cannot mean anything
    /// stable there. Seeking a live view is refused like any out-of-range frame.
    SeekFrame(StageKind, SubView, usize),
    /// `hrw://stage/<Stage>/<SubView>/node/<path>` — open a stage tree and **point at a
    /// node**, expanding its ancestors and scrolling it into view.
    ///
    /// `<path>` is exactly what a capture writes: `error.unmatched_unknowns[0]`. See
    /// `bridge::parse_path`, the documented inverse of `bridge::describe_path`.
    ///
    /// This was the last and largest parity gap. A node path is the capture's **richest
    /// noun** — it is what a left-click produces, and what most of Doug's questions have
    /// been about — and until 2026-07-29 a tour could open a tree but not point into it.
    PointAtNode(StageKind, SubView, Vec<Seg>),
    /// `hrw://follow/<name>` — follow an identifier, as a right-click Follow would.
    ///
    /// The other half of the composition primitives: the capture has always carried a
    /// `tracking` section, and no link could set one. A stop can now say "follow
    /// `emf.phi` and watch where it goes", which is the gesture the cross-stage view was
    /// built for.
    Follow(String),
    /// `hrw://notebook/<name.nb>` — open a Wolfram notebook in Wolfram Desktop.
    ///
    /// The cross-platform tours route through a notebook, and a plain markdown link to
    /// one is handed to the *browser* — which does nothing useful with a `.nb`. Doug hit
    /// exactly that on the first cross-platform tour (2026-07-30). A tour should drive
    /// the reader to the stop, not tell him to go and find the file.
    ///
    /// The name is resolved by `bridge::resolve_notebook`, which restricts it to a file
    /// name in one of two known directories.
    OpenNotebook(String),
    /// `hrw://systemmodeler/<Specimen>` — open a specimen in Wolfram System Modeler.
    ///
    /// **The adjudicator verb.** System Modeler is an independent Modelica
    /// implementation, so "SM rejects this model that Rumoca accepts" is the strongest
    /// claim a tour can make — see `docs/upstream-issues.md` #2, which exists because of
    /// exactly that comparison.
    ///
    /// No new mechanism: the System Modeler installer already associates `.mo` with
    /// `ModelCenter.exe` (verified 2026-07-30), so this is the same OS hand-off that
    /// opens a notebook. HRW never learns where System Modeler lives.
    OpenInSystemModeler(String),
}

impl HrwLink {
    /// One line naming what this link does, for the action trail.
    ///
    /// Reconstructs the canonical URL rather than `Debug`-printing the enum: the trail
    /// is read by Claude alongside the tour markdown, and matching the tour's own text
    /// is what makes "Doug clicked Stop 3" legible at a glance.
    /// Whether this link needs a specimen already loaded.
    ///
    /// Doug clicked a tour's *fourth* stop first, without the first three, and nothing
    /// happened. With no specimen the whole stage area returns early, so a stage link
    /// set state that nothing consumed — silently, which is the failure mode every other
    /// verb has been taught to avoid.
    ///
    /// The three that do **not** need one are the three that make sense on their own: the
    /// two load verbs, and opening a notebook.
    fn requires_specimen(&self) -> bool {
        !matches!(
            self,
            Self::LoadSpecimen(_)
                | Self::LoadAndSwitch(..)
                | Self::OpenNotebook(_)
                | Self::OpenInSystemModeler(_)
        )
    }

    fn describe(&self) -> String {
        match self {
            Self::LoadSpecimen(name) => format!("load/{name}"),
            Self::SwitchStage(kind, None) => format!("stage/{}", kind.slug()),
            Self::SwitchStage(kind, Some(sub)) => {
                format!("stage/{}/{}", kind.slug(), sub.slug())
            }
            Self::LoadAndSwitch(name, kind, None) => format!("load/{name}/{}", kind.slug()),
            Self::LoadAndSwitch(name, kind, Some(sub)) => {
                format!("load/{name}/{}/{}", kind.slug(), sub.slug())
            }
            Self::ShowSource(None) => "source".to_owned(),
            Self::ShowSource(Some(line)) => format!("source/{line}"),
            Self::AimAtEquation(kind, sub, n) => {
                format!("stage/{}/{}/equation/{n}", kind.slug(), sub.slug())
            }
            Self::SeekFrame(kind, sub, n) => {
                // +1 back to the 1-based form the link was written in.
                format!("stage/{}/{}/frame/{}", kind.slug(), sub.slug(), n + 1)
            }
            Self::PointAtNode(kind, sub, path) => format!(
                "stage/{}/{}/node/{}",
                kind.slug(),
                sub.slug(),
                bridge::describe_path(path),
            ),
            Self::Follow(name) => format!("follow/{name}"),
            Self::OpenNotebook(name) => format!("notebook/{name}"),
            Self::OpenInSystemModeler(name) => format!("systemmodeler/{name}"),
        }
    }
}

/// Parse an `hrw://` URL into a navigation action, or `None` if malformed.
fn parse_hrw_link(url: &str) -> Option<HrwLink> {
    let path = url.strip_prefix("hrw://")?;
    // 5, not 4: the node form (`stage/<Stage>/<View>/node/<n>`) is five segments.
    // With a cap of 4 the trailing `node/<n>` glommed into one segment and the link
    // silently failed to parse — a link that does nothing is the worst outcome in a
    // tour, since nothing on screen says why.
    let parts: Vec<&str> = path.splitn(5, '/').collect();
    match parts.as_slice() {
        ["load", specimen, stage, view] => {
            let kind = StageKind::from_slug(stage)?;
            let sub = SubView::from_slug(kind, view)?;
            Some(HrwLink::LoadAndSwitch((*specimen).to_owned(), kind, Some(sub)))
        }
        ["load", specimen, stage] => {
            let kind = StageKind::from_slug(stage)?;
            Some(HrwLink::LoadAndSwitch((*specimen).to_owned(), kind, None))
        }
        ["load", specimen] => Some(HrwLink::LoadSpecimen((*specimen).to_owned())),
        ["stage", stage, view] => {
            let kind = StageKind::from_slug(stage)?;
            let sub = SubView::from_slug(kind, view)?;
            Some(HrwLink::SwitchStage(kind, Some(sub)))
        }
        ["stage", stage] => {
            let kind = StageKind::from_slug(stage)?;
            Some(HrwLink::SwitchStage(kind, None))
        }
        // Longest patterns first: `splitn` caps the segment count, so the node form
        // must be matched before the shorter stage forms can swallow it.
        ["follow", name] => Some(HrwLink::Follow((*name).to_owned())),
        // A non-empty name: `hrw://notebook/` alone names nothing, and accepting it
        // meant a prose mention of the verb parsed as a link to an unnamed file.
        ["systemmodeler", name] if !name.is_empty() => {
            Some(HrwLink::OpenInSystemModeler((*name).to_owned()))
        }
        ["notebook", name] if !name.is_empty() => {
            Some(HrwLink::OpenNotebook((*name).to_owned()))
        }
        ["stage", stage, view, "node", path] => {
            let kind = StageKind::from_slug(stage)?;
            Some(HrwLink::PointAtNode(
                kind,
                SubView::from_slug(kind, view)?,
                bridge::parse_path(path)?,
            ))
        }
        ["stage", stage, view, "frame", n] => {
            let kind = StageKind::from_slug(stage)?;
            // **1-based, matching the frame counter on screen** ("Frame 3/11"). Links
            // were 0-based until 2026-07-29, so a tour saying `frame/40` landed on a
            // view reading "41" — the link vocabulary and the display disagreeing about
            // the same noun, which is the drift the parity audit exists to catch. The
            // fixture tour had the discrepancy *written into it* as a parenthetical,
            // which is documenting a bug rather than fixing it.
            //
            // `checked_sub(1)` rejects `frame/0` for free: under 1-based numbering there
            // is no frame zero, and a link saying so is a mistake worth surfacing.
            Some(HrwLink::SeekFrame(
                kind,
                SubView::from_slug(kind, view)?,
                n.parse::<usize>().ok()?.checked_sub(1)?,
            ))
        }
        ["stage", stage, view, "equation", n] => {
            let kind = StageKind::from_slug(stage)?;
            Some(HrwLink::AimAtEquation(
                kind,
                SubView::from_slug(kind, view)?,
                n.parse().ok()?,
            ))
        }
        ["source", line] => Some(HrwLink::ShowSource(Some(line.parse().ok()?))),
        ["source"] => Some(HrwLink::ShowSource(None)),
        _ => None,
    }
}

/// Scan markdown text for all unique `hrw://` URLs (for hook registration).
fn extract_hrw_links(text: &str) -> Vec<String> {
    let mut links = Vec::new();
    for cap in text.match_indices("hrw://") {
        let start = cap.0;
        let rest = &text[start..];
        // A backtick ends a URL too: `hrw://notebook/` written in a **code span** is
        // prose *about* the verb, not a link. Without it the extractor captured the
        // backtick as part of the name and registered a hook that could never fire —
        // found 2026-07-30 by the fixture file-reference test, which duly reported a
        // notebook named "`" as missing.
        let end = rest
            .find([')', ' ', '\n', '"', '>', '`'])
            .unwrap_or(rest.len());
        let url = &rest[..end];
        if !links.contains(&url.to_owned()) {
            links.push(url.to_owned());
        }
    }
    links
}

/// Register link hooks so `egui_commonmark` reports clicks on `hrw://` links.
fn register_hrw_hooks(cache: &mut egui_commonmark::CommonMarkCache, links: &[String]) {
    for link in links {
        if cache.get_link_hook(link).is_none() {
            cache.add_link_hook(link);
        }
    }
}

/// Check registered hooks for a click and return the first triggered action.
fn drain_hrw_hooks(cache: &mut egui_commonmark::CommonMarkCache, links: &[String]) -> Option<HrwLink> {
    for link in links {
        if cache.get_link_hook(link) == Some(true) {
            return parse_hrw_link(link);
        }
    }
    None
}

/// What the Purpose tab shows when there is no note to render.
///
/// Extracted from the view so the wording is testable. Both messages it replaced were
/// wrong, and Doug found both by using the app (2026-07-29):
///
/// 1. They said **"narrative"**, a term retired when the narratives were. A renamed
///    concept leaves its old name in the strings nobody greps for.
/// 2. Worse, selecting a *second* specimen showed **"Select a specimen"** — advising
///    Doug to do the thing he had just done. The note is keyed on the *model* name,
///    which stays `None` until compilation finishes, so a selected-but-compiling
///    specimen fell through to the nothing-selected arm. That was a **missing state**,
///    not merely bad wording, which is why this returns three cases and not two.
fn purpose_placeholder(model: Option<&str>, selected: Option<&Path>) -> Vec<String> {
    match (model, selected) {
        // Compiled, and this model has no note. Saying *where* one would live makes
        // the absence actionable instead of a dead end.
        (Some(name), _) => vec![
            format!("No purpose note for {name}."),
            format!(
                "One would live at docs/specimen-notebook/{name}/purpose.md \u{2014} why the \
                 specimen exists, and which questions it has answered.",
            ),
        ],
        // Selected, still compiling. Name the file so the wait is legible.
        (None, Some(path)) => {
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("specimen");
            vec![
                format!("Compiling {stem}\u{2026}"),
                "Its purpose note appears once the model name is known.".to_owned(),
            ]
        }
        (None, None) => vec!["Select a specimen to see its purpose.".to_owned()],
    }
}

/// Hand a file to the operating system's association for its type.
///
/// A `.nb` opens in Wolfram Desktop, because that is what Windows associates it with —
/// HRW does not need to know where Wolfram lives, and would be wrong the moment it moved.
///
/// `cmd /C start` rather than a crate: adding a dependency needs asking first, and this
/// is four lines. The empty string after `start` is **required** — `start` treats a lone
/// quoted argument as a window title, so omitting it opens a console instead of the file.
#[cfg(target_os = "windows")]
fn open_with_os(path: &Path) -> std::io::Result<()> {
    std::process::Command::new("cmd")
        .args(["/C", "start", "", &path.display().to_string()])
        .spawn()
        .map(|_| ())
}

/// Non-Windows fallback. HRW is Windows-only in practice (charter Decision 5 rules out
/// other targets), but a `cfg` that silently compiles to nothing would be worse than one
/// that says so.
#[cfg(not(target_os = "windows"))]
fn open_with_os(path: &Path) -> std::io::Result<()> {
    std::process::Command::new("xdg-open").arg(path).spawn().map(|_| ())
}

/// Resolve a pending tree jump target against the stage's IR.
///
/// `Ok(target)` when it resolves; `Err(message)` naming the path when it does not.
///
/// A free function rather than a method because the caller holds an immutable borrow of
/// the stage while rendering, so it cannot also take `&mut self`. The caller applies the
/// message and clears the target.
///
/// **Why validate at all:** the tree otherwise expands as far as the path goes and stops,
/// which reads as "it opened something" rather than "that path is wrong". The camera aim
/// and the frame seek both refuse-and-report; this makes the third verb consistent. A
/// link naming something that is not there is a bug in the tour, and must be visible.
fn resolve_jump_target(stage_value: &Value, target: &[Seg]) -> Result<(), String> {
    if target.is_empty() || bridge::navigate(stage_value, target).is_some() {
        return Ok(());
    }
    Err(format!(
        "no node at {} in this stage \u{2014} the link names a path that is not here",
        bridge::describe_path(target),
    ))
}

/// Cap markdown heading size to 1.15x body so rendered tour/narrative text stays compact.
fn set_markdown_text_sizes(ui: &mut egui::Ui) {
    let body_size = ui.text_style_height(&egui::TextStyle::Body);
    ui.style_mut().text_styles.insert(
        egui::TextStyle::Heading,
        egui::FontId::proportional(body_size * 1.15),
    );
}

const GOLDEN_RATIO: f32 = 0.618_034;

/// The colour for simulation series `i` — egui_plot's own auto-colour palette
/// (golden-ratio hue, `Hsva`), replicated so we can pin it explicitly. We must:
/// a variable plotted as several segments (broken at discontinuities) would else
/// take a different auto-colour per segment. Keyed on the variable index, this
/// equals the colour egui_plot picked when each variable was a single line.
///
/// The **golden-ratio hue trick**: multiplying the series index by the golden
/// ratio (0.618...) and using the fractional part as a hue angle produces
/// colors that are maximally spread around the color wheel. Each new series
/// lands as far as possible from all previous ones in hue space, giving
/// visually distinct colors without a hand-picked palette. `Hsva` constructs
/// a color from Hue/Saturation/Value/Alpha; egui wraps hue mod 1.0
/// automatically.
fn series_color(i: usize) -> egui::Color32 {
    egui::ecolor::Hsva::new(i as f32 * GOLDEN_RATIO, 0.85, 0.5, 1.0).into()
}

/// A stage-tab label, coloured by outcome so the whole pipeline's health reads off
/// the tab row without opening each stage: **red** if the stage errored, **green**
/// if it produced its IR (succeeded), and the normal colour for an
/// in-between/neutral status — "not reached" after an upstream failure, or no data
/// yet (before/while compiling).
///
/// `RichText` is egui's styled-text type: you create it with `RichText::new(…)`
/// and chain formatting methods (`.color()`, `.monospace()`, `.strong()`, etc.).
/// The resulting `RichText` can be passed anywhere a label/button expects text.
/// Here we use `.color()` to tint the tab label — the text itself is unchanged,
/// only its rendering color varies based on the stage's outcome.
fn tab_label(
    label: &str,
    stage: &Stage,
    ok_color: egui::Color32,
    err_color: egui::Color32,
) -> egui::RichText {
    let text = egui::RichText::new(label);
    if stage.note_is_error() {
        text.color(err_color)
    } else if stage.value.is_some() {
        text.color(ok_color)
    } else {
        // No color override — uses the theme's default text color. This
        // neutral state covers "not yet reached" (an upstream stage failed
        // or hasn't completed) and "still compiling".
        text
    }
}

#[cfg(test)]
impl App {
    /// pub(crate) so the headless UI tests in a sibling module can build an App.
    pub(crate) fn test_default() -> Self {
        Self::test_with_sender().0
    }

    // ---- Test-only accessors for the headless UI suite -------------------
    //
    // `app::tests` reaches `App`'s private fields because it is a child module.
    // `ui_tests` is a **sibling**, so it cannot — and the fix is not to widen the
    // fields. Production encapsulation is unchanged; these exist only under
    // `cfg(test)` and say so by name.

    pub(crate) fn test_stage(&self) -> StageKind {
        self.stage
    }

    pub(crate) fn test_model(&self) -> Option<&str> {
        self.model.as_deref()
    }

    pub(crate) fn test_selected_name(&self) -> Option<String> {
        self.selected.as_ref().map(|p| p.display().to_string())
    }

    pub(crate) fn test_selection_is_library(&self) -> bool {
        self.selected_is_library
    }

    pub(crate) fn test_set_filter(&mut self, s: &str) {
        self.list_filter = s.to_owned();
    }

    /// Populate the HRW specimen list without touching the filesystem.
    ///
    /// **Also parks the scratch poll**, which would otherwise fire on the first
    /// frame, see a bridge directory that disagrees with this injected list, and
    /// `rescan()` it straight back to empty from a `specimen_dir` no test sets.
    ///
    /// Not hypothetical: the first version of the divider tests passed
    /// **vacuously** because of it — "no specimen row is rendered" is true of a
    /// collapsed section and equally true of a list with nothing in it. The
    /// identical trap took the corpus test earlier the same day.
    /// Put a specimen's source on screen and aim a programmatic scroll at a line,
    /// without a compile.
    pub(crate) fn test_set_source(&mut self, text: &str, scroll_to_line: u32) {
        self.selected = Some(PathBuf::from("Fixture.mo"));
        self.cached_source = Some(text.to_owned());
        self.cached_highlight = None;
        self.source_scroll_target = Some(scroll_to_line);
        self.specimen_detail = SpecimenDetail::Source;
    }

    pub(crate) fn test_source_scroll_offset(&self) -> egui::Vec2 {
        self.source_scroll_offset
    }

    pub(crate) fn test_set_specimen_files(&mut self, names: &[&str]) {
        self.files = names.iter().map(PathBuf::from).collect();
        self.scratch_polled_at = Some(std::time::Instant::now());
    }

    pub(crate) fn test_set_ui_mode_specimen(&mut self) {
        self.ui_mode = UiMode::Specimen;
    }


    /// Drive a link the way a tour click would, without a rendered hyperlink.
    pub(crate) fn follow_link_for_test(&mut self, url: &str) {
        if let Some(link) = parse_hrw_link(url) {
            self.dispatch_hrw_link(link);
        }
    }

    /// Put the right-hand side into the state a walked-into tour would leave.
    pub(crate) fn test_set_walked_state(&mut self, specimen: &str, model: &str, stage: StageKind) {
        self.selected = Some(PathBuf::from(specimen));
        self.model = Some(model.to_owned());
        self.stage = stage;
    }

    /// Drop the model name, leaving the selection: the mid-compile state.
    pub(crate) fn test_clear_model(&mut self) {
        self.model = None;
    }

    fn test_with_sender() -> (Self, std::sync::mpsc::Sender<FromWorker>) {
        let (tx, _) = std::sync::mpsc::channel();
        let (from_tx, rx) = std::sync::mpsc::channel();
        let app = App {
            worker: Worker { tx, rx, send_failed: false },
            libraries_text: String::new(),
            library_status: String::new(),
            libraries_busy: false,
            specimen_dir: String::new(),
            files: Vec::new(),
            specimen_purposes: HashMap::new(),
            scratch_specimens: HashSet::new(),
            selected_is_library: false,
            corpus: None,
            list_filter: String::new(),
            scratch_arrived: false,
            shadowed_specimens: Vec::new(),
            scan_error: None,
            selected: None,
            compiling: false,
            model: None,
            stages: StageBundle::default(),
            stage: StageKind::Parse,
            def_index: BTreeMap::new(),
            nav: Vec::new(),
            nav_loading: None,
            nav_error: None,
            context_seq: 0,
            notice: None,
            ui_mode: UiMode::Tour,
            specimen_detail: SpecimenDetail::default(),
            show_settings: false,
            show_help: false,
            show_about: false,
            field_help: HashMap::new(),
            flatten_view: FlattenView::Equations,
            events_view: EventsView::default(),
            init_view: InitView::default(),
            highlighted_eq_row: None,
            highlighted_source_line: None,
            structural_view: StructuralView::SpyPlot,
            spy_canvas: Canvas::default().with_fit_vertical_bias(0.15),
            incidence_canvas: Canvas::default().with_fit_vertical_bias(0.15),
            log_entries: Vec::new(),
            viewing_log: false,

            tracing_enabled: false,
            simulation: Stage::default(),
            sim_data: None,
            sim_running: false,
            sim_error: None,
            sim_t_end: 2.0,
            cached_report_stage: None,
            cached_spy_plot: None,
            cached_incidence: None,
            cached_reduction: None,
            cached_equation_sheet: None,
            identifier_index: None,
            tracked_identifier: None,
            cached_matching_anim: None,
            matching_anim_canvas: Canvas::default().with_fit_vertical_bias(0.15),
            cached_tarjan_anim: None,
            tarjan_anim_canvas: Canvas::default().with_fit_vertical_bias(0.15),
            index_reduction_frames: Vec::new(),
            pre_lowering_frames: Vec::new(),
            connection_frames: Vec::new(),
            cached_flat: None,
            cached_pre_lowering_anim: None,
            cached_tearing_anim: None,
            cached_alias_anim: None,
            cached_ic_plan_anim: None,
            cached_connection_anim: None,
            cached_reduction_anim: None,
            cached_dae: None,
            cached_before_incidence: None,
            before_incidence_canvas: Canvas::default().with_fit_vertical_bias(0.15),
            commonmark_cache: egui_commonmark::CommonMarkCache::default(),
            problem_lines: Vec::new(),
            source_scroll_target: None,
            cached_tour: None,
            tours: Vec::new(),
            selected_tour: None,
            tour_polled_at: None,
            aim_at_equation: None,
            seek_frame: None,
            scratch_polled_at: None,
            cached_purpose_notes: HashMap::new(),
            cached_source: None,
            library_source_uri: None,
            library_source_error: None,
            source_scroll_offset: egui::Vec2::ZERO,
            cached_highlight: None,
            known_variables: None,
            declaring_classes: HashMap::new(),
            expand_trackable: false,
            pointed_at: None,
            point_error: None,
            tracking_summary: None,
            track_seq: 0,
            jump_matches: Vec::new(),
            jump_matches_key: None,
            jump_index: 0,
            jump_target: None,
            jump_highlight: None,
            scrolled_source_for: None,
            pending_live_debug: None,
            live_breakpoint_armed: false,
            pending_stage: None,
            pending_sub_view: None,
            // Tests drive `tick_prewarm` explicitly; nothing is armed for them.
            prewarm: Prewarm::NotStarted,
        };
        (app, from_tx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cycling, wrap-around, and the cache key that keeps "3 of 4" honest.
    ///
    /// The index is per (stage, followed name). Carrying it across a stage
    /// switch would leave it pointing into a list that no longer exists, and the
    /// counter would be describing a different set of matches than the arrows
    /// move through.
    #[test]
    fn jumping_cycles_within_the_current_stage_and_resets_across_stages() {
        let mut app = App::test_default();
        app.stages.flatten.value = Some(serde_json::json!({
            "variables": { "emf.w": 1 },
            "equations": [{ "text": "emf.w - der(emf.phi)" }, { "text": "emf.k * emf.w" }],
        }));
        app.stages.parse.value = Some(serde_json::json!({ "classes": { "M": { "name": "M" } } }));
        app.stage = StageKind::Flatten;
        app.tracked_identifier = Some("emf.w".to_owned());

        app.refresh_jump_matches();
        assert_eq!(app.jump_matches.len(), 3, "one key plus two equations");
        assert_eq!(app.jump_index, 0);

        // Forward through the list and around the end. Wrapping beats a dead
        // button: with a handful of matches, stopping is the worse surprise.
        app.jump_to_next_match(true);
        assert_eq!(app.jump_index, 1);
        app.jump_to_next_match(true);
        assert_eq!(app.jump_index, 2);
        app.jump_to_next_match(true);
        assert_eq!(app.jump_index, 0, "forward from the last match wraps");
        app.jump_to_next_match(false);
        assert_eq!(app.jump_index, 2, "and back again from the first");

        // The jump must have asked the tree for something, and must have left
        // the log view — the matches live in a stage IR, so a jump with the log
        // showing would look broken.
        assert!(app.jump_target.is_some());
        assert!(!app.viewing_log);

        // Switching stage rebuilds the list and restarts the cycle.
        app.stage = StageKind::Parse;
        app.refresh_jump_matches();
        assert!(app.jump_matches.is_empty(), "emf.w does not exist before flattening");
        assert_eq!(app.jump_index, 0, "a stale index would describe another stage's list");

        // Nothing to jump to is not an error; the control simply does nothing.
        app.jump_target = None;
        app.jump_to_next_match(true);
        assert!(app.jump_target.is_none());

        // And with nothing followed at all, the list empties rather than lingering.
        app.stage = StageKind::Flatten;
        app.tracked_identifier = None;
        app.refresh_jump_matches();
        assert!(app.jump_matches.is_empty());
    }

    /// The empty-context hint must name a gesture that is actually available.
    ///
    /// Regression for the exact state Doug hit: start HRW, switch to Specimen
    /// mode, load a specimen. The first version of the hint said "left-click a
    /// node to point at it, or right-click a variable name to follow it" and was
    /// wrong twice over — the log view was showing so there was no tree and no
    /// node, and the only clickable things on screen were source identifiers,
    /// which are LEFT-click-to-follow.
    ///
    /// A hint naming an unavailable gesture is worse than no hint, and it is the
    /// same defect the Context Bar exists to prevent: a confident statement that
    /// does not match the state.
    #[test]
    fn the_empty_hint_names_only_gestures_that_are_available() {
        let mut app = App::test_default();

        // The state Doug hit: specimen loaded, log showing, source on the left,
        // a compile finished so identifiers are underlined.
        app.ui_mode = UiMode::Specimen;
        app.specimen_detail = SpecimenDetail::Source;
        app.viewing_log = true;
        app.identifier_index = Some(identifier_index::IdentifierIndex::default());

        let hint = app.empty_context_hint();
        assert!(
            hint.contains("left-click an underlined identifier"),
            "must name the gesture that works here: {hint}",
        );
        assert!(
            !hint.contains("right-click"),
            "the source view has no context menu; naming one is the original bug: {hint}",
        );
        assert!(
            !hint.contains("a node to point at"),
            "no tree is showing, so there is no node to left-click: {hint}",
        );
        assert!(hint.contains("stage tab"), "the way to an IR view is a tab: {hint}");

        // Before the compile lands there is no index, so nothing is underlined
        // and that gesture must not be offered.
        app.identifier_index = None;
        let hint = app.empty_context_hint();
        assert!(!hint.contains("underlined"), "nothing is clickable yet: {hint}");

        // With a stage view open instead of the log, pointing is available.
        app.viewing_log = false;
        let hint = app.empty_context_hint();
        assert!(hint.contains("a node to point at"), "{hint}");
        assert!(!hint.contains("stage tab"), "a tab is already open: {hint}");
    }

    /// Recompiling the same specimen must not destroy the assembled context.
    ///
    /// The workflow this broke: point at a node, ask for breakpoints, then
    /// recompile to hit them — and the recompile wiped the very context that
    /// motivated the breakpoints. Doug hit it the first time the breakpoints
    /// actually fired.
    ///
    /// Switching to a *different* specimen must still clear, because a key-path
    /// addresses one model's IR and means nothing in another's.
    #[test]
    fn reselecting_the_same_specimen_keeps_the_context_but_switching_clears_it() {
        let (mut app, _tx) = App::test_with_sender();
        let motor = PathBuf::from("specimens/MotorWithBrake.mo");

        app.selected = Some(motor.clone());
        app.pointed_at = Some(PointedAt {
            seq: 1,
            target: "components.src.V".to_owned(),
            kind: PointKind::Stage,
            stage: StageKind::Flatten,
            request: bridge::AskRequest::Explain,
        });
        app.tracked_identifier = Some("emf.w".to_owned());

        app.open(motor.clone());
        assert!(app.pointed_at.is_some(), "a reselect must keep the point");
        assert_eq!(app.tracked_identifier.as_deref(), Some("emf.w"), "and the follow");

        // The jump list belonged to the old IR and must not be reused, even
        // though the stage and followed name are unchanged — which is exactly
        // the key `refresh_jump_matches` caches on.
        assert!(app.jump_matches_key.is_none(), "stale match list must be invalidated");

        app.open(PathBuf::from("specimens/BouncingBall.mo"));
        assert!(app.pointed_at.is_none(), "a different specimen clears the point");
        assert!(app.tracked_identifier.is_none(), "and the follow");
    }

    /// A retained point that no longer resolves is dropped, and says so.
    ///
    /// Keeping it would leave the Context Bar naming a node that does not exist
    /// and the emitted `node.subtree` as `null` — a confident claim about
    /// nothing. A stage point cannot dangle, so it survives.
    #[test]
    fn a_retained_point_that_no_longer_resolves_is_dropped_out_loud() {
        let (mut app, _tx) = App::test_with_sender();
        app.stages.flatten.value = Some(serde_json::json!({ "variables": { "emf.w": 1 } }));

        // Addresses something the new IR does not have.
        app.pointed_at = Some(PointedAt {
            seq: 1,
            target: "variables.gone".to_owned(),
            kind: PointKind::Node(vec![Seg::Key("variables".into()), Seg::Key("gone".into())]),
            stage: StageKind::Flatten,
            request: bridge::AskRequest::Explain,
        });
        app.revalidate_point_against_new_ir();
        assert!(app.pointed_at.is_none(), "a dangling point must not be kept");
        let notice = app.notice.as_deref().unwrap_or_default();
        assert!(notice.contains("point dropped"), "the drop must be stated: {notice}");
        assert!(notice.contains("variables.gone"), "and must name what was lost: {notice}");

        // One that still resolves survives untouched.
        app.notice = None;
        app.pointed_at = Some(PointedAt {
            seq: 2,
            target: "variables.emf.w".to_owned(),
            kind: PointKind::Node(vec![Seg::Key("variables".into()), Seg::Key("emf.w".into())]),
            stage: StageKind::Flatten,
            request: bridge::AskRequest::Explain,
        });
        app.revalidate_point_against_new_ir();
        assert!(app.pointed_at.is_some(), "a resolvable point survives a recompile");
        assert!(app.notice.is_none(), "and says nothing");

        // A stage point cannot dangle — there is always a stage.
        app.pointed_at = Some(PointedAt {
            seq: 3,
            target: "stage".to_owned(),
            kind: PointKind::Stage,
            stage: StageKind::Parse,
            request: bridge::AskRequest::Explain,
        });
        app.revalidate_point_against_new_ir();
        assert!(app.pointed_at.is_some(), "a stage point always resolves");
    }

    /// Every live-debug variant must be recognised by the arming machinery.
    ///
    /// Regression for the Debug button doing **nothing** on the `pre()`-lowering
    /// view. `live_debug_poll` and `is_arming` compared variants by a
    /// hand-written list of matching pairs — `(Matching, Matching) | (Tarjan,
    /// Tarjan) | (Reduction, Reduction)` — so a fourth variant compiled cleanly
    /// and silently never matched. No error, no arming badge, no session.
    ///
    /// Iterating `ALL` rather than naming variants keeps this honest: a fifth
    /// view added without touching the arming code still gets checked here,
    /// which is the point. Derived `PartialEq` is what makes it pass, but the
    /// test is what makes the next omission loud.
    #[test]
    fn every_live_debug_variant_is_recognised_while_arming() {
        for &variant in PendingLiveDebug::ALL {
            let mut app = App::test_default();
            assert!(!app.is_arming(variant), "{variant:?} must not arm on its own");

            app.pending_live_debug = Some((std::time::Instant::now(), variant));
            assert!(app.is_arming(variant), "{variant:?} armed but not recognised");

            // ...and must not be mistaken for any other view's session.
            for &other in PendingLiveDebug::ALL {
                if other != variant {
                    assert!(
                        !app.is_arming(other),
                        "{variant:?} armed, but {other:?} also reported arming",
                    );
                }
            }
        }
    }

    /// Every combination of the two primitives must be reachable, and the
    /// emitted file must describe each one honestly.
    ///
    /// Doug asked directly ("So there is now support for all combinations of
    /// context?") after the point became clearable. Reading the code says yes;
    /// this says yes and keeps saying it. Four states, and the two that used to
    /// be wrong are the ones with no point: **follow-only** emitted
    /// `kind: "stage"`, attributing a subject the user never chose, and
    /// **neither** was unreachable at all because the point could not be cleared.
    ///
    /// Reads `.hrw-bridge/focus.json` because the file is the artifact that
    /// matters — asserting on app fields would pass even if emission were broken,
    /// which is precisely the bar/file disagreement this design keeps hitting.
    #[test]
    fn every_point_and_follow_combination_emits_honestly() {
        fn emitted() -> Value {
            let path = std::path::Path::new(bridge::BRIDGE_DIR).join("focus.json");
            let text = std::fs::read_to_string(path).expect("focus.json should exist");
            serde_json::from_str(&text).expect("focus.json should be valid JSON")
        }
        fn a_point() -> PointedAt {
            PointedAt {
                seq: 1,
                target: "components.src.V".to_owned(),
                kind: PointKind::Stage,
                stage: StageKind::Flatten,
                request: bridge::AskRequest::Explain,
            }
        }

        let (mut app, _tx) = App::test_with_sender();

        // 1. Neither. Nothing is claimed at all.
        app.emit_context();
        let doc = emitted();
        assert_eq!(doc["kind"], serde_json::json!("none"), "no point must not become a stage");
        assert!(doc.get("tracking").is_none(), "nothing is being followed");
        // `request` belongs to the point. With no point, defaulting it to
        // "explain" would claim an intent the user never expressed — the same
        // species of phantom as the `kind: "stage"` this test was written for.
        assert!(doc["request"].is_null(), "no point means no request: {}", doc["request"]);

        // 2. Follow only — the state Doug wanted and could not reach.
        app.set_tracked_identifier("h".to_owned());
        let doc = emitted();
        assert_eq!(doc["kind"], serde_json::json!("none"));
        assert_eq!(doc["tracking"]["identifier"], serde_json::json!("h"));
        assert!(doc["request"].is_null(), "following carries no point-request either");

        // 3. Both, independent of each other.
        app.pointed_at = Some(a_point());
        app.emit_context();
        let doc = emitted();
        assert_eq!(doc["kind"], serde_json::json!("stage"));
        assert_eq!(doc["tracking"]["identifier"], serde_json::json!("h"));
        assert_eq!(doc["request"], serde_json::json!("explain"), "a point does carry one");

        // 4. Point only — reached by dropping the follow, which must not
        //    disturb the point.
        app.set_tracked_identifier("h".to_owned()); // toggles it off
        assert!(app.tracked_identifier.is_none(), "clicking the followed name again clears it");
        let doc = emitted();
        assert_eq!(doc["kind"], serde_json::json!("stage"));
        assert!(doc.get("tracking").is_none());
        assert!(app.pointed_at.is_some(), "dropping the follow must not drop the point");

        // ...and back to neither, by clearing the point.
        app.pointed_at = None;
        app.point_error = None;
        app.context_seq = app.next_seq();
        app.emit_context();
        let doc = emitted();
        assert_eq!(doc["kind"], serde_json::json!("none"));
        assert!(doc.get("tracking").is_none());
    }

    /// Following is context, so changing it must re-emit — and must not destroy
    /// the point. That independence is the property the Context Bar's honesty
    /// rests on.
    #[test]
    fn following_re_emits_without_losing_the_point() {
        let (mut app, _tx) = App::test_with_sender();
        app.pointed_at = Some(PointedAt {
            seq: 3,
            target: "components.src.V".to_owned(),
            kind: PointKind::Node(vec![Seg::Key("components".into())]),
            stage: StageKind::Flatten,
            request: bridge::AskRequest::Explain,
        });

        app.set_tracked_identifier("h".to_owned());
        assert_eq!(app.tracked_identifier.as_deref(), Some("h"));
        assert!(
            app.pointed_at.is_some(),
            "ambient following must not clear a deliberate capture"
        );
        assert_eq!(app.track_seq, 1, "the thread's own recency counter advanced");

        // Un-following also re-emits, and still leaves the point alone.
        app.set_tracked_identifier("h".to_owned());
        assert!(app.tracked_identifier.is_none());
        assert!(app.pointed_at.is_some());
        assert_eq!(app.track_seq, 2);
    }

    /// One counter for both halves, so the two stamps are comparable.
    ///
    /// Two independent counters *looked* comparable and were not: after twelve
    /// captures and one follow they read 12 and 1, and the emitted instructions
    /// told the reader to compare them. Found on the first real `explain`.
    #[test]
    fn point_and_thread_stamps_are_comparable() {
        let (mut app, _tx) = App::test_with_sender();

        app.emit_focus(Focus::Stage);
        let after_point = app.pointed_at.as_ref().unwrap().seq;

        app.set_tracked_identifier("h".to_owned());
        assert!(
            app.track_seq > after_point,
            "following happened later, so its stamp must be higher \
             (point {after_point}, thread {})",
            app.track_seq,
        );

        app.emit_focus(Focus::Stage);
        assert!(
            app.pointed_at.as_ref().unwrap().seq > app.track_seq,
            "pointing happened later, so now the point's stamp must be higher"
        );
    }

    /// Every capture shape is recorded, not just node captures.
    ///
    /// Clicking a stage tab emits a *stage* capture. That path used to write
    /// the file without updating `pointed_at`, so the emitted context changed
    /// while the Context Bar kept displaying the previous node — the exact
    /// drift the bar's rule forbids.
    #[test]
    fn stage_and_specimen_captures_are_recorded_too() {
        let (mut app, _tx) = App::test_with_sender();

        app.emit_focus(Focus::Stage);
        let point = app.pointed_at.as_ref().expect("a stage capture is still a point");
        assert!(matches!(point.kind, PointKind::Stage));
        assert!(point.target.contains("stage"));

        app.emit_focus(Focus::Specimen);
        assert!(matches!(
            app.pointed_at.as_ref().unwrap().kind,
            PointKind::Specimen
        ));
    }

    /// The bar reports the stage the capture was *made* in. Switching tabs
    /// afterwards must not change what it claims Claude has.
    #[test]
    fn the_point_remembers_its_own_stage() {
        let (mut app, _tx) = App::test_with_sender();
        app.stage = StageKind::Flatten;
        app.pointed_at = Some(PointedAt {
            seq: 1,
            target: "x".to_owned(),
            kind: PointKind::Stage,
            stage: StageKind::Flatten,
            request: bridge::AskRequest::Explain,
        });

        app.stage = StageKind::Structural;
        assert_eq!(
            app.pointed_at.as_ref().unwrap().stage,
            StageKind::Flatten,
            "the captured stage is a property of the capture, not of the view"
        );
    }

    /// The pre-warm state machine: arm → await ack → remove, and — critically —
    /// abandon *without consuming the ack* if a Debug click takes over.
    ///
    /// Both paths live in one test because they share the single
    /// `.hrw-bridge/breakpoint-{request,ack}.json` pair; as separate tests they
    /// would race each other (the same reason `bridge`'s arm/remove/ack test is
    /// combined).
    #[test]
    fn prewarm_arms_awaits_ack_then_removes() {
        let ctx = egui::Context::default();
        let (mut app, _tx) = App::test_with_sender();
        let ack = std::path::Path::new(bridge::BREAKPOINT_ACK_FILE);
        let _ = std::fs::remove_file(ack);

        assert_eq!(app.prewarm, Prewarm::NotStarted);

        // First tick writes the arm request and begins waiting for the ack.
        app.tick_prewarm(&ctx);
        assert!(
            matches!(app.prewarm, Prewarm::Awaiting(_)),
            "first tick should arm and wait, got {:?}", app.prewarm
        );

        // Without an ack it keeps waiting (the 3s timeout has not elapsed).
        app.tick_prewarm(&ctx);
        assert!(matches!(app.prewarm, Prewarm::Awaiting(_)), "should still be waiting");

        // The extension acks; the next tick removes the breakpoint and finishes.
        std::fs::write(ack, r#"{"acked":true}"#).unwrap();
        app.tick_prewarm(&ctx);
        assert_eq!(app.prewarm, Prewarm::Done, "ack should complete the pre-warm");
        assert!(!ack.exists(), "pre-warm consumes its own ack");

        // --- Abandon path: a Debug click owns the handshake mid-pre-warm. ---
        app.prewarm = Prewarm::NotStarted;
        app.tick_prewarm(&ctx);
        assert!(matches!(app.prewarm, Prewarm::Awaiting(_)));

        std::fs::write(ack, r#"{"acked":true}"#).unwrap();
        app.pending_live_debug = Some((
            std::time::Instant::now(),
            PendingLiveDebug::Reduction,
        ));
        app.tick_prewarm(&ctx);

        assert_eq!(app.prewarm, Prewarm::Done, "should abandon, not keep polling");
        assert!(
            ack.exists(),
            "abandoning must NOT consume the ack — the Debug click is waiting for it"
        );

        let _ = std::fs::remove_file(ack);
    }

    /// `src.V` is not declared in the specimen — it is a parameter of `src`'s
    /// type. Resolving the component gives the class that declares it, which
    /// turns "not declared in this specimen" into a navigable answer.
    #[test]
    fn declaring_classes_resolves_a_component_type() {
        use crate::equation_sheet::{ClassifiedVariable, EquationSheet};

        let stages = StageBundle {
            resolve: Stage::ok(serde_json::json!({
                "components": {
                    "src": { "type_def_id": 6005 },
                    "plain": { "type_def_id": 4047 },
                }
            })),
            ..Default::default()
        };
        let mut def_index = BTreeMap::new();
        def_index.insert(6005u64, DefInfo {
            name: "Modelica.Electrical.Analog.Sources.ConstantVoltage".to_owned(),
            kind: DefKind::Class,
            class_type: Some("model".to_owned()),
            file_name: None,
            line: None,
        });
        // A non-class definition must not be offered as a declaring class.
        def_index.insert(4047u64, DefInfo {
            name: "Modelica.Units.SI.Voltage".to_owned(),
            kind: DefKind::Definition,
            class_type: None,
            file_name: None,
            line: None,
        });

        let var = |name: &str| ClassifiedVariable {
            name: name.to_owned(), kind: "parameter",
            unit: None, description: None, start: None,
        };
        let sheet = EquationSheet {
            variables: vec![var("src.V"), var("plain.x"), var("h"), var("nosuch.y")],
            ..Default::default()
        };

        let map = App::build_declaring_classes(&stages, &def_index, Some(&sheet));
        assert_eq!(
            map.get("src.V").map(String::as_str),
            Some("Modelica.Electrical.Analog.Sources.ConstantVoltage")
        );
        assert!(!map.contains_key("plain.x"), "a non-class DefId is not a declaring class");
        assert!(!map.contains_key("h"), "an unqualified name has no component to resolve");
        assert!(!map.contains_key("nosuch.y"), "unknown components resolve to nothing");
    }

    /// Tracking is a toggle from every view, and derivative mentions resolve to
    /// the base variable before being stored.
    /// The point must be clearable, so "explain only what I am following" is
    /// askable.
    ///
    /// Found by Doug in testing: the Following row had a clear button and the
    /// Pointing at row did not, so a point could only ever be *replaced*. The
    /// sole escape was reloading the specimen, which recompiles and discards
    /// everything.
    ///
    /// The emitted `kind` is the load-bearing half. Clearing must NOT fall back
    /// to `Focus::Stage`: "pointing at the Typecheck stage as a whole" is a
    /// claim the user makes by clicking a tab, and attributing it to someone who
    /// pointed at nothing is the confident lie this design exists to prevent.
    #[test]
    fn clearing_the_point_emits_nothing_not_the_current_stage() {
        let empty = BTreeMap::new();
        let stages: [(&str, Option<&Value>); 0] = [];
        let ask = Ask {
            seq: 3,
            request: bridge::AskRequest::Explain,
            specimen: None,
            model: Some("MotorWithBrake"),
            // A stage is still reported — it is where the user is looking —
            // but it must not become the subject.
            stage: Some(StageKind::Typecheck),
            libraries: vec![],
            def_index: &empty,
            parse_value: None,
            resolve_value: None,
            focus: Focus::Nothing,
            tracking: Some(bridge::Tracking {
                seq: 4,
                name: "emf.w",
                declared_line: None,
                declaring_class: None,
                stage_values: &stages,
            }),
            view: bridge::View {
                ui_mode: "Specimen",
                stage_view: None,
                specimen_detail: None,
                viewing_log: false,
                animation: None,
            },
            failure: None,
        };
        let doc = bridge::build_for_test(&ask);

        assert_eq!(doc["kind"], serde_json::json!("none"), "absence must be stated, not implied");
        assert!(doc.get("node").is_none(), "there is no node to describe");
        assert!(doc.get("cross_stage").is_none());
        assert_eq!(doc["tracking"]["identifier"], serde_json::json!("emf.w"));
        assert!(
            doc["instructions"].as_str().is_some_and(|i| i.contains("kind: \"none\"")),
            "the file must explain what `none` means to whoever reads it",
        );
    }

    #[test]
    fn set_tracked_identifier_toggles_and_strips_der() {
        let (mut app, _tx) = App::test_with_sender();

        app.set_tracked_identifier("h".to_owned());
        assert_eq!(app.tracked_identifier.as_deref(), Some("h"));

        // Clicking the same name again clears it.
        app.set_tracked_identifier("h".to_owned());
        assert_eq!(app.tracked_identifier, None);

        // A derivative mention tracks the variable it differentiates...
        app.set_tracked_identifier("der(h)".to_owned());
        assert_eq!(app.tracked_identifier.as_deref(), Some("h"));
        // ...and so clicking `h` elsewhere is recognised as the same thing.
        app.set_tracked_identifier("h".to_owned());
        assert_eq!(app.tracked_identifier, None, "der(h) and h are one target");
    }

    /// The source view scrolls on *change* only. Without this the view would be
    /// re-centred every frame while an identifier stayed tracked, pinning it and
    /// making the scrollbar unusable.
    #[test]
    fn source_scroll_is_armed_only_when_the_tracked_identifier_changes() {
        let (mut app, _tx) = App::test_with_sender();
        assert_eq!(app.scrolled_source_for, None);

        app.set_tracked_identifier("h".to_owned());
        assert_ne!(
            app.tracked_identifier, app.scrolled_source_for,
            "a newly tracked identifier must still be pending a scroll"
        );

        // Simulate the source view having scrolled to it.
        app.scrolled_source_for = app.tracked_identifier.clone();
        assert_eq!(
            app.tracked_identifier, app.scrolled_source_for,
            "once scrolled, no further scroll is armed for the same identifier"
        );
    }

    #[test]
    fn read_purpose_extracts_hint() {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let path = dir.join("specimens/BouncingBall.mo");
        let purpose = read_purpose(&path);
        assert!(purpose.is_some(), "BouncingBall should have a // purpose: comment");
        let text = purpose.unwrap();
        assert!(!text.is_empty());
        assert!(text.to_lowercase().contains("event"), "purpose should mention events: {text}");
    }

    #[test]
    fn read_purpose_returns_none_for_missing_file() {
        let purpose = read_purpose(Path::new("/nonexistent/specimen.mo"));
        assert!(purpose.is_none());
    }

    #[test]
    fn every_specimen_has_a_purpose_comment() {
        let dir = std::path::PathBuf::from(format!("{}/specimens", env!("CARGO_MANIFEST_DIR")));
        let entries: Vec<_> = std::fs::read_dir(&dir)
            .expect("read specimens dir")
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "mo"))
            .collect();
        assert!(!entries.is_empty(), "no .mo files found in specimens/");
        for entry in entries {
            let path = entry.path();
            let name = path.file_stem().unwrap().to_str().unwrap();
            assert!(
                read_purpose(&path).is_some(),
                "specimen {name} is missing a // purpose: comment"
            );
        }
    }

    fn make_app_with_stages(ok_through: StageKind) -> App {
        let ok_stage = Stage::ok(serde_json::json!({}));
        let empty = Stage::default();
        let stages_in_order = [
            StageKind::Parse, StageKind::Resolve, StageKind::Instantiate,
            StageKind::Typecheck, StageKind::Flatten, StageKind::Structural,
            StageKind::IndexReduction, StageKind::Initialization,
            StageKind::Events, StageKind::SolveLowering,
        ];
        let cutoff = stages_in_order.iter().position(|&s| s == ok_through).unwrap_or(0);
        let mut bundle = StageBundle::default();
        for (i, &kind) in stages_in_order.iter().enumerate() {
            let stage = if i <= cutoff { ok_stage.clone() } else { empty.clone() };
            match kind {
                StageKind::Parse => bundle.parse = stage,
                StageKind::Resolve => bundle.resolve = stage,
                StageKind::Instantiate => bundle.instantiate = stage,
                StageKind::Typecheck => bundle.typecheck = stage,
                StageKind::Flatten => bundle.flatten = stage,
                StageKind::Structural => bundle.structural = stage,
                StageKind::IndexReduction => bundle.index_reduction = stage,
                StageKind::Initialization => bundle.initialization = stage,
                StageKind::Events => bundle.events = stage,
                StageKind::SolveLowering => bundle.solve_lowering = stage,
                StageKind::Simulation => {}
            }
        }
        App { stages: bundle, ..App::test_default() }
    }

    #[test]
    fn last_successful_stage_selects_furthest_ok() {
        let app = make_app_with_stages(StageKind::Flatten);
        assert_eq!(app.last_successful_stage(), StageKind::Flatten);
    }

    #[test]
    fn last_successful_stage_falls_back_to_parse() {
        let app = App { stages: StageBundle::default(), ..App::test_default() };
        assert_eq!(app.last_successful_stage(), StageKind::Parse);
    }

    #[test]
    fn last_successful_stage_skips_errored() {
        let mut app = make_app_with_stages(StageKind::Structural);
        app.stages.structural = Stage::recovered(serde_json::json!({}), "singular");
        assert_eq!(app.last_successful_stage(), StageKind::Flatten);
    }

    #[test]
    fn previous_stage_value_parse_is_none() {
        let mut app = make_app_with_stages(StageKind::SolveLowering);
        app.stage = StageKind::Parse;
        assert!(app.previous_stage_value().is_none());
    }

    #[test]
    fn previous_stage_value_instantiate_returns_resolve() {
        let mut app = make_app_with_stages(StageKind::SolveLowering);
        app.stage = StageKind::Instantiate;
        assert!(app.previous_stage_value().is_some());
    }

    #[test]
    fn stage_name_exhaustive() {
        let all = [
            StageKind::Parse, StageKind::Resolve, StageKind::Instantiate,
            StageKind::Typecheck, StageKind::Flatten, StageKind::Structural,
            StageKind::IndexReduction, StageKind::Initialization,
            StageKind::Events, StageKind::SolveLowering, StageKind::Simulation,
        ];
        for kind in all {
            let name = kind.name();
            assert!(!name.is_empty(), "{kind:?} has an empty name");
        }
    }

    #[test]
    fn parse_library_paths_splits_lines() {
        let mut app = App::test_default();
        app.libraries_text = "/path/one\n/path/two\n".to_owned();
        let paths = app.parse_library_paths();
        assert_eq!(paths, vec![PathBuf::from("/path/one"), PathBuf::from("/path/two")]);
    }

    #[test]
    fn parse_library_paths_trims_whitespace() {
        let mut app = App::test_default();
        app.libraries_text = "  /trimmed  \n".to_owned();
        let paths = app.parse_library_paths();
        assert_eq!(paths, vec![PathBuf::from("/trimmed")]);
    }

    #[test]
    fn parse_library_paths_skips_blank_lines() {
        let mut app = App::test_default();
        app.libraries_text = "/first\n\n  \n/last\n".to_owned();
        let paths = app.parse_library_paths();
        assert_eq!(paths, vec![PathBuf::from("/first"), PathBuf::from("/last")]);
    }

    #[test]
    fn parse_library_paths_empty_text() {
        let app = App::test_default();
        assert!(app.parse_library_paths().is_empty());
    }

    /// A successful point says nothing in the status bar.
    ///
    /// The Context Bar names the point persistently; a second, transient
    /// description of the same thing could only go stale and disagree with it.
    /// Two places claiming to describe what Claude has is the failure mode this
    /// design keeps running into, and the weaker one is the one to drop.
    #[test]
    fn a_successful_point_is_silent() {
        let msg = status_line(1, "equations.3.lhs", "explain", Ok(PathBuf::from("/tmp/focus.json")));
        assert_eq!(msg, None, "the Context Bar states this; the status bar must not repeat it");
    }

    /// The debugger request still speaks, because it asks for something next.
    /// An instruction is not a confirmation.
    #[test]
    fn a_debug_point_still_tells_the_user_what_to_do() {
        let msg = status_line(2, "def_id", "debug-where-set", Ok(PathBuf::from("/tmp/f.json")))
            .expect("debug requests carry an instruction");
        assert!(msg.contains("debugger"), "debug request should mention debugger: {msg}");
        assert!(msg.contains("context #2"), "should carry the shared counter: {msg}");
    }

    /// A failure must still be stated. This is the one case the Context Bar
    /// cannot cover alone — it renders the point either way, so silence would
    /// leave it describing context that was never written.
    #[test]
    fn a_failed_emission_is_never_silent() {
        let err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
        let msg = status_line(1, "x", "explain", Err(err)).expect("failures are always reported");
        assert!(msg.contains("not emitted"), "should say it was not emitted: {msg}");
        assert!(msg.contains("denied"), "should carry the cause: {msg}");
    }

    #[test]
    fn report_cache_invalidated_on_stage_switch() {
        let mut app = App::test_default();
        // Simulate having cached data for Structural.
        app.cached_report_stage = Some(StageKind::Structural);
        app.cached_spy_plot = Some(None);
        app.cached_incidence = Some(None);
        app.cached_reduction = Some(None);
        app.cached_matching_anim = Some(None);
        app.cached_tarjan_anim = Some(None);

        // Switch to IndexReduction — caches should be invalidated.
        app.stage = StageKind::IndexReduction;
        if app.cached_report_stage != Some(app.stage) {
            app.cached_spy_plot = None;
            app.cached_incidence = None;
            app.cached_reduction = None;
            app.cached_matching_anim = None;
            app.cached_tarjan_anim = None;
            app.cached_report_stage = Some(app.stage);
        }
        assert!(app.cached_spy_plot.is_none());
        assert!(app.cached_incidence.is_none());
        assert!(app.cached_reduction.is_none());
        assert!(app.cached_matching_anim.is_none());
        assert!(app.cached_tarjan_anim.is_none());
        assert_eq!(app.cached_report_stage, Some(StageKind::IndexReduction));
    }

    #[test]
    fn report_cache_preserved_for_same_stage() {
        let mut app = App::test_default();
        app.cached_report_stage = Some(StageKind::Structural);
        app.cached_spy_plot = Some(None);
        app.stage = StageKind::Structural;

        // Same stage — cache should NOT be invalidated.
        if app.cached_report_stage != Some(app.stage) {
            app.cached_spy_plot = None;
            app.cached_report_stage = Some(app.stage);
        }
        assert!(app.cached_spy_plot.is_some());
    }

    #[test]
    fn drain_worker_libraries_ok_updates_status() {
        let (mut app, tx) = App::test_with_sender();
        app.libraries_busy = true;
        tx.send(FromWorker::Libraries(Ok(3))).unwrap();
        app.drain_worker();
        assert!(!app.libraries_busy);
        assert!(app.library_status.contains("3"));
    }

    #[test]
    fn drain_worker_libraries_err_updates_status() {
        let (mut app, tx) = App::test_with_sender();
        app.libraries_busy = true;
        tx.send(FromWorker::Libraries(Err("boom".into()))).unwrap();
        app.drain_worker();
        assert!(!app.libraries_busy);
        assert!(app.library_status.contains("boom"));
    }

    #[test]
    fn drain_worker_compile_progress_updates_stages_for_current_specimen() {
        let (mut app, tx) = App::test_with_sender();
        let path = PathBuf::from("/test/specimen.mo");
        app.selected = Some(path.clone());
        let stages = StageBundle {
            parse: Stage::ok(serde_json::json!({"parsed": true})),
            ..Default::default()
        };
        tx.send(FromWorker::CompileProgress { path, stages }).unwrap();
        app.drain_worker();
        assert!(app.stages.parse.value.is_some());
    }

    #[test]
    fn drain_worker_compile_progress_ignored_for_stale_specimen() {
        let (mut app, tx) = App::test_with_sender();
        app.selected = Some(PathBuf::from("/test/current.mo"));
        let stages = StageBundle {
            parse: Stage::ok(serde_json::json!({"parsed": true})),
            ..Default::default()
        };
        tx.send(FromWorker::CompileProgress { path: PathBuf::from("/test/stale.mo"), stages }).unwrap();
        app.drain_worker();
        assert!(app.stages.parse.value.is_none());
    }

    #[test]
    fn drain_worker_compiled_clears_caches_and_updates_state() {
        let (mut app, tx) = App::test_with_sender();
        let path = PathBuf::from("/test/specimen.mo");
        app.selected = Some(path.clone());
        app.compiling = true;
        app.cached_spy_plot = Some(None);
        app.cached_incidence = Some(None);
        app.cached_report_stage = Some(StageKind::Parse);
        app.live_breakpoint_armed = false;

        let stages = StageBundle {
            parse: Stage::ok(serde_json::json!({})),
            ..Default::default()
        };
        tx.send(FromWorker::Compiled {
            path,
            model: Some("TestModel".into()),
            stages,
            def_index: BTreeMap::new(),
            equation_sheet: None,
            identifier_index: None,
            index_reduction_frames: Vec::new(),
            pre_lowering_frames: Vec::new(),
            connection_frames: Vec::new(),
            flat: None,
            dae: None,
            library_source: None,
        }).unwrap();
        app.drain_worker();

        assert!(!app.compiling);
        assert_eq!(app.model.as_deref(), Some("TestModel"));
        assert!(app.cached_spy_plot.is_none());
        assert!(app.cached_incidence.is_none());
        assert!(app.cached_report_stage.is_none());
        assert!(app.pending_live_debug.is_none());
    }

    #[test]
    fn drain_worker_compiled_stale_path_ignored() {
        let (mut app, tx) = App::test_with_sender();
        app.selected = Some(PathBuf::from("/test/current.mo"));
        app.compiling = true;

        tx.send(FromWorker::Compiled {
            path: PathBuf::from("/test/stale.mo"),
            model: Some("StaleModel".into()),
            stages: StageBundle::default(),
            def_index: BTreeMap::new(),
            equation_sheet: None,
            identifier_index: None,
            index_reduction_frames: Vec::new(),
            pre_lowering_frames: Vec::new(),
            connection_frames: Vec::new(),
            flat: None,
            dae: None,
            library_source: None,
        }).unwrap();
        app.drain_worker();

        assert!(app.compiling);
        assert!(app.model.is_none());
    }

    #[test]
    fn drain_worker_compiled_applies_pending_stage() {
        let (mut app, tx) = App::test_with_sender();
        let path = PathBuf::from("/test/specimen.mo");
        app.selected = Some(path.clone());
        app.compiling = true;
        app.pending_stage = Some(StageKind::Flatten);

        let stages = StageBundle {
            parse: Stage::ok(serde_json::json!({})),
            resolve: Stage::ok(serde_json::json!({})),
            ..Default::default()
        };
        tx.send(FromWorker::Compiled {
            path,
            model: Some("TestModel".into()),
            stages,
            def_index: BTreeMap::new(),
            equation_sheet: None,
            identifier_index: None,
            index_reduction_frames: Vec::new(),
            pre_lowering_frames: Vec::new(),
            connection_frames: Vec::new(),
            flat: None,
            dae: None,
            library_source: None,
        }).unwrap();
        app.drain_worker();

        assert_eq!(app.stage, StageKind::Flatten, "pending_stage should override last_successful_stage");
        assert!(app.pending_stage.is_none(), "pending_stage should be consumed");
        assert!(!app.viewing_log, "viewing_log should be cleared after compilation");
    }

    #[test]
    fn drain_worker_compiled_falls_back_without_pending_stage() {
        let (mut app, tx) = App::test_with_sender();
        let path = PathBuf::from("/test/specimen.mo");
        app.selected = Some(path.clone());
        app.compiling = true;
        app.pending_stage = None;

        let stages = StageBundle {
            parse: Stage::ok(serde_json::json!({})),
            resolve: Stage::ok(serde_json::json!({})),
            ..Default::default()
        };
        tx.send(FromWorker::Compiled {
            path,
            model: Some("TestModel".into()),
            stages,
            def_index: BTreeMap::new(),
            equation_sheet: None,
            identifier_index: None,
            index_reduction_frames: Vec::new(),
            pre_lowering_frames: Vec::new(),
            connection_frames: Vec::new(),
            flat: None,
            dae: None,
            library_source: None,
        }).unwrap();
        app.drain_worker();

        assert_eq!(app.stage, StageKind::Resolve, "should fall back to last_successful_stage");
    }

    #[test]
    fn drain_worker_compiled_preserves_log_view() {
        let (mut app, tx) = App::test_with_sender();
        let path = PathBuf::from("/test/specimen.mo");
        app.selected = Some(path.clone());
        app.compiling = true;
        app.viewing_log = true;

        let stages = StageBundle {
            parse: Stage::ok(serde_json::json!({})),
            resolve: Stage::ok(serde_json::json!({})),
            ..Default::default()
        };
        tx.send(FromWorker::Compiled {
            path,
            model: Some("TestModel".into()),
            stages,
            def_index: BTreeMap::new(),
            equation_sheet: None,
            identifier_index: None,
            index_reduction_frames: Vec::new(),
            pre_lowering_frames: Vec::new(),
            connection_frames: Vec::new(),
            flat: None,
            dae: None,
            library_source: None,
        }).unwrap();
        app.drain_worker();

        assert!(app.viewing_log, "should not yank user off the Log tab");
        assert_eq!(app.stage, StageKind::Resolve, "stage should still be updated for when user clicks away");
    }

    #[test]
    fn drain_worker_simulated_ok_stores_data() {
        let (mut app, tx) = App::test_with_sender();
        let path = PathBuf::from("/test/specimen.mo");
        app.selected = Some(path.clone());
        app.sim_running = true;

        tx.send(FromWorker::Simulated {
            path,
            result: Ok(SimData {
                times: vec![0.0, 1.0],
                names: vec!["x".into()],
                data: vec![vec![0.0, 1.0]],
                n_states: 1,
                has_discontinuities: false,
                solver_steps: vec![],
            }),
        }).unwrap();
        app.drain_worker();

        assert!(!app.sim_running);
        assert!(app.sim_data.is_some());
        assert!(app.sim_error.is_none());
    }

    #[test]
    fn drain_worker_simulated_err_stores_error() {
        let (mut app, tx) = App::test_with_sender();
        let path = PathBuf::from("/test/specimen.mo");
        app.selected = Some(path.clone());
        app.sim_running = true;

        tx.send(FromWorker::Simulated {
            path,
            result: Err("solver diverged".into()),
        }).unwrap();
        app.drain_worker();

        assert!(!app.sim_running);
        assert!(app.sim_data.is_none());
        assert!(app.sim_error.as_deref() == Some("solver diverged"));
    }

    #[test]
    fn drain_worker_log_appends_entry() {
        let (mut app, tx) = App::test_with_sender();
        tx.send(FromWorker::Log(LogEntry {
            elapsed_secs: 0.0,
            level: crate::worker::LogLevel::Info,
            message: "test log".into(),
        })).unwrap();
        app.drain_worker();
        assert_eq!(app.log_entries.len(), 1);
        assert!(app.log_entries[0].message.contains("test log"));
    }

    #[test]
    fn parse_hrw_link_load_specimen() {
        let link = parse_hrw_link("hrw://load/BouncingBall");
        assert!(matches!(link, Some(HrwLink::LoadSpecimen(ref s)) if s == "BouncingBall"));
    }

    #[test]
    fn parse_hrw_link_switch_stage() {
        let link = parse_hrw_link("hrw://stage/Structural");
        assert!(matches!(link, Some(HrwLink::SwitchStage(StageKind::Structural, None))));
    }

    #[test]
    fn parse_hrw_link_load_and_switch() {
        let link = parse_hrw_link("hrw://load/GearWithBrake/Parse");
        assert!(matches!(link, Some(HrwLink::LoadAndSwitch(ref s, StageKind::Parse, None)) if s == "GearWithBrake"));
    }

    #[test]
    fn parse_hrw_link_invalid_stage() {
        assert!(parse_hrw_link("hrw://stage/Bogus").is_none());
    }

    #[test]
    fn parse_hrw_link_not_hrw_scheme() {
        assert!(parse_hrw_link("https://example.com").is_none());
    }

    #[test]
    fn extract_hrw_links_from_markdown() {
        let md = "Click [here](hrw://load/Foo) or [there](hrw://stage/Parse) end.";
        let links = extract_hrw_links(md);
        assert_eq!(links, vec!["hrw://load/Foo", "hrw://stage/Parse"]);
    }

    #[test]
    fn extract_hrw_links_deduplicates() {
        let md = "[a](hrw://load/X) and [b](hrw://load/X) again.";
        let links = extract_hrw_links(md);
        assert_eq!(links.len(), 1);
    }

    #[test]
    fn stage_kind_from_slug_round_trips() {
        for kind in StageKind::ALL {
            let slug = kind.slug();
            assert_eq!(StageKind::from_slug(slug), Some(*kind));
        }
    }

    /// A scratch specimen is listed, marked, and findable by name (ideas #42).
    ///
    /// The point of the split is that Claude can write "here is the smallest model
    /// that shows the thing you asked about" and have it appear in HRW without
    /// touching the curated corpus — whose portable-subset, `// purpose:` and
    /// System-Modeler-round-trip properties a disposable probe would degrade.
    #[test]
    fn a_scratch_specimen_is_listed_and_marked() {
        let mut app = App::test_default();
        app.specimen_dir = DEFAULT_SPECIMEN_DIR.to_owned();
        app.rescan();

        let probe = std::path::Path::new(crate::bridge::SCRATCH_SPECIMEN_DIR)
            .join("ScratchProbe.mo");
        if !probe.exists() {
            return; // no probe written in this checkout
        }
        assert!(app.files.contains(&probe), "scratch specimens join the list");
        // Scratch sorts FIRST, matching the tour list: the just-written thing is the
        // one most likely wanted next, and burying it under 18 curated specimens made
        // the common case the awkward one.
        assert_eq!(
            app.files.first(),
            Some(&probe),
            "scratch specimens lead the list: {:?}",
            app.files.iter().take(3).collect::<Vec<_>>(),
        );
        assert!(app.scratch_specimens.contains(&probe), "and are marked as scratch");
        assert_eq!(
            app.find_specimen("ScratchProbe"),
            Some(probe),
            "and are reachable by name, so `hrw://load/ScratchProbe` works",
        );

        // The curated corpus is untouched and still unmarked.
        let curated = app
            .files
            .iter()
            .find(|p| p.file_name().and_then(|n| n.to_str()) == Some("BouncingBall.mo"))
            .expect("BouncingBall is curated");
        assert!(!app.scratch_specimens.contains(curated));
    }

    /// A scratch specimen may not shadow a curated one.
    ///
    /// Loading a different model than the name says is the "makes Claude guess"
    /// failure: Claude would reason confidently about source Doug is not looking at.
    /// So the collision is reported and the scratch file skipped, rather than either
    /// one silently winning.
    #[test]
    fn a_scratch_specimen_cannot_shadow_a_curated_one() {
        let dir = std::path::Path::new(crate::bridge::SCRATCH_SPECIMEN_DIR);
        if std::fs::create_dir_all(dir).is_err() {
            return;
        }
        let clash = dir.join("BouncingBall.mo");
        if std::fs::write(&clash, "model BouncingBall end BouncingBall;
").is_err() {
            return;
        }

        let mut app = App::test_default();
        app.specimen_dir = DEFAULT_SPECIMEN_DIR.to_owned();
        app.rescan();

        assert!(
            app.shadowed_specimens.iter().any(|n| n == "BouncingBall.mo"),
            "the collision is reported: {:?}",
            app.shadowed_specimens,
        );
        assert!(!app.scratch_specimens.contains(&clash), "and the scratch file is skipped");
        // The curated one still wins, and is what the name resolves to.
        let found = app.find_specimen("BouncingBall").expect("still findable");
        assert!(found.starts_with(DEFAULT_SPECIMEN_DIR), "curated wins: {found:?}");

        let _ = std::fs::remove_file(&clash);
    }

    /// The Purpose tab's placeholder never says "narrative", and never tells Doug to
    /// select a specimen he has already selected.
    ///
    /// Both were real bugs he hit by using the app (2026-07-29). The second is the
    /// interesting one: it was a **missing state**, not a typo. The note is keyed on
    /// the model name, which is unknown until compilation finishes, so selecting a
    /// second specimen briefly showed "Select a specimen to see its narrative" — advice
    /// to do the thing just done.
    #[test]
    fn the_purpose_placeholder_fits_the_actual_state() {
        let path = std::path::Path::new("/x/CapacitorLoop.mo");

        // Compiled, no note: says so, and says where one would go.
        let compiled = purpose_placeholder(Some("CapacitorLoop"), Some(path));
        assert!(compiled[0].contains("No purpose note for CapacitorLoop"), "{compiled:?}");
        assert!(
            compiled.iter().any(|l| l.contains("docs/specimen-notebook/CapacitorLoop/purpose.md")),
            "the absence must be actionable: {compiled:?}",
        );

        // Selected but still compiling: names the file, does NOT ask for a selection.
        let compiling = purpose_placeholder(None, Some(path));
        assert!(compiling[0].contains("Compiling CapacitorLoop"), "{compiling:?}");
        assert!(
            !compiling.iter().any(|l| l.contains("Select a specimen")),
            "must not advise selecting a specimen that IS selected: {compiling:?}",
        );

        // Genuinely nothing selected: the advice is now correct.
        let idle = purpose_placeholder(None, None);
        assert!(idle[0].contains("Select a specimen"), "{idle:?}");

        // No state mentions the retired term.
        for lines in [compiled, compiling, idle] {
            for l in lines {
                assert!(
                    !l.to_lowercase().contains("narrative"),
                    "retired term leaked into user-visible text: {l}",
                );
            }
        }
    }

    #[test]
    fn find_specimen_matches_by_filename() {
        let mut app = App::test_default();
        app.files = vec![
            PathBuf::from("/specimens/BouncingBall.mo"),
            PathBuf::from("/specimens/Drivetrain.mo"),
        ];
        assert_eq!(
            app.find_specimen("BouncingBall"),
            Some(PathBuf::from("/specimens/BouncingBall.mo"))
        );
    }

    #[test]
    fn find_specimen_returns_none_for_missing() {
        let mut app = App::test_default();
        app.files = vec![PathBuf::from("/specimens/BouncingBall.mo")];
        assert!(app.find_specimen("NonExistent").is_none());
    }

    #[test]
    fn find_specimen_does_not_match_substring() {
        let mut app = App::test_default();
        app.files = vec![PathBuf::from("/specimens/BouncingBall.mo")];
        assert!(app.find_specimen("Bouncing").is_none());
    }

    /// A link can point at the source, with or without a line.
    ///
    /// Closes the second quiet tour hole: two tours *quoted* a source line because
    /// nothing could point at one.
    #[test]
    fn a_link_can_point_at_a_source_line() {
        assert_eq!(parse_hrw_link("hrw://source/9"), Some(HrwLink::ShowSource(Some(9))));
        assert_eq!(parse_hrw_link("hrw://source"), Some(HrwLink::ShowSource(None)));
        // A non-numeric line is malformed, not line 0 and not "the whole file".
        assert!(parse_hrw_link("hrw://source/nine").is_none());
    }

    /// **A high-index model must never have its source blamed.**
    ///
    /// This is the design condition, not a nicety. `MotorWithBrake` is structurally
    /// singular, has an unmatched unknown, and has a source line for it — and it is a
    /// perfectly good model that index reduction solves by demoting a state. Painting
    /// its `connect()` as a problem would teach the opposite of the lesson the
    /// Structural/IndexReduction contrast exists to teach.
    ///
    /// `CapacitorLoop` is the case where blame is real: states 1 → 1, nothing demoted,
    /// still singular, so nothing downstream can save it.
    #[test]
    fn only_an_unrescuable_model_gets_its_source_blamed() {
        // Structural failed, index reduction rescued it → no blame.
        let mut app = App::test_default();
        // Struct literals rather than `Stage::ok`/`recovered`: those constructors are
        // the worker's own, and the UI consumes stages read-only.
        let stage = Stage::ok;
        app.stages.structural = stage(serde_json::json!({ "error": { "kind": "singular" } }));
        app.stages.index_reduction = stage(serde_json::json!({ "blocks": [] }));
        app.compute_problem_lines();
        assert!(
            app.problem_lines.is_empty(),
            "a high-index model that index reduction fixed must not be blamed",
        );

        // Structural failed AND index reduction failed → blame, with the line.
        app.stages.index_reduction = stage(serde_json::json!({
            "error": {
                "kind": "singular",
                "unmatched_unknown_locations": [
                    { "unknown": "gnd.p.i", "location": { "line": 9 } }
                ]
            }
        }));
        app.compute_problem_lines();
        assert_eq!(app.problem_lines.len(), 1);
        assert_eq!(app.problem_lines[0].0, 9);
        assert!(
            app.problem_lines[0].1.contains("ill-posed"),
            "the hover must say why: {}",
            app.problem_lines[0].1,
        );

        // An unknown with no source provenance contributes no blamed line rather than
        // a bogus one — manufactured and solver-vector variables have no source.
        app.stages.index_reduction = stage(serde_json::json!({
            "error": {
                "kind": "singular",
                "unmatched_unknown_locations": [
                    { "unknown": "__solver_y_3", "location": null }
                ]
            }
        }));
        app.compute_problem_lines();
        assert!(app.problem_lines.is_empty(), "no span means no line to blame");
    }

    /// A link can address a sub-view, on both the load and the switch forms.
    ///
    /// Closes the quiet tour hole logged 2026-07-29: links reached a stage tab and
    /// no further, so every animation had to be handed off in prose ("same tab →
    /// now click **Incidence**"). The first tour had two working links and four
    /// such hand-offs.
    #[test]
    fn a_link_can_address_a_sub_view() {
        assert_eq!(
            parse_hrw_link("hrw://stage/Structural/MatchingAnim"),
            Some(HrwLink::SwitchStage(
                StageKind::Structural,
                Some(SubView::Structural(StructuralView::MatchingAnim)),
            )),
        );
        assert_eq!(
            parse_hrw_link("hrw://load/MotorWithBrake/IndexReduction/AliasAnim"),
            Some(HrwLink::LoadAndSwitch(
                "MotorWithBrake".to_owned(),
                StageKind::IndexReduction,
                Some(SubView::Structural(StructuralView::AliasAnim)),
            )),
        );
        // The bare forms still work — a sub-view is optional, not required.
        assert_eq!(
            parse_hrw_link("hrw://stage/Flatten"),
            Some(HrwLink::SwitchStage(StageKind::Flatten, None)),
        );
    }

    /// A sub-view slug is resolved **against its stage**, so the same word means
    /// different things in different stages and a wrong pairing does not navigate.
    /// **The noun/verb parity audit, as a test.**
    ///
    /// #42's design principle is that `hrw://` must express any noun `focus.json` can
    /// describe — same vocabulary, opposite directions. `SubView::from_slug`'s doc
    /// comment asserted "the slugs are exactly the names the capture emits", and until
    /// 2026-07-29 **nothing checked it**: an unverified claim about verification, which
    /// is the failure this project keeps finding in its own records.
    ///
    /// Doug asked for the audit to be run "as often as necessary". A manual audit rots;
    /// this one runs in the 7-second loop. It fails when a view variant is added to one
    /// side only — which is exactly what happens when a new feature introduces a noun.
    #[test]
    fn every_capture_view_name_round_trips_as_a_link_slug() {
        // (stage the sub-view belongs to, its capture name, the expected parse)
        let mut checked = 0usize;

        for v in StructuralView::ALL {
            // Structural sub-views are reachable under both stages that share the enum.
            for stage in [StageKind::Structural, StageKind::IndexReduction] {
                let name = structural_view_name(*v);
                assert_eq!(
                    SubView::from_slug(stage, name),
                    Some(SubView::Structural(*v)),
                    "capture emits {name:?} for {v:?} under {stage:?}, but hrw:// cannot parse it \
                     back — the two vocabularies have drifted",
                );
                checked += 1;
            }
        }
        for v in FlattenView::ALL {
            let name = flatten_view_name(*v);
            assert_eq!(
                SubView::from_slug(StageKind::Flatten, name),
                Some(SubView::Flatten(*v)),
                "Flatten: capture emits {name:?}, hrw:// cannot parse it",
            );
            checked += 1;
        }
        for v in EventsView::ALL {
            let name = events_view_name(*v);
            assert_eq!(
                SubView::from_slug(StageKind::Events, name),
                Some(SubView::Events(*v)),
                "Events: capture emits {name:?}, hrw:// cannot parse it",
            );
            checked += 1;
        }
        for v in InitView::ALL {
            let name = init_view_name(*v);
            assert_eq!(
                SubView::from_slug(StageKind::Initialization, name),
                Some(SubView::Init(*v)),
                "Initialization: capture emits {name:?}, hrw:// cannot parse it",
            );
            checked += 1;
        }

        assert!(checked >= 26, "expected every view variant covered, checked {checked}");
    }

    /// **Every noun the capture can describe is reachable by a link.**
    ///
    /// The whole of #42's design principle, in one assertion per noun. Written as an
    /// exhaustive match on `Focus` and a field-by-field walk of `Tracking`, so *adding a
    /// noun to the capture fails this test until a verb exists for it* — which is the
    /// only way the principle stays true rather than becoming a paragraph nobody checks.
    ///
    /// Two gaps stood open until 2026-07-29: `Focus::Node` (the capture's richest noun,
    /// produced by every left-click) and the follow. Both are closed here.
    #[test]
    fn every_capture_noun_is_reachable_by_a_link() {
        // `Focus`, exhaustively. The match is the point: a new variant will not compile
        // until it is considered here.
        let unreachable: Vec<&str> = [
            (
                "Focus::Node",
                parse_hrw_link("hrw://stage/Structural/Tree/node/error.unmatched_unknowns[0]")
                    .is_some(),
            ),
            ("Focus::Stage", parse_hrw_link("hrw://stage/Structural").is_some()),
            ("Focus::Specimen", parse_hrw_link("hrw://load/CapacitorLoop").is_some()),
            // `Focus::Nothing` is the absence of a point; there is nothing to navigate
            // to, and a verb for it would mean "un-point", which no tour has wanted.
            ("Tracking::name", parse_hrw_link("hrw://follow/emf.phi").is_some()),
            // The rest of `Tracking` is derived from the name (declaring class, source
            // line, per-stage mentions), so setting the name sets all of it.
            (
                "view.stage_view",
                parse_hrw_link("hrw://stage/Structural/MatchingAnim").is_some(),
            ),
            (
                "view.animation.frame",
                parse_hrw_link("hrw://stage/Structural/MatchingAnim/frame/1").is_some(),
            ),
            (
                "specimen source line",
                parse_hrw_link("hrw://source/9").is_some(),
            ),
        ]
        .into_iter()
        .filter_map(|(noun, reachable)| (!reachable).then_some(noun))
        .collect();

        assert!(
            unreachable.is_empty(),
            "the capture can describe these, and no link can reach them: {unreachable:?}",
        );
    }

    /// Every stage a capture can name is reachable by a link, and back.
    ///
    /// The other half of parity: `focus.json` carries a `stage`, so `hrw://stage/<X>`
    /// must accept every one of them. A stage added to `StageKind` without a slug would
    /// be describable but not navigable.
    #[test]
    fn every_stage_round_trips_between_capture_and_link() {
        for kind in StageKind::ALL {
            let slug = kind.slug();
            assert_eq!(
                StageKind::from_slug(slug),
                Some(*kind),
                "{kind:?} emits slug {slug:?} which does not parse back",
            );
            assert_eq!(
                parse_hrw_link(&format!("hrw://stage/{slug}")),
                Some(HrwLink::SwitchStage(*kind, None)),
                "hrw://stage/{slug} must navigate to {kind:?}",
            );
        }
    }

    /// The camera-aiming verb parses, and only where it makes sense.
    /// Every link in every **fixture tour** resolves against the current parser.
    ///
    /// A fixture tour is kept and versioned — unlike an ad hoc tour, which is gitignored
    /// and regenerated per question. The ephemerality rule was never about tours; it was
    /// about *explanation*, which rots because nothing checks it. A fixture tour has a
    /// pass/fail criterion, and **this test is what makes that true**: without something
    /// executing it, a saved tour is stored prose with extra steps, and would drift from
    /// the app exactly as `end_to_end_tour.md`'s 7x7 matrix did.
    ///
    /// Checks the links only. Whether the camera *looks* right is Doug's half — that is
    /// the whole reason the fixture exists.
    /// The picker names each tour by what it *is*, not by where it lives.
    #[test]
    fn tour_labels_name_what_the_tour_is() {
        assert!(
            TourSource::AdHoc.label().contains("Claude's answer"),
            "the ad hoc tour is named by its role; its filename is an implementation \
             detail nobody should need to know",
        );
        let fixture = TourSource::Fixture(PathBuf::from("/x/docs/fixture-tours/camera-aiming.md"));
        assert_eq!(fixture.label(), "camera-aiming");
        assert_eq!(fixture.path(), PathBuf::from("/x/docs/fixture-tours/camera-aiming.md"));
        assert_eq!(TourSource::AdHoc.path(), PathBuf::from(crate::bridge::TOUR_FILE));
    }

    /// The list offers the fixtures, ad hoc first when one exists.
    ///
    /// Doug asked for in-app selection so a fixture tour no longer has to be copied over
    /// `.hrw-bridge/tour.md` before starting HRW. Ad hoc goes first because it answers
    /// the question just asked; burying it under the fixtures would make the common case
    /// the awkward one.
    /// Switching tours re-initialises the right-hand side; re-selecting does not.
    ///
    /// Doug: clicking a link in one tour and then choosing a second tour left the first
    /// tour's specimen on screen. A tour is a self-contained sequence whose first stop
    /// loads a specimen, so the leftover state invites reading the new tour's stops
    /// against the old tour's model — and makes Stop 1 look already done.
    ///
    /// The reset reuses `open`'s own field list via `clear_specimen_state`, rather than
    /// a second copy that would drift from it.
    #[test]
    fn switching_tours_resets_the_stage_side() {
        let a = TourSource::Fixture(PathBuf::from("/x/a.md"));
        let b = TourSource::Fixture(PathBuf::from("/x/b.md"));

        let mut app = App::test_default();
        app.select_tour(a.clone());
        // Simulate having walked a stop: a specimen loaded, a stage reached.
        app.selected = Some(PathBuf::from("/x/RcCircuit.mo"));
        app.model = Some("RcCircuit".to_owned());
        app.stage = StageKind::Structural;

        // Re-selecting the SAME tour must not throw away work in progress.
        app.select_tour(a.clone());
        assert_eq!(app.selected, Some(PathBuf::from("/x/RcCircuit.mo")), "reselect keeps state");
        assert_eq!(app.model.as_deref(), Some("RcCircuit"));

        // A different tour starts clean.
        app.select_tour(b.clone());
        assert_eq!(app.selected, None, "the specimen is cleared");
        assert_eq!(app.model, None, "and so is the model");
        assert_eq!(app.stage, StageKind::Parse, "and the stage returns to the start");
        assert_eq!(app.selected_tour, Some(b));
    }

    #[test]
    fn the_tour_list_offers_fixtures_with_ad_hoc_first() {
        let mut app = App::test_default();
        app.poll_tour_file();

        assert!(
            app.tours.iter().any(|t| matches!(t, TourSource::Fixture(_))),
            "the checked-in fixture tours should be listed: {:?}",
            app.tours.iter().map(TourSource::label).collect::<Vec<_>>(),
        );

        // **A README is not a tour.** `docs/fixture-tours/` gained one on
        // 2026-08-01 under the two-audience convention (`DECISIONS.md`), and the
        // enumeration takes every `.md` in the directory — so without the
        // exclusion in `bridge::fixture_tours` the picker offers a tour called
        // "README" whose stops do not exist. Pinned here because the next
        // directory README would reintroduce it silently.
        let labels: Vec<String> = app.tours.iter().map(TourSource::label).collect();
        assert!(
            !labels.iter().any(|l| l.eq_ignore_ascii_case("README")),
            "README.md must not be offered as a tour: {labels:?}",
        );
        if app.tours.contains(&TourSource::AdHoc) {
            assert_eq!(app.tours[0], TourSource::AdHoc, "ad hoc sorts first");
            assert_eq!(app.selected_tour, Some(TourSource::AdHoc), "and is selected by default");
        }

        // Selecting a fixture drops the previous text immediately rather than leaving
        // it on screen until the next poll.
        let fixture = app
            .tours
            .iter()
            .find(|t| matches!(t, TourSource::Fixture(_)))
            .cloned()
            .expect("a fixture exists");
        app.select_tour(fixture.clone());
        assert!(app.cached_tour.is_none(), "old text cleared on switch");
        app.tour_polled_at = None;
        app.poll_tour_file();
        assert_eq!(app.selected_tour, Some(fixture));
        assert!(app.cached_tour.is_some(), "the chosen fixture is loaded");
    }

    /// Node paths in the **node-pointing** fixture resolve against the real IR.
    ///
    /// `fixture_tour_links_all_resolve` checks only the grammar — a path can parse
    /// perfectly and point at nothing. A fixture tour with a made-up path is a broken
    /// test that *looks* fine, which is the worst kind, so the paths are checked against
    /// the specimen's own trace.
    ///
    /// Stop 5 is deliberately unresolvable (it belongs to `CapacitorLoop`, which fails
    /// structurally); the tour expects a notice there, so it is excluded by name.
    #[test]
    fn node_pointing_fixture_paths_exist_in_the_real_ir() {
        let trace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("docs/specimen-notebook/RcCircuit/trace/structural.json");
        let Ok(text) = std::fs::read_to_string(&trace) else {
            return; // trace not generated in this checkout
        };
        let ir: Value = serde_json::from_str(&text).unwrap();

        let tour = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("docs/fixture-tours/node-pointing.md");
        let Ok(md) = std::fs::read_to_string(&tour) else {
            return;
        };

        // The one path the tour expects to fail, by design.
        const DELIBERATELY_ABSENT: &str = "error.unmatched_unknowns[0]";
        let mut checked = 0usize;
        for link in extract_hrw_links(&md) {
            let Some(raw) = link.split("/node/").nth(1) else {
                continue;
            };
            let path = bridge::parse_path(raw).expect("fixture paths must be well-formed");
            if raw == DELIBERATELY_ABSENT {
                assert!(
                    bridge::navigate(&ir, &path).is_none(),
                    "Stop 5 relies on {raw} being absent from RcCircuit; if it now exists \
                     the tour tests nothing",
                );
                continue;
            }
            assert!(
                bridge::navigate(&ir, &path).is_some(),
                "{raw} is in the fixture tour but not in RcCircuit's structural IR",
            );
            checked += 1;
        }
        assert!(checked >= 4, "expected the fixture's real paths to be checked, saw {checked}");
    }

    /// A fixture tour's referenced files exist.
    ///
    /// The cross-platform tour points at a Wolfram notebook, and a stop referencing a
    /// file that is not there tests nothing while looking fine — the same failure as a
    /// made-up node path. Fixture notebooks are therefore **versioned beside their
    /// tour**, not written to the gitignored bridge directory: an *ad hoc* notebook is
    /// ephemeral like an ad hoc tour, but a fixture has expected outcomes, and a test
    /// that vanishes on a fresh checkout is not a test.
    #[test]
    fn fixture_tours_reference_files_that_exist() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/fixture-tours");
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return;
        };
        let mut checked = 0usize;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();

            // Relative markdown links, excluding schemes and anchors.
            for target in text.split("](").skip(1).filter_map(|t| t.split(')').next()) {
                if target.starts_with('#') || target.contains("://") {
                    continue;
                }
                assert!(
                    dir.join(target).exists(),
                    "{} references {target}, which does not exist",
                    path.display(),
                );
                checked += 1;
            }

            // And `hrw://notebook/<name>` targets, which are references too — the link
            // parses whatever the name, so grammar alone proves nothing about the file.
            for link in extract_hrw_links(&text) {
                let Some(name) = link.strip_prefix("hrw://notebook/") else {
                    continue;
                };
                assert!(
                    bridge::resolve_notebook(name).is_some(),
                    "{} opens notebook {name}, which does not resolve",
                    path.display(),
                );
                checked += 1;
            }
        }
        // Non-vacuity. The first version of this test asserted only on relative links,
        // and converting the notebook link to `hrw://notebook/` left it with nothing to
        // check — it failed rather than passing empty, which is the behaviour to keep.
        assert!(checked > 0, "expected at least one file reference across the fixtures");
    }

    /// Every `hrw://` link in every fixture tour parses.
    ///
    /// **Enumerates through `bridge::fixture_tours`, not its own `read_dir`.** It had
    /// a private copy until 2026-08-01, which is a second definition of "what is a
    /// fixture tour" — and the two drifted the moment the directory gained a
    /// `README.md`: the app correctly stopped offering it as a tour while this test
    /// still scanned it and failed on the bare `hrw://` in its prose. *A check that
    /// exists twice is a check that drifts*, which is the same lesson F1 and F7 both
    /// produced.
    #[test]
    fn fixture_tour_links_all_resolve() {
        let mut tours = 0usize;
        let mut links = 0usize;
        for path in bridge::fixture_tours() {
            tours += 1;
            let text = std::fs::read_to_string(&path).unwrap();
            let found = extract_hrw_links(&text);
            assert!(
                !found.is_empty(),
                "a fixture tour with no links tests nothing: {}",
                path.display(),
            );
            for link in found {
                assert!(
                    parse_hrw_link(&link).is_some(),
                    "unresolvable link in {}: {link}",
                    path.display(),
                );
                links += 1;
            }
        }
        assert!(tours > 0 && links > 0, "expected at least one fixture tour with links");
    }

    /// The frame-seek verb parses everywhere an animation lives.
    /// **Every link that names a sub-view defers it, rather than applying it.**
    ///
    /// This is the bug Doug found by clicking the fixture tour in order: Stop 5's
    /// `hrw://stage/IndexReduction/Animate/frame/2` showed the Index Reduction
    /// *Summary* the first time, and the replay only on a second click.
    ///
    /// Cause: the centre panel resets the sub-view whenever a report stage is entered
    /// — forcing `Summary` for Index Reduction — and that reset runs *after* link
    /// dispatch. A sub-view applied during dispatch is therefore overwritten. The
    /// second click works because the stage no longer changes, so the reset is skipped.
    ///
    /// `pending_sub_view` exists precisely to survive that reset, and `LoadAndSwitch`
    /// already used it. The three sibling verbs did not. **The symptom to remember is
    /// "works on the second click" — it almost always means set-then-overwritten.**
    #[test]
    fn every_sub_view_link_defers_through_pending_sub_view() {
        let animate = SubView::Structural(StructuralView::Animate);

        for (label, link) in [
            ("switch", HrwLink::SwitchStage(StageKind::IndexReduction, Some(animate))),
            (
                "seek",
                HrwLink::SeekFrame(StageKind::IndexReduction, animate, 2),
            ),
            (
                "aim",
                HrwLink::AimAtEquation(StageKind::IndexReduction, animate, 0),
            ),
        ] {
            let mut app = App::test_default();
            app.selected = Some(PathBuf::from("/x/RcCircuit.mo"));
            // A sub-view the reset would clobber, so the test cannot pass by accident.
            app.structural_view = StructuralView::SpyPlot;
            app.dispatch_hrw_link(link);

            assert_eq!(app.stage, StageKind::IndexReduction, "{label}: stage switched");
            assert_eq!(
                app.pending_sub_view,
                Some(animate),
                "{label}: the sub-view must be DEFERRED so the stage-entry reset cannot \
                 overwrite it",
            );
            assert_eq!(
                app.structural_view,
                StructuralView::SpyPlot,
                "{label}: and must NOT be applied during dispatch",
            );
        }
    }

    /// A seek aimed at a view with no animation gives up instead of lingering armed.
    ///
    /// Without a budget it would sit pending until the reader wandered into an animated
    /// view and then fire there — a link taking effect somewhere it was never pointed.
    /// Stop 6 of the frame-seeking fixture is exactly this case.
    #[test]
    fn a_seek_that_never_lands_expires() {
        let mut app = App::test_default();
        app.selected = Some(PathBuf::from("/x/RcCircuit.mo"));
        // Incidence has no animation, ever.
        app.dispatch_hrw_link(HrwLink::SeekFrame(
            StageKind::Structural,
            SubView::Structural(StructuralView::Incidence),
            3,
        ));
        assert!(app.seek_frame.is_some(), "armed on dispatch");

        for _ in 0..SEEK_ATTEMPTS {
            app.apply_pending_seek();
        }
        assert!(
            app.seek_frame.is_none(),
            "the seek must expire rather than stay armed for a later view",
        );
    }

    /// A link's frame number is the one on screen — the two must not be off by one.
    ///
    /// Doug walked the fixture tour and found the link and the counter disagreeing.
    /// The fixture had even *documented* the discrepancy ("frames are 0-based in links,
    /// 1-based in the display"), which is writing a bug down instead of fixing it.
    ///
    /// The rule this pins: **each verb matches how its own thing is displayed.** Frames
    /// read "Frame 3/11" from one, so frame links count from one; equations read
    /// `f_x[46]` from zero, so equation links count from zero. Uniformity between the
    /// two verbs would force one to disagree with the screen, which is the drift that
    /// actually costs something.
    /// A link can point at a node, using the capture's own spelling of the path.
    ///
    /// This closes the last parity gap: `Focus::Node` is the capture's richest noun and
    /// no link could express it. The path grammar is not re-stated here — that is
    /// round-tripped in `bridge::tests` — this checks the link layer consumes it.
    /// A node path that does not exist is reported, not half-followed.
    ///
    /// Without this the tree expands as far as the path goes and stops, which reads as
    /// "it opened something" rather than "that path is wrong". The camera aim and the
    /// frame seek both refuse-and-report; this is the third verb made consistent.
    #[test]
    fn an_unresolvable_node_path_is_reported() {
        let stage = serde_json::json!({
            "error": { "unmatched_unknowns": ["gnd.p.i"] },
            "blocks": [{ "kind": "scalar" }],
        });

        // Paths that exist resolve silently.
        for good in ["", "error", "error.unmatched_unknowns[0]", "blocks[0].kind"] {
            let path = bridge::parse_path(good).expect("well-formed");
            assert_eq!(resolve_jump_target(&stage, &path), Ok(()), "{good:?} should resolve");
        }

        // Well-formed but absent: parses fine, navigates to nothing, must be reported.
        let path = bridge::parse_path("error.matched_unknowns[0]").expect("well-formed");
        let Err(msg) = resolve_jump_target(&stage, &path) else {
            panic!("a path that is not in the stage must be reported");
        };
        assert!(msg.contains("error.matched_unknowns[0]"), "the message names the path: {msg}");

        // Past the end of a real array counts as absent too.
        let path = bridge::parse_path("blocks[9]").expect("well-formed");
        assert!(resolve_jump_target(&stage, &path).is_err());
    }

    /// A link's trail entry is the link, so the trail can be read against the tour.
    ///
    /// Doug asked whether Claude can see him click a tour link. It could not — the
    /// action trail showed the specimen load and nothing after, so a report of "several
    /// bugs in the node-pointing tour" had to be reconstructed by asking. Now every
    /// followed link is recorded, and recorded as its **canonical URL** rather than a
    /// `Debug` dump, so it lines up with the tour's own text at a glance.
    ///
    /// Round-tripped rather than pinned to literals: `describe` and `parse_hrw_link`
    /// must agree, which is the same parity rule as everywhere else.
    /// The compile outcome names the first failing stage, or how far it got.
    ///
    /// Doug found the gap by reloading a tour and asking what the trail said an hour
    /// later: it still read `compiling: true, model: null`, because the trail ended at
    /// "specimen sent to the worker" and nothing recorded the finish. The app block was
    /// accurate for the last *action* and increasingly wrong about *now*.
    ///
    /// The **first** failing stage is what gets reported, because everything after it
    /// says "not reached" and carries no information.
    #[test]
    fn the_compile_outcome_names_the_first_failure() {
        let ok = Stage::ok(serde_json::json!({}));
        // `err_with_details` = Outcome::Failed: the value holds the error payload,
        // not IR, so nothing downstream can consume it.
        let failed = Stage::err_with_details(serde_json::json!({}), "boom");

        // A clean run reports how far it reached.
        let mut app = App::test_default();
        app.model = Some("RcCircuit".to_owned());
        app.stages = StageBundle {
            parse: ok.clone(), resolve: ok.clone(), instantiate: ok.clone(),
            typecheck: ok.clone(), flatten: ok.clone(), structural: ok.clone(),
            index_reduction: ok.clone(), initialization: ok.clone(), events: ok.clone(),
            solve_lowering: ok.clone(),
        };
        let outcome = app.compile_outcome();
        assert!(outcome.starts_with("RcCircuit: reached "), "{outcome}");

        // A failure names the stage, and names the FIRST one.
        let mut app = App::test_default();
        app.model = Some("UnbalancedShaft".to_owned());
        app.stages = StageBundle {
            parse: ok.clone(), resolve: ok.clone(), instantiate: ok.clone(),
            typecheck: ok.clone(),
            flatten: failed.clone(),
            // A later stage also "fails" (not reached); it must not be the one named.
            structural: failed.clone(),
            ..StageBundle::default()
        };
        let outcome = app.compile_outcome();
        assert_eq!(
            outcome, "UnbalancedShaft: FAILED at Flatten",
            "the first failure is the diagnostic; later stages are just not reached",
        );
    }

    #[test]
    fn a_recorded_link_round_trips_to_the_same_link() {
        for url in [
            "hrw://load/CapacitorLoop",
            "hrw://stage/Structural",
            "hrw://stage/Structural/Incidence",
            "hrw://load/RcCircuit/Structural/Tree",
            "hrw://source",
            "hrw://source/9",
            "hrw://stage/Structural/TarjanAnim/equation/13",
            "hrw://stage/Structural/MatchingAnim/frame/41",
            "hrw://stage/Structural/Tree/node/incidence.rows[0].equation_text",
            "hrw://follow/C.v",
        ] {
            let link = parse_hrw_link(url).unwrap_or_else(|| panic!("{url} should parse"));
            assert_eq!(
                format!("hrw://{}", link.describe()),
                url,
                "a recorded link must read back as the link that was clicked",
            );
        }
    }

    /// `SubView::slug` is `from_slug`'s inverse, for every variant.
    ///
    /// The missing-inverse gap again: `from_slug` existed alone, which is exactly how
    /// the stage vocabulary drifted into four copies. `slug` dispatches to the same
    /// functions the capture uses, so the two vocabularies are equal by construction.
    #[test]
    fn every_sub_view_slug_round_trips() {
        let cases: Vec<(StageKind, SubView)> = StructuralView::ALL
            .iter()
            .map(|v| (StageKind::Structural, SubView::Structural(*v)))
            .chain(FlattenView::ALL.iter().map(|v| (StageKind::Flatten, SubView::Flatten(*v))))
            .chain(EventsView::ALL.iter().map(|v| (StageKind::Events, SubView::Events(*v))))
            .chain(
                InitView::ALL
                    .iter()
                    .map(|v| (StageKind::Initialization, SubView::Init(*v))),
            )
            .collect();

        for (stage, sub) in cases {
            assert_eq!(
                SubView::from_slug(stage, sub.slug()),
                Some(sub),
                "{sub:?} writes slug {:?} which does not parse back under {stage:?}",
                sub.slug(),
            );
        }
    }

    /// A node link marks the row it pointed at, and the mark outlives the scroll.
    ///
    /// Doug walked the node-pointing fixture and reported the node was not highlighted.
    /// He was right twice over: the tour asserted a highlight, and **there was none** —
    /// `scroll_if_jump_target` only ever scrolled. The tour was right about what should
    /// happen, though: a row scrolled to the centre of a screen of near-identical rows,
    /// unmarked, leaves the reader guessing which one was meant.
    ///
    /// `jump_target` lasts exactly one frame, so highlighting on that alone would flash
    /// for 16ms. `jump_highlight` persists until Doug does something of his own.
    #[test]
    fn a_node_link_marks_the_row_until_doug_moves_on() {
        let path = bridge::parse_path("incidence.rows[0].equation_text").expect("well-formed");

        let mut app = App::test_default();
        // These verbs now require a loaded specimen — clicking a stop out of order is
        // refused rather than half-applied. Give it one so dispatch proceeds.
        app.selected = Some(PathBuf::from("/x/RcCircuit.mo"));
        app.dispatch_hrw_link(HrwLink::PointAtNode(
            StageKind::Structural,
            SubView::Structural(StructuralView::Tree),
            path.clone(),
        ));
        assert_eq!(app.jump_target.as_ref(), Some(&path), "scrolls to it");
        assert_eq!(app.jump_highlight.as_ref(), Some(&path), "and marks it");

        // The scroll is consumed after one frame; the mark is not.
        app.jump_target = None;
        assert_eq!(
            app.jump_highlight.as_ref(),
            Some(&path),
            "the mark must outlive the one-frame scroll, or it flashes and tells nobody",
        );

        // A point of Doug's own answers a different question, so the mark goes.
        app.emit_node_focus(vec![Seg::Key("blocks".into())], bridge::AskRequest::Explain);
        assert!(
            app.jump_highlight.is_none(),
            "Doug pointing at something supersedes the link's mark",
        );
    }

    #[test]
    fn a_link_can_point_at_a_node() {
        let Some(HrwLink::PointAtNode(stage, sub, path)) =
            parse_hrw_link("hrw://stage/Structural/Tree/node/error.unmatched_unknowns[0]")
        else {
            panic!("should parse");
        };
        assert_eq!(stage, StageKind::Structural);
        assert_eq!(sub, SubView::Structural(StructuralView::Tree));
        assert_eq!(bridge::describe_path(&path), "error.unmatched_unknowns[0]");

        // The tree root is a legitimate target.
        assert!(matches!(
            parse_hrw_link("hrw://stage/Flatten/Tree/node/"),
            Some(HrwLink::PointAtNode(StageKind::Flatten, _, _)),
        ));
        // A malformed path fails the whole link rather than pointing somewhere near.
        assert!(parse_hrw_link("hrw://stage/Structural/Tree/node/a..b").is_none());
    }

    /// A link can set the follow, independently of what is pointed at.
    ///
    /// The two composition primitives are independent by design — point-only,
    /// follow-only and both are all normal states — so `follow` deliberately does not
    /// touch the stage.
    /// The notebook verb parses a real name and refuses an empty one.
    ///
    /// `hrw://notebook/` alone names nothing. Accepting it meant a **prose mention** of
    /// the verb inside a code span parsed as a link to an unnamed file — which the
    /// fixture reference test duly reported as a missing notebook called "". Two small
    /// faults met there: the extractor did not stop at a backtick, and the grammar
    /// accepted an empty name.
    /// Sub-view availability depends on the model, not only the stage.
    ///
    /// Doug found the cross-platform tour linking to `Structural/Summary` on
    /// `ProportionalLoop`. The slug is valid for the stage, so `SubView::from_slug`
    /// accepts it — but Summary only has a tab when a model is **singular**, and
    /// ProportionalLoop is not. The link selected a view with no tab and the panel
    /// rendered the singular summary for a non-singular model.
    ///
    /// One predicate now answers this for both the tab bar and the link guard, so a
    /// tab that exists and a link that is honoured cannot disagree.
    /// A stop clicked out of order says so, instead of doing nothing.
    ///
    /// Doug clicked a tour's fourth stop first. Nothing happened: with no specimen the
    /// stage area returns early, so the link set state nothing consumed. Silence is the
    /// one outcome a tour cannot survive, because there is no way to tell it from a
    /// broken link.
    ///
    /// The state is **not** left pending. Setting it and returning would be worse than
    /// doing nothing — it would fire when a specimen arrived later, sending the reader
    /// somewhere no link had pointed.
    #[test]
    fn a_stop_needing_a_specimen_refuses_without_one() {
        let needs = [
            HrwLink::SwitchStage(StageKind::Structural, None),
            HrwLink::ShowSource(Some(9)),
            HrwLink::Follow("C.v".to_owned()),
            HrwLink::PointAtNode(
                StageKind::Structural,
                SubView::Structural(StructuralView::Tree),
                vec![Seg::Key("blocks".into())],
            ),
            HrwLink::SeekFrame(
                StageKind::Structural,
                SubView::Structural(StructuralView::MatchingAnim),
                0,
            ),
            HrwLink::AimAtEquation(
                StageKind::Structural,
                SubView::Structural(StructuralView::TarjanAnim),
                0,
            ),
        ];
        for link in needs {
            assert!(link.requires_specimen(), "{link:?} needs a specimen");
            let mut app = App::test_default();
            app.dispatch_hrw_link(link);
            assert!(app.notice.is_some(), "it must say so");
            assert!(app.pending_stage.is_none(), "and leave nothing armed to fire later");
            assert!(app.pending_sub_view.is_none());
            assert!(app.seek_frame.is_none());
            assert!(app.aim_at_equation.is_none());
            assert!(app.jump_target.is_none());
        }

        // The three that stand alone are unaffected.
        for link in [
            HrwLink::LoadSpecimen("RcCircuit".to_owned()),
            HrwLink::LoadAndSwitch("RcCircuit".to_owned(), StageKind::Structural, None),
            HrwLink::OpenNotebook("x.nb".to_owned()),
        ] {
            assert!(!link.requires_specimen(), "{link:?} makes sense on its own");
        }
    }

    #[test]
    fn a_sub_view_is_available_only_when_its_tab_is() {
        let clean = Stage::ok(serde_json::json!({}));
        // `recovered` = Outcome::Flagged, which is what the worker really builds for
        // a singular structural analysis: real IR *plus* an error, and the pipeline
        // carries on into index reduction.
        let singular = Stage::recovered(serde_json::json!({ "error": {} }), "singular");

        // A non-singular Structural stage: no Summary, but the pattern views are there.
        let mut app = App::test_default();
        app.stage = StageKind::Structural;
        app.stages.structural = clean.clone();
        assert!(!app.structural_view_available(StructuralView::Summary), "no Summary here");
        assert!(app.structural_view_available(StructuralView::SpyPlot));
        assert!(app.structural_view_available(StructuralView::TearingAnim));
        assert!(app.structural_view_available(StructuralView::Incidence));

        // Singular: Summary appears, and the views needing a full matching vanish.
        app.stages.structural = singular;
        assert!(app.structural_view_available(StructuralView::Summary));
        assert!(!app.structural_view_available(StructuralView::SpyPlot));
        assert!(!app.structural_view_available(StructuralView::TearingAnim));
        // ...except Matching, whose *failure* is the point on a singular system (#44).
        assert!(app.structural_view_available(StructuralView::MatchingAnim));
        assert!(app.structural_view_available(StructuralView::Tree));

        // Index Reduction always has a Summary, and the reduction replay only with frames.
        app.stage = StageKind::IndexReduction;
        app.stages.index_reduction = clean;
        assert!(app.structural_view_available(StructuralView::Summary));
        assert!(!app.structural_view_available(StructuralView::Animate), "no frames yet");
    }

    /// The System Modeler verb parses, needs a name, and stands alone.
    ///
    /// It needs no specimen *loaded* — like the load verbs, it makes sense on its own,
    /// which matters because the adjudicator case is often "open this in SM and see that
    /// it refuses", reached without walking a tour first.
    #[test]
    fn the_system_modeler_verb_stands_alone() {
        assert_eq!(
            parse_hrw_link("hrw://systemmodeler/IncompatibleConnect"),
            Some(HrwLink::OpenInSystemModeler("IncompatibleConnect".to_owned())),
        );
        assert!(parse_hrw_link("hrw://systemmodeler/").is_none(), "a bare verb names nothing");
        assert!(
            !HrwLink::OpenInSystemModeler("X".to_owned()).requires_specimen(),
            "opening a specimen in another tool does not need one loaded here",
        );
        // Round-trips into the action trail like every other verb.
        let link = parse_hrw_link("hrw://systemmodeler/CapacitorLoop").unwrap();
        assert_eq!(format!("hrw://{}", link.describe()), "hrw://systemmodeler/CapacitorLoop");
    }

    #[test]
    fn the_notebook_verb_needs_a_name() {
        assert_eq!(
            parse_hrw_link("hrw://notebook/structural-vs-numerical-rank.nb"),
            Some(HrwLink::OpenNotebook("structural-vs-numerical-rank.nb".to_owned())),
        );
        assert!(parse_hrw_link("hrw://notebook/").is_none(), "a bare verb names nothing");
        assert!(parse_hrw_link("hrw://notebook").is_none());
    }

    /// A verb written in prose, inside a code span, is not a link.
    ///
    /// Documentation about `hrw://` belongs in tours and doc comments, and writing it in
    /// backticks is how one writes it. The extractor must not turn that into a hook.
    #[test]
    fn a_code_span_mention_is_not_extracted_as_a_link() {
        let md = "Use the [notebook verb](hrw://notebook/x.nb). \
                  Writing `hrw://notebook/` in prose must not register a hook.";
        let links = extract_hrw_links(md);
        assert_eq!(
            links,
            vec!["hrw://notebook/x.nb", "hrw://notebook/"],
            "the code-span mention stops at the backtick rather than swallowing it",
        );
        // ...and the truncated mention does not parse, so nothing acts on it.
        assert!(parse_hrw_link("hrw://notebook/").is_none());
    }

    #[test]
    fn a_link_can_set_the_follow() {
        assert_eq!(
            parse_hrw_link("hrw://follow/emf.phi"),
            Some(HrwLink::Follow("emf.phi".to_owned())),
        );

        let mut app = App::test_default();
        app.stage = StageKind::Events;
        app.dispatch_hrw_link(HrwLink::Follow("load.w".to_owned()));
        assert_eq!(app.stage, StageKind::Events, "following does not navigate");
    }

    #[test]
    fn a_frame_link_and_the_frame_counter_agree() {
        for shown in [1usize, 2, 7, 41] {
            let link = format!("hrw://stage/Structural/MatchingAnim/frame/{shown}");
            let Some(HrwLink::SeekFrame(_, _, cursor)) = parse_hrw_link(&link) else {
                panic!("{link} should parse");
            };
            // What the label would render for that cursor.
            let label = crate::frame_label(cursor, 100, crate::LiveState::Idle);
            assert!(
                label.starts_with(&format!("Frame {shown}/")),
                "{link} should land on a view reading \"Frame {shown}/…\", got {label:?}",
            );
        }
    }

    #[test]
    fn a_link_can_seek_to_a_frame() {
        assert_eq!(
            parse_hrw_link("hrw://stage/Structural/MatchingAnim/frame/7"),
            Some(HrwLink::SeekFrame(
                StageKind::Structural,
                SubView::Structural(StructuralView::MatchingAnim),
                6, // 1-based link, 0-based cursor
            )),
        );
        // The non-structural animated views too — one per stage that has one.
        for (stage, view) in [
            ("Events", "PreLowering"),
            ("Initialization", "IcPlan"),
            ("Flatten", "Connections"),
        ] {
            let link = format!("hrw://stage/{stage}/{view}/frame/3");
            assert!(
                matches!(parse_hrw_link(&link), Some(HrwLink::SeekFrame(_, _, 2))),
                "{link} should seek",
            );
        }
        // Links are **1-based**, matching the on-screen counter, so `frame/1` is the
        // first frame and `frame/0` does not exist.
        assert!(matches!(
            parse_hrw_link("hrw://stage/Structural/TarjanAnim/frame/1"),
            Some(HrwLink::SeekFrame(_, _, 0)),
        ));
        assert!(
            parse_hrw_link("hrw://stage/Structural/TarjanAnim/frame/0").is_none(),
            "there is no frame zero when the counter starts at one",
        );
        // Garbage still fails rather than defaulting.
        assert!(parse_hrw_link("hrw://stage/Structural/TarjanAnim/frame/last").is_none());
        assert!(parse_hrw_link("hrw://stage/Structural/Tree/frame/1").is_some());
        assert!(parse_hrw_link("hrw://stage/Events/TarjanAnim/frame/1").is_none());
    }

    #[test]
    fn a_link_can_aim_at_an_equation() {
        assert_eq!(
            parse_hrw_link("hrw://stage/Structural/TarjanAnim/equation/13"),
            Some(HrwLink::AimAtEquation(
                StageKind::Structural,
                SubView::Structural(StructuralView::TarjanAnim),
                13,
            )),
        );
        // Works on the other stage that shares the sub-view enum.
        assert!(matches!(
            parse_hrw_link("hrw://stage/IndexReduction/MatchingAnim/equation/0"),
            Some(HrwLink::AimAtEquation(StageKind::IndexReduction, _, 0)),
        ));

        // A sub-view the stage does not have still fails, rather than aiming blindly.
        assert!(parse_hrw_link("hrw://stage/Events/TarjanAnim/equation/1").is_none());
        // A non-numeric index fails rather than silently becoming 0.
        assert!(parse_hrw_link("hrw://stage/Structural/TarjanAnim/equation/x").is_none());
        // The shorter forms still parse — raising `splitn` to 5 must not break them.
        assert!(matches!(
            parse_hrw_link("hrw://stage/Structural/Incidence"),
            Some(HrwLink::SwitchStage(StageKind::Structural, Some(_))),
        ));
        assert!(matches!(
            parse_hrw_link("hrw://load/CapacitorLoop/Structural/Summary"),
            Some(HrwLink::LoadAndSwitch(_, StageKind::Structural, Some(_))),
        ));
    }

    #[test]
    fn sub_view_slugs_are_stage_scoped() {
        // `Tree` exists under four stages and means a different enum in each.
        assert_eq!(
            SubView::from_slug(StageKind::Flatten, "Tree"),
            Some(SubView::Flatten(FlattenView::Tree)),
        );
        assert_eq!(
            SubView::from_slug(StageKind::Events, "Tree"),
            Some(SubView::Events(EventsView::Tree)),
        );

        // A slug from the wrong stage must not resolve — better a dead link than
        // one that navigates somewhere the author did not mean.
        assert!(SubView::from_slug(StageKind::Flatten, "MatchingAnim").is_none());
        assert!(SubView::from_slug(StageKind::Events, "IcPlan").is_none());
        // Stages with no sub-views reject every slug.
        assert!(SubView::from_slug(StageKind::Parse, "Tree").is_none());
        // And a malformed link is None rather than a partial navigation.
        assert!(parse_hrw_link("hrw://stage/Structural/NoSuchView").is_none());
    }

    /// **Every sub-view name the capture emits is addressable by a link, and vice
    /// versa.** This is #42's design principle as an assertion: `hrw://` should
    /// express any noun `focus.json` can describe, so the two directions share one
    /// vocabulary. Without this test the two lists drift, and a tour would point at
    /// a view whose capture name had been renamed.
    #[test]
    fn link_slugs_and_capture_names_are_the_same_vocabulary() {
        let cases: &[(StageKind, &[&str])] = &[
            (
                StageKind::Structural,
                &[
                    structural_view_name(StructuralView::Summary),
                    structural_view_name(StructuralView::SpyPlot),
                    structural_view_name(StructuralView::Incidence),
                    structural_view_name(StructuralView::MatchingAnim),
                    structural_view_name(StructuralView::TarjanAnim),
                    structural_view_name(StructuralView::TearingAnim),
                    structural_view_name(StructuralView::AliasAnim),
                    structural_view_name(StructuralView::Animate),
                    structural_view_name(StructuralView::Tree),
                ],
            ),
            (
                StageKind::Flatten,
                &[
                    flatten_view_name(FlattenView::Equations),
                    flatten_view_name(FlattenView::SourceMap),
                    flatten_view_name(FlattenView::Connections),
                    flatten_view_name(FlattenView::Tree),
                ],
            ),
            (
                StageKind::Events,
                &[events_view_name(EventsView::Tree), events_view_name(EventsView::PreLowering)],
            ),
            (
                StageKind::Initialization,
                &[init_view_name(InitView::Tree), init_view_name(InitView::IcPlan)],
            ),
        ];
        for (stage, names) in cases {
            for name in *names {
                assert!(
                    SubView::from_slug(*stage, name).is_some(),
                    "capture emits {name:?} for {stage:?} but no link can address it",
                );
            }
        }
    }

    /// An ad hoc tour written to the bridge round-trips, and its links parse.
    ///
    /// Replaces `tour_document_hrw_links_are_valid`, which checked the links in
    /// `end_to_end_tour.md` — a document HRW no longer shows. Its prose was
    /// retired 2026-07-29 and tour mode now renders whatever Claude writes to
    /// `.hrw-bridge/tour.md`, so the subject of that test no longer existed.
    ///
    /// Touches the shared bridge directory, so it needs `--test-threads=1` like
    /// the other bridge tests.
    #[test]
    fn an_ad_hoc_tour_round_trips_through_the_bridge() {
        let saved = std::fs::read_to_string(bridge::TOUR_FILE).ok();

        let tour = "# Stop 1

Open [the Structural tab](hrw://stage/Structural).

                    # Stop 2

Now [load MotorWithBrake](hrw://load/MotorWithBrake/IndexReduction).
";
        std::fs::create_dir_all(bridge::BRIDGE_DIR).unwrap();
        std::fs::write(bridge::TOUR_FILE, tour).unwrap();

        let (text, _mtime) = bridge::read_tour().expect("a written tour is readable");
        assert!(text.contains("Stop 1"), "{text}");

        let links = extract_hrw_links(&text);
        assert_eq!(links.len(), 2, "both links found: {links:?}");
        for link in &links {
            assert!(parse_hrw_link(link).is_some(), "unparseable link: {link}");
        }

        // Absence is the normal state, not an error.
        std::fs::remove_file(bridge::TOUR_FILE).unwrap();
        assert!(bridge::read_tour().is_none(), "no tour file means no tour");

        if let Some(prev) = saved {
            std::fs::write(bridge::TOUR_FILE, prev).unwrap();
        }
    }

    /// Every `hrw://` link in every specimen `purpose.md` parses.
    ///
    /// **Counts the files it checked and asserts the count is right.** Until
    /// 2026-07-29 this looked for `narrative.md`, and when those were renamed to
    /// `purpose.md` the `continue` swallowed every directory — the test passed by
    /// checking nothing. A silent-skip test is worse than no test, so the count
    /// is now part of the assertion.
    #[test]
    fn purpose_note_hrw_links_are_valid() {
        let notebook_dir =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/specimen-notebook");
        let mut checked = 0usize;
        let mut notes = std::collections::BTreeSet::new();
        for entry in std::fs::read_dir(&notebook_dir).unwrap() {
            let path = entry.unwrap().path();
            if !path.is_dir() {
                continue;
            }
            let purpose = path.join("purpose.md");
            assert!(purpose.exists(), "every specimen dir needs a purpose.md: {}", path.display());
            notes.insert(
                path.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_owned(),
            );
            checked += 1;
            let text = std::fs::read_to_string(&purpose).unwrap();
            for link in extract_hrw_links(&text) {
                assert!(
                    parse_hrw_link(&link).is_some(),
                    "invalid hrw link in {}: {link}",
                    purpose.display()
                );
            }
        }
        // Tied to the corpus rather than to a magic number. The literal `14` here
        // failed the moment four diagnostic specimens were added (2026-07-29) — it was
        // guarding the right property with the wrong constant, so it reported a
        // *correct* corpus as broken. Every `specimens/*.mo` must have a note, and
        // every note must belong to a specimen; both directions matter, because an
        // orphaned note is prose about a model that no longer exists.
        let specimen_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("specimens");
        let specimens: std::collections::BTreeSet<String> = std::fs::read_dir(&specimen_dir)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("mo"))
            .filter_map(|p| p.file_stem().and_then(|s| s.to_str()).map(str::to_owned))
            .collect();

        assert_eq!(
            notes, specimens,
            "every specimen needs a purpose note and every note needs a specimen; \
             missing notes: {:?}; orphaned notes: {:?}",
            specimens.difference(&notes).collect::<Vec<_>>(),
            notes.difference(&specimens).collect::<Vec<_>>(),
        );
        assert_eq!(checked, specimens.len());
    }

    #[test]
    fn open_resets_all_specimen_state() {
        let mut app = App::test_default();
        // Populate fields with non-default values to detect missed resets.
        app.model = Some(String::from("OldModel"));
        app.sim_data = Some(crate::worker::SimData {
            times: vec![0.0],
            names: vec![],
            data: vec![],
            n_states: 0,
            has_discontinuities: false,
            solver_steps: vec![],
        });
        app.sim_error = Some("stale error".into());
        app.sim_running = true;
        app.def_index.insert(1, crate::worker::DefInfo {
            name: "x".into(),
            kind: crate::worker::DefKind::Definition,
            class_type: None,
            file_name: None,
            line: None,
        });
        app.cached_equation_sheet = Some(crate::equation_sheet::EquationSheet {
            groups: vec![],
            n_equations: 0,
            variables: vec![],
            n_states: 0,
            n_algebraics: 0,
            n_parameters: 0,
            n_constants: 0,
            n_discrete: 0,
            n_inputs: 0,
            n_outputs: 0,
            source_lines: vec![],
        });
        app.identifier_index = Some(crate::identifier_index::IdentifierIndex::default());
        app.tracked_identifier = Some("h".into());
        app.cached_source = Some("old source".into());
        app.highlighted_eq_row = Some(0);
        app.highlighted_source_line = Some(0);
        app.nav.push(NavEntry {
            name: "x".into(),
            value: serde_json::Value::Null,
            def_index: BTreeMap::new(),
        });
        app.nav_loading = Some("y".into());
        app.nav_error = Some("z".into());
        app.pending_stage = Some(StageKind::Resolve);
        app.viewing_log = false;

        app.open(PathBuf::from("specimens/BouncingBall.mo"));

        assert!(app.compiling, "compiling should be true");
        assert!(app.model.is_none(), "model should be cleared");
        assert!(app.sim_data.is_none(), "sim_data should be cleared");
        assert!(app.sim_error.is_none(), "sim_error should be cleared");
        assert!(!app.sim_running, "sim_running should be false");
        assert!(app.def_index.is_empty(), "def_index should be cleared");
        assert!(app.cached_equation_sheet.is_none(), "cached_equation_sheet should be cleared");
        assert!(app.identifier_index.is_none(), "identifier_index should be cleared");
        assert!(app.tracked_identifier.is_none(), "tracked_identifier should be cleared");
        assert!(app.cached_source.is_none(), "cached_source should be cleared");
        assert!(app.highlighted_eq_row.is_none(), "highlighted_eq_row should be cleared");
        assert!(app.highlighted_source_line.is_none(), "highlighted_source_line should be cleared");
        assert!(app.nav.is_empty(), "nav should be cleared");
        assert!(app.nav_loading.is_none(), "nav_loading should be cleared");
        assert!(app.nav_error.is_none(), "nav_error should be cleared");
        assert!(app.pending_stage.is_none(), "pending_stage should be cleared");
        assert!(app.viewing_log, "viewing_log should be true");
    }
}
