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

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use eframe::egui;
use rumoca_compile::compile::{FailedPhase, PhaseResult, SourceRootKind};
use rumoca_compile::source_roots::{parse_source_root_with_cache, source_root_source_set_key};
use rumoca_compile::{Session, SessionConfig};

/// A request from the UI thread to the worker.
pub enum ToWorker {
    /// Replace the library source roots (reloads them into a fresh session).
    SetLibraries(Vec<PathBuf>),
    /// Run parse → resolve on a specimen file.
    Compile(PathBuf),
    /// Extract an arbitrary class from the resolved tree by qualified name, so
    /// the UI can navigate into a definition a `def_id`/`type_def_id` points at.
    OpenDef(String),
}

/// One pipeline stage's outcome for the selected model: the serialized IR node
/// (if the stage produced one) plus an optional note (error or status).
#[derive(Clone, Default)]
pub struct Stage {
    pub value: Option<serde_json::Value>,
    pub note: Option<String>,
    /// True when `note` is an error (rendered red); false = an informational
    /// status like "succeeded" or "not reached" (rendered neutral).
    pub note_is_error: bool,
}

impl Stage {
    fn ok(value: serde_json::Value) -> Self {
        Stage { value: Some(value), note: None, note_is_error: false }
    }
    fn err(note: impl Into<String>) -> Self {
        Stage { value: None, note: Some(note.into()), note_is_error: true }
    }
    /// A non-error status note for a stage with no IR of its own to show.
    fn info(note: impl Into<String>) -> Self {
        Stage { value: None, note: Some(note.into()), note_is_error: false }
    }
    /// A best-effort IR plus an error note (e.g. resolve recovered a partial tree).
    fn recovered(value: serde_json::Value, note: impl Into<String>) -> Self {
        Stage { value: Some(value), note: Some(note.into()), note_is_error: true }
    }
}

/// Resolved identity of a `DefId` referenced in a stage's IR — what an opaque
/// integer like `type_def_id: 27579` actually points at. A deterministic lookup
/// against the resolved tree (which the worker owns), *not* reasoning: the UI
/// shows it inline and the bridge hands it to Claude so answers follow real
/// pointers instead of narrating faith in a number.
#[derive(Clone)]
pub struct DefInfo {
    /// Qualified name, e.g. "Modelica.Mechanics.Rotational.Components.Inertia".
    pub name: String,
    /// "class" (a class definition) or "definition" (component/other non-class).
    pub kind: &'static str,
    /// Class keyword ("model", "block", …) when this DefId names a class.
    pub class_type: Option<String>,
    /// Source location of the class definition (when a class).
    pub file_name: Option<String>,
    pub line: Option<u32>,
}

impl DefInfo {
    /// Compact inline label for the tree, e.g. "model Modelica.…Inertia".
    pub fn label(&self) -> String {
        match &self.class_type {
            Some(ct) => format!("{ct} {}", self.name),
            None => self.name.clone(),
        }
    }

    /// JSON form for the bridge focus file (DefInfo doesn't derive Serialize to
    /// avoid taking a direct `serde` dependency).
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "name": self.name,
            "kind": self.kind,
            "class_type": self.class_type,
            "file_name": self.file_name,
            "line": self.line,
        })
    }
}

/// A result from the worker back to the UI thread.
pub enum FromWorker {
    /// Outcome of loading libraries: total documents loaded, or an error.
    Libraries(Result<usize, String>),
    /// Outcome of compiling a specimen through the pipeline stages.
    Compiled {
        path: PathBuf,
        /// Simple name of the model whose IR the stages show.
        model: Option<String>,
        parse: Stage,
        resolve: Stage,
        instantiate: Stage,
        typecheck: Stage,
        flatten: Stage,
        /// Arc 3: structural analysis of the DAE — matching + BLT blocks + tearing.
        structural: Stage,
        /// Resolved identity of every DefId referenced in the model's IR.
        def_index: BTreeMap<u64, DefInfo>,
    },
    /// A class opened by navigation: its qualified name and (on success) its
    /// resolved IR plus the DefIds it references, so navigation can continue.
    DefTree {
        name: String,
        result: Result<(serde_json::Value, BTreeMap<u64, DefInfo>), String>,
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
            ToWorker::OpenDef(name) => self.open_def(&name),
        }
    }

    /// Extract a class from the resolved tree by qualified name for navigation.
    fn open_def(&mut self, name: &str) -> FromWorker {
        let rt = match self.session.resolved() {
            Ok(rt) => rt,
            Err(e) => return FromWorker::DefTree { name: name.to_owned(), result: Err(format!("{e:#}")) },
        };
        let result = match rt.0.get_class_by_qualified_name(name) {
            Some(class) => {
                let value = serde_json::to_value(class).unwrap_or_default();
                let def_index = build_def_index(&rt.0, &value);
                Ok((value, def_index))
            }
            None => Err(format!("`{name}` not found in resolved tree")),
        };
        FromWorker::DefTree { name: name.to_owned(), result }
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
                    instantiate: Stage::default(),
                    typecheck: Stage::default(),
                    flatten: Stage::default(),
                    structural: Stage::default(),
                    def_index: BTreeMap::new(),
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
        // Resolutions for the DefIds referenced in the resolved model, built
        // wherever we successfully extract a resolved class below.
        let mut def_index = BTreeMap::new();
        // Increment 2: instantiate + instanced-typecheck stages, computed from
        // the resolved tree while we still hold it (default = empty if resolve fails).
        let mut instantiate = Stage::default();
        let mut typecheck = Stage::default();
        let resolve = match &model {
            None => Stage::err("parse produced no model to resolve"),
            Some(simple_name) => {
                let qualified = self.session.qualify_model_name(&uri, simple_name);
                match self.session.resolved() {
                    Ok(rt) => {
                        let stage = extract_class(&rt.0, &qualified);
                        if let Some(v) = &stage.value {
                            def_index = build_def_index(&rt.0, v);
                        }
                        let (i, t) = instantiate_and_typecheck(&rt.0, &qualified);
                        instantiate = i;
                        typecheck = t;
                        stage
                    }
                    Err(e) => {
                        // Show the error, and a best-effort tree if one exists.
                        let note = format!("{e:#}");
                        match self.session.resolved_cached() {
                            Some(rt) => match extract_class(&rt.0, &qualified) {
                                Stage { value: Some(v), .. } => {
                                    def_index = build_def_index(&rt.0, &v);
                                    Stage::recovered(v, note)
                                }
                                _ => Stage::err(note),
                            },
                            None => Stage::err(note),
                        }
                    }
                }
            }
        };

        // --- Flatten stage: from the reachable-closure pipeline (increment 1).
        // Instantiate/Typecheck were computed above from the resolved tree. ---
        let (flatten, structural) = match &model {
            None => {
                let e = "parse produced no model to compile";
                (Stage::err(e), Stage::err(e))
            }
            Some(simple_name) => {
                let qualified = self.session.qualify_model_name(&uri, simple_name);
                let report = self.session.compile_model_strict_reachable_with_recovery(&qualified);
                let result = report.requested_result.as_ref();
                (flatten_stage(result), structural_stage(result))
            }
        };

        FromWorker::Compiled {
            path: path.to_owned(),
            model,
            parse,
            resolve,
            instantiate,
            typecheck,
            flatten,
            structural,
            def_index,
        }
    }
}

/// Compile a specimen through every pipeline stage with the given library roots,
/// headlessly — the exact path the worker thread runs, minus the thread/channel.
/// Used by `examples/gen_trace` (trace-log generation) and tests, so their output
/// is byte-identical to what the running app produces. Returns the `Compiled`
/// result, or an error if the libraries fail to load.
pub fn compile_specimen(specimen: &Path, libraries: Vec<PathBuf>) -> Result<FromWorker, String> {
    let mut state = WorkerState::new();
    state.load_libraries(libraries)?;
    Ok(state.compile(specimen))
}

/// Structural analysis of the model's DAE (Arc 3): maximum matching + BLT blocks
/// + tearing, from `build_structural_report`. Only available on a full Success
/// (the DAE must exist). The report types aren't `Serialize`, so build JSON.
fn structural_stage(result: Option<&PhaseResult>) -> Stage {
    match result {
        Some(PhaseResult::Success(cr)) => {
            match rumoca_phase_structural::build_structural_report(&cr.dae) {
                Ok(rep) => Stage::ok(structural_to_json(&rep)),
                Err(e) => Stage::err(format!("structural analysis failed: {e}")),
            }
        }
        Some(PhaseResult::Failed { phase, .. }) => {
            Stage::info(format!("not reached ({phase} failed earlier)"))
        }
        Some(PhaseResult::NeedsInner { .. }) => {
            Stage::info("not reached (model needs inner declarations)")
        }
        None => Stage::err("the reachable-closure pipeline produced no result for this model"),
    }
}

fn structural_to_json(rep: &rumoca_phase_structural::StructuralReport) -> serde_json::Value {
    serde_json::json!({
        "n_equations": rep.n_equations,
        "n_unknowns": rep.n_unknowns,
        "coupled_block_count": rep.coupled_block_count(),
        "matching": rep
            .matching
            .iter()
            .map(|(e, u)| serde_json::json!({ "equation": e, "unknown": u }))
            .collect::<Vec<_>>(),
        "blocks": rep.blocks.iter().map(block_to_json).collect::<Vec<_>>(),
    })
}

fn block_to_json(b: &rumoca_phase_structural::BlockReport) -> serde_json::Value {
    use rumoca_phase_structural::BlockReport;
    match b {
        BlockReport::Scalar { equation, unknown } => serde_json::json!({
            "kind": "scalar",
            "size": 1,
            "equation": equation,
            "unknown": unknown,
        }),
        BlockReport::Coupled { equations, unknowns, tearing } => serde_json::json!({
            "kind": "coupled",
            "size": unknowns.len(),
            "equations": equations,
            "unknowns": unknowns,
            "tearing": tearing.as_ref().map(tearing_to_json),
        }),
    }
}

fn tearing_to_json(t: &rumoca_phase_structural::TearingReport) -> serde_json::Value {
    serde_json::json!({
        "tear_vars": t.tear_vars,
        "residual_equations": t.residual_equations,
        "causal_sequence": t
            .causal_sequence
            .iter()
            .map(|(e, v)| serde_json::json!({ "equation": e, "variable": v }))
            .collect::<Vec<_>>(),
    })
}

/// Instantiate the model directly from the resolved tree and serialize the
/// resulting `InstanceOverlay` for the Instantiate tab; then run the instanced
/// typecheck, which enriches the *same* overlay in place (evaluated dimensions,
/// resolved component types), and serialize it again for the Typecheck tab.
/// The cross-stage diff between the two shows exactly what typecheck contributed.
fn instantiate_and_typecheck(tree: &rumoca_ir_ast::ClassTree, model_name: &str) -> (Stage, Stage) {
    match rumoca_phase_instantiate::instantiate_model(tree, model_name) {
        Ok(mut overlay) => {
            let instantiate = Stage::ok(serde_json::to_value(&overlay).unwrap_or_default());
            let typecheck = match rumoca_phase_typecheck::typecheck_instanced(tree, &mut overlay, model_name) {
                Ok(()) => Stage::ok(serde_json::to_value(&overlay).unwrap_or_default()),
                // Best-effort: still show the (partially) enriched overlay + the note.
                Err(_diags) => Stage::recovered(
                    serde_json::to_value(&overlay).unwrap_or_default(),
                    "instanced typecheck reported diagnostics",
                ),
            };
            (instantiate, typecheck)
        }
        Err(e) => (
            Stage::err(format!("instantiate failed: {e}")),
            Stage::info("not reached (instantiate failed)"),
        ),
    }
}

/// Extract just the Flatten stage from the reachable-closure pipeline's
/// `PhaseResult` (the flat IR on success, or per-phase status/error).
fn flatten_stage(result: Option<&PhaseResult>) -> Stage {
    match result {
        Some(PhaseResult::Success(cr)) => match serde_json::to_value(&cr.flat) {
            Ok(v) => Stage::ok(v),
            Err(e) => Stage::err(format!("serialize flat model: {e}")),
        },
        Some(PhaseResult::Failed { phase, error, diagnostics, .. }) => {
            let msg = if diagnostics.is_empty() {
                error.clone()
            } else {
                format!("{error}  ({} diagnostic(s))", diagnostics.len())
            };
            match phase {
                FailedPhase::Flatten => Stage::err(msg),
                FailedPhase::ToDae => {
                    Stage::info("flatten succeeded; DAE construction failed (later arc)")
                }
                other => Stage::info(format!("not reached ({other} failed earlier)")),
            }
        }
        Some(PhaseResult::NeedsInner { missing_inners, .. }) => {
            Stage::info(format!("needs inner declaration(s) for: {}", missing_inners.join(", ")))
        }
        None => Stage::err("the reachable-closure pipeline produced no result for this model"),
    }
}

/// Field names in the IR whose values are `DefId`s (resolved definition ids).
const DEF_ID_KEYS: [&str; 3] = ["def_id", "type_def_id", "base_def_id"];

/// True when `key` names a `DefId`-valued field.
pub fn is_def_id_key(key: &str) -> bool {
    DEF_ID_KEYS.contains(&key)
}

/// Collect every DefId appearing under a DefId-named key anywhere in the IR.
fn collect_def_ids(v: &serde_json::Value, out: &mut BTreeSet<u64>) {
    match v {
        serde_json::Value::Object(map) => {
            for (k, val) in map {
                if is_def_id_key(k) {
                    if let Some(n) = val.as_u64() {
                        out.insert(n);
                    }
                }
                collect_def_ids(val, out);
            }
        }
        serde_json::Value::Array(arr) => arr.iter().for_each(|val| collect_def_ids(val, out)),
        _ => {}
    }
}

/// Resolve every DefId referenced in `value` against the resolved tree, into a
/// `DefId → DefInfo` map. Iterates `def_map` (whose `DefId` key exposes a public
/// `.0` field) so we never name the `DefId` type — keeping `rumoca-core` out of
/// our direct dependencies.
fn build_def_index(
    tree: &rumoca_ir_ast::ClassTree,
    value: &serde_json::Value,
) -> BTreeMap<u64, DefInfo> {
    let name_by_id: std::collections::HashMap<u32, &str> =
        tree.def_map.iter().map(|(k, v)| (k.0, v.as_str())).collect();

    let mut ids = BTreeSet::new();
    collect_def_ids(value, &mut ids);

    let mut index = BTreeMap::new();
    for id in ids {
        let Some(name) = name_by_id.get(&(id as u32)) else { continue };
        let name = (*name).to_owned();
        // A class DefId resolves to a ClassDef (with a location); anything else
        // in def_map (e.g. a component) resolves to a name only.
        let info = match tree.get_class_by_qualified_name(&name) {
            Some(class) => DefInfo {
                name,
                kind: "class",
                class_type: Some(class.class_type.as_str().to_owned()),
                file_name: Some(class.location.file_name.clone()),
                line: Some(class.location.start_line),
            },
            None => DefInfo { name, kind: "definition", class_type: None, file_name: None, line: None },
        };
        index.insert(id, info);
    }
    index
}

#[cfg(test)]
mod tests {
    use super::*;

    /// End-to-end: after resolving `RotationalInertia` against the MSL, the
    /// component *types* (`type_def_id`) must resolve to their MSL classes.
    #[test]
    fn resolves_def_ids_against_msl() {
        let base = concat!(env!("CARGO_MANIFEST_DIR"), "/vendor/msl");
        let roots = vec![
            PathBuf::from(format!("{base}/Modelica 4.1.0")),
            PathBuf::from(format!("{base}/ModelicaServices 4.1.0")),
            PathBuf::from(format!("{base}/Complex.mo")),
        ];
        let mut state = WorkerState::new();
        state.load_libraries(roots).expect("load MSL");

        let path = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/specimens/RotationalInertia.mo"));
        let FromWorker::Compiled { def_index, resolve, .. } = state.compile(path) else {
            panic!("expected Compiled");
        };
        assert!(resolve.value.is_some(), "resolve failed: {:?}", resolve.note);
        assert!(!def_index.is_empty(), "no DefIds resolved");

        let names: Vec<&str> = def_index.values().map(|d| d.name.as_str()).collect();
        // The three declared component types resolved to their MSL classes.
        for expected in [
            "Mechanics.Rotational.Components.Inertia",
            "Mechanics.Rotational.Sources.Torque",
            "Blocks.Sources.Constant",
        ] {
            assert!(
                def_index.values().any(|d| d.kind == "class" && d.name.ends_with(expected)),
                "{expected} not resolved as a class; got {names:?}"
            );
        }
    }

    /// Navigation: after compiling the specimen, opening a class the model
    /// points at (the MSL `Inertia`) returns its IR and its own DefId index.
    #[test]
    fn open_def_extracts_a_navigated_class() {
        let base = concat!(env!("CARGO_MANIFEST_DIR"), "/vendor/msl");
        let roots = vec![
            PathBuf::from(format!("{base}/Modelica 4.1.0")),
            PathBuf::from(format!("{base}/ModelicaServices 4.1.0")),
            PathBuf::from(format!("{base}/Complex.mo")),
        ];
        let mut state = WorkerState::new();
        state.load_libraries(roots).expect("load MSL");
        let path = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/specimens/RotationalInertia.mo"));
        state.compile(path); // register the specimen document

        let name = "Modelica.Mechanics.Rotational.Components.Inertia";
        let FromWorker::DefTree { result, .. } = state.open_def(name) else {
            panic!("expected DefTree");
        };
        let (value, def_index) = result.expect("Inertia class extracted");
        // It's a class body with a name matching Inertia.
        assert_eq!(value["name"]["text"], serde_json::json!("Inertia"));
        // Its own references resolved, so navigation can continue from here.
        assert!(!def_index.is_empty(), "navigated class has no resolved DefIds");
    }

    /// The Arc-2 drivetrain specimen compiles through the whole pipeline (it
    /// crosses electrical → rotational → translational, so this exercises
    /// connector expansion / flow-sum generation across domains).
    #[test]
    fn drivetrain_compiles_through_flatten() {
        let base = concat!(env!("CARGO_MANIFEST_DIR"), "/vendor/msl");
        let roots = vec![
            PathBuf::from(format!("{base}/Modelica 4.1.0")),
            PathBuf::from(format!("{base}/ModelicaServices 4.1.0")),
            PathBuf::from(format!("{base}/Complex.mo")),
        ];
        let mut state = WorkerState::new();
        state.load_libraries(roots).expect("load MSL");
        let path = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/specimens/Drivetrain.mo"));
        let FromWorker::Compiled { model, flatten, .. } = state.compile(path) else {
            panic!("expected Compiled");
        };
        assert_eq!(model.as_deref(), Some("Drivetrain"));
        assert!(
            flatten.value.is_some(),
            "Drivetrain did not flatten: {:?}",
            flatten.note
        );
    }

    /// The structural stage builds a matching + BLT report for an index-1 model.
    #[test]
    fn structural_report_for_rotational_inertia() {
        let base = concat!(env!("CARGO_MANIFEST_DIR"), "/vendor/msl");
        let roots = vec![
            PathBuf::from(format!("{base}/Modelica 4.1.0")),
            PathBuf::from(format!("{base}/ModelicaServices 4.1.0")),
            PathBuf::from(format!("{base}/Complex.mo")),
        ];
        let mut state = WorkerState::new();
        state.load_libraries(roots).expect("load MSL");
        let path = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/specimens/RotationalInertia.mo"));
        let FromWorker::Compiled { structural, .. } = state.compile(path) else {
            panic!("expected Compiled");
        };
        let v = structural.value.expect("structural report");
        assert!(v["matching"].as_array().is_some_and(|a| !a.is_empty()), "no matching");
        assert!(v["blocks"].as_array().is_some_and(|a| !a.is_empty()), "no BLT blocks");
        // A plain index-1 ODE sorts into scalar blocks only — no algebraic loop.
        assert_eq!(v["coupled_block_count"], serde_json::json!(0), "unexpected coupled block");
    }

    /// The Arc-3 proportional-loop specimen closes an algebraic feedback loop, so
    /// structural analysis MUST report a coupled block (a simultaneous algebraic
    /// SCC) — the case the BLT spy-plot draws as a box. This is the specimen's
    /// whole reason for existing, so guard it.
    #[test]
    fn proportional_loop_has_a_coupled_block() {
        let base = concat!(env!("CARGO_MANIFEST_DIR"), "/vendor/msl");
        let roots = vec![
            PathBuf::from(format!("{base}/Modelica 4.1.0")),
            PathBuf::from(format!("{base}/ModelicaServices 4.1.0")),
            PathBuf::from(format!("{base}/Complex.mo")),
        ];
        let mut state = WorkerState::new();
        state.load_libraries(roots).expect("load MSL");
        let path = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/specimens/ProportionalLoop.mo"));
        let FromWorker::Compiled { structural, .. } = state.compile(path) else {
            panic!("expected Compiled");
        };
        let v = structural.value.unwrap_or_else(|| panic!("no structural report: {:?}", structural.note));
        let count = v["coupled_block_count"].as_u64().unwrap_or(0);
        assert!(count >= 1, "expected a coupled algebraic block, got {count}; blocks = {}", v["blocks"]);
        // The coupled block should carry a tearing report (iteration variable(s)).
        let coupled = v["blocks"].as_array().into_iter().flatten()
            .find(|b| b["kind"] == serde_json::json!("coupled"))
            .expect("a coupled block");
        assert!(coupled["size"].as_u64().unwrap_or(0) >= 2, "coupled block must be size >= 2");
    }
}

/// Serialize a single class from a class tree by its qualified name.
fn extract_class(tree: &rumoca_ir_ast::ClassTree, qualified_name: &str) -> Stage {
    match tree.get_class_by_qualified_name(qualified_name) {
        Some(class) => Stage::ok(serde_json::to_value(class).unwrap_or_default()),
        None => Stage::err(format!("`{qualified_name}` not found in resolved tree")),
    }
}
