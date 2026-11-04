//! **Capture and inspect** — HRW photographs its own window, Claude reads the PNG.
//!
//! # The problem this closes
//!
//! Roughly half of HRW is painter-drawn: the incidence matrix, the spy plot, the
//! Tarjan graph, and the four algorithm animations are shapes on a canvas, not
//! widgets in an accessibility tree. `Cargo.toml` records the consequence where
//! `egui_kittest` is declared — *"the painter-drawn views are invisible to an
//! accessibility-tree harness, so they remain the fixture tours' job."* The fixture
//! tours are Doug walking them, which makes **his attention the instrument** for
//! exactly the part of HRW that the algorithms live in.
//!
//! The ledger in `docs/tech-debt.md` says what that has cost. Of Doug's ten findings
//! in the 08-16 section, nine were *things visibly wrong on screen*: a tab row drawing
//! five of eight tabs, a banner contradicting its own payload, an animation that never
//! drained its channel. Each one was found by looking, and could only be found by
//! looking — and each one sat in the tree, green, until he ran the app.
//!
//! This module lets Claude do the looking. A request names what to navigate to; HRW
//! dispatches the links, settles, photographs the window, and writes a PNG plus a
//! result file saying what was on screen. Claude reads the PNG in the conversation
//! that asked for it.
//!
//! # What it deliberately is not
//!
//! **Not image-snapshot testing.** Nothing is committed, no reference image exists,
//! and no test asserts on a pixel — the 2026-08-01 decision against golden images
//! stands, and for its original reasons (pixels are brittle, and a GPU in the test
//! path is a liability). The difference is the consumer: a golden-image test compares
//! today's pixels against yesterday's and fails on a font update; this hands one
//! photograph to a reader who can say *"the Matching tab is there but greyed"*.
//!
//! **Not a second way to drive the UI.** Every navigation goes through
//! `App::dispatch_hrw_link`, the function a clicked `hrw://` link goes through, one
//! link per settle interval exactly as autoplay dispatches a beat. A capture that
//! navigated by reaching into the app's fields could photograph a state no user
//! gesture can reach, and would then testify about it — which is worse than no
//! photograph, because it looks like evidence.
//!
//! # The protocol
//!
//! Lives in [`crate::bridge`] with the other three channels:
//! [`bridge::SCREENSHOT_REQUEST_FILE`] in, [`bridge::SCREENSHOT_RESULT_FILE`] and a
//! PNG under [`bridge::SCREENSHOTS_DIR`] out. This module is the clock and the state
//! machine between them.
//!
//! # Why the failure path is the designed part
//!
//! A shot has five ways to produce no image — a malformed request, a link that does
//! not parse, a compile that never finishes, a backend that never delivers the
//! pixels, a failed write — and in all five **the window still looks fine**. So the
//! one unacceptable outcome is silence: Claude would find the *previous* request's PNG
//! still on disk, read it as this request's answer, and testify about a screen that
//! was never photographed.
//!
//! Hence [`ShotStep::Failed`] is a single arm carrying everything a result file needs,
//! so every failure leaves the same trail and a new failure kind cannot forget to
//! report itself. `every_failure_answers_the_request` is the test that holds it.

use std::collections::VecDeque;
use std::time::{Duration, Instant, SystemTime};

use eframe::egui;

use crate::bridge::{self, ScreenshotRequest};

/// How long the backend gets to deliver the pixels after the command is sent.
///
/// The wgpu path is asynchronous — the capture buffer is read back on a later frame
/// and the event arrives after that — so this cannot be "next frame". Five seconds is
/// thousands of frames at the repaint rate a pending shot forces, and short enough
/// that a backend which never answers is reported inside one conversational turn
/// rather than looking like a hang.
const DELIVERY_DEADLINE: Duration = Duration::from_secs(5);

/// How long the whole request gets, from pickup to photograph.
///
/// Covers the case `busy` never clears: a compile that wedges, or a specimen whose
/// link loaded nothing. Without it a pending shot would hold the app at full repaint
/// forever and answer nothing — the silence this module exists to prevent, arriving
/// by a different route. Sized for the slowest specimen compile (the MSL-heavy ones
/// run tens of seconds) with room to spare.
const OVERALL_DEADLINE: Duration = Duration::from_secs(120);

/// What the app should do about screenshots this frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ShotStep {
    /// Nothing to do — the overwhelmingly common case.
    Idle,
    /// Dispatch this `hrw://` link through `dispatch_hrw_link`, then settle again.
    Follow(String),
    /// Send `ViewportCommand::Screenshot`, tagged with this `seq`.
    Shoot(u64),
    /// Answer this request with an error and **no image**.
    ///
    /// One arm for all five failure kinds, deliberately: the request's author is a
    /// process that will go looking for a PNG, and a failure that wrote nothing is
    /// indistinguishable from one HRW never saw. Carrying the request's own fields
    /// means the error result is as complete as a successful one.
    Failed {
        /// `None` only for a request so malformed no `seq` could be read from it.
        seq: Option<u64>,
        why: Option<String>,
        followed: Vec<String>,
        /// See `ShotRecord::notice_floor`. `u64::MAX` for a failure with no request
        /// behind it, so a standing notice can never be attributed to one.
        notice_floor: u64,
        error: String,
    },
}

/// What a delivered shot knows about the request that asked for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ShotRecord {
    pub(crate) seq: u64,
    pub(crate) why: Option<String>,
    /// The links actually dispatched, in order. Shorter than the request's list when
    /// one of them failed — which is why this is recorded rather than re-read from
    /// the request.
    pub(crate) followed: Vec<String>,
    /// The region the request asked for. Carried here rather than re-read from the
    /// file, because by the time the pixels arrive the request may have been
    /// rewritten — the image answers the request that *was* made.
    pub(crate) region: bridge::ShotRegion,
    /// `App::notice_seq` as it stood when this request was picked up.
    ///
    /// **The reply's `notice` means "what this shot caused", not "what is on screen".**
    /// A notice persists until something replaces it, so reporting the standing one
    /// made three consecutive shots claim a refusal belonging to an earlier request —
    /// true about the screen and false about the shot. The caller reports a notice
    /// only when the counter has moved past this floor.
    pub(crate) notice_floor: u64,
}

/// One request being worked on.
#[derive(Debug)]
struct Pending {
    request: ScreenshotRequest,
    followed: Vec<String>,
    /// `App::notice_seq` when this request was picked up. See `ShotRecord::notice_floor`.
    notice_floor: u64,
    queue: VecDeque<String>,
    /// When the current settle interval started. Reset by every dispatch, and by
    /// every frame the app is compiling.
    since: Instant,
    /// When the request was picked up, for [`OVERALL_DEADLINE`].
    started: Instant,
    /// When the viewport command was sent. `Some` means we are waiting for pixels.
    shot_at: Option<Instant>,
}

/// The screenshot channel's state. One per `App`.
#[derive(Debug)]
pub(crate) struct ScreenshotState {
    /// The highest `seq` already answered, so a request is acted on exactly once.
    ///
    /// **Seeded from the result file at startup.** A request left on disk from a
    /// previous session would otherwise be honoured the moment HRW opens, navigating
    /// Doug's window somewhere nobody asked for while he was using it.
    last_answered: Option<u64>,
    /// The mtime of the request file as last parsed. The single rule is *act on the
    /// request file when it changes*; without this a malformed file would be
    /// re-reported four times a second forever.
    seen_mtime: Option<SystemTime>,
    /// Whether this state may read the request file at all.
    ///
    /// **False in every test App, and that is isolation by construction rather than
    /// by convention.** `.hrw-bridge/` belongs to Doug's *running* HRW and his running
    /// VS Code, not to a fixture, and the headless `egui_kittest` harnesses paint real
    /// frames — so a watching `ScreenshotState` in a test App would poll his live
    /// request file, dispatch its links inside the harness, and fire viewport commands
    /// at a window that does not exist.
    ///
    /// This is not hypothetical: `tick_prewarm` did exactly that shape of thing and
    /// armed real breakpoints in his editor on every harness's first paint, reported
    /// twice, with the first two fixes missing it because no test body mentions the
    /// pre-warm at all. A source check can only see what a test names, and this would
    /// arrive through the frame loop the same way.
    watching: bool,
    pending: Option<Pending>,
}

impl ScreenshotState {
    /// The live channel: seeded from the result file, so a stale request is not
    /// re-honoured on launch, and given a waker thread.
    ///
    /// # Why a thread, found by the feature not working at all
    ///
    /// The poll runs inside `frame_ui`, and **egui does not paint an idle window**.
    /// The first live request was written, sat on disk, and was never noticed: the
    /// session log showed the last frame fourteen seconds *before* the request
    /// arrived. A Claude-driven capture is precisely the case where nobody is
    /// touching the window, so the one state the channel has to work in is the one
    /// state the app is asleep in.
    ///
    /// The obvious fix — `request_repaint_after(250ms)` from the tick — was measured
    /// against what a frame costs here and rejected: every frame runs `drain_worker`,
    /// republishes the diagnostic snapshot and **writes the session file to disk**, so
    /// it would mean four disk writes a second, forever, for a channel that is idle
    /// almost always.
    ///
    /// So the thread does one `stat` per interval and wakes the UI **only when the
    /// request file changes**. It deliberately does not read or parse the request:
    /// the UI thread's poll stays the single reader, so a spurious wake costs one
    /// frame and cannot produce a second opinion about what the request says.
    ///
    /// This is the same arrangement the worker already uses — a background thread
    /// holding a cloned `Context` so it can wake a sleeping UI, documented in
    /// `App::new`.
    pub(crate) fn new(ctx: &egui::Context) -> Self {
        spawn_waker(ctx.clone());
        Self {
            last_answered: bridge::last_answered_screenshot(),
            seen_mtime: None,
            watching: true,
            pending: None,
        }
    }

    /// A state that will never read the request file. See [`Self::watching`].
    ///
    /// `cfg(test)` so the *binary* cannot construct one: a non-watching state in a
    /// running HRW would make the channel silently dead, which is the one failure
    /// mode this module has no way to report.
    #[cfg(test)]
    pub(crate) fn for_tests() -> Self {
        Self {
            last_answered: None,
            seen_mtime: None,
            watching: false,
            pending: None,
        }
    }

    /// Whether a shot is in flight — the app repaints while one is, since a capture
    /// that waits for the next mouse move would wait forever on an idle window.
    pub(crate) fn is_pending(&self) -> bool {
        self.pending.is_some()
    }

    /// Advance the state machine one frame.
    ///
    /// `busy` is the app's `compiling` flag. A compile **restarts the settle clock**
    /// rather than merely delaying the shot: the frames while a specimen compiles
    /// show a half-built window, and the interesting content arrives on the frame the
    /// compile finishes.
    ///
    /// `now` is passed in rather than read here so the whole machine is testable
    /// against a synthetic clock.
    pub(crate) fn step(&mut self, busy: bool, notice_seq: u64, now: Instant) -> ShotStep {
        let Some(pending) = self.pending.as_mut() else {
            return self.poll(notice_seq, now);
        };

        // Waiting for pixels. Nothing else may happen until they arrive or the
        // deadline passes — a second command would leave two events in flight and no
        // way to tell which image answers which request.
        if let Some(sent) = pending.shot_at {
            if now.duration_since(sent) <= DELIVERY_DEADLINE {
                return ShotStep::Idle;
            }
            return self.fail(format!(
                "the backend never delivered the pixels within {}s of \
                 ViewportCommand::Screenshot",
                DELIVERY_DEADLINE.as_secs()
            ));
        }

        if now.duration_since(pending.started) > OVERALL_DEADLINE {
            let stage = if busy {
                "still compiling"
            } else {
                "never settled"
            };
            let left = pending.queue.len();
            return self.fail(format!(
                "gave up after {}s: {stage} with {left} link(s) left to follow",
                OVERALL_DEADLINE.as_secs(),
            ));
        }

        if busy {
            pending.since = now;
            return ShotStep::Idle;
        }
        if now.duration_since(pending.since) < pending.request.settle {
            return ShotStep::Idle;
        }
        if let Some(link) = pending.queue.pop_front() {
            pending.followed.push(link.clone());
            pending.since = now;
            return ShotStep::Follow(link);
        }
        pending.shot_at = Some(now);
        ShotStep::Shoot(pending.request.seq)
    }

    /// Abandon the pending request with `error`, marking its `seq` answered.
    ///
    /// Marking it answered is the part that matters: a failure that left the mark
    /// unmoved would be retried on the next poll, forever, and each retry would
    /// overwrite the result file that said what went wrong.
    fn fail(&mut self, error: String) -> ShotStep {
        let Some(pending) = self.pending.take() else {
            return ShotStep::Idle;
        };
        self.last_answered = Some(pending.request.seq);
        ShotStep::Failed {
            seq: Some(pending.request.seq),
            why: pending.request.why,
            followed: pending.followed,
            notice_floor: pending.notice_floor,
            error,
        }
    }

    /// Report that a `follow` link could not be parsed, abandoning the shot.
    ///
    /// **Abandoned rather than skipped.** A link that does not parse is a request for
    /// a state the app was never put into, so photographing the window anyway would
    /// produce an image of the wrong thing with nothing to say so. The app calls this
    /// when `parse_hrw_link` returns `None` for a link [`ShotStep::Follow`] just
    /// handed it.
    pub(crate) fn link_failed(&mut self, link: &str) -> ShotStep {
        self.fail(format!(
            "`{link}` is not a link HRW can follow \u{2014} nothing was photographed, \
             because the window was never put in the state the request asked for"
        ))
    }

    /// Take the record for a delivered image.
    ///
    /// `None` when `seq` does not match the pending request, which means a **stale
    /// event**: the image of an abandoned shot arriving after the deadline. Dropping
    /// it is right — its request has already been answered with an error — but the
    /// caller should say so rather than silently discard, since a stale delivery is
    /// evidence the deadline is too short.
    pub(crate) fn delivered(&mut self, seq: u64) -> Option<ShotRecord> {
        if self.pending.as_ref().map(|p| p.request.seq) != Some(seq) {
            return None;
        }
        let pending = self.pending.take()?;
        self.last_answered = Some(seq);
        Some(ShotRecord {
            seq,
            why: pending.request.why,
            followed: pending.followed,
            region: pending.request.region,
            notice_floor: pending.notice_floor,
        })
    }

    /// Look for a new request. One `stat`, and no rate limiter.
    ///
    /// **The rate limiter was removed after it raced the waker.** A timer here cannot
    /// see the wake the waker thread just sent, so a wake arriving just after a poll
    /// would find the next one "not due", return `Idle`, and let the app go back to
    /// sleep with an unanswered request on disk — and nothing would wake it again
    /// until the file changed a second time. One `stat` per painted frame is far
    /// cheaper than the session file this same frame already writes to disk, and it
    /// has no such hole.
    fn poll(&mut self, notice_seq: u64, now: Instant) -> ShotStep {
        if !self.watching {
            return ShotStep::Idle;
        }
        let Some((parsed, mtime)) = bridge::read_screenshot_request() else {
            return ShotStep::Idle;
        };
        // Act on the file when it CHANGES. An unchanged file costs one `stat`.
        if self.seen_mtime == Some(mtime) {
            return ShotStep::Idle;
        }
        self.seen_mtime = Some(mtime);

        let request = match parsed {
            Ok(request) => request,
            // A file that is not a request still gets a reply: whoever wrote it is
            // waiting for a PNG, and "no `seq`" is a far more useful answer than a
            // missing result file.
            Err(error) => {
                return ShotStep::Failed {
                    seq: None,
                    why: None,
                    followed: Vec::new(),
                    notice_floor: u64::MAX,
                    error,
                };
            }
        };
        // Already answered — the restart case, and the case of a file touched without
        // its `seq` moving. Neither is an error, and neither should move the window.
        if self.last_answered.is_some_and(|last| request.seq <= last) {
            return ShotStep::Idle;
        }
        self.pending = Some(Pending {
            followed: Vec::new(),
            notice_floor: notice_seq,
            queue: request.follow.iter().cloned().collect(),
            since: now,
            started: now,
            shot_at: None,
            request,
        });
        ShotStep::Idle
    }
}

/// Wake the UI thread whenever the request file changes. See [`ScreenshotState::new`].
///
/// Runs for the life of the process. There is nothing to shut down: it holds only a
/// `Context` clone, does one `stat` per interval, and the alternative — a stop flag
/// and a join — would be machinery guarding a thread whose whole body is a sleep.
fn spawn_waker(ctx: egui::Context) {
    let _ = std::thread::Builder::new()
        .name("hrw-screenshot-waker".to_string())
        .spawn(move || {
            let mut seen: Option<SystemTime> = None;
            loop {
                std::thread::sleep(crate::app::LAB_POLL_INTERVAL);
                let mtime = std::fs::metadata(bridge::SCREENSHOT_REQUEST_FILE)
                    .ok()
                    .and_then(|m| m.modified().ok());
                // Only a *change* wakes the UI. On the first iteration a request file
                // already on disk counts as one, which is harmless: the poll finds its
                // `seq` already answered and does nothing.
                if mtime.is_some() && mtime != seen {
                    ctx.request_repaint();
                }
                seen = mtime;
            }
        });
}

/// Keep only `region` of `image`, and say which region was actually kept.
///
/// `pane` with no `pane_rect` — nothing has drawn a central panel yet — falls back to
/// the whole window **and reports the fallback**, rather than failing the shot. A
/// photograph of too much is still a photograph; one that was never taken is not, and
/// the reply's `region` field is what makes the difference legible.
///
/// Clamped to the image rather than trusted: `pane_rect` is recorded on the frame the
/// central panel draws, and the pixels arrive a few frames later, so a window resized
/// in between would otherwise index past the end and panic inside `region`.
pub(crate) fn crop(
    image: &egui::ColorImage,
    region: bridge::ShotRegion,
    pane_rect: Option<egui::Rect>,
    pixels_per_point: f32,
) -> (egui::ColorImage, bridge::ShotRegion) {
    let (bridge::ShotRegion::Pane, Some(rect)) = (region, pane_rect) else {
        return (image.clone(), bridge::ShotRegion::Window);
    };
    let [width, height] = image.size;
    let scale = |v: f32| (v * pixels_per_point).round().max(0.0) as usize;
    let x = scale(rect.min.x).min(width);
    let y = scale(rect.min.y).min(height);
    let w = scale(rect.width()).min(width - x);
    let h = scale(rect.height()).min(height - y);
    if w == 0 || h == 0 {
        return (image.clone(), bridge::ShotRegion::Window);
    }
    (
        image.region_by_pixels([x, y], [w, h]),
        bridge::ShotRegion::Pane,
    )
}

/// egui's pixels as straight 8-bit RGBA, row by row from the top.
///
/// [`egui::Color32`] is **premultiplied**, and a PNG's RGBA is not, so writing the
/// raw bytes would darken every partially transparent pixel — most visibly the
/// shadows and the hover highlights, which is to say the parts of the window a reader
/// uses to tell an enabled control from a greyed one.
pub(crate) fn rgba_bytes(image: &egui::ColorImage) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(image.pixels.len() * 4);
    for pixel in &image.pixels {
        bytes.extend_from_slice(&pixel.to_srgba_unmultiplied());
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A request with one link, and a clock that advances past the settle interval.
    fn one_link_pending(now: Instant) -> ScreenshotState {
        let mut state = ScreenshotState::for_tests();
        state.pending = Some(Pending {
            request: ScreenshotRequest {
                seq: 7,
                why: Some("does the tab row offer Matching?".to_string()),
                follow: vec!["hrw://load/Drivetrain".to_string()],
                settle: Duration::from_millis(100),
                region: bridge::ShotRegion::Window,
            },
            followed: Vec::new(),
            queue: VecDeque::from(["hrw://load/Drivetrain".to_string()]),
            since: now,
            started: now,
            notice_floor: 0,
            shot_at: None,
        });
        state
    }

    /// The order is: settle, follow each link, settle again, then shoot.
    #[test]
    fn a_request_follows_its_links_before_shooting() {
        let t0 = Instant::now();
        let mut state = one_link_pending(t0);

        // Inside the settle interval: nothing yet.
        assert_eq!(
            state.step(false, 0, t0 + Duration::from_millis(50)),
            ShotStep::Idle
        );

        let t1 = t0 + Duration::from_millis(150);
        assert_eq!(
            state.step(false, 0, t1),
            ShotStep::Follow("hrw://load/Drivetrain".to_string()),
            "the queued link should be dispatched once the settle interval elapses"
        );
        // The dispatch restarts the clock, so the shot does not happen on the next
        // frame — the link's effects have not reached the screen yet.
        assert_eq!(
            state.step(false, 0, t1 + Duration::from_millis(10)),
            ShotStep::Idle
        );
        assert_eq!(
            state.step(false, 0, t1 + Duration::from_millis(150)),
            ShotStep::Shoot(7)
        );
    }

    /// A compile restarts the settle clock rather than merely delaying the shot.
    #[test]
    fn compiling_restarts_the_settle_clock() {
        let t0 = Instant::now();
        let mut state = one_link_pending(t0);
        state.pending.as_mut().unwrap().queue.clear();

        // Busy for a while: the interval elapses several times over, and nothing
        // fires, because the window is half-built the whole time.
        for ms in [50, 200, 400, 600] {
            assert_eq!(
                state.step(true, 0, t0 + Duration::from_millis(ms)),
                ShotStep::Idle,
                "no shot while compiling"
            );
        }
        // The compile ends at 600ms. A full settle interval must pass FROM THERE.
        let end = t0 + Duration::from_millis(600);
        assert_eq!(
            state.step(false, 0, end + Duration::from_millis(50)),
            ShotStep::Idle
        );
        assert_eq!(
            state.step(false, 0, end + Duration::from_millis(150)),
            ShotStep::Shoot(7)
        );
    }

    /// Every way a shot can fail must still answer the request. This is the
    /// must-fire test for this module: a failure that wrote nothing would leave the
    /// previous PNG on disk to be read as this request's answer.
    #[test]
    fn every_failure_answers_the_request() {
        let t0 = Instant::now();

        // 1. The backend never delivers.
        let mut state = one_link_pending(t0);
        state.pending.as_mut().unwrap().queue.clear();
        state.pending.as_mut().unwrap().shot_at = Some(t0);
        let step = state.step(false, 0, t0 + DELIVERY_DEADLINE + Duration::from_millis(1));
        let ShotStep::Failed {
            seq, error, why, ..
        } = step
        else {
            panic!("an undelivered shot must fail loudly, got {step:?}");
        };
        assert_eq!(seq, Some(7), "the error must name the request it answers");
        assert!(error.contains("never delivered"), "{error}");
        assert_eq!(
            why.as_deref(),
            Some("does the tab row offer Matching?"),
            "the request's own question belongs in its failure, so a result file read \
             later still says what was being asked"
        );
        assert!(
            !state.is_pending(),
            "a failed request must not stay pending, or it repaints forever"
        );
        assert_eq!(
            state.last_answered,
            Some(7),
            "a failed request must be marked answered, or the next poll retries it \
             and overwrites the result that said what went wrong"
        );

        // 2. The app never settles.
        let mut state = one_link_pending(t0);
        let step = state.step(true, 0, t0 + OVERALL_DEADLINE + Duration::from_millis(1));
        let ShotStep::Failed { seq, error, .. } = step else {
            panic!("a request that never settles must fail loudly, got {step:?}");
        };
        assert_eq!(seq, Some(7));
        assert!(error.contains("still compiling"), "{error}");
        assert!(
            error.contains("1 link"),
            "the error should say what was left: {error}"
        );

        // 3. A link that does not parse.
        let mut state = one_link_pending(t0);
        let step = state.link_failed("hrw://nonsense/42");
        let ShotStep::Failed { seq, error, .. } = step else {
            panic!("an unparseable link must fail loudly, got {step:?}");
        };
        assert_eq!(seq, Some(7));
        assert!(error.contains("hrw://nonsense/42"), "{error}");
        assert!(
            error.contains("nothing was photographed"),
            "the error must say no image exists, because one from a previous request \
             may still be on disk: {error}"
        );
        assert!(!state.is_pending());
    }

    /// **The notice floor is the counter as it stood when the request was picked up.**
    ///
    /// Found 2026-11-04 by the capture suite's first run: a notice stays on screen
    /// until something replaces it, so three consecutive shots came back carrying a
    /// refusal that belonged to an earlier request. The caller compares the app's
    /// counter against this floor, so a notice is reported only when the shot caused
    /// one — the floor being *recorded at pickup* is what makes that comparison mean
    /// anything.
    #[test]
    fn the_record_carries_the_notice_count_from_when_the_shot_began() {
        let t0 = Instant::now();
        let mut state = one_link_pending(t0);
        // 17 notices had already been shown when this request was picked up.
        state.pending.as_mut().unwrap().notice_floor = 17;
        state.pending.as_mut().unwrap().queue.clear();
        assert_eq!(
            state.step(false, 99, t0 + Duration::from_millis(150)),
            ShotStep::Shoot(7),
            "the counter passed in while a shot is pending must not disturb the floor"
        );
        let record = state.delivered(7).expect("delivered");
        assert_eq!(
            record.notice_floor, 17,
            "the floor is the count at PICKUP; taking it at delivery would make every \
             notice look pre-existing and none would ever be reported"
        );
    }

    /// The delivered image must match the request that asked for it.
    #[test]
    fn a_stale_delivery_is_refused() {
        let t0 = Instant::now();
        let mut state = one_link_pending(t0);
        assert_eq!(
            state.delivered(6),
            None,
            "an image tagged with another request's seq is not this request's answer"
        );
        assert!(state.is_pending(), "and must not consume the pending shot");

        let record = state.delivered(7).expect("the matching seq is accepted");
        assert_eq!(record.seq, 7);
        assert_eq!(state.last_answered, Some(7));
        assert_eq!(
            state.delivered(7),
            None,
            "a second delivery of the same shot has nothing left to answer"
        );
    }

    /// `followed` records what was dispatched, not what was asked for.
    #[test]
    fn the_record_names_the_links_actually_dispatched() {
        let t0 = Instant::now();
        let mut state = ScreenshotState::for_tests();
        state.pending = Some(Pending {
            request: ScreenshotRequest {
                seq: 1,
                why: None,
                follow: vec![
                    "hrw://load/Drivetrain".to_string(),
                    "hrw://simulate".to_string(),
                ],
                settle: Duration::from_millis(10),
                region: bridge::ShotRegion::Pane,
            },
            followed: Vec::new(),
            queue: VecDeque::from([
                "hrw://load/Drivetrain".to_string(),
                "hrw://simulate".to_string(),
            ]),
            since: t0,
            started: t0,
            notice_floor: 0,
            shot_at: None,
        });
        let mut clock = t0;
        let mut steps = Vec::new();
        for _ in 0..4 {
            clock += Duration::from_millis(20);
            steps.push(state.step(false, 0, clock));
        }
        assert_eq!(
            steps,
            vec![
                ShotStep::Follow("hrw://load/Drivetrain".to_string()),
                ShotStep::Follow("hrw://simulate".to_string()),
                ShotStep::Shoot(1),
                ShotStep::Idle,
            ]
        );
        let record = state.delivered(1).expect("delivered");
        assert_eq!(
            record.followed,
            vec![
                "hrw://load/Drivetrain".to_string(),
                "hrw://simulate".to_string()
            ],
            "the result file must name the links HRW dispatched, since a failed one \
             means the list is shorter than the request's"
        );
    }

    /// A pane crop keeps the pane, and the reply says which region it kept.
    #[test]
    fn a_pane_crop_keeps_the_pane_and_reports_itself() {
        // Four columns of one row, each a different colour, so a crop that took the
        // wrong columns cannot pass by accident.
        let colours = [
            egui::Color32::from_rgb(1, 0, 0),
            egui::Color32::from_rgb(2, 0, 0),
            egui::Color32::from_rgb(3, 0, 0),
            egui::Color32::from_rgb(4, 0, 0),
        ];
        let image = egui::ColorImage::new([4, 1], colours.to_vec());
        let pane = egui::Rect::from_min_size(egui::pos2(2.0, 0.0), egui::vec2(2.0, 1.0));

        let (cropped, kept) = crop(&image, bridge::ShotRegion::Pane, Some(pane), 1.0);
        assert_eq!(kept, bridge::ShotRegion::Pane);
        assert_eq!(cropped.size, [2, 1]);
        assert_eq!(
            cropped.pixels,
            vec![colours[2], colours[3]],
            "the crop must keep the pane's own columns, not the window's first ones"
        );

        // `pixels_per_point` is the whole reason the rect cannot be used raw: the rect
        // is in points and the image is in physical pixels.
        let hidpi =
            egui::ColorImage::new([8, 1], colours.iter().copied().cycle().take(8).collect());
        let (scaled, _) = crop(&hidpi, bridge::ShotRegion::Pane, Some(pane), 2.0);
        assert_eq!(
            scaled.size,
            [4, 1],
            "a 2-point-wide pane is 4 pixels wide at 2x; ignoring the scale would keep \
             half the pane and call it the pane"
        );
    }

    /// **A crop that cannot be made is reported as a window, never as a pane.**
    ///
    /// The silent-wrong-picture case: a reply claiming `pane` while holding the whole
    /// window would make every size check in a capture suite meaningless, and the
    /// reader would be looking at more than they were told.
    #[test]
    fn a_crop_that_cannot_be_made_says_window() {
        let image = egui::ColorImage::new([4, 1], vec![egui::Color32::WHITE; 4]);
        for (case, rect) in [
            // Nothing has drawn a central panel yet.
            ("no pane recorded", None),
            // A rect from before a resize, now entirely outside the image.
            (
                "pane outside the image",
                Some(egui::Rect::from_min_size(
                    egui::pos2(99.0, 99.0),
                    egui::vec2(10.0, 10.0),
                )),
            ),
            // A degenerate rect, which `region_by_pixels` would accept and return
            // empty — a zero-pixel PNG is not a photograph.
            (
                "empty pane",
                Some(egui::Rect::from_min_size(
                    egui::pos2(1.0, 0.0),
                    egui::vec2(0.0, 0.0),
                )),
            ),
        ] {
            let (out, kept) = crop(&image, bridge::ShotRegion::Pane, rect, 1.0);
            assert_eq!(
                kept,
                bridge::ShotRegion::Window,
                "{case}: the fallback must be reported, or the reply claims a crop it \
                 did not make"
            );
            assert_eq!(out.size, [4, 1], "{case}");
        }
    }

    /// Premultiplied in, straight out — and a solid pixel must survive untouched.
    #[test]
    fn rgba_bytes_undoes_the_premultiplication() {
        let opaque = egui::Color32::from_rgb(10, 200, 30);
        // `from_rgba_unmultiplied` premultiplies on the way in, so this round-trips
        // only if the conversion back out is applied.
        let half = egui::Color32::from_rgba_unmultiplied(255, 0, 0, 128);
        let image = egui::ColorImage::new([2, 1], vec![opaque, half]);
        let bytes = rgba_bytes(&image);
        assert_eq!(
            &bytes[0..4],
            &[10, 200, 30, 255],
            "an opaque pixel is unchanged"
        );
        let [r, g, b, a] = [bytes[4], bytes[5], bytes[6], bytes[7]];
        assert_eq!(a, 128);
        assert!(
            r >= 250 && g == 0 && b == 0,
            "a half-transparent red must come back out red, not the darkened \
             premultiplied value: got {r},{g},{b}"
        );
    }
}
