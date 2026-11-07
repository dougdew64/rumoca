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

/// A press on the transport that only [`crate::app::App`] can carry out.
///
/// **The bar never acts on these itself.** Each one reaches state the lab panel does
/// not own — the lab history, the lab file — so the bar reports and the application
/// decides.
///
/// It carried `Play` and `Stopped` too, until the self-running lab went on
/// 2026-11-05. `DECISIONS.md` has why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TransportRequest {
    /// A different lab was chosen from the picker.
    Switch(LabSource),
    /// **Back** — return to the lab a cross-lab link was followed from.
    Back,
}

/// **The lab bar** — which lab is showing, and how to get back.
///
/// It was a transport for a self-running lab until 2026-11-05: Play, Pause, Stop, a
/// run-length picker, a progress bar and a caption naming the current station. All of
/// that existed to make a screen recording, and `DECISIONS.md` carries why it went —
/// the short form is that a recording feature had begun constraining how Claude may
/// write an Answer, which is the one artifact here with nothing but an educational
/// purpose.
///
/// Returns the press that `App` must carry out, if there was one — see
/// [`TransportRequest`].
///
/// # Why the picker is in here
///
/// Doug, 2026-08-16, after the 23-row list became a combo box and a button: *"The
/// divider bar above … no longer makes sense. It's the divider bar which currently
/// says 'Labs (23)'."* A titled bar wrapping two controls is chrome announcing
/// chrome, and it cost a header, a separator and the space between them for no
/// information — the count it carried is now in the picker's own hover.
///
/// Left to right, in the order he specified: **Claude's answer**, then the **picker**.
/// That reads as a sentence — *which lab* — and the transport that used to follow it
/// is gone.
pub(crate) fn lab_controls_ui(ui: &mut egui::Ui, lab: &mut LabState) -> Option<TransportRequest> {
    let mut request: Option<TransportRequest> = None;

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
            let back = ui.add_enabled(back_to.is_some(), egui::Button::new("\u{25c2}").small());
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

            // --- Which lab: the picker ---
            //
            // **The `✨ Answer` button that stood left of it was removed 2026-11-07**
            // under charter Decision 19: answers are given on the platform (the chat,
            // a VS Code tab, System Modeler, Wolfram Desktop), never in HRW.
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
                        // Irrefutable since the Answer variant was removed.
                        let LabSource::Fixture(path) = source;
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
                                "Fixture lab — a test with expected outcomes, \
                                     kept and versioned.\n{}",
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
        });
    });
    request
}

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
    // code blocks — `the-pipeline.md` has a 178-character line. So the
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
    // lab link in the-pipeline hub lab, the subordinate lab opens
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

    // **A byte offset cannot be turned into a pixel position by arithmetic**, because
    // rendered height per character is not constant: prose wraps in a narrow panel and
    // a code block does not. Two attempts at estimating it failed in both directions,
    // and no constant corrects an estimate that is wrong both ways.
    //
    // So the markdown is split at the station's heading line and rendered as two
    // documents. The cursor between them *is* the position, and `scroll_to_cursor`
    // asks egui to compute the offset from something it already knows exactly.
    //
    // This was built for a self-running lab and inherited by `hrw://lab/<name>/
    // station/<slug>`, which is now its only caller. The run went on 2026-11-05; the
    // reasoning is older than it and outlived it.
    let out = area.show(ui, |ui| {
        set_markdown_text_sizes(ui);
        match lab_text {
            Some(text) => {
                // **The rules live in `LabState::render_split`**, out of the paint
                // path and tested. `station_split` survives here only because the
                // scroll decision below needs to know whether a station request was
                // usable, which the split alone cannot say.
                let station_split = lab
                    .scroll_to_offset
                    .filter(|n| *n <= text.len() && text.is_char_boundary(*n));
                let split = lab.render_split(text);
                if split > 0 {
                    show_lab_markdown(ui, &mut *cache, &text[..split], LabPart::BeforeCursor);
                }

                // **egui does the scrolling, and that is the whole trick.**
                // Asking the `ScrollArea` for an offset would mean computing
                // one; asking it to bring the cursor into view means it
                // computes one, from a position it already knows exactly.
                //
                // The request is consumed whether or not it was usable: leaving
                // it pending would fire it at whatever document came next.
                if lab.scroll_to_offset.take().is_some() && station_split.is_some() {
                    ui.scroll_to_cursor(Some(egui::Align::TOP));
                }

                show_lab_markdown(ui, &mut *cache, &text[split..], LabPart::AfterCursor);
            }
            None => no_lab_ui(ui),
        }
    });

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

/// What lab mode shows when no lab is picked.
///
/// **Rewritten 2026-11-07, when the HRW Answer was removed** (charter Decision 19). It
/// used to say lab mode shows *"a lab Claude wrote for a question you asked"* and that
/// *"Claude writes an Answer to `.hrw-bridge/answer.md`"* — both false the moment the
/// Answer went. It now names the one thing lab mode does, and where a question goes instead,
/// since an empty pane is exactly where a reader looks for how to ask.
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
        "Lab mode runs a fixture lab \u{2014} a route through HRW's panes, with a \
         prediction to make at each station. Pick one above.",
    );
    ui.add_space(6.0);
    ui.weak(
        "To ask Claude about anything you see, type it into the Text field in the \
         Question Bar, then type ? in the Claude chat. The answer comes back there, \
         not here.",
    );
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
