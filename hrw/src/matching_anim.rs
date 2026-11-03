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

use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use eframe::egui;

use rumoca_phase_structural::LiveTrace;
use rumoca_phase_structural::matching::{MatchingFrame, MatchingStep, maximum_matching_with_trace};

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
    playback: Playback<MatchingFrame>,
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
    /// The distinction from [`Self::from_incidence`] is provenance, not output.
    /// `from_incidence` re-runs matching when the tab is opened; because matching is
    /// deterministic the two agree, but the animation then replays a search that
    /// produced nothing, while the blocks on screen came from a different execution
    /// nobody watched.
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
        frames: &[rumoca_phase_structural::matching::MatchingFrame],
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

    /// **Re-runs matching from scratch. Test-only, and enforced by the compiler.**
    ///
    /// Gated `#[cfg(test)]` on 2026-08-04. It has had no production caller since the
    /// capture scopes landed, and what *guarded* that was
    /// `doc_citations::no_animation_re_runs_a_phase_by_default` — a **grep of
    /// `app.rs` for the string `from_incidence`**. That guard works and is fragile in
    /// the specific way this project forbids elsewhere: a re-export, an alias, a
    /// wrapper, or moving the call into any other module defeats it silently, and
    /// nothing decides identity by substring here (`docs/identity-and-provenance.md`).
    ///
    /// Compiling it out of the binary makes the claim structural instead: the UI
    /// cannot call this, because in a non-test build **it does not exist.** The
    /// remaining value of the grep is that it also asserts the capture path *is*
    /// reached, which a `cfg` cannot say.
    ///
    /// Kept rather than deleted because
    /// `worker::tests::hrw_rederived_matching_matches_rumocas_report` cross-checks
    /// HRW's own matching against Rumoca's report — a genuine fidelity test that
    /// needs an independent implementation to compare against.
    #[cfg(test)]
    pub fn from_incidence(mat: &IncidenceMatrix) -> Self {
        let eq_vars: Vec<HashSet<usize>> = mat
            .rows()
            .iter()
            .map(|cols| cols.iter().copied().collect())
            .collect();
        let result = maximum_matching_with_trace(mat.n_eq(), mat.n_var(), &eq_vars, None);
        Self {
            // `Playback::recorded` sets `live_done` true, which is the honest
            // answer to "is a live session still running?" for a recorded
            // animation. **The second half of this comment was wrong and is
            // corrected here**: it said `live_debug_lifecycle` relies on the
            // flag to release a breakpoint armed for a session that is never
            // coming. That function is gone, that safety net was deliberately
            // deleted (`docs/ideas.md` #74), and nothing reads the flag on a
            // recorded playback at all — see `Playback::recorded`.
            playback: Playback::recorded(result.frames, FRAME_INTERVAL),
            n_eq: mat.n_eq(),
            n_var: mat.n_var(),
            equation_names: mat.equation_texts().to_vec(),
            unknown_names: mat.unknown_names().to_vec(),
            rows: mat.rows().to_vec(),
        }
    }

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
    pub fn start_live(mat: &IncidenceMatrix, frame_delay: std::time::Duration) -> Option<Self> {
        let (lt, rx) = LiveTrace::new();
        let lt = lt.with_frame_delay(frame_delay);
        let done = Arc::new(AtomicBool::new(false));
        let done_for_thread = Arc::clone(&done);

        let eq_vars: Vec<HashSet<usize>> = mat
            .rows()
            .iter()
            .map(|cols| cols.iter().copied().collect())
            .collect();
        let n_eq = mat.n_eq();
        let n_var = mat.n_var();

        thread::Builder::new()
            .name("matching-debug".to_owned())
            .spawn(move || {
                lt.wait_for_debugger();
                // Where HRW's `LiveTrace` meets the phase's observer callback.
                // The phase crate never learns `LiveTrace` exists — see
                // `rumoca_core::FrameObserver`.
                let observe = |f: &MatchingFrame| lt.push(f.clone());
                maximum_matching_with_trace(n_eq, n_var, &eq_vars, Some(&observe));
                done_for_thread.store(true, Ordering::Release);
            })
            .ok()?;

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
    fn render_running_state(&self, ui: &mut egui::Ui, frame: &MatchingFrame) {
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
                .take(6)
                .collect();
            if !unmatched.is_empty() {
                ui.weak(format!(
                    "\u{2014} still unmatched: {}",
                    unmatched.join(", ")
                ));
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
        use rumoca_phase_structural::matching::{MatchingFrame, MatchingStep};

        let mat = IncidenceMatrix::from_report(&sample_report()).unwrap();
        let wrong_size = mat.n_eq() + 5;
        let alien = vec![MatchingFrame {
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
        let fitting = vec![MatchingFrame {
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
        use rumoca_phase_structural::matching::{MatchingFrame, MatchingStep};

        let mat = IncidenceMatrix::from_report(&sample_report()).unwrap();
        // Right number of equations, but matched to a column beyond this matrix —
        // the shape of a capture from a wider system of the same height.
        let mut match_eq = vec![None; mat.n_eq()];
        match_eq[0] = Some(mat.n_var() + 3);
        let same_size_different_system = vec![MatchingFrame {
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
        let mat = IncidenceMatrix::from_report(&sample_report()).unwrap();
        let anim = MatchingAnimation::from_incidence(&mat);
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
        let mat = IncidenceMatrix::from_report(&sample_report()).unwrap();
        let anim = MatchingAnimation::from_incidence(&mat);
        assert_eq!(anim.live_state(false), crate::LiveState::Idle);
    }

    #[test]
    fn animation_starts_paused_at_frame_zero() {
        let mat = IncidenceMatrix::from_report(&sample_report()).unwrap();
        let anim = MatchingAnimation::from_incidence(&mat);
        assert_eq!(anim.position().0, 0);
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
        let mat = IncidenceMatrix::from_report(&sample_report()).unwrap();
        let anim = MatchingAnimation::from_incidence(&mat);
        let last = anim.playback.frames().last().unwrap();
        let matched = last.match_eq.iter().filter(|m| m.is_some()).count();
        assert_eq!(matched, 3, "3x3 system should have perfect matching");
    }

    #[test]
    fn live_mode_receives_all_frames() {
        let mat = IncidenceMatrix::from_report(&sample_report()).unwrap();
        // The unarmed delay deliberately: no breakpoint exists in a test, and
        // the stepped delay would make this sleep for seconds.
        let mut anim = MatchingAnimation::start_live(&mat, crate::live_frame_delay(false))
            .expect("spawn thread");
        for _ in 0..100 {
            if anim.live_state(false) == crate::LiveState::Finished {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        anim.playback.sync_live();
        assert!(!anim.playback.frames().is_empty());
        assert_eq!(anim.live_state(false), crate::LiveState::Finished);
        let last = anim.playback.frames().last().unwrap();
        let matched = last.match_eq.iter().filter(|m| m.is_some()).count();
        assert_eq!(matched, 3, "live mode should reach same final matching");
    }

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
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("docs/specimen-notebook/SingleInertia/trace/structural.json");
        let text = std::fs::read_to_string(&path).expect("SingleInertia has a committed trace");
        let report: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");
        let mat = crate::incidence_view::IncidenceMatrix::from_report(&report)
            .expect("SingleInertia's report carries an incidence matrix");
        let anim = MatchingAnimation::from_incidence(&mat);
        let steps = anim.steps();

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
        // Frame 4 is the one a lab would cite for "der(phi) gets matched".
        assert!(
            matches!(steps[4], MatchingStep::Assign { eq: 0, var: 0 }),
            "frame 4 must be the assignment of the first unknown; found {:?}",
            steps[4],
        );
        assert!(
            matches!(steps[8], MatchingStep::Assign { eq: 1, var: 1 }),
            "frame 8 must be the assignment of the second; found {:?}",
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
        let anim = MatchingAnimation::from_incidence(
            &IncidenceMatrix::from_report(&serde_json::json!({
                "incidence": {
                    "n_eq": 2,
                    "n_var": 2,
                    "unknown_names": ["inertia.w", "w"],
                    "rows": [[0], [1]],
                },
            }))
            .expect("a two-by-two report parses"),
        );

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
}
