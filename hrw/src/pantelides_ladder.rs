//! The acceptance ladder for general Pantelides — `docs/ideas.md` **#83**.
//!
//! ## What this is, and why it is a ladder rather than one test
//!
//! `CartesianPendulum` is the canonical index-3 DAE: a point mass on a rigid rod
//! in Cartesian coordinates, whose constraint `x² + y² = L²` is **nonlinear**, so
//! no substitution removes it and differentiation is the only route to index 1.
//! Rumoca's index reduction is pattern-based rather than general Pantelides, so
//! it leaves the model at four states and structurally singular. Wolfram System
//! Modeler reduces it to two and simulates it; that trajectory is committed in
//! [`docs/specimen-notebook/CartesianPendulum/oracle/`], captured 2026-08-23
//! while a machine with System Modeler was available.
//!
//! **Doug, 2026-08-23, on why five tests and not one:** a textbook's
//! end-of-chapter project is *scoped and graded* — each step completable in a
//! sitting, each giving its own sense of progress. A single binary test says
//! when you are **done**; it never says whether you are **on track**. So the
//! acceptance criterion is a ladder, and each rung can turn green on its own:
//!
//! | rung | claim | today |
//! |---|---|---|
//! | 1 | the deficiency is detected, at the constraint | **green** |
//! | 2 | a constraint equation is differentiated | red |
//! | 3 | the reduction reaches a non-singular fixed point | red |
//! | 4 | the pendulum reduces to two states | red |
//! | 5 | its trajectory matches the independent oracle | red |
//!
//! **Rung 1 is green today and that is the point** — it pins the *input*
//! Pantelides consumes. Rumoca already identifies exactly the right equation and
//! the right unknown; what is missing is the differentiation that follows. A
//! climber who knows rung 1 holds knows the gap is narrower than "it does not
//! work".
//!
//! ## Why the red rungs are `#[ignore]`d rather than absent
//!
//! They are written against **today's API**, so they compile now and the
//! compiler keeps them honest for however many months #83 waits. The day
//! Pantelides lands, this is a red test turning green rather than a project
//! needing a plan. Each `ignore` reason names #83; where a rung needs a *value*
//! that no entry point reports yet, the reason says so rather than inventing
//! one.
//!
//! ## The tolerance trap
//!
//! The oracle's README states it, and it is the one way this file could do
//! damage: **do not demand agreement tight enough to encode System Modeler's
//! particular numerical choices as truth.** Its own constraint residual is
//! 1.23 × 10⁻⁴, and a different-but-correct reduction drifts differently. A test
//! pinning that trajectory to machine precision would fail a *correct*
//! implementation, which is worse than no test.
//!
//! So the strongest assertions come first, because they cannot be wrong — the
//! state count is 2, `lambda` peaks at the bottom at *m*(*g* + *v*²/*L*) = 29.43
//! (hand-derivable, independent of both tools), and the constraint holds. The
//! 101 samples are the differential half, at the stated relative tolerance of
//! [`TRAJECTORY_REL_TOL`], per charter §4.3.
//!
//! ## What is verified tonight and what is not
//!
//! Rung 1 runs. Rungs 2–5 do not, so **their plumbing is unexecuted code** — and
//! unexecuted code that will not be read again for months is exactly how a wrong
//! comparison ships. The arithmetic is therefore factored out of them into
//! [`max_relative_error`], which is pure, and is tested today against synthetic
//! data by [`tests::the_error_metric_measures_what_it_claims`]. What stays
//! unexecuted is the plumbing around it, not the judgement inside it.

/// Relative tolerance for the trajectory comparison in rung 5.
///
/// Stated here rather than inline because charter §4.3 requires the tolerance to
/// be *stated*, and because it is the number most likely to need revisiting when
/// rung 5 first runs for real. It is deliberately loose: the oracle's own
/// constraint residual is 1.23e-4, so agreement much tighter than this would be
/// measuring System Modeler's solver rather than Rumoca's correctness.
#[cfg(test)]
const TRAJECTORY_REL_TOL: f64 = 1e-3;

/// Linear interpolation of `(ts, vs)` at `t`, clamped at both ends.
///
/// Separate from the comparison so the two can be reasoned about apart, and so
/// the clamping — which is what makes an endpoint sample safe — is visible.
#[cfg(test)]
fn interpolate_at(ts: &[f64], vs: &[f64], t: f64) -> Option<f64> {
    if ts.len() != vs.len() || ts.is_empty() {
        return None;
    }
    if t <= ts[0] {
        return Some(vs[0]);
    }
    if t >= ts[ts.len() - 1] {
        return Some(vs[vs.len() - 1]);
    }
    let i = ts.windows(2).position(|w| t >= w[0] && t <= w[1])?;
    let (t0, t1) = (ts[i], ts[i + 1]);
    let span = t1 - t0;
    if span == 0.0 {
        return Some(vs[i]);
    }
    let f = (t - t0) / span;
    Some(vs[i] + f * (vs[i + 1] - vs[i]))
}

/// Largest relative error between an oracle series and a simulated one, the
/// simulated series interpolated onto the oracle's sample times.
///
/// **Relative to the series' own scale, not pointwise**, and that choice is the
/// whole reason this function is tested rather than inlined. A pendulum's `x`
/// passes through zero twice a swing, and a pointwise relative error divides by
/// that: two trajectories agreeing to 1e-9 in absolute terms would report an
/// unbounded error at the zero crossing and fail a correct implementation. The
/// denominator is therefore the oracle's peak magnitude over the whole series,
/// which is the scale a reader means by "1 % agreement".
///
/// Returns `None` when the series cannot be compared at all — mismatched lengths
/// or an empty simulation — rather than a zero error, because "nothing to
/// compare" must never read as "agreed perfectly".
#[cfg(test)]
fn max_relative_error(
    oracle_t: &[f64],
    oracle_v: &[f64],
    sim_t: &[f64],
    sim_v: &[f64],
) -> Option<f64> {
    if oracle_t.len() != oracle_v.len() || oracle_t.is_empty() || sim_t.is_empty() {
        return None;
    }
    let scale = oracle_v.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    // A series that is identically zero has no scale to divide by; compare it
    // absolutely instead, which is the honest reading of "how far off is it".
    let denom = if scale == 0.0 { 1.0 } else { scale };

    let mut worst = 0.0_f64;
    for (&t, &want) in oracle_t.iter().zip(oracle_v) {
        let got = interpolate_at(sim_t, sim_v, t)?;
        worst = worst.max((got - want).abs() / denom);
    }
    Some(worst)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    use crate::worker::{FromWorker, test_msl::compile_specimen_shared};

    const MODEL: &str = "CartesianPendulum";

    /// The index-reduction stage's JSON for the pendulum.
    fn reduction_stage() -> serde_json::Value {
        let FromWorker::Compiled { stages, .. } = compile_specimen_shared(MODEL) else {
            panic!("expected Compiled for {MODEL}");
        };
        stages
            .index_reduction
            .value
            .clone()
            .unwrap_or_else(|| panic!("the Index Reduction stage produced no IR for {MODEL}"))
    }

    /// The committed System Modeler reference.
    fn oracle() -> serde_json::Value {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("docs/specimen-notebook/CartesianPendulum/oracle/system-modeler-15.0.json");
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("the oracle at {} must be readable: {e}", path.display()));
        serde_json::from_str(&text).expect("the oracle must be valid JSON")
    }

    fn series(v: &serde_json::Value, name: &str) -> Vec<f64> {
        v["samples"][name]
            .as_array()
            .unwrap_or_else(|| panic!("the oracle must carry a `{name}` series"))
            .iter()
            .map(|x| x.as_f64().expect("a sample must be a number"))
            .collect()
    }

    /// **Rung 1 — the deficiency is detected, and at the right equation.**
    ///
    /// Green today, and it pins what Pantelides consumes. Rumoca reports the
    /// system structurally singular with a rank deficiency of exactly one, names
    /// the **constraint** as the unmatched equation and **lambda** as the
    /// unmatched unknown. That is precisely the input the algorithm needs: the
    /// equation to differentiate and the variable no equation determines.
    ///
    /// So the gap #83 closes is narrower than "index reduction does not work" —
    /// the diagnosis is right and the action is missing. Recorded as a rung
    /// rather than a footnote because a green rung is worth as much as a red one
    /// to someone deciding where to start.
    ///
    /// **It asserts singularity, not high-index-ness**, and the distinction is
    /// deliberate: nothing in Rumoca today concludes *"this is a high-index DAE"*
    /// — it concludes *"this system is structurally singular"*, which is the
    /// symptom. Naming this test for the stronger claim would be the shape this
    /// repository keeps catching, a name promising more than the mechanism gives.
    #[test]
    #[cfg_attr(
        not(feature = "slow-tests"),
        ignore = "compile-heavy; run with --features slow-tests"
    )]
    fn rung_1_the_deficiency_is_detected_at_the_constraint() {
        let stage = reduction_stage();
        let error = &stage["error"];

        assert_eq!(
            error["kind"].as_str(),
            Some("singular"),
            "the pendulum must still be reported structurally singular; if this now \
             passes cleanly, #83 may have landed and rungs 2-5 are waiting to be \
             un-ignored. Stage error was: {error}"
        );
        assert_eq!(
            error["rank_deficiency"].as_u64(),
            Some(1),
            "one constraint, one deficiency"
        );
        assert_eq!(
            error["unmatched_unknowns"].as_array().map(Vec::as_slice),
            Some([serde_json::json!("lambda")].as_slice()),
            "lambda is the variable no equation determines — it appears in no \
             derivative, which is what makes this index 3"
        );

        // The unmatched equation is the constraint. Its identity is checked by
        // POSITION in the flat equation list rather than by matching on its text:
        // `docs/identity-and-provenance.md` — no substring search ever decides
        // identity. `f_x[4]` is the fifth equation, and the model declares the
        // constraint fifth.
        assert_eq!(
            error["unmatched_equations"].as_array().map(Vec::as_slice),
            Some([serde_json::json!("f_x[4]")].as_slice()),
            "the fifth equation of the model is `x^2 + y^2 = L^2`, and it is the one \
             left unmatched"
        );
    }

    /// **Rung 2 — a constraint equation is differentiated.**
    ///
    /// The first act of Pantelides: on finding a structurally singular subset,
    /// differentiate its equations with respect to time and try again. The
    /// reduction stage already reports `n_differentiations` and
    /// `differentiated_rows`, so this rung needs no new entry point — only for
    /// the algorithm to do the thing and record it.
    ///
    /// Today both are zero and empty: the funnel runs every step and moves
    /// nothing.
    #[test]
    #[ignore = "general Pantelides is not implemented — docs/ideas.md #83 (rung 2 of 5: no constraint is differentiated; `n_differentiations` is 0 today)"]
    fn rung_2_a_constraint_equation_is_differentiated() {
        let stage = reduction_stage();
        let reduction = &stage["reduction"];

        let n = reduction["n_differentiations"].as_u64().unwrap_or(0);
        assert!(
            n >= 1,
            "index 3 needs the constraint differentiated twice to reach index 1, so at \
             least one differentiation must be recorded; got {n}"
        );
        let rows = reduction["differentiated_rows"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_default();
        assert!(
            !rows.is_empty(),
            "a differentiation count with no rows to show for it would be a claim \
             without evidence — the two are written by the same pass"
        );
    }

    /// **Rung 3 — the reduction reaches a non-singular fixed point.**
    ///
    /// Pantelides iterates: differentiate, re-match, and repeat until the system
    /// is no longer structurally singular. Two things have to hold — the loop
    /// **terminates** (a bug here is an infinite loop, not a wrong answer), and
    /// what it terminates at is **solvable**.
    ///
    /// Today the funnel completes and the system is still singular, which is why
    /// `funnel_completed` alone was never the interesting half. This rung asserts
    /// both, so a future implementation cannot satisfy it by giving up tidily.
    #[test]
    #[ignore = "general Pantelides is not implemented — docs/ideas.md #83 (rung 3 of 5: the funnel completes today, but the system it leaves behind is still singular)"]
    fn rung_3_the_reduction_reaches_a_non_singular_fixed_point() {
        let stage = reduction_stage();

        assert_eq!(
            stage["reduction"]["funnel_completed"].as_bool(),
            Some(true),
            "the iteration must terminate on its own rather than being stopped: \
             stopped_at = {}",
            stage["reduction"]["stopped_at"]
        );
        assert!(
            stage["error"].is_null(),
            "a fixed point that is still structurally singular is not a fixed point \
             worth reaching — the reduced system must be solvable. Error: {}",
            stage["error"]
        );
    }

    /// **Rung 4 — the pendulum reduces to two states.**
    ///
    /// The strongest assertion in the ladder, and the one no implementation
    /// choice can move: a point mass on a rigid rod has **one degree of freedom**,
    /// so a correct reduction leaves two states (a position and a velocity along
    /// the constraint manifold). System Modeler gets two, by dynamic state
    /// selection. Rumoca's four is the defect, stated as a number.
    ///
    /// **Which** two is deliberately not asserted. System Modeler selects
    /// `{vy}`/`{y}` dynamically, and a correct implementation may legitimately
    /// pick a different pair — or switch pairs during the swing, which is what
    /// dummy-derivative methods do. Pinning the identity would encode one tool's
    /// choice as truth, the trap the oracle's README names.
    #[test]
    #[ignore = "general Pantelides is not implemented — docs/ideas.md #83 (rung 4 of 5: 4 states after reduction today, oracle says 2)"]
    fn rung_4_the_pendulum_reduces_to_two_states() {
        let stage = reduction_stage();
        let after = stage["reduction"]["n_states_after"].as_u64();

        assert_eq!(
            after,
            Some(2),
            "one degree of freedom means two states after reduction; the oracle \
             observed 2 and Rumoca reports {after:?}. states_after = {}",
            stage["reduction"]["states_after"]
        );
        assert_eq!(
            oracle()["reduction_observed"]["states_after_reduction"].as_u64(),
            Some(2),
            "the oracle file itself must still say 2, or this rung is comparing \
             against something that changed underneath it"
        );
    }

    /// **Rung 5 — the trajectory matches the independent oracle.**
    ///
    /// The differential half, and the rarest thing this project can offer the
    /// Rumoca maintainers (`docs/upstream-strategy.md`): a comparison against an
    /// independent implementation rather than against ourselves.
    ///
    /// Three claims, weakest last on purpose:
    ///
    /// 1. **`lambda` peaks at *m*(*g* + *v*²/*L*) = 29.43** at the bottom of the
    ///    swing. Hand-derivable from Newton's second law in the radial direction,
    ///    so it is independent of *both* tools — the strongest number here.
    /// 2. **The constraint holds** throughout, to a stated tolerance. A reduction
    ///    that differentiates correctly but drifts off the manifold is the
    ///    classic index-reduction failure, and it is invisible in the trajectory
    ///    alone.
    /// 3. **The states agree** with the oracle to [`TRAJECTORY_REL_TOL`],
    ///    relative to each series' own scale — see [`max_relative_error`] on why
    ///    pointwise relative error is the wrong metric for a signal that crosses
    ///    zero.
    #[test]
    #[ignore = "general Pantelides is not implemented — docs/ideas.md #83 (rung 5 of 5: the model is structurally singular, so it does not simulate at all today)"]
    fn rung_5_the_trajectory_matches_the_independent_oracle() {
        let oracle = oracle();
        let t_end = oracle["simulation"]["t_end"].as_f64().unwrap_or(10.0);

        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("specimens/{MODEL}.mo"));
        let sim = crate::worker::simulate_specimen(&path, MODEL, t_end, Vec::new())
            .unwrap_or_else(|e| panic!("{MODEL} must simulate once #83 lands: {e}"));

        let at = |name: &str| -> Vec<f64> {
            let i = sim
                .names
                .iter()
                .position(|n| n == name)
                .unwrap_or_else(|| panic!("the simulation must report `{name}`"));
            sim.data[i].clone()
        };

        // 1. The analytic peak. `m(g + v²/L)` with m = L = 1 and the velocity at
        //    the bottom of a swing released from horizontal: v² = 2gL, so the
        //    tension is m(g + 2g) = 3mg = 29.43.
        let lambda = at("lambda");
        let peak = lambda.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
        let analytic = oracle["invariants"]["lambda_peak_analytic_m_g_plus_v2_over_L"]
            .as_f64()
            .expect("the oracle carries the hand-computed peak");
        assert!(
            (peak - analytic).abs() / analytic < 1e-2,
            "lambda peaked at {peak}, against the hand-computed {analytic}"
        );

        // 2. The constraint. Looser than the oracle's own 1.23e-4 residual, so a
        //    correct-but-differently-drifting reduction is not failed for it.
        let (x, y) = (at("x"), at("y"));
        let worst = x
            .iter()
            .zip(&y)
            .fold(0.0_f64, |m, (x, y)| m.max((x * x + y * y - 1.0).abs()));
        assert!(
            worst < 1e-3,
            "the mass left the rod: max |x² + y² − L²| = {worst}, oracle's own is {}",
            oracle["invariants"]["max_constraint_residual"]
        );

        // 3. The trajectories.
        let ts = series(&oracle, "t");
        for name in ["x", "y", "vx", "vy"] {
            let err = max_relative_error(&ts, &series(&oracle, name), &sim.times, &at(name))
                .unwrap_or_else(|| panic!("`{name}` could not be compared against the oracle"));
            assert!(
                err < TRAJECTORY_REL_TOL,
                "`{name}` differs from the oracle by {err} relative to its own scale, \
                 tolerance {TRAJECTORY_REL_TOL}"
            );
        }
    }

    /// **The metric rung 5 rests on, measured today rather than in some months.**
    ///
    /// Rung 5 cannot run until #83 lands, so without this its arithmetic would be
    /// unexecuted code — and a comparison that is wrong in the *lenient*
    /// direction is the dangerous kind: it would pass on a broken implementation
    /// and nobody would look again.
    ///
    /// The zero-crossing case is the one that matters. A pendulum's `x` passes
    /// through zero twice a swing; a pointwise relative error divides by that and
    /// reports an unbounded error for two trajectories that agree to 1e-9.
    #[test]
    fn the_error_metric_measures_what_it_claims() {
        let t = [0.0, 1.0, 2.0];

        // Identical series agree exactly, on the oracle's own sample times.
        assert_eq!(
            max_relative_error(&t, &[1.0, 0.0, -1.0], &t, &[1.0, 0.0, -1.0]),
            Some(0.0)
        );

        // A signal through zero: absolute agreement of 0.01 against a peak of 1.0
        // is 1 %, not the infinity a pointwise relative error would report.
        let err = max_relative_error(&t, &[1.0, 0.0, -1.0], &t, &[1.0, 0.01, -1.0])
            .expect("comparable series");
        assert!((err - 0.01).abs() < 1e-12, "got {err}");

        // Interpolation: a simulation sampled more coarsely than the oracle is
        // read between its own points rather than rejected.
        let err = max_relative_error(&t, &[0.0, 1.0, 2.0], &[0.0, 2.0], &[0.0, 2.0])
            .expect("comparable series");
        assert!(
            err < 1e-12,
            "a straight line interpolates exactly; got {err}"
        );

        // Nothing to compare must never read as perfect agreement.
        assert_eq!(max_relative_error(&t, &[1.0, 0.0, -1.0], &[], &[]), None);
        assert_eq!(max_relative_error(&[], &[], &t, &[1.0, 0.0, -1.0]), None);

        // A series that is identically zero has no scale; it is compared
        // absolutely rather than dividing by nothing.
        let err = max_relative_error(&t, &[0.0, 0.0, 0.0], &t, &[0.0, 0.5, 0.0])
            .expect("comparable series");
        assert!((err - 0.5).abs() < 1e-12, "got {err}");
    }

    /// The ladder is legible as a ladder: five rungs, each naming its number.
    ///
    /// **A guard against the ladder quietly becoming one test again.** Doug's
    /// reason for five was that a single binary test says when you are done but
    /// never whether you are on track; that property lives in the *count* and in
    /// the names being readable in a test run, neither of which the compiler
    /// holds.
    #[test]
    fn the_ladder_has_five_rungs_and_each_names_its_position() {
        let src = include_str!("pantelides_ladder.rs");
        let rungs: Vec<&str> = src
            .lines()
            .filter_map(|l| l.trim().strip_prefix("fn rung_"))
            .collect();
        assert_eq!(
            rungs.len(),
            5,
            "the ladder is five rungs by design; found {rungs:?}"
        );
        for (i, rung) in rungs.iter().enumerate() {
            let want = format!("{}_", i + 1);
            assert!(
                rung.starts_with(&want),
                "rung {} is out of order or misnumbered: {rung}",
                i + 1
            );
        }
        // Every red rung cites the idea it is waiting on, so a reader who runs
        // the suite and sees four ignores learns why from the output alone.
        //
        // **The needle is assembled**, because the first draft counted six: four
        // ignore reasons, a since-deleted constant, and *this line matching
        // itself*. Third instance in this repository of a source check finding
        // its own prose, and the second in one night.
        let cited = format!("docs/ideas.md {}83 (rung", '#');
        assert_eq!(
            src.matches(cited.as_str()).count(),
            4,
            "each red rung must cite the idea it waits on, in the reason a test run \
             prints — four rungs are red today"
        );
    }
}
