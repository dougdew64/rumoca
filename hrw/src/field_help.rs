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
        "Simulation" => (
            "Phase 9 · Simulation",
            "docs/compiler-phases/phase9_simulation/simulation.md",
        ),
        _ => (
            "Understanding · Overview",
            "docs/compiler-phases/high_level_overview.md",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The embedded field_help.json parses and contains entries.
    #[test]
    fn field_help_loads_non_empty() {
        let help = load();
        assert!(!help.is_empty(), "field_help.json should contain entries");
    }

    /// Key IR field names that the tree inspector relies on are present.
    #[test]
    fn field_help_contains_core_ir_fields() {
        let help = load();
        for key in ["def_id", "classes", "components", "equations", "name"] {
            assert!(help.contains_key(key), "field_help missing expected key: {key}");
        }
    }

    /// Every known stage maps to a chapter file that exists on disk.
    #[test]
    fn chapter_for_stage_files_exist() {
        let stages = [
            "Parse", "Resolve", "Typecheck", "Instantiate", "Flatten",
            "Structural", "Index reduction", "Initialization", "Events",
            "Solve lowering", "Simulation",
        ];
        let root = env!("CARGO_MANIFEST_DIR");
        for stage in stages {
            let (label, rel) = chapter_for_stage(stage);
            assert!(!label.is_empty(), "{stage}: empty label");
            let abs = format!("{root}/{rel}");
            assert!(
                std::path::Path::new(&abs).exists(),
                "{stage}: chapter file not found: {rel}"
            );
        }
    }

    /// An unknown stage falls back to the overview.
    #[test]
    fn chapter_for_unknown_stage_falls_back() {
        let (label, _) = chapter_for_stage("Nonexistent");
        assert!(label.contains("Overview"));
    }
}
