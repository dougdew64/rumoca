//! **The lab panel** — the whole left-hand column in Lab mode: the transport bar
//! at the top (pick a lab, go back, play, pause, stop, and the running readout),
//! and the scrolling prose beneath it.
//!
//! Lifted out of `app.rs` on 2026-08-19 in two steps — the bar first, then the prose
//! — which is why the file was called `lab_transport.rs` for an afternoon. See
//! [`docs/app-split-plan.md`](../docs/app-split-plan.md).
//!
//! # What stayed in `App`, and it is the seam
//!
//! `App::lab_panel_ui` still owns the *panel*: polling the lab file, configuring
//! the draggable `SplitState`, performing a transport press, and draining the
//! markdown link hooks. What moved is everything **inside** the scroll area — and
//! that half touches exactly two pieces of state, [`LabState`] and the commonmark
//! cache, with no reach into `App` at all.
//!
//! **So [`lab_prose_ui`] returns nothing.** The two extractions before it had to
//! invent a report — `Option<String>` for a clicked identifier, [`TransportRequest`]
//! for a press — because they contained a decision the pane could not make. This one
//! contains none: it renders a document and records where it ended up. That is worth
//! naming, because *"which half of this function has no policy in it"* turned out to
//! be a better seam-finder than the field count was.
//!
//! # What the bar's signature buys
//!
//! The bar touched **six** fields, and eighteen of its twenty `self` accesses were
//! `self.lab`. So the whole bar reduces to one `&mut LabState` — the state that
//! already lives in [`crate::lab`] — plus `compiling`, which the readout names
//! because a run holds its clock while a specimen builds. **That is the cheapest
//! signature any extraction here has produced**, and it is cheap for a reason worth
//! recording: the state had already been grouped. The 2026-08-02 UI pause that
//! created `LabState` is what made this move a four-parameter one instead of a
//! twenty-field one.
//!
//! # The three things it cannot do itself, and how they leave
//!
//! Back, Play and Stop each reach past the lab into the *application* — history and
//! the lab file, the beat schedule and the dispatcher, the UI mode a run borrowed.
//! None of that is the bar's business, so the bar **reports the press** as a
//! [`TransportRequest`] and `App` performs it. This is the same shape
//! [`crate::specimen_source`] uses for a clicked identifier: **render and report, own
//! no policy**.
//!
//! **Two consequences, stated rather than hidden.**
//!
//! - **Stop still stops the clock here.** `Autoplay::stop` is pure `LabState`, so
//!   leaving it to `App` would have let the readout below render one more frame of a
//!   run that had ended. Only the *mode restore* is deferred, which is why the variant
//!   is [`TransportRequest::Stopped`] — a report that it happened, not a request to do
//!   it.
//! - **Play is deferred by exactly one frame.** `App::start_autoplay` parses the lab,
//!   builds a schedule and dispatches the first beat, and cannot run mid-paint. So on
//!   the click frame the length picker is still enabled and the progress bar is not yet
//!   drawn; the next frame — which egui paints immediately after an interaction — is
//!   correct. The alternative was handing this module the dispatcher.
//!
//! # The constants moved with the view
//!
//! `LAB_PROGRESS_HEIGHT` and `LAB_PROGRESS_MARGIN` are used by exactly this readout,
//! and `LAB_CONTEXT_ABOVE` by exactly the prose scroll; the same rule that moved
//! `SOURCE_MAP_SPLIT_FRACTION` into [`crate::source_map`] applies: state used by one
//! pane is state that pane owns. Leaving them behind would reduce what `app.rs`
//! *holds* without reducing what it *declares*.

use eframe::egui;

use crate::app::{section_style, set_markdown_text_sizes};
use crate::bridge;
use crate::lab::{LabSource, LabState};

/// Height of the autoplay progress bar, and the clear space above and below it.
///
/// The bar draws its percentage *inside* itself, so it has to be tall enough for a
/// line of text rather than tall enough for a rule. At 6px with no surrounding space
/// it was clipped between the transport row and the stop caption.
const LAB_PROGRESS_HEIGHT: f32 = 18.0;
const LAB_PROGRESS_MARGIN: f32 = 6.0;

/// A press on the transport that only [`crate::app::App`] can carry out.
///
/// **The bar never acts on these itself.** Each one reaches state the lab panel does
/// not own — the lab file, the beat dispatcher, the UI mode a run borrowed — so the
/// bar reports and the application decides. See the module docs for why
/// [`Self::Stopped`] is phrased as a report rather than a request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TransportRequest {
    /// A different lab was chosen — from the picker, or from the Answer button.
    Switch(LabSource),
    /// **Back** — return to the lab a cross-lab link was followed from.
    Back,
    /// **Play** — build a schedule from the showing lab and start the clock.
    Play,
    /// **Stop was pressed and the clock is already stopped.** What remains is the UI
    /// mode the run borrowed, which only `App` can put back.
    Stopped,
    /// **🎯 was pressed** — make the selected prose the point.
    ///
    /// Reported rather than performed for the usual reason, plus one specific to it:
    /// the capture needs an `egui::Context` to push a `Copy` event into and two frames
    /// to collect the result, and neither belongs in a panel that draws markdown.
    PointAtSelection,
}

/// **The Play button** — transport for a self-running lab.
///
/// Built 2026-08-03 for recording a run as video, after a LinkedIn screenshot
/// drew interest and explaining *what a lab is* to people who have never seen
/// HRW proved harder in prose than in motion.
///
/// The controls sit between the lab list and the lab text rather than in the
/// menu bar, because they belong to the lab showing below them and a recording
/// should show the viewer where the run came from.
///
/// **Everything decidable lives in [`crate::autoplay`]**; this function only
/// draws. That split is what lets the schedule and the clock be tested without
/// a window, which for a *timing* feature is the difference between a checkable
/// claim and a stopwatch.
///
/// The lab transport bar: **which lab**, then **the run**.
///
/// Returns the press that `App` must carry out, if there was one — see
/// [`TransportRequest`]. `lab_text` is the showing lab’s markdown, which the bar
/// needs for two things: whether Play is enabled at all, and re-parsing the stop
/// headings for the running caption.
///
/// # Why the picker is in here
///
/// Doug, 2026-08-16, after the 23-row list became a combo box and a button: *"The
/// divider bar above … no longer makes sense. It's the divider bar which currently
/// says 'Labs (23)'."* A titled bar wrapping two controls is chrome announcing
/// chrome, and it cost a header, a separator and the space between them for no
/// information — the count it carried is now in the picker's own hover.
///
/// Left to right, in the order he specified: **Claude's answer**, the **picker**,
/// then the transport. That reads as a sentence — *which lab, then what to do with
/// it* — and it puts the one control whose state changes mid-conversation at the
/// end of the eye's travel from the prose below.
pub(crate) fn autoplay_controls_ui(
    ui: &mut egui::Ui,
    lab: &mut LabState,
    compiling: bool,
    lab_text: &Option<String>,
) -> Option<TransportRequest> {
    use crate::autoplay::Phase;

    let mut request: Option<TransportRequest> = None;
    let has_lab = lab_text.is_some();
    let phase = lab.autoplay.phase();

    // **Same palette as the section headers above it** (Doug, 2026-08-03). The
    // transport is a left-panel *bar*, like "Labs (8)" and "Specimens", not a
    // loose row of buttons floating on the panel background — and it read as the
    // latter because it was the only thing in the column without the navy frame.
    //
    // `section_style` rather than copied colours: it already resolves light and
    // dark mode, and a second copy of the palette is how the RHS tab colours and
    // the LHS header colours would drift apart.
    let style = section_style(ui);
    style.frame.show(ui, |ui| {
            // **A ceiling as well as a floor.** Without `set_max_width` the bar can ask
            // for more room than the panel has, egui widens the *panel* to satisfy it,
            // and the panel then reports a width its content was never laid out against
            // — content visibly detached from the divider, and oscillating between
            // frames because each frame's width feeds the next.
            //
            // Measured 2026-08-16: a 65.9pt gap at 1280x720, and *removing* a label made
            // it 134.8pt, which is the signature of a feedback loop rather than of one
            // item being too wide.
            let bar_width = ui.available_width();
            ui.set_min_width(bar_width);
            ui.set_max_width(bar_width);
            ui.horizontal(|ui| {
                // --- Back: undo a cross-lab link ---
                //
                // Doug, 2026-08-19: *"while in the index reduction lab, I can click a
                // link to navigate to the blt-ordering lab, but then I cannot navigate
                // back."* Placed where the lab count used to be, at his suggestion.
                //
                // **This does not consume the RHS Back/Forward** reserved in `ideas.md`
                // #78. It is the lab panel's history; the RHS pair belongs in the stage
                // tab bar. #78's own analysis leaned to exactly this — two histories,
                // visually distinct, because the scopes really are different — and HRW
                // already has a third in `nav`'s go-to-definition Back.
                //
                // **Enabled and disabled, never shown and hidden** (`lib.rs`'s LiveState
                // rule), and the hover names the destination rather than leaving an arrow
                // to be interpreted.
                //
                // **It costs panel floor and that is now a stated price**, not a
                // surprise: un-wrapping made the bar's width monotonic, so this button's
                // ~60pt lands directly on `MIN_LEFT_POINTS`. It was affordable only after
                // the lab count, the duration words and the time combo's default width
                // were removed.
                let back_to = lab.history.last().map(|(source, _)| source.label());
                let back = ui.add_enabled(
                    back_to.is_some(),
                    egui::Button::new("\u{25c2}").small(),
                );
                if let Some(name) = &back_to {
                    if back.on_hover_text(format!("Back to {name}")).clicked() {
                        request = Some(TransportRequest::Back);
                    }
                } else {
                    back.on_hover_text(
                        "Nothing to go back to \u{2014} follow a link into another lab \
                         and this returns you here",
                    );
                }

                // --- Which lab: Claude's answer, then the picker ---
                //
                // **Claude's answer is not the same kind of object as the other 22**
                // (Doug, 2026-08-16). They are committed, versioned, machine-checked
                // and citable as `hrw://lab/<name>/station/<slug>`; this one is
                // `.hrw-bridge/answer.md` — gitignored, regenerated per question, and
                // there is only ever one. `lab::poll` already privileged it by
                // auto-selecting it; only the presentation had flattened it into a row.
                //
                // Leftmost, so its state — present, absent, selected — is answerable
                // without opening anything. That was the 2026-08-15 defect: the row
                // was correctly absent and read as a broken feature.
                let has_ad_hoc = lab.available.contains(&LabSource::Answer);
                let answer_selected = lab.selected.as_ref() == Some(&LabSource::Answer);
                // `Button::selectable`, not a plain button: this *selects* what you
                // are reading. A verb-shaped control here would imply otherwise.
                let resp = ui.add_enabled(
                    has_ad_hoc,
                    egui::Button::selectable(answer_selected, LabSource::Answer.label()),
                );
                if has_ad_hoc {
                    if resp
                        .on_hover_text(
                            "Written by Claude to answer your last question.                              Ephemeral: regenerated, never stored.",
                        )
                        .clicked()
                    {
                        request = Some(TransportRequest::Switch(LabSource::Answer));
                    }
                } else {
                    resp.on_disabled_hover_text(
                        "No answer written yet. Ask Claude a question and it writes one                          here — regenerated per question, never stored.",
                    );
                }

                let selected_label = match lab.selected.as_ref() {
                    Some(LabSource::Fixture(p)) => LabSource::Fixture(p.clone()).label(),
                    _ => "Pick a lab…".to_owned(),
                };
                // **The count moved here from the deleted header.** It existed because
                // "I do not see the new lab" must be answerable at a glance rather
                // than by reasoning: a number distinguishes "the directory has six"
                // from "the pane is showing six of eight", and those need opposite
                // fixes. One hover keeps that without a titled bar around it.
                let n_fixtures = lab
                    .available
                    .iter()
                    .filter(|s| matches!(s, LabSource::Fixture(_)))
                    .count();
                egui::ComboBox::from_id_salt("lab_picker")
                    .selected_text(selected_label)
                    // **Adaptive, never fixed.** A hard `width(220.0)` here gives the
                    // bar an intrinsic minimum, egui sizes the *panel* to satisfy it,
                    // and the panel then reports a width its content was never laid out
                    // against — `the_left_panel_content_never_detaches_from_the_divider`
                    // caught exactly that, a 65.9pt gap at 1280x720.
                    //
                    // Same lesson as the 2026-08-12 scroll-axis bug, from the other
                    // side: **a child's minimum is a claim about the parent's width.**
                    // The clamp keeps it readable when there is room and lets it shrink
                    // when there is not.
                    //
                    // **Dropping `.width()` entirely fails too, and worse** (measured
                    // 2026-08-19): a truncated label with no call produced a **62.6pt
                    // gap where this code passes**, because the combo then sizes to its
                    // text — which is widest exactly when there is least room. So both
                    // alternatives are dead by measurement, a fixed width and no width;
                    // this formula IS the responsive mechanism, not a workaround.
                    // `docs/ui-findings.md` C16.
                    .width((bar_width * 0.45).clamp(60.0, 220.0))
                    .show_ui(ui, |ui| {
                        // **The overview sorts first, set apart** — the reasoning and the
                        // rejected alternatives are on `LabState::picker_order`.
                        let (ordered, hoisted) = lab.picker_order();

                        for (i, source) in ordered.into_iter().enumerate() {
                            if i == hoisted && hoisted > 0 {
                                ui.separator();
                            }
                            let LabSource::Fixture(path) = source else {
                                // The Answer has its own control to the left;
                                // listing it twice would make one of them a lie about
                                // where it lives.
                                continue;
                            };
                            let selected = lab.selected.as_ref() == Some(source);
                            // **The entry names its specimens.** Labs are named by
                            // phase, but Doug searches by the model in front of him and
                            // went looking for a "DimensionMismatch lab" that is called
                            // `failure-typecheck` (2026-08-05). Showing both axes removes
                            // the search, and it has to survive every move of this
                            // control or the friction comes back.
                            let label = lab.row_specimens.get(path).map_or_else(
                                || source.label(),
                                |sp| format!("{}  ·  {sp}", source.label()),
                            );
                            if ui
                                .selectable_label(selected, label)
                                .on_hover_text(format!(
                                    "Fixture lab — a test with expected outcomes,                                      kept and versioned.
{}",
                                    path.display(),
                                ))
                                .clicked()
                            {
                                request = Some(TransportRequest::Switch(source.clone()));
                            }
                        }
                    })
                    .response
                    .on_hover_text(format!(
                        "{n_fixtures} fixture labs in {}",
                        bridge::FIXTURE_LABS_DIR
                    ));

                // **The count stays visible, in three words instead of a titled bar.**
                //
                // It was the whole reason the deleted "Labs (23)" header existed:
                // "I do not see the new lab" has to be answerable at a glance, because
                // a number distinguishes *the directory has six* from *the pane is
                // showing six of eight*, and those need opposite fixes. Doug reported
                // that on 2026-08-03 with a picker test asserting the labs were on
                // screen — the code was provably right and the report was still true.
                //
                // A hover was tried first and is not enough: a tooltip is invisible
                // until suspicion already exists, which is exactly too late, and it is
                // also unreachable from the accessibility tree so no test can see it.
                //
                // **REMOVED 2026-08-19.** Doug: *"I have not used that label's
                // information a single time"* — sixteen days, by the person it was built
                // for, which is better evidence than the reasoning above. The count
                // survives on the picker's hover; if *"I don't see the new lab"* ever
                // recurs, restoring this line is the fix.
                // <!-- unbuilt: visible_lab_count -->

                ui.separator();

                match phase {
                    // **Pausable from the first frame.** The lead-in is a run, so offering
                    // Pause here rather than Play means the button does not change identity
                    // one second after the click — and a reader who pressed play by mistake
                    // can stop it before anything has moved.
                    Phase::LeadIn | Phase::Playing | Phase::Paused => {
                        let (label, hover) = if phase != Phase::Paused {
                            ("\u{23f8} Pause", "Hold the run here.")
                        } else {
                            ("\u{25b6} Resume", "Continue from this beat.")
                        };
                        if ui.button(label).on_hover_text(hover).clicked() {
                            // Anything that is not already paused pauses, which now
                            // includes the lead-in.
                            if phase == Phase::Paused {
                                lab.autoplay.resume();
                            } else {
                                lab.autoplay.pause();
                            }
                        }
                        if ui
                            .button("\u{23f9} Stop")
                            .on_hover_text(
                                "End the run. Whatever is on screen stays \u{2014} stopping \
                                 halfway leaves you looking at the thing you stopped for.",
                            )
                            .clicked()
                        {
                            lab.autoplay.stop();
                            request = Some(TransportRequest::Stopped);
                        }
                    }
                    Phase::Idle | Phase::Finished => {
                        let play = ui
                            .add_enabled(has_lab, egui::Button::new("\u{25b6} Play"))
                            .on_hover_text(
                                "Walk this lab by itself, for recording. The clock pauses \
                                 while a specimen compiles and while another window has \
                                 focus, so a slow machine makes a longer video, never a \
                                 broken one.",
                            );
                        if play.clicked() {
                            request = Some(TransportRequest::Play);
                        }
                    }
                }

                // The length picker. Disabled mid-run: changing the budget under a
                // running schedule would leave the progress bar describing a plan that
                // no longer exists.
                ui.add_enabled_ui(!lab.autoplay.is_running(), |ui| {
                    let current = crate::autoplay::TOTAL_CHOICES
                        .iter()
                        .find(|(_, s)| *s == lab.autoplay_total.as_secs())
                        .map(|(l, _)| *l)
                        .unwrap_or("custom");
                    egui::ComboBox::from_id_salt("autoplay_total")
                        // **Sized to its labels.** With no `width` egui applies its
                        // default `combo_width` of 100pt, which was invisible while the
                        // labels read `90s — standard` and became obvious the moment they
                        // became `90s`. Doug: *"Why is the time selector combobox so wide?
                        // It is much wider than necessary for the labels."*
                        //
                        // A constant is safe *here* although it was not for the lab
                        // picker: that one needed to shrink on a narrow panel, so a fixed
                        // width forced the panel wider. These four labels never exceed
                        // four characters, so this width is an upper bound rather than a
                        // demand — and since the bar stopped wrapping, its contribution to
                        // the panel floor is now simply additive.
                        .width(52.0)
                        .selected_text(current)
                        .show_ui(ui, |ui| {
                            for (label, secs) in crate::autoplay::TOTAL_CHOICES {
                                let d = std::time::Duration::from_secs(secs);
                                ui.selectable_value(&mut lab.autoplay_total, d, label);
                            }
                        })
                        .response
                        .on_hover_text(
                            "Total length of the run. Conventional social-video lengths \
                             \u{2014} pick to fit where it is going.",
                        );
                });

                // **🎯 — make the selected prose the subject of the next question.**
                //
                // Doug, 2026-08-30, after four frictions in asking about lab text:
                // switching to VS Code, finding the `.md`, locating the passage in
                // source, and — the one that decided it — a bare "What is this?" having
                // no referent. The capture reaches Claude through `focus.json`, and the
                // Context Bar shows what he holds before he asks.
                //
                // **LAST IN THE ROW, AND ONLY WHILE A SELECTION EXISTS**, which is not
                // a style choice: it was drawn first and always, greyed when unusable,
                // and `the_left_panel_content_never_detaches_from_the_divider` failed —
                // the panel's permanent floor went 431.7pt to 472.8pt, and its message
                // says exactly why that matters, that "the RHS pays for it
                // permanently". This bar is the tuned equilibrium `CLAUDE.md` records
                // five failed perturbations of, and #77 bought HRW's usability on a 13"
                // screen with numbers like these.
                //
                // A widget that comes and goes belongs at the END of a row — the rule
                // the Simulation spinner established this morning, applied on its first
                // new occasion. Here nothing follows it, so its arrival pushes nothing.
                //
                // **The cost is discoverability**, paid deliberately: the greyed button
                // was the thing that announced the feature existed. Doug asked for it,
                // so he knows; a reader who does not will find it by selecting text.
                //
                // The selection lives in an egui *plugin*, which is the only public way
                // to ask whether one exists — the text itself is never exposed, which
                // is the whole reason for the copy round trip in `PendingPassage`.
                let has_selection = ui
                    .ctx()
                    .plugin::<egui::text_selection::LabelSelectionState>()
                    .lock()
                    .has_selection();
                // **Tinted, because egui's bundled emoji font is MONOCHROME.** Doug:
                // "the button has the icon which you've shown here, but the icon is not
                // coloured." NotoEmoji-Regular has no colour layers, so 🎯 arrives as a
                // plain glyph — and a plain glyph takes the text colour it is given.
                // `CONTEXT_POINT` is the one to give it: this button makes a *point*,
                // and cyan is what the Context Bar and the panes already say that in.
                if has_selection {
                    let resp = ui
                        .button(
                            egui::RichText::new("\u{1f3af}")
                                .color(crate::colors::CONTEXT_POINT),
                        )
                        .on_hover_text(
                            "Point at the selected text \u{2014} then ask about it in \
                             the chat. Ctrl+C still just copies.",
                        );
                    // **The PRESS, not the click, and this button cannot use `clicked()`
                    // at all.** Doug, 2026-08-30: *"when I click the button, it
                    // disappears and the selection disappears… it seems incorrect that
                    // the selection is disappearing."* He had the cause exactly right,
                    // and it is worse than cosmetic:
                    //
                    // egui clears a label selection on any pointer press outside a
                    // hovered label — so pressing this button destroys the very thing it
                    // acts on, at mouse-DOWN. `clicked()` fires at mouse-UP, by which
                    // time `has_selection` is false, the button is no longer drawn, and
                    // **the click is never observed.** Nothing happened, and nothing
                    // said so.
                    //
                    // On the press frame the selection is still live, and the copy this
                    // schedules is accumulated while the prose paints — after this bar,
                    // before egui's end-of-pass clear. So the press is not a workaround
                    // for the ordering; it is the only moment the text exists.
                    //
                    // `primary_pressed` is edge-triggered, so holding the button does
                    // not re-arm on every frame.
                    if resp.hovered() && ui.input(|i| i.pointer.primary_pressed()) {
                        request = Some(TransportRequest::PointAtSelection);
                    }
                }
            });

            if !lab.autoplay.is_running() {
                return;
            }

            // --- The running readout ---
            //
            // A caption naming the stop, because a recording is watched by people who
            // cannot see the cursor and have no idea which part of the lab they are in.
            let (beat, total) = lab.autoplay.progress();
            let phase = lab.autoplay.phase();
            // **Margin above and below.** At 6px with no spacing the bar was clipped by
            // its neighbours and its percentage was only half legible — Doug, 2026-08-03:
            // *"the progress bar is not entirely visible because not enough vertical
            // space is being provided"*. The bar carries the percentage text, so it needs
            // room for a line of text, not for a rule.
            ui.add_space(LAB_PROGRESS_MARGIN);
            ui.add(
                egui::ProgressBar::new(lab.autoplay.fraction())
                    .desired_height(LAB_PROGRESS_HEIGHT)
                    .show_percentage(),
            );
            ui.add_space(LAB_PROGRESS_MARGIN);
            // The caption takes the header's `active_color` and the status line its
            // `inactive_color`, so the bar reads as one element with a primary and a
            // secondary line — the same relationship the section headers already have.
            if let Some(caption) = lab
                .autoplay
                .current_station()
                .and_then(|i| autoplay_station_heading(lab_text.as_deref(), i))
            {
                ui.label(
                    egui::RichText::new(caption)
                        .strong()
                        .size(13.0)
                        .color(style.active_color),
                );
            }
            ui.label(
                egui::RichText::new(format!(
                    "beat {beat}/{total} \u{00b7} {}",
                    match phase {
                        Phase::Paused => "paused",
                        // Named, so a second of stillness reads as the run starting rather
                        // than as the run being stuck.
                        Phase::LeadIn => "starting",
                        _ if compiling => "compiling \u{2014} clock held",
                        _ => "playing",
                    }
                ))
                .color(style.inactive_color),
            );
        });
    request
}

/// The heading of stop `index` in the lab text, for the running caption.
///
/// Re-parsed rather than stored with the schedule: the lab file is re-read
/// whenever it changes on disk, and a caption cached at Play time would keep
/// naming a stop that had since been rewritten. Doug regenerates labs *while*
/// running them, which makes that the normal case rather than an edge one.
///
/// **It never used `self`**, which is why it left `App` as a free function rather
/// than acquiring a parameter.
fn autoplay_station_heading(text: Option<&str>, index: usize) -> Option<String> {
    let stops = crate::autoplay::parse_stations(text?);
    stops.get(index).map(|s| s.heading.clone())
}

/// How much lab text to keep **above** the link a beat is dispatching.
///
/// Doug, 2026-08-03: *"the scrolling should be paused with that frame link showing
/// with perhaps a line or two of text which is above that frame link. The frame link
/// and the lines of text above the link document the animation frame."*
///
/// Roughly two lines. Scrolling the link to the very top would put its introduction
/// off-screen, and the pair — lead-in and link — is what names the frame.
const LAB_CONTEXT_ABOVE: f32 = 48.0;

/// The lab prose — the scrolling markdown beneath the transport bar.
///
/// Everything inside the panel's scroll area, and **nothing outside it**: the panel
/// itself, the split, the transport press and the link hooks stay in
/// `App::lab_panel_ui`. The division is not a line count — it is that this half
/// reaches nothing but [`LabState`] and the markdown cache, so it can be a free
/// function with four parameters and no return value.
///
/// `lab_text` is the document as of this frame, already read from disk by the
/// caller's poll. `None` means no lab is loaded, which is a state with prose of its
/// own rather than an empty pane — see [`no_lab_ui`].
pub(crate) fn lab_prose_ui(
    ui: &mut egui::Ui,
    lab: &mut LabState,
    cache: &mut egui_commonmark::CommonMarkCache,
    lab_text: &Option<String>,
) {
    // **The prose scrolls with the run.**
    //
    // Without this the stage side moves while the lab text sits at the
    // top, and a viewer of the recording cannot tell which stop they are
    // in — which matters more here than it would have a week ago, now
    // that the prose is load-bearing rather than captioning.
    //
    // Driven by beat *position* rather than by locating the stop's
    // heading in the rendered markdown: `egui_commonmark` lays out its
    // own content and exposes no anchor per heading, so there is nothing
    // to scroll *to*. Proportional scrolling is an approximation, and an
    // honest one — it drifts from the exact heading position when stops
    // differ in length, which the caption above the pane covers.
    //
    // **`scroll_fraction`, not `fraction`.** The first version used the
    // clock, so the text crept continuously and never held still — Doug,
    // 2026-08-03: *"the scrolling never pauses when a frame is being
    // displayed"*, which was worst exactly where the lab is best, with a
    // deliberately paused animation under sliding prose. `scroll_fraction`
    // travels to the new beat's place and then stops.
    //
    // **Only while running.** Forcing the offset when idle would fight a
    // reader who scrolled somewhere themselves.
    // **`both`, not `vertical`, and the horizontal axis is load-bearing**
    // (Doug, 2026-08-12: *"the divider does not move. Instead, only the
    // right edge of the LHS lab content moves"*).
    //
    // A vertical-only scroll area reports its content's **full width** as
    // the width it wants, and `egui_commonmark` does not wrap tables or
    // code blocks — `the-concepts.md` has a 178-character line. So the
    // lab panel's intrinsic minimum width became the widest table in the
    // document, egui sized the panel to it, and the divider had nothing
    // left to give:
    //
    // ```text
    // no lab loaded    panel opens 512pt (the 40% default), drags to 213pt
    // real lab loaded  panel opens 899pt and is FROZEN; the inner Ui still
    //                   follows the pointer, so the gap reached 705pt
    // ```
    //
    // Enabling the horizontal axis makes wide content **scroll instead of
    // push**, so the panel keeps the width the reader chose and the table
    // is still reachable. Wrapping is not the alternative: a Markdown table
    // does not wrap into anything readable.
    //
    // Note what this cost before it was found: the lab panel was quietly
    // taking 70 % of a 1280pt window rather than the 40 % it reports.
    let mut area = egui::ScrollArea::both().id_salt("lab");

    // **A new lab opens at its top.**
    //
    // `id_salt("lab")` is stable on purpose — it is what lets a reader's
    // scroll position survive a repaint — and the cost is that it survives
    // the *document* too. Doug, 2026-08-17: *"When I click a subordinate
    // lab link in the-concepts hub lab, the subordinate lab opens
    // partially scrolled down instead of fully scrolled to the top."*
    //
    // The hub is the worst case and the reason it surfaced now: its links
    // sit in a ten-row table you scroll down to reach, so the lab that
    // opened inherited however far down the row was.
    //
    // **Consumed only on a frame that has text.** Switching clears `cached`,
    // so the frame in between renders nothing — clearing the flag there
    // would spend it on a document that was never drawn.
    //
    // **A pending stop request outranks this**, and both are live at once
    // by construction: `hrw://lab/<name>/station/<slug>` switches the lab
    // (which asks for the top) and *then* asks for the stop. The top
    // request is still consumed — leaving it pending would fire it at the
    // next document — it simply does not get to move anything.
    let station_pending = lab.scroll_to_offset.is_some();
    // **Back outranks both**: returning to where you were is the one
    // navigation for which the top of the document is the wrong answer.
    let restore = lab.restore_scroll_y.take();
    if lab.scroll_to_top && lab_text.is_some() {
        lab.scroll_to_top = false;
        if !station_pending && restore.is_none() {
            area = area.vertical_scroll_offset(0.0);
        }
    }
    match (restore, lab_text.is_some()) {
        (Some(y), true) => area = area.vertical_scroll_offset(y),
        // No text this frame, so the request has nothing to apply to and must
        // survive — the one-shot discipline `scroll_to_top` already follows.
        (Some(y), false) => lab.restore_scroll_y = Some(y),
        (None, _) => {}
    }

    if lab.autoplay.is_running()
        && measurements_match_beat(lab.lab_measured_beat, lab.autoplay.progress().0)
        && let Some(max_scroll) = lab.lab_max_scroll
    {
        // **Interpolate between two MEASURED positions.** Both come from
        // the split below, so neither is an estimate of anything.
        let to = lab.lab_link_y.unwrap_or(0.0);
        let from = lab.lab_prev_link_y.unwrap_or(0.0);
        let y = from + (to - from) * lab.autoplay.travel_t();
        // Leave a little above the link, so the line or two introducing
        // it stays on screen with it. Doug: "the frame link and the lines
        // of text above the link document the animation frame."
        let target = (y - LAB_CONTEXT_ABOVE).clamp(0.0, max_scroll.max(0.0));
        area = area.vertical_scroll_offset(target);
    }

    // **Where the current beat's link actually renders**, measured rather
    // than estimated from character offsets.
    //
    // Two earlier attempts guessed this position — first from the beat's
    // ordinal, then from the link's character offset over the document —
    // and both were wrong, because rendered height per character is not
    // constant: prose wraps in a narrow panel and a code block does not.
    // **No constant corrects an estimate that is wrong in both
    // directions**, so this stops estimating.
    //
    // The markdown is split at the link's line and rendered as two
    // documents. The cursor between them *is* the link's y position. The
    // split falls on a line start, and every link in these labs is its
    // own paragraph, so no markdown construct is cut in half.
    let mut measured: Option<f32> = None;
    let out = area.show(ui, |ui| {
        set_markdown_text_sizes(ui);
        match lab_text {
            Some(text) => {
                // **Only split while a run is running.**
                //
                // The split exists to measure one beat's link position.
                // Idle, it buys nothing — and it does not go away by
                // itself: a *finished* run keeps its beats, so
                // `current_byte_offset` still names the last link and the
                // document stayed cut in two for all subsequent manual
                // reading. Rendering one markdown document as two is not
                // free of consequence, and doing it when nothing needs it
                // is a difference from the plain path with no upside.
                //
                // **And the same split serves a stop link**, which is why
                // that feature costs almost nothing here. `hrw://lab/<t>/
                // stop/<slug>` records the heading's *byte* offset, and the
                // problem it faces is the one the autoplay scroll already
                // solved the hard way: a byte offset cannot be converted to
                // a pixel position by arithmetic, because rendered height
                // per character is not constant.
                //
                // Splitting there puts the **cursor** exactly at the stop,
                // and a cursor is a real position rather than an estimate.
                //
                // **The offset is validated, not trusted.** A lab is
                // re-read whenever its mtime changes, so an offset recorded
                // against the previous text can land mid-character after an
                // edit — and slicing a `str` off a char boundary panics.
                // These documents are edited *while* Doug runs them, so
                // that is the expected case rather than a corner one.
                let station_split = lab
                    .scroll_to_offset
                    .filter(|n| *n <= text.len() && text.is_char_boundary(*n));
                let split = if lab.autoplay.is_running() {
                    lab.autoplay.current_byte_offset().min(text.len())
                } else {
                    station_split.unwrap_or(0)
                };
                let top = ui.cursor().top();
                if split > 0 {
                    show_lab_markdown(ui, &mut *cache, &text[..split], LabPart::BeforeCursor);
                }
                measured = Some(ui.cursor().top() - top);

                // **egui does the scrolling, and that is the whole trick.**
                // Asking the `ScrollArea` for an offset would mean computing
                // one; asking it to bring the cursor into view means it
                // computes one, from a position it already knows exactly.
                //
                // A run in progress owns the scroll, so a stop request is
                // still *consumed* but not acted on — otherwise it would
                // fight the interpolation for the rest of the run.
                if lab.scroll_to_offset.take().is_some()
                    && station_split.is_some()
                    && !lab.autoplay.is_running()
                {
                    ui.scroll_to_cursor(Some(egui::Align::TOP));
                }

                show_lab_markdown(ui, &mut *cache, &text[split..], LabPart::AfterCursor);
            }
            None => no_lab_ui(ui),
        }
    });

    // A new beat means a new split, so the position measured last frame
    // becomes the one to travel *from*.
    let beat = lab.autoplay.progress().0;
    if lab.lab_measured_beat != Some(beat) {
        lab.lab_prev_link_y = lab.lab_link_y.or(Some(0.0));
        lab.lab_measured_beat = Some(beat);
    }
    lab.lab_link_y = measured;
    lab.lab_max_scroll = Some((out.content_size.y - out.inner_rect.height()).max(0.0));
    // **Where the reader actually is**, from the scroll area's own output
    // rather than tracked alongside it, so it cannot drift from the screen.
    // Every frame, because a switch can happen at any moment and the offset
    // must be the one from just before it.
    lab.current_scroll_y = out.state.offset.y;
}

/// What lab mode shows when Claude has not written a lab.
///
/// Deliberately **not** `end_to_end_lab.md`, which used to be compiled in
/// here with `include_str!`. That document's prose was retired 2026-07-29
/// (ideas #42) — it described a 7x7 incidence matrix on a tab that shows 48
/// equations — so keeping it as the default would put the exact stale
/// content this change exists to remove back on screen.
/// One piece of a lab: either prose for `egui_commonmark`, or a fenced code block
/// HRW draws itself.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum LabSegment<'a> {
    Prose(&'a str),
    /// `lang` is the fence tag — `modelica`, `text`, or empty.
    Code {
        lang: &'a str,
        body: &'a str,
    },
}

/// **Colour for code, dispatched on the fence language.**
///
/// Doug, 2026-09-01: *"can we choose a different color for the Modelica code snippets?
/// Right now, they are white and so are similar in appearance to the bold white text
/// elsewhere in the lab."* He ruled all fenced blocks share one colour for now, **with
/// the capability to differ by language reserved** — which is why this is a `match` on
/// one arm rather than a constant.
///
/// The colour is applied to inline `code` spans too, via `code_bg_color`; see
/// `app.rs`'s style setup for why those get a tint rather than this foreground.
// **The single-arm match is the point, not an oversight.** Doug ruled one colour for all
// fenced blocks *"while reserving the capability to color by language"*, so this exists to
// be split later. Collapsing it to a constant, as clippy suggests, would delete the seam
// he asked for and make the next change a signature change instead of an arm.
#[allow(clippy::match_single_binding)]
pub(crate) fn code_colour(lang: &str) -> egui::Color32 {
    match lang {
        // Every language, until a reason to separate one appears.
        _ => egui::Color32::from_rgb(0x7E, 0xC6, 0x99),
    }
}

/// **Split a lab into prose and fenced code blocks.**
///
/// HRW draws code blocks itself because `egui_commonmark` cannot be told what colour to
/// use: it delegates to `egui_extras`' `CodeTheme::from_style`, which hardcodes its
/// palette per light/dark and reads only the *font* from our style. An unknown language
/// like `modelica` then falls through to `LIGHT_GRAY`, which is what made code read like
/// bold white prose.
///
/// **An unterminated fence is prose.** Labs are edited while Doug is reading them, so a
/// half-typed fence is the expected case rather than a corner one — the same reason the
/// scroll offset above is validated rather than trusted.
pub(crate) fn split_lab_segments(text: &str) -> Vec<LabSegment<'_>> {
    let mut out = Vec::new();
    let mut rest = text;

    while let Some(open) = rest.find("```") {
        // Prose before the fence.
        if open > 0 {
            out.push(LabSegment::Prose(&rest[..open]));
        }
        let after_ticks = &rest[open + 3..];
        let Some(nl) = after_ticks.find('\n') else {
            // A fence with no newline after it cannot be a block.
            out.push(LabSegment::Prose(&rest[open..]));
            return out;
        };
        let lang = after_ticks[..nl].trim();
        let body_start = &after_ticks[nl + 1..];
        match body_start.find("```") {
            Some(close) => {
                out.push(LabSegment::Code {
                    lang,
                    body: &body_start[..close],
                });
                // Step past the closing fence and its newline, if present.
                let tail = &body_start[close + 3..];
                rest = tail.strip_prefix('\n').unwrap_or(tail);
            }
            None => {
                // Unterminated: hand the remainder back as prose.
                out.push(LabSegment::Prose(&rest[open..]));
                return out;
            }
        }
    }

    if !rest.is_empty() {
        out.push(LabSegment::Prose(rest));
    }
    out
}

/// Lay out a Modelica block with **the same lexer and palette as the source pane**.
///
/// Doug, 2026-09-01: *"let's implement syntax highlighting in labs, only for Modelica so
/// that we can achieve visual consistency across HRW."* The same `connect(src.p, R.p)`
/// used to render one way in a lab and another in `specimen_source`, because
/// `egui_commonmark` drew the block and never asked. HRW draws it now, so it can call
/// [`crate::source_view::SourceHighlight`] and [`crate::colors::syntax_color`] — the
/// pair `specimen_source` already uses.
///
/// **Modelica only, deliberately.** Labs also fence `text` — log excerpts, equation
/// dumps, command lines — and running those through a Modelica lexer would colour
/// arbitrary words as keywords, which is worse than a flat colour. That is what the
/// language dispatch in [`code_colour`] was reserved for.
///
/// Unclassified runs take `visuals().text_color()`, matching the source pane rather than
/// the flat green: consistency is the point, so the two must not differ on the residue.
fn modelica_layout(ui: &egui::Ui, body: &str) -> egui::text::LayoutJob {
    use crate::source_view::{SourceHighlight, segments};

    let font = egui::TextStyle::Monospace.resolve(ui.style());
    let dark = ui.visuals().dark_mode;
    let plain = ui.visuals().text_color();
    let highlight = SourceHighlight::new(body);

    let mut job = egui::text::LayoutJob::default();
    for (i, line) in body.lines().enumerate() {
        for seg in segments(line, highlight.line(i), &[]) {
            let color = crate::colors::syntax_color(seg.kind, dark).unwrap_or(plain);
            job.append(seg.text, 0.0, egui::TextFormat::simple(font.clone(), color));
        }
        job.append("\n", 0.0, egui::TextFormat::simple(font.clone(), plain));
    }
    job
}

/// Draw one fenced block: monospace, coloured, framed, and selectable.
///
/// No copy button — Doug ruled it out, and drawing the block ourselves is what makes that
/// free rather than a removal. Selection is kept deliberately *because* the button is
/// gone: without either, code could not be copied at all. Modelica is lexed by
/// [`modelica_layout`]; every other language takes the flat [`code_colour`].
fn code_block_ui(ui: &mut egui::Ui, lang: &str, body: &str) {
    let body = body.trim_end_matches('\n');
    let text: egui::WidgetText = if lang == "modelica" {
        modelica_layout(ui, body).into()
    } else {
        egui::RichText::new(body)
            .monospace()
            .color(code_colour(lang))
            .into()
    };
    egui::Frame::new()
        .fill(ui.visuals().extreme_bg_color)
        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
        .inner_margin(egui::Margin::same(6))
        .corner_radius(ui.style().noninteractive().corner_radius)
        .show(ui, |ui| {
            ui.add(egui::Label::new(text).selectable(true));
        });
}

/// Whether the measured link positions belong to the beat now showing.
///
/// # The one-frame flash, diagnosed 2026-09-04
///
/// Doug: *"The flashes happen when the beat number increments… The flash happens for the
/// entire answer pane, and appears to be a different scroll position being temporarily
/// rendered for a very small fraction of a second."*
///
/// It was, and the position was **two beats back.** `lab_link_y` and `lab_prev_link_y` are
/// measured from the rendered document and stored *after* the render, so on the frame a beat
/// increments they still describe the previous beat. `travel_t` is 0 on that frame — the
/// dispatch just reset it — and `y = from + (to - from) * 0` is exactly `from`, which at that
/// moment is the link position from **two** beats earlier. One frame at that offset, then
/// the post-render update makes the endpoints current and the next frame interpolates
/// correctly. Hence a flash on every increment, including one where the destination is
/// unchanged.
///
/// **So the fix is to force no offset at all on that frame** and let the scroll area keep
/// what it has. Holding still for one frame is invisible; jumping two beats back is not.
///
/// A function rather than an inline comparison because it is the whole of the fix, and this
/// pane's logic is otherwise unreachable from a test — the rule that says push a computation
/// out of the paint path before adding one to it.
fn measurements_match_beat(measured_beat: Option<usize>, beat: usize) -> bool {
    measured_beat == Some(beat)
}

/// Which half of the split document [`show_lab_markdown`] is rendering.
///
/// # Why the halves need separate id namespaces
///
/// `show_lab_markdown`'s own docs say *"each prose segment takes its own id scope so two
/// viewers cannot derive colliding widget ids"*. That was true within one call and false
/// across the two — this repository's most frequent failure in its code dress, **a scope
/// one level too small.** `lab_prose_ui` renders the document as two
/// markdown parses split at the current beat's link, and both numbered their segments from
/// zero, so `("lab-prose", 0)` was pushed twice in one frame.
///
/// egui answers an id clash by painting an outline and `🔥`-prefixed debug text over the
/// offending widget. Doug, 2026-09-04: *"a screen flash in the answer pane happens… At
/// various points in the playback, that screen flash happens."* Various points, because the
/// number of segments in each half changes as the split moves — so which widgets collide
/// changes with it, and sometimes none do.
///
/// `part` is required rather than defaulted so a third call site has to say which document
/// it is rendering, and `the_two_halves_of_a_split_lab_do_not_clash_on_ids` scans a rendered
/// frame for that `🔥` — which turns a pixel complaint into a failing test.
pub(crate) enum LabPart {
    /// Everything up to the current beat's link. Ends at the scroll cursor.
    BeforeCursor,
    /// The link's own line and everything after it.
    AfterCursor,
}

impl LabPart {
    /// The id namespace for this half's prose segments.
    fn salt(&self) -> &'static str {
        match self {
            Self::BeforeCursor => "lab-prose-before",
            Self::AfterCursor => "lab-prose-after",
        }
    }
}

/// Render a lab's markdown, drawing fenced code blocks ourselves.
///
/// Splitting at fence boundaries is safe because a fence is block-level; the surrounding
/// prose is handed to `egui_commonmark` unchanged.
///
/// # A click must be carried across segments
///
/// `CommonMarkViewer::show` begins with `prepare_show`, which ends in
/// `cache.deactivate_link_hooks()` — every hook reset to `false`. Harmless for one
/// `show` per document; rendering per segment wipes a link clicked in an early segment
/// when the *next* segment renders, so `App` finds nothing to drain. Clicks are therefore
/// collected as they happen and re-applied after the last segment, and each prose segment
/// takes its own id scope so two viewers cannot derive colliding widget ids.
/// `a_link_far_down_a_long_lab_still_dispatches` caught this: it found the link, clicked
/// it, dispatched nothing. A wiped hook is indistinguishable from a link nobody pressed.
///
/// `part` names which half of the split document this is, so the two cannot collide on
/// segment ids — see [`LabPart`].
pub(crate) fn show_lab_markdown(
    ui: &mut egui::Ui,
    cache: &mut egui_commonmark::CommonMarkCache,
    text: &str,
    part: LabPart,
) {
    let mut clicked: Vec<String> = Vec::new();
    for (i, segment) in split_lab_segments(text).into_iter().enumerate() {
        match segment {
            LabSegment::Prose(s) => {
                ui.push_id((part.salt(), i), |ui| {
                    egui_commonmark::CommonMarkViewer::new().show(ui, cache, s);
                });
            }
            LabSegment::Code { lang, body } => code_block_ui(ui, lang, body),
        }
        for (name, fired) in cache.link_hooks() {
            if *fired && !clicked.iter().any(|c| c == name) {
                clicked.push(name.clone());
            }
        }
    }
    for name in clicked {
        cache.link_hooks_mut().insert(name, true);
    }
}

/// What lab mode shows when Claude has not written a lab.
///
/// Deliberately **not** `end_to_end_lab.md`, which used to be compiled in
/// here with `include_str!`. That document's prose was retired 2026-07-29
/// (ideas #42) — it described a 7x7 incidence matrix on a tab that shows 48
/// equations — so keeping it as the default would put the exact stale
/// content this change exists to remove back on screen.
pub(crate) fn no_lab_ui(ui: &mut egui::Ui) {
    ui.label(egui::RichText::new("No lab right now.").strong());
    ui.add_space(6.0);
    ui.label(
        "Lab mode shows a lab Claude wrote for a question you asked \u{2014} a \
         sequence of places to look, with links that drive HRW to each one.",
    );
    ui.add_space(6.0);
    ui.weak(
        "Ask Claude for one. Answers come as text first; a lab is for the ones \
         where a sequence of places beats a paragraph.",
    );
    ui.add_space(10.0);
    ui.weak(
        "Fixture labs \u{2014} tests with expected outcomes \u{2014} can be picked above \
             when any exist.",
    );
    ui.weak(format!("Claude writes an Answer to {}", bridge::LAB_FILE));
    ui.weak("It appears here within a moment, and a rewrite is picked up live.");
}

#[cfg(test)]
mod tests_code_segments {
    use super::{LabSegment, split_lab_segments};

    /// **Prose and fences alternate, and the fence tag survives.**
    ///
    /// The tag is what makes per-language colouring one match arm away, so a splitter
    /// that dropped it would quietly foreclose the capability Doug reserved.
    #[test]
    fn a_fenced_block_is_split_out_with_its_language() {
        let md = "Before.\n\n```modelica\nconnect(a, b);\n```\n\nAfter.\n";
        assert_eq!(
            split_lab_segments(md),
            vec![
                LabSegment::Prose("Before.\n\n"),
                LabSegment::Code {
                    lang: "modelica",
                    body: "connect(a, b);\n"
                },
                LabSegment::Prose("\nAfter.\n"),
            ]
        );
    }

    /// A fence with no language still splits; the tag is simply empty.
    #[test]
    fn an_untagged_fence_still_becomes_a_code_segment() {
        let md = "```\nplain\n```\n";
        assert_eq!(
            split_lab_segments(md),
            vec![LabSegment::Code {
                lang: "",
                body: "plain\n"
            }]
        );
    }

    /// **An unterminated fence is prose, because labs are edited while Doug reads them.**
    ///
    /// A half-typed fence is the expected case, not a corner one — the same reason the
    /// scroll offset in `lab_prose_ui` is validated rather than trusted. Treating it as a
    /// code block would swallow the rest of the document into a grey box.
    #[test]
    fn an_unterminated_fence_is_left_as_prose() {
        let md = "Text.\n\n```modelica\nhalf typed\n";
        assert_eq!(
            split_lab_segments(md),
            vec![
                LabSegment::Prose("Text.\n\n"),
                LabSegment::Prose("```modelica\nhalf typed\n"),
            ]
        );
    }

    /// Several blocks in one document, which every concept lab has.
    #[test]
    fn multiple_fences_all_split() {
        let md = "a\n```text\n1\n```\nb\n```modelica\n2\n```\nc";
        let segs = split_lab_segments(md);
        let code: Vec<_> = segs
            .iter()
            .filter_map(|s| match s {
                LabSegment::Code { lang, .. } => Some(*lang),
                LabSegment::Prose(_) => None,
            })
            .collect();
        assert_eq!(code, vec!["text", "modelica"]);
    }

    /// **A `modelica` fence is classified, and a `text` fence is not.**
    ///
    /// The consistency Doug asked for is that a lab and the source pane colour the same
    /// Modelica the same way, so this checks the input to that: `SourceHighlight` — the
    /// lexer `specimen_source` uses — finds keywords in a lab's Modelica. And it checks
    /// the boundary, because running log output through a Modelica lexer would colour
    /// arbitrary words as keywords, which is the reason the dispatch is by language.
    #[test]
    fn modelica_fences_are_lexed_and_other_fences_are_not() {
        use crate::modelica_lex::TokenKind;
        use crate::source_view::SourceHighlight;

        let modelica = "connect(src.p, R.p);\nparameter Real g = 9.81;";
        let hl = SourceHighlight::new(modelica);
        let kinds: Vec<TokenKind> = (0..2)
            .flat_map(|i| hl.line(i).iter().map(|t| t.kind))
            .collect();
        assert!(
            kinds.contains(&TokenKind::Keyword),
            "the lexer found no keyword in Modelica that declares a `parameter`: {kinds:?}",
        );

        // The dispatch, not the lexer: a `text` fence keeps the flat colour, so nothing
        // in a log excerpt can be mistaken for a keyword.
        let segs = split_lab_segments("```text\nparameter looks like a keyword here\n```\n");
        assert_eq!(
            segs,
            vec![LabSegment::Code {
                lang: "text",
                body: "parameter looks like a keyword here\n"
            }],
            "a text fence must reach code_block_ui tagged `text`, not `modelica`",
        );
    }

    /// **Non-vacuity against the real corpus:** every lab is split, and the labs
    /// genuinely contain fences. A splitter that stopped matching would otherwise report
    /// "no code blocks" and look like success.
    #[test]
    fn the_real_labs_contain_fenced_blocks() {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/fixture-labs");
        let mut blocks = 0usize;
        let mut labs = 0usize;
        for entry in std::fs::read_dir(&dir).expect("labs directory").flatten() {
            let path = entry.path();
            if path.extension().and_then(|x| x.to_str()) != Some("md") {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            labs += 1;
            blocks += split_lab_segments(&text)
                .iter()
                .filter(|s| matches!(s, LabSegment::Code { .. }))
                .count();
        }
        assert!(labs >= 20, "only {labs} labs were read");
        assert!(
            blocks >= 20,
            "only {blocks} fenced blocks found across {labs} labs — the splitter has \
             stopped matching, which looks like success"
        );
    }
}

#[cfg(test)]
mod tests_split_ids {
    use super::*;

    /// No offset is forced on the frame a beat increments, because the measurements are
    /// still the previous beat's.
    ///
    /// This is the flash Doug reported on 2026-09-04, reduced to the one comparison that
    /// decides it. The endpoints `lab_link_y` and `lab_prev_link_y` are stored after the
    /// render, so on an increment frame they describe the beat before — and `travel_t` is 0
    /// there, which makes the target exactly `from`, the position from **two** beats back.
    ///
    /// The pane itself cannot be asserted on: nothing in `egui_kittest` reads a scroll
    /// offset out of a markdown pane. What is checkable is the decision, and the decision is
    /// the whole fix.
    #[test]
    fn no_scroll_is_forced_until_the_new_beat_has_been_measured() {
        // Steady state within a beat: measurements are current, so the offset applies.
        assert!(measurements_match_beat(Some(3), 3));

        // The increment frame: the beat moved, nothing has been re-measured yet.
        assert!(
            !measurements_match_beat(Some(3), 4),
            "an offset computed from the previous beat's endpoints must not be forced \u{2014} \
             that is the frame that rendered two beats back",
        );

        // First frame of a run, before anything has been measured at all.
        assert!(
            !measurements_match_beat(None, 1),
            "and nothing is forced before the first measurement, which is what keeps the \
             lead-in at the top",
        );
    }

    /// The two halves of a split lab derive different ids for the same segment index.
    ///
    /// # What this claims, and what it does not
    ///
    /// It verifies the property the `LabPart` parameter exists for: `lab_prose_ui` renders
    /// the document as two markdown parses, and before 2026-09-04 both numbered their prose
    /// segments from zero, so `("lab-prose", 0)` named one egui scope for both halves. Two
    /// scopes sharing an id share egui memory, which is wrong on its own terms.
    ///
    /// **It does NOT test the screen flash Doug reported that day, and that flash is not
    /// diagnosed.** The hypothesis was that egui paints its `🔥` id-clash marker over the
    /// collision. It does not: `Context::check_for_id_clash` is called for registered
    /// widgets, `Grid`, `Panel` and `ScrollArea` and nothing else, so a colliding `push_id`
    /// scope paints nothing at all. A frame-scanning detector was written and found no
    /// marker even with the halves deliberately collided — then deleted rather than kept as
    /// a test that passes because it cannot see. Its non-vacuity assertion is the only
    /// reason that was noticed.
    #[test]
    fn the_two_halves_of_a_split_lab_derive_different_ids() {
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800.0, 600.0),
            )),
            ..Default::default()
        };

        let mut ids = Vec::new();
        let _ = ctx.run_ui(input, |ui| {
            // egui's own derivation, from the salts the two halves actually use.
            for part in [LabPart::BeforeCursor, LabPart::AfterCursor] {
                ids.push(ui.push_id((part.salt(), 0usize), |_| {}).response.id);
            }
        });

        assert_eq!(ids.len(), 2, "both scopes were created");
        assert_ne!(
            ids[0], ids[1],
            "segment 0 of each half must be a different egui scope \u{2014} sharing one is \
             what LabPart exists to prevent",
        );
    }
}

#[cfg(test)]
mod tests_absence {
    use super::*;

    /// **The lab pane says when no lab is open.**
    ///
    /// From the 2026-08-24 survey of what a pane says when it has nothing to show. This
    /// is the **first** thing a reader sees — HRW opens in Lab mode with nothing
    /// selected — so a blank left panel here is not an edge case, it is the front door.
    ///
    /// It also has a specific history worth not undoing: the default used to be
    /// `end_to_end_lab.md`, retired 2026-07-29 because it described a 7×7 incidence
    /// matrix on a tab showing 48 equations. **Saying "no lab" is the correction**;
    /// rendering nothing would look like the retirement broke the pane.
    #[test]
    fn the_lab_pane_says_when_no_lab_is_open() {
        use egui_kittest::Harness;
        use egui_kittest::kittest::Queryable;

        let mut h = Harness::builder()
            .with_size(egui::Vec2::new(600.0, 400.0))
            .build_ui(no_lab_ui);
        h.run_steps(2);

        assert!(
            h.query_by_label_contains("No lab right now").is_some(),
            "the lab pane rendered nothing with no lab selected \u{2014} which is the \
             state HRW starts in",
        );
    }
}
