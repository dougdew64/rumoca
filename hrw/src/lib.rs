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
//! - **`canvas`** — reusable pan/zoom scaffold for custom-painted views.
//! - **`spyplot`** — BLT (block lower triangular) spy-plot, a custom-painter view.
//! - **`incidence_view`** — incidence matrix (equation x unknown adjacency) view.
//! - **`reduction_view`** — index reduction process summary (the Pantelides funnel).
//! - **`log_view`** — timestamped compilation/simulation log panel.
//! - **`field_help`** — build-time-embedded doc comments for IR fields (fast help).

pub mod app;
pub mod bridge;
pub mod canvas;
pub mod field_help;
pub mod incidence_view;
pub mod log_view;
pub mod reduction_view;
pub mod spyplot;
pub mod tree;
pub mod worker;
