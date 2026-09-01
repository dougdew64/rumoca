//! The **model list**: the left panel's browser over every model HRW can open.
//!
//! Three sources behind one widget — curated `specimens/`, scratch
//! `.hrw-bridge/specimens/`, and the 2,626-row MSL corpus — with one filter
//! narrowing all of them.
//!
//! # Why this is a module and the other panes are not, yet
//!
//! Split out of `app.rs` on 2026-08-02, and it is the **first** pane that could
//! be. A module boundary is only free once a pane stops reaching into `App`:
//! this one owns its state ([`ModelListState`]) and *reports* what the reader
//! did ([`ModelListOutcome`]) instead of acting on it, so moving the file
//! changed no visibility except making the struct `pub(crate)`.
//!
//! Panes that still take `&mut App` cannot move without widening `App`'s fields,
//! which would undo the encapsulation the extraction just bought. The order is
//! therefore *narrow first, move second* — never the reverse.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use eframe::egui;

use crate::app::{DEFAULT_SPECIMEN_DIR, SCRATCH_POLL_INTERVAL};
use crate::app::{read_purpose, section_header};
use crate::{bridge, survey};

/// The glyph prefixing a **scratch specimen** in the list.
///
/// **U+2731 HEAVY ASTERISK, and the choice was measured rather than picked.** This was
/// U+270E (LOWER RIGHT PENCIL) until 2026-08-30, when Doug reported the three
/// scratch rows as *"boxes"* — a pencil is in none of egui's four bundled fonts, so it
/// rendered as tofu and said nothing at all. `App::hrw_font_definitions` already
/// widens every font into both families, and that cannot help for a codepoint no
/// bundled font contains.
///
/// [`tests_absence::the_scratch_marker_glyph_actually_renders`] asks those exact fonts
/// whether this glyph exists, so the next marker cannot regress to a box unnoticed.
const SCRATCH_MARKER: char = '\u{2731}';

/// What the model list wants the app to do next.
///
/// The pane renders a list; **loading a specimen resets stages, the log, the
/// context bar and the source view**, none of which it owns. Reporting the intent
/// instead of acting keeps the list a list, and is what lets it be rendered in a
/// test without constructing an `App`.
#[derive(Default)]
pub(crate) struct ModelListOutcome {
    pub(crate) nav: Option<ModelListNav>,
    /// The whole specimen was pointed at, from the row's context menu. A separate
    /// field rather than a `ModelListNav` variant because it can accompany a
    /// navigation in the same frame, exactly as it could before the split.
    pub(crate) point_at_specimen: bool,
}

/// What a row's context menu was asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RowAction {
    /// Re-run the compiler on this row.
    Recompile,
    /// Make this model the subject of the next question.
    PointAt,
}

/// **The row context menu, defined once for every list.**
///
/// Doug, 2026-08-04: *"unlike the correctly-working items in the HRW specimens
/// list, the items in the MSL Corpus list do not provide right-click context
/// menus. The context menus for MSL Corpus items should be consistent with the
/// context menus for HRW specimen items."*
///
/// **Extracted rather than copied.** "Consistent with" is a property that decays
/// the moment there are two copies — the next person to add a verb, change a
/// wording or fix an enablement rule updates the list they happened to be looking
/// at. One definition makes consistency structural instead of remembered, which is
/// the same reasoning that put the stage vocabulary behind `slug()`/`from_slug`
/// after it drifted into four hand-written copies.
///
/// The verbs are identical; only what a row *does* with them differs, so the
/// caller supplies the enablement and interprets the result.
fn row_context_menu(
    resp: &egui::Response,
    can_recompile: bool,
    can_capture: bool,
    what: &str,
) -> Option<RowAction> {
    let mut action = None;
    resp.context_menu(|ui| {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);

        let btn = ui.add_enabled(can_recompile, egui::Button::new("\u{1f504} Recompile"));
        let btn = if can_recompile {
            btn.on_hover_text(
                "Re-run the compiler on this model (e.g. to hit an armed breakpoint).",
            )
        } else {
            btn.on_disabled_hover_text(format!("Left-click to load this {what} first."))
        };
        if btn.clicked() {
            action = Some(RowAction::Recompile);
            ui.close();
        }

        ui.separator();

        let btn = ui.add_enabled(can_capture, egui::Button::new("\u{1f3af} Point at"));
        let btn = if can_capture {
            btn.on_hover_text(
                "Make the whole model the subject of your next question, then ask in \
                 the chat.",
            )
        } else {
            btn.on_disabled_hover_text(format!(
                "Left-click to load & compile this {what} first, then point at it.",
            ))
        };
        if btn.clicked() {
            action = Some(RowAction::PointAt);
            ui.close();
        }
    });
    action
}

pub(crate) enum ModelListNav {
    /// A corpus row: open by qualified name.
    OpenLibrary(String),
    /// "Recompile" from the context menu — always reloads.
    Reload(std::path::PathBuf),
    /// A row was clicked. May already be selected; only the caller knows.
    Select(std::path::PathBuf),
}

/// Everything the left panel's **model list** owns.
///
/// Three sources behind one widget — curated `specimens/`, scratch
/// `.hrw-bridge/specimens/`, and the 2,626-row MSL corpus — plus the filter that
/// narrows all three and the bookkeeping that keeps them fresh.
///
/// # Why these ten and not the whole left panel
///
/// Measured 2026-08-02: each of these is read only by the list's own rendering
/// and by the three methods that maintain it (`rescan`, `poll_scratch_specimens`,
/// `corpus_rows`). **None is read by a stage view, the chat, or the source pane.**
/// `selected` is the boundary — the list *produces* it and everything else
/// consumes it, so it stays on `App` where the measurement found it genuinely
/// shared.
///
/// # Why a struct rather than ten fields on `App`
///
/// It is the difference between "extract a function" and "extract state"
/// (`docs/ui-pause-plan.md`). A `model_list_ui(&mut self, ..)` could still reach
/// any of `App`'s fields, so nothing would become independently testable and the
/// next defect would hide exactly as well as the last three did — the corpus
/// hidden without a filter, the vacuous startup test, the shadowing notice.
pub(crate) struct ModelListState {
    /// Directory scanned for curated `.mo` specimens.
    pub(crate) dir: String,
    /// Every specimen currently listed, curated and scratch together.
    pub(crate) files: Vec<PathBuf>,
    /// One-line `// purpose:` hint per specimen, read at rescan, so the list
    /// reads as an index of what each specimen teaches.
    pub(crate) purposes: HashMap<PathBuf, String>,
    /// Which entries of [`Self::files`] came from the gitignored scratch
    /// directory (ideas #42). A set rather than a flag on the path, so `files`
    /// stays a plain `Vec<PathBuf>` everything else can use.
    pub(crate) scratch: HashSet<PathBuf>,
    /// Scratch specimens skipped because a curated specimen owns the name.
    /// Reported rather than silently resolved: loading a different model than
    /// the name says would have Claude reason confidently about source Doug is
    /// not looking at.
    pub(crate) shadowed: Vec<String>,
    /// Why the scan failed, if it did.
    pub(crate) scan_error: Option<String>,
    /// The MSL corpus, read from the survey on first use.
    ///
    /// **The survey is the corpus definition** — the same rule `fidelity_msl`
    /// follows. Re-enumerating models from the session would be a second
    /// definition of "which models exist", and the two would drift the moment
    /// MSL moved.
    pub(crate) corpus: Option<Vec<survey::SurveyRow>>,
    /// Filter text. **Narrows every source**, so a curated specimen and a corpus
    /// model are found the same way.
    pub(crate) filter: String,
    /// A scratch specimen appeared since the list was last drawn.
    ///
    /// Holds the HRW section open, which is otherwise collapsed at startup. A
    /// scratch specimen is written *for the question being asked right now*, so
    /// it is the one file that must not need a click to be seen. Sticky for the
    /// session rather than one frame: a section that sprang open and shut again
    /// would read as a glitch.
    pub(crate) scratch_arrived: bool,
    /// When the scratch directory was last polled.
    pub(crate) polled_at: Option<std::time::Instant>,
}

impl ModelListState {
    /// Re-read the specimen directory, rebuilding the file list and scanning
    /// each `.mo` file for its `// purpose:` comment. Called at startup and
    /// when the user changes the directory in Settings.
    /// The left panel's **model list**: curated specimens, scratch specimens, and
    /// the MSL corpus, behind one filter.
    ///
    /// Lifted out of `frame_ui` on 2026-08-02. It was ~270 lines inline, in the
    /// region edited three times on 2026-08-01 alone -- and two of those edits
    /// shipped defects Doug caught, the corpus hidden without a filter and then a
    /// startup test that passed vacuously.
    ///
    /// **The signature is still `&mut self`, and that is not the finished shape.**
    /// The state it owns already lives in [`ModelListState`]; narrowing this to
    /// take that rather than all of `App` is the next step, and it is what makes
    /// the pane renderable in a test without constructing an `App`. Splitting the
    /// move from the narrowing keeps each one verifiable against the baseline
    /// suite instead of landing 270 moved lines and a changed signature together.
    pub(crate) fn ui(
        &mut self,
        ui: &mut egui::Ui,
        sel: Option<&Path>,
        compiling: bool,
        has_model: bool,
    ) -> ModelListOutcome {
        let mut outcome = ModelListOutcome::default();
        section_header(ui, "Models");
        ui.add_space(4.0);
        // The filter is a PREREQUISITE, not an enhancement:
        // 18 curated files need none, 2,644 rows do.
        ui.horizontal(|ui| {
            ui.label("\u{1f50d}");
            ui.add(
                egui::TextEdit::singleline(&mut self.filter)
                    .hint_text("filter by name or outcome")
                    .desired_width(f32::INFINITY),
            );
        });
        ui.add_space(4.0);

        // **Report, but do not return** — this guard predates the
        // corpus and used to guard too much. An empty or unscanned
        // `specimens/` would take the 2,626 MSL models down with it,
        // leaving the whole corpus unreachable because a *different*
        // source was empty. Found 2026-08-01 by the headless test,
        // which runs with no specimens scanned.
        if let Some(err) = &self.scan_error {
            ui.colored_label(ui.visuals().error_fg_color, err);
        } else if self.files.is_empty() {
            ui.weak("(no .mo specimens found)");
        }

        self.poll_scratch_specimens();
        // A scratch name that collides with a curated one is skipped,
        // and said so out loud — silently loading a different model
        // than the one named is the failure this guards against.
        if !self.shadowed.is_empty() {
            ui.colored_label(
                crate::colors::ANIM_FAIL,
                format!(
                    "\u{26a0} ignored scratch specimen(s) shadowing curated names: {}",
                    self.shadowed.join(", "),
                ),
            );
        }
        egui::ScrollArea::vertical()
            .id_salt("specimen_list")
            .show(ui, |ui| {
                let mut to_open = None;
                let mut recompile = None;
                let mut capture_specimen = false;
                // ---- HRW specimens: curated `specimens/` + scratch ----
                //
                // **Collapsed at startup, with MSL expanded** (Doug,
                // 2026-08-01) -- the reverse of the first arrangement.
                // The corpus is now the surface most sessions browse, and
                // 18 curated files are the ones already known by name.
                //
                // Counted before the header is drawn, because the header
                // has to say how many are inside it while it is shut. A
                // collapsed section with no count is a section a reader has
                // to open to learn whether opening it was worth it.
                let hrw_filter = self.filter.trim().to_lowercase();
                let hrw_hits = self
                    .files
                    .iter()
                    .filter(|p| {
                        hrw_filter.is_empty()
                            || p.file_name()
                                .and_then(|n| n.to_str())
                                .is_some_and(|n| n.to_lowercase().contains(&hrw_filter))
                    })
                    .count();
                let hrw_header = if hrw_filter.is_empty() {
                    format!("HRW specimens \u{2014} {}", self.files.len())
                } else {
                    format!("HRW specimens \u{2014} {hrw_hits} of {}", self.files.len(),)
                };
                // **Forced open when a scratch specimen just arrived.**
                // Claude writes those mid-conversation to answer the
                // question being asked, and HRW lists them within a second
                // without a restart. Left shut, the one file written *for
                // the current question* would be the one file not on
                // screen. Same rule as the filter: open when there is a
                // reason to look.
                let hrw_open = if !hrw_filter.is_empty() || self.scratch_arrived {
                    Some(true)
                } else {
                    None
                };
                egui::CollapsingHeader::new(hrw_header)
                    .id_salt("hrw_specimen_list")
                    .default_open(false)
                    .open(hrw_open)
                    .show(ui, |ui| {
                        if hrw_hits == 0 {
                            ui.weak("no match");
                        }
                        for path in &self.files {
                            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("<?>");
                            // One box narrows every source: a curated specimen
                            // and a corpus model are found the same way.
                            if !self.filter.trim().is_empty()
                                && !name
                                    .to_lowercase()
                                    .contains(&self.filter.trim().to_lowercase())
                            {
                                continue;
                            }
                            let selected = sel == Some(path.as_path());
                            let can_capture = selected && !compiling && has_model;
                            let can_recompile = selected && !compiling;
                            let purpose = self.purposes.get(path);
                            // Scratch specimens are marked: "a probe Claude wrote for
                            // one question" and "part of the curated corpus" carry very
                            // different weight, and the list is where that shows.
                            let is_scratch = self.scratch.contains(path);
                            let label = if is_scratch {
                                egui::RichText::new(format!("{SCRATCH_MARKER} {name}"))
                                    .color(crate::colors::SCRATCH_SPECIMEN)
                            } else {
                                egui::RichText::new(name)
                            };
                            let mut resp = ui.selectable_label(selected, label);
                            if is_scratch {
                                resp = resp.on_hover_text(purpose.map(String::as_str).unwrap_or(
                                    "Scratch specimen \u{2014} written by Claude to answer a \
                                     question. Ephemeral: it lives in the gitignored bridge \
                                     directory and is not part of the curated corpus.",
                                ));
                            } else if let Some(hint) = purpose {
                                resp = resp.on_hover_text(hint);
                            }
                            match row_context_menu(&resp, can_recompile, can_capture, "specimen") {
                                Some(RowAction::Recompile) => recompile = Some(path.clone()),
                                Some(RowAction::PointAt) => capture_specimen = true,
                                None => {}
                            }
                            if resp.clicked() {
                                to_open = Some(path.clone());
                            }
                        }
                    });
                // ---- The corpus: the 2,626 MSL models ----
                //
                // **Expanded at startup** (Doug, 2026-08-01). The comment
                // that stood here said the section was "shown only while
                // filtering" -- true of the first version, false of the
                // code beneath it since the same day, and left behind. A
                // comment that describes a design the code abandoned is
                // worse than none: it is read as intent.
                let filter = self.filter.trim().to_owned();
                let mut open_model: Option<String> = None;
                {
                    let rows = self.corpus_rows();
                    let total = rows.len();
                    let hits: Vec<(String, String)> = rows
                        .iter()
                        .filter(|r| survey::matches_filter(r, &filter))
                        .map(|r| (r.name.clone(), r.outcome.clone()))
                        .collect();
                    if total > 0 {
                        ui.add_space(6.0);
                        ui.separator();
                        // **Always visible, collapsed by default.**
                        //
                        // The first version rendered this section only
                        // while filtering, so an unfiltered list showed no
                        // sign the corpus existed at all — Doug started HRW
                        // and reported the MSL examples "not showing", which
                        // was exactly right from where he sat. **An absence
                        // you cannot see is indistinguishable from a feature
                        // that was never built**, and the headless test had
                        // asserted the hidden behaviour, so it encoded the
                        // defect as a requirement.
                        //
                        // **Open at startup, with HRW specimens shut.**
                        // The earlier worry -- 2,626 rows burying 18
                        // curated files -- is answered by giving the 18
                        // their own header rather than by hiding the 2,626.
                        // Only `MAX_LISTED` rows render, so an open corpus
                        // costs a bounded amount of screen either way.
                        let header = if filter.is_empty() {
                            format!("MSL corpus \u{2014} {total} models")
                        } else {
                            format!("MSL corpus \u{2014} {} of {total}", hits.len())
                        };
                        egui::CollapsingHeader::new(header)
                            .id_salt("corpus_list")
                            .default_open(true)
                            .open(if filter.is_empty() { None } else { Some(true) })
                            .show(ui, |ui| {
                                if hits.is_empty() {
                                    ui.weak("no match");
                                }
                                for (name, outcome) in hits.iter().take(survey::MAX_LISTED) {
                                    // The corpus row's identity is the qualified
                                    // name, which `selected` holds verbatim for a
                                    // library model.
                                    let is_sel = sel
                                        .map(|p| p.as_os_str() == name.as_str())
                                        .unwrap_or(false);
                                    // The leaf name reads; the qualified name is
                                    // 60 characters and would wrap every row.
                                    let leaf = name.rsplit('.').next().unwrap_or(name);
                                    let label = egui::RichText::new(format!("\u{1f4e6} {leaf}"))
                                        .color(crate::colors::INCIDENCE_CELL);
                                    let resp = ui
                                        .selectable_label(is_sel, label)
                                        .on_hover_text(format!("{name}\n\noutcome: {outcome}"));

                                    // **The same menu the specimen rows get.** These
                                    // rows had none at all until 2026-08-04 — a
                                    // corpus model could be opened but never
                                    // recompiled or pointed at, so the one list with
                                    // 2,626 entries was the one that could not be
                                    // made the subject of a question.
                                    //
                                    // Enablement matches the specimen rows exactly:
                                    // recompiling needs the row loaded, pointing at
                                    // it also needs a compiled model. `Recompile` is
                                    // `OpenLibrary` again, which always reloads —
                                    // there is no cached path to re-run, the
                                    // qualified name *is* the identity.
                                    let can_recompile = is_sel && !compiling;
                                    let can_capture = is_sel && !compiling && has_model;
                                    match row_context_menu(
                                        &resp,
                                        can_recompile,
                                        can_capture,
                                        "model",
                                    ) {
                                        Some(RowAction::Recompile) => {
                                            open_model = Some(name.clone())
                                        }
                                        Some(RowAction::PointAt) => capture_specimen = true,
                                        None => {}
                                    }

                                    if resp.clicked() {
                                        open_model = Some(name.clone());
                                    }
                                }
                                // **No silent caps.** A truncated list that does not
                                // say so reads as "that is all there is".
                                if hits.len() > survey::MAX_LISTED {
                                    ui.weak(format!(
                                        "\u{2026} and {} more \u{2014} narrow the filter",
                                        hits.len() - survey::MAX_LISTED,
                                    ));
                                }
                            });
                    }
                }
                // **Reported, not applied.** Loading a specimen
                // resets stages, the log and the context bar, none of
                // which this pane owns. Returning the intent keeps the
                // list a *list*.
                if let Some(name) = open_model {
                    outcome.nav = Some(ModelListNav::OpenLibrary(name));
                } else if let Some(path) = recompile {
                    outcome.nav = Some(ModelListNav::Reload(path));
                } else if let Some(path) = to_open {
                    outcome.nav = Some(ModelListNav::Select(path));
                }
                // A separate `if`, exactly as before: "point at" comes
                // from the context menu and can accompany a click.
                if capture_specimen {
                    outcome.point_at_specimen = true;
                }
            });
        outcome
    }

    pub(crate) fn rescan(&mut self) {
        self.files.clear();
        self.scratch.clear();
        self.shadowed.clear();
        self.scan_error = None;
        match std::fs::read_dir(&self.dir) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|e| e.to_str()) == Some("mo") {
                        self.files.push(path);
                    }
                }
                self.files.sort();
            }
            Err(e) => self.scan_error = Some(format!("{}: {e}", self.dir)),
        }

        // Scratch specimens Claude wrote to answer a question (ideas #42). Appended
        // after the curated corpus and **never allowed to shadow it**: a name
        // collision would silently load a different model than the one Doug named,
        // and Claude would then reason confidently about source Doug is not looking
        // at. That is the "makes Claude guess" failure, so the collision is reported
        // and the scratch file is skipped rather than winning or losing quietly.
        let curated: HashSet<String> = self
            .files
            .iter()
            .filter_map(|p| p.file_name().and_then(|n| n.to_str()).map(str::to_owned))
            .collect();
        let mut scratch = Vec::new();
        for path in bridge::scratch_specimens() {
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if curated.contains(name) {
                self.shadowed.push(name.to_owned());
                continue;
            }
            self.scratch.insert(path.clone());
            scratch.push(path);
        }
        // **Scratch first, matching the lab list** (Doug, 2026-07-29). The ephemeral,
        // just-written thing is the one most likely to be wanted next — a probe exists
        // because a question is open right now. Appending it after 18 curated specimens
        // buried the common case, which is the same mistake the lab picker avoids by
        // putting Claude's answer at the top.
        //
        // Safe to reorder because a scratch name colliding with a curated one is
        // skipped above, so `files` never holds two entries with the same file name and
        // `find_specimen`'s first-match cannot become ambiguous.
        scratch.extend(std::mem::take(&mut self.files));
        self.files = scratch;
        // Scan each specimen's `// purpose:` hint (cheap; no compile), so the list
        // can show what each one demonstrates.
        self.purposes = self
            .files
            .iter()
            .filter_map(|p| read_purpose(p).map(|hint| (p.clone(), hint)))
            .collect();
    }

    /// Re-scan when the set of scratch specimens changes.
    ///
    /// Cheaper than it looks: a `read_dir` of a directory that is usually empty, at
    /// most once per [`SCRATCH_POLL_INTERVAL`]. A full `rescan()` only runs when the
    /// *set of paths* actually differs, so the per-file `// purpose:` reads are not
    /// repeated for an unchanged directory.
    pub(crate) fn poll_scratch_specimens(&mut self) {
        let due = self
            .polled_at
            .is_none_or(|last| last.elapsed() >= SCRATCH_POLL_INTERVAL);
        if !due {
            return;
        }
        self.polled_at = Some(std::time::Instant::now());

        let found: HashSet<PathBuf> = bridge::scratch_specimens().into_iter().collect();
        // Compare against what was *accepted* plus what was shadowed, so a scratch
        // file appearing under a curated name still triggers a rescan and gets
        // reported rather than being invisible until the next restart.
        let known = self.scratch.len() + self.shadowed.len();
        if found.len() != known || !found.iter().all(|p| self.scratch.contains(p)) {
            // Only an *arrival* opens the section. A scratch specimen being
            // deleted is not a reason to show the reader anything.
            if found.len() > known {
                self.scratch_arrived = true;
            }
            self.rescan();
        }
    }

    /// The corpus rows, read from the survey on first use.
    ///
    /// Returns an empty slice when the survey has not been generated — a fresh
    /// clone has no `docs/reports/msl-survey.csv` until someone runs the survey,
    /// and the list should degrade to curated specimens rather than error.
    pub(crate) fn corpus_rows(&mut self) -> &[survey::SurveyRow] {
        self.corpus.get_or_insert_with(|| {
            let path = concat!(env!("CARGO_MANIFEST_DIR"), "/docs/reports/msl-survey.csv");
            std::fs::read_to_string(path)
                .map(|t| survey::parse_csv(&t))
                .unwrap_or_default()
        })
    }
}

impl Default for ModelListState {
    fn default() -> Self {
        Self {
            // The only field with a meaningful default; the rest are empty.
            dir: DEFAULT_SPECIMEN_DIR.to_owned(),
            files: Vec::new(),
            purposes: HashMap::new(),
            scratch: HashSet::new(),
            shadowed: Vec::new(),
            scan_error: None,
            corpus: None,
            filter: String::new(),
            scratch_arrived: false,
            polled_at: None,
        }
    }
}

#[cfg(test)]
mod tests_absence {
    use super::*;

    /// **The scratch marker is a glyph HRW's fonts actually have.**
    ///
    /// Doug, 2026-08-30: *"three specimen items are shown in yellow, preceded by
    /// boxes."* The marker was U+270E (LOWER RIGHT PENCIL), which is in none of
    /// egui's four bundled fonts — Hack, Ubuntu-Light, NotoEmoji-Regular,
    /// emoji-icon-font — so every scratch row was prefixed by tofu.
    ///
    /// **Widening the fallbacks could never have fixed it**, and that is the trap worth
    /// recording: `App::hrw_font_definitions` already makes every bundled font a
    /// fallback for both families, added when arrows were boxes in proportional text.
    /// It decides *which loaded font may supply* a glyph, so it does nothing for a
    /// codepoint none of them contains. A marker is a claim the reader can see, and a
    /// box makes it a claim about nothing.
    ///
    /// # The second assertion is the non-vacuity guard, and it is not decoration
    ///
    /// `has_glyphs` returning `true` for everything — a stub, a changed default, a
    /// harness that installs its own font — would let the first assertion pass while
    /// measuring nothing. Asking the *replaced* pencil and requiring **false** proves
    /// the check can still tell the two apart. That is what made this measurable at
    /// all: the probe predicted the defect Doug had already seen, which is why its
    /// verdict on the replacement can be trusted.
    ///
    /// # `has_glyphs` HAS FALSE NEGATIVES, and one was caught the same day
    ///
    /// The survey that chose U+2731 also reported **U+25B6 absent** — and U+25B6 is the
    /// ▶ on the run button, which plainly renders, and was until that afternoon the
    /// prefix on 101 lab hyperlinks Doug described as *triangles*. So this API, asked
    /// this way, says "missing" about glyphs that draw.
    ///
    /// **That does not weaken either assertion here, and the asymmetry is the reason
    /// to write it down rather than distrust the test.** A check prone to false
    /// *negatives* cannot manufacture a false *positive*: `true` for U+2731 is
    /// therefore reliable, and `false` for U+270E is corroborated by Doug seeing a box.
    /// What it does mean is that **a `false` from this API is not on its own evidence
    /// that a glyph will not render** — do not use it to reject a candidate marker
    /// without confirming on screen, which is how ▶ would have been wrongly condemned.
    ///
    /// **U+2731 WAS SO CONFIRMED, 2026-08-30**: Doug reported the scratch rows
    /// rendering correctly after the swap. So the marker rests on two independent
    /// legs — this check, and the screen — which is the standard a glyph choice should
    /// meet here given the false negatives above.
    #[test]
    fn the_scratch_marker_glyph_actually_renders() {
        use egui_kittest::Harness;

        let mut h = Harness::new_ui(|_ui| {});
        h.ctx.set_fonts(crate::app::hrw_font_definitions());
        h.run_steps(2);
        let id = egui::FontId::proportional(14.0);
        let has = |c: char| {
            h.ctx
                .fonts_mut(|f| f.has_glyphs(&id, c.encode_utf8(&mut [0u8; 4])))
        };

        assert!(
            has(SCRATCH_MARKER),
            "the scratch marker U+{:04X} is in none of HRW's bundled fonts, so every \
             scratch row would be prefixed by a tofu box \u{2014} pick a glyph these \
             fonts have, not one that merely looks right in the editor",
            SCRATCH_MARKER as u32,
        );
        assert!(
            !has('\u{270e}'),
            "non-vacuity: U+270E is the pencil this replaced and Doug saw it render as \
             a box, so it must still report as ABSENT \u{2014} if it does not, this \
             check has stopped distinguishing present from missing and the assertion \
             above proves nothing",
        );
    }

    /// **An empty specimen list says so, and a scan ERROR outranks it.**
    ///
    /// From the 2026-08-24 survey. Two branches, and the order between them is the
    /// finding this pane already carries: the guard *"report, but do not return"*
    /// predates the corpus, and an empty `specimens/` once took the 2,626 MSL models
    /// down with it — the whole corpus unreachable because a *different* source was
    /// empty (2026-08-01).
    ///
    /// So the assertions are a pair. An empty scan says it found nothing; a **failed**
    /// scan says why instead, because *"found none"* and *"could not look"* are
    /// different facts and only one of them is about the models.
    ///
    /// # Both harnesses park the scratch poll, and without it neither branch survives
    ///
    /// [`ModelListState::ui`] draws this pair of branches and *then* calls
    /// [`ModelListState::poll_scratch_specimens`], which is due immediately while
    /// `polled_at` is `None`. If `.hrw-bridge/specimens/` holds any `.mo` file, that
    /// poll sees an arrival and calls [`ModelListState::rescan`] — which fills `files`
    /// from the 24 curated specimens **and clears `scan_error`** — so frame two renders
    /// neither message and `run_steps(2)` reads frame two.
    ///
    /// **So this test was green when it landed and turned red the day a scratch
    /// specimen was left on disk**, having never been about the environment at all.
    /// Reproduced both ways on 2026-08-30: emptying that directory made it pass, and
    /// restoring one file made it fail again. This trap has a findings entry of its
    /// own, `docs/ui-findings.md` R2, and the older instance is
    /// `the_model_list_renders_and_reports_without_an_app`, which parks the poll too.
    ///
    /// **Nothing was wrong in production.** `App::new` calls `rescan` before the first
    /// frame, so the absence message never flashes ahead of a real list; the pair of
    /// branches is what this test isolates, and parking the poll is what isolates it.
    #[test]
    fn an_empty_specimen_scan_says_so_and_an_error_says_why_instead() {
        use egui_kittest::Harness;
        use egui_kittest::kittest::Queryable;

        let empty = ModelListState {
            polled_at: Some(std::time::Instant::now()),
            ..Default::default()
        };
        let mut h = Harness::builder()
            .with_size(egui::Vec2::new(400.0, 700.0))
            .build_ui_state(
                |ui, s: &mut ModelListState| {
                    s.ui(ui, None, false, false);
                },
                empty,
            );
        h.run_steps(2);
        assert!(
            h.query_by_label_contains("(no .mo specimens found)")
                .is_some(),
            "an empty specimen scan rendered blank; the list is the app's front door",
        );

        let with_error = ModelListState {
            scan_error: Some("specimens/ is not readable".to_owned()),
            polled_at: Some(std::time::Instant::now()),
            ..Default::default()
        };
        let mut h = Harness::builder()
            .with_size(egui::Vec2::new(400.0, 700.0))
            .build_ui_state(
                |ui, s: &mut ModelListState| {
                    s.ui(ui, None, false, false);
                },
                with_error,
            );
        h.run_steps(2);
        assert!(
            h.query_by_label_contains("is not readable").is_some(),
            "a scan failure must say why it found nothing",
        );
        assert!(
            h.query_by_label_contains("(no .mo specimens found)")
                .is_none(),
            "\u{201c}found none\u{201d} and \u{201c}could not look\u{201d} are different \
             facts, and reporting both invites the reader to believe the first",
        );
    }
}
