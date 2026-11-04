//! Index-reduction animation — **the stepping is disabled; the stage's data is not.**
//! Awaiting a public per-event seam, not a vocabulary.
//!
//! # What was here, and why it is gone rather than ported
//!
//! This module stepped through a **ten-step dummy-derivative funnel**:
//! `demote_exact_alias_component_states`,
//! `reduce_constrained_dummy_derivatives`,
//! `index_reduce_missing_state_derivatives` and seven more, each mutating a
//! `&mut Dae` in place, with HRW's instrumentation emitting an
//! `IndexReductionFrame` per step.
//!
//! **Rumoca 0.10.0 does not run that algorithm.** `dae_prepare` is gone.
//! Reduction is now two fixed-point *lanes* — Direct and Holonomic — driven by
//! a candidate / attempt / selection protocol, and a `Dae` is immutable, so
//! there is no in-place mutation to watch. Every name this module animated has
//! stopped existing, and the shape it drew is not the shape of the search.
//!
//! Porting it was therefore not available: there is nothing to port it *to*.
//! Keeping the old renderer behind a feature flag would be worse than deleting
//! it, because the one thing a reader could then do is look at a correct-looking
//! animation of an algorithm the compiler does not perform. The implementation
//! is in git history, which is the right place for the record of an algorithm
//! Rumoca used to run.
//!
//! # What replaces it, and why not now
//!
//! `rumoca_phase_structural::inspect_prepare_for_solve` returns the real
//! reduction plus an owned `ReductionReport`: per-round residue, unmatched
//! equations and unknowns, candidate counts per proof boundary, **every attempt
//! with its outcome including why it was rejected**, each selection with
//! residue before and after, pristine retries, the stop reason, and a snapshot
//! of the stalled DAE when preparation fails. That is strictly richer than the
//! funnel frames were, and it observes the compile that actually happened
//! instead of a re-run.
//!
//! # The stage's re-model has landed; this view's has not, and the reason changed
//!
//! **Since 2026-11-04 `worker::index_reduction_from_observation` publishes the whole
//! search** — every round, candidate group, attempt, refusal and selection from the
//! one reduction the compile performed. So the Index Reduction tab is no longer
//! empty, and the thing missing here is narrower than it was: **stepping**, not the
//! data.
//!
//! It stayed a placeholder through that commit for a reason worth recording, because
//! it is not "no time left". **A live session needs a per-event seam, and Rumoca's
//! does not exist publicly.** `ReductionObserver` and `ReductionEvent` are
//! `pub(super)` by design; `inspect_prepare_for_solve` is the crate-external surface
//! and it hands back an owned report *after the run finishes*. HRW's live stepping
//! works by pacing a real algorithm against a breakpoint, so feeding a finished
//! report into the channel on a timer would be a **recording wearing a live label** —
//! the one fiction this repository has already removed twice.
//!
//! That makes the observer seam the real instrumentation candidate for this view,
//! and `CLAUDE.md` is why it matters rather than being optional:
//! `PendingLiveDebug::Reduction` is named there as one of the four live paths that
//! re-run a phase that still exists and **must not be "fixed"** by deletion. A
//! public observer keeps that path honest; without one, the Debug button on this tab
//! is an affordance that arms a breakpoint and reports nothing.
//!
//! Doug's ruling that re-models land one at a time, where each can be watched
//! running, is in `DECISIONS.md`, 2026-11-02.

use eframe::egui;
use serde_json::Value;

use crate::LiveState;
use crate::playback::Animated;

/// A placeholder that reports its own absence.
///
/// Keeps the surface `app.rs` drives — `from_frames`, `start_live`, `is_empty`,
/// `ui`, and [`Animated`] — so the pane still exists, still appears in the stage
/// roster, and still answers an `hrw://stage/.../frame/<n>` link with a truthful
/// "no". A missing pane would read as a phase that produced nothing, which is a
/// false claim about the compiler: Rumoca runs index reduction on 0.10.0
/// perfectly well.
#[derive(Default)]
pub struct ReductionAnimation {
    /// Nothing. Named rather than a unit struct so the type can gain real state
    /// when the re-model lands without every construction site changing shape.
    _awaiting_remodel: (),
}

impl ReductionAnimation {
    /// **Takes no frames, because none are produced any more.**
    ///
    /// The old signature was `from_frames(Vec<IndexReductionFrame>)`. Keeping a
    /// frames parameter that is always empty would leave the plumbing looking
    /// alive, and a later session would reasonably conclude the frames were
    /// simply not arriving — a bug hunt with no bug in it.
    #[must_use]
    pub fn disabled() -> Self {
        Self::default()
    }

    /// Always `None`: there is no live session to start.
    ///
    /// Live stepping drove the funnel through HRW's `LiveTrace` and the
    /// `*_with_trace` entry points, none of which exist on 0.10.0. Returning
    /// `None` is the same answer this gave when a thread could not be spawned,
    /// so the caller's existing fallback path is already correct.
    #[must_use]
    pub fn start_live(
        _dae: &rumoca_ir_dae::Dae,
        _frame_delay: std::time::Duration,
    ) -> Option<Self> {
        None
    }

    /// Always empty — there are no frames.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        true
    }

    /// Draw the explanation. Returns `false`: nothing here can be armed.
    ///
    /// **Re-worded 2026-11-04, when the stage's re-model landed.** The previous text
    /// described the replacement as future work — *"the replacement reads Rumoca's
    /// own observation seam … more than this view ever showed"* — which stopped being
    /// true the moment `worker::index_reduction_from_observation` shipped. A pane
    /// that describes a present thing in the future tense sends the reader looking
    /// for something they already have.
    pub fn ui(&mut self, ui: &mut egui::Ui, _arming: bool, _debug_enabled: bool) -> bool {
        ui.label(egui::RichText::new("The search is published; the stepping is not").strong());
        ui.add_space(4.0);
        ui.label(
            "Rumoca 0.10.0 replaced the ten-step dummy-derivative funnel this view \
             animated with two fixed-point lanes and a candidate/attempt/selection \
             protocol, and made the DAE immutable. The algorithm this view stepped \
             through is not the one the compiler runs any more.",
        );
        ui.add_space(4.0);
        ui.label(
            "What the compiler did run is on this tab already: every round with its \
             residue and unmatched names, every candidate group, every attempt with \
             its outcome and why it was refused, and each selection. Read it in the \
             IR tree — the stage publishes it from Rumoca's own observation of the \
             one reduction this compile performed.",
        );
        ui.add_space(4.0);
        ui.label(
            "Stepping through it one record at a time is what is still missing here, \
             and live stepping needs something HRW cannot reach: Rumoca's \
             ReductionObserver is private, so there is no per-event seam to pace \
             against a breakpoint. inspect_prepare_for_solve hands back the whole \
             report after the fact, and replaying that as though it were live would \
             be a recording wearing a live label.",
        );
        false
    }
}

impl Animated for ReductionAnimation {
    fn which(&self) -> &'static str {
        "reduction"
    }

    fn position(&self) -> (usize, usize) {
        (0, 0)
    }

    fn live_state(&self, _arming: bool) -> LiveState {
        LiveState::Idle
    }

    /// `None`, which the trait already defines as a real state rather than a
    /// failure: "no frame under the cursor".
    fn current_frame_context(&self) -> Option<Value> {
        None
    }

    /// Always `false`, so a `frame/<n>` link reports that the frame does not
    /// exist instead of landing somewhere plausible.
    fn seek(&mut self, _n: usize) -> bool {
        false
    }
}
