//! Hover labels for the simulation plots — **the numbers, at a precision that survives**.
//!
//! # Why this module exists
//!
//! Doug, 2026-09-25, while trying to verify lecture 1's claims about Rumoca's solver:
//!
//! > *"HRW's simulation plots do not permit me to verify the numbers which you claim in the
//! > lecture. For example, there is no way for me to verify the size of the time step during the
//! > simulation as I cannot hover over the time step plot and receive a tool tip which reports the
//! > time step value at the hover point."*
//!
//! **The tooltip was there; it was rounding every interesting value to zero.** `egui_plot`'s
//! [`default_label_formatter`] formats with `{:.3}` — three decimal places — so hovering the
//! step-size line on `SingleInertia` reports `y = 0.000` for a step of `1e-4`, and every step it
//! takes is smaller than the rounding. The same default flattens a trajectory error of `9.8e-9`
//! into a value indistinguishable from the exact answer.
//!
//! So this is not a missing feature so much as a **default that silently destroys the evidence**,
//! which is the shape of defect this project keeps finding: the pane reported, the report was
//! wrong, and nothing said so.
//!
//! # Why the logic lives here rather than in the closure
//!
//! `CLAUDE.md`'s rule for view code — *push logic out of the paint path into checkable data*.
//! A label built inside a `label_formatter` closure is unreachable by any test, and
//! `egui_kittest` cannot read a hover tooltip's pixels. These functions take plain numbers,
//! return a `String`, and know nothing about egui, so the precision that Doug needs is pinned by
//! [`tests`] rather than by whoever next edits the plot.
//!
//! **The precision choices are the whole point and are asserted, not merely chosen:**
//!
//! | quantity | format | why |
//! |---|---|---|
//! | time | `{:.6}` | Rumoca's first step lands at `t = 0.0001`; three decimals shows `0.000` |
//! | step size `h` | `{:.3e}` | spans `1e-7` to `3e-3` in one run — no fixed-point width works |
//! | BDF order | integer | it is a small integer and lecture 1's whole §4 turns on reading it |
//! | trajectory value | `{:.12}` | to see a `9.8e-9` offset on a value of `0.5` you need 12 places |
//!
//! [`default_label_formatter`]: https://docs.rs/egui_plot/0.36.0/egui_plot/fn.default_label_formatter.html

/// One solver step as the label needs it: time, step size, method order.
///
/// A plain tuple-struct rather than `rumoca_solver::SolverStepRecord` so the tests need no
/// compile of the solver crate, and so this module cannot drift into depending on solver
/// internals it has no business reading.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct StepRow {
    pub(crate) t: f64,
    pub(crate) h: f64,
    pub(crate) order: usize,
}

/// The hover label for the **solver diagnostics** plot.
///
/// `index` is `egui_plot`'s nearest-data-point index, which both series share because both are
/// built from `solver_steps` in order. When the cursor is not near a point, `index` is `None` and
/// the cursor position is reported instead — still at full precision, because a cursor reading of
/// `0.000` is the defect this module exists to fix.
pub(crate) fn solver_step_label(
    steps: &[StepRow],
    index: Option<usize>,
    cursor: (f64, f64),
) -> String {
    match index.and_then(|i| steps.get(i).map(|s| (i, s))) {
        Some((i, s)) => {
            // **Two different step sizes, and calling either one "h" would be a claim.**
            //
            // `SolverStepRecord::h` is captured *after* the step completes, from diffsol's
            // current step size — which by then is the size chosen for the step **about to be
            // taken**. Its field doc says "Step size used", and the data disagrees: on
            // `SingleInertia` 11 of 34 records have `h != t_n - t_{n-1}`, every mismatch at a
            // point where the solver doubled. So the recorded `h` spans `[t_n, t_{n+1}]`, not
            // `[t_{n-1}, t_n}`.
            //
            // The label therefore reports the arrival step separately and says which is which.
            // Anything less would attribute a size to a step that did not use it — the same
            // defect class as the rounding this module was written for, one level subtler.
            let arrived_by = i
                .checked_sub(1)
                .and_then(|p| steps.get(p))
                .map_or(s.t, |prev| s.t - prev.t);
            format!(
                "step {} of {}\nt = {:.6}\norder = {}\narrived by h = {:.3e}\nnext h = {:.3e}",
                i,
                steps.len(),
                s.t,
                s.order,
                arrived_by,
                s.h,
            )
        }
        None => format!("t = {:.6}\ny = {:.3e}", cursor.0, cursor.1),
    }
}

/// Staircase for a value describing the interval that **ended** at its point — the BDF order.
///
/// # Why a line chart of these series was a false statement
///
/// Doug, 2026-09-25, on the BDF order series: a `Line` interpolates between its points, so between
/// a step at order 1 and the next at order 2 it draws 1.3, 1.6, 1.8 — **orders the solver never
/// used**. Order is an integer that changes discontinuously. The same is true of the step size:
/// `h` is constant across a step and jumps, and a diagonal between two step sizes describes a ramp
/// that never happened.
///
/// **The two series hold their value over *different* intervals**, which is why there are two
/// functions here rather than one with a flag — and why each takes the boundary it actually needs:
///
/// - **order** belongs to the step that *ended* at `t_n`, so it spans `[t_{n-1}, t_n]` and needs
///   the **run start** for its first segment. That is this function.
/// - **`h`** is the size selected for the step *about to be taken*, so it spans `[t_n, t_{n+1}]`
///   and needs the **run end** for its last. That is [`staircase_leading`].
///
/// **Getting that backwards would draw a truthful-looking staircase offset by one step**, which no
/// amount of looking at it would reveal.
pub(crate) fn staircase_trailing(points: &[(f64, f64)], run_start: f64) -> Vec<[f64; 2]> {
    let mut out: Vec<[f64; 2]> = Vec::with_capacity(points.len() * 2);
    let mut from = run_start;
    for &(t, v) in points {
        out.push([from, v]);
        out.push([t, v]);
        from = t;
    }
    out
}

/// Staircase for a value describing the interval that **starts** at its point — the step size.
///
/// The counterpart of [`staircase_trailing`], which carries the reasoning for both.
pub(crate) fn staircase_leading(points: &[(f64, f64)], run_end: f64) -> Vec<[f64; 2]> {
    let mut out: Vec<[f64; 2]> = Vec::with_capacity(points.len() * 2);
    for (i, &(t, v)) in points.iter().enumerate() {
        let to = points.get(i + 1).map_or(run_end, |&(nt, _)| nt);
        out.push([t, v]);
        out.push([to.max(t), v]);
    }
    out
}

/// The hover label for a **trajectory** series.
///
/// Twelve decimal places because the question this is built for is *"is this value exactly
/// `t²/2`?"*, and lecture 1's answer is that it differs by `9.8e-9` — invisible at three places
/// and at six.
pub(crate) fn trajectory_label(
    series: &str,
    times: &[f64],
    index: Option<usize>,
    cursor: (f64, f64),
) -> String {
    match index.and_then(|i| times.get(i).map(|t| (i, *t))) {
        Some((_, t)) => format!("{series}\nt = {t:.6}\nvalue = {:.12}", cursor.1),
        None => format!("t = {:.6}\nvalue = {:.12}", cursor.0, cursor.1),
    }
}

/// The floor applied to a step size before taking its logarithm.
///
/// `log10(0)` is `-inf` and `log10(negative)` is `NaN`; either poisons the plot's auto-bounds and
/// takes the whole curve with it. A step size is positive by construction, so this guard should
/// never fire — but *should never fire* is exactly the kind of claim this repository has been
/// wrong about, and a silently blank plot would read as "the solver took no steps".
const MIN_PLOTTABLE_STEP: f64 = 1e-300;

/// A step size as a **log₁₀ y-coordinate**, because a linear axis cannot show one.
///
/// # Why the axis has to be logarithmic
///
/// Measured 2026-09-25 across the corpus: `BenchActuator`'s step size runs from `2.49e-7` to
/// `6.52e-2` in a single run — a factor of **262,000** — and `BouncingBall`'s from `8.99e-6` to
/// `3.00e-3`. On a linear axis the small steps are pinned to zero, and the small steps are the
/// interesting ones: they are where the solver met something hard. On `BenchActuator` that is the
/// fast electrical transient, which is the entire reason that specimen exists.
///
/// `egui_plot` 0.36 has no log-scaled axis — only [`log_grid_spacer`] for the gridlines — so the
/// transform happens here and [`log_step_tick`] turns the tick values back into step sizes.
///
/// [`log_grid_spacer`]: https://docs.rs/egui_plot/0.36.0/egui_plot/fn.log_grid_spacer.html
pub(crate) fn log_step(h: f64) -> f64 {
    h.max(MIN_PLOTTABLE_STEP).log10()
}

/// A y-axis tick on the log step-size plot, rendered as **the step size it stands for**.
///
/// **The axis would otherwise be a lie.** A tick at `-4` means a step of `1e-4`, and an axis
/// labelled `-4` invites the reader to believe the solver took a negative step. `CLAUDE.md`'s rule
/// — *a renamed field is a claim* — applies to an axis as much as to a JSON key.
pub(crate) fn log_step_tick(log_value: f64) -> String {
    let rounded = log_value.round();
    if (log_value - rounded).abs() < 1e-6 {
        // A clean decade: `1e-4` reads better than `1.00e-4`.
        format!("1e{rounded:.0}")
    } else {
        format!("{:.1e}", 10_f64.powf(log_value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows() -> Vec<StepRow> {
        // The first three steps of `SingleInertia` on Rumoca 0.9.20, measured 2026-09-24.
        vec![
            StepRow {
                t: 0.0001,
                h: 1.0e-4,
                order: 1,
            },
            StepRow {
                t: 0.0002,
                h: 2.0e-4,
                order: 2,
            },
            StepRow {
                t: 0.0004,
                h: 2.0e-4,
                order: 2,
            },
        ]
    }

    /// **The defect, pinned: a step of `1e-4` must not render as `0.000`.**
    ///
    /// This is the assertion the whole module is for. `egui_plot`'s default formatter uses
    /// `{:.3}` and would satisfy every other test here while failing this one, so it is written
    /// against the *rounding* rather than against the text.
    #[test]
    fn a_step_size_smaller_than_a_millisecond_survives_the_label() {
        let label = solver_step_label(&rows(), Some(0), (0.0, 0.0));
        assert!(
            label.contains("1.000e-4"),
            "a 1e-4 step must appear at full precision, got:\n{label}"
        );
        assert!(
            !label.contains("h = 0.000"),
            "the three-decimal default is exactly the bug this replaces, got:\n{label}"
        );
    }

    /// **The order is why lecture 1 §4 can be checked at all**, so the label must carry it —
    /// `egui_plot` never would, because order is a *third* quantity and a plot point has two.
    #[test]
    fn the_label_reports_the_bdf_order_which_no_axis_can_show() {
        let steps = rows();
        assert!(solver_step_label(&steps, Some(0), (0.0, 0.0)).contains("order = 1"));
        assert!(solver_step_label(&steps, Some(1), (0.0, 0.0)).contains("order = 2"));
    }

    /// The step's ordinal and the total, so "exactly one step ran at order 1" is countable.
    #[test]
    fn the_label_locates_the_step_within_the_run() {
        let label = solver_step_label(&rows(), Some(2), (0.0, 0.0));
        assert!(label.contains("step 2 of 3"), "got:\n{label}");
    }

    /// Hovering away from any point still reports something usable rather than `0.000`.
    #[test]
    fn a_cursor_reading_away_from_the_data_is_still_at_full_precision() {
        let label = solver_step_label(&rows(), None, (0.25, 1.5e-5));
        assert!(label.contains("1.500e-5"), "got:\n{label}");
    }

    /// An out-of-range index falls back rather than panicking — the index comes from a library
    /// and is not this module's to trust.
    #[test]
    fn an_index_past_the_end_falls_back_instead_of_panicking() {
        let label = solver_step_label(&rows(), Some(99), (0.5, 0.25));
        assert!(label.contains("t = 0.500000"), "got:\n{label}");
    }

    /// **Twelve places, because that is what the question needs.**
    ///
    /// `SingleInertia`'s `phi` at `t = 1` is `0.500000009801` against an exact `0.5`. At three
    /// decimals both read `0.500` and the lecture's central measurement is unverifiable.
    #[test]
    fn a_trajectory_value_shows_the_offset_lecture_one_measures() {
        let label = trajectory_label("phi", &[1.0], Some(0), (1.0, 0.500_000_009_801));
        assert!(
            label.contains("0.500000009801"),
            "12 places are needed to see a 9.8e-9 offset on 0.5, got:\n{label}"
        );
    }

    /// **The order staircase holds each value to the step that ENDED at its time.**
    ///
    /// A `Line` through the raw points draws a diagonal from order 1 to order 2, asserting
    /// fractional orders the solver never used. The staircase asserts only what happened.
    #[test]
    fn the_order_staircase_holds_a_value_across_the_step_that_ended_at_it() {
        // Two steps: order 1 reaching t = 0.1, then order 2 reaching t = 0.3.
        let pts = staircase_trailing(&[(0.1, 1.0), (0.3, 2.0)], 0.0);
        assert_eq!(
            pts,
            vec![[0.0, 1.0], [0.1, 1.0], [0.1, 2.0], [0.3, 2.0]],
            "expected a flat-then-jump staircase anchored at the run start",
        );
        // The jump is vertical: two points share an x and differ in y.
        assert_eq!(pts[1][0], pts[2][0]);
        assert_ne!(pts[1][1], pts[2][1]);
        // **No interpolated order exists anywhere in the series.**
        assert!(
            pts.iter().all(|p| p[1] == 1.0 || p[1] == 2.0),
            "a fractional order appeared: {pts:?}",
        );
    }

    /// **The step-size staircase holds each value FORWARD**, because `h` is the size selected for
    /// the step about to be taken. Offsetting this by one would look equally plausible and be
    /// wrong by exactly one step.
    #[test]
    fn the_step_size_staircase_holds_a_value_forward_to_the_next_point() {
        let pts = staircase_leading(&[(0.1, 1e-3), (0.3, 2e-3)], 0.5);
        assert_eq!(
            pts,
            vec![[0.1, 1e-3], [0.3, 1e-3], [0.3, 2e-3], [0.5, 2e-3]],
            "expected each h held from its own time to the next, ending at the run end",
        );
    }

    /// Neither builder panics or inverts on a single point or none at all.
    #[test]
    fn a_staircase_of_one_point_or_none_is_well_formed() {
        assert!(staircase_trailing(&[], 0.0).is_empty());
        assert!(staircase_leading(&[], 1.0).is_empty());
        assert_eq!(
            staircase_trailing(&[(0.2, 3.0)], 0.0),
            vec![[0.0, 3.0], [0.2, 3.0]]
        );
        assert_eq!(
            staircase_leading(&[(0.2, 3.0)], 1.0),
            vec![[0.2, 3.0], [1.0, 3.0]]
        );
        // A run end before the last point must not draw backwards.
        let pts = staircase_leading(&[(0.9, 3.0)], 0.5);
        assert!(pts[1][0] >= pts[0][0], "segment ran backwards: {pts:?}");
    }

    /// **The label must not call the recorded `h` the step that was taken.**
    ///
    /// `SolverStepRecord::h` is captured after the step and is diffsol's *next* size; its field
    /// doc says "Step size used". On `SingleInertia`, 11 of 34 records disagree with
    /// `t_n - t_{n-1}`. A label saying plain "h" would attribute a size to a step that did not
    /// use it.
    #[test]
    fn the_label_separates_the_arrival_step_from_the_next_one() {
        // Step 1 arrived by 1e-4 (0.0002 - 0.0001) while its recorded h is 2e-4.
        let label = solver_step_label(&rows(), Some(1), (0.0, 0.0));
        assert!(label.contains("arrived by h = 1.000e-4"), "got:\n{label}");
        assert!(label.contains("next h = 2.000e-4"), "got:\n{label}");
        assert!(
            !label.contains("\nh = "),
            "a bare `h` claims to be the step taken, which the record does not say:\n{label}",
        );
    }

    /// **A decade tick reads as the step size, never as its exponent.**
    ///
    /// The failure this guards is an axis labelled −7 … −1 beside a curve of positive step sizes,
    /// which invites exactly the wrong reading.
    #[test]
    fn a_log_tick_is_labelled_with_the_step_size_it_stands_for() {
        assert_eq!(log_step_tick(-4.0), "1e-4");
        assert_eq!(log_step_tick(-7.0), "1e-7");
        assert_eq!(log_step_tick(0.0), "1e0");
        assert!(
            !log_step_tick(-4.0).starts_with('-'),
            "a tick must never render as a negative number — the step size is positive",
        );
    }

    /// A tick between decades still reports a step size rather than a fraction.
    #[test]
    fn a_tick_between_decades_is_still_a_step_size() {
        let label = log_step_tick(-3.5);
        assert!(label.contains("e-4"), "about 3.2e-4 expected, got {label}");
    }

    /// **The transform round-trips**, which is what makes the axis honest.
    #[test]
    fn the_log_transform_round_trips_across_the_measured_range() {
        // The extremes measured across the corpus on 2026-09-25.
        for h in [2.49e-7, 8.99e-6, 1.0e-4, 3.0e-3, 6.52e-2, 2.05e-1] {
            let back = 10_f64.powf(log_step(h));
            assert!(
                (back - h).abs() / h < 1e-12,
                "log_step round-trip lost {h}: got {back}",
            );
        }
    }

    /// **A non-positive step cannot blank the plot.**
    ///
    /// `log10(0)` is `-inf` and `log10(-1)` is `NaN`; either poisons auto-bounds and takes the
    /// whole curve with it, which would read as *"the solver took no steps"* rather than as a bug.
    #[test]
    fn a_non_positive_step_is_floored_rather_than_poisoning_the_axis() {
        for bad in [0.0, -1.0, -1e-9] {
            let v = log_step(bad);
            assert!(v.is_finite(), "log_step({bad}) must be finite, got {v}");
        }
    }
}
