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
