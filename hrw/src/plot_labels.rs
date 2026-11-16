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
/// A plain tuple-struct rather than the solver's own step record (`MeStepRecord` on 0.10.x;
/// `rumoca_solver::SolverStepRecord` before it) so the tests need no compile of the solver
/// crate, and so this module cannot drift into depending on solver internals it has no
/// business reading.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StepRow {
    /// The time the step started from (`MeStepRecord::start_time`).
    pub start: f64,
    /// The time the step reached (`MeStepRecord::accepted_time`).
    pub t: f64,
    /// `t - start`: the size of **this** step.
    pub h: f64,
    pub order: usize,
}

/// The hover label for the **solver diagnostics** plot.
///
/// `index` is the step under the cursor, found by [`step_at`] from the cursor's time. **It was
/// `egui_plot`'s nearest-point index until 2026-11-16**, which was wrong twice over: a staircase
/// draws two points per step, so the index ran ahead of the step number and past the end; and
/// with one line per run (see [`step_runs`]) the index is relative to whichever line is nearest.
/// When no step covers the cursor, `index` is `None` and the cursor position is reported instead —
/// still at full precision, because a cursor reading of `0.000` is the defect this module exists
/// to fix.
pub(crate) fn solver_step_label(
    steps: &[StepRow],
    index: Option<usize>,
    cursor: (f64, f64),
) -> String {
    match index.and_then(|i| steps.get(i).map(|s| (i, s))) {
        Some((i, s)) => {
            // **One step size on v0.10.x, and reporting two was the 0.9.20 shape left
            // behind by the rebase.**
            //
            // On 0.9.20 `SolverStepRecord::h` was captured *after* the step completed,
            // from diffsol's current step size — by then the size chosen for the step
            // **about to be taken**. On `SingleInertia`, 11 of 34 records disagreed with
            // `t_n - t_{n-1}`. So the label reported `arrived by h` and `next h`
            // separately, correctly, and that is what Lecture 1 §4 documents.
            //
            // **v0.10.x removed the discrepancy at the source.**
            // `MeStepRecord::step_size()` is `accepted_time - start_time`, *derived* so
            // it "cannot disagree with the endpoints it is computed from". The recorded
            // `h` is now the step that WAS taken, and HRW carries no next-step size at
            // all — `MeStepRecord` has only `start_time`, `accepted_time` and `order`.
            //
            // So the two-value label printed **the same number twice**, one of them
            // under a name asserting something nothing in the data says. Measured
            // 2026-11-05 on `SingleInertia`: 0 of 34 rows differ. A reader hovering saw
            // "next h" and could only conclude the controller had chosen that size
            // next, which HRW does not know.
            let mut label = format!(
                "step {} of {}\nt = {:.6}\norder = {}\nh (this step) = {:.3e}",
                i,
                steps.len(),
                s.t,
                s.order,
                s.h,
            );
            // **Say when the solver went back over this step.** A later step that starts
            // before this one ended means integration restarted inside it — at a state event,
            // whose crossing is only found after a step has gone past it.
            if let Some(restart) = restarted_inside(steps, i) {
                label.push_str(&format!(
                    "\nintegration restarted inside this step, at t = {restart:.6}"
                ));
            }
            label
        }
        None => format!("t = {:.6}\ny = {:.3e}", cursor.0, cursor.1),
    }
}

/// **Each step drawn over the interval it covered**, `[start, t]`, holding `value` flat across it.
///
/// # Why a line chart of these series was a false statement
///
/// Doug, 2026-09-25, on the BDF order series: a `Line` interpolates between its points, so between
/// a step at order 1 and the next at order 2 it draws 1.3, 1.6, 1.8 — **orders the solver never
/// used**. Order is an integer that changes discontinuously, and `h` is constant across a step and
/// jumps. So both are flat segments.
///
/// # Why each step carries its own interval — 2026-11-16
///
/// This replaced two staircases built from consecutive step *ends*, and they were wrong two ways:
///
/// - **The step size was offset by one step.** It held each `h` *forward*, from its step's end to
///   the next, because on Rumoca 0.9.20 `h` was the size chosen for the step *about to be taken*.
///   Since 0.10.x `h` is `accepted_time - start_time`, the step that *ended* there — so every `h`
///   was drawn over the following step's interval, the exact error the old doc warned *"no amount
///   of looking at it would reveal"*. Nobody did see it; the time-travel defect below led here.
/// - **At an event the line ran backwards in time.** Doug, 2026-11-16, on `BouncingBall`: *"The
///   plots for BDF order and step size go backwards in time occasionally."* A state event is found
///   only after a step has passed it; the solver then restarts from the event time, so the next
///   step **starts earlier than the previous one ended**. Joining consecutive ends drew that as a
///   segment pointing back in time. [`step_runs`] now breaks the line there instead.
///
/// With `start` carried per step, neither needs a run boundary or a neighbour: each segment is a
/// fact about one record.
pub(crate) fn step_segments(rows: &[StepRow], value: impl Fn(&StepRow) -> f64) -> Vec<[f64; 2]> {
    rows.iter()
        .flat_map(|r| {
            let v = value(r);
            [[r.start, v], [r.t, v]]
        })
        .collect()
}

/// The steps split into **runs of continuous integration**, as index ranges into `rows`.
///
/// A new run begins wherever a step starts before the previous step ended — an integration
/// restart, which on this corpus is a state event (`BouncingBall` restarts at each bounce). Each
/// run is drawn as its own line, so no segment ever joins a later time to an earlier one. The step
/// that overshot the event stays drawn over its whole interval: it **was** taken, and the overlap
/// with the restart is what a restart looks like.
pub(crate) fn step_runs(rows: &[StepRow]) -> Vec<std::ops::Range<usize>> {
    let mut runs = Vec::new();
    let mut from = 0;
    for i in 1..rows.len() {
        if rows[i].start < rows[i - 1].t - restart_tolerance(rows[i - 1].t) {
            runs.push(from..i);
            from = i;
        }
    }
    if !rows.is_empty() {
        runs.push(from..rows.len());
    }
    runs
}

/// How far a step may start before its predecessor's end and still count as continuous.
///
/// Consecutive records share the boundary exactly or to rounding (`BouncingBall`'s contiguous
/// rows agree to ~1e-14); a restart backs up by a sizeable fraction of a step (0.054 s at the
/// first bounce). A relative tolerance keeps rounding on the continuous side.
fn restart_tolerance(t: f64) -> f64 {
    1e-9 * t.abs().max(1.0)
}

/// The step covering time `x` — **the latest one**, in the order the solver took them.
///
/// Where a restart overlaps an earlier step, the later step is the one the trajectory was
/// finally computed from, so it is the honest answer for a cursor there.
pub(crate) fn step_at(rows: &[StepRow], x: f64) -> Option<usize> {
    rows.iter().rposition(|r| r.start <= x && x <= r.t)
}

/// If integration later restarted from inside step `i`, the time it restarted from.
pub(crate) fn restarted_inside(rows: &[StepRow], i: usize) -> Option<f64> {
    let end = rows.get(i)?.t;
    rows.get(i + 1..)?
        .iter()
        .find(|r| r.start < end - restart_tolerance(end))
        .map(|r| r.start)
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

    /// Three contiguous steps in the v0.10.x shape: each `h` is its own `t - start`.
    fn rows() -> Vec<StepRow> {
        let row = |start: f64, t: f64, order| StepRow {
            start,
            t,
            h: t - start,
            order,
        };
        vec![
            row(0.0, 0.0001, 1),
            row(0.0001, 0.0003, 2),
            row(0.0003, 0.0005, 2),
        ]
    }

    /// `BouncingBall`'s first bounce, records 34–38 as Rumoca 0.10.4 recorded them
    /// (`solver_steps`, 2026-11-16): record 35 passes the bounce at `t ≈ 0.451524`, and 36 and
    /// 37 restart from it — over the same interval twice.
    fn first_bounce() -> Vec<StepRow> {
        let row = |start: f64, t: f64, order| StepRow {
            start,
            t,
            h: t - start,
            order,
        };
        vec![
            row(0.34976546, 0.42752487, 3),
            row(0.42752487, 0.50528428, 3),
            row(0.45152364, 0.45156161, 1),
            row(0.45152364, 0.45156161, 1),
            row(0.45156161, 0.45159957, 2),
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

    /// **Each step's `h` and order are held over the interval THAT step covered.**
    ///
    /// The step-size staircase this replaced held each `h` forward over the *next* step's
    /// interval — a 0.9.20 meaning `h` lost in 0.10.x. Here step 1 (`h = 2e-4`) covers
    /// `[0.0001, 0.0003]`; the old staircase drew `1e-4` there.
    #[test]
    fn each_step_holds_its_values_over_the_interval_it_covered() {
        let steps = rows();
        let h = step_segments(&steps, |r| r.h);
        assert_eq!(
            h[2],
            [0.0001, steps[1].h],
            "step 1's own size, from its own start"
        );
        assert_eq!(h[3], [0.0003, steps[1].h], "...to its own end");
        assert_ne!(
            steps[0].h, steps[1].h,
            "premise: the two sizes differ, so an offset shows"
        );
        let order = step_segments(&steps, |r| r.order as f64);
        assert_eq!(
            order,
            vec![
                [0.0, 1.0],
                [0.0001, 1.0],
                [0.0001, 2.0],
                [0.0003, 2.0],
                [0.0003, 2.0],
                [0.0005, 2.0],
            ],
            "flat segments, vertical jumps, and no fractional order anywhere",
        );
        assert_eq!(
            step_runs(&steps),
            vec![0..3],
            "contiguous steps are one run"
        );
        assert!(step_segments(&[], |r| r.h).is_empty());
        assert!(step_runs(&[]).is_empty());
    }

    /// **At an event the line breaks instead of running backwards in time.**
    ///
    /// Doug, 2026-11-16, on `BouncingBall`. Step 35 passes the bounce and steps 36 and 37
    /// restart from it, each starting before its predecessor ended; joining the ends drew a
    /// segment from `0.5053` back to `0.4516`.
    #[test]
    fn an_event_restart_starts_a_new_run_and_no_segment_runs_backwards() {
        let steps = first_bounce();
        let runs = step_runs(&steps);
        assert_eq!(runs, vec![0..2, 2..3, 3..5], "one break per restart");
        for run in runs {
            let pts = step_segments(&steps[run.clone()], |r| r.order as f64);
            assert!(
                pts.windows(2).all(|w| w[1][0] >= w[0][0]),
                "run {run:?} has a segment running backwards in time: {pts:?}",
            );
        }
    }

    /// **The cursor finds the step under it, and a cut-back step says so.**
    #[test]
    fn hovering_finds_the_step_under_the_cursor_and_names_a_restart() {
        let steps = first_bounce();
        // Only step 1 covers 0.44: before the bounce, after step 0.
        assert_eq!(step_at(&steps, 0.44), Some(1));
        // Past the bounce, the restart's steps cover it too, and the latest one wins.
        assert_eq!(step_at(&steps, 0.45158), Some(4));
        assert_eq!(step_at(&steps, 0.9), None);

        let label = solver_step_label(&steps, step_at(&steps, 0.44), (0.44, 3.0));
        assert!(label.contains("step 1 of 5"), "{label}");
        assert!(
            label.contains("integration restarted inside this step, at t = 0.451524"),
            "the step that overshot the bounce must say it was gone back over:\n{label}",
        );
        let plain = solver_step_label(&steps, Some(0), (0.0, 0.0));
        assert!(
            !plain.contains("restarted"),
            "an ordinary step says nothing:\n{plain}"
        );
    }

    /// **The label reports one step size, and names which one it is.**
    ///
    /// # What this test used to assert, and why it was green and vacuous
    ///
    /// It was `the_label_separates_the_arrival_step_from_the_next_one`, and it asserted
    /// `arrived by h = 1.000e-4` beside `next h = 2.000e-4` — a true description of
    /// Rumoca **0.9.20**, where `h` was captured after the step and was diffsol's next
    /// size. It passed on v0.10.4 too, because it builds [`rows()`] by hand and can put
    /// any numbers it likes in them. **Production data can no longer take that shape**:
    /// `MeStepRecord::step_size()` is `accepted_time - start_time`, so `h` is the step
    /// taken and 0 of `SingleInertia`'s 34 rows differ from the interval.
    ///
    /// So the label was printing one number twice, the second under a name claiming a
    /// controller decision HRW does not record. The test could not see it: **a fixture
    /// that can express a state the system cannot produce will keep asserting that
    /// state long after it stops occurring.**
    ///
    /// Found during `updating-rumoca.md` step 6b, re-measuring Lecture 1 — which is the
    /// only scheduled read of these numbers, and is why a lecture earns its place as an
    /// instrument rather than only as prose.
    #[test]
    fn the_label_names_which_step_size_it_reports() {
        let label = solver_step_label(&rows(), Some(1), (0.0, 0.0));
        assert!(
            label.contains("h (this step) = "),
            "the size must be named for the step it describes:\n{label}",
        );
        assert!(
            !label.contains("next h"),
            "HRW records no next-step size -- `MeStepRecord` carries start_time, \
             accepted_time and order, and nothing else:\n{label}",
        );
        assert!(
            !label.contains("arrived by"),
            "with one size there is nothing to separate it from:\n{label}",
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
