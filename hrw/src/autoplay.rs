//! **Self-running tours** — the schedule and the clock behind the Play button.
//!
//! Doug, 2026-08-03: a LinkedIn screenshot of HRW drew immediate interest, and
//! explaining *what a tour is* in prose to people who have never seen the tool is
//! harder than showing one. So a tour needs to run itself, for long enough to be
//! captured as a video and no longer.
//!
//! # Why this module is pure
//!
//! Everything here is arithmetic over `Duration` and `&str`. No egui, no
//! `App`, no clock of its own — [`Autoplay::tick`] is *told* how much time
//! passed. That is what makes a timing feature testable at all: a schedule that
//! could only be checked by watching it is a schedule nobody checks. The two
//! properties worth guaranteeing (the run lasts exactly as long as asked; a stop
//! with more prose gets more time) are both plain assertions here and would be
//! stopwatch work anywhere else.
//!
//! # The three decisions that shape a watchable video
//!
//! 1. **A beat is a link, not a stop.** `dae-construction.md` has seven stops and
//!    about twenty links. Advancing per *stop* would mean seven jumps separated by
//!    long stillness; advancing per *link* keeps something moving — the tree opens,
//!    then a node highlights, then another — which is what makes a recording read
//!    as a demonstration rather than a slideshow.
//!
//! 2. **Time is weighted by prose length.** Stops are not equal: a stop that sets
//!    up the phase deserves longer on screen than one that points at a field. Prose
//!    length is a crude proxy for that and a good one, now that
//!    [`the tour prose is load-bearing`](../docs/fixture-tours/README.md).
//!
//! 3. **The clock stops while the app is busy.** A `load` beat compiles a
//!    specimen. If the countdown ran through the compile, the video would spend its
//!    budget on "compiling…" and the interesting frame would be cut off by the next
//!    beat. [`Autoplay::tick`] takes a `busy` flag and simply does not advance —
//!    so a slow machine produces a *longer* video, never a broken one.
//!
//!    **This is the one place the promised duration is deliberately not honoured**,
//!    and that trade is the right way round: a video that runs eight seconds long
//!    is fine, one that cuts away mid-compile is not.

use std::time::Duration;

/// Total run lengths offered in the UI.
///
/// **These are conventional social-video lengths, not a measured optimum**, and
/// they are a picker rather than a constant because the guidance moves and the
/// judgement is the author's. The default is [`DEFAULT_TOTAL`].
pub const TOTAL_CHOICES: [(&str, u64); 4] = [
    ("30s \u{2014} teaser", 30),
    ("60s \u{2014} short", 60),
    ("90s \u{2014} standard", 90),
    ("3min \u{2014} deep", 180),
];

/// Default run length: 90 seconds.
///
/// The middle of the commonly cited 30–90s range for feed video, and long
/// enough that a seven-stop tour still gets several seconds per beat.
pub const DEFAULT_TOTAL: Duration = Duration::from_secs(90);

/// Floor on any single beat, so a short stop is still readable.
///
/// Without it, a stop whose prose is one line would flash past at a fraction of a
/// second on a 30s run and read as a glitch rather than a step.
const MIN_BEAT: Duration = Duration::from_millis(900);

/// How long the tour text takes to travel to a new beat's position.
///
/// **After this, it holds still for the rest of the beat.** Doug, 2026-08-03:
/// *"the scrolling never pauses when a frame is being displayed."* The scroll was
/// driven by [`Autoplay::fraction`], which is a **clock** — it advances every frame,
/// so the prose crept continuously while the reader was trying to read it, and a
/// paused animation sat under text that would not stay still.
///
/// Position and time are different quantities and this is where they part company.
/// The progress bar still tracks the clock, because that is what a progress bar is
/// for; the text tracks the *beat*, which changes in steps.
const SCROLL_TRAVEL: Duration = Duration::from_millis(450);

/// Beats that open another application get this much extra weight.
///
/// Wolfram Desktop and System Modeler come to the front and need a moment to be
/// *seen* — and, unlike an HRW beat, the viewer has to reorient to a different
/// window. Prestarting them (as Doug does when recording) removes the launch cost
/// but not the reorientation.
const EXTERNAL_WEIGHT_MULTIPLIER: f64 = 2.5;

/// One link, and **where in the document it sits**.
///
/// The position is what lets the tour text scroll to the link being dispatched
/// rather than to a guess. Doug, 2026-08-03: *"when a link for a frame is
/// encountered … the scrolling should be paused with that frame link showing with
/// perhaps a line or two of text which is above that frame link."*
///
/// `doc_fraction` is the link's character offset over the document's length. It is
/// an **approximation of rendered position** — a code block or a table is denser per
/// character than prose — but it tracks the actual document, which the previous
/// beat-ordinal scheme did not: that spaced beats evenly regardless of how much text
/// lay between them, so a stop with seven links and a stop with one advanced by the
/// same distance.
#[derive(Debug, Clone, PartialEq)]
pub struct TourLink {
    pub url: String,
    /// Byte offset of the **start of the line** this link is on.
    ///
    /// A byte offset rather than a fraction because the consumer splits the
    /// markdown here and renders the two halves, which yields the link's exact
    /// pixel position. A fraction can only ever be an estimate of that, and the
    /// estimate was wrong in a direction no constant could correct.
    pub byte_offset: usize,
}

/// One stop of a tour: a heading, its prose, and the links inside it in order.
#[derive(Debug, Clone, PartialEq)]
pub struct TourStop {
    /// The heading text, with the `##` and any leading `Stop N —` intact, because
    /// that is what a viewer sees as the caption.
    pub heading: String,
    /// Characters of prose, used to weight this stop's share of the run.
    pub prose_chars: usize,
    /// Byte offset of the heading line, for a stop that dispatches nothing.
    pub heading_offset: usize,
    /// Every `hrw://` link in this stop, **in document order and not deduplicated**.
    ///
    /// Deduplicating would be wrong here in a way it is not for hook registration:
    /// `dae-construction.md` returns to `hrw://load/SingleInertia/Dae` three times,
    /// and those are three different moments in the walk.
    pub links: Vec<TourLink>,
}

/// One dispatched moment: show this link for this long.
#[derive(Debug, Clone, PartialEq)]
pub struct Beat {
    /// Index into the stop list, for the caption and the progress readout.
    pub stop: usize,
    /// The link to dispatch, or `None` for a stop that is prose only.
    pub link: Option<String>,
    /// How long to remain here once the app is idle.
    pub dwell: Duration,
    /// Byte offset of the line this beat's link is on. The tour text is split
    /// here and rendered in two halves, so the seam **is** the link's position and
    /// the scroll lands on it exactly.
    pub byte_offset: usize,
}

/// Split tour markdown into stops.
///
/// A `##` heading starts a stop; everything before the first one becomes the
/// **preamble stop**, titled from the `#` heading. The preamble is a stop rather
/// than being skipped because a video needs a beat of title card before it starts
/// moving, and the tour already has the words for one.
///
/// Fenced code blocks are skipped when looking for headings, so a `##` comment
/// inside a Modelica or shell block cannot invent a stop.
pub fn parse_stops(text: &str) -> Vec<TourStop> {
    let mut stops: Vec<TourStop> = Vec::new();
    let mut title = String::from("Tour");
    let mut current: Option<TourStop> = None;
    let mut in_fence = false;

    // Byte offsets of line starts. Always char boundaries, so the consumer can
    // split the string here without checking.
    let mut seen_bytes = 0usize;

    for line in text.lines() {
        let line_start = seen_bytes;
        seen_bytes += line.len() + 1; // + the newline
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            in_fence = !in_fence;
        }
        if !in_fence && let Some(rest) = trimmed.strip_prefix("# ")
            && current.is_none()
            && stops.is_empty()
        {
            title = rest.trim().to_owned();
        }
        if !in_fence && let Some(rest) = trimmed.strip_prefix("## ") {
            if let Some(done) = current.take() {
                stops.push(done);
            } else if !stops.is_empty() || title != "Tour" {
                // Close the preamble, if anything preceded the first `##`.
            }
            current = Some(TourStop {
                heading: rest.trim().to_owned(),
                prose_chars: 0,
                heading_offset: line_start,
                links: Vec::new(),
            });
            continue;
        }
        let stop = match current.as_mut() {
            Some(s) => s,
            None => {
                // Still in the preamble; open it lazily so a tour that starts
                // with `##` does not get an empty leading card.
                current = Some(TourStop {
                    heading: title.clone(),
                    prose_chars: 0,
                    heading_offset: 0,
                    links: Vec::new(),
                });
                current.as_mut().expect("just set")
            }
        };
        stop.prose_chars += line.trim().chars().count();
        stop.links.extend(
            links_in_order(line)
                .into_iter()
                .map(|url| TourLink { url, byte_offset: line_start }),
        );
    }
    if let Some(done) = current.take() {
        stops.push(done);
    }
    stops
}

/// Every `hrw://` URL on one line, in order, **keeping duplicates**.
///
/// Terminator set matches `app::extract_hrw_links`, including the backtick: a verb
/// named inside a code span is prose *about* the link, not a link.
fn links_in_order(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (start, _) in line.match_indices("hrw://") {
        let rest = &line[start..];
        let end = rest.find([')', ' ', '\n', '"', '>', '`']).unwrap_or(rest.len());
        out.push(rest[..end].to_owned());
    }
    out
}

/// Build the run: one beat per link, weighted by prose, summing to `total`.
///
/// `is_external` marks links that leave HRW (a notebook or System Modeler), which
/// earn extra dwell. It is a closure so this module stays ignorant of the link
/// grammar, which lives in `app.rs`.
///
/// # The guarantee
///
/// **The dwells sum to exactly `total`.** Rounding is absorbed by the final beat
/// rather than distributed, so the promise the UI makes ("90 seconds") is one the
/// schedule keeps rather than approximately keeps. What the *run* may exceed is
/// covered in the module docs: waiting on a compile is untimed by design.
pub fn schedule(
    stops: &[TourStop],
    total: Duration,
    is_external: impl Fn(&str) -> bool,
) -> Vec<Beat> {
    // Every stop contributes at least one beat, so a prose-only stop still shows.
    let mut raw: Vec<(usize, Option<String>, f64, usize)> = Vec::new();
    for (i, stop) in stops.iter().enumerate() {
        let n = stop.links.len().max(1) as f64;
        // Prose is shared across the stop's beats: a long stop with one link
        // lingers, a long stop with six links moves through them.
        let per = (stop.prose_chars as f64 / n).max(1.0);
        if stop.links.is_empty() {
            raw.push((i, None, per, stop.heading_offset));
            continue;
        }
        for link in &stop.links {
            let w =
                if is_external(&link.url) { per * EXTERNAL_WEIGHT_MULTIPLIER } else { per };
            raw.push((i, Some(link.url.clone()), w, link.byte_offset));
        }
    }
    if raw.is_empty() {
        return Vec::new();
    }

    let total_ms = total.as_millis() as f64;
    let sum: f64 = raw.iter().map(|(_, _, w, _)| w).sum();
    let min_ms = MIN_BEAT.as_millis() as f64;

    // The floor can overrun the budget on a long tour with a short total. When it
    // does, honouring the floor would silently break the duration promise, so the
    // floor yields: an even split is the least-bad answer and is still uniform.
    let floored = min_ms * raw.len() as f64 > total_ms;

    let mut beats: Vec<Beat> = Vec::with_capacity(raw.len());
    let mut spent_ms = 0u128;
    for (idx, (stop, link, w, byte_offset)) in raw.iter().enumerate() {
        let share = if floored {
            total_ms / raw.len() as f64
        } else {
            (total_ms * w / sum).max(min_ms)
        };
        let ms = if idx + 1 == raw.len() {
            // Last beat absorbs all rounding, so the sum is exact.
            (total.as_millis()).saturating_sub(spent_ms)
        } else {
            let ms = share.round() as u128;
            spent_ms += ms;
            ms
        };
        beats.push(Beat {
            stop: *stop,
            link: link.clone(),
            dwell: Duration::from_millis(ms as u64),
            byte_offset: *byte_offset,
        });
    }
    beats
}

/// What the clock is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// No run in progress.
    Idle,
    /// Running; the current beat is counting down.
    Playing,
    /// Held by the user, or by the window losing focus.
    Paused,
    /// The last beat finished.
    Finished,
}

/// The clock. Told how much time passed; says when to move.
#[derive(Debug, Default)]
pub struct Autoplay {
    beats: Vec<Beat>,
    /// Index of the beat currently showing.
    index: usize,
    /// Time spent on the current beat.
    in_beat: Duration,
    phase: Option<Phase>,
    /// Set while a beat has been dispatched but the app has not gone idle.
    waiting: bool,
    /// Total time actually spent, including waits — what the recording will be.
    real_elapsed: Duration,
    /// Time since the current beat was **dispatched**, which keeps running while
    /// the app is busy. Drives [`Self::travel_t`] so the tour text moves to the
    /// link *during* the compile it caused rather than after it.
    since_dispatch: Duration,
    /// **Who paused matters.** A pause caused by the window losing focus should
    /// lift the moment focus returns; a pause the user asked for must survive
    /// clicking into another window and back, or the Pause button would be
    /// undone by the act of using anything else.
    paused_by_focus: bool,
}

impl Autoplay {
    pub fn phase(&self) -> Phase {
        self.phase.unwrap_or(Phase::Idle)
    }

    pub fn is_running(&self) -> bool {
        matches!(self.phase(), Phase::Playing | Phase::Paused)
    }

    /// Begin a run. Returns the first beat to dispatch, if there is one.
    pub fn start(&mut self, beats: Vec<Beat>) -> Option<Beat> {
        self.beats = beats;
        self.index = 0;
        self.in_beat = Duration::ZERO;
        self.real_elapsed = Duration::ZERO;
        self.since_dispatch = Duration::ZERO;
        self.waiting = true;
        if self.beats.is_empty() {
            self.phase = Some(Phase::Finished);
            return None;
        }
        self.phase = Some(Phase::Playing);
        self.beats.first().cloned()
    }

    /// Pause because the user asked. Survives focus changes.
    pub fn pause(&mut self) {
        if self.phase() == Phase::Playing {
            self.phase = Some(Phase::Paused);
            self.paused_by_focus = false;
        }
    }

    /// Resume because the user asked.
    pub fn resume(&mut self) {
        if self.phase() == Phase::Paused {
            self.phase = Some(Phase::Playing);
            self.paused_by_focus = false;
        }
    }

    /// Track window focus, pausing while HRW is not the front window.
    ///
    /// An external stop brings Wolfram Desktop or System Modeler forward, and a
    /// clock that kept running behind another window would advance the walk while
    /// nobody was looking at it — the recording would come back to a tour that had
    /// moved on. Focus returning lifts *this* pause and no other, so a user-pressed
    /// Pause is not undone by clicking away and back.
    ///
    /// **This is also what makes an external hop last as long as the viewer wants**
    /// rather than as long as the schedule guessed.
    pub fn set_focused(&mut self, focused: bool) {
        match (focused, self.phase()) {
            (false, Phase::Playing) => {
                self.phase = Some(Phase::Paused);
                self.paused_by_focus = true;
            }
            (true, Phase::Paused) if self.paused_by_focus => {
                self.phase = Some(Phase::Playing);
                self.paused_by_focus = false;
            }
            _ => {}
        }
    }

    /// Abandon the run. The tour text and the app's state stay as they are — a
    /// viewer who stops halfway is looking at something they wanted to look at.
    pub fn stop(&mut self) {
        self.phase = Some(Phase::Idle);
        self.beats.clear();
        self.index = 0;
        self.in_beat = Duration::ZERO;
    }

    /// Advance by `dt`. Returns the next beat to dispatch, when one is due.
    ///
    /// `busy` is the app compiling or otherwise mid-work. **While busy the clock
    /// does not advance at all** — see the module docs: the dwell is time the
    /// viewer spends *looking at a finished frame*, and counting a compile against
    /// it would spend the budget on a progress spinner.
    pub fn tick(&mut self, dt: Duration, busy: bool) -> Option<Beat> {
        if self.phase() != Phase::Playing {
            return None;
        }
        self.real_elapsed += dt;
        // **Before the `busy` return.** The text must travel to the link while the
        // compile it triggered is still running; see `travel_t`.
        self.since_dispatch += dt;
        if busy {
            self.waiting = true;
            return None;
        }
        // First idle frame after a dispatch: the beat's own time starts here.
        if self.waiting {
            self.waiting = false;
            self.in_beat = Duration::ZERO;
            return None;
        }
        self.in_beat += dt;
        let due = self.beats.get(self.index).map(|b| b.dwell).unwrap_or_default();
        if self.in_beat < due {
            return None;
        }
        self.index += 1;
        self.in_beat = Duration::ZERO;
        self.since_dispatch = Duration::ZERO;
        match self.beats.get(self.index).cloned() {
            Some(next) => {
                self.waiting = true;
                Some(next)
            }
            None => {
                self.phase = Some(Phase::Finished);
                None
            }
        }
    }

    /// Which beat is showing, out of how many (1-based for display).
    pub fn progress(&self) -> (usize, usize) {
        (self.index.min(self.beats.len().saturating_sub(1)) + 1, self.beats.len())
    }

    /// Fraction of the run completed, for a progress bar and for scrolling the
    /// tour text in step with the walk.
    pub fn fraction(&self) -> f32 {
        if self.beats.is_empty() {
            return 0.0;
        }
        if self.phase() == Phase::Finished {
            return 1.0;
        }
        let done: u128 = self.beats[..self.index].iter().map(|b| b.dwell.as_millis()).sum();
        let total: u128 = self.beats.iter().map(|b| b.dwell.as_millis()).sum();
        if total == 0 {
            return 0.0;
        }
        ((done + self.in_beat.as_millis()) as f64 / total as f64).clamp(0.0, 1.0) as f32
    }

    /// **How far the text has travelled toward this beat's link**, 0.0 to 1.0.
    ///
    /// The caller interpolates the scroll offset between the previous link's
    /// measured position and this one's; at 1.0 the text is on the link and
    /// **stops**, leaving the rest of the beat for reading.
    ///
    /// # It advances while the app is busy, and that is the fix
    ///
    /// Doug, 2026-08-03: *"while that log view is showing that compilation is
    /// happening … the link which caused all of the changes on the RHS is not being
    /// shown."* The travel used to run off `in_beat`, which [`Self::tick`] freezes at
    /// zero for the whole compile — so the text sat at the *previous* beat's link
    /// until the compile finished, which is precisely when the reader wants to see
    /// **which link asked for this**.
    ///
    /// So this runs off `since_dispatch`, which starts when the beat is dispatched
    /// and keeps running regardless of `busy`. **The text leads and the right-hand
    /// side follows**, rather than both arriving after the wait.
    ///
    /// The dwell is still not charged during a compile — that separation is the
    /// point. *When the reader is shown the link* and *how long the beat lasts* are
    /// different questions, and conflating them is what produced this bug.
    pub fn travel_t(&self) -> f32 {
        if self.beats.is_empty() {
            return 1.0;
        }
        let travel = SCROLL_TRAVEL;
        let t = (self.since_dispatch.as_secs_f32() / travel.as_secs_f32()).clamp(0.0, 1.0);
        // Smoothstep: a hard jump reads as a glitch, a linear ramp as drift.
        t * t * (3.0 - 2.0 * t)
    }

    /// Byte offset in the tour text of the line the current beat's link is on.
    ///
    /// The consumer splits the markdown here and renders both halves, so the seam
    /// is the link's exact pixel position — no estimate involved.
    pub fn current_byte_offset(&self) -> usize {
        self.beats
            .get(self.index.min(self.beats.len().saturating_sub(1)))
            .map(|b| b.byte_offset)
            .unwrap_or(0)
    }

    /// The stop index currently showing, for the caption.
    pub fn current_stop(&self) -> Option<usize> {
        self.beats.get(self.index.min(self.beats.len().saturating_sub(1))).map(|b| b.stop)
    }

    /// Wall-clock time actually spent, which exceeds the requested total by
    /// however long the compiles took.
    pub fn real_elapsed(&self) -> Duration {
        self.real_elapsed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A beat for tests, positioned a quarter of the document apart.
    ///
    /// A constructor rather than struct literals at every site: `Beat` gained
    /// `doc_fraction` on 2026-08-03 and broke seven literals at once, which is the
    /// tax that makes tests feel expensive to keep.
    fn beat(stop: usize, link: Option<&str>, secs: u64) -> Beat {
        Beat {
            stop,
            link: link.map(str::to_owned),
            dwell: Duration::from_secs(secs),
            byte_offset: stop * 100,
        }
    }

    const SAMPLE: &str = "\
# Fixture tour — demo

Preamble prose that sets the scene for the whole thing.

## Stop 1 — first

Some prose here, a moderate amount of it, enough to weigh more than stop two.
More prose on a second line to make the difference unmistakable.

[a](hrw://load/M/Dae)
[b](hrw://stage/Dae/node/x)

## Stop 2 — second

Short.

[c](hrw://notebook/n.nb)
";

    #[test]
    fn parsing_finds_the_preamble_and_every_stop_in_order() {
        let stops = parse_stops(SAMPLE);
        assert_eq!(stops.len(), 3, "preamble plus two stops: {stops:#?}");
        assert_eq!(stops[0].heading, "Fixture tour — demo", "the preamble is titled by `#`");
        assert!(stops[0].links.is_empty(), "the preamble has no links here");
        assert_eq!(stops[1].heading, "Stop 1 — first");
        let urls: Vec<&str> = stops[1].links.iter().map(|l| l.url.as_str()).collect();
        assert_eq!(urls, vec!["hrw://load/M/Dae", "hrw://stage/Dae/node/x"]);
        assert_eq!(stops[2].links[0].url, "hrw://notebook/n.nb");
        assert!(
            stops[1].prose_chars > stops[2].prose_chars,
            "stop 1 is visibly longer, and the schedule depends on noticing that",
        );
    }

    /// **Duplicate links are kept**, unlike hook registration.
    ///
    /// `dae-construction.md` returns to the same DAE tab three times, and those are
    /// three different moments in the walk. Deduplicating would silently shorten
    /// the run and drop two of them.
    #[test]
    fn a_repeated_link_is_a_repeated_beat() {
        let stops = parse_stops(
            "## S\n[a](hrw://load/M/Dae)\ntext\n[b](hrw://load/M/Dae)\n",
        );
        assert_eq!(stops[0].links.len(), 2, "the same target twice is two beats");
    }

    /// A `##` inside a fenced block is code, not a stop.
    #[test]
    fn a_heading_inside_a_code_fence_does_not_invent_a_stop() {
        let stops = parse_stops("## Real\n```sh\n## not a heading\n```\ntext\n");
        assert_eq!(stops.len(), 1, "one real stop: {stops:#?}");
    }

    /// **The schedule lasts exactly as long as it promised.**
    ///
    /// The reason this is a test and not a comment: the run length is the entire
    /// feature request. A schedule that drifts by a few percent per beat is
    /// invisible in review and obvious in a recording that overruns its slot.
    #[test]
    fn the_dwells_sum_to_the_requested_total() {
        let stops = parse_stops(SAMPLE);
        for secs in [30u64, 60, 90, 180] {
            let total = Duration::from_secs(secs);
            let beats = schedule(&stops, total, |l| l.contains("notebook"));
            let sum: Duration = beats.iter().map(|b| b.dwell).sum();
            assert_eq!(sum, total, "a {secs}s run must schedule exactly {secs}s");
            assert!(!beats.is_empty());
        }
    }

    /// More prose earns more time, and an external hop earns more still.
    #[test]
    fn weighting_favours_long_stops_and_external_hops() {
        let stops = parse_stops(SAMPLE);
        let beats = schedule(&stops, Duration::from_secs(90), |l| l.contains("notebook"));

        let stop1: Duration = beats.iter().filter(|b| b.stop == 1).map(|b| b.dwell).sum();
        let stop2: Duration = beats.iter().filter(|b| b.stop == 2).map(|b| b.dwell).sum();
        assert!(
            stop1 > stop2,
            "stop 1 has far more prose and two links; it must not get less time \
             ({stop1:?} vs {stop2:?})",
        );

        // The external beat outweighs a plain beat drawn from the same stop's prose.
        let external = beats.iter().find(|b| b.link.as_deref() == Some("hrw://notebook/n.nb"));
        assert!(external.is_some_and(|b| b.dwell >= MIN_BEAT), "external beats are not clipped");
    }

    /// Every stop appears, including one with no links at all.
    #[test]
    fn a_prose_only_stop_still_gets_a_beat() {
        let stops = parse_stops("# T\nintro\n\n## Silent\njust words, no links\n");
        let beats = schedule(&stops, Duration::from_secs(30), |_| false);
        let covered: Vec<usize> = beats.iter().map(|b| b.stop).collect();
        for i in 0..stops.len() {
            assert!(covered.contains(&i), "stop {i} was scheduled out of the run");
        }
        assert!(beats.iter().any(|b| b.link.is_none()), "a prose stop dwells without dispatching");
    }

    /// **The clock does not run while the app is busy.**
    ///
    /// The behaviour that keeps a recording watchable: a `load` beat compiles, and
    /// counting the compile against the dwell would spend the beat on a spinner and
    /// cut away as the interesting frame arrived.
    #[test]
    fn a_busy_app_does_not_burn_the_dwell() {
        let beats = vec![
            beat(0, Some("a"), 2),
            beat(1, Some("b"), 2),
        ];
        let mut ap = Autoplay::default();
        assert_eq!(ap.start(beats).and_then(|b| b.link), Some("a".to_owned()));

        // Ten seconds of compiling: no advance, and no time charged to the beat.
        for _ in 0..10 {
            assert!(ap.tick(Duration::from_secs(1), true).is_none(), "busy must not advance");
        }
        // First idle frame arms the beat without consuming it.
        assert!(ap.tick(Duration::from_millis(16), false).is_none());
        // Now the dwell runs.
        assert!(ap.tick(Duration::from_millis(1_500), false).is_none(), "not due yet");
        let next = ap.tick(Duration::from_millis(600), false);
        assert_eq!(next.and_then(|b| b.link), Some("b".to_owned()), "due, so it advances");

        // And the run reports the real cost, which is longer than the schedule.
        assert!(
            ap.real_elapsed() >= Duration::from_secs(12),
            "the wait is invisible to the schedule but not to the recording",
        );
    }

    /// Pause holds the clock; resume continues from where it stopped.
    #[test]
    fn pause_holds_and_resume_continues() {
        let beats =
            vec![beat(0, None, 2)];
        let mut ap = Autoplay::default();
        ap.start(beats);
        ap.tick(Duration::from_millis(16), false); // arm
        ap.tick(Duration::from_millis(1_000), false);
        ap.pause();
        assert_eq!(ap.phase(), Phase::Paused);
        for _ in 0..100 {
            assert!(ap.tick(Duration::from_secs(1), false).is_none(), "paused must not advance");
        }
        ap.resume();
        assert!(ap.tick(Duration::from_millis(1_100), false).is_none(), "last beat, so no next");
        assert_eq!(ap.phase(), Phase::Finished, "the run ends rather than looping");
        assert_eq!(ap.fraction(), 1.0);
    }

    /// **The text travels to the link and then stops.**
    ///
    /// Doug: *"the scrolling never pauses when a frame is being displayed."* The
    /// travel used to be driven by `fraction()`, a **clock**, so the prose crept
    /// every frame -- worst exactly where the tour is best, with a deliberately
    /// paused animation under text that would not stay still.
    #[test]
    fn the_text_travels_then_stops_until_the_beat_changes() {
        let mut ap = Autoplay::default();
        ap.start(vec![beat(0, Some("a"), 6), beat(1, Some("b"), 6)]);
        assert_eq!(ap.travel_t(), 0.0, "a fresh beat has not travelled yet");

        ap.tick(SCROLL_TRAVEL, false);
        assert_eq!(ap.travel_t(), 1.0, "and arrives within the travel window");

        // Then it is STILL -- five seconds of ticking must not move it.
        for _ in 0..50 {
            ap.tick(Duration::from_millis(100), false);
            assert_eq!(
                ap.travel_t(),
                1.0,
                "the text must hold still for the rest of the beat; a reader cannot                  read prose that is sliding, and the animation under it is paused",
            );
        }

        // **Non-vacuity**: a new beat restarts the travel and re-aims it, so
        // `travel_t` is not simply pinned at 1.0.
        ap.tick(Duration::from_secs(2), false);
        assert_eq!(ap.progress().0, 2, "precondition: the beat advanced");
        assert!(ap.travel_t() < 1.0, "a new beat starts a new travel");
        assert_eq!(ap.current_byte_offset(), 100, "and aims at the new link's line");
    }

    /// **The text reaches the link WHILE the app is still compiling.**
    ///
    /// Doug, 2026-08-03: *"while that log view is showing that compilation is
    /// happening, and when the RHS is navigated to the Structural -> Matching view,
    /// the link which caused all of the changes on the RHS is not being shown."*
    ///
    /// The travel ran off `in_beat`, which `tick` freezes at zero for the whole
    /// compile -- so the text stayed at the *previous* link until the compile
    /// finished, which is exactly when a viewer wants to know **which link asked for
    /// this**. `since_dispatch` runs regardless of `busy`, so the text leads and the
    /// right-hand side follows.
    ///
    /// The dwell is still not charged during a compile, and keeping those apart is
    /// the point: *when the reader is shown the link* and *how long the beat lasts*
    /// are different questions, and conflating them produced this bug.
    #[test]
    fn the_text_reaches_the_link_while_the_app_is_still_busy() {
        let mut ap = Autoplay::default();
        ap.start(vec![beat(0, Some("a"), 6), beat(1, Some("load"), 6)]);
        ap.tick(Duration::from_millis(16), false);
        ap.tick(Duration::from_secs(7), false); // finish beat 0, dispatch beat 1
        assert_eq!(ap.progress().0, 2, "precondition: the load beat is showing");

        // The compile starts. The clock is held...
        ap.tick(SCROLL_TRAVEL, true);
        // ...but the text has already arrived at the link that caused it.
        assert_eq!(
            ap.travel_t(),
            1.0,
            "the link must be on screen DURING its own compile, not after it",
        );

        // And the dwell genuinely was not charged.
        let before = ap.progress().0;
        for _ in 0..100 {
            ap.tick(Duration::from_millis(100), true);
        }
        assert_eq!(ap.progress().0, before, "a busy app must not burn the dwell");
        assert_eq!(ap.travel_t(), 1.0, "and the text stays on the link throughout");
    }

    /// The clock and the text are **different quantities**, and only one of them
    /// advances continuously.
    #[test]
    fn the_progress_bar_tracks_time_while_the_text_tracks_the_beat() {
        let mut ap = Autoplay::default();
        ap.start(vec![beat(0, None, 10), beat(1, None, 10)]);
        ap.tick(Duration::from_millis(16), false);
        ap.tick(SCROLL_TRAVEL, false);

        let f0 = ap.fraction();
        ap.tick(Duration::from_secs(4), false);

        assert!(ap.fraction() > f0, "the clock must keep running: {f0} -> {}", ap.fraction());
        assert_eq!(ap.travel_t(), 1.0, "the text must not");
    }

    /// **A beat's position is the byte offset of its link's line, exactly.**
    ///
    /// Two estimates preceded this and both failed. The first scrolled by the clock,
    /// so the text crept continuously. The second scrolled by beat *ordinal* -- Doug:
    /// *"the scroll is being advanced by a constant number of tour prose lines for
    /// each advancement"* -- which spaced beats evenly regardless of how much text
    /// lay between them.
    ///
    /// A character-offset *fraction* was the third, and it failed too, because
    /// rendered height per character is not constant: prose wraps in a narrow panel
    /// and a code block does not. **No constant corrects an estimate that is wrong in
    /// both directions**, so `app.rs` splits the markdown at this offset and measures
    /// the seam instead.
    ///
    /// **Checks the correspondence, not the arithmetic**: adjacent links stay close,
    /// a link after a wall of prose lands far below it, and the offsets index the
    /// text they claim to.
    #[test]
    fn a_beats_position_is_its_links_line_not_its_ordinal() {
        // Built by joining lines rather than as one long literal. The first version
        // used a `\` continuation, which left indentation in front of `[d]`, and the
        // test then failed on its own fixture rather than on the code.
        let mut lines: Vec<String> = vec![
            "# T".to_owned(),
            "intro".to_owned(),
            String::new(),
            "## Crowded".to_owned(),
            "[a](hrw://x/1)".to_owned(),
            "[b](hrw://x/2)".to_owned(),
            "[c](hrw://x/3)".to_owned(),
            "[d](hrw://x/4)".to_owned(),
            String::new(),
        ];
        for _ in 0..40 {
            lines.push("padding prose that occupies real document space".to_owned());
        }
        lines.push("## Far below".to_owned());
        lines.push("[e](hrw://x/5)".to_owned());
        let text = lines.join("\n");
        let stops = parse_stops(&text);
        let beats = schedule(&stops, Duration::from_secs(60), |_| false);

        let by_url = |u: &str| {
            beats
                .iter()
                .find(|b| b.link.as_deref() == Some(u))
                .unwrap_or_else(|| panic!("{u} missing from the run"))
        };

        // **The offset must actually index this link's line.** This is the property
        // `app.rs` relies on when it splits the string, and the one an ordinal or a
        // fraction cannot provide.
        for u in ["hrw://x/1", "hrw://x/4", "hrw://x/5"] {
            let b = by_url(u);
            assert!(
                text[b.byte_offset..].starts_with('['),
                "{u} must point at the start of its own line, got {:?}",
                &text[b.byte_offset..(b.byte_offset + 20).min(text.len())],
            );
            assert!(
                text[b.byte_offset..].contains(u),
                "and that line must contain {u}",
            );
        }

        // Four adjacent links are close; the one past 40 lines of filler is not.
        let (a, d, e) = (by_url("hrw://x/1"), by_url("hrw://x/4"), by_url("hrw://x/5"));
        let crowded = d.byte_offset - a.byte_offset;
        let gap = e.byte_offset - d.byte_offset;
        assert!(
            gap > crowded * 10,
            "the big gap in the document must be the big gap in the scroll              ({gap} bytes vs {crowded})",
        );

        // Positions must not go backwards; the walk reads top to bottom.
        for w in beats.windows(2) {
            assert!(
                w[1].byte_offset >= w[0].byte_offset,
                "beats must advance down the page: {} then {}",
                w[0].byte_offset,
                w[1].byte_offset,
            );
        }
    }

    /// **Losing focus pauses; regaining it resumes — but only its own pause.**
    ///
    /// The two pauses look identical in `phase()` and must not behave identically.
    /// If focus-return lifted a user pause, pressing Pause and then clicking any
    /// other window would silently restart the recording.
    #[test]
    fn focus_pauses_itself_without_undoing_a_user_pause() {
        let beats = vec![beat(0, None, 5)];
        let mut ap = Autoplay::default();

        // Focus loss pauses, focus return resumes.
        ap.start(beats.clone());
        ap.set_focused(false);
        assert_eq!(ap.phase(), Phase::Paused, "an external window pauses the walk");
        assert!(ap.tick(Duration::from_secs(10), false).is_none(), "and the clock is held");
        ap.set_focused(true);
        assert_eq!(ap.phase(), Phase::Playing, "coming back resumes it");

        // A user pause survives the same round trip.
        ap.start(beats);
        ap.pause();
        ap.set_focused(false);
        ap.set_focused(true);
        assert_eq!(
            ap.phase(),
            Phase::Paused,
            "clicking away and back must not undo a deliberate Pause",
        );
        ap.resume();
        assert_eq!(ap.phase(), Phase::Playing);
    }

    /// The whole of `dae-construction.md` schedules, and does so sanely.
    ///
    /// **Non-vacuity for the feature as a whole**: the tour this was built for must
    /// produce a run with real structure, not one giant beat or a hundred flashes.
    #[test]
    fn the_dae_tour_schedules_into_a_watchable_run() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/fixture-tours/dae-construction.md"
        );
        let text = std::fs::read_to_string(path).expect("the tour is versioned");
        let stops = parse_stops(&text);
        assert!(stops.len() >= 8, "preamble plus seven stops, got {}", stops.len());

        let beats = schedule(
            &stops,
            DEFAULT_TOTAL,
            |l| l.starts_with("hrw://notebook/") || l.starts_with("hrw://systemmodeler/"),
        );
        assert!(
            beats.len() >= 15,
            "about twenty links; a run of {} beats would be a slideshow",
            beats.len(),
        );
        assert_eq!(
            beats.iter().map(|b| b.dwell).sum::<Duration>(),
            DEFAULT_TOTAL,
        );
        // Every stop of the tour is represented; none is scheduled out.
        for i in 0..stops.len() {
            assert!(beats.iter().any(|b| b.stop == i), "stop {i} missing from the run");
        }
    }
}
