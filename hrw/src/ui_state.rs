//! **Where the reader is, at the coarsest grain** — which mode is showing, which
//! detail of a specimen, and the go-to-definition stack.
//!
//! Lifted out of `app.rs` on 2026-08-19, fifth move of
//! [`docs/app-split-plan.md`](../docs/app-split-plan.md).
//!
//! # Why these three together
//!
//! They are the three types the very first extraction attempt collided with: cutting a
//! span between two viewport items swallowed them, because `app.rs`'s types are
//! interleaved rather than clustered. Moving them deliberately is the tidy end of that
//! accident.
//!
//! They also belong together on their own merits. `UiMode` says which pane is showing,
//! `SpecimenDetail` says which face of a specimen, and `NavEntry` records where a jump
//! came from — three answers to *where is the reader*, at a coarser grain than
//! [`crate::stage_view`], which answers it within a single stage.

use std::collections::BTreeMap;

use crate::worker::DefInfo;

/// What the bottom two-thirds of the Specimen mode LHS shows.
///
/// `Debug` so the crash log can name it — the derived variant name is exactly
/// the right thing to record, and hand-writing a second mapping would let the
/// two drift.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum SpecimenDetail {
    /// The specimen's Modelica source text.
    #[default]
    Source,
    /// The specimen's purpose note from
    /// `docs/specimen-notebook/<Model>/purpose.md`. Renamed from `narrative.md`
    /// 2026-07-29 when the stage-by-stage prose was retired — a file called
    /// `narrative.md` containing no narrative is the kind of stale signal that
    /// retirement was meant to remove. See `docs/ideas.md` #42.
    /// The specimen's purpose note (`purpose.md`). Was `Narrative` until
    /// 2026-07-29; the stage-by-stage prose it named is retired.
    Purpose,
}

/// One level of "go to definition" navigation: a class extracted from the
/// resolved tree, shown in the same generic tree the specimen stages use.
///
/// Navigation forms a stack: clicking "Go to definition" pushes a `NavEntry`,
/// "Back" pops one, and "Specimen" clears the stack entirely. Each entry
/// carries its own `def_index` so the tree inspector can resolve DefIds
/// (numeric cross-references) to human-readable class names within that class.
pub(crate) struct NavEntry {
    pub(crate) name: String,
    /// The serde_json representation of this class's IR — the same format every
    /// stage uses, so the generic tree inspector renders it without any special
    /// logic.
    pub(crate) value: serde_json::Value,
    /// Maps numeric DefIds (compiler-internal identifiers) to their resolved
    /// class names, enabling the tree view to show "type_def_id: 27579 ->
    /// model Resistor" rather than a bare number.
    pub(crate) def_index: BTreeMap<u64, DefInfo>,
}

/// Which left-panel content is active. Determines both what occupies the LHS
/// of the window and whether the LHS is visible at all.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UiMode {
    /// Guided tour: LHS shows the tour document, RHS shows stage tabs.
    #[default]
    Tour,
    /// Specimen exploration: LHS shows specimen list + purpose note, RHS shows stage tabs.
    Specimen,
    /// Debugger-assisted: LHS hidden, stage tabs fill the window. VS Code alongside.
    Debug,
}
