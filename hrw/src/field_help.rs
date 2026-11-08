//! Generic (build-time) field help — delivered as hover tooltips on tree nodes.
//!
//! ## Two-tier help architecture
//!
//! HRW provides two kinds of help for understanding IR fields:
//!
//! 1. **Fast tier (this module):** instant, offline, generic. The `///` doc
//!    comments that Rumoca's authors wrote on struct fields in the [`IR_CRATES`]
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
// Two tables: `fields` (field -> crate -> doc) and `types` (crate -> type -> field -> doc).
const FIELD_HELP_JSON: &str = include_str!("field_help.json");

/// The embedded file's shape, exactly as `gen_field_help` writes it.
#[derive(serde::Deserialize, Default)]
struct Table {
    /// `field -> crate -> doc`, longest doc winning within a crate.
    fields: HashMap<String, HashMap<String, String>>,
    /// `crate -> type -> field -> doc`, the doc on that type's own field.
    types: HashMap<String, HashMap<String, HashMap<String, String>>>,
}

/// Where the doc for one key a stage publishes lives: **a named Rumoca type's field**.
///
/// For a stage whose value HRW assembles by hand rather than serialises, the key is
/// HRW's and the meaning is Rumoca's, so the link between them is a fact HRW must
/// state. It cannot be inferred from the key: `role` is documented in two harvested
/// crates, and the doc that means a pin's role is not the longest of them.
#[derive(Debug, Clone, Copy)]
pub struct Route {
    /// The key as the stage publishes it.
    pub key: &'static str,
    /// The harvested crate that owns the type.
    pub krate: &'static str,
    /// The type, as declared (`InitialValuePin`).
    pub ty: &'static str,
    /// The type's field. Usually `key`; differs where HRW renames it.
    pub field: &'static str,
}

/// The stages whose tooltips come from [`Route`]s instead of from a crate.
///
/// **A routed stage gets exactly its routes and nothing else** — no longest-doc
/// fallback. That is the point: before 2026-11-08 the Initialization pane, having no
/// crate, borrowed the longest doc for every key, and five of its nine tooltips were
/// false (`definitions` read "The parsed class definitions", `role` described a
/// Solve-lowering slot, `column` said "`y` or `p`"). A key with no route has no
/// tooltip, which is the honest answer for the values HRW computes itself.
pub fn routes(stage: StageKind) -> Option<&'static [Route]> {
    match stage {
        StageKind::Initialization => Some(INITIALIZATION_ROUTES),
        _ => None,
    }
}

/// `worker::initialization_from_initial_pins` builds each pin from one
/// `rumoca_phase_structural::InitialValuePin`, and the stage's `pins` array is
/// `PreparedSystem::pins`. Every route here is read from that function.
const INITIALIZATION_ROUTES: &[Route] = &[
    Route::same("pins", "PreparedSystem"),
    Route::same("role", "InitialValuePin"),
    Route::same("coordinate", "InitialValuePin"),
    Route::same("scalar", "InitialValuePin"),
    // HRW renders the pin's signed terms as one expression; the doc is the terms'.
    Route::same("value", "InitialValuePin"),
    Route::same("source", "InitialValuePin"),
    Route::same("source_scalar", "InitialValuePin"),
    // **Renamed**: HRW publishes the pin's `provenance` as a resolved source location.
    Route {
        key: "declared_at",
        krate: "rumoca-phase-structural",
        ty: "InitialValuePin",
        field: "provenance",
    },
];

/// Keys the Initialization stage publishes that **deliberately carry no tooltip**.
///
/// `determinacy` and its six counts are HRW's own summary over the pins and the DAE's
/// states — Rumoca documents none of them, so any doc would be HRW-written prose, and
/// that waits for real use (Doug's choice, 2026-11-08). The rest are the parts of a
/// source location HRW resolves for `declared_at`.
/// `the_initialization_pane_routes_every_key_it_publishes` holds the two lists, this
/// and [`INITIALIZATION_ROUTES`], against what the stage actually publishes.
pub const INITIALIZATION_UNDOCUMENTED: &[&str] = &[
    "determinacy",
    "state_scalars",
    "determined",
    "only_guessed",
    "definitions",
    "checks",
    "partly_fixed_states",
    "file",
    "line",
    "column",
    "excerpt",
    "line_text",
    "byte_start",
    "byte_end",
];

impl Route {
    /// A route to `rumoca-phase-structural`'s `ty`, whose field is named like the key.
    const fn same(key: &'static str, ty: &'static str) -> Self {
        Self {
            key,
            krate: "rumoca-phase-structural",
            ty,
            field: key,
        }
    }
}

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
///    That is what makes a residual mismatch *visible* rather than silent. It is also
///    useful in itself while Doug is learning the IR: the tooltip says which crate owns
///    the node.
///
/// # And a third, for a stage whose value HRW assembles — routes (2026-11-08)
///
/// Mechanism 2 was the only protection for `Initialization`, whose crate is `None`, and
/// it was not enough: a labelled tooltip is still a false one when five of nine are
/// borrowed from the wrong type. A stage with [`routes`] gets **exactly** its routed
/// keys, each the doc on a named type's field, and no fallback. `Simulation` is the one
/// stage still on the labelled fallback; giving it routes is a separate item.
pub struct FieldHelp {
    /// `field -> crate -> doc`, exactly as `gen_field_help` writes it.
    by_field: HashMap<String, HashMap<String, String>>,
    /// `crate -> type -> field -> doc`, exactly as `gen_field_help` writes it.
    by_type: HashMap<String, HashMap<String, HashMap<String, String>>>,
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
        let Table {
            fields: by_field,
            types: by_type,
        } = serde_json::from_str(FIELD_HELP_JSON).unwrap_or_default();

        let mut per_stage = HashMap::new();
        for stage in StageKind::ALL {
            if let Some(routes) = routes(*stage) {
                // Exactly the routed keys. A route that no longer resolves is dropped
                // here and named by `every_route_resolves_to_a_harvested_doc`.
                let table = routes
                    .iter()
                    .filter_map(|r| {
                        let doc = type_doc(&by_type, r)?;
                        Some((
                            r.key.to_owned(),
                            format!(
                                "{doc}\n\n\u{2014} {} \u{00b7} {}::{}",
                                r.krate, r.ty, r.field
                            ),
                        ))
                    })
                    .collect();
                per_stage.insert(stage.slug(), table);
                continue;
            }
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
            by_type,
            per_stage,
            empty: HashMap::new(),
        }
    }

    /// The doc a route points at, or `None` when that type's field is not harvested.
    pub fn route_doc(&self, route: &Route) -> Option<&str> {
        type_doc(&self.by_type, route)
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

/// `route`'s doc in the `types` table.
fn type_doc<'t>(
    by_type: &'t HashMap<String, HashMap<String, HashMap<String, String>>>,
    route: &Route,
) -> Option<&'t str> {
    by_type
        .get(route.krate)?
        .get(route.ty)?
        .get(route.field)
        .map(String::as_str)
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
    /// Both halves of the fix were needed and this test failed without either: the
    /// per-stage keying, and the `rumoca-ir-solve` doc comment that gave the stage
    /// something of its own to prefer. Named after the symptom so a future reader can
    /// find it from the complaint.
    ///
    /// # RE-GROUNDED 2026-11-06 — the symptom is unreachable, so this asserts that
    ///
    /// **v0.10.4 did not merely delete that doc comment; it deleted the field.**
    /// `rumoca-ir-solve`'s `SolverNameIndexMaps` is gone, and measuring all 28
    /// specimens' IR confirms the consequence: **`names` appears in 0 of 258 stage JSON
    /// files.** No pane can show the field, so no pane can show a wrong tooltip for it.
    ///
    /// The original assertions could therefore never pass again, and keeping them would
    /// have blocked `gen_field_help` forever to protect a tooltip nobody can see — which
    /// is what left step 4 of `docs/updating-rumoca.md` unfinished. The general property
    /// they guarded, *a stage prefers its own crate's doc*, is held by
    /// [`a_stage_prefers_its_own_crates_doc`] over `causality`, which **is** still a
    /// contested name and still published by the DAE stage.
    ///
    /// **What this now guards is the claim of absence.** If `rumoca-ir-solve` ever
    /// documents `names` again, the field is plausibly back and the symptom with it — so
    /// this fails and tells the reader to restore the assertions above rather than
    /// discover the complaint a second time.
    #[test]
    fn the_solve_lowering_names_symptom_is_unreachable() {
        let help = FieldHelp::load();
        assert_eq!(
            help.crates_documenting("names"),
            vec!["rumoca-ir-ast"],
            "`names` is contested again, so the 2026-09-03 symptom may be reachable: \
             restore the per-stage assertions this test replaced (see its doc) rather \
             than widening this one",
        );
        assert!(
            help.for_stage(StageKind::Parse)
                .get("names")
                .is_some_and(|t| t.contains("imported")),
            "the AST's import-clause meaning is still the one Parse should carry",
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
                // The label is the last paragraph: `— <crate>`, or for a routed stage
                // `— <crate> · <Type>::<field>`.
                let label = tip.rsplit("\n\n").next().unwrap_or_default();
                assert!(
                    IR_CRATES.iter().any(|c| {
                        label == format!("\u{2014} {c}")
                            || label.starts_with(&format!("\u{2014} {c} \u{00b7} "))
                    }),
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

    /// A stage with no crate **and no routes** keeps labelled help rather than none.
    ///
    /// Keying strictly by stage would have emptied such a pane. This was `Initialization`
    /// until 2026-11-08, when it got [`routes`] because its labelled fallback was mostly
    /// false; `Simulation` is the stage left on the fallback, pending its own routes.
    #[test]
    fn a_stage_with_no_known_crate_still_gets_labelled_help() {
        let help = FieldHelp::load();
        assert_eq!(StageKind::Simulation.ir_crate(), None, "premise");
        assert!(routes(StageKind::Simulation).is_none(), "premise");
        let table = help.for_stage(StageKind::Simulation);
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
        let manifest =
            std::fs::read_to_string(crate::manifest_path!("Cargo.toml")).expect("hrw/Cargo.toml");

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

    /// Every route names a type's field that the harvest actually documents.
    ///
    /// A route that stopped resolving — an upstream rename, a deleted doc comment — is
    /// dropped silently by `load`, so its key loses its tooltip with nothing on screen to
    /// say so. This is the loud half. The fabricated route proves the lookup can fail,
    /// so the loop below is not passing because `route_doc` always answers.
    #[test]
    fn every_route_resolves_to_a_harvested_doc() {
        let help = FieldHelp::load();
        let mut checked = 0usize;
        for stage in StageKind::ALL {
            for route in routes(*stage).unwrap_or_default() {
                assert!(
                    help.route_doc(route).is_some(),
                    "{}'s route for `{}` names {}::{}::{}, which the harvest does not \
                     document \u{2014} re-run gen_field_help, or the type was renamed upstream",
                    stage.slug(),
                    route.key,
                    route.krate,
                    route.ty,
                    route.field,
                );
                checked += 1;
            }
        }
        assert!(checked >= 8, "only {checked} routes were checked");
        let bogus = Route {
            key: "scalar",
            krate: "rumoca-phase-structural",
            ty: "NoSuchType",
            field: "scalar",
        };
        assert!(
            help.route_doc(&bogus).is_none(),
            "a missing type must not resolve"
        );
    }

    /// The routed table is the reason for the `types` table: a pin's `scalar` is the
    /// **pin's** doc, not the longer one on `PinTerm`.
    ///
    /// Before 2026-11-08 the Initialization pane showed *"zero for scalar terms"* for a
    /// pin's `scalar`, because the crate-level table keeps one doc per name per crate and
    /// `PinTerm`'s is one character longer. The premise assertion keeps this meaningful:
    /// if the crate-level doc ever becomes the pin's, the type table is no longer what
    /// this test is measuring.
    #[test]
    fn a_pins_scalar_is_described_as_a_pins() {
        let help = FieldHelp::load();
        let crate_level = help
            .by_field
            .get("scalar")
            .and_then(|m| m.get("rumoca-phase-structural"))
            .expect("rumoca-phase-structural documents `scalar`");
        assert!(
            crate_level.contains("terms"),
            "premise: the crate-level doc is PinTerm's \u{2014} got {crate_level:?}",
        );

        let tip = help
            .for_stage(StageKind::Initialization)
            .get("scalar")
            .expect("Initialization routes `scalar`");
        assert!(
            tip.contains("scalar pins") && !tip.contains("terms"),
            "a pin's `scalar` must carry InitialValuePin's doc: {tip:?}",
        );
        assert!(tip.ends_with("InitialValuePin::scalar"), "{tip:?}");
    }

    /// A routed stage carries exactly its routes: no key borrows another type's doc.
    ///
    /// This is what removed the five false tooltips — `definitions` as "The parsed class
    /// definitions", `role` as a Solve-lowering slot, `value` and `source` as a
    /// modification's, `column` as "`y` or `p`". Each is checked by name.
    #[test]
    fn no_initialization_key_borrows_another_types_doc() {
        let help = FieldHelp::load();
        let table = help.for_stage(StageKind::Initialization);
        let routed = routes(StageKind::Initialization).expect("Initialization has routes");
        assert_eq!(
            table.len(),
            routed.len(),
            "the table must be the routes and nothing else: {:?}",
            table.keys().collect::<Vec<_>>(),
        );
        for key in INITIALIZATION_UNDOCUMENTED {
            assert!(
                !table.contains_key(*key),
                "`{key}` is HRW's own value and must carry no tooltip, got {:?}",
                table.get(*key),
            );
        }
        for (key, must_not) in [
            ("role", "announced_time"),
            ("value", "modification"),
            ("source", "eager resolution"),
        ] {
            let tip = table.get(key).expect("routed");
            assert!(
                !tip.contains(must_not),
                "`{key}` borrowed a wrong doc: {tip:?}"
            );
        }
    }

    /// Every key the Initialization stage publishes is routed or deliberately bare, and
    /// both lists name only keys it publishes.
    ///
    /// Read from the committed `MeasuredOscillator` trace — the first curated specimen
    /// with pins, so the only one where a pin's keys are observable at all. A new key
    /// in the stage fails here until someone decides whether it has a Rumoca doc; a key
    /// the stage stops publishing fails here instead of leaving a dead route.
    #[test]
    fn the_initialization_pane_routes_every_key_it_publishes() {
        fn keys(v: &serde_json::Value, out: &mut std::collections::BTreeSet<String>) {
            match v {
                serde_json::Value::Object(m) => {
                    for (k, child) in m {
                        out.insert(k.clone());
                        keys(child, out);
                    }
                }
                serde_json::Value::Array(a) => a.iter().for_each(|c| keys(c, out)),
                _ => {}
            }
        }
        let path = crate::manifest_path!(
            "docs",
            "specimen-notebook",
            "MeasuredOscillator",
            "trace",
            "initialization.json"
        );
        let stage: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).expect("committed trace"))
                .expect("trace parses");
        assert!(
            stage["pins"].as_array().is_some_and(|p| !p.is_empty()),
            "premise: MeasuredOscillator publishes a pin, or a pin's keys are unobserved",
        );
        let mut published = std::collections::BTreeSet::new();
        keys(&stage, &mut published);

        let routed: Vec<&str> = routes(StageKind::Initialization)
            .expect("routes")
            .iter()
            .map(|r| r.key)
            .collect();
        for key in &published {
            let r = routed.contains(&key.as_str());
            let u = INITIALIZATION_UNDOCUMENTED.contains(&key.as_str());
            assert!(
                r ^ u,
                "`{key}` must be in exactly one of the routes and the undocumented list \
                 (routed {r}, undocumented {u})",
            );
        }
        for key in routed.iter().chain(INITIALIZATION_UNDOCUMENTED) {
            assert!(
                published.contains(*key),
                "`{key}` is listed but the stage no longer publishes it",
            );
        }
    }
}
