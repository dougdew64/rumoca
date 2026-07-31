//! **Survey every model in the vendored MSL.**
//!
//! Produces `docs/msl-survey.csv`: one row per model, recording whether Rumoca
//! compiles it, where it stops if not, and — when it succeeds — the IR shape
//! metrics that stratify the fidelity sample.
//!
//! Two deliverables come out of one run (`docs/upstream-strategy.md`):
//!
//! 1. **A capability map of Rumoca over its own standard library** — how much of
//!    MSL compiles and where the rest stops. Useful to a Rumoca maintainer
//!    independently of HRW, and the reason the survey is unfiltered: *"we
//!    surveyed all N models"* is a different claim from *"we surveyed the ones we
//!    expected to work"*.
//! 2. **The stratified sample** for the large-scale fidelity suite, chosen by IR
//!    shape rather than physics domain — a `Fluid` and an `Electrical` model with
//!    the same shape test HRW identically.
//!
//! # This measures Rumoca, not HRW — deliberately
//!
//! It calls `Session` directly rather than going through `WorkerState::compile`.
//! If HRW's stage extraction had a bug, a survey routed through it would report
//! HRW's defect as a Rumoca failure — precisely the misattribution
//! `docs/upstream-strategy.md` warns makes a capability map read as an unfair
//! scorecard. The fidelity harness runs HRW's path separately, over the sample
//! this produces.
//!
//! # Reproducibility
//!
//! Checked in with its output, and deterministic: models are surveyed in sorted
//! order and nothing samples or randomises. A maintainer can regenerate the CSV
//! and diff it. See `docs/upstream-strategy.md` planning rule 2.
//!
//! ```text
//! cargo run -p hrw --release --example survey_msl -- [--limit N] [--out PATH]
//! ```
//!
//! `--limit` surveys the first N models only, for a quick check that the run
//! works before committing an hour to it.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use rumoca_compile::compile::{PhaseResult, SourceRootKind};
use rumoca_compile::source_roots::{parse_source_root_with_cache, source_root_source_set_key};
use rumoca_compile::{Session, SessionConfig};

fn msl_roots() -> Vec<PathBuf> {
    let base = format!("{}/vendor/msl", env!("CARGO_MANIFEST_DIR"));
    vec![
        PathBuf::from(format!("{base}/Modelica 4.1.0")),
        PathBuf::from(format!("{base}/ModelicaServices 4.1.0")),
        PathBuf::from(format!("{base}/Complex.mo")),
    ]
}

/// Load the MSL into a fresh session.
///
/// Mirrors `WorkerState::load_libraries`, which is `pub(crate)` and takes a
/// worker this example has no use for.
fn load_msl() -> Session {
    let mut session = Session::new(SessionConfig::default());
    for root in msl_roots() {
        let parsed = parse_source_root_with_cache(&root)
            .unwrap_or_else(|e| panic!("parse {}: {e:#}", root.display()));
        let key = source_root_source_set_key(&root.to_string_lossy());
        session.replace_parsed_source_set(
            &key,
            SourceRootKind::DurableExternal,
            parsed.documents,
            None,
        );
    }
    session
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let limit = arg_value(&args, "--limit").and_then(|v| v.parse::<usize>().ok());
    let out = arg_value(&args, "--out").unwrap_or_else(|| {
        format!("{}/docs/msl-survey.csv", env!("CARGO_MANIFEST_DIR"))
    });

    eprintln!("loading MSL…");
    let t0 = Instant::now();
    let mut session = load_msl();

    let counts = session.class_type_counts().unwrap_or_default();
    let mut names: Vec<String> = session
        .model_names()
        .expect("MSL should resolve")
        .to_vec();
    names.sort();
    names.dedup();
    let available = names.len();
    eprintln!(
        "loaded in {:.1}s — {} models among {} classes by type: {}",
        t0.elapsed().as_secs_f64(),
        names.len(),
        counts.values().sum::<usize>(),
        summarize(&counts),
    );

    if let Some(n) = limit {
        // Spread the sample across the alphabet rather than taking a prefix: the
        // first N sorted names are all `Modelica.Blocks.*`, which would time one
        // package and call it MSL.
        let step = names.len().div_ceil(n.max(1));
        names = names.into_iter().step_by(step.max(1)).collect();
        eprintln!("--limit {n}: surveying {} models, every {step}th", names.len());
    }

    let t_compile = Instant::now();
    let mut outcomes: HashMap<String, usize> = HashMap::new();
    let mut rows = vec![Row::HEADER.to_owned()];

    for (i, name) in names.iter().enumerate() {
        if i % 100 == 0 && i > 0 {
            eprintln!("  {i}/{} ({:.0}s elapsed)", names.len(), t_compile.elapsed().as_secs_f64());
        }
        let row = survey_one(&mut session, name);
        *outcomes.entry(row.outcome.clone()).or_default() += 1;
        rows.push(row.to_csv());
    }

    let total = t_compile.elapsed().as_secs_f64();
    if let Err(e) = std::fs::write(&out, rows.join("\n") + "\n") {
        eprintln!("could not write {out}: {e}");
    }

    // Provenance, as a sidecar rather than CSV comment lines, which strict
    // readers and spreadsheets both mishandle.
    //
    // **A survey that cannot say what it describes is not reproducible**, and
    // reproducibility is what makes it publishable (`docs/upstream-strategy.md`
    // planning rule 2). It is also what HRW's planned Test mode needs to caption
    // a loaded report: which Rumoca, which MSL, how much of it, and when.
    let mut tally: Vec<(&String, &usize)> = outcomes.iter().collect();
    tally.sort_by_key(|(k, _)| k.as_str());
    let meta = format!(
        "{{\n  \"rumoca_version\": \"{}\",\n  \"hrw_version\": \"{}\",\n  \
         \"msl_roots\": [{}],\n  \"models_surveyed\": {},\n  \"models_available\": {},\n  \
         \"partial_survey\": {},\n  \"seconds\": {:.1},\n  \"generated_unix\": {},\n  \
         \"outcomes\": {{{}}}\n}}\n",
        env!("HRW_RUMOCA_VERSION"),
        env!("CARGO_PKG_VERSION"),
        msl_roots()
            .iter()
            .map(|r| format!("\"{}\"", r.file_name().unwrap_or_default().to_string_lossy()))
            .collect::<Vec<_>>()
            .join(", "),
        names.len(),
        available,
        limit.is_some(),
        total,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs()),
        tally
            .iter()
            .map(|(k, n)| format!("\"{k}\": {n}"))
            .collect::<Vec<_>>()
            .join(", "),
    );
    let meta_path = out.replace(".csv", ".meta.json");
    if let Err(e) = std::fs::write(&meta_path, meta) {
        eprintln!("could not write {meta_path}: {e}");
    } else {
        eprintln!("wrote {meta_path}");
    }

    eprintln!(
        "\n{} models in {total:.1}s = {:.2}s/model",
        names.len(),
        total / names.len() as f64,
    );
    eprintln!("outcomes: {}", summarize(&outcomes));
    eprintln!("wrote {out}");
}

/// One model's row. **Every field is either measured or empty** — never a
/// default standing in for a measurement, because a `0` that means "not
/// applicable" would be indistinguishable from a `0` that means "measured zero"
/// once this is a published table.
#[derive(Default)]
struct Row {
    name: String,
    /// `Examples` / `Interfaces` / `BaseClasses` / … from the qualified name.
    ///
    /// A rough proxy for intent, and it is **load-bearing for fairness**: an
    /// `Interfaces` class is usually partial and not meant to compile on its own,
    /// so counting its failure against Rumoca would be the misattribution
    /// `docs/upstream-strategy.md` warns turns a capability map into a scorecard.
    /// Kept as raw data rather than a verdict — the analysis decides, and shows
    /// its working.
    kind: String,
    outcome: String,
    /// First line only: enough to cluster failures, short enough for a table.
    message: String,
    secs: f64,
    // --- shape, when the compile succeeded ---
    n_equations: Option<usize>,
    n_states: Option<usize>,
    n_algebraic: Option<usize>,
    n_discrete: Option<usize>,
    n_parameters: Option<usize>,
    /// Structural analysis of the raw DAE: `ok`, `singular`, or an error kind.
    structural: String,
    n_blocks: Option<usize>,
    n_coupled: Option<usize>,
    /// Largest coupled block — one enormous algebraic loop and many small ones
    /// are different rendering problems, and F4's partition sees them alike.
    largest_coupled: Option<usize>,
    /// Any variable name carrying a subscript. Array IR is a shape our authored
    /// specimens barely contain.
    has_arrays: bool,
    /// Deepest component path, by dots in a flat name. Deep hierarchies stress
    /// the tree, and dotted names are what broke F7.
    max_depth: usize,
    n_functions: Option<usize>,
}

impl Row {
    const HEADER: &'static str = "name,kind,outcome,message,secs,n_equations,n_states,\
        n_algebraic,n_discrete,n_parameters,structural,n_blocks,n_coupled,largest_coupled,\
        has_arrays,max_depth,n_functions";

    fn to_csv(&self) -> String {
        let n = |v: Option<usize>| v.map_or(String::new(), |x| x.to_string());
        format!(
            "{},{},{},{},{:.3},{},{},{},{},{},{},{},{},{},{},{},{}",
            csv_field(&self.name), csv_field(&self.kind), csv_field(&self.outcome),
            csv_field(&self.message), self.secs,
            n(self.n_equations), n(self.n_states), n(self.n_algebraic), n(self.n_discrete),
            n(self.n_parameters), csv_field(&self.structural),
            n(self.n_blocks), n(self.n_coupled), n(self.largest_coupled),
            self.has_arrays, self.max_depth, n(self.n_functions),
        )
    }
}

/// The MSL sub-package a name sits in, as a fairness signal — see [`Row::kind`].
fn classify(name: &str) -> String {
    for marker in ["Examples", "Interfaces", "BaseClasses", "Internal", "Types", "Icons", "Tests"] {
        if name.split('.').any(|seg| seg == marker) {
            return marker.to_owned();
        }
    }
    "Component".to_owned()
}

fn survey_one(session: &mut Session, name: &str) -> Row {
    let mut row = Row { name: name.to_owned(), kind: classify(name), ..Default::default() };
    let t = Instant::now();
    let report = session.compile_model_strict_reachable_uncached_with_recovery(name);
    row.secs = t.elapsed().as_secs_f64();

    let cr = match report.requested_result {
        Some(PhaseResult::Success(cr)) => {
            row.outcome = "success".to_owned();
            cr
        }
        Some(PhaseResult::Failed { phase, error, .. }) => {
            row.outcome = format!("failed:{phase}");
            row.message = first_line(&error);
            return row;
        }
        Some(PhaseResult::NeedsInner { .. }) => {
            row.outcome = "needs_inner".to_owned();
            return row;
        }
        None => {
            row.outcome = "no_result".to_owned();
            row.message = first_line(&report.failure_summary(0));
            return row;
        }
    };

    let v = &cr.dae.variables;
    row.n_equations = Some(cr.dae.continuous.equations.len());
    row.n_states = Some(v.states.len());
    row.n_algebraic = Some(v.algebraics.len());
    row.n_discrete = Some(v.discrete_reals.len() + v.discrete_valued.len());
    row.n_parameters = Some(v.parameters.len());
    row.n_functions = Some(cr.dae.symbols.functions.len());

    let all_names = || {
        v.states.keys().chain(v.algebraics.keys()).chain(v.parameters.keys())
            .chain(v.discrete_reals.keys()).chain(v.discrete_valued.keys())
            .map(ToString::to_string)
    };
    row.has_arrays = all_names().any(|n| n.contains('['));
    row.max_depth = all_names().map(|n| n.matches('.').count()).max().unwrap_or(0);

    match rumoca_phase_structural::build_structural_report(&cr.dae) {
        Ok(rep) => {
            row.structural = "ok".to_owned();
            row.n_blocks = Some(rep.blocks.len());
            let coupled: Vec<usize> = rep
                .blocks
                .iter()
                .filter_map(|b| match b {
                    rumoca_phase_structural::BlockReport::Coupled { unknowns, .. } => {
                        Some(unknowns.len())
                    }
                    rumoca_phase_structural::BlockReport::Scalar { .. } => None,
                })
                .collect();
            row.n_coupled = Some(coupled.len());
            row.largest_coupled = Some(coupled.into_iter().max().unwrap_or(0));
        }
        Err(e) => {
            // Singular here is NOT a failure — a high-index model is singular by
            // construction and index reduction fixes it. Recorded as a shape,
            // because it is one: it decides which DAE the Index Reduction tab
            // animates, the distinction that broke F1's first draft.
            row.structural = match e {
                rumoca_phase_structural::StructuralError::Singular { .. } => "singular".to_owned(),
                other => format!("error:{}", first_line(&other.to_string())),
            };
        }
    }
    row
}

fn first_line(s: &str) -> String {
    s.lines().next().unwrap_or("").chars().take(200).collect()
}

/// RFC-4180 quoting, since messages carry commas and quotes freely.
fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_owned()
    }
}

fn arg_value(args: &[String], flag: &str) -> Option<String> {
    let i = args.iter().position(|a| a == flag)?;
    args.get(i + 1).cloned()
}

fn summarize(counts: &HashMap<String, usize>) -> String {
    let mut v: Vec<(&String, &usize)> = counts.iter().collect();
    v.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
    v.iter().take(8).map(|(k, n)| format!("{k}={n}")).collect::<Vec<_>>().join(" ")
}
