//! **Promoting a finished run into the repository** — the logic half of what
//! was `scripts/promote-run.ps1`.
//!
//! Split from the script on 2026-08-01 (`docs/verification-plan.md` item 3) so
//! the parts that can be wrong can be tested. The driver is
//! `examples/promote_run.rs`; everything here is pure and takes its inputs as
//! values.
//!
//! # Why this one moved to Rust and the watchdog did not
//!
//! `scripts/measure-fidelity.ps1` earns its counter-argument: it needs process
//! memory sampling (a crate), and being editable while a sweep holds the binary
//! mattered on 2026-08-01. **Neither applies here.** This runs for seconds after
//! the sweep, samples nothing, and does the one thing in the whole pipeline that
//! **writes a published claim**: the sidecar's `not_checked` sentence, which
//! travels to a maintainer with the table and is read as fact.
//!
//! The project's rule is *speed on actions, care on records* — and this is the
//! record.

use std::collections::BTreeMap;

/// One row of the memory profile: what a model cost and how it ended.
#[derive(Debug, Clone, PartialEq)]
pub struct ProfileRow {
    pub name: String,
    pub peak_ws_mb: u64,
    pub verdict: String,
}

/// Parse the profile CSV written by the watchdog: `name,peak_ws_mb,secs,verdict`.
///
/// Unparseable rows are **skipped, not guessed at** — a partial line at the tail
/// of a killed run is normal, and inventing a verdict for it would put a fiction
/// into the sidecar.
pub fn parse_profile(text: &str) -> Vec<ProfileRow> {
    text.lines()
        .skip(1)
        .filter_map(|line| {
            let f: Vec<&str> = line.split(',').collect();
            if f.len() < 4 || f[0].is_empty() {
                return None;
            }
            Some(ProfileRow {
                name: f[0].to_owned(),
                peak_ws_mb: f[1].trim().parse().ok()?,
                verdict: f[3].trim().to_owned(),
            })
        })
        .collect()
}

/// Why a promotion was refused, or that it may proceed.
#[derive(Debug, PartialEq)]
pub enum Verdict {
    Proceed,
    /// Far too few rows to be a corpus run — most likely a specimen or partial run.
    TooFew { rows: usize },
    /// Smaller than what is already committed.
    Shrinks { existing: usize, incoming: usize },
}

/// **The two guards, and the reason each exists.**
///
/// `TooFew` catches promoting a specimen-sized or half-finished run over the
/// corpus artifact. `Shrinks` catches the likelier accident: promoting a partial
/// **re-run** over a complete sweep — the retry pass on 2026-08-01 wrote to the
/// same path and would have replaced 2,610 rows with 16 had it been promoted
/// mid-flight.
///
/// Both are overridable with `--force`, because both are heuristics about intent
/// and the operator can know better.
pub fn guard(incoming: usize, existing: Option<usize>, force: bool) -> Verdict {
    if force {
        return Verdict::Proceed;
    }
    if incoming < 100 {
        return Verdict::TooFew { rows: incoming };
    }
    match existing {
        Some(e) if incoming < e => Verdict::Shrinks { existing: e, incoming },
        _ => Verdict::Proceed,
    }
}

/// The `not_checked` sentence — **the published claim this whole module exists
/// to get right.**
///
/// A table handed to a maintainer travels without its conversation, so whatever
/// it does *not* cover has to be readable from the artifact itself. Two failure
/// modes are distinguished because they mean different things:
///
/// - **memory** — the model wants more than the machine can safely provide. A
///   fact about the hardware.
/// - **time** — the model exceeded the run's budget. A fact about the run
///   configuration.
///
/// **Neither is a finding about HRW or Rumoca**, and the sentence says so in
/// those words, because a reader who assumes otherwise has been misled by our
/// own artifact.
///
/// Returns an empty string when nothing was skipped — silence is correct there,
/// and a "0 models were not checked" sentence would be noise.
pub fn not_checked_sentence(profile: &[ProfileRow]) -> String {
    let ceiling: Vec<&ProfileRow> =
        profile.iter().filter(|r| r.verdict == "aborted:proc-ceiling").collect();
    let timeout: Vec<&ProfileRow> =
        profile.iter().filter(|r| r.verdict == "aborted:timeout").collect();

    let mut parts: Vec<String> = Vec::new();
    if !ceiling.is_empty() {
        let worst = ceiling.iter().map(|r| r.peak_ws_mb).max().unwrap_or(0);
        parts.push(format!(
            "{} model(s) exceeded the memory this machine can safely provide \
             (observed above {:.1} GB and still growing when stopped) and were NOT checked",
            ceiling.len(),
            worst as f64 / 1024.0,
        ));
    }
    if !timeout.is_empty() {
        parts.push(format!(
            "{} model(s) exceeded the time limit and were NOT checked",
            timeout.len(),
        ));
    }
    if parts.is_empty() {
        return String::new();
    }
    format!(
        "{}. These are limits of the machine and the run configuration, not findings \
         about HRW or Rumoca.",
        parts.join("; "),
    )
}

/// Count each verdict, for the sidecar's `run_verdicts`.
pub fn verdict_tally(profile: &[ProfileRow]) -> BTreeMap<String, usize> {
    let mut out = BTreeMap::new();
    for r in profile {
        *out.entry(r.verdict.clone()).or_insert(0) += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_guards_refuse_what_they_are_for_and_allow_what_they_are_not() {
        assert_eq!(guard(2614, Some(2610), false), Verdict::Proceed, "a bigger run promotes");
        assert_eq!(guard(2614, None, false), Verdict::Proceed, "so does a first run");
        assert_eq!(
            guard(16, Some(2610), false),
            Verdict::TooFew { rows: 16 },
            "a partial retry pass must not replace the corpus",
        );
        assert_eq!(
            guard(2000, Some(2610), false),
            Verdict::Shrinks { existing: 2610, incoming: 2000 },
            "nor a smaller full-looking run",
        );
        // --force is the operator saying they know better, and must beat both.
        assert_eq!(guard(16, Some(2610), true), Verdict::Proceed);
    }

    /// **The published sentence, checked in both directions.**
    ///
    /// Silence when nothing was skipped is as important as the sentence when
    /// something was: a sidecar that always claims a bound teaches a reader to
    /// ignore it.
    #[test]
    fn the_not_checked_sentence_reports_both_causes_and_stays_silent_otherwise() {
        let clean =
            vec![ProfileRow { name: "a".into(), peak_ws_mb: 100, verdict: "ok".into() }];
        assert_eq!(not_checked_sentence(&clean), "", "nothing skipped, nothing claimed");

        let mixed = vec![
            ProfileRow { name: "a".into(), peak_ws_mb: 100, verdict: "ok".into() },
            ProfileRow { name: "b".into(), peak_ws_mb: 11671, verdict: "aborted:proc-ceiling".into() },
            ProfileRow { name: "c".into(), peak_ws_mb: 10614, verdict: "aborted:proc-ceiling".into() },
            ProfileRow { name: "d".into(), peak_ws_mb: 2445, verdict: "aborted:timeout".into() },
        ];
        let s = not_checked_sentence(&mixed);
        assert!(s.contains("2 model(s) exceeded the memory"), "counts the memory aborts: {s}");
        assert!(s.contains("11.4 GB"), "and reports the WORST observed peak, not the last: {s}");
        assert!(s.contains("1 model(s) exceeded the time limit"), "counts timeouts separately: {s}");
        assert!(
            s.contains("not findings about HRW or Rumoca"),
            "and disclaims the reading a maintainer would otherwise make: {s}",
        );
    }

    /// A truncated tail is skipped rather than guessed at.
    #[test]
    fn a_partial_profile_row_is_skipped_not_invented() {
        let text = "name,peak_ws_mb,secs,verdict\n\
                    good,100,1.0,ok\n\
                    truncated,200\n\
                    ,,,\n\
                    also_good,300,2.0,aborted:timeout\n";
        let rows = parse_profile(text);
        assert_eq!(rows.len(), 2, "two complete rows: {rows:?}");
        assert_eq!(rows[0].name, "good");
        assert_eq!(rows[1].verdict, "aborted:timeout");
        assert_eq!(verdict_tally(&rows).get("aborted:timeout"), Some(&1));
    }
}
