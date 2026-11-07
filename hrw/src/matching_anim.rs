//! Animated matching stepper — replays Kuhn's augmenting-path algorithm
//! frame by frame on the incidence matrix.
//!
//! The animation uses `MatchingFrame`s recorded by the traced matching
//! in `rumoca_phase_structural::matching`. Each frame captures one
//! algorithmic decision (explore an edge, find a free variable, displace
//! a match, assign a pair, or fail) plus a snapshot of the partial
//! matching at that moment.
//!
//! The stepper renders the incidence matrix with:
//! - Partial matching: green circles on currently-matched cells
//! - Active exploration: yellow highlight on the edge being explored
//! - Success/failure flash: green or red on the last outcome
//!
//! Controls: play/pause, step forward/back, reset, speed slider.

use eframe::egui;

use rumoca_phase_structural::{MatchingRecord, MatchingStep, StructuralRecord};

use crate::canvas::Canvas;
use crate::incidence_view::IncidenceMatrix;
use crate::playback::{Animated, Playback};

/// Seconds between auto-advance frames.
const FRAME_INTERVAL: f64 = 0.4;

/// Animation state machine — supports three modes:
/// 1. **Recorded**: pre-computed frames from `from_incidence` (play/pause/step)
/// 2. **Live**: reads frames from an `mpsc` channel receiver as a debugger
///    steps through the algorithm on a worker thread
pub struct MatchingAnimation {
    /// Cursor, timing and live-session state — see [`Playback`]. The three
    /// animation views used to declare those seven fields each; now only the
    /// matrix geometry below is this view's own.
    playback: Playback<MatchingRecord>,
    n_eq: usize,
    n_var: usize,
    equation_names: Vec<String>,
    unknown_names: Vec<String>,
    rows: Vec<Vec<usize>>,
}

impl MatchingAnimation {
    /// Equations the search gave up on, in the order it gave up.
    ///
    /// One entry per `MatchingStep::EquationFailed`, which is exactly the rank
    /// deficiency of the incidence matrix: each failure is an equation with no
    /// augmenting path left, so it stays unmatched. An empty result means a
    /// perfect matching.
    ///
    /// Reads the whole frame stream rather than the cursor, so it answers "how
    /// does this end" regardless of where playback has got to.
    pub fn failed_equations(&self) -> Vec<usize> {
        self.playback
            .frames()
            .iter()
            .filter_map(|f| match f.step {
                MatchingStep::EquationFailed(eq) => Some(eq),
                _ => None,
            })
            .collect()
    }

    /// `(matched, total)` at the end of the trace — the final matching, not the
    /// cursor's partial one. `matched < total` is a structurally singular system.
    /// **`None` before any frame has arrived**, rather than `(0, n_eq)`.
    ///
    /// The doc line above states the reading: `matched < total` is a structurally
    /// singular system. So `unwrap_or(0)` on an empty frame list did not report
    /// "nothing yet" — it reported **"every equation is unmatched"**, the strongest
    /// possible claim about the model, from having no data at all.
    ///
    /// A recorded animation always has frames (`from_captured_frames` returns `None`
    /// otherwise), so this was unreachable there. **A live debug session is the case
    /// that reaches it**: the panel renders while frames are still arriving from the
    /// debugger thread, and until the first one lands the system looked singular.
    /// Found by the 2026-08-04 sweep.
    pub fn match_progress(&self) -> Option<(usize, usize)> {
        let last = self.playback.frames().last()?;
        Some((
            last.match_eq.iter().filter(|m| m.is_some()).count(),
            self.n_eq,
        ))
    }

    /// **The matching this animation ends on**, per equation row — the answer
    /// HRW re-derived, as opposed to the one Rumoca reported.
    ///
    /// `docs/fidelity-plan.md` **F1**. `match_progress` already reads the last
    /// frame, but returns only a *count*: two different matchings of the same
    /// size are indistinguishable through it, and a permutation is exactly the
    /// failure worth catching. Compare against
    /// [`IncidenceMatrix::reported_matching`].
    ///
    /// Empty frames yield an all-`None` vector rather than a short one, so the
    /// comparison stays index-aligned instead of failing on length.
    pub fn final_matching(&self) -> Vec<Option<usize>> {
        self.playback
            .frames()
            .last()
            .map(|f| f.match_eq.clone())
            .unwrap_or_else(|| vec![None; self.n_eq])
    }

    /// Every step in the trace, in order.
    pub fn steps(&self) -> Vec<MatchingStep> {
        self.playback
            .frames()
            .iter()
            .map(|f| f.step.clone())
            .collect()
    }

    /// Build the animation trace from a parsed incidence matrix (recorded mode).
    /// Build from **frames captured during the compile**, with the matrix supplying
    /// only the names and shape to draw them against.
    ///
    /// It replaced `from_incidence`, which re-ran matching when the tab was opened:
    /// because matching is deterministic the two agreed, but the animation then replayed
    /// a search that produced nothing, while the blocks on screen came from a different
    /// execution nobody watched. `from_incidence` is gone (past tense since the
    /// 2026-11-07 sweep).
    ///
    /// Added 2026-08-04 alongside `rumoca-phase-structural`'s capture scope. Doug:
    /// *"our ability to play animations is tremendously valuable and I want to
    /// preserve that. But I want to capture the data for those animations during the
    /// actual compilation rather than use replays."*
    ///
    /// **Returns `None` when the capture is empty or does not fit this matrix.**
    ///
    /// *(Corrected 2026-08-04: this said "falls back", which had stopped being true
    /// the same day the fallback was removed. The body below returns `None` and the
    /// caller states the absence — `App::structural_unavailable`. A doc comment
    /// describing behaviour the function no longer has is the same defect class as a
    /// pane describing a run that did not happen, and the source is a learning
    /// artifact here, so it is held to the rule too.)*
    pub fn from_captured_frames(
        mat: &IncidenceMatrix,
        frames: &[rumoca_phase_structural::MatchingRecord],
    ) -> Option<Self> {
        // **The frames must describe THIS matrix.**
        //
        // The matching and Tarjan views render under Structural *and* Index
        // Reduction, and the incidence matrix comes from whichever stage is showing —
        // so on the Index Reduction tab `mat` is the **reduced** system while the
        // captured frames are from the raw one. Their indices would then address
        // rows that are not there, or the wrong rows.
        //
        // Caught by Doug on 2026-08-04 asking whether the fallbacks were still
        // replays: they are, and checking that question found this. **The mismatch is
        // worse than the replay it replaced** — a re-derivation from the reduced
        // matrix is at least self-consistent, while these frames would animate one
        // system's search over another's rows with nothing on screen to say so.
        //
        // Validated here rather than only gated at the call site, because a call site
        // can be forgotten and this cannot: every frame carries `match_eq`, whose
        // length *is* the equation count of the system that produced it.
        // **A count is not an identity**, so check the columns too.
        //
        // `match_eq.len() == n_eq` catches the case that actually occurred (raw frames
        // against the reduced matrix, 97 equations vs 20). It does **not** catch two
        // systems with the *same* equation count — which index reduction can produce,
        // since demotion moves a variable from state to algebraic without necessarily
        // changing how many equations there are.
        //
        // The variable indices inside the frames give a second, free constraint: every
        // matched column must exist in this matrix. Still a proxy rather than an
        // identity (see `docs/identity-and-provenance.md` on counts deciding identity),
        // and strictly stronger than the length alone. Tightened 2026-08-04.
        let fits = frames.first().is_some_and(|f| {
            f.match_eq.len() == mat.n_eq() && f.match_eq.iter().flatten().all(|&v| v < mat.n_var())
        });
        if frames.is_empty() || !fits {
            // **`None`, not a re-derivation.** Re-running the search here would draw
            // a picture of a run that did not happen; the caller says so instead.
            // See `App::structural_unavailable`.
            return None;
        }
        Some(Self {
            playback: Playback::recorded(frames.to_vec(), FRAME_INTERVAL),
            n_eq: mat.n_eq(),
            n_var: mat.n_var(),
            equation_names: mat.equation_texts().to_vec(),
            unknown_names: mat.unknown_names().to_vec(),
            rows: mat.rows().to_vec(),
        })
    }

    // `from_incidence` is deleted. It re-ran the algorithm from scratch over
    // the pane's rows, and it survived as test-only because one fidelity test
    // wanted an *independent* implementation to cross-check Rumoca's report
    // against.
    //
    // **Both halves of that purpose are gone.** On 0.10.0 there is no
    // independent implementation to be had — `maximum_matching` is private and
    // the `*_with_trace` entry points do not exist — and more to the point
    // there is nothing left to cross-check: the animation reads the compile's
    // own steps through `inspect_structural_report`, so there is one matching
    // rather than two that might disagree.
    //
    // The test's other stated purpose, that the report's JSON round trip
    // preserves row and column order, is covered by `fidelity::check_f2`,
    // which compares the published unknown names, row labels and per-row
    // column sets against a fresh `build_incidence` **as ordered sequences**.

    /// Start a live debug session: spawn a thread that runs the matching
    /// algorithm with a `LiveTrace` producer, then return an animation that
    /// drains frames from the channel receiver. The debugger breakpoint on
    /// `live_trace_breakpoint` pauses the thread after each frame push;
    /// the user steps with F5.
    ///
    /// **The session ending does not release the anchor breakpoint**, which is
    /// why there is no completion callback here. There used to be an
    /// `on_complete` whose sole job was removing it before the thread exited,
    /// so LLDB would not deliver SIGSTOP/SIGCHLD on thread termination — a
    /// workaround for a debugger and platform HRW no longer runs on, and the
    /// exact mechanism that made every Debug press after the first silently
    /// fail to stop (`docs/ideas.md` #74). The anchor now clears only on the
    /// three events that end its reason to exist: a failed spawn, a specimen
    /// change, and app exit.
    /// **Takes the DAE, not just the matrix, since the v0.10.0 rebase.** It
    /// used to rebuild `eq_vars` from `mat.rows()` and re-run
    /// `maximum_matching` over that — stepping a projection of the system
    /// rather than the system. `spawn_structural_live` runs the analysis
    /// Rumoca runs, over the DAE itself; the matrix is still needed for the
    /// labels and geometry the pane draws.
    pub fn start_live(
        mat: &IncidenceMatrix,
        dae: std::sync::Arc<rumoca_ir_dae::Dae>,
        frame_delay: std::time::Duration,
    ) -> Option<Self> {
        let (rx, done) = crate::live_trace::spawn_structural_live(
            dae,
            frame_delay,
            "matching-debug",
            |record| match record {
                StructuralRecord::Matching(matching) => Some(matching),
                StructuralRecord::Tarjan(_) | StructuralRecord::Tearing(_) => None,
            },
        )?;

        Some(Self {
            playback: Playback::live(rx, done, FRAME_INTERVAL),
            n_eq: mat.n_eq(),
            n_var: mat.n_var(),
            equation_names: mat.equation_texts().to_vec(),
            unknown_names: mat.unknown_names().to_vec(),
            rows: mat.rows().to_vec(),
        })
    }

    pub fn is_empty(&self) -> bool {
        self.playback.is_empty()
    }

    /// Render the animation controls and the annotated incidence matrix.
    ///
    /// Returns `true` on the frame the Debug button is clicked — the caller
    /// owns the bridge state needed to actually arm a session.
    #[must_use]
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        canvas: &mut Canvas,
        tracked: Option<&str>,
        arming: bool,
        debug_enabled: bool,
    ) -> bool {
        // In live mode, sync new frames from the channel receiver.
        self.playback.sync_live();
        let live = self.playback.live_state(arming);

        // Nothing to show at all — no recorded frames and no live session.
        if self.playback.is_empty() && !arming {
            ui.label("No matching trace available.");
            return false;
        }

        let dt = ui.input(|i| i.stable_dt) as f64;
        if self.playback.tick(dt, live) {
            ui.ctx().request_repaint();
        }

        // --- Controls ---
        let debug_clicked =
            crate::animation_controls(ui, self.playback.controls(), live, debug_enabled);

        // A session is starting or the debugger is parked at the startup gate,
        // so no frames have arrived. The controls above stay rendered (disabled)
        // rather than the whole row vanishing until the first Continue.
        if self.playback.frames().is_empty() {
            ui.add_space(4.0);
            ui.label("Waiting for first frame from debugger\u{2026}");
            ui.ctx().request_repaint();
            return debug_clicked;
        }

        // --- Step description ---
        if let Some(frame) = self.playback.current() {
            ui.horizontal(|ui| {
                let (icon, desc) =
                    step_description(&frame.step, &self.equation_names, &self.unknown_names);
                ui.label(egui::RichText::new(icon).size(16.0));
                ui.label(desc);
            });
            self.render_running_state(ui, frame);
        }

        ui.add_space(4.0);

        // --- Animated incidence matrix ---
        self.draw_matrix(ui, canvas, tracked);

        debug_clicked
    }
}

impl Animated for MatchingAnimation {
    fn which(&self) -> &'static str {
        "matching"
    }

    fn position(&self) -> (usize, usize) {
        self.playback.position()
    }

    fn live_state(&self, arming: bool) -> crate::LiveState {
        self.playback.live_state(arming)
    }

    /// The step description the view is drawing, plus how much of the matching
    /// is settled.
    ///
    /// `matched` is the count the frame's own `match_eq` snapshot carries, so it
    /// is the state *at this frame* rather than the final result — which is the
    /// whole point of watching an augmenting-path algorithm run. `step` comes
    /// from `step_description`, shared with the on-screen label so the two
    /// cannot give different accounts of the same frame.
    fn current_frame_context(&self) -> Option<serde_json::Value> {
        let frame = self.playback.current()?;
        let (_, desc) = step_description(&frame.step, &self.equation_names, &self.unknown_names);
        Some(serde_json::json!({
            "step": desc,
            "matched_so_far": frame.match_eq.iter().filter(|m| m.is_some()).count(),
            "n_equations": self.n_eq,
            "n_unknowns": self.n_var,
        }))
    }

    /// Seek is delegated to [`Playback`], so all eight views agree on what a frame
    /// index means and on refusing an out-of-range one.
    fn seek(&mut self, n: usize) -> bool {
        self.playback.seek(n)
    }
}

impl MatchingAnimation {
    /// What the matrix is showing, in words.
    ///
    /// The step line says what just happened; this says **where the algorithm
    /// stands**, and states the goal once so the picture is legible rather than
    /// decorative. Doug, 2026-07-29: the text-only reduction and `pre()` replays
    /// turned out more useful than expected — *"the text only playbacks provide
    /// useful summaries of what I will find if I decide to step through the
    /// algorithm code"* — and he asked for the same beside the visual ones.
    ///
    /// The counts come from the frame's own `match_eq` snapshot, so they are the
    /// state *at this frame*, not the final result. Watching "3 of 8" climb is
    /// the content of an augmenting-path algorithm; the final number says
    /// nothing about how it got there.
    fn render_running_state(&self, ui: &mut egui::Ui, frame: &MatchingRecord) {
        let matched = frame.match_eq.iter().filter(|m| m.is_some()).count();
        ui.add_space(2.0);
        ui.horizontal_wrapped(|ui| {
            ui.weak("Goal:");
            ui.weak(
                "pair every equation with a distinct unknown it will solve for. \
                 Kuhn's algorithm grows the pairing one equation at a time, \
                 backtracking through already-paired unknowns when it has to.",
            );
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(
                egui::RichText::new(format!("Matched {matched} of {}", self.n_eq))
                    .strong()
                    .color(if matched == self.n_eq {
                        crate::colors::ANIM_PATH_FOUND
                    } else {
                        crate::colors::ANIM_EXPLORE
                    }),
            );
            // Naming what is *not* matched yet is the useful half: a system that
            // ends with an unmatched equation is structurally singular, and that
            // is the outcome the Index Reduction stage exists to fix.
            let unmatched: Vec<&str> = frame
                .match_eq
                .iter()
                .enumerate()
                .filter(|(_, m)| m.is_none())
                .map(|(eq, _)| {
                    self.equation_names
                        .get(eq)
                        .map(String::as_str)
                        .unwrap_or("?")
                })
                .collect();
            if let Some(phrase) = unmatched_phrase(&unmatched, UNMATCHED_NAMES_SHOWN) {
                ui.weak(phrase);
            }
        });
    }

    /// The column showing `name`, if this matrix has one.
    ///
    /// **Moved out of [`Self::draw_matrix`] on 2026-08-23**, under the standing
    /// rule for the five files Doug edits himself: *move a computation out
    /// before adding one in*. An opening-frame arm was added to the paint path
    /// that day, and this is what paid for it.
    ///
    /// It is worth this one rather than something else because it decides
    /// **identity**. `docs/identity-and-provenance.md` forbids letting a
    /// substring decide whether two names are the same variable, and
    /// [`crate::identifier_index::same_variable`] is what honours that — but
    /// while the call sat inside the painter, nothing could test that this view
    /// used it. Now something can.
    fn tracked_column(&self, name: &str) -> Option<usize> {
        self.unknown_names
            .iter()
            .position(|u| crate::identifier_index::same_variable(u, name))
    }

    fn draw_matrix(&self, ui: &mut egui::Ui, canvas: &mut Canvas, tracked: Option<&str>) {
        let label_headroom = 1.0_f32;
        let matrix_rect = egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(self.n_var as f32, self.n_eq as f32),
        );
        let bounds = egui::Rect::from_min_max(
            egui::pos2(matrix_rect.min.x, matrix_rect.min.y - label_headroom),
            matrix_rect.max,
        );
        let (response, view, painter) = canvas.show(ui, bounds);

        let visuals = ui.visuals();
        painter.rect_filled(
            view.to_screen_rect(matrix_rect),
            egui::CornerRadius::ZERO,
            visuals.extreme_bg_color,
        );

        let grid = visuals
            .weak_text_color()
            .gamma_multiply(crate::colors::GRID_ALPHA);
        view.draw_grid(&painter, self.n_var, self.n_eq, grid);

        let cell_color = crate::colors::INCIDENCE_CELL.gamma_multiply(0.4);

        // Draw incidence cells (dimmed — the animation overlays are the focus).
        for (row, cols) in self.rows.iter().enumerate() {
            for &col in cols {
                let rect = view.cell_rect(col, row).shrink(view.zoom() * 0.08);
                painter.rect_filled(rect, egui::CornerRadius::ZERO, cell_color);
            }
        }

        let Some(frame) = self.playback.current() else {
            return;
        };

        // Draw partial matching from the frame snapshot.
        let matched_color = crate::colors::MATCHED_MARKER;
        for (row, mc) in frame.match_eq.iter().enumerate() {
            if let Some(col) = *mc {
                let rect = view.cell_rect(col, row);
                // Matched cell: bright fill.
                painter.rect_filled(
                    rect.shrink(view.zoom() * 0.08),
                    egui::CornerRadius::ZERO,
                    crate::colors::INCIDENCE_CELL,
                );
                let center = rect.center();
                let r = view.zoom() * 0.2;
                painter.circle_filled(center, r, matched_color);
            }
        }

        // Highlight based on current step.
        match &frame.step {
            // The opening frame highlights nothing: the algorithm has not looked
            // at a row yet, and drawing a band would claim it had. The matrix
            // above is the whole content of this frame.
            MatchingStep::Start { .. } => {}
            MatchingStep::TryEquation(eq) => {
                // Highlight the entire row being attempted.
                let band = egui::Rect::from_min_size(
                    egui::pos2(0.0, *eq as f32),
                    egui::vec2(self.n_var as f32, 1.0),
                );
                painter.rect_filled(
                    view.to_screen_rect(band),
                    egui::CornerRadius::ZERO,
                    crate::colors::ANIM_EXPLORE.gamma_multiply(0.2),
                );
            }
            MatchingStep::Explore { eq, var } => {
                let rect = view.cell_rect(*var, *eq);
                painter.rect_filled(
                    rect,
                    egui::CornerRadius::ZERO,
                    crate::colors::ANIM_EXPLORE.gamma_multiply(0.5),
                );
                painter.rect_stroke(
                    rect,
                    egui::CornerRadius::ZERO,
                    egui::Stroke::new(2.0, crate::colors::ANIM_EXPLORE),
                    egui::StrokeKind::Inside,
                );
            }
            MatchingStep::FoundFree { eq, var } | MatchingStep::DisplaceOk { eq, var } => {
                let rect = view.cell_rect(*var, *eq);
                painter.rect_filled(
                    rect,
                    egui::CornerRadius::ZERO,
                    crate::colors::ANIM_PATH_FOUND.gamma_multiply(0.5),
                );
            }
            MatchingStep::DisplaceFail { eq, var } => {
                let rect = view.cell_rect(*var, *eq);
                painter.rect_filled(
                    rect,
                    egui::CornerRadius::ZERO,
                    crate::colors::ANIM_FAIL.gamma_multiply(0.5),
                );
            }
            MatchingStep::TryDisplace { eq, var, holder } => {
                // The contested cell.
                let rect = view.cell_rect(*var, *eq);
                painter.rect_stroke(
                    rect,
                    egui::CornerRadius::ZERO,
                    egui::Stroke::new(2.0, crate::colors::ANIM_EXPLORE),
                    egui::StrokeKind::Inside,
                );
                // Highlight the holder's row — it must find an alternative.
                let band = egui::Rect::from_min_size(
                    egui::pos2(0.0, *holder as f32),
                    egui::vec2(self.n_var as f32, 1.0),
                );
                painter.rect_filled(
                    view.to_screen_rect(band),
                    egui::CornerRadius::ZERO,
                    crate::colors::ANIM_EXPLORE.gamma_multiply(0.15),
                );
            }
            MatchingStep::Assign { eq, var } => {
                let rect = view.cell_rect(*var, *eq);
                painter.rect_filled(
                    rect,
                    egui::CornerRadius::ZERO,
                    crate::colors::ANIM_PATH_FOUND.gamma_multiply(0.6),
                );
                let center = rect.center();
                let r = view.zoom() * 0.25;
                painter.circle_filled(center, r, crate::colors::MATCHED_MARKER);
            }
            MatchingStep::EquationFailed(eq) => {
                let band = egui::Rect::from_min_size(
                    egui::pos2(0.0, *eq as f32),
                    egui::vec2(self.n_var as f32, 1.0),
                );
                painter.rect_filled(
                    view.to_screen_rect(band),
                    egui::CornerRadius::ZERO,
                    crate::colors::ANIM_FAIL.gamma_multiply(0.2),
                );
            }
            // **Two phases that bracket Kuhn's, new on 0.10.0.** A seeded row
            // was matched from a regular-family descriptor before the
            // augmenting search began; a claimed preference was moved onto its
            // causal target after the search finished. Both are drawn as a
            // settled assignment rather than as exploration, because that is
            // what they are — no path was searched for either.
            MatchingStep::SeedStructured { eq, var }
            | MatchingStep::ClaimedPreference { eq, var } => {
                let rect = view.cell_rect(*var, *eq);
                painter.rect_filled(
                    rect,
                    egui::CornerRadius::ZERO,
                    crate::colors::ANIM_PATH_FOUND.gamma_multiply(0.6),
                );
                let center = rect.center();
                let r = view.zoom() * 0.25;
                painter.circle_filled(center, r, crate::colors::MATCHED_MARKER);
            }
            // The preference could not be taken without reducing the
            // matching's cardinality, so the matching is **unchanged**. Drawn
            // on the cell that was refused, not on a row: nothing about the
            // equation failed, only this one target was unavailable.
            MatchingStep::PreferenceUnavailable { eq, var } => {
                let rect = view.cell_rect(*var, *eq);
                painter.rect_stroke(
                    rect,
                    egui::CornerRadius::ZERO,
                    egui::Stroke::new(2.0, crate::colors::ANIM_FAIL),
                    egui::StrokeKind::Inside,
                );
            }
        }

        // A let-chain: both conditions in one `if`, which is the shape rustc
        // and clippy want here. Reads as "if something is tracked AND this
        // matrix has a column for it" — the C/Java `if (a != null && ...)`,
        // except each binding is usable in the body.
        if let Some(name) = tracked
            && let Some(col) = self.tracked_column(name)
        {
            let band = egui::Rect::from_min_size(
                egui::pos2(col as f32, 0.0),
                egui::vec2(1.0, self.n_eq as f32),
            );
            painter.rect_filled(
                view.to_screen_rect(band),
                egui::CornerRadius::ZERO,
                crate::colors::TRACKED_FILL,
            );
        }

        if view.zoom() >= crate::LABEL_ZOOM_THRESHOLD {
            crate::draw_matrix_axis_labels(
                ui,
                &painter,
                view,
                &self.unknown_names,
                &self.equation_names,
                20,
                20,
            );
        }

        // Cell tooltip — shows full equation and variable names on hover.
        let hovered_cell = view.hovered_cell(&response, self.n_var, self.n_eq);
        if let Some((col, row)) = hovered_cell {
            response.on_hover_ui(|ui| {
                ui.label(egui::RichText::new("equation (row)").weak());
                ui.label(
                    egui::RichText::new(
                        self.equation_names
                            .get(row)
                            .map(String::as_str)
                            .unwrap_or("?"),
                    )
                    .monospace(),
                );
                ui.add_space(4.0);
                ui.label(egui::RichText::new("unknown (col)").weak());
                ui.label(
                    egui::RichText::new(
                        self.unknown_names
                            .get(col)
                            .map(String::as_str)
                            .unwrap_or("?"),
                    )
                    .monospace(),
                );
            });
        }
    }
}

/// How many unmatched equation names the status line lists before summarising.
///
/// Six fits the line at the usual window width. The number is not the point — what
/// matters is that the line says when it is showing six **of ninety-seven**.
const UNMATCHED_NAMES_SHOWN: usize = 6;

/// The *"still unmatched: …"* phrase, or `None` when everything is matched.
///
/// # Why the count is in the text
///
/// *Found 2026-11-04 by the capture suite's first photograph of this pane.* The list
/// was built with `.take(6)` and printed with no sign of truncation, so at frame 1 of
/// `Drivetrain` — where **all 97 equations are unmatched** — the line read
/// *"Matched 0 of 97 — still unmatched: <six names>"*. Six names, offered as the
/// answer to a question the reader has just been told is about ninety-seven.
///
/// That is load-bearing rather than cosmetic, and the code's own comment says why:
/// *"naming what is not matched yet is the useful half: a system that ends with an
/// unmatched equation is structurally singular."* A reader who counts the names to
/// find the rank deficiency gets six on any system deficient by more than six, and the
/// pane gave them no way to know.
///
/// The accessibility tree carries this label, so a headless test *could* have read it
/// — but no test asked, because nobody knew to ask. It took looking at the pane.
fn unmatched_phrase(names: &[&str], shown: usize) -> Option<String> {
    let total = names.len();
    if total == 0 {
        return None;
    }
    if total <= shown {
        return Some(format!("\u{2014} still unmatched: {}", names.join(", ")));
    }
    Some(format!(
        "\u{2014} still unmatched ({total}): {}, and {} more",
        names[..shown].join(", "),
        total - shown
    ))
}

fn step_description(
    step: &MatchingStep,
    eq_names: &[String],
    var_names: &[String],
) -> (&'static str, String) {
    let eq_name = |i: usize| eq_names.get(i).map(String::as_str).unwrap_or("?");
    let var_name = |i: usize| var_names.get(i).map(String::as_str).unwrap_or("?");
    match step {
        // Clapper board, matching every other opening frame in the project —
        // `34c22d56`: a start icon, not a finish flag.
        MatchingStep::Start {
            n_equations,
            n_unknowns,
        } => (
            "\u{1f3ac}",
            format!(
                "Starting point: {n_equations} equations, {n_unknowns} unknowns, nothing \
                 matched yet"
            ),
        ),
        MatchingStep::TryEquation(eq) => (
            "\u{1f50d}",
            format!(
                "Starting augmenting-path search for equation {}: {}",
                eq,
                eq_name(*eq)
            ),
        ),
        MatchingStep::Explore { eq, var } => (
            "\u{1f449}",
            format!(
                "Equation {} ({}) exploring variable {} ({})",
                eq,
                eq_name(*eq),
                var,
                var_name(*var),
            ),
        ),
        MatchingStep::FoundFree { eq, var } => (
            "\u{2705}",
            format!(
                "Variable {} ({}) is free — augmenting path found for eq {}",
                var,
                var_name(*var),
                eq,
            ),
        ),
        MatchingStep::TryDisplace { eq: _, var, holder } => (
            "\u{1f504}",
            format!(
                "Variable {} ({}) held by eq {} ({}). Can eq {} find an alternative?",
                var,
                var_name(*var),
                holder,
                eq_name(*holder),
                holder,
            ),
        ),
        MatchingStep::DisplaceOk { eq, var } => (
            "\u{2714}",
            format!(
                "Displacement succeeded — eq {} can take variable {} ({})",
                eq,
                var,
                var_name(*var),
            ),
        ),
        MatchingStep::DisplaceFail { eq, var } => (
            "\u{274c}",
            format!(
                "Displacement failed — variable {} ({}) cannot be freed for eq {}",
                var,
                var_name(*var),
                eq,
            ),
        ),
        MatchingStep::Assign { eq, var } => (
            "\u{1f517}",
            format!(
                "Matched: equation {} ({}) \u{2194} variable {} ({})",
                eq,
                eq_name(*eq),
                var,
                var_name(*var),
            ),
        ),
        MatchingStep::EquationFailed(eq) => (
            "\u{26a0}",
            format!(
                "Equation {} ({}) has no augmenting path — unmatched (rank deficiency)",
                eq,
                eq_name(*eq),
            ),
        ),
        // **Seeded before the augmenting search runs at all**, straight from a
        // regular equation family's descriptor. Said plainly, because a reader
        // watching Kuhn's would otherwise see rows already matched and have no
        // account of why.
        MatchingStep::SeedStructured { eq, var } => (
            "\u{1f9f1}",
            format!(
                "Seeded from a regular family before the search begins: equation {} ({}) \
                 \u{2194} variable {} ({})",
                eq,
                eq_name(*eq),
                var,
                var_name(*var),
            ),
        ),
        // **After the search, not during it.** Rumoca prefers an equation to
        // own its causal target where that costs no matching cardinality, so
        // this phase can move an already-matched row.
        MatchingStep::ClaimedPreference { eq, var } => (
            "\u{1f3af}",
            format!(
                "Equation {} ({}) takes its causal target {} ({}) — the matching stayed \
                 maximum, so the preference was free",
                eq,
                eq_name(*eq),
                var,
                var_name(*var),
            ),
        ),
        MatchingStep::PreferenceUnavailable { eq, var } => (
            "\u{1f6ab}",
            format!(
                "Equation {} ({}) cannot take its causal target {} ({}) without shrinking \
                 the matching — left as it was",
                eq,
                eq_name(*eq),
                var,
                var_name(*var),
            ),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A recorded animation over [`sample_report`]'s 3x3 system, from
    /// hand-built steps.
    ///
    /// **Replaces `MatchingAnimation::from_incidence`, which ran the algorithm.**
    /// Every test below asserts *playback* behaviour — not empty, starts paused
    /// at frame zero, reports no live session — and none of them cares where the
    /// steps came from. Running a real search to obtain them made these tests
    /// depend on the algorithm's output, so a change to Kuhn's tie-breaking
    /// could fail a test about a pause button.
    ///
    /// The steps are a plausible run over that matrix rather than the exact one
    /// Rumoca produces, and that is the point: a fixture is allowed to be a
    /// fixture. What they must satisfy is `from_captured_frames`' addressing
    /// check — `match_eq` as long as the matrix has rows, no column beyond its
    /// width — because that check is what stops frames from one system being
    /// drawn on another.
    fn recorded_animation() -> MatchingAnimation {
        let mat = IncidenceMatrix::from_report(&sample_report()).expect("3x3 sample matrix");
        let frame =
            |step: MatchingStep, match_eq: Vec<Option<usize>>| MatchingRecord { step, match_eq };
        let frames = vec![
            frame(
                MatchingStep::Start {
                    n_equations: 3,
                    n_unknowns: 3,
                },
                vec![None, None, None],
            ),
            frame(MatchingStep::TryEquation(0), vec![None, None, None]),
            frame(
                MatchingStep::Assign { eq: 0, var: 0 },
                vec![Some(0), None, None],
            ),
            frame(MatchingStep::TryEquation(1), vec![Some(0), None, None]),
            frame(
                MatchingStep::Assign { eq: 1, var: 1 },
                vec![Some(0), Some(1), None],
            ),
            frame(MatchingStep::TryEquation(2), vec![Some(0), Some(1), None]),
            frame(
                MatchingStep::Assign { eq: 2, var: 2 },
                vec![Some(0), Some(1), Some(2)],
            ),
        ];
        MatchingAnimation::from_captured_frames(&mat, &frames)
            .expect("the fixture steps address this matrix")
    }

    fn sample_report() -> serde_json::Value {
        json!({
            "matching": [
                { "equation": "f_x[0]", "unknown": "der(x)" },
                { "equation": "f_x[1]", "unknown": "y" },
                { "equation": "f_x[2]", "unknown": "z" },
            ],
            "blocks": [],
            "incidence": {
                "n_eq": 3,
                "n_var": 3,
                "unknown_names": ["der(x)", "y", "z"],
                "rows": [
                    { "equation": "f_x[0]", "unknowns": [0, 1] },
                    { "equation": "f_x[1]", "unknowns": [1, 2] },
                    { "equation": "f_x[2]", "unknowns": [0, 2] },
                ],
            }
        })
    }

    /// **Frames from another system are refused, not drawn.**
    ///
    /// The matching view renders under Structural *and* Index Reduction, and the
    /// incidence matrix follows whichever stage is showing — so on the Index
    /// Reduction tab the matrix is the **reduced** system while the captured frames
    /// are from the raw one. Drivetrain measures that gap at **97 equations versus
    /// 20**: the frames' indices would address rows that do not exist.
    ///
    /// Doug found this by asking whether the fallbacks were still replays
    /// (2026-08-04). They are — and checking the question surfaced a capture that was
    /// **worse than the replay it replaced**, because a re-derivation from the
    /// reduced matrix is at least self-consistent.
    ///
    /// Validated in the constructor rather than gated at the call site: a gate can be
    /// forgotten, and `match_eq`'s length *is* the equation count of the system that
    /// produced the frame.
    #[test]
    fn frames_from_a_different_system_fall_back_instead_of_misaddressing() {
        use rumoca_phase_structural::{MatchingRecord, MatchingStep};

        let mat = IncidenceMatrix::from_report(&sample_report()).unwrap();
        let wrong_size = mat.n_eq() + 5;
        let alien = vec![MatchingRecord {
            step: MatchingStep::TryEquation(0),
            match_eq: vec![None; wrong_size],
        }];

        assert!(
            MatchingAnimation::from_captured_frames(&mat, &alien).is_none(),
            "frames sized for a {wrong_size}-equation system must be refused for this \
             {}-equation matrix \u{2014} and refused means **no animation**, not a \
             re-derived one. Drawing a search the compiler did not run is the fiction \
             this capture exists to remove.",
            mat.n_eq(),
        );

        // Non-vacuity: a correctly sized capture IS used, so the check above is not
        // passing because the capture path never works.
        let fitting = vec![MatchingRecord {
            step: MatchingStep::TryEquation(0),
            match_eq: vec![None; mat.n_eq()],
        }];
        let kept = MatchingAnimation::from_captured_frames(&mat, &fitting)
            .expect("a capture that fits the matrix is used");
        assert_eq!(
            kept.position().1,
            1,
            "a one-frame capture that fits is played as-is"
        );
    }

    /// **The same equation count is not the same system**, and the length check alone
    /// cannot tell them apart.
    ///
    /// Tightened 2026-08-04, item 3 of the accuracy sweep. `match_eq.len() == n_eq`
    /// catches the mismatch that actually happened (raw frames against the reduced
    /// matrix, 97 equations vs 20) and is blind to two systems of *equal* size — which
    /// index reduction can produce, since demoting a state to algebraic moves a
    /// variable without necessarily changing the equation count.
    ///
    /// The variable indices inside the frames give a second constraint for free: a
    /// matched column that does not exist in this matrix proves the frames are not
    /// about it. Still a proxy rather than an identity, and strictly stronger.
    #[test]
    fn frames_that_match_in_size_but_address_missing_columns_are_refused() {
        use rumoca_phase_structural::{MatchingRecord, MatchingStep};

        let mat = IncidenceMatrix::from_report(&sample_report()).unwrap();
        // Right number of equations, but matched to a column beyond this matrix —
        // the shape of a capture from a wider system of the same height.
        let mut match_eq = vec![None; mat.n_eq()];
        match_eq[0] = Some(mat.n_var() + 3);
        let same_size_different_system = vec![MatchingRecord {
            step: MatchingStep::TryEquation(0),
            match_eq,
        }];

        assert!(
            MatchingAnimation::from_captured_frames(&mat, &same_size_different_system).is_none(),
            "these frames pass the equation-count check and still describe a system \
             with more unknowns than this matrix has \u{2014} drawing them would \
             address columns that do not exist",
        );
    }

    #[test]
    fn animation_from_incidence_produces_frames() {
        let anim = recorded_animation();
        assert!(!anim.is_empty());
        assert!(anim.position().1 > 3); // at least TryEquation + Explore + Assign per eq
    }

    /// A recorded animation must report that no live session is running, so the
    /// Debug button and the playback controls stay enabled.
    ///
    /// `LiveState::is_busy` gates both, so a view that reported `Running` over
    /// recorded frames would be permanently inert. That is the property this
    /// holds, and it is per-view on purpose: `alias_anim` and `ic_plan_anim`
    /// hardcode `Idle`, and `connection_anim`'s did too **after it had a live
    /// path**, which is the kind of stub this catches.
    ///
    /// **It does NOT guard `live_done`, and its previous doc claimed it did.**
    /// That version said the flag fed `live_debug_lifecycle`'s breakpoint-cleanup
    /// safety net and that being `false` here once made the net inert. The
    /// function is gone and the net was deliberately deleted (`docs/ideas.md`
    /// #74) — and measured 2026-08-20, this test **passes** with
    /// `Playback::recorded` flipped to `false`, because `live_state` decides
    /// from `is_live()` before it ever reads the flag. The one test that fails
    /// is `playback::tests::a_recorded_animation_reports_no_running_session`.
    #[test]
    fn recorded_animation_reports_no_live_session() {
        let anim = recorded_animation();
        assert_eq!(anim.live_state(false), crate::LiveState::Idle);
    }

    #[test]
    fn animation_starts_paused_at_frame_zero() {
        let anim = recorded_animation();
        assert_eq!(anim.position().0, 0);
    }

    /// **`"No matching trace available."` cannot be reached, and this pins why.**
    ///
    /// Written 2026-11-03 while trying to *cover* that message, which turned out to
    /// be impossible: `MatchingAnimation` has exactly two constructors and neither
    /// can produce one for which `is_empty()` is true.
    ///
    /// - `from_captured_frames` returns `None` on an empty frame list by design —
    ///   `app.rs` reports the absence instead of the pane re-deriving anything — so
    ///   every recorded animation that exists has at least one frame.
    /// - `start_live` builds `Playback::live`, and `is_empty()` requires
    ///   `live_rx.is_none()`. `sync_live` never drops the receiver, so a live
    ///   animation stays non-empty even after the session finishes with no frames.
    ///
    /// **So the branch is dead and the string is a claim the pane can never make.**
    /// That is the C17 class, which `every_absence_message_a_pane_shows_is_named_by_a_test`
    /// accepts rather than tests — on the reasoning that a test on such a string
    /// would pass forever regardless of what the pane does.
    ///
    /// **This test exists because that reasoning was only ever written in prose.**
    /// An unreachability argument is itself checkable, and if a later change gives
    /// either constructor an empty path, the message becomes reachable and untested
    /// at the same moment — the exact combination the budget is there to prevent.
    /// This fails then, and the fix at that point is a real rendering test.
    ///
    /// *Deliberately asserts the invariant rather than the label*: no `Harness`,
    /// because there is no state in which to render one.
    #[test]
    fn the_no_trace_label_stays_unreachable() {
        let mat = IncidenceMatrix::from_report(&sample_report()).expect("3x3 sample matrix");

        assert!(
            MatchingAnimation::from_captured_frames(&mat, &[]).is_none(),
            "an empty frame list must yield no animation at all \u{2014} if it now \
             yields one, `No matching trace available.` has become reachable and \
             needs a rendering test",
        );

        // A live session that finished without delivering a frame: the nearest thing
        // to empty that can actually exist.
        let (tx, rx) = std::sync::mpsc::channel::<MatchingRecord>();
        drop(tx);
        let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        let mut anim = MatchingAnimation {
            playback: Playback::live(rx, done, FRAME_INTERVAL),
            n_eq: mat.n_eq(),
            n_var: mat.n_var(),
            equation_names: mat.equation_texts().to_vec(),
            unknown_names: mat.unknown_names().to_vec(),
            rows: mat.rows().to_vec(),
        };
        anim.playback.sync_live();
        assert!(
            !anim.is_empty(),
            "a finished, frameless live session must still not read as empty \u{2014} \
             `is_empty()` requires the receiver to be gone, and `sync_live` does not \
             drop it. If that changed, the dead branch above is now live",
        );
    }

    #[test]
    fn step_description_produces_readable_text() {
        let eq_names = vec!["eq_a".to_string()];
        let var_names = vec!["x".to_string()];
        let (icon, desc) = step_description(&MatchingStep::TryEquation(0), &eq_names, &var_names);
        assert!(!icon.is_empty());
        assert!(desc.contains("eq_a"));
    }

    #[test]
    fn final_frame_has_complete_matching() {
        let anim = recorded_animation();
        let last = anim.playback.frames().last().unwrap();
        let matched = last.match_eq.iter().filter(|m| m.is_some()).count();
        assert_eq!(matched, 3, "3x3 system should have perfect matching");
    }

    // `live_mode_receives_all_frames` is retired rather than ported, and it
    // was testing two things that now live elsewhere.
    //
    // **Channel draining and live-cursor behaviour** are `Playback`'s, and
    // `playback::tests` covers them directly without a thread —
    // `live_frames_pull_the_cursor_to_the_newest_arrival` and
    // `a_busy_live_session_stops_playback_rather_than_pausing_it`.
    //
    // **Spawning and delivery** are no longer this type's at all: all three
    // animations go through `live_trace::spawn_structural_live`, so one test of
    // that helper covers what three animation-level tests used to, and covers
    // it where the code is. See `live_trace::tests`.
    //
    // Porting it would also have meant a real compile here, because
    // `start_live` takes a `Dae` now — the point of the redesign being that a
    // live session steps `sort` over the real system rather than re-running an
    // algorithm over the pane's rows.

    /// **Frame numbering is a contract that labs depend on.**
    ///
    /// `hrw://stage/Structural/MatchingAnim/frame/<n>` puts a *number* in a
    /// document, and `examples/frame_index` reads those numbers off this
    /// sequence. If the sequence shifts, every such link silently lands on the
    /// wrong step — the link checker cannot help, because a wrong-but-valid
    /// index resolves fine.
    ///
    /// So this pins the smallest real case. **A failure here is not
    /// necessarily a bug**: if the matching algorithm legitimately changes,
    /// the right response is to re-run `frame_index` and update the labs,
    /// which is `CLAUDE.md`'s guided-lab rule made mechanical.
    #[test]
    fn the_frame_sequence_labs_cite_is_pinned() {
        // **The real compile's steps, not a re-derivation over a stored
        // matrix.** This read the committed trace's incidence and re-ran
        // matching over it; the labs cite what HRW *shows*, which since the
        // v0.10.0 rebase is `inspect_structural_report`'s own sequence. Pinning
        // the re-run pinned something no reader ever saw.
        //
        // `SingleInertia` is MSL-free, so no library roots are loaded and this
        // stays out of the slow lane.
        let specimen =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("specimens/SingleInertia.mo");
        let compiled =
            crate::worker::compile_specimen(&specimen, Vec::new()).expect("SingleInertia compiles");
        let crate::worker::FromWorker::Compiled {
            matching_frames, ..
        } = compiled
        else {
            panic!("SingleInertia must reach structural analysis");
        };
        let steps: Vec<MatchingStep> = matching_frames
            .iter()
            .map(|record| record.step.clone())
            .collect();

        assert_eq!(
            steps.len(),
            9,
            "SingleInertia matches two equations with no displacement: an opening frame, \
             then try/explore/found/assign twice. A different count means the frame numbers \
             in every lab have moved",
        );
        // **The opening frame, added 2026-08-23**, is what shifted every index
        // below by one. It describes the problem before the search — the state
        // a replay needs in order to show what the search changed.
        assert!(
            matches!(
                steps[0],
                MatchingStep::Start {
                    n_equations: 2,
                    n_unknowns: 2
                }
            ),
            "frame 0 must be the starting point; found {:?}",
            steps[0],
        );
        // **The equations are processed 1 then 0, and that is 0.10.0 being
        // deliberate rather than arbitrary.** `kuhn_equation_order` runs
        // *untargeted* equations first and *targeted* ones last, so a targeted
        // equation gets the final augmenting-path opportunity to reclaim its
        // causal target. `explicit_derivative_preferences` gives equation 0 a
        // preference here, so it goes second.
        //
        // The old pinned sequence had 0 first, because it came from re-running
        // matching over the published matrix with no preferences supplied. Same
        // nine frames, different transversal order — which is exactly the
        // divergence between a re-run and the compile that this test now
        // cannot miss.
        //
        // **Every lab citing a frame number here has moved**, and that is the
        // mechanical response this test's doc prescribes: re-run
        // `examples/frame_index` and update the citations. See
        // `docs/updating-rumoca.md` step 6.
        assert!(
            matches!(steps[4], MatchingStep::Assign { eq: 1, var: 1 }),
            "frame 4 must be the assignment of the untargeted equation, which runs \
             first; found {:?}",
            steps[4],
        );
        assert!(
            matches!(steps[8], MatchingStep::Assign { eq: 0, var: 0 }),
            "frame 8 must be the assignment of the equation holding a causal \
             preference, which runs last; found {:?}",
            steps[8],
        );
    }

    /// **The tracked column is decided by identity, never by substring.**
    ///
    /// Could not be written before `tracked_column` left `draw_matrix`: the
    /// resolution sat inside the painter, so nothing could check which rule this
    /// view used to decide that two names are the same variable.
    ///
    /// The case that matters is the third assertion. `docs/identity-and-provenance.md`
    /// bars a substring from deciding identity, and `w` is a substring of every
    /// other name here — a `contains` would light the wrong column, and on screen
    /// a highlighted column looks equally deliberate whichever one it is.
    #[test]
    fn the_tracked_column_is_decided_by_identity_not_by_substring() {
        // Only the column *names* matter here, so the steps are a single
        // opening frame — enough to construct the animation, and nothing the
        // assertions below read.
        let mat = IncidenceMatrix::from_report(&serde_json::json!({
            "incidence": {
                "n_eq": 2,
                "n_var": 2,
                "unknown_names": ["inertia.w", "w"],
                "rows": [[0], [1]],
            },
        }))
        .expect("a two-by-two report parses");
        let anim = MatchingAnimation::from_captured_frames(
            &mat,
            &[MatchingRecord {
                step: MatchingStep::Start {
                    n_equations: 2,
                    n_unknowns: 2,
                },
                match_eq: vec![None, None],
            }],
        )
        .expect("one opening frame addresses this matrix");

        assert_eq!(anim.tracked_column("inertia.w"), Some(0));
        assert_eq!(
            anim.tracked_column("w"),
            Some(1),
            "the exact name, not the one containing it"
        );
        assert_eq!(
            anim.tracked_column("nowhere.v"),
            None,
            "absent means absent, not column 0"
        );

        // `der(x)` and `x` are the same variable — the one equivalence
        // `same_variable` deliberately allows, and the reason this is not a
        // string comparison either.
        assert_eq!(anim.tracked_column("der(w)"), Some(1));
    }

    /// **A truncated list must say what it is a sample of.**
    ///
    /// The regression test for the capture suite's first photograph of this pane: at
    /// frame 1 of `Drivetrain` all 97 equations are unmatched, and the line listed six
    /// with nothing to say so. A reader counting the names to find the rank deficiency
    /// — which is exactly what this line is for — would have read six.
    #[test]
    fn a_truncated_unmatched_list_says_how_many_it_is_hiding() {
        let all: Vec<&str> = (0..97).map(|_| "f_x[n]").collect();
        let phrase = unmatched_phrase(&all, UNMATCHED_NAMES_SHOWN).expect("97 is not none");
        assert!(
            phrase.contains("(97)"),
            "the total belongs in the line, because the pane is where a reader counts \
             the deficiency: {phrase}"
        );
        assert!(phrase.contains("and 91 more"), "{phrase}");

        // A list that fits is printed whole, with no count to read as a second number.
        let few = ["a", "b", "c"];
        let phrase = unmatched_phrase(&few, UNMATCHED_NAMES_SHOWN).expect("3 is not none");
        assert_eq!(phrase, "\u{2014} still unmatched: a, b, c");

        // Exactly at the cap is still whole: an "and 0 more" would be absurd.
        let six: Vec<&str> = (0..6).map(|_| "x").collect();
        let phrase = unmatched_phrase(&six, UNMATCHED_NAMES_SHOWN).expect("6 is not none");
        assert!(!phrase.contains("more"), "{phrase}");

        // Nothing unmatched is the end state, and says nothing at all — the
        // "Matched 97 of 97" beside it is the whole message.
        assert_eq!(unmatched_phrase(&[], UNMATCHED_NAMES_SHOWN), None);
    }
}
