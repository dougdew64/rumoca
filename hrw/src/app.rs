//! The observatory shell.
//!
//! Eframe shell (charter §4.2.1, §4.4): a file picker over the specimen
//! directory, a library-path (source-root) configuration for dependency
//! resolution, and the generic serde-value tree inspector showing each stage's
//! IR for the selected model. Stages present so far: Parse, Resolve.

use std::path::{Path, PathBuf};

use eframe::egui;

use std::collections::{BTreeMap, HashMap};

use crate::bridge::{self, Ask, Focus, Seg};
use crate::canvas::Canvas;
use crate::field_help;
use crate::incidence_view;
use crate::log_view;
use crate::reduction_view;
use crate::spyplot;
use crate::tree;
use crate::worker::{
    discontinuity_segments, DefInfo, FromWorker, LogEntry, SimData, Stage, StageBundle, ToWorker,
    Worker,
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum StageKind {
    Parse,
    Resolve,
    Instantiate,
    Typecheck,
    Flatten,
    Structural,
    IndexReduction,
    Initialization,
    Events,
    SolveLowering,
    Simulation,
}

/// How to render the Structural / Index-reduction stages: the custom BLT
/// spy-plot, the incidence matrix, the reduction process
/// summary (Index reduction only), or the generic serde tree.
#[derive(Clone, Copy, PartialEq, Eq)]
enum StructuralView {
    SpyPlot,
    Incidence,
    Reduction,
    Tree,
}

/// One level of "go to definition" navigation: a class extracted from the
/// resolved tree, shown in the same generic tree the specimen stages use.
struct NavEntry {
    name: String,
    value: serde_json::Value,
    def_index: BTreeMap<u64, DefInfo>,
}

pub struct App {
    worker: Worker,

    // Library source roots (dependency resolution context).
    libraries_text: String,
    library_status: String,
    libraries_busy: bool,

    // Specimen directory + file list.
    specimen_dir: String,
    files: Vec<PathBuf>,
    // Per-specimen one-line purpose hint (the `// purpose:` comment), scanned at
    // rescan so the specimen list reads as an index of what each one teaches.
    specimen_purposes: HashMap<PathBuf, String>,
    scan_error: Option<String>,

    // Current selection + results.
    selected: Option<PathBuf>,
    compiling: bool,
    model: Option<String>,
    parse: Stage,
    resolve: Stage,
    instantiate: Stage,
    typecheck: Stage,
    flatten: Stage,
    structural: Stage,
    index_reduction: Stage,
    initialization: Stage,
    events: Stage,
    solve_lowering: Stage,
    stage: StageKind,
    /// True once the user explicitly clicks a stage tab; false on specimen open.
    /// While false the RHS panel shows specimen info, not stage-specific help.
    stage_clicked: bool,
    // Resolved identity of every DefId referenced in the current model's IR.
    def_index: BTreeMap<u64, DefInfo>,

    // "Go to definition" navigation stack (empty ⇒ showing the specimen stages).
    nav: Vec<NavEntry>,
    nav_loading: Option<String>,
    nav_error: Option<String>,

    // Claude bridge: monotonic ask counter + last-write feedback.
    ask_seq: u64,
    bridge_status: Option<String>,

    // Windows toggled from the menu bar.
    show_settings: bool,
    show_help: bool,
    show_about: bool,

    // Generic (build-time) field help + the field the user last left-clicked.
    field_help: HashMap<String, String>,
    selected_field: Option<String>,

    // The Structural stage's custom views and their pan/zoom cameras.
    structural_view: StructuralView,
    spy_canvas: Canvas,
    incidence_canvas: Canvas,

    // The compilation log — timestamped events streamed from the worker thread.
    // `viewing_log` is true when the log view is selected (the Log button left of
    // the stage tabs); auto-selected when a specimen is opened.
    log_entries: Vec<LogEntry>,
    viewing_log: bool,
    tracing_enabled: bool,

    // On-demand simulation (the Simulation tab) — not a compile stage.
    // `simulation` is an always-empty placeholder so `current_stage` has a Stage
    // to return; the Simulation view is the egui_plot pane, rendered specially.
    simulation: Stage,
    sim_data: Option<SimData>,
    sim_running: bool,
    sim_error: Option<String>,
    sim_t_end: f64,
}

impl App {
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
            parse: Stage::default(),
            resolve: Stage::default(),
            instantiate: Stage::default(),
            typecheck: Stage::default(),
            flatten: Stage::default(),
            structural: Stage::default(),
            index_reduction: Stage::default(),
            initialization: Stage::default(),
            events: Stage::default(),
            solve_lowering: Stage::default(),
            stage: StageKind::Resolve,
            stage_clicked: false,
            def_index: BTreeMap::new(),
            nav: Vec::new(),
            nav_loading: None,
            nav_error: None,
            ask_seq: 0,
            bridge_status: None,
            show_settings: false,
            show_help: false,
            show_about: false,
            field_help: field_help::load(),
            selected_field: None,
            structural_view: StructuralView::SpyPlot,
            spy_canvas: Canvas::default(),
            incidence_canvas: Canvas::default(),
            log_entries: Vec::new(),
            viewing_log: false,
            tracing_enabled: false,
            simulation: Stage::default(),
            sim_data: None,
            sim_running: false,
            sim_error: None,
            sim_t_end: 2.0,
        };
        app.rescan();
        app.load_libraries(); // load MSL at startup so resolve works immediately
        app
    }

    fn parse_library_paths(&self) -> Vec<PathBuf> {
        self.libraries_text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(PathBuf::from)
            .collect()
    }

    fn load_libraries(&mut self) {
        let roots = self.parse_library_paths();
        self.libraries_busy = true;
        self.library_status = format!("loading {} source root(s)…", roots.len());
        self.worker.send(ToWorker::SetLibraries(roots));
    }

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
    }

    fn open(&mut self, path: PathBuf) {
        self.compiling = true;
        self.model = None;
        self.parse = Stage::default();
        self.resolve = Stage::default();
        self.instantiate = Stage::default();
        self.typecheck = Stage::default();
        self.flatten = Stage::default();
        self.structural = Stage::default();
        self.index_reduction = Stage::default();
        self.initialization = Stage::default();
        self.events = Stage::default();
        self.solve_lowering = Stage::default();
        self.sim_data = None;
        self.sim_error = None;
        self.sim_running = false;
        self.def_index = BTreeMap::new();
        self.stage_clicked = false;
        self.nav.clear();
        self.nav_loading = None;
        self.nav_error = None;
        self.selected_field = None;
        self.log_entries.clear();
        self.viewing_log = true;
        self.worker.send(ToWorker::Compile(path.clone()));
        self.selected = Some(path);
    }

    /// Fetch a class by qualified name for navigation (async; pushed on arrival).
    fn navigate_to(&mut self, name: String) {
        self.nav_loading = Some(name.clone());
        self.nav_error = None;
        self.worker.send(ToWorker::OpenDef(name));
    }

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
                    let StageBundle {
                        parse, resolve, instantiate, typecheck, flatten, structural,
                        index_reduction, initialization, events, solve_lowering,
                    } = stages;
                    self.parse = parse;
                    self.resolve = resolve;
                    self.instantiate = instantiate;
                    self.typecheck = typecheck;
                    self.flatten = flatten;
                    self.structural = structural;
                    self.index_reduction = index_reduction;
                    self.initialization = initialization;
                    self.events = events;
                    self.solve_lowering = solve_lowering;
                }
                FromWorker::Compiled {
                    path, model, parse, resolve, instantiate, typecheck, flatten, structural,
                    index_reduction, initialization, events, solve_lowering, def_index,
                } => {
                    if self.selected.as_deref() != Some(path.as_path()) {
                        continue; // stale result
                    }
                    self.compiling = false;
                    self.model = model;
                    self.parse = parse;
                    self.resolve = resolve;
                    self.instantiate = instantiate;
                    self.typecheck = typecheck;
                    self.flatten = flatten;
                    self.structural = structural;
                    self.index_reduction = index_reduction;
                    self.initialization = initialization;
                    self.events = events;
                    self.solve_lowering = solve_lowering;
                    self.def_index = def_index;
                    // Re-fit the custom-view cameras to the new report.
                    self.spy_canvas.request_fit();
                    self.incidence_canvas.request_fit();
                    // Land on the furthest stage that completed cleanly.
                    self.stage = self.last_successful_stage();
                    // Publish every stage's full IR so Claude can diff any pair.
                    let _ = bridge::write_stages(&[
                        ("parse", self.parse.value.as_ref()),
                        ("resolve", self.resolve.value.as_ref()),
                        ("instantiate", self.instantiate.value.as_ref()),
                        ("typecheck", self.typecheck.value.as_ref()),
                        ("flatten", self.flatten.value.as_ref()),
                        ("structural", self.structural.value.as_ref()),
                        ("index_reduction", self.index_reduction.value.as_ref()),
                        ("initialization", self.initialization.value.as_ref()),
                        ("events", self.events.value.as_ref()),
                        ("solve_lowering", self.solve_lowering.value.as_ref()),
                    ]);
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

    fn current_stage(&self) -> &Stage {
        match self.stage {
            StageKind::Parse => &self.parse,
            StageKind::Resolve => &self.resolve,
            StageKind::Instantiate => &self.instantiate,
            StageKind::Typecheck => &self.typecheck,
            StageKind::Flatten => &self.flatten,
            StageKind::Structural => &self.structural,
            StageKind::IndexReduction => &self.index_reduction,
            StageKind::Initialization => &self.initialization,
            StageKind::Events => &self.events,
            StageKind::SolveLowering => &self.solve_lowering,
            // Simulation isn't a compile stage; this placeholder is always empty
            // (the Simulation view is the plot pane, rendered specially).
            StageKind::Simulation => &self.simulation,
        }
    }

    fn stage_name(&self) -> &'static str {
        match self.stage {
            StageKind::Parse => "Parse",
            StageKind::Resolve => "Resolve",
            StageKind::Instantiate => "Instantiate",
            StageKind::Typecheck => "Typecheck",
            StageKind::Flatten => "Flatten",
            StageKind::Structural => "Structural",
            StageKind::IndexReduction => "Index reduction",
            StageKind::Initialization => "Initialization",
            StageKind::Events => "Events",
            StageKind::SolveLowering => "Solve lowering",
            StageKind::Simulation => "Simulation",
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
                .parse
                .value
                .as_ref()?
                .get("classes")?
                .get(self.model.as_deref()?),
            StageKind::Instantiate => self.resolve.value.as_ref(),
            StageKind::Typecheck => self.instantiate.value.as_ref(),
            StageKind::Flatten => self.typecheck.value.as_ref(),
            // The structural report is a different shape from the flat model —
            // no path-aligned previous, so nothing to highlight.
            StageKind::Structural => None,
            // Diff the reduced report against the raw one: for an already-index-1
            // model they're identical (nothing highlights); for a reduced
            // high-index model the raw report is absent (it was singular).
            StageKind::IndexReduction => self.structural.value.as_ref(),
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
        if ok(&self.solve_lowering) {
            StageKind::SolveLowering
        } else if ok(&self.events) {
            StageKind::Events
        } else if ok(&self.initialization) {
            StageKind::Initialization
        } else if ok(&self.index_reduction) {
            StageKind::IndexReduction
        } else if ok(&self.structural) {
            StageKind::Structural
        } else if ok(&self.flatten) {
            StageKind::Flatten
        } else if ok(&self.typecheck) {
            StageKind::Typecheck
        } else if ok(&self.instantiate) {
            StageKind::Instantiate
        } else if ok(&self.resolve) {
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
    fn emit_focus(&mut self, focus: Focus) {
        self.ask_seq += 1;
        let seq = self.ask_seq;
        // Name what was captured, so the status confirms the *right* thing was
        // written — not just that some focus was.
        let target = match &focus {
            Focus::Node { key_path, .. } => bridge::describe_path(key_path),
            Focus::Stage => format!("stage “{}”", self.stage_name()),
            Focus::Specimen => format!(
                "specimen “{}”",
                self.selected
                    .as_deref()
                    .and_then(|p| p.file_name())
                    .and_then(|n| n.to_str())
                    .unwrap_or("?"),
            ),
        };
        let ask = Ask {
            seq,
            request: "explain",
            specimen: self.selected.as_deref(),
            model: self.model.as_deref(),
            stage: self.stage_name(),
            libraries: self.library_strings(),
            def_index: &self.def_index,
            parse_value: self.parse.value.as_ref(),
            resolve_value: self.resolve.value.as_ref(),
            focus,
        };
        self.bridge_status = Some(status_line(seq, &target, "explain", bridge::write(&ask)));
    }

    /// Capture the node the user acted on — scoped to the navigated class when
    /// navigating, else to the current specimen stage (with cross-stage diff).
    /// `request` is "explain" (Ask Claude) or "debug-where-set" (🐞 debugger).
    fn emit_node_focus(&mut self, key_path: Vec<Seg>, request: &'static str) {
        self.ask_seq += 1;
        let seq = self.ask_seq;
        let target = bridge::describe_path(&key_path);
        let libraries = self.library_strings();

        let status = if let Some(entry) = self.nav.last() {
            // Navigated library class: no Parse stage, so no cross-stage diff.
            let ask = Ask {
                seq,
                request,
                specimen: None,
                model: Some(&entry.name),
                stage: "(navigated definition)",
                libraries,
                def_index: &entry.def_index,
                parse_value: None,
                resolve_value: None,
                focus: Focus::Node { key_path, stage_value: &entry.value },
            };
            status_line(seq, &target, request, bridge::write(&ask))
        } else {
            let stage_value = self.current_stage().value.clone();
            match &stage_value {
                Some(value) => {
                    let ask = Ask {
                        seq,
                        request,
                        specimen: self.selected.as_deref(),
                        model: self.model.as_deref(),
                        stage: self.stage_name(),
                        libraries,
                        def_index: &self.def_index,
                        parse_value: self.parse.value.as_ref(),
                        resolve_value: self.resolve.value.as_ref(),
                        focus: Focus::Node { key_path, stage_value: value },
                    };
                    status_line(seq, &target, request, bridge::write(&ask))
                }
                None => "(no IR for this stage to point at)".to_owned(),
            }
        };
        self.bridge_status = Some(status);
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
        if run
            && let (Some(path), Some(model)) = (self.selected.clone(), self.model.clone())
        {
            self.sim_running = true;
            self.sim_data = None;
            self.sim_error = None;
            self.worker.send(ToWorker::Simulate { path, model, t_end: self.sim_t_end });
        }
        ui.separator();
        match &self.sim_data {
            Some(data) => {
                Plot::new("sim_plot")
                    // Top-LEFT so the legend clears the right-hand panel (the
                    // default top-right corner sits against it).
                    .legend(Legend::default().position(Corner::LeftTop))
                    .x_axis_label("time")
                    .show(ui, |plot_ui| {
                        for (i, name) in data.names.iter().enumerate() {
                            let series = &data.data[i];
                            // A model with discrete updates can jump a variable at an
                            // event (BouncingBall's velocity flips at each bounce). Break
                            // the polyline there so the plot shows a true discontinuity,
                            // not a sloped line through the jump. Continuous models draw
                            // as one segment.
                            let segments = if data.has_discontinuities {
                                discontinuity_segments(series)
                            } else {
                                std::iter::once(0..series.len()).collect()
                            };
                            // Pin an explicit colour per VARIABLE. egui_plot's auto-colour
                            // increments per Line added, so a variable's multiple segments
                            // would otherwise each get a different hue while the legend
                            // (grouped by name) shows only one. Keyed on `i`, every segment
                            // matches the legend and equals the old one-line-per-variable
                            // colour.
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
            }
            None if !self.sim_running => {
                ui.weak("Press ▶ Run to simulate this specimen and plot its state trajectories.");
            }
            None => {}
        }
    }

    /// The right-hand context panel. Before any stage tab is clicked it shows
    /// specimen-level info; after a click it shows stage-specific content
    /// (Simulation gets its own panel; everything else gets field help).
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
        if let Some(model) = &self.model {
            let rel = format!("docs/specimen-notebook/{model}/narrative.md");
            let abs = format!("{}/{}", env!("CARGO_MANIFEST_DIR"), rel);
            if std::path::Path::new(&abs).exists() {
                ui.add_space(8.0);
                ui.separator();
                if ui
                    .button("Read: specimen narrative")
                    .on_hover_text(&rel)
                    .clicked()
                {
                    let _ = std::process::Command::new("code").arg(&abs).spawn();
                }
            }
        }
    }

    /// Generic (build-time) field help for the last-clicked tree item — the fast
    /// tier (no Claude). Shown for every stage whose view is the IR tree.
    fn right_panel_field_help(&mut self, ui: &mut egui::Ui) {
        // Title the pane with the stage on screen (e.g. "Flatten") — "About this
        // field" was meaningless when nothing was selected. While navigating a
        // definition the view is Resolve context, so say so.
        let title = if self.nav.is_empty() { self.stage_name() } else { "Resolve" };
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
    fn right_panel_read_links(&self, ui: &mut egui::Ui) {
        // Concept-level link: the docs/compiler-phases chapter for the phase whose
        // view is on screen (Resolve while navigating a definition).
        let stage_ctx = if self.nav.is_empty() { self.stage_name() } else { "Resolve" };
        let (label, rel) = field_help::chapter_for_stage(stage_ctx);
        if ui
            .button(format!("Read: {label}"))
            .on_hover_text("Open this docs/compiler-phases chapter (generic phase theory) in your editor")
            .clicked()
        {
            let abs = format!("{}/{}", env!("CARGO_MANIFEST_DIR"), rel);
            let _ = std::process::Command::new("code").arg(abs).spawn();
        }
        // Specimen-specific link: this specimen's compilation narrative, when one exists.
        if self.nav.is_empty()
            && let Some(model) = &self.model
        {
            let rel = format!("docs/specimen-notebook/{model}/narrative.md");
            let abs = format!("{}/{}", env!("CARGO_MANIFEST_DIR"), rel);
            if std::path::Path::new(&abs).exists()
                && ui
                    .button("Read: specimen narrative")
                    .on_hover_text(rel)
                    .clicked()
            {
                let _ = std::process::Command::new("code").arg(&abs).spawn();
            }
        }
    }
}

/// One-line status for a completed bridge write, tailored to the request kind.
fn status_line(seq: u64, target: &str, request: &str, result: std::io::Result<std::path::PathBuf>) -> String {
    match result {
        Err(e) => format!("bridge write failed: {e}"),
        Ok(_) if request == "debug-where-set" => {
            format!("🐞 captured  {target}  for the debugger — say “arm it” in chat and I'll set the breakpoint  (focus #{seq})")
        }
        Ok(_) => format!("captured  {target}  — now ask me about it in the chat  (focus #{seq})"),
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.drain_worker();

        egui::Panel::top("bar").show(ui, |ui| {
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

        // Help / About windows (visibility driven by the menu-bar toggles;
        // the close button flips the bool back via `.open`).
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
                    "Shortcut: right after capturing, just type “explain” in the chat — Claude \
                     explains what you captured, no need to phrase a question.",
                );
                ui.add_space(6.0);
                ui.strong("Diff stages");
                ui.label(
                    "Every capture publishes all five stages' full IR, so Claude can compare any \
                     two on request. Capture anything, then ask in the chat — e.g. “what did \
                     Typecheck change vs Instantiate?” (the resolved type_ids) or “diff Parse and \
                     Resolve here” (def_ids filled in) — and Claude reads the two stages and reports \
                     the differences. (A node captured on Parse/Resolve also carries its own \
                     before/after inline, so “explain” alone shows what Resolve changed.)",
                );
                ui.add_space(6.0);
                ui.strong("Structural (spy-plot)");
                ui.label(
                    "On the Structural stage, the BLT block structure is drawn as a spy-plot: \
                     diagonal blocks in evaluation order — scalar solves are single cells, coupled \
                     algebraic loops are boxes. Drag to pan, scroll to zoom, hover a block to see its \
                     equations/unknowns/tearing, and click it to capture it for “explain”. Toggle to \
                     Tree for the raw report.",
                );
                ui.add_space(6.0);
                ui.strong("Navigate");
                ui.label(
                    "Some fields hold a DefId that resolves to a class — the tree shows it inline \
                     (e.g. “type_def_id: 27579 → model …”). Right-click that field and choose \
                     “Go to …” to open that class's own IR. Use Back to step up one level, or \
                     Specimen to return to the top.",
                );
                ui.add_space(6.0);
                ui.strong("Debugger");
                ui.label(
                    "Right-click a field and choose 🐞 “Show this being set”, then tell Claude \
                     “arm it” in the chat. Claude sets a breakpoint where Rumoca assigns that field; \
                     launch “Debug HRW — break where Claude armed” and select the specimen.",
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
                // Derived from Cargo.lock by build.rs — always matches what was
                // compiled in; never hand-edit (see docs/updating-rumoca.md).
                ui.label(format!(
                    "Built against Rumoca {} · git {}",
                    env!("HRW_RUMOCA_VERSION"),
                    env!("HRW_RUMOCA_REV"),
                ));
                ui.label("Rumoca is linked as a library; compilation runs on a worker thread.");
            });

        // Settings window. The library "Load" is deferred to a flag so the
        // closure only borrows disjoint fields (not whole `self` via a method),
        // which keeps it compatible with `.open(&mut self.show_settings)`.
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

        // Full-width status bar (added before the side panel so it spans the
        // whole window bottom): the last bridge capture / write result.
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.add_space(1.0);
            match &self.bridge_status {
                Some(s) => ui.weak(s),
                None => ui.weak("Left-click a tree node to capture it, then ask about it in the chat (right-click for more actions)."),
            };
            ui.add_space(1.0);
        });

        let specimen_width = {
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
        };
        egui::Panel::left("file_list")
            .resizable(true)
            .default_size(specimen_width)
            .min_size(120.0)
            .show(ui, |ui| {
                ui.strong("Specimens");
                ui.separator();

                if let Some(err) = &self.scan_error {
                    ui.colored_label(ui.visuals().error_fg_color, err);
                    return;
                }
                if self.files.is_empty() {
                    ui.weak("(no .mo specimens found)");
                    return;
                }

                egui::ScrollArea::vertical().show(ui, |ui| {
                    let mut to_open = None;
                    let mut capture_specimen = false;
                    for path in &self.files {
                        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("<?>");
                        let selected = self.selected.as_deref() == Some(path.as_path());
                        // Whole-specimen capture is offered only once this specimen is
                        // the loaded, done-compiling one — so it captures real IR, and we
                        // compile exactly once (the left-click load), never again just to
                        // capture.
                        let can_capture = selected && !self.compiling && self.model.is_some();
                        let purpose = self.specimen_purposes.get(path);
                        let mut resp = ui.selectable_label(selected, name);
                        if let Some(hint) = purpose {
                            resp = resp.on_hover_text(hint);
                        }
                        // Right-click → "🔎 Capture" the whole specimen (mirrors the tree
                        // rows). Disabled until this specimen has finished compiling, so
                        // the capture carries its IR — no second compile.
                        resp.context_menu(|ui| {
                            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
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
                    if let Some(path) = to_open {
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

        // Right panel: generic (build-time) field help for the last-clicked tree
        // item — the FAST tier (no Claude). The specific tier ("why did THIS one
        // happen") is the bridge + chat, via "explain".
        egui::Panel::right("field_help")
            .resizable(true)
            .default_size(380.0)
            .min_size(220.0)
            .show(ui, |ui| self.right_panel(ui));

        // Bridge "ask" + navigation requests collected during this frame, acted
        // on after the panel closure releases its borrow of `self`.
        let mut node_ask: Option<Vec<Seg>> = None;
        let mut debug_ask: Option<Vec<Seg>> = None;
        let mut canvas_capture: Option<Vec<Seg>> = None;
        let mut nav_to: Option<String> = None;
        let mut want_stage_ask = false;
        let mut go_back = false;
        let mut go_home = false;

        egui::CentralPanel::default().show(ui, |ui| {
            if self.nav.is_empty() {
                // ---- Specimen stage view ----
                // No specimen yet → no stages to show, so don't render the tab
                // row (a highlighted tab before any compile is misleading).
                if self.selected.is_none() {
                    ui.weak("Select a specimen to compile.");
                    return;
                }
                ui.horizontal_wrapped(|ui| {
                    // Stage selectors + capture-stage, a divider, then the model
                    // label + capture-model. Wrapped: with 11 stage tabs the row
                    // won't fit a narrow window, so it flows onto a second line
                    // rather than pushing Simulation off the right edge. Whole-
                    // stage/model buttons capture context; node-level captures come
                    // from the right-click menu on any tree row. A stage's tab label is
                    // painted red when that stage errored, so failed stages are
                    // visible without opening each (e.g. CapacitorLoop fails at
                    // Structural + Index reduction while landing on Initialization).
                    if ui.selectable_label(self.viewing_log, "Log").clicked() {
                        self.viewing_log = true;
                    }
                    ui.separator();
                    let can_sim = !self.compiling
                        && !self.sim_running
                        && self.model.is_some()
                        && self.solve_lowering.value.is_some();
                    if ui
                        .add_enabled(can_sim, egui::Button::new("▶"))
                        .on_hover_text("Run simulation (stays on the current view)")
                        .on_disabled_hover_text("Compile a specimen first")
                        .clicked()
                    {
                        if let (Some(path), Some(model)) =
                            (self.selected.clone(), self.model.clone())
                        {
                            self.sim_running = true;
                            self.sim_data = None;
                            self.sim_error = None;
                            self.worker.send(ToWorker::Simulate {
                                path,
                                model,
                                t_end: self.sim_t_end,
                            });
                        }
                    }
                    if self.sim_running {
                        ui.spinner();
                    }
                    ui.separator();
                    let err = ui.visuals().error_fg_color;
                    let ok = if ui.visuals().dark_mode {
                        egui::Color32::from_rgb(0x3f, 0xb9, 0x50)
                    } else {
                        egui::Color32::from_rgb(0x1a, 0x7f, 0x37)
                    };
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
                    if ui.selectable_label(stage_selected && self.stage == StageKind::Parse, tab_label("Parse", &self.parse, ok, err)).clicked() {
                        self.stage = StageKind::Parse;
                        stage_tab_clicked = true;
                    }
                    if ui.selectable_label(stage_selected && self.stage == StageKind::Resolve, tab_label("Resolve", &self.resolve, ok, err)).clicked() {
                        self.stage = StageKind::Resolve;
                        stage_tab_clicked = true;
                    }
                    if ui.selectable_label(stage_selected && self.stage == StageKind::Instantiate, tab_label("Instantiate", &self.instantiate, ok, err)).clicked() {
                        self.stage = StageKind::Instantiate;
                        stage_tab_clicked = true;
                    }
                    if ui.selectable_label(stage_selected && self.stage == StageKind::Typecheck, tab_label("Typecheck (instanced)", &self.typecheck, ok, err))
                        .on_hover_text(
                            "The model-scoped instanced typecheck: it types the instantiated \
                             overlay (fills in type_ids, evaluates dimensions), so it runs AFTER \
                             Instantiate — not in Rumoca's nominal phase-3 slot. HRW can't use the \
                             pre-instantiation whole-tree typecheck; it fails on the full MSL.",
                        )
                        .clicked()
                    {
                        self.stage = StageKind::Typecheck;
                        stage_tab_clicked = true;
                    }
                    if ui.selectable_label(stage_selected && self.stage == StageKind::Flatten, tab_label("Flatten", &self.flatten, ok, err)).clicked() {
                        self.stage = StageKind::Flatten;
                        stage_tab_clicked = true;
                    }
                    if ui.selectable_label(stage_selected && self.stage == StageKind::Structural, tab_label("Structural", &self.structural, ok, err))
                        .on_hover_text(
                            "Structural analysis of the RAW DAE (Rumoca phase 7): maximum matching \
                             (equation↔unknown), BLT blocks (size>1 = algebraic loop), and tearing. \
                             A high-index system (rigid constraints) reports SINGULAR here — see the \
                             Index reduction tab for the reduced, solvable form. BLT spy-plot (drag \
                             to pan, scroll to zoom, click a block to capture) or the raw report tree.",
                        )
                        .clicked()
                    {
                        self.stage = StageKind::Structural;
                        stage_tab_clicked = true;
                    }
                    if ui.selectable_label(stage_selected && self.stage == StageKind::IndexReduction, tab_label("Index reduction", &self.index_reduction, ok, err))
                        .on_hover_text(
                            "Structural analysis of the DAE AFTER index reduction (Pantelides / \
                             dummy derivatives): the funnel differentiates constraints and demotes states \
                             so a high-index singular system becomes matchable. For an already-index-1 \
                             model this equals Structural. Same BLT spy-plot / tree.",
                        )
                        .clicked()
                    {
                        self.stage = StageKind::IndexReduction;
                        stage_tab_clicked = true;
                    }
                    if ui.selectable_label(stage_selected && self.stage == StageKind::Initialization, tab_label("Initialization", &self.initialization, ok, err))
                        .on_hover_text(
                            "The consistent-initial-condition solve plan (build_ic_plan): the \
                             ordered blocks that compute a valid state at t=0 — direct symbolic solves, \
                             scalar Newton, torn/coupled loops — plus the relaxation hint (equations \
                             dropped / unknowns pinned) when the initial subsystem is singular, and a \
                             determinacy check that flags an OVER-determined init (more explicit initial \
                             conditions than states — conflicting/redundant ICs).",
                        )
                        .clicked()
                    {
                        self.stage = StageKind::Initialization;
                        stage_tab_clicked = true;
                    }
                    if ui.selectable_label(stage_selected && self.stage == StageKind::Events, tab_label("Events", &self.events, ok, err))
                        .on_hover_text(
                            "The DAE's hybrid / event structure: the conditions (relations that \
                             trigger events), the discrete updates lowered from `when` clauses (f_z real, \
                             f_m valued), and the event partition (zero-crossing root conditions + scheduled \
                             time events). A smooth (continuous) model shows none.",
                        )
                        .clicked()
                    {
                        self.stage = StageKind::Events;
                        stage_tab_clicked = true;
                    }
                    if ui.selectable_label(stage_selected && self.stage == StageKind::SolveLowering, tab_label("Solve lowering", &self.solve_lowering, ok, err))
                        .on_hover_text(
                            "The DAE lowered to a SolveModel (phase 8): the solvable form the \
                             simulator runs — residual programs, variable layout, mass matrix, Jacobian \
                             sparsity. This is the compile step just before simulation.",
                        )
                        .clicked()
                    {
                        self.stage = StageKind::SolveLowering;
                        stage_tab_clicked = true;
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
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut self.structural_view, StructuralView::SpyPlot, "Spy-plot");
                        ui.selectable_value(&mut self.structural_view, StructuralView::Incidence, "Incidence");
                        if is_index_reduction {
                            ui.selectable_value(&mut self.structural_view, StructuralView::Reduction, "Reduction");
                        }
                        ui.selectable_value(&mut self.structural_view, StructuralView::Tree, "Tree");
                    });
                    ui.separator();
                }

                if report_ready && self.structural_view == StructuralView::SpyPlot {
                    match self.current_stage().value.as_ref().and_then(spyplot::Plot::from_report) {
                        Some(plot) => {
                            ui.weak(plot.caption());
                            plot.ui(ui, &mut self.spy_canvas, &mut canvas_capture);
                        }
                        None => {
                            ui.weak("(the structural report has no BLT blocks to plot)");
                        }
                    }
                } else if report_ready && self.structural_view == StructuralView::Incidence {
                    match self.current_stage().value.as_ref().and_then(incidence_view::IncidenceMatrix::from_report) {
                        Some(mat) => {
                            ui.weak(mat.caption());
                            mat.ui(ui, &mut self.incidence_canvas, &mut canvas_capture);
                        }
                        None => {
                            ui.weak("(no incidence data in this report)");
                        }
                    }
                } else if report_ready && self.structural_view == StructuralView::Reduction {
                    match self.current_stage().value.as_ref().and_then(reduction_view::ReductionView::from_report) {
                        Some(view) => {
                            view.ui(ui);
                        }
                        None => {
                            ui.weak("(no reduction data in this report)");
                        }
                    }
                } else {
                    let stage = self.current_stage();
                    match &stage.value {
                        Some(value) => {
                            let label = self.model.as_deref().unwrap_or("model");
                            let prev = self.previous_stage_value();
                            egui::ScrollArea::both().id_salt("tree").auto_shrink(false).show(ui, |ui| {
                                tree::tree_ui(ui, label, value, prev, &mut node_ask, &mut nav_to, &mut debug_ask, &self.def_index);
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
                    tree::tree_ui(ui, &entry.name, &entry.value, None, &mut node_ask, &mut nav_to, &mut debug_ask, &entry.def_index);
                });
            }
        });

        // Act on requests now that `self` is no longer borrowed by the UI.
        if go_home {
            self.nav.clear();
        } else if go_back {
            self.nav.pop();
        }
        if let Some(name) = nav_to {
            self.navigate_to(name);
        }
        // A spy-plot block click is an "explain" capture, same as a tree click.
        if canvas_capture.is_some() {
            node_ask = canvas_capture;
        }
        // Populate the generic field-help panel from whichever node was clicked.
        if let Some(kp) = debug_ask.as_ref().or(node_ask.as_ref()) {
            self.selected_field = field_name_from_path(kp);
        }
        if let Some(key_path) = debug_ask {
            self.emit_node_focus(key_path, "debug-where-set");
        } else if let Some(key_path) = node_ask {
            self.emit_node_focus(key_path, "explain");
        } else if want_stage_ask {
            self.emit_focus(Focus::Stage);
        }
    }
}

/// Read a specimen's one-line purpose hint — the first `// purpose:` comment in
/// the file (the phenomenon it's authored to exercise). Scanned without compiling
/// so every file in the list gets a hint, even one that fails to compile. `None`
/// if the convention is absent.
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
fn series_color(i: usize) -> egui::Color32 {
    let golden_ratio = (5.0_f32.sqrt() - 1.0) / 2.0; // 0.61803398875
    egui::ecolor::Hsva::new(i as f32 * golden_ratio, 0.85, 0.5, 1.0).into()
}

/// A stage-tab label, coloured by outcome so the whole pipeline's health reads off
/// the tab row without opening each stage: **red** if the stage errored, **green**
/// if it produced its IR (succeeded), and the normal colour for an
/// in-between/neutral status — "not reached" after an upstream failure, or no data
/// yet (before/while compiling).
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
        text
    }
}

/// The field name to look up generic help for = the last object-key segment in
/// the clicked path (an array-index tail falls back to its enclosing field).
fn field_name_from_path(path: &[Seg]) -> Option<String> {
    path.iter().rev().find_map(|seg| match seg {
        Seg::Key(k) => Some(k.clone()),
        Seg::Index(_) => None,
    })
}
