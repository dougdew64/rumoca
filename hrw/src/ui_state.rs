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
    /// Guided lab: LHS shows the lab document, RHS shows stage tabs.
    #[default]
    Lab,
    /// Specimen exploration: LHS shows specimen list + purpose note, RHS shows stage tabs.
    Specimen,
    /// Debugger-assisted: LHS hidden, stage tabs fill the window. VS Code alongside.
    Debug,
}

impl UiMode {
    /// Every variant, for tests that must not miss one.
    pub const ALL: [UiMode; 3] = [UiMode::Lab, UiMode::Specimen, UiMode::Debug];

    /// **The one spelling of a mode's name.** Both the View menu's label and
    /// `focus.json`'s `ui_mode` field come from here, so they cannot disagree.
    ///
    /// They used to be two independent string literals — `app.rs`'s
    /// `view_context` matched the enum to produce the bridge string, and the
    /// menu carried its own `"Lab"` — which is a silent-drift shape this
    /// repository has been bitten by repeatedly: nothing compared them, and
    /// `ui_mode` is read by Claude on every prompt rather than by anyone
    /// looking at the screen. Renaming a mode would have changed the button
    /// and left the reported value stale, or the reverse.
    ///
    /// Added 2026-09-01, before the lab → lab rename, precisely because that
    /// rename touches both sites. **Do not reintroduce a bare mode-name string
    /// literal**; `every_ui_mode_has_one_spelling` is the guard.
    pub fn label(self) -> &'static str {
        match self {
            UiMode::Lab => "Lab",
            UiMode::Specimen => "Specimen",
            UiMode::Debug => "Debug",
        }
    }
}

#[cfg(test)]
mod tests_ui_mode_label {
    use super::UiMode;

    /// `UiMode::ALL` lists every variant, and every label is distinct and
    /// non-empty. The count is asserted rather than derived, following
    /// `stage_kind_all_is_exhaustive`: adding a mode means wiring it into the
    /// View menu *and* into what `focus.json` reports, so the count failing is
    /// the reminder.
    #[test]
    fn every_ui_mode_has_a_distinct_label() {
        assert_eq!(
            UiMode::ALL.len(),
            3,
            "UiMode::ALL should list every variant (Lab, Specimen, Debug)"
        );
        let names: Vec<&str> = UiMode::ALL.iter().map(|m| m.label()).collect();
        for name in &names {
            assert!(!name.is_empty(), "a UiMode label is empty");
        }
        let unique: std::collections::HashSet<&&str> = names.iter().collect();
        assert_eq!(unique.len(), names.len(), "duplicate UiMode labels in ALL");
    }

    /// **The must-fire half: no mode name may be written as a bare literal in
    /// `app.rs`.** The test above cannot catch the failure that matters — two
    /// call sites agreeing today and drifting tomorrow — because it only reads
    /// `label()`. This reads the source instead, and fails if the View menu or
    /// `view_context` goes back to spelling a mode out by hand.
    ///
    /// **Silence must be a failure**, so it also asserts the call sites are
    /// still there: a test that passes because `app.rs` stopped mentioning
    /// modes at all would be reporting nothing.
    ///
    /// Reverted-and-checked 2026-09-01: restoring either literal fails this by
    /// name. It is the guard that made the lab → lab rename safe to start,
    /// since `ui_mode` is read by Claude on every prompt and by nobody looking
    /// at the screen.
    #[test]
    fn every_ui_mode_has_one_spelling() {
        let src = include_str!("app.rs");
        for mode in UiMode::ALL {
            let literal = format!("\"{}\"", mode.label());
            let bare = format!("UiMode::{:?}, {literal}", mode);
            assert!(
                !src.contains(&bare),
                "app.rs spells the {:?} mode name as a bare literal ({literal}). \
                 Use `UiMode::{:?}.label()` — the View menu and focus.json's \
                 `ui_mode` must have one source, or a rename changes one and \
                 leaves the other stale.",
                mode,
                mode
            );
            let via_label = format!("UiMode::{:?}.label()", mode);
            assert!(
                src.contains(&via_label),
                "app.rs no longer calls `UiMode::{:?}.label()`. If the View menu \
                 was restructured, point this test at the new call site — a pass \
                 here must mean the labels are shared, not that nothing uses them.",
                mode
            );
        }
        assert!(
            src.contains("ui_mode: self.ui_mode.label()"),
            "`view_context` no longer takes `ui_mode` from `UiMode::label()`; \
             focus.json's mode string can now drift from the button Doug clicks."
        );
    }
}
