//! HRW Observatory library crate — module registration and shared exports.
//!
//! ## Why a library crate?
//!
//! Rust binaries (`main.rs`) cannot be depended on by other targets. By putting
//! all modules in a library crate (`lib.rs`), both the GUI binary (`main.rs`)
//! *and* headless tools (`examples/gen_trace`, which writes a specimen's durable
//! compilation trace log) can share one implementation of the compilation
//! pipeline. The binary is a thin shell that launches eframe; all logic lives here.
//!
//! ## Module map
//!
//! The observatory is organized around a few key roles:
//!
//! - **`app`** — the top-level `eframe::App` implementation: UI layout, tab bar,
//!   specimen list, and the glue that wires everything together each frame.
//! - **`worker`** — background-thread compilation and simulation; sends results
//!   back to the UI over a channel so the GUI never blocks.
//! - **`bridge`** — the "Claude bridge": writes a JSON focus file describing
//!   what the user captured, so Claude Code can reason about it.
//! - **`tree`** — the generic serde-value tree inspector, used for every pipeline
//!   stage's IR (one widget, many stages).
//! - **`expr_format`** — Modelica-like expression pretty-printer (precedence-aware).
//! - **`canvas`** — reusable pan/zoom scaffold for custom-painted views.
//! - **`spyplot`** — BLT (block lower triangular) spy-plot, a custom-painter view.
//! - **`incidence_view`** — incidence matrix (equation x unknown adjacency) view.
//! - **`matching_anim`** — animated matching stepper (augmenting-path replay).
//! - **`tarjan_anim`** — animated Tarjan SCC stepper (BLT discovery replay).
//! - **`reduction_view`** — index reduction process summary (the Pantelides funnel).
//! - **`equation_sheet`** — readable equation sheet from the flat DAE (grouped by origin).
//! - **`log_view`** — timestamped compilation/simulation log panel.
//! - **`colors`** — shared color constants used across canvas and view modules.
//! - **`field_help`** — build-time-embedded doc comments for IR fields (fast help).

pub mod app;
pub mod bridge;
pub mod canvas;
pub mod colors;
pub mod equation_sheet;
pub mod expr_format;
pub mod field_help;
pub mod incidence_view;
pub mod matching_anim;
pub mod log_view;
pub mod reduction_view;
pub mod spyplot;
pub mod tarjan_anim;
pub mod tree;
pub mod worker;

/// Extract a JSON array of strings into a `Vec<String>`.
///
/// Defensive — returns an empty vec if the value is missing or not an array.
/// Used by multiple views to extract equation names, unknown names, etc.
pub fn str_vec(v: Option<&serde_json::Value>) -> Vec<String> {
    v.and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn str_vec_extracts_strings() {
        let arr = json!(["a", "b", "c"]);
        assert_eq!(str_vec(Some(&arr)), vec!["a", "b", "c"]);
    }

    #[test]
    fn str_vec_skips_non_strings() {
        let arr = json!(["a", 42, "b", null]);
        assert_eq!(str_vec(Some(&arr)), vec!["a", "b"]);
    }

    #[test]
    fn str_vec_returns_empty_on_none() {
        assert!(str_vec(None).is_empty());
    }

    #[test]
    fn str_vec_returns_empty_on_non_array() {
        assert!(str_vec(Some(&json!("not an array"))).is_empty());
    }
}
