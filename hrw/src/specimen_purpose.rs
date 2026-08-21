//! **The specimen purpose pane** — the note saying why a specimen exists, and what
//! stands in its place when there is none.
//!
//! Lifted out of `frame_ui`'s Specimen left panel on 2026-08-21; the last *extraction*
//! [`docs/app-split-plan.md`](../docs/app-split-plan.md) names. It is the twin of
//! [`crate::specimen_source`], and that is exactly how the seam was found: the two are
//! the arms of one `match self.specimen_detail`, and one of them had been a function
//! call since 2026-08-19 while the other was still forty-eight lines of body. **The
//! odd member of a two-member list is the one that does not look like the other.**
//!
//! # `App` resolves and dispatches; this renders
//!
//! The same split [`crate::tour_panel::tour_prose_ui`] uses, and for the same reason:
//! `App::specimen_purpose_ui` reads the note, registers the markdown link hooks, calls
//! [`purpose_ui`], and drains the hooks — so an `hrw://` link inside a purpose note is
//! dispatched by the one caller that already dispatches the tour's, and this module
//! owns no policy about what following a link means. [`purpose_ui`] therefore returns
//! nothing and never sees an `App`.
//!
//! # Reading the note is a filesystem call in the paint path, and the memo is what
//! makes that safe
//!
//! [`purpose_note`] memoises **the misses as well as the hits**, which is the whole
//! point. Most specimens have a note; a model that does not would otherwise re-stat an
//! absent file on every frame, sixty times a second — the hazard
//! `SourceViewState::load_error` exists to close on the source side. `entry(…)
//! .or_insert_with(…)` gives the memo and the read in one expression, and
//! [`tests::a_missing_note_is_only_looked_for_once`] pins it by seeding the map with an
//! answer the disk could not have produced.
//!
//! # The key is the MODEL name, and it used to be typed as a path
//!
//! The memo was `HashMap<PathBuf, Option<String>>` keyed by `PathBuf::from(model)` — a
//! model name is not a path, and the type said it was. Changed to `String` while
//! writing [`purpose_note`]'s signature; recorded as a consequence of the move rather
//! than claimed as its return.
//!
//! **Why the key is the model and not the selected file:** the model name is only known
//! once the compile finishes, and that gap is a *state the pane must distinguish* — see
//! [`purpose_placeholder`], which returns three cases because of it.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use eframe::egui;

use crate::app::set_markdown_text_sizes;

/// Where a specimen's purpose note lives.
///
/// One place, so the pane and any test that reaches for a real note agree about the
/// layout. `docs/specimen-notebook/<Model>/purpose.md`, beside that model's generated
/// `trace/`.
fn note_path(model: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("docs/specimen-notebook")
        .join(model)
        .join("purpose.md")
}

/// The purpose note for `model`, read from disk once and remembered — **including when
/// there is none**.
///
/// `None` when no model is compiled yet, and equally when the compiled model has no
/// note; the pane does not distinguish those two here because
/// [`purpose_placeholder`] does, from the same two inputs.
pub(crate) fn purpose_note<'a>(
    notes: &'a mut HashMap<String, Option<String>>,
    model: Option<&str>,
) -> Option<&'a str> {
    let name = model?;
    // `or_insert_with` and not `or_insert`: the read must not happen on a hit.
    notes
        .entry(name.to_owned())
        .or_insert_with(|| std::fs::read_to_string(note_path(name)).ok())
        .as_deref()
}

/// The **Purpose** half of the Specimen left panel: the note if there is one, and an
/// account of the absence if there is not.
///
/// `note` is what [`purpose_note`] resolved; `model` and `selected` are passed even
/// though the note came from the first of them, because they are what tells *why* a
/// note is missing. They are read only on the empty path, so a specimen that has a
/// note pays nothing for them.
pub(crate) fn purpose_ui(
    ui: &mut egui::Ui,
    cache: &mut egui_commonmark::CommonMarkCache,
    note: Option<&str>,
    model: Option<&str>,
    selected: Option<&Path>,
) {
    match note {
        Some(text) => {
            egui::ScrollArea::vertical()
                .id_salt("purpose")
                .show(ui, |ui| {
                    set_markdown_text_sizes(ui);
                    egui_commonmark::CommonMarkViewer::new().show(ui, cache, text);
                });
        }
        None => {
            for line in purpose_placeholder(model, selected) {
                ui.weak(line);
            }
        }
    }
}

/// What the Purpose tab shows when there is no note to render.
///
/// Extracted from the view so the wording is testable. Both messages it replaced were
/// wrong, and Doug found both by using the app (2026-07-29):
///
/// 1. They said **"narrative"**, a term retired when the narratives were. A renamed
///    concept leaves its old name in the strings nobody greps for.
/// 2. Worse, selecting a *second* specimen showed **"Select a specimen"** — advising
///    Doug to do the thing he had just done. The note is keyed on the *model* name,
///    which stays `None` until compilation finishes, so a selected-but-compiling
///    specimen fell through to the nothing-selected arm. That was a **missing state**,
///    not merely bad wording, which is why this returns three cases and not two.
fn purpose_placeholder(model: Option<&str>, selected: Option<&Path>) -> Vec<String> {
    match (model, selected) {
        // Compiled, and this model has no note. Saying *where* one would live makes
        // the absence actionable instead of a dead end.
        (Some(name), _) => vec![
            format!("No purpose note for {name}."),
            format!(
                "One would live at docs/specimen-notebook/{name}/purpose.md \u{2014} why the \
                 specimen exists, and which questions it has answered.",
            ),
        ],
        // Selected, still compiling. Name the file so the wait is legible.
        (None, Some(path)) => {
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("specimen");
            vec![
                format!("Compiling {stem}\u{2026}"),
                "Its purpose note appears once the model name is known.".to_owned(),
            ]
        }
        (None, None) => vec!["Select a specimen to see its purpose.".to_owned()],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    /// A specimen that really does have a note in the corpus.
    const REAL_SPECIMEN: &str = "RcCircuit";
    /// A model name no specimen will ever have, so its note is genuinely absent.
    const NO_SUCH_MODEL: &str = "NoSuchSpecimen";

    /// Render the pane once and return the harness, so a test can ask what reached the
    /// screen.
    ///
    /// **Sized like a panel, not like a widget** (`420×900` — the Specimen left panel is
    /// a third of HRW's width). A clipped widget stays in the accessibility tree while
    /// behaving as though it is not there, which four earlier modules lost time to.
    fn draw(note: Option<&'static str>, model: Option<&'static str>) -> Harness<'static, ()> {
        let mut cache = egui_commonmark::CommonMarkCache::default();
        let selected = model.map(|m| PathBuf::from(format!("{m}.mo")));
        let mut h = Harness::builder()
            .with_size(egui::Vec2::new(420.0, 900.0))
            .build_ui(move |ui| {
                purpose_ui(ui, &mut cache, note, model, selected.as_deref());
            });
        h.run_steps(2);
        h
    }

    /// **A note on disk is what the pane shows, and the placeholder stays away.**
    ///
    /// The two arms are exclusive by construction, which is worth asserting rather than
    /// reading: the failure this shape produces elsewhere in HRW is *beside* becoming
    /// *instead of* (see [`crate::artifact_pane`]), and here the reverse — an "absence"
    /// message printed under a note that rendered fine — would read as a broken corpus.
    #[test]
    fn a_note_is_rendered_and_the_placeholder_is_not() {
        let h = draw(
            Some("# Why RcCircuit exists\n\nA first-order circuit."),
            Some(REAL_SPECIMEN),
        );
        assert!(
            h.query_by_label_contains("Why RcCircuit exists").is_some(),
            "the note's own heading must reach the screen",
        );
        assert!(
            h.query_by_label_contains("No purpose note").is_none(),
            "and nothing may claim the note is missing while it is on screen",
        );
    }

    /// **No note reaches the reader as an address, not as a blank.**
    #[test]
    fn a_missing_note_renders_the_placeholder() {
        let h = draw(None, Some(NO_SUCH_MODEL));
        assert!(
            h.query_by_label_contains("No purpose note").is_some(),
            "the absence must be stated",
        );
        assert!(
            h.query_by_label_contains("docs/specimen-notebook/NoSuchSpecimen/purpose.md")
                .is_some(),
            "and it must say where a note would live, or the reader has a dead end",
        );
    }

    /// The Purpose tab's placeholder never says "narrative", and never tells Doug to
    /// select a specimen he has already selected.
    ///
    /// Both were real bugs he hit by using the app (2026-07-29). The second is the
    /// interesting one: it was a **missing state**, not a typo. The note is keyed on the
    /// model name, which is unknown until compilation finishes, so selecting a second
    /// specimen briefly showed "Select a specimen to see its narrative" — advice to do
    /// the thing just done.
    #[test]
    fn the_purpose_placeholder_fits_the_actual_state() {
        let path = Path::new("/x/CapacitorLoop.mo");

        // Compiled, no note: says so, and says where one would go.
        let compiled = purpose_placeholder(Some("CapacitorLoop"), Some(path));
        assert!(
            compiled[0].contains("No purpose note for CapacitorLoop"),
            "{compiled:?}"
        );
        assert!(
            compiled
                .iter()
                .any(|l| l.contains("docs/specimen-notebook/CapacitorLoop/purpose.md")),
            "the absence must be actionable: {compiled:?}",
        );

        // Selected but still compiling: names the file, does NOT ask for a selection.
        let compiling = purpose_placeholder(None, Some(path));
        assert!(
            compiling[0].contains("Compiling CapacitorLoop"),
            "{compiling:?}"
        );
        assert!(
            !compiling.iter().any(|l| l.contains("Select a specimen")),
            "must not advise selecting a specimen that IS selected: {compiling:?}",
        );

        // Genuinely nothing selected: the advice is now correct.
        let idle = purpose_placeholder(None, None);
        assert!(idle[0].contains("Select a specimen"), "{idle:?}");

        // No state mentions the retired term.
        for lines in [compiled, compiling, idle] {
            for l in lines {
                assert!(
                    !l.to_lowercase().contains("narrative"),
                    "retired term leaked into user-visible text: {l}",
                );
            }
        }
    }

    /// **The pane's path and the corpus on disk agree.**
    ///
    /// [`note_path`] is built from three literals, and nothing else in HRW would notice
    /// if one of them drifted — the notebook checks walk the directory themselves. The
    /// pane would simply go quiet and report every specimen as unnoted.
    #[test]
    fn a_real_specimens_note_is_found_where_the_pane_looks() {
        let mut notes = HashMap::new();
        let note = purpose_note(&mut notes, Some(REAL_SPECIMEN));
        assert!(
            note.is_some_and(|t| t.contains(REAL_SPECIMEN)),
            "{REAL_SPECIMEN} has a purpose.md in the corpus and the pane must find it",
        );
    }

    /// **A miss is remembered, so an unnoted model does not re-stat an absent file every
    /// frame.**
    ///
    /// Checked by seeding the memo with an answer the disk *cannot* produce — there is
    /// no such note — and asserting it comes back. A test that only counted entries would
    /// pass against a version that re-read and overwrote on every call.
    #[test]
    fn a_missing_note_is_only_looked_for_once() {
        let mut notes = HashMap::new();
        assert!(
            purpose_note(&mut notes, Some(NO_SUCH_MODEL)).is_none(),
            "precondition: {NO_SUCH_MODEL} really has no note on disk",
        );
        assert!(
            notes.contains_key(NO_SUCH_MODEL),
            "the miss itself must be recorded, or the read repeats forever",
        );

        notes.insert(NO_SUCH_MODEL.to_owned(), Some("from the memo".to_owned()));
        assert_eq!(
            purpose_note(&mut notes, Some(NO_SUCH_MODEL)),
            Some("from the memo"),
            "a second call must answer from the memo rather than from the filesystem",
        );
    }

    /// No compiled model means no lookup at all — not a lookup under an empty name.
    #[test]
    fn no_model_reads_nothing() {
        let mut notes = HashMap::new();
        assert!(purpose_note(&mut notes, None).is_none());
        assert!(
            notes.is_empty(),
            "an absent model must not leave an entry behind: {notes:?}",
        );
    }
}
