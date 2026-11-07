//! **The fidelity checks at scale** — F1-F9 over an MSL corpus.
//!
//! The scale counterpart to the test in `src/fidelity.rs`. Same check
//! functions, two callers: that test is the fast pre-commit gate over the
//! curated specimens, this is the run that produces the report
//! (`docs/reports.md`). **A check that exists twice is a check that drifts**,
//! which is the exact defect F1 and F7 both found on 2026-07-31.
//!
//! # Staged, because most first-run violations are the CHECK's fault
//!
//! Of twelve violations across F6-F9's first runs, **nine were the check's
//! fault**, all of one shape: a check that knows one form of the truth reports
//! every other form as a defect. Running the full corpus first would produce a
//! flood dominated by my own bugs. So:
//!
//! | stage | `--limit` | for |
//! |---|---|---|
//! | B | ~20 | shake out "my check assumes a shape MSL does not have" |
//! | C | ~60 | measure real per-model cost; shape bugs B missed |
//! | D | full | the run that produces the artifact |
//!
//! # Violations are grouped, not listed
//!
//! F7's first run produced **6,169** violations and was diagnosable only because
//! the first few lines happened to be representative. At corpus scale that is
//! luck. Output is grouped by check with counts and examples, so a flood reads
//! as *"F7: 6,169, all of one shape"*.
//!
//! # Reduction is capped, models are not excluded
//!
//! # Bounded by process lifetime, and why that is not paranoia
//!
//! On 2026-07-31 an unbounded run of 53 models **made Doug's machine
//! unusable** and forced a hard power-cycle. The runner rebuilt its session
//! every 200 models, so on a 53-model corpus **no rebuild ever fired**, and the
//! session accumulated across the largest systems in the MSL — including a
//! 10,175-equation model and one with 110 functions whose inlined bodies are
//! enormous.
//!
//! **A session rebuild cannot be the guarantee.** It releases what the session
//! holds; it cannot release what the allocator has fragmented or what any other
//! cache retains. **Only process exit does**, because the OS reclaims
//! everything unconditionally.
//!
//! So `--max-models` (default **25**) processes a chunk and exits; `--resume`
//! skips rows already in the report and the sink appends. A driver loop runs
//! chunks until nothing is left, and peak memory is bounded by one chunk rather
//! than by the whole corpus. `--rebuild-every` (default 10) reduces growth
//! *within* a chunk, but it is the belt, not the braces.
//!
//! **Never run this unbounded.** The cost of being wrong is not a slow run, it
//! is someone's machine.
//!
//! `--max-reduce-eq` is accepted and **still ignored**: index reduction is not checked
//! by this runner (the re-model landed 2026-11-04, but the runner was never re-wired to
//! it — `fidelity::check_model`'s note). It mirrored the survey, for the same reason: reduction cost
//! explodes on large systems. Above the cap the `IndexReduction` subjects are
//! absent and their checks skip — so a 10,175-equation model still contributes
//! F2, F3, F5, F6 and F7 rather than being dropped. **F8 explicitly wants the
//! largest models**, so excluding them would cost the scale coverage that is
//! most of the point.
//!
//! ```text
//! cargo run -p hrw --release --example fidelity_msl -- [--limit N] [--slice i/N]
//!                                    [--models a,b,c] [--out PATH] [--max-reduce-eq N]
//! ```

use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::time::Instant;

use hrw::fidelity::{CheckTiming, Coverage, Violation, check_model, group_by_check};
use hrw::survey::{SurveyRow, classify};
use hrw::worker::{FromWorker, StageKind, WorkerState};

/// Candidate models, read from the survey rather than re-derived.
///
/// **The survey is the corpus definition.** Re-enumerating from a session here
/// would be a second definition of "which models exist", and the two would
/// drift the moment MSL moves.
fn corpus() -> Vec<SurveyRow> {
    let path = hrw::manifest_path!("docs", "reports", "msl-survey.csv");
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("run the survey first: cannot read {path}: {e}"));
    hrw::survey::parse_csv(&text)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    // **No default into `docs/`.** This used to default to
    // `docs/fidelity-report.csv`, which (a) is a retired name — the specimen
    // report is `specimen-fidelity-report.csv` and the corpus artifact is
    // `msl-fidelity-report.csv` — and (b) meant any bare invocation silently
    // overwrote a committed artifact. That happened on 2026-07-31 during a
    // profiling run. A corpus run always passes `--out`; requiring it is free.
    let Some(out) = arg(&args, "--out") else {
        eprintln!(
            "--out is required. Corpus runs write to a working directory and are promoted \
             into docs/ by scripts/promote-run.ps1; writing there directly would overwrite an artifact."
        );
        std::process::exit(2);
    };
    // `--max-reduce-eq` is accepted and ignored: index reduction is not checked here,
    // so there is no reduction cost to cap. Left accepted rather than rejected so a
    // saved command line keeps working. The re-model that makes the check possible
    // landed 2026-11-04; wiring it in is open (`check_model`'s note).
    let resume = args.iter().any(|a| a == "--resume");
    // `--only-checks F2` runs one check, so the same model profiled once per
    // check yields per-check time AND peak memory from the existing watchdog.
    let only: Option<std::collections::BTreeSet<String>> = arg(&args, "--only-checks")
        .map(|v| v.split(',').map(|c| c.trim().to_uppercase()).collect());
    if let Some(set) = &only {
        // **An unrecognised check name is fatal now, and this is what it cost.**
        //
        // On 2026-11-05 this flag was handed `F1,F2,F3,F5,F6,F7,F8,F9` to exclude the
        // blind F4. `F1` is retired and `F8` raises no violation, so both
        // selected nothing; `F10` was simply absent and so was skipped; and the 2.5-hour
        // result was read as eight checks when five had run. Every part of that was
        // accepted in silence. Exiting here costs one retyped command.
        let unknown = hrw::fidelity::unknown_checks(set.iter().map(String::as_str));
        if !unknown.is_empty() {
            eprintln!(
                "--only-checks: {} is not a check. The roster is {}.\n  \
                 F1 is retired (its surviving half is F2) and F8 is a size measurement \
                 that raises no violation, so neither can be selected -- see \
                 fidelity::CHECKS.",
                unknown.join(", "),
                hrw::fidelity::CHECKS.join(","),
            );
            std::process::exit(2);
        }
        // **Report what is being SKIPPED, not only what was asked for.**
        //
        // Printing the request back cannot reveal a check left out of it, and an
        // omission is the failure this flag actually suffered: F10 — the only check
        // about whether a pane's claim is honest, and the one with the best must-fire
        // coverage — was dropped by a string nobody re-read against the roster.
        let skipped: Vec<&str> = hrw::fidelity::CHECKS
            .iter()
            .copied()
            .filter(|c| !set.contains(*c))
            .collect();
        eprintln!(
            "--only-checks: running {}",
            set.iter().cloned().collect::<Vec<_>>().join(",")
        );
        if !skipped.is_empty() {
            eprintln!("  NOT running: {}", skipped.join(","));
        }
    }
    // **Process lifetime is the only hard memory bound.** A session rebuild
    // releases what the session holds; it cannot release what the allocator has
    // fragmented or what any other cache retains. Exiting does, because the OS
    // reclaims everything. See the note on `--max-models` in the module docs.
    let max_models: usize = arg(&args, "--max-models")
        .and_then(|v| v.parse().ok())
        .unwrap_or(25);
    let rebuild_every: usize = arg(&args, "--rebuild-every")
        .and_then(|v| v.parse().ok())
        .unwrap_or(10);

    let mut rows = corpus();
    eprintln!("survey corpus: {} models", rows.len());

    // An explicit list beats any sampling when chasing a specific failure.
    if let Some(list) = arg(&args, "--models") {
        let want: Vec<&str> = list.split(',').map(str::trim).collect();
        rows.retain(|r| want.contains(&r.name.as_str()));
        eprintln!("--models: {} matched", rows.len());
    }
    if let Some(n) = arg(&args, "--limit").and_then(|v| v.parse::<usize>().ok()) {
        // Spread across the corpus, not a prefix: the first N are all
        // `Modelica.Blocks.*` and would test one package's shapes.
        let step = rows.len().div_ceil(n.max(1));
        rows = rows.into_iter().step_by(step.max(1)).collect();
        eprintln!("--limit {n}: {} models, every {step}th", rows.len());
    }
    if let Some((i, n)) = arg(&args, "--slice").and_then(|s| {
        let (a, b) = s.split_once('/')?;
        Some((a.parse::<usize>().ok()?, b.parse::<usize>().ok()?))
    }) {
        assert!(n > 0 && i < n, "--slice i/N needs 0 <= i < N");
        rows = rows.into_iter().skip(i).step_by(n).collect();
        eprintln!("--slice {i}/{n}: {} models", rows.len());
    }

    // Rows already reported, when resuming a chunked run.
    let done: std::collections::BTreeSet<String> = if resume {
        std::fs::read_to_string(&out)
            .map(|t| {
                hrw::report::parse(&t)
                    .rows
                    .into_iter()
                    .filter(|r| !r.name.is_empty() && !r.outcome.is_empty())
                    .map(|r| r.name)
                    .collect()
            })
            .unwrap_or_default()
    } else {
        std::collections::BTreeSet::new()
    };
    if !done.is_empty() {
        rows.retain(|r| !done.contains(&r.name));
        eprintln!(
            "--resume: {} already done, {} remaining",
            done.len(),
            rows.len()
        );
    }
    let total_remaining = rows.len();
    if rows.len() > max_models {
        rows.truncate(max_models);
        eprintln!(
            "--max-models {max_models}: doing {} of {total_remaining}, then exiting so the OS \
             reclaims. Re-run with --resume for the next chunk.",
            rows.len(),
        );
    }
    if rows.is_empty() {
        eprintln!("[done] nothing left to do");
        return;
    }

    // **Memory is this run's binding constraint, so the parse memo is off.**
    //
    // `worker::parsed_source_root` keeps each library root's parsed documents for
    // the process lifetime, which is ~326 MB of retained working set (measured
    // 2026-08-21: peak 522 MB without it against 848 MB with it). That is the right
    // trade for the test suite and the app, which load the MSL often and save ~48 s.
    //
    // **It is the wrong trade here.** This run rebuilds its `WorkerState` every N
    // models precisely to reclaim memory, runs under a 3 GB free-RAM watchdog, and
    // has already lost models to memory limits — so it would gain ~2.5 s per rebuild,
    // which it does rarely, while paying 326 MB continuously.
    //
    // Disabling costs nothing in fidelity: the memo only ever served a parse of the
    // exact bytes on disk. See `disable_parsed_source_root_memo` for why disabling
    // is the mechanism rather than clearing between loads.
    hrw::worker::disable_parsed_source_root_memo();

    eprintln!("loading MSL…");
    let t0 = Instant::now();
    let mut w = WorkerState::new();
    w.load_libraries(hrw::msl_roots()).expect("load MSL");
    eprintln!("loaded in {:.1}s", t0.elapsed().as_secs_f64());

    let mut sink = open_sink(&out, done.is_empty()).expect("open the report");
    let mut all: Vec<Violation> = Vec::new();
    let mut checked = 0usize;
    let mut no_dae = 0usize;
    let mut sizes: Vec<(usize, String)> = Vec::new();
    let mut cov = Coverage::default();
    let mut timing = CheckTiming::default();
    let t_run = Instant::now();

    for (i, row) in rows.iter().enumerate() {
        if i > 0 && i % rebuild_every == 0 {
            w = WorkerState::new();
            w.load_libraries(hrw::msl_roots()).expect("reload MSL");
            eprintln!("  [rebuild] fresh session after {i} models");
        }

        let t = Instant::now();
        // **Capture the per-stage timings the worker already emits.**
        //
        // `worker.rs` logs `Parse (12.3ms)`, `Resolve (…)`, `Flatten (…)` and so
        // on for every model — and this callback used to ignore its argument, so
        // all of it went into a void. Which phase of HRW's compile path dominates
        // on a large model was recorded in `docs/architecture.md` as an
        // *Inference* because of that, when the measurement was already being
        // produced and thrown away.
        //
        // Each stage marker is also written to stderr AS IT HAPPENS and flushed,
        // so the watchdog can read the last line and say which phase a hung model
        // is sitting in — the thing that could not be seen while a model burned
        // 900 s.
        let stage_ms: std::sync::Mutex<Vec<(String, f64)>> = std::sync::Mutex::new(Vec::new());
        let on_event = |ev: FromWorker| {
            let FromWorker::Log(entry) = ev else { return };
            match entry.level {
                // **StageStart is what makes the narration answer the question.**
                //
                // Reporting only StageEnd showed the last COMPLETED phase, so a
                // model stuck 500 s into its first phase displayed the startup
                // line and the phase had to be inferred. Emitting the start means
                // the tail of stderr always names the phase actually RUNNING.
                hrw::worker::LogLevel::StageStart => {
                    // **Written to a FILE, not to stderr.**
                    //
                    // `WorkerState::compile` starts an `OutputCapture` that
                    // redirects stdout and stderr at the FILE-DESCRIPTOR level so
                    // Rumoca's own `println!` diagnostics can be forwarded as log
                    // entries. This callback runs INSIDE that compile, so anything
                    // printed here is swallowed by the capture and never reaches
                    // the real stderr — which is exactly why the first attempt at
                    // live narration produced nothing while the after-the-fact
                    // summary worked fine.
                    //
                    // A direct file write bypasses the redirected descriptors. The
                    // watchdog reads this file to say which phase is running.
                    let _ = std::fs::write(
                        std::env::temp_dir().join("fid-phase.txt"),
                        format!("{} (running, {:.0}s in)", entry.message, entry.elapsed_secs),
                    );
                }
                hrw::worker::LogLevel::StageEnd => {
                    // Messages look like `Parse (12.3ms)`; keep the name and number.
                    if let Some((name, rest)) = entry.message.split_once(" (") {
                        let ms = rest.trim_end_matches("ms)").parse::<f64>().unwrap_or(0.0);
                        let _ = std::fs::write(
                            std::env::temp_dir().join("fid-phase.txt"),
                            format!("{name} done in {:.1}s", ms / 1000.0),
                        );
                        if let Ok(mut v) = stage_ms.lock() {
                            v.push((name.to_owned(), ms));
                        }
                    }
                }
                _ => {}
            }
        };

        // **A panic here used to kill the sweep and leave no trace of the model.**
        //
        // `compile_model_by_name` is not wrapped in `worker.rs`'s `guard`, whose only
        // caller is `Worker::spawn`'s message loop — which is why the GUI survives a
        // model that kills this binary. Worse, `WorkerState::compile` holds an
        // `OutputCapture` that redirects fds 1 and 2 through a pipe, and the default
        // panic hook prints BEFORE unwinding reaches that guard's `Drop`, so the message
        // went into the pipe and died with the process. Two MSL models did exactly that
        // on 2026-11-05: exit 101, an empty stderr even under `RUST_BACKTRACE=full`, and
        // no report row — while the watchdog recorded both as `ok`.
        //
        // `catch_unwind` recovers the PAYLOAD, which the fd redirect never touches, so
        // the message survives into the report rather than going nowhere.
        //
        // **The session is rebuilt, not reused.** `AssertUnwindSafe` is honest only
        // because of that: a panicked `Session` can be mid-mutation, and `worker.rs`'s
        // `guard` carries the same note for the same reason — reusing it would risk IR
        // that is subtly wrong rather than absent, which this project ranks as worse.
        let attempt = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            w.compile_model_by_name(&row.name, &on_event)
        }));
        let event = match attempt {
            Ok(event) => event,
            Err(payload) => {
                let message = hrw::panic_message(payload.as_ref());
                eprintln!("  [PANIC] {}: {message}", row.name);
                let _ = writeln!(sink, "{}", note_row(&row.name, "panicked", &message))
                    .and_then(|()| sink.flush());
                w = WorkerState::new();
                w.load_libraries(hrw::msl_roots()).expect("reload MSL");
                continue;
            }
        };
        let FromWorker::Compiled {
            stages,
            dae,
            equation_sheet,
            identifier_index,
            ..
        } = event
        else {
            // **Not a panic, but the same silent drop.** A non-`Compiled` event used to
            // `continue` with no row and no note, so the report's row count could
            // disagree with the corpus and nothing would say so. It did not happen on
            // 2026-11-05 — 2,613 rows plus 11 ceiling aborts plus 2 panics accounts for
            // all 2,626 — which is the reason to close it now rather than after it does.
            let _ = writeln!(
                sink,
                "{}",
                note_row(
                    &row.name,
                    "no-compile",
                    "compile produced no Compiled event"
                )
            )
            .and_then(|()| sink.flush());
            continue;
        };

        // F8: every stage serialises, and the sizes are on the record.
        //
        // **Counted, not materialised.** `v.to_string().len()` builds the whole
        // stage IR as a String just to measure it — fine at 3 MB, ruinous on
        // `Media.Examples.TwoPhaseWater.TestTwoPhaseStates`, which has only 48
        // equations but **110 functions** and whose inlined bodies pushed this
        // to 4.5 GB resident and 100% CPU for over an hour. The survey compiled
        // that same model in under 5 seconds, which is what proved the cost was
        // the check rather than the compile.
        let bytes: usize = StageKind::COMPILATION
            .iter()
            .map(|k| stages.get(*k).value.as_ref().map_or(0, json_bytes))
            .sum();
        sizes.push((bytes, row.name.clone()));

        let mut violations = Vec::new();
        if let Some(dae) = dae.as_ref() {
            // **No reduced DAE passed, and so no reduction cap.** The reduced system
            // is available since the 2026-11-04 re-model (`PreparedDae::as_dae()`),
            // but this runner is not handed it yet, so `--max-reduce-eq` has nothing
            // to cap. `check_model`'s note records that the cap must return with the
            // wiring, because it is what keeps the largest models in the corpus.
            violations = check_model(
                &stages,
                dae,
                equation_sheet.as_ref(),
                identifier_index.as_ref(),
                &mut cov,
                &mut timing,
                only.as_ref(),
            );
            checked += 1;
        } else {
            no_dae += 1;
        }
        // **F9 moved into `check_model` on 2026-11-06**, with the rest of the checks.
        // It lived here because it needs the whole bundle rather than a subject, which
        // also put it outside `--only-checks` -- asking for F9 did not select it and
        // omitting F9 did not skip it -- and out of reach of `cargo test`, which is why
        // nothing had ever driven it with a deliberate violation. `check_f10` has the
        // same signature and never left `fidelity.rs`, so the split bought nothing.

        // One compact line per model: the phases that actually cost something.
        // Sorted by cost and truncated, because a full ten-phase dump per model
        // across 2,626 models is noise rather than data.
        if let Ok(v) = stage_ms.lock() {
            let mut top: Vec<&(String, f64)> = v.iter().filter(|(_, ms)| *ms >= 1.0).collect();
            top.sort_by(|a, b| b.1.total_cmp(&a.1));
            if !top.is_empty() {
                let total: f64 = v.iter().map(|(_, ms)| ms).sum();
                let parts: Vec<String> = top
                    .iter()
                    .take(4)
                    .map(|(n, ms)| format!("{n} {:.1}s", ms / 1000.0))
                    .collect();
                eprintln!(
                    "  [phases] {:.1}s total: {}",
                    total / 1000.0,
                    parts.join(", ")
                );
            }
        }

        let secs = t.elapsed().as_secs_f64();
        if secs >= 10.0 {
            eprintln!("  [slow] {secs:.1}s  {}", row.name);
        }
        let _ =
            writeln!(sink, "{}", report_row(&row.name, &violations)).and_then(|()| sink.flush());
        all.extend(violations);

        if (i + 1) % 25 == 0 || i + 1 == rows.len() {
            eprintln!(
                "  {}/{} ({:.0}s) — {} violations so far",
                i + 1,
                rows.len(),
                t_run.elapsed().as_secs_f64(),
                all.len(),
            );
        }
    }

    // **Where the time actually went.** Printed always, not behind a flag: the
    // cost distribution is the finding, and a run that does not say which check
    // dominated leaves the next person guessing, which is how this got recorded
    // as inference in the first place.
    let total_ms = timing.total_ms();
    if total_ms > 0.0 {
        eprintln!(
            "\n[check cost] {:.1}s inside the checks:",
            total_ms / 1000.0
        );
        for (check, ms) in timing.ranked() {
            eprintln!(
                "  {check}  {:>8.1}s  {:>5.1}%",
                ms / 1000.0,
                100.0 * ms / total_ms,
            );
        }
    }

    sizes.sort_by_key(|(b, _)| std::cmp::Reverse(*b));
    eprintln!(
        "\n[done] {} models in {:.0}s; {checked} with a DAE, {no_dae} without, \
         reduction not checked (see check_model)",
        rows.len(),
        t_run.elapsed().as_secs_f64(),
    );
    eprintln!(
        "coverage: {} subjects ({} with blocks, {} with a matching), {} stage IRs run, \
         {} equation sheets, {} identifier indexes, {} empty stages",
        cov.subjects,
        cov.with_blocks,
        cov.with_matching,
        cov.stage_irs,
        cov.with_sheet,
        cov.with_index,
        cov.empty_stages,
    );
    // **`empty stages` is F10's coverage, and on this corpus it is expected to be 0.**
    //
    // Measured 2026-08-05: no MSL model produces an empty stage, so F10's absence
    // clause has nothing to act on and its zero violations cover only the other two.
    // Said out loud here rather than left for a reader to infer from a bare 0, which
    // is the same mistake as reading "0 violations" as "everything was checked".
    if cov.empty_stages == 0 {
        eprintln!(
            "note: 0 empty stages, so F10's absence clause was never exercised \u{2014} \
             its zero covers the provenance clauses only. Absence is a property of \
             failing compiles, and this corpus has none.",
        );
    }
    eprintln!("largest stage IR:");
    for (b, n) in sizes.iter().take(3) {
        eprintln!("  {:>9} bytes  {n}", b);
    }

    // **The triage view.** Grouped by check, largest first, with examples —
    // the difference between a diagnosis and a wall of text.
    eprintln!("\n=== {} violations ===", all.len());
    for (check, n, examples) in group_by_check(&all, 3) {
        eprintln!("{check}  {n} violations");
        for e in examples {
            eprintln!("      {}", e.chars().take(160).collect::<String>());
        }
    }
    eprintln!("\nwrote {out}");
}

/// One row of the fidelity report, sharing its first four columns with every
/// other report so one loader serves all three (`docs/reports.md`).
fn report_row(name: &str, violations: &[Violation]) -> String {
    let mut checks: Vec<&str> = violations.iter().map(|v| v.check).collect();
    checks.sort_unstable();
    checks.dedup();
    let f = hrw::survey::csv_field;
    format!(
        "{},{},{},{},{},{}",
        f(name),
        f(&classify(name)),
        if violations.is_empty() {
            "ok"
        } else {
            "violations"
        },
        f(violations.first().map_or("", |v| v.detail.as_str())),
        f(&checks.join(" ")),
        violations.len(),
    )
}

/// One row for a model that produced **no checks at all**, with the reason in the
/// `message` column.
///
/// Shares [`report_row`]'s six columns so the report keeps one schema, and carries
/// `panicked` or `no-compile` in `outcome` rather than `ok`. That distinction is the
/// whole point: in the 2026-11-05 report, 2,613 rows meaning "measured and clean" and
/// two models meaning "never compiled" were indistinguishable, because the second kind
/// had no row and the watchdog beside it said `ok`. **A model the runner could not
/// measure must appear, and must not read as a pass.**
fn note_row(name: &str, outcome: &str, message: &str) -> String {
    let f = hrw::survey::csv_field;
    format!(
        "{},{},{},{},,0",
        f(name),
        f(&classify(name)),
        f(outcome),
        f(message),
    )
}

/// Serialised size of a JSON value **without building it**.
///
/// `serde_json::to_writer` streams into a sink that counts and discards, so the
/// peak allocation is a small buffer rather than the whole document.
fn json_bytes(v: &serde_json::Value) -> usize {
    struct Counting(usize);
    impl std::io::Write for Counting {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0 += buf.len();
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut c = Counting(0);
    serde_json::to_writer(&mut c, v).map_or(0, |()| c.0)
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
        writeln!(f, "name,kind,outcome,message,checks_failed,n_violations")?;
        f.flush()?;
    }
    Ok(f)
}

fn arg(args: &[String], flag: &str) -> Option<String> {
    let i = args.iter().position(|a| a == flag)?;
    args.get(i + 1).cloned()
}

/// Unused today; keeps the map type available for per-kind grouping if a
/// stage-B run shows one check producing two genuinely different failures.
#[allow(
    dead_code,
    reason = "reserved for per-kind grouping; see fidelity::Violation"
)]
type ByKind = BTreeMap<String, usize>;
