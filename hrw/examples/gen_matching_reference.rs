//! **The matching live-trace reference — generated from the algorithm's source.**
//!
//! ```text
//! cargo run -p hrw --example gen_matching_reference
//! ```
//!
//! Writes `docs/compiler-phases/phase7_structural_analysis/matching-live-reference.md`:
//! the emit site of every `MatchingStep`, and the frame-by-frame ledger with
//! recursion depth for one specimen that succeeds and one that fails.
//!
//! **The generator lives in `hrw::matching_ledger::reference`**, so
//! `the_generated_reference_is_current` checks the same code that writes the
//! file rather than a second implementation of it — the drift `fidelity-plan.md`
//! warns about.
//!
//! Run this after **any** change to `crates/rumoca-phase-structural/src/matching.rs`.
//! Line numbers move when code above them moves, and `CLAUDE.md`'s standing
//! warning is that nothing compiles a Markdown table — a tour quoting a shifted
//! line simply confuses the reader. The test turns that silent rot into a
//! failure that names this command.

use std::path::PathBuf;

fn main() {
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(hrw::matching_ledger::REFERENCE_PATH);
    let text = hrw::matching_ledger::reference();
    std::fs::write(&out, &text).expect("write matching reference");
    println!("wrote {} ({} bytes)", out.display(), text.len());
}
