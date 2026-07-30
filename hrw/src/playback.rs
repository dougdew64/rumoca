//! Frame playback shared by every animated algorithm view.
//!
//! ## What this replaces
//!
//! `MatchingAnimation`, `TarjanAnimation` and `ReductionAnimation` each carried
//! the *same seven fields* — `frames`, `cursor`, `playing`, `interval`,
//! `elapsed`, `live_rx`, `live_done` — differing only in the frame type. On top
//! of them sat five methods (`position`, `is_live`, `live_finished`,
//! `live_state`, `is_empty`) that were **byte-identical** in all three files,
//! plus ~30 lines of identical timing prologue at the top of each `ui()`.
//! `ReductionAnimation` was those seven fields and nothing else.
//!
//! The duplication was logged and *deliberately deferred* — Phase 7 reworks the
//! animation views, and de-duplicating early risks churn. What changed the
//! calculus is `docs/ideas.md` #40, which adds a **fourth** animated view
//! (`pre()` lowering) before Phase 7 arrives. Copying the pattern a fourth time
//! would leave Phase 7 four near-duplicates instead of three, so the debt was
//! paid first — and the new view is built on this rather than beside it.
//!
//! ## Why a generic struct rather than a trait
//!
//! A trait would have shared the *behaviour* and left the seven fields declared
//! three times, so the state could still drift. Composition shares both: an
//! animation owns a `Playback<T>` and adds only what is genuinely its own —
//! `MatchingAnimation` keeps `n_eq`/`n_var`/names/rows, `ReductionAnimation`
//! keeps nothing at all. [`Animated`] is the small trait *on top*,
//! for the parts that cannot be generic (what the current frame *means*).
//!
//! ## Live sessions
//!
//! An animation is in one of two modes, and the same type covers both:
//!
//! - **Recorded** — frames computed during compilation, played back with
//!   play/pause/step and a timed auto-advance.
//! - **Live** — an algorithm thread pushes frames through an `mpsc` channel as a
//!   debugger steps it. The cursor follows the newest frame, timed playback is
//!   suppressed (the debugger owns the cursor), and `live_done` reports
//!   completion without taking a lock, so LLDB's post-step variable evaluation
//!   never contends with the producer.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};

use serde_json::Value;

use crate::LiveState;

/// What every animated algorithm view can say about itself.
///
/// Named `Animated` rather than `AnimationView` so it cannot be mistaken for
/// [`crate::bridge::AnimationView`], the *emitted* shape it feeds — both appear
/// in `app.rs`.
///
/// Three of these delegate straight to [`Playback`]; the fourth cannot, because
/// only the view knows what its current frame *means*. That is the whole reason
/// this is a trait sitting on top of a generic struct rather than one or the
/// other: state and timing are genuinely identical across the views, and
/// interpretation genuinely is not.
///
/// **`current_frame_context` is why this trait exists now rather than later.**
/// The capture's `view.animation` used to emit position only — `which`,
/// `frame`, `frame_count`, `live_state` — so a question asked while paused on
/// frame 12 told Claude *where* the user was but not *what they were looking
/// at*, since frames live only in memory. Each view already computes a
/// human-readable description of the current step in order to draw it; this
/// hands that same description to the capture instead of re-deriving it, so the
/// screen and the emitted context cannot disagree.
pub trait Animated {
    /// Stable identifier for the algorithm: `"matching"`, `"tarjan"`,
    /// `"reduction"`. Matched against, so it does not change with display text.
    fn which(&self) -> &'static str;

    /// `(cursor, frame count)`.
    fn position(&self) -> (usize, usize);

    fn live_state(&self, arming: bool) -> LiveState;

    /// What the frame under the cursor shows. `None` before the first frame of
    /// a live session has arrived — a real state, not a failure.
    fn current_frame_context(&self) -> Option<Value>;

    /// Jump to frame `n`, for `hrw://…/frame/<n>`. `false` when the frame does not
    /// exist, so the caller can say so instead of landing somewhere plausible.
    ///
    /// Takes `&mut self`, which is why the app looks animations up mutably to seek and
    /// immutably to report position — the capture must never move what it describes.
    fn seek(&mut self, n: usize) -> bool;
}

/// Cursor, timing and live-session state for a sequence of algorithm frames.
///
/// `T` is the frame type — `MatchingFrame`, `TarjanFrame`,
/// `IndexReductionFrame`, and whatever #40 adds.
pub struct Playback<T> {
    frames: Vec<T>,
    cursor: usize,
    playing: bool,
    /// Seconds between auto-advance frames.
    interval: f64,
    elapsed: f64,
    /// Receiver end of the live trace channel. The animation drains new frames
    /// from it without contending on the producer's lock.
    live_rx: Option<mpsc::Receiver<T>>,
    /// Whether the live algorithm thread has finished. Atomic rather than
    /// mutex-guarded so a debugger evaluating variables after each step cannot
    /// block the producer.
    live_done: Arc<AtomicBool>,
}

impl<T> Playback<T> {
    /// Playback over frames already computed during compilation.
    ///
    /// `live_done` starts **true**: a recorded animation has no live session
    /// running, and `live_debug_lifecycle` uses that to release a breakpoint
    /// left armed by a session that never started.
    pub fn recorded(frames: Vec<T>, interval: f64) -> Self {
        Self {
            frames,
            cursor: 0,
            playing: false,
            interval,
            elapsed: 0.0,
            live_rx: None,
            live_done: Arc::new(AtomicBool::new(true)),
        }
    }

    /// Playback fed by a live algorithm thread. Starts empty — frames arrive as
    /// the debugger advances the producer.
    pub fn live(rx: mpsc::Receiver<T>, done: Arc<AtomicBool>, interval: f64) -> Self {
        Self {
            frames: Vec::new(),
            cursor: 0,
            playing: false,
            interval,
            elapsed: 0.0,
            live_rx: Some(rx),
            live_done: done,
        }
    }

    /// Where playback stands: `(cursor, frame count)`.
    pub fn position(&self) -> (usize, usize) {
        (self.cursor, self.frames.len())
    }

    /// Jump the cursor to frame `n`, pausing playback. Returns `false` — and changes
    /// nothing — when `n` is past the end.
    ///
    /// **Refuses rather than clamps**, the same rule as camera aiming: a tour naming a
    /// frame this trace does not have is a bug *in the tour*, and landing on the last
    /// frame instead would look deliberate and hide it.
    ///
    /// **Pauses**, because a link that seeks into a running animation would be
    /// overtaken by the next tick before the reader's eyes arrived. A stop that says
    /// "watch this moment" has to hold still on it.
    pub fn seek(&mut self, n: usize) -> bool {
        if n >= self.frames.len() {
            return false;
        }
        self.cursor = n;
        self.playing = false;
        self.elapsed = 0.0;
        true
    }

    pub fn frames(&self) -> &[T] {
        &self.frames
    }

    /// The frame under the cursor, or `None` before any frame has arrived.
    pub fn current(&self) -> Option<&T> {
        self.frames.get(self.cursor)
    }

    pub fn is_live(&self) -> bool {
        self.live_rx.is_some()
    }

    pub fn live_finished(&self) -> bool {
        self.live_done.load(Ordering::Acquire)
    }

    /// Nothing to show: no recorded frames and no live session to produce any.
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty() && self.live_rx.is_none()
    }

    /// Map the raw live-session flags onto the shared [`LiveState`].
    ///
    /// `arming` comes from the app: the Debug button's breakpoint handshake
    /// takes several frames, and throughout them this view is still showing the
    /// *recorded* animation — so the animation alone cannot tell that a session
    /// is starting, and its controls would stay enabled after the click.
    pub fn live_state(&self, arming: bool) -> LiveState {
        if self.is_live() {
            if self.live_finished() { LiveState::Finished } else { LiveState::Running }
        } else if arming {
            LiveState::Arming
        } else {
            LiveState::Idle
        }
    }

    /// Drain any frames the live producer has pushed since the last frame.
    ///
    /// The cursor jumps to the newest arrival, which is what makes a debugger
    /// step visibly advance the animation.
    pub fn sync_live(&mut self) {
        if let Some(rx) = &self.live_rx {
            let before = self.frames.len();
            self.frames.extend(rx.try_iter());
            if self.frames.len() > before {
                self.cursor = self.frames.len().saturating_sub(1);
            }
        }
    }

    /// Advance timed playback by `dt`. Returns whether a repaint is needed.
    ///
    /// A live session **takes the cursor**: timed playback is stopped rather
    /// than paused, so it cannot resume when the session finishes and re-enables
    /// the controls. A busy session still requests repaints, because frames
    /// arrive from another thread and nothing else would wake the UI.
    pub fn tick(&mut self, dt: f64, live: LiveState) -> bool {
        if live.is_busy() {
            self.playing = false;
            return true;
        }
        if !self.playing {
            return false;
        }
        self.elapsed += dt;
        if self.elapsed >= self.interval {
            self.elapsed = 0.0;
            if self.cursor + 1 < self.frames.len() {
                self.cursor += 1;
            } else {
                self.playing = false;
            }
        }
        true
    }

    /// Mutable access for [`crate::animation_controls`], which owns the
    /// play/pause/step/scrub widgets. Deliberately one accessor rather than four
    /// — the previous signature took `cursor`, `playing`, `elapsed` and
    /// `interval` as separate `&mut` arguments, two of them adjacent bools, so
    /// transposing a pair compiled silently.
    pub fn controls(&mut self) -> PlaybackControls<'_> {
        PlaybackControls {
            cursor: &mut self.cursor,
            playing: &mut self.playing,
            elapsed: &mut self.elapsed,
            interval: &mut self.interval,
            n_frames: self.frames.len(),
        }
    }
}

/// The mutable slice of [`Playback`] the control widgets need.
///
/// Borrowed as a bundle so the four values arrive together and named, rather
/// than as four positional arguments of which two were interchangeable `&mut
/// bool`s. Same reasoning as `TreeActions` and `TreeOptions`.
pub struct PlaybackControls<'a> {
    pub cursor: &'a mut usize,
    pub playing: &'a mut bool,
    pub elapsed: &'a mut f64,
    pub interval: &'a mut f64,
    pub n_frames: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Seeking lands on the frame, pauses, and refuses to go past the end.
    ///
    /// All three matter for a tour stop. **Landing** is the point. **Pausing** is what
    /// makes it hold still — a link that seeks into a running animation would be
    /// overtaken by the next tick before the reader's eyes arrived. **Refusing** keeps a
    /// tour bug visible: clamping to the last frame would look deliberate.
    #[test]
    fn seeking_lands_pauses_and_refuses_to_overshoot() {
        let mut p = Playback::recorded(vec![10, 20, 30, 40], 0.5);
        assert!(p.seek(2), "frame 2 exists");
        assert_eq!(p.position(), (2, 4));

        assert!(p.seek(0), "the first frame is seekable");
        assert_eq!(p.position(), (0, 4));

        assert!(p.seek(3), "the last frame is seekable");
        assert!(!p.seek(4), "one past the end is refused");
        assert!(!p.seek(999));
        assert_eq!(p.position(), (3, 4), "a refused seek changes nothing");
    }

    /// An empty replay accepts no seek at all, rather than panicking on frame 0.
    ///
    /// Live sessions start empty, which is exactly when a stray link is most likely.
    #[test]
    fn seeking_an_empty_replay_is_refused() {
        let mut p: Playback<u8> = Playback::recorded(Vec::new(), 0.5);
        assert!(!p.seek(0));
        assert_eq!(p.position(), (0, 0));
    }


    fn recorded(n: usize) -> Playback<usize> {
        Playback::recorded((0..n).collect(), 0.5)
    }

    #[test]
    fn recorded_playback_advances_then_stops_at_the_end() {
        let mut p = recorded(3);
        *p.controls().playing = true;

        assert!(p.tick(0.6, LiveState::Idle), "playing requests repaints");
        assert_eq!(p.position(), (1, 3));
        p.tick(0.6, LiveState::Idle);
        assert_eq!(p.position(), (2, 3));

        // At the last frame it stops rather than wrapping — playback is a
        // replay of an algorithm, and looping would suggest the algorithm did.
        p.tick(0.6, LiveState::Idle);
        assert_eq!(p.position(), (2, 3));
        assert!(!p.tick(0.6, LiveState::Idle), "stopped playback needs no repaint");
    }

    #[test]
    fn a_busy_live_session_stops_playback_rather_than_pausing_it() {
        let mut p = recorded(5);
        *p.controls().playing = true;

        assert!(p.tick(0.1, LiveState::Running), "a live session still repaints");
        assert_eq!(p.position(), (0, 5), "the debugger owns the cursor");

        // Stopped, not paused: when the session finishes and the controls come
        // back, playback must not resume on its own.
        assert!(!p.tick(0.6, LiveState::Idle));
        assert_eq!(p.position(), (0, 5));
    }

    #[test]
    fn live_frames_pull_the_cursor_to_the_newest_arrival() {
        let (tx, rx) = mpsc::channel();
        let done = Arc::new(AtomicBool::new(false));
        let mut p = Playback::live(rx, Arc::clone(&done), 0.5);

        assert_eq!(p.live_state(false), LiveState::Running);
        assert!(!p.is_empty(), "a live session is not empty even before its first frame");

        tx.send(10).unwrap();
        tx.send(20).unwrap();
        p.sync_live();
        assert_eq!(p.position(), (1, 2), "the cursor follows the newest frame");
        assert_eq!(p.current(), Some(&20));

        done.store(true, Ordering::Release);
        assert_eq!(p.live_state(false), LiveState::Finished);
    }

    /// `Arming` is distinct from `Running`: the breakpoint handshake takes
    /// several frames, during which the *recorded* animation is still showing.
    #[test]
    fn arming_is_reported_before_a_session_exists() {
        let p = recorded(2);
        assert_eq!(p.live_state(false), LiveState::Idle);
        assert_eq!(p.live_state(true), LiveState::Arming);
    }

    /// A recorded animation reports its session as finished, which is what lets
    /// the lifecycle release a breakpoint armed for a session that never began.
    #[test]
    fn a_recorded_animation_reports_no_running_session() {
        let p = recorded(2);
        assert!(!p.is_live());
        assert!(p.live_finished());
        assert!(!p.is_empty());
        assert!(Playback::<usize>::recorded(vec![], 0.5).is_empty());
    }
}
