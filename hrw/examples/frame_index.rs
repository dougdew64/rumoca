//! **Which frame handles which identifier** — the lookup that makes a lab link
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
//! frame-seeking lab. What was missing was any way to know `<n>` without
//! watching the animation — so a lab author would be **guessing a number that
//! the link checker cannot catch**, because a wrong-but-valid frame index
//! resolves fine and simply lands on the wrong step. That is the quiet failure
//! this removes.
//!
//! # It reads the frames HRW will show — and that claim went stale once
//!
//! **Rewritten 2026-08-04.** This tool used to compile nothing. It read the
//! committed structural trace and called `MatchingAnimation::from_incidence`,
//! justified by a header saying it *"drives the same constructor the panel
//! does"*. **That stopped being true the same day the panel switched to captured
//! frames**, and nothing failed: matching is deterministic, so the two agreed —
//! by luck of the algorithm, exactly the reasoning the capture scopes were built
//! to stop relying on.
//!
//! It now **compiles the specimen and reads `matching_frames` off the
//! result** — the identical values `App` hands to the animation. Two sources of
//! frame numbering cannot drift apart when there is only one.
//!
//! This is the **third** defect in this file, after the 0-based/1-based link
//! confusion and printing `equation_names()` where the animation renders
//! `equation_texts()`. All three shared one shape: a tool built so an author
//! would not have to guess, confidently supplying a wrong answer.

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

    // **A real compile, because the frames must be the compile's.**
    //
    // The committed trace (`docs/specimen-notebook/<Model>/trace/structural.json`)
    // still supplies the incidence matrix's names and shape — it is generated and
    // correct by construction — but it holds no frames, because frames are a record
    // of an *execution* and a trace file is a record of a *result*. That distinction
    // is the whole reason the capture scopes exist.
    let specimen = PathBuf::from(format!(
        "{}/specimens/{model}.mo",
        env!("CARGO_MANIFEST_DIR")
    ));
    let libs = vec![PathBuf::from(format!(
        "{}/vendor/msl",
        env!("CARGO_MANIFEST_DIR")
    ))];
    let compiled = match hrw::worker::compile_specimen(&specimen, libs) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{model}: compile failed: {e}");
            eprintln!("(a specimen that does not compile has no matching to animate)");
            std::process::exit(1);
        }
    };
    let hrw::worker::FromWorker::Compiled {
        stages,
        matching_frames,
        ..
    } = compiled
    else {
        eprintln!("{model}: the worker returned something other than a compiled result");
        std::process::exit(1);
    };

    let Some(report) = stages.structural.value.as_ref() else {
        eprintln!("{model}: structural analysis produced no report");
        std::process::exit(1);
    };
    let Some(mat) = IncidenceMatrix::from_report(report) else {
        eprintln!("{model}: the structural report carries no incidence matrix");
        eprintln!("(a model that fails before structural analysis has no matching to animate)");
        std::process::exit(1);
    };

    // **No fallback.** If the capture is empty this prints nothing and says so.
    // Re-deriving here would resurrect precisely the defect this rewrite removed,
    // and `from_incidence` is now `#[cfg(test)]` so it is not reachable anyway.
    let Some(anim) = MatchingAnimation::from_captured_frames(&mat, &matching_frames) else {
        eprintln!("{model}: the compile recorded no matching frames for this system");
        eprintln!("(nothing to number \u{2014} a lab cannot link to a frame that does not exist)");
        std::process::exit(1);
    };

    // **`equation_texts`, not `equation_names` — what the viewer actually reads.**
    //
    // The animation stores `mat.equation_texts()` and `step_description` renders
    // those, so equation 0 of `ProportionalLoop` is labelled
    // `error - (reference - measurement)`. This tool printed `equation_names()`
    // instead — `f_x[0] (top-level model equation)` — so a lab author quoting it
    // wrote an expectation naming a string **that never appears on screen**, and the
    // walk would fail on a stop where nothing was actually wrong.
    //
    // Found 2026-08-03 while auditing `docs/fixture-labs/matching.md` against the
    // strings the animation renders.
    let eqs = mat.equation_texts();
    let vars = mat.unknown_names();
    let name = |v: &[String], i: usize| v.get(i).cloned().unwrap_or_else(|| format!("#{i}"));

    println!(
        "{model}: {} frames (captured during this compile)",
        anim.steps().len()
    );
    println!("  link form:  hrw://stage/Structural/MatchingAnim/frame/<n>");
    println!();

    let mut shown = 0usize;
    for (n, step) in anim.steps().iter().enumerate() {
        // `var` is what a lab points at; `eq` is which equation was trying.
        let (var, line) = match step {
            // The opening frame names no variable, because the search has not
            // reached one — it is the system before any of this.
            MatchingStep::Start {
                n_equations,
                n_unknowns,
            } => (
                None,
                format!("start: {n_equations} equations, {n_unknowns} unknowns, nothing matched"),
            ),
            MatchingStep::TryEquation(e) => (None, format!("try equation {}", name(eqs, *e))),
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
            MatchingStep::EquationFailed(e) => (None, format!("equation FAILED {}", name(eqs, *e))),
        };

        if let Some(f) = &filter {
            // Substring, because a lab author knows `der(w)` and not its index.
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
        // resolves fine, shows the wrong step, and no link checker can see.
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
    println!(
        "The links are 1-based, matching the counter on screen. Copy the link, not the number."
    );
}
