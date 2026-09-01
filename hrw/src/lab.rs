//! The **lab panel's state**: which lab is showing, and its text.
//!
//! Split out of `app.rs` on 2026-08-02, the second pane able to move. Like
//! [`crate::model_list`], it moved because it had already stopped reaching into
//! `App`: [`LabState::select`] and [`LabState::poll`] report *whether the
//! selection changed* and leave the consequences — clearing stages, the log and
//! the selection — to `App::reset_for_new_lab`.
//!
//! The rendering itself stays in `app.rs` for now. `lab_panel_ui` still reads
//! `commonmark_cache` and dispatches through `App`, so moving it would mean
//! widening fields rather than narrowing a signature. **Narrow first, move
//! second.**

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::PathBuf;

use crate::app::LAB_POLL_INTERVAL;
use crate::bridge;

/// Everything the **lab panel** owns.
///
/// The fourth grouping lifted off `App` during the UI pause, and the smallest.
/// Its shape is `ModelListState`'s: the pane holds its own world, reports what
/// changed, and leaves the consequences to the caller.
///
/// # The seam
///
/// Selecting a lab re-initialises the right-hand side, because a lab starts
/// from its own first stop and leaving the previous lab's model on screen makes
/// Station 1 look as though it has already been done. **That reset is not this
/// struct's business.** `select` and `poll` return *whether the selection
/// changed*; `App::reset_for_new_lab` decides what that invalidates.
pub(crate) struct LabState {
    /// The selected lab's text and the mtime it was read at.
    pub(crate) cached: Option<(String, std::time::SystemTime)>,
    /// Every lab on offer: the ad hoc one first when it exists, then fixtures.
    pub(crate) available: Vec<LabSource>,
    /// Which lab is showing.
    pub(crate) selected: Option<LabSource>,
    /// When the lab directory was last polled.
    pub(crate) polled_at: Option<std::time::Instant>,
    /// Specimens each fixture lab points at, for the list row. Filled at poll time,
    /// never in the paint path. See the note where it is populated.
    pub(crate) row_specimens: std::collections::HashMap<PathBuf, String>,
    /// Byte offset in the lab text to scroll to on the next frame, then cleared.
    ///
    /// Set by `hrw://lab/<name>/station/<slug>` so a citation lands **at the stop**
    /// rather than at the top of the lab. One-shot deliberately: a persistent target
    /// would fight the scrollbar on every frame, which is the defect the autoplay
    /// scroll work spent four attempts on (`ui-findings.md` C15).
    ///
    /// **It was written and never read until 2026-08-17** — set here, consumed nowhere,
    /// so every stop link opened its lab and landed wherever the pane happened to be.
    /// It survived because the corpus holds exactly **one** such link, and because the
    /// symptom is indistinguishable from the scroll bug fixed the same day: both look
    /// like *"the lab opened in the wrong place"*.
    ///
    /// **The pane consumes it by splitting the document at this offset and calling
    /// `scroll_to_cursor`** — the same split the autoplay scroll uses. The byte offset
    /// is never converted to a pixel position, because that conversion is exactly what
    /// four attempts proved impossible: rendered height per character is not constant.
    /// The cursor at the split *is* the position, and egui computes the offset from it.
    pub(crate) scroll_to_offset: Option<usize>,
    /// **Put the lab panel back at the top on the next frame that renders text.**
    ///
    /// One-shot, and consumed only on a frame that actually drew a document — a switch
    /// clears `cached`, so the frame in between has nothing to scroll and consuming the
    /// flag there would drop it on the floor.
    ///
    /// **[`Self::scroll_to_offset`] outranks it when both are pending**, which is the
    /// normal case rather than a corner one: a `stop/<slug>` link switches the lab —
    /// asking for the top — and *then* asks for the stop. The top request is still
    /// consumed on that frame, because leaving it pending would fire it at whatever
    /// document came next; it simply does not move anything.
    pub(crate) scroll_to_top: bool,
    /// **Where the reader came from**, so a cross-lab link can be undone.
    ///
    /// Doug, 2026-08-19: *"while in the index reduction lab, I can click a link to
    /// navigate to the blt-ordering lab, but then I cannot navigate back."*
    ///
    /// **Authored back-links cannot solve this**, which is why a stack exists at all. The
    /// hub-to-lab edge is 1:many with a canonical parent, so every phase lab carries one
    /// `▲ The chain overview` link. Cross-references are many:many with no canonical
    /// parent — `blt-ordering` is cited by index-reduction, tearing and the hub — so it
    /// cannot link back to all of them.
    ///
    /// Each entry is the lab **and the offset it was left at**: returning to the top of a
    /// document you were halfway down is most of the friction, not a detail.
    ///
    /// **A lab's location is one value**, which is why this is a `Vec` rather than the
    /// design problem `ideas.md` #78 describes. The RHS's location is five — stage, its
    /// sub-view, `viewing_log`, `ui_mode`, `specimen_detail` — and deciding what a location
    /// *is* is the hard part there. **That reservation is untouched:** this is the lab
    /// panel's history, and the RHS pair will live in the stage tab bar.
    pub(crate) history: Vec<(LabSource, f32)>,
    /// The lab pane's scroll offset as of the last painted frame, read from the
    /// `ScrollArea`'s own output so it cannot drift from what is on screen.
    pub(crate) current_scroll_y: f32,
    /// **Restore this exact offset on the next frame with text**, set by Back.
    ///
    /// Outranks [`Self::scroll_to_top`], which every switch requests: going *back* is the
    /// one navigation for which the top of the document is the wrong answer.
    pub(crate) restore_scroll_y: Option<f32>,

    /// **The self-running run** of whichever lab is showing.
    ///
    /// Lives here rather than on `App` because it plays *this* lab and means
    /// nothing without one — `app_does_not_regrow_its_field_count` asked the
    /// question ("does the new field belong on App, or on the pane that owns
    /// it?") and this was the answer. The seam is the one this struct already
    /// documents: the pane holds its own world, and `App` keeps the
    /// consequences, since dispatching a beat needs `dispatch_hrw_link` and
    /// knowing when to hold the clock needs `compiling`.
    pub(crate) autoplay: crate::autoplay::Autoplay,
    /// Requested run length, from [`crate::autoplay::TOTAL_CHOICES`].
    pub(crate) autoplay_total: std::time::Duration,
    /// **Measured y of the current beat's link** inside the lab text.
    ///
    /// `app.rs` splits the markdown at the link's line and renders both halves; the
    /// cursor between them is this. Measured rather than estimated because two
    /// estimates failed — by beat ordinal, then by character offset — and rendered
    /// height per character is simply not constant: prose wraps in a narrow panel
    /// and a code block does not.
    pub(crate) lab_link_y: Option<f32>,
    /// The previous beat's measured y, to travel *from*.
    pub(crate) lab_prev_link_y: Option<f32>,
    /// Which beat `lab_link_y` was measured for, so a beat change can roll the
    /// current position into the previous one exactly once.
    pub(crate) lab_measured_beat: Option<usize>,
    /// Maximum scroll offset of the lab text, measured last frame — the clamp.
    pub(crate) lab_max_scroll: Option<f32>,

    /// **The mode a self-running run started in**, to return to when it ends.
    ///
    /// A lab stop may legitimately leave Lab mode: `hrw://source/<line>` switches
    /// to Specimen mode because that is the only place the source renders, and a
    /// reader clicking it wants to be taken there. `matching.md` ends Station 3 with one,
    /// so a Play run finished in Specimen mode with the lab nowhere on screen —
    /// Doug, 2026-08-03: *"at the completion of the lab, the mode is being switched
    /// from lab mode to specimen mode."*
    ///
    /// **The stop is not wrong; the run just has to clean up after itself.** A run
    /// is a round trip, so it restores what it borrowed, on finishing *or* on Stop.
    pub(crate) mode_before_autoplay: Option<crate::ui_state::UiMode>,
}

impl Default for LabState {
    /// Hand-written for one field: `autoplay_total` starts at
    /// [`crate::autoplay::DEFAULT_TOTAL`] rather than at zero, and a derived
    /// `Default` would schedule every beat into no time at all.
    fn default() -> Self {
        Self {
            cached: None,
            available: Vec::new(),
            selected: None,
            polled_at: None,
            row_specimens: std::collections::HashMap::new(),
            scroll_to_offset: None,
            scroll_to_top: false,
            history: Vec::new(),
            current_scroll_y: 0.0,
            restore_scroll_y: None,
            autoplay: crate::autoplay::Autoplay::default(),
            autoplay_total: crate::autoplay::DEFAULT_TOTAL,
            lab_link_y: None,
            lab_prev_link_y: None,
            lab_measured_beat: None,
            lab_max_scroll: None,
            mode_before_autoplay: None,
        }
    }
}

impl LabState {
    /// Switch to `source`, discarding the previous text.
    ///
    /// Returns **true when the selection actually changed**, which is the
    /// caller's cue to re-initialise the stage side. Re-clicking the lab already
    /// showing returns false, so a reader partway through a specimen keeps it.
    ///
    /// Clears `cached` rather than letting the poll notice: without this the old
    /// lab stays on screen until the next mtime comparison, and a reader who
    /// just clicked a different lab sees the previous one for up to an interval.
    pub(crate) fn select(&mut self, source: LabSource) -> bool {
        let changed = self.selected.as_ref() != Some(&source);
        self.selected = Some(source);
        self.cached = None;
        changed
    }

    /// The selected lab's text, if any has been read.
    pub(crate) fn text(&self) -> Option<&str> {
        self.cached.as_ref().map(|(t, _)| t.as_str())
    }

    /// **The order the picker lists labs in, and where the separator goes.**
    ///
    /// Returns the labs with [`OVERVIEW_LAB`] hoisted to the front, plus the count of
    /// hoisted entries — which is both the index the separator is drawn at and the answer
    /// to *"is the overview on disk at all?"*. Zero means no separator: a rule with
    /// nothing above it is just a line.
    ///
    /// **Why hoisting, and why here.** Doug, 2026-08-17: *"I really want to be able to
    /// navigate backward from a subordinate lab to the top-level lab so that I can then
    /// navigate downward to another subordinate lab."* Nine phase labs hang off the
    /// overview, which in a flat alphabetical list sits between `frame-seeking` and
    /// `initialization` — a hub rendered as a peer. Hoisting makes that structure visible
    /// at **no cost in panel width**, which is why it was chosen over a dedicated "up"
    /// button: the transport bar already sets `MIN_LEFT_POINTS`, and another control
    /// raises it.
    ///
    /// **Ordering only — not filtering and not ranking.** Every lab is still offered, so
    /// charter Decision 8 holds: the hoist encodes a *fixed* fact about the set (one of
    /// these is the hub), not a judgement about which lab is relevant now.
    ///
    /// Extracted from the combo-box closure rather than left inline, per
    /// `format-and-app-plan.md`: the position of a row inside a popup that exists only
    /// while open is awkward to assert, and the ordering is the part that can be wrong.
    pub(crate) fn picker_order(&self) -> (Vec<&LabSource>, usize) {
        let (mut ordered, rest): (Vec<&LabSource>, Vec<&LabSource>) =
            self.available.iter().partition(|s| s.is_overview());
        // Taken *before* the extend, so the boundary cannot drift as the tail is appended.
        let hoisted = ordered.len();
        ordered.extend(rest);
        (ordered, hoisted)
    }

    /// **Forget where the text was scrolled to.**
    ///
    /// These are pixel positions measured in *one particular document at one
    /// particular beat*, so they mean nothing after a lab changes or a run starts
    /// over — and they are worse than meaningless, because the scroll interpolates
    /// **from** the stale value on the first frame of a new run.
    ///
    /// Doug, 2026-08-03: stop mid-lab, switch labs, switch back, press Play, and
    /// *"the matching lab rescrolls very visibly from the stopped position back up
    /// to the top before the lab begins playing."* The pane was correctly at the top
    /// already; the *bookkeeping* still held the stopped position, so the first frame
    /// aimed there and then travelled back.
    ///
    /// With these cleared, both ends of the interpolation are zero on the first beat,
    /// so a lab that is already at the top does not move at all — which is the
    /// behaviour asked for. `lab_max_scroll` is deliberately **kept**: it describes
    /// the pane, not the position, and re-measures on the next frame anyway.
    pub(crate) fn reset_scroll(&mut self) {
        self.lab_link_y = None;
        self.lab_prev_link_y = None;
        self.lab_measured_beat = None;
        // **And put the new document at its top.**
        //
        // The three fields above are HRW's *own* measurements, used to interpolate an
        // autoplay scroll. Clearing them was never enough, because **none of them is
        // what positions the view** — that is egui's `ScrollArea`, which keeps an offset
        // per id, and the lab panel's id (`id_salt("lab")`) is deliberately stable.
        //
        // So the offset survived the document. Doug, 2026-08-17: *"When I click a
        // subordinate lab link in the-concepts hub lab, the subordinate lab opens
        // partially scrolled down instead of fully scrolled to the top."* Exactly the
        // hub's shape — its links live in a table you scroll down to reach, and the
        // lab that opened inherited however far down that was.
        //
        // **The bug reads as "sometimes"**, which is why it survived: arrive at a lab
        // from the top of a short document and nothing looks wrong.
        self.scroll_to_top = true;
    }

    /// Re-read the list and the selected lab, at most once per
    /// [`LAB_POLL_INTERVAL`]. Returns true when a lab was newly selected.
    pub(crate) fn poll(&mut self) -> bool {
        let mut newly_selected = false;
        let due = self
            .polled_at
            .is_none_or(|last| last.elapsed() >= LAB_POLL_INTERVAL);
        if !due {
            return false;
        }
        self.polled_at = Some(std::time::Instant::now());

        // --- Rebuild the pick list ---
        //
        // Ad hoc first when it exists: it is the answer to the question just asked,
        // and burying it under the fixtures would make the common case the awkward one.
        let mut labs = Vec::new();
        if std::path::Path::new(bridge::LAB_FILE).exists() {
            labs.push(LabSource::AdHoc);
        }
        labs.extend(bridge::fixture_labs().into_iter().map(LabSource::Fixture));
        let list_changed = labs != self.available;
        self.available = labs;

        // **Which specimens each lab uses, read here rather than per frame.**
        //
        // The row shows it so that searching by model finds the lab — Doug went
        // looking for a "DimensionMismatch lab" and it is `failure-typecheck`. Read
        // at poll time because the paint path must not touch the filesystem
        // (`CLAUDE.md`, debugging conventions), and re-read on every poll because a
        // lab edited between polls would otherwise advertise the wrong model, which
        // is worse than advertising none.
        if list_changed || self.row_specimens.is_empty() {
            self.row_specimens.clear();
            for source in &self.available {
                if let LabSource::Fixture(p) = source
                    && let Ok(md) = std::fs::read_to_string(p)
                {
                    let names = LabSource::specimens_in(&md);
                    if !names.is_empty() {
                        self.row_specimens.insert(p.clone(), names.join(", "));
                    }
                }
            }
        }

        // A selection that no longer exists (the ad hoc lab was deleted, a fixture
        // renamed) must not leave stale text on screen attributed to a live file.
        if self
            .selected
            .as_ref()
            .is_some_and(|t| !self.available.contains(t))
        {
            self.selected = None;
            self.cached = None;
        }
        // Default to the ad hoc lab when one appears and nothing is chosen.
        if self.selected.is_none() && self.available.contains(&LabSource::AdHoc) {
            newly_selected |= self.select(LabSource::AdHoc);
        }

        // --- Re-read the selected lab if it changed on disk ---
        let Some(selected) = self.selected.clone() else {
            self.cached = None;
            return newly_selected;
        };
        let path = selected.path();
        let mtime = std::fs::metadata(&path)
            .ok()
            .and_then(|m| m.modified().ok());
        match mtime {
            Some(mtime) => {
                let unchanged = self.cached.as_ref().is_some_and(|(_, seen)| *seen == mtime);
                if !unchanged || list_changed {
                    self.cached = std::fs::read_to_string(&path)
                        .ok()
                        .map(|text| (strip_html_comments(&text), mtime));
                }
            }
            // The file vanished between listing and reading. Drop the text rather
            // than keep a stale copy: for an ad hoc lab, absence is the normal state.
            None => {
                self.cached = None;
                self.selected = None;
            }
        }
        newly_selected
    }
}

/// Which lab the Lab panel is showing.
///
/// Two kinds, with different lifetimes and different jobs:
///
/// - **`AdHoc`** — `.hrw-bridge/lab.md`, written by Claude to answer the question just
///   asked. Gitignored, regenerated, ephemeral by construction.
/// - **`Fixture`** — a file in `docs/fixture-labs/`, kept and versioned because it is a
///   *test* with a pass/fail criterion rather than an explanation that would rot.
///
/// Keeping them in one list (rather than one panel each) is deliberate: from the
/// reader's side both are "a sequence of stops to run", and the distinction is about
/// where the file lives, not about how it is used.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum LabSource {
    AdHoc,
    Fixture(PathBuf),
}

/// **Remove `<!-- … -->` spans from lab text before anything sees it.**
///
/// # Why this exists
///
/// Doug, 2026-08-17: *"The 'kind' metadata which you've added to the labs is now visible
/// in the HRW rendering of those labs."* `egui_commonmark` renders an HTML comment as
/// literal text rather than hiding it, so `<!-- kind: concept -->` sat under the title of
/// every lab.
///
/// **The claim that it would be invisible was an assumption, never checked** — written
/// into `fixture-labs/README.md` as *"invisible in the pane, greppable by a checker"* on
/// the strength of the marker convention already in use. Which is the second half of the
/// story: `<!-- pane-groups -->` and friends have been rendering for weeks. Thirty-three
/// comments were on screen before this; they went unreported because they sit beside
/// tables in the middle of a document, and the kind tag put one under every H1.
///
/// # Why it strips at CACHE time rather than at render time
///
/// Byte offsets. `parse_stations` slugs, autoplay's beat positions and a `stop/<slug>`
/// link's destination are all computed from [`LabState::text`], and the pane splits the
/// document at those offsets. Stripping later would shift every one of them and the
/// splits would land mid-word.
///
/// So the cached string **is** the display string, and every offset in the program is
/// measured against the same text. The checkers are unaffected: they read the files from
/// disk, where the markers still are.
///
/// Comments are removed rather than their whole lines, which can leave a blank line
/// behind. That is deliberate — a blank line is invisible in rendered markdown, while
/// deleting lines would make the stripped text disagree with the file about line
/// numbers, and `matching-live.md` cites source lines.
fn strip_html_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("<!--") {
        out.push_str(&rest[..start]);
        match rest[start..].find("-->") {
            Some(end) => rest = &rest[start + end + "-->".len()..],
            // **An unterminated comment keeps its text rather than eating the rest of
            // the document.** A lab is re-read on every mtime change, so a save
            // mid-keystroke is a normal state to render, not a corrupt file — and a
            // pane that empties while Doug types reads as a much worse bug than a
            // stray `<!--`.
            None => {
                out.push_str(&rest[start..]);
                rest = "";
                break;
            }
        }
    }
    out.push_str(rest);
    out
}

/// The one lab that is a **hub** rather than a peer: it links to the nine phase labs,
/// and each of them links back to it.
///
/// **Named once, here, because three places need the same answer** — the picker hoists it,
/// `doc_citations` reads its rows to know which labs must carry a back-link, and the
/// bidirectionality check needs both ends. Spelling it three times is how one of them
/// would end up disagreeing after a rename, silently, since a lab that is merely
/// mis-sorted still works.
///
/// The stem, not the filename: `LabSource::Fixture` holds a full path and every
/// comparison in the codebase goes through `file_stem`.
pub(crate) const OVERVIEW_LAB: &str = "the-concepts";

impl LabSource {
    pub(crate) fn path(&self) -> PathBuf {
        match self {
            Self::AdHoc => PathBuf::from(bridge::LAB_FILE),
            Self::Fixture(p) => p.clone(),
        }
    }

    /// Whether this is the chain overview — the hub the phase labs hang off.
    ///
    /// `AdHoc` is never the overview: it is Claude's answer to the last question, and it
    /// has its own control beside the picker.
    pub(crate) fn is_overview(&self) -> bool {
        match self {
            Self::AdHoc => false,
            Self::Fixture(p) => p.file_stem().and_then(|s| s.to_str()) == Some(OVERVIEW_LAB),
        }
    }

    /// The specimens a lab points at, in first-appearance order.
    ///
    /// **Doug went looking for a "DimensionMismatch lab" and it is called
    /// `failure-typecheck`** (2026-08-05). The labs are named by *phase* — which is
    /// what he asked for, because a per-phase name sets a per-phase expectation — but
    /// he was thinking about the *model* in front of him. Neither name is wrong; the
    /// row was just showing one axis and he was searching on the other.
    ///
    /// Derived from the `hrw://load/<Specimen>` links rather than declared, so it
    /// cannot disagree with where the lab actually sends him. Same extraction the
    /// catalogue uses; shared rather than written twice.
    pub(crate) fn specimens_in(md: &str) -> Vec<String> {
        let mut seen: Vec<String> = Vec::new();
        for raw in md.split("hrw://load/").skip(1) {
            let name: String = raw
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if !name.is_empty() && !seen.contains(&name) {
                seen.push(name);
            }
        }
        seen
    }

    /// **A lab's catalogue blurb: its first bolded line.**
    ///
    /// Extracted from the catalogue builder on 2026-09-01 so the pin and the catalogue
    /// share one derivation — this file already records that duplicating an extraction
    /// produced a false claim in a comment.
    ///
    /// **The trap this exists to make checkable.** The blurb is derived from POSITION, so
    /// **inserting any bolded paragraph above a lab's opening line silently replaces its
    /// catalogue summary** — once with a mid-sentence fragment. Nothing about headings is
    /// involved, and nothing reports it; the reflex became regenerating the catalogue
    /// after every prose edit, ~10 s each in the mode where Doug feels every second. It
    /// bit three times on 2026-08-31. `every_pinned_lab_claim_holds` now pins the result
    /// through the `blurb` verb in `pinned-claims.txt`.
    ///
    /// **A new intro section goes BELOW the opening bold line**, or the pin fails by name.
    pub(crate) fn blurb_of(md: &str) -> String {
        md.lines()
            .find(|l| l.trim_start().starts_with("**") && l.len() > 12)
            .map(|l| l.trim().trim_start_matches("**").replace("**", ""))
            .unwrap_or_default()
    }

    /// Label for the picker. The ad hoc lab is named by what it *is* rather than by
    /// its filename, which is an implementation detail nobody should have to know.
    pub(crate) fn label(&self) -> String {
        match self {
            // **`✨ Answer`, shortened 2026-08-19.** The sparkle identifies it — the only
            // emoji in the bar — and "Claude's" was the widest word in a strip whose
            // one-row width is now a hard floor on the left panel.
            //
            // **The word is kept rather than going icon-only:** Doug asked for this
            // control to be prominent and reported it as a broken feature the day it
            // briefly vanished. An unlabelled sparkle is a puzzle; `✨ Answer` is not.
            Self::AdHoc => "\u{2728} Answer".to_owned(),
            Self::Fixture(p) => p
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("fixture")
                .to_owned(),
        }
    }
}

/// Build the lab catalogue text — the `CATALOGUE.md` written for Claude.
///
/// **In the library rather than in `examples/gen_lab_catalogue.rs`** so that
/// `lab_catalogue_is_current` calls the same code that writes the file. A checker
/// that reimplements what it checks is the drift `docs/fidelity-plan.md` warns
/// about, and a `#[path]` include of an example does not compile inside the lib.
pub fn catalogue() -> String {
    // **The same list the picker shows**, from the one function that decides what a
    // lab file is. This used to be a second `read_dir` with its own exclusions —
    // which is how the catalogue came to exclude itself while the picker did not, and
    // Doug found `CATALOGUE` offered as a lab in his list.
    let labs: Vec<PathBuf> = bridge::fixture_labs();

    let mut s = String::new();
    s.push_str("# Lab catalogue\n\n");
    s.push_str(
        "**Generated — do not edit.** `cargo run -p hrw --example gen_lab_catalogue`.\n\n\
         **Audience: Claude.** This exists so a question can be answered by citing a lab that \
         already demonstrates the thing, rather than by writing a new one that retells it \
         without its checked expectations (`docs/ideas.md` #63). Cite a stop with \
         `hrw://lab/<name>/station/<slug>`.\n\n\
         **Re-read a lab before citing it.** Everything below is derived and current; whether \
         a lab's *claims* still hold is not something a catalogue can know, and a lab \
         promised a tree the pane did not show for its whole existence.\n\n",
    );

    for path in &labs {
        let md = std::fs::read_to_string(path).unwrap_or_default();
        let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("?");

        let title = md
            .lines()
            .find_map(|l| l.strip_prefix("# "))
            .unwrap_or("(untitled)");
        let lead = LabSource::blurb_of(&md);

        // **The same extraction the lab list uses**, not a second copy of it. The
        // first version of this duplicated the loop, and the doc comment on
        // `specimens_in` claimed they were shared when they were not — a false claim
        // in a comment, caught the same day by `unreachable_pub` forcing a look at
        // the function's visibility.
        let specimens = LabSource::specimens_in(&md);
        let mut stages: BTreeSet<&str> = BTreeSet::new();
        for raw in md.split("hrw://load/").skip(1) {
            let cite: &str = raw
                .split(|c: char| c.is_whitespace() || c == ')' || c == '`')
                .next()
                .unwrap_or_default();
            if let Some(st) = cite.split('/').nth(1).filter(|s| !s.is_empty()) {
                stages.insert(st);
            }
        }

        let stops: Vec<String> = crate::autoplay::parse_stations(&md)
            .iter()
            .map(|st| {
                format!(
                    "  - `{}` — {}",
                    crate::autoplay::station_slug(&st.heading),
                    st.heading.trim_start_matches('#').trim(),
                )
            })
            .collect();

        let _ = writeln!(s, "## `{name}`\n");
        let _ = writeln!(s, "**{title}**\n");
        if !lead.is_empty() {
            let lead = lead.chars().take(220).collect::<String>();
            let _ = writeln!(s, "{lead}\n");
        }
        if !specimens.is_empty() {
            let _ = writeln!(
                s,
                "- **Specimens:** {}",
                specimens
                    .iter()
                    .map(|x| format!("`{x}`"))
                    .collect::<Vec<_>>()
                    .join(", "),
            );
        }
        if !stages.is_empty() {
            let _ = writeln!(
                s,
                "- **Stages:** {}",
                stages
                    .iter()
                    .map(|x| format!("`{x}`"))
                    .collect::<Vec<_>>()
                    .join(", "),
            );
        }
        if !stops.is_empty() {
            let _ = writeln!(s, "- **Stations:**");
            for line in &stops {
                let _ = writeln!(s, "{line}");
            }
        }
        s.push('\n');
    }
    s
}

#[cfg(test)]
mod tests_comment_stripping {
    use super::*;

    /// **A marker never reaches the pane, and the text around it survives intact.**
    ///
    /// Doug found `<!-- kind: concept -->` under the title of every lab. The claim that
    /// it would be invisible was written into the README without being checked, on the
    /// strength of a convention that had in fact been rendering for weeks.
    #[test]
    fn html_comments_are_stripped_from_what_the_pane_sees() {
        assert_eq!(
            strip_html_comments("# Title\n\n<!-- kind: concept -->\n\nBody.\n"),
            "# Title\n\n\n\nBody.\n",
            "the marker goes and everything else stays; the blank line it leaves is \
             invisible in rendered markdown",
        );

        // Every marker shape actually present in the corpus, not just the new one.
        for marker in [
            "<!-- pane-groups -->",
            "<!-- pane-origins -->",
            "<!-- pane-frames -->",
            "<!-- unbuilt: survey::sort_rows -->",
            "<!-- kind: calibration -->",
        ] {
            let stripped = strip_html_comments(&format!("a {marker} b"));
            assert!(
                !stripped.contains("<!--") && !stripped.contains("-->"),
                "{marker:?} must not survive into the pane; got {stripped:?}",
            );
            assert!(
                stripped.starts_with('a') && stripped.ends_with('b'),
                "the prose around {marker:?} must be untouched; got {stripped:?}",
            );
        }
    }

    /// **Two markers in one document, and one is enough to prove neither is special.**
    ///
    /// The loop has to continue past the first `-->`; an early implementation that
    /// returned after one match would have left the second visible and passed the test
    /// above.
    #[test]
    fn every_comment_is_stripped_not_only_the_first() {
        let out = strip_html_comments("<!-- one -->A<!-- two -->B<!-- three -->");
        assert_eq!(out, "AB");
    }

    /// **An unterminated comment keeps its text instead of eating the document.**
    ///
    /// Labs are re-read on every mtime change and Doug edits them while running, so a
    /// file saved mid-keystroke is a normal thing to render. **A pane that empties while
    /// he types would read as a far worse bug than a stray `<!--`** — and it would be
    /// blamed on whatever he had just typed.
    #[test]
    fn an_unterminated_comment_does_not_swallow_the_rest_of_the_lab() {
        let out = strip_html_comments("# Title\n\nBody.\n\n<!-- kind: conc");
        assert!(
            out.contains("# Title") && out.contains("Body."),
            "the document before an unterminated comment must survive: {out:?}",
        );
    }

    /// **Stripping must not disturb what the rest of the program measures.**
    ///
    /// Every byte offset in HRW — `parse_stations` slugs, autoplay beat positions, a
    /// `stop/<slug>` destination — is computed from the cached text, and the pane splits
    /// the document at those offsets. Stripping at *render* time would shift all of them
    /// and the splits would land mid-word.
    ///
    /// This is the property that makes cache-time stripping correct rather than merely
    /// convenient, so it is asserted rather than left to the comment above.
    #[test]
    fn offsets_are_measured_against_the_stripped_text() {
        let raw =
            "# T\n\n<!-- kind: concept -->\n\n## Station 1 — First\n\n[x](hrw://load/M/Dae)\n";
        let shown = strip_html_comments(raw);

        // **Not `first()` — the H1 is a stop too.** `parse_stations` slugifies *every*
        // heading, which is why `CATALOGUE.md` lists a lab's title among its stops.
        // Doug named that over-breadth on 2026-08-17; it is out of scope here and
        // logged in `lab-kinds-plan.md` §6, but it will trip anyone who assumes the
        // first stop is Station 1.
        let stops = crate::autoplay::parse_stations(&shown);
        let stop = stops
            .iter()
            .find(|s| s.heading.contains("Station 1"))
            .expect("the sample has a Station 1");
        assert!(
            shown[stop.heading_offset..].starts_with("## Station 1"),
            "an offset taken from the stripped text must resolve inside it — this is \
             what breaks if stripping ever moves to the render path: {:?}",
            &shown[stop.heading_offset..],
        );
        // And the same offset against the RAW text lands somewhere else entirely, which
        // is the failure this arrangement avoids.
        assert!(
            !raw[stop.heading_offset..].starts_with("## Station 1"),
            "precondition: the two texts really do disagree about offsets, or this test \
             proves nothing",
        );
    }
}
