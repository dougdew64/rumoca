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
