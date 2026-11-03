//! Which phase flags this model, and what does it say?
//!
//! ```text
//! cargo run -p hrw --example stage_outcomes -- <a.mo> [b.mo ...]
//! ```
//!
//! One line per stage: whether IR came out, the `Outcome`, and the stage's note. Any path
//! works, including a scratch specimen under `.hrw-bridge/specimens/` written moments ago.
//!
//! # Why this is committed when probes normally are not
//!
//! `CLAUDE.md` says a probe lives in the working tree until it earns permanence, and names the
//! test: *should the repository keep this?* — not *did it work?* This one earns it by answering
//! a question that recurs and that nothing else answers cheaply.
//!
//! **The alternative is loading the model in the running app and reading tabs**, which costs a
//! round trip through Doug and — measured twice on 2026-09-04 — produces hedged readings on the
//! panes that are hardest to read. It also cannot compare two models side by side, which is
//! exactly what a diagnosis usually needs: on 2026-09-12 the answer to *"if a potential variable
//! is in no `connect`, which phase fails?"* was the **difference** between two specimens —
//! `DanglingPin` balanced at 17/17 and clean, `OrphanConnector` at 10/9 and flagged `singular` —
//! and neither alone would have answered it.
//!
//! **It reports rather than judges.** `Outcome` and `note` are printed as the stage set them, so
//! a stage that flagged nothing prints nothing; the absence is the finding. That is the same rule
//! the bridge follows, and the reason this is safe to read as evidence.
//!
//! Deliberately **not** in the gate: it compiles models against the MSL, which is what the
//! notebook check and the fidelity suite already do on a fixed corpus. This is for models that
//! are not in any corpus yet.
use std::path::PathBuf;

use hrw::worker::{FromWorker, StageKind, compile_specimen};

fn msl_roots() -> Vec<PathBuf> {
    let base = format!("{}/vendor/msl", env!("CARGO_MANIFEST_DIR"));
    vec![
        PathBuf::from(format!("{base}/Modelica 4.1.0")),
        PathBuf::from(format!("{base}/ModelicaServices 4.1.0")),
        PathBuf::from(format!("{base}/Complex.mo")),
    ]
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        // **Silence would be indistinguishable from "nothing was flagged"**, which is the one
        // reading this tool must never invite.
        eprintln!("usage: cargo run -p hrw --example stage_outcomes -- <a.mo> [b.mo ...]");
        eprintln!("       any path works, including .hrw-bridge/specimens/<Scratch>.mo");
        std::process::exit(2);
    }
    for arg in args {
        let path = PathBuf::from(&arg);
        println!("\n================ {} ================", path.display());
        match compile_specimen(&path, msl_roots()) {
            Ok(FromWorker::Compiled { stages, .. }) => {
                for kind in StageKind::COMPILATION {
                    let stage = stages.get(*kind);
                    let has = if stage.value.is_some() { "IR " } else { "-- " };
                    let outcome = format!("{:?}", stage.outcome);
                    let note = stage.note.as_deref().unwrap_or("");
                    println!("  {has} {:<16} {:<10} {note}", kind.slug(), outcome);
                }
            }
            Ok(_) => println!("  unexpected worker message (not Compiled)"),
            Err(e) => println!("  compile failed: {e}"),
        }
    }
}
