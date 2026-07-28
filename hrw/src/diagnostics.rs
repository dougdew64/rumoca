//! Crash and diagnostic log — evidence that outlives the process.
//!
//! When HRW dies, the evidence dies with it. HRW is a *windowed* application:
//! a Rust panic prints to stderr, and launched from the VS Code debugger or
//! from Explorer there is frequently no stderr anyone reads. Twice now that has
//! cost real diagnostic time — a panic on clicking an identifier, and an
//! `exit code 101` from egui-wgpu's staging buffer during a long debugger
//! pause. Both were eventually solved, but only because the failure happened to
//! be re-creatable; a crash in the paint path or one depending on window state
//! would have left nothing but a description of what was clicked.
//!
//! This module writes that evidence to disk instead.
//!
//! ## Who this is for
//!
//! **Claude is the consumer.** These files are not user-facing error reports,
//! and they are not tuned for readability or brevity. They exist to let a
//! reasoner diagnose a failure it did not witness, so they carry raw detail and
//! nothing is summarised away. That is the same principle the bridge's
//! `focus.json` follows — see `DECISIONS.md` (2026-07-28).
//!
//! ## Why a backtrace is the *less* useful half
//!
//! A panic message and backtrace say **where** the process died. They rarely say
//! **why the app was there.** Reconstructing that from a user's sentence is the
//! expensive part, and every field of it already lives in `App`:
//!
//! - the assembled noun — what was pointed at, what was being followed;
//! - the specimen, model, stage tab and detail view;
//! - which animation was on screen and at which frame;
//! - the tail of the compilation log the log view already collects;
//! - which build this was.
//!
//! So the snapshot is the point of this module and the backtrace is a bonus.
//!
//! ## Two files, because there are two kinds of death
//!
//! 1. **`crash-<utc>.json`** — written from a panic hook. Complete: panic
//!    message, location, backtrace, app snapshot, recent actions, log tail.
//!    Rust's `exit code 101` is a panic, so this covers the egui-wgpu class of
//!    failure as well as our own bugs.
//! 2. **`session.json`** — rewritten on every recorded *user action*. Deliberately
//!    cheap and deliberately not per-frame. A stack overflow, a `SIGSEGV` from a
//!    graphics driver, or a hard kill runs **no** hook at all and would otherwise
//!    leave nothing; this file survives them because it was already on disk.
//!
//! `write_on_demand` produces the same content as a crash file for a session
//! that is misbehaving without dying — Doug asked for "crashes *and other
//! problems*", and a hang or a wrong-looking view needs the identical evidence.
//!
//! ## The action ring buffer
//!
//! A crash's cause is usually **the action before last**, not the state after.
//! State alone is a still photograph; the ring buffer is a reproduction script:
//! *selected MotorWithBrake, switched to Specimen source, clicked identifier
//! `overSpeed`* is something that can be replayed, and a final state is not.
//!
//! ## Design constraints worth knowing before editing
//!
//! - **A panic hook cannot borrow `App`.** It is `'static + Send + Sync` and
//!   runs on whichever thread panicked. So the app pushes a snapshot into a
//!   global here each frame, and the hook reads that global.
//! - **The hook must never panic**, or the process aborts and we lose the file
//!   we were trying to write. Every step is fallible-and-ignored: `try_lock`
//!   rather than `lock` (a panic *while the app holds the lock* would otherwise
//!   deadlock against itself — `std::sync::Mutex` is not reentrant), poisoned
//!   locks are recovered rather than unwrapped, and I/O errors are discarded.
//! - **The previous hook still runs**, so stderr keeps its normal message for
//!   anyone who does have a console.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

/// Where crash and session files are written.
///
/// Under `.hrw-bridge/` on purpose: that directory is already gitignored, and
/// already the place Claude reads for context. A crash file is context of a
/// different kind, and putting it anywhere else would mean explaining where to
/// look every time.
pub const DIAGNOSTICS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/.hrw-bridge/diagnostics");

/// How many user actions to retain. Generous because actions are rare (a click,
/// a tab switch) and because the useful one is often not the last.
const MAX_ACTIONS: usize = 200;

/// How many log lines to retain. A full compile of a specimen against the MSL
/// produces a few hundred; this keeps roughly one compile's worth.
const MAX_LOG: usize = 500;

/// One timestamped thing that happened, in the order it happened.
#[derive(Clone)]
struct Event {
    /// Milliseconds since the Unix epoch — absolute, so a crash file can be
    /// lined up against a git commit time or a note about what was being tried.
    at_ms: u128,
    /// Free-form category (`"click"`, `"stage"`, `"log:Error"`), used to scan.
    kind: String,
    detail: String,
}

impl Event {
    fn to_json(&self) -> Value {
        json!({ "at": format_utc_millis(self.at_ms), "kind": self.kind, "detail": self.detail })
    }
}

/// Everything the panic hook needs, reachable without touching `App`.
struct Diag {
    build: Value,
    /// The most recent per-frame app snapshot. `None` before the first frame —
    /// a crash during startup is real and must still produce a file.
    snapshot: Option<Value>,
    actions: VecDeque<Event>,
    log: VecDeque<Event>,
    /// Set once the panic hook has written a file, so a *second* panic while
    /// unwinding does not overwrite the first — the first is the interesting one.
    crash_written: bool,
}

/// `Option` rather than a const-constructed `Diag` so this needs no const
/// constructors; `Mutex::new(None)` is const and works on any std version we
/// build against.
static DIAG: Mutex<Option<Diag>> = Mutex::new(None);

/// Run `f` with the global state, doing nothing if it is unreachable.
///
/// `try_lock` is the important detail. Called from the panic hook, a blocking
/// `lock()` would deadlock if the panic happened while the app itself held the
/// lock — same thread, non-reentrant mutex, no timeout. Losing the file is bad;
/// hanging the process is worse. A poisoned lock (some earlier thread panicked
/// while holding it) is recovered rather than propagated, because the data is
/// plain records with no invariant to violate.
fn with_diag<R>(f: impl FnOnce(&mut Diag) -> R) -> Option<R> {
    let mut guard = match DIAG.try_lock() {
        Ok(g) => g,
        Err(std::sync::TryLockError::Poisoned(p)) => p.into_inner(),
        Err(std::sync::TryLockError::WouldBlock) => return None,
    };
    guard.as_mut().map(f)
}

/// Install the panic hook and record build identity. Call once, at startup.
///
/// Safe to call more than once — later calls reset the buffers but do not stack
/// extra hooks, which matters because tests call this.
pub fn init() {
    let already_initialised = {
        let mut guard = DIAG.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let was = guard.is_some();
        *guard = Some(Diag {
            build: build_info(),
            snapshot: None,
            actions: VecDeque::new(),
            log: VecDeque::new(),
            crash_written: false,
        });
        was
    };
    if already_initialised {
        return;
    }

    // Chain rather than replace: whoever would have seen the message on stderr
    // still sees it. `take_hook` returns the default hook the first time.
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        write_crash_file(info);
        previous(info);
    }));
}

/// Record a user action. Cheap, event-driven, and the reason a crash file reads
/// as a reproduction script rather than a snapshot.
///
/// Also rewrites `session.json`, which is what survives a death with no hook —
/// a stack overflow or a driver `SIGSEGV`. Writing a small file per click is
/// affordable precisely because clicks are rare; this must never be called from
/// the paint path.
pub fn record_action(kind: &str, detail: impl Into<String>) {
    let event = Event { at_ms: now_millis(), kind: kind.to_owned(), detail: detail.into() };
    let session = with_diag(|d| {
        push_capped(&mut d.actions, event, MAX_ACTIONS);
        d.report(None)
    });
    if let Some(report) = session {
        let _ = write_json(&dir().join("session.json"), &report);
    }
}

/// Mirror a log line into the crash buffer as it arrives.
///
/// Called once per `FromWorker::Log`, not per frame: the log view's own `Vec`
/// cannot be cloned into a snapshot 60 times a second, and it does not have to
/// be — entries only ever arrive one at a time.
pub fn record_log(level: &str, message: &str) {
    let event =
        Event { at_ms: now_millis(), kind: format!("log:{level}"), detail: message.to_owned() };
    with_diag(|d| push_capped(&mut d.log, event, MAX_LOG));
}

/// Replace the app-state snapshot. Called every frame from `ui()`.
///
/// Every frame, not throttled: the whole value of the snapshot is that it
/// describes the state *at the instant of the crash*, and a stale one would
/// misdirect exactly when it matters most. The cost is a few dozen small
/// allocations per frame, which is far below what an egui frame already spends
/// laying out text.
pub fn set_snapshot(snapshot: Value) {
    with_diag(|d| d.snapshot = Some(snapshot));
}

/// Write a diagnostic file for a session that is misbehaving but has not died.
///
/// Same content as a crash file minus the panic. Returns the path written, for
/// the UI to show — a file nobody can find is a file nobody sends.
pub fn write_on_demand() -> std::io::Result<PathBuf> {
    let report = with_diag(|d| d.report(None))
        .ok_or_else(|| std::io::Error::other("diagnostics not initialised"))?;
    let path = dir().join(format!("diagnostic-{}.json", file_stamp()));
    write_json(&path, &report)?;
    Ok(path)
}

impl Diag {
    /// Assemble the full report. `panic` is `None` for on-demand snapshots.
    ///
    /// Ordering is deliberate: `panic` and `app` first because they answer
    /// "what broke and where was the user", then `actions` (how they got
    /// there), then `log` (what the compiler was doing), then `build`.
    fn report(&self, panic: Option<Value>) -> Value {
        json!({
            "note": "HRW diagnostic capture. Written for Claude, not for display. \
                     `app` is the state at the moment of capture; `actions` is the \
                     user's path to it, most recent last, and is usually the more \
                     informative of the two. `log` is the tail of the compilation \
                     log the Log view shows. Absent fields mean the state did not \
                     exist, not that it was omitted.",
            "captured_at": format_utc_millis(now_millis()),
            "panic": panic,
            "app": self.snapshot,
            "actions": self.actions.iter().map(Event::to_json).collect::<Vec<_>>(),
            "log": self.log.iter().map(Event::to_json).collect::<Vec<_>>(),
            "build": self.build,
        })
    }
}

/// The panic hook's body. Must not panic, and must not block.
fn write_crash_file(info: &std::panic::PanicHookInfo<'_>) {
    // `PanicHookInfo::payload` is the `Box<dyn Any>` from `panic!`. The two
    // concrete types it is ever built from are `&'static str` (a literal
    // message) and `String` (a formatted one); anything else is a
    // `panic_any` and has no text to show.
    let payload = info
        .payload()
        .downcast_ref::<&str>()
        .map(|s| (*s).to_owned())
        .or_else(|| info.payload().downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "<non-string panic payload>".to_owned());

    let panic = json!({
        "message": payload,
        "location": info.location().map(|l| json!({
            "file": l.file(), "line": l.line(), "column": l.column(),
        })),
        "thread": std::thread::current().name().unwrap_or("<unnamed>"),
        // `force_capture` ignores RUST_BACKTRACE — a crash file with the
        // backtrace switched off would be missing the one thing that is free.
        "backtrace": std::backtrace::Backtrace::force_capture().to_string(),
    });

    let report = with_diag(|d| {
        if d.crash_written {
            // A panic while unwinding a panic. The first one is the cause.
            return None;
        }
        d.crash_written = true;
        Some(d.report(Some(panic)))
    });

    if let Some(Some(report)) = report {
        let _ = write_json(&dir().join(format!("crash-{}.json", file_stamp())), &report);
    }
}

fn push_capped(queue: &mut VecDeque<Event>, event: Event, cap: usize) {
    if queue.len() == cap {
        queue.pop_front();
    }
    queue.push_back(event);
}

fn dir() -> PathBuf {
    PathBuf::from(DIAGNOSTICS_DIR)
}

fn write_json(path: &Path, value: &Value) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // Pretty-printed: these are read, not parsed, and a wall of one-line JSON
    // costs more to work with than the bytes save.
    std::fs::write(path, serde_json::to_vec_pretty(value)?)
}

/// What was running. Enough to know which code produced the crash — the git rev
/// especially, since a crash file may arrive after further commits have landed.
fn build_info() -> Value {
    json!({
        "hrw_version": env!("CARGO_PKG_VERSION"),
        // All three come from `build.rs`. The rev is the *workspace* HEAD, which
        // since the in-workspace move is HRW's own commit as well as Rumoca's.
        "rumoca_version": env!("HRW_RUMOCA_VERSION"),
        "git_rev": env!("HRW_RUMOCA_REV"),
        // Without this the rev is actively misleading mid-session: it names a
        // commit whose code is not what ran.
        "git_dirty": env!("HRW_GIT_DIRTY") == "1",
        "profile": if cfg!(debug_assertions) { "debug" } else { "release" },
        "target_os": std::env::consts::OS,
        "target_arch": std::env::consts::ARCH,
        // The graphics backend override. `WGPU_BACKEND=gl` is what stopped
        // egui-wgpu losing its device during long debugger pauses, so its
        // presence or absence is load-bearing for a whole class of crash.
        "wgpu_backend": std::env::var("WGPU_BACKEND").unwrap_or_else(|_| "<unset>".to_owned()),
    })
}

fn now_millis() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis())
}

/// `YYYYmmdd-HHMMSS-mmm`, for file names — sorts chronologically as text.
fn file_stamp() -> String {
    let ms = now_millis();
    let (y, mo, d, h, mi, s, milli) = civil_from_millis(ms);
    format!("{y:04}{mo:02}{d:02}-{h:02}{mi:02}{s:02}-{milli:03}")
}

/// `YYYY-MM-DD HH:MM:SS.mmm UTC`, for reading.
fn format_utc_millis(ms: u128) -> String {
    let (y, mo, d, h, mi, s, milli) = civil_from_millis(ms);
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:02}.{milli:03} UTC")
}

/// Split Unix milliseconds into UTC calendar fields.
///
/// Hand-rolled to avoid taking a date/time dependency for two format strings
/// (`DECISIONS.md`: ask before adding a dependency). The date part is Howard
/// Hinnant's `civil_from_days`, the standard branch-free algorithm behind
/// `std::chrono`: it shifts the epoch to 0000-03-01 so that the leap day falls
/// at the *end* of the year, which is what removes the special-casing. UTC only
/// — no leap seconds, no zones, none of which a crash file needs.
fn civil_from_millis(ms: u128) -> (i64, u32, u32, u32, u32, u32, u32) {
    let secs = (ms / 1000) as i64;
    let milli = (ms % 1000) as u32;
    let days = secs.div_euclid(86_400);
    let sod = secs.rem_euclid(86_400);

    let z = days + 719_468; // days since 0000-03-01
    let era = z.div_euclid(146_097); // a 400-year cycle
    let doe = z.rem_euclid(146_097); // day of era, 0..=146096
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // 0..=399
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // 0..=365, March-based
    let mp = (5 * doy + 2) / 153; // month, March=0
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = if m <= 2 { y + 1 } else { y };

    (y, m, d, (sod / 3600) as u32, ((sod / 60) % 60) as u32, (sod % 60) as u32, milli)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_conversion_matches_known_instants() {
        // The epoch itself.
        assert_eq!(civil_from_millis(0), (1970, 1, 1, 0, 0, 0, 0));
        // 2000-02-29: a leap day in a century year that *is* a leap year — the
        // case the 400-year rule exists for.
        assert_eq!(civil_from_millis(951_782_400_000), (2000, 2, 29, 0, 0, 0, 0));
        // 2100-03-01: the day after February in a century year that is *not* a
        // leap year, so the 100-year rule must have skipped a 29th.
        assert_eq!(civil_from_millis(4_107_542_400_000), (2100, 3, 1, 0, 0, 0, 0));
        // A time of day with milliseconds.
        assert_eq!(civil_from_millis(1_753_660_496_789), (2025, 7, 27, 23, 54, 56, 789));
    }

    #[test]
    fn stamps_are_sortable_and_readable() {
        let stamp = file_stamp();
        assert_eq!(stamp.len(), "YYYYmmdd-HHMMSS-mmm".len(), "unexpected stamp shape: {stamp}");
        assert!(format_utc_millis(0).starts_with("1970-01-01 00:00:00.000"));
    }

    #[test]
    fn ring_buffer_keeps_the_most_recent() {
        let mut q = VecDeque::new();
        for i in 0..5 {
            push_capped(&mut q, Event { at_ms: i, kind: "k".into(), detail: i.to_string() }, 3);
        }
        assert_eq!(q.len(), 3);
        let details: Vec<&str> = q.iter().map(|e| e.detail.as_str()).collect();
        assert_eq!(details, ["2", "3", "4"], "oldest events must be the ones dropped");
    }

    /// The report must be well-formed before the first frame. A crash during
    /// startup is exactly when there is no snapshot, and it must still write.
    #[test]
    fn report_is_complete_without_a_snapshot() {
        let d = Diag {
            build: build_info(),
            snapshot: None,
            actions: VecDeque::new(),
            log: VecDeque::new(),
            crash_written: false,
        };
        let r = d.report(None);
        for key in ["note", "captured_at", "panic", "app", "actions", "log", "build"] {
            assert!(r.get(key).is_some(), "report is missing `{key}`");
        }
        assert!(r["app"].is_null());
        assert!(r["build"]["git_rev"].is_string());
    }

    /// `with_diag` must be a no-op rather than a panic when uninitialised —
    /// `record_action` is called from paths that tests exercise directly.
    #[test]
    fn recording_before_init_is_harmless() {
        // Whether another test has already run `init` is unknown (tests share a
        // process), so this asserts only that neither call panics.
        record_log("Info", "no panic expected");
        record_action("test", "no panic expected");
    }

    /// End-to-end: init, act, snapshot, write — and a real file on disk with
    /// the reproduction script in it.
    ///
    /// The valuable assertion is the last one. A crash file that carries state
    /// but not the path to it is the still-photograph failure this module
    /// exists to avoid, and only an ordering assertion catches a ring buffer
    /// that silently drops or reorders.
    ///
    /// Shares a process with every other test, so it must not assume it owns
    /// the buffers — it asserts about *its own* events by searching for them.
    #[test]
    fn writes_a_file_carrying_state_and_the_path_to_it() {
        init();
        record_action("specimen", "MotorWithBrake.mo");
        record_action("stage-tab", "Resolve");
        record_action("follow", "follow overSpeed (in Resolve)");
        record_log("Error", "structurally singular \u{2014} see the Index Reduction tab");
        set_snapshot(json!({ "model": "MotorWithBrake", "stage_tab": "Resolve" }));

        let path = write_on_demand().expect("diagnostic should be written");
        let text = std::fs::read_to_string(&path).expect("written file should be readable");
        std::fs::remove_file(&path).ok();

        let report: Value = serde_json::from_str(&text).expect("valid JSON");
        assert_eq!(report["app"]["model"], "MotorWithBrake");
        assert!(report["panic"].is_null(), "on-demand snapshots carry no panic");
        assert!(report["build"]["git_rev"].is_string());

        // The em dash is the character that crashed the lexer. A log line
        // carrying one must survive into the file intact — the crash log must
        // not itself choke on the text describing a crash.
        let log = report["log"].as_array().expect("log array");
        assert!(
            log.iter().any(|e| e["detail"].as_str().is_some_and(|d| d.contains('\u{2014}'))),
            "log entries must round-trip non-ASCII",
        );

        // Ordering: the three actions must appear, oldest first. Matched
        // exactly, not by substring — "follow overSpeed (in Resolve)" contains
        // "Resolve" too, and a substring search silently compared an entry with
        // itself.
        let details: Vec<&str> =
            report["actions"].as_array().expect("actions array")
                .iter()
                .filter_map(|e| e["detail"].as_str())
                .collect();
        let pos = |needle: &str| {
            details.iter().rposition(|d| *d == needle).unwrap_or_else(|| {
                panic!("action {needle:?} missing from {details:?}")
            })
        };
        assert!(
            pos("MotorWithBrake.mo") < pos("Resolve")
                && pos("Resolve") < pos("follow overSpeed (in Resolve)"),
            "actions must read in the order they happened: {details:?}",
        );
    }
}
