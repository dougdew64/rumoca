//! `pre()`-lowering animation — **disabled for the v0.10.0 rebase, awaiting a
//! re-model against DAE construction.**
//!
//! # What was here, and why it is gone rather than ported
//!
//! This module stepped through `lower_pre_operator(&mut dae::Dae)`: a distinct
//! pass that walked a finished DAE and rewrote every `pre(x)` occurrence,
//! emitting a `PreLoweringStep` per rewrite. Live stepping re-ran it through
//! `to_dae_with_options_traced`.
//!
//! **Rumoca 0.10.0 has no such pass.** `pre()` is lowered *during* construction,
//! as one arm of `construction::expression`'s builtin dispatch beside `der`,
//! `edge`, `previous` and `hold` — `lower_pre` is called while the expression
//! arena is being built, not afterwards over a completed DAE. There is no
//! `lower_pre_operator`, no `to_dae_with_options_traced`, and no
//! `PreLoweringStep`. A `Dae` is also immutable now, so a pass that rewrote one
//! in place could not exist even in principle.
//!
//! So this is a **re-model, not a port**: the phenomenon is still there and is
//! still worth watching, but it happens somewhere else and in a different
//! shape. Keeping the old renderer behind a flag would be worse than deleting
//! it, because the one thing a reader could then do is look at a
//! correct-looking animation of a pass the compiler does not run. The
//! implementation is in git history, which is the right place for the record of
//! an algorithm Rumoca used to have.
//!
//! # What a replacement would read, and why not now
//!
//! Construction is where the information is, and it is richer than the old
//! pass's: `lower_pre` sits beside the other history operators, so the same
//! seam would show `pre`, `edge`, `change` and `previous` being lowered
//! together — which is the comparison that actually explains MLS §3.7.5's
//! discrete-time semantics. The old pass could only ever show `pre`.
//!
//! It is not built here because a re-model cannot be done blind. Nothing in
//! `hrw` compiles mid-rebase, so a pane re-modelled now would get no reader
//! until every other file is finished, and a vocabulary chosen by reading
//! `construction/expression.rs` and never watched running is how a pane ends up
//! correct-looking and wrong. Doug's ruling — ports reach the build, re-models
//! land afterwards one at a time — is in `DECISIONS.md`, 2026-11-02, and he
//! extended it to every surface needing a re-model.
//!
//! The replacement is recruited by an `unbuilt:` tag in `docs/tech-debt.md`
//! naming `worker::pre_lowering_from_construction`, so
//! `doc_citations::claims_of_absence_are_still_true` fails the moment that
//! symbol exists and the claim of absence stops being true.

use eframe::egui;
use serde_json::Value;

use crate::LiveState;
use crate::playback::Animated;

/// A placeholder that reports its own absence.
///
/// Keeps the surface `app.rs` drives — `disabled`, `start_live`, `is_empty`,
/// `ui`, and [`Animated`] — so the pane still exists, still appears in the
/// stage roster, and still answers an `hrw://stage/.../frame/<n>` link with a
/// truthful "no". A missing pane would read as a phase that produced nothing,
/// which is a false claim about the compiler: Rumoca lowers `pre()` on 0.10.0
/// perfectly well, just not in a pass of its own.
#[derive(Default)]
pub struct PreLoweringAnimation {
    /// Nothing. Named rather than a unit struct so the type can gain real state
    /// when the re-model lands without every construction site changing shape.
    _awaiting_remodel: (),
}

impl PreLoweringAnimation {
    /// **Takes no frames, because none are produced any more.**
    ///
    /// The old signature was `from_frames(Vec<PreLoweringFrame>)`. Keeping a
    /// frames parameter that is always empty would leave the plumbing looking
    /// alive, and a later session would reasonably conclude the frames were
    /// simply not arriving — a bug hunt with no bug in it.
    #[must_use]
    pub fn disabled() -> Self {
        Self::default()
    }

    /// Always `None`: there is no live session to start.
    ///
    /// Live stepping re-ran `to_dae_with_options_traced` over the flat model,
    /// which does not exist on 0.10.0. Returning `None` is the same answer this
    /// gave when a thread could not be spawned, so the caller's existing
    /// fallback path is already correct.
    #[must_use]
    pub fn start_live(
        _flat: rumoca_ir_flat::Model,
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
        ui.label(egui::RichText::new("pre() lowering stepping is being rebuilt").strong());
        ui.add_space(4.0);
        ui.label(
            "Rumoca 0.10.0 removed the separate pass this view animated. pre() is now \
             lowered while the DAE's expression arena is being built, as one arm of the \
             builtin dispatch beside der, edge, change and previous — so there is no \
             pass over a finished DAE to step through, and a DAE can no longer be \
             mutated in place at all.",
        );
        ui.add_space(4.0);
        ui.label(
            "The replacement reads construction itself, which can show pre() beside the \
             other history operators rather than alone — the comparison that explains \
             Modelica's discrete-time semantics. Rumoca still lowers pre() correctly; \
             only this display is missing.",
        );
        false
    }
}

impl Animated for PreLoweringAnimation {
    fn which(&self) -> &'static str {
        "pre_lowering"
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
