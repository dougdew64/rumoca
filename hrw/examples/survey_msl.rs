//! **Survey every model in the vendored MSL.**
//!
//! Produces `docs/reports/msl-survey.csv`: one row per model, recording whether Rumoca
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
//!    shape rather than physics domain.
//!
//! # This measures Rumoca, not HRW — deliberately
//!
//! It calls `Session` directly rather than going through `WorkerState::compile`.
//! Routed through HRW, a bug in HRW's stage extraction would be recorded as a
//! *Rumoca* failure — the misattribution `docs/upstream-strategy.md` warns turns
//! a capability map into an unfair scorecard.
//!
//! # The cost is in a handful of models, so reduction is capped
//!
//! Measured 2026-07-31, and it decided this program's shape. A full run with
//! index reduction reached model **1,400 of 2,626 in 29 minutes**, then spent
//! **97 more minutes on four models**:
//!
//! | model | equations |
//! |---|---|
//! | `…Spice3BenchmarkFourBitBinaryAdder` | 10,175 |
//! | `….FOURBIT` | 10,046 |
//! | `….TWOBIT` | 4,992 |
//! | `….ONEBIT` | 2,477 |
//!
//! Among the 1,209 models needing reduction the median is **24** equations, so
//! `--max-reduce-eq` (default 800) skips **5.9%** of reductions and removes
//! **~93%** of the cost, recording `index_reduced = skipped:too-large` rather
//! than omitting the fact. **A stated bound is honest; an unfinishable run is
//! not** (`docs/upstream-strategy.md` planning rule 3).
//!
//! `--only-skipped` then revisits exactly those rows with no cap, so the survey
//! runs in two parts and part 1 stands alone if part 2 never finishes.
//!
//! # Running it
//!
//! ```text
//! # part 1 — everything, reduction capped, 8 shards in parallel
//! for i in 0..8: survey_msl --slice i/8 --out part-$i.csv
//! survey_msl --merge part-0.csv,…,part-7.csv --out docs/reports/msl-survey.csv
//!
//! # part 2 — the models part 1 capped, no limit, however long it takes
//! survey_msl --only-skipped --out docs/reports/msl-survey.csv
//! ```
//!
//! **Parallelism is by process, not thread.** `Session` is not thread-safe, and
//! separate processes make the question moot — each gets its own session, a
//! crash in one shard does not take the run with it, and memory is observable
//! per worker. Slicing is by index into the sorted name list and the merge
//! sorts, so **the output is byte-identical regardless of shard count**, which
//! is what keeps it reproducible (planning rule 2).
//!
//! # Written incrementally, and instrumented
//!
//! Each row is appended and flushed as produced: the run is resumable, the file
//! is watchable, and a partial file is a usable report. The first full run wrote
//! nothing until the end and was piped through `tail` (which buffers to EOF), so
//! a kill at 96% would have discarded everything and there was no progress to
//! read. **Do not pipe this through `tail`.**
//!
//! A health line every `--window` models reports rate, outcome mix and the
//! slowest model, and flags anomalies **against the run's own history** rather
//! than against thresholds someone guessed: a window far slower than the median
//! so far, or a success rate that collapses relative to the run's own average
//! (the signature a poisoned session would leave). Memory is watched externally
//! — reading RSS in-process needs a dependency, and adding one needs approval.

use std::collections::{BTreeSet, HashMap};
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::time::Instant;

use rumoca_compile::compile::{PhaseResult, SourceRootKind};
use rumoca_compile::source_roots::{parse_source_root_with_cache, source_root_source_set_key};
use rumoca_compile::{Session, SessionConfig};

use hrw::survey::{Summary, SurveyRow, classify, package_of};
use hrw::worker::index_reduce_in_place;

/// Marks a reduction the cap declined to attempt. Part 2 looks for exactly this.
const SKIPPED: &str = "skipped:too-large";

fn msl_roots() -> Vec<PathBuf> {
    let base = format!("{}/vendor/msl", env!("CARGO_MANIFEST_DIR"));
    vec![
        PathBuf::from(format!("{base}/Modelica 4.1.0")),
        PathBuf::from(format!("{base}/ModelicaServices 4.1.0")),
        PathBuf::from(format!("{base}/Complex.mo")),
    ]
}

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

struct Config {
    out: String,
    limit: Option<usize>,
    resume: bool,
    only_skipped: bool,
    slice: Option<(usize, usize)>,
    max_reduce_eq: usize,
    rebuild_every: usize,
    slow_secs: f64,
    window: usize,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if let Some(list) = arg_value(&args, "--merge") {
        merge(
            &list,
            &arg_value(&args, "--out").expect("--merge needs --out"),
        );
        return;
    }

    let cfg = Config {
        out: arg_value(&args, "--out").unwrap_or_else(|| {
            format!("{}/docs/reports/msl-survey.csv", env!("CARGO_MANIFEST_DIR"))
        }),
        limit: arg_value(&args, "--limit").and_then(|v| v.parse().ok()),
        resume: args.iter().any(|a| a == "--resume"),
        only_skipped: args.iter().any(|a| a == "--only-skipped"),
        slice: arg_value(&args, "--slice").and_then(|s| {
            let (i, n) = s.split_once('/')?;
            Some((i.parse().ok()?, n.parse().ok()?))
        }),
        max_reduce_eq: arg_value(&args, "--max-reduce-eq")
            .and_then(|v| v.parse().ok())
            .unwrap_or(800),
        rebuild_every: arg_value(&args, "--rebuild-every")
            .and_then(|v| v.parse().ok())
            .unwrap_or(500),
        slow_secs: arg_value(&args, "--slow-secs")
            .and_then(|v| v.parse().ok())
            .unwrap_or(10.0),
        window: arg_value(&args, "--window")
            .and_then(|v| v.parse().ok())
            .unwrap_or(100),
    };
    run(cfg);
}

fn run(cfg: Config) {
    eprintln!("loading MSL…");
    let t0 = Instant::now();
    let mut session = load_msl();
    let counts = session.class_type_counts().unwrap_or_default();
    let mut names: Vec<String> = session.model_names().expect("MSL should resolve").to_vec();
    names.sort();
    names.dedup();
    let available = names.len();
    eprintln!(
        "loaded in {:.1}s — {available} models among {} classes: {}",
        t0.elapsed().as_secs_f64(),
        counts.values().sum::<usize>(),
        summarize(&counts),
    );

    if let Some(n) = cfg.limit {
        let step = names.len().div_ceil(n.max(1));
        names = names.into_iter().step_by(step.max(1)).collect();
        eprintln!("--limit {n}: {} models, every {step}th", names.len());
    }

    // **Slice by index into the SORTED list**, so shard membership is a pure
    // function of the corpus. Any shard count produces the same union, and the
    // merge sorts — the output is byte-identical regardless of parallelism.
    if let Some((i, n)) = cfg.slice {
        assert!(n > 0 && i < n, "--slice i/N needs 0 <= i < N");
        names = names.into_iter().skip(i).step_by(n).collect();
        eprintln!("--slice {i}/{n}: {} models", names.len());
    }

    // Part 2: revisit only what part 1's cap declined, with no cap.
    let (mut rows, todo) = if cfg.only_skipped {
        let prior = load_partial(&cfg.out);
        let redo: BTreeSet<String> = prior
            .iter()
            .filter(|r| r.index_reduced == SKIPPED)
            .map(|r| r.name.clone())
            .collect();
        eprintln!("--only-skipped: {} rows to revisit with no cap", redo.len());
        let keep: Vec<SurveyRow> = prior
            .into_iter()
            .filter(|r| !redo.contains(&r.name))
            .collect();
        let todo: Vec<String> = names
            .iter()
            .filter(|n| redo.contains(*n))
            .cloned()
            .collect();
        // Rewritten without the rows being redone, so resume semantics hold.
        rewrite(&cfg.out, &keep);
        (keep, todo)
    } else {
        let prior = if cfg.resume {
            load_partial(&cfg.out)
        } else {
            Vec::new()
        };
        if !prior.is_empty() {
            eprintln!("--resume: {} rows already surveyed", prior.len());
        }
        let done: BTreeSet<String> = prior.iter().map(|r| r.name.clone()).collect();
        let todo: Vec<String> = names
            .iter()
            .filter(|n| !done.contains(*n))
            .cloned()
            .collect();
        (prior, todo)
    };

    let cap = if cfg.only_skipped {
        usize::MAX
    } else {
        cfg.max_reduce_eq
    };
    eprintln!(
        "surveying {} models; index reduction {}",
        todo.len(),
        if cap == usize::MAX {
            "uncapped".to_owned()
        } else {
            format!("capped at {cap} equations")
        },
    );

    // What this process is responsible for, for the progress denominator.
    let expected_total = rows.len() + todo.len();
    let fresh = rows.is_empty();
    let mut sink = match open_sink(&cfg.out, fresh) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("cannot write {}: {e}", cfg.out);
            return;
        }
    };

    let mut health = Health::new(cfg.window, &cfg.out);
    let t_run = Instant::now();

    for (i, name) in todo.iter().enumerate() {
        // A fresh session every `rebuild_every` models bounds what the session
        // accumulates — 8.3 GB of committed memory over 2,626 compiles, measured
        // 2026-07-31 — and is what makes several shards fit in RAM at once. It
        // also bounds exposure to `docs/upstream-issues.md` #1, where a failed
        // resolve can poison a later compile in the same session.
        if i > 0 && i % cfg.rebuild_every == 0 {
            eprintln!("  [rebuild] fresh session after {i} models");
            session = load_msl();
        }

        let row = survey_one(&mut session, name, cap);
        if let Err(e) = writeln!(sink, "{}", row.to_csv()).and_then(|()| sink.flush()) {
            eprintln!("write failed after {} rows: {e}", rows.len());
            break;
        }
        // Denominator is THIS shard's workload, not the corpus: a worker
        // reporting `30/2626` reads as 1% done when it is halfway.
        health.record(&row, cfg.slow_secs, rows.len() + 1, expected_total);
        rows.push(row);
    }

    let total = t_run.elapsed().as_secs_f64();
    drop(sink);
    // Sorted on completion: `--resume` and `--only-skipped` both append out of
    // order, and a diffable file is the point of checking it in.
    rows.sort_by(|a, b| a.name.cmp(&b.name));
    rewrite(&cfg.out, &rows);

    let summary = Summary::of(&rows);
    write_meta(&cfg, &rows, &summary, available, Some(total));
    health.finish(&summary, total);

    // **A column measuring nothing is a defect, and nothing used to ask.**
    // `n_event_eq` was zero across a whole corpus, survived a commit, and
    // reached a published artifact. Reported rather than fatal: on a small or
    // filtered corpus an all-zero column can be legitimate.
    let dead = hrw::survey::all_zero_columns(&rows);
    if !dead.is_empty() && rows.len() > 100 {
        eprintln!(
            "[WARNING] {} column(s) are zero for every row: {} — a column that never              fires looks exactly like one that works",
            dead.len(),
            dead.join(", "),
        );
    }
}

/// Per-window health, judged **against the run's own history**.
///
/// Absolute thresholds would be guesses; a window that is far slower than the
/// median so far, or whose success rate collapses relative to the run's own
/// average, is anomalous by the run's own standard. That is what makes this
/// useful for spotting a misbehaving run early enough to kill it.
struct Health {
    window: usize,
    log: Option<BufWriter<File>>,
    window_start: Instant,
    window_times: Vec<f64>,
    window_outcomes: HashMap<String, usize>,
    all_outcomes: HashMap<String, usize>,
    slowest: (f64, String),
    anomalies: usize,
}

impl Health {
    fn new(window: usize, out: &str) -> Health {
        let log = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(format!("{out}.health.log"))
            .ok()
            .map(BufWriter::new);
        Health {
            window,
            log,
            window_start: Instant::now(),
            window_times: Vec::new(),
            window_outcomes: HashMap::new(),
            all_outcomes: HashMap::new(),
            slowest: (0.0, String::new()),
            anomalies: 0,
        }
    }

    fn say(&mut self, line: &str) {
        eprintln!("{line}");
        if let Some(f) = self.log.as_mut() {
            let _ = writeln!(f, "{line}").and_then(|()| f.flush());
        }
    }

    fn record(&mut self, row: &SurveyRow, slow_secs: f64, done: usize, total: usize) {
        *self.window_outcomes.entry(row.outcome.clone()).or_default() += 1;
        *self.all_outcomes.entry(row.outcome.clone()).or_default() += 1;
        if row.secs > self.slowest.0 {
            self.slowest = (row.secs, row.name.clone());
        }
        // Named immediately, not at the end: this is the line that would have
        // told us on the first run which four models were eating the clock.
        if row.secs >= slow_secs {
            let n_eq = row.n_equations.unwrap_or(0);
            self.say(&format!(
                "  [slow] {:.1}s  {} ({n_eq} equations, structural={})",
                row.secs, row.name, row.structural,
            ));
        }
        if !done.is_multiple_of(self.window) {
            return;
        }

        let secs = self.window_start.elapsed().as_secs_f64();
        self.window_start = Instant::now();
        let ok = *self.window_outcomes.get("success").unwrap_or(&0);
        let win_rate = ok as f64 / self.window as f64;
        let all_ok: usize = *self.all_outcomes.get("success").unwrap_or(&0);
        let all_rate = all_ok as f64 / done as f64;

        let mix: Vec<String> = {
            let mut v: Vec<(&String, &usize)> = self.window_outcomes.iter().collect();
            v.sort_by_key(|(k, _)| k.as_str());
            v.iter().map(|(k, n)| format!("{k}={n}")).collect()
        };
        let line = format!(
            "[health] {done}/{total}  window {secs:.0}s ({:.1} models/min)  success {:.0}%  \
             slowest {:.1}s {}  |  {}",
            self.window as f64 / secs * 60.0,
            win_rate * 100.0,
            self.slowest.0,
            self.slowest.1,
            mix.join(" "),
        );
        self.say(&line);

        // --- anomalies, judged against this run's own history ---
        let median = {
            let mut t = self.window_times.clone();
            t.sort_by(f64::total_cmp);
            t.get(t.len() / 2).copied()
        };
        if let Some(m) = median
            && secs > m * 3.0
        {
            self.anomalies += 1;
            self.say(&format!(
                "  [ANOMALY] this window took {secs:.0}s, {:.1}x the median {m:.0}s — \
                 expected <=3x. A model is dominating; check the [slow] lines above.",
                secs / m,
            ));
        }
        if self.window_times.len() >= 2 && all_rate > 0.6 && win_rate < all_rate * 0.5 {
            self.anomalies += 1;
            self.say(&format!(
                "  [ANOMALY] success fell to {:.0}% this window against {:.0}% for the run — \
                 a poisoned session would look like this (upstream-issues #1).",
                win_rate * 100.0,
                all_rate * 100.0,
            ));
        }
        self.window_times.push(secs);
        self.window_outcomes.clear();
        self.slowest = (0.0, String::new());
    }

    fn finish(&mut self, summary: &Summary, total: f64) {
        let n = summary.total.max(1);
        self.say(&format!(
            "[done] {} models in {total:.0}s ({:.2}s/model), {} anomalies",
            summary.total,
            total / n as f64,
            self.anomalies,
        ));
        self.say(&format!(
            "  outcomes: {}",
            summary
                .outcomes
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect::<Vec<_>>()
                .join(" "),
        ));
        self.say(&format!(
            "  solvable {} of {}  (rescued by reduction {}, still singular {}, no equations {})",
            summary.solvable,
            summary.total,
            summary.rescued_by_reduction,
            summary.still_singular,
            summary.empty,
        ));
        for (c, n) in summary.causes.iter().take(5) {
            self.say(&format!("  cause {n:>5}  {c}"));
        }
    }
}

/// Survey one model: compile it, then measure its shape.
fn survey_one(session: &mut Session, name: &str, max_reduce_eq: usize) -> SurveyRow {
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
            row.compile_cost = SurveyRow::cost_bucket(row.secs).to_owned();
            return row;
        }
        Some(PhaseResult::NeedsInner { .. }) => {
            row.outcome = "needs_inner".to_owned();
            row.compile_cost = SurveyRow::cost_bucket(row.secs).to_owned();
            return row;
        }
        None => {
            row.outcome = "no_result".to_owned();
            row.message = first_line(&report.failure_summary(0));
            row.compile_cost = SurveyRow::cost_bucket(row.secs).to_owned();
            return row;
        }
    };

    let v = &cr.dae.variables;
    let n_eq = cr.dae.continuous.equations.len();
    row.n_equations = Some(n_eq);
    row.n_states = Some(v.states.len());
    row.n_algebraic = Some(v.algebraics.len());
    row.n_discrete = Some(v.discrete_reals.len() + v.discrete_valued.len());
    row.n_parameters = Some(v.parameters.len());
    row.n_functions = Some(cr.dae.symbols.functions.len());

    // Phenomena HRW has views for, from each equation's recorded `origin` — the
    // field `equation_sheet::categorize_origin` reads. Free, where re-running
    // flatten to trace connection expansion would need the resolved MSL again.
    let (mut connect, mut flow) = (0usize, 0usize);
    for eq in &cr.dae.continuous.equations {
        let o = eq.origin.trim();
        if o.starts_with("connection equation") {
            connect += 1;
        } else if o.starts_with("flow sum") || o.starts_with("unconnected flow") {
            flow += 1;
        }
    }
    row.n_connect_eq = Some(connect);
    row.n_flow_eq = Some(flow);

    // **Events do not live in `continuous.equations`.** They live in three
    // separate structures, which `events_to_json` reads and the first version of
    // this counter did not — so `n_event_eq` was zero for all 2,237 successes
    // while 1,089 models had discrete variables.
    row.n_event_conditions = Some(
        cr.dae.conditions.equations.len()
            + cr.dae.conditions.relations.len()
            + cr.dae.events.synthetic_root_conditions.len()
            + cr.dae.events.scheduled_time_events.len(),
    );
    row.n_discrete_updates =
        Some(cr.dae.discrete.real_updates.len() + cr.dae.discrete.valued_updates.len());

    let all_names = || {
        v.states
            .keys()
            .chain(v.algebraics.keys())
            .chain(v.parameters.keys())
            .chain(v.discrete_reals.keys())
            .chain(v.discrete_valued.keys())
            .map(ToString::to_string)
    };
    row.has_arrays = all_names().any(|n| n.contains('['));
    row.max_depth = all_names()
        .map(|n| n.matches('.').count())
        .max()
        .unwrap_or(0);

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
            if row.structural == "singular" {
                // **The cap.** Four Spice3 benchmark models at 2,477-10,175
                // equations consumed 97 of the first full run's 127 minutes.
                // Recorded as skipped rather than omitted, so the report states
                // its own bound and `--only-skipped` can come back for them.
                if n_eq > max_reduce_eq {
                    row.index_reduced = SKIPPED.to_owned();
                } else {
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
    }
    row.secs = t.elapsed().as_secs_f64();
    row.compile_cost = SurveyRow::cost_bucket(row.secs).to_owned();
    row
}

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

/// Concatenate shard CSVs into one sorted, deduplicated report.
fn merge(list: &str, out: &str) {
    let mut rows: Vec<SurveyRow> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for part in list.split(',') {
        let text = std::fs::read_to_string(part.trim()).unwrap_or_else(|e| {
            panic!("cannot read shard {part}: {e}");
        });
        for r in hrw::survey::parse_csv(&text) {
            if !r.name.is_empty() && !r.outcome.is_empty() && seen.insert(r.name.clone()) {
                rows.push(r);
            }
        }
    }
    rows.sort_by(|a, b| a.name.cmp(&b.name));
    rewrite(out, &rows);
    // The merged file is the published artifact, so it needs its OWN provenance:
    // the shards each wrote a sidecar describing only their slice, and a report
    // that cannot say what it describes is not reproducible (planning rule 2).
    let summary = Summary::of(&rows);
    let cfg = Config {
        out: out.to_owned(),
        limit: None,
        resume: false,
        only_skipped: false,
        slice: None,
        // The merge cannot know the shards' configured cap, and guessing one
        // would put a number nobody set into a published artifact. The
        // observable `largest_reduction_attempted` carries the real information.
        max_reduce_eq: 0,
        rebuild_every: 0,
        slow_secs: 0.0,
        window: 0,
    };
    let n = rows.len();
    write_meta(&cfg, &rows, &summary, n, None);
    eprintln!("merged {n} rows into {out}");
}

fn rewrite(out: &str, rows: &[SurveyRow]) {
    let mut csv = vec![SurveyRow::HEADER.to_owned()];
    csv.extend(rows.iter().map(SurveyRow::to_csv));
    if let Err(e) = std::fs::write(out, csv.join("\n") + "\n") {
        eprintln!("could not write {out}: {e}");
    }
}

fn write_meta(
    cfg: &Config,
    rows: &[SurveyRow],
    summary: &Summary,
    available: usize,
    secs: Option<f64>,
) {
    let mut tally: Vec<(&String, &usize)> = summary.outcomes.iter().map(|(k, v)| (k, v)).collect();
    tally.sort_by_key(|(k, _)| k.as_str());
    let meta = format!(
        "{{\n  \"rumoca_version\": \"{}\",\n  \"hrw_version\": \"{}\",\n  \
         \"msl_roots\": [{}],\n  \"models_surveyed\": {},\n  \"models_available\": {},\n  \
         \"partial_survey\": {},\n  \"max_reduce_equations\": {},
  \"largest_reduction_attempted\": {},\n  \"reductions_skipped\": {},\n  \
         \"seconds\": {},\n  \"generated_unix\": {},\n  \"outcomes\": {{{}}}\n}}\n",
        env!("HRW_RUMOCA_VERSION"),
        env!("CARGO_PKG_VERSION"),
        msl_roots()
            .iter()
            .map(|r| format!(
                "\"{}\"",
                r.file_name().unwrap_or_default().to_string_lossy()
            ))
            .collect::<Vec<_>>()
            .join(", "),
        rows.len(),
        available,
        rows.len() < available,
        // Null when the writer does not know it — a merge cannot. `0` would
        // read as a cap of zero, which is the same class of mistake as the
        // derived value it replaced.
        if cfg.max_reduce_eq == 0 {
            "null".to_owned()
        } else {
            cfg.max_reduce_eq.to_string()
        },
        // A lower bound on the cap that was actually in force, observable from
        // the data alone: the largest system reduction was ATTEMPTED on.
        // Distinct from `max_reduce_equations`, which is the configured cap —
        // conflating them would read as a cap nobody set.
        rows.iter()
            .filter(|r| r.index_reduced == "ok" || r.index_reduced == "singular")
            .filter_map(|r| r.n_equations)
            .max()
            .unwrap_or(0),
        rows.iter().filter(|r| r.index_reduced == SKIPPED).count(),
        secs.map_or("null".to_owned(), |v| format!("{v:.1}")),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs()),
        tally
            .iter()
            .map(|(k, n)| format!("\"{k}\": {n}"))
            .collect::<Vec<_>>()
            .join(", "),
    );
    let path = cfg.out.replace(".csv", ".meta.json");
    if let Err(e) = std::fs::write(&path, meta) {
        eprintln!("could not write {path}: {e}");
    }
}

fn open_sink(out: &str, fresh: bool) -> std::io::Result<BufWriter<File>> {
    let mut f = BufWriter::new(
        OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(fresh)
            .append(!fresh)
            .open(out)?,
    );
    if fresh {
        writeln!(f, "{}", SurveyRow::HEADER)?;
        f.flush()?;
    }
    Ok(f)
}

/// Rows already in a partial CSV, **dropping a torn final line**.
///
/// Per-row flushing makes a half-written line unlikely but not impossible, and a
/// resumed run that trusted one would carry a corrupt row into a published
/// report. A row missing its name or outcome cannot have been written
/// completely, so it is discarded and re-surveyed.
fn load_partial(out: &str) -> Vec<SurveyRow> {
    let Ok(text) = std::fs::read_to_string(out) else {
        return Vec::new();
    };
    hrw::survey::parse_csv(&text)
        .into_iter()
        .filter(|r| !r.name.is_empty() && !r.outcome.is_empty())
        .collect()
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
    v.iter()
        .take(8)
        .map(|(k, n)| format!("{k}={n}"))
        .collect::<Vec<_>>()
        .join(" ")
}
