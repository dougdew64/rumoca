//! **Which frame handles which identifier** — the lookup that makes a tour link
//! land on an algorithm *step* rather than a stage.
//!
//! ```text
//! cargo run -p hrw --example frame_index -- SingleInertia
//! cargo run -p hrw --example frame_index -- MotorWithBrake der(w)
//! ```
//!
//! # Why this exists
//!
//! Doug, 2026-08-03: *"a step in the algorithm where the identifier you are
//! having me consider is being handled … you could pre-run the specimen to
//! monitor its operation and determine which step that identifier is handled
//! by. Then you could capture that fact in the navigation link."*
//!
//! `hrw://stage/Structural/MatchingAnim/frame/<n>` has worked since the
//! frame-seeking tour. What was missing was any way to know `<n>` without
//! watching the animation — so a tour author would be **guessing a number that
//! the link checker cannot catch**, because a wrong-but-valid frame index
//! resolves fine and simply lands on the wrong step. That is the quiet failure
//! this removes.
//!
//! # It reads the frames HRW will show, not a re-derivation
//!
//! The numbering must match what the reader sees, so this drives the same
//! `MatchingAnimation::from_incidence` the panel does and enumerates its
//! `steps()`. Recomputing the matching independently would be a *second*
//! definition of the frame sequence, and the two would drift the first time
//! either changed.
//!
//! Input is the specimen's committed trace (`docs/specimen-notebook/<Model>/
//! trace/structural.json`), which is generated and therefore correct by
//! construction — the same rule the rest of the notebook follows.

use std::path::PathBuf;

use hrw::incidence_view::IncidenceMatrix;
use hrw::matching_anim::MatchingAnimation;
use rumoca_phase_structural::matching::MatchingStep;

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(model) = args.next() else {
        eprintln!("usage: frame_index <Model> [identifier]");
        eprintln!("  with an identifier, prints only the frames that touch it");
        std::process::exit(2);
    };
    let filter = args.next();

    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("docs/specimen-notebook")
        .join(&model)
        .join("trace/structural.json");
    let Ok(text) = std::fs::read_to_string(&path) else {
        eprintln!("no structural trace for {model} at {}", path.display());
        eprintln!("generate one first:  cargo run -p hrw --example gen_trace -- {model}");
        std::process::exit(1);
    };
    let report: serde_json::Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{}: {e}", path.display());
            std::process::exit(1);
        }
    };

    let Some(mat) = IncidenceMatrix::from_report(&report) else {
        eprintln!("{model}: the structural report carries no incidence matrix");
        eprintln!("(a model that fails before structural analysis has no matching to animate)");
        std::process::exit(1);
    };
    let anim = MatchingAnimation::from_incidence(&mat);
    let eqs = mat.equation_names();
    let vars = mat.unknown_names();
    let name = |v: &[String], i: usize| v.get(i).cloned().unwrap_or_else(|| format!("#{i}"));

    println!("{model}: {} frames", anim.steps().len());
    println!("  link form:  hrw://stage/Structural/MatchingAnim/frame/<n>");
    println!();

    let mut shown = 0usize;
    for (n, step) in anim.steps().iter().enumerate() {
        // `var` is what a tour points at; `eq` is which equation was trying.
        let (var, line) = match step {
            MatchingStep::TryEquation(e) => {
                (None, format!("try equation {}", name(eqs, *e)))
            }
            MatchingStep::Explore { eq, var } => (
                Some(*var),
                format!("explore {} x {}", name(eqs, *eq), name(vars, *var)),
            ),
            MatchingStep::FoundFree { eq, var } => (
                Some(*var),
                format!("FOUND FREE {} -> {}", name(eqs, *eq), name(vars, *var)),
            ),
            MatchingStep::TryDisplace { eq, var, holder } => (
                Some(*var),
                format!(
                    "displace {} from {} (wanted by {})",
                    name(vars, *var),
                    name(eqs, *holder),
                    name(eqs, *eq),
                ),
            ),
            MatchingStep::DisplaceOk { eq, var } => (
                Some(*var),
                format!("displaced ok {} -> {}", name(eqs, *eq), name(vars, *var)),
            ),
            MatchingStep::DisplaceFail { eq, var } => (
                Some(*var),
                format!("displace FAILED {} x {}", name(eqs, *eq), name(vars, *var)),
            ),
            MatchingStep::Assign { eq, var } => (
                Some(*var),
                format!("ASSIGN {} := {}", name(vars, *var), name(eqs, *eq)),
            ),
            MatchingStep::EquationFailed(e) => {
                (None, format!("equation FAILED {}", name(eqs, *e)))
            }
        };

        if let Some(f) = &filter {
            // Substring, because a tour author knows `der(w)` and not its index.
            // Deliberately not identity matching: this is an authoring aid whose
            // output a human reads before using, not something that decides
            // identity on its own. See `docs/identity-and-provenance.md`.
            let hit = var.is_some_and(|v| name(vars, v).contains(f.as_str()))
                || line.contains(f.as_str());
            if !hit {
                continue;
            }
        }
        // **The link, fully formed** — not a number for the author to adjust.
        //
        // This used to print the 0-based index and then claim it worked verbatim in
        // `hrw://…/frame/<n>`. It does not: links are 1-based, matching the counter
        // on screen, and `parse_hrw_link` subtracts one. So every link written from
        // this output landed **one frame early** — a wrong-but-valid index that
        // resolves fine, shows the wrong step, and no link checker can see. That is
        // exactly the failure this tool exists to prevent, and it was committing it.
        //
        // Fixed 2026-08-03 by emitting the link instead of a number, through
        // `hrw::app::frame_link`, which `a_frame_link_round_trips_through_the_parser`
        // binds to the parser.
        println!(
            "  frame {n:>4}  {line}\n              {}",
            hrw::app::frame_link("Structural", "MatchingAnim", n),
        );
        shown += 1;
    }

    if shown == 0 {
        println!("  (no frame mentions {:?})", filter.unwrap_or_default());
    }
    println!();
    println!("Frame numbers above are 0-based, as the algorithm's step list numbers them.");
    println!("The links are 1-based, matching the counter on screen. Copy the link, not the number.");
}
