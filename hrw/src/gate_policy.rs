//! Which pre-commit gate a change needs, decided from the paths it touches.
//!
//! `CLAUDE.md` states the rule as a shell grep: a change touching `src/`,
//! `crates/`, `examples/` or a `Cargo.toml` needs the **full** gate (~230 s);
//! anything else is docs or tours and needs the **fast** one. *"A docs-only change
//! cannot regress compile-heavy behaviour, so paying 225 s for it is ritual rather
//! than evidence."*
//!
//! # Why this is a library function and not three lines in the runner
//!
//! `examples/gate.rs` is the only caller, and an example's code is **reachable by no
//! test** — `cargo test -p hrw --lib` does not run one. That matters here more than
//! it usually would, because of which way this decision fails: choosing FULL when
//! FAST would do costs four minutes and is obvious, while **choosing FAST when FULL
//! was needed is silent** and lets a `src/` change reach a commit gated only by the
//! cheap suite.
//!
//! A wrong negative that nothing checks is the error this repository treats as the
//! one nobody catches, so the rule lives where a test can reach it and the runner
//! keeps only the part that talks to git.

/// Does a change touching these paths need the full gate?
///
/// Paths are repository-relative and in git's spelling (forward slashes), which is
/// what `git status --porcelain` emits on every platform including Windows.
///
/// **Unknown means expensive.** An empty iterator returns `false` — there is nothing
/// to gate — but any path the caller could not classify should be passed through
/// rather than dropped, because the safe verdict is the costly one.
pub fn needs_full_gate<'a>(changed: impl IntoIterator<Item = &'a str>) -> bool {
    changed.into_iter().any(|p| {
        p.starts_with("hrw/src/")
            || p.starts_with("hrw/examples/")
            || p.starts_with("crates/")
            || p.ends_with("Cargo.toml")
    })
}

/// Does a docs-only change touch a tour region that is verified against a **real
/// compile**?
///
/// # Why a third verdict, when FAST and FULL had covered everything
///
/// Doug, 2026-08-31: *"every time that we are forced to perform a full gate simply because
/// I've asked a question or offered an opinion about tour content… it's time for a pause on
/// tour content improvement to focus on eliminating tour friction."*
///
/// A tour's `<!-- pane-groups -->`, `pane-origins` and `pane-frames` tables are checked by
/// **slow-gated** tests that compile a specimen. Editing one touches only `docs/`, so
/// [`needs_full_gate`] says FAST — correctly, since no `src/` file moved — and the FAST
/// suite then **cannot see the change at all**, because those tests are gated off. The
/// standing advice was therefore "run FULL", which is right about needing a compile and
/// wrong about needing **910 tests**.
///
/// **Measured 2026-08-31: the tour-relevant slow tests cost 11.1 s** (median of 3, spread
/// 2.5 %, via `examples/measure`), against ~101 s for FULL. The binary split was the
/// problem, not either of its halves.
///
/// # What this is NOT
///
/// **Not a cheaper FULL.** It runs the fast suite plus the slow tests whose names match
/// `doc_citations` and `tour` — enough to verify a tour's numbers against a compile, and
/// nothing about `worker.rs`, simulation or the corpus. A change touching `src/` still
/// needs FULL, which is why this is only ever consulted when [`needs_full_gate`] is false.
pub fn touches_a_verified_tour_region<'a>(changed: impl IntoIterator<Item = &'a str>) -> bool {
    changed
        .into_iter()
        .any(|p| p.starts_with("hrw/docs/fixture-tours/") && p.ends_with(".md"))
}

/// Does a change touch the path that decides **what HRW reports Rumoca did**?
///
/// # Why this needs its own question
///
/// The gate answers *"did anything break"*. It cannot answer *"did this change what
/// Rumoca produces"*, **because the gate was green before the change too** — and on
/// 2026-08-04 a corpus-scale fidelity programme reported 2,614 models green with zero
/// violations while the observatory's log and UI carried fictions.
///
/// `CLAUDE.md` already prescribes the notebook content check for any change to the
/// compile or library-loading path: it compares every specimen's committed per-stage IR
/// against a fresh compile, so drift is visible rather than merely possible. It costs
/// about 109 s and, until 2026-08-24, **it was a rule to remember rather than a step
/// that runs.**
///
/// # Why the answer is a whole file rather than a line range
///
/// `compile_target` is 1,085 lines and moves. A range would rot on the next edit and
/// fail *open* — silently answering "not the compile path" for a change that is. The
/// whole of `worker.rs` is the honest over-approximation: everything in it either is
/// the compile path, feeds it, or reads what it produced, and the cost of a false
/// positive is 109 s while the cost of a false negative is undetected drift.
pub fn touches_the_compile_path<'a>(changed: impl IntoIterator<Item = &'a str>) -> bool {
    changed
        .into_iter()
        .any(|p| p.starts_with("hrw/src/worker.rs") || p.starts_with("crates/"))
}

/// The `crates/<name>/…` packages a change touches, deduplicated.
///
/// Each needs **both** `cargo fmt` and `cargo clippy` before it is committed. `fmt`
/// was missing from that rule until 2026-08-05 and cost 82 unformatted hunks across
/// four crates, accumulated over a week in which clippy was run every time — so the
/// list is derived rather than remembered.
pub fn touched_rumoca_crates<'a>(changed: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut out: Vec<String> = changed
        .into_iter()
        .filter_map(|p| p.strip_prefix("crates/"))
        .filter_map(|rest| rest.split('/').next())
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect();
    out.sort();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The costly verdict is the safe one, and each trigger is checked
    /// separately.**
    ///
    /// One assertion per path class rather than one mixed list: a single list would
    /// pass on any one trigger working, which is how a rule ends up covering less
    /// than it claims.
    #[test]
    fn every_full_gate_trigger_fires_on_its_own() {
        assert!(needs_full_gate(["hrw/src/app.rs"]));
        assert!(needs_full_gate(["hrw/examples/gate.rs"]));
        assert!(needs_full_gate([
            "crates/rumoca-phase-structural/src/matching.rs"
        ]));
        assert!(needs_full_gate(["hrw/Cargo.toml"]));
        assert!(needs_full_gate(["Cargo.toml"]));
    }

    /// Docs, tours and notebooks are the fast case — the one the rule exists to make
    /// cheap, since most commits in a walking session are exactly this.
    #[test]
    fn documents_and_tours_do_not_need_the_full_gate() {
        assert!(!needs_full_gate(["hrw/docs/fixture-tours/matching.md"]));
        assert!(!needs_full_gate(["hrw/CLAUDE.md", "hrw/DECISIONS.md"]));
        assert!(!needs_full_gate([
            "hrw/docs/specimen-notebook/RcCircuit/purpose.md"
        ]));
        assert!(!needs_full_gate([]), "nothing changed, nothing to gate");
    }

    /// **Raising a reading-path budget is a prose commit, and gates like one.**
    ///
    /// This is the whole return on moving those numbers out of `doc_citations.rs` on
    /// 2026-08-31, so it is pinned rather than assumed. Doug: *"It seems that we trigger a
    /// lot of full test runs because of that one file."* While the budgets were `const`s in
    /// `src/`, a prose commit that pushed a document over its limit had to raise one — and
    /// that single edited number turned a ~6 s gate into a ~170 s one. Two commits pushed
    /// that day were exactly this shape.
    ///
    /// **It changes no verdict that was ever right.** The budgets are data about documents;
    /// they move when documents move. Editing a *checker* in `doc_citations.rs` still
    /// means FULL, as the case below requires — which is not a nicety, since four tests in
    /// that file are slow-gated and guard `connect-expansion`'s tables against a real
    /// compile.
    #[test]
    fn a_budget_bump_beside_prose_stays_fast_but_a_checker_edit_does_not() {
        assert!(!needs_full_gate([
            "hrw/docs/reading-budgets.txt",
            "hrw/docs/fixture-tours/connect-expansion.md",
            "hrw/CLAUDE.md",
        ]));
        assert!(
            needs_full_gate(["hrw/docs/reading-budgets.txt", "hrw/src/doc_citations.rs"]),
            "editing the checker itself is still a src/ change",
        );
    }

    /// **A tour edit selects TOUR: not FAST, which cannot see it, and not FULL.**
    ///
    /// The three-way verdict is the whole point, so all three are asserted together —
    /// separate tests would each pass on one arm working, which is how a rule ends up
    /// covering less than it claims.
    #[test]
    fn a_tour_edit_selects_the_tour_gate_and_a_source_edit_still_selects_full() {
        let tour = "hrw/docs/fixture-tours/connect-expansion.md";
        assert!(!needs_full_gate([tour]), "a tour is not a src/ change");
        assert!(
            touches_a_verified_tour_region([tour]),
            "a tour's guarded tables need a compile the FAST suite gates off"
        );

        // Prose elsewhere in docs/ is genuinely FAST: nothing there is checked against a
        // compile, so paying 11 s for it would be the same ritual at a smaller price.
        assert!(!touches_a_verified_tour_region(["hrw/docs/ideas.md"]));
        assert!(!touches_a_verified_tour_region(["hrw/CLAUDE.md"]));

        // The data files that back the checkers are not tours, and neither is the
        // generated catalogue — regenerating it must not drag in a compile.
        assert!(!touches_a_verified_tour_region([
            "hrw/docs/fixture-tours/pinned-claims.txt"
        ]));

        // And FULL still wins outright: `tour` is only consulted when needs_full_gate is
        // false, but a caller that got that backwards would silently under-gate.
        assert!(needs_full_gate([tour, "hrw/src/worker.rs"]));
    }

    /// **One `src/` file among twenty documents still means FULL.**
    ///
    /// The mixed commit is the case that matters: a session that edits ten tours and
    /// one module must not be gated by the tours.
    #[test]
    fn one_source_file_among_documents_still_needs_the_full_gate() {
        assert!(needs_full_gate([
            "hrw/docs/fixture-tours/matching.md",
            "hrw/docs/ideas.md",
            "hrw/src/playback.rs",
            "hrw/docs/vision.md",
        ]));
    }

    /// A path that looks close but is not one of the triggers must not fire.
    ///
    /// `hrw/src/` is a prefix match, so a *document* whose path merely contains
    /// `src` must not be mistaken for source — and `Cargo.lock` is not `Cargo.toml`.
    #[test]
    fn a_near_miss_is_not_a_trigger() {
        assert!(!needs_full_gate(["hrw/docs/source-tooling-plan.md"]));
        assert!(!needs_full_gate(["hrw/vscode-extension/src/extension.ts"]));
        assert!(
            !needs_full_gate(["Cargo.lock"]),
            "the lock file moves on any dependency change and builds nothing by itself"
        );
    }

    /// **The compile path is recognised, and the over-approximation is deliberate.**
    ///
    /// A `crates/rumoca-*` change alters what the compiler itself does, and a
    /// `worker.rs` change alters what HRW reports it did — both are exactly the
    /// question the notebook check answers and the gate cannot.
    ///
    /// The last two assertions are the point of the shape: `app.rs` renders what the
    /// compile produced but cannot change it, and a tour is prose. Charging those 109 s
    /// would make the step feel like a tax and get it switched off, which is how a
    /// check that cries wolf dies.
    #[test]
    fn a_change_to_what_the_compiler_reports_asks_for_the_notebook_check() {
        assert!(touches_the_compile_path(["hrw/src/worker.rs"]));
        assert!(touches_the_compile_path([
            "crates/rumoca-phase-structural/src/matching.rs"
        ]));
        assert!(
            touches_the_compile_path(["hrw/docs/vision.md", "hrw/src/worker.rs"]),
            "one compile-path file among documents is still the compile path",
        );

        assert!(
            !touches_the_compile_path(["hrw/src/app.rs", "hrw/src/playback.rs"]),
            "rendering what a compile produced cannot change what it produced",
        );
        assert!(!touches_the_compile_path([
            "hrw/docs/fixture-tours/matching.md"
        ]));
    }

    #[test]
    fn touched_crates_are_named_once_each() {
        let changed = [
            "crates/rumoca-phase-structural/src/matching.rs",
            "crates/rumoca-phase-structural/src/tarjan.rs",
            "crates/rumoca-compile/src/lib.rs",
            "hrw/src/playback.rs",
            "hrw/docs/vision.md",
        ];
        assert_eq!(
            touched_rumoca_crates(changed),
            vec![
                "rumoca-compile".to_owned(),
                "rumoca-phase-structural".to_owned()
            ],
            "two files in one crate are one crate, and hrw/ is not under crates/",
        );
        assert!(touched_rumoca_crates(["hrw/src/app.rs"]).is_empty());
    }
}
