//! **The tour catalogue — written for Claude, generated from the tours themselves.**
//!
//! ```text
//! cargo run -p hrw --example gen_tour_catalogue
//! ```
//!
//! Writes `docs/fixture-tours/CATALOGUE.md`. **The generator itself lives in
//! `hrw::tour::catalogue`**, so `tour_catalogue_is_current` checks the same code that
//! writes the file rather than a second implementation of it.
//!
//! `docs/ideas.md` #63: Claude could answer with text or a freshly written ad hoc
//! tour, and had no way to say *"the answer already exists -- walk
//! `failure-typecheck` from stop 2"*. At fourteen tours, reading all of them to
//! answer one question is the cost this removes.

use std::path::PathBuf;

fn main() {
    let dir = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/fixture-tours"));
    let out = dir.join("CATALOGUE.md");
    let text = hrw::tour::catalogue();
    std::fs::write(&out, &text).expect("write catalogue");
    println!("wrote {} ({} bytes)", out.display(), text.len());
}
