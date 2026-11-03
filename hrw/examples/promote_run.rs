//! **Promote a finished run's output into the repository as a deliverable.**
//!
//! The Rust replacement for `scripts/promote-run.ps1`
//! (`docs/verification-plan.md` item 3). All the logic that can be wrong lives
//! in `hrw::promote` and is tested; this is the file handling around it.
//!
//! ```text
//! cargo run -p hrw --example promote_run -- --run-dir C:/Users/dougd/rumoca-runs/<run>
//! cargo run -p hrw --example promote_run -- --report <csv> --profile <csv> [--force]
//! ```
//!
//! # Why the finished CSVs do not stay where they were written
//!
//! They cost **hours**, and they are exactly the zero-adoption-cost artifacts
//! `docs/upstream-strategy.md` wants to hand to maintainers. A working directory
//! is the wrong home for both reasons. This **copies, never moves**, into
//! `docs/reports/` under names that cannot be confused with the small
//! pre-commit test's output, and writes the provenance sidecar beside them.

use std::path::{Path, PathBuf};

use hrw::promote::{Verdict, guard, not_checked_sentence, parse_profile, verdict_tally};

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let force = args.iter().any(|a| a == "--force");

    let (report, profile) = match arg(&args, "--run-dir") {
        Some(dir) => {
            let d = PathBuf::from(dir);
            (d.join("fid-full.csv"), Some(d.join("fid-full-memory.csv")))
        }
        None => match arg(&args, "--report") {
            Some(r) => (PathBuf::from(r), arg(&args, "--profile").map(PathBuf::from)),
            None => die("give --run-dir, or --report and optionally --profile"),
        },
    };

    if !report.exists() {
        die(&format!("no such report: {}", report.display()));
    }
    let report_text = std::fs::read_to_string(&report)
        .unwrap_or_else(|e| die(&format!("cannot read {}: {e}", report.display())));
    let parsed = hrw::report::parse(&report_text);
    let incoming = parsed.rows.len();

    // `docs/reports/`, beside the reports.md that explains how the three
    // compose — not loose among the prose.
    let docs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/reports");
    let dest_report = docs.join("msl-fidelity-report.csv");
    let dest_meta = docs.join("msl-fidelity-report.meta.json");

    let existing = std::fs::read_to_string(&dest_report)
        .ok()
        .map(|t| hrw::report::parse(&t).rows.len());

    match guard(incoming, existing, force) {
        Verdict::Proceed => {}
        Verdict::TooFew { rows } => die(&format!(
            "only {rows} rows — that looks like a partial or specimen run. \
             Use --force if you mean it."
        )),
        Verdict::Shrinks { existing, incoming } => die(&format!(
            "refusing to replace {existing} rows with {incoming}. \
             Use --force if that is intended."
        )),
    }

    std::fs::create_dir_all(&docs).ok();
    copy(&report, &dest_report);

    let profile_rows = profile
        .as_ref()
        .filter(|p| p.exists())
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|t| parse_profile(&t))
        .unwrap_or_default();

    let profile_note = if profile_rows.is_empty() {
        "null".to_owned()
    } else {
        copy(
            profile.as_ref().unwrap(),
            &docs.join("msl-fidelity-profile.csv"),
        );
        "\"msl-fidelity-profile.csv\"".to_owned()
    };

    let outcomes = json_map(parsed.outcome_tally().into_iter());
    let verdicts = json_map(verdict_tally(&profile_rows).into_iter());
    let unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let meta = format!(
        "{{\n  \"generated_unix\": {unix},\n  \"generated_utc\": \"{}\",\n  \
         \"models\": {incoming},\n  \
         \"outcomes\": {outcomes},\n  \"profile\": {profile_note},\n  \
         \"run_verdicts\": {verdicts},\n  \"source_report\": \"{}\",\n  \
         \"not_checked\": \"{}\",\n  \
         \"note\": \"F1-F9 over the MSL corpus. Establishes that HRW agrees with Rumoca, \
         NOT that Rumoca is correct, and does not test the rendered UI.\"\n}}\n",
        utc_now(),
        report.display().to_string().replace('\\', "/"),
        not_checked_sentence(&profile_rows).replace('"', "'"),
    );
    std::fs::write(&dest_meta, meta).unwrap_or_else(|e| die(&format!("cannot write sidecar: {e}")));

    println!("promoted {incoming} rows");
    println!("  -> docs/reports/msl-fidelity-report.csv");
    if profile_note != "null" {
        println!("  -> docs/reports/msl-fidelity-profile.csv");
    }
    println!("  -> docs/reports/msl-fidelity-report.meta.json");
    println!("\nnow commit them:  git add hrw/docs/reports/msl-fidelity-*  && git commit");
}

fn copy(from: &Path, to: &Path) {
    std::fs::copy(from, to).unwrap_or_else(|e| {
        die(&format!(
            "cannot copy {} -> {}: {e}",
            from.display(),
            to.display()
        ))
    });
}

fn json_map(pairs: impl Iterator<Item = (String, usize)>) -> String {
    let body: Vec<String> = pairs.map(|(k, v)| format!("\"{k}\": {v}")).collect();
    format!("{{{}}}", body.join(", "))
}

/// Seconds since the epoch rendered as a UTC timestamp, without a date crate.
fn utc_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = secs / 86_400;
    let (h, mi, s) = ((secs % 86_400) / 3600, (secs % 3600) / 60, secs % 60);
    // Civil-from-days, the standard algorithm — no dependency for one timestamp.
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

fn die(msg: &str) -> ! {
    eprintln!("{msg}");
    std::process::exit(2);
}
