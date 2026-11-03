//! **Why `upstream-issues.md` #1 was withdrawn** — the symptom is real, and Rumoca did
//! not cause it.
//!
//! ```text
//! cargo run -p hrw --example repro_stale_resolve
//! ```
//!
//! # What it demonstrates
//!
//! A consumer that leaves each specimen's document in the `Session` gets a later, good
//! model reported with an earlier, broken one's error. That reads as a stale cache and
//! is not one: **a `Session` resolves every document it holds**, so the broken file was
//! still being resolved and still being reported. Correctly.
//!
//! Drop the previous document first — one line, which HRW gained on 2026-08-21 — and it
//! is clean in every configuration tried: both resolve entry points, with two three-line
//! models and with the whole MSL plus the specimens the issue named.
//!
//! # Why it was written this way
//!
//! Because it was written to *file* the bug, before the bug turned out not to exist.
//! `docs/upstream-strategy.md` says to order deliverables by their **cost to accept**, so
//! it uses no HRW types, no test harness, and — in the minimal arm — no MSL, to be
//! liftable into the Rumoca repository unchanged. That property is why it could answer
//! the question it was built to beg.
//!
//! **It is kept as the evidence for the withdrawal**, and it would catch a recurrence:
//! the accumulating case reproducing is the expected result, and the run warns if that
//! ever stops being true.

use rumoca_compile::{Session, SessionConfig};

/// Resolves cleanly. Nothing here refers to anything outside itself.
const GOOD: &str = "model Good\n  Real x;\nequation\n  x = 1.0;\nend Good;\n";

/// Fails to resolve: `missingThing` is declared nowhere.
const BAD: &str = "model Bad\n  Real y;\nequation\n  y = missingThing;\nend Bad;\n";

/// One "compile" as a consumer performs it: drop the previous document, register this
/// one, resolve. This is the shape HRW uses and the shape an IDE would.
fn compile(
    session: &mut Session,
    drop_uri: Option<&str>,
    uri: &str,
    src: &str,
    strict: bool,
) -> Result<(), String> {
    if let Some(prev) = drop_uri {
        session.remove_document(prev);
    }
    session.remove_document(uri);
    session.update_document(uri, src);
    // **Two entry points, and a consumer picks one.** HRW's compile path calls
    // `strict_compile_resolved`; the issue write-up said `resolved`. They run different
    // builders -- `build_resolved_for_strict_compile_with_diagnostics` against
    // `build_resolved` -- so which one a reproduction uses is not a detail.
    if strict {
        // **`Ok` does not mean "resolved" here.** `StrictCompileRecovery` recovers from
        // errors and hands them back *beside* the tree, so a consumer that checks only
        // `Result` sees success on a model that did not resolve. HRW does not make that
        // mistake — `resolve_diagnostics_indicate_failure` inspects the severities — and
        // an earlier draft of this reproduction did, which made its whole strict arm
        // measure nothing while looking like a clean negative.
        match session.strict_compile_resolved() {
            Ok((_, diags)) => {
                let errors: Vec<String> = diags
                    .iter()
                    .filter(|d| d.severity == rumoca_core::DiagnosticSeverity::Error)
                    .map(|d| d.message.clone())
                    .collect();
                if errors.is_empty() {
                    Ok(())
                } else {
                    Err(format!("Resolve errors: {}", errors.join("; ")))
                }
            }
            Err(e) => Err(format!("{e:#}")),
        }
    } else {
        session.resolved().map(|_| ()).map_err(|e| format!("{e:#}"))
    }
}

fn main() {
    // **The pre-2026-08-21 consumer shape, tested first.** HRW gained
    // `remove_document(previous)` on that date; before it, the session accumulated one
    // document per specimen ever compiled. Every variant below INCLUDES that removal,
    // and none of them reproduces — so the obvious next question is whether the removal
    // is what fixed it, which would mean the defect was never in Rumoca's cache but in
    // a consumer leaving documents behind.
    println!("=== consumer that does NOT drop the previous document (HRW before 2026-08-21) ===");
    let accumulating = run_accumulating();

    // **The accumulating case reproducing is the EXPECTED result, not a failure.** It
    // shows the symptom is explained by the consumer keeping the document, so it must
    // not set the exit code. What would be alarming is the opposite: if it stopped
    // reproducing, the explanation below would no longer be demonstrated.
    if !accumulating {
        println!(
            "\nWARNING: the accumulating shape no longer reproduces either, so this \
             example no longer demonstrates why upstream-issues.md #1 was withdrawn."
        );
    }

    let mut reproduced = false;
    for strict in [false, true] {
        let entry = if strict {
            "strict_compile_resolved"
        } else {
            "resolved"
        };
        println!("\n=== entry point: {entry}() ===");
        println!("--- minimal: two tiny models, no libraries ---");
        reproduced |= run(strict);
        println!("--- the write-up's own sequence: MSL loaded, real specimens ---");
        reproduced |= run_at_msl_scale(strict);
    }
    if reproduced {
        std::process::exit(1);
    }
}

fn run(strict: bool) -> bool {
    let mut session = Session::new(SessionConfig::default());

    println!("1. resolve Good           ...");
    let first = compile(&mut session, None, "good.mo", GOOD, strict);
    println!("   {}", outcome(&first));

    println!("2. resolve Bad (expected to fail) ...");
    let second = compile(&mut session, Some("good.mo"), "bad.mo", BAD, strict);
    println!("   {}", outcome(&second));

    println!("3. resolve Good again     ...");
    let third = compile(&mut session, Some("bad.mo"), "good.mo", GOOD, strict);
    println!("   {}", outcome(&third));

    println!();
    match (&first, &second, &third) {
        (Ok(_), Err(_), Err(stale)) => {
            println!("REPRODUCED: step 3 failed after step 2's document was removed.");
            println!("  step 3 says: {stale}");
            if stale.contains("missingThing") {
                println!(
                    "  and it names `missingThing`, which appears ONLY in bad.mo \u{2014} the \
                     document removed before step 3 ran."
                );
            }
            return true;
        }
        (Ok(_), Err(_), Ok(_)) => {
            println!(
                "NOT REPRODUCED at this scale. Step 3 resolved cleanly, so the stale state \
                 needs something this reproduction does not have \u{2014} a loaded library, a \
                 larger resolved tree, or a different failure kind. That narrows the search."
            );
        }
        (Ok(_), Ok(_), _) => {
            println!(
                "INCONCLUSIVE: step 2 was expected to FAIL and did not, so the sequence never \
                 created the stale state. `missingThing` must be an unresolved reference for \
                 this to test anything. Not fatal: the MSL-scale variant below is the \
                 configuration HRW actually runs, and it is the one worth reaching."
            );
        }
        _ => {
            println!("INCONCLUSIVE: step 1 did not resolve, so the baseline is not clean.");
        }
    }
    false
}

fn outcome(r: &Result<(), String>) -> String {
    match r {
        Ok(()) => "resolved".to_owned(),
        Err(e) => format!("failed: {}", e.lines().next().unwrap_or(e)),
    }
}

/// The issue's **own stated reproduction**, at the scale it was written against: the
/// whole MSL loaded, two real specimens, `CapacitorLoop -> UndefinedRef ->
/// CapacitorLoop`.
///
/// # Why this is run beside the minimal case rather than instead of it
///
/// The minimal case establishes that the trigger needs *something more* than a bare
/// session. This establishes whether the write-up's own sequence still exhibits the
/// bug, and **through which entry point** — because the write-up says the consumer
/// calls `resolved()` while HRW's compile path calls `strict_compile_resolved()`.
///
/// **If a maintainer follows the write-up and it does not reproduce, the report costs
/// credibility rather than earning it**, which is the risk `docs/upstream-strategy.md`
/// exists to avoid. Verifying it before filing is cheaper than being wrong in public.
fn run_at_msl_scale(strict: bool) -> bool {
    use rumoca_compile::compile::SourceRootKind;
    use rumoca_compile::source_roots::{parse_source_root_with_cache, source_root_source_set_key};

    let base = concat!(env!("CARGO_MANIFEST_DIR"), "/vendor/msl");
    let roots = [
        format!("{base}/Modelica 4.1.0"),
        format!("{base}/ModelicaServices 4.1.0"),
        format!("{base}/Complex.mo"),
    ];

    let mut session = Session::new(SessionConfig::default());
    for root in &roots {
        let path = std::path::Path::new(root);
        match parse_source_root_with_cache(path) {
            Ok(parsed) => {
                let key = source_root_source_set_key(root);
                session.replace_parsed_source_set(
                    &key,
                    SourceRootKind::DurableExternal,
                    parsed.documents,
                    None,
                );
            }
            Err(e) => {
                println!("   could not load {root}: {e:#}");
                return false;
            }
        }
    }

    let read = |name: &str| {
        let p = format!("{}/specimens/{name}.mo", env!("CARGO_MANIFEST_DIR"));
        std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {p}: {e}"))
    };
    let (good_uri, bad_uri) = ("CapacitorLoop.mo", "UndefinedRef.mo");
    let (good, bad) = (read("CapacitorLoop"), read("UndefinedRef"));

    println!("1. resolve CapacitorLoop  ...");
    let first = compile(&mut session, None, good_uri, &good, strict);
    println!("   {}", outcome(&first));

    println!("2. resolve UndefinedRef (expected to fail) ...");
    let second = compile(&mut session, Some(good_uri), bad_uri, &bad, strict);
    println!("   {}", outcome(&second));

    println!("3. resolve CapacitorLoop again ...");
    let third = compile(&mut session, Some(bad_uri), good_uri, &good, strict);
    println!("   {}", outcome(&third));

    println!();
    match (&first, &second, &third) {
        (Ok(_), Err(_), Err(stale)) if stale.contains("missingGain") => {
            println!(
                "REPRODUCED, and step 3 names `missingGain` \u{2014} a reference that appears \
                 ONLY in UndefinedRef.mo, the document removed before step 3 ran."
            );
            true
        }
        (Ok(_), Err(_), Err(other)) => {
            println!("step 3 failed, but not with the other file's error: {other}");
            false
        }
        (Ok(_), Err(_), Ok(_)) => {
            println!(
                "NOT REPRODUCED through this entry point. The write-up's sequence resolves \
                 cleanly at step 3."
            );
            false
        }
        (_, Ok(_), _) => {
            println!("INCONCLUSIVE: step 2 did not fail, so no stale state was created.");
            false
        }
        _ => {
            println!("INCONCLUSIVE: step 1 did not resolve.");
            false
        }
    }
}

/// The consumer shape HRW had **before** 2026-08-21: each compile registers its own
/// document and never drops the previous one, so the session accumulates them.
///
/// If the stale error appears here and nowhere else, `upstream-issues.md` #1 is not an
/// upstream bug at all — it is a consumer that left a broken document in the session
/// and then asked why the session still reported it broken.
fn run_accumulating() -> bool {
    let mut session = Session::new(SessionConfig::default());

    println!("1. resolve Good  (good.mo added)  ...");
    let first = compile(&mut session, None, "good.mo", GOOD, false);
    println!("   {}", outcome(&first));

    println!("2. resolve Bad   (bad.mo added, good.mo LEFT IN)  ...");
    let second = compile(&mut session, None, "bad.mo", BAD, false);
    println!("   {}", outcome(&second));

    println!("3. resolve Good  (bad.mo LEFT IN)  ...");
    let third = compile(&mut session, None, "good.mo", GOOD, false);
    println!("   {}", outcome(&third));

    println!();
    match (&first, &second, &third) {
        (Ok(_), Err(_), Err(stale)) => {
            println!("REPRODUCED without any document removal at all.");
            println!("  step 3 says: {stale}");
            println!(
                "  bad.mo was never removed, so the session is correctly reporting a                  document it still holds. That is not a stale cache."
            );
            true
        }
        _ => {
            println!("not reproduced in the accumulating shape either.");
            false
        }
    }
}
