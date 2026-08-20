//! **The tour panel** — the whole left-hand column in Tour mode: the transport bar
//! at the top (pick a tour, go back, play, pause, stop, and the running readout),
//! and the scrolling prose beneath it.
//!
//! Lifted out of `app.rs` on 2026-08-19 in two steps — the bar first, then the prose
//! — which is why the file was called `tour_transport.rs` for an afternoon. See
//! [`docs/app-split-plan.md`](../docs/app-split-plan.md).
//!
//! # What stayed in `App`, and it is the seam
//!
//! `App::tour_panel_ui` still owns the *panel*: polling the tour file, configuring
//! the draggable `SplitState`, performing a transport press, and draining the
//! markdown link hooks. What moved is everything **inside** the scroll area — and
//! that half touches exactly two pieces of state, [`TourState`] and the commonmark
//! cache, with no reach into `App` at all.
//!
//! **So [`tour_prose_ui`] returns nothing.** The two extractions before it had to
//! invent a report — `Option<String>` for a clicked identifier, [`TransportRequest`]
//! for a press — because they contained a decision the pane could not make. This one
//! contains none: it renders a document and records where it ended up. That is worth
//! naming, because *"which half of this function has no policy in it"* turned out to
//! be a better seam-finder than the field count was.
//!
//! # What the bar's signature buys
//!
//! The bar touched **six** fields, and eighteen of its twenty `self` accesses were
//! `self.tour`. So the whole bar reduces to one `&mut TourState` — the state that
//! already lives in [`crate::tour`] — plus `compiling`, which the readout names
//! because a walk holds its clock while a specimen builds. **That is the cheapest
//! signature any extraction here has produced**, and it is cheap for a reason worth
//! recording: the state had already been grouped. The 2026-08-02 UI pause that
//! created `TourState` is what made this move a four-parameter one instead of a
//! twenty-field one.
//!
//! # The three things it cannot do itself, and how they leave
//!
//! Back, Play and Stop each reach past the tour into the *application* — history and
//! the tour file, the beat schedule and the dispatcher, the UI mode a run borrowed.
//! None of that is the bar's business, so the bar **reports the press** as a
//! [`TransportRequest`] and `App` performs it. This is the same shape
//! [`crate::specimen_source`] uses for a clicked identifier: **render and report, own
//! no policy**.
//!
//! **Two consequences, stated rather than hidden.**
//!
//! - **Stop still stops the clock here.** `Autoplay::stop` is pure `TourState`, so
//!   leaving it to `App` would have let the readout below render one more frame of a
//!   run that had ended. Only the *mode restore* is deferred, which is why the variant
//!   is [`TransportRequest::Stopped`] — a report that it happened, not a request to do
//!   it.
//! - **Play is deferred by exactly one frame.** `App::start_autoplay` parses the tour,
//!   builds a schedule and dispatches the first beat, and cannot run mid-paint. So on
//!   the click frame the length picker is still enabled and the progress bar is not yet
//!   drawn; the next frame — which egui paints immediately after an interaction — is
//!   correct. The alternative was handing this module the dispatcher.
//!
//! # The constants moved with the view
//!
//! `TOUR_PROGRESS_HEIGHT` and `TOUR_PROGRESS_MARGIN` are used by exactly this readout,
//! and `TOUR_CONTEXT_ABOVE` by exactly the prose scroll; the same rule that moved
//! `SOURCE_MAP_SPLIT_FRACTION` into [`crate::source_map`] applies: state used by one
//! pane is state that pane owns. Leaving them behind would reduce what `app.rs`
//! *holds* without reducing what it *declares*.

use eframe::egui;

use crate::app::{section_style, set_markdown_text_sizes};
use crate::bridge;
use crate::tour::{TourSource, TourState};

/// Height of the autoplay progress bar, and the clear space above and below it.
///
/// The bar draws its percentage *inside* itself, so it has to be tall enough for a
/// line of text rather than tall enough for a rule. At 6px with no surrounding space
/// it was clipped between the transport row and the stop caption.
const TOUR_PROGRESS_HEIGHT: f32 = 18.0;
const TOUR_PROGRESS_MARGIN: f32 = 6.0;

/// A press on the transport that only [`crate::app::App`] can carry out.
///
/// **The bar never acts on these itself.** Each one reaches state the tour panel does
/// not own — the tour file, the beat dispatcher, the UI mode a run borrowed — so the
/// bar reports and the application decides. See the module docs for why
/// [`Self::Stopped`] is phrased as a report rather than a request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TransportRequest {
    /// A different tour was chosen — from the picker, or from the ad hoc button.
    Switch(TourSource),
    /// **Back** — return to the tour a cross-tour link was followed from.
    Back,
    /// **Play** — build a schedule from the showing tour and start the clock.
    Play,
    /// **Stop was pressed and the clock is already stopped.** What remains is the UI
    /// mode the run borrowed, which only `App` can put back.
    Stopped,
}

/// **The Play button** — transport for a self-running tour.
///
/// Built 2026-08-03 for recording a walk as video, after a LinkedIn screenshot
/// drew interest and explaining *what a tour is* to people who have never seen
/// HRW proved harder in prose than in motion.
///
/// The controls sit between the tour list and the tour text rather than in the
/// menu bar, because they belong to the tour showing below them and a recording
/// should show the viewer where the run came from.
///
/// **Everything decidable lives in [`crate::autoplay`]**; this function only
/// draws. That split is what lets the schedule and the clock be tested without
/// a window, which for a *timing* feature is the difference between a checkable
/// claim and a stopwatch.
///
/// The tour transport bar: **which tour**, then **the walk**.
///
/// Returns the press that `App` must carry out, if there was one — see
/// [`TransportRequest`]. `tour_text` is the showing tour’s markdown, which the bar
/// needs for two things: whether Play is enabled at all, and re-parsing the stop
/// headings for the running caption.
///
/// # Why the picker is in here
///
/// Doug, 2026-08-16, after the 23-row list became a combo box and a button: *"The
/// divider bar above … no longer makes sense. It's the divider bar which currently
/// says 'Tours (23)'."* A titled bar wrapping two controls is chrome announcing
/// chrome, and it cost a header, a separator and the space between them for no
/// information — the count it carried is now in the picker's own hover.
///
/// Left to right, in the order he specified: **Claude's answer**, the **picker**,
/// then the transport. That reads as a sentence — *which tour, then what to do with
/// it* — and it puts the one control whose state changes mid-conversation at the
/// end of the eye's travel from the prose below.
pub(crate) fn autoplay_controls_ui(
    ui: &mut egui::Ui,
    tour: &mut TourState,
    compiling: bool,
    tour_text: &Option<String>,
) -> Option<TransportRequest> {
    use crate::autoplay::Phase;

    let mut request: Option<TransportRequest> = None;
    let has_tour = tour_text.is_some();
    let phase = tour.autoplay.phase();

    // **Same palette as the section headers above it** (Doug, 2026-08-03). The
    // transport is a left-panel *bar*, like "Tours (8)" and "Specimens", not a
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
                // --- Back: undo a cross-tour link ---
                //
                // Doug, 2026-08-19: *"while in the index reduction tour, I can click a
                // link to navigate to the blt-ordering tour, but then I cannot navigate
                // back."* Placed where the tour count used to be, at his suggestion.
                //
                // **This does not consume the RHS Back/Forward** reserved in `ideas.md`
                // #78. It is the tour panel's history; the RHS pair belongs in the stage
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
                // the tour count, the duration words and the time combo's default width
                // were removed.
                let back_to = tour.history.last().map(|(source, _)| source.label());
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
                        "Nothing to go back to \u{2014} follow a link into another tour \
                         and this returns you here",
                    );
                }

                // --- Which tour: Claude's answer, then the picker ---
                //
                // **Claude's answer is not the same kind of object as the other 22**
                // (Doug, 2026-08-16). They are committed, versioned, machine-checked
                // and citable as `hrw://tour/<name>/stop/<slug>`; this one is
                // `.hrw-bridge/tour.md` — gitignored, regenerated per question, and
                // there is only ever one. `tour::poll` already privileged it by
                // auto-selecting it; only the presentation had flattened it into a row.
                //
                // Leftmost, so its state — present, absent, selected — is answerable
                // without opening anything. That was the 2026-08-15 defect: the row
                // was correctly absent and read as a broken feature.
                let has_ad_hoc = tour.available.contains(&TourSource::AdHoc);
                let ad_hoc_selected = tour.selected.as_ref() == Some(&TourSource::AdHoc);
                // `Button::selectable`, not a plain button: this *selects* what you
                // are reading. A verb-shaped control here would imply otherwise.
                let resp = ui.add_enabled(
                    has_ad_hoc,
                    egui::Button::selectable(ad_hoc_selected, TourSource::AdHoc.label()),
                );
                if has_ad_hoc {
                    if resp
                        .on_hover_text(
                            "Written by Claude to answer your last question.                              Ephemeral: regenerated, never stored.",
                        )
                        .clicked()
                    {
                        request = Some(TransportRequest::Switch(TourSource::AdHoc));
                    }
                } else {
                    resp.on_disabled_hover_text(
                        "No answer written yet. Ask Claude a question and it writes one                          here — regenerated per question, never stored.",
                    );
                }

                let selected_label = match tour.selected.as_ref() {
                    Some(TourSource::Fixture(p)) => TourSource::Fixture(p.clone()).label(),
                    _ => "Pick a tour…".to_owned(),
                };
                // **The count moved here from the deleted header.** It existed because
                // "I do not see the new tour" must be answerable at a glance rather
                // than by reasoning: a number distinguishes "the directory has six"
                // from "the pane is showing six of eight", and those need opposite
                // fixes. One hover keeps that without a titled bar around it.
                let n_fixtures = tour
                    .available
                    .iter()
                    .filter(|s| matches!(s, TourSource::Fixture(_)))
                    .count();
                egui::ComboBox::from_id_salt("tour_picker")
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
                    .width((bar_width * 0.45).clamp(60.0, 220.0))
                    .show_ui(ui, |ui| {
                        // **The overview sorts first, set apart** — the reasoning and the
                        // rejected alternatives are on `TourState::picker_order`.
                        let (ordered, hoisted) = tour.picker_order();

                        for (i, source) in ordered.into_iter().enumerate() {
                            if i == hoisted && hoisted > 0 {
                                ui.separator();
                            }
                            let TourSource::Fixture(path) = source else {
                                // The ad hoc tour has its own control to the left;
                                // listing it twice would make one of them a lie about
                                // where it lives.
                                continue;
                            };
                            let selected = tour.selected.as_ref() == Some(source);
                            // **The entry names its specimens.** Tours are named by
                            // phase, but Doug searches by the model in front of him and
                            // went looking for a "DimensionMismatch tour" that is called
                            // `failure-typecheck` (2026-08-05). Showing both axes removes
                            // the search, and it has to survive every move of this
                            // control or the friction comes back.
                            let label = tour.row_specimens.get(path).map_or_else(
                                || source.label(),
                                |sp| format!("{}  ·  {sp}", source.label()),
                            );
                            if ui
                                .selectable_label(selected, label)
                                .on_hover_text(format!(
                                    "Fixture tour — a test with expected outcomes,                                      kept and versioned.
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
                        "{n_fixtures} fixture tours in {}",
                        bridge::FIXTURE_TOURS_DIR
                    ));

                // **The count stays visible, in three words instead of a titled bar.**
                //
                // It was the whole reason the deleted "Tours (23)" header existed:
                // "I do not see the new tour" has to be answerable at a glance, because
                // a number distinguishes *the directory has six* from *the pane is
                // showing six of eight*, and those need opposite fixes. Doug reported
                // that on 2026-08-03 with a picker test asserting the tours were on
                // screen — the code was provably right and the report was still true.
                //
                // A hover was tried first and is not enough: a tooltip is invisible
                // until suspicion already exists, which is exactly too late, and it is
                // also unreachable from the accessibility tree so no test can see it.
                //
                // **REMOVED 2026-08-19.** Doug: *"I have not used that label's
                // information a single time"* — sixteen days, by the person it was built
                // for, which is better evidence than the reasoning above. The count
                // survives on the picker's hover; if *"I don't see the new tour"* ever
                // recurs, restoring this line is the fix.
                // <!-- unbuilt: visible_tour_count -->

                ui.separator();

                match phase {
                    Phase::Playing | Phase::Paused => {
                        let (label, hover) = if phase == Phase::Playing {
                            ("\u{23f8} Pause", "Hold the walk here.")
                        } else {
                            ("\u{25b6} Resume", "Continue from this beat.")
                        };
                        if ui.button(label).on_hover_text(hover).clicked() {
                            if phase == Phase::Playing {
                                tour.autoplay.pause();
                            } else {
                                tour.autoplay.resume();
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
                            tour.autoplay.stop();
                            request = Some(TransportRequest::Stopped);
                        }
                    }
                    Phase::Idle | Phase::Finished => {
                        let play = ui
                            .add_enabled(has_tour, egui::Button::new("\u{25b6} Play"))
                            .on_hover_text(
                                "Walk this tour by itself, for recording. The clock pauses \
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
                ui.add_enabled_ui(!tour.autoplay.is_running(), |ui| {
                    let current = crate::autoplay::TOTAL_CHOICES
                        .iter()
                        .find(|(_, s)| *s == tour.autoplay_total.as_secs())
                        .map(|(l, _)| *l)
                        .unwrap_or("custom");
                    egui::ComboBox::from_id_salt("autoplay_total")
                        // **Sized to its labels.** With no `width` egui applies its
                        // default `combo_width` of 100pt, which was invisible while the
                        // labels read `90s — standard` and became obvious the moment they
                        // became `90s`. Doug: *"Why is the time selector combobox so wide?
                        // It is much wider than necessary for the labels."*
                        //
                        // A constant is safe *here* although it was not for the tour
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
                                ui.selectable_value(&mut tour.autoplay_total, d, label);
                            }
                        })
                        .response
                        .on_hover_text(
                            "Total length of the walk. Conventional social-video lengths \
                             \u{2014} pick to fit where it is going.",
                        );
                });
            });

            if !tour.autoplay.is_running() {
                return;
            }

            // --- The running readout ---
            //
            // A caption naming the stop, because a recording is watched by people who
            // cannot see the cursor and have no idea which part of the tour they are in.
            let (beat, total) = tour.autoplay.progress();
            let phase = tour.autoplay.phase();
            // **Margin above and below.** At 6px with no spacing the bar was clipped by
            // its neighbours and its percentage was only half legible — Doug, 2026-08-03:
            // *"the progress bar is not entirely visible because not enough vertical
            // space is being provided"*. The bar carries the percentage text, so it needs
            // room for a line of text, not for a rule.
            ui.add_space(TOUR_PROGRESS_MARGIN);
            ui.add(
                egui::ProgressBar::new(tour.autoplay.fraction())
                    .desired_height(TOUR_PROGRESS_HEIGHT)
                    .show_percentage(),
            );
            ui.add_space(TOUR_PROGRESS_MARGIN);
            // The caption takes the header's `active_color` and the status line its
            // `inactive_color`, so the bar reads as one element with a primary and a
            // secondary line — the same relationship the section headers already have.
            if let Some(caption) = tour
                .autoplay
                .current_stop()
                .and_then(|i| autoplay_stop_heading(tour_text.as_deref(), i))
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
                        _ if compiling => "compiling \u{2014} clock held",
                        _ => "playing",
                    }
                ))
                .color(style.inactive_color),
            );
        });
    request
}

/// The heading of stop `index` in the tour text, for the running caption.
///
/// Re-parsed rather than stored with the schedule: the tour file is re-read
/// whenever it changes on disk, and a caption cached at Play time would keep
/// naming a stop that had since been rewritten. Doug regenerates tours *while*
/// walking them, which makes that the normal case rather than an edge one.
///
/// **It never used `self`**, which is why it left `App` as a free function rather
/// than acquiring a parameter.
fn autoplay_stop_heading(text: Option<&str>, index: usize) -> Option<String> {
    let stops = crate::autoplay::parse_stops(text?);
    stops.get(index).map(|s| s.heading.clone())
}

/// How much tour text to keep **above** the link a beat is dispatching.
///
/// Doug, 2026-08-03: *"the scrolling should be paused with that frame link showing
/// with perhaps a line or two of text which is above that frame link. The frame link
/// and the lines of text above the link document the animation frame."*
///
/// Roughly two lines. Scrolling the link to the very top would put its introduction
/// off-screen, and the pair — lead-in and link — is what names the frame.
const TOUR_CONTEXT_ABOVE: f32 = 48.0;

/// The tour prose — the scrolling markdown beneath the transport bar.
///
/// Everything inside the panel's scroll area, and **nothing outside it**: the panel
/// itself, the split, the transport press and the link hooks stay in
/// `App::tour_panel_ui`. The division is not a line count — it is that this half
/// reaches nothing but [`TourState`] and the markdown cache, so it can be a free
/// function with four parameters and no return value.
///
/// `tour_text` is the document as of this frame, already read from disk by the
/// caller's poll. `None` means no tour is loaded, which is a state with prose of its
/// own rather than an empty pane — see [`no_tour_ui`].
pub(crate) fn tour_prose_ui(
    ui: &mut egui::Ui,
    tour: &mut TourState,
    cache: &mut egui_commonmark::CommonMarkCache,
    tour_text: &Option<String>,
) {
    // **The prose scrolls with the walk.**
    //
    // Without this the stage side moves while the tour text sits at the
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
    // displayed"*, which was worst exactly where the tour is best, with a
    // deliberately paused animation under sliding prose. `scroll_fraction`
    // travels to the new beat's place and then stops.
    //
    // **Only while running.** Forcing the offset when idle would fight a
    // reader who scrolled somewhere themselves.
    // **`both`, not `vertical`, and the horizontal axis is load-bearing**
    // (Doug, 2026-08-12: *"the divider does not move. Instead, only the
    // right edge of the LHS tour content moves"*).
    //
    // A vertical-only scroll area reports its content's **full width** as
    // the width it wants, and `egui_commonmark` does not wrap tables or
    // code blocks — `the-concepts.md` has a 178-character line. So the
    // tour panel's intrinsic minimum width became the widest table in the
    // document, egui sized the panel to it, and the divider had nothing
    // left to give:
    //
    // ```text
    // no tour loaded    panel opens 512pt (the 40% default), drags to 213pt
    // real tour loaded  panel opens 899pt and is FROZEN; the inner Ui still
    //                   follows the pointer, so the gap reached 705pt
    // ```
    //
    // Enabling the horizontal axis makes wide content **scroll instead of
    // push**, so the panel keeps the width the reader chose and the table
    // is still reachable. Wrapping is not the alternative: a Markdown table
    // does not wrap into anything readable.
    //
    // Note what this cost before it was found: the tour panel was quietly
    // taking 70 % of a 1280pt window rather than the 40 % it reports.
    let mut area = egui::ScrollArea::both().id_salt("tour");

    // **A new tour opens at its top.**
    //
    // `id_salt("tour")` is stable on purpose — it is what lets a reader's
    // scroll position survive a repaint — and the cost is that it survives
    // the *document* too. Doug, 2026-08-17: *"When I click a subordinate
    // tour link in the-concepts hub tour, the subordinate tour opens
    // partially scrolled down instead of fully scrolled to the top."*
    //
    // The hub is the worst case and the reason it surfaced now: its links
    // sit in a ten-row table you scroll down to reach, so the tour that
    // opened inherited however far down the row was.
    //
    // **Consumed only on a frame that has text.** Switching clears `cached`,
    // so the frame in between renders nothing — clearing the flag there
    // would spend it on a document that was never drawn.
    //
    // **A pending stop request outranks this**, and both are live at once
    // by construction: `hrw://tour/<name>/stop/<slug>` switches the tour
    // (which asks for the top) and *then* asks for the stop. The top
    // request is still consumed — leaving it pending would fire it at the
    // next document — it simply does not get to move anything.
    let stop_pending = tour.scroll_to_offset.is_some();
    // **Back outranks both**: returning to where you were is the one
    // navigation for which the top of the document is the wrong answer.
    let restore = tour.restore_scroll_y.take();
    if tour.scroll_to_top && tour_text.is_some() {
        tour.scroll_to_top = false;
        if !stop_pending && restore.is_none() {
            area = area.vertical_scroll_offset(0.0);
        }
    }
    match (restore, tour_text.is_some()) {
        (Some(y), true) => area = area.vertical_scroll_offset(y),
        // No text this frame, so the request has nothing to apply to and must
        // survive — the one-shot discipline `scroll_to_top` already follows.
        (Some(y), false) => tour.restore_scroll_y = Some(y),
        (None, _) => {}
    }

    if tour.autoplay.is_running()
        && let Some(max_scroll) = tour.tour_max_scroll
    {
        // **Interpolate between two MEASURED positions.** Both come from
        // the split below, so neither is an estimate of anything.
        let to = tour.tour_link_y.unwrap_or(0.0);
        let from = tour.tour_prev_link_y.unwrap_or(0.0);
        let y = from + (to - from) * tour.autoplay.travel_t();
        // Leave a little above the link, so the line or two introducing
        // it stays on screen with it. Doug: "the frame link and the lines
        // of text above the link document the animation frame."
        let target = (y - TOUR_CONTEXT_ABOVE).clamp(0.0, max_scroll.max(0.0));
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
    // split falls on a line start, and every link in these tours is its
    // own paragraph, so no markdown construct is cut in half.
    let mut measured: Option<f32> = None;
    let out = area.show(ui, |ui| {
        set_markdown_text_sizes(ui);
        match tour_text {
            Some(text) => {
                // **Only split while a walk is running.**
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
                // that feature costs almost nothing here. `hrw://tour/<t>/
                // stop/<slug>` records the heading's *byte* offset, and the
                // problem it faces is the one the autoplay scroll already
                // solved the hard way: a byte offset cannot be converted to
                // a pixel position by arithmetic, because rendered height
                // per character is not constant.
                //
                // Splitting there puts the **cursor** exactly at the stop,
                // and a cursor is a real position rather than an estimate.
                //
                // **The offset is validated, not trusted.** A tour is
                // re-read whenever its mtime changes, so an offset recorded
                // against the previous text can land mid-character after an
                // edit — and slicing a `str` off a char boundary panics.
                // These documents are edited *while* Doug walks them, so
                // that is the expected case rather than a corner one.
                let stop_split = tour
                    .scroll_to_offset
                    .filter(|n| *n <= text.len() && text.is_char_boundary(*n));
                let split = if tour.autoplay.is_running() {
                    tour.autoplay.current_byte_offset().min(text.len())
                } else {
                    stop_split.unwrap_or(0)
                };
                let top = ui.cursor().top();
                if split > 0 {
                    egui_commonmark::CommonMarkViewer::new().show(ui, &mut *cache, &text[..split]);
                }
                measured = Some(ui.cursor().top() - top);

                // **egui does the scrolling, and that is the whole trick.**
                // Asking the `ScrollArea` for an offset would mean computing
                // one; asking it to bring the cursor into view means it
                // computes one, from a position it already knows exactly.
                //
                // A run in progress owns the scroll, so a stop request is
                // still *consumed* but not acted on — otherwise it would
                // fight the interpolation for the rest of the walk.
                if tour.scroll_to_offset.take().is_some()
                    && stop_split.is_some()
                    && !tour.autoplay.is_running()
                {
                    ui.scroll_to_cursor(Some(egui::Align::TOP));
                }

                egui_commonmark::CommonMarkViewer::new().show(ui, &mut *cache, &text[split..]);
            }
            None => no_tour_ui(ui),
        }
    });

    // A new beat means a new split, so the position measured last frame
    // becomes the one to travel *from*.
    let beat = tour.autoplay.progress().0;
    if tour.tour_measured_beat != Some(beat) {
        tour.tour_prev_link_y = tour.tour_link_y.or(Some(0.0));
        tour.tour_measured_beat = Some(beat);
    }
    tour.tour_link_y = measured;
    tour.tour_max_scroll = Some((out.content_size.y - out.inner_rect.height()).max(0.0));
    // **Where the reader actually is**, from the scroll area's own output
    // rather than tracked alongside it, so it cannot drift from the screen.
    // Every frame, because a switch can happen at any moment and the offset
    // must be the one from just before it.
    tour.current_scroll_y = out.state.offset.y;
}

/// What tour mode shows when Claude has not written a tour.
///
/// Deliberately **not** `end_to_end_tour.md`, which used to be compiled in
/// here with `include_str!`. That document's prose was retired 2026-07-29
/// (ideas #42) — it described a 7x7 incidence matrix on a tab that shows 48
/// equations — so keeping it as the default would put the exact stale
/// content this change exists to remove back on screen.
pub(crate) fn no_tour_ui(ui: &mut egui::Ui) {
    ui.label(egui::RichText::new("No tour right now.").strong());
    ui.add_space(6.0);
    ui.label(
        "Tour mode shows a tour Claude wrote for a question you asked \u{2014} a \
         sequence of places to look, with links that drive HRW to each one.",
    );
    ui.add_space(6.0);
    ui.weak(
        "Ask Claude for one. Answers come as text first; a tour is for the ones \
         where a sequence of places beats a paragraph.",
    );
    ui.add_space(10.0);
    ui.weak(
        "Fixture tours \u{2014} tests with expected outcomes \u{2014} can be picked above \
             when any exist.",
    );
    ui.weak(format!(
        "Claude writes an ad hoc tour to {}",
        bridge::TOUR_FILE
    ));
    ui.weak("It appears here within a moment, and a rewrite is picked up live.");
}
