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
            autoplay: crate::autoplay::Autoplay::default(),
            autoplay_total: crate::autoplay::DEFAULT_TOTAL,
            tour_link_y: None,
            tour_prev_link_y: None,
            tour_measured_beat: None,
            tour_max_scroll: None,
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

        // A selection that no longer exists (the ad hoc tour was deleted, a fixture
        // renamed) must not leave stale text on screen attributed to a live file.
        if self.selected.as_ref().is_some_and(|t| !self.available.contains(t)) {
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
        let mtime = std::fs::metadata(&path).ok().and_then(|m| m.modified().ok());
        match mtime {
            Some(mtime) => {
                let unchanged =
                    self.cached.as_ref().is_some_and(|(_, seen)| *seen == mtime);
                if !unchanged || list_changed {
                    self.cached =
                        std::fs::read_to_string(&path).ok().map(|text| (text, mtime));
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
