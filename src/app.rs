//! The observatory shell.
//!
//! Arc 1 (charter §4.2.1, §4.4): eframe shell, a file picker over the specimen
//! directory, a library-path (source-root) configuration for dependency
//! resolution, and the generic serde-value tree inspector showing each stage's
//! IR for the selected model. Stages present so far: Parse, Resolve.

use std::path::PathBuf;

use eframe::egui;

use crate::tree;
use crate::worker::{FromWorker, Stage, ToWorker, Worker};

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
    scan_error: Option<String>,

    // Current selection + results.
    selected: Option<PathBuf>,
    compiling: bool,
    model: Option<String>,
    parse: Stage,
    resolve: Stage,
    stage: StageKind,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let worker = Worker::spawn(cc.egui_ctx.clone());
        let mut app = App {
            worker,
            libraries_text: DEFAULT_LIBRARIES.to_owned(),
            library_status: String::new(),
            libraries_busy: false,
            specimen_dir: DEFAULT_SPECIMEN_DIR.to_owned(),
            files: Vec::new(),
            scan_error: None,
            selected: None,
            compiling: false,
            model: None,
            parse: Stage::default(),
            resolve: Stage::default(),
            stage: StageKind::Resolve,
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
    }

    fn open(&mut self, path: PathBuf) {
        self.compiling = true;
        self.model = None;
        self.parse = Stage::default();
        self.resolve = Stage::default();
        self.worker.send(ToWorker::Compile(path.clone()));
        self.selected = Some(path);
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
                FromWorker::Compiled { path, model, parse, resolve } => {
                    if self.selected.as_deref() != Some(path.as_path()) {
                        continue; // stale result
                    }
                    self.compiling = false;
                    self.model = model;
                    self.parse = parse;
                    self.resolve = resolve;
                }
            }
        }
    }

    fn current_stage(&self) -> &Stage {
        match self.stage {
            StageKind::Parse => &self.parse,
            StageKind::Resolve => &self.resolve,
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.drain_worker();

        egui::Panel::top("bar").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading("HRW Observatory");
                ui.separator();
                ui.label("Arc 1 · Parse → Resolve → Typecheck");
            });
            egui::CollapsingHeader::new("Libraries (source roots)")
                .default_open(false)
                .show(ui, |ui| {
                    ui.label("One package directory (or single .mo) per line:");
                    ui.add(
                        egui::TextEdit::multiline(&mut self.libraries_text)
                            .desired_rows(3)
                            .desired_width(f32::INFINITY)
                            .font(egui::TextStyle::Monospace),
                    );
                    ui.horizontal(|ui| {
                        if ui.add_enabled(!self.libraries_busy, egui::Button::new("Load libraries")).clicked() {
                            self.load_libraries();
                        }
                        if self.libraries_busy {
                            ui.spinner();
                        }
                        ui.weak(&self.library_status);
                    });
                });
            ui.add_space(2.0);
        });

        egui::Panel::left("file_list")
            .resizable(true)
            .default_size(240.0)
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
                        if ui.selectable_label(selected, name).clicked() {
                            to_open = Some(path.clone());
                        }
                    }
                    if let Some(path) = to_open {
                        self.open(path);
                    }
                });
            });

        egui::CentralPanel::default().show(ui, |ui| {
            // Stage selector.
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.stage, StageKind::Parse, "Parse");
                ui.selectable_value(&mut self.stage, StageKind::Resolve, "Resolve");
                ui.add_enabled(false, egui::Button::new("Typecheck"))
                    .on_disabled_hover_text(
                        "Model-scoped typecheck needs instantiation (Arc 2); \
                         whole-tree typecheck fails on the full MSL.",
                    );
                ui.separator();
                if let Some(m) = &self.model {
                    ui.label(egui::RichText::new(m).monospace().strong());
                }
                if self.compiling {
                    ui.spinner();
                }
            });
            ui.separator();

            if self.selected.is_none() {
                ui.weak("Select a specimen to compile.");
                return;
            }

            let stage = self.current_stage();
            if let Some(note) = &stage.note {
                let color = if stage.value.is_some() {
                    ui.visuals().warn_fg_color
                } else {
                    ui.visuals().error_fg_color
                };
                egui::ScrollArea::horizontal().id_salt("note").show(ui, |ui| {
                    ui.colored_label(color, egui::RichText::new(note).monospace());
                });
                ui.separator();
            }

            match &stage.value {
                Some(value) => {
                    let label = self.model.as_deref().unwrap_or("model");
                    egui::ScrollArea::both().id_salt("tree").show(ui, |ui| {
                        tree::tree_ui(ui, label, value);
                    });
                }
                None if stage.note.is_none() => {
                    ui.weak(if self.compiling { "compiling…" } else { "(no output for this stage)" });
                }
                None => {}
            }
        });
    }
}
