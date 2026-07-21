//! HRW Observatory library crate.
//!
//! The observatory's modules live here (not in `main.rs`) so both the GUI binary
//! and headless tools — `examples/gen_trace`, which writes a specimen's durable
//! compilation *trace log* — share one implementation of the pipeline. See
//! `docs/CHARTER.md` and `CLAUDE.md`.

pub mod app;
pub mod bridge;
pub mod canvas;
pub mod field_help;
pub mod incidence_view;
pub mod spyplot;
pub mod tree;
pub mod worker;
