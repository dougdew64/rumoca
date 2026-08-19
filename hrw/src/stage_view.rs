//! **Which sub-view of a stage is showing** — the four selector enums and their
//! display names.
//!
//! Lifted out of `app.rs` on 2026-08-19, the first item-by-item move of
//! [`docs/app-split-plan.md`](../docs/app-split-plan.md).
//!
//! # Why these twelve items, and not the whole of field group 9
//!
//! **They depend on nothing.** Four plain enums, their small impls, and four pure
//! `fn(enum) -> &'static str` helpers — no `App`, no `Viewport`, no worker. That makes
//! this the one part of the group that can leave without dragging anything behind it,
//! and the right size for the first move under a plan whose previous attempt cut a
//! 530-line span and produced 179 errors.
//!
//! `Viewport` itself stays in `app.rs` for now: `sub_view_name_for` takes a `&Viewport`,
//! so moving the names without the struct would only invert the coupling.
//!
//! # The vocabulary is deliberate, not derived
//!
//! The name functions are written out rather than produced from `Debug`, because these
//! strings are read by Claude and appear in `docs/context-assembly.md` — a `#[derive]`
//! rename would silently change the emitted vocabulary. The enums stay display-only.

use crate::canvas::Canvas;
use crate::worker::StageKind;

/// How to render the Structural / Index-reduction stages: the custom BLT
/// spy-plot, the incidence matrix, the reduction process
/// summary (Index reduction only), or the generic serde tree.
///
/// On the Index Reduction tab, comparative views (SpyPlot, Incidence) render
/// in a Before/After split; Summary, Animate, and Tree are full-width.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StructuralView {
    Summary,
    SpyPlot,
    Incidence,
    MatchingAnim,
    TarjanAnim,
    /// Replay of tearing an algebraic loop open. Shares this enum (rather than
    /// getting a stage of its own) because tearing is part of what the
    /// Structural stage reports — its output is already in `blocks`.
    TearingAnim,
    /// Reveal of the alias eliminations. Index Reduction only -- that is the
    /// stage whose report carries them.
    AliasAnim,
    Animate,
    Tree,
}

impl StructuralView {
    /// Every variant, so the noun/verb parity test can iterate without naming them.
    /// **Add new variants here** — that is what makes the omission loud instead of silent.
    #[cfg(test)]
    pub(crate) const ALL: &'static [StructuralView] = &[
        StructuralView::Summary,
        StructuralView::SpyPlot,
        StructuralView::Incidence,
        StructuralView::MatchingAnim,
        StructuralView::TarjanAnim,
        StructuralView::TearingAnim,
        StructuralView::AliasAnim,
        StructuralView::Animate,
        StructuralView::Tree,
    ];
}

/// Sub-tab selector for the Initialization stage: the IR tree, or a walk of the
/// initial-condition solve plan.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum InitView {
    #[default]
    Tree,
    IcPlan,
}

impl InitView {
    /// Every variant — see `StructuralView::ALL`.
    #[cfg(test)]
    pub(crate) const ALL: &'static [InitView] = &[InitView::Tree, InitView::IcPlan];
}

pub(crate) fn init_view_name(v: InitView) -> &'static str {
    match v {
        InitView::Tree => "Tree",
        InitView::IcPlan => "IcPlan",
    }
}

/// Sub-tab selector for the Flatten stage: readable equation sheet, source
/// traceability map, or the generic serde tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FlattenView {
    Equations,
    SourceMap,
    /// Replay of connection expansion (MLS §9) — where most of a flat model's
    /// equations come from.
    Connections,
    Tree,
}

impl FlattenView {
    /// Every variant — see `StructuralView::ALL`.
    #[cfg(test)]
    pub(crate) const ALL: &'static [FlattenView] = &[
        FlattenView::Equations,
        FlattenView::SourceMap,
        FlattenView::Connections,
        FlattenView::Tree,
    ];
}

/// Sub-tab selector for the Events stage: the IR tree, or a replay of `pre()`
/// lowering — where the `__pre__.x` slots the Events IR references get made.
///
/// Events hosts that replay even though the pass belongs to DAE construction:
/// the slots exist *because* of `when` equations, and this is the stage that
/// shows them. A separate `StageKind` would have to be wired into every
/// per-stage system (tabs, diffs, stage files, capture, notebook) to say
/// something that belongs beside what is already here.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum EventsView {
    #[default]
    Tree,
    PreLowering,
}

impl EventsView {
    /// Every variant — see `StructuralView::ALL`.
    #[cfg(test)]
    pub(crate) const ALL: &'static [EventsView] = &[EventsView::Tree, EventsView::PreLowering];
}

pub(crate) fn events_view_name(v: EventsView) -> &'static str {
    match v {
        EventsView::Tree => "Tree",
        EventsView::PreLowering => "PreLowering",
    }
}

/// Sub-view names for the capture's `view` section.
///
/// Written out rather than derived from `Debug` because these strings are read
/// by Claude and appear in `docs/context-assembly.md`; a `#[derive(Debug)]`
/// rename would silently change the emitted vocabulary. The enums themselves
/// stay display-only.
pub(crate) fn structural_view_name(v: StructuralView) -> &'static str {
    match v {
        StructuralView::Summary => "Summary",
        StructuralView::SpyPlot => "SpyPlot",
        StructuralView::Incidence => "Incidence",
        StructuralView::MatchingAnim => "MatchingAnim",
        StructuralView::TarjanAnim => "TarjanAnim",
        StructuralView::TearingAnim => "TearingAnim",
        StructuralView::AliasAnim => "AliasAnim",
        StructuralView::Animate => "Animate",
        StructuralView::Tree => "Tree",
    }
}

pub(crate) fn flatten_view_name(v: FlattenView) -> &'static str {
    match v {
        FlattenView::Equations => "EquationSheet",
        FlattenView::SourceMap => "SourceMap",
        FlattenView::Connections => "Connections",
        FlattenView::Tree => "Tree",
    }
}

/// The name of the sub-view the given stage is currently showing.
///
/// `None` for the stages that have only one view — a tree-only stage has no sub-tab,
/// and reporting an invented name for it would be a claim about UI that does not exist.
pub(crate) fn sub_view_name_for(stage: StageKind, viewport: &Viewport) -> Option<&'static str> {
    match stage {
        StageKind::Flatten => Some(flatten_view_name(viewport.flatten)),
        StageKind::Structural | StageKind::IndexReduction => {
            Some(structural_view_name(viewport.structural))
        }
        StageKind::Initialization => Some(init_view_name(viewport.init)),
        StageKind::Events => Some(events_view_name(viewport.events)),
        _ => None,
    }
}

/// **How the reader is looking at the current stage** — not what it holds.
///
/// Eleven fields with one thing in common: each records a *choice the reader
/// made about the view*, and none of them is derived from a compile. Which
/// sub-view is open, where each camera is panned, which row is highlighted.
///
/// # Why this is the right seam
///
/// It is the complement of [`StageViewCaches`], and the pair together are the
/// whole story of a stage view: **the caches are what was computed, the viewport
/// is what is being looked at.** They also have opposite lifetimes — a cache is
/// dropped whenever the stage changes, while a camera deliberately survives, so
/// returning to a view finds it where you left it.
///
/// Keeping them apart is what makes that difference visible. Together on `App`
/// they were eleven fields among eighty-five, and nothing said which ones a
/// stage switch was allowed to touch.
pub(crate) struct Viewport {
    /// Which sub-view is open on the Flatten stage.
    pub(crate) flatten: FlattenView,
    /// Which sub-view is open on the Events stage.
    pub(crate) events: EventsView,
    /// Which sub-view is open on the Initialization stage.
    pub(crate) init: InitView,
    /// Which sub-view is open on the report stages (Structural, Index Reduction).
    pub(crate) structural: StructuralView,
    /// Pan/zoom camera for the spy plot.
    pub(crate) spy: Canvas,
    /// Pan/zoom camera for the incidence matrix.
    pub(crate) incidence: Canvas,
    /// Pan/zoom camera for the matching animation.
    pub(crate) matching_anim: Canvas,
    /// Pan/zoom camera for the Tarjan animation.
    pub(crate) tarjan_anim: Canvas,
    /// Pan/zoom camera for the "before" incidence matrix in the Index Reduction
    /// split.
    pub(crate) before_incidence: Canvas,
    /// Equation-sheet row under the reader's attention, if any.
    pub(crate) highlighted_eq_row: Option<usize>,
    /// Source line under the reader's attention, if any.
    pub(crate) highlighted_source_line: Option<u32>,
    /// What `.hrw-bridge/view.json` was last written for, as `"Stage/SubView"`.
    ///
    /// **Here rather than on `App` deliberately**: this is viewport state, and `App`'s
    /// field count is ratcheted by
    /// `doc_citations::app_does_not_regrow_its_field_count`. A field that genuinely
    /// belongs to an existing grouping should go into it rather than spend the budget
    /// — which is the question the ratchet exists to force.
    ///
    /// **Change detection rather than interception.** The sub-view is set from several
    /// places — a sub-tab click, an `hrw://` link through `apply_sub_view`, and the
    /// default-sub-view logic that forces Summary on a singular report stage. Comparing
    /// once per frame catches all of them; hooking each one would miss whichever is
    /// added next.
    pub(crate) last_published_view: Option<String>,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            // **Not `FlattenView::default()`.** Equations is the sub-view worth
            // opening on, and `FlattenView` has no meaningful default of its own
            // — which is why `derive(Default)` does not compile here, and a good
            // thing: it forced these two choices to stay explicit.
            flatten: FlattenView::Equations,
            events: EventsView::default(),
            init: InitView::default(),
            structural: StructuralView::SpyPlot,
            // The bias lifts the fitted content slightly above centre, leaving
            // room for the labels drawn under each matrix.
            spy: Canvas::default().with_fit_vertical_bias(0.15),
            incidence: Canvas::default().with_fit_vertical_bias(0.15),
            matching_anim: Canvas::default().with_fit_vertical_bias(0.15),
            tarjan_anim: Canvas::default().with_fit_vertical_bias(0.15),
            before_incidence: Canvas::default().with_fit_vertical_bias(0.15),
            highlighted_eq_row: None,
            highlighted_source_line: None,
            last_published_view: None,
        }
    }
}
