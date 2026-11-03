//! **Measure a command honestly, and hand back a number that carries its provenance.**
//!
//! ```text
//! cargo run -p hrw --example measure -- test -p hrw --lib --features slow-tests
//! cargo run -p hrw --example measure -- --runs 5 -- test -p hrw --lib
//! cargo run -p hrw --example measure -- test -p hrw --lib --versus test -p hrw --lib --skip slow_thing
//! ```
//!
//! # Why this exists rather than a rule
//!
//! On 2026-08-26 five timing claims died in one day, every one the same shape: **a
//! single measurement, treated as a fact, reasoned forward from.** A rule against it
//! already existed — `CLAUDE.md` records this suite's variance as 0.7 %, measured that
//! morning by the session that then ignored it. So this repository did what it does
//! with anything got wrong repeatedly and built the tool instead, exactly as
//! `examples/gate` replaced an ordering that had been got wrong ten times.
//!
//! **It works by making the honest number cheaper to obtain than the dishonest one.**
//! Running this is less effort than timing something by hand, and what it prints is
//! ready to paste — so the quotable string and the correct string are the same string.
//!
//! # `--versus`, and the error it exists for
//!
//! The costliest mistake of that day was not the single sample; it was **subtracting
//! two totals taken under different conditions** — a notebook step "measured" by
//! differencing two gate runs on different diffs, and a 79.3 s marginal cost obtained
//! by differencing a normal suite run against an anomalous one.
//!
//! `--versus` runs the two commands **interleaved** — A B A B A B — in one session, so
//! drift, thermal state and background load hit both arms equally. A difference that
//! survives interleaving is a difference; one that does not was never there.
//!
//! **What it still cannot do is tell you the two arms answer the same question.** The
//! seven log tests genuinely cost 13.7 s alone and appeared to cost far more in the
//! suite, and no amount of repetition fixes a comparison between a subset and a whole.
//! `--versus` controls for *when*, never for *what*.

use hrw::timing::{Samples, harness_seconds};
use std::process::Command;
use std::time::Instant;

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let (runs, rest) = parse_runs(&argv);

    let (a, b) = match rest.iter().position(|s| s == "--versus") {
        Some(i) => (rest[..i].to_vec(), Some(rest[i + 1..].to_vec())),
        None => (rest.clone(), None),
    };

    if a.is_empty() {
        eprintln!(
            "usage: cargo run -p hrw --example measure -- [--runs N] <cargo args> \
             [--versus <cargo args>]"
        );
        std::process::exit(2);
    }

    match b {
        None => {
            let s = measure(&a, runs, "");
            report("", &a, &s);
        }
        Some(b) => {
            // Interleaved, so drift cannot masquerade as a difference.
            let (mut sa, mut sb) = (Vec::new(), Vec::new());
            for i in 0..runs {
                sa.push(one(&a, &format!("A run {}", i + 1)));
                sb.push(one(&b, &format!("B run {}", i + 1)));
            }
            let (sa, sb) = (Samples { seconds: sa }, Samples { seconds: sb });
            report("A: ", &a, &sa);
            report("B: ", &b, &sb);
            compare(&sa, &sb);
        }
    }
}

/// `--runs N` anywhere before the command; default 3, never fewer than 1.
fn parse_runs(argv: &[String]) -> (usize, Vec<String>) {
    let mut runs = 3usize;
    let mut rest = Vec::new();
    let mut seen_separator = false;
    let mut i = 0;
    while i < argv.len() {
        match argv[i].as_str() {
            "--runs" if i + 1 < argv.len() => {
                runs = argv[i + 1].parse().unwrap_or(3);
                i += 2;
            }
            // **Only the FIRST bare `--` is ours.** It separates our flags from the
            // cargo args; every later one belongs to cargo, since `cargo test … --
            // --test-threads=1` needs its own. Stripping them all turned this tool's
            // very first trial run into `cargo test --test-threads=1`, which cargo
            // rejects — caught only because a failed command says so loudly.
            "--" if !seen_separator => {
                seen_separator = true;
                i += 1;
            }
            _ => {
                rest.push(argv[i].clone());
                i += 1;
            }
        }
    }
    (runs.max(1), rest)
}

fn measure(args: &[String], runs: usize, label: &str) -> Samples {
    let seconds = (0..runs)
        .map(|i| one(args, &format!("{label}run {}", i + 1)))
        .collect();
    Samples { seconds }
}

/// One run. Prefers the harness's own duration over wall time, because wall time
/// includes compilation — the confound that made a gate's 442.7 s test step unreadable.
fn one(args: &[String], label: &str) -> f64 {
    eprint!("  {label} … ");
    let started = Instant::now();
    let out = Command::new("cargo")
        .args(args)
        .output()
        .expect("cargo is on PATH");
    let wall = started.elapsed().as_secs_f64();
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let (secs, source) = match harness_seconds(&text) {
        Some(h) => (h, "harness"),
        None => (wall, "wall"),
    };
    if !out.status.success() {
        // **Refused, not reported.** A timing from a command that failed is worse than
        // no timing: it looks quotable and measures nothing. This tool's own first run
        // produced exactly that — 0.1 s from a cargo invocation that never ran a test —
        // and printed it beside the failure, which is how the argument bug above was
        // found. Exiting here means the number can never be pasted anywhere.
        eprintln!("{secs:.1}s ({source}) — COMMAND FAILED");
        eprintln!();
        for line in text.lines().rev().take(6).collect::<Vec<_>>().iter().rev() {
            eprintln!("  {line}");
        }
        eprintln!("\nmeasure: refusing to report a timing for a command that failed.");
        std::process::exit(1);
    }
    eprintln!("{secs:.1}s ({source})");
    secs
}

fn report(prefix: &str, args: &[String], s: &Samples) {
    println!("\n{prefix}cargo {}", args.join(" "));
    println!("  {}", s.provenance());
    if !s.is_stable() {
        println!(
            "  ^ NOT STABLE. Do not quote this as a fact, and do not subtract it from \
             anything. Re-run with more runs, or find what varied."
        );
    }
}

/// Whether a difference between the two arms survives their own spread.
fn compare(a: &Samples, b: &Samples) {
    let diff = a.median() - b.median();
    let noise = a.spread_pct().max(b.spread_pct()) / 100.0 * a.median().max(b.median());
    println!("\n  A - B = {diff:+.1} s");
    if diff.abs() <= noise {
        println!(
            "  ^ WITHIN NOISE ({noise:.1} s). The two arms are not distinguishable by \
             this measurement; reporting the difference as real is the 2026-08-26 error."
        );
    } else {
        println!("  ^ larger than the {noise:.1} s spread, so the difference is real.");
    }
}
