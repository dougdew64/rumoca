//! **The lab catalogue — written for Claude, generated from the labs themselves.**
//!
//! ```text
//! cargo run -p hrw --example gen_lab_catalogue
//! ```
//!
//! Writes `docs/fixture-labs/CATALOGUE.md`. **The generator itself lives in
//! `hrw::lab::catalogue`**, so `lab_catalogue_is_current` checks the same code that
//! writes the file rather than a second implementation of it.
//!
//! `docs/ideas.md` #63: Claude could answer with text or a freshly written ad hoc
//! lab, and had no way to say *"the answer already exists -- run
//! `failure-typecheck` from stop 2"*. At fourteen labs, reading all of them to
//! answer one question is the cost this removes.

use std::path::PathBuf;

fn main() {
    let dir = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/fixture-labs"));
    let out = dir.join("CATALOGUE.md");
    let text = hrw::lab::catalogue();
    std::fs::write(&out, &text).expect("write catalogue");
    println!("wrote {} ({} bytes)", out.display(), text.len());
}
