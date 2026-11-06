//! Verify an Answer's pointers before handing it to Doug.
//!
//! ```text
//! cargo run -q -p hrw --example check_answer
//! ```
//!
//! # Why a tool and not a test
//!
//! `.hrw-bridge/answer.md` is gitignored and holds one Answer at a time, so there is no
//! commit to gate and no versioned artifact to guard. What there is, is a moment — after
//! writing an Answer and before saying "read this" — and this runs there.
//!
//! # Why `.hrw-bridge/stages/`, not the notebook
//!
//! An Answer describes **what is on Doug's screen now**, so it is checked against the
//! files the pane is actually fed. The committed notebook trace is the right source of
//! truth for a *lab*, which is versioned, and `every_lab_node_link_lands_on_a_real_node`
//! uses it there.
//!
//! **The distinction is a bug that already happened.** On 2026-09-03, fixing a claim
//! about Solve lowering, the notebook was consulted instead of the bridge file. The two
//! agreed, so nothing broke — but a sibling artifact had been checked in place of the
//! screen, which is the habit that produced the defects being fixed.
//!
//! # Exit status
//!
//! Non-zero if any pointer is a defect. **Unjudged pointers are printed and do not
//! fail** — a stage with no file on disk means the specimen is not loaded, which is a
//! fact about the session rather than about the prose. They are counted out loud so a
//! clean run cannot mean "nothing was examined".

use std::path::{Path, PathBuf};

use hrw::answer_check::{self, Verdict};
use hrw::bridge;
use hrw::worker::StageKind;

fn main() -> std::process::ExitCode {
    let hrw = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let answer = hrw.join(".hrw-bridge/answer.md");
    let stages = PathBuf::from(bridge::STAGES_DIR);

    let Ok(text) = std::fs::read_to_string(&answer) else {
        println!(
            "no Answer at {} \u{2014} nothing to check.",
            answer.display()
        );
        return std::process::ExitCode::SUCCESS;
    };

    // **Which specimen the bridge files describe, because they do not say.**
    //
    // `load_stage` was handed the model and ignored it, so an Answer about BouncingBall
    // would have been resolved against whatever IR happened to be on disk — and reported
    // as fine. That is the confidently-wrong failure this tool exists to prevent,
    // committed by the tool itself. Found within minutes of it working: the test suite
    // compiles its own specimens and left two unrelated stage files behind.
    let session_model = std::fs::read_to_string(hrw.join(".hrw-bridge/diagnostics/session.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|d| d["app"]["model"].as_str().map(str::to_owned));

    let notebook = hrw.join("docs/specimen-notebook");
    let mut from_notebook = 0usize;
    let mut from_pane = 0usize;
    // **The IR that actually judged the pointers, which is what the advisory must scan.**
    // Scanning the bridge directory instead made the advisory worthless in exactly the
    // case the fallback exists for: with a stale pane, real node names like `VarRef` and
    // `initial_y` were reported as appearing nowhere.
    let mut used_ir: Vec<serde_json::Value> = Vec::new();

    let found = answer_check::check(&text, |model, stage| {
        // The pane's own input, but ONLY when the pane is showing this specimen.
        if session_model.as_deref() == Some(model)
            && let Some(ir) = load_stage(&stages, stage)
        {
            from_pane += 1;
            used_ir.push(ir.clone());
            return Some(ir);
        }
        // Otherwise the committed trace, which is correct by construction but is not
        // what is on screen. Counted separately and reported, never silently blended.
        let file = stage.stage_file_name()?;
        let at = notebook.join(model).join("trace").join(file);
        let ir: serde_json::Value = std::fs::read_to_string(at)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())?;
        from_notebook += 1;
        used_ir.push(ir.clone());
        Some(ir)
    });

    // The advisory scans whatever the pane currently holds — it is about the screen.
    let loaded: Vec<serde_json::Value> = StageKind::ALL
        .iter()
        .filter_map(|s| load_stage(&stages, *s))
        .collect();

    let mut defects = 0usize;
    let mut resolved = 0usize;
    let mut unjudged = 0usize;

    println!("Answer:  {}", answer.display());
    println!(
        "On screen: specimen {}, {} stage file(s) in the bridge",
        session_model.as_deref().unwrap_or("<unknown>"),
        loaded.len(),
    );
    println!(
        "Judged:  {from_pane} pointer(s) against the PANE, {from_notebook} against the \
         committed notebook\n"
    );
    if from_notebook > 0 {
        println!(
            "  NOTE: a notebook-judged pointer is verified against IR that is correct by\n\
             \x20 construction but is NOT what is on screen. Load the specimen in HRW and\n\
             \x20 re-run to check the screen itself.\n"
        );
    }

    for p in &found {
        let tag = match p.verdict {
            Verdict::Resolved => {
                resolved += 1;
                continue;
            }
            Verdict::AbsentAsExpected => {
                resolved += 1;
                continue;
            }
            Verdict::NoSuchNode => "DEFECT   no such node",
            Verdict::Malformed => "DEFECT   unparseable path",
            Verdict::UnexpectedlyPresent => "DEFECT   marked absent but resolves",
            // Resolves, and does nothing unless the tree happens to be the showing
            // sub-view. Name it: `stage/Flatten/Tree/node/...`.
            Verdict::SubViewUnstated => "DEFECT   sub-view not named",
            Verdict::StageUnavailable => "unjudged no IR for that stage on disk",
            Verdict::NoSpecimen => "unjudged no load link before it",
        };
        if p.verdict.is_defect() {
            defects += 1;
        } else {
            unjudged += 1;
        }
        println!(
            "  line {:>4}  {tag}\n            {}  (stage {})",
            p.line,
            p.path,
            p.stage.slug()
        );
    }

    println!(
        "\n{resolved} resolved, {defects} defect(s), {unjudged} unjudged, \
         {} pointer(s) examined",
        found.len()
    );

    // **Must-fire, and this is the assertion that matters.** A run that examined no
    // pointers is not a clean run; it is a broken one that looks identical. Every Answer
    // written since this tool existed has carried pointers, because pointing at the pane
    // is what an Answer is for.
    if found.is_empty() {
        println!(
            "\nNO POINTERS FOUND. Either this Answer cites nothing in HRW \u{2014} in which \
             case it is prose that did not need HRW \u{2014} or the extractor is broken."
        );
        return std::process::ExitCode::FAILURE;
    }

    // **The whole pane when the pane is this specimen's, else only the IR that judged.**
    //
    // `used_ir` alone was too narrow: it holds only the stages a `node` pointer reached, so
    // an Answer that says *"hover `f_x[0]` in the DAE tree"* — a load link plus prose,
    // which is what a tooltip-era Answer looks like — had the DAE outside the haystack, and
    // every real DAE key was reported as appearing nowhere.
    //
    // It was narrow for a reason, and the reason still holds when the pane is stale or
    // showing something else: scanning a different specimen's IR reported `VarRef` and
    // `initial_y` as absent. `from_pane > 0` is the discriminator, because a pointer only
    // resolves against the pane when the session's specimen matches.
    let scanned = if from_pane > 0 || used_ir.is_empty() {
        &loaded
    } else {
        &used_ir
    };
    let absent = answer_check::tokens_absent_from_every_stage(&text, scanned);
    if !absent.is_empty() {
        println!(
            "\nADVISORY \u{2014} backticked tokens in no loaded stage's IR. Expect false\n\
             positives (Modelica keywords, Rust names, file names); read, do not obey:"
        );
        for t in &absent {
            println!("  `{t}`");
        }
        println!("\n  This is the check that would have caught `Y[1]` and `Minus` on 2026-09-03.");
    }

    // **An Answer carries no bold, and nothing was checking that.**
    //
    // Doug ruled on 2026-09-01 that labs are plain prose — measured then at 98 bold spans in
    // one lab, and swept across all 22 from 667 to 153. `docs/fixture-labs/README.md` keeps
    // exactly three exceptions, and **not one of them can occur in an Answer**: `Predict.`
    // and `Expected:` are the station grammar, and the first bolded line is read by
    // `LabSource::blurb_of` to build the catalogue, which an Answer is not in. So the
    // correct count here is zero.
    //
    // He had to point it out again on 2026-09-04, at 24 spans. A ruling with no mechanism
    // is a ruling Claude re-breaks, and this is the sixth thing today that was true of the
    // labs and unchecked for the Answer.
    let bold = text.matches("**").count();
    if bold > 0 {
        println!(
            "\nBOLD: {} `**` marker(s), and an Answer's correct count is ZERO.\n  \
             The three exceptions in docs/fixture-labs/README.md are the station grammar\n  \
             and the catalogue blurb; an Answer has neither. Write plain prose \u{2014} the\n  \
             renderings, code fences and links already carry the emphasis.",
            bold
        );
        return std::process::ExitCode::FAILURE;
    }

    if defects > 0 {
        println!("\nFIX THE DEFECTS BEFORE HANDING THIS OVER.");
        return std::process::ExitCode::FAILURE;
    }
    std::process::ExitCode::SUCCESS
}

fn load_stage(dir: &Path, stage: StageKind) -> Option<serde_json::Value> {
    let file = stage.stage_file_name()?;
    let text = std::fs::read_to_string(dir.join(file)).ok()?;
    serde_json::from_str(&text).ok()
}
