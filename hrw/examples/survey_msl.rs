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

use hrw::survey::{SurveyRow, Summary, classify, package_of};
use hrw::worker::index_reduce_in_place;

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
    let mut rows: Vec<SurveyRow> = Vec::new();

    for (i, name) in names.iter().enumerate() {
        if i % 100 == 0 && i > 0 {
            eprintln!("  {i}/{} ({:.0}s elapsed)", names.len(), t_compile.elapsed().as_secs_f64());
        }
        rows.push(survey_one(&mut session, name));
    }

    let total = t_compile.elapsed().as_secs_f64();
    let mut csv = vec![SurveyRow::HEADER.to_owned()];
    csv.extend(rows.iter().map(SurveyRow::to_csv));
    if let Err(e) = std::fs::write(&out, csv.join("\n") + "\n") {
        eprintln!("could not write {out}: {e}");
    }

    // Computed through the same `Summary` the Test-mode panel will use, so the
    // console tally and the rendered one cannot disagree.
    let summary = Summary::of(&rows);
    let outcomes: HashMap<String, usize> = summary.outcomes.iter().cloned().collect();

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
    eprintln!(
        "solvable: {} of {} — {} reached a sound system directly or after reduction;          {} rescued by index reduction, {} still singular, {} with no equations",
        summary.solvable, summary.total, summary.solvable,
        summary.rescued_by_reduction, summary.still_singular, summary.empty,
    );
    eprintln!("top failure causes:");
    for (c, n) in summary.causes.iter().take(5) {
        eprintln!("  {n:>5}  {c}");
    }
    eprintln!("wrote {out}");
}

/// Survey one model: compile it, then measure its shape.
fn survey_one(session: &mut Session, name: &str) -> SurveyRow {
    let mut row = SurveyRow {
        name: name.to_owned(),
        kind: classify(name),
        package: package_of(name),
        ..Default::default()
    };
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

    // Phenomena HRW has views for, counted from each equation's recorded
    // `origin` — the same field `equation_sheet::categorize_origin` reads. Free,
    // where re-running flatten to trace connection expansion would need the
    // whole resolved MSL again.
    let mut connect = 0usize;
    let mut flow = 0usize;
    let mut event = 0usize;
    for eq in &cr.dae.continuous.equations {
        let o = eq.origin.trim();
        if o.starts_with("connection equation") {
            connect += 1;
        } else if o.starts_with("flow sum") || o.starts_with("unconnected flow") {
            flow += 1;
        } else if o.contains("when") || o.contains("reinit") {
            event += 1;
        }
    }
    row.n_connect_eq = Some(connect);
    row.n_flow_eq = Some(flow);
    row.n_event_eq = Some(event);

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
            fill_blocks(&mut row, &rep);
        }
        Err(e) => {
            row.structural = match e {
                rumoca_phase_structural::StructuralError::Singular { .. } => "singular".to_owned(),
                other => format!("error:{}", first_line(&other.to_string())),
            };
            // **Run the reduction funnel and record whether it rescues the
            // system.** Without this, `singular` conflates a healthy high-index
            // model with a genuinely ill-posed one, and the first survey could
            // characterise neither. Uses HRW's own `index_reduce_in_place`, so
            // the survey and the app cannot disagree about what reduction means.
            if row.structural == "singular" {
                let mut reduced = cr.dae.clone();
                index_reduce_in_place(&mut reduced);
                match rumoca_phase_structural::build_structural_report(&reduced) {
                    Ok(rep) => {
                        row.index_reduced = "ok".to_owned();
                        fill_blocks(&mut row, &rep);
                    }
                    Err(rumoca_phase_structural::StructuralError::Singular { .. }) => {
                        row.index_reduced = "singular".to_owned();
                    }
                    Err(other) => {
                        row.index_reduced = format!("error:{}", first_line(&other.to_string()));
                    }
                }
            }
        }
    }
    row
}

/// Block counts from whichever structural report is the solvable one.
fn fill_blocks(row: &mut SurveyRow, rep: &rumoca_phase_structural::StructuralReport) {
    row.n_blocks = Some(rep.blocks.len());
    let coupled: Vec<usize> = rep
        .blocks
        .iter()
        .filter_map(|b| match b {
            rumoca_phase_structural::BlockReport::Coupled { unknowns, .. } => Some(unknowns.len()),
            rumoca_phase_structural::BlockReport::Scalar { .. } => None,
        })
        .collect();
    row.n_coupled = Some(coupled.len());
    row.largest_coupled = Some(coupled.into_iter().max().unwrap_or(0));
}

fn first_line(s: &str) -> String {
    s.lines().next().unwrap_or("").chars().take(200).collect()
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
