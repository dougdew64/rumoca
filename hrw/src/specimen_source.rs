//! **The specimen source pane** — one file's Modelica text, highlighted and clickable.
//!
//! Lifted out of `app.rs` on 2026-08-19, the second *rendering* function to leave, and
//! the first that had to be moved **by hand**. See
//! [`docs/app-split-plan.md`](../docs/app-split-plan.md).
//!
//! # Why the first attempt was reverted
//!
//! It was rewritten by regex, and four things defeated that in sequence: a method
//! call (`set_tracked_identifier`) rather than a field, `self` accesses split across
//! lines by rustfmt, a parameter that turns out to need `&mut`, and finally a local
//! `let source = …` **shadowing** the parameter of the same name — which a regex
//! cannot tell apart from the parameter. The local is now `source_text`, and the
//! parameter keeps the name the state has on `App`.
//!
//! **The lesson recorded in the plan is about method, not difficulty:** read the body
//! before estimating, because whether an extraction is mechanically rewritable decides
//! its cost far more than how many fields it touches.
//!
//! # The signature is the point
//!
//! Seven pieces of state instead of `&mut self`, and the click goes back the other
//! way as a return value. **`App` performs the follow**, exactly as it already does
//! for `model_list` — so this module renders and reports, and owns no policy about
//! what following an identifier means.
//!
//! [`SourceViewState`] moved with the view, for the same reason
//! `SOURCE_MAP_SPLIT_FRACTION` moved into [`crate::source_map`]: state used by exactly
//! one pane is state that pane owns.

use std::path::PathBuf;

use eframe::egui;

use crate::identifier_index as ident_index;

/// Everything the **source view** owns.
///
/// One file's text on screen, plus everything about *how it is being shown*: the
/// lexed highlight, which line a jump is heading for, where the pane is scrolled,
/// and — for a library model — which file the text came from or why it could not
/// be read.
///
/// # Seven fields, not eight
///
/// `identifier_index` looks like it belongs here and does not. It is a **compile
/// output**, built by the worker and read by `source_map_ui` as well, so it lives
/// with the other results. The test for membership is not "does the source view
/// use it" but **"is it nobody else's business"**.
///
/// # Why `text` is `Option<String>` and not a path
///
/// A specimen's text is re-read from disk each time so live edits show. A library
/// model's cannot be: Rumoca discards source-root text, so the worker reads the
/// declaring file once and hands it over. `library_uri` says which file that was,
/// which the reader needs — `Resistor` lands you inside `Basic.mo` among dozens
/// of classes.
#[derive(Default)]
pub(crate) struct SourceViewState {
    /// The text on screen. For a specimen, re-read from disk; for a library
    /// model, seeded by the worker from the declaring file.
    pub(crate) text: Option<String>,
    /// Why the disk read failed, when it did.
    ///
    /// **Added by the 2026-08-04 sweep.** The read was
    /// `read_to_string(path).unwrap_or_default()`, so a failure produced an **empty
    /// string that then got cached** — and the pane's fallback arm said *"Select a
    /// specimen to view its source"* while a specimen was selected. A read failure
    /// was rendered as a different, plausible, false claim.
    ///
    /// It also doubles as the retry guard: without it, a file that cannot be read
    /// would be re-read on **every frame**, which is a filesystem call in the paint
    /// path (see the debugging conventions in `CLAUDE.md`).
    pub(crate) load_error: Option<String>,
    /// Lexed spans for [`Self::text`], rebuilt whenever the text changes.
    pub(crate) highlight: Option<crate::source_view::SourceHighlight>,
    /// Which library file [`Self::text`] came from, when it is a library model.
    pub(crate) library_uri: Option<String>,
    /// Why the declaring file could not be read, when it could not.
    ///
    /// Kept apart from the text so the pane distinguishes *"unreadable"* from
    /// *"nothing selected"*. Both would otherwise render the same blank.
    pub(crate) library_error: Option<String>,
    /// A line a link or a declaration jump is heading for, consumed on arrival.
    pub(crate) scroll_target: Option<u32>,
    /// The line a jump landed on, washed so it can be found after the scroll.
    ///
    /// **Distinct from `scroll_target`, which is consumed on arrival.** Doug,
    /// 2026-08-05, after "Show in the Modelica source" shipped: *"Would it be
    /// possible to add visual highlighting of the item being shown in the source?"*
    /// Scrolling puts the line somewhere in the pane; **it does not say which line**,
    /// and a reader arriving in a 40-line file still has to find it.
    ///
    /// Outlives the scroll deliberately — the same reasoning as `TreeOptions::
    /// highlight`, which exists because highlighting on the one-frame `jump_to`
    /// would flash and be gone. Cleared when the specimen changes, not on a timer:
    /// a fade would be one more thing to tune and nothing asked for it.
    pub(crate) jump_line: Option<u32>,
    /// Where the pane is scrolled, recorded so the horizontal offset is
    /// **checkable** — the one layout property that has actually bitten.
    pub(crate) scroll_offset: egui::Vec2,
    /// Which tracked identifier the view has already scrolled to, so reverse
    /// tracking fires on a *change* rather than pinning the view every frame.
    pub(crate) scrolled_for: Option<String>,
}

/// The specimen's Modelica source: syntax-highlighted, with clickable
/// identifiers, and scrolled to whatever is being followed.
///
/// Extracted from `ui` during the 2026-07-28 sweep. It is the one place
/// three separate mechanisms have to agree about a line — the lexer's
/// tokens, `IdentifierIndex`'s clickable spans, and `source_view::segments`
/// merging them — and that is much easier to keep straight in its own
/// function than buried a thousand lines into a panel closure.
///
/// **Returns the identifier that was clicked, if one was.** The follow itself is
/// `App`'s to perform — this pane reports the click and does not decide what it
/// means, the same shape `model_list` already uses.
pub(crate) fn specimen_source_ui(
    ui: &mut egui::Ui,
    source: &mut SourceViewState,
    model: &Option<String>,
    selected: &Option<PathBuf>,
    selected_is_library: bool,
    tracked_identifier: &Option<String>,
    identifier_index: &Option<ident_index::IdentifierIndex>,
    problem_lines: &[(u32, String)],
) -> Option<String> {
    // **Which file this is.** A library model's source is a whole package
    // file declaring dozens of classes, so a reader who asked for `Resistor`
    // and is looking at `Basic.mo` needs to be told. A specimen needs no such
    // line: its file is the thing selected, and the name is already on screen.
    if let Some(uri) = source.library_uri.clone() {
        let file = std::path::Path::new(&uri)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(uri.as_str())
            .to_owned();
        let model = model.clone().unwrap_or_default();
        ui.horizontal(|ui| {
            ui.weak(file.clone())
                .on_hover_text(format!("{uri}\n\nthe library file declaring {model}"));
            ui.weak("\u{00b7}");
            ui.weak(format!("declares {model} among others"));
        });
        ui.separator();
    }
    // **A read failure is reported, never rendered as blank.** Blank would be
    // indistinguishable from the refusal this replaced, and from an empty file.
    if let Some(why) = source.library_error.clone() {
        ui.label(
            egui::RichText::new(format!("cannot show this file\n\n{why}"))
                .color(crate::colors::ANIM_FAIL),
        );
        return None;
    }
    // A library model's text is seeded by the worker from the document it
    // compiled; a specimen reads from its own file so live edits show.
    //
    // *(Corrected 2026-08-04: this said `get_or_insert_with` "therefore does not
    // run, and `selected` is never read as a path". The first half was true and
    // the second did not follow from it — the closure ran whenever the worker had
    // not yet supplied the text, and then read the qualified name as a path. The
    // guard is now explicit rather than a consequence of timing.)*
    //
    // **This pane used to refuse library models outright**, claiming there was
    // "no single source file to show". That was untrue: the worker reads that
    // very file out of the session in order to compile it. Doug, 2026-08-01:
    // *"The modelica source view for an MSL model should be just as functional
    // as for an HRW specimen."*
    // **A library selection is never read from disk.** `selected` holds the
    // qualified name for one (`Modelica.Blocks.Continuous.SecondOrder`), which is
    // not a path — the worker sends the declaring file's text instead. Reading it
    // was harmless only because the worker usually wins the race; when it did
    // not, the failure became an empty string and then a false message.
    let is_library = selected_is_library;
    if source.text.is_none()
        && source.load_error.is_none()
        && !is_library
        && let Some(path) = selected.clone()
    {
        match std::fs::read_to_string(&path) {
            Ok(text) => source.text = Some(text),
            // Recorded, not defaulted. This also stops the retry, keeping the
            // filesystem out of the per-frame paint path.
            Err(e) => source.load_error = Some(format!("{}: {e}", path.display())),
        }
    }
    let source_text = source.text.as_deref();
    let mut clicked_id: Option<String> = None;
    match source_text {
        Some(text) if !text.is_empty() => {
            let scroll_out = egui::ScrollArea::both()
                .id_salt("specimen_source")
                .auto_shrink(false)
                .show(ui, |ui| {
                    let tracked = tracked_identifier.as_deref();
                    let dark = ui.visuals().dark_mode;
                    // Reverse tracking: when the tracked
                    // identifier changes — typically from a click
                    // in a downstream view — bring its
                    // declaration into view. Gated on *change*,
                    // not on the value: scrolling every frame
                    // while an identifier stays tracked would peg
                    // the view and fight the scrollbar.
                    let scroll_to = (*tracked_identifier != source.scrolled_for)
                        .then(|| {
                            tracked_identifier.as_deref().and_then(|name| {
                                identifier_index
                                    .as_ref()
                                    .and_then(|idx| idx.variables.get(name))
                                    .map(|v| v.source_line)
                            })
                        })
                        .flatten();
                    if scroll_to.is_some() || tracked_identifier.is_none() {
                        source.scrolled_for = tracked_identifier.clone();
                    }
                    // A link-driven scroll, taken once so it cannot re-scroll every
                    // frame and pin the view — the same discipline as `jump_target`.
                    let source_scroll_to = source.scroll_target.take();
                    // Tokenized once per specimen, not per frame.
                    let highlight = source
                        .highlight
                        .get_or_insert_with(|| crate::source_view::SourceHighlight::new(text));
                    for (i, line) in text.lines().enumerate() {
                        let line_1 = (i + 1) as u32;
                        // Why this line was blamed, if it was. `problem_lines` is only
                        // non-empty for a model index reduction could not rescue, so a
                        // high-index model like MotorWithBrake is never marked.
                        let blamed = problem_lines
                            .iter()
                            .find(|(l, _)| *l == line_1)
                            .map(|(_, why)| why.as_str());
                        let line_tokens = highlight.line(i);
                        let spans = identifier_index
                            .as_ref()
                            .map(|idx| idx.clickable_spans(line_1, line, line_tokens))
                            .unwrap_or_default();
                        // One pass produces both colour and click
                        // targets, so the two cannot disagree about
                        // where a run of text begins and ends.
                        let segments = crate::source_view::segments(line, line_tokens, &spans);
                        // **Reserve a paint slot behind the row**, then fill it
                        // after layout if this is the line a jump landed on. The
                        // same trick `tree.rs` uses for its jump wash and hover
                        // highlight: painted behind the text rather than over it,
                        // because a wash on top would dim the syntax colouring
                        // that makes the line readable in the first place.
                        let bg_slot = ui.painter().add(egui::Shape::Noop);
                        let row = ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 0.0;
                            // A blamed line's number is coloured rather than
                            // gutter-marked: a marker glyph would widen this column and
                            // shift every line, and a layout regression is precisely the
                            // class of defect Claude cannot see.
                            let mut num =
                                egui::RichText::new(format!("{:>4} ", line_1)).monospace();
                            num = if blamed.is_some() {
                                num.color(crate::colors::ANIM_FAIL).strong()
                            } else {
                                num.weak()
                            };
                            ui.label(num);
                            for seg in &segments {
                                match seg.link {
                                    Some(name) => {
                                        // Clickable identifiers keep their own
                                        // colours, which outrank syntax colour —
                                        // interactivity has to win visually.
                                        let color = if tracked == Some(name) {
                                            crate::colors::TRACKED_GOLD
                                        } else {
                                            crate::colors::CLICKABLE_IDENT
                                        };
                                        let label = egui::Label::new(
                                            egui::RichText::new(seg.text)
                                                .monospace()
                                                .color(color)
                                                .underline(),
                                        )
                                        .sense(egui::Sense::click());
                                        // Say which verb the click carries,
                                        // before it fires. This used to hover
                                        // with the bare name, confirming what
                                        // was under the cursor and nothing
                                        // about what clicking would do.
                                        let hover =
                                            crate::follow_hover(name, tracked == Some(name));
                                        if ui.add(label).on_hover_text(hover).clicked() {
                                            clicked_id = Some(name.to_owned());
                                        }
                                    }
                                    None => {
                                        let mut rt = egui::RichText::new(seg.text).monospace();
                                        if let Some(c) = crate::colors::syntax_color(seg.kind, dark)
                                        {
                                            rt = rt.color(c);
                                        }
                                        ui.label(rt);
                                    }
                                }
                            }
                        });
                        // **The line a jump landed on**, washed so it is findable.
                        // Filled into the slot reserved before layout, so it sits
                        // behind the syntax colouring rather than dimming it.
                        if source.jump_line == Some(line_1) {
                            ui.painter().set(
                                bg_slot,
                                egui::Shape::rect_filled(
                                    row.response.rect,
                                    egui::CornerRadius::ZERO,
                                    crate::colors::JUMP_FILL,
                                ),
                            );
                        }
                        if let Some(why) = blamed {
                            // Painted *over* the row at low alpha rather than behind it.
                            // A `Frame` fill would be cleaner-looking but adds margins,
                            // and any layout shift here is a rendered defect Claude has
                            // no way to notice. An overpaint cannot move anything.
                            ui.painter().rect_filled(
                                row.response.rect,
                                egui::CornerRadius::ZERO,
                                crate::colors::ANIM_FAIL.gamma_multiply(0.18),
                            );
                            row.response.clone().on_hover_text(why);
                        }
                        if scroll_to == Some(line_1) || source_scroll_to == Some(line_1) {
                            // **Scroll to the line's START, not to the line.**
                            //
                            // This is a `ScrollArea::both`, and `scroll_to_me`
                            // aligns on *both* axes. Centring a row horizontally
                            // centres a line that may be 200 characters wide, so
                            // its opening characters — the indentation, the
                            // keyword, the declared name — end up off the left
                            // edge. Doug, 2026-08-01: *"The text in the modelica
                            // source view is positioned too far to the left. The
                            // left-most characters in many source lines are being
                            // cut off."*
                            //
                            // Latent before MSL models: a specimen's lines are
                            // short and shallowly indented, so centring one moved
                            // the view barely at all. A library file is nested
                            // several packages deep with long signatures, and the
                            // scroll now fires on every library load to reach the
                            // declaration line.
                            //
                            // Collapsing the target to a sliver at the row's left
                            // edge fixes both axes at once: vertically it still
                            // centres the line, and horizontally the offset that
                            // would centre a sliver at the content's left margin
                            // is negative, so egui clamps it to 0 — the start of
                            // the line, which is where reading begins.
                            let mut target = row.response.rect;
                            target.max.x = target.min.x + 1.0;
                            ui.scroll_to_rect(target, Some(egui::Align::Center));
                        }
                    }
                });
            // **Observation hook, not decoration.** `ScrollArea` keeps its
            // offset in `Memory` under an id derived from the parent `Ui`,
            // which a test cannot reconstruct; the returned state is the only
            // honest way to read it. One `Vec2` per frame buys a headless
            // guard on the one layout property that has bitten -- the
            // horizontal offset drifting off the left margin.
            source.scroll_offset = scroll_out.state.offset;
        }
        // **Four different reasons there is no source, and they used to share
        // one sentence.** Saying "select a specimen" to someone who has selected
        // one is not a smaller error than showing wrong text — it sends them to
        // fix something that is not broken.
        _ => {
            if let Some(err) = &source.load_error {
                ui.colored_label(
                    ui.visuals().error_fg_color,
                    format!("The source file could not be read \u{2014} {err}"),
                );
            } else if selected.is_none() {
                ui.weak("Select a specimen to view its source.");
            } else if is_library {
                ui.weak(
                    "The declaring file has not arrived from the compiler yet \u{2014} \
                     a library model's source comes from the session, not from disk.",
                );
            } else {
                ui.weak("This file is empty.");
            }
        }
    }
    // Handed back rather than acted on. `App::set_tracked_identifier` is shared with
    // every other entry point, so clicking a name here toggles exactly as following
    // from a tree or the equation sheet does — and keeping the call on `App` is what
    // lets this pane take seven refs instead of `&mut self`.
    clicked_id
}
