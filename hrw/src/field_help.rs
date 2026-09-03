//! Generic (build-time) field help — delivered as hover tooltips on tree nodes.
//!
//! ## Two-tier help architecture
//!
//! HRW provides two kinds of help for understanding IR fields:
//!
//! 1. **Fast tier (this module):** instant, offline, generic. The `///` doc
//!    comments that Rumoca's authors wrote on `rumoca-ir-ast` IR struct fields
//!    are extracted at build time and embedded as `field_help.json`. When you
//!    hover a field in the tree inspector, its doc comment appears as a tooltip
//!    — no network call, no latency. The help is *generic* (it describes what
//!    `def_id` means in general, not why *this particular* `def_id` has *this
//!    particular* value).
//!
//! 2. **Specific tier (the Claude bridge):** contextual, on-demand, slow. When
//!    the user captures a node and asks in the chat, Claude reasons about *why*
//!    this specific value appeared, using the specimen source, the staged IR,
//!    and the Rumoca phase code as context. See `bridge.rs`.
//!
//! ## How field_help.json is generated
//!
//! The `gen_field_help` example (`examples/gen_field_help.rs`) parses the
//! Rumoca IR AST source files, extracts `///` doc comments from struct fields,
//! and writes `src/field_help.json`. This file is checked into the repo and
//! baked into the binary via `include_str!`. Regenerate after a Rumoca update
//! with `cargo run -p hrw --example gen_field_help`.

use std::collections::HashMap;

use crate::worker::StageKind;

/// IR crates whose `///` field docs are harvested into `field_help.json`.
///
/// **Lives here rather than in `gen_field_help` so a test can reach it.** The list is a
/// fact about which stages have in-app help; the example is only the tool that acts on it.
///
/// It fell a stage behind for weeks: DAE got a tab, `rumoca-ir-dae` was not added, and the
/// pane rendered with no field help while Rumoca already carried 290 lines of `///` docs
/// for it. Nothing failed, because a missing tooltip is invisible.
/// [`tests::every_ir_dependency_is_harvested_for_field_help`] derives the requirement
/// from `Cargo.toml` so the next `rumoca-ir-*` dependency cannot be forgotten.
///
/// # Harvesting does NOT require depending, and assuming it did nearly cost a re-parse
///
/// `rumoca-ir-solve` is here while HRW has **no dependency on it** — Solve lowering's IR
/// reaches the pane as serialised JSON through `rumoca-compile`. `gen_field_help` locates
/// sources through `cargo metadata`, which lists every **workspace member** whether or not
/// anything depends on it, so the docs were reachable all along.
///
/// This was first reported to Doug as costing a dependency, a moved `Cargo.lock` and one
/// full MSL re-parse. He accepted that price; it did not exist. **The cheapest check —
/// does `cargo metadata` already list it — was not run before quoting a cost**, which is
/// the same shape as this repository's four dead levers: a plausible figure derived by
/// reasoning rather than measured.
///
/// # `rumoca-phase-structural` is here although it is not an IR crate
///
/// `structural_to_json` renders `rumoca_phase_structural::StructuralReport`, so the
/// Structural and Index-reduction panes show a **phase** crate's type. It carries 87
/// documented fields and they are precisely those panes' vocabulary — `n_equations`,
/// `n_unknowns`, `unmatched_unknowns`, `matching_size`, `algebraic_loops` — the nodes the
/// labs point at. Keying help by stage without harvesting it would have *removed* those
/// tooltips, which is how a mapping assumed from the stage list rather than measured
/// turns an accuracy fix into a regression.
pub const IR_CRATES: &[&str] = &[
    "rumoca-ir-ast",
    "rumoca-ir-flat",
    "rumoca-ir-dae",
    "rumoca-ir-solve",
    "rumoca-phase-structural",
];

// The field help table is embedded at compile time via `include_str!`.
// This means the binary is self-contained — no runtime file I/O needed.
// The JSON maps field names (strings) to their doc-comment text (strings).
const FIELD_HELP_JSON: &str = include_str!("field_help.json");

/// The embedded table, resolved per stage.
///
/// # The defect this replaced
///
/// `load()` returned one flat `field name -> doc` map, and the comment that stood here
/// named the gap without closing it: *"a future refinement could disambiguate by owning
/// type for fields whose name appears in multiple IR structs."* Until then, a name
/// documented in **any** harvested crate supplied the tooltip for that name in **every**
/// pane. Hovering `names` under Solve lowering's `solver_maps` showed *"All names imported
/// from the package"* — `rumoca-ir-ast`'s doc for an **import clause**. 66 field names are
/// documented in two or more harvested crates.
///
/// **A wrong tooltip is worse than a missing one**: it teaches something false, and the
/// reader has no way to tell which tooltips to trust.
///
/// # Two mechanisms, because the mapping cannot be complete
///
/// 1. **Prefer the stage's own crate** — [`StageKind::ir_crate`], every arm of which is
///    evidence from the `*_to_json` that builds that stage's value.
/// 2. **Label every tooltip with the crate it came from**, always, even when it matches.
///    That is what makes a residual mismatch *visible* rather than silent, and it is the
///    only protection available where `ir_crate` is `None` — `Initialization`, whose
///    source type was not established. It is also useful in itself while Doug is
///    learning the IR: the tooltip says which crate owns the node.
///
/// Mechanism 2 is why this is not a regression for panes whose crate is unknown: they keep
/// their tooltips, now marked with their true origin.
pub struct FieldHelp {
    /// `field -> crate -> doc`, exactly as `gen_field_help` writes it.
    by_field: HashMap<String, HashMap<String, String>>,
    /// `stage slug -> field -> tooltip`, precomputed at load.
    per_stage: HashMap<&'static str, HashMap<String, String>>,
    /// Handed out for a stage with no table, so callers need no `Option`.
    empty: HashMap<String, String>,
}

impl FieldHelp {
    /// Parse the embedded table and resolve it for every stage.
    ///
    /// An empty table if parsing fails — defensive, since our own tooling writes it.
    pub fn load() -> Self {
        let by_field: HashMap<String, HashMap<String, String>> =
            serde_json::from_str(FIELD_HELP_JSON).unwrap_or_default();

        let mut per_stage = HashMap::new();
        for stage in StageKind::ALL {
            let want = stage.ir_crate();
            let mut table: HashMap<String, String> = HashMap::new();
            for (field, by_crate) in &by_field {
                // The stage's own crate when it documents this field; otherwise the
                // longest doc available, which is only ever a fallback and is labelled
                // as belonging to another crate.
                let chosen = want
                    .and_then(|w| by_crate.get_key_value(w))
                    .or_else(|| by_crate.iter().max_by_key(|(_, d)| d.len()));
                if let Some((from, doc)) = chosen {
                    table.insert(field.clone(), format!("{doc}\n\n\u{2014} {from}"));
                }
            }
            per_stage.insert(stage.slug(), table);
        }

        Self {
            by_field,
            per_stage,
            empty: HashMap::new(),
        }
    }

    /// The `field -> tooltip` table for one stage. Every tooltip names its crate.
    pub fn for_stage(&self, stage: StageKind) -> &HashMap<String, String> {
        self.per_stage.get(stage.slug()).unwrap_or(&self.empty)
    }

    /// Every crate documenting `field`, for tests and for reasoning about collisions.
    pub fn crates_documenting(&self, field: &str) -> Vec<&str> {
        let mut v: Vec<&str> = self
            .by_field
            .get(field)
            .map(|m| m.keys().map(String::as_str).collect())
            .unwrap_or_default();
        v.sort_unstable();
        v
    }

    /// How many field names the table carries.
    pub fn len(&self) -> usize {
        self.by_field.len()
    }

    /// Whether the table is empty.
    pub fn is_empty(&self) -> bool {
        self.by_field.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The embedded field_help.json parses and contains entries.
    #[test]
    fn field_help_loads_non_empty() {
        let help = FieldHelp::load();
        assert!(!help.is_empty(), "field_help.json should contain entries");
    }

    /// Key IR field names that the tree inspector relies on are present.
    #[test]
    fn field_help_contains_core_ir_fields() {
        let help = FieldHelp::load();
        let parse = help.for_stage(StageKind::Parse);
        for key in ["def_id", "classes", "components", "equations", "name"] {
            assert!(
                parse.contains_key(key),
                "field_help missing expected key: {key}"
            );
        }
    }

    /// A stage gets **its own crate's** doc for a name several crates document.
    ///
    /// `causality` is documented three ways: `rumoca-ir-ast` explains type-alias
    /// causality at length, `rumoca-ir-flat` says "input, output, or empty", and
    /// `rumoca-ir-dae` says "FMI-style causality metadata for downstream JSON consumers".
    /// The flat table handed the longest — ast's — to every pane, so the DAE pane
    /// explained a Modelica type alias when asked about FMI metadata.
    #[test]
    fn a_stage_prefers_its_own_crates_doc() {
        let help = FieldHelp::load();
        assert!(
            help.crates_documenting("causality").len() > 1,
            "this test needs a genuinely contested name; `causality` has become \
             uncontested: {:?}",
            help.crates_documenting("causality"),
        );

        let dae = help
            .for_stage(StageKind::Dae)
            .get("causality")
            .expect("dae doc");
        let parse = help
            .for_stage(StageKind::Parse)
            .get("causality")
            .expect("ast doc");

        assert!(
            dae.contains("FMI"),
            "the DAE pane must get rumoca-ir-dae's meaning, got: {dae}",
        );
        assert_ne!(
            dae, parse,
            "two stages documenting a name differently must not share one tooltip",
        );
    }

    /// The reported symptom: `names` under Solve lowering is about slots, not imports.
    ///
    /// Doug, 2026-09-03, five times in one afternoon: he could not find `Y` in Solve
    /// lowering. Part of why is that the pane's own help was lying — hovering `names`
    /// under `solver_maps` produced `rumoca-ir-ast`'s *"All names imported from the
    /// package"*, a doc about an **import clause**.
    ///
    /// Both halves of the fix are needed and this test fails without either: the
    /// per-stage keying, and the `rumoca-ir-solve` doc comment that gives the stage
    /// something of its own to prefer. Named after the symptom so a future reader can
    /// find it from the complaint.
    #[test]
    fn solve_lowerings_names_is_documented_as_slots_not_imports() {
        let help = FieldHelp::load();
        assert_eq!(
            help.crates_documenting("names"),
            vec!["rumoca-ir-ast", "rumoca-ir-solve"],
            "premise: both meanings are on record",
        );

        let tip = help
            .for_stage(StageKind::SolveLowering)
            .get("names")
            .expect("Solve lowering documents `names`");
        assert!(
            tip.contains("slot order"),
            "Solve lowering must get the solver's meaning, got: {tip}",
        );
        assert!(
            !tip.contains("imported"),
            "the import-clause doc must not reach this pane: {tip}",
        );
        assert!(
            help.for_stage(StageKind::Parse)
                .get("names")
                .is_some_and(|t| t.contains("imported")),
            "and Parse must still get the AST's meaning",
        );
    }

    /// Every tooltip names the crate it came from.
    ///
    /// **This is the half that protects the stages `ir_crate` cannot name.**
    /// `Initialization` returns `None`, so its tooltips are a labelled fallback rather
    /// than a silent guess — and a reader seeing `rumoca-ir-ast` under Solve lowering
    /// knows to distrust it, which is the whole difference between a wrong claim and a
    /// qualified one.
    #[test]
    fn every_tooltip_names_its_crate() {
        let help = FieldHelp::load();
        let mut checked = 0usize;
        for stage in StageKind::ALL {
            for (field, tip) in help.for_stage(*stage) {
                assert!(
                    IR_CRATES.iter().any(|c| tip.ends_with(c)),
                    "{}'s tooltip for `{field}` names no crate: {tip:?}",
                    stage.slug(),
                );
                checked += 1;
            }
        }
        assert!(
            checked > 1_000,
            "only {checked} tooltips were examined \u{2014} the scan is broken, which \
             looks like success",
        );
    }

    /// `Initialization` keeps its tooltips rather than losing them to a `None` crate.
    ///
    /// Keying strictly by stage would have emptied this pane. The fallback is what makes
    /// the accuracy fix not a regression.
    #[test]
    fn a_stage_with_no_known_crate_still_gets_labelled_help() {
        let help = FieldHelp::load();
        assert_eq!(StageKind::Initialization.ir_crate(), None, "premise");
        let table = help.for_stage(StageKind::Initialization);
        assert!(
            !table.is_empty(),
            "a stage whose crate is unknown must keep labelled help, not lose it",
        );
    }

    /// Every crate `StageKind::ir_crate` names is actually harvested.
    ///
    /// Naming a crate no one harvests is the silent way to empty a pane: `for_stage`
    /// would prefer a table that does not exist and fall back to another crate's doc
    /// **for every field**, which is the defect this arrangement exists to end, wearing
    /// the costume of the fix. `rumoca-phase-structural` was in exactly that position
    /// when the mapping was first written.
    #[test]
    fn every_stage_ir_crate_is_harvested_or_none() {
        let mut named = 0usize;
        for stage in StageKind::ALL {
            if let Some(c) = stage.ir_crate() {
                assert!(
                    IR_CRATES.contains(&c),
                    "{} maps to {c:?}, which IR_CRATES does not harvest \u{2014} that pane \
                     would silently fall back to another crate's docs for every field",
                    stage.slug(),
                );
                named += 1;
            }
        }
        assert!(
            named >= 9,
            "only {named} stages name a crate; the mapping has been gutted",
        );
    }

    /// Every `rumoca-ir-*` crate HRW depends on is harvested for field help.
    ///
    /// # What this prevents, and why nothing noticed for weeks
    ///
    /// [`IR_CRATES`] listed only `rumoca-ir-ast` and `rumoca-ir-flat`, with a note saying
    /// to extend it "as later stages get their own tabs". The tabs arrived; the list did
    /// not follow. So the DAE pane offered **no field help at all** while
    /// `rumoca-ir-dae` already carried 290 lines of `///` docs — the answers existed and
    /// HRW was not asking for them.
    ///
    /// **A missing tooltip is invisible.** Nothing rendered wrong, no test failed, and the
    /// loss only surfaced when Doug said he needed to learn the IR and went looking at
    /// the stage Rumoca documents best.
    ///
    /// Derived from `Cargo.toml` rather than a second hand-written list, because a
    /// hand-written one is the thing that just rotted.
    #[test]
    fn every_ir_dependency_is_harvested_for_field_help() {
        let manifest = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"))
            .expect("hrw/Cargo.toml");

        let mut depended: Vec<String> = Vec::new();
        for line in manifest.lines() {
            let t = line.trim();
            if t.starts_with('#') {
                continue;
            }
            if let Some((name, _)) = t.split_once('=') {
                let name = name.trim();
                if name.starts_with("rumoca-ir-") && !depended.iter().any(|d| d == name) {
                    depended.push(name.to_owned());
                }
            }
        }

        assert!(
            depended.len() >= 3,
            "only {} rumoca-ir-* dependencies were found in Cargo.toml \u{2014} the scan is \
             broken, which looks like success",
            depended.len(),
        );

        let missed: Vec<&String> = depended
            .iter()
            .filter(|d| !IR_CRATES.contains(&d.as_str()))
            .collect();
        assert!(
            missed.is_empty(),
            "HRW depends on {missed:?} but does not harvest their `///` field docs, so \
             those stages render with no field help. Add them to `field_help::IR_CRATES` \
             and re-run `cargo run -p hrw --example gen_field_help`.",
        );
    }
}
