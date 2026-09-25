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
        Some((i, s)) => format!(
            "step {} of {}\nt = {:.6}\nh = {:.3e}\norder = {}",
            i,
            steps.len(),
            s.t,
            s.h,
            s.order,
        ),
        None => format!("t = {:.6}\ny = {:.3e}", cursor.0, cursor.1),
    }
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
}
