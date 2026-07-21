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
    /// Arc 7: compile the model, lower it to a `SolveModel`, and run a simulation
    /// to `t_end`, returning the state trajectories to plot. Runs on this worker
    /// thread so the UI never blocks.
    Simulate { path: PathBuf, model: String, t_end: f64 },
}

/// Arc 7: simulation output for plotting — one time axis and, per output variable,
/// its trajectory (`data[var][t]`). Deliberately plain (no Rumoca types) so the UI
/// stays decoupled from the solver crates.
pub struct SimData {
    pub times: Vec<f64>,
    pub names: Vec<String>,
    pub data: Vec<Vec<f64>>,
    /// The first `n_states` names are true states (the rest are algebraics/outputs).
    pub n_states: usize,
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
    /// A successful IR plus an informational (non-error) note — e.g. the
    /// index-reduction stage's "already index-1" / "reduced from singular".
    fn ok_with_note(value: serde_json::Value, note: impl Into<String>) -> Self {
        Stage { value: Some(value), note: Some(note.into()), note_is_error: false }
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
        /// Arc 3: structural analysis of the RAW DAE — matching + BLT + tearing
        /// (errors "singular" on a high-index system).
        structural: Stage,
        /// Arc 4: structural analysis of the DAE AFTER index reduction (the
        /// dummy-derivative funnel) — solvable even when `structural` is singular.
        index_reduction: Stage,
        /// Arc 5: the initial-condition solve plan (`build_ic_plan`) + relaxation hint.
        initialization: Stage,
        /// Arc 6: the DAE's hybrid / event structure (conditions, discrete updates, events).
        events: Stage,
        /// Arc 7 (phase 8): the DAE lowered to a `SolveModel` (the simulator's input).
        solve_lowering: Stage,
        /// Resolved identity of every DefId referenced in the model's IR.
        def_index: BTreeMap<u64, DefInfo>,
    },
    /// A class opened by navigation: its qualified name and (on success) its
    /// resolved IR plus the DefIds it references, so navigation can continue.
    DefTree {
        name: String,
        result: Result<(serde_json::Value, BTreeMap<u64, DefInfo>), String>,
    },
    /// Arc 7: the outcome of a simulation request — trajectories or an error.
    Simulated {
        path: PathBuf,
        result: Result<SimData, String>,
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
            ToWorker::Simulate { path, model, t_end } => {
                let result = self.simulate(&path, &model, t_end);
                FromWorker::Simulated { path, result }
            }
        }
    }

    /// Arc 7: compile the model to its DAE, lower it to a `SolveModel`, and run a
    /// simulation to `t_end` — returning the state trajectories. On this worker
    /// thread; the UI drives it via `ToWorker::Simulate` and never blocks.
    fn simulate(&mut self, path: &Path, model: &str, t_end: f64) -> Result<SimData, String> {
        let source = std::fs::read_to_string(path).map_err(|e| format!("read error: {e}"))?;
        let uri = path.to_string_lossy().to_string();
        self.session.update_document(&uri, &source);
        let qualified = self.session.qualify_model_name(&uri, model);
        let report = self.session.compile_model_strict_reachable_with_recovery(&qualified);
        let cr = match report.requested_result.as_ref() {
            Some(PhaseResult::Success(cr)) => cr,
            Some(PhaseResult::Failed { phase, error, .. }) => {
                return Err(format!("compile failed at {phase}: {error}"));
            }
            _ => return Err("the pipeline produced no simulable result for this model".to_owned()),
        };
        let sm = rumoca_phase_solve::lower_dae_to_solve_model(&cr.dae)
            .map_err(|e| format!("solve lowering failed: {e}"))?;
        let opts = rumoca_sim::SimOptions { t_end, ..Default::default() };
        let res = rumoca_sim::simulate_solve_model(&sm, &opts)
            .map_err(|e| format!("simulation failed: {e}"))?;
        Ok(SimData { times: res.times, names: res.names, data: res.data, n_states: res.n_states })
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
                    index_reduction: Stage::default(),
                    initialization: Stage::default(),
                    events: Stage::default(),
                    solve_lowering: Stage::default(),
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
        let (flatten, structural, index_reduction, initialization, events, solve_lowering) = match &model {
            None => {
                let e = "parse produced no model to compile";
                (Stage::err(e), Stage::err(e), Stage::err(e), Stage::err(e), Stage::err(e), Stage::err(e))
            }
            Some(simple_name) => {
                let qualified = self.session.qualify_model_name(&uri, simple_name);
                let report = self.session.compile_model_strict_reachable_with_recovery(&qualified);
                let result = report.requested_result.as_ref();
                (
                    flatten_stage(result),
                    structural_stage(result),
                    index_reduction_stage(result),
                    initialization_stage(result),
                    events_stage(result),
                    solve_lowering_stage(result),
                )
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
            index_reduction,
            initialization,
            events,
            solve_lowering,
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

/// Arc 4: structural analysis of the DAE **after** index reduction. Runs the
/// dummy-derivative funnel (`index_reduce_for_structural_analysis`) on a copy of
/// the raw DAE, then `build_structural_report` on the result — so a high-index
/// system that `structural_stage` reports singular becomes solvable here. The
/// note says whether reduction was actually needed.
fn index_reduction_stage(result: Option<&PhaseResult>) -> Stage {
    match result {
        Some(PhaseResult::Success(cr)) => {
            let raw_ok = rumoca_phase_structural::build_structural_report(&cr.dae).is_ok();
            let mut reduced = cr.dae.clone();
            index_reduce_for_structural_analysis(&mut reduced);
            match rumoca_phase_structural::build_structural_report(&reduced) {
                Ok(rep) => {
                    let note = if raw_ok {
                        "already index-1 — the reduction funnel is a no-op here (same as the Structural tab)"
                    } else {
                        "index-reduced from a structurally singular (high-index) system — now solvable"
                    };
                    Stage::ok_with_note(structural_to_json(&rep), note)
                }
                Err(e) => Stage::err(format!("still singular after index reduction: {e}")),
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

/// Arc 5: the initial-condition solve plan — how Rumoca computes a consistent
/// initial state at t=0. `build_ic_plan(dae, n_states)` yields the ordered blocks
/// (direct symbolic solves, scalar Newton, torn/coupled loops);
/// `build_ic_relaxation_hint` names the equations dropped / unknowns pinned when
/// the initial algebraic subsystem is structurally singular. The IC types carry
/// `rumoca_core::Expression`, so build JSON (like the structural report).
fn initialization_stage(result: Option<&PhaseResult>) -> Stage {
    match result {
        Some(PhaseResult::Success(cr)) => {
            let n_x = cr.dae.variables.states.len();
            let n_eq = cr.dae.continuous.equations.len();
            // Determinacy of the *user* initialization (idea #6): the explicit
            // initial conditions (initial equations + fixed-start states) vs the
            // states. A surplus means an OVER-determined init (conflicting /
            // redundant conditions — a blow-up `build_ic_plan` alone doesn't catch,
            // since it plans the algebraic subsystem, not the user's init).
            // Under-determination isn't flagged: remaining states initialize from
            // their `start` attributes (default init), so a negative count is normal.
            let n_initial_eq = cr.dae.initialization.equations.len();
            let n_fixed_start_states = cr
                .dae
                .variables
                .states
                .values()
                .filter(|v| v.fixed == Some(true))
                .count();
            let explicit = n_initial_eq + n_fixed_start_states;
            let surplus = explicit as i64 - n_x as i64;
            let determinacy = serde_json::json!({
                "states": n_x,
                "initial_equations": n_initial_eq,
                "fixed_start_states": n_fixed_start_states,
                "explicit_initial_conditions": explicit,
                "surplus_over_states": surplus,
                "verdict": if surplus > 0 {
                    "over-determined"
                } else {
                    "well-posed (remaining states initialize from their start attributes)"
                },
            });
            match rumoca_phase_structural::build_ic_plan(&cr.dae, n_x) {
                Ok(plan) => {
                    let hint = rumoca_phase_structural::build_ic_relaxation_hint(&cr.dae, n_x);
                    let mut json = ic_plan_to_json(&plan, hint.as_ref(), n_x, n_eq);
                    if let Some(obj) = json.as_object_mut() {
                        obj.insert("determinacy".to_owned(), determinacy);
                    }
                    if surplus > 0 {
                        // Over-determined: still show the plan, but flag it red.
                        Stage::recovered(
                            json,
                            format!(
                                "OVER-DETERMINED initialization: {explicit} explicit initial condition(s) \
                                 ({n_initial_eq} initial equation(s) + {n_fixed_start_states} fixed start(s)) \
                                 for {n_x} state(s) — {surplus} too many; conflicting / redundant ICs"
                            ),
                        )
                    } else if plan.is_empty() {
                        Stage::ok_with_note(json, "no algebraic initialization subsystem (equations ≤ states)")
                    } else {
                        Stage::ok(json)
                    }
                }
                Err(e) => Stage::err(format!("IC planning failed: {e}")),
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

fn ic_plan_to_json(
    plan: &[rumoca_phase_structural::IcBlock],
    hint: Option<&rumoca_phase_structural::IcRelaxationHint>,
    n_x: usize,
    n_eq: usize,
) -> serde_json::Value {
    use rumoca_phase_structural::IcBlock;
    let blocks: Vec<serde_json::Value> = plan
        .iter()
        .map(|b| match b {
            IcBlock::ScalarDirect { var_name, solution_expr, .. } => serde_json::json!({
                "kind": "scalar_direct",
                "var": var_name,
                "solution": serde_json::to_value(solution_expr).unwrap_or_default(),
            }),
            IcBlock::ScalarNewton { var_name, eq_idx, .. } => serde_json::json!({
                "kind": "scalar_newton",
                "var": var_name,
                "equation": eq_idx,
            }),
            IcBlock::TornBlock { tear_var_names, causal_sequence, residual_eq_indices, .. } => {
                serde_json::json!({
                    "kind": "torn_block",
                    "tear_vars": tear_var_names,
                    "residual_equations": residual_eq_indices,
                    "causal_steps": causal_sequence.iter().map(|s| serde_json::json!({
                        "var": s.var_name,
                        "equation": s.eq_idx,
                        "newton": s.solution_expr.is_none(),
                    })).collect::<Vec<_>>(),
                })
            }
            IcBlock::CoupledLM { eq_indices, var_names, .. } => serde_json::json!({
                "kind": "coupled_lm",
                "vars": var_names,
                "equations": eq_indices,
            }),
        })
        .collect();
    serde_json::json!({
        "n_states": n_x,
        "n_equations": n_eq,
        "block_count": blocks.len(),
        "blocks": blocks,
        "relaxation_hint": hint.map(|h| serde_json::json!({
            "dropped_equations": h.dropped_eq_global,
            "pinned_unknowns": h.dropped_unknown_names,
        })),
    })
}

/// Arc 6: the DAE's hybrid / event structure — where the equation set changes at
/// discrete events. Read directly from the public `rumoca-ir-dae` partitions:
/// `conditions` (the `f_c` equations + the `relation` expressions that trigger
/// events), `discrete` (the `f_z`/`f_m` update equations lowered from `when`
/// clauses), and `events` (zero-crossing root conditions + scheduled time events).
fn events_stage(result: Option<&PhaseResult>) -> Stage {
    match result {
        Some(PhaseResult::Success(cr)) => {
            let json = events_to_json(&cr.dae);
            let total = json["summary"]
                .as_object()
                .map(|s| s.values().filter_map(serde_json::Value::as_u64).sum::<u64>())
                .unwrap_or(0);
            if total == 0 {
                Stage::ok_with_note(json, "no events — this model is a smooth (continuous) system")
            } else {
                Stage::ok(json)
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

fn events_to_json(dae: &rumoca_ir_dae::Dae) -> serde_json::Value {
    let conditions = &dae.conditions;
    let discrete = &dae.discrete;
    let events = &dae.events;
    serde_json::json!({
        "summary": {
            "condition_equations": conditions.equations.len(),
            "relations": conditions.relations.len(),
            "discrete_real_updates": discrete.real_updates.len(),
            "discrete_valued_updates": discrete.valued_updates.len(),
            "zero_crossing_conditions": events.synthetic_root_conditions.len(),
            "scheduled_time_events": events.scheduled_time_events.len(),
        },
        "conditions": {
            "equations_f_c": serde_json::to_value(&conditions.equations).unwrap_or_default(),
            "relations": serde_json::to_value(&conditions.relations).unwrap_or_default(),
        },
        "discrete_updates": {
            "real_updates_f_z": serde_json::to_value(&discrete.real_updates).unwrap_or_default(),
            "valued_updates_f_m": serde_json::to_value(&discrete.valued_updates).unwrap_or_default(),
        },
        "events": {
            "zero_crossing_conditions": serde_json::to_value(&events.synthetic_root_conditions).unwrap_or_default(),
            "scheduled_time_events": serde_json::to_value(&events.scheduled_time_events).unwrap_or_default(),
        },
    })
}

/// Arc 7 (phase 8): solve lowering — the DAE lowered to a `SolveModel`, the
/// solvable form the simulator runs (residual programs, variable layout, mass
/// matrix, Jacobian sparsity). `SolveModel` derives `Serialize`, so render it in
/// the generic tree. This closes the "solve lowering not instrumented" gap.
fn solve_lowering_stage(result: Option<&PhaseResult>) -> Stage {
    match result {
        Some(PhaseResult::Success(cr)) => {
            match rumoca_phase_solve::lower_dae_to_solve_model(&cr.dae) {
                Ok(sm) => match serde_json::to_value(&sm) {
                    Ok(v) => Stage::ok(v),
                    Err(e) => Stage::err(format!("serialize SolveModel: {e}")),
                },
                Err(e) => Stage::err(format!("solve lowering failed: {e}")),
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
    use std::sync::{Mutex, OnceLock};

    fn msl_roots() -> Vec<PathBuf> {
        let base = concat!(env!("CARGO_MANIFEST_DIR"), "/vendor/msl");
        vec![
            PathBuf::from(format!("{base}/Modelica 4.1.0")),
            PathBuf::from(format!("{base}/ModelicaServices 4.1.0")),
            PathBuf::from(format!("{base}/Complex.mo")),
        ]
    }

    /// One MSL-loaded worker, built once and shared across the worker tests behind
    /// a mutex. Each test needs the full MSL (~430MB resolved); loading it per-test
    /// OOMs / thrashes when cargo runs them in parallel. So tests lock this shared,
    /// already-loaded worker and run serially against it — MSL is parsed once, and
    /// peak memory stays at a single session. The session accumulates each
    /// specimen's document (distinct URIs), which is fine: `compile` qualifies the
    /// requested model by its own URI.
    fn shared_worker() -> &'static Mutex<WorkerState> {
        static WORKER: OnceLock<Mutex<WorkerState>> = OnceLock::new();
        WORKER.get_or_init(|| {
            let mut state = WorkerState::new();
            state.load_libraries(msl_roots()).expect("load MSL once for tests");
            Mutex::new(state)
        })
    }

    /// Compile `specimens/<name>.mo` against the shared MSL worker.
    fn compile_specimen_shared(name: &str) -> FromWorker {
        let path = PathBuf::from(format!("{}/specimens/{name}.mo", env!("CARGO_MANIFEST_DIR")));
        let mut w = shared_worker().lock().unwrap_or_else(|e| e.into_inner());
        w.compile(&path)
    }

    /// End-to-end: after resolving `RotationalInertia` against the MSL, the
    /// component *types* (`type_def_id`) must resolve to their MSL classes.
    #[test]
    fn resolves_def_ids_against_msl() {
        let FromWorker::Compiled { def_index, resolve, .. } = compile_specimen_shared("RotationalInertia") else {
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
        let name = "Modelica.Mechanics.Rotational.Components.Inertia";
        let result = {
            let mut w = shared_worker().lock().unwrap_or_else(|e| e.into_inner());
            let path = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/specimens/RotationalInertia.mo"));
            w.compile(path); // register the specimen document
            let FromWorker::DefTree { result, .. } = w.open_def(name) else {
                panic!("expected DefTree");
            };
            result
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
        let FromWorker::Compiled { model, flatten, .. } = compile_specimen_shared("Drivetrain") else {
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
        let FromWorker::Compiled { structural, .. } = compile_specimen_shared("RotationalInertia") else {
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
        let FromWorker::Compiled { structural, .. } = compile_specimen_shared("ProportionalLoop") else {
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

    /// Compile a `specimens/<name>.mo` against the MSL and return its structural
    /// report JSON — shared by the block-structure guards below.
    fn structural_report_for(name: &str) -> serde_json::Value {
        let FromWorker::Compiled { structural, .. } = compile_specimen_shared(name) else {
            panic!("expected Compiled");
        };
        structural.value.unwrap_or_else(|| panic!("no structural report for {name}: {:?}", structural.note))
    }

    fn block_kinds(v: &serde_json::Value) -> Vec<String> {
        v["blocks"].as_array().into_iter().flatten()
            .filter_map(|b| b["kind"].as_str().map(str::to_owned))
            .collect()
    }

    /// MixedLoop brackets an algebraic loop with scalar solves, so its BLT must
    /// contain BOTH scalar and coupled blocks — the mixed spy-plot case.
    #[test]
    fn mixed_loop_has_scalar_and_coupled_blocks() {
        let v = structural_report_for("MixedLoop");
        assert_eq!(v["coupled_block_count"], serde_json::json!(1));
        let kinds = block_kinds(&v);
        assert!(
            kinds.iter().any(|k| k == "scalar") && kinds.iter().any(|k| k == "coupled"),
            "expected mixed scalar + coupled blocks, got {kinds:?}"
        );
    }

    /// TwoLoops chains two algebraic loops, so structural analysis must report
    /// TWO coupled blocks (two orange boxes).
    #[test]
    fn two_loops_has_two_coupled_blocks() {
        let v = structural_report_for("TwoLoops");
        assert_eq!(v["coupled_block_count"], serde_json::json!(2));
    }

    /// NonlinearLoop is structurally identical to ProportionalLoop (structure is
    /// blind to the nonlinearity) — still one coupled block.
    #[test]
    fn nonlinear_loop_has_a_coupled_block() {
        let v = structural_report_for("NonlinearLoop");
        assert_eq!(v["coupled_block_count"], serde_json::json!(1));
    }

    /// Arc 4: the `dae_prepare` funnel (mirroring rumoca-sim's internal
    /// `prepare_dae_for_structural_analysis` — the shared prep the simulator and
    /// `--inspect structure` both run) reduces Drivetrain's **singular, high-index**
    /// DAE to a non-singular, structurally analyzable one. This confirms Rumoca can
    /// index-reduce (not blocked-on-upstream) and pins the exact public API the
    /// Arc-4 observatory stage will call. NOTE: HRW mirrors Rumoca's funnel *order*;
    /// re-verify it against `rumoca-sim/src/solve_lowering/structural_lowering.rs`
    /// on a pin bump.
    #[test]
    fn drivetrain_index_reduces_from_singular_to_solvable() {
        let report = {
            let mut w = shared_worker().lock().unwrap_or_else(|e| e.into_inner());
            let path = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/specimens/Drivetrain.mo"));
            let source = std::fs::read_to_string(path).unwrap();
            let uri = path.to_string_lossy().to_string();
            w.session.update_document(&uri, &source);
            let qualified = w.session.qualify_model_name(&uri, "Drivetrain");
            w.session.compile_model_strict_reachable_with_recovery(&qualified)
        };
        let cr = match report.requested_result.as_ref() {
            Some(PhaseResult::Success(cr)) => cr,
            _ => panic!("expected a Success result for Drivetrain"),
        };
        // Before: the raw DAE is structurally singular (high index).
        let before = rumoca_phase_structural::build_structural_report(&cr.dae);
        assert!(before.is_err(), "expected Drivetrain to start singular, got {before:?}");

        // Apply the index-reduction funnel, then re-analyze.
        let mut reduced = cr.dae.clone();
        index_reduce_for_structural_analysis(&mut reduced);
        let after = rumoca_phase_structural::build_structural_report(&reduced);
        assert!(
            after.is_ok(),
            "index reduction should make Drivetrain structurally analyzable, got {after:?}"
        );
    }


    /// Arc 5 (blow-up): a capacitor directly across an ideal source can't be
    /// consistently initialized — its state voltage is pinned to the source. Unlike
    /// Drivetrain, index reduction can NOT rescue it: both Structural and Index
    /// reduction stay singular (an observable initialization blow-up).
    #[test]
    fn capacitor_loop_is_singular_and_irreducible() {
        let FromWorker::Compiled { flatten, structural, index_reduction, .. } = compile_specimen_shared("CapacitorLoop") else {
            panic!("expected Compiled");
        };
        assert!(flatten.value.is_some(), "CapacitorLoop should still flatten");
        assert!(structural.value.is_none() && structural.note_is_error, "expected singular Structural");
        assert!(
            index_reduction.value.is_none() && index_reduction.note_is_error,
            "index reduction should NOT rescue a capacitor-across-source loop"
        );
    }

    /// Arc 5: the Initialization stage plans a consistent initial state for the RC
    /// circuit — a non-empty IC plan plus the ground-current relaxation hint.
    #[test]
    fn rc_circuit_has_an_ic_plan() {
        let FromWorker::Compiled { initialization, .. } = compile_specimen_shared("RcCircuit") else {
            panic!("expected Compiled");
        };
        let v = initialization.value.unwrap_or_else(|| panic!("no IC plan: {:?}", initialization.note));
        assert!(v["block_count"].as_u64().unwrap_or(0) >= 1, "expected a non-empty IC plan");
        assert!(v["relaxation_hint"].is_object(), "expected a relaxation hint (ground-current redundancy)");
        // Well-posed init must NOT be mis-flagged as over-determined (idea #6).
        assert_ne!(v["determinacy"]["verdict"], serde_json::json!("over-determined"));
    }

    /// Idea #6: over-specified initialization is flagged. `OverInitRc` pins the
    /// capacitor voltage twice (`C.v = 0` and `der(C.v) = 0`), so the
    /// Initialization stage reports an over-determined init (surplus > 0) with a
    /// red note — the pure init blow-up `build_ic_plan` alone doesn't catch.
    #[test]
    fn over_init_rc_is_flagged_over_determined() {
        let FromWorker::Compiled { initialization, .. } = compile_specimen_shared("OverInitRc") else {
            panic!("expected Compiled");
        };
        let v = initialization.value.expect("IC plan");
        assert_eq!(v["determinacy"]["verdict"], serde_json::json!("over-determined"));
        assert!(v["determinacy"]["surplus_over_states"].as_i64().unwrap_or(0) >= 1);
        assert!(initialization.note_is_error, "over-determined init should be flagged red");
    }



    /// Arc 7 increment 1: HRW can RUN a model, not just inspect it. Lower
    /// `SingleInertia`'s DAE to a `SolveModel` and simulate it, checking the
    /// trajectory is produced AND numerically right: constant torque tau=1 with
    /// J=1 gives der(w)=1, so w(t)=t and w(2) is ~2.
    #[test]
    fn single_inertia_simulates_to_a_correct_trajectory() {
        let report = {
            let mut w = shared_worker().lock().unwrap_or_else(|e| e.into_inner());
            let path = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/specimens/SingleInertia.mo"));
            let src = std::fs::read_to_string(path).unwrap();
            let uri = path.to_string_lossy().to_string();
            w.session.update_document(&uri, &src);
            let q = w.session.qualify_model_name(&uri, "SingleInertia");
            w.session.compile_model_strict_reachable_with_recovery(&q)
        };
        let cr = match report.requested_result.as_ref() {
            Some(PhaseResult::Success(cr)) => cr,
            _ => panic!("expected Success for SingleInertia"),
        };
        let sm = rumoca_phase_solve::lower_dae_to_solve_model(&cr.dae).expect("lower DAE -> SolveModel");
        let opts = rumoca_sim::SimOptions { t_end: 2.0, ..Default::default() };
        let result = rumoca_sim::simulate_solve_model(&sm, &opts).expect("simulate");

        assert!(result.times.last().copied().unwrap_or(0.0) >= 1.99, "should integrate to t_end");
        let w_idx = result.names.iter().position(|n| n == "w").expect("w in outputs");
        assert_eq!(result.data[w_idx].len(), result.times.len(), "trajectory length = time points");
        let w_final = *result.data[w_idx].last().unwrap();
        assert!((w_final - 2.0).abs() < 0.05, "w(2) should be ~2.0 (constant torque), got {w_final}");
    }

    /// Arc 7 #3: the worker's `simulate` path (compile → lower → integrate) runs a
    /// hybrid model — BouncingBall — and returns trajectories. Exercises event
    /// handling in the solver (the ball must stay ~above the floor).
    #[test]
    fn worker_simulate_runs_bouncing_ball() {
        let data = {
            let mut w = shared_worker().lock().unwrap_or_else(|e| e.into_inner());
            let path = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/specimens/BouncingBall.mo"));
            w.simulate(path, "BouncingBall", 3.0)
        }
        .expect("simulate BouncingBall");
        assert!(!data.times.is_empty(), "should produce a trajectory");
        let h_idx = data.names.iter().position(|n| n == "h").expect("h in outputs");
        assert!(
            data.data[h_idx].iter().all(|&h| h > -0.5),
            "the ball should stay ~above the floor (events reflect it)"
        );
    }

    /// Arc 7 (phase 8): the Solve-lowering stage lowers the DAE to a `SolveModel`
    /// (the solvable form the simulator consumes) and renders it.
    #[test]
    fn single_inertia_lowers_to_a_solve_model() {
        let FromWorker::Compiled { solve_lowering, .. } = compile_specimen_shared("SingleInertia") else {
            panic!("expected Compiled");
        };
        let v = solve_lowering.value.expect("SolveModel IR");
        assert!(v.get("problem").is_some(), "SolveModel should carry the solve problem");
        assert!(v.get("variable_meta").is_some(), "SolveModel should carry variable metadata");
    }

    /// Arc 6: BouncingBall is a hybrid model — the Events stage reports its
    /// condition (`h <= 0`) + discrete update (the `reinit`). A smooth model
    /// (SingleInertia) reports none.
    #[test]
    fn bouncing_ball_has_events_smooth_model_has_none() {
        let total_events = |v: &serde_json::Value| -> u64 {
            v["summary"].as_object().into_iter().flatten()
                .filter_map(|(_, x)| x.as_u64()).sum()
        };
        let FromWorker::Compiled { events, .. } = compile_specimen_shared("BouncingBall") else {
            panic!("expected Compiled");
        };
        let v = events.value.expect("events IR");
        assert!(total_events(&v) >= 1, "BouncingBall should have hybrid structure");
        assert!(
            v["discrete_updates"]["real_updates_f_z"].as_array().is_some_and(|a| !a.is_empty()),
            "expected the reinit as a discrete real update"
        );

        let FromWorker::Compiled { events: smooth, .. } = compile_specimen_shared("SingleInertia") else {
            panic!("expected Compiled");
        };
        assert_eq!(total_events(&smooth.value.expect("events IR")), 0, "SingleInertia is smooth");
    }

    /// Arc 4: the parked hand-built PlanarMechanics library (the four-bar-linkage
    /// prerequisite, deferred until Rumoca's Rust-path reduction handles nonlinear
    /// holonomic constraints — see DECISIONS.md) still parses as a source root, so
    /// it doesn't bit-rot while deferred.
    #[test]
    fn planar_mechanics_library_parses() {
        let roots = vec![PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/lib/PlanarMechanics.mo"))];
        let mut state = WorkerState::new();
        let loaded = state.load_libraries(roots).expect("planar mechanics library should parse");
        assert!(loaded >= 1, "expected the planar mechanics library to load");
    }

    /// Arc 4: for the high-index Drivetrain, the raw `structural` stage is singular
    /// (no IR), but the `index_reduction` stage recovers a solvable report — the
    /// before/after the two tabs show side by side.
    #[test]
    fn drivetrain_index_reduction_stage_recovers_singular() {
        let FromWorker::Compiled { structural, index_reduction, .. } = compile_specimen_shared("Drivetrain") else {
            panic!("expected Compiled");
        };
        assert!(structural.value.is_none(), "raw Structural should be singular for Drivetrain");
        let v = index_reduction.value.unwrap_or_else(|| {
            panic!("index reduction should recover Drivetrain: {:?}", index_reduction.note)
        });
        assert!(v["coupled_block_count"].as_u64().is_some(), "reduced report missing block count");
    }
}

/// Apply Rumoca's index-reduction / dummy-derivative funnel to a DAE in place —
/// the public `dae_prepare` building blocks, in the same order as rumoca-sim's
/// internal `prepare_dae_for_structural_analysis`. Turns a structurally singular
/// high-index DAE (e.g. Drivetrain's ideal gears) into a matchable, index-1 one.
///
/// HRW mirrors Rumoca's funnel *order*; re-verify it against
/// `rumoca-sim/src/solve_lowering/structural_lowering.rs` on a Rumoca pin bump
/// (see `docs/updating-rumoca.md`). Steps that can fail return a `StructuralError`;
/// on failure we stop and leave the DAE partially reduced (the caller re-runs the
/// structural report, which will report whatever singularity remains).
fn index_reduce_for_structural_analysis(dae: &mut rumoca_ir_dae::Dae) {
    use rumoca_phase_structural::dae_prepare as dp;
    // Fallible steps stop the funnel on error, leaving the DAE partially reduced
    // (the caller's structural report then names whatever singularity remains);
    // the infallible steps run in their funnel positions.
    if dp::demote_exact_alias_component_states(dae).is_err() { return; }
    if dp::demote_direct_assigned_states(dae).is_err() { return; }
    if dp::reduce_constrained_dummy_derivatives(dae).is_err() { return; }
    if dp::index_reduce_missing_state_derivatives(dae).is_err() { return; }
    dp::demote_states_without_assignable_derivative_rows(dae);
    if dp::eliminate_derivative_aliases(dae).is_err() { return; }
    if dp::demote_states_without_retained_derivative_rows(dae).is_err() { return; }
    dp::expand_compound_derivatives(dae);
    dp::substitute_standalone_state_derivatives_in_non_ode_rows(dae);
    // Then Rumoca's elimination pass, matching its real sim funnel: `eliminate_trivial`
    // *computes* the trivial substitutions (aliases, single-unknown rows) and
    // `apply_..._to_dae` applies them. (It does not resolve nonlinear holonomic
    // constraints — e.g. a Cartesian pendulum's x²+y²=L² stays singular; that class
    // is deferred, see DECISIONS.md — but it makes the reduction faithful for the
    // cases Rumoca does handle, like Drivetrain's linear gear constraints.)
    use rumoca_phase_structural::eliminate;
    if let Ok(elim) = eliminate::eliminate_trivial(dae) {
        let _ = eliminate::apply_elimination_substitutions_to_dae(dae, &elim.substitutions);
    }
}

/// Serialize a single class from a class tree by its qualified name.
fn extract_class(tree: &rumoca_ir_ast::ClassTree, qualified_name: &str) -> Stage {
    match tree.get_class_by_qualified_name(qualified_name) {
        Some(class) => Stage::ok(serde_json::to_value(class).unwrap_or_default()),
        None => Stage::err(format!("`{qualified_name}` not found in resolved tree")),
    }
}
