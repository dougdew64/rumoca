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
use std::sync::{Arc, mpsc};

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
    /// `live_done` starts **true** because that is the honest answer to *"is a
    /// live session still running?"* for frames that came out of a compile —
    /// **and for no other reason, which is a correction.** This doc used to say
    /// `live_debug_lifecycle` released a stray breakpoint on the strength of it.
    /// That function no longer exists, and the breakpoint-cleanup safety net it
    /// names was deliberately deleted (`docs/ideas.md` #74): releasing the anchor
    /// when nothing was in flight is what stopped the *next* Debug press
    /// working. What releases it now is on `App::live_breakpoint_armed` — a
    /// failed `start_live`, a specimen change, app exit — and **not one of those
    /// reads a [`LiveState`], so none of them can see this flag.**
    ///
    /// **Measured 2026-08-20, because the field still looks load-bearing:**
    /// flipping this to `false` fails exactly **one** test in the suite,
    /// [`tests::a_recorded_animation_reports_no_running_session`], which reads it
    /// directly. Every other reader arrives through [`Self::live_state`], which
    /// consults `live_finished()` only inside `if self.is_live()` — and a
    /// recorded playback has no `live_rx`, so the flag is unreachable from
    /// there. It is `true` because it is true, not because something depends
    /// on it.
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

    /// The cursor, **0-based**.
    ///
    /// The on-screen counter and `hrw://…/frame/<n>` links are **1-based** — this
    /// plus one. Anything publishing a frame position for a reader must add the one,
    /// or a tour link built from it lands a frame early.
    #[must_use]
    pub fn cursor(&self) -> usize {
        self.cursor
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
            if self.live_finished() {
                LiveState::Finished
            } else {
                LiveState::Running
            }
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
        assert!(
            !p.tick(0.6, LiveState::Idle),
            "stopped playback needs no repaint"
        );
    }

    #[test]
    fn a_busy_live_session_stops_playback_rather_than_pausing_it() {
        let mut p = recorded(5);
        *p.controls().playing = true;

        assert!(
            p.tick(0.1, LiveState::Running),
            "a live session still repaints"
        );
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
        assert!(
            !p.is_empty(),
            "a live session is not empty even before its first frame"
        );

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

    /// A recorded animation reports its session as finished.
    ///
    /// **This is the only test in the suite that can see `live_done`'s value for
    /// a recorded playback**, verified 2026-08-20 by flipping
    /// [`Playback::recorded`] to `false` and running the fast suite: this one
    /// failed and nothing else did. In particular the two view-level guards
    /// named `recorded_animation_reports_no_live_session`
    /// (`matching_anim`, `tarjan_anim`) **stayed green**, because they assert
    /// [`Playback::live_state`] and that returns `Idle` from `is_live()` being
    /// false, without ever consulting the flag.
    ///
    /// Those two were written as the must-fire guard for the 2026-07-27
    /// regression where `live_done` was `false` here, and **they could not have
    /// caught it** — a wrong negative of exactly the kind `CLAUDE.md` warns
    /// about, since believing them means not looking. They still assert
    /// something worth holding, one layer up; this holds the flag itself.
    #[test]
    fn a_recorded_animation_reports_no_running_session() {
        let p = recorded(2);
        assert!(!p.is_live());
        assert!(p.live_finished());
        assert!(!p.is_empty());
        assert!(Playback::<usize>::recorded(vec![], 0.5).is_empty());
    }
}

/// The no-nested-scroll rule, enforced over every view it applies to.
///
/// This lives here rather than beside any one view because the rule is about a
/// *family*: every member of the derived set today is a playback view, and this
/// module already owns what they share. A future non-animation entry would still
/// be checked — the set follows the wrapper, not the trait.
#[cfg(test)]
mod tests_layout {
    use std::path::{Path, PathBuf};

    /// The views `app.rs` draws inside a vertical scroll area of its own.
    ///
    /// **Derived from `app.rs`, never listed here**, and that is the point. A
    /// hand-written roster is a claim that outruns its evidence: the next view
    /// wrapped in a scroll area would simply be absent from it, and absence
    /// leaves no gap where the missing check was. The 2026-08-22 audit found
    /// that same shape three times in one day, one of them a test named
    /// `…shows_every_fixture…` that checked nine of twenty-two.
    ///
    /// The pairing is by name — `App::alias_anim_ui` owns `src/alias_anim.rs` —
    /// and a view whose file cannot be read **fails** rather than being skipped,
    /// because a rename is exactly how a member would leave the set unnoticed.
    ///
    /// Reading `app.rs` line by line rather than with a parser is sound here for
    /// one specific reason: `cargo fmt` runs before every gate, so a method of
    /// `impl App` opens at four spaces and closes with `}` at four spaces.
    pub(super) fn scroll_wrapped_views() -> Vec<(String, PathBuf)> {
        let hrw = Path::new(env!("CARGO_MANIFEST_DIR"));
        let app = std::fs::read_to_string(hrw.join("src/app.rs")).expect("app.rs must be readable");

        // Assembled rather than written, so this file does not contain the
        // strings it searches for. The first draft of the original check matched
        // its own explanation, four lines below the paragraph warning about it.
        let vertical = format!("{}::vertical()", "ScrollArea");

        let mut out = Vec::new();
        let mut current: Option<&str> = None;
        let mut wraps = false;

        for line in app.lines() {
            let opener = line
                .strip_prefix("    fn ")
                .or_else(|| line.strip_prefix("    pub fn "))
                .and_then(|rest| rest.split_once("_ui(").map(|(name, _)| name));

            if let Some(name) = opener {
                current = Some(name);
                wraps = false;
            } else if line == "    }" {
                if let Some(name) = current.take()
                    && wraps
                {
                    out.push((name.to_owned(), hrw.join(format!("src/{name}.rs"))));
                }
            } else if line.contains(vertical.as_str()) {
                wraps = true;
            }
        }
        out
    }

    /// **A view drawn inside a scrolling pane must not scroll or cap itself.**
    ///
    /// Doug, 2026-08-16: *"the connection sets lists are not using all available
    /// vertical space… showing only three connection sets per list."*
    ///
    /// `App::connection_anim_ui` already wrapped that whole view in a vertical
    /// scroll area, and three more were nested inside it, each with a magic
    /// height — 240pt for the lanes, 200pt for the frame's lists. A connection
    /// set costs a header plus a line per variable plus a line per equation, so
    /// 240pt is about three sets: the content overflowed a small box while the
    /// pane around it stayed empty, and the wheel scrolled the box instead of the
    /// page.
    ///
    /// **The nesting is the defect; the height cap only set how obvious it was.**
    /// The parent owns the scrolling and the height, and a child view just
    /// renders. A tall model then makes a tall pane, which is the honest result.
    ///
    /// # Why this replaced a per-file check
    ///
    /// The rule was enforced for **one** file, by `connection_anim`'s own test,
    /// and was never generalised — so the same defect sat in two other views for
    /// a week. This test found both on its first run: `alias_anim` capped at
    /// 320pt, holding ~16–18 rows against `Drivetrain`'s **77** alias
    /// eliminations, and `ic_plan_anim` capped at 300pt against `RcCircuit`'s 21
    /// blocks. `Drivetrain` is the index-reduction tour's centrepiece, so the
    /// tour's own specimen was showing under a quarter of its list.
    ///
    /// # What this checks, and what only Doug can
    ///
    /// It reads the source. It **cannot** tell whether a pane now looks right —
    /// `CLAUDE.md`: Claude verifies content, never pixels. Both scroll-area bugs
    /// in this project were reported by Doug and neither is visible to
    /// `egui_kittest`, since a clipped child is still in the accessibility tree.
    /// That half is his report.
    #[test]
    fn a_view_inside_a_scrolling_pane_does_not_scroll_or_cap_itself() {
        let views = scroll_wrapped_views();

        // Non-vacuity, and it names a specific member for a reason: if the scan
        // ever stops matching, an empty set would pass while checking nothing —
        // the silent-vacuity failure this protocol exists to avoid. The
        // connections view is the one the rule was born on.
        assert!(
            views.iter().any(|(name, _)| name == "connection_anim"),
            "the scan found no wrapped view named `connection_anim`, so it has \
             stopped reading app.rs correctly and is checking nothing. Found: {:?}",
            views.iter().map(|(n, _)| n).collect::<Vec<_>>(),
        );

        let scroll = format!("{}::", "ScrollArea");
        let cap = format!("{}(", "max_height");

        let mut offenders = Vec::new();
        for (name, path) in &views {
            let src = std::fs::read_to_string(path).unwrap_or_else(|e| {
                panic!(
                    "app.rs wraps `{name}` but {} cannot be read: {e}",
                    path.display()
                )
            });
            if src.contains(scroll.as_str()) {
                offenders.push(format!(
                    "{name} constructs a scroll area; the parent already scrolls, so a \
                     nested one caps the content and eats the mouse wheel"
                ));
            }
            if src.contains(cap.as_str()) {
                offenders.push(format!(
                    "{name} sets a fixed height; the pane's height is the parent's to \
                     decide, and a magic cap is what limited each lane to three sets"
                ));
            }
        }

        assert!(
            offenders.is_empty(),
            "{} of {} scroll-wrapped views nest their own scrolling. The fix is \
             subtractive — delete the inner scroll area and its height cap:\n  {}",
            offenders.len(),
            views.len(),
            offenders.join("\n  "),
        );
    }
}

/// The [`Animated`] contract, held across every implementor at once.
///
/// From a column read of the eight implementations on 2026-08-23 — reading a
/// list of siblings down a column and looking for the member shaped differently.
/// **The column read found no stranded member**: both dispatchers in `app.rs`
/// cover all eight, and the two views whose `live_state` diverges diverge
/// *correctly*. What it found is that nothing held either fact in place.
#[cfg(test)]
mod tests_animated_contract {
    use std::collections::BTreeSet;
    use std::path::Path;

    fn hrw() -> &'static Path {
        Path::new(env!("CARGO_MANIFEST_DIR"))
    }

    /// One animated view, as the source describes it.
    ///
    /// A struct rather than a tuple because three strings positionally is
    /// exactly the shape that gets transposed silently — the same reasoning as
    /// [`super::PlaybackControls`].
    struct AnimatedView {
        /// Module name, which is also the file stem: `alias_anim`.
        module: String,
        /// The whole file, for questions about what the view renders.
        source: String,
        /// Just the `impl Animated for …` block, for questions about the trait.
        impl_body: String,
    }

    /// Each `impl Animated for` in `src/`.
    ///
    /// Discovered rather than listed, for the reason `tests_layout` gives: a
    /// ninth implementor must arrive already covered, not wait to be added to a
    /// roster nobody remembers.
    fn animated_views() -> Vec<AnimatedView> {
        let mut out = Vec::new();
        let dir = hrw().join("src");
        for entry in std::fs::read_dir(&dir)
            .expect("src/ must be readable")
            .flatten()
        {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("a source file must be readable");
            let Some(start) = text.find("\nimpl Animated for ") else {
                continue;
            };
            // `impl` blocks are at column 0, so `\n}\n` closes this one.
            let rest = &text[start + 1..];
            let end = rest.find("\n}\n").map_or(rest.len(), |i| i + 2);
            let module = path
                .file_stem()
                .expect("a .rs file has a stem")
                .to_string_lossy()
                .into_owned();
            out.push(AnimatedView {
                module,
                impl_body: rest[..end].to_owned(),
                source: text,
            });
        }
        out.sort_by(|a, b| a.module.cmp(&b.module));
        out
    }

    /// The views `app.rs` gives a live-debug entry point, read from the call
    /// sites that pair a `PendingLiveDebug` variant with the view's cache field.
    ///
    /// **Read from the pairing rather than from the variant names**, which do not
    /// map cleanly onto module names — `Connections` drives `connection_anim` and
    /// `PreLowering` drives `pre_lowering_anim`, so any name-derived mapping would
    /// need a hand-written exception table, and a hand-written table is the thing
    /// this file keeps refusing to write.
    fn live_debug_gated_views() -> BTreeSet<String> {
        let app =
            std::fs::read_to_string(hrw().join("src/app.rs")).expect("app.rs must be readable");
        let lines: Vec<&str> = app.lines().collect();
        let needle = format!("live_debug_gate(ui.ctx(), {}::", "PendingLiveDebug");

        let mut out = BTreeSet::new();
        for (i, line) in lines.iter().enumerate() {
            if !line.contains(needle.as_str()) {
                continue;
            }
            // The closure body naming the cache field follows within a line or
            // two, depending on how rustfmt wrapped the call.
            let field = lines[i..(i + 4).min(lines.len())]
                .iter()
                .find_map(|l| l.trim().rsplit('.').next().filter(|t| t.ends_with("_anim")));
            if let Some(field) = field {
                out.insert(field.to_owned());
            }
        }
        out
    }

    /// **A view reports a live session if and only if it can have one.**
    ///
    /// Six of the eight delegate `live_state` to [`super::Playback`]; `alias_anim`
    /// and `ic_plan_anim` return `Idle` unconditionally and ignore `arming`. That
    /// is **correct today** and is not a defect: those two phases have no search
    /// to trace, no Debug button and no `PendingLiveDebug` variant, and a view
    /// with no live path that delegated would report `Arming` whenever a session
    /// was starting on some *other* stage, disabling controls for no reason.
    ///
    /// # Why it still needs a guard
    ///
    /// **`connection_anim` carried exactly this stub, and it became a real bug**
    /// the day a live path was added: its comment records that the hardcoded
    /// `Idle` "outlived the reason for it", so during a genuine session the
    /// playback controls stayed enabled while the debugger owned the cursor, and
    /// `Finished` never arrived to re-enable them afterwards.
    ///
    /// Nothing would have failed. The stub is *invisible* to the compiler, the
    /// view still renders, and the trait is still implemented — which is why this
    /// is the shape the run log calls a claim outrunning its evidence: a comment
    /// saying "never runs live" with no mechanism holding it true.
    ///
    /// So the invariant is checked in **both** directions. Adding a live path to
    /// `alias_anim` without changing its `live_state` fails here by name, and so
    /// does removing one while leaving a view delegating.
    #[test]
    fn only_the_views_with_a_live_path_report_a_live_session() {
        let views = animated_views();
        let gated = live_debug_gated_views();

        // Non-vacuity, both halves. An empty gated set would let the equality
        // pass against an empty delegating set while proving nothing, and an
        // implementor set that no longer contains a hardcoding view would mean
        // the interesting branch had stopped being exercised.
        assert!(
            !gated.is_empty(),
            "no live-debug gate call sites were found in app.rs, so this check is \
             reading nothing"
        );
        assert!(
            views.len() > gated.len(),
            "every Animated implementor has a live path, so the hardcoded-Idle \
             branch this guards is no longer exercised: {} views, {} gated",
            views.len(),
            gated.len(),
        );

        let delegates = format!("self.playback.{}(", "live_state");
        let delegating: BTreeSet<String> = views
            .iter()
            .filter(|v| v.impl_body.contains(delegates.as_str()))
            .map(|v| v.module.clone())
            .collect();

        assert_eq!(
            delegating, gated,
            "\nthe views that report a live session and the views that can HAVE one \
             have diverged.\n  delegating to Playback: {delegating:?}\n  given a live \
             path by app.rs: {gated:?}\n\nA view with a live path must delegate, or it \
             reports `no session` during a real one — connection_anim's own comment \
             records that happening. A view without one must not, or it reports \
             `Arming` whenever a session starts on another stage.",
        );
    }

    /// **Every animated view renders an opening frame.**
    ///
    /// Doug, 2026-08-23, on finding two views that did not: *"if some animation
    /// panes open with no attempt yet made to begin their algorithms, but two
    /// animation panes begin after progress has been made, then that is an
    /// inconsistency which causes learning friction."* The eight now agree —
    /// frame 0 is the system as the algorithm was handed it.
    ///
    /// # Why this exists when eight per-view tests already pass
    ///
    /// **They are eight separate claims, and a ninth view makes none of them
    /// false.** That is exactly how `alias_anim` and `ic_plan_anim` opened on a
    /// completed step from the day they were written while `connection_anim`'s
    /// own rule was enforced for one file — the same shape, twice in one night,
    /// which is what made a family-level check worth building.
    ///
    /// Each view's step enum is read out of the view's own source rather than
    /// listed here, so a view added tomorrow is checked tomorrow.
    ///
    /// # What this does NOT check, and it is the larger half
    ///
    /// It proves each view **renders** an opening frame, not that its trace
    /// **emits** one first. Those are different claims and the second is the one
    /// that matters on screen; it is held per view, where the frames are
    /// produced — `matching.rs` and `tarjan.rs`'s
    /// `trace_starts_before_the_…` tests in the Rumoca crate, and
    /// `alias_anim`/`ic_plan_anim`'s `…opens_before_anything…` tests here.
    ///
    /// So this is the cheap net under those, catching the case none of them
    /// can: a **new** view with no opening frame at all. Stating that limit is
    /// the point — `CLAUDE.md`'s most frequent failure is a rule read wider
    /// than the mechanism under it.
    #[test]
    fn every_animated_view_renders_an_opening_frame() {
        let views = animated_views();
        assert!(
            views.len() >= 8,
            "eight animated views existed on 2026-08-23; finding fewer means the \
             scan broke rather than that views were deleted: {:?}",
            views.iter().map(|v| &v.module).collect::<Vec<_>>(),
        );

        // Assembled, because this file is not scanned but the habit is what
        // stops the fourth self-match — see `tests_layout`.
        let opening = format!("::{}", "Start");

        let mut missing = Vec::new();
        for view in &views {
            // The step enums a view names, e.g. `MatchingStep::` — a view may
            // reference more than one (`tarjan_anim` also drives matching), so
            // ONE of them carrying an opening frame is the honest bar.
            assert!(
                view.source.contains("Step::"),
                "{} implements Animated but names no step enum, so this check \
                 cannot see what it renders",
                view.module,
            );
            if !view.source.contains(opening.as_str()) {
                missing.push(view.module.clone());
            }
        }

        assert!(
            missing.is_empty(),
            "these animated views render no opening frame: {missing:?}\n\n\
             Every replay opens on the system as the algorithm was handed it — \
             nothing done and nothing attempted. A view whose first frame is \
             already a completed step teaches that the algorithm had started \
             before the reader arrived. Add a `Start` step to its trace (see \
             `MatchingStep::Start`) or, for a view built from a report list, wrap \
             its payload the way `AliasStep` does.",
        );
    }

    /// **Every animated view is accounted for by the scroll rule — wrapped, or a
    /// canvas.**
    ///
    /// `tests_layout` derives the views `app.rs` wraps in a scroll area and checks
    /// those. **What it never said is what the other views are.** A view drawn with
    /// no scroll parent is outside the rule *and* outside the check, and today that
    /// is correct — `matching_anim` and `tarjan_anim` paint onto a [`crate::canvas::Canvas`],
    /// which pans and zooms rather than scrolling, so "the parent owns the
    /// scrolling" does not apply to them.
    ///
    /// **Correct but unstated is the dangerous shape**, and this repository has the
    /// scar: the no-nested-scroll rule was enforced for one file while two others
    /// carried the same defect, because nothing said which files the rule covered.
    /// A view that lost its scroll parent would drop out of `tests_layout`'s roster
    /// **silently** — no failure, just one less thing checked.
    ///
    /// So both sides are derived and the partition must be exact: wrapped from
    /// `app.rs`'s call sites, canvas from the view's own use of the type. Neither is
    /// a list. If a view is ever in both or in neither, this fails naming it and the
    /// question — is it a canvas view now, or did it lose its parent? — has to be
    /// answered rather than assumed.
    #[test]
    fn every_animated_view_is_accounted_for_by_the_scroll_rule() {
        let views = animated_views();
        let animated: BTreeSet<&String> = views.iter().map(|v| &v.module).collect();
        let wrapped: BTreeSet<String> = super::tests_layout::scroll_wrapped_views()
            .into_iter()
            .map(|(module, _)| module)
            .collect();
        let canvas: BTreeSet<&String> = views
            .iter()
            .filter(|v| v.source.contains("canvas::Canvas"))
            .map(|v| &v.module)
            .collect();

        assert!(
            !wrapped.is_empty() && !canvas.is_empty(),
            "both sides must be found"
        );

        let unaccounted: Vec<&&String> = animated
            .iter()
            .filter(|m| !wrapped.contains(**m) && !canvas.contains(**m))
            .collect();
        assert!(
            unaccounted.is_empty(),
            "these animated views are neither wrapped in a scroll area by app.rs nor \
             painted on a Canvas: {unaccounted:?}\n\n\
             Such a view is outside the no-nested-scroll rule AND outside the check \
             that enforces it — the silent gap that let alias_anim and ic_plan_anim \
             carry the same defect for a week. Either give it a scroll parent, or say \
             here why it needs neither.",
        );

        let both: Vec<&&String> = animated
            .iter()
            .filter(|m| wrapped.contains(**m) && canvas.contains(**m))
            .collect();
        assert!(
            both.is_empty(),
            "these views both pan/zoom on a Canvas and sit inside a scroll area: \
             {both:?}. That nests two different ways of moving the same content, \
             which is the wheel-capture bug in a new dress.",
        );
    }

    /// **Every animation's capture names the step on screen.**
    ///
    /// [`Animated::current_frame_context`] exists so a question asked mid-animation
    /// says *what* the reader was looking at, not just where the cursor was. The
    /// capture renders it as `view.animation.showing`, and `bridge.rs` reads
    /// `showing["step"]` — so a view that omits the key produces a capture that is
    /// well-formed, present, and says nothing.
    ///
    /// That is the failure this repository treats as worst: not an error, but a
    /// **silent absence** in the one artifact Claude uses to answer a question about
    /// a frame he cannot see.
    ///
    /// # Scope, which is narrower than the name
    ///
    /// This reads the source: it proves each view's `impl` **writes** the key, not
    /// that **every branch** does. Two views build their context in branches —
    /// `alias_anim` and `ic_plan_anim`, whose opening frame carries no substitution
    /// or block — and those are covered behaviourally where they live, by
    /// `alias_anim::tests::every_frame_describes_itself` and its `ic_plan_anim`
    /// twin. The other six build one expression, which has no branch to miss.
    #[test]
    fn every_animation_context_names_the_step_on_screen() {
        let views = animated_views();
        assert!(!views.is_empty(), "no Animated implementors found");

        // Assembled, for the reason `tests_layout` gives.
        let key = format!("\"{}\"", "step");

        let missing: Vec<&String> = views
            .iter()
            .filter(|v| !v.impl_body.contains(key.as_str()))
            .map(|v| &v.module)
            .collect();

        assert!(
            missing.is_empty(),
            "these animations build a frame context with no `step` key: {missing:?}\n\n\
             `bridge.rs` reads `showing[\"step\"]` to say what the reader was looking \
             at. Without it the capture is present and empty, which is worse than \
             absent — it answers the question with nothing.",
        );
    }

    /// **`which()` values are matched against, so a collision misroutes silently.**
    ///
    /// They reach the capture (`app.rs` builds `AnimationView { which }`) and the
    /// crash log, where a duplicate would attribute one view's frame position to
    /// another — wrong, plausible, and invisible.
    #[test]
    fn every_animation_identifies_itself_uniquely() {
        let views = animated_views();
        let marker = "fn which(&self) -> &'static str {";

        let mut seen: Vec<(String, String)> = Vec::new();
        for view in &views {
            let (module, body) = (&view.module, &view.impl_body);
            let literal = body
                .split_once(marker)
                .and_then(|(_, rest)| rest.lines().nth(1))
                .map(str::trim)
                .and_then(|l| l.strip_prefix('"')?.strip_suffix('"'))
                .unwrap_or_else(|| {
                    panic!("{module} implements Animated but its `which` body was not readable")
                });
            seen.push((literal.to_owned(), module.clone()));
        }

        let unique: BTreeSet<&String> = seen.iter().map(|(w, _)| w).collect();
        assert_eq!(
            unique.len(),
            seen.len(),
            "two animations share a `which()` value, so one misroutes into the \
             other's capture and crash-log rows: {seen:?}",
        );
    }
}
