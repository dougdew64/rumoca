//! **The two painter-drawn matrix panes of the report stages** — the BLT spy plot and
//! the incidence matrix — including the Index Reduction Before/After split.
//!
//! Lifted out of `central_panel_ui` on 2026-08-20, the first cut into a *router*. See
//! [`docs/app-split-plan.md`](../docs/app-split-plan.md).
//!
//! # Why these two, and why together
//!
//! `central_panel_ui`'s sub-view dispatch has thirteen arms. Eleven were a single
//! delegation line (`self.tarjan_anim_ui(ui, ir_split)`); **these two carried their whole
//! pane inline**, 121 lines between them. That asymmetry is not cosmetic: the check that
//! caught the stranded `Animate` arm on 2026-08-20 was *"read the chain's arms as a column
//! and look for the odd one"*, and a column of eleven one-liners with two 35-to-86-line
//! bodies wedged into it cannot be read that way. With them gone every arm is one line, so
//! a missing `report_ready &&` is visible on sight rather than after a grep.
//!
//! They belong in one module because they are **one idea**: the two views of the report
//! that are drawn with `Painter` rather than widgets, memoised the same way, and both
//! subject to the same Before/After split on Index Reduction.
//!
//! # What is NOT here
//!
//! The matrices themselves. [`crate::spyplot`] and [`crate::incidence_view`] own the
//! parsing and the painting; this module owns **the pane** — which cache is filled from
//! which half of the report, which camera looks at it, and what is said when there is
//! nothing to show. That division is why the extraction was cheap: the drawing had left
//! `app.rs` long ago and only the wiring stayed behind.
//!
//! # "Not queryable" was true of the pixels, never of the pane
//!
//! `ui_tests.rs` puts the incidence matrix and the spy plot in its *"not queryable"*
//! column, and that is still true of what the `Painter` emits. It was never true of the
//! surrounding pane: the captions, the split headings and the four absence notices are
//! ordinary labels, and the caches are plain fields a test can read afterwards.
//!
//! **The dimensions of the matrix that got built are the assertion that matters.** A
//! Before/After mix-up produces a perfectly well-formed matrix of the wrong system under a
//! heading naming the right one — no screenshot and no pair of eyes would catch it, and
//! `n_eq × n_var` catches it in one line.

use eframe::egui;

use crate::bridge::Seg;
use crate::canvas::Canvas;
use crate::incidence_view::IncidenceMatrix;
use crate::spyplot;
use serde_json::Value;

/// One matrix on screen: **where it is remembered, and what is looking at it.**
///
/// The two always travel together and have **opposite lifetimes**, which is exactly why
/// they live in different structs on `App` and are paired only here. The cache is dropped
/// the moment the report stage changes ([`crate::stage_caches`]); the camera deliberately
/// survives, so returning to a view finds it where you left it
/// ([`crate::stage_view::Viewport`]). Bundling them into a parameter struct at the call
/// site is the [`crate::tree::TreeOptions`] pattern: it keeps the signature under the
/// argument limit without pretending the two halves are one field.
pub(crate) struct MatrixPane<'a, T> {
    /// Outer `Option` = built for this stage yet; inner = there was anything to build.
    pub(crate) cache: &'a mut Option<Option<T>>,
    /// Pan/zoom state, which outlives the cache.
    pub(crate) camera: &'a mut Canvas,
}

/// The Index Reduction split's left-hand heading — the system as the DAE stated it.
///
/// Red, because Before is the pane whose defect Index Reduction exists to remove. The
/// colours are [`crate::colors`]' animation pair rather than raw values, so the split
/// reads the same as every animation that shows a failure being resolved.
fn before_heading(ui: &mut egui::Ui) {
    ui.label(
        egui::RichText::new("Before (raw DAE)")
            .strong()
            .color(crate::colors::ANIM_FAIL),
    );
}

/// The Index Reduction split's right-hand heading — the system after differentiation.
fn after_heading(ui: &mut egui::Ui) {
    ui.label(
        egui::RichText::new("After (reduced)")
            .strong()
            .color(crate::colors::ANIM_PATH_FOUND),
    );
}

/// The BLT spy plot: block structure along the diagonal.
///
/// `report` is the report stage's value. **It is always `Some` here** — the caller only
/// reaches this arm under `report_ready`, which is *"a report stage **and** it produced a
/// value"* — so the notice below always means *the report has no BLT blocks*, never *the
/// stage has not run*. The parameter is nonetheless an `Option`, and that is a borrow
/// fact rather than a doubt: `report_ready` is a `bool` settled forty lines above the
/// dispatch chain, and a `&Value` held from there would outlive the `&mut self` calls the
/// chain's other eleven arms make.
///
/// # Why `ir_split` is a `bool` here and a `None` next door
///
/// [`incidence_pane_ui`] takes `before: Option<MatrixPane>`, so the Before pane exists
/// exactly when the split is on and the two facts cannot disagree. **The spy plot cannot
/// use that shape, because it has no Before pane at all** — a spy plot needs a full
/// matching, and a system that reached Index Reduction did not have one beforehand. The
/// `bool` is therefore not a weaker version of the same parameter; it asks a different
/// question: *do I owe the reader an explanation for an empty left half?*
pub(crate) fn spy_plot_pane_ui(
    ui: &mut egui::Ui,
    report: Option<&Value>,
    pane: MatrixPane<'_, spyplot::Plot>,
    capture: &mut Option<Vec<Seg>>,
    tracked: Option<&str>,
    ir_split: bool,
) {
    if ir_split {
        // No spy-plot for the Before pane (needs full matching), show only the
        // After pane.
        before_heading(ui);
        ui.weak("Spy-plot unavailable (structurally singular \u{2014} no BLT decomposition)");
        ui.add_space(12.0);
        after_heading(ui);
    }
    let cached = pane
        .cache
        .get_or_insert_with(|| report.and_then(spyplot::Plot::from_report));
    if let Some(plot) = cached {
        ui.weak(plot.caption());
        plot.ui(ui, pane.camera, capture, tracked);
    } else {
        ui.weak("(the structural report has no BLT blocks to plot)");
    }
}

/// The incidence matrix: which equations reference which unknowns.
///
/// `report` is `Some` whenever this is reached, for the reason [`spy_plot_pane_ui`] gives.
///
/// `before` is `Some` exactly on an Index Reduction stage that needed reducing, and its
/// matrix is parsed from **`report["before"]`** while `after` is parsed from the report
/// root. Those two halves are the whole point of the split and the one thing here that can
/// go wrong invisibly, so the test asserts which system each pane built rather than that
/// both panes appeared.
///
/// `tracked` highlights a column, and **only in the After pane**: the tracked identifier is
/// a name in the model as it now stands, and a column index is a position in one particular
/// matrix. Resolving it against Before would highlight whichever variable happened to sit at
/// that position in the un-reduced system — the right column number for the wrong matrix.
pub(crate) fn incidence_pane_ui(
    ui: &mut egui::Ui,
    report: Option<&Value>,
    after: MatrixPane<'_, IncidenceMatrix>,
    before: Option<MatrixPane<'_, IncidenceMatrix>>,
    capture: &mut Option<Vec<Seg>>,
    tracked: Option<&str>,
    highlighted_row: Option<usize>,
) {
    let Some(before) = before else {
        let cached = after
            .cache
            .get_or_insert_with(|| report.and_then(IncidenceMatrix::from_report));
        if let Some(mat) = cached {
            mat.caption_ui(ui);
            let tracked_col = tracked.and_then(|name| mat.column_index(name));
            mat.ui(ui, after.camera, capture, highlighted_row, tracked_col);
        } else {
            ui.weak("(no incidence data in this report)");
        }
        return;
    };

    // Before/After split for incidence matrices.
    let before_cached = before.cache.get_or_insert_with(|| {
        report
            .and_then(|v| v.get("before"))
            .and_then(IncidenceMatrix::from_report)
    });
    let after_cached = after
        .cache
        .get_or_insert_with(|| report.and_then(IncidenceMatrix::from_report));
    ui.columns(2, |cols| {
        // Before pane
        before_heading(&mut cols[0]);
        if let Some(mat) = before_cached {
            mat.caption_ui(&mut cols[0]);
            mat.ui(&mut cols[0], before.camera, capture, highlighted_row, None);
        } else {
            cols[0].weak("(no before incidence data)");
        }
        // After pane
        after_heading(&mut cols[1]);
        if let Some(mat) = after_cached {
            mat.caption_ui(&mut cols[1]);
            let tracked_col = tracked.and_then(|name| mat.column_index(name));
            mat.ui(
                &mut cols[1],
                after.camera,
                capture,
                highlighted_row,
                tracked_col,
            );
        } else {
            cols[1].weak("(no after incidence data)");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;
    use serde_json::json;

    /// A report whose incidence matrix is `n_eq` × `n_var`.
    ///
    /// **The dimensions are the identity here.** Every test below tells the two halves of
    /// the split apart by their size, because that is the one difference a reader could
    /// not see: two matrices under two correct headings look right whichever way round
    /// they are.
    fn matrix_report(n_eq: usize, n_var: usize) -> Value {
        json!({
            "incidence": {
                "n_eq": n_eq,
                "n_var": n_var,
                "unknown_names": (0..n_var).map(|i| format!("v{i}")).collect::<Vec<_>>(),
                "rows": (0..n_eq)
                    .map(|i| json!({ "equation": format!("f_x[{i}]"), "unknowns": [0] }))
                    .collect::<Vec<_>>(),
            }
        })
    }

    /// An Index Reduction report: the reduced system at the root, the raw DAE under
    /// `"before"`, and deliberately **different sizes** so a swap is visible.
    fn split_report() -> Value {
        let mut after = matrix_report(2, 2);
        after["before"] = matrix_report(3, 3);
        after
    }

    /// What the pane is given and what it leaves behind, so a test can read the caches
    /// after the frame rather than only what reached the accessibility tree.
    struct Panes {
        report: Value,
        split: bool,
        after_cache: Option<Option<IncidenceMatrix>>,
        before_cache: Option<Option<IncidenceMatrix>>,
        after_camera: Canvas,
        before_camera: Canvas,
        capture: Option<Vec<Seg>>,
    }

    impl Panes {
        fn new(report: Value, split: bool) -> Self {
            Panes {
                report,
                split,
                after_cache: None,
                before_cache: None,
                after_camera: Canvas::default(),
                before_camera: Canvas::default(),
                capture: None,
            }
        }
    }

    /// Draw the incidence pane once and hand back the harness, caches included.
    ///
    /// **Sized like a real pane** (`1200×900`) — `ui.columns(2, …)` clips, and a clipped
    /// widget stays in the accessibility tree while behaving as though it is not there,
    /// which is the trap `stage_tabs` and `equation_sheet_view` both recorded.
    fn draw_incidence(state: Panes) -> Harness<'static, Panes> {
        let mut h = Harness::builder()
            .with_size(egui::Vec2::new(1200.0, 900.0))
            .build_ui_state(
                |ui, p: &mut Panes| {
                    let before = p.split.then_some(MatrixPane {
                        cache: &mut p.before_cache,
                        camera: &mut p.before_camera,
                    });
                    incidence_pane_ui(
                        ui,
                        Some(&p.report),
                        MatrixPane {
                            cache: &mut p.after_cache,
                            camera: &mut p.after_camera,
                        },
                        before,
                        &mut p.capture,
                        None,
                        None,
                    );
                },
                state,
            );
        h.run_steps(2);
        h
    }

    /// The dimensions of whatever ended up in a cache, or `None` if nothing was built.
    fn built(cache: &Option<Option<IncidenceMatrix>>) -> Option<(usize, usize)> {
        cache.as_ref()?.as_ref().map(|m| (m.n_eq(), m.n_var()))
    }

    /// **The one defect here that no reader could catch: the halves swapped.**
    ///
    /// Both panes would render a well-formed matrix under a correct-looking heading, and
    /// the Before pane would be showing the reduced system while the tour says it is the
    /// raw DAE — the whole point of the stage, inverted, in silence. Must-fire verified by
    /// exchanging the two `get_or_insert_with` sources: this then fails with `(2, 2)`
    /// where it expects `(3, 3)`.
    #[test]
    fn the_before_pane_reads_the_before_half_of_the_report() {
        let h = draw_incidence(Panes::new(split_report(), true));

        assert_eq!(
            built(&h.state().before_cache),
            Some((3, 3)),
            "the Before pane must be built from report[\"before\"], the raw DAE"
        );
        assert_eq!(
            built(&h.state().after_cache),
            Some((2, 2)),
            "the After pane must be built from the report root, the reduced system"
        );
    }

    /// **Both headings, or the split is a lie.** Two matrices side by side with no labels
    /// say nothing about which direction time runs in.
    #[test]
    fn the_split_labels_both_halves() {
        let h = draw_incidence(Panes::new(split_report(), true));

        assert!(
            h.query_by_label_contains("Before (raw DAE)").is_some(),
            "the left column must say which system it is"
        );
        assert!(
            h.query_by_label_contains("After (reduced)").is_some(),
            "the right column must say which system it is"
        );
        // Non-vacuity: the raw DAE's own caption reached the screen too, so the queries
        // above ran against a populated pane rather than an empty one.
        assert!(
            h.query_by_label_contains("3\u{00d7}3 incidence").is_some(),
            "the raw DAE's caption must render under its heading"
        );
    }

    /// **No split, no split chrome.** A Structural stage shows one matrix, and a stray
    /// "After (reduced)" over it would claim a reduction that never ran.
    #[test]
    fn an_unsplit_pane_draws_neither_heading() {
        let h = draw_incidence(Panes::new(matrix_report(2, 2), false));

        assert!(
            h.query_by_label_contains("Before (raw DAE)").is_none(),
            "a single-matrix pane must not label itself as half of a split"
        );
        assert!(
            h.query_by_label_contains("After (reduced)").is_none(),
            "a single-matrix pane must not label itself as half of a split"
        );
        assert!(
            h.query_by_label_contains("2\u{00d7}2 incidence").is_some(),
            "the matrix itself must still render"
        );
    }

    /// **Absence is stated, never filled** — the rule this repository treats as a defect
    /// class. A split report whose `"before"` half is missing must say the Before pane has
    /// nothing, not quietly render the After matrix twice.
    #[test]
    fn a_missing_before_half_is_reported_rather_than_substituted() {
        let h = draw_incidence(Panes::new(matrix_report(2, 2), true));

        assert!(
            h.query_by_label_contains("(no before incidence data)")
                .is_some(),
            "the Before pane must say the report carried no raw DAE matrix"
        );
        assert_eq!(
            built(&h.state().before_cache),
            None,
            "and it must not have borrowed the After matrix to fill the gap"
        );
        assert!(
            h.query_by_label_contains("2\u{00d7}2 incidence").is_some(),
            "the After pane still renders — the split degrades to one half, it does not fail"
        );
    }

    /// **The cache is what stops a re-parse every frame, so it must actually be
    /// consulted.** Seeded with a matrix that could not have come from this report, the
    /// pane must leave it alone. Must-fire verified by replacing `get_or_insert_with` with
    /// an unconditional rebuild: this then fails with `(2, 2)`.
    #[test]
    fn a_built_matrix_is_not_rebuilt_on_the_next_frame() {
        let seeded = IncidenceMatrix::from_report(&matrix_report(7, 7));
        assert!(
            seeded.is_some(),
            "the fixture must parse, or the test proves nothing"
        );

        let mut state = Panes::new(matrix_report(2, 2), false);
        state.after_cache = Some(seeded);
        let h = draw_incidence(state);

        assert_eq!(
            built(&h.state().after_cache),
            Some((7, 7)),
            "a matrix already built for this stage must survive the frame"
        );
    }

    /// **The spy plot owes an explanation for its empty left half**, and it is the only
    /// pane here that has one to give: a spy plot needs a full matching, which a system
    /// that required index reduction did not have. Silently drawing one plot during a
    /// split would read as *"this covers both"*.
    #[test]
    fn a_split_spy_plot_says_why_there_is_no_before_plot() {
        let mut cache: Option<Option<spyplot::Plot>> = None;
        let mut camera = Canvas::default();
        let mut capture: Option<Vec<Seg>> = None;
        let report = json!({ "blocks": [] });

        let mut h = Harness::builder()
            .with_size(egui::Vec2::new(1200.0, 900.0))
            .build_ui(move |ui| {
                spy_plot_pane_ui(
                    ui,
                    Some(&report),
                    MatrixPane {
                        cache: &mut cache,
                        camera: &mut camera,
                    },
                    &mut capture,
                    None,
                    true,
                );
            });
        h.run_steps(2);

        assert!(
            h.query_by_label_contains("Spy-plot unavailable").is_some(),
            "the missing Before plot must be explained, not merely absent"
        );
        assert!(
            h.query_by_label_contains("no BLT blocks to plot").is_some(),
            "an empty block list must say the report had nothing, not draw an empty plot"
        );
    }
}
