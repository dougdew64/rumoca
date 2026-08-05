//! The **tour panel's state**: which tour is showing, and its text.
//!
//! Split out of `app.rs` on 2026-08-02, the second pane able to move. Like
//! [`crate::model_list`], it moved because it had already stopped reaching into
//! `App`: [`TourState::select`] and [`TourState::poll`] report *whether the
//! selection changed* and leave the consequences — clearing stages, the log and
//! the selection — to `App::reset_for_new_tour`.
//!
//! The rendering itself stays in `app.rs` for now. `tour_panel_ui` still reads
//! `commonmark_cache` and dispatches through `App`, so moving it would mean
//! widening fields rather than narrowing a signature. **Narrow first, move
//! second.**

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::PathBuf;

use crate::app::TOUR_POLL_INTERVAL;
use crate::bridge;

/// Everything the **tour panel** owns.
///
/// The fourth grouping lifted off `App` during the UI pause, and the smallest.
/// Its shape is `ModelListState`'s: the pane holds its own world, reports what
/// changed, and leaves the consequences to the caller.
///
/// # The seam
///
/// Selecting a tour re-initialises the right-hand side, because a tour starts
/// from its own first stop and leaving the previous tour's model on screen makes
/// Stop 1 look as though it has already been done. **That reset is not this
/// struct's business.** `select` and `poll` return *whether the selection
/// changed*; `App::reset_for_new_tour` decides what that invalidates.
pub(crate) struct TourState {
    /// The selected tour's text and the mtime it was read at.
    pub(crate) cached: Option<(String, std::time::SystemTime)>,
    /// Every tour on offer: the ad hoc one first when it exists, then fixtures.
    pub(crate) available: Vec<TourSource>,
    /// Which tour is showing.
    pub(crate) selected: Option<TourSource>,
    /// When the tour directory was last polled.
    pub(crate) polled_at: Option<std::time::Instant>,
    /// Specimens each fixture tour points at, for the list row. Filled at poll time,
    /// never in the paint path. See the note where it is populated.
    pub(crate) row_specimens: std::collections::HashMap<PathBuf, String>,
    /// Byte offset in the tour text to scroll to on the next frame, then cleared.
    ///
    /// Set by `hrw://tour/<name>/stop/<slug>` so a citation lands **at the stop**
    /// rather than at the top of the tour. One-shot deliberately: a persistent target
    /// would fight the scrollbar on every frame, which is the defect the autoplay
    /// scroll work spent four attempts on (`ui-findings.md` C15).
    pub(crate) scroll_to_offset: Option<usize>,

    /// **The self-running walk** of whichever tour is showing.
    ///
    /// Lives here rather than on `App` because it plays *this* tour and means
    /// nothing without one — `app_does_not_regrow_its_field_count` asked the
    /// question ("does the new field belong on App, or on the pane that owns
    /// it?") and this was the answer. The seam is the one this struct already
    /// documents: the pane holds its own world, and `App` keeps the
    /// consequences, since dispatching a beat needs `dispatch_hrw_link` and
    /// knowing when to hold the clock needs `compiling`.
    pub(crate) autoplay: crate::autoplay::Autoplay,
    /// Requested run length, from [`crate::autoplay::TOTAL_CHOICES`].
    pub(crate) autoplay_total: std::time::Duration,
    /// **Measured y of the current beat's link** inside the tour text.
    ///
    /// `app.rs` splits the markdown at the link's line and renders both halves; the
    /// cursor between them is this. Measured rather than estimated because two
    /// estimates failed — by beat ordinal, then by character offset — and rendered
    /// height per character is simply not constant: prose wraps in a narrow panel
    /// and a code block does not.
    pub(crate) tour_link_y: Option<f32>,
    /// The previous beat's measured y, to travel *from*.
    pub(crate) tour_prev_link_y: Option<f32>,
    /// Which beat `tour_link_y` was measured for, so a beat change can roll the
    /// current position into the previous one exactly once.
    pub(crate) tour_measured_beat: Option<usize>,
    /// Maximum scroll offset of the tour text, measured last frame — the clamp.
    pub(crate) tour_max_scroll: Option<f32>,

    /// **The mode a self-running walk started in**, to return to when it ends.
    ///
    /// A tour stop may legitimately leave Tour mode: `hrw://source/<line>` switches
    /// to Specimen mode because that is the only place the source renders, and a
    /// reader clicking it wants to be taken there. `matching.md` ends Act 3 with one,
    /// so a Play run finished in Specimen mode with the tour nowhere on screen —
    /// Doug, 2026-08-03: *"at the completion of the tour, the mode is being switched
    /// from tour mode to specimen mode."*
    ///
    /// **The stop is not wrong; the run just has to clean up after itself.** A walk
    /// is a round trip, so it restores what it borrowed, on finishing *or* on Stop.
    pub(crate) mode_before_autoplay: Option<crate::app::UiMode>,
}

impl Default for TourState {
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
            autoplay: crate::autoplay::Autoplay::default(),
            autoplay_total: crate::autoplay::DEFAULT_TOTAL,
            tour_link_y: None,
            tour_prev_link_y: None,
            tour_measured_beat: None,
            tour_max_scroll: None,
            mode_before_autoplay: None,
        }
    }
}

impl TourState {
    /// Switch to `source`, discarding the previous text.
    ///
    /// Returns **true when the selection actually changed**, which is the
    /// caller's cue to re-initialise the stage side. Re-clicking the tour already
    /// showing returns false, so a reader partway through a specimen keeps it.
    ///
    /// Clears `cached` rather than letting the poll notice: without this the old
    /// tour stays on screen until the next mtime comparison, and a reader who
    /// just clicked a different tour sees the previous one for up to an interval.
    pub(crate) fn select(&mut self, source: TourSource) -> bool {
        let changed = self.selected.as_ref() != Some(&source);
        self.selected = Some(source);
        self.cached = None;
        changed
    }

    /// The selected tour's text, if any has been read.
    pub(crate) fn text(&self) -> Option<&str> {
        self.cached.as_ref().map(|(t, _)| t.as_str())
    }

    /// **Forget where the text was scrolled to.**
    ///
    /// These are pixel positions measured in *one particular document at one
    /// particular beat*, so they mean nothing after a tour changes or a run starts
    /// over — and they are worse than meaningless, because the scroll interpolates
    /// **from** the stale value on the first frame of a new run.
    ///
    /// Doug, 2026-08-03: stop mid-tour, switch tours, switch back, press Play, and
    /// *"the matching tour rescrolls very visibly from the stopped position back up
    /// to the top before the tour begins playing."* The pane was correctly at the top
    /// already; the *bookkeeping* still held the stopped position, so the first frame
    /// aimed there and then travelled back.
    ///
    /// With these cleared, both ends of the interpolation are zero on the first beat,
    /// so a tour that is already at the top does not move at all — which is the
    /// behaviour asked for. `tour_max_scroll` is deliberately **kept**: it describes
    /// the pane, not the position, and re-measures on the next frame anyway.
    pub(crate) fn reset_scroll(&mut self) {
        self.tour_link_y = None;
        self.tour_prev_link_y = None;
        self.tour_measured_beat = None;
    }

    /// Re-read the list and the selected tour, at most once per
    /// [`TOUR_POLL_INTERVAL`]. Returns true when a tour was newly selected.
    pub(crate) fn poll(&mut self) -> bool {
        let mut newly_selected = false;
        let due = self
            .polled_at
            .is_none_or(|last| last.elapsed() >= TOUR_POLL_INTERVAL);
        if !due {
            return false;
        }
        self.polled_at = Some(std::time::Instant::now());

        // --- Rebuild the pick list ---
        //
        // Ad hoc first when it exists: it is the answer to the question just asked,
        // and burying it under the fixtures would make the common case the awkward one.
        let mut tours = Vec::new();
        if std::path::Path::new(bridge::TOUR_FILE).exists() {
            tours.push(TourSource::AdHoc);
        }
        tours.extend(bridge::fixture_tours().into_iter().map(TourSource::Fixture));
        let list_changed = tours != self.available;
        self.available = tours;

        // **Which specimens each tour uses, read here rather than per frame.**
        //
        // The row shows it so that searching by model finds the tour — Doug went
        // looking for a "DimensionMismatch tour" and it is `failure-typecheck`. Read
        // at poll time because the paint path must not touch the filesystem
        // (`CLAUDE.md`, debugging conventions), and re-read on every poll because a
        // tour edited between polls would otherwise advertise the wrong model, which
        // is worse than advertising none.
        if list_changed || self.row_specimens.is_empty() {
            self.row_specimens.clear();
            for source in &self.available {
                if let TourSource::Fixture(p) = source
                    && let Ok(md) = std::fs::read_to_string(p)
                {
                    let names = TourSource::specimens_in(&md);
                    if !names.is_empty() {
                        self.row_specimens.insert(p.clone(), names.join(", "));
                    }
                }
            }
        }

        // A selection that no longer exists (the ad hoc tour was deleted, a fixture
        // renamed) must not leave stale text on screen attributed to a live file.
        if self
            .selected
            .as_ref()
            .is_some_and(|t| !self.available.contains(t))
        {
            self.selected = None;
            self.cached = None;
        }
        // Default to the ad hoc tour when one appears and nothing is chosen.
        if self.selected.is_none() && self.available.contains(&TourSource::AdHoc) {
            newly_selected |= self.select(TourSource::AdHoc);
        }

        // --- Re-read the selected tour if it changed on disk ---
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
                        .map(|text| (text, mtime));
                }
            }
            // The file vanished between listing and reading. Drop the text rather
            // than keep a stale copy: for an ad hoc tour, absence is the normal state.
            None => {
                self.cached = None;
                self.selected = None;
            }
        }
        newly_selected
    }
}

/// Which tour the Tour panel is showing.
///
/// Two kinds, with different lifetimes and different jobs:
///
/// - **`AdHoc`** — `.hrw-bridge/tour.md`, written by Claude to answer the question just
///   asked. Gitignored, regenerated, ephemeral by construction.
/// - **`Fixture`** — a file in `docs/fixture-tours/`, kept and versioned because it is a
///   *test* with a pass/fail criterion rather than an explanation that would rot.
///
/// Keeping them in one list (rather than one panel each) is deliberate: from the
/// reader's side both are "a sequence of stops to walk", and the distinction is about
/// where the file lives, not about how it is used.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TourSource {
    AdHoc,
    Fixture(PathBuf),
}

impl TourSource {
    pub(crate) fn path(&self) -> PathBuf {
        match self {
            Self::AdHoc => PathBuf::from(bridge::TOUR_FILE),
            Self::Fixture(p) => p.clone(),
        }
    }

    /// Label for the picker. The ad hoc tour is named by what it *is* rather than by
    /// its filename, which is an implementation detail nobody should have to know.
    /// The specimens a tour points at, in first-appearance order.
    ///
    /// **Doug went looking for a "DimensionMismatch tour" and it is called
    /// `failure-typecheck`** (2026-08-05). The tours are named by *phase* — which is
    /// what he asked for, because a per-phase name sets a per-phase expectation — but
    /// he was thinking about the *model* in front of him. Neither name is wrong; the
    /// row was just showing one axis and he was searching on the other.
    ///
    /// Derived from the `hrw://load/<Specimen>` links rather than declared, so it
    /// cannot disagree with where the tour actually sends him. Same extraction the
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

    pub(crate) fn label(&self) -> String {
        match self {
            Self::AdHoc => "\u{2728} Claude's answer".to_owned(),
            Self::Fixture(p) => p
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("fixture")
                .to_owned(),
        }
    }
}

/// Build the tour catalogue text — the `CATALOGUE.md` written for Claude.
///
/// **In the library rather than in `examples/gen_tour_catalogue.rs`** so that
/// `tour_catalogue_is_current` calls the same code that writes the file. A checker
/// that reimplements what it checks is the drift `docs/fidelity-plan.md` warns
/// about, and a `#[path]` include of an example does not compile inside the lib.
pub fn catalogue(dir: &std::path::Path) -> String {
    let mut tours: Vec<PathBuf> = std::fs::read_dir(dir)
        .expect("read fixture-tours")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("md"))
        .filter(|p| {
            !matches!(
                p.file_stem().and_then(|s| s.to_str()),
                Some("README") | Some("CATALOGUE")
            )
        })
        .collect();
    tours.sort();

    let mut s = String::new();
    s.push_str("# Tour catalogue\n\n");
    s.push_str(
        "**Generated — do not edit.** `cargo run -p hrw --example gen_tour_catalogue`.\n\n\
         **Audience: Claude.** This exists so a question can be answered by citing a tour that \
         already demonstrates the thing, rather than by writing a new one that retells it \
         without its checked expectations (`docs/ideas.md` #63). Cite a stop with \
         `hrw://tour/<name>/stop/<slug>`.\n\n\
         **Re-read a tour before citing it.** Everything below is derived and current; whether \
         a tour's *claims* still hold is not something a catalogue can know, and a tour \
         promised a tree the pane did not show for its whole existence.\n\n",
    );

    for path in &tours {
        let md = std::fs::read_to_string(path).unwrap_or_default();
        let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("?");

        let title = md
            .lines()
            .find_map(|l| l.strip_prefix("# "))
            .unwrap_or("(untitled)");
        // The first bolded line: tours open with their subject in bold.
        let lead = md
            .lines()
            .find(|l| l.trim_start().starts_with("**") && l.len() > 12)
            .map(|l| l.trim().trim_start_matches("**").replace("**", ""))
            .unwrap_or_default();

        // **The same extraction the tour list uses**, not a second copy of it. The
        // first version of this duplicated the loop, and the doc comment on
        // `specimens_in` claimed they were shared when they were not — a false claim
        // in a comment, caught the same day by `unreachable_pub` forcing a look at
        // the function's visibility.
        let specimens = TourSource::specimens_in(&md);
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

        let stops: Vec<String> = crate::autoplay::parse_stops(&md)
            .iter()
            .map(|st| {
                format!(
                    "  - `{}` — {}",
                    crate::autoplay::stop_slug(&st.heading),
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
            let _ = writeln!(s, "- **Stops:**");
            for line in &stops {
                let _ = writeln!(s, "{line}");
            }
        }
        s.push('\n');
    }
    s
}
