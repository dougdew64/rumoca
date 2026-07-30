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
//!    (what the user wants: "explain" or "debug-where-set"), `kind` (node/stage/
//!    specimen), and the node/provenance/cross-stage data.
//!
//! 2. **Stage files** (`.hrw-bridge/stages/<name>.json`): one file per pipeline
//!    stage's full IR, rewritten once per compile. Claude can diff any two stages
//!    by reading two files (e.g., `instantiate.json` vs `typecheck.json`).
//!
//! 3. **Tour file** (`.hrw-bridge/tour.md`) — the one channel that runs the *other
//!    way*, added 2026-07-29 (ideas #42). Claude writes a markdown tour; HRW's
//!    tour mode renders it and picks up a rewrite without a restart. Where
//!    `focus.json` carries a noun *out* to Claude, this carries a sequence of
//!    nouns *back*, as `hrw://` links the reader can click.
//!
//!    Living in the gitignored bridge directory is deliberate: #42 says ad hoc
//!    tours are **ephemeral by default** — regenerated against the current tree
//!    rather than retrieved and re-checked — and putting them here makes the
//!    filesystem enforce that instead of Claude's discipline. What persists is
//!    the *question* the tour answered, in `docs/question-ledger.md`.
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
//! So the bridge walks **up** the tree from the clicked node to the root,
//! looking for the tightest enclosing provenance:
//! - `location` (preferred): `rumoca_core::Location` with byte offsets + `file_name`
//! - `span` (fallback): `rumoca_core::Span` with byte offsets + opaque `source` id
//!
//! Once found, the byte range is sliced out of the Modelica source file, and
//! the enclosing line(s) are included for context. This walk is fully generic
//! (it pattern-matches on JSON structure, not Rumoca types), maintaining the
//! one-generic-tree rule.

use std::collections::BTreeMap;
use std::fmt::{self, Write as _};
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

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
pub const BRIDGE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/.hrw-bridge");

/// Path to the stage-files directory (one JSON file per pipeline stage).
///
/// These files are rewritten once per compile. Claude can diff any two stages
/// by reading two files — e.g., comparing `instantiate.json` to `typecheck.json`
/// shows what the instanced typecheck phase added (type_ids resolved, dimensions
/// evaluated). This avoids bloating the focus file with all stages' IR.
pub const STAGES_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/.hrw-bridge/stages");

/// HRW writes breakpoint requests here; the VS Code extension watches it.
const BREAKPOINT_REQUEST_FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/.hrw-bridge/breakpoint-request.json");
/// The extension writes this ack file after processing a request, confirming
/// the breakpoint is registered with LLDB. HRW polls for it before spawning
/// the algorithm thread (see `check_breakpoint_ack`).
pub(crate) const BREAKPOINT_ACK_FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/.hrw-bridge/breakpoint-ack.json");

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
/// construction**, the same rule as `tour.md`: what persists is the *question*, in
/// `docs/question-ledger.md`. A probe worth keeping gets promoted into `specimens/`
/// deliberately, with a `// purpose:` line — which is the moment it stops being a probe.
pub const SCRATCH_SPECIMEN_DIR: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/.hrw-bridge/specimens");

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

/// An ad hoc tour written by Claude, rendered by HRW's tour mode (ideas #42).
///
/// The only bridge file that flows *into* HRW rather than out of it.
pub const TOUR_FILE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/.hrw-bridge/tour.md");

/// Fixture tours — kept, versioned, and executed by `fixture_tour_links_all_resolve`.
///
/// Hard-coded rather than configurable: the directory is part of the repository layout,
/// not a user preference, and one fewer setting is one fewer thing to be wrong.
pub const FIXTURE_TOURS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/docs/fixture-tours");

/// Notebooks belonging to fixture tours — versioned, because a fixture has expected
/// outcomes and a test that vanishes on a fresh checkout is not a test.
pub const FIXTURE_NOTEBOOKS_DIR: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/docs/fixture-tours/notebooks");

/// Notebooks Claude writes to answer one question — ephemeral, like `tour.md`.
pub const SCRATCH_NOTEBOOKS_DIR: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/.hrw-bridge/notebooks");

/// Resolve a `hrw://notebook/<name>` target to a file on disk.
///
/// Looks in the fixture directory first, then the scratch one, so a fixture tour keeps
/// working even when an ad hoc notebook of the same name exists.
///
/// **Rejects anything with a path separator or `..`.** A tour is authored by Claude and
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

/// List the fixture tours, sorted by file name.
///
/// Distinct from the ad hoc tour in [`TOUR_FILE`]: an ad hoc tour answers one question
/// and is regenerated, a fixture tour is a **test** with a pass/fail criterion and is
/// kept. See `docs/fixture-tours/camera-aiming.md` for the shape.
pub fn fixture_tours() -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(FIXTURE_TOURS_DIR) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("md"))
        .collect();
    found.sort();
    found
}

/// Read the ad hoc tour, with the modification time it was read at.
///
/// Returns `None` when no tour has been written — the common case, and not an
/// error. The mtime lets a caller re-read only when the file actually changed,
/// so a tour Claude rewrites mid-conversation appears without a restart and an
/// unchanged one costs one `stat` per poll.
pub fn read_tour() -> Option<(String, std::time::SystemTime)> {
    let meta = fs::metadata(TOUR_FILE).ok()?;
    let mtime = meta.modified().ok()?;
    let text = fs::read_to_string(TOUR_FILE).ok()?;
    Some((text, mtime))
}

/// Absolute path to `live_trace_breakpoint` in the structural crate, resolved
/// at compile time so the VS Code extension can set breakpoints without path
/// resolution.
const LIVE_TRACE_FILE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../crates/rumoca-phase-structural/src/live_trace.rs"
);

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
    "structural.json",
    "index_reduction.json",
    "initialization.json",
    "events.json",
    "solve_lowering.json",
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
One node, one stage, chosen deliberately. For `explain` this is the subject; \
for `debug-where-set` it wants ONE breakpoint, where that value is set.\n\
  • FOLLOWING — the `tracking` section. One identifier, everywhere it appears \
across every stage. For `explain` this is the lens; for `debug-where-set` it \
wants SEVERAL breakpoints along the identifier's trajectory.\n\n\
Either may be absent, and absence is stated rather than implied. \
`kind: \"none\"` means the user DELIBERATELY CLEARED the point (or never made \
one), so the `tracking` section is the whole subject — do not fall back to \
describing the current stage, which they did not choose. A missing `tracking` \
section likewise means nothing is being followed. `request` is a property of \
the point, so it is null whenever `kind` is \"none\". If BOTH are absent, \
nothing has been assembled: say so rather than answering from `stage` or from \
whatever was captured before.\n\n\
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
A `tracking.generated` section means the followed name was SYNTHESIZED by a compiler phase and is declared nowhere; `declared_at_line` is absent in that case rather than pointing at the base variable's declaration, which would name a different variable.

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
    /// `pub` as `get_in` for the tree, which walks a jump target's ancestors to
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

/// Format a key-path as a human-readable dotted string.
///
/// Examples:
/// - `[Key("components"), Key("inertia"), Key("def_id")]` -> `"components.inertia.def_id"`
/// - `[Key("equations"), Index(0), Key("Connect")]` -> `"equations[0].Connect"`
/// - `[]` -> `"(tree root)"`
///
/// Used in the UI status bar to show what was captured, and in the bridge
/// focus file as a human-readable path alongside the machine-readable key array.
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
/// (tree root)  or  <empty>      ->  []            (the root itself)
/// ```
///
/// Returns `None` on anything malformed rather than guessing: a link that silently
/// pointed at the wrong node would be worse than one that visibly does nothing, because
/// the reader would take the wrong subtree for the answer.
pub fn parse_path(s: &str) -> Option<Vec<Seg>> {
    if s.is_empty() || s == "(tree root)" {
        return Some(Vec::new());
    }
    let mut out = Vec::new();
    let mut rest = s;
    loop {
        // A key runs to the next '.' or '[' — but an index may come first, when a
        // previous segment ended with one (`blocks[2][0]`).
        if let Some(after) = rest.strip_prefix('[') {
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

pub fn describe_path(path: &[Seg]) -> String {
    if path.is_empty() {
        return "(tree root)".to_owned();
    }
    let mut s = String::new();
    for seg in path {
        match seg {
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
    Node { key_path: Vec<Seg>, stage_value: &'a Value },
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
}

/// What the user wants Claude to do with the captured focus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AskRequest {
    Explain,
    DebugWhereSet,
}

impl AskRequest {
    pub fn as_str(self) -> &'static str {
        match self {
            AskRequest::Explain => "explain",
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
/// For the Context Bar, which states what a question will have to work with
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
/// `"a.b[0].c"` cannot be walked, and re-parsing one would be guesswork the
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
    find_mentions(stage_value, name, &mut Vec::new(), &mut paths, &mut total, usize::MAX);
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
                find_mentions(v, t.name, &mut Vec::new(), &mut paths, &mut total, MAX_MENTION_PATHS);
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
    /// Which of the three left-panel modes: Tour, Specimen, Debug.
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
/// `worker.rs`), not an interpretation of what the phases do. `None` for
/// Simulation-adjacent entries that HRW reaches through a different path.
fn phase_source(stage: StageKind) -> Value {
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
        StageKind::Flatten => {
            ("crates/rumoca-compile", "Session::compile_model_strict_reachable_with_recovery")
        }
        StageKind::Structural => ("crates/rumoca-phase-structural", "build_structural_report"),
        StageKind::IndexReduction => (
            "crates/rumoca-phase-structural",
            "dae_prepare::reduce_constrained_dummy_derivatives / \
             dae_prepare::index_reduce_missing_state_derivatives",
        ),
        StageKind::Initialization => ("crates/rumoca-phase-structural", "build_ic_plan"),
        // Events are not produced by a phase call — the hybrid structure is
        // already in the DAE and HRW reads it out. Naming the IR is the honest
        // answer to "where is this computed?".
        StageKind::Events => ("crates/rumoca-ir-dae", "Dae::discrete (populated during flatten)"),
        StageKind::SolveLowering => ("crates/rumoca-phase-solve", "lower_dae_to_solve_model"),
        StageKind::Simulation => ("crates/rumoca-sim", "simulate_solve_model"),
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
    /// What the user wants.
    pub request: AskRequest,
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
    /// The first pipeline stage that failed, if any. See [`PipelineFailure`].
    pub failure: Option<PipelineFailure<'a>>,
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
    fs::create_dir_all(BRIDGE_DIR)?;
    let path = Path::new(BRIDGE_DIR).join("focus.json");
    let doc = build(ask);
    // Pretty-print for readability — Doug reads these during dogfooding.
    fs::write(&path, serde_json::to_string_pretty(&doc).unwrap_or_default())?;
    Ok(path)
}

/// Check whether the extension has acknowledged the last breakpoint request.
/// Returns `true` and deletes the ack file if it exists.
pub fn check_breakpoint_ack() -> bool {
    if std::path::Path::new(BREAKPOINT_ACK_FILE).exists() {
        let _ = fs::remove_file(BREAKPOINT_ACK_FILE);
        true
    } else {
        false
    }
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
fn find_live_trace_line() -> std::io::Result<(std::path::PathBuf, usize)> {
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
pub fn arm_live_trace_breakpoint(specimen: Option<&str>) -> std::io::Result<()> {
    let _ = fs::remove_file(BREAKPOINT_ACK_FILE);
    let (file, line) = find_live_trace_line()?;
    let path_str = file.display().to_string();
    let mut request = json!({
        "version": 1,
        "breakpoints": [{ "path": path_str, "line": line }]
    });
    if let Some(s) = specimen {
        request["specimen"] = json!(s);
    }
    let text = serde_json::to_string_pretty(&request)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    fs::write(BREAKPOINT_REQUEST_FILE, text)
}

/// Remove the `live_trace_breakpoint` breakpoint when the live debug session
/// finishes, preventing a SIGSTOP signal when the algorithm thread exits.
pub fn remove_live_trace_breakpoint() -> std::io::Result<()> {
    let (file, line) = find_live_trace_line()?;
    let path_str = file.display().to_string();
    let request = json!({
        "version": 1,
        "action": "remove",
        "breakpoints": [{ "path": path_str, "line": line }]
    });
    let text = serde_json::to_string_pretty(&request)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    fs::write(BREAKPOINT_REQUEST_FILE, text)
}

/// Write each stage's full IR to `.hrw-bridge/stages/<name>.json`.
///
/// Called once per compile (not per capture). Each stage's entire IR is
/// written as a separate file so Claude can diff any two stages by reading
/// both files. A stage with no IR (e.g., it failed or doesn't apply) has
/// its file removed, keeping the directory in sync with the current specimen.
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
//   "request": "explain" | "debug-where-set" | null  (null: no point, so no request),
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
/// `app.rs` owns the Context Bar and therefore owns "what happens when the point
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
    };
    // **The slug, not the display name.** This used to emit `StageKind::name`, which
    // reads "Index reduction" with a space — so the capture named a stage that
    // `hrw://stage/<X>` could not parse, for two of the eleven stages. The capture is
    // read by Claude in order to *act*, so the actionable form wins; `stage_display`
    // carries the prose label alongside it.
    let stage_str = ask.stage.map_or("(navigated definition)", StageKind::slug);
    let stage_display = ask.stage.map_or("(navigated definition)", StageKind::name);
    let mut doc = json!({
        "instructions": INSTRUCTIONS,
        "seq": ask.seq,
        // `request` is a property of the POINT — "explain this node" versus
        // "show me where this node gets set". With no point there is no request,
        // and emitting a default `"explain"` would claim an intent the user
        // never expressed. Null rather than omitted, for the same reason
        // `kind: "none"` beats a missing `kind`: absence stated, not implied.
        "request": match ask.focus {
            Focus::Nothing => Value::Null,
            _ => Value::String(ask.request.as_str().to_owned()),
        },
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
            "note": "the FIRST stage that failed. A failure cascades, so stages after it                      report \"not reached\" and are consequences rather than problems.                      `error` carries the diagnosis: counts, unmatched names, source                      locations, guidance. Full IR for every stage is under `stages.dir`.",
            "stage": f.stage,
            "summary": f.note,
            "error": f.error,
            "downstream_not_reached": f.not_reached,
        });
    }
    if let Focus::Node { key_path, stage_value } = &ask.focus {
        doc["node"] = build_node(key_path, stage_value, ask.specimen);
        doc["cross_stage"] = build_cross_stage(ask, key_path);
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
        let Some(node) = navigate(root, &path[..depth]) else { continue };
        let bytes = serde_json::to_string(node).map(|s| s.len()).unwrap_or(usize::MAX);
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
    let Some((last, parent_path)) = path.split_last() else { return Value::Null };
    let Some(parent) = navigate(root, parent_path) else { return Value::Null };

    let (count, position, names): (usize, Option<usize>, Vec<String>) = match parent {
        Value::Object(map) => {
            let keys: Vec<&String> = map.keys().collect();
            let Seg::Key(k) = last else { return Value::Null };
            let at = keys.iter().position(|key| *key == k);
            (keys.len(), at, keys.iter().map(|k| (*k).clone()).collect())
        }
        Value::Array(arr) => {
            let Seg::Index(i) = last else { return Value::Null };
            (arr.len(), Some(*i), (0..arr.len()).map(|n| format!("[{n}]")).collect())
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
                    None => out.push(json!({ "path": path.join("."), "parse": va, "resolve": null })),
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
/// that no longer resolves must not be kept: the Context Bar would name a node
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

// The span-ascent algorithm: walk from the clicked node up to the root,
// returning the tightest (deepest) enclosing `location` or `span`.
//
// Why "ascent"? The clicked leaf usually has no provenance of its own.
// For example, clicking `"name": "flange_a"` gives a bare string — no
// byte offsets. But its parent (a component object) carries a `location`
// with byte offsets into the Modelica source. By walking up the path from
// leaf to root, we find the nearest ancestor that has provenance.
//
// The loop iterates `depth` from `path.len()` (the leaf) down to 0 (the root).
// At each depth, it navigates to `path[..depth]` and checks for a `location`
// or `span` field. `location` is preferred (it includes a `file_name`);
// `span` is a fallback (it has an opaque `source` id instead of a filename).
fn ascend_provenance(root: &Value, path: &[Seg], specimen: Option<&Path>) -> Value {
    for depth in (0..=path.len()).rev() {
        let Some(Value::Object(map)) = navigate(root, &path[..depth]) else { continue };
        if let Some(loc) = map.get("location").filter(|v| is_location(v)) {
            return provenance(loc, "location", depth, specimen);
        }
        if let Some(span) = map.get("span").filter(|v| is_span(v)) {
            return provenance(span, "span", depth, specimen);
        }
    }
    json!({ "found": false, "note": "no location/span on this node or its ancestors" })
}

// Check if a JSON value is a `rumoca_core::Location` (byte offsets + file_name).
// A Location is the preferred provenance type because it includes the source
// filename, enabling direct file reads for the excerpt.
fn is_location(v: &Value) -> bool {
    v.get("start").and_then(Value::as_u64).is_some()
        && v.get("end").and_then(Value::as_u64).is_some()
        && v.get("file_name").and_then(Value::as_str).is_some()
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
    let path = if !file_name.is_empty() && Path::new(file_name).is_file() {
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
    let line_start = bytes[..start].iter().rposition(|&b| b == b'\n').map_or(0, |i| i + 1);
    let line_end = bytes[end..].iter().position(|&b| b == b'\n').map_or(bytes.len(), |i| end + i);
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
            "notes.txt",                 // must be a notebook
            "structural-vs-numerical-rank",  // ...with the extension
        ] {
            assert!(resolve_notebook(bad).is_none(), "{bad:?} must be refused");
        }

        // The real fixture notebook resolves, if this checkout has it.
        let real = "structural-vs-numerical-rank.nb";
        if Path::new(FIXTURE_NOTEBOOKS_DIR).join(real).is_file() {
            let found = resolve_notebook(real).expect("the fixture notebook should resolve");
            assert!(found.ends_with(real));
            assert!(found.starts_with(FIXTURE_NOTEBOOKS_DIR), "resolved inside the fixture dir");
        }

        // A name that is well-formed but absent still resolves to nothing.
        assert!(resolve_notebook("no-such-notebook.nb").is_none());
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
            vec![Seg::Key("error".into()), Seg::Key("unmatched_unknowns".into()), Seg::Index(0)],
            vec![Seg::Key("blocks".into()), Seg::Index(2), Seg::Key("equations".into()), Seg::Index(0)],
            // Consecutive indices, which `describe_path` writes without a separator.
            vec![Seg::Key("rows".into()), Seg::Index(3), Seg::Index(1)],
            // A leading index is unusual but expressible.
            vec![Seg::Index(7)],
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
        for bad in ["a..b", ".a", "a.", "a[", "a[]", "a[x]", "a]0[", "a[0]b"] {
            assert!(parse_path(bad).is_none(), "{bad:?} should not parse");
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
            request: AskRequest::Explain,
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
        };
        let doc = build(&ask);

        let f = &doc["pipeline_failure"];
        assert!(!f.is_null(), "a failure must be stated prominently, not left to be found");
        assert_eq!(f["stage"], json!(StageKind::Flatten.name()));
        assert_eq!(f["error"]["balance"], json!(-1), "the diagnosis travels with it");
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
            request: AskRequest::Explain,
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
        let root = json!({
            "components": [
                {
                    "location": { "start": start, "end": end, "file_name": "M.mo" },
                    "name": "flange_a"
                }
            ]
        });
        let path = vec![Seg::Key("components".into()), Seg::Index(0), Seg::Key("name".into())];

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
        let specimen = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/specimens/RotationalInertia.mo"));
        let source = fs::read_to_string(specimen).expect("read specimen");
        let ast = rumoca_phase_parse::parse_to_ast(&source, "RotationalInertia.mo").expect("parse");
        let root = serde_json::to_value(&ast).expect("to_value");

        // Find a path to some object carrying a real (non-dummy) location.
        let mut path = Vec::new();
        let found = first_location_path(&root, &mut path);
        assert!(found, "expected at least one located node in the parsed AST");

        let prov = ascend_provenance(&root, &path, Some(specimen));
        assert_eq!(prov["found"], json!(true), "provenance: {prov}");
        let excerpt = prov["excerpt"].as_str().expect("excerpt string");
        assert!(source.contains(excerpt), "excerpt {excerpt:?} not found in source");
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
            request: AskRequest::Explain,
            specimen: None,
            model: Some("M"),
            stage: Some(StageKind::Resolve),
            libraries: vec![],
            def_index: &empty,
            parse_value: Some(&parse),
            resolve_value: Some(&resolve),
            focus: Focus::Node { key_path: key_path.clone(), stage_value: &resolve },
            tracking: None,
            view: test_view(),
            failure: None,
        };

        let cs = build(&ask)["cross_stage"].clone();
        assert_eq!(cs["applicable"], json!(true), "{cs}");
        // Parse side found the node under classes.M.
        assert_eq!(cs["parse"]["value"]["def_id"], json!(null));
        assert_eq!(cs["resolve"]["value"]["def_id"], json!(9));
        // The two field changes are reported.
        let changes = cs["changes"].as_array().unwrap();
        let has = |p: &str, r: i64| {
            changes.iter().any(|c| c["path"] == json!(p) && c["parse"] == json!(null) && c["resolve"] == json!(r))
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
            request: AskRequest::Explain,
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
        assert_eq!(by_stage[0]["mentions"], json!(1), "the key `h` is a mention");
        // Absence is emitted, not omitted.
        assert_eq!(by_stage[1]["mentions"], json!(0), "genuinely absent here");
        // And "produced no IR" is distinct from "IR without the name in it".
        assert_eq!(by_stage[2]["produced_no_ir"], json!(true));
        assert!(by_stage[2].get("mentions").is_none());
    }

    /// Emission uses the same matching rules as the views, or the Context Bar
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
        let paths: Vec<&str> = entry["paths"].as_array().unwrap()
            .iter().map(|p| p.as_str().unwrap()).collect();
        assert!(paths.iter().any(|p| *p == "equation"));
        assert!(paths.iter().any(|p| *p == "unknown"));
        assert!(paths.iter().any(|p| p.contains("nested")));
        assert!(!paths.iter().any(|p| *p == "other"), "substring is not a mention");
        assert!(!paths.iter().any(|p| *p == "description"), "prose is not a mention");
    }

    /// Following and pointing are independent: either may be absent.
    #[test]
    fn tracking_section_absent_when_nothing_is_followed() {
        let empty = BTreeMap::new();
        let ask = Ask {
            seq: 1,
            request: AskRequest::Explain,
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

        assert_eq!(entry["mentions"], json!(n), "count is exact regardless of the caps");
        assert_eq!(entry["paths"].as_array().unwrap().len(), MAX_MENTION_PATHS);
        assert_eq!(entry["paths_truncated"], json!(true));
        assert_eq!(entry["contexts"].as_array().unwrap().len(), MAX_MENTION_CONTEXTS);
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
        let window = ctx["siblings"]["window"].as_array().expect("sibling window");
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
        let window: Vec<&str> =
            sib["window"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
        assert!(window.contains(&"zzz_target"), "the window must contain the hit: {window:?}");
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
        assert_eq!(depth, 0, "a small tree fits entirely, so the root is returned");
        assert_eq!(ctx, small);

        // Now make the root overflow the budget: the leaf's parent still fits.
        let filler: String = "x".repeat(MAX_CONTEXT_BYTES);
        let big = json!({ "eq": { "lhs": "a", "rhs": "b" }, "bulk": filler });
        let (depth, ctx) = enclosing_context(&big, &path);
        assert_eq!(depth, 1, "the root no longer fits, so its child is returned");
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
    /// what Claude is told. They come from one walk on purpose — a second
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
        let jump: Vec<String> =
            mention_paths(&stage, "emf.w").iter().map(|p| describe_path(p)).collect();

        assert_eq!(jump, emitted, "the tree and the capture must agree node for node");
        assert_eq!(jump.len(), 3, "key, and two of the three equations: {jump:?}");
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
        let rows: Vec<Value> = (0..n).map(|_| serde_json::json!({ "unknown": "h" })).collect();
        let stage = serde_json::json!({ "rows": rows });
        let stages: Vec<(&str, Option<&Value>)> = vec![("structural", Some(&stage))];
        let empty = BTreeMap::new();
        let doc = build(&tracking_ask("h", &stages, &empty));

        assert_eq!(doc["tracking"]["stages"][0]["paths"].as_array().unwrap().len(), MAX_MENTION_PATHS);
        assert_eq!(mention_paths(&stage, "h").len(), n, "cycling must reach the last one");
        // The count was always exact; that is what makes the cap safe.
        assert_eq!(doc["tracking"]["stages"][0]["mentions"], serde_json::json!(n));
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
            request: AskRequest::Explain,
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
            doc["view"]["animation"]["showing"]["step"].as_str().is_some_and(|s| s.contains("emf.phi")),
            "the frame's own description must reach the capture: {}",
            doc["view"]["animation"],
        );

        assert_eq!(doc["phase_source"]["crate"], json!("crates/rumoca-ir-dae"));
        assert!(doc["phase_source"]["entry"].as_str().is_some_and(|e| e.contains("discrete")));
    }

    /// Every stage names a crate that exists. A `phase_source` pointing at a
    /// directory that was renamed during a rebase is worse than none — it sends
    /// the reader somewhere confidently wrong.
    #[test]
    fn every_phase_source_crate_exists_on_disk() {
        let root = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/.."));
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
        assert_eq!(node["neighbourhood"]["context"]["name"], json!("MotorWithBrake"));
        let window: Vec<&str> = node["neighbourhood"]["siblings"]["window"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .collect();
        assert!(window.contains(&"name") && window.contains(&"class_type"), "{window:?}");
    }

    /// A node outside the model class (e.g. the parse `within`) is not diffable.
    #[test]
    fn cross_stage_not_applicable_outside_class() {
        let parse = json!({ "classes": { "M": {} }, "within": "Foo" });
        let empty = BTreeMap::new();
        let ask = Ask {
            seq: 1,
            request: AskRequest::Explain,
            specimen: None,
            model: Some("M"),
            stage: Some(StageKind::Parse),
            libraries: vec![],
            def_index: &empty,
            parse_value: Some(&parse),
            resolve_value: None,
            focus: Focus::Node { key_path: vec![Seg::Key("within".into())], stage_value: &parse },
            tracking: None,
            view: test_view(),
            failure: None,
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
            request: AskRequest::Explain,
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
    /// This test checks the count matches the pipeline's 10 stages so a new stage
    /// addition without updating the constant is caught.
    #[test]
    fn stage_file_names_covers_all_pipeline_stages() {
        // One file per pipeline stage (Parse through Solve lowering, excluding Simulation).
        let pipeline_stage_count = StageKind::ALL.len() - 1;
        assert_eq!(
            STAGE_FILE_NAMES.len(),
            pipeline_stage_count,
            "STAGE_FILE_NAMES has {} entries but the pipeline has {pipeline_stage_count} stages",
            STAGE_FILE_NAMES.len(),
        );
        // Every name must end with .json and be unique.
        for name in STAGE_FILE_NAMES {
            assert!(name.ends_with(".json"), "stage file name should end with .json: {name}");
        }
        let unique: std::collections::HashSet<&&str> = STAGE_FILE_NAMES.iter().collect();
        assert_eq!(unique.len(), STAGE_FILE_NAMES.len(), "duplicate stage file names");
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
            request: AskRequest::Explain,
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
        assert_eq!(AskRequest::Explain.as_str(), "explain");
        assert_eq!(AskRequest::DebugWhereSet.as_str(), "debug-where-set");
    }

    #[test]
    fn ask_stage_none_shows_navigated_definition() {
        let empty = BTreeMap::new();
        let ask = Ask {
            seq: 1,
            request: AskRequest::Explain,
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
        assert!(!trimmed.starts_with("//"), "target should not be a comment: {found}");
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

    /// Tests arm, remove, and ack together to avoid races on the shared
    /// bridge directory (all three write to the same request/ack files).
    #[test]
    fn live_trace_breakpoint_arm_remove_and_ack() {
        // arm
        arm_live_trace_breakpoint(Some("TestModel")).expect("arm");
        let content = fs::read_to_string(BREAKPOINT_REQUEST_FILE).expect("read request");
        let req: serde_json::Value = serde_json::from_str(&content).expect("parse JSON");
        assert_eq!(req["version"], json!(1));
        assert_eq!(req["specimen"], json!("TestModel"));
        let bp = &req["breakpoints"][0];
        assert!(bp["path"].as_str().unwrap().contains("live_trace.rs"));
        assert!(bp["line"].as_u64().unwrap() > 0);
        assert_eq!(bp.get("condition"), None, "no condition field when absent");

        // remove
        remove_live_trace_breakpoint().expect("remove");
        let content = fs::read_to_string(BREAKPOINT_REQUEST_FILE).expect("read request");
        let req: serde_json::Value = serde_json::from_str(&content).expect("parse JSON");
        assert_eq!(req["version"], json!(1));
        assert_eq!(req["action"], json!("remove"));
        assert!(req["breakpoints"][0]["path"].as_str().unwrap().contains("live_trace.rs"));

        // ack
        fs::write(BREAKPOINT_ACK_FILE, r#"{"acked":true}"#).unwrap();
        assert!(check_breakpoint_ack(), "should return true when ack file exists");
        assert!(!Path::new(BREAKPOINT_ACK_FILE).exists(), "ack file should be deleted");
        assert!(!check_breakpoint_ack(), "should return false when ack file is gone");

        let _ = fs::remove_file(BREAKPOINT_REQUEST_FILE);
    }

    #[test]
    fn write_stages_creates_and_removes_files() {
        let val = json!({"equations": [1, 2, 3]});
        let stages: Vec<(&str, Option<&Value>)> = vec![
            ("test_alpha", Some(&val)),
            ("test_beta", None),
        ];
        write_stages(&stages).expect("write_stages");
        let alpha_path = Path::new(STAGES_DIR).join("test_alpha.json");
        assert!(alpha_path.exists(), "stage file should be created");
        let content: Value = serde_json::from_str(
            &fs::read_to_string(&alpha_path).unwrap()
        ).unwrap();
        assert_eq!(content["equations"], json!([1, 2, 3]));

        let beta_path = Path::new(STAGES_DIR).join("test_beta.json");
        assert!(!beta_path.exists(), "None stage should not create a file");

        // A second call with None removes a previously written file.
        let stages_remove: Vec<(&str, Option<&Value>)> = vec![
            ("test_alpha", None),
        ];
        write_stages(&stages_remove).expect("cleanup write");
        assert!(!alpha_path.exists(), "stage file should be removed when None");
    }

    #[test]
    fn write_creates_focus_json() {
        let val = json!({"name": "test_write_model"});
        let ask = Ask {
            seq: 99,
            request: AskRequest::Explain,
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
        };
        let path = write(&ask).expect("write focus");
        assert!(path.exists(), "focus.json should exist");
        let content: Value = serde_json::from_str(
            &fs::read_to_string(&path).unwrap()
        ).unwrap();
        assert_eq!(content["seq"], json!(99));
        assert_eq!(content["model"], json!("TestWriteModel"));
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

    #[test]
    fn slice_source_file_end() {
        let path = test_file_path();
        let src = fs::read_to_string(&path).unwrap();
        let len = src.len();
        assert!(slice_source(&path, None, len - 2, len).is_some(), "file end should succeed");
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
