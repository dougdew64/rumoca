//! **The navigation view** — the pane that replaces the whole stage view once the reader
//! follows "Go to definition" into a library class.
//!
//! Lifted out of `central_panel_ui` on 2026-08-20, the second cut into that router. See
//! [`docs/app-split-plan.md`](../docs/app-split-plan.md).
//!
//! # Why this is a seam at all
//!
//! `central_panel_ui`'s outermost shape is `if self.nav.is_empty() { … } else { … }`, and
//! the two halves have **nothing in common but the tree widget**. The `if` is the
//! specimen's compiled stages: tab row, context bar, sub-view dispatch, thirteen panes.
//! The `else` is one breadcrumb, two buttons and one tree over an IR that came from
//! somewhere else entirely. Nothing crosses between them — no shared local, no shared
//! guard — so the `else` is the cheapest whole branch the router has.
//!
//! It is also the branch that **calls no `App` method**, which is `tour_prose_ui`'s rule
//! for a body (*"which contiguous region calls no `App` method?"*) coming out true for
//! once on a router. The two buttons want to mutate `self.nav`, which is why they report
//! a [`NavCommand`] instead: the same deferred-intent pattern the rest of the frame uses.
//!
//! # A navigated class is a DIFFERENT IR, and that governs everything here
//!
//! The tree on screen is a library class — `Modelica.Electrical.Analog.Basic.Resistor`,
//! say — pulled out of the resolved tree by the worker. It is **not** a stage of the
//! specimen's compilation. Every address computed against a stage therefore means
//! nothing here, which is why [`nav_view_ui`] blanks `jump_to` and `highlight` no matter
//! what it is handed: an `hrw://` node link addressed into the DAE would land on an
//! unrelated node of the Resistor, or on nothing, and either way would look like it
//! worked.
//!
//! **The blanking lives here rather than at the call site on purpose.** It used to be two
//! `None`s written into a `TreeOptions` literal in `app.rs`, correct by the author's
//! attention; now it is a property of the pane, and a caller that passes a stage's live
//! jump target gets the same suppression.
//!
//! **The other five `TreeOptions` fields are NOT blanked yet, and that is a KNOWN DEFECT
//! with a decision already made** — Doug, 2026-08-20: *the navigated tree is annotated from
//! the class or not at all.* `tracked`, `known_variables`, `declaring_classes`,
//! `variable_lines` and `path_lines` all describe **the specimen**, so a library class here
//! can be underlined, offered for Follow, and shown a *"declared at line N"* naming a line of
//! the specimen's file. They join the two above; the edit, the test's exact name and the
//! must-fire recipe are queued in [`docs/app-split-plan.md`](../docs/app-split-plan.md).
//!
//! **Why nothing caught it:** `docs/identity-and-provenance.md` forbids *substring* search
//! deciding identity, and all five use exact equality, so they comply as written. What they
//! step outside is that rule's unstated precondition — **that both sides are the same
//! model**, true of everything until "Go to definition" existed. Exact equality across two
//! namespaces is a collision wearing identity's clothes.

use std::collections::HashMap;

use eframe::egui;

use crate::tree;
use crate::ui_state::NavEntry;

/// What the reader asked the navigation stack to do this frame.
///
/// An enum rather than the two `bool`s it replaced, because the two are **mutually
/// exclusive by nature** and were only exclusive by an `else if` in the caller. Home
/// still wins if both fire in one frame — see [`nav_view_ui`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NavCommand {
    /// "Specimen" — leave navigation entirely and return to the stage view.
    Home,
    /// "← Back" — pop one level.
    Back,
}

/// The three strings above the navigated tree: where the reader came from, what is still
/// arriving, and what failed.
///
/// Bundled for the [`tree::TreeOptions`] reason — three more positional arguments on a
/// signature that already carries the tree's own two bundles.
pub(crate) struct NavChrome<'a> {
    /// The specimen the navigation started from; the root of the breadcrumb.
    pub(crate) model: Option<&'a str>,
    /// A class whose fetch is in flight, named so the spinner says *what* is opening.
    pub(crate) loading: Option<&'a str>,
    /// Why the last "Go to definition" produced nothing.
    ///
    /// **Rendered here as well as in the stage view**, deliberately: a failed navigation
    /// can leave the reader on either side of the `nav.is_empty()` split, and a message
    /// shown on only one of them would vanish depending on how deep they already were.
    pub(crate) error: Option<&'a str>,
}

/// Draw the navigated class, its breadcrumb and its two navigation buttons.
///
/// Returns what the reader asked for, if anything; the caller owns `nav` and is the only
/// thing that may push or pop it.
///
/// `nav` is the whole stack rather than its top, because the breadcrumb names every level
/// and the tree renders the last. An empty stack draws nothing and reports nothing —
/// unreachable from `central_panel_ui`, which tests `nav.is_empty()` to choose this pane
/// at all, and an early return rather than an `unwrap` because a pane that panics on
/// empty state is one more thing a test must avoid rather than assert.
pub(crate) fn nav_view_ui(
    ui: &mut egui::Ui,
    nav: &[NavEntry],
    chrome: NavChrome<'_>,
    field_help: &HashMap<String, String>,
    opts: tree::TreeOptions<'_>,
    actions: &mut tree::TreeActions,
) -> Option<NavCommand> {
    let entry = nav.last()?;

    let mut home = false;
    let mut back = false;
    ui.horizontal(|ui| {
        if ui
            .button("Specimen")
            .on_hover_text("Return to the specimen stages (top of navigation)")
            .clicked()
        {
            home = true;
        }
        if ui.button("\u{2190} Back").clicked() {
            back = true;
        }
        ui.separator();
        let mut crumb = chrome.model.unwrap_or("model").to_owned();
        for e in nav {
            crumb.push_str("  \u{25b8}  ");
            crumb.push_str(&e.name);
        }
        ui.label(egui::RichText::new(crumb).monospace().strong());
        if let Some(n) = chrome.loading {
            ui.weak(format!("opening {n}\u{2026}"));
            ui.spinner();
        }
    });
    if let Some(err) = chrome.error {
        ui.colored_label(ui.visuals().error_fg_color, err);
    }
    ui.separator();

    egui::ScrollArea::both()
        .id_salt("nav_tree")
        .auto_shrink(false)
        .show(ui, |ui| {
            tree::tree_ui(
                ui,
                &entry.name,
                &entry.value,
                None,
                actions,
                &entry.def_index,
                field_help,
                tree::TreeOptions {
                    // **A navigated library class is a different IR**, so a jump target
                    // addressed into the stage tree would land on an unrelated node or on
                    // nothing at all. The highlight is suppressed for the same reason, and
                    // travels with it: highlighting the row a stage link named would mark
                    // an arbitrary row of this class.
                    jump_to: None,
                    highlight: None,
                    ..opts
                },
            );
        });

    // **Home wins**, which is what the caller's `if go_home { … } else if go_back { … }`
    // did with the two flags this enum replaced. Unreachable with a mouse — one click
    // per frame — and stated so that collapsing two flags into one value cannot quietly
    // reverse a precedence.
    if home {
        Some(NavCommand::Home)
    } else if back {
        Some(NavCommand::Back)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::Seg;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;
    use serde_json::json;

    /// Everything the pane is handed, plus what it handed back.
    struct Nav {
        nav: Vec<NavEntry>,
        model: Option<String>,
        loading: Option<String>,
        error: Option<String>,
        field_help: HashMap<String, String>,
        /// Set by the jump-suppression test; every other test leaves it `None`.
        jump_to: Option<Vec<Seg>>,
        actions: tree::TreeActions,
        command: Option<NavCommand>,
    }

    impl Nav {
        /// A stack of one class whose IR nests a leaf one level down.
        ///
        /// **The nesting is the fixture's whole point.** `tree_ui` opens the root by
        /// default and nothing else, so `inner_leaf` is on screen exactly when something
        /// forced `outer` open — which is what `jump_to` does and what this pane must not
        /// let it do.
        fn one(name: &str) -> Self {
            Nav {
                nav: vec![NavEntry {
                    name: name.to_owned(),
                    value: json!({ "outer": { "inner_leaf": 42 } }),
                    def_index: std::collections::BTreeMap::new(),
                }],
                model: Some("Specimen".to_owned()),
                loading: None,
                error: None,
                field_help: HashMap::new(),
                jump_to: None,
                actions: tree::TreeActions::default(),
                command: None,
            }
        }

        /// Push another level, so the breadcrumb has something to join.
        fn then(mut self, name: &str) -> Self {
            self.nav.push(NavEntry {
                name: name.to_owned(),
                value: json!({ "outer": { "inner_leaf": 42 } }),
                def_index: std::collections::BTreeMap::new(),
            });
            self
        }
    }

    /// Draw the pane once and hand back the harness.
    ///
    /// **Sized generously** (`1000×800`): a clipped widget stays in the
    /// accessibility tree while behaving as though it is not there, which is the trap
    /// `stage_tabs` and `equation_sheet_view` both recorded.
    fn draw(state: Nav) -> Harness<'static, Nav> {
        let mut h = Harness::builder()
            .with_size(egui::Vec2::new(1000.0, 800.0))
            .build_ui_state(
                |ui, s: &mut Nav| {
                    let cmd = nav_view_ui(
                        ui,
                        &s.nav,
                        NavChrome {
                            model: s.model.as_deref(),
                            loading: s.loading.as_deref(),
                            error: s.error.as_deref(),
                        },
                        &s.field_help,
                        tree::TreeOptions {
                            jump_to: s.jump_to.as_deref(),
                            ..Default::default()
                        },
                        &mut s.actions,
                    );
                    if cmd.is_some() {
                        s.command = cmd;
                    }
                },
                state,
            );
        h.run_steps(2);
        h
    }

    /// **The breadcrumb is the only thing on screen that says where you are**, and it has
    /// to name the specimen *and* every level — a trail showing just the class you landed
    /// on cannot be walked back mentally, which is the whole reason "Back" is not enough.
    #[test]
    fn the_crumb_names_the_specimen_then_every_level() {
        let h = draw(Nav::one("Resistor").then("Pin"));

        assert!(
            h.query_by_label_contains("Specimen  \u{25b8}  Resistor  \u{25b8}  Pin")
                .is_some(),
            "the trail must read specimen-first, in the order the stack was pushed"
        );
    }

    /// **Nothing loaded is still somewhere**, so the crumb keeps its root rather than
    /// starting at the class. A trail beginning mid-way reads as though the class were
    /// the specimen.
    #[test]
    fn a_nameless_specimen_still_roots_the_crumb() {
        let mut state = Nav::one("Resistor");
        state.model = None;
        let h = draw(state);

        assert!(
            h.query_by_label_contains("model  \u{25b8}  Resistor")
                .is_some(),
            "with no specimen name the crumb must still have a root"
        );
    }

    /// **The two buttons are the only way out of this pane**, and they report rather than
    /// act: the caller owns the stack.
    #[test]
    fn specimen_reports_home() {
        let mut h = draw(Nav::one("Resistor"));

        h.get_by_label("Specimen").click();
        h.run_steps(2);

        assert_eq!(
            h.state().command,
            Some(NavCommand::Home),
            "\"Specimen\" must ask the caller to clear the whole stack"
        );
    }

    /// The other half of the pair. Separate tests, because a single test asserting both
    /// would pass while one button was wired to the other.
    #[test]
    fn back_reports_back() {
        let mut h = draw(Nav::one("Resistor"));

        h.get_by_label("\u{2190} Back").click();
        h.run_steps(2);

        assert_eq!(
            h.state().command,
            Some(NavCommand::Back),
            "\"Back\" must ask the caller to pop one level"
        );
    }

    /// **An idle frame asks for nothing.** Reporting a command every frame would clear
    /// the stack continuously — the failure mode a returned value makes cheap to check
    /// and two `bool`s in a caller's locals did not.
    #[test]
    fn an_untouched_pane_asks_for_nothing() {
        let h = draw(Nav::one("Resistor"));

        assert_eq!(h.state().command, None, "no click, no command");
    }

    /// **A fetch in flight says which class**, because the pane still shows the previous
    /// level while the next one arrives — an unnamed spinner beside a stale tree reads as
    /// though *this* tree were loading.
    #[test]
    fn an_opening_class_is_named() {
        let mut state = Nav::one("Resistor");
        state.loading = Some("Pin".to_owned());
        let h = draw(state);

        assert!(
            h.query_by_label_contains("opening Pin").is_some(),
            "the spinner must name the class it is waiting for"
        );
    }

    /// **A failed navigation is reported here too.** The message is set wherever the
    /// reader happened to be, and this pane is half of where they can be.
    #[test]
    fn a_failed_navigation_is_reported() {
        let mut state = Nav::one("Resistor");
        state.error = Some("no definition for Pin".to_owned());
        let h = draw(state);

        assert!(
            h.query_by_label_contains("no definition for Pin").is_some(),
            "the navigation error must reach this pane, not only the stage view"
        );
    }

    /// **The defect this pane exists to prevent: a stage's address honoured against
    /// another IR.**
    ///
    /// `jump_to` force-opens every ancestor of its target and scrolls to it. Handed a
    /// path from the specimen's DAE, it would open whatever node of the *library class*
    /// happens to sit at that address and scroll the reader to it — a correct-looking
    /// reveal of an unrelated node, with nothing on screen admitting the address came
    /// from somewhere else. Same shape as the stranded sub-view of 2026-08-20: presence
    /// substituted, not absence filled.
    ///
    /// Must-fire verified by deleting `jump_to: None` from the `TreeOptions` literal:
    /// `inner_leaf` then reaches the screen and this fails.
    #[test]
    fn a_jump_target_is_not_honoured_against_a_navigated_class() {
        let mut state = Nav::one("Resistor");
        state.jump_to = Some(vec![Seg::Key("outer".to_owned())]);
        let h = draw(state);

        assert!(
            h.query_by_label_contains("outer").is_some(),
            "precondition: the root opened by itself, so `outer` is on screen to be opened"
        );
        assert!(
            h.query_by_label_contains("inner_leaf").is_none(),
            "a jump target addressed into the stage tree must not open this class's nodes"
        );
    }
}
