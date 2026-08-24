//! **Run the pre-commit sequence in the one order that works.**
//!
//! ```text
//! cargo run -p hrw --example gate            # decides FAST or FULL from the diff
//! cargo run -p hrw --example gate -- --full  # force the full suite
//! cargo run -p hrw --example gate -- --fast  # force the cheap suite
//! ```
//!
//! # Why this exists, measured rather than assumed
//!
//! `CLAUDE.md` states the order — **fmt, then generate, then lint, then test** — and
//! explains what each mis-ordering costs. On 2026-08-23 Claude got it wrong **six
//! times in one session**, most of them by running `clippy` and the gate straight
//! after `cargo fmt`: formatting rewraps lines, `docs/architecture.md` carries module
//! line counts derived from the source, and `architecture_regions_are_current` then
//! fails **at the end of a 230-second run**.
//!
//! Before that session the same mistake had cost the gate four times. Ten instances
//! of one ordering error is not a memory problem, and the repository's own rule for
//! this case is that **a rule which keeps being got wrong wants a mechanism**, the way
//! `no_function_has_two_test_attributes` replaced remembering where to put a test.
//!
//! **It automates an order, not a judgement.** Every command here is one `CLAUDE.md`
//! already prescribes; none is new, and none is skipped on this tool's own opinion.
//!
//! # The two-gate rule for `crates/`, which is the other thing it enforces
//!
//! Touching a `crates/rumoca-*` file requires **both** `cargo fmt` and `cargo clippy`
//! for that crate. `fmt` was missing from the written rule until 2026-08-05 and cost
//! **82 unformatted hunks across four crates**, accumulated over a week in which
//! clippy was run every single time — *a rule that names one of two gates reads as
//! complete*. So the changed crates are detected from the diff and both are run,
//! rather than left to be remembered.
//!
//! They interact, which is why the order matters here too: formatting rewrapped
//! `reduce_constrained_dummy_derivatives_with_trace` from 99 lines to 102, over
//! `too_many_lines`'s threshold — so fmt-then-clippy is the only sequence that lints
//! the code in the shape it will ship in.
//!
//! # What it does not do
//!
//! It does not commit, and it does not push. It does not run the notebook content
//! check (`--features notebook-check`), which is deliberately outside both gates and
//! has its own triggers. And a green run here is not a claim that the change is
//! *right* — only that it is formatted, generated, linted and tested in the order
//! that makes those answers trustworthy.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

fn hrw_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn repo_root() -> PathBuf {
    hrw_dir()
        .parent()
        .expect("hrw/ lives inside the workspace")
        .to_path_buf()
}

/// One step of the sequence.
struct Step {
    label: &'static str,
    args: Vec<String>,
    /// Printed when it fails, so the reader learns what the step was protecting.
    why: &'static str,
}

fn step(label: &'static str, args: &[&str], why: &'static str) -> Step {
    Step {
        label,
        args: args.iter().map(|s| (*s).to_owned()).collect(),
        why,
    }
}

/// Is `hrw.exe` held open by a running HRW?
///
/// The same question `check_machine` asks, and asked again here because it is the
/// difference between a 230-second run and a 230-second run that could never have
/// passed. Opening the file for write without truncating tests the real failure —
/// whether cargo can replace the binary — rather than a proxy for it.
fn binary_is_locked(exe: &Path) -> bool {
    if !exe.exists() {
        return false;
    }
    std::fs::OpenOptions::new().write(true).open(exe).is_err()
}

/// Repository-relative paths of everything the working tree has changed.
///
/// `git status --porcelain` rather than `git diff --cached`: the two-gate rule is
/// about the code about to be committed, and the working tree is what a session
/// actually edits. Column 3 onward is the path in git's own spelling, which uses
/// forward slashes on every platform.
fn changed_paths(repo: &Path) -> Vec<String> {
    let Ok(out) = Command::new("git")
        .current_dir(repo)
        .args(["status", "--porcelain"])
        .output()
    else {
        return Vec::new();
    };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| l.get(3..))
        .map(|p| p.trim_matches('"').to_owned())
        .collect()
}

fn main() {
    let repo = repo_root();
    let flags: Vec<String> = std::env::args().skip(1).collect();
    let forced_fast = flags.iter().any(|f| f == "--fast");
    let forced_full = flags.iter().any(|f| f == "--full");

    let exe = repo.join("target/debug/hrw.exe");
    if binary_is_locked(&exe) {
        eprintln!(
            "HRW is running, so the gate cannot relink hrw.exe.\n\
             Close it and re-run \u{2014} after a clippy --all-targets that failure is \
             permanent, not transient."
        );
        std::process::exit(1);
    }

    let changed = changed_paths(&repo);
    let refs: Vec<&str> = changed.iter().map(String::as_str).collect();
    let crates = hrw::gate_policy::touched_rumoca_crates(refs.iter().copied());
    let detected_full = hrw::gate_policy::needs_full_gate(refs.iter().copied());
    let full = if forced_fast {
        false
    } else {
        forced_full || detected_full
    };
    if forced_fast && detected_full {
        eprintln!(
            "note: --fast was given, but the working tree touches src/, examples/, \
             crates/ or a Cargo.toml. That is what the FULL gate is for."
        );
    }

    let mut steps = vec![step(
        "fmt hrw",
        &["fmt", "-p", "hrw"],
        "formatting rewraps lines, and the generated docs measure the source",
    )];

    // The two-gate rule, for whichever Rumoca crates this change touched.
    for name in &crates {
        steps.push(step(
            Box::leak(format!("fmt {name}").into_boxed_str()),
            &["fmt", "-p", name.as_str()],
            "upstream CI gates on `cargo fmt --all -- --check`",
        ));
    }

    steps.push(step(
        "gen_architecture",
        &["run", "-q", "-p", "hrw", "--example", "gen_architecture"],
        "module sizes and App field groups are derived from the formatted source",
    ));
    steps.push(step(
        "gen_tour_catalogue",
        &["run", "-q", "-p", "hrw", "--example", "gen_tour_catalogue"],
        "any `##` heading edit changes a tour's stop list",
    ));
    steps.push(step(
        "gen_matching_reference",
        &[
            "run",
            "-q",
            "-p",
            "hrw",
            "--example",
            "gen_matching_reference",
        ],
        "emit lines and anchors are read from matching.rs as compiled",
    ));

    for name in &crates {
        steps.push(step(
            Box::leak(format!("clippy {name}").into_boxed_str()),
            &["clippy", "-p", name.as_str(), "--all-targets"],
            "the Rumoca crates are clippy-clean and [workspace.lints] denies",
        ));
    }

    steps.push(step(
        "clippy hrw",
        &["clippy", "-p", "hrw", "--all-targets"],
        "--all-targets covers the bin, which `cargo test` never builds",
    ));

    // **The notebook check, when the change can move what HRW reports.**
    //
    // Placed BEFORE the gate deliberately. It is the slower of the two (~109 s against
    // ~250 s) and it answers a different question — *did this change what Rumoca
    // produces* — which the gate cannot, because the gate was green before the change
    // too. Running it first means a fidelity drift is reported in two minutes rather
    // than six, and a drift makes the gate's verdict uninteresting anyway.
    //
    // It has its own feature because it must give each specimen a **fresh**
    // `WorkerState`: against the shared worker it is order-dependent, passing alone and
    // failing in company.
    if hrw::gate_policy::touches_the_compile_path(refs.iter().copied()) {
        steps.push(step(
            "notebook (fidelity)",
            &[
                "test",
                "-p",
                "hrw",
                "--lib",
                "--features",
                "notebook-check",
                "--",
                "--test-threads=1",
                "the_committed_notebook",
            ],
            "the gate cannot tell you the compiler's OUTPUT changed \u{2014} it was \
             green before the change too",
        ));
    }

    if full {
        steps.push(step(
            "gate (full)",
            &[
                "test",
                "-p",
                "hrw",
                "--lib",
                "--test",
                "msl_resolve",
                "--features",
                "slow-tests",
                "--",
                "--test-threads=1",
            ],
            "the slow-gated checks are the ones a src/ change can break",
        ));
    } else {
        steps.push(step(
            "gate (fast)",
            &["test", "-p", "hrw", "--lib", "--", "--test-threads=1"],
            "the doc and tour checkers are what a docs-only change can break",
        ));
    }

    println!(
        "HRW gate  --  {} ({} selected{})",
        repo.display(),
        if full { "FULL" } else { "FAST" },
        if forced_fast || forced_full {
            ", forced"
        } else {
            " from the working tree"
        },
    );
    println!();

    let started = Instant::now();
    for s in &steps {
        let at = Instant::now();
        print!("  {:<24}", s.label);
        let _ = std::io::Write::flush(&mut std::io::stdout());

        let status = Command::new("cargo")
            .current_dir(&repo)
            .args(&s.args)
            .status();

        let secs = at.elapsed().as_secs_f32();
        match status {
            Ok(st) if st.success() => println!(" ok    {secs:>6.1}s"),
            Ok(_) => {
                println!(" FAIL  {secs:>6.1}s");
                eprintln!(
                    "\n`cargo {}` failed.\n  why this step is here: {}\n\n\
                     Nothing after it ran, so the tree is formatted and generated only \
                     as far as this point.",
                    s.args.join(" "),
                    s.why,
                );
                std::process::exit(1);
            }
            Err(e) => {
                println!(" ERROR {secs:>6.1}s");
                eprintln!("\ncould not run `cargo {}`: {e}", s.args.join(" "));
                std::process::exit(1);
            }
        }
    }

    println!();
    println!(
        "Green in {:.1}s. Ready to commit.",
        started.elapsed().as_secs_f32()
    );
}
