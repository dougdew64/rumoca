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
//! It is also the branch that **calls no `App` method**, which is `lab_prose_ui`'s rule
//! for a body (*"which contiguous region calls no `App` method?"*) coming out true for
//! once on a router. The two buttons want to mutate `self.nav`, which is why they report
//! a [`NavCommand`] instead: the same deferred-intent pattern the rest of the frame uses.
//!
//! # A navigated class is a DIFFERENT IR, and that is why this pane takes NO annotations
//!
//! The tree on screen is a library class — `Modelica.Electrical.Analog.Basic.Resistor`,
//! say — pulled out of the resolved tree by the worker. It is **not** a stage of the
//! specimen's compilation, and **every field of [`tree::TreeOptions`] describes the
//! specimen**: two of them address a node of a stage's tree, and five carry what the
//! specimen's compile learned about its own variables. So [`nav_view_ui`] takes no
//! `TreeOptions` at all and hands `tree_ui` a `TreeOptions::default()`.
//!
//! **Doug ruled on it, 2026-08-20:** *the navigated tree is annotated from the class, or
//! not at all.* What each of the five would otherwise claim about a library class:
//!
//! | field | what it is | on a navigated class |
//! |---|---|---|
//! | `path_lines` | *stage* node path → source line | a colliding path resolves to the **specimen's** DAE line |
//! | `variable_lines` | variable name → declaring line **in the specimen** | `R` in `Resistor` gets the specimen's `R` |
//! | `declaring_classes` | variable name → declaring class, **of the specimen** | the same collision, feeding "Go to definition" |
//! | `known_variables` | the **specimen's** variables | decides what is *trackable*; a name in both is offered |
//! | `tracked` | the identifier being followed | a flat name (`resistor.R`), so a hit here is a bare-name collision |
//!
//! The failure mode is **presence substituted, not absence filled**: the tooltip would say
//! *"declared at line 41"* over a row of the Resistor, naming a line of your specimen's
//! file, with nothing on screen admitting where the number came from.
//!
//! **The five turned out to be the WHOLE STRUCT** — `jump_to` and `highlight` were already
//! blanked here for the same reason (a stage's address means nothing against another IR),
//! and five plus two is every field `TreeOptions` has. So the parameter went instead of
//! gaining five `None`s: **the caller can no longer hand this pane the specimen's
//! annotations at all**, which the compiler enforces and no test has to.
//!
//! **Why nothing caught it:** `docs/identity-and-provenance.md` forbids *substring* search
//! deciding identity, and all five use exact equality, so they comply as written. What they
//! step outside is that rule's unstated precondition — **that both sides are the same
//! model**, true of everything until "Go to definition" existed. Exact equality across two
//! namespaces is a collision wearing identity's clothes.
//!
//! **What the fix does NOT remove is the confirming detail.** `def_index` is per-[`NavEntry`]
//! — the class's *own* DefId table, resolved structurally by the worker — so "Go to
//! definition" keeps working *through DefIds* while the name-matched shortcuts go. The
//! structural route that document prescribes survives untouched.
//!
//! **Blanking is the correct answer NOW, not the destination.** Nothing indexes an MSL
//! class's own variables, declaring positions or source lines, so the class-derived versions
//! do not exist to substitute. If this tree should ever be annotated, the annotations must be
//! built **from the class** — never re-derived from the specimen, which is exactly what the
//! missing parameter now prevents.

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
///
/// **There is no [`tree::TreeOptions`] parameter, and its absence is load-bearing** — see
/// the module docs. Every field of that struct describes the specimen, and this tree is a
/// different IR; the caller cannot pass one, so it cannot pass the wrong one.
///
/// `field_help` is the exception that proves the rule: it maps a *field name* to Rumoca's
/// own documentation for that field, so it is about the IR's schema rather than about any
/// one model, and it is as true of a library class as of a specimen.
pub(crate) fn nav_view_ui(
    ui: &mut egui::Ui,
    nav: &[NavEntry],
    chrome: NavChrome<'_>,
    field_help: &HashMap<String, String>,
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
                // **Every field of `TreeOptions` describes the specimen, and this is a
                // different IR** — so the default, in full, rather than a literal that
                // blanks seven fields one at a time. A jump target addressed into the
                // stage tree would open an unrelated node of this class; a name matched
                // against the specimen's variables would underline it, offer it for
                // Follow, and cite a line of the specimen's file. Absence stated, not
                // filled. See the module docs for the ruling and the table.
                tree::TreeOptions::default(),
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
        actions: tree::TreeActions,
        command: Option<NavCommand>,
    }

    /// The one IR every fixture renders: a class with a leaf that **looks exactly like a
    /// specimen variable**, and a nested one.
    ///
    /// `name: "R"` is the shape the annotation defect needs — `trackable_name` accepts a
    /// *string* leaf under a non-prose key, and `R` is a name a real specimen very
    /// plausibly also declares. A number could never be trackable, so a numeric fixture
    /// would make the annotation tests pass for the wrong reason.
    ///
    /// **The nesting is the second half of the point.** `tree_ui` opens the root and
    /// nothing else, so `inner_leaf` reaches the screen only if something forced `outer`
    /// open — which is what a `jump_to` does, and what this pane can no longer be handed.
    fn class_ir() -> serde_json::Value {
        json!({ "name": "R", "outer": { "inner_leaf": 42 } })
    }

    impl Nav {
        /// A stack of one class.
        fn one(name: &str) -> Self {
            Nav {
                nav: vec![NavEntry {
                    name: name.to_owned(),
                    value: class_ir(),
                    def_index: std::collections::BTreeMap::new(),
                }],
                model: Some("Specimen".to_owned()),
                loading: None,
                error: None,
                field_help: HashMap::new(),
                actions: tree::TreeActions::default(),
                command: None,
            }
        }

        /// Push another level, so the breadcrumb has something to join.
        fn then(mut self, name: &str) -> Self {
            self.nav.push(NavEntry {
                name: name.to_owned(),
                value: class_ir(),
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

    /// **The defect this pane exists to prevent: a library class annotated from the
    /// specimen.** Doug's ruling, 2026-08-20 — *annotated from the class, or not at all.*
    ///
    /// Four of the five annotations reach the reader through the **row menu**, which is
    /// why this test right-clicks: `known_variables` decides whether *"🔎 Follow R"* is
    /// offered, `variable_lines` adds *"📄 Show R in the Modelica source"*, and
    /// `declaring_classes` adds *"↪ Go to …"*. All three are gated on `trackable_name`,
    /// so a fixture whose leaf is *not* a plausible variable would pass vacuously — see
    /// [`class_ir`] for why the leaf is `name: "R"`.
    ///
    /// **What the menu keeps is as much the assertion as what it loses.** *"Point at"*,
    /// *"Show this being set"* and *"Copy text"* are properties of a JSON node, true of
    /// any IR; the three that go are claims about a model this tree is not. Asserting the
    /// survivors is what makes the negatives mean *"suppressed"* rather than *"the menu
    /// never opened"*.
    ///
    /// **Must-fire recipe — the perturbation is the defect itself.** Replace the
    /// `TreeOptions::default()` in [`nav_view_ui`] with a literal setting
    /// `known_variables` to a set containing `"R"`; the Follow item appears and this
    /// fails on that assertion, while the seven tests around it stay green. Verified
    /// 2026-08-20.
    ///
    /// **THE THREE ITEMS ARE NOT INDEPENDENT, and the perturbation is what showed it.**
    /// All three are gated on `trackable_name`, which returns `None` without
    /// `known_variables` — so restoring `variable_lines` or `declaring_classes` *alone*
    /// changes nothing on screen, and their assertions can only fire in company. That
    /// makes `known_variables` the one field whose blanking suppresses the other two, and
    /// the two later assertions guard the combined regression rather than one apiece.
    ///
    /// **What it cannot catch: `tracked`.** A tracked identifier is a painted fill behind
    /// the row, not a widget, so it leaves no accessibility node to query. It is blanked
    /// with the rest and rests on the signature alone.
    #[test]
    fn a_navigated_class_is_not_annotated_from_the_specimen() {
        let mut h = draw(Nav::one("Resistor"));

        // The tree renders a string value with `{:?}`, so the row reads `name: "R"`.
        h.get_by_label_contains(r#"name: "R""#).click_secondary();
        h.run_steps(2);

        assert!(
            h.query_by_label_contains("Point at").is_some(),
            "precondition: the right-click must actually have opened the row menu"
        );
        assert!(
            h.query_by_label_contains("Follow").is_none(),
            "`known_variables` is the SPECIMEN's variables — a library class's `R` that \
             collides with one must not be offered for Follow"
        );
        assert!(
            h.query_by_label_contains("Modelica source").is_none(),
            "`variable_lines` maps the specimen's names to the specimen's lines — citing \
             one over a row of this class would name a line of the wrong file"
        );
        assert!(
            h.query_by_label_contains("Go to").is_none(),
            "`declaring_classes` is the specimen's name-to-class map; go-to-definition \
             from here must go through this entry's own `def_index` or not at all"
        );
    }

    /// The fifth annotation, `path_lines`, and it takes a **different route through the
    /// tree**: it is keyed by node *path* rather than by name, so it is offered on a
    /// collapsible header rather than on a leaf, and by the arm of `node_ui` the test
    /// above never reaches.
    ///
    /// **The collision is the cheapest of the five to hit.** `path_lines` maps a *stage*
    /// node path — `outer`, say — to a line of the specimen's DAE, and a library class has
    /// nodes at ordinary paths too. The reader would be offered a jump into the specimen's
    /// source from a row of the Resistor.
    ///
    /// Must-fire: set `path_lines` to a map containing `outer` in [`nav_view_ui`]'s
    /// options; the source item appears on this header and this fails.
    #[test]
    fn a_navigated_node_is_not_given_the_specimens_source_line() {
        let mut h = draw(Nav::one("Resistor"));

        h.get_by_label_contains("outer").click_secondary();
        h.run_steps(2);

        assert!(
            h.query_by_label_contains("Point at").is_some(),
            "precondition: the right-click must actually have opened the header's menu"
        );
        assert!(
            h.query_by_label_contains("Modelica source").is_none(),
            "a node path resolved against the specimen's DAE must not offer this class's \
             rows a jump into the specimen's source"
        );
    }
}
