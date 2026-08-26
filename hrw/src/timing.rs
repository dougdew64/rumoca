//! Turning a set of runs into a number that may be quoted.
//!
//! # Why this exists
//!
//! On 2026-08-26 five separate timing claims died in one day, all the same shape: **a
//! single measurement, treated as a fact, reasoned forward from.** A 442.7 s gate step
//! that was an outlier. A 68 s figure measured in isolation and quoted as a marginal
//! cost. A 256 s notebook cost obtained by subtracting two gate totals taken on
//! different diffs. A variance claim citing a run whose real cause was a sleeping
//! machine. A 299.49 s suite run that spawned an entire investigation into a gap that
//! did not exist.
//!
//! **A rule against this already existed and was ignored.** `CLAUDE.md` records this
//! suite's variance as 0.7 % — measured that same morning, by the same session, from
//! three back-to-back runs — and hours later a single sample 20 % off that baseline was
//! accepted without a re-run. So the fix cannot be another sentence.
//!
//! **The mechanism is to make the honest number the easy one to get.** `examples/measure`
//! runs a command repeatedly and hands back a string that carries its own provenance:
//! *"245 s (median of 3, spread 0.7 %)"*. Quoting that is less work than quoting a bare
//! number, which is the only property that reliably changes behaviour.
//!
//! # The distinction worth keeping
//!
//! The one figure that survived 2026-08-26 was **1.95 s**, and it survived because it
//! came from a counter wrapped around the work itself. Every figure that died came from
//! subtracting one total from another. **Instrument the thing; do not subtract its
//! surroundings** — and when subtraction is unavoidable, interleave the two conditions
//! so drift cannot masquerade as a difference. That is what `--versus` is for.

/// The runs behind a quotable timing.
#[derive(Debug, Clone, PartialEq)]
pub struct Samples {
    /// Seconds, one per run, in the order they were taken.
    pub seconds: Vec<f64>,
}

impl Samples {
    /// Middle value; the mean of the middle two when the count is even.
    ///
    /// **Median rather than mean, because the failure being guarded is an outlier.**
    /// One sleeping machine or one contended run moves a mean and barely moves a median.
    pub fn median(&self) -> f64 {
        let mut v = self.seconds.clone();
        v.sort_by(|a, b| a.partial_cmp(b).expect("timings are never NaN"));
        let n = v.len();
        if n == 0 {
            return 0.0;
        }
        if n % 2 == 1 {
            v[n / 2]
        } else {
            (v[n / 2 - 1] + v[n / 2]) / 2.0
        }
    }

    /// `(max - min)` as a percentage of the median.
    pub fn spread_pct(&self) -> f64 {
        let median = self.median();
        if self.seconds.is_empty() || median == 0.0 {
            return 0.0;
        }
        let max = self.seconds.iter().cloned().fold(f64::MIN, f64::max);
        let min = self.seconds.iter().cloned().fold(f64::MAX, f64::min);
        (max - min) / median * 100.0
    }

    /// Is this stable enough that the difference between two such numbers means
    /// anything?
    ///
    /// **10 % is deliberately loose.** The point is not to certify precision, it is to
    /// catch the case where a number is quoted as if it were repeatable and is not.
    /// This suite measures at 0.7 %; a reading above 10 % is a different kind of animal.
    pub fn is_stable(&self) -> bool {
        self.seconds.len() >= 3 && self.spread_pct() <= 10.0
    }

    /// The string to paste into a document, carrying its own provenance.
    ///
    /// **The provenance is not decoration.** A bare "245 s" cannot be audited later; a
    /// reader cannot tell a median of five from one lucky run, and neither can the
    /// author six hours on. Every timing that misled on 2026-08-26 was quoted bare.
    pub fn provenance(&self) -> String {
        match self.seconds.len() {
            0 => "no runs".to_owned(),
            1 => format!(
                "{:.1} s (ONE RUN \u{2014} not quotable; re-run with --runs 3)",
                self.seconds[0]
            ),
            n => format!(
                "{:.1} s (median of {n}, spread {:.1} %)",
                self.median(),
                self.spread_pct()
            ),
        }
    }
}

/// Pull the test harness's own reported duration out of `cargo test` output.
///
/// **Preferred over wall time when present**, because it excludes compilation — which
/// is the confound that made the gate's 442.7 s test step unreadable. Sums every
/// `finished in Xs`, so a run covering several targets is counted whole.
pub fn harness_seconds(output: &str) -> Option<f64> {
    let mut total = 0.0;
    let mut found = false;
    for line in output.lines() {
        let Some(rest) = line.split("finished in ").nth(1) else {
            continue;
        };
        let Some(num) = rest.split('s').next() else {
            continue;
        };
        if let Ok(v) = num.trim().parse::<f64>() {
            total += v;
            found = true;
        }
    }
    found.then_some(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[f64]) -> Samples {
        Samples {
            seconds: v.to_vec(),
        }
    }

    #[test]
    fn a_median_ignores_the_outlier_a_mean_would_follow() {
        // The 2026-08-26 suite readings, plus the anomalous one that started a
        // day-long investigation into a gap that did not exist.
        let with_outlier = s(&[244.98, 219.51, 246.72, 299.49]);
        assert!(
            (with_outlier.median() - 245.85).abs() < 0.01,
            "median {} should sit among the three agreeing runs",
            with_outlier.median(),
        );
        let mean: f64 = with_outlier.seconds.iter().sum::<f64>() / 4.0;
        assert!(
            mean > 252.0,
            "the mean is dragged to {mean:.1}, which is the behaviour being avoided",
        );
    }

    #[test]
    fn one_run_is_reported_as_not_quotable() {
        let single = s(&[442.7]);
        assert!(
            single.provenance().contains("ONE RUN"),
            "a single sample must announce itself: {}",
            single.provenance(),
        );
        assert!(!single.is_stable(), "one run can never be stable");
    }

    #[test]
    fn spread_separates_a_repeatable_number_from_a_lucky_one() {
        let steady = s(&[244.98, 219.51, 246.72]);
        assert!(
            steady.spread_pct() < 12.0,
            "spread {:.1}%",
            steady.spread_pct()
        );

        // A sleeping machine, which is what the 10,780 s gate run turned out to be.
        let slept = s(&[240.0, 287.0, 10780.0]);
        assert!(
            !slept.is_stable(),
            "a run that slept must not be quotable alongside two that did not",
        );
        assert!(
            slept.provenance().contains("median of 3"),
            "the provenance still states what it is: {}",
            slept.provenance(),
        );
    }

    #[test]
    fn the_harness_duration_is_preferred_to_wall_time() {
        let out = "running 887 tests\n\
                   test result: ok. 887 passed; finished in 244.98s\n\
                   \n\
                   running 2 tests\n\
                   test result: ok. 2 passed; finished in 6.27s\n";
        assert_eq!(
            harness_seconds(out),
            Some(251.25),
            "every target's time is summed, so a multi-target run is counted whole",
        );
        assert_eq!(
            harness_seconds("no timings here"),
            None,
            "absence must be None so the caller falls back to wall time knowingly",
        );
    }
}
