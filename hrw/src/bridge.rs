//! The Claude bridge — the app's communication channel to Claude Code.
//!
//! ## Architecture: thin emitter, thick reasoner
//!
//! When the user captures a node (clicks in the tree inspector or a custom view)
//! and then asks a question in the Claude Code chat, this module writes a single
//! JSON **focus file** (`focus.json`) describing what the user is looking at:
//!
//! - Which specimen (`.mo` file)
//! - Which pipeline stage (Parse, Resolve, Typecheck, etc.)
//! - Which IR node (by key-path from the stage root)
//! - The node's source provenance (which Modelica source line it came from)
//! - A cross-stage diff (how the node differs between Parse and Resolve)
//! - A DefId resolution table (mapping numeric ids to human-readable names)
//!
//! The focus file carries **no answers** and embeds **no language model**. It is
//! a pure description of context. The reasoning happens entirely in the Claude
//! Code session, which reads the focus file along with the specimen source, the
//! staged IR files, the Rumoca phase code, and Doug's `docs/compiler-phases`.
//! This "thin emitter, thick reasoner" split is documented in DECISIONS.md.
//!
//! ## The JSON file protocol
//!
//! The bridge uses the filesystem as the communication channel:
//!
//! 1. **Focus file** (`.hrw-bridge/focus.json`): written on each capture. Contains
//!    the `instructions` (self-describing), `seq` (monotonic counter), `request`
//!    (a verb a gesture *declared* — only "debug-where-set" today — or null, which is
//!    the ordinary case: pointing declares a noun, and the verb is in the chat), `kind`
//!    (node/stage/specimen), and the node/provenance/cross-stage data.
//!
//! 2. **Stage files** (`.hrw-bridge/stages/<name>.json`): one file per pipeline
//!    stage's full IR, rewritten once per compile. Claude can diff any two stages
//!    by reading two files (e.g., `instantiate.json` vs `typecheck.json`).
//!
//! 3. **Question file** (`.hrw-bridge/question.json`) — the question Doug typed into
//!    the Question Bar, written with the context it is about; see [`write_question`].
//!
//!    *(This slot was the lab file, `.hrw-bridge/answer.md`, the one channel that ran
//!    the other way: Claude wrote an Answer and HRW rendered it. It was removed on
//!    2026-11-07 under charter Decision 19 — HRW causes questions, and answers live
//!    on the platform. `docs/architecture.md` lists the channels as they are now.)*
//!
//! The `.hrw-bridge/` directory is gitignored. The paths are repo-relative
//! (via `CARGO_MANIFEST_DIR`) so they are stable across Claude Code sessions.
//!
//! ## Span-ascent (source provenance)
//!
//! Rumoca IR nodes carry source provenance (`location` or `span` fields with
//! byte offsets into the Modelica source). However, leaf nodes you typically
//! click (e.g., a bare `"name": "flange_a"`) usually have **no provenance of
//! their own** — the nearest `location`/`span` lives on an ancestor node.
//!
//! So the bridge runs **up** the tree from the clicked node to the root,
//! looking for the tightest enclosing provenance:
//! - `location` (preferred): `rumoca_core::Location` with byte offsets + `file_name`
//! - `span` (fallback): `rumoca_core::Span` with byte offsets + opaque `source` id
//!
//! Once found, the byte range is sliced out of the Modelica source file, and
//! the enclosing line(s) are included for context. This run is fully generic
//! (it pattern-matches on JSON structure, not Rumoca types), maintaining the
//! one-generic-tree rule.

use std::collections::BTreeMap;
use std::fmt::{self, Write as _};
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::worker::{DefInfo, StageKind};

/// Strip the `\\?\` extended-length prefix that `std::fs::canonicalize` adds on Windows.
#[cfg(windows)]
fn strip_windows_prefix(p: &Path) -> PathBuf {
    p.to_str()
        .and_then(|s| s.strip_prefix(r"\\?\"))
        .map_or_else(|| p.to_path_buf(), PathBuf::from)
}

/// Path to the bridge directory, resolved at compile time via `CARGO_MANIFEST_DIR`.
///
/// Using `CARGO_MANIFEST_DIR` (the directory containing `Cargo.toml`) makes
/// this path repo-relative and stable regardless of the working directory.
/// The directory is gitignored so focus files don't pollute version control.
///
/// # Under `cargo test` this is `.hrw-bridge/test-run/`, and every path below follows
///
/// **The test suite was overwriting the entire live bridge, and nothing said so.**
/// Measured 2026-11-06: one `cargo test` run rewrote `focus.json`, **all eleven
/// `stages/*.json`**, `answer.md`, `breakpoint-ack.json` and a scratch specimen — the
/// capture Doug had assembled, the IR files `CLAUDE.md` names as *"the file the pane is
/// actually fed"*, and the Answer artifact itself.
///
/// That is a trap laid exactly where it does the most damage. **The gate runs before
/// every commit**, so the sequence is ordinary: Doug captures a node, Claude runs the
/// gate, Doug asks *"what does this mean?"* — and the files Claude is told to reason
/// over now describe the suite. Nothing in them is false, which is why it reads as a
/// confident answer about the wrong thing: the most-recorded failure class in this
/// repository, with a mechanism that fired on every gate run.
///
/// **The fix is not discipline in each test — it is that a test cannot address the live
/// files at all.** Redirecting at the *definition* rather than at the writers makes it
/// exhaustive by construction: all ~70 call sites across `src/` follow without being
/// touched, and a writer nobody enumerated cannot escape. Tests still exercise the real
/// write path, one directory over. `tests_never_write_the_live_bridge_files` fails by
/// name if any of these stops being redirected.
///
/// This is `diagnostics::dir()`'s ruling applied to the other half of the bridge — that
/// one was fixed on 2026-09-02 after an out-of-memory abort whose only record was
/// `session.json`, and the same hazard sat unnoticed in `focus.json` and `stages/` for
/// two more months. **Examples are deliberately NOT redirected**: `capture_suite` and
/// the app itself must write the live files, and they are not compiled with `cfg(test)`.
#[cfg(not(test))]
pub const BRIDGE_DIR: &str = crate::manifest_path!(".hrw-bridge");
/// The bridge directory under `cargo test` — see the non-test [`BRIDGE_DIR`].
#[cfg(test)]
pub const BRIDGE_DIR: &str = crate::manifest_path!(".hrw-bridge", "test-run");

/// Path to the stage-files directory (one JSON file per pipeline stage).
///
/// These files are rewritten once per compile. Claude can diff any two stages
/// by reading two files — e.g., comparing `instantiate.json` to `typecheck.json`
/// shows what the instanced typecheck phase added (type_ids resolved, dimensions
/// evaluated). This avoids bloating the focus file with all stages' IR.
///
/// Redirected under `cargo test`; see [`BRIDGE_DIR`] for why.
#[cfg(not(test))]
pub const STAGES_DIR: &str = crate::manifest_path!(".hrw-bridge", "stages");
/// The stage-files directory under `cargo test` — see [`BRIDGE_DIR`].
#[cfg(test)]
pub const STAGES_DIR: &str = crate::manifest_path!(".hrw-bridge", "test-run", "stages");

/// HRW writes breakpoint requests here; the VS Code extension watches it.
///
/// `pub(crate)` to match [`BREAKPOINT_ACK_FILE`]: the shutdown-release test in
/// `app.rs` asserts on the request this path receives, and a test that rebuilt
/// the path itself could pass while the real one moved.
#[cfg(not(test))]
pub(crate) const BREAKPOINT_REQUEST_FILE: &str =
    crate::manifest_path!(".hrw-bridge", "breakpoint-request.json");
/// The breakpoint request under `cargo test` — see [`BRIDGE_DIR`].
#[cfg(test)]
pub(crate) const BREAKPOINT_REQUEST_FILE: &str =
    crate::manifest_path!(".hrw-bridge", "test-run", "breakpoint-request.json");
/// The extension writes this ack file after processing a request, confirming
/// the breakpoint is registered with LLDB. HRW polls for it before spawning
/// the algorithm thread (see `check_breakpoint_ack`).
#[cfg(not(test))]
pub(crate) const BREAKPOINT_ACK_FILE: &str =
    crate::manifest_path!(".hrw-bridge", "breakpoint-ack.json");
/// The breakpoint ack under `cargo test` — see [`BRIDGE_DIR`].
#[cfg(test)]
pub(crate) const BREAKPOINT_ACK_FILE: &str =
    crate::manifest_path!(".hrw-bridge", "test-run", "breakpoint-ack.json");

/// Scratch specimens written by Claude to answer a question (ideas #42).
///
/// **Why a second directory rather than `specimens/`.** The curated corpus has
/// properties worth protecting: portable Modelica subset, a `// purpose:` line on every
/// file, and the intent that each round-trips through System Modeler. Disposable probes
/// — "here is the smallest model that shows the thing you asked about" — have none of
/// those and would quietly degrade it. Doug offered `specimens/` for repurposing on
/// 2026-07-29 and Claude recommended against; this is that split.
///
/// Living under the gitignored bridge directory makes them **ephemeral by
/// construction**, the same rule as `lab.md`: what persists is the *question*, in
/// `docs/question-ledger.md`. A probe worth keeping gets promoted into `specimens/`
/// deliberately, with a `// purpose:` line — which is the moment it stops being a probe.
#[cfg(not(test))]
pub const SCRATCH_SPECIMEN_DIR: &str = crate::manifest_path!(".hrw-bridge", "specimens");
/// Scratch specimens under `cargo test` — see [`BRIDGE_DIR`]. A test wrote
/// `ScratchProbe.mo` into the live directory on 2026-11-06, where it then showed up in
/// HRW's specimen list as though Claude had authored it to answer something.
#[cfg(test)]
pub const SCRATCH_SPECIMEN_DIR: &str =
    crate::manifest_path!(".hrw-bridge", "test-run", "specimens");

/// List the scratch specimens, sorted. Empty when none have been written — the common
/// case, and not an error.
pub fn scratch_specimens() -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(SCRATCH_SPECIMEN_DIR) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("mo"))
        .collect();
    found.sort();
    found
}

// **`LAB_FILE` (`.hrw-bridge/answer.md`) was here** — the HRW Answer, the one bridge file
// that flowed *into* HRW. Removed 2026-11-07 under charter Decision 19: answers are given on
// the platform (the chat, a VS Code tab, System Modeler, Wolfram Desktop), never in HRW. A
// stale `answer.md` left in a clone's bridge directory is now simply ignored.

/// A question Doug asked from the Question Bar, **written together with the context
/// that was live when he asked it** — see [`write_question`].
#[cfg(not(test))]
pub const QUESTION_FILE: &str = crate::manifest_path!(".hrw-bridge", "question.json");
/// The question file under `cargo test` — see [`BRIDGE_DIR`].
#[cfg(test)]
pub const QUESTION_FILE: &str = crate::manifest_path!(".hrw-bridge", "test-run", "question.json");

/// Fixture labs — kept, versioned, and executed by `fixture_lab_links_all_resolve`.
///
/// Hard-coded rather than configurable: the directory is part of the repository layout,
/// not a user preference, and one fewer setting is one fewer thing to be wrong.
pub const FIXTURE_LABS_DIR: &str = crate::manifest_path!("docs", "fixture-labs");

/// Notebooks belonging to fixture labs — versioned, because a fixture has expected
/// outcomes and a test that vanishes on a fresh checkout is not a test.
pub const FIXTURE_NOTEBOOKS_DIR: &str = crate::manifest_path!("docs", "fixture-labs", "notebooks");

/// Notebooks Claude writes to answer one question — ephemeral, like `lab.md`.
#[cfg(not(test))]
pub const SCRATCH_NOTEBOOKS_DIR: &str = crate::manifest_path!(".hrw-bridge", "notebooks");
/// Scratch notebooks under `cargo test` — see [`BRIDGE_DIR`].
#[cfg(test)]
pub const SCRATCH_NOTEBOOKS_DIR: &str =
    crate::manifest_path!(".hrw-bridge", "test-run", "notebooks");

/// The repository documents a lab may link to with `hrw://doc/<name>`.
pub const DOCS_DIR: &str = crate::manifest_path!("docs");

/// The workspace root — the tree `hrw://src/<path>` resolves against.
///
/// One level above this crate, so it covers both `crates/rumoca-*` and `hrw/` itself.
/// A lab grounded in Rumoca's code cites both.
#[must_use]
pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")))
        .to_path_buf()
}

/// Where a `hrw://src/<path>[#<symbol>]` link lands: a file, and a 1-based line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceTarget {
    /// The resolved file, always inside [`workspace_root`].
    pub path: PathBuf,
    /// The line the symbol was found on. `None` when the link named no symbol.
    pub line: Option<usize>,
}

/// Resolve `hrw://src/<path>[#<symbol>]` to a file and, if named, the symbol's line.
///
/// # Why a symbol rather than a line number
///
/// This is [`crate::matching_ledger::anchor_by_name`]'s decision applied to lab prose,
/// and `HrwLink::OpenLab`'s before it: **a number in prose rots silently when code
/// moves, while a name fails loudly.** The line is computed from the file at click time,
/// so a link that resolves is right by construction, and a symbol that has been renamed
/// away fails in `doc_citations::lab_source_links_resolve` rather than in front of Doug.
///
/// # What counts as finding a symbol
///
/// The first line whose code — comments stripped, so a doc comment mentioning the name
/// cannot win — contains one of `fn NAME`, `struct NAME`, `enum NAME`, `const NAME`,
/// `static NAME`, `trait NAME`, `type NAME`, `mod NAME`, or `NAME:` for a struct field.
/// Deliberately shallow: this locates a *definition a reader wants to look at*, and is
/// not a resolver — `identity-and-provenance.md` forbids substring matching deciding
/// **identity**, and nothing here decides identity. A miss is a finding, never a
/// fallback to line 1, because scrolling a reader to the top of a 1,745-line file while
/// claiming to show them `union` is exactly the quiet wrongness this repository bans.
#[must_use]
pub fn resolve_source(target: &str) -> Option<SourceTarget> {
    let (rel, symbol) = match target.split_once('#') {
        Some((p, s)) if !s.is_empty() => (p, Some(s)),
        Some((p, _)) => (p, None),
        None => (target, None),
    };
    if rel.is_empty() || rel.contains('\\') || rel.contains("..") {
        return None;
    }
    let root = workspace_root();
    let path = root.join(rel);
    let (real_root, real_path) = (root.canonicalize().ok()?, path.canonicalize().ok()?);
    if !real_path.starts_with(&real_root) || !real_path.is_file() {
        return None;
    }
    let Some(symbol) = symbol else {
        return Some(SourceTarget { path, line: None });
    };
    let text = fs::read_to_string(&path).ok()?;
    let line = line_defining(&text, symbol)?;
    Some(SourceTarget {
        path,
        line: Some(line),
    })
}

/// 1-based line where `symbol` appears to be defined, or `None`.
///
/// **Two shapes, because the first version only had one and the checker caught it
/// immediately.** Keyword-prefixed items (`fn union`, `struct UnionFind`) were covered;
/// **enum variants were not**, and `hrw://src/…/trace.rs#SetFormed` failed on the first
/// run of `fixture_labs_reference_files_that_exist` — which is the whole argument for
/// the checker, since the alternative was Doug clicking it.
fn line_defining(source: &str, symbol: &str) -> Option<usize> {
    let keywords = [
        "fn ", "struct ", "enum ", "const ", "static ", "trait ", "type ", "mod ",
    ];
    source
        .lines()
        .enumerate()
        .find(|(_, raw)| {
            // Comments stripped first: a doc comment naming the symbol must not win
            // over the definition it documents, and in this repository doc comments
            // routinely name the item several lines above it.
            let code = raw.split("//").next().unwrap_or("");
            if keywords
                .iter()
                .any(|kw| code.contains(&format!("{kw}{symbol}")))
            {
                return true;
            }
            // An enum variant or a struct field: the name opens the line, followed by
            // its payload, a comma, or a type. A *use* site cannot match, because there
            // the name is qualified (`ConnectionStep::SetFormed`) and so does not start
            // the line's code.
            code.trim_start().strip_prefix(symbol).is_some_and(|rest| {
                // `SetFormed {` leaves a SPACE before the brace, so the remainder is
                // trimmed before it is judged. Trimming here also keeps a longer name
                // from matching a shorter query — `SetFormed` against `Set` leaves
                // `Formed {`, which is neither empty nor punctuation.
                let rest = rest.trim_start();
                rest.is_empty() || rest.starts_with(['{', '(', ',', ':'])
            })
        })
        .map(|(i, _)| i + 1)
}

/// Resolve a `hrw://notebook/<name>` target to a file on disk.
///
/// Looks in the fixture directory first, then the scratch one, so a fixture lab keeps
/// working even when an ad hoc notebook of the same name exists.
///
/// **Rejects anything with a path separator or `..`.** A lab is authored by Claude and
/// versioned, but the verb hands a path to the operating system's file association — so
/// the set of things it can open stays a *file name in one of two known directories*,
/// rather than whatever a link happens to spell.
pub fn resolve_notebook(name: &str) -> Option<PathBuf> {
    if name.is_empty()
        || name.contains('/')
        || name.contains('\\')
        || name.contains("..")
        || !name.ends_with(".nb")
    {
        return None;
    }
    [FIXTURE_NOTEBOOKS_DIR, SCRATCH_NOTEBOOKS_DIR]
        .into_iter()
        .map(|dir| Path::new(dir).join(name))
        .find(|p| p.is_file())
}

/// Resolve `hrw://doc/<name>` to a document under `hrw/docs/`.
///
/// # The same defect as [`resolve_notebook`]'s, one file type later
///
/// That verb exists because a plain markdown link to a `.nb` is handed to the **browser**,
/// which does nothing useful with it — Doug hit it on 2026-07-30. **It recurred on
/// 2026-08-31 with `.md`**: five labs linked `[upstream-issues.md](../upstream-issues.md)`,
/// and clicking one opened Chrome — *"instead of attempting to open the file in VS Code."*
/// **The July fix did not generalise because it was a verb, not a rule**: nothing stopped
/// the next relative link from being written, and
/// `doc_citations::no_lab_links_to_a_bare_file_path` is the half that does.
///
/// # What it refuses, and why a subdirectory is allowed here
///
/// A notebook is a bare file name in one of two flat directories. Docs nest —
/// `compiler-phases/the-chain-of-problems.md` is a legitimate target — so a `/` cannot
/// simply be banned. **`..` still is**, and the resolved path is checked to be inside
/// `docs/` afterwards, so a traversal that survives the textual check cannot escape.
pub fn resolve_doc(name: &str) -> Option<PathBuf> {
    if name.is_empty() || name.contains('\\') || name.contains("..") || !name.ends_with(".md") {
        return None;
    }
    let root = Path::new(DOCS_DIR);
    let path = root.join(name);
    // Belt and braces: refuse anything that does not land under `docs/` once resolved,
    // whatever the textual check let through.
    let (real_root, real_path) = (root.canonicalize().ok()?, path.canonicalize().ok()?);
    if !real_path.starts_with(&real_root) || !real_path.is_file() {
        return None;
    }
    Some(path)
}

/// List the fixture labs, sorted by file name.
///
/// A fixture lab is a **test** with a pass/fail criterion, and is kept. (It was once
/// contrasted here with the regenerated Answer, removed 2026-11-07.) See
/// `docs/fixture-labs/camera-aiming.md` for the shape.
/// **`README.md` is excluded**: the directory gained one on 2026-08-01 under the
/// two-audience convention (`DECISIONS.md`), and without this it would appear in
/// the lab picker as a lab called "README" whose stops do not exist. A README
/// describes the directory; it is not a member of it.
pub fn fixture_labs() -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(FIXTURE_LABS_DIR) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("md"))
        // **Not every `.md` in this directory is a lab.**
        //
        // `README.md` documents the directory. `CATALOGUE.md` is *generated for
        // Claude* — it exists so a question can be answered by citing a lab rather
        // than retelling one (`docs/ideas.md` #63) — and Doug found it sitting in his
        // picker on 2026-08-05, one row among fifteen, offering to be run.
        //
        // **This is the single definition of "is a lab file".** `lab::catalogue`
        // had its own copy of this filter, which is how the catalogue managed to
        // exclude itself while the picker did not. It now calls this function.
        .filter(|p| {
            !matches!(
                p.file_stem().and_then(|n| n.to_str()),
                Some("README") | Some("CATALOGUE")
            )
        })
        .collect();
    found.sort();
    found
}

/// Claude asks HRW to photograph itself here; [`SCREENSHOT_RESULT_FILE`] is the reply.
///
/// # Why a fourth channel, and what it is actually for
///
/// Everything HRW publishes about itself is *derived*: `focus.json` says which node
/// is captured, `session.json` says which links were followed, the stage files say
/// what the IR holds. **None of them can say what the window looked like**, and a
/// large part of HRW is painter-drawn — the incidence matrix, the spy plot, the
/// Tarjan graph, the four animations — which `Cargo.toml` already records as
/// *invisible to an accessibility-tree harness*. For those views Doug's eyes have
/// been the only instrument, and `docs/tech-debt.md` logs what that costs: nine of
/// his ten findings in the 08-16 ledger were *things visibly wrong on screen*.
///
/// This channel does not replace his judgement. It removes the **mechanical** half —
/// is the tab there, did the link land, is the pane empty — from the set of things
/// only he can check, which is the same argument that bought `egui_kittest`.
///
/// **This is not image-snapshot testing**, which `Cargo.toml` rules out and this does
/// not re-open: nothing is committed, no reference image exists, and no test asserts
/// on a pixel. The photograph is read *by Claude, once*, in the conversation that
/// asked for it.
///
/// # The request
///
/// ```json
/// {
///   "seq": 3,
///   "why": "does the Index Reduction tab row offer Matching?",
///   "follow": ["hrw://load/Drivetrain/IndexReduction/Summary"],
///   "settleMs": 600
/// }
/// ```
///
/// `seq` is required and must increase. HRW seeds its "already answered" mark from
/// the **result** file at startup, so a request left on disk is not re-honoured on
/// the next launch — which would otherwise navigate Doug's window somewhere no one
/// asked for while he was using it.
///
/// `follow` is a list of `hrw://` links, dispatched **through the same
/// `dispatch_hrw_link` a click goes through** — one per settle interval, exactly as
/// autoplay dispatches a beat. Reusing that path rather than reaching into the app's
/// fields is the point: a capture-and-inspect run that navigated by its own private
/// route would photograph a state no user gesture can produce, and then testify
/// about it.
#[cfg(not(test))]
pub const SCREENSHOT_REQUEST_FILE: &str =
    crate::manifest_path!(".hrw-bridge", "screenshot-request.json");
/// The screenshot request under `cargo test` — see [`BRIDGE_DIR`].
#[cfg(test)]
pub const SCREENSHOT_REQUEST_FILE: &str =
    crate::manifest_path!(".hrw-bridge", "test-run", "screenshot-request.json");

/// HRW's reply to a [`SCREENSHOT_REQUEST_FILE`]: where the PNG landed, how big it
/// is, what was on screen, and **what went wrong** when nothing landed.
///
/// # Every request gets one of these, including the ones that fail
///
/// A capture has four ways to produce no image — a malformed request, a link that
/// does not parse, a backend that never delivers the pixels, a failed write — and in
/// all four the *window still looks fine*. So silence here is the worst possible
/// outcome: Claude would find a stale PNG from the previous request, read it as this
/// one's answer, and testify about a screen that was never photographed. That is the
/// wrong-negative shape this repository treats as the error nobody catches.
///
/// Hence: `seq` and `writtenAtMs` on every reply so a stale one is *detectable* (the
/// rule `debug-state.json` already follows), `error` set whenever `image` is absent,
/// and `notice` carrying HRW's own on-screen message — because a refused link
/// ("no specimen loaded") leaves a perfectly healthy-looking window.
#[cfg(not(test))]
pub const SCREENSHOT_RESULT_FILE: &str =
    crate::manifest_path!(".hrw-bridge", "screenshot-result.json");
/// The screenshot reply under `cargo test` — see [`BRIDGE_DIR`].
#[cfg(test)]
pub const SCREENSHOT_RESULT_FILE: &str =
    crate::manifest_path!(".hrw-bridge", "test-run", "screenshot-result.json");

/// Where the PNGs land, one per honoured request, named by `seq`.
///
/// Gitignored with the rest of `.hrw-bridge/`, which is what makes "nothing is
/// committed" structural rather than a promise.
#[cfg(not(test))]
pub const SCREENSHOTS_DIR: &str = crate::manifest_path!(".hrw-bridge", "screenshots");
/// The screenshot directory under `cargo test` — see [`BRIDGE_DIR`].
#[cfg(test)]
pub const SCREENSHOTS_DIR: &str = crate::manifest_path!(".hrw-bridge", "test-run", "screenshots");

/// How long HRW lets the UI settle before shooting, when the request does not say.
///
/// Long enough for a dispatched link's effects to reach the screen — a stage switch
/// re-lays out, a sub-view reset runs on the following frame — and short enough that
/// a run of a dozen shots is not a coffee break. The compile is *not* what this
/// covers: a capture also waits for `compiling` to clear, which is seconds, not
/// milliseconds.
pub const SCREENSHOT_SETTLE_DEFAULT: std::time::Duration = std::time::Duration::from_millis(600);

/// How much of the window a shot keeps.
///
/// # Why cropping exists at all
///
/// The reader of these photographs is a language model with a context budget, and a
/// full HRW window is 3440x1369 of which the pane under test is the right-hand third.
/// A suite of a dozen shots spends most of that budget re-reading the same menu bar,
/// specimen list and lab prose.
///
/// **`Window` stays the default, deliberately.** The first defect this feature
/// confirmed was a *tab row* drawing tabs it should not — chrome, not pane content —
/// and a crop that had been on by default would have cut it out. A shot asks for less
/// than the window only when it knows what it is looking at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShotRegion {
    /// The whole viewport.
    #[default]
    Window,
    /// Just the central panel: the stage tab row, the sub-view row and the pane.
    Pane,
}

impl ShotRegion {
    /// The spelling used in both the request and the reply, so what was asked for and
    /// what was done are written in one vocabulary.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Window => "window",
            Self::Pane => "pane",
        }
    }
}

/// A request to photograph the window, as [`SCREENSHOT_REQUEST_FILE`] carries it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenshotRequest {
    /// Monotonic counter. Echoed in the result so a reply can be matched to its
    /// request, and compared against the last honoured one so a request is acted
    /// on exactly once.
    pub seq: u64,
    /// Free text: what this shot is meant to show. Echoed, never interpreted —
    /// it is there so a result file read later still says what the question was.
    pub why: Option<String>,
    /// `hrw://` links to dispatch before shooting, in order.
    pub follow: Vec<String>,
    /// How long to let the UI settle after each dispatch, and before the shot.
    pub settle: std::time::Duration,
    /// How much of the window to keep. See [`ShotRegion`].
    pub region: ShotRegion,
}

/// Read the screenshot request, with the mtime it was read at.
///
/// `None` means **no request file**, which is the overwhelmingly common case and not
/// an error — HRW runs for hours without one.
///
/// `Some(Err(why))` means a file that is not a request: unreadable, not JSON, or
/// missing `seq`. It is returned rather than dropped so the caller can answer it with
/// an `error` result. A malformed request that HRW ignored in silence would be
/// indistinguishable from one it never saw, and the author of the request is a
/// process that will go looking for a PNG.
pub fn read_screenshot_request()
-> Option<(Result<ScreenshotRequest, String>, std::time::SystemTime)> {
    let meta = fs::metadata(SCREENSHOT_REQUEST_FILE).ok()?;
    let mtime = meta.modified().ok()?;
    Some((parse_screenshot_request_at(SCREENSHOT_REQUEST_FILE), mtime))
}

/// The parsing half of [`read_screenshot_request`], against any path, so a test can
/// drive it without writing into the live bridge directory.
fn parse_screenshot_request_at(path: &str) -> Result<ScreenshotRequest, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("cannot read request: {e}"))?;
    parse_screenshot_request(&text)
}

/// Parse a screenshot request from the text of the request file.
///
/// **`seq` is required.** Defaulting it to zero or to "one more than last time" would
/// make the once-only rule depend on HRW's memory rather than on the request, and the
/// first thing that costs is a request re-honoured after a restart.
pub fn parse_screenshot_request(text: &str) -> Result<ScreenshotRequest, String> {
    let value: Value =
        serde_json::from_str(text).map_err(|e| format!("request is not valid JSON: {e}"))?;
    let seq = value
        .get("seq")
        .and_then(Value::as_u64)
        .ok_or_else(|| "request has no numeric `seq`".to_string())?;
    let follow = match value.get("follow") {
        None | Some(Value::Null) => Vec::new(),
        // A single link is the common case, so accept the bare string as well as the
        // list — a request that failed because it said `"follow": "hrw://..."` would
        // be a protocol detail costing a round trip to discover.
        Some(Value::String(one)) => vec![one.clone()],
        Some(Value::Array(many)) => {
            let mut links = Vec::with_capacity(many.len());
            for item in many {
                let Some(link) = item.as_str() else {
                    return Err(format!("`follow` holds a non-string entry: {item}"));
                };
                links.push(link.to_string());
            }
            links
        }
        Some(other) => return Err(format!("`follow` must be a string or a list: {other}")),
    };
    let settle = value
        .get("settleMs")
        .and_then(Value::as_u64)
        .map_or(SCREENSHOT_SETTLE_DEFAULT, std::time::Duration::from_millis);
    // **A region we do not recognise is refused, not defaulted.** Defaulting would
    // answer a request for a crop with a full window and nothing saying so, and the
    // reply's own `region` field would then be the only clue — read after the fact,
    // by someone already looking at the wrong picture.
    let region = match value.get("region").and_then(Value::as_str) {
        None | Some("window") => ShotRegion::Window,
        Some("pane") => ShotRegion::Pane,
        Some(other) => {
            return Err(format!(
                "`region` must be `window` or `pane`, not `{other}`"
            ));
        }
    };
    Ok(ScreenshotRequest {
        region,
        seq,
        why: value.get("why").and_then(Value::as_str).map(str::to_string),
        follow,
        settle,
    })
}

/// The `seq` the result file already carries, if any.
///
/// Read at startup to seed the "already answered" mark, so a request still sitting on
/// disk from a previous session does not move Doug's window the moment HRW opens.
#[must_use]
pub fn last_answered_screenshot() -> Option<u64> {
    let text = fs::read_to_string(SCREENSHOT_RESULT_FILE).ok()?;
    serde_json::from_str::<Value>(&text)
        .ok()?
        .get("seq")?
        .as_u64()
}

/// The PNG a given request's shot is written to.
#[must_use]
pub fn screenshot_path(seq: u64) -> PathBuf {
    // Zero-padded so a directory listing sorts in request order rather than
    // lexicographically — `10.png` before `2.png` is a small thing that makes a
    // sequence of shots read wrongly.
    Path::new(SCREENSHOTS_DIR).join(format!("shot-{seq:04}.png"))
}

/// The image half of a [`ShotResult`]: where it landed and how big it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShotImage {
    /// Absolute path to the PNG, as written.
    pub path: PathBuf,
    /// Pixel dimensions, **as captured** rather than as requested — a window the
    /// user resized, or a HiDPI scale factor, makes these differ from anything HRW
    /// could predict, and the measurement is the useful one.
    pub width: usize,
    pub height: usize,
}

/// Everything HRW says back about one honoured (or failed) screenshot request.
pub struct ShotResult<'a> {
    /// The request this answers. `None` only for a request so malformed that no
    /// `seq` could be read from it.
    pub seq: Option<u64>,
    /// Echoed from the request.
    pub why: Option<&'a str>,
    /// The links HRW actually dispatched, in order — which can be shorter than the
    /// request's list when one of them failed to parse.
    pub followed: &'a [String],
    /// The PNG, when there is one. Absent exactly when `error` is set.
    pub image: Option<ShotImage>,
    /// The region the shot **actually** kept, which can differ from the one asked for:
    /// a `pane` crop falls back to the whole window when no pane has been drawn yet.
    /// Reported so the fallback is visible in the reply rather than inferred from a
    /// picture that is larger than expected.
    pub region: ShotRegion,
    /// What HRW was showing, from the same builder the capture uses.
    pub showing: Option<&'a View<'a>>,
    /// The loaded specimen's name, when one is loaded.
    pub specimen: Option<&'a str>,
    /// The current stage tab, by `StageKind::name`.
    pub stage: &'a str,
    /// HRW's on-screen notice at the moment of the shot, if any.
    ///
    /// **Load-bearing, not decoration.** A link HRW refuses — "no specimen loaded",
    /// "no fixture lab named X", a frame seek out of range — leaves a window that
    /// looks entirely healthy, and the photograph of it would support a confident
    /// wrong conclusion. The notice is the only place that refusal is recorded.
    pub notice: Option<&'a str>,
    /// Why there is no image. `None` on success.
    pub error: Option<&'a str>,
}

/// Write the reply to [`SCREENSHOT_RESULT_FILE`].
///
/// Pretty-printed, like `focus.json`: Doug reads these when a capture-and-inspect run
/// is the thing under suspicion.
pub fn write_screenshot_result(result: &ShotResult<'_>) -> std::io::Result<PathBuf> {
    fs::create_dir_all(BRIDGE_DIR)?;
    let path = Path::new(SCREENSHOT_RESULT_FILE).to_path_buf();
    fs::write(
        &path,
        serde_json::to_string_pretty(&screenshot_result_json(result)).unwrap_or_default(),
    )?;
    Ok(path)
}

/// Build the result document. Split from the write so a test can assert on the
/// shape without touching the filesystem.
fn screenshot_result_json(result: &ShotResult<'_>) -> Value {
    json!({
        "instructions": "HRW's reply to .hrw-bridge/screenshot-request.json. `seq` \
                         identifies the request; `writtenAtMs` is when this reply was \
                         written, so a reply left over from an earlier request is \
                         detectable rather than mistaken for this one's. When `image` \
                         is null, `error` says why and NO photograph was taken — the \
                         previous PNG on disk is not this request's answer. `notice` \
                         carries HRW's own on-screen message, which is where a refused \
                         `follow` link is recorded; the window itself looks healthy \
                         when a link is refused.",
        "seq": result.seq,
        "writtenAtMs": now_ms(),
        "why": result.why,
        "followed": result.followed,
        "image": result.image.as_ref().map(|i| json!({
            "path": i.path.display().to_string(),
            "width": i.width,
            "height": i.height,
        })),
        "region": result.region.as_str(),
        "specimen": result.specimen,
        "stage": result.stage,
        "showing": result.showing.map(View::to_json),
        "notice": result.notice,
        "error": result.error,
    })
}

/// Milliseconds since the Unix epoch, for the result's staleness stamp.
fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// Write `rgba` as a PNG at `path`, creating [`SCREENSHOTS_DIR`] if needed.
///
/// `rgba` is **straight** (not premultiplied) 8-bit RGBA, row by row from the top —
/// the conversion from egui's premultiplied `Color32` happens in `screenshot.rs`,
/// which owns the egui types. This function deliberately knows nothing about egui, so
/// the bridge module stays the file protocol and nothing else.
///
/// # Errors
///
/// Returns the io error from creating the directory or writing the file, and an
/// `InvalidData` error when `rgba` is not exactly `width * height * 4` bytes — a
/// mismatch there would otherwise produce a PNG that decodes to garbage or a panic
/// inside the encoder.
pub fn write_png(path: &Path, width: usize, height: usize, rgba: &[u8]) -> std::io::Result<()> {
    let expected = width
        .checked_mul(height)
        .and_then(|px| px.checked_mul(4))
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("screenshot dimensions overflow: {width}x{height}"),
            )
        })?;
    if rgba.len() != expected {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "screenshot is {} bytes, but {width}x{height} RGBA needs {expected}",
                rgba.len()
            ),
        ));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = fs::File::create(path)?;
    let mut encoder = png::Encoder::new(
        std::io::BufWriter::new(file),
        u32::try_from(width).unwrap_or(u32::MAX),
        u32::try_from(height).unwrap_or(u32::MAX),
    );
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(std::io::Error::other)?;
    writer.write_image_data(rgba).map_err(std::io::Error::other)
}

/// Absolute path to `live_trace_breakpoint`, resolved at compile time so the
/// VS Code extension can set breakpoints without path resolution.
///
/// **Moved out of `crates/rumoca-phase-structural/` in the v0.10.0 rebase.**
/// No Rumoca phase ever named `LiveTrace`, so a file of HRW plumbing inside a
/// crate meant for a clean upstream cherry-pick was the wrong place for it;
/// see `live_trace.rs`'s own header. The anchor is still on the algorithm
/// thread and still inside the algorithm's call stack, so stepping is
/// unchanged — only the file differs.
const LIVE_TRACE_FILE: &str = crate::manifest_path!("src", "live_trace.rs");

/// The traced matching algorithm, for `hrw://breakpoint/<anchor>` links.
const MATCHING_FILE: &str = crate::manifest_path!(
    "..",
    "crates",
    "rumoca-phase-structural",
    "src",
    "matching.rs"
);

/// Resolve a short source-file name to the path the extension needs.
///
/// **Only the files a lab may arm a breakpoint in**, deliberately: a link is
/// user-facing text, and resolving arbitrary paths from one would let a lab point
/// the debugger anywhere.
pub fn traced_source_path(file: &str) -> std::io::Result<std::path::PathBuf> {
    let raw = match file {
        "live_trace.rs" => LIVE_TRACE_FILE,
        "matching.rs" => MATCHING_FILE,
        other => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("no traced source named {other}"),
            ));
        }
    };
    let path = std::fs::canonicalize(raw)?;
    // Same treatment as the anchor: VS Code's breakpoint URIs will not match a
    // `\\?\` extended-length path, so the prefix comes off.
    #[cfg(windows)]
    let path = strip_windows_prefix(&path);
    Ok(path)
}

/// Arm a breakpoint at `file:line` on behalf of an `hrw://breakpoint/<anchor>`
/// link.
///
/// **Add-only, and never a toggle.** `docs/ideas.md` #74: `cppvsdbg` will not
/// re-bind a location whose breakpoint has left its active set during a session,
/// so a link that removed a breakpoint would make the *next* click at the same
/// line silently do nothing. Clicking twice is therefore harmless — the
/// extension reports it as already present and changes nothing.
///
/// **No `specimen` field, also deliberately.** The extension clears every armed
/// breakpoint when the specimen *changes*, so a link carrying one could tear
/// down the live-trace anchor as a side effect of arming something else.
pub fn arm_source_breakpoint(file: &str, line: usize) -> std::io::Result<()> {
    // A stale ack must not be mistaken for this request's reply.
    let _ = fs::remove_file(BREAKPOINT_ACK_FILE);
    let path = traced_source_path(file)?;
    let request = json!({
        "version": 1,
        "breakpoints": [{ "path": path.display().to_string(), "line": line }]
    });
    let text = serde_json::to_string_pretty(&request)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    fs::write(BREAKPOINT_REQUEST_FILE, text)
}

/// The canonical list of pipeline stage file names written to `.hrw-bridge/stages/`.
///
/// This constant is the single source of truth for which stage files exist.
/// Both the focus JSON (so Claude knows what to read) and `write_stages` callers
/// should reference this list. If a new stage is added to the pipeline, add its
/// filename here — a test (`focus_json_stage_files_match_constant`) will fail if
/// the focus JSON and this list diverge.
pub const STAGE_FILE_NAMES: &[&str] = &[
    "parse.json",
    "resolve.json",
    "instantiate.json",
    "typecheck.json",
    "flatten.json",
    "dae.json",
    "structural.json",
    "index_reduction.json",
    "initialization.json",
    "events.json",
    "solve_lowering.json",
    // **Written on a simulate, not on a compile** — the only entry here that is. Kept in
    // step with `App::sim_data` by `App::set_sim_data`, so its presence means *this*
    // model's run and never a previous one's.
    "simulation.json",
];

/// Maximum size of a captured node's subtree before it degrades to a shape
/// summary (its key names, or an array length).
///
/// **Re-justified 2026-07-28.** The old reasoning was "prevent the focus file
/// from exploding", which is a tidiness argument, and it produced a perverse
/// result: past the limit the node was replaced by its *shape*, so the largest
/// and most interesting nodes were the ones taught about least. A captured
/// class definition would arrive as a list of key names.
///
/// The right question is what a reader needs, and Doug's direction is explicit
/// — answer quality first, token consumption not a constraint. So this is now
/// 256 KiB: large enough that a whole flattened class or a stage's equation
/// list arrives intact, and the degradation is a genuine last resort rather
/// than a routine event. Past it, the shape summary plus the stage file under
/// `stages/` is a real fallback, because a node that big is being skimmed
/// rather than read.
const MAX_NODE_BYTES: usize = 256 * 1024;

// Self-describing instructions embedded in every focus file. When Doug opens
// `focus.json` directly while dogfooding, this text explains what it is and
// how to use it, without needing to consult separate documentation.
const INSTRUCTIONS: &str = "\
HRW bridge focus file. Writing it does NOT ask anything by itself — the user \
asks in the Claude Code chat, and this describes what they had assembled. \
Reason over it together with the specimen source, the staged IR under \
`stages/`, the Rumoca phase code, and docs/compiler-phases.\n\n\
It carries context in two shapes, and the difference matters:\n\
  • POINTING AT — the `kind`/`node`/`cross_stage`/`stage` fields at top level. \
One node, one stage, chosen deliberately. It is a NOUN: pointing declares no \
verb, so `request` is normally null and THE VERB IS THE USER'S CHAT MESSAGE — \
read it from there, and do not assume they want an explanation. Only when \
`request` is `debug-where-set`, declared by the debugger menu item, is ONE \
breakpoint wanted, where that value is set.\n\
  • FOLLOWING — the `tracking` section. One identifier, everywhere it appears \
across every stage: a lens for whatever the user asks. When `request` is \
`debug-where-set` it wants SEVERAL breakpoints along the identifier's trajectory.\n\n\
Either may be absent, and absence is stated rather than implied. \
`kind: \"none\"` means the user DELIBERATELY CLEARED the point (or never made \
one), so the `tracking` section is the whole subject — do not fall back to \
describing the current stage, which they did not choose. A missing `tracking` \
section likewise means nothing is being followed. `request` is a property of \
the point, so it is null whenever `kind` is \"none\". If BOTH are absent, \
nothing has been assembled: say so rather than answering from `stage` or from \
whatever was pointed at before.\n\n\
When both are present and the request is ambiguous, compare `seq` with \
`tracking.seq`: whichever is higher was acted on last and is almost certainly \
the subject.\n\n\
Three sections exist so you do not have to reconstruct by hand what HRW \
already knew:\n\
  • `view` — what was ON SCREEN. A point made in a tree and one made paused \
mid-animation are different questions; only this section distinguishes them.\n\
  • `phase_source` — where this stage's algorithm lives in the workspace. \
Read the code rather than inferring the algorithm from its output.\n\
  • `neighbourhood` (on `node`, and on each `tracking` context) — the IR \
AROUND an address: the largest enclosing node that fit, plus the names \
adjacent to the hit. Fields on the enclosing object are often the answer \
(`generated: true` says a variable was manufactured by a phase, not \
declared), and adjacency is often the finding (a manufactured companion sits \
beside the variable it shadows).\n\n\
A `tracking.generated` section means the followed name was SYNTHESIZED by a compiler \
phase and is declared nowhere; `declared_at_line` is absent in that case rather than \
pointing at the base variable's declaration, which would name a different variable.\n\n\
In `tracking.stages`, `mentions: 0` is information, not a gap — the name is \
genuinely absent from that stage, which is how a demoted or alias-eliminated \
variable announces itself. Counts are exact; `paths` and `contexts` are \
samples, and say so when truncated.\n\n\
Everything here is a fact about the IR or the app. Nothing is an \
interpretation — that part is yours.";

/// One segment of a key-path into a JSON tree.
///
/// A key-path is a sequence of `Seg` values that addresses a specific node
/// from the tree root. For example, the path to `components.inertia.def_id`
/// would be `[Key("components"), Key("inertia"), Key("def_id")]`. Array
/// elements use `Index(n)` — e.g., `equations[0]` is `[Key("equations"), Index(0)]`.
///
/// This is the tree-agnostic addressing scheme that lets the bridge, tree
/// inspector, and custom views all refer to the same node without knowing
/// Rumoca types.
/// `PartialEq` so a rendered tree row can ask "am I the node being jumped to?"
/// by comparing paths. Pointer identity is not usable there — the jump target
/// arrives as a path from `find_mentions`, not as a reference into the tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Seg {
    /// An object field name (e.g., "components", "def_id").
    Key(String),
    /// An array index (e.g., 0 for the first element).
    Index(usize),
}

impl fmt::Display for Seg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Seg::Key(k) => write!(f, "{k}"),
            Seg::Index(i) => write!(f, "[{i}]"),
        }
    }
}

impl Seg {
    // Serialize this segment for inclusion in the focus JSON.
    // Keys become JSON strings, indices become JSON numbers.
    fn as_json(&self) -> Value {
        match self {
            Seg::Key(k) => Value::String(k.clone()),
            Seg::Index(i) => json!(i),
        }
    }

    /// Navigate one step into a JSON value: `Key("x")` does `v["x"]`,
    /// `Index(3)` does `v[3]`. Returns `None` if the key/index doesn't exist.
    /// This is the fundamental building block of `navigate()`.
    ///
    /// `pub` as `get_in` for the tree, which runs a jump target's ancestors to
    /// open them. Named differently from the private `get` so it does not read
    /// like a `serde_json` method at the call site.
    pub fn get_in<'a>(&self, v: &'a Value) -> Option<&'a Value> {
        self.get(v)
    }

    fn get<'a>(&self, v: &'a Value) -> Option<&'a Value> {
        match self {
            Seg::Key(k) => v.get(k),
            Seg::Index(i) => v.get(i),
        }
    }
}

/// Parse a path written by [`describe_path`] back into segments.
///
/// **The documented inverse**, so `hrw://…/node/<path>` accepts exactly the string a
/// capture emits. That is #42's parity principle at its sharpest: a node path is the
/// capture's richest noun, and a link that could not consume the capture's own spelling
/// of it would be a vocabulary that agrees with itself and nothing else.
///
/// Grammar, matching `describe_path` exactly:
///
/// ```text
/// error.unmatched_unknowns[0]   ->  [Key("error"), Key("unmatched_unknowns"), Index(0)]
/// blocks[2].equations[0]        ->  [Key("blocks"), Index(2), Key("equations"), Index(0)]
/// enum_ordinals["StateSelect.never"]
///                               ->  [Key("enum_ordinals"), Key("StateSelect.never")]
/// (tree root)  or  <empty>      ->  []            (the root itself)
/// ```
///
/// # Quoted keys
///
/// A JSON object key may contain the separators this grammar uses. Modelica IR is
/// full of them — `enum_literal_ordinals` is keyed by qualified names like
/// `StateSelect.never`, and flat variable names are dotted throughout. Written
/// bare, such a key splits into two segments and lands on a node that does not
/// exist. **`docs/fidelity-plan.md` F7 found this on 2026-07-31**, across every
/// model in the corpus.
///
/// So a key containing `.`, `[`, `]` or `"` — or an empty key — is written
/// `["…"]` with `\` and `"` backslash-escaped, and no `.` before it. Keys without
/// those characters are unchanged, so every path emitted before this existed
/// still parses to the same segments.
///
/// Returns `None` on anything malformed rather than guessing: a link that silently
/// pointed at the wrong node would be worse than one that visibly does nothing, because
/// the reader would take the wrong subtree for the answer.
pub fn parse_path(s: &str) -> Option<Vec<Seg>> {
    if s.is_empty() || s == ROOT_SENTINEL {
        return Some(Vec::new());
    }
    let mut out = Vec::new();
    let mut rest = s;
    loop {
        // A key runs to the next '.' or '[' — but a bracket may come first, when a
        // previous segment ended with one (`blocks[2][0]`) or when the key itself
        // needed quoting (`a["b.c"]`).
        if let Some(after) = rest.strip_prefix("[\"") {
            let (key, tail) = parse_quoted_key(after)?;
            out.push(Seg::Key(key));
            rest = tail;
        } else if let Some(after) = rest.strip_prefix('[') {
            let (digits, tail) = after.split_once(']')?;
            out.push(Seg::Index(digits.parse().ok()?));
            rest = tail;
        } else {
            let end = rest.find(['.', '[']).unwrap_or(rest.len());
            let key = &rest[..end];
            if key.is_empty() {
                return None; // ".." or a leading '.', neither of which describe_path emits
            }
            out.push(Seg::Key(key.to_owned()));
            rest = &rest[end..];
        }
        match rest.chars().next() {
            None => return Some(out),
            Some('.') => rest = &rest[1..],
            Some('[') => {}
            Some(_) => return None,
        }
        if rest.is_empty() {
            return None; // trailing '.', which describe_path never produces
        }
    }
}

/// Read a `["key"]` segment, starting just after the opening quote.
///
/// Returns the unescaped key and the remainder after the closing `"]`. `None` on
/// an unterminated quote or a dangling escape — malformed input never guesses.
fn parse_quoted_key(after_quote: &str) -> Option<(String, &str)> {
    let mut key = String::new();
    let mut chars = after_quote.char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '\\' => key.push(chars.next()?.1),
            '"' => {
                let tail = after_quote.get(i + 1..)?.strip_prefix(']')?;
                return Some((key, tail));
            }
            _ => key.push(c),
        }
    }
    None
}

/// How [`describe_path`] spells the empty path, and what [`parse_path`] reads back as
/// one. **One constant because three places must agree on it** — and a key spelled the
/// same way is the collision `key_needs_quoting` now guards.
const ROOT_SENTINEL: &str = "(tree root)";

/// Does this key need `["…"]` rather than a bare `.key`?
///
/// Empty keys too: a bare empty key produces `a..b`, which `parse_path` rejects.
///
/// **And a key spelled [`ROOT_SENTINEL`]** (2026-11-07, the post-rebase sweep's open
/// item 11). Written bare, `[Key("(tree root)")]` described exactly as the empty path
/// does, and navigating the parsed description landed on the whole tree instead of
/// the child. Latent — no Modelica IR key is spelled that way — but F7's must-fire test
/// had been *using* it, which is why `fidelity::check_f7_with` was made injectable
/// first: fixing the grammar alone would have left F7 with nothing that could fail it.
fn key_needs_quoting(k: &str) -> bool {
    k.is_empty() || k == ROOT_SENTINEL || k.contains(['.', '[', ']', '"', '\\'])
}

/// Format a key-path as a human-readable dotted string.
///
/// Examples:
/// - `[Key("components"), Key("inertia"), Key("def_id")]` -> `"components.inertia.def_id"`
/// - `[Key("equations"), Index(0), Key("Connect")]` -> `"equations[0].Connect"`
/// - `[]` -> `"(tree root)"`
///
/// Used in the UI status bar to show what was captured, and in the bridge
/// focus file as a human-readable path alongside the machine-readable key array.
pub fn describe_path(path: &[Seg]) -> String {
    if path.is_empty() {
        return ROOT_SENTINEL.to_owned();
    }
    let mut s = String::new();
    for seg in path {
        match seg {
            // A key carrying this grammar's own separators is bracketed and
            // quoted, and takes no leading `.` — see `parse_path`'s note on why
            // Modelica IR makes this the common case rather than an edge one.
            Seg::Key(k) if key_needs_quoting(k) => {
                s.push_str("[\"");
                for c in k.chars() {
                    if c == '"' || c == '\\' {
                        s.push('\\');
                    }
                    s.push(c);
                }
                s.push_str("\"]");
            }
            Seg::Key(_) => {
                if !s.is_empty() {
                    s.push('.');
                }
                write!(s, "{seg}").unwrap();
            }
            Seg::Index(_) => write!(s, "{seg}").unwrap(),
        }
    }
    s
}

/// What the user captured — the "focus" of their question.
///
/// This enum distinguishes three granularity levels:
/// - `Node`: a specific IR node (the most common — user clicked a field in the tree)
/// - `Stage`: the entire stage's IR (user captured from the stage tab header)
/// - `Specimen`: the whole `.mo` file (user captured from the specimen list)
///
/// The lifetime `'a` borrows the IR data from the app's state, avoiding clones
/// of potentially large JSON trees during the focus-file build.
pub enum Focus<'a> {
    /// A specific IR node in the current stage, at `key_path` from the stage root.
    /// `stage_value` is a reference to the stage's full IR (used to navigate to
    /// the node and build provenance).
    Node {
        key_path: Vec<Seg>,
        stage_value: &'a Value,
    },
    /// The current stage's IR as a whole.
    Stage,
    /// The whole specimen (the `.mo` file) — captured from the specimen list.
    /// Distinct from the `model` field of `Ask`, which names the compiled class
    /// (a `.mo` file can contain multiple classes).
    Specimen,
    /// **Nothing is pointed at.** The user cleared the point, leaving only what
    /// they are following.
    ///
    /// A distinct variant rather than reusing `Stage`, and rather than
    /// `Ask { focus: Option<Focus> }`, for one reason each:
    ///
    /// - Not `Stage`, because "pointing at the Typecheck stage as a whole" is a
    ///   *claim the user made* by clicking a tab. Emitting it for someone who
    ///   pointed at nothing would attribute a subject they never chose — the
    ///   confident lie this design exists to prevent.
    /// - Not `Option`, because absence must be **stated, not implied**. An
    ///   omitted field reads as "unknown"; `kind: "none"` reads as "deliberately
    ///   empty — the thread is the whole subject". Same reasoning as
    ///   `mentions: 0` in the tracking section.
    Nothing,
    /// **A passage of lab prose**, selected in the lab panel and captured with 🎯.
    ///
    /// The first focus that is not part of a compile: there is no stage, no IR and no
    /// key path, which is why [`Ask::stage`] is `None` for one of these and why
    /// [`crate::question_bar::PointedAt::stage`] became an `Option` the same day.
    ///
    /// **`text` is what the pane RENDERED**, markdown stripped — that is what egui can
    /// hand back, and Doug ruled it sufficient. It is enough to answer *"what does this
    /// mean?"*; for *"improve this paragraph"* Claude locates the source itself. Nothing in
    /// that source marks who wrote a passage: the markers that did were retired on
    /// 2026-08-31 (`DECISIONS.md`), and this doc named them as live until 2026-11-08.
    Selection {
        /// Where the text came from, declared by the region that rendered it.
        origin: &'a crate::pointing::PointOrigin,
        /// The selected text, verbatim as rendered.
        text: &'a str,
        /// Whether `text` was found in that lab's source. See
        /// [`crate::question_bar::PointKind`]'s `LabPassage` for why this is checked
        /// rather than assumed.
        in_source: bool,
    },
}

/// A verb the user **declared by a gesture** — never one HRW presumes.
///
/// # The rule, which is Doug's (2026-11-07)
///
/// *"The context bar is not supposed to presume verbs. It is only supposed to show a
/// bunch of nouns which I have identified by pointing and clicking with my mouse. Unless
/// I select something like a context menu item which explicitly declares a verb, no verbs
/// should be associated with my context nouns."*
///
/// That is charter Decision 8 — *the noun is assembled by mouse; the verb is an
/// unbounded utterance* — applied to the data model, and the data model had drifted from
/// it. **So this type holds only verbs a gesture names in its own label**, and the field
/// carrying it is an `Option`: `None` is the ordinary case and means *the verb is in the
/// chat message*.
///
/// # Why there is no `Explain` variant, and why that is the fix rather than a detail
///
/// Until 2026-11-07 there was one, and **every capture path except the debugger's
/// supplied it** — a tree-node click, a stage focus, a specimen focus, the context emit
/// and a text selection all hard-wired `Explain`. **Nothing in HRW ever branched on
/// it.** It was only broadcast: to the Question Bar as `(explain)`, into `focus.json`,
/// into `session.json`, and through the `[hrw-context]` hook to Claude — so every
/// question Doug asked reached Claude pre-labelled as a request for an explanation he had
/// not made, biasing the reading of his actual sentence toward explaining.
///
/// **Hiding the label when it was the default was proposed first, and would have been
/// wrong**: it hides a presumed verb while keeping it. Removing the variant makes the
/// presumption *inexpressible* — charter Decision 16's strongest form, a design in which
/// the error cannot be written — so a later capture path has no default verb to reach
/// for. The compiler, not a test, is what stops it coming back.
///
/// # What a declared verb does, which is less than its name suggests
///
/// Not even this one makes HRW *act*: the status line answers a debugger point with
/// *"say 'debug' in chat and I'll set the breakpoint."* **Every verb that takes effect is
/// one Doug types.** What the declaration buys is that the noun was assembled *for* that
/// verb — a field whose setting site is wanted — which changes what Claude should
/// extract from it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AskRequest {
    /// Declared by **🐞 Show this being set (debugger)** on an IR tree node: find the
    /// one place in Rumoca that sets this value, so a breakpoint can be armed there.
    DebugWhereSet,
}

impl AskRequest {
    pub fn as_str(self) -> &'static str {
        match self {
            AskRequest::DebugWhereSet => "debug-where-set",
        }
    }
}

/// The ambient half of the context: what the user is *following*.
///
/// Where [`Focus`] is a **point** — one node, one stage, deliberately chosen —
/// this is a **thread**: one identifier, everywhere it appears, across every
/// stage. Both are context assembly; see `docs/context-assembly.md`.
///
/// Emitting it turns work HRW already does into context it can be asked about.
/// Every stage view already sweeps its own data each frame to decide what to
/// highlight, then discards the answer at the end of the paint. This keeps it.
///
/// ## Why the two must stay distinguishable
///
/// They drive different behaviour, so flattening them degrades both:
///
/// - For `explain`, the point is the **subject** and the thread is the
///   **lens**. "Pointed at this node, following `src.V`" asks for the node
///   explained as part of that variable's story.
/// - For `debug-where-set`, it decides **how many breakpoints**: a point wants
///   one site, a thread wants several — resolved, flattened, matched, demoted.
///
/// Hence `seq` here as well as on the focus: when both are present and the
/// request is ambiguous, whichever was acted on *last* is almost certainly the
/// subject. One shared counter could not express that.
pub struct Tracking<'a> {
    /// Monotonic counter for *this* section, compared against the focus `seq`
    /// to tell "pointed at this, then went following" from the reverse.
    pub seq: u64,
    /// The followed identifier, as a qualified flat name.
    pub name: &'a str,
    /// 1-based source line, when the specimen declares it.
    pub declared_line: Option<u32>,
    /// The class that declares it, when a component type does.
    pub declaring_class: Option<&'a str>,
    /// Every pipeline stage's IR, in order, as `(name, value)`.
    pub stage_values: &'a [(&'a str, Option<&'a Value>)],
}

/// How much a followed identifier actually has behind it: total mentions, and
/// how many stages contain at least one.
///
/// For the Question Bar, which states what a question will have to work with
/// *before* it is asked — the difference between a rich answer and a thin one.
/// Computed at emission time (a click), never per frame: it walks every stage's
/// IR.
pub fn summarize_tracking(t: &Tracking) -> (usize, usize) {
    let mut mentions = 0usize;
    let mut stages = 0usize;
    for (_, value) in t.stage_values {
        if let Some(v) = value {
            let mut here = 0usize;
            find_mentions(v, t.name, &mut Vec::new(), &mut Vec::new(), &mut here, 0);
            if here > 0 {
                stages += 1;
            }
            mentions += here;
        }
    }
    (mentions, stages)
}

/// Cap on recorded mention paths per stage.
///
/// **The count is always exact; only the addresses are sampled.** The cap is
/// about how much a reader can hold in view, not about file size — following a
/// common variable can produce hundreds of mentions, and a list that long stops
/// being a map of where the identifier lives and becomes something to skim.
///
/// Forty rather than the original twelve: twelve was chosen when the focus file
/// was judged against the stage IR's size, which was the wrong yardstick. At
/// forty, a variable's whole footprint in a stage is usually listed rather than
/// glimpsed, and `paths_truncated` still says when it is not.
const MAX_MENTION_PATHS: usize = 40;

/// How many mentions per stage get their surrounding IR, not just an address.
///
/// Two tiers on purpose. `paths` answers *where does this identifier appear*,
/// cheaply and nearly completely. `contexts` answers *what does each appearance
/// look like* — and that is what carries `generated: true`, the neighbouring
/// `__pre__` companions, the enclosing equation. Six is enough to see the
/// pattern in a stage; past that the answers repeat and the addresses suffice.
const MAX_MENTION_CONTEXTS: usize = 6;

/// Walk one stage's IR, counting and locating mentions of `name`.
///
/// Uses exactly the rules the views use, so the emitted context and the
/// on-screen highlighting can never disagree: exact identity
/// (`same_variable`), lexical mention for code-bearing strings
/// (`mentions_identifier`), and prose fields excluded by name.
///
/// Collects `Vec<Seg>` rather than formatted strings because the caller needs
/// to navigate back to each hit to build its neighbourhood; a rendered
/// `"a.b[0].c"` cannot be run, and re-parsing one would be guesswork the
/// moment a key contains a dot — which, in `bindings.__pre__.overSpeed`, it does.
///
/// Wrapped by [`mention_paths`] for callers outside this module.
fn find_mentions(
    value: &Value,
    name: &str,
    path: &mut Vec<Seg>,
    paths: &mut Vec<Vec<Seg>>,
    total: &mut usize,
    cap: usize,
) {
    let hit = |path: &[Seg], total: &mut usize, paths: &mut Vec<Vec<Seg>>| {
        // `total` is always exact; `cap` only bounds how many addresses are
        // kept. The emitted context caps them (a reader cannot hold hundreds in
        // view); the jump control does not (it cycles through every one).
        *total += 1;
        if paths.len() < cap {
            paths.push(path.to_vec());
        }
    };
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                path.push(Seg::Key(k.clone()));
                // A key *is* the name — e.g. `variables.states["emf.phi"]`.
                if crate::identifier_index::same_variable(k, name) {
                    hit(path, total, paths);
                } else if let Value::String(s) = v {
                    if !crate::identifier_index::is_prose_field(k)
                        && (crate::identifier_index::same_variable(s, name)
                            || crate::source_view::mentions_identifier(s, name))
                    {
                        hit(path, total, paths);
                    }
                } else {
                    find_mentions(v, name, path, paths, total, cap);
                }
                path.pop();
            }
        }
        Value::Array(arr) => {
            for (i, v) in arr.iter().enumerate() {
                path.push(Seg::Index(i));
                find_mentions(v, name, path, paths, total, cap);
                path.pop();
            }
        }
        _ => {}
    }
}

/// Every node in `stage_value` that mentions `name`, in tree order.
///
/// **The same list the capture emits.** `tracking.paths` in `focus.json` is
/// `describe_path` applied to exactly this, so the tree's "where is it" and
/// Claude's "where is it" cannot diverge. A second matcher written for the UI
/// would be a second definition of *mention*, and this project has spent a phase
/// removing exactly that kind of drift — the app must not highlight one set of
/// nodes while telling Claude about another.
///
/// Uncapped, unlike the emitted `paths`: the cap there bounds what a *reader*
/// can hold in view, while a jump control needs every match to cycle through.
pub fn mention_paths(stage_value: &Value, name: &str) -> Vec<Vec<Seg>> {
    let mut paths = Vec::new();
    let mut total = 0usize;
    find_mentions(
        stage_value,
        name,
        &mut Vec::new(),
        &mut paths,
        &mut total,
        usize::MAX,
    );
    paths
}

/// Build the `tracking` section: where the followed identifier lives, stage by
/// stage — **including where it does not**.
///
/// `pub` so `examples/capture_probe.rs` can print it against a real compiled
/// specimen. Checking that the capture carries what a reader would otherwise
/// hunt for by hand means *reading the emitted value*; unit tests check shape,
/// and shape is not the thing in question.
///
/// Absence is emitted deliberately. "Not present in Initialization or Solve
/// lowering" is how a demoted or alias-eliminated variable announces itself, and
/// a hits-only capture cannot express disappearance. The disappearance is often
/// the whole story.
pub fn build_tracking(t: &Tracking) -> Value {
    let stages: Vec<Value> = t
        .stage_values
        .iter()
        .map(|(stage_name, value)| match value {
            Some(v) => {
                let mut paths = Vec::new();
                let mut total = 0usize;
                find_mentions(
                    v,
                    t.name,
                    &mut Vec::new(),
                    &mut paths,
                    &mut total,
                    MAX_MENTION_PATHS,
                );
                json!({
                    "stage": stage_name,
                    "mentions": total,
                    "paths": paths.iter().map(|p| describe_path(p)).collect::<Vec<_>>(),
                    "paths_truncated": total > paths.len(),
                    // The surrounding IR for the first few. An address alone
                    // makes the reader open the stage file *and* already know
                    // what to look for; this carries what they would have found.
                    "contexts": paths.iter()
                        .take(MAX_MENTION_CONTEXTS)
                        .map(|p| neighbourhood(v, p))
                        .collect::<Vec<_>>(),
                    "contexts_truncated": paths.len() > MAX_MENTION_CONTEXTS,
                })
            }
            // No IR at all is different from IR without the name in it.
            None => json!({ "stage": stage_name, "produced_no_ir": true }),
        })
        .collect();

    let mut out = json!({
        "seq": t.seq,
        "note": "the identifier the user is FOLLOWING — a thread through the \
                 pipeline, as opposed to `node`, which is the point they are \
                 POINTING AT. Stages with `mentions: 0` are meaningful: the \
                 name is genuinely absent there, which is how a demoted or \
                 alias-eliminated variable shows itself. `mentions` is the \
                 exact count; `paths` are addresses (a sample when \
                 `paths_truncated`); `contexts` carry the surrounding IR for \
                 the first few — `context` is the largest enclosing node that \
                 fit the budget, and `siblings.window` is what sits beside the \
                 hit in IR order.",
        "identifier": t.name,
        "stages": stages,
    });
    // Where it came from — a source line, a class, or neither.
    //
    // A **generated** name gets neither, because it has no declaration to point
    // at. Emitting `declared_at_line` for one was a real defect: following
    // `__pre__.overSpeed` reported line 41, which is where `overSpeed` is
    // declared. The number was real (the generated variable inherits its base's
    // span) but the *field name asserted a declaration that does not exist* —
    // the same species as a phantom `request` or a `kind` of "stage" for a
    // cleared point. A reader trusting the field would look for a declaration on
    // line 41 and find a different variable.
    if let Some(generated) = generated_origin(t.name) {
        out["generated"] = generated;
        if let Some(line) = t.declared_line {
            // Kept, renamed. The span is genuine provenance — it is where the
            // base variable is declared — so it is worth having under a name
            // that says what it is.
            out["span_inherited_from_base_at_line"] = json!(line);
        }
    } else {
        if let Some(line) = t.declared_line {
            out["declared_at_line"] = json!(line);
        }
        if let Some(class) = t.declaring_class {
            out["declared_in_class"] = json!(class);
        }
    }
    out
}

/// Whether this name was synthesized by a compiler phase, and by which.
///
/// **Uses Rumoca's sanctioned inverses, never a string match.**
/// `rumoca_core`'s `generated_names` module is the owning definition of these
/// conventions and says so explicitly: *"Consumers must never string-match
/// `\"__pre__\"` directly — construct slot names with `pre_slot_name` and
/// recover structure with `pre_slot_base` / `is_pre_slot`."* Recognising a
/// generated name by spelling would re-derive a convention this crate owns, and
/// would break silently the day it changes.
///
/// Returns `None` for ordinary names, which is what routes them to
/// `declared_at_line` / `declared_in_class` instead.
fn generated_origin(name: &str) -> Option<Value> {
    let base = rumoca_core::pre_slot_base(name)?;
    Some(json!({
        "kind": "pre-slot",
        "base": base,
        "note": "synthesized by DAE pre-lowering, which replaces `pre(x)` with a \
                 generated PARAMETER variable named `__pre__.x` — see \
                 crates/rumoca-core/src/ir_primitives/generated_names.rs, the owning \
                 definition of the convention. It is declared nowhere: it exists \
                 because a `when` equation needs a value to hold when no branch \
                 fires, and a DAE has no way to say `unchanged`.",
    }))
}

/// What HRW is actually showing — the view the user was looking at when they
/// assembled this context.
///
/// **The capture used to be blind to this.** A point at a node in the Resolve
/// tree and a point made while paused mid-index-reduction at frame 12 produced
/// *identical* files, even though in the second case the frame is most of the
/// question. Which view is on screen changes what "explain this" means, and it
/// is free to emit.
///
/// Every field is what the app is showing, not what it means. `stage_view`
/// carries the sub-tab's own name (`"MatchingAnim"`, `"EquationSheet"`) because
/// the enum variant is the exact fact, and a hand-written prettier string would
/// be a second thing to keep in sync.
#[derive(Clone)]
pub struct View<'a> {
    /// Which of the three left-panel modes: Lab, Specimen, Debug.
    pub ui_mode: &'a str,
    /// The sub-view within the current stage, when the stage has sub-tabs
    /// (Structural and Flatten do; the generic tree stages do not).
    pub stage_view: Option<&'a str>,
    /// Which specimen-mode detail pane: source or narrative.
    pub specimen_detail: Option<&'a str>,
    /// True when the Log pane has replaced the stage view.
    pub viewing_log: bool,
    /// Where an on-screen animation stands, if one is showing.
    pub animation: Option<AnimationView<'a>>,
}

/// An animation's position **and what it is showing**, for the capture.
#[derive(Clone)]
pub struct AnimationView<'a> {
    /// Which algorithm: `"matching"`, `"tarjan"`, `"reduction"`.
    pub which: &'a str,
    /// Cursor position and total frames — "frame 12 of 47".
    pub frame: usize,
    pub frame_count: usize,
    /// `LiveState` as a name: Idle, Arming, Running, Finished.
    pub live_state: &'a str,
    /// What the frame under the cursor shows — the same description the view is
    /// drawing, from `playback::Animated::current_frame_context`.
    ///
    /// **Position alone was not enough.** This section used to carry only
    /// `frame: 12, frame_count: 47`, which says *where* the user is but not
    /// *what they are looking at* — the frames live in memory and are in no
    /// stage IR, so a question asked mid-animation could not be answered
    /// precisely. That gap mattered because Doug's stated route into the
    /// algorithms is to watch them and ask, before he knows enough to phrase a
    /// question about the algorithm itself.
    ///
    /// `None` before the first frame of a live session arrives — a real state,
    /// not a failure.
    pub frame_context: Option<Value>,
}

impl View<'_> {
    fn to_json(&self) -> Value {
        json!({
            "note": "what HRW was showing when this context was assembled. A point \
                     made in a tree and one made mid-animation are different questions, \
                     and only this section can tell them apart.",
            "ui_mode": self.ui_mode,
            "stage_view": self.stage_view,
            "specimen_detail": self.specimen_detail,
            "viewing_log": self.viewing_log,
            "animation": self.animation.as_ref().map(|a| json!({
                "which": a.which,
                "frame": a.frame,
                "frame_count": a.frame_count,
                "live_state": a.live_state,
                "showing": a.frame_context,
            })),
        })
    }
}

/// Where in Rumoca a stage's code lives.
///
/// **Emitted so the algorithm can be read rather than inferred from its
/// output.** Explaining what the Events phase does by reading `events.json` is
/// working backwards from a result; the in-workspace move exists precisely so
/// the phase source is readable, and this closes the last gap — knowing which
/// file to open.
///
/// These are facts about the build (HRW calls exactly these functions from
/// `worker.rs`), not an interpretation of what the phases do.
///
/// **Six of the twelve named functions that no longer existed**, from the v0.10.0
/// rebase until the 2026-11-07 sweep: `to_dae_with_options_traced`, two `dae_prepare`
/// reductions, `build_ic_plan`, `lower_dae_to_solve_model` and `simulate_solve_model`.
/// Claude was sent to read code that was not there, which is worse than no pointer.
/// `every_phase_source_names_a_defined_function` now checks each entry's function
/// against the workspace.
pub(crate) fn phase_source(stage: StageKind) -> Value {
    // (crate directory, the entry point HRW actually calls)
    let (krate, entry) = match stage {
        StageKind::Parse => ("crates/rumoca-phase-parse", "parse_to_ast"),
        // Resolution is driven through the compile session rather than a free
        // function, so the entry named here is the session method.
        StageKind::Resolve => ("crates/rumoca-compile", "Session::resolved"),
        StageKind::Instantiate => ("crates/rumoca-phase-instantiate", "instantiate_model"),
        StageKind::Typecheck => ("crates/rumoca-phase-typecheck", "typecheck_instanced"),
        // Flatten has no standalone entry point: it is extracted from the
        // reachable-closure pipeline result. Saying so is more useful than
        // naming a function that does not exist.
        StageKind::Flatten => (
            "crates/rumoca-compile",
            "Session::compile_model_strict_reachable_with_recovery",
        ),
        // DAE construction runs inside the same reachable-closure pipeline as
        // Flatten, and the DAE comes back in its result; the construction record
        // (pre() lowering included) is read from that DAE, not from a second call.
        StageKind::Dae => (
            "crates/rumoca-phase-dae",
            "to_dae_with_evidence (run by the compile session, compile_support.rs)",
        ),
        StageKind::Structural => ("crates/rumoca-phase-structural", "build_structural_report"),
        StageKind::IndexReduction => (
            "crates/rumoca-phase-structural",
            "inspect_prepare_for_solve",
        ),
        // The same preparation Index reduction reports on: its prepared system's
        // initial-value pins (`worker::initialization_from_initial_pins`).
        StageKind::Initialization => (
            "crates/rumoca-phase-structural",
            "inspect_prepare_for_solve (the prepared system's pins)",
        ),
        // Events are not produced by a phase call — the condition algebra is already
        // in the DAE and HRW reads it out (`worker::events_from_condition_algebra`).
        StageKind::Events => ("crates/rumoca-ir-dae", "DaeView::conditions"),
        StageKind::SolveLowering => ("crates/rumoca-phase-solve", "lower_solve_problem"),
        StageKind::Simulation => ("crates/rumoca-sim", "simulate_with_optional_step_records"),
    };
    json!({
        "note": "where this stage's algorithm lives in the Rumoca workspace — read \
                 the code rather than inferring the algorithm from its output. \
                 Paths are relative to the repository root; HRW calls these from \
                 hrw/src/worker.rs.",
        "crate": krate,
        "entry": entry,
    })
}

/// All the context needed to write one focus file.
///
/// This struct aggregates everything the bridge needs from the app's state:
/// the capture target, the current specimen/stage, and the IR data for
/// cross-stage diffing. It borrows everything (lifetime `'a`) to avoid
/// cloning large IR trees.
pub struct Ask<'a> {
    /// Monotonically increasing sequence number — lets Claude detect when a
    /// new capture has occurred since the last question.
    pub seq: u64,
    /// The verb a gesture **declared**, if any. `None` is the ordinary case: the user
    /// pointed at a noun, and the verb is whatever they then type. See [`AskRequest`]
    /// for why there is no default verb to put here.
    pub request: Option<AskRequest>,
    /// Path to the Modelica source file (`.mo`). `None` if no specimen is loaded.
    pub specimen: Option<&'a Path>,
    /// The class name being compiled (e.g., "RotationalInertia"). A specimen file
    /// can contain multiple classes; this names the one currently viewed.
    pub model: Option<&'a str>,
    /// The pipeline stage (`None` for a navigated library definition).
    pub stage: Option<StageKind>,
    /// Paths to any Modelica library files used during compilation.
    pub libraries: Vec<String>,
    /// Resolved identity of DefIds in the model's IR. The IR contains numeric
    /// DefIds (e.g., `type_def_id: 27579`); this table maps each to its
    /// human-readable name and kind, so the focus file is self-explanatory.
    pub def_index: &'a BTreeMap<u64, DefInfo>,
    /// The Parse stage's IR (if available). Used for cross-stage diffing.
    pub parse_value: Option<&'a Value>,
    /// The Resolve stage's IR (if available). Used for cross-stage diffing.
    pub resolve_value: Option<&'a Value>,
    /// What the user captured (node, stage, or specimen) — the *point*.
    pub focus: Focus<'a>,
    /// What the user is following — the *thread*. Independent of `focus`:
    /// point-only, thread-only, and both are all normal states.
    pub tracking: Option<Tracking<'a>>,
    /// What HRW was showing. See [`View`] — a point made in a tree and one made
    /// mid-animation used to produce identical files.
    pub view: View<'a>,
    /// The stage that **stopped** the pipeline, if one did. See [`PipelineFailure`].
    pub failure: Option<PipelineFailure<'a>>,
    /// Stages Rumoca flagged and **carried on past**. See [`FlaggedStage`].
    pub flags: Vec<FlaggedStage<'a>>,
}

/// A stage where Rumoca reported a problem and the pipeline **continued**.
///
/// # The problem this exists to stop asserting, measured 2026-11-05
///
/// Everything abnormal used to arrive as [`PipelineFailure`], because
/// `App::failure_context` selected on `Stage::note_is_error`, which is
/// `outcome != Ok`. **`Drivetrain`'s capture therefore told Claude the pipeline failed
/// at Structural** — in a file whose own `downstream_not_reached` was `[]`, about a
/// model that simulates 501 samples on 3 states. Doug read the table recording the
/// code-level suspicion and said *"I very much want to widen the scope"*; the capture
/// above was then taken to confirm it, because the suspicion had been read from the
/// source rather than seen.
///
/// **Six corpus specimens are in this class** — `BenchActuator`, `CapacitorLoop`,
/// `CartesianPendulum`, `Drivetrain`, `GearWithBrake`, `MotorWithBrake` — so it is the
/// ordinary condition of a healthy high-index model, not an edge case. Rumoca is a
/// recovering compiler: it flags and continues, and `Outcome::Flagged` exists to say so.
///
/// **The payload is kept, only the framing changes.** `error` here carries the same
/// unmatched unknowns and source locations the failure section carried, because that is
/// real diagnosis and discarding it would send Claude to `stages/structural.json` to
/// dig it out again.
pub struct FlaggedStage<'a> {
    /// Stage name, as `StageKind::name` reports it.
    pub stage: &'static str,
    /// The stage's note — the one-line summary already shown in the UI.
    pub note: &'a str,
    /// The stage's structured payload, when it has one.
    pub error: Option<&'a Value>,
}

/// The first stage of the pipeline that failed, and what it said.
///
/// Added 2026-07-29 (ideas #45 step 3). Until then a capture never mentioned that
/// anything had *failed* — it described what Doug was pointing at and left Claude to
/// discover the failure by reading the stage files. That worked because Doug named the
/// stage in his question ("why did the **structural** phase fail?"). Someone under
/// deadline pressure says "it doesn't work", and then the capture has to know.
///
/// **First failing stage, not the current one.** A failure cascades: everything
/// downstream reports "not reached", so the *earliest* error is the cause and the rest
/// are consequences. Naming the stage Doug happens to be looking at would often name a
/// consequence.
///
/// # What this is NOT, since 2026-11-05
///
/// **A stage being abnormal is not a stop**, and this type is now reserved for a stage
/// after which the pipeline genuinely did not continue — `not_reached` non-empty.
/// Everything else arrives as [`FlaggedStage`]. The discriminator is deliberately the
/// *measured* tail rather than the outcome label, because the label does not decide it:
/// `DimensionMismatch`'s Typecheck is `Flagged` and **does** stop the pipeline, while
/// `Drivetrain`'s Structural is `Flagged` and does not.
///
/// **And a `Failed` stage is a stop with or without a tail** (2026-11-07). The last
/// compilation stage has nothing after it to be "not reached", so Solve lowering's
/// refusal of `IncompatibleConnect` and `TwiceDefined` arrived as a [`FlaggedStage`] —
/// under a note telling Claude the compile continued. `not_reached` is then empty.
pub struct PipelineFailure<'a> {
    /// Stage name, as `StageKind::name` reports it.
    pub stage: &'static str,
    /// The stage's note — the one-line summary already shown in the UI.
    pub note: &'a str,
    /// The stage's structured `error` payload, when it has one. This is where the
    /// diagnosis lives: counts, unmatched names, source locations, guidance.
    pub error: Option<&'a Value>,
    /// Stages after the failing one, which will all say "not reached". Named so Claude
    /// does not read their emptiness as a second problem.
    pub not_reached: Vec<&'static str>,
}

/// Write the focus file to `.hrw-bridge/focus.json`.
///
/// Called by the app whenever the user captures a node/stage/specimen.
/// Creates the bridge directory if it doesn't exist. Returns the path
/// on success (used by the app to show a status message).
pub fn write(ask: &Ask) -> std::io::Result<PathBuf> {
    let result = write_focus_file(ask);
    // **Every write of the context records the write, here, at the one choke point.**
    //
    // There are two context files and they are written by different mechanisms:
    // `focus.json` by this function, and `diagnostics/session.json` — the file the
    // `[hrw-context]` hook summarises above each of Doug's messages — only at the end of
    // a frame **in which an action was recorded**. Nothing tied the two together, so any
    // caller that changed the context without also recording a gesture left
    // `session.json` describing the previous context:
    //
    // - the Question Bar's two × buttons recorded nothing, so **the hook went on
    //   reporting a point Doug had cleared**;
    // - a recompile that kept the point re-emitted silently, so the hook's cross-check
    //   warned *"do NOT read it as Doug's context"* about a file that was correct.
    //
    // Doug, 2026-11-07: *"That is a bad problem, in the part of this project which
    // matters most."* Recording **inside the writer** rather than at each gesture makes
    // the two files unable to diverge: a future caller cannot write the context without
    // recording that it did. The hook's freshness cross-check compares `focus.json`
    // against *this* action — the event that actually wrote it — rather than against
    // whatever gesture came last.
    //
    // Recorded whether or not the write succeeded, because a failed write is a context
    // change too: the snapshot carries the emission error, and the hook must see it.
    crate::diagnostics::record_action(
        "context-emitted",
        match &result {
            Ok(_) => format!("focus.json seq {}", ask.seq),
            Err(e) => format!("focus.json seq {} NOT WRITTEN: {e}", ask.seq),
        },
    );
    result
}

/// A question Doug asked from the Question Bar.
pub struct Question<'a> {
    /// Its own sequence number, from the shared context counter.
    pub seq: u64,
    /// The question, as typed — trimmed, never empty.
    pub text: &'a str,
    // **No `this` field since 2026-11-07.** It carried the bar's resolution of "this"
    // (point, else follow, else stage). Doug removed the resolver: for him "this" often
    // means the whole assembled context, so Claude asks when it is unclear.
}

// "ANSWER FROM THE CONVERSATION" -- never "in the chat". Doug, 2026-11-07: "you are free to
// use this chat window, a VS code tab, System Modeler or Wolfram Desktop in whichever way
// you believe would be most effective." This text said "ANSWER IT IN THE CHAT" until then,
// and it is what Claude reads at the very moment of choosing how to answer.

/// The instructions `question.json` carries for its reader, which is Claude.
const QUESTION_INSTRUCTIONS: &str = "HRW question file. Doug typed this question into \
HRW's Question Bar; it waits here until he types ? in the Claude chat. ANSWER IT FROM THE \
CONVERSATION, NEVER IN HRW — using the chat, a VS Code tab, System Modeler or Wolfram \
Desktop, alone or combined, whichever will teach best; the choice is Claude's. Commit an \
answer to the repository only if Doug asks. If the question says \"this\" and what it \
refers to is not clear, ASK DOUG — HRW does not resolve it, and for him it often means \
the whole assembled context. `context` is the full focus document as it was AT THE MOMENT OF ASKING, written in the same \
call as the question — so it cannot disagree with it, unlike focus.json, which later clicks \
rewrite.";

/// Write a question **and the context it was asked about**, in one file, atomically.
///
/// # Why one file, and why not `focus.json`
///
/// Doug's questions used to be typed in the chat and joined to his context only by
/// *time*: Claude read whatever `focus.json` held when the hook ran. Every defect in the
/// Q&A path that week was a failure of that join. **Here the question and its context
/// leave HRW as one document**, so they cannot disagree.
///
/// **It is a separate file because `focus.json` is the LIVE context.** Every click
/// re-emits it, so a question stored there would be overwritten by the next point or
/// follow before Doug had typed `?`. `question.json` is written only when he asks, so it
/// holds the question as asked, beside the context as it was.
///
/// # What is atomic
///
/// The document is written to a temporary file and **renamed** over the old one, so a
/// reader never sees half a question — the hook may read at any moment. Like [`write`],
/// the attempt is recorded (`question-asked`) whether or not it succeeded, so
/// `session.json` moves in the same frame and the trail says a question was asked.
pub fn write_question(ask: &Ask, question: &Question) -> std::io::Result<PathBuf> {
    let result = write_question_file(ask, question);
    crate::diagnostics::record_action(
        "question-asked",
        match &result {
            Ok(_) => format!("question.json seq {}", question.seq),
            Err(e) => format!("question.json seq {} NOT WRITTEN: {e}", question.seq),
        },
    );
    result
}

/// The document [`write_question`] writes, without the file I/O, for tests.
pub fn build_question_for_test(ask: &Ask, question: &Question) -> Value {
    build_question(ask, question)
}

fn build_question(ask: &Ask, question: &Question) -> Value {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    json!({
        "instructions": QUESTION_INSTRUCTIONS,
        "seq": question.seq,
        "asked_at": crate::diagnostics::format_utc_millis(now_ms),
        "question": question.text,
        // The SAME document `write` puts in focus.json in this frame — built by the same
        // function from the same `Ask`, not copied from the file afterwards.
        "context": build(ask),
    })
}

fn write_question_file(ask: &Ask, question: &Question) -> std::io::Result<PathBuf> {
    let path = Path::new(QUESTION_FILE).to_path_buf();
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let text = serde_json::to_string_pretty(&build_question(ask, question)).unwrap_or_default();
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, text)?;
    // `rename` replaces an existing file on Windows as well as Unix, so the switch
    // from the old question to the new one is a single step.
    fs::rename(&tmp, &path)?;
    Ok(path)
}

/// The file I/O half of [`write`], split out so the recording cannot be bypassed by an
/// early `?` return — a failed `create_dir_all` must still be recorded.
fn write_focus_file(ask: &Ask) -> std::io::Result<PathBuf> {
    fs::create_dir_all(BRIDGE_DIR)?;
    let path = Path::new(BRIDGE_DIR).join("focus.json");
    let doc = build(ask);
    // Pretty-print for readability — Doug reads these during dogfooding.
    fs::write(
        &path,
        serde_json::to_string_pretty(&doc).unwrap_or_default(),
    )?;
    Ok(path)
}

/// What the extension said about the last breakpoint request.
///
/// **The ack used to be a `bool`, and that was the bug** (`docs/ideas.md` #75).
/// The extension wrote `{"acked": true}` unconditionally at the end of every
/// request — after arming nothing because every entry was a duplicate, after a
/// removal that matched nothing, after any request at all — and HRW consumed it
/// as `live_breakpoint_armed = acked`. The file answered *"I read your
/// request"*; HRW read it as *"a breakpoint exists"*.
///
/// Those differ, and `#74` made the gap routine rather than theoretical: after
/// the first Debug press of a session the anchor is already armed, so every
/// later press correctly arms nothing. One click of VS Code's *Disable All
/// Breakpoints* then produced nothing armed, nothing enabled, `acked: true`,
/// HRW announcing a stepped session, and the algorithm running to completion
/// with no stop and no notice — `#71`'s fiction, one layer down.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BreakpointAck {
    /// No ack file yet — the extension has not replied.
    Pending,
    /// An **enabled** breakpoint now exists at every requested line.
    Armed,
    /// The extension replied and nothing is in place. Carries its reason.
    NotArmed(String),
    /// The extension replied in the pre-`#75` format, which cannot say what it
    /// armed.
    ///
    /// **Treated as not-armed, and said out loud.** Guessing either way is
    /// wrong: assuming armed reinstates the fiction, and assuming not-armed
    /// silently breaks live trace against a stale build. Saying so converts the
    /// hazard that started 2026-08-08 — an extension twelve days behind its
    /// source, because a `git pull` runs no `tsc` — into a loud one at the first
    /// Debug press.
    Unreportable,
}

impl BreakpointAck {
    /// Whether HRW may claim a breakpoint is armed. **Only [`Self::Armed`].**
    #[must_use]
    pub fn is_armed(&self) -> bool {
        matches!(self, BreakpointAck::Armed)
    }

    /// Whether the extension replied at all — the handshake is over either way,
    /// so the caller stops waiting and spawns.
    #[must_use]
    pub fn replied(&self) -> bool {
        !matches!(self, BreakpointAck::Pending)
    }
}

/// Parse an ack payload. Split from the file read so it is testable without one.
///
/// **A payload without `breakpointPresent` is [`BreakpointAck::Unreportable`]**,
/// not a failure and not a success: that is exactly the old
/// `{"acked": true}`, and the whole point is that it cannot answer the question.
#[must_use]
pub fn parse_breakpoint_ack(text: &str) -> BreakpointAck {
    let Ok(v) = serde_json::from_str::<Value>(text) else {
        return BreakpointAck::NotArmed(format!(
            "the bridge wrote an ack that is not JSON ({} bytes)",
            text.len()
        ));
    };
    match v.get("breakpointPresent").and_then(Value::as_bool) {
        Some(true) => BreakpointAck::Armed,
        Some(false) => BreakpointAck::NotArmed(
            v.get("reason")
                .and_then(Value::as_str)
                .unwrap_or("the bridge did not say why")
                .to_owned(),
        ),
        None => BreakpointAck::Unreportable,
    }
}

/// Check whether the extension has acknowledged the last breakpoint request,
/// consuming the ack file if it exists.
pub fn check_breakpoint_ack() -> BreakpointAck {
    check_breakpoint_ack_at(std::path::Path::new(BREAKPOINT_ACK_FILE))
}

/// [`check_breakpoint_ack`] against an arbitrary path, so a test can exercise the
/// consume-and-delete behaviour without touching the live bridge directory.
pub(crate) fn check_breakpoint_ack_at(path: &Path) -> BreakpointAck {
    if !path.exists() {
        return BreakpointAck::Pending;
    }
    let text = fs::read_to_string(path).unwrap_or_default();
    let _ = fs::remove_file(path);
    parse_breakpoint_ack(&text)
}

/// Locate the breakpoint target inside `live_trace_breakpoint` and return the
/// source file's canonical path plus a 1-based line number.
///
/// ## Why this targets the body, not the signature
///
/// Debuggers skip a function's prologue: ask for a breakpoint on the `pub fn`
/// line and it resolves to the first *statement* instead (`exact_match = 0` in
/// LLDB's `breakpoint list`). That is correct behavior, but it means the bridge
/// and the debugger disagree about which line the breakpoint is on — so a
/// bridge-armed breakpoint and a hand-set one at the same place appear as two
/// separate entries in VS Code's breakpoint list, and the extension's duplicate
/// check does not recognize them as the same location.
///
/// Asking for the line the debugger will actually use removes the discrepancy.
///
/// The scan is deliberately structural rather than a hard-coded offset: find
/// the signature, advance to the line opening the body (which may be a later
/// line if the signature ever wraps), then take the first line that is neither
/// blank nor a comment.
pub(crate) fn find_live_trace_line() -> std::io::Result<(std::path::PathBuf, usize)> {
    let file = std::fs::canonicalize(LIVE_TRACE_FILE)?;
    // On Windows, canonicalize produces \\?\C:\... extended-length paths.
    // LLDB doesn't recognize that prefix, so strip it for breakpoint matching.
    #[cfg(windows)]
    let file = strip_windows_prefix(&file);
    let source = fs::read_to_string(&file)?;
    let lines: Vec<&str> = source.lines().collect();

    let not_found = |what: &str| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("live_trace_breakpoint: {what} not found in source"),
        )
    };

    let sig = lines
        .iter()
        .position(|l| l.contains("pub fn live_trace_breakpoint("))
        .ok_or_else(|| not_found("signature"))?;

    // The body opens on the signature line in the normal case; scanning forward
    // keeps this correct if the parameter list is ever wrapped across lines.
    let open = lines[sig..]
        .iter()
        .position(|l| l.contains('{'))
        .map(|offset| sig + offset)
        .ok_or_else(|| not_found("opening brace"))?;

    // First real statement after the brace. An empty body would fall through to
    // the closing `}` — which cannot happen, because an empty body is exactly
    // the bug `breakpoint_anchor_store_is_observable` exists to prevent.
    let stmt = lines[open + 1..]
        .iter()
        .position(|l| {
            let t = l.trim();
            !t.is_empty() && !t.starts_with("//")
        })
        .map(|offset| open + 1 + offset)
        .ok_or_else(|| not_found("body statement"))?;

    Ok((file, stmt + 1))
}

/// Arm a breakpoint on `live_trace_breakpoint` for live algorithm stepping.
///
/// Writes a breakpoint request to `.hrw-bridge/breakpoint-request.json`.
/// The VS Code extension processes it, registers the breakpoint with LLDB,
/// and writes an ack file. The caller should poll `check_breakpoint_ack()`
/// before spawning the algorithm thread.
///
/// Clears any stale ack file first so that only a fresh ack from *this*
/// request triggers the spawn.
///
/// # Returns the request it wrote, and that is not a convenience
///
/// **The file cannot be read back, because it does not stay there.** `.hrw-bridge/`
/// is watched by a live VS Code extension that *consumes* a request — reads it,
/// acts, `unlinkSync`s it, writes an ack — so any code verifying what was written
/// by re-opening the path is racing a filesystem watcher on a file it does not own.
///
/// The test did exactly that and never failed, because its read landed microseconds
/// after its write. That is a flake with a long fuse, not a passing test, and the
/// fuse is lit only on a machine where the extension is installed. Returning the
/// value removes the race rather than narrowing it: the same move as
/// `Plot::problems()` and `IncidenceMatrix::problems()` — **push the checkable data
/// out of the I/O path instead of testing through it.**
///
/// Both in-app call sites ignore the value, so this is additive.
pub fn arm_live_trace_breakpoint(specimen: Option<&str>) -> std::io::Result<serde_json::Value> {
    arm_live_trace_breakpoint_to(Path::new(BREAKPOINT_REQUEST_FILE), specimen)
}

/// [`arm_live_trace_breakpoint`], writing to `request_path`.
///
/// # This exists so that NO TEST EVER ARMS A REAL BREAKPOINT
///
/// Doug, 2026-08-15, after a first fix that was not enough: *"There's still a
/// live_trace.rs breakpoint set."* Writing an arm request to
/// `BREAKPOINT_REQUEST_FILE` is not a simulation — the extension is watching that
/// exact path and puts a breakpoint in his editor. A guard that removes it
/// afterwards was the wrong shape of answer, because **the extension declines to
/// remove a breakpoint it did not add**: an arm landing on a location that already
/// has one is reported `alreadyEnabled` and never enters `armedBreakpoints`, which
/// is the only list either removal path consults. One reload of the VS Code window
/// resets that list, and any breakpoint HRW armed before it becomes permanently
/// unremovable — by HRW, and by the extension's own Clear command.
///
/// So the fix is not to clean up more reliably. It is to **never create it**:
/// every test writes to a temp path, and
/// `no_test_arms_a_breakpoint_on_the_watched_path` fails if one stops doing so.
///
/// The ack is cleared *beside the request*, not at `BREAKPOINT_ACK_FILE`, so a
/// test cannot delete an ack the extension just wrote for the running app.
pub(crate) fn arm_live_trace_breakpoint_to(
    request_path: &Path,
    specimen: Option<&str>,
) -> std::io::Result<serde_json::Value> {
    let _ = fs::remove_file(request_path.with_file_name("breakpoint-ack.json"));
    let (file, line) = find_live_trace_line()?;
    let path_str = file.display().to_string();
    let mut request = json!({
        "version": 1,
        "breakpoints": [{ "path": path_str, "line": line }]
    });
    if let Some(s) = specimen {
        request["specimen"] = json!(s);
    }
    write_breakpoint_request_to(request_path, &request)?;
    Ok(request)
}

/// Serialize a breakpoint request to `path`.
///
/// Split out of the two public functions so that **one test can prove the file
/// receives exactly the value they return**, which is the claim their new return
/// type rests on and which cannot be checked at
/// `BREAKPOINT_REQUEST_FILE` — the extension deletes that one. Pointed at a temp
/// path, this is an ordinary round-trip test with no second process in it.
///
/// Without this seam the return value would be trusted rather than verified: today
/// the same `request` is both written and returned, but nothing would fail if a
/// later edit wrote a modified copy.
fn write_breakpoint_request_to(path: &Path, request: &serde_json::Value) -> std::io::Result<()> {
    let text = serde_json::to_string_pretty(request)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    fs::write(path, text)
}

/// Remove the `live_trace_breakpoint` breakpoint.
///
/// **Not called when a live session ends**, which is the opposite of what it
/// was written for. It existed to prevent a SIGSTOP signal when the algorithm
/// thread exited under LLDB; that call site is **deleted** (`docs/ideas.md`
/// #74). `cppvsdbg` will not re-bind a location whose breakpoint left its
/// active set within the same debug session, so releasing the anchor at the end
/// of a run made the *next* Debug press arm a breakpoint that never bound.
///
/// It remains in use for the three events that end the breakpoint's reason to
/// exist: a session that failed to spawn, a specimen change, and app exit.
///
/// Returns the request it wrote, for the same reason as
/// [`arm_live_trace_breakpoint`] — see that function's docs.
pub fn remove_live_trace_breakpoint() -> std::io::Result<serde_json::Value> {
    remove_live_trace_breakpoint_to(Path::new(BREAKPOINT_REQUEST_FILE))
}

/// [`remove_live_trace_breakpoint`], writing to `request_path`.
///
/// Exists for the same reason as [`arm_live_trace_breakpoint_to`], though the
/// stakes are lower: a removal request cannot *create* a breakpoint. What it
/// avoids is the read-back race — the extension consumes a request and unlinks it,
/// so a test that reopens the path to check what it wrote is racing a watcher.
pub(crate) fn remove_live_trace_breakpoint_to(
    request_path: &Path,
) -> std::io::Result<serde_json::Value> {
    let (file, line) = find_live_trace_line()?;
    let path_str = file.display().to_string();
    let request = json!({
        "version": 1,
        "action": "remove",
        "breakpoints": [{ "path": path_str, "line": line }]
    });
    write_breakpoint_request_to(request_path, &request)?;
    Ok(request)
}

/// Write each stage's full IR to `.hrw-bridge/stages/<name>.json`.
///
/// Called once per compile (not per capture). Each stage's entire IR is
/// written as a separate file so Claude can diff any two stages by reading
/// both files. A stage with no IR (e.g., it failed or doesn't apply) has
/// its file removed, keeping the directory in sync with the current specimen.
/// `.hrw-bridge/view.json` — **what the pane on screen is currently showing.**
#[cfg(not(test))]
pub const VIEW_FILE: &str = crate::manifest_path!(".hrw-bridge", "view.json");
/// The published view under `cargo test` — see [`BRIDGE_DIR`].
#[cfg(test)]
pub const VIEW_FILE: &str = crate::manifest_path!(".hrw-bridge", "test-run", "view.json");

/// Publish the current view's content, or remove the file when there is none.
///
/// # Why one file, rewritten, rather than one per view
///
/// Doug, 2026-08-13, arguing for an MCP server instead: *"HRW having to write out
/// lengthy files which are usually ignored"*. He is right about the cost — the bridge
/// already writes **1.5 MB per compile** for a four-component circuit, and publishing
/// every view on every compile would add to a path that `docs/architecture.md` §11
/// already measures as HRW's scaling bottleneck.
///
/// So this writes **only the view the reader is actually looking at**, at the moment it
/// changes. One small file, always current, nothing speculative. That removes the waste
/// the pull-based design was proposed to solve, without a server that would not travel
/// between machines and would not exist headlessly.
///
/// # Absence is stated, never stale
///
/// A view with no publisher **removes** the file rather than leaving the previous view's
/// content in place. A stale `view.json` describing a pane the reader left is precisely
/// the "confidently wrong" failure this repository spends most of its rules on — and it
/// would be indistinguishable from a current one by content alone. `kind` names the view
/// so a reader can tell what they are holding.
pub fn write_view(kind: Option<&str>, body: Option<&Value>) -> std::io::Result<()> {
    write_view_to(Path::new(VIEW_FILE), kind, body)
}

/// [`write_view`] against a caller-chosen path.
///
/// **Split out by the 2026-08-15 sweep so the absence behaviour could be tested.**
/// `write_view`'s whole safety property is that a view with no publisher *removes* the
/// file rather than leaving the previous pane's contents — and that was asserted in a
/// doc comment and checked nowhere. A test could not use `write_view` directly without
/// clobbering the running app's live `view.json`, and `VIEW_FILE` is a single constant
/// path, so there was no test-safe name to use the way `write_stages`' test uses
/// `test_alpha`.
fn write_view_to(path: &Path, kind: Option<&str>, body: Option<&Value>) -> std::io::Result<()> {
    match (kind, body) {
        (Some(kind), Some(body)) => {
            // The file's own directory, not `BRIDGE_DIR` — otherwise a caller-chosen
            // path outside the bridge silently fails to have its parent created, and
            // the only caller that does that is the test proving this function works.
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            let doc = serde_json::json!({
                "note": "What the HRW pane on screen is currently showing. Written for \
                         Claude. This is the RENDERER'S INPUT, serialized -- not a \
                         description of what was drawn. Every row carries an `id` that \
                         names the same object across views.",
                "view": kind,
                "content": body,
            });
            fs::write(path, serde_json::to_string_pretty(&doc).unwrap_or_default())
        }
        _ => {
            let _ = fs::remove_file(path);
            Ok(())
        }
    }
}

pub fn write_stages(stages: &[(&str, Option<&Value>)]) -> std::io::Result<()> {
    fs::create_dir_all(STAGES_DIR)?;
    for (name, value) in stages {
        let path = Path::new(STAGES_DIR).join(format!("{name}.json"));
        match value {
            Some(v) => fs::write(&path, serde_json::to_string_pretty(v).unwrap_or_default())?,
            None => {
                let _ = fs::remove_file(&path);
            }
        }
    }
    Ok(())
}

// Build the complete focus JSON document from an `Ask`.
//
// The document structure:
// {
//   "instructions": <self-describing text>,
//   "seq": <monotonic counter>,
//   "request": "debug-where-set" | null  (null: no gesture DECLARED a verb — the verb is
//              in the chat message; this is the ordinary case, point or no point),
//   "kind": "node" | "stage" | "specimen",
//   "specimen": <path to .mo file>,
//   "model": <class name>,
//   "stage": <pipeline stage name>,
//   "libraries": [<library paths>],
//   "def_resolutions": { "<id>": { "name": ..., "kind": ... }, ... },
//   "stages": { "dir": ..., "files": [...] },
//   "view": { ... },          // what HRW was showing (mode, sub-view, animation frame)
//   "phase_source": { ... },  // where this stage's algorithm lives in Rumoca
//   "node": { ... }           // only for kind=node
//   "cross_stage": { ... }    // only for kind=node
//   "tracking": { ... }       // only when following an identifier
// }
/// Build the focus document without writing it, for tests outside this module.
///
/// `app.rs` owns the Question Bar and therefore owns "what happens when the point
/// is cleared", so that behaviour is tested there — but the assertion worth
/// making is about the *emitted document*, not the app field. This exposes the
/// document without the file I/O.
pub fn build_for_test(ask: &Ask) -> Value {
    build(ask)
}

fn build(ask: &Ask) -> Value {
    let kind = match ask.focus {
        Focus::Node { .. } => "node",
        Focus::Stage => "stage",
        Focus::Specimen => "specimen",
        Focus::Nothing => "none",
        // **Two kinds out of one variant, because `in_source` changes what the
        // capture IS.** Emitting the prose kind for a pane selection put two
        // contradictory claims in one document: this field said prose while the
        // section below said the text is in no lab. That is the identical defect the
        // stage sentinel below records having fixed once already.
        Focus::Selection {
            in_source: true, ..
        } => "selection",
        Focus::Selection { .. } => "selection",
    };
    // **The slug, not the display name.** This used to emit `StageKind::name`, which
    // reads "Index reduction" with a space — so the capture named a stage that
    // `hrw://stage/<X>` could not parse, for two of the eleven stages. The capture is
    // read by Claude in order to *act*, so the actionable form wins; `stage_display`
    // carries the prose label alongside it.
    // **`None` needs a REASON, not one spelling, since 2026-08-31.** There had been a
    // single sentinel — *"(navigated definition)"* — because that was the only way a
    // capture could lack a stage. A lab passage is now the second, and it emitted the
    // first one's text: `kind: "selection"` beside `stage: "(navigated definition)"`,
    // two contradictory claims about one capture. Found by a column read of the arms
    // added the day before, which is the tool's own advertised yield: a list of siblings
    // where one member is wrong.
    //
    // Absence stated, and stated *accurately* — the same rule that made the field an
    // `Option` rather than letting it borrow whichever stage was selected.
    let no_stage = match ask.focus {
        Focus::Selection {
            in_source: true, ..
        } => "(lab prose, not a compile phase)",
        // **A third reason, added 2026-09-22.** The text was selected from a pane, so
        // it was almost certainly rendered *from* some stage — but the capture does not
        // record which, and `view.stage_view` names what was on screen rather than what
        // the selection came from. Saying so beats borrowing a stage that was never
        // checked, which is the rule this field exists to enforce.
        // **Defers to `from`, which names the region exactly.** It said "a selection from a
        // pane" until 2026-09-22, which was wrong for the first real capture made this way:
        // the text came from Claude's answer document, which is not a pane.
        Focus::Selection { .. } => "(not a compile phase \u{2014} see `from` for the region)",
        _ => "(navigated definition)",
    };
    let stage_str = ask.stage.map_or(no_stage, StageKind::slug);
    let stage_display = ask.stage.map_or(no_stage, StageKind::name);
    let mut doc = json!({
        "instructions": INSTRUCTIONS,
        "seq": ask.seq,
        // **A declared verb, or null — never a default.** This comment used to say
        // that *"emitting a default `"explain"` would claim an intent the user never
        // expressed"* — and then applied that only when there was no point, emitting
        // `"explain"` for every point anyway. The principle was right and drawn at the
        // wrong boundary: a point is a NOUN, and pointing does not declare a verb.
        // Fixed 2026-11-07 on Doug's ruling; see [`AskRequest`].
        //
        // Null rather than omitted, for the same reason `kind: "none"` beats a missing
        // `kind`: absence stated, not implied. **Null here means "read the verb from the
        // chat message"**, which the instructions above now say.
        "request": ask
            .request
            .map_or(Value::Null, |r| Value::String(r.as_str().to_owned())),
        "kind": kind,
        "specimen": ask.specimen.map(|p| p.to_string_lossy().into_owned()),
        "model": ask.model,
        "stage": stage_str,
        "stage_display": stage_display,
        "libraries": ask.libraries,
        "def_resolutions": def_resolutions(ask.def_index),
        "view": ask.view.to_json(),
        "phase_source": ask.stage.map(phase_source),
        "stages": {
            "dir": STAGES_DIR,
            "note": "each <name>.json is that stage's FULL IR for the current specimen \
                     (absent if the stage produced none). To diff two stages, read the two \
                     files and compare — e.g. instantiate.json vs typecheck.json shows what the \
                     instanced typecheck added (type_ids resolved, dimensions evaluated).",
            "files": STAGE_FILE_NAMES,
        },
    });
    // A failure outranks everything else in the file: if the pipeline broke, that is
    // the answer to almost any question about this specimen. Emitted before the
    // point-specific sections for that reason.
    if let Some(f) = &ask.failure {
        doc["pipeline_failure"] = json!({
            "note": "the stage that STOPPED the pipeline: every stage after it says it \
                     never ran (an empty list means it was the last compilation stage, \
                     and the model cannot be simulated). `error` carries the diagnosis — counts, unmatched names, \
                     source locations, guidance. Full IR for every stage is under \
                     `stages.dir`. A stage that reported a problem and was CARRIED PAST \
                     is not here; see `pipeline_flags`.",
            "stage": f.stage,
            "summary": f.note,
            "error": f.error,
            "downstream_not_reached": f.not_reached,
        });
    }
    // **Emitted separately from a failure, and both may appear.** A flag before a real
    // stop was reported as the stop itself until 2026-11-05, which was wrong twice over:
    // it named the wrong stage and it called a recovery a failure.
    if !ask.flags.is_empty() {
        doc["pipeline_flags"] = json!({
            "note": "stages where Rumoca reported a problem and the compile CONTINUED — \
                     every later stage ran. These are not failures and usually not bugs: \
                     Rumoca is a recovering compiler, and a structurally singular report \
                     here is the normal, expected state of a high-index model that a \
                     later phase goes on to reduce and simulate. Do not present one as a \
                     reason the model does not work without checking that it does not.",
            "stages": ask.flags.iter().map(|f| json!({
                "stage": f.stage,
                "summary": f.note,
                "error": f.error,
            })).collect::<Vec<_>>(),
        });
    }
    if let Focus::Node {
        key_path,
        stage_value,
    } = &ask.focus
    {
        doc["node"] = build_node(key_path, stage_value, ask.specimen);
        doc["cross_stage"] = build_cross_stage(ask, key_path);
    }
    // **A lab passage carries its own section**, because none of the IR machinery
    // above applies: there is no stage IR to navigate, no key path, and no cross-stage
    // diff to build. The note says what the text IS, so a reader does not go looking
    // for the passage byte-for-byte in the markdown and conclude the capture is wrong.
    if let Focus::Selection {
        origin,
        text,
        in_source,
    } = &ask.focus
    {
        // **The file is the origin's to name, and only lab prose has one.** This used to be
        // `format!("hrw/docs/fixture-labs/{lab}.md")` from whichever lab was open, which
        // produced a path for Claude's answer document (`✨ Answer`) that has never existed,
        // and a path for a stage-pane selection that does not contain the text.
        doc["selection"] = match (origin.source_file(), in_source) {
            (Some(file), true) => json!({
                "note": "prose the reader selected in HRW's lab panel and pointed at. This is \
                         the RENDERED text, so markdown markup is stripped and it will not \
                         match the source byte-for-byte -- locate it in the file named below, \
                         which HRW verified does contain it. Lab prose carries no provenance \
                         markers: nothing records who wrote a passage or whether Doug has run \
                         it (retired 2026-08-31), so judge that from the conversation, as \
                         docs/fixture-labs/README.md says. Change the file only when the \
                         request asks for a change.",
                "from": origin.describe(),
                "file": file,
                "text": text,
            }),
            (Some(file), false) => json!({
                "note": "the reader pointed at text in a lab's panel, and HRW could NOT find \
                         it in that lab's source. Either the prose has changed since, or the \
                         selection came from something else drawn in the same panel. Treat the \
                         file below as a lead, not a location, and do not edit it to match.",
                "from": origin.describe(),
                "file_searched": file,
                "text": text,
            }),
            (None, _) => json!({
                // **Rewritten 2026-11-07, for two false claims.** It said the text was "NOT
                // in any repository file" and that "there is no file it is in" — but this
                // arm also serves `SpecimenSource` and `SpecimenPurpose`, whose text comes
                // from the specimen's `.mo` file and its `purpose.md`. Their variants'
                // docs say why no file is *named*: HRW has not verified the text is in
                // it, not that no file exists. It also listed "Claude's own answer
                // document", which left HRW that day with the Answer (charter Decision 19).
                "note": "text the reader pointed at, for which HRW names no file -- `from` \
                         says which region drew it. Compiler output and the other stage \
                         panes, the specimen list and the compile log are in no file: **do \
                         not go looking for them in the markdown, and do not edit a lab to \
                         make them match.** The specimen's Modelica source and its purpose \
                         note do come from files (the capture's `specimen`, and the model's \
                         purpose.md under hrw/docs/specimen-notebook/), but HRW has not \
                         checked that the text is in them, so treat either as a lead, not a \
                         location.",
                "from": origin.describe(),
                "text": text,
            }),
        };
    }
    // The ambient half. A sibling section rather than nested, so the point and
    // the thread are structurally separate and neither can be mistaken for the
    // other — see `Tracking` for why that distinction is load-bearing.
    if let Some(t) = &ask.tracking {
        doc["tracking"] = build_tracking(t);
    }
    doc
}

/// Limit on scalar changes reported in a cross-stage diff.
///
/// Kept at 400 after review, and the reasoning is different from the other two
/// caps. Those bound *how much surrounding IR to carry*, where more is better
/// until it stops being readable. This bounds a **list of differences**, and a
/// cross-stage diff that runs to hundreds of entries is no longer telling a
/// reader what the phase did to the node — it is saying the node was rebuilt.
/// Real diffs are a handful of `null -> id` fields; the interesting signal is
/// gone long before 400. This stays a backstop, not a sample.
const MAX_CHANGES: usize = 400;

// Build the cross-stage diff for a captured node.
//
// This answers "what did Resolve do to this node?" by showing the SAME node
// as it appears in Parse and in Resolve, plus a list of scalar field changes.
//
// ## Class-relative path alignment
//
// Parse and Resolve have different root structures:
// - Parse wraps the class in `{ "classes": { "M": { ... } }, "within": ... }`
// - Resolve extracts the class directly: `{ "def_id": ..., "components": ... }`
//
// To find the same node in both stages, we strip the class prefix: if the
// clicked path starts with `classes.M.`, we drop those two segments to get a
// class-relative path, then navigate from each stage's class subtree.
// `class_subtree()` handles the detection: it returns the class subtree and
// the prefix depth (2 for Parse's wrapped form, 0 for Resolve's flat form).
fn build_cross_stage(ask: &Ask, key_path: &[Seg]) -> Value {
    let Some(model) = ask.model else {
        return json!({ "applicable": false, "reason": "no model name" });
    };
    let current = match ask.stage {
        Some(StageKind::Parse) => ask.parse_value,
        Some(StageKind::Resolve) => ask.resolve_value,
        _ => None,
    };
    let Some(current) = current else {
        return json!({ "applicable": false, "reason": "cross-stage diff not yet implemented for this stage" });
    };

    // Strip the current stage's class prefix to get the class-relative path.
    let (_, cur_depth) = class_subtree(current, model);
    let rel: &[Seg] = if cur_depth == 0 {
        key_path
    } else if key_path.len() >= cur_depth
        && matches!(&key_path[0], Seg::Key(k) if k == "classes")
        && matches!(&key_path[1], Seg::Key(k) if k == model)
    {
        &key_path[cur_depth..]
    } else {
        return json!({ "applicable": false, "reason": "node is outside the model class" });
    };

    let stage_node = |value: Option<&Value>| -> Value {
        match value {
            Some(v) => {
                let (class, _) = class_subtree(v, model);
                match navigate(class, rel) {
                    Some(n) => {
                        let mut out = capped(n);
                        out["found"] = json!(true);
                        out
                    }
                    None => json!({ "found": false }),
                }
            }
            None => json!({ "found": false, "reason": "stage not available" }),
        }
    };

    let parse_node = stage_node(ask.parse_value);
    let resolve_node = stage_node(ask.resolve_value);

    // Scalar deltas, only when both nodes are present in full.
    let mut changes = Vec::new();
    if let (Some(p), Some(r)) = (parse_node.get("value"), resolve_node.get("value")) {
        let mut path = Vec::new();
        diff(p, r, &mut path, &mut changes);
    }

    json!({
        "applicable": true,
        "note": "the same node before (parse) and after (resolve) name resolution; `changes` lists scalar field deltas",
        "class_relative_path": rel.iter().map(Seg::as_json).collect::<Vec<_>>(),
        "parse": parse_node,
        "resolve": resolve_node,
        "changes": changes,
    })
}

// Find the class subtree within a stage's IR.
//
// Parse wraps the class in a `StoredDefinition`: `{ "classes": { "M": { ... } } }`.
// Resolve extracts the class directly. This function detects which form is
// present and returns (subtree, prefix_depth):
// - Parse: returns (`classes.M` subtree, 2)
// - Resolve: returns (root, 0)
//
// The prefix_depth tells the caller how many segments to strip from the
// clicked path to get a class-relative path.
fn class_subtree<'a>(stage_value: &'a Value, model: &str) -> (&'a Value, usize) {
    if let Some(class) = stage_value.get("classes").and_then(|c| c.get(model)) {
        return (class, 2);
    }
    (stage_value, 0)
}

// Wrap a node for inclusion in the focus file.
// Small nodes (< MAX_NODE_BYTES) are inlined as `{ "value": <node> }`.
// Large nodes are truncated to a shape summary: `{ "truncated": true, "bytes": N, "shape": [...] }`.
// Claude can always get the full node from the staged IR file if needed.
fn capped(node: &Value) -> Value {
    let bytes = serde_json::to_string(node).map(|s| s.len()).unwrap_or(0);
    if bytes <= MAX_NODE_BYTES {
        json!({ "value": node })
    } else {
        json!({ "truncated": true, "bytes": bytes, "shape": shape(node) })
    }
}

/// Byte budget for one *enclosing context* block.
///
/// Not a tidiness limit. It bounds how much surrounding IR one address is worth
/// carrying, and it is spent greedily: [`enclosing_context`] hands back the
/// **largest** ancestor that fits, so a small leaf in a small object yields the
/// whole equation while a leaf in a 988-entry map yields only its immediate
/// parent. 8 KiB is roughly one flattened equation with all its spans — the
/// unit at which IR stops being self-explanatory and needs the stage file
/// anyway.
const MAX_CONTEXT_BYTES: usize = 8 * 1024;

/// How many sibling names to show around a hit, on each side.
///
/// A *window centred on the hit*, not the first N — the two are very different.
/// Following `__pre__.overSpeed` into Solve lowering lands in a map of 988
/// bindings, and the finding worth having is that `__pre__.c`, `__pre__.load.w`
/// and `__pre__.maxSpeed` sit immediately beside it (the phase makes a
/// pre-companion for everything the event logic samples). The first 40 keys of
/// that map would have said nothing. Position is an exact fact about the IR, so
/// this stays inside the "emit facts, not interpretation" rule.
const SIBLING_WINDOW: usize = 12;

/// The IR *around* an address, not just the value at it.
///
/// A path alone forces the reader to go open the stage file, and — worse —
/// forces them to already know what to look for. Following `__pre__.overSpeed`
/// produced the path
/// `discrete_updates.valued_updates_f_m[0].rhs.If.else_branch.VarRef.name.name`,
/// and the decisive fact about that mention was `generated: true` on the object
/// one level up: the variable is *manufactured by the Events phase*, which is
/// the entire explanation. Nothing in the path says so. Four separate reads of
/// `events.json` are what turned it up, and only because the reader thought to
/// look.
///
/// So this returns the enclosing IR, the position among siblings, and the value
/// itself — the three things that were reconstructed by hand.
fn neighbourhood(root: &Value, path: &[Seg]) -> Value {
    let (depth, context) = enclosing_context(root, path);
    json!({
        "path": describe_path(path),
        "value": navigate(root, path),
        // How far up the returned context sits. 0 is the stage root; equal to
        // the path length means the value stood alone in its budget.
        "context_at_depth": depth,
        "context_path": describe_path(&path[..depth]),
        "context": context,
        "siblings": siblings(root, path),
    })
}

/// The largest ancestor of `path` that fits in [`MAX_CONTEXT_BYTES`].
///
/// Walks *up* from the addressed value. Ancestors only grow, so the first one
/// that overflows ends the search and the previous one is the answer. Returns
/// `(depth, value)` where `depth` indexes into `path`.
///
/// Spending the budget greedily is the point: it adapts to the IR instead of
/// fixing an arbitrary number of levels. One level up from a leaf inside a huge
/// map is all that fits; from a leaf inside a small equation, the whole equation
/// comes along. Neither case needs a rule of its own.
fn enclosing_context(root: &Value, path: &[Seg]) -> (usize, Value) {
    let mut best = (path.len(), Value::Null);
    for depth in (0..=path.len()).rev() {
        let Some(node) = navigate(root, &path[..depth]) else {
            continue;
        };
        let bytes = serde_json::to_string(node)
            .map(|s| s.len())
            .unwrap_or(usize::MAX);
        if bytes > MAX_CONTEXT_BYTES {
            break;
        }
        best = (depth, node.clone());
    }
    best
}

/// What sits beside the addressed value in its parent.
///
/// `count` is exact; `window` is a positional sample around the hit. For an
/// array element the window is the neighbouring indices; for an object field it
/// is the neighbouring keys in IR order. Returns `Null` at the root, which has
/// no parent and therefore no siblings — a fact, not a missing value.
fn siblings(root: &Value, path: &[Seg]) -> Value {
    let Some((last, parent_path)) = path.split_last() else {
        return Value::Null;
    };
    let Some(parent) = navigate(root, parent_path) else {
        return Value::Null;
    };

    let (count, position, names): (usize, Option<usize>, Vec<String>) = match parent {
        Value::Object(map) => {
            let keys: Vec<&String> = map.keys().collect();
            let Seg::Key(k) = last else {
                return Value::Null;
            };
            let at = keys.iter().position(|key| *key == k);
            (keys.len(), at, keys.iter().map(|k| (*k).clone()).collect())
        }
        Value::Array(arr) => {
            let Seg::Index(i) = last else {
                return Value::Null;
            };
            (
                arr.len(),
                Some(*i),
                (0..arr.len()).map(|n| format!("[{n}]")).collect(),
            )
        }
        // A scalar has no siblings. Reaching here means the path addressed
        // something inside a scalar, which cannot happen via `navigate`.
        _ => return Value::Null,
    };

    let centre = position.unwrap_or(0);
    let lo = centre.saturating_sub(SIBLING_WINDOW);
    let hi = (centre + SIBLING_WINDOW + 1).min(names.len());
    json!({
        "count": count,
        "position": position,
        "window": &names[lo..hi],
        "window_is_complete": lo == 0 && hi == names.len(),
        "note": "names adjacent to this one in IR order, centred on it. \
                 Adjacency is often the finding: a manufactured companion \
                 variable sits beside the variable it shadows.",
    })
}

// Recursively diff two JSON subtrees, collecting scalar-level changes.
//
// Each change is a `{ "path": "...", "parse": <old>, "resolve": <new> }` record.
// The diff is structural:
// - Objects: diff by key (report added/removed/changed keys)
// - Arrays: diff by index (element-wise comparison)
// - Scalars: report if they differ
//
// `path` is a mutable stack of path segments (same push/pop pattern as tree.rs).
// `out` collects the change records, capped at MAX_CHANGES.
fn diff(a: &Value, b: &Value, path: &mut Vec<String>, out: &mut Vec<Value>) {
    if out.len() >= MAX_CHANGES {
        return;
    }
    match (a, b) {
        (Value::Object(ma), Value::Object(mb)) => {
            for (k, va) in ma {
                path.push(k.clone());
                match mb.get(k) {
                    Some(vb) => diff(va, vb, path, out),
                    None => {
                        out.push(json!({ "path": path.join("."), "parse": va, "resolve": null }))
                    }
                }
                path.pop();
            }
            for (k, vb) in mb {
                if !ma.contains_key(k) {
                    path.push(k.clone());
                    out.push(json!({ "path": path.join("."), "parse": null, "resolve": vb }));
                    path.pop();
                }
            }
        }
        (Value::Array(aa), Value::Array(ab)) => {
            for i in 0..aa.len().max(ab.len()) {
                path.push(format!("[{i}]"));
                match (aa.get(i), ab.get(i)) {
                    (Some(x), Some(y)) => diff(x, y, path, out),
                    (x, y) => out.push(json!({
                        "path": path.join("."),
                        "parse": x.cloned().unwrap_or(Value::Null),
                        "resolve": y.cloned().unwrap_or(Value::Null),
                    })),
                }
                path.pop();
            }
        }
        _ if a != b => out.push(json!({ "path": path.join("."), "parse": a, "resolve": b })),
        _ => {}
    }
}

// Convert the DefId -> DefInfo lookup table to a JSON object.
// Keys are stringified numeric ids (JSON object keys must be strings).
// This table is included in every focus file so that when Claude sees
// `type_def_id: 27579`, it can immediately look up that 27579 = "model
// Modelica.Mechanics.Rotational.Inertia" without needing to re-run the resolver.
fn def_resolutions(index: &BTreeMap<u64, DefInfo>) -> Value {
    let mut map = serde_json::Map::new();
    for (id, info) in index {
        map.insert(id.to_string(), info.to_json());
    }
    Value::Object(map)
}

// Build the `node` section of the focus file.
//
// Contains:
// - `key_path`: the machine-readable path segments (for Claude to navigate)
// - `subtree`: the node's value (inlined if small, shape if large)
// - `provenance`: the source-code excerpt this node came from (via span-ascent)
fn build_node(key_path: &[Seg], root: &Value, specimen: Option<&Path>) -> Value {
    let key_path_json: Vec<Value> = key_path.iter().map(Seg::as_json).collect();

    let subtree = match navigate(root, key_path) {
        Some(node) => capped(node),
        None => Value::Null,
    };

    json!({
        "key_path": key_path_json,
        "subtree": subtree,
        // What the node sits in. A captured scalar is often uninterpretable
        // alone: pointing at `def_id: 85` gives an integer and a path, and
        // answering "the def_id of *what*?" meant reconstructing the parent by
        // hand. `neighbourhood` carries the enclosing object and the node's
        // position among its siblings, so the subject arrives whole.
        "neighbourhood": neighbourhood(root, key_path),
        "provenance": ascend_provenance(root, key_path, specimen),
    })
}

/// Whether a key-path still addresses something in `root`.
///
/// Used after a recompile to decide whether a retained point survived. A path
/// that no longer resolves must not be kept: the Question Bar would name a node
/// that does not exist, and the emitted `node.subtree` would be `null` — a
/// confident claim about nothing, which is the failure this design keeps
/// eliminating.
pub fn node_exists(root: &Value, path: &[Seg]) -> bool {
    navigate(root, path).is_some()
}

// Navigate a key-path from the root to a specific node.
//
// Returns `None` if any segment in the path doesn't exist (e.g., the key
// is missing or the index is out of bounds). This is the inverse of the
// path-accumulation done during tree traversal.
/// Walk `path` from `root`, or `None` if any segment is missing.
///
/// `pub` since 2026-07-29 so the app can tell whether a `hrw://…/node/<path>` link
/// actually resolves before honouring it — see `App::validate_jump_target`.
pub fn navigate<'a>(root: &'a Value, path: &[Seg]) -> Option<&'a Value> {
    let mut cur = root;
    for seg in path {
        cur = seg.get(cur)?;
    }
    Some(cur)
}

// The span-ascent algorithm: run from the clicked node up to the root,
// returning the tightest (deepest) enclosing `location` or `span`.
//
// Why "ascent"? The clicked leaf usually has no provenance of its own.
// For example, clicking `"name": "flange_a"` gives a bare string — no
// byte offsets. But its parent (a component object) carries a `location`
// with byte offsets into the Modelica source. By running up the path from
// leaf to root, we find the nearest ancestor that has provenance.
//
// The loop iterates `depth` from `path.len()` (the leaf) down to 0 (the root).
// At each depth, it navigates to `path[..depth]` and checks for a `location`
// or `span` field. `location` is still preferred, but **not for the reason this
// comment used to give**: it said `location` "includes a `file_name`" while
// `span` has "an opaque `source` id instead of a filename". On v0.10.0
// `Location` carries `source: SourceId` too, so neither can name its own file
// and both rely on the specimen path. `location` is preferred now because it
// additionally carries line and column.
fn ascend_provenance(root: &Value, path: &[Seg], specimen: Option<&Path>) -> Value {
    for depth in (0..=path.len()).rev() {
        let Some(Value::Object(map)) = navigate(root, &path[..depth]) else {
            continue;
        };
        if let Some(loc) = map.get("location").filter(|v| is_location(v)) {
            return provenance(loc, "location", depth, specimen);
        }
        if let Some(span) = map.get("span").filter(|v| is_span(v)) {
            return provenance(span, "span", depth, specimen);
        }
    }
    json!({ "found": false, "note": "no location/span on this node or its ancestors" })
}

// Check if a JSON value is a `rumoca_core::Location`.
//
// **`file_name` is gone on v0.10.0**, and with it the one advantage `Location`
// had over `Span`: it identified the source by filename, so an excerpt could be
// read without being told which file to look in. `Location` now carries
// `source: SourceId`, the same opaque id `Span` does, so **both** provenance
// types need the specimen path handed to them. The old comment claiming
// `location` is "preferred (it includes a `file_name`)" stopped being true.
//
// It is still worth distinguishing, for a different reason: a `Location` also
// carries line and column, which a `Span` does not, and those are what the
// distinction now rests on. Testing them rather than `file_name` is also what
// keeps this from matching a `Span` by accident, since the two otherwise agree
// on every field this checks.
fn is_location(v: &Value) -> bool {
    v.get("start").and_then(Value::as_u64).is_some()
        && v.get("end").and_then(Value::as_u64).is_some()
        && v.get("start_line").and_then(Value::as_u64).is_some()
        && v.get("start_column").and_then(Value::as_u64).is_some()
}

// Check if a JSON value is a `rumoca_core::Span` (byte offsets + opaque source id).
// A Span is the fallback provenance type — it has byte offsets but the source
// is identified by an opaque `SourceId` rather than a filename, so we fall
// back to using the specimen path to read the source.
fn is_span(v: &Value) -> bool {
    v.get("start").and_then(Value::as_u64).is_some()
        && v.get("end").and_then(Value::as_u64).is_some()
        && v.get("source").is_some()
}

// Build the provenance result JSON from a found location/span.
//
// Extracts byte offsets, resolves the source file, slices the excerpt,
// and expands to enclosing lines. The result includes:
// - `found: true` / `false`
// - `kind`: "location" or "span"
// - `at_depth`: how many levels up from the leaf the provenance was found
// - `raw`: the original location/span value
// - `byte_range`: [start, end]
// - `file`: resolved source file path
// - `excerpt`: the exact bytes from [start, end)
// - `line_context`: the enclosing full source line(s)
fn provenance(raw: &Value, kind: &str, depth: usize, specimen: Option<&Path>) -> Value {
    let start = raw.get("start").and_then(Value::as_u64).unwrap_or(0) as usize;
    let end = raw.get("end").and_then(Value::as_u64).unwrap_or(0) as usize;
    let file_name = raw.get("file_name").and_then(Value::as_str).unwrap_or("");
    let sliced = slice_source(file_name, specimen, start, end);

    let mut out = json!({
        "found": true,
        "kind": kind,
        "at_depth": depth,
        "raw": raw,
        "byte_range": [start, end],
    });
    match sliced {
        Some((file, excerpt, line_context)) => {
            out["file"] = json!(file);
            out["excerpt"] = json!(excerpt);
            out["line_context"] = json!(line_context);
        }
        None => {
            out["excerpt"] = Value::Null;
            out["note"] = json!("could not resolve/read the source file for these offsets");
        }
    }
    out
}

// Resolve the source file and slice the byte range into an excerpt.
//
// Tries two strategies to find the source file:
// 1. Use `file_name` from a Location (if non-empty and the file exists)
// 2. Fall back to the specimen path (for Spans with opaque source ids)
//
// Then slices `bytes[start..end]` for the exact excerpt, and expands to
// the enclosing full lines for `line_context` (so Claude sees the complete
// Modelica statement, not just a fragment).
//
// Returns `(file_path, excerpt, line_context)` or None if the file can't
// be read or the byte range is invalid.
fn slice_source(
    file_name: &str,
    specimen: Option<&Path>,
    start: usize,
    end: usize,
) -> Option<(String, String, String)> {
    // **Absolute only.** `Path::new(file_name).is_file()` is resolved against the
    // *current working directory*, so a bare `"Resistor.mo"` would match a
    // same-named file wherever HRW happened to be launched from — and the excerpt
    // would be sliced out of a stranger's file and emitted as fact.
    //
    // Nothing observed it: every `Location` HRW emits today carries a full
    // document URI, because `parse_to_ast` is handed one (`worker.rs`). This is a
    // guard on the *shape of the input*, not a fix for a live bug — which is
    // exactly when it is cheapest to add, and it is the failure mode this project
    // treats as unrecoverable: a **confident wrong answer**, indistinguishable
    // from a right one.
    //
    // Falling back to the specimen when the name is relative is right, not a
    // degradation: a relative `file_name` means the location came from a document
    // this process cannot identify, and the specimen is the only file it *can*.
    let named = Path::new(file_name);
    let path = if !file_name.is_empty() && named.is_absolute() && named.is_file() {
        PathBuf::from(file_name)
    } else {
        specimen?.to_path_buf()
    };
    let src = fs::read_to_string(&path).ok()?;
    let bytes = src.as_bytes();
    if start > end || end > bytes.len() {
        return None;
    }
    let excerpt = String::from_utf8_lossy(&bytes[start..end]).into_owned();
    // Expand to whole containing lines (byte-wise, so we never split a char).
    let line_start = bytes[..start]
        .iter()
        .rposition(|&b| b == b'\n')
        .map_or(0, |i| i + 1);
    let line_end = bytes[end..]
        .iter()
        .position(|&b| b == b'\n')
        .map_or(bytes.len(), |i| end + i);
    let line_context = String::from_utf8_lossy(&bytes[line_start..line_end]).into_owned();
    Some((path.to_string_lossy().into_owned(), excerpt, line_context))
}

// A compact shape summary for an over-large node that was truncated from
// the focus file. Shows object keys (so Claude knows the node's structure)
// or array length (so Claude knows how many elements). This gives Claude
// enough information to navigate to the full node in the staged IR file.
fn shape(v: &Value) -> Value {
    match v {
        Value::Object(m) => json!(m.keys().cloned().collect::<Vec<_>>()),
        Value::Array(a) => json!(format!("[{} items]", a.len())),
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **A PANE selection emits neither the prose `kind` nor the prose sentinel.**
    ///
    /// The capture button copies whatever is selected anywhere in the window, so a
    /// selection made in a stage pane reaches the same code path as lab prose. Until
    /// 2026-09-22 it emitted `kind: "selection"` and `stage: "(lab prose, not a
    /// compile phase)"` **while the section below said the text is in no lab** — two
    /// contradictory claims in one document, which is exactly the defect that sentinel
    /// was introduced to fix the first time.
    ///
    /// Pinned because the two fields are read by different things: `kind` routes
    /// Claude's reasoning, and the sentinel is what a reader sees when they ask which
    /// phase a capture belongs to.
    #[test]
    fn a_pane_selection_emits_selection_and_says_it_is_not_prose() {
        let ask = Ask {
            seq: 4,
            request: None,
            specimen: None,
            model: Some("RcCircuit"),
            stage: None,
            libraries: Vec::new(),
            def_index: &Default::default(),
            parse_value: None,
            resolve_value: None,
            focus: Focus::Selection {
                origin: &crate::pointing::PointOrigin::StagePane {
                    stage: StageKind::Flatten,
                    sub_view: Some("Connections".to_owned()),
                },
                text: "flow \u{b7} 1 set(s)",
                in_source: false,
            },
            tracking: None,
            view: View {
                ui_mode: "Lab",
                stage_view: Some("Connections"),
                specimen_detail: None,
                viewing_log: false,
                animation: None,
            },
            failure: None,
            flags: vec![],
        };

        let doc = build(&ask);
        assert_eq!(
            doc["kind"], "selection",
            "calling a pane selection a lab passage is the claim this whole check exists \
             to stop",
        );
        assert_ne!(
            doc["stage"], "(lab prose, not a compile phase)",
            "the prose sentinel must not be reused for text that is not prose",
        );
        assert!(
            doc["selection"]["file"].is_null(),
            "no file may be named for text that is in no file",
        );
        assert_eq!(
            doc["selection"]["from"], "Flatten \u{2192} Connections",
            "the origin names the region that drew the text, which is the whole point of \
             declaring it rather than inferring it from the stage tab",
        );
    }

    /// **A lab-passage capture emits its lab, its text, and NO stage.**
    ///
    /// The file is what Claude actually reads, so this is the half that matters for
    /// accuracy. Three claims, each of which the obvious implementation gets wrong:
    ///
    /// - `stage` names **why** there is no stage, not merely that there is none. It
    ///   first emitted *"(navigated definition)"* — the only sentinel that existed —
    ///   which was a false claim standing beside `kind: "selection"`. A column read
    ///   found it the next day: absence must be stated *accurately*, not just stated.
    /// - `kind` is `lab_passage`, distinguishable from the three IR shapes and from a
    ///   `selection` made anywhere that is not a lab.
    /// - the passage section names the **file**, because the emitted text is what the
    ///   pane rendered and will not match the markdown byte-for-byte.
    #[test]
    fn lab_prose_emits_its_file_and_no_stage() {
        let defs = BTreeMap::new();
        let ask = Ask {
            seq: 12,
            request: None,
            specimen: None,
            model: None,
            stage: None,
            libraries: Vec::new(),
            def_index: &defs,
            parse_value: None,
            resolve_value: None,
            focus: Focus::Selection {
                in_source: true,
                origin: &crate::pointing::PointOrigin::LabProse {
                    lab: "dae-construction".to_owned(),
                },
                text: "A DAE is well posed when the counts agree.",
            },
            tracking: None,
            view: View {
                ui_mode: "Lab",
                stage_view: None,
                specimen_detail: None,
                viewing_log: false,
                animation: None,
            },
            failure: None,
            flags: vec![],
        };

        let doc = build(&ask);
        assert_eq!(doc["kind"], "selection");
        assert_eq!(
            doc["stage"], "(lab prose, not a compile phase)",
            "the sentinel must say WHY there is no stage \u{2014} \u{201c}(navigated \
             definition)\u{201d} was a false claim about this capture, and naming a \
             phase would send Claude to the wrong place entirely",
        );
        assert_eq!(doc["selection"]["from"], "prose in dae-construction");
        assert_eq!(
            doc["selection"]["file"], "hrw/docs/fixture-labs/dae-construction.md",
            "the file is named because the rendered text will not match the source",
        );
        assert_eq!(
            doc["selection"]["text"],
            "A DAE is well posed when the counts agree.",
        );
        // **Inverted 2026-11-08.** This asserted the note named the run and authored
        // regions — markers retired on 2026-08-31 — so the test kept a dead rule alive, and
        // Claude obeyed it while answering Doug about `the-pipeline.md`. The note must now
        // say there are no markers and point at where the real rule lives.
        let note = doc["selection"]["note"].as_str().unwrap_or_default();
        assert!(
            note.contains("no provenance markers")
                && note.contains("judge that from the conversation")
                && note.contains("docs/fixture-labs/README.md"),
            "the note must carry the CURRENT editing rule, since this capture is what an \
             \"improve this paragraph\" request arrives with: {note}",
        );
        assert!(
            !note.contains("authored") && !note.contains("region"),
            "the note revives the retired markers: {note}",
        );
    }

    /// **A selection with no named file does not claim there is no file.**
    ///
    /// The fileless note told Claude the text was *"NOT in any repository file"* and that
    /// *"there is no file it is in"*. That was true for compiler output and false for two
    /// origins that reach the same arm: the specimen's Modelica source and its purpose
    /// note, which are files HRW has simply not verified the text against. Found
    /// 2026-11-07 while restoring the note's lost line continuations. Checked here for
    /// `SpecimenSource`, the origin the false claim would mislead most.
    #[test]
    fn a_fileless_selection_does_not_deny_that_a_file_exists() {
        let defs = BTreeMap::new();
        let ask = Ask {
            seq: 13,
            request: None,
            specimen: Some(std::path::Path::new("hrw/specimens/RcCircuit.mo")),
            model: Some("RcCircuit"),
            stage: None,
            libraries: Vec::new(),
            def_index: &defs,
            parse_value: None,
            resolve_value: None,
            focus: Focus::Selection {
                in_source: false,
                origin: &crate::pointing::PointOrigin::SpecimenSource,
                text: "C.v",
            },
            tracking: None,
            view: View {
                ui_mode: "Specimen",
                stage_view: None,
                specimen_detail: None,
                viewing_log: false,
                animation: None,
            },
            failure: None,
            flags: vec![],
        };

        let doc = build(&ask);
        let note = doc["selection"]["note"]
            .as_str()
            .expect("the note is a string");
        assert!(
            !note.contains("NOT in any repository file") && !note.contains("no file it is in"),
            "the specimen's source IS a file; the note may only say none is named: {note}",
        );
        assert!(
            note.contains("Modelica source") && note.contains("`specimen`"),
            "the note must say where specimen-source text does come from: {note}",
        );
        assert!(
            !note.contains("  "),
            "the note has a run of spaces, which is a lost line continuation: {note:?}",
        );
    }

    /// `hrw://notebook/<name>` resolves only to a notebook in a known directory.
    ///
    /// The verb hands a path to the operating system's file association, so the set of
    /// things a link can open stays "a file name in one of two directories" rather than
    /// whatever a link happens to spell. Traversal and separators are refused outright —
    /// not sanitised, because a rejected link is visible and a quietly rewritten one is
    /// not.
    #[test]
    fn a_notebook_name_cannot_escape_its_directory() {
        for bad in [
            "",
            "..",
            "../secrets.nb",
            r"..\secrets.nb",
            "sub/dir.nb",
            r"sub\dir.nb",
            r"C:\Windows\notepad.exe",
            "notes.txt",                    // must be a notebook
            "structural-vs-numerical-rank", // ...with the extension
        ] {
            assert!(resolve_notebook(bad).is_none(), "{bad:?} must be refused");
        }

        // The real fixture notebook resolves, if this checkout has it.
        let real = "structural-vs-numerical-rank.nb";
        if Path::new(FIXTURE_NOTEBOOKS_DIR).join(real).is_file() {
            let found = resolve_notebook(real).expect("the fixture notebook should resolve");
            assert!(found.ends_with(real));
            assert!(
                found.starts_with(FIXTURE_NOTEBOOKS_DIR),
                "resolved inside the fixture dir"
            );
        }

        // A name that is well-formed but absent still resolves to nothing.
        assert!(resolve_notebook("no-such-notebook.nb").is_none());
    }

    /// `hrw://doc/<name>` resolves only to a markdown file under `hrw/docs/`.
    ///
    /// **The interesting difference from [`resolve_notebook`] is that a `/` must be
    /// ALLOWED here** — docs nest, and `compiler-phases/the-chain-of-problems.md` is a
    /// legitimate target — so the separator cannot carry the refusal the way it does for a
    /// notebook. `..` still can, and the containment check afterwards is what makes that
    /// safe rather than merely careful: whatever the textual test lets through, a path
    /// that does not land inside `docs/` once resolved is refused.
    ///
    /// The two are tested the same shape on purpose. A future third verb should be able
    /// to copy either one and get the refusals right by imitation.
    ///
    /// # Each half is proven separately, because must-fire found neither was
    ///
    /// The first version of this test listed eight escapes and **passed with either guard
    /// deleted** — every case was caught twice, so the test proved only that *something*
    /// refused them. Two cases were added to separate them, and each is here to make one
    /// specific deletion fail:
    ///
    /// - **an absolute path spelled with forward slashes** contains no `\` and no `..`, so
    ///   the textual check waves it through; `Path::join` then *discards the base* and
    ///   only the containment check refuses it. This is not a contrived spelling — it is
    ///   what a path pasted from a URL or a git-bash prompt looks like.
    /// - **`compiler-phases/../upstream-issues.md`** canonicalises to a real file *inside*
    ///   `docs/`, so containment is satisfied and only the textual check refuses it. It is
    ///   refused deliberately rather than tolerated: the link checkers resolve a doc link
    ///   by its literal name, so a second spelling for the same file is a name they cannot
    ///   follow.
    #[test]
    fn a_doc_name_cannot_escape_the_docs_directory() {
        // An absolute path into this very checkout, in the spelling the textual guard
        // does not see. Built rather than hard-coded, so it names a file that exists on
        // whichever machine is running.
        let absolute_outside_docs = format!(
            "{}/CLAUDE.md",
            env!("CARGO_MANIFEST_DIR").replace('\\', "/")
        );

        for bad in [
            "",
            "..",
            "../../Cargo.toml",
            "../CLAUDE.md",                 // one level up is still outside `docs/`
            r"..\CLAUDE.md",                // the Windows spelling of the same escape
            r"C:\Windows\System32\eula.md", // absolute, and `join` would honour it
            "upstream-issues",              // the extension is part of the name
            "specimens/RcCircuit.mo",       // a real file, but not a document
            &absolute_outside_docs,         // only the containment check refuses this
            "compiler-phases/../upstream-issues.md", // only the textual check refuses this
        ] {
            assert!(resolve_doc(bad).is_none(), "{bad:?} must be refused");
        }

        // The document the labs actually link to, and one in a subdirectory — the case
        // `resolve_notebook`'s rule would have rejected outright.
        for good in [
            "upstream-issues.md",
            "compiler-phases/the-chain-of-problems.md",
        ] {
            let found = resolve_doc(good).unwrap_or_else(|| panic!("{good} should resolve"));
            assert!(found.is_file());
            assert!(
                found.starts_with(DOCS_DIR),
                "{good} must resolve inside docs/, got {}",
                found.display(),
            );
        }

        // Well-formed and absent is still nothing — the same clause as the notebook's,
        // because a link to a deleted document must fail rather than open something else.
        assert!(resolve_doc("no-such-document.md").is_none());
    }

    /// `hrw://src/<path>#<symbol>` finds the definition, and refuses everything else.
    ///
    /// **The symbol half is the point, so it is what the assertions are about.** A path
    /// alone is `resolve_doc` with a different root; what is new here is resolving a NAME
    /// to a line at click time, so a lab citing `union` keeps working when the function
    /// moves and fails loudly when it is renamed away.
    ///
    /// Two properties are pinned that a naive `contains` would get wrong, both of which
    /// this repository would actually hit:
    ///
    /// - **A doc comment naming the symbol must not win.** These crates document items
    ///   heavily, and `/// ... union ...` sits above `fn union` constantly — landing the
    ///   reader on prose *about* the function instead of the function is a quiet wrong
    ///   answer, which is worse than none.
    /// - **A miss is `None`, never line 1.** Scrolling someone to the top of a
    ///   1,745-line file while claiming to show them a symbol is exactly the invented
    ///   answer `CLAUDE.md` forbids.
    #[test]
    fn a_source_link_finds_a_symbol_and_refuses_an_escape() {
        let rel = "crates/rumoca-phase-flatten/src/connections/mod.rs";
        let full = workspace_root().join(rel);
        assert!(
            full.is_file(),
            "{rel} must exist for this test to mean anything"
        );
        let text = fs::read_to_string(&full).expect("readable");

        // A path with no symbol resolves to the file and no line.
        let plain = resolve_source(rel).expect("the path alone must resolve");
        assert_eq!(plain.line, None);
        assert!(plain.path.ends_with("mod.rs"));

        // Each symbol lands on its own definition, located in the CURRENT source rather
        // than from a number stored here — so this test cannot rot into a false pass.
        for (symbol, keyword) in [
            ("union", "fn union"),
            ("UnionFind", "struct UnionFind"),
            // A struct FIELD, which no keyword introduces.
            ("parent", "parent:"),
        ] {
            let found =
                resolve_source(&format!("{rel}#{symbol}")).unwrap_or_else(|| panic!("{symbol}"));
            let n = found.line.expect("a named symbol resolves to a line");
            let line = text.lines().nth(n - 1).expect("in range");
            assert!(
                line.contains(keyword),
                "line {n} for `{symbol}` should define it, got: {line}",
            );
            assert!(
                !line.trim_start().starts_with("///"),
                "line {n} for `{symbol}` is a doc comment, not the definition",
            );
        }

        // A symbol that is not defined here is a finding, not line 1.
        assert!(resolve_source(&format!("{rel}#no_such_symbol_anywhere")).is_none());

        // **An enum VARIANT resolves, and a use site does not steal it.** The first
        // version of this resolver knew only keyword-introduced items, so the lab's
        // `#SetFormed` link failed on the checker's first run. `mod.rs` mentions
        // `ConnectionStep::SetFormed` at its emit site; the definition is in `trace.rs`,
        // and the qualified use must not match there or anywhere.
        let trace = "crates/rumoca-phase-flatten/src/connections/trace.rs";
        let variant = resolve_source(&format!("{trace}#SetFormed")).expect("a variant resolves");
        let vtext = fs::read_to_string(workspace_root().join(trace)).expect("readable");
        let vline = vtext
            .lines()
            .nth(variant.line.expect("a line") - 1)
            .expect("in range");
        assert!(
            vline.trim_start().starts_with("SetFormed"),
            "should land on the variant's own line, got: {vline}",
        );
        assert!(
            resolve_source(&format!("{rel}#SetFormed")).is_none(),
            "mod.rs only USES ConnectionStep::SetFormed; a qualified use is not a definition",
        );

        for bad in [
            "",
            "..",
            "../secrets.rs",
            r"crates\rumoca-core\src\lib.rs", // the Windows spelling
            "crates/../../outside.rs",
            "crates/rumoca-phase-flatten/src/connections/nope.rs", // absent
        ] {
            assert!(resolve_source(bad).is_none(), "{bad:?} must be refused");
        }
    }

    /// `parse_path` is exactly `describe_path`'s inverse.
    ///
    /// The property that matters for #42's parity principle: a link must consume the
    /// capture's own spelling of a node path. Tested by round-tripping rather than by
    /// restating either format, so it fails if either side moves — the same shape as
    /// the stage-slug and frame-counter tests.
    #[test]
    fn a_node_path_round_trips_between_capture_and_link() {
        let cases: Vec<Vec<Seg>> = vec![
            vec![],
            vec![Seg::Key("error".into())],
            vec![
                Seg::Key("error".into()),
                Seg::Key("unmatched_unknowns".into()),
                Seg::Index(0),
            ],
            vec![
                Seg::Key("blocks".into()),
                Seg::Index(2),
                Seg::Key("equations".into()),
                Seg::Index(0),
            ],
            // Consecutive indices, which `describe_path` writes without a separator.
            vec![Seg::Key("rows".into()), Seg::Index(3), Seg::Index(1)],
            // A leading index is unusual but expressible.
            vec![Seg::Index(7)],
            // --- Keys carrying this grammar's own separators. ---
            //
            // NOT an edge case in Modelica IR. `enum_literal_ordinals` is keyed by
            // qualified names, and before 2026-07-31 `StateSelect.never` was written
            // bare, split into two segments on the way back, and landed on a node
            // that does not exist. `docs/fidelity-plan.md` F7 found it on every model
            // in the corpus at once — 6,169 broken paths.
            vec![
                Seg::Key("enum_literal_ordinals".into()),
                Seg::Key("StateSelect.never".into()),
            ],
            // Modelica's single-quoted identifiers, which appear beside the plain ones.
            vec![
                Seg::Key("enum_literal_ordinals".into()),
                Seg::Key("StateSelect.'never'".into()),
            ],
            // A dotted key followed by more path, so the closing `"]` must hand back
            // the remainder correctly rather than swallowing it.
            vec![
                Seg::Key("m".into()),
                Seg::Key("gear.flange_b.tau".into()),
                Seg::Index(2),
                Seg::Key("v".into()),
            ],
            // The quoting characters themselves.
            vec![Seg::Key(r#"a"b"#.into())],
            vec![Seg::Key(r"a\b".into())],
            vec![Seg::Key("has[bracket]".into())],
            // An empty key: legal JSON, and bare it would produce `a..b`.
            vec![
                Seg::Key("a".into()),
                Seg::Key(String::new()),
                Seg::Key("b".into()),
            ],
        ];
        for path in cases {
            let written = describe_path(&path);
            assert_eq!(
                parse_path(&written).as_deref(),
                Some(path.as_slice()),
                "{path:?} wrote as {written:?} and did not parse back",
            );
        }
    }

    /// Malformed paths are refused, not guessed at.
    ///
    /// A link that silently pointed at the *wrong* node would be worse than one that
    /// visibly does nothing: the reader would take the wrong subtree for the answer.
    #[test]
    fn a_malformed_node_path_is_refused() {
        for bad in [
            "a..b",
            ".a",
            "a.",
            "a[",
            "a[]",
            "a[x]",
            "a]0[",
            "a[0]b",
            // Quoted-key forms that are not closed properly. `a["b"` and
            // `a["b"x` must fail rather than silently yielding key `b`.
            r#"a["b"#,
            r#"a["b""#,
            r#"a["b"x"#,
            r#"a["b\"#,
        ] {
            assert!(parse_path(bad).is_none(), "{bad:?} should not parse");
        }
    }

    /// **Ordinary paths are spelled exactly as before the quoting rule existed.**
    ///
    /// Quoting had to be additive: `hrw://` links live in checked-in fixture
    /// labs and in every `focus.json` written so far, so a key that never needed
    /// escaping must still render byte-identically or those links break.
    #[test]
    fn quoting_did_not_change_how_ordinary_paths_are_written() {
        let cases: Vec<(Vec<Seg>, &str)> = vec![
            (
                vec![
                    Seg::Key("error".into()),
                    Seg::Key("unmatched_unknowns".into()),
                    Seg::Index(0),
                ],
                "error.unmatched_unknowns[0]",
            ),
            (
                vec![
                    Seg::Key("blocks".into()),
                    Seg::Index(2),
                    Seg::Key("equations".into()),
                ],
                "blocks[2].equations",
            ),
            (vec![], "(tree root)"),
        ];
        for (path, want) in cases {
            assert_eq!(describe_path(&path), want, "spelling changed for {path:?}");
            assert_eq!(parse_path(want).as_deref(), Some(path.as_slice()));
        }
    }

    /// A neutral `View` for tests that are not about the view section.
    ///
    /// Named rather than inlined so adding a field to `View` is a one-line
    /// change here instead of an edit at every `Ask` in this module.
    fn test_view() -> View<'static> {
        View {
            ui_mode: "Specimen",
            stage_view: None,
            specimen_detail: None,
            viewing_log: false,
            animation: None,
        }
    }

    /// A capture names the **first** failing stage, not the current one.
    ///
    /// #45 step 3. A failure cascades: everything downstream reports "not reached", so
    /// the earliest error is the cause and the rest are consequences. A capture that
    /// named whichever stage Doug happened to be looking at would routinely name a
    /// consequence — the wrong answer to "why doesn't this work?".
    #[test]
    fn a_capture_names_the_first_failing_stage_and_its_diagnosis() {
        let empty = BTreeMap::new();
        let error = json!({
            "kind": "dae_construction",
            "message": "unbalanced model: 2 equations, 3 unknowns (balance = -1)",
            "balance": -1,
        });
        let ask = Ask {
            seq: 1,
            request: None,
            specimen: None,
            model: Some("UnbalancedShaft"),
            // Doug is looking at Structural, which is downstream of the real failure.
            stage: Some(StageKind::Structural),
            libraries: vec![],
            def_index: &empty,
            parse_value: None,
            resolve_value: None,
            focus: Focus::Stage,
            tracking: None,
            view: test_view(),
            failure: Some(PipelineFailure {
                stage: StageKind::Flatten.name(),
                note: "unbalanced model: 2 equations, 3 unknowns (balance = -1)",
                error: Some(&error),
                not_reached: vec![StageKind::Structural.name(), StageKind::Events.name()],
            }),
            flags: vec![],
        };
        let doc = build(&ask);

        let f = &doc["pipeline_failure"];
        assert!(
            !f.is_null(),
            "a failure must be stated prominently, not left to be found"
        );
        assert_eq!(f["stage"], json!(StageKind::Flatten.name()));
        assert_eq!(
            f["error"]["balance"],
            json!(-1),
            "the diagnosis travels with it"
        );
        assert!(
            f["downstream_not_reached"]
                .as_array()
                .is_some_and(|a| a.contains(&json!(StageKind::Structural.name()))),
            "downstream emptiness must be labelled as a consequence: {f:?}",
        );
        // The stage Doug is *looking at* is still reported, separately — it is where he
        // is, and it must not be confused with what broke.
        assert_eq!(doc["stage"], json!(StageKind::Structural.name()));
    }

    /// A clean compile emits **no** failure section.
    ///
    /// Absent rather than present-and-empty: a key that always exists would make
    /// "nothing failed" indistinguishable from "the field was not populated".
    #[test]
    fn a_clean_compile_has_no_failure_section() {
        let empty = BTreeMap::new();
        let ask = Ask {
            seq: 1,
            request: None,
            specimen: None,
            model: Some("RcCircuit"),
            stage: Some(StageKind::Structural),
            libraries: vec![],
            def_index: &empty,
            parse_value: None,
            resolve_value: None,
            focus: Focus::Stage,
            tracking: None,
            view: test_view(),
            failure: None,
            flags: vec![],
        };
        assert!(build(&ask).get("pipeline_failure").is_none());
    }

    /// Span-ascent picks the *tightest* enclosing location, and the slice is
    /// expanded to whole source lines. The clicked node is a bare string leaf
    /// with no location of its own — provenance must come from its ancestor.
    #[test]
    fn ascent_finds_tightest_location_and_slices_lines() {
        // Source whose bytes 8..17 are `flange_a` on line 2.
        let src = "model M\n  flange_a x;\nend M;\n";
        let start = src.find("flange_a").unwrap();
        let end = start + "flange_a".len();

        let dir = std::env::var("CARGO_TARGET_TMPDIR")
            .unwrap_or_else(|_| std::env::temp_dir().to_string_lossy().into_owned());
        let file = Path::new(&dir).join("hrw_bridge_ascent.mo");
        fs::write(&file, src).unwrap();

        // A tree where the leaf `name` has no location; its parent component does.
        //
        // **The fixture carries `start_line`/`start_column`, not `file_name`** —
        // updated 2026-11-03. v0.10.0's `Location` dropped `file_name` and gained
        // the line/column pair, and `is_location` was changed to match; this
        // fixture kept the 0.9.20 shape, so the recognizer rejected it and the
        // ascent found nothing. **The fixture was wrong, not the ascent** — which is
        // why it is rebuilt rather than the recognizer loosened. A recognizer that
        // accepted either shape would have hidden that the field is gone.
        //
        // The line/column values are the real ones for byte 10 of `src`: line 2,
        // column 3, since `flange_a` follows two spaces on the second line.
        let root = json!({
            "components": [
                {
                    "location": {
                        "start": start,
                        "end": end,
                        "start_line": 2,
                        "start_column": 3
                    },
                    "name": "flange_a"
                }
            ]
        });
        let path = vec![
            Seg::Key("components".into()),
            Seg::Index(0),
            Seg::Key("name".into()),
        ];

        let prov = ascend_provenance(&root, &path, Some(&file));
        assert_eq!(prov["found"], json!(true));
        assert_eq!(prov["kind"], json!("location"));
        // Ascended one level from the leaf to the component object.
        assert_eq!(prov["at_depth"], json!(2));
        assert_eq!(prov["excerpt"], json!("flange_a"));
        // Line context is the whole line, not just the token.
        assert_eq!(prov["line_context"], json!("  flange_a x;"));
    }

    /// End-to-end over real Rumoca parse IR: every `location`-bearing node's
    /// byte range must slice cleanly out of the specimen source.
    #[test]
    fn provenance_holds_over_real_parsed_specimen() {
        let specimen = Path::new(crate::manifest_path!("specimens", "RotationalInertia.mo"));
        let source = fs::read_to_string(specimen).expect("read specimen");
        let ast = rumoca_phase_parse::parse_to_ast(&source, "RotationalInertia.mo").expect("parse");
        let root = serde_json::to_value(&ast).expect("to_value");

        // Find a path to some object carrying a real (non-dummy) location.
        let mut path = Vec::new();
        let found = first_location_path(&root, &mut path);
        assert!(
            found,
            "expected at least one located node in the parsed AST"
        );

        let prov = ascend_provenance(&root, &path, Some(specimen));
        assert_eq!(prov["found"], json!(true), "provenance: {prov}");
        let excerpt = prov["excerpt"].as_str().expect("excerpt string");
        assert!(
            source.contains(excerpt),
            "excerpt {excerpt:?} not found in source"
        );
    }

    /// A node captured in the Resolve tab carries the same node from Parse and
    /// the scalar deltas, even though Parse is rooted at the `StoredDefinition`
    /// (class under `classes.<model>`) and Resolve at the class itself.
    #[test]
    fn cross_stage_diffs_the_same_node_across_roots() {
        // Parse: wrapped in a StoredDefinition, def_ids still null.
        let parse = json!({
            "classes": { "M": { "def_id": null, "components": {
                "c": { "def_id": null, "type_def_id": null }
            }}},
            "within": null
        });
        // Resolve: extracted class, def_ids populated.
        let resolve = json!({
            "def_id": 5,
            "components": { "c": { "def_id": 9, "type_def_id": 100 } }
        });
        let empty = BTreeMap::new();
        let key_path = vec![Seg::Key("components".into()), Seg::Key("c".into())];
        let ask = Ask {
            seq: 1,
            request: None,
            specimen: None,
            model: Some("M"),
            stage: Some(StageKind::Resolve),
            libraries: vec![],
            def_index: &empty,
            parse_value: Some(&parse),
            resolve_value: Some(&resolve),
            focus: Focus::Node {
                key_path: key_path.clone(),
                stage_value: &resolve,
            },
            tracking: None,
            view: test_view(),
            failure: None,
            flags: vec![],
        };

        let cs = build(&ask)["cross_stage"].clone();
        assert_eq!(cs["applicable"], json!(true), "{cs}");
        // Parse side found the node under classes.M.
        assert_eq!(cs["parse"]["value"]["def_id"], json!(null));
        assert_eq!(cs["resolve"]["value"]["def_id"], json!(9));
        // The two field changes are reported.
        let changes = cs["changes"].as_array().unwrap();
        let has = |p: &str, r: i64| {
            changes.iter().any(|c| {
                c["path"] == json!(p) && c["parse"] == json!(null) && c["resolve"] == json!(r)
            })
        };
        assert!(has("def_id", 9), "changes: {changes:?}");
        assert!(has("type_def_id", 100), "changes: {changes:?}");
    }

    /// Build an `Ask` that is following `name` across `stages`.
    fn tracking_ask<'a>(
        name: &'a str,
        stages: &'a [(&'a str, Option<&'a Value>)],
        def_index: &'a BTreeMap<u64, DefInfo>,
    ) -> Ask<'a> {
        Ask {
            seq: 1,
            request: None,
            specimen: None,
            model: Some("M"),
            stage: Some(StageKind::Flatten),
            libraries: vec![],
            def_index,
            parse_value: None,
            resolve_value: None,
            focus: Focus::Stage,
            tracking: Some(Tracking {
                seq: 7,
                name,
                declared_line: Some(23),
                declaring_class: None,
                stage_values: stages,
            }),
            view: test_view(),
            failure: None,
            flags: vec![],
        }
    }

    /// The compound capture records where the followed identifier lives **and
    /// where it does not** — absence is how a demoted variable announces itself.
    #[test]
    fn tracking_section_reports_presence_and_absence() {
        let flatten = json!({ "variables": { "states": { "h": { "kind": "state" } } } });
        let initialization = json!({ "plan": [] });
        let stages: Vec<(&str, Option<&Value>)> = vec![
            ("flatten", Some(&flatten)),
            ("initialization", Some(&initialization)),
            ("events", None),
        ];
        let empty = BTreeMap::new();
        let doc = build(&tracking_ask("h", &stages, &empty));
        let t = &doc["tracking"];

        assert_eq!(t["identifier"], json!("h"));
        assert_eq!(t["seq"], json!(7), "its own counter, not the focus seq");
        assert_eq!(t["declared_at_line"], json!(23));

        let by_stage = t["stages"].as_array().expect("stages array");
        assert_eq!(by_stage[0]["stage"], json!("flatten"));
        assert_eq!(
            by_stage[0]["mentions"],
            json!(1),
            "the key `h` is a mention"
        );
        // Absence is emitted, not omitted.
        assert_eq!(by_stage[1]["mentions"], json!(0), "genuinely absent here");
        // And "produced no IR" is distinct from "IR without the name in it".
        assert_eq!(by_stage[2]["produced_no_ir"], json!(true));
        assert!(by_stage[2].get("mentions").is_none());
    }

    /// Emission uses the same matching rules as the views, or the Question Bar
    /// would describe context that differs from what is highlighted on screen.
    #[test]
    fn tracking_matches_like_the_views_do() {
        let stage = json!({
            "equation": "der(h) - v",       // mentions h, lexically
            "other": "height",              // one identifier, NOT a mention of h
            "description": "height of h",   // prose: not a mention
            "unknown": "h",                 // exact identity
            "nested": { "list": [ { "name": "der(h)" } ] },
        });
        let stages: Vec<(&str, Option<&Value>)> = vec![("flatten", Some(&stage))];
        let empty = BTreeMap::new();
        let doc = build(&tracking_ask("h", &stages, &empty));
        let entry = &doc["tracking"]["stages"][0];

        // equation, unknown, nested name — but not `other`, not `description`.
        assert_eq!(entry["mentions"], json!(3), "got: {:?}", entry["paths"]);
        let paths: Vec<&str> = entry["paths"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p.as_str().unwrap())
            .collect();
        assert!(paths.contains(&"equation"));
        assert!(paths.contains(&"unknown"));
        assert!(paths.iter().any(|p| p.contains("nested")));
        assert!(!paths.contains(&"other"), "substring is not a mention");
        assert!(!paths.contains(&"description"), "prose is not a mention");
    }

    /// Following and pointing are independent: either may be absent.
    #[test]
    fn tracking_section_absent_when_nothing_is_followed() {
        let empty = BTreeMap::new();
        let ask = Ask {
            seq: 1,
            request: None,
            specimen: None,
            model: Some("M"),
            stage: Some(StageKind::Parse),
            libraries: vec![],
            def_index: &empty,
            parse_value: None,
            resolve_value: None,
            focus: Focus::Stage,
            tracking: None,
            view: test_view(),
            failure: None,
            flags: vec![],
        };
        assert!(build(&ask).get("tracking").is_none());
    }

    /// The samples are capped; the count is not. Two tiers, two caps.
    ///
    /// Sized from the constants rather than a literal — this test previously
    /// hard-coded 40 mentions, which silently stopped testing truncation the
    /// day the cap was raised to 40 and the two coincided.
    #[test]
    fn mention_samples_are_capped_but_counted_exactly() {
        let n = MAX_MENTION_PATHS + 5;
        let many: Vec<Value> = (0..n).map(|_| json!({ "unknown": "h" })).collect();
        let stage = json!({ "rows": many });
        let stages: Vec<(&str, Option<&Value>)> = vec![("structural", Some(&stage))];
        let empty = BTreeMap::new();
        let doc = build(&tracking_ask("h", &stages, &empty));
        let entry = &doc["tracking"]["stages"][0];

        assert_eq!(
            entry["mentions"],
            json!(n),
            "count is exact regardless of the caps"
        );
        assert_eq!(entry["paths"].as_array().unwrap().len(), MAX_MENTION_PATHS);
        assert_eq!(entry["paths_truncated"], json!(true));
        assert_eq!(
            entry["contexts"].as_array().unwrap().len(),
            MAX_MENTION_CONTEXTS
        );
        assert_eq!(entry["contexts_truncated"], json!(true));
    }

    /// A mention arrives with the IR around it, not just an address.
    ///
    /// Regression for the finding that cost four manual reads of `events.json`:
    /// `__pre__.overSpeed` is *manufactured*, and the only thing that says so is
    /// `generated: true` on the object one level above the matching leaf. The
    /// address alone cannot carry it.
    #[test]
    fn mention_contexts_carry_the_enclosing_ir_and_siblings() {
        // Shaped like the real Events IR: the name sits in a small object whose
        // sibling field is the decisive one.
        let stage = json!({
            "discrete_updates": {
                "valued_updates_f_m": [{
                    "lhs": { "name": "overSpeed" },
                    "rhs": { "else_branch": { "VarRef": {
                        "name": { "name": "__pre__.overSpeed", "generated": true }
                    }}},
                }]
            }
        });
        let stages: Vec<(&str, Option<&Value>)> = vec![("events", Some(&stage))];
        let empty = BTreeMap::new();
        let doc = build(&tracking_ask("__pre__.overSpeed", &stages, &empty));
        let ctx = &doc["tracking"]["stages"][0]["contexts"][0];

        assert_eq!(ctx["value"], json!("__pre__.overSpeed"));
        // The enclosing node fits well inside the budget, so the whole update
        // comes along — and with it, `generated`.
        assert!(
            ctx["context"].to_string().contains("\"generated\":true"),
            "the decisive sibling field must arrive with the mention: {ctx}",
        );
        // And the leaf's own siblings are named.
        let window = ctx["siblings"]["window"]
            .as_array()
            .expect("sibling window");
        assert!(
            window.iter().any(|v| v == "generated"),
            "siblings must list what sits beside the hit: {window:?}",
        );
        assert_eq!(ctx["siblings"]["window_is_complete"], json!(true));
    }

    /// In a large map the budget buys only the immediate parent — but the
    /// sibling window still lands on the neighbours, which is where the signal
    /// was for `__pre__.overSpeed` in Solve lowering (988 bindings, and the
    /// other `__pre__` companions immediately beside it).
    #[test]
    fn sibling_window_is_centred_on_the_hit_not_the_start() {
        let mut bindings = serde_json::Map::new();
        for i in 0..400 {
            bindings.insert(format!("filler_{i:03}"), json!({ "P": { "index": i } }));
        }
        bindings.insert("zzz_target".to_owned(), json!("x"));
        bindings.insert("zzz_neighbour".to_owned(), json!("y"));
        let root = json!({ "bindings": Value::Object(bindings) });

        let path = vec![Seg::Key("bindings".into()), Seg::Key("zzz_target".into())];
        let sib = siblings(&root, &path);

        assert_eq!(sib["count"], json!(402));
        assert_eq!(sib["window_is_complete"], json!(false));
        let window: Vec<&str> = sib["window"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .collect();
        assert!(
            window.contains(&"zzz_target"),
            "the window must contain the hit: {window:?}"
        );
        assert!(
            window.contains(&"zzz_neighbour"),
            "the window must reach the hit's neighbours, not the map's first keys: {window:?}",
        );
        assert!(
            !window.contains(&"filler_000"),
            "a first-N sample would have shown this and said nothing: {window:?}",
        );
    }

    /// The budget is spent greedily upward, so a small leaf brings its whole
    /// enclosing structure and a leaf in something huge brings only its parent.
    #[test]
    fn enclosing_context_takes_the_largest_ancestor_that_fits() {
        let small = json!({ "eq": { "lhs": "a", "rhs": "b" } });
        let path = vec![Seg::Key("eq".into()), Seg::Key("lhs".into())];
        let (depth, ctx) = enclosing_context(&small, &path);
        assert_eq!(
            depth, 0,
            "a small tree fits entirely, so the root is returned"
        );
        assert_eq!(ctx, small);

        // Now make the root overflow the budget: the leaf's parent still fits.
        let filler: String = "x".repeat(MAX_CONTEXT_BYTES);
        let big = json!({ "eq": { "lhs": "a", "rhs": "b" }, "bulk": filler });
        let (depth, ctx) = enclosing_context(&big, &path);
        assert_eq!(
            depth, 1,
            "the root no longer fits, so its child is returned"
        );
        assert_eq!(ctx, json!({ "lhs": "a", "rhs": "b" }));
    }

    /// A synthesized name has no declaration, and must not claim one.
    ///
    /// Following `__pre__.overSpeed` reported `declared_at_line: 41` — which is
    /// where `overSpeed` is declared. The number was real (a generated variable
    /// inherits its base's span) but the field name asserted a declaration that
    /// does not exist, so a reader would look at line 41 and find a *different*
    /// variable. Same species as a phantom `request`, or `kind: "stage"` for a
    /// cleared point.
    ///
    /// Recognition goes through `rumoca_core::pre_slot_base`, never a string
    /// match: `generated_names.rs` is the owning definition of the convention
    /// and forbids consumers from spelling it out themselves.
    #[test]
    fn a_generated_name_reports_its_origin_instead_of_a_declaration() {
        let stages: [(&str, Option<&Value>); 0] = [];
        let generated = build_tracking(&Tracking {
            seq: 1,
            name: "__pre__.overSpeed",
            // The app supplies this from the identifier index — the generated
            // variable really does carry the base's span.
            declared_line: Some(41),
            declaring_class: None,
            stage_values: &stages,
        });

        assert!(
            generated.get("declared_at_line").is_none(),
            "a synthesized variable is declared nowhere: {generated}",
        );
        assert_eq!(generated["generated"]["kind"], json!("pre-slot"));
        assert_eq!(generated["generated"]["base"], json!("overSpeed"));
        // The span is still worth having — under a name that says what it is.
        assert_eq!(generated["span_inherited_from_base_at_line"], json!(41));

        // An ordinary name is unaffected: it really is declared where it says.
        let declared = build_tracking(&Tracking {
            seq: 1,
            name: "overSpeed",
            declared_line: Some(41),
            declaring_class: Some("MotorWithBrake"),
            stage_values: &stages,
        });
        assert_eq!(declared["declared_at_line"], json!(41));
        assert_eq!(declared["declared_in_class"], json!("MotorWithBrake"));
        assert!(declared.get("generated").is_none());

        // Nested slots peel exactly one level, matching `pre_slot_base`.
        let nested = build_tracking(&Tracking {
            seq: 1,
            name: "__pre__.__pre__.x",
            declared_line: None,
            declaring_class: None,
            stage_values: &stages,
        });
        assert_eq!(nested["generated"]["base"], json!("__pre__.x"));
    }

    /// The jump control and the emitted context must see the same nodes.
    ///
    /// `mention_paths` is what the tree scrolls through; `tracking.paths` is
    /// what Claude is told. They come from one run on purpose — a second
    /// matcher written for the UI would be a second definition of *mention*, and
    /// the app would highlight one set of nodes while describing another. That
    /// class of drift is what the whole phase was spent removing.
    #[test]
    fn the_jump_list_and_the_emitted_paths_are_one_list() {
        let stage = serde_json::json!({
            "variables": { "emf.w": { "kind": "algebraic" } },
            "equations": [
                { "text": "emf.w - der(emf.phi)" },
                { "text": "load.w - emf.k" },
                { "text": "emf.k * emf.w - emf.v" },
            ],
        });
        let stages: Vec<(&str, Option<&Value>)> = vec![("flatten", Some(&stage))];
        let empty = BTreeMap::new();
        let doc = build(&tracking_ask("emf.w", &stages, &empty));

        let emitted: Vec<String> = doc["tracking"]["stages"][0]["paths"]
            .as_array()
            .expect("paths")
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect();
        let jump: Vec<String> = mention_paths(&stage, "emf.w")
            .iter()
            .map(|p| describe_path(p))
            .collect();

        assert_eq!(
            jump, emitted,
            "the tree and the capture must agree node for node"
        );
        assert_eq!(
            jump.len(),
            3,
            "key, and two of the three equations: {jump:?}"
        );
        assert!(
            !jump.iter().any(|p| p.contains("[1]")),
            "the equation mentioning only load.w must not match: {jump:?}",
        );
    }

    /// The emitted list is capped for readability; the jump list is not, because
    /// a control that cycles must reach every occurrence.
    #[test]
    fn the_jump_list_is_uncapped_where_the_emitted_one_is_sampled() {
        let n = MAX_MENTION_PATHS + 7;
        let rows: Vec<Value> = (0..n)
            .map(|_| serde_json::json!({ "unknown": "h" }))
            .collect();
        let stage = serde_json::json!({ "rows": rows });
        let stages: Vec<(&str, Option<&Value>)> = vec![("structural", Some(&stage))];
        let empty = BTreeMap::new();
        let doc = build(&tracking_ask("h", &stages, &empty));

        assert_eq!(
            doc["tracking"]["stages"][0]["paths"]
                .as_array()
                .unwrap()
                .len(),
            MAX_MENTION_PATHS
        );
        assert_eq!(
            mention_paths(&stage, "h").len(),
            n,
            "cycling must reach the last one"
        );
        // The count was always exact; that is what makes the cap safe.
        assert_eq!(
            doc["tracking"]["stages"][0]["mentions"],
            serde_json::json!(n)
        );
    }

    /// The capture says what was on screen, and points at the phase code.
    ///
    /// Both were absent until 2026-07-28. Without `view`, a point made in a
    /// tree and one made paused at animation frame 12 produced identical files.
    /// Without `phase_source`, the algorithm could only be inferred from its
    /// output — which is backwards, and the whole reason HRW moved into the
    /// Rumoca workspace was to make the phase code readable.
    #[test]
    fn the_capture_reports_the_view_and_the_phase_source() {
        let empty = BTreeMap::new();
        let ask = Ask {
            seq: 1,
            request: None,
            specimen: None,
            model: Some("M"),
            stage: Some(StageKind::Events),
            libraries: vec![],
            def_index: &empty,
            parse_value: None,
            resolve_value: None,
            focus: Focus::Stage,
            tracking: None,
            view: View {
                ui_mode: "Specimen",
                stage_view: Some("MatchingAnim"),
                specimen_detail: Some("Source"),
                viewing_log: false,
                animation: Some(AnimationView {
                    which: "reduction",
                    frame: 12,
                    frame_count: 47,
                    live_state: "Running",
                    frame_context: Some(json!({ "step": "Round 0: state emf.phi" })),
                }),
            },
            failure: None,
            flags: vec![],
        };
        let doc = build(&ask);

        assert_eq!(doc["view"]["ui_mode"], json!("Specimen"));
        assert_eq!(doc["view"]["stage_view"], json!("MatchingAnim"));
        assert_eq!(doc["view"]["animation"]["frame"], json!(12));
        assert_eq!(doc["view"]["animation"]["frame_count"], json!(47));
        assert_eq!(doc["view"]["animation"]["live_state"], json!("Running"));
        // Position alone said where the user was; `showing` says what they were
        // looking at, which is what makes a mid-animation question answerable.
        assert!(
            doc["view"]["animation"]["showing"]["step"]
                .as_str()
                .is_some_and(|s| s.contains("emf.phi")),
            "the frame's own description must reach the capture: {}",
            doc["view"]["animation"],
        );

        assert_eq!(doc["phase_source"]["crate"], json!("crates/rumoca-ir-dae"));
        // It asserted "discrete" — `Dae::discrete`, which 0.10.0 removed — until the
        // 2026-11-07 sweep, so this test held the stale entry in place.
        // `every_phase_source_names_a_defined_function` now checks existence for all.
        assert!(
            doc["phase_source"]["entry"]
                .as_str()
                .is_some_and(|e| e.contains("DaeView::conditions")),
            "Events reads the condition algebra: {}",
            doc["phase_source"]["entry"],
        );
    }

    /// Every stage names a crate that exists. A `phase_source` pointing at a
    /// directory that was renamed during a rebase is worse than none — it sends
    /// the reader somewhere confidently wrong.
    #[test]
    fn every_phase_source_crate_exists_on_disk() {
        let root = std::path::Path::new(crate::manifest_path!(".."));
        for &stage in StageKind::ALL {
            let src = phase_source(stage);
            let dir = src["crate"].as_str().expect("crate path");
            assert!(
                root.join(dir).is_dir(),
                "{stage:?} points at `{dir}`, which is not a directory in the workspace",
            );
            assert!(!src["entry"].as_str().unwrap_or_default().is_empty());
        }
    }

    /// The captured node arrives with what it belongs to.
    ///
    /// Pointing at `def_id: 85` used to emit an integer and a path; answering
    /// "the def_id of *what*?" meant rebuilding the parent by hand.
    #[test]
    fn a_captured_node_carries_its_neighbourhood() {
        let root = json!({ "def_id": 85, "name": "MotorWithBrake", "class_type": "model" });
        let node = build_node(&[Seg::Key("def_id".into())], &root, None);

        assert_eq!(node["neighbourhood"]["value"], json!(85));
        assert_eq!(
            node["neighbourhood"]["context"]["name"],
            json!("MotorWithBrake")
        );
        let window: Vec<&str> = node["neighbourhood"]["siblings"]["window"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .collect();
        assert!(
            window.contains(&"name") && window.contains(&"class_type"),
            "{window:?}"
        );
    }

    /// A node outside the model class (e.g. the parse `within`) is not diffable.
    #[test]
    fn cross_stage_not_applicable_outside_class() {
        let parse = json!({ "classes": { "M": {} }, "within": "Foo" });
        let empty = BTreeMap::new();
        let ask = Ask {
            seq: 1,
            request: None,
            specimen: None,
            model: Some("M"),
            stage: Some(StageKind::Parse),
            libraries: vec![],
            def_index: &empty,
            parse_value: Some(&parse),
            resolve_value: None,
            focus: Focus::Node {
                key_path: vec![Seg::Key("within".into())],
                stage_value: &parse,
            },
            tracking: None,
            view: test_view(),
            failure: None,
            flags: vec![],
        };
        assert_eq!(build(&ask)["cross_stage"]["applicable"], json!(false));
    }

    /// The focus JSON's `stages.files` array must list exactly `STAGE_FILE_NAMES`.
    /// This test catches the staleness bug where new stages were added to
    /// `write_stages` but not to the focus JSON (TD-16).
    #[test]
    fn focus_json_stage_files_match_constant() {
        let empty = BTreeMap::new();
        let ask = Ask {
            seq: 1,
            request: None,
            specimen: None,
            model: Some("M"),
            stage: Some(StageKind::Parse),
            libraries: vec![],
            def_index: &empty,
            parse_value: None,
            resolve_value: None,
            focus: Focus::Specimen,
            tracking: None,
            view: test_view(),
            failure: None,
            flags: vec![],
        };
        let doc = build(&ask);
        let files = doc["stages"]["files"]
            .as_array()
            .expect("stages.files should be an array");
        let file_strs: Vec<&str> = files
            .iter()
            .map(|v| v.as_str().expect("each file should be a string"))
            .collect();
        assert_eq!(
            file_strs.as_slice(),
            STAGE_FILE_NAMES,
            "focus JSON's stages.files is out of sync with STAGE_FILE_NAMES"
        );
    }

    /// `STAGE_FILE_NAMES` must cover every stage the app writes via `write_stages`.
    /// This test checks the count matches the pipeline's stages so a new stage
    /// addition without updating the constant is caught.
    ///
    /// **The `- 1` here was Simulation's exemption, removed 2026-09-04.** Every stage now
    /// has a file, so the count is exact rather than exact-but-one — and an off-by-one
    /// that used to be absorbed by the subtraction now fails.
    #[test]
    fn stage_file_names_covers_all_pipeline_stages() {
        // One file per stage, Parse through Simulation, with no exemptions.
        let pipeline_stage_count = StageKind::ALL.len();
        assert_eq!(
            STAGE_FILE_NAMES.len(),
            pipeline_stage_count,
            "STAGE_FILE_NAMES has {} entries but the pipeline has {pipeline_stage_count} stages",
            STAGE_FILE_NAMES.len(),
        );
        // Every name must end with .json and be unique.
        for name in STAGE_FILE_NAMES {
            assert!(
                name.ends_with(".json"),
                "stage file name should end with .json: {name}"
            );
        }
        let unique: std::collections::HashSet<&&str> = STAGE_FILE_NAMES.iter().collect();
        assert_eq!(
            unique.len(),
            STAGE_FILE_NAMES.len(),
            "duplicate stage file names"
        );
    }

    /// `StageKind::stage_file_name` and `STAGE_FILE_NAMES` agree, in both directions.
    ///
    /// The test above pins the constant's shape — count, suffix, uniqueness — and
    /// nothing tied the *mapping* to it. `node_link_check` resolves a lab's node link by
    /// asking a `StageKind` for its file, so a stage whose spelling drifted from the
    /// constant would make every pointer into that stage read as `StageUnavailable`:
    /// **unjudged, which is not a failure.** The checker would go quiet on one stage and
    /// still report success, which is the shape it was built to end.
    #[test]
    fn every_stage_file_name_is_in_the_canonical_list() {
        let mut mapped: Vec<&str> = Vec::new();
        for stage in StageKind::ALL {
            let stage = *stage;
            match stage.stage_file_name() {
                Some(file) => {
                    assert!(
                        STAGE_FILE_NAMES.contains(&file),
                        "{} maps to {file:?}, which is not in STAGE_FILE_NAMES",
                        stage.slug(),
                    );
                    mapped.push(file);
                }
                None => assert_eq!(
                    stage,
                    StageKind::Simulation,
                    "{} has no stage file, but only Simulation may lack one \u{2014} it is \
                     a plot, not IR",
                    stage.slug(),
                ),
            }
        }
        let mut expected: Vec<&str> = STAGE_FILE_NAMES.to_vec();
        mapped.sort_unstable();
        expected.sort_unstable();
        assert_eq!(
            mapped, expected,
            "every entry in STAGE_FILE_NAMES must be reachable from some StageKind, or a \
             stage file exists that no link can address",
        );
    }

    /// Depth-first search for the first object with a usable `location`,
    /// recording its key-path into `path`.
    fn first_location_path(v: &Value, path: &mut Vec<Seg>) -> bool {
        match v {
            Value::Object(map) => {
                if map.get("location").is_some_and(is_location) {
                    return true;
                }
                for (k, child) in map {
                    path.push(Seg::Key(k.clone()));
                    if first_location_path(child, path) {
                        return true;
                    }
                    path.pop();
                }
                false
            }
            Value::Array(arr) => {
                for (i, child) in arr.iter().enumerate() {
                    path.push(Seg::Index(i));
                    if first_location_path(child, path) {
                        return true;
                    }
                    path.pop();
                }
                false
            }
            _ => false,
        }
    }

    #[test]
    fn seg_display_key() {
        assert_eq!(Seg::Key("components".into()).to_string(), "components");
    }

    #[test]
    fn seg_display_index() {
        assert_eq!(Seg::Index(3).to_string(), "[3]");
    }

    #[test]
    fn describe_path_uses_display() {
        let path = vec![
            Seg::Key("equations".into()),
            Seg::Index(0),
            Seg::Key("Connect".into()),
        ];
        assert_eq!(describe_path(&path), "equations[0].Connect");
    }

    #[test]
    fn cross_stage_fallback_message_for_unsupported_stage() {
        let empty = BTreeMap::new();
        let ask = Ask {
            seq: 1,
            request: None,
            specimen: None,
            model: Some("M"),
            stage: Some(StageKind::Typecheck),
            libraries: vec![],
            def_index: &empty,
            parse_value: Some(&json!({})),
            resolve_value: Some(&json!({})),
            focus: Focus::Node {
                key_path: vec![Seg::Key("x".into())],
                stage_value: &json!({"x": 1}),
            },
            tracking: None,
            view: test_view(),
            failure: None,
            flags: vec![],
        };
        let doc = build(&ask);
        let cs = &doc["cross_stage"];
        assert_eq!(cs["applicable"], json!(false));
        let reason = cs["reason"].as_str().unwrap();
        assert!(
            reason.contains("not yet implemented"),
            "expected 'not yet implemented' in reason, got: {reason}"
        );
    }

    #[test]
    fn ask_request_as_str_round_trips() {
        assert_eq!(AskRequest::DebugWhereSet.as_str(), "debug-where-set");
    }

    /// **Pointing declares a noun, never a verb** — Doug's ruling of 2026-11-07, and the
    /// must-fire half of removing `AskRequest::Explain`.
    ///
    /// The compiler already prevents a capture path from *presuming* `explain`, since the
    /// variant is gone. What it cannot see is the emitted document, which is where the
    /// presumption actually did its damage: `focus.json` said `"request": "explain"` for
    /// every point, and that reached Claude as a request Doug had never made. So this
    /// asserts on the **document**, for both cases, in the same test so they cannot
    /// drift apart:
    ///
    /// - a plain point emits `"request": null` — absence stated, not a default;
    /// - a point made through the debugger menu emits the verb it declared.
    ///
    /// Without the second case this would pass on a `build` that dropped the field
    /// entirely, which is the vacuous shape this repository keeps catching itself
    /// writing.
    #[test]
    fn a_point_carries_no_verb_unless_a_gesture_declared_one() {
        let val = json!({"name": "x"});
        let index = std::collections::BTreeMap::new();
        let ask_with = |request: Option<AskRequest>| Ask {
            seq: 1,
            request,
            specimen: None,
            model: Some("M"),
            stage: None,
            libraries: Vec::new(),
            def_index: &index,
            parse_value: None,
            resolve_value: None,
            focus: Focus::Node {
                key_path: vec![Seg::Key("name".to_owned())],
                stage_value: &val,
            },
            tracking: None,
            view: test_view(),
            failure: None,
            flags: vec![],
        };

        let plain = build(&ask_with(None));
        assert_eq!(
            plain["request"],
            Value::Null,
            "pointing declared no verb, so none may be emitted — the verb is in the chat",
        );
        assert_eq!(
            plain["kind"],
            json!("node"),
            "the noun is still fully emitted"
        );

        let declared = build(&ask_with(Some(AskRequest::DebugWhereSet)));
        assert_eq!(
            declared["request"],
            json!("debug-where-set"),
            "a verb the debugger menu declared must still reach Claude",
        );
    }

    /// **A question carries the exact context it was asked about.**
    ///
    /// The whole design rests on one property: the question and its context leave HRW
    /// as one document, so they cannot disagree. So this asserts the embedded `context`
    /// is **equal** to the focus document `build` makes from the same `Ask` — not merely
    /// present — because a question file whose context was assembled some other way
    /// would reintroduce the join-by-time gap with a different file name.
    #[test]
    fn a_question_carries_the_exact_context_it_was_asked_about() {
        let val = json!({"name": "x"});
        let index = std::collections::BTreeMap::new();
        let ask = Ask {
            seq: 4,
            request: None,
            specimen: None,
            model: Some("M"),
            stage: None,
            libraries: Vec::new(),
            def_index: &index,
            parse_value: None,
            resolve_value: None,
            focus: Focus::Node {
                key_path: vec![Seg::Key("name".to_owned())],
                stage_value: &val,
            },
            tracking: None,
            view: test_view(),
            failure: None,
            flags: vec![],
        };
        let question = Question {
            seq: 9,
            text: "why is this singular?",
        };
        let doc = build_question(&ask, &question);

        assert_eq!(doc["question"], json!("why is this singular?"));
        // **No resolution of "this" is recorded** (Doug, 2026-11-07): the file carried
        // `this` and `this_rule` until the resolver was removed. The instructions tell
        // Claude to ask instead.
        assert!(
            doc.get("this").is_none() && doc.get("this_rule").is_none(),
            "question.json resolves \"this\" for Claude; Doug ruled that Claude asks",
        );
        assert!(
            doc["instructions"]
                .as_str()
                .is_some_and(|s| s.contains("ASK DOUG")),
            "the instructions must say to ask when \"this\" is unclear",
        );
        assert_eq!(doc["seq"], json!(9), "the question has its own number");
        assert!(
            doc["asked_at"]
                .as_str()
                .is_some_and(|s| s.ends_with(" UTC")),
            "and a time, in the trail's own format: {}",
            doc["asked_at"],
        );
        assert_eq!(
            doc["context"],
            build(&ask),
            "the context is the SAME document focus.json gets — built, not copied",
        );
    }

    #[test]
    fn ask_stage_none_shows_navigated_definition() {
        let empty = BTreeMap::new();
        let ask = Ask {
            seq: 1,
            request: None,
            specimen: None,
            model: Some("M"),
            stage: None,
            libraries: vec![],
            def_index: &empty,
            parse_value: None,
            resolve_value: None,
            focus: Focus::Specimen,
            tracking: None,
            view: test_view(),
            failure: None,
            flags: vec![],
        };
        let doc = build(&ask);
        assert_eq!(doc["stage"], json!("(navigated definition)"));
    }

    /// The bridge must target the anchor's first *statement*, not its signature.
    ///
    /// Debuggers skip the prologue, so a signature-line request silently
    /// resolves one line lower — and the bridge-armed breakpoint then looks
    /// like a different location from a hand-set one, producing a phantom
    /// duplicate in VS Code's breakpoint list. See `find_live_trace_line`.
    #[test]
    fn find_live_trace_line_targets_first_body_statement() {
        let (path, line) = find_live_trace_line().expect("find_live_trace_line");
        let source = fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = source.lines().collect();
        let found = lines[line - 1];
        let sig = lines
            .iter()
            .position(|l| l.contains("pub fn live_trace_breakpoint("))
            .expect("anchor signature");

        assert!(
            line - 1 > sig,
            "target should be after the signature (line {}), got line {line}",
            sig + 1
        );
        let trimmed = found.trim();
        assert!(!trimmed.is_empty(), "target should not be a blank line");
        assert!(
            !trimmed.starts_with("//"),
            "target should not be a comment: {found}"
        );
        assert!(
            !trimmed.contains("pub fn"),
            "target should not be the signature line: {found}"
        );
        // The anchor's first statement records the frame index. If this fails,
        // the anchor body changed — check that it is still non-empty (see
        // `breakpoint_anchor_store_is_observable` in the structural crate).
        assert!(
            trimmed.contains("LAST_FRAME_INDEX"),
            "expected the anchor's first statement, got: {found}"
        );
    }

    /// A request path in the system temp directory — **never** the bridge
    /// directory.
    ///
    /// `.hrw-bridge/breakpoint-request.json` is watched by Doug's running VS Code,
    /// so writing an arm request there is not a simulation: a breakpoint appears in
    /// his editor. See [`arm_live_trace_breakpoint_to`] for why cleaning up
    /// afterwards was the wrong shape of answer.
    fn temp_request_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("hrw-breakpoint-request-{name}.json"))
    }

    /// Tests arm, remove, and ack together, against a temp path.
    ///
    /// **Every assertion is on the value the bridge function returned, never on the
    /// file it wrote.** Even off the watched path that is the better shape, and on
    /// the watched path it was a race: the extension consumes a request and unlinks
    /// it, and the read landed microseconds after the write, so it never lost —
    /// which is what a flake looks like before it surfaces.
    #[test]
    fn live_trace_breakpoint_arm_remove_and_ack() {
        let request = temp_request_path("arm-remove-ack");
        let ack = request.with_file_name("breakpoint-ack.json");
        let _ = fs::remove_file(&request);
        let _ = fs::remove_file(&ack);

        // arm
        let req = arm_live_trace_breakpoint_to(&request, Some("TestModel")).expect("arm");
        assert_eq!(req["version"], json!(1));
        assert_eq!(req["specimen"], json!("TestModel"));
        let bp = &req["breakpoints"][0];
        assert!(bp["path"].as_str().unwrap().contains("live_trace.rs"));
        assert!(bp["line"].as_u64().unwrap() > 0);
        assert_eq!(bp.get("condition"), None, "no condition field when absent");

        // remove
        let req = remove_live_trace_breakpoint_to(&request).expect("remove");
        assert_eq!(req["version"], json!(1));
        assert_eq!(req["action"], json!("remove"));
        assert!(
            req["breakpoints"][0]["path"]
                .as_str()
                .unwrap()
                .contains("live_trace.rs")
        );

        // ack — the LEGACY payload, which is what a stale extension writes.
        // It replies, so the handshake ends; it cannot say what it armed, so it
        // must not be read as armed. See `docs/ideas.md` #75.
        fs::write(&ack, r#"{"acked":true}"#).unwrap();
        let verdict = check_breakpoint_ack_at(&ack);
        assert_eq!(
            verdict,
            BreakpointAck::Unreportable,
            "a pre-#75 ack cannot report what it armed"
        );
        assert!(verdict.replied(), "it did reply, so the wait is over");
        assert!(
            !verdict.is_armed(),
            "but replying is not evidence that a breakpoint exists"
        );
        assert!(!ack.exists(), "ack file should be deleted");
        assert_eq!(
            check_breakpoint_ack_at(&ack),
            BreakpointAck::Pending,
            "should be Pending once the ack file is gone"
        );

        let _ = fs::remove_file(&request);
    }

    /// **Arming writes only where it was told to, and clears only the ack beside
    /// it.**
    ///
    /// The isolation the rest of this fix rests on. If `arm_live_trace_breakpoint_to`
    /// leaked back to the constants — a stray `BREAKPOINT_REQUEST_FILE` in the body,
    /// which is exactly the edit someone makes while "simplifying" — every test
    /// would resume arming breakpoints in Doug's editor and
    /// [`no_test_arms_a_breakpoint_on_the_watched_path`] below would still pass,
    /// because the call site would look correct.
    ///
    /// So this asserts on the bridge directory *not changing*: a real observation of
    /// the real path, and the one case where reading it is safe, because nothing in
    /// this test ever writes there.
    #[test]
    fn arming_to_a_temp_path_does_not_touch_the_bridge_directory() {
        let request = temp_request_path("isolation");
        let _ = fs::remove_file(&request);

        // Snapshot the real paths. Absent is the ordinary case and is a state too.
        let before_request = fs::read_to_string(BREAKPOINT_REQUEST_FILE).ok();
        let before_ack = fs::read_to_string(BREAKPOINT_ACK_FILE).ok();

        arm_live_trace_breakpoint_to(&request, Some("TestModel")).expect("arm");
        remove_live_trace_breakpoint_to(&request).expect("remove");

        assert!(
            request.exists(),
            "the temp path must be where the request actually went",
        );
        assert_eq!(
            fs::read_to_string(BREAKPOINT_REQUEST_FILE).ok(),
            before_request,
            "arming to a temp path must not write the watched request file",
        );
        assert_eq!(
            fs::read_to_string(BREAKPOINT_ACK_FILE).ok(),
            before_ack,
            "arming to a temp path must not clear the live bridge's ack",
        );

        let _ = fs::remove_file(&request);
    }

    /// **No test arms a breakpoint on the watched path.**
    ///
    /// Doug, 2026-08-15, after a first fix that only cleaned up afterwards:
    /// *"There's still a live_trace.rs breakpoint set."* Cleanup could not work,
    /// because **the extension declines to remove a breakpoint it did not add** —
    /// an arm onto an existing breakpoint is reported `alreadyEnabled` and never
    /// enters `armedBreakpoints`, the only list either removal path consults. A
    /// window reload empties that list, and anything armed before it becomes
    /// permanently unremovable by HRW *and* by the extension's own Clear command.
    ///
    /// So the invariant is not "clean up reliably", it is **never arm**. Every test
    /// uses `arm_live_trace_breakpoint_to` with a temp path; calling the
    /// bridge-path `arm_live_trace_breakpoint` from a test is the regression.
    ///
    /// # Why a source check, and what the previous one taught
    ///
    /// Nothing dynamic can see this: the damage lands in an editor, and `#75`
    /// records that VS Code exposes no `verified` field to extensions. The check it
    /// replaces needed **three attempts to fire**, each failing the same way —
    /// matching prose rather than code — and this one needed a fourth for the same
    /// reason in a new dress: **a source check is text, and its own text is part of
    /// the corpus.** Written literally, it matched itself, and the attribute it
    /// splits on appeared in its own prose, cutting a chunk out of a doc comment.
    ///
    /// So the needle is *assembled at runtime* and the attribute is never written
    /// out here. Both are load-bearing, not style: spelling either one literally
    /// makes this test report itself and mask a real offender behind the noise.
    /// # Why every source file, not just this one
    ///
    /// The first version scanned `bridge.rs` alone and passed while a full gate run
    /// was still arming a real breakpoint — `app.rs`'s `prewarm_arms_awaits_ack_then_removes`
    /// drives `tick_prewarm`, which arms the anchor for real. **A check scoped to
    /// the file it lives in reads as a check on the invariant**, and the gap was
    /// visible only in an ack timestamped inside the run.
    #[test]
    fn no_test_arms_a_breakpoint_on_the_watched_path() {
        // Assembled so these files do not contain the strings being searched for.
        let attribute = format!("#[{}]", "test");
        let watched_call = format!("{}(", "arm_live_trace_breakpoint");
        let prewarm_call = format!("{}(", "tick_prewarm");

        let src = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut offenders: Vec<String> = Vec::new();
        let mut scanned = 0usize;
        let mut files = 0usize;

        for entry in fs::read_dir(&src).expect("src/ is readable").flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let source = fs::read_to_string(&path).expect("a source file is readable");
            let file = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("?")
                .to_owned();
            files += 1;

            for chunk in source.split(attribute.as_str()).skip(1) {
                // Everything past the next doc comment belongs to a different item.
                let body = chunk.split("\n    ///").next().unwrap_or(chunk);
                let Some(name) = body
                    .lines()
                    .find(|l| l.trim_start().starts_with("fn "))
                    .map(|l| l.trim().to_owned())
                else {
                    // Not a test body — a prose fragment, or an attribute mentioned
                    // in a comment. Skipped rather than counted, so `scanned` stays
                    // a count of real bodies.
                    continue;
                };
                scanned += 1;
                // The direct call, and the app-level state machine that performs
                // one. Both reach the same watched file.
                if body.contains(watched_call.as_str()) || body.contains(prewarm_call.as_str()) {
                    offenders.push(format!("{file}: {name}"));
                }
            }
        }

        assert!(
            files >= 10 && scanned >= 100,
            "only {scanned} test bodies across {files} files were scanned; the run \
             or the split stopped matching, so this checks nothing",
        );
        assert!(
            offenders.is_empty(),
            "these tests arm the anchor on the WATCHED path, which puts a real \
             breakpoint in the editor that the extension will refuse to remove \
             later \u{2014} use `arm_live_trace_breakpoint_to` with \
             `temp_request_path`: {offenders:?}",
        );
    }

    /// **What the bridge functions return is what reaches the file.**
    ///
    /// `arm_live_trace_breakpoint` and `remove_live_trace_breakpoint` now return
    /// their request so tests need not re-read a path the VS Code extension
    /// deletes. That trade is only sound if the returned value and the written
    /// bytes cannot disagree — otherwise every assertion above would be checking a
    /// value that never reached disk, which is a *worse* failure than the race it
    /// replaced: it would pass while the extension received something else.
    ///
    /// Verified against a **temp path**, which nothing watches, so the round trip
    /// is an ordinary file test with no second process in it.
    #[test]
    fn a_written_breakpoint_request_round_trips_to_the_returned_value() {
        let path = std::env::temp_dir().join("hrw-breakpoint-request-test.json");
        let _ = fs::remove_file(&path);

        // The two real shapes, so neither is checked only in the abstract.
        for request in [
            json!({
                "version": 1,
                "specimen": "TestModel",
                "breakpoints": [{ "path": "x/live_trace.rs", "line": 173 }],
            }),
            json!({
                "version": 1,
                "action": "remove",
                "breakpoints": [{ "path": "x/live_trace.rs", "line": 173 }],
            }),
        ] {
            write_breakpoint_request_to(&path, &request).expect("write");
            let text = fs::read_to_string(&path).expect("read back");
            let parsed: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");
            assert_eq!(
                parsed, request,
                "the file must carry exactly the request the caller was handed, or \
                 asserting on the return value proves nothing about what the \
                 extension will read",
            );
        }

        let _ = fs::remove_file(&path);
    }

    /// **An ack that cannot answer must not be read as either answer**
    /// (`docs/ideas.md` #75).
    ///
    /// The old payload was `{"acked": true}` written after *every* request,
    /// including ones that armed nothing. Reading it as armed reinstates `#71`'s
    /// fiction; reading it as not-armed silently breaks live trace against a
    /// stale extension. Neither is acceptable, so it gets its own verdict and
    /// HRW says so on screen.
    ///
    /// **The stale-extension case is not hypothetical**: on 2026-08-08 this
    /// machine ran a build twelve days behind its source, because `git pull`
    /// runs no `tsc`. `Unreportable` is what makes that announce itself.
    #[test]
    fn an_ack_without_a_verdict_is_unreportable_not_armed() {
        assert_eq!(
            parse_breakpoint_ack(r#"{"acked":true}"#),
            BreakpointAck::Unreportable
        );
        assert!(!parse_breakpoint_ack(r#"{"acked":true}"#).is_armed());
        assert!(parse_breakpoint_ack(r#"{"acked":true}"#).replied());
    }

    /// The three verdicts a current bridge can send, and the reason survives.
    #[test]
    fn a_verdict_ack_is_read_exactly_as_sent() {
        assert_eq!(
            parse_breakpoint_ack(r#"{"version":2,"breakpointPresent":true}"#),
            BreakpointAck::Armed
        );

        let disabled = parse_breakpoint_ack(
            r#"{"version":2,"breakpointPresent":false,"reason":"a breakpoint exists at live_trace.rs:173 but is DISABLED"}"#,
        );
        assert!(!disabled.is_armed(), "a disabled breakpoint stops nothing");
        match disabled {
            // The reason reaches the user, so it must not be dropped in parsing:
            // "not armed" without a cause is the silence this feature removes.
            BreakpointAck::NotArmed(why) => assert!(why.contains("DISABLED"), "{why}"),
            other => panic!("expected NotArmed, got {other:?}"),
        }

        // Absent reason is still NotArmed — a bridge that forgets to explain
        // itself must not be promoted to success.
        assert_eq!(
            parse_breakpoint_ack(r#"{"version":2,"breakpointPresent":false}"#),
            BreakpointAck::NotArmed("the bridge did not say why".to_owned())
        );
    }

    /// Garbage on disk is a failure, not a success and not a crash.
    ///
    /// A torn or truncated write is the plausible cause; the extension writes
    /// the ack directly rather than via temp-file+rename, unlike
    /// `debug-state.json`.
    #[test]
    fn an_unparseable_ack_is_not_armed() {
        let ack = parse_breakpoint_ack("{not json");
        assert!(!ack.is_armed());
        assert!(ack.replied(), "something was there, so the wait is over");
        assert!(matches!(ack, BreakpointAck::NotArmed(_)));
    }

    #[test]
    fn write_stages_creates_and_removes_files() {
        let val = json!({"equations": [1, 2, 3]});
        let stages: Vec<(&str, Option<&Value>)> =
            vec![("test_alpha", Some(&val)), ("test_beta", None)];
        write_stages(&stages).expect("write_stages");
        let alpha_path = Path::new(STAGES_DIR).join("test_alpha.json");
        assert!(alpha_path.exists(), "stage file should be created");
        let content: Value =
            serde_json::from_str(&fs::read_to_string(&alpha_path).unwrap()).unwrap();
        assert_eq!(content["equations"], json!([1, 2, 3]));

        let beta_path = Path::new(STAGES_DIR).join("test_beta.json");
        assert!(!beta_path.exists(), "None stage should not create a file");

        // A second call with None removes a previously written file.
        let stages_remove: Vec<(&str, Option<&Value>)> = vec![("test_alpha", None)];
        write_stages(&stages_remove).expect("cleanup write");
        assert!(
            !alpha_path.exists(),
            "stage file should be removed when None"
        );
    }

    #[test]
    fn write_creates_focus_json() {
        let val = json!({"name": "test_write_model"});
        let ask = Ask {
            seq: 99,
            request: None,
            specimen: None,
            model: Some("TestWriteModel"),
            stage: None,
            libraries: Vec::new(),
            def_index: &std::collections::BTreeMap::new(),
            parse_value: None,
            resolve_value: None,
            focus: Focus::Node {
                key_path: vec![Seg::Key("name".to_owned())],
                stage_value: &val,
            },
            tracking: None,
            view: test_view(),
            failure: None,
            flags: vec![],
        };
        let path = write(&ask).expect("write focus");
        assert!(path.exists(), "focus.json should exist");
        let content: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(content["seq"], json!(99));
        assert_eq!(content["model"], json!("TestWriteModel"));
    }

    /// **No test may address a live bridge file.**
    ///
    /// The companion to `diagnostics::tests_never_write_the_live_diagnostic_files`, and
    /// written for the same reason two months later: on 2026-11-06 one `cargo test` run
    /// was found to have rewritten `focus.json`, all eleven `stages/*.json`, `answer.md`,
    /// `breakpoint-ack.json` and a scratch specimen in the **live** directory.
    ///
    /// **Why this is the dangerous kind of defect rather than untidiness.** The gate runs
    /// before every commit, and `CLAUDE.md` sends Claude to `stages/` as *"the file the
    /// pane is actually fed"*. So Doug captures a node, Claude runs the gate, Doug asks
    /// *"what does this mean?"* — and the artifacts Claude is instructed to trust describe
    /// the suite. **Nothing in them is false**, which is precisely why it produces a
    /// confident answer about the wrong subject and no symptom either of them can see.
    ///
    /// The assertion walks the consts rather than one path, because the fix is only
    /// exhaustive while **every** bridge-rooted const stays redirected — a new one added
    /// without its `cfg(test)` twin is the way this comes back.
    #[test]
    fn tests_never_write_the_live_bridge_files() {
        // Normalise separators on BOTH sides. `CARGO_MANIFEST_DIR` is backslashed on
        // Windows while the literals below are not, so comparing one normalised string
        // against one raw one fails on a path that is in fact correct.
        let live = format!("{}/.hrw-bridge", env!("CARGO_MANIFEST_DIR")).replace('\\', "/");
        let redirected = format!("{live}/test-run");

        let paths: &[(&str, &str)] = &[
            ("BRIDGE_DIR", BRIDGE_DIR),
            ("STAGES_DIR", STAGES_DIR),
            ("SCRATCH_SPECIMEN_DIR", SCRATCH_SPECIMEN_DIR),
            ("SCRATCH_NOTEBOOKS_DIR", SCRATCH_NOTEBOOKS_DIR),
            ("QUESTION_FILE", QUESTION_FILE),
            ("VIEW_FILE", VIEW_FILE),
            ("SCREENSHOTS_DIR", SCREENSHOTS_DIR),
            ("SCREENSHOT_REQUEST_FILE", SCREENSHOT_REQUEST_FILE),
            ("SCREENSHOT_RESULT_FILE", SCREENSHOT_RESULT_FILE),
            ("BREAKPOINT_REQUEST_FILE", BREAKPOINT_REQUEST_FILE),
            ("BREAKPOINT_ACK_FILE", BREAKPOINT_ACK_FILE),
        ];

        for (name, path) in paths {
            let p = path.replace('\\', "/");
            assert!(
                p.starts_with(&redirected),
                "{name} must resolve under {redirected} while testing, not the live \
                 bridge — a `cargo test` run would overwrite Doug's own file. Got: {p}",
            );
        }

        // Non-vacuity: the two prefixes must actually differ, or every assertion above
        // passes on a redirect that redirects nothing.
        assert_ne!(
            live, redirected,
            "the test prefix must differ from the live one",
        );
    }

    fn test_file_path() -> String {
        format!("{}/src/bridge.rs", env!("CARGO_MANIFEST_DIR"))
    }

    #[test]
    fn slice_source_zero_length() {
        let path = test_file_path();
        let (_, excerpt, _) = slice_source(&path, None, 0, 0).expect("zero-length should succeed");
        assert!(excerpt.is_empty());
    }

    #[test]
    fn slice_source_file_start() {
        let path = test_file_path();
        let (_, excerpt, _) = slice_source(&path, None, 0, 2).expect("file start should succeed");
        assert_eq!(excerpt.len(), 2);
    }

    /// A **relative** `file_name` is never resolved against the working directory.
    ///
    /// `Path::new("Cargo.toml").is_file()` is true whenever HRW runs from a crate
    /// root — so the old code would open *that* file and emit an excerpt from it,
    /// attributed to whatever location produced the name. Nothing crashes; the
    /// answer is simply about a different file.
    ///
    /// **This guards a shape, not a live bug.** Every `Location` HRW emits today
    /// carries a full document URI. That is precisely when the guard is cheapest,
    /// and the failure it prevents — a confident wrong excerpt — is the one this
    /// project treats as unrecoverable.
    ///
    /// Uses `Cargo.toml` deliberately: it is real, it is readable, and it is
    /// nothing to do with Modelica, so a slice taken from it could only have come
    /// from resolving the name against the wrong directory.
    #[test]
    fn slice_source_refuses_a_relative_name() {
        let specimen = test_file_path();
        assert!(
            Path::new("Cargo.toml").is_file(),
            "precondition: the test runs from the crate root, so a relative name WOULD \
             resolve — without this the test proves nothing",
        );

        let (used, _, _) = slice_source("Cargo.toml", Some(Path::new(&specimen)), 0, 2)
            .expect("it must fall back to the specimen, not fail");

        assert_eq!(
            used, specimen,
            "a relative name must be ignored in favour of the specimen; resolving it \
             against the working directory slices a stranger's file and reports it as \
             the model's source",
        );
    }

    #[test]
    fn slice_source_file_end() {
        let path = test_file_path();
        let src = fs::read_to_string(&path).unwrap();
        let len = src.len();
        assert!(
            slice_source(&path, None, len - 2, len).is_some(),
            "file end should succeed"
        );
    }

    #[test]
    fn slice_source_invalid_range() {
        let path = test_file_path();
        assert!(slice_source(&path, None, 5, 3).is_none());
    }

    #[test]
    fn slice_source_past_end() {
        let path = test_file_path();
        let src = fs::read_to_string(&path).unwrap();
        assert!(slice_source(&path, None, 0, src.len() + 1).is_none());
    }
}

#[cfg(test)]
mod view_file_tests {
    use super::*;

    fn temp_view_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("hrw-view-test-{name}.json"))
    }

    /// **A view with no publisher REMOVES the file; it never leaves the last one.**
    ///
    /// The 2026-08-15 sweep found this asserted in a doc comment and checked nowhere.
    /// It is the property the whole file rests on: a stale `view.json` naming a pane
    /// the reader has left is **indistinguishable from a current one by content
    /// alone**, so Claude would answer questions about the equation sheet while Doug
    /// stood in front of the incidence matrix — confidently, and with a real file to
    /// point at.
    ///
    /// Uses a temp path rather than `VIEW_FILE`, which is a single constant and would
    /// clobber the running app's live view.
    #[test]
    fn a_view_with_no_publisher_removes_the_file() {
        let path = temp_view_path("absence");
        let body = json!({"rows": [1, 2, 3]});

        write_view_to(&path, Some("Flatten/EquationSheet"), Some(&body)).expect("write");
        assert!(path.exists(), "a published view must be written");

        let doc: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(
            doc["view"], "Flatten/EquationSheet",
            "the file must name which pane it describes, or a reader cannot tell \
             whether it is the one on screen",
        );
        assert_eq!(doc["content"]["rows"], json!([1, 2, 3]));

        write_view_to(&path, None, None).expect("remove");
        assert!(
            !path.exists(),
            "a pane with no publisher must remove the file \u{2014} leaving the \
             previous pane's contents is a confidently wrong report",
        );

        let _ = fs::remove_file(&path);
    }

    /// A half-specified call is treated as absence, not as a partial write.
    ///
    /// `kind` without `body` (or the reverse) would otherwise produce a file naming a
    /// pane whose content is missing, which reads as "this pane is empty".
    #[test]
    fn a_half_specified_view_is_absence() {
        let path = temp_view_path("half");
        let body = json!({"rows": []});
        write_view_to(&path, Some("Structural/Incidence"), Some(&body)).expect("write");
        assert!(path.exists());

        write_view_to(&path, Some("Structural/Incidence"), None).expect("kind only");
        assert!(!path.exists(), "kind without body is absence");

        write_view_to(&path, Some("Structural/Incidence"), Some(&body)).expect("write");
        write_view_to(&path, None, Some(&body)).expect("body only");
        assert!(!path.exists(), "body without kind is absence");

        let _ = fs::remove_file(&path);
    }
}

/// The screenshot channel's file protocol: what a request may say, what a reply
/// must say, and that the PNG is a PNG.
///
/// The *timing* half — settle, deadlines, once-only — lives in
/// `crate::screenshot`, which owns the clock. This module owns the bytes.
#[cfg(test)]
mod screenshot_protocol_tests {
    use super::*;

    fn temp_png_path(name: &str) -> PathBuf {
        // A temp path rather than `screenshot_path`, which names the live bridge
        // directory and would overwrite a photograph the running app just took.
        std::env::temp_dir().join(format!("hrw-shot-test-{name}.png"))
    }

    fn a_view() -> View<'static> {
        View {
            ui_mode: "Lab",
            stage_view: Some("Summary"),
            specimen_detail: None,
            viewing_log: false,
            animation: None,
        }
    }

    /// **`seq` is the whole once-only rule**, so a request without one must be
    /// refused rather than defaulted. A defaulted `seq` would be re-honoured on
    /// every restart, navigating Doug's window while he was using it.
    #[test]
    fn a_request_without_a_numeric_seq_is_refused() {
        for text in [
            "{}",
            r#"{"why": "no seq here"}"#,
            r#"{"seq": "3"}"#,
            r#"{"seq": null}"#,
            "not json at all",
        ] {
            let err = parse_screenshot_request(text)
                .expect_err(&format!("{text} should not parse as a request"));
            assert!(
                err.contains("seq") || err.contains("JSON"),
                "the reason must name what is wrong, since it is written back as the \
                 reply: {err}"
            );
        }
    }

    /// A single link may be written bare. The list is the general form, but the
    /// one-link case is the common one and a protocol detail costing a round trip to
    /// discover is a protocol detail too many.
    #[test]
    fn follow_accepts_one_link_or_a_list() {
        let bare = parse_screenshot_request(r#"{"seq": 1, "follow": "hrw://simulate"}"#)
            .expect("a bare string is a one-link list");
        assert_eq!(bare.follow, vec!["hrw://simulate".to_string()]);

        let listed = parse_screenshot_request(
            r#"{"seq": 2, "follow": ["hrw://load/RcCircuit", "hrw://simulate"]}"#,
        )
        .expect("a list is the general form");
        assert_eq!(listed.follow.len(), 2);

        let absent = parse_screenshot_request(r#"{"seq": 3}"#)
            .expect("no `follow` means photograph what is already on screen");
        assert!(absent.follow.is_empty());

        // A non-string entry is refused rather than skipped: skipping would
        // photograph a state the request did not describe.
        let err = parse_screenshot_request(r#"{"seq": 4, "follow": [7]}"#)
            .expect_err("a numeric link is not a link");
        assert!(err.contains("non-string"), "{err}");
    }

    /// The settle interval is the request's to set, and the default is used when it
    /// does not — stated here because the default is what nearly every request gets.
    #[test]
    fn settle_comes_from_the_request_or_the_default() {
        let stated = parse_screenshot_request(r#"{"seq": 1, "settleMs": 1500}"#).unwrap();
        assert_eq!(stated.settle, std::time::Duration::from_millis(1500));
        let unstated = parse_screenshot_request(r#"{"seq": 1}"#).unwrap();
        assert_eq!(unstated.settle, SCREENSHOT_SETTLE_DEFAULT);
    }

    /// **An unrecognised `region` is refused, not quietly defaulted.** A request for a
    /// crop answered with a whole window would send the reader a picture of more than
    /// they asked about, with only the reply's own `region` field to say so.
    #[test]
    fn region_is_window_or_pane_and_nothing_else() {
        assert_eq!(
            parse_screenshot_request(r#"{"seq": 1}"#).unwrap().region,
            ShotRegion::Window,
            "the whole window is the default, so a shot keeps the chrome unless it \
             says otherwise"
        );
        assert_eq!(
            parse_screenshot_request(r#"{"seq": 1, "region": "pane"}"#)
                .unwrap()
                .region,
            ShotRegion::Pane
        );
        let err = parse_screenshot_request(r#"{"seq": 1, "region": "centre"}"#)
            .expect_err("an unknown region is not a region");
        assert!(err.contains("centre"), "{err}");
    }

    /// **The photograph must be a readable PNG**, which is the entire point of the
    /// dependency: Claude's own file reader accepts PNG and not the uncompressed
    /// formats that would have needed no crate at all.
    ///
    /// Decoded with `png`'s own reader rather than compared byte-for-byte against a
    /// stored file: a byte comparison would pin the encoder's compression choices,
    /// which is the brittleness that ruled out image snapshots in the first place.
    #[test]
    fn write_png_round_trips_through_a_decoder() {
        let path = temp_png_path("round-trip");
        let _ = fs::remove_file(&path);
        // Two rows of two, deliberately asymmetric so a transposed or mirrored write
        // cannot pass: row-major from the TOP is the contract.
        let pixels: Vec<u8> = vec![
            255, 0, 0, 255, // top-left: red
            0, 255, 0, 255, // top-right: green
            0, 0, 255, 255, // bottom-left: blue
            10, 20, 30, 128, // bottom-right: translucent dark
        ];
        write_png(&path, 2, 2, &pixels).expect("the PNG should write");

        let decoder = png::Decoder::new(std::io::BufReader::new(
            fs::File::open(&path).expect("the file exists"),
        ));
        let mut reader = decoder.read_info().expect("a valid PNG header");
        let info = reader.info();
        assert_eq!((info.width, info.height), (2, 2));
        assert_eq!(info.color_type, png::ColorType::Rgba);
        assert_eq!(info.bit_depth, png::BitDepth::Eight);
        let mut out = vec![0; reader.output_buffer_size().unwrap()];
        let frame = reader.next_frame(&mut out).expect("one frame");
        assert_eq!(
            &out[..frame.buffer_size()],
            &pixels[..],
            "the decoded pixels must be the ones written, in the order written"
        );

        let _ = fs::remove_file(&path);
    }

    /// A byte count that does not match the dimensions is refused **before** the
    /// encoder sees it.
    ///
    /// Not defensive clutter: `width`, `height` and the buffer arrive from three
    /// different places in `collect_screenshot` (the event's `ColorImage::size` and
    /// a conversion over its pixels), and a PNG written from a mismatched trio
    /// decodes to garbage or panics inside the encoder — either of which would be
    /// discovered by a reader looking at a corrupted photograph and wondering what
    /// the app had done.
    #[test]
    fn write_png_refuses_a_buffer_that_does_not_match_its_dimensions() {
        let path = temp_png_path("mismatch");
        let _ = fs::remove_file(&path);
        let err = write_png(&path, 2, 2, &[0; 12]).expect_err("12 bytes is not 2x2 RGBA");
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
        assert!(
            err.to_string().contains("needs 16"),
            "the error should say what was expected: {err}"
        );
        assert!(
            !path.exists(),
            "and must not leave a truncated file behind to be read as the photograph"
        );
    }

    /// **A reply with no image says so, in the field a reader checks.**
    ///
    /// The failure mode this closes: the previous request's PNG is still on disk, so
    /// a reply that merely omitted `image` would let Claude find that file, read it
    /// as this request's answer, and describe a screen that was never photographed.
    #[test]
    fn a_failed_reply_names_the_error_and_carries_no_image() {
        let showing = a_view();
        let followed = vec!["hrw://load/Drivetrain".to_string()];
        let doc = screenshot_result_json(&ShotResult {
            seq: Some(9),
            why: Some("does the tab row offer Matching?"),
            followed: &followed,
            image: None,
            region: ShotRegion::Window,
            showing: Some(&showing),
            specimen: Some("Drivetrain"),
            stage: "IndexReduction",
            notice: Some("no specimen loaded \u{2014} this link needs one."),
            error: Some("the backend never delivered the pixels"),
        });
        assert_eq!(doc["seq"], 9);
        assert!(
            doc["image"].is_null(),
            "no image means null, not a stale path"
        );
        assert_eq!(doc["error"], "the backend never delivered the pixels");
        assert!(
            doc["writtenAtMs"].as_u64().is_some_and(|ms| ms > 0),
            "every reply is stamped, so one left over from an earlier request is \
             detectable rather than mistaken for this one's"
        );
        assert_eq!(doc["why"], "does the tab row offer Matching?");
        assert_eq!(doc["followed"][0], "hrw://load/Drivetrain");
        assert_eq!(doc["specimen"], "Drivetrain");
        assert_eq!(doc["stage"], "IndexReduction");
        assert_eq!(doc["showing"]["stage_view"], "Summary");
        assert!(
            doc["notice"]
                .as_str()
                .is_some_and(|n| n.contains("no specimen loaded")),
            "the notice is where a REFUSED link is recorded; the window itself looks \
             healthy when HRW refuses one, so a photograph cannot show it"
        );
    }

    /// And a successful reply carries the measurement, not the request's guess.
    #[test]
    fn a_successful_reply_carries_the_captured_size() {
        let showing = a_view();
        let doc = screenshot_result_json(&ShotResult {
            seq: Some(10),
            why: None,
            followed: &[],
            image: Some(ShotImage {
                path: PathBuf::from("/tmp/shot-0010.png"),
                width: 2560,
                height: 1395,
            }),
            region: ShotRegion::Pane,
            showing: Some(&showing),
            specimen: None,
            stage: "Resolve",
            notice: None,
            error: None,
        });
        assert!(doc["error"].is_null());
        assert_eq!(doc["image"]["width"], 2560);
        assert_eq!(doc["image"]["height"], 1395);
        assert!(
            doc["image"]["path"]
                .as_str()
                .is_some_and(|p| p.ends_with("shot-0010.png"))
        );
    }

    /// The PNG name sorts in request order.
    #[test]
    fn screenshot_paths_sort_in_request_order() {
        let mut names: Vec<String> = [2u64, 10, 1]
            .iter()
            .map(|seq| {
                screenshot_path(*seq)
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        names.sort();
        assert_eq!(
            names,
            vec![
                "shot-0001.png".to_string(),
                "shot-0002.png".to_string(),
                "shot-0010.png".to_string()
            ],
            "a lexicographic listing must read as the sequence it is"
        );
    }
}
