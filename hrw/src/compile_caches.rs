//! **What the last compile produced** — the replays that no stage's report backs.
//!
//! Split out of [`crate::stage_caches`] on 2026-08-20, after the question the
//! live-debug deduplication left behind (*"decide `pre_lowering_anim`'s cache
//! lifetime"*, [`docs/app-split-plan.md`](../docs/app-split-plan.md)) turned out to
//! have a different answer than the question assumed.
//!
//! # The distinction, which is now three ways rather than two
//!
//! [`crate::stage_view`] holds **what is being looked at** (a camera, which survives
//! deliberately). [`crate::stage_caches`] holds **what was computed for the current
//! stage** (dropped the moment the displayed report changes). This holds **what the
//! compile itself observed** — captured algorithm frames, and one report that is read
//! from a fixed stage rather than the current one. Its contents change when, and only
//! when, a compile lands.
//!
//! # Why these four were in the wrong bag
//!
//! Each of them appears on **exactly one stage**, so its input never varies with
//! `self.stage`:
//!
//! | view | built from | shown on |
//! |---|---|---|
//! | `reduction_anim` | `frames.index_reduction` | Index Reduction |
//! | `connection_anim` | `frames.connection` | Flatten ▸ Connections |
//! | `ic_plan_anim` | `stages.initialization` — *a fixed stage, not the current one* | Initialization ▸ IC Plan |
//! | `pre_lowering_anim` | `frames.pre_lowering` | Events ▸ pre() lowering |
//!
//! Three of the four sat in `StageViewCaches`, whose own doc promises *"views derived
//! from a stage's report, all valid for exactly one stage"* — **false of all three**,
//! and the fourth had been given the right lifetime by hand and left as the odd one
//! out. The remaining eight fields there really do read `stages.get(self.stage)` or
//! branch on it, so moving these makes that sentence true rather than aspirational.
//!
//! # The behaviour this changes, and why it was not a design
//!
//! `StageViewCaches::reset_for` is called from **one place** —
//! [`crate::report_sub_view::report_sub_view_row_ui`] — and that row is drawn only on
//! the Structural and Index Reduction stages. So `built_for` never held any other
//! stage, and the rule actually in force was not *"a replay restarts when you come
//! back to it"* but ***"a replay restarts if you happened to pass through a report
//! stage in between."*** Events → Flatten → Events dropped nothing; Flatten →
//! Structural → Flatten dropped the connection replay. Nobody would design that, which
//! is the evidence it was a filing accident rather than an intention.
//!
//! Since [`crate::playback::Playback::recorded`] starts at `cursor: 0, playing: false`,
//! the cost was **losing your place**: paused on frame 12 of the index-reduction
//! replay, clicking Structural to compare and clicking back put you on frame 0. Now all
//! four behave the way the `pre()` replay already did — and the way a camera does.
//!
//! **It also removes a way to strand a live session.** A live replay owns the receiving
//! end of the trace channel; dropping it mid-run left the algorithm thread pushing into
//! a closed channel, with the armed breakpoint released by nothing (see
//! `live_debug_poll` on the absent safety net). A stage click can no longer do that.
//!
//! # Why no key, where `StageViewCaches` has `built_for`
//!
//! There is nothing to key on. A stage-keyed cache needs to know whether what it holds
//! still matches what is on screen; these match until `App` is handed a new compile, at
//! which point the caller drops the whole struct. Adding a key would invite the reader
//! to think some *other* event could invalidate one of these, and none can.

use crate::{connection_anim, ic_plan_anim, pre_lowering_anim, reduction_anim};

/// Replays derived from a **compile**, all valid until the next one.
///
/// # What the struct buys
///
/// The same thing [`crate::stage_caches::StageViewCaches`] buys, for the same reason:
/// [`Self::invalidate_all`] assigns a whole `Self`, so a replay added tomorrow is
/// cleared by construction instead of by someone remembering. Before this existed,
/// `pre_lowering_anim` was cleared by a hand-written line at compile completion — one
/// item on a list of one, which is exactly how a list of two goes wrong.
///
/// # Why the double `Option`
///
/// As in `StageViewCaches`: the outer says **whether the view has been built for this
/// compile yet**, the inner **whether it could be built at all**. A model with no
/// `connect` equations has a connection replay that is legitimately `None`, and
/// collapsing the two would make "not computed yet" and "computed, and there is nothing
/// to show" the same value — the absence-versus-silence distinction this project treats
/// as a defect class.
#[derive(Default)]
pub(crate) struct CompileViewCaches {
    // Outer `Option` is cache state (None = not yet computed); inner `Option` is
    // the build result (None = this compile produced nothing for this view).
    pub(crate) reduction_anim: Option<Option<reduction_anim::ReductionAnimation>>,
    pub(crate) connection_anim: Option<Option<connection_anim::ConnectionAnimation>>,
    pub(crate) ic_plan_anim: Option<Option<ic_plan_anim::IcPlanAnimation>>,
    pub(crate) pre_lowering_anim: Option<Option<pre_lowering_anim::PreLoweringAnimation>>,
}

impl CompileViewCaches {
    /// Drop every replay, so the next frame rebuilds from the new compile's frames.
    ///
    /// Named to match `StageViewCaches::invalidate_all` so the two calls at compile
    /// completion read as one act, which is what they are.
    pub(crate) fn invalidate_all(&mut self) {
        *self = Self::default();
    }
}
