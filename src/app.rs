//! The observatory shell.
//!
//! Arc 1 (charter §4.2.1, §4.4): eframe shell, a file picker over the specimen
//! directory, a library-path (source-root) configuration for dependency
//! resolution, and the generic serde-value tree inspector showing each stage's
//! IR for the selected model. Stages present so far: Parse, Resolve.

use std::path::{Path, PathBuf};

use eframe::egui;

use std::collections::{BTreeMap, HashMap};

use crate::bridge::{self, Ask, Focus, Seg};
use crate::canvas::Canvas;
use crate::field_help;
use crate::spyplot;
use crate::tree;
use crate::worker::{DefInfo, FromWorker, SimData, Stage, ToWorker, Worker};

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

/// How to render the Structural stage: the custom BLT spy-plot (the visual
/// emitter) or the generic serde tree over the same report. Only this stage has
/// a custom view; every other stage is always the tree.
#[derive(Clone, Copy, PartialEq, Eq)]
enum StructuralView {
    SpyPlot,
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

    // Arc 3: the Structural stage's custom BLT spy-plot and its pan/zoom camera.
    structural_view: StructuralView,
    spy_canvas: Canvas,

    // Arc 7: on-demand simulation (the Simulation tab) — not a compile stage.
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
        self.nav.clear();
        self.nav_loading = None;
        self.nav_error = None;
        self.selected_field = None;
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
                    // Re-fit the spy-plot camera to the new report's matrix.
                    self.spy_canvas.request_fit();
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
            Focus::Model => format!("model “{}”", self.model.as_deref().unwrap_or("?")),
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

    /// Arc 7: the Simulation view — a Run control + an `egui_plot` pane of the
    /// state trajectories. Running the model is on-demand (not a compile stage):
    /// Run dispatches `ToWorker::Simulate` to the worker thread, and the plot
    /// appears when `FromWorker::Simulated` lands (see `drain_worker`).
    fn simulation_pane(&mut self, ui: &mut egui::Ui) {
        use egui_plot::{Legend, Line, Plot, PlotPoints};

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
            self.sim_error = None;
            self.worker.send(ToWorker::Simulate { path, model, t_end: self.sim_t_end });
        }
        ui.separator();
        match &self.sim_data {
            Some(data) => {
                Plot::new("sim_plot")
                    .legend(Legend::default())
                    .x_axis_label("time")
                    .show(ui, |plot_ui| {
                        for (i, name) in data.names.iter().enumerate() {
                            let pts: PlotPoints = data
                                .times
                                .iter()
                                .zip(&data.data[i])
                                .map(|(&t, &y)| [t, y])
                                .collect();
                            plot_ui.line(Line::new(name.clone(), pts));
                        }
                    });
            }
            None if !self.sim_running => {
                ui.weak("Press ▶ Run to simulate this specimen and plot its state trajectories.");
            }
            None => {}
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

        egui::Panel::left("file_list")
            .resizable(true)
            .default_size(440.0)
            .min_size(340.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.strong("Specimens");
                    if ui.button("⟳").on_hover_text("Rescan directory").clicked() {
                        self.rescan();
                    }
                });
                ui.add(
                    egui::TextEdit::singleline(&mut self.specimen_dir)
                        .desired_width(f32::INFINITY)
                        .font(egui::TextStyle::Monospace),
                );
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
                    for path in &self.files {
                        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("<?>");
                        let selected = self.selected.as_deref() == Some(path.as_path());
                        let purpose = self.specimen_purposes.get(path);
                        let mut resp = ui.selectable_label(selected, name);
                        if let Some(hint) = purpose {
                            resp = resp.on_hover_text(hint);
                            // Weak, truncated subtext turns the list into an index
                            // of what each specimen demonstrates; hover for the full line.
                            ui.horizontal(|ui| {
                                ui.add_space(14.0);
                                ui.add(
                                    egui::Label::new(egui::RichText::new(hint).weak().small())
                                        .truncate(),
                                )
                                .on_hover_text(hint);
                            });
                        }
                        if resp.clicked() {
                            to_open = Some(path.clone());
                        }
                    }
                    if let Some(path) = to_open {
                        self.open(path);
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
            .show(ui, |ui| {
                ui.strong("About this field");
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
                // Concept-level link: the docs/compiler-phases chapter for the phase
                // whose IR is on screen (Resolve while navigating a definition).
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
                // Specimen-specific link: this specimen's compilation narrative
                // (docs/specimen-notebook/<model>/narrative.md), shown only when one exists.
                // The generic chapter above is phase theory; this is the story of
                // *this* specimen's trip through the pipeline, grounded in its trace.
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
            });

        // Bridge "ask" + navigation requests collected during this frame, acted
        // on after the panel closure releases its borrow of `self`.
        let mut node_ask: Option<Vec<Seg>> = None;
        let mut debug_ask: Option<Vec<Seg>> = None;
        let mut canvas_capture: Option<Vec<Seg>> = None;
        let mut nav_to: Option<String> = None;
        let mut want_stage_ask = false;
        let mut want_model_ask = false;
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
                ui.horizontal(|ui| {
                    // One row: stage selectors + capture-stage, then a divider,
                    // then the model label + capture-model. Whole-stage/model
                    // buttons capture context; node-level captures come from the
                    // right-click menu on any tree row. A stage's tab label is
                    // painted red when that stage errored, so failed stages are
                    // visible without opening each (e.g. CapacitorLoop fails at
                    // Structural + Index reduction while landing on Initialization).
                    let err = ui.visuals().error_fg_color;
                    ui.selectable_value(&mut self.stage, StageKind::Parse, tab_label("Parse", self.parse.note_is_error, err));
                    ui.selectable_value(&mut self.stage, StageKind::Resolve, tab_label("Resolve", self.resolve.note_is_error, err));
                    ui.selectable_value(&mut self.stage, StageKind::Instantiate, tab_label("Instantiate", self.instantiate.note_is_error, err));
                    ui.selectable_value(&mut self.stage, StageKind::Typecheck, tab_label("Typecheck (instanced)", self.typecheck.note_is_error, err))
                        .on_hover_text(
                            "The model-scoped instanced typecheck: it types the instantiated \
                             overlay (fills in type_ids, evaluates dimensions), so it runs AFTER \
                             Instantiate — not in Rumoca's nominal phase-3 slot. HRW can't use the \
                             pre-instantiation whole-tree typecheck; it fails on the full MSL.",
                        );
                    ui.selectable_value(&mut self.stage, StageKind::Flatten, tab_label("Flatten", self.flatten.note_is_error, err));
                    ui.selectable_value(&mut self.stage, StageKind::Structural, tab_label("Structural", self.structural.note_is_error, err))
                        .on_hover_text(
                            "Structural analysis of the RAW DAE (Rumoca phase 7): maximum matching \
                             (equation↔unknown), BLT blocks (size>1 = algebraic loop), and tearing. \
                             A high-index system (rigid constraints) reports SINGULAR here — see the \
                             Index reduction tab for the reduced, solvable form. BLT spy-plot (drag \
                             to pan, scroll to zoom, click a block to capture) or the raw report tree.",
                        );
                    ui.selectable_value(&mut self.stage, StageKind::IndexReduction, tab_label("Index reduction", self.index_reduction.note_is_error, err))
                        .on_hover_text(
                            "Structural analysis of the DAE AFTER index reduction (Arc 4, Pantelides / \
                             dummy derivatives): the funnel differentiates constraints and demotes states \
                             so a high-index singular system becomes matchable. For an already-index-1 \
                             model this equals Structural. Same BLT spy-plot / tree.",
                        );
                    ui.selectable_value(&mut self.stage, StageKind::Initialization, tab_label("Initialization", self.initialization.note_is_error, err))
                        .on_hover_text(
                            "The consistent-initial-condition solve plan (Arc 5, build_ic_plan): the \
                             ordered blocks that compute a valid state at t=0 — direct symbolic solves, \
                             scalar Newton, torn/coupled loops — plus the relaxation hint (equations \
                             dropped / unknowns pinned) when the initial subsystem is singular, and a \
                             determinacy check that flags an OVER-determined init (more explicit initial \
                             conditions than states — conflicting/redundant ICs).",
                        );
                    ui.selectable_value(&mut self.stage, StageKind::Events, tab_label("Events", self.events.note_is_error, err))
                        .on_hover_text(
                            "The DAE's hybrid / event structure (Arc 6): the conditions (relations that \
                             trigger events), the discrete updates lowered from `when` clauses (f_z real, \
                             f_m valued), and the event partition (zero-crossing root conditions + scheduled \
                             time events). A smooth (continuous) model shows none.",
                        );
                    ui.selectable_value(&mut self.stage, StageKind::SolveLowering, tab_label("Solve lowering", self.solve_lowering.note_is_error, err))
                        .on_hover_text(
                            "The DAE lowered to a SolveModel (Arc 7, phase 8): the solvable form the \
                             simulator runs — residual programs, variable layout, mass matrix, Jacobian \
                             sparsity. This is the compile step just before simulation.",
                        );
                    ui.selectable_value(&mut self.stage, StageKind::Simulation, "▶ Simulation")
                        .on_hover_text(
                            "Run the model (Arc 7, phase 9): compile → lower to a SolveModel → integrate \
                             (Auto: BDF for stiff, RK45 otherwise), then plot the state trajectories. Runs \
                             on the worker thread, so the UI stays live.",
                        );
                    if self.selected.is_some()
                        && ui
                            .button("🔎 Capture")
                            .on_hover_text("Capture the whole current stage's IR, then ask Claude about it here in the chat.")
                            .clicked()
                    {
                        want_stage_ask = true;
                    }
                    ui.separator();
                    if let Some(m) = &self.model {
                        ui.label(egui::RichText::new(m).monospace().strong());
                    }
                    if self.selected.is_some()
                        && ui
                            .button("🔎 Capture")
                            .on_hover_text("Capture the specimen as a whole, then ask Claude about it here in the chat.")
                            .clicked()
                    {
                        want_model_ask = true;
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

                // Simulation is a plot, not IR — render the plot pane and skip the
                // whole note/report/tree block below.
                if self.stage == StageKind::Simulation {
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
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut self.structural_view, StructuralView::SpyPlot, "Spy-plot");
                        ui.selectable_value(&mut self.structural_view, StructuralView::Tree, "Tree");
                    });
                    ui.separator();
                }

                if report_ready && self.structural_view == StructuralView::SpyPlot {
                    // Build the plot (owns its strings, so the immutable borrow of
                    // the current stage is released before we touch `spy_canvas`).
                    match self.current_stage().value.as_ref().and_then(spyplot::Plot::from_report) {
                        Some(plot) => {
                            ui.weak(plot.caption());
                            plot.ui(ui, &mut self.spy_canvas, &mut canvas_capture);
                        }
                        None => {
                            ui.weak("(the structural report has no BLT blocks to plot)");
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
        } else if want_model_ask {
            self.emit_focus(Focus::Model);
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

/// A stage-tab label, painted red when that stage errored (`note_is_error`), so
/// failed stages are visible in the tab row without opening each one. A non-error
/// status ("not reached", "already index-1") stays the normal color.
fn tab_label(label: &str, failed: bool, err_color: egui::Color32) -> egui::RichText {
    let text = egui::RichText::new(label);
    if failed { text.color(err_color) } else { text }
}

/// The field name to look up generic help for = the last object-key segment in
/// the clicked path (an array-index tail falls back to its enclosing field).
fn field_name_from_path(path: &[Seg]) -> Option<String> {
    path.iter().rev().find_map(|seg| match seg {
        Seg::Key(k) => Some(k.clone()),
        Seg::Index(_) => None,
    })
}
