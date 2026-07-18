//! The compilation worker thread.
//!
//! Charter §4.4 / Decision 6: compilation runs on a worker thread, results
//! returned over a channel. The egui `update()` loop never blocks and never
//! calls into the compiler directly. Breakpoints for studying a phase belong
//! here (in `compile`), never in the paint path.
//!
//! The worker owns a persistent Rumoca `Session` — an incremental compilation
//! workspace (the same type the LSP uses). Library dependencies (the MSL) are
//! loaded once as **source roots**; thereafter each specimen edit re-resolves
//! incrementally (~0.3s) rather than re-parsing thousands of library files.
//!
//! For each stage we serialize only the **user model's** IR node (a few KB),
//! never the whole resolved aggregate — resolving with the full MSL loaded
//! produces a ~430MB tree, of which the user's model is a tiny slice.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use eframe::egui;
use rumoca_compile::compile::SourceRootKind;
use rumoca_compile::source_roots::{parse_source_root_with_cache, source_root_source_set_key};
use rumoca_compile::{Session, SessionConfig};

/// A request from the UI thread to the worker.
pub enum ToWorker {
    /// Replace the library source roots (reloads them into a fresh session).
    SetLibraries(Vec<PathBuf>),
    /// Run parse → resolve on a specimen file.
    Compile(PathBuf),
}

/// One pipeline stage's outcome for the selected model: the serialized IR node
/// (if the stage produced one) plus an optional note (error or status).
#[derive(Clone, Default)]
pub struct Stage {
    pub value: Option<serde_json::Value>,
    pub note: Option<String>,
}

impl Stage {
    fn ok(value: serde_json::Value) -> Self {
        Stage { value: Some(value), note: None }
    }
    fn err(note: impl Into<String>) -> Self {
        Stage { value: None, note: Some(note.into()) }
    }
}

/// A result from the worker back to the UI thread.
pub enum FromWorker {
    /// Outcome of loading libraries: total documents loaded, or an error.
    Libraries(Result<usize, String>),
    /// Outcome of compiling a specimen through the arc-1 stages.
    Compiled {
        path: PathBuf,
        /// Simple name of the model whose IR the stages show.
        model: Option<String>,
        parse: Stage,
        resolve: Stage,
    },
}

/// Handle held by the UI thread for talking to the worker.
pub struct Worker {
    tx: Sender<ToWorker>,
    pub rx: Receiver<FromWorker>,
}

impl Worker {
    /// Spawn the worker thread. `ctx` wakes the UI when a result is ready.
    pub fn spawn(ctx: egui::Context) -> Worker {
        let (tx_req, rx_req) = mpsc::channel::<ToWorker>();
        let (tx_res, rx_res) = mpsc::channel::<FromWorker>();

        thread::Builder::new()
            .name("rumoca-worker".to_owned())
            .spawn(move || {
                let mut state = WorkerState::new();
                while let Ok(msg) = rx_req.recv() {
                    let response = state.handle(msg);
                    if tx_res.send(response).is_err() {
                        break; // UI gone
                    }
                    ctx.request_repaint();
                }
            })
            .expect("failed to spawn rumoca-worker thread");

        Worker { tx: tx_req, rx: rx_res }
    }

    /// Send a request to the worker. Never blocks.
    pub fn send(&self, req: ToWorker) {
        let _ = self.tx.send(req);
    }
}

/// Worker-thread-owned state: the persistent session and its loaded libraries.
struct WorkerState {
    session: Session,
    /// Library roots currently loaded, so a specimen compile knows they're ready.
    libraries: Vec<PathBuf>,
}

impl WorkerState {
    fn new() -> Self {
        WorkerState { session: Session::new(SessionConfig::default()), libraries: Vec::new() }
    }

    /// Dispatch one request. Natural breakpoint site for studying a phase.
    fn handle(&mut self, msg: ToWorker) -> FromWorker {
        match msg {
            ToWorker::SetLibraries(roots) => FromWorker::Libraries(self.load_libraries(roots)),
            ToWorker::Compile(path) => self.compile(&path),
        }
    }

    /// Rebuild the session and load each library root as a durable source set.
    fn load_libraries(&mut self, roots: Vec<PathBuf>) -> Result<usize, String> {
        let mut session = Session::new(SessionConfig::default());
        let mut total = 0usize;
        for root in &roots {
            let parsed = parse_source_root_with_cache(root)
                .map_err(|e| format!("{}: {e:#}", root.display()))?;
            let key = source_root_source_set_key(&root.to_string_lossy());
            total += session.replace_parsed_source_set(
                &key,
                SourceRootKind::DurableExternal,
                parsed.documents,
                None,
            );
        }
        self.session = session;
        self.libraries = roots;
        Ok(total)
    }

    /// Run parse → resolve on a specimen, extracting the user model's IR at each
    /// stage. Typecheck is deferred: a clean, model-scoped typecheck needs
    /// instantiation (Arc 2); the pre-instantiation whole-tree typecheck fails
    /// on the full MSL.
    fn compile(&mut self, path: &Path) -> FromWorker {
        let source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                return FromWorker::Compiled {
                    path: path.to_owned(),
                    model: None,
                    parse: Stage::err(format!("read error: {e}")),
                    resolve: Stage::default(),
                };
            }
        };
        let uri = path.to_string_lossy().to_string();
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("buffer.mo");

        // --- Parse stage: raw AST of the user file (def_ids all None). ---
        let (parse, model) = match rumoca_phase_parse::parse_to_ast(&source, file_name) {
            Ok(ast) => {
                let model = ast.classes.keys().next().cloned();
                let value = serde_json::to_value(&ast).unwrap_or_default();
                (Stage::ok(value), model)
            }
            Err(e) => (Stage::err(format!("{e:#}")), None),
        };

        // --- Resolve stage: the user model's class with def_ids populated. ---
        // The session resolves the whole aggregate (user model + libraries); we
        // pull out just the user model's resolved class for display.
        self.session.update_document(&uri, &source);
        let resolve = match &model {
            None => Stage::err("parse produced no model to resolve"),
            Some(simple_name) => {
                let qualified = self.session.qualify_model_name(&uri, simple_name);
                match self.session.resolved() {
                    Ok(rt) => extract_class(&rt.0, &qualified),
                    Err(e) => {
                        // Show the error, and a best-effort tree if one exists.
                        let note = format!("{e:#}");
                        match self.session.resolved_cached() {
                            Some(rt) => match extract_class(&rt.0, &qualified) {
                                Stage { value: Some(v), .. } => Stage {
                                    value: Some(v),
                                    note: Some(note),
                                },
                                _ => Stage::err(note),
                            },
                            None => Stage::err(note),
                        }
                    }
                }
            }
        };

        FromWorker::Compiled { path: path.to_owned(), model, parse, resolve }
    }
}

/// Serialize a single class from a class tree by its qualified name.
fn extract_class(tree: &rumoca_ir_ast::ClassTree, qualified_name: &str) -> Stage {
    match tree.get_class_by_qualified_name(qualified_name) {
        Some(class) => Stage::ok(serde_json::to_value(class).unwrap_or_default()),
        None => Stage::err(format!("`{qualified_name}` not found in resolved tree")),
    }
}
