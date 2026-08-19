//! **What was computed for the current stage** — the memoised heavy views.
//!
//! Lifted out of `app.rs` on 2026-08-19, third move of
//! [`docs/app-split-plan.md`](../docs/app-split-plan.md) (field group 12).
//!
//! # The distinction this module exists to hold up
//!
//! **A cache is what was computed; a viewport is what is being looked at**
//! ([`crate::stage_view`]). They have opposite lifetimes — everything here is dropped
//! the moment the stage changes, while a camera deliberately survives so that returning
//! to a view finds it where you left it. Keeping them in separate modules is what makes
//! that difference impossible to overlook while editing either one.
//!
//! # Why the double `Option`
//!
//! `Option<Option<T>>` is not an accident: the outer says **whether the view has been
//! built for this stage yet**, the inner says **whether it could be built at all**. A
//! model with no coupled blocks has a spy plot that is legitimately `None`, and
//! collapsing the two would make "not computed yet" and "computed, and there is nothing
//! to show" the same value — which is the absence-versus-silence distinction this
//! project treats as a defect class.

use crate::worker::StageKind;
use crate::{
    alias_anim, connection_anim, ic_plan_anim, incidence_view, matching_anim, reduction_anim,
    reduction_view, spyplot, tarjan_anim, tearing_anim,
};
/// Views derived from a **stage's report**, all valid for exactly one stage.
///
/// # Why these eleven and not the other nine `cached_*` fields
///
/// Measured 2026-08-02 before the extraction, because the plan assumed all
/// twenty caches shared one lifetime and **they do not**. Three families:
///
/// - **These** — rebuilt whenever the displayed report stage changes, and
///   again on every new compile.
/// - **Compile outputs** (`cached_flat`, `cached_dae`, `cached_equation_sheet`)
///   — named "cached" but never invalidated, because they are *results*
///   assigned from a finished compile.
/// - **Self-keying memos** (`cached_purpose_notes` keyed by model,
///   `cached_tour` keyed by mtime, `cached_source` per specimen) — each already
///   carries whatever tells it when it is stale.
///
/// Folding all twenty into one bag would have cleared the memos on every stage
/// change, which is a behaviour change disguised as a refactor.
///
/// # What the struct buys
///
/// The eleven were listed **by hand in two places** — once at compile
/// completion, once on stage change — so a new view cache had to be added to
/// both or it would silently serve a previous stage's data. `reset_for` makes
/// that impossible: it assigns a whole `Self`, so a field added tomorrow is
/// covered by construction. **The bug class is removed rather than tested for.**
#[derive(Default)]
pub(crate) struct StageViewCaches {
    /// The stage these views were built from. `None` means "nothing built yet".
    pub(crate) built_for: Option<StageKind>,
    // Outer `Option` is cache state (None = not yet computed); inner `Option` is
    // the parse result (None = the report held no data for this view).
    pub(crate) spy_plot: Option<Option<spyplot::Plot>>,
    pub(crate) incidence: Option<Option<incidence_view::IncidenceMatrix>>,
    pub(crate) reduction: Option<Option<reduction_view::ReductionView>>,
    pub(crate) matching_anim: Option<Option<matching_anim::MatchingAnimation>>,
    pub(crate) tarjan_anim: Option<Option<tarjan_anim::TarjanAnimation>>,
    pub(crate) tearing_anim: Option<Option<tearing_anim::TearingAnimation>>,
    pub(crate) alias_anim: Option<Option<alias_anim::AliasAnimation>>,
    pub(crate) ic_plan_anim: Option<Option<ic_plan_anim::IcPlanAnimation>>,
    pub(crate) connection_anim: Option<Option<connection_anim::ConnectionAnimation>>,
    pub(crate) reduction_anim: Option<Option<reduction_anim::ReductionAnimation>>,
    pub(crate) before_incidence: Option<Option<incidence_view::IncidenceMatrix>>,
}

impl StageViewCaches {
    /// Drop every view unless it was already built for `stage`.
    ///
    /// Returns `true` when it actually reset, so the caller can do the rest of
    /// its stage-change work — picking a default sub-view — only when the stage
    /// really changed.
    pub(crate) fn reset_for(&mut self, stage: StageKind) -> bool {
        if self.built_for == Some(stage) {
            return false;
        }
        // **Whole-struct assignment, deliberately.** Clearing field by field is
        // what produced two lists to keep in step; this cannot go out of date.
        *self = Self {
            built_for: Some(stage),
            ..Self::default()
        };
        true
    }

    /// Drop every view **and** the key, so the next frame rebuilds from scratch.
    ///
    /// Used when a compile lands: the reports themselves changed, so even the
    /// stage that is already showing must be rebuilt.
    pub(crate) fn invalidate_all(&mut self) {
        *self = Self::default();
    }
}
