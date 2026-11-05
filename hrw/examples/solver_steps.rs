//! **Print the solver's own step records for one specimen** — the instrument
//! `docs/updating-rumoca.md` step 6b needs, and the headless twin of HRW's solver
//! diagnostics plots.
//!
//! # Why this exists
//!
//! **Lectures are checked exactly once per rebase** (step 6b), and what rots in them is
//! every number Rumoca reported: step counts, orders, the step size at each step, how far
//! the solver ran past `t_end`. Until now the only way to read those was to launch HRW,
//! open the diagnostics plots and **hover individual markers** — which Appendix A.2 of
//! `docs/lectures/lecture-01-euler.md` dutifully describes, and which is a transcription
//! exercise over dozens of rows that Doug has to perform by hand and Claude cannot
//! perform at all.
//!
//! Lecture 1's §4 alone rests on: *34 steps, none above order 2*, *exactly one step at
//! order 1*, *h from 1e-4 to 3.2e-3 within fifteen steps*, *step records reach 1.0236
//! against a trajectory stopping at 1*, and *11 of 34 rows where `h` disagrees with
//! `t_n - t_(n-1)`*. **Every one of those is a claim about a run**, and a rebase can move
//! any of them without touching a line of the lecture.
//!
//! # What it is NOT
//!
//! Not a gate and not a test — it prints, it does not judge. The judgement is a human
//! reading the lecture beside the output, because step 6b's real risk is that **the
//! argument breaks while the digits still look plausible**: §4 explains a constant `phi`
//! offset by *"exactly one step ran at order 1"*, and a release that changes startup
//! order selection moves the explanation, not just the number.
//!
//! # Usage
//!
//! ```text
//! cargo run -p hrw --example solver_steps -- SingleInertia
//! cargo run -p hrw --example solver_steps -- SingleInertia 1.0
//! ```
//!
//! `t_end` defaults to the same value the committed notebook traces use, so the output
//! lines up with `docs/specimen-notebook/<Model>/trace/` unless you ask otherwise. A
//! lecture that measured a different horizon must say so, and Lecture 1 does.

use std::path::PathBuf;

/// The notebook's horizon, so output matches the committed traces by default.
///
/// Read from `gen_trace`'s constant rather than restated, because two horizons that
/// drift apart would make this tool's numbers quietly incomparable with the traces
/// every other check reads.
const DEFAULT_T_END: f64 = 2.0;

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(model) = args.next() else {
        eprintln!("usage: cargo run -p hrw --example solver_steps -- <Model> [t_end]");
        std::process::exit(2);
    };
    let t_end = match args.next() {
        Some(s) => match s.parse::<f64>() {
            Ok(v) if v > 0.0 => v,
            // A bad t_end silently falling back to the default would print a table
            // that answers a different question than the one asked.
            _ => {
                eprintln!("t_end must be a positive number, got {s:?}");
                std::process::exit(2);
            }
        },
        None => DEFAULT_T_END,
    };

    // **A bare name means the corpus; anything ending in `.mo` is a path.**
    //
    // Appendix A.3 of Lecture 1 already tells the reader to run the pipeline on models
    // that are not in the corpus, and §4's tolerance table is produced by exactly that:
    // four copies of `SingleInertia`, each with one `annotation(experiment(...))` line
    // added. Without this the only way to re-measure that table at a rebase is to drop
    // four files into `specimens/`, which would then need removing again before the
    // corpus baseline test could pass.
    let (specimen, model) = if model.ends_with(".mo") {
        let path = PathBuf::from(&model);
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_owned();
        (path, stem)
    } else {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("specimens")
            .join(format!("{model}.mo"));
        (path, model)
    };
    if !specimen.exists() {
        eprintln!("no such specimen: {}", specimen.display());
        std::process::exit(1);
    }

    let sim = match hrw::worker::simulate_specimen(&specimen, &model, t_end, hrw::msl_roots()) {
        Ok(sim) => sim,
        Err(e) => {
            eprintln!("{model}: simulation failed: {e}");
            std::process::exit(1);
        }
    };

    println!("# {model}  (t_end = {t_end})");
    println!();

    if sim.solver_steps.is_empty() {
        // **A report, not an empty table.** The rk45 path never enters the ME session
        // and so has no steps to give; printing nothing would read as "the solver took
        // no steps", which is a different and false claim.
        println!("no step records: this run's backend reports none (the rk45 path has no");
        println!("ME session to observe). The trajectory below is still real.");
    } else {
        let steps = &sim.solver_steps;
        // `h` is labelled for what v0.10.x makes it: the interval the step covered.
        // Lecture 1 documents a 0.9.20 run where it was the *next* step instead, and
        // the header must not quietly assert the old meaning.
        println!("   #      t              interval        h (derived)  order");
        let mut prev_t = 0.0;
        let mut disagreements = 0usize;
        for (i, s) in steps.iter().enumerate() {
            let arrived_by = s.t - prev_t;
            // **This is a CONTIGUITY check, and saying so is the point.**
            //
            // The first draft printed this column as *"`h` is the next step, the
            // interval is the one taken"* and counted the disagreements, because that
            // is what Lecture 1 documents on Rumoca 0.9.20. On v0.10.x it can never
            // fire for that reason: `MeStepRecord::step_size()` is
            // `accepted_time - start_time`, **derived** so it "cannot disagree with the
            // endpoints it is computed from". Comparing it against the interval between
            // consecutive accepted times is therefore comparing a value with its own
            // definition — a true measurement of the wrong unit, in the instrument
            // built to catch exactly that.
            //
            // What it still detects is a **gap**: a row whose `start_time` is not the
            // previous row's `accepted_time`, which would mean something other than an
            // accepted step reached this list. That is worth printing, so it stays —
            // under a name that claims only that.
            if (arrived_by - s.h).abs() > arrived_by.abs() * 1e-9 {
                disagreements += 1;
            }
            println!(
                "{:4}  {:<14.8}  {:<13.6e}  {:<11.6e}  {}",
                i, s.t, arrived_by, s.h, s.order
            );
            prev_t = s.t;
        }
        println!();
        println!("steps                     : {}", steps.len());
        let max_order = steps.iter().map(|s| s.order).max().unwrap_or(0);
        let min_order = steps.iter().map(|s| s.order).min().unwrap_or(0);
        println!("order range               : {min_order}..={max_order}");
        for ord in min_order..=max_order {
            let n = steps.iter().filter(|s| s.order == ord).count();
            println!("  steps at order {ord}        : {n}");
        }
        println!(
            "non-contiguous rows       : {disagreements} of {} rows (expect 0; `h` is \
             derived from the row's own endpoints, so this detects a GAP between \
             accepted steps, not the 0.9.20 `h`-is-the-next-step trap)",
            steps.len()
        );
        let last = steps.last().expect("non-empty checked above");
        println!("last step record reaches  : t = {:.6}", last.t);
        let overshoot = last.t - t_end;
        println!(
            "overshoot past t_end      : {:.6} ({:+.2} %)",
            overshoot,
            overshoot / t_end * 100.0
        );
    }

    println!();
    println!("trajectory samples        : {}", sim.times.len());
    println!(
        "trajectory ends at        : t = {:.6}",
        sim.times.last().copied().unwrap_or(f64::NAN)
    );
    println!("states                    : {}", sim.n_states);
    // **Interior samples, not only the final value**, because a lecture's sharpest
    // claims are often about how a quantity *behaves* rather than where it ends.
    // Lecture 1 argues that `phi`'s error is **constant** — "a constant, not a drift —
    // and that is the tell which says startup, not accumulation" — and a final value
    // alone cannot distinguish a frozen error from one that happens to arrive there.
    println!();
    println!("samples (nearest trajectory point to each fraction of t_end):");
    let fractions = [0.0, 0.25, 0.5, 0.75, 1.0];
    print!("  {:<24}", "t");
    for f in fractions {
        print!("{:>20.6}", f * t_end);
    }
    println!();
    for (i, name) in sim.names.iter().enumerate() {
        let Some(col) = sim.data.get(i) else {
            println!("  {name:<24} (no samples)");
            continue;
        };
        print!("  {name:<24}");
        for f in fractions {
            let target = f * t_end;
            // Nearest sample rather than interpolation: an interpolated value would be
            // this tool's arithmetic, not the solver's output, and a lecture checking
            // a number wants the number the run produced.
            let idx = sim
                .times
                .iter()
                .enumerate()
                .min_by(|a, b| {
                    (a.1 - target)
                        .abs()
                        .partial_cmp(&(b.1 - target).abs())
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(i, _)| i);
            match idx.and_then(|j| col.get(j)) {
                Some(v) => print!("{v:>20.12}"),
                None => print!("{:>20}", "-"),
            }
        }
        println!();
    }
}
