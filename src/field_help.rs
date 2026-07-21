//! Generic (build-time) field help — the **fast tier**.
//!
//! These are the `///` doc comments Rumoca's authors wrote on `rumoca-ir-ast`
//! IR fields, extracted at the pinned rev and embedded here (`field_help.json`).
//! Keyed by field name (v1; disambiguation by owning type is a later refinement).
//! Shown instantly in the right-hand panel on left-click — no Claude, no latency.
//! The *specific* tier ("why did THIS one happen") stays with the bridge + chat.
//!
//! Regenerate after a Rumoca pin bump with `cargo run --example gen_field_help`
//! (see `examples/gen_field_help.rs` and the checklist in `docs/updating-rumoca.md`).

use std::collections::HashMap;

const FIELD_HELP_JSON: &str = include_str!("field_help.json");

/// Parse the embedded table (field name → doc text).
pub fn load() -> HashMap<String, String> {
    serde_json::from_str(FIELD_HELP_JSON).unwrap_or_default()
}

/// The `docs/compiler-phases` chapter (label, repo-relative path) for the phase
/// whose IR is being viewed — the concept-level "read more" link.
pub fn chapter_for_stage(stage: &str) -> (&'static str, &'static str) {
    match stage {
        "Parse" => (
            "Phase 1 · Parsing & AST",
            "docs/compiler-phases/phase1_parsing_and_ast/parsing_and_ast.md",
        ),
        "Resolve" => (
            "Phase 2 · Resolve & Scope",
            "docs/compiler-phases/phase2_resolve_and_scope/resolve_and_scope.md",
        ),
        "Typecheck" => (
            "Phase 3 · Typecheck & Dimensions",
            "docs/compiler-phases/phase3_typecheck_and_dims/typecheck_and_dims.md",
        ),
        "Instantiate" => (
            "Phase 4 · Instantiate",
            "docs/compiler-phases/phase4_instantiate/instantiate.md",
        ),
        "Flatten" => (
            "Phase 5 · Flatten",
            "docs/compiler-phases/phase5_flatten/flatten.md",
        ),
        "Structural" => (
            "Phase 7 · Structural Analysis",
            "docs/compiler-phases/phase7_structural_analysis/structural_analysis.md",
        ),
        "Index reduction" => (
            "Index reduction (Pantelides / dummy derivatives)",
            "docs/compiler-phases/phase6_dae_construction/index_reduction.md",
        ),
        "Initialization" => (
            "Initialization · IC planning",
            "docs/compiler-phases/phase7_structural_analysis/ic_plan.md",
        ),
        "Events" => (
            "DAE construction · events & hybrid structure",
            "docs/compiler-phases/phase6_dae_construction/dae_construction.md",
        ),
        "Solve lowering" => (
            "Phase 8 · Solve lowering",
            "docs/compiler-phases/phase8_solve_lowering/solve_lowering.md",
        ),
        _ => (
            "Understanding · Overview",
            "docs/compiler-phases/high_level_overview.md",
        ),
    }
}
