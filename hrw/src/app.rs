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
//! IR for the selected model. Stages present so far: Parse, Resolve.

use std::path::{Path, PathBuf};

use eframe::egui;

use std::collections::{BTreeMap, HashMap};

// The bridge module handles communication with Claude Code (the AI assistant
// running in a terminal alongside this app): we write JSON "focus" files that
// Claude reads to understand what the user is looking at.
use crate::bridge::{self, Ask, Focus, Seg};
use crate::equation_sheet;
// Canvas provides a pan/zoom camera for custom-painted views (spy-plot,
// incidence matrix). It tracks the transform and handles drag/scroll input.
use crate::canvas::Canvas;
use crate::field_help;
use crate::incidence_view;
use crate::matching_anim;
use crate::tarjan_anim;
use crate::log_view;
use crate::reduction_view;
use crate::spyplot;
use crate::tree;
// The worker module runs compilation and simulation on a background thread so
// the UI never blocks. Communication is via channels: we send `ToWorker`
// commands and receive `FromWorker` results. `Stage` holds one pipeline stage's
// output (its serde_json::Value IR + optional error note).
use crate::worker::{
    discontinuity_segments, DefInfo, FromWorker, LogEntry, SimData, Stage, StageBundle, StageKind,
    ToWorker, Worker,
};

/// Initial UI zoom (fonts + spacing) — readable on a hi-dpi display. Adjustable
/// live via Settings (or Ctrl +/−); egui's `zoom_factor` is the idiomatic knob.
const DEFAULT_ZOOM: f32 = 2.0;

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

/// How to render the Structural / Index-reduction stages: the custom BLT
/// spy-plot, the incidence matrix, the reduction process
/// summary (Index reduction only), or the generic serde tree.
#[derive(Clone, Copy, PartialEq, Eq)]
enum StructuralView {
    SpyPlot,
    Incidence,
    MatchingAnim,
    TarjanAnim,
    Reduction,
    Tree,
}

/// Sub-tab selector for the Flatten stage: readable equation sheet, source
/// traceability map, or the generic serde tree.
#[derive(Clone, Copy, PartialEq, Eq)]
enum FlattenView {
    Equations,
    SourceMap,
    Tree,
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
    /// True once the user explicitly clicks a stage tab; false on specimen open.
    /// While false the RHS panel shows specimen info, not stage-specific help.
    /// This two-phase UX lets the right panel start with a specimen overview,
    /// then transition to field-level help once the user dives into a stage.
    stage_clicked: bool,
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
    // understand what the user is looking at. `ask_seq` is a monotonic counter
    // so each capture gets a unique ID; `bridge_status` shows confirmation in
    // the bottom status bar.
    ask_seq: u64,
    bridge_status: Option<String>,
    /// When set, the tree inspector forces open all nodes along this path
    /// (deep link navigation from tour documents). Cleared after one frame.
    pending_nav_path: Option<Vec<Seg>>,

    // ---- 7. Panels and windows toggled from the menu bar ----
    // `show_left_panel` / `show_right_panel` control whether the side panels
    // are rendered at all — hiding them gives the center stage view full width,
    // which is especially useful during live debug sessions.
    show_left_panel: bool,
    show_right_panel: bool,
    // egui's `Window::open(&mut bool)` pattern: the bool controls visibility,
    // and the window's close button flips it back to false.
    show_settings: bool,
    show_help: bool,
    show_about: bool,

    // ---- 8. Generic field help ----
    // `field_help` is a compile-time HashMap<field_name, explanation> loaded
    // from a generated help table. `selected_field` tracks the last
    // left-clicked tree node so the right panel can show its documentation.
    // This is the "fast tier" (instant, no AI); the "specific tier" is the
    // bridge capture + Claude chat.
    field_help: HashMap<String, String>,
    selected_field: Option<String>,

    // ---- 9. Custom views for Structural/Index-reduction stages ----
    // These stages have a spy-plot and incidence-matrix visualization in
    // addition to the generic JSON tree. Each custom view has its own `Canvas`
    // (pan/zoom camera state).
    flatten_view: FlattenView,
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

    // ---- 12. Cached path checks ----
    // Avoids calling `Path::exists()` every frame for the narrative button.
    // Invalidated on specimen change (set in `drain_worker` when `Compiled` lands).
    narrative_exists: bool,

    // ---- 13. Cached layout ----
    // Avoids running `layout_no_wrap` on the longest filename every frame.
    // Invalidated on rescan (when `files` changes).
    cached_specimen_width: Option<f32>,

    // ---- 14. Cached structural views ----
    // Avoids re-parsing `from_report` JSON every frame. Invalidated when
    // `stages` changes (in `drain_worker` on `Compiled`). Outer Option is
    // the cache state (None = not yet computed); inner Option is the parse
    // result (None = report had no data for this view).
    cached_spy_plot: Option<Option<spyplot::Plot>>,
    cached_incidence: Option<Option<incidence_view::IncidenceMatrix>>,
    cached_reduction: Option<Option<reduction_view::ReductionView>>,
    cached_equation_sheet: Option<equation_sheet::EquationSheet>,
    cached_matching_anim: Option<Option<matching_anim::MatchingAnimation>>,
    matching_anim_canvas: Canvas,
    cached_tarjan_anim: Option<Option<tarjan_anim::TarjanAnimation>>,
    tarjan_anim_canvas: Canvas,

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
}

#[derive(Clone, Copy)]
enum PendingLiveDebug {
    Matching,
    Tarjan,
}

impl App {
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
            scan_error: None,
            selected: None,
            compiling: false,
            model: None,
            stages: StageBundle::default(),
            stage: StageKind::Resolve,
            stage_clicked: false,
            def_index: BTreeMap::new(),
            nav: Vec::new(),
            nav_loading: None,
            nav_error: None,
            ask_seq: 0,
            bridge_status: None,
            pending_nav_path: None,
            show_left_panel: true,
            show_right_panel: true,
            show_settings: false,
            show_help: false,
            show_about: false,
            field_help: field_help::load(),
            selected_field: None,
            flatten_view: FlattenView::Equations,
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
            narrative_exists: false,
            cached_specimen_width: None,
            cached_spy_plot: None,
            cached_incidence: None,
            cached_reduction: None,
            cached_equation_sheet: None,
            cached_matching_anim: None,
            matching_anim_canvas: Canvas::default().with_fit_vertical_bias(0.15),
            cached_tarjan_anim: None,
            tarjan_anim_canvas: Canvas::default().with_fit_vertical_bias(0.15),
            pending_live_debug: None,
            live_breakpoint_armed: false,
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
        // Scan each specimen's `// purpose:` hint (cheap; no compile), so the list
        // can show what each one demonstrates.
        self.specimen_purposes = self
            .files
            .iter()
            .filter_map(|p| read_purpose(p).map(|hint| (p.clone(), hint)))
            .collect();
        self.cached_specimen_width = None;
    }

    /// Open (compile) a specimen. Called when the user clicks a file in the
    /// left panel.
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
        // Mark as compiling — this disables stage tab highlighting and shows a
        // spinner.
        self.compiling = true;
        // Clear all previous results. Every field that could hold stale data
        // from the last specimen is reset to its default.
        self.model = None;
        self.narrative_exists = false;
        self.stages = StageBundle::default();
        self.sim_data = None;
        self.sim_error = None;
        self.sim_running = false;
        self.def_index = BTreeMap::new();
        self.cached_equation_sheet = None;
        self.highlighted_eq_row = None;
        self.highlighted_source_line = None;
        // Reset the "user has clicked a stage" flag so the right panel starts
        // with specimen info, not stage-specific help.
        self.stage_clicked = false;
        self.nav.clear();
        self.nav_loading = None;
        self.nav_error = None;
        self.selected_field = None;
        // Clean up any in-flight or active live debug session. Without this,
        // switching specimens while arming or running leaves the breakpoint
        // armed on the old specimen and the polling state dangling.
        if self.live_breakpoint_armed {
            let _ = bridge::remove_live_trace_breakpoint();
        }
        self.live_breakpoint_armed = false;
        self.pending_live_debug = None;
        // Start with the log view so the user sees compilation progress.
        self.log_entries.clear();
        self.viewing_log = true;
        // Send the compile command to the worker thread. Results will arrive
        // asynchronously via `FromWorker::CompileProgress` and `FromWorker::Compiled`.
        self.worker.send(ToWorker::Compile(path.clone()));
        self.selected = Some(path);
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
    /// Poll for a navigate request from a tour deep link (written by the VS
    /// Code extension when the user clicks an `hrw://` link in a markdown doc).
    /// Switches to the requested stage tab and stores the JSON path so the tree
    /// inspector forces those nodes open this frame.
    fn drain_navigate_requests(&mut self) {
        if let Some(req) = bridge::read_navigate_request() {
            if let Some(stage) = StageKind::from_name(&req.stage) {
                self.stage = stage;
                self.stage_clicked = true;
                self.viewing_log = false;
                self.nav.clear();
                let segs: Vec<Seg> = req.path.iter().map(|s| {
                    if let Ok(i) = s.parse::<usize>() {
                        Seg::Index(i)
                    } else {
                        Seg::Key(s.clone())
                    }
                }).collect();
                self.pending_nav_path = if segs.is_empty() { None } else { Some(segs) };
            }
        }
    }

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
                } => {
                    if self.selected.as_deref() != Some(path.as_path()) {
                        continue; // stale result
                    }
                    self.compiling = false;
                    self.narrative_exists = model.as_ref().is_some_and(|m| {
                        let p = format!("{}/docs/specimen-notebook/{m}/narrative.md", env!("CARGO_MANIFEST_DIR"));
                        std::path::Path::new(&p).exists()
                    });
                    self.model = model;
                    self.stages = stages;
                    self.def_index = def_index;
                    self.cached_equation_sheet = equation_sheet;
                    self.cached_spy_plot = None;
                    self.cached_incidence = None;
                    self.cached_reduction = None;
                    self.cached_matching_anim = None;
                    self.cached_tarjan_anim = None;
                    if self.live_breakpoint_armed {
                        let _ = bridge::remove_live_trace_breakpoint();
                    }
                    self.live_breakpoint_armed = false;
                    self.pending_live_debug = None;
                    self.spy_canvas.request_fit();
                    self.incidence_canvas.request_fit();
                    self.matching_anim_canvas.request_fit();
                    self.tarjan_anim_canvas.request_fit();
                    // Land on the furthest stage that completed cleanly.
                    self.stage = self.last_successful_stage();
                    // Publish every stage's full IR so Claude can diff any pair.
                    if let Err(e) = bridge::write_stages(&self.stages.as_stage_pairs()) {
                        self.bridge_status = Some(format!("write_stages failed: {e}"));
                    }
                }
                FromWorker::Simulated { path, result } => {
                    if self.selected.as_deref() != Some(path.as_path()) {
                        continue; // stale result for a different specimen
                    }
                    self.sim_running = false;
                    match result {
                        Ok(data) => {
                            self.sim_data = Some(data);
                            self.sim_error = None;
                        }
                        Err(e) => {
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
        let ok = |s: &Stage| s.value.is_some() && !s.note_is_error;
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
    fn base_ask<'a>(&'a self, seq: u64, request: bridge::AskRequest, focus: Focus<'a>) -> Ask<'a> {
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
        }
    }

    /// The `ask_seq` counter makes each capture unique, and the status bar
    /// confirms what was captured ("captured equations.3.lhs -- now ask me
    /// about it in the chat").
    fn emit_focus(&mut self, focus: Focus) {
        self.ask_seq += 1;
        let seq = self.ask_seq;
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
        };
        let ask = self.base_ask(seq, bridge::AskRequest::Explain, focus);
        self.bridge_status = Some(status_line(seq, &target, "explain", bridge::write(&ask)));
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
        self.ask_seq += 1;
        let seq = self.ask_seq;
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
            };
            status_line(seq, &target, request_str, bridge::write(&ask))
        } else {
            let stage_value = self.current_stage().value.clone();
            match &stage_value {
                Some(value) => {
                    let focus = Focus::Node { key_path, stage_value: value };
                    let ask = self.base_ask(seq, request, focus);
                    status_line(seq, &target, request_str, bridge::write(&ask))
                }
                None => "(no IR for this stage to point at)".to_owned(),
            }
        };
        self.bridge_status = Some(status);
    }

    fn narrative_button(&mut self, ui: &mut egui::Ui) {
        if !self.narrative_exists {
            return;
        }
        let Some(model) = &self.model else { return };
        let rel = format!("docs/specimen-notebook/{model}/narrative.md");
        ui.add_space(8.0);
        ui.separator();
        if ui
            .button("Read: specimen narrative")
            .on_hover_text(&rel)
            .clicked()
        {
            let abs = format!("{}/{}", env!("CARGO_MANIFEST_DIR"), rel);
            self.open_in_editor(&abs);
        }
    }

    fn open_in_editor(&mut self, path: impl AsRef<std::ffi::OsStr>) {
        let editor = std::env::var("EDITOR").unwrap_or_else(|_| "code".to_owned());
        match std::process::Command::new(&editor).arg(path).spawn() {
            Ok(_) => {}
            Err(e) => {
                self.bridge_status = Some(format!("{editor}: launch failed: {e}"));
            }
        }
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
                        .height(ui.available_height() * 0.65);
                }
                trajectory_plot.show(ui, |plot_ui| {
                    for (i, name) in data.names.iter().enumerate() {
                        let series = &data.data[i];
                        let segments = if data.has_discontinuities {
                            discontinuity_segments(series)
                        } else {
                            std::iter::once(0..series.len()).collect()
                        };
                        let color = series_color(i);
                        for seg in segments {
                            let pts: PlotPoints = data.times[seg.clone()]
                                .iter()
                                .zip(&series[seg])
                                .map(|(&t, &y)| [t, y])
                                .collect();
                            plot_ui.line(Line::new(name.clone(), pts).color(color));
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
                                    .color(egui::Color32::from_rgb(70, 130, 230)),
                            );

                            let order_pts: PlotPoints = data.solver_steps.iter()
                                .map(|s| [s.t, s.order as f64])
                                .collect();
                            plot_ui.line(
                                Line::new("BDF order k", order_pts)
                                    .color(egui::Color32::from_rgb(230, 130, 70)),
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

    /// The right-hand context panel. Routes to one of three sub-panels based
    /// on the current state.
    ///
    /// The routing implements a **specimen -> stage transition**: when a
    /// specimen is first opened, `stage_clicked` is false, so the right panel
    /// shows specimen-level info (name, model, purpose, narrative link). Once
    /// the user clicks any stage tab, `stage_clicked` flips to true and the
    /// panel switches to stage-specific content — either the Simulation help
    /// panel or the generic field-help panel (which shows documentation for
    /// the last-clicked IR tree node). This two-phase UX avoids showing an
    /// empty "no field selected" message before the user has engaged with a
    /// stage.
    fn right_panel(&mut self, ui: &mut egui::Ui) {
        if !self.stage_clicked && self.nav.is_empty() {
            self.right_panel_specimen(ui);
        } else if self.nav.is_empty() && self.stage == StageKind::Simulation {
            self.right_panel_simulation(ui);
        } else {
            self.right_panel_field_help(ui);
        }
    }

    /// Specimen-level info shown in the RHS before the user clicks any stage tab.
    fn right_panel_specimen(&mut self, ui: &mut egui::Ui) {
        let Some(path) = &self.selected else {
            ui.weak("Select a specimen to begin.");
            return;
        };
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("<?>");
        ui.strong(name);
        ui.separator();
        if let Some(m) = &self.model {
            ui.horizontal(|ui| {
                ui.label("Model:");
                ui.label(egui::RichText::new(m).monospace());
            });
        } else if self.compiling {
            ui.weak("Compiling…");
        }
        if let Some(purpose) = self.specimen_purposes.get(path) {
            ui.add_space(4.0);
            ui.label(purpose);
        }
        self.narrative_button(ui);
    }

    /// Generic (build-time) field help for the last-clicked tree item — the fast
    /// tier (no Claude). Shown for every stage whose view is the IR tree.
    fn right_panel_field_help(&mut self, ui: &mut egui::Ui) {
        // Title the pane with the stage on screen (e.g. "Flatten") — "About this
        // field" was meaningless when nothing was selected. While navigating a
        // definition the view is Resolve context, so say so.
        let title = if self.nav.is_empty() { self.stage.name() } else { "Resolve" };
        ui.strong(title);
        ui.separator();
        match &self.selected_field {
            Some(name) => {
                ui.label(egui::RichText::new(name).monospace().strong());
                ui.add_space(4.0);
                match self.field_help.get(name) {
                    Some(doc) => {
                        ui.label(doc);
                    }
                    None => {
                        ui.weak(format!(
                            "No generic help for “{name}”. Left-click captures it; type \
                             “explain” in the chat for a specific explanation.",
                        ));
                    }
                }
            }
            None => {
                ui.weak(
                    "Left-click a tree item to see what it is (generic help). Then type \
                     “explain” in the chat for the specific story.",
                );
            }
        }
        ui.add_space(8.0);
        ui.separator();
        self.right_panel_read_links(ui);
    }

    /// The Simulation view's right-hand panel — about the *run*, not a tree field.
    /// PLANNED: this is where a plot-question view will live (capture a curve or a
    /// time window → ask Claude about the trajectory, the events, the stiffness).
    /// For now it explains the plot controls and points questions at the chat.
    fn right_panel_simulation(&mut self, ui: &mut egui::Ui) {
        ui.strong("Simulation");
        ui.separator();
        ui.label("Press ▶ Run to integrate the model; each state/output is plotted vs time.");
        ui.add_space(4.0);
        ui.weak("Drag to pan, scroll to zoom, double-click to reset; toggle a series in the legend.");
        ui.add_space(8.0);
        // Plan-ahead placeholder for the bigger simulation work: a view that
        // captures a plotted curve / time window as question context for Claude.
        ui.weak("Coming soon: capture a curve or a time window to ask Claude about the run.");
        ui.add_space(8.0);
        ui.separator();
        self.right_panel_read_links(ui);
    }

    /// The two "Read: …" doc links shared by both right-hand panels: the phase's
    /// generic `docs/compiler-phases` chapter, and this specimen's notebook narrative.
    fn right_panel_read_links(&mut self, ui: &mut egui::Ui) {
        // Concept-level link: the docs/compiler-phases chapter for the phase whose
        // view is on screen (Resolve while navigating a definition).
        let stage_ctx = if self.nav.is_empty() { self.stage.name() } else { "Resolve" };
        let (label, rel) = field_help::chapter_for_stage(stage_ctx);
        if ui
            .button(format!("Read: {label}"))
            .on_hover_text("Open this docs/compiler-phases chapter (generic phase theory) in your editor")
            .clicked()
        {
            let abs = format!("{}/{}", env!("CARGO_MANIFEST_DIR"), rel);
            self.open_in_editor(abs);
        }
        if self.nav.is_empty() {
            self.narrative_button(ui);
        }
    }
}

/// One-line status for a completed bridge write, tailored to the request kind.
/// Shown in the bottom status bar to confirm what was captured and what the
/// user should do next (e.g. "captured equations.3.lhs -- now ask me about it
/// in the chat"). The `seq` number helps the user correlate captures with
/// Claude's responses.
fn status_line(seq: u64, target: &str, request: &str, result: std::io::Result<std::path::PathBuf>) -> String {
    match result {
        Err(e) => format!("bridge write failed: {e}"),
        Ok(_) if request == "debug-where-set" => {
            format!("🐞 captured  {target}  for the debugger — say “debug” in chat and I'll set the breakpoint  (focus #{seq})")
        }
        Ok(_) => format!("captured  {target}  — now ask me about it in the chat  (focus #{seq})"),
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
                ui.strong("Ask Claude");
                ui.label(
                    "Left-click a node to capture it (right-click for more actions), then ask \
                     your question in the Claude Code chat — Claude reads the capture.",
                );
                ui.add_space(2.0);
                ui.label(
                    "Shortcut: right after capturing, just type \u{201c}explain\u{201d} in the chat — Claude \
                     explains what you captured, no need to phrase a question.",
                );
                ui.add_space(6.0);
                ui.strong("Diff stages");
                ui.label(
                    "Every capture publishes all five stages\u{2019} full IR, so Claude can compare any \
                     two on request. Capture anything, then ask in the chat — e.g. \u{201c}what did \
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

                for (cat, eqs) in &sheet.groups {
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new(format!("{} ({})", cat.label(), eqs.len())).strong());
                    ui.weak(cat.description());
                    ui.add_space(2.0);
                    for eq in eqs {
                        let selected = self.highlighted_eq_row == Some(eq.index);
                        let text = egui::RichText::new(&eq.text).monospace();
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
                            ui.label(egui::RichText::new(&v.name).monospace());
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
                self.stage_clicked = true;
            }
        }
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
        let left_width = (avail.x * 0.45).max(200.0);

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
                    let has_equations = !sl.equation_indices.is_empty();

                    let line_num = format!("{:>4} ", sl.line_number);
                    let mut text = egui::RichText::new(
                        format!("{}{}", line_num, &sl.text),
                    ).monospace();

                    if is_eq_linked {
                        text = text.background_color(
                            egui::Color32::from_rgba_premultiplied(100, 180, 255, 40),
                        );
                    }

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
                        let is_line_linked = !line_eq_indices.is_empty()
                            && line_eq_indices.contains(&eq.index);

                        let mut text = egui::RichText::new(&eq.text).monospace();
                        if is_line_linked {
                            text = text.color(cat.color());
                        }

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
        // First thing every frame: check for results from the worker thread
        // and for navigate requests from tour deep links.
        self.drain_worker();
        self.drain_navigate_requests();

        // ---- Top menu bar ----
        // `Panel::top` claims a strip at the top of the window. Its string ID
        // ("bar") must be unique among panels so egui can track its state.
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
                    ui.checkbox(&mut self.show_left_panel, "Specimens panel");
                    ui.checkbox(&mut self.show_right_panel, "Help panel");
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
                });
            });
        });

        self.floating_windows(ui);

        // ---- Bottom status bar ----
        // Added BEFORE the side panels so it spans the full window width (egui
        // panels claim space in the order they're created — a bottom panel added
        // after left/right panels would only fill the remaining center).
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.add_space(1.0);
            match &self.bridge_status {
                Some(s) => ui.weak(s),
                None => ui.weak("Left-click a tree node to capture it, then ask about it in the chat (right-click for more actions)."),
            };
            ui.add_space(1.0);
        });

        // ---- Left panel: specimen file list ----
        // Auto-size the panel width to fit the longest specimen filename, with
        // padding for spacing, margins, and the scrollbar. The text is measured
        // using egui's layout engine (`layout_no_wrap`) so the width adapts to
        // the actual font/zoom level.
        if self.show_left_panel {
        let specimen_width = *self.cached_specimen_width.get_or_insert_with(|| {
            let longest = self.files.iter()
                .filter_map(|p| p.file_name().and_then(|n| n.to_str()))
                .max_by_key(|n| n.len())
                .unwrap_or("");
            let galley = ui.painter().layout_no_wrap(
                longest.to_owned(),
                egui::TextStyle::Body.resolve(ui.style()),
                egui::Color32::WHITE,
            );
            let spacing = ui.style().spacing.item_spacing.x;
            let margin = ui.style().spacing.window_margin.sum().x;
            let scrollbar = 16.0;
            (galley.size().x + spacing * 2.0 + margin + scrollbar).max(120.0)
        });
        // `Panel::left` claims a resizable strip on the left side. The
        // specimen list scrolls vertically if there are more files than fit.
        egui::Panel::left("file_list")
            .resizable(true)
            .default_size(specimen_width)
            .min_size(120.0)
            .show(ui, |ui| {
                ui.strong("Specimens");
                // `ui.separator()` draws a horizontal line — a visual divider
                // between the heading and the list content.
                ui.separator();

                if let Some(err) = &self.scan_error {
                    // `colored_label` draws text in a specific color.
                    // `visuals().error_fg_color` is the theme's standard error
                    // color (red in both light and dark themes).
                    ui.colored_label(ui.visuals().error_fg_color, err);
                    return;
                }
                if self.files.is_empty() {
                    // `ui.weak(…)` renders text in the theme's "weak" (dimmed) color,
                    // for secondary/placeholder text.
                    ui.weak("(no .mo specimens found)");
                    return;
                }

                // `ScrollArea::vertical()` wraps its content in a scrollable
                // region. The `.show()` closure provides the scrollable content.
                egui::ScrollArea::vertical().show(ui, |ui| {
                    // "Deferred action" pattern again: we can't call
                    // `self.open()` inside this closure (it borrows `self`
                    // through `self.files`), so we collect the path to open
                    // and act after the closure.
                    let mut to_open = None;
                    let mut recompile = None;
                    let mut capture_specimen = false;
                    for path in &self.files {
                        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("<?>");
                        let selected = self.selected.as_deref() == Some(path.as_path());
                        // Whole-specimen capture is offered only once this specimen is
                        // the loaded, done-compiling one — so it captures real IR, and we
                        // compile exactly once (the left-click load), never again just to
                        // capture.
                        let can_capture = selected && !self.compiling && self.model.is_some();
                        let can_recompile = selected && !self.compiling;
                        let purpose = self.specimen_purposes.get(path);
                        // `selectable_label` renders a label that highlights when
                        // `selected` is true and responds to clicks. It returns a
                        // `Response` that we can query for `.clicked()`, add hover
                        // text to, or attach a context menu to.
                        let mut resp = ui.selectable_label(selected, name);
                        // `.on_hover_text()` adds a tooltip shown when the mouse
                        // hovers over this widget. Returns the same Response so
                        // calls can be chained.
                        if let Some(hint) = purpose {
                            resp = resp.on_hover_text(hint);
                        }
                        // `.context_menu()` attaches a right-click popup menu to
                        // this widget. The closure builds the menu content, and
                        // `ui.close()` dismisses it after an action.
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
                            let btn = ui.add_enabled(can_capture, egui::Button::new("🔎 Capture"));
                            let btn = if can_capture {
                                btn.on_hover_text(
                                    "Capture the whole specimen, then ask Claude about it in the chat.",
                                )
                            } else {
                                btn.on_disabled_hover_text(
                                    "Left-click to load & compile this specimen first, then Capture.",
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
                    if let Some(path) = recompile {
                        self.open(path);
                    } else if let Some(path) = to_open {
                        if self.selected.as_ref() == Some(&path) {
                            self.stage_clicked = false;
                            self.viewing_log = false;
                        } else {
                            self.open(path);
                        }
                    }
                    if capture_specimen {
                        self.emit_focus(Focus::Specimen);
                    }
                });
            });
        }

        // ---- Right panel: context help ----
        // `Panel::right` claims a strip on the right. It shows generic
        // (build-time) field help for the last-clicked tree item — the FAST
        // tier (no Claude). The specific tier ("why did THIS one happen") is
        // the bridge + chat, via "explain".
        if self.show_right_panel {
            egui::Panel::right("field_help")
                .resizable(true)
                .default_size(380.0)
                .min_size(220.0)
                .show(ui, |ui| self.right_panel(ui));
        }

        // ---- Center panel: stage tabs + main content ----
        //
        // More "deferred action" variables: the CentralPanel closure borrows
        // `self` through the panel, so we can't call methods like
        // `emit_node_focus` inside it. Instead, the closure sets these flags
        // and the actual method calls happen after the closure, below.
        let mut node_ask: Option<Vec<Seg>> = None;   // left-click capture (explain)
        let mut debug_ask: Option<Vec<Seg>> = None;  // right-click debugger capture
        let mut canvas_capture: Option<Vec<Seg>> = None; // spy-plot block click
        let mut nav_to: Option<String> = None;       // "Go to definition" target
        let mut want_stage_ask = false;              // stage tab was clicked
        let mut go_back = false;                     // navigation "Back" button
        let mut go_home = false;                     // navigation "Specimen" button

        // `CentralPanel` fills all remaining space after top/bottom/left/right
        // panels have claimed theirs. This is where the main content lives.
        egui::CentralPanel::default().show(ui, |ui| {
            if self.nav.is_empty() {
                // ---- Specimen stage view ----
                // No specimen yet → no stages to show, so don't render the tab
                // row (a highlighted tab before any compile is misleading).
                if self.selected.is_none() {
                    ui.weak("Select a specimen to compile.");
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
                    // acts on that flag to: set `stage_clicked = true` (transition
                    // the right panel), turn off `viewing_log`, and emit a stage
                    // capture for the bridge. This avoids duplicating that logic
                    // in every tab's click handler.
                    //
                    // TAB COLORING:
                    //
                    // Each tab label is colored via `tab_label()`:
                    // - Red if the stage errored (so you see pipeline failures at a glance)
                    // - Green if the stage produced IR (success)
                    // - Default color if not yet reached or still compiling
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
                        (StageKind::Typecheck, "Typecheck (instanced)", &self.stages.typecheck, Some(
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
                    for &(kind, label, ref stage, hover) in tabs {
                        let mut resp = ui.selectable_label(
                            stage_selected && self.stage == kind,
                            tab_label(label, stage, ok, err),
                        );
                        if let Some(tip) = hover {
                            resp = resp.on_hover_text(tip);
                        }
                        if resp.clicked() {
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
                        self.stage_clicked = true;
                        self.viewing_log = false;
                    }
                    if stage_tab_clicked {
                        self.stage_clicked = true;
                        self.viewing_log = false;
                        if self.selected.is_some() {
                            want_stage_ask = true;
                        }
                    }
                    ui.separator();
                    // The compiled-model identity (first class in the AST — not the
                    // filename, and None until parse succeeds). Whole-specimen capture
                    // now lives on the specimen list's right-click menu, not here.
                    if let Some(m) = &self.model {
                        ui.label(egui::RichText::new(m).monospace().strong());
                    }
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
                    if let Some(note) = &stage.note {
                        let color = if stage.note_is_error {
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
                        ui.selectable_value(&mut self.flatten_view, FlattenView::Tree, "Tree");
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
                    let is_index_reduction = self.stage == StageKind::IndexReduction;
                    // If switching away from IndexReduction while Reduction is
                    // selected, fall back to SpyPlot (Structural has no reduction).
                    if !is_index_reduction && self.structural_view == StructuralView::Reduction {
                        self.structural_view = StructuralView::SpyPlot;
                    }
                    // Here we DO use `selectable_value` (unlike the stage tabs
                    // above) because these sub-view tabs have no conditional
                    // suppression — one is always selected. `selectable_value`
                    // acts as a radio button: it highlights when the enum
                    // matches the given variant, and sets the enum on click.
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut self.structural_view, StructuralView::SpyPlot, "Spy-plot");
                        ui.selectable_value(&mut self.structural_view, StructuralView::Incidence, "Incidence");
                        ui.selectable_value(&mut self.structural_view, StructuralView::MatchingAnim, "Matching \u{25b6}");
                        ui.selectable_value(&mut self.structural_view, StructuralView::TarjanAnim, "BLT \u{25b6}");
                        if is_index_reduction {
                            ui.selectable_value(&mut self.structural_view, StructuralView::Reduction, "Reduction");
                        }
                        ui.selectable_value(&mut self.structural_view, StructuralView::Tree, "Tree");
                    });
                    ui.separator();
                }

                if report_ready && self.structural_view == StructuralView::SpyPlot {
                    let cached = self.cached_spy_plot.get_or_insert_with(|| {
                        self.stages.get(self.stage).value.as_ref().and_then(spyplot::Plot::from_report)
                    });
                    if let Some(plot) = cached {
                        ui.weak(plot.caption());
                        plot.ui(ui, &mut self.spy_canvas, &mut canvas_capture);
                    } else {
                        ui.weak("(the structural report has no BLT blocks to plot)");
                    }
                } else if report_ready && self.structural_view == StructuralView::Incidence {
                    let cached = self.cached_incidence.get_or_insert_with(|| {
                        self.stages.get(self.stage).value.as_ref().and_then(incidence_view::IncidenceMatrix::from_report)
                    });
                    if let Some(mat) = cached {
                        ui.weak(mat.caption());
                        mat.ui(ui, &mut self.incidence_canvas, &mut canvas_capture, self.highlighted_eq_row);
                    } else {
                        ui.weak("(no incidence data in this report)");
                    }
                } else if report_ready && self.structural_view == StructuralView::MatchingAnim {
                    // Ensure incidence is computed first.
                    if self.cached_incidence.is_none() {
                        self.cached_incidence = Some(
                            self.stages.get(self.stage).value.as_ref()
                                .and_then(incidence_view::IncidenceMatrix::from_report)
                        );
                    }
                    // ── Live debug session lifecycle (matching) ──
                    //
                    // Three-phase state machine:
                    //   1. Idle (no live session) → Debug button visible
                    //   2. Arming (pending_live_debug is Some) → polls for ack
                    //   3. Running (live animation active) → user steps with F5
                    //
                    // Phase 3→1 transition: the algorithm thread's `on_complete`
                    // callback removes the breakpoint via the bridge before
                    // exiting; `live_just_finished` below is a UI-side fallback
                    // that clears the armed flag.
                    let is_live_running = self.cached_matching_anim.as_ref()
                        .and_then(|o| o.as_ref())
                        .map_or(false, |a| a.is_live() && !a.live_finished());

                    // Safety-net: if the thread finished but the armed flag is
                    // still set (the on_complete removal is the primary path),
                    // clean up here so the Debug button reappears.
                    let live_just_finished = self.live_breakpoint_armed
                        && self.cached_matching_anim.as_ref()
                            .and_then(|o| o.as_ref())
                            .map_or(false, |a| a.is_live() && a.live_finished());
                    if live_just_finished {
                        let _ = bridge::remove_live_trace_breakpoint();
                        self.live_breakpoint_armed = false;
                    }

                    // Phase 1: show the Debug button when no live session is
                    // running and no arm request is pending.
                    if !is_live_running && self.pending_live_debug.is_none() {
                        if let Some(Some(_)) = &self.cached_incidence {
                            if ui.button("Debug").on_hover_text(
                                "Start live debug session \u{2014} arms a breakpoint on \
                                 live_trace_breakpoint, then step with Continue (F5)"
                            ).clicked() {
                                let _ = bridge::arm_live_trace_breakpoint(self.model.as_deref());
                                self.pending_live_debug = Some((
                                    std::time::Instant::now(),
                                    PendingLiveDebug::Matching,
                                ));
                            }
                        }
                    }

                    // Phase 2: wait for the extension to acknowledge that the
                    // breakpoint is registered. Poll `check_breakpoint_ack()`
                    // each frame (~16ms); fall back to a 3s timeout if the
                    // extension isn't running.
                    if let Some((armed_at, PendingLiveDebug::Matching)) = self.pending_live_debug {
                        let acked = bridge::check_breakpoint_ack();
                        let timed_out = armed_at.elapsed() >= std::time::Duration::from_secs(3);
                        if acked || timed_out {
                            self.pending_live_debug = None;
                            self.live_breakpoint_armed = true;
                            if let Some(Some(mat)) = &self.cached_incidence {
                                // on_complete runs inside the algorithm thread
                                // after the last frame — removes the breakpoint
                                // before the thread exits (prevents SIGSTOP).
                                self.cached_matching_anim = Some(Some(
                                    matching_anim::MatchingAnimation::start_live(mat, || {
                                        let _ = bridge::remove_live_trace_breakpoint();
                                    })
                                ));
                                self.matching_anim_canvas.request_fit();
                            }
                        } else {
                            ui.weak("Arming breakpoint\u{2026}");
                            ui.ctx().request_repaint();
                        }
                    }
                    if self.cached_matching_anim.is_none() {
                        let inc = self.cached_incidence.as_ref().unwrap();
                        self.cached_matching_anim = Some(
                            inc.as_ref().map(matching_anim::MatchingAnimation::from_incidence)
                        );
                    }
                    if let Some(Some(anim)) = &mut self.cached_matching_anim {
                        anim.ui(ui, &mut self.matching_anim_canvas);
                    } else {
                        ui.weak("(no incidence data for matching animation)");
                    }
                } else if report_ready && self.structural_view == StructuralView::TarjanAnim {
                    if self.cached_incidence.is_none() {
                        self.cached_incidence = Some(
                            self.stages.get(self.stage).value.as_ref()
                                .and_then(incidence_view::IncidenceMatrix::from_report)
                        );
                    }
                    // ── Live debug session lifecycle (Tarjan) ──
                    // Same three-phase state machine as matching above.
                    let is_live_running = self.cached_tarjan_anim.as_ref()
                        .and_then(|o| o.as_ref())
                        .map_or(false, |a| a.is_live() && !a.live_finished());
                    let live_just_finished = self.live_breakpoint_armed
                        && self.cached_tarjan_anim.as_ref()
                            .and_then(|o| o.as_ref())
                            .map_or(false, |a| a.is_live() && a.live_finished());
                    if live_just_finished {
                        let _ = bridge::remove_live_trace_breakpoint();
                        self.live_breakpoint_armed = false;
                    }
                    if !is_live_running && self.pending_live_debug.is_none() {
                        if let Some(Some(_)) = &self.cached_incidence {
                            if ui.button("Debug").on_hover_text(
                                "Start live debug session \u{2014} arms a breakpoint on \
                                 live_trace_breakpoint, then step with Continue (F5)"
                            ).clicked() {
                                let _ = bridge::arm_live_trace_breakpoint(self.model.as_deref());
                                self.pending_live_debug = Some((
                                    std::time::Instant::now(),
                                    PendingLiveDebug::Tarjan,
                                ));
                            }
                        }
                    }
                    if let Some((armed_at, PendingLiveDebug::Tarjan)) = self.pending_live_debug {
                        let acked = bridge::check_breakpoint_ack();
                        let timed_out = armed_at.elapsed() >= std::time::Duration::from_secs(3);
                        if acked || timed_out {
                            self.pending_live_debug = None;
                            self.live_breakpoint_armed = true;
                            if let Some(Some(mat)) = &self.cached_incidence {
                                self.cached_tarjan_anim = Some(
                                    tarjan_anim::TarjanAnimation::start_live(mat, || {
                                        let _ = bridge::remove_live_trace_breakpoint();
                                    })
                                );
                                self.tarjan_anim_canvas.request_fit();
                            }
                        } else {
                            ui.weak("Arming breakpoint\u{2026}");
                            ui.ctx().request_repaint();
                        }
                    }
                    if self.cached_tarjan_anim.is_none() {
                        let inc = self.cached_incidence.as_ref().unwrap();
                        self.cached_tarjan_anim = Some(
                            inc.as_ref().and_then(tarjan_anim::TarjanAnimation::from_incidence)
                        );
                    }
                    if let Some(Some(anim)) = &mut self.cached_tarjan_anim {
                        anim.ui(ui, &mut self.tarjan_anim_canvas);
                    } else {
                        ui.weak("(no dependency graph for BLT animation)");
                    }
                } else if report_ready && self.structural_view == StructuralView::Reduction {
                    let cached = self.cached_reduction.get_or_insert_with(|| {
                        self.stages.get(self.stage).value.as_ref().and_then(reduction_view::ReductionView::from_report)
                    });
                    if let Some(view) = cached {
                        view.ui(ui);
                    } else {
                        ui.weak("(no reduction data in this report)");
                    }
                } else if flatten_ready && self.flatten_view == FlattenView::Equations {
                    self.equation_sheet_ui(ui);
                } else if flatten_ready && self.flatten_view == FlattenView::SourceMap {
                    self.source_map_ui(ui);
                } else {
                    let stage = self.current_stage();
                    match &stage.value {
                        Some(value) => {
                            let label = self.model.as_deref().unwrap_or("model");
                            let prev = self.previous_stage_value();
                            let op = self.pending_nav_path.as_deref();
                            egui::ScrollArea::both().id_salt("tree").auto_shrink(false).show(ui, |ui| {
                                tree::tree_ui(ui, label, value, prev, &mut node_ask, &mut nav_to, &mut debug_ask, &self.def_index, op);
                            });
                        }
                        None if stage.note.is_none() => {
                            ui.weak(if self.compiling { "compiling…" } else { "(no output for this stage)" });
                        }
                        None => {}
                    }
                }
                } // end: non-Simulation stage rendering
            } else {
                // ---- Navigation view (a class reached via "Go to definition") ----
                ui.horizontal(|ui| {
                    if ui.button("Specimen").on_hover_text("Return to the specimen stages (top of navigation)").clicked() {
                        go_home = true;
                    }
                    if ui.button("← Back").clicked() {
                        go_back = true;
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
                    tree::tree_ui(ui, &entry.name, &entry.value, None, &mut node_ask, &mut nav_to, &mut debug_ask, &entry.def_index, None);
                });
            }
        });

        // ---- Deferred actions ----
        //
        // All the flags/options collected during the CentralPanel closure are
        // now acted on. The panel closure has ended, so `self` is no longer
        // borrowed and we can call methods freely. This is the payoff of the
        // "collect intent, act later" pattern used throughout this function.
        if go_home {
            self.nav.clear();
        } else if go_back {
            self.nav.pop();
        }
        if let Some(name) = nav_to {
            self.navigate_to(name);
        }
        // A spy-plot block click is treated identically to a tree-node click
        // for capture purposes.
        if canvas_capture.is_some() {
            node_ask = canvas_capture;
        }
        // Update the right-panel field help to match whichever node was clicked
        // (debug or explain — both select the same field for documentation).
        if let Some(kp) = debug_ask.as_ref().or(node_ask.as_ref()) {
            self.selected_field = field_name_from_path(kp);
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

        // Deep-link paths are one-shot: force the tree open this frame, then
        // clear so subsequent frames don't keep overriding user interaction.
        self.pending_nav_path = None;
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
const GOLDEN_RATIO: f32 = 0.618_033_99;

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
    if stage.note_is_error {
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

/// The field name to look up generic help for = the last object-key segment in
/// the clicked path (an array-index tail falls back to its enclosing field).
///
/// A `Seg` (from the bridge module) represents one segment of a JSON path:
/// - `Seg::Key(String)` — an object field name (e.g. "equations", "lhs", "type_id")
/// - `Seg::Index(usize)` — an array index (e.g. the `3` in `equations[3]`)
///
/// When the user clicks a tree node, the tree inspector produces a path like
/// `[Key("equations"), Index(3), Key("lhs")]`. To look up field help, we want
/// the deepest *named* field — here, "lhs". If the path ends with an array
/// index (e.g. `[Key("equations"), Index(3)]`), we skip the index and use the
/// enclosing field name ("equations") instead, since the help table is keyed
/// on field names, not array positions.
fn field_name_from_path(path: &[Seg]) -> Option<String> {
    // Walk the path backwards (`.rev()`) and return the first `Key` segment.
    // `find_map` combines `find` + `map`: it stops at the first `Some` return.
    path.iter().rev().find_map(|seg| match seg {
        Seg::Key(k) => Some(k.clone()),
        Seg::Index(_) => None,
    })
}

#[cfg(test)]
impl App {
    #[cfg(test)]
    fn test_default() -> Self {
        let (tx, _) = std::sync::mpsc::channel();
        let (_, rx) = std::sync::mpsc::channel();
        App {
            worker: Worker { tx, rx },
            libraries_text: String::new(),
            library_status: String::new(),
            libraries_busy: false,
            specimen_dir: String::new(),
            files: Vec::new(),
            specimen_purposes: HashMap::new(),
            scan_error: None,
            selected: None,
            compiling: false,
            model: None,
            stages: StageBundle::default(),
            stage: StageKind::Parse,
            stage_clicked: false,
            def_index: BTreeMap::new(),
            nav: Vec::new(),
            nav_loading: None,
            nav_error: None,
            ask_seq: 0,
            bridge_status: None,
            pending_nav_path: None,
            show_left_panel: true,
            show_right_panel: true,
            show_settings: false,
            show_help: false,
            show_about: false,
            field_help: HashMap::new(),
            selected_field: None,
            flatten_view: FlattenView::Equations,
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
            narrative_exists: false,
            cached_specimen_width: None,
            cached_spy_plot: None,
            cached_incidence: None,
            cached_reduction: None,
            cached_equation_sheet: None,
            cached_matching_anim: None,
            matching_anim_canvas: Canvas::default().with_fit_vertical_bias(0.15),
            cached_tarjan_anim: None,
            tarjan_anim_canvas: Canvas::default().with_fit_vertical_bias(0.15),
            pending_live_debug: None,
            live_breakpoint_armed: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn field_name_from_path_extracts_key() {
        assert_eq!(
            field_name_from_path(&[Seg::Key("classes".to_owned()), Seg::Key("name".to_owned())]),
            Some("name".to_owned())
        );
    }

    #[test]
    fn field_name_from_path_skips_trailing_index() {
        assert_eq!(
            field_name_from_path(&[Seg::Key("equations".to_owned()), Seg::Index(3)]),
            Some("equations".to_owned())
        );
    }

    #[test]
    fn field_name_from_path_empty() {
        assert_eq!(field_name_from_path(&[]), None);
    }

    fn make_app_with_stages(ok_through: StageKind) -> App {
        let ok_stage = Stage { value: Some(serde_json::json!({})), note: None, note_is_error: false };
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
        app.stages.structural = Stage { value: Some(serde_json::json!({})), note: Some("singular".into()), note_is_error: true };
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
}
