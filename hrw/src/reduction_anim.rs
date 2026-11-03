//! Index-reduction animation — **disabled for the v0.10.0 rebase, awaiting a
//! re-model against Rumoca's own observation seam.**
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
//! It is not built here because a re-model cannot be done blind. Nothing in
//! `hrw` compiles mid-rebase, so a pane re-modelled now would get no reader
//! until every other file is finished — and this pane is lecture-facing. A
//! vocabulary chosen by reading `dae_transform/observation.rs` and never watched
//! running is how a pane ends up correct-looking and wrong. Doug's ruling is in
//! `DECISIONS.md`, 2026-11-02.
//!
//! The replacement is recruited by an `unbuilt:` tag in `docs/tech-debt.md`
//! naming `worker::index_reduction_from_observation`, so
//! `doc_citations::claims_of_absence_are_still_true` fails the moment that
//! symbol exists and the claim of absence stops being true.

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
    pub fn ui(&mut self, ui: &mut egui::Ui, _arming: bool, _debug_enabled: bool) -> bool {
        ui.label(egui::RichText::new("Index-reduction stepping is being rebuilt").strong());
        ui.add_space(4.0);
        ui.label(
            "Rumoca 0.10.0 replaced the ten-step dummy-derivative funnel this view \
             animated with two fixed-point lanes and a candidate/attempt/selection \
             protocol, and made the DAE immutable. The algorithm this view stepped \
             through is not the one the compiler runs any more.",
        );
        ui.add_space(4.0);
        ui.label(
            "The replacement reads Rumoca's own observation seam \
             (inspect_prepare_for_solve), which reports every candidate it tried and \
             why each was rejected — more than this view ever showed. Rumoca still \
             performs index reduction correctly; only this display is missing.",
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
