//! Dump every stage's IR for a model that is **not** in the corpus.
//!
//! ```text
//! cargo run -p hrw --example stage_ir -- <out-dir> <a.mo> [b.mo ...]
//! ```
//!
//! Writes `<out-dir>/<stem>.<stage>.json` for each stage that produced IR, so an
//! arbitrary `.mo` — including a scratch specimen written minutes ago — can be
//! disassembled the way a corpus specimen can.
//!
//! # Why this exists when `gen_trace` already writes stage IR
//!
//! `gen_trace` is **corpus-bound by construction**: it builds its path as
//! `specimens/<Name>.mo` and writes into `docs/specimen-notebook/<Model>/trace/`. That is
//! correct for what it is — the durable, committed, regenerable record — and it means it
//! cannot look at anything else. **Every model that did the diagnostic work of 2026-09 was
//! outside `specimens/`**: `BareRc`, `ConnRc`, `RcStartProbe`, `RcFixedProbe`, `ThrownBall`.
//!
//! So the division is:
//!
//! | tool | question | scope |
//! |---|---|---|
//! | `gen_trace` | what is this specimen's committed IR? | the corpus, written to the notebook |
//! | `stage_outcomes` | which phase flagged this, and what did it say? | any path, notes only |
//! | `stage_ir` (here) | what does that phase's IR actually contain? | any path, full JSON |
//!
//! # What it earned permanence with
//!
//! Two findings that could not have been reached any other way, both from disassembling an
//! initialization residual this produced:
//!
//! - **A published mechanism was withdrawn.** `upstream-issues.md` asserted that eliminating
//!   the derivative left the state among the unknowns *"with nothing pinning it to `start`"*.
//!   Dumping `BareRc` — which simulates *correctly* — showed the same substitution, the same
//!   state-as-unknown plan, and a derivative-zero system with the same wrong solution. Every
//!   condition blamed held in the model that worked, so the cause was not the cause.
//! - **The trigger was isolated to connectors.** `ConnRc` versus `BareRc`, neither using a
//!   library, separated *connectors* from *the MSL* as the thing that decides it.
//!
//! **Expect to want it again right after a rebase**, which is the argument Doug made for
//! keeping it: the notebook check and fidelity suite will say *that* the IR moved, and this is
//! what says *how*. It reads whatever JSON a stage emits, so a changed IR shape costs it
//! nothing.
//!
//! Deliberately outside the gate: it compiles models against the MSL, which the notebook check
//! and fidelity suite already do on a fixed corpus. This is for models in no corpus.
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
    let mut args = std::env::args().skip(1);
    let Some(out) = args.next().map(PathBuf::from) else {
        eprintln!("usage: cargo run -p hrw --example stage_ir -- <out-dir> <a.mo> [b.mo ...]");
        std::process::exit(2);
    };
    std::fs::create_dir_all(&out).expect("create out dir");

    let models: Vec<PathBuf> = args.map(PathBuf::from).collect();
    if models.is_empty() {
        eprintln!("no models given; nothing to dump");
        std::process::exit(2);
    }

    for path in models {
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_owned());
        match compile_specimen(&path, msl_roots()) {
            Ok(FromWorker::Compiled { stages, .. }) => {
                // **Every stage, not a chosen pair.** The first version dumped only the two
                // the hour's question needed, which is exactly the shape that makes a tool
                // useless for the next question.
                for kind in StageKind::COMPILATION {
                    let Some(value) = stages.get(*kind).value.as_ref() else {
                        continue;
                    };
                    let dest = out.join(format!("{stem}.{}.json", kind.slug()));
                    let text = serde_json::to_string_pretty(value).expect("serialize stage IR");
                    std::fs::write(&dest, text).expect("write stage IR");
                    eprintln!("{}", dest.display());
                }
            }
            // Reported rather than skipped: a model that did not compile is a finding, and a
            // silent absence here would read as "this stage produced nothing".
            Ok(_) => eprintln!("{stem}: worker returned something other than Compiled"),
            Err(e) => eprintln!("{stem}: compile failed: {e}"),
        }
    }
}
