//! The `[hrw-context]` hook: report HRW's assembled context on every prompt.
//!
//! Run by Claude Code's `UserPromptSubmit` hook (`.claude/settings.json`) through the
//! thin binary `src/bin/hrw-context.rs`. Prints a few lines describing what HRW
//! currently has assembled, so a question arrives with the context it is about.
//!
//! # The problem it solves
//!
//! HRW writes the capture to `.hrw-bridge/focus.json`, but nothing pushes that at
//! Claude — he has to think to read it. That makes an unprefaced question a coin-flip:
//! usually he looks, sometimes he answers about the wrong subject, and the failure is
//! silent. This turns "Claude must remember" into "Claude is always told".
//!
//! # Why it is Rust (2026-11-26)
//!
//! It was a PowerShell script in `hrw/scripts/` until the Fedora XPS joined the Windows
//! machine (deleted 2026-11-26; git has it):
//! Windows PowerShell exists on neither Linux nor macOS, and `pwsh` is installed on
//! neither of Doug's machines. Doug chose a Rust rewrite over a bash port, which would
//! have been a second copy of this logic to keep in step. Being Rust bought two further
//! things: the string contracts with `record_action` are now checked by the compiler's
//! neighbours rather than by parsing a script, and the behavioural tests below run
//! in-process on every OS — they were `#[cfg(windows)]` and spawned `powershell`.
//!
//! # It always prints something
//!
//! Every line carries the `[hrw-context]` tag, so its ABSENCE is evidence the hook is
//! not running — on a machine where it was never built, say. Silence would be
//! indistinguishable from "nothing is captured", which is the wrong-negative shape
//! this repository treats as the error nobody catches. Same reasoning as
//! `debug-state.json`'s `variables: null` meaning NOT FETCHED rather than none.
//!
//! # It reads `session.json`, not `focus.json`
//!
//! The capture file is ~740 KB and parsing it on every prompt would tax every message
//! Doug sends. `session.json` is ~7 KB and already carries the same summary —
//! `context.pointing_at`, `context.following`, the open lab and the UI mode.
//!
//! **The two files are written by different mechanisms, and keeping them in step is
//! the whole correctness of this hook.** `focus.json` is written by `bridge::write`
//! whenever the context changes; `session.json` only at the end of a frame in which an
//! action was RECORDED. Since 2026-11-07 `bridge::write` records a `context-emitted`
//! action itself, so every context change moves both files in the same frame. Before
//! that, the Question Bar's × buttons recorded nothing, and this hook went on
//! reporting a point Doug had cleared.
//!
//! So: the point's AGE comes from the last point gesture in the trail; whether a point
//! EXISTS comes from the snapshot; and `focus.json`'s mtime is a CROSS-CHECK against
//! the `context-emitted` action that wrote it. (Until 2026-11-06 the age came from that
//! mtime, which any writer could move.)

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

use crate::diagnostics::{format_utc_millis, parse_utc_millis};

/// The tag on every line. Its absence means the hook did not run.
pub const TAG: &str = "[hrw-context]";

/// **The action kinds that mark a point gesture — the actual recorded kinds.**
///
/// The PowerShell draft of this filter matched `'capture'`, which exists nowhere in
/// the trail: the word appeared only inside a doc comment, and a grep hit in prose was
/// read as a code fact. The bug would have been silent in the worst way — *"NO
/// capture"* forever, a false absence replacing a merely-unreliable age.
/// `doc_citations::the_hook_matches_real_action_kinds` fails by name if any name here
/// stops being a `record_action` literal in `src/`.
pub const POINT_KINDS: &[&str] = &["point-at", "point-at-selection", "point-dropped"];

/// The action `bridge::write` records whenever it writes `focus.json` — the
/// cross-check's reference. Checked by the same test as [`POINT_KINDS`].
pub const EMIT_KINDS: &[&str] = &["context-emitted"];

/// Everything the report depends on that is not a file under `root`.
///
/// Passed in rather than read inside [`report`] so the tests can fix each one: the
/// clock, the prompt, and the process probe are exactly the inputs the behavioural
/// tests need to hold still.
pub struct Inputs {
    /// The project root — `$CLAUDE_PROJECT_DIR`, the directory holding `hrw/`.
    pub root: PathBuf,
    /// The prompt text from the hook's stdin JSON, if any.
    pub prompt: Option<String>,
    /// The time the message was sent.
    pub now: SystemTime,
    /// Whether a process named `hrw` was running at `now`; `None` when this OS has no
    /// probe (see [`hrw_is_running`]).
    pub hrw_running: Option<bool>,
}

/// Build the report. Never panics on bad input: every unreadable file becomes a
/// tagged line saying so, because a crash here would be the silence this exists to
/// prevent. It does write one file, `question-seen.json`, when a question is shown.
pub fn report(inputs: &Inputs) -> Vec<String> {
    let mut out = Vec::new();
    let bridge = inputs.root.join("hrw").join(".hrw-bridge");
    let now_ms = millis(inputs.now);

    // The question block runs first, before anything that can exit early, so an
    // unreadable session.json cannot hide a waiting question.
    report_question(&mut out, &bridge, inputs.prompt.as_deref(), now_ms);
    report_context(&mut out, &bridge, now_ms, inputs.hrw_running);
    out
}

/// Push one tagged line.
fn say(out: &mut Vec<String>, line: impl AsRef<str>) {
    out.push(format!("{TAG} {}", line.as_ref()));
}

/// `"42s ago"`, `"8m ago"`, `"3h ago"` — the hook's one age format.
///
/// Rounds half to even, which is what PowerShell's `[int]` cast did, so an age reads
/// the same as it did from the script.
fn format_age(seconds: i64) -> String {
    if seconds < 90 {
        return format!("{seconds}s ago");
    }
    if seconds < 5400 {
        return format!("{}m ago", (seconds as f64 / 60.0).round_ties_even() as i64);
    }
    format!(
        "{}h ago",
        (seconds as f64 / 3600.0).round_ties_even() as i64
    )
}

/// Unix milliseconds of a `SystemTime`; 0 for a time before 1970.
fn millis(t: SystemTime) -> u128 {
    t.duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// Seconds from `then_ms` to `now_ms`, negative if `then` is in the future.
fn seconds_between(then_ms: u128, now_ms: u128) -> i64 {
    let (then, now) = (then_ms as i128, now_ms as i128);
    ((now - then) / 1000) as i64
}

/// A file's mtime in Unix milliseconds, or `None` if it cannot be read.
fn mtime_ms(path: &Path) -> Option<u128> {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .map(millis)
}

/// Read and parse a JSON file, with the error as text for the line that reports it.
fn read_json(path: &Path) -> Result<Value, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
}

/// A JSON value as PowerShell interpolated it: a string bare, null as nothing, and
/// anything else in its JSON form.
fn text(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// PowerShell truthiness, for the `if ($app.tour)` shape: null, `false`, `""` and 0
/// are absent.
fn present(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::String(s) => !s.is_empty(),
        Value::Number(n) => n.as_f64() != Some(0.0),
        _ => true,
    }
}

/// Remove the editor's `<ide_…>…</ide_…>` notes from a prompt.
///
/// **The VS Code extension puts its own notes in front of what Doug typed.** Found on
/// the FIRST REAL USE, 2026-11-07: Doug opened a file, typed `?`, and the message
/// arrived as `<ide_opened_file>The user opened the file …</ide_opened_file>` followed
/// by `?` — so the whole prompt was not `?`, and the hook announced "Doug has not
/// typed ? yet" to the `?` itself. Every test had sent a bare `?`, which is not what
/// the editor sends. The tags have a fixed, documented form, so removing them is
/// stripping the harness's markup, not interpreting Doug's words; what remains is
/// still compared exactly.
///
/// Matches what the script's regex `(?s)<ide_[a-z_]+>.*?</ide_[a-z_]+>` matched: an
/// opening tag, then the NEAREST closing tag of the same form.
fn strip_ide_notes(prompt: &str) -> String {
    /// The length of `<ide_name>` or `</ide_name>` at the start of `s`, if it is one.
    fn tag_len(s: &str, opener: &str) -> Option<usize> {
        let rest = s.strip_prefix(opener)?;
        let name = rest
            .bytes()
            .take_while(|b| b.is_ascii_lowercase() || *b == b'_')
            .count();
        (name > 0 && rest.as_bytes().get(name) == Some(&b'>')).then_some(opener.len() + name + 1)
    }

    let mut kept = String::with_capacity(prompt.len());
    let mut rest = prompt;
    while let Some(at) = rest.find("<ide_") {
        let candidate = &rest[at..];
        let closing = tag_len(candidate, "<ide_").and_then(|open| {
            let body = &candidate[open..];
            body.match_indices("</ide_")
                .find_map(|(i, _)| tag_len(&body[i..], "</ide_").map(|n| open + i + n))
        });
        match closing {
            Some(end) => {
                kept.push_str(&rest[..at]);
                rest = &candidate[end..];
            }
            None => {
                // Not a note: keep the `<` and look again after it.
                kept.push_str(&rest[..at + 1]);
                rest = &rest[at + 1..];
            }
        }
    }
    kept.push_str(rest);
    kept
}

// ===========================================================================
// THE QUESTION DOUG ASKED IN HRW (2026-11-07)
// ===========================================================================

/// Report the waiting question, or deliver it on `?`.
///
/// Doug asks HRW questions in HRW's Question Bar. HRW writes the question TOGETHER
/// WITH the context it is about to `question.json`, and cannot wake Claude — so the
/// question waits until Doug types `?` here. On `?` this prints it; on any other
/// prompt it says, in one line, that a question is waiting, so a question can never
/// sit unanswered because the trigger was forgotten.
///
/// **The question's context is not the live context, and the output says so.** The
/// lines printed after this describe what HRW holds NOW; the question was asked about
/// the context frozen in question.json. If Doug clicked after asking, they differ — and
/// reading the live `pointing at:` line as the question's referent would be the exact
/// join-by-time defect this design removes.
fn report_question(out: &mut Vec<String>, bridge: &Path, prompt: Option<&str>, now_ms: u128) {
    let question_path = bridge.join("question.json");
    let seen_path = bridge.join("question-seen.json");

    // `?` on its own is a PROTOCOL TOKEN, matched exactly. It is never a substring
    // search over Doug's words, which docs/identity-and-provenance.md forbids.
    let asks_for_question = prompt.is_some_and(|p| strip_ide_notes(p).trim() == "?");

    let Some(question_mtime) = mtime_ms(&question_path) else {
        if asks_for_question {
            say(
                out,
                "no HRW question has been asked - type one into HRW's Question Bar first",
            );
        }
        return;
    };

    // "Seen" is keyed to the question file's own mtime: question.json is written only
    // when Doug asks, so a new mtime is exactly a new question, and nothing needs
    // parsing on the prompts that are not `?`. (The script stored .NET ticks as
    // `question_mtime_ticks`; a seen-file from it reads as unseen, which costs one
    // extra reminder and nothing else.)
    let seen = read_json(&seen_path)
        .ok()
        .and_then(|v| v.get("question_mtime_ms").and_then(Value::as_u64));
    let already_shown = seen.is_some_and(|s| u128::from(s) == question_mtime);
    let age = format_age(seconds_between(question_mtime, now_ms));

    if !asks_for_question {
        if !already_shown {
            say(
                out,
                format!(
                    "AN HRW QUESTION IS WAITING (asked {age}) - Doug has not typed ? yet; if this message is not about it, say so"
                ),
            );
        }
        return;
    }

    let q = match read_json(&question_path) {
        Ok(q) => q,
        Err(e) => {
            say(out, format!("question.json exists but is unreadable: {e}"));
            return;
        }
    };
    let again = if already_shown {
        " (shown before - this is the same question)"
    } else {
        ""
    };
    // Doug, 2026-11-07: "you are free to use this chat window, a VS code tab, System
    // Modeler or Wolfram Desktop in whichever way you believe would be most effective."
    // This line said "ANSWER IT HERE, in the chat" until then — narrower than his
    // ruling, and the line Claude reads at the very moment of choosing.
    say(
        out,
        format!(
            "HRW QUESTION #{}, asked {age}{again} - answer from this conversation, NEVER in HRW, using the chat, a VS Code tab, System Modeler or Wolfram Desktop - whichever teaches best:",
            text(&q["seq"])
        ),
    );
    say(out, format!("  question: {}", text(&q["question"])));
    // A resolved "this = ..." line stood here until 2026-11-07. Doug: "for me, 'this'
    // often means the entire assembled context. ... If ever I use 'this' in a question
    // and you need clarification, then you should ask."
    say(
        out,
        "  if the question says 'this' and its referent is unclear, ASK Doug - HRW does not resolve it, and for him it often means the whole assembled context",
    );
    say(
        out,
        "  the context it was asked about is question.json's \"context\" - the SAME document focus.json held at that moment; open it for node detail",
    );
    say(
        out,
        "  the lines below are the LIVE context NOW; if Doug clicked after asking they differ, and the question is about the one above",
    );

    // Remember it was shown, so the "waiting" reminder stops.
    let seen = serde_json::json!({ "question_mtime_ms": question_mtime as u64, "seq": q["seq"] });
    if let Err(e) = std::fs::write(&seen_path, seen.to_string()) {
        say(
            out,
            format!("could not record the question as shown ({e}) - the reminder will repeat"),
        );
    }
}

// ===========================================================================
// THE LIVE CONTEXT
// ===========================================================================

/// The last action in the trail whose kind is one of `kinds`, with its time parsed.
fn last_action<'a>(actions: &'a [Value], kinds: &[&str]) -> Option<(&'a Value, Option<u128>)> {
    actions
        .iter()
        .rev()
        .find(|a| a["kind"].as_str().is_some_and(|k| kinds.contains(&k)))
        .map(|a| (a, a["at"].as_str().and_then(parse_utc_millis)))
}

/// Report what HRW holds now: freshness, whether HRW runs, and the point and follow.
fn report_context(out: &mut Vec<String>, bridge: &Path, now_ms: u128, hrw_running: Option<bool>) {
    let session_path = bridge.join("diagnostics").join("session.json");
    let focus_path = bridge.join("focus.json");

    let Some(session_mtime) = mtime_ms(&session_path) else {
        say(
            out,
            "HRW has not run in this clone (no session.json), so there is no assembled context.",
        );
        return;
    };
    let session = match read_json(&session_path) {
        Ok(s) => s,
        Err(e) => {
            say(out, format!("session.json is unreadable: {e}"));
            return;
        }
    };
    let app = &session["app"];
    let ctx = &app["context"];
    let actions: &[Value] = session["actions"].as_array().map_or(&[], Vec::as_slice);

    // **BOTH freshnesses, because they are different and the first version conflated
    // them.** It took the age from focus.json (written on capture) and the CONTENT from
    // session.json — and session.json is written only when HRW records an *action*, so
    // after a launch with no clicking it still describes the app at startup. The line
    // read "captured 8m ago" over state that was minutes stale. Found 2026-08-30 while
    // diagnosing why the 🎯 button did nothing: session.json's mtime never moved.
    say(
        out,
        format!(
            "state as of {} - session.json is rewritten only when HRW records an action",
            format_age(seconds_between(session_mtime, now_ms))
        ),
    );

    report_process(out, now_ms, hrw_running);

    let mut place = vec![format!("mode: {}", text(&app["ui_mode"]))];
    for (label, key) in [("tour", "tour"), ("model", "model"), ("stage", "stage_tab")] {
        if present(&app[key]) {
            place.push(format!("{label}: {}", text(&app[key])));
        }
    }
    say(
        out,
        format!("context #{} - {}", text(&ctx["seq"]), place.join(" - ")),
    );

    // **A capture older than the session is not this session's.** `seq` restarts at 0
    // on launch, so a leftover focus.json from a previous run would otherwise read as a
    // recent capture. The session's own start is the first thing in the action trail.
    let started = actions
        .iter()
        .find(|a| a["kind"] == "session")
        .and_then(|a| a["at"].as_str())
        .and_then(parse_utc_millis);

    report_point_age(out, ctx, actions, now_ms);
    report_focus_file(out, &focus_path, actions, started, now_ms);
    report_point_and_follow(out, ctx);
}

/// Whether HRW is running — a snapshot, said in the past tense.
///
/// **No file age can tell you this, and Claude kept inferring it from one.** A bridge
/// file outlives the process that wrote it, so a focus.json written 54s ago is equally
/// consistent with an app that exited 53s ago. On 2026-11-05 Claude asserted "HRW is
/// open" twice from exactly that.
///
/// **And it is a snapshot, which is why it does not say "is" (2026-11-07).** The probe
/// runs once, when Doug presses Enter, and a turn can last minutes. On 2026-11-07
/// Claude reported the line as a present fact three minutes later and POSTPONED A
/// COMMIT on it, after Doug had closed the app. So the line carries the time it was
/// taken and says plainly that it is not a reason to skip the gate: **the gate is what
/// decides whether the gate can run.** The time is UTC since the Rust rewrite, which
/// takes no time-zone dependency; it says so.
fn report_process(out: &mut Vec<String>, now_ms: u128, hrw_running: Option<bool>) {
    // `format_utc_millis` gives `YYYY-MM-DD HH:MM:SS.mmm UTC`; keep `HH:MM:SS`.
    let stamp = format_utc_millis(now_ms);
    let probed_at = stamp.get(11..19).unwrap_or(&stamp);
    match hrw_running {
        Some(true) => say(
            out,
            format!(
                "HRW WAS RUNNING at {probed_at} UTC, when this message was sent - a snapshot, not a fact about later: never report it as current, and never skip or postpone the gate on it (run the gate; it checks for itself)"
            ),
        ),
        Some(false) => say(
            out,
            format!(
                "HRW was NOT running at {probed_at} UTC, when this message was sent - every age below is a file's, not a live session's"
            ),
        ),
        None => say(
            out,
            "whether HRW is running is UNKNOWN - this OS has no process probe; every age below is a file's, not a live session's",
        ),
    }
}

/// The point's age, from the action trail — and agreeing with the snapshot.
///
/// **One source per claim.** The age comes from the action trail, the SAME file as the
/// context, because the version before 2026-11-06 took the age from focus.json's mtime
/// and the content from session.json, so the two lines were never about the same
/// event — and that mtime had once been moved 28 minutes earlier by `cargo test`
/// writing the live bridge.
///
/// **The line agrees with the snapshot, not merely with the last gesture**
/// (2026-11-07). The previous version reported the age of the latest point-kind action
/// whether or not that point still existed — so after the Question Bar's ×, which then
/// recorded nothing, it went on saying "pointed at 26m ago" over a point Doug had
/// cleared. `pointing_at` is the current state; the trail only supplies its age.
///
/// **No claim about "this" on these lines** (2026-11-07). What "this" means is Doug's,
/// and Claude asks when it is unclear. The lines state the fact only.
fn report_point_age(out: &mut Vec<String>, ctx: &Value, actions: &[Value], now_ms: u128) {
    let last_point = last_action(actions, POINT_KINDS);
    let dropped = last_point.is_some_and(|(a, _)| a["kind"] == "point-dropped");
    let point_time = last_point.and_then(|(_, t)| t);

    if ctx["pointing_at"].is_null() {
        match point_time {
            Some(t) if dropped => say(
                out,
                format!(
                    "the point was CLEARED {} - nothing is pointed at",
                    format_age(seconds_between(t, now_ms))
                ),
            ),
            _ => say(out, "nothing is pointed at"),
        }
        return;
    }
    match (last_point, point_time) {
        (Some((a, _)), Some(t)) if !dropped => say(
            out,
            format!(
                "pointed at {} via {} - from the action trail, the same file as the context below",
                format_age(seconds_between(t, now_ms)),
                text(&a["kind"])
            ),
        ),
        // **The trail is capped at 200 actions, so say that rather than claim an age.**
        // A long session can push the gesture out; the point itself is real (it is in
        // the snapshot), only its age is unknown.
        _ => say(
            out,
            "a point is set but its gesture is not in the trail (capped at 200 actions) - its age is unknown",
        ),
    }
}

/// Cross-check `focus.json`'s mtime against the emission that should have written it.
///
/// **focus.json is the DETAIL file, and its mtime is a CROSS-CHECK against the
/// EMISSION that wrote it** (2026-11-07). Every write of focus.json records a
/// `context-emitted` action in the same frame, at the one choke point
/// (`bridge::write`). So the two must agree to within that frame; a disagreement means
/// something OTHER than HRW's emitter touched the file, and reading it would describe
/// that writer.
///
/// The check before that compared against the last POINT gesture — and focus.json is
/// also rewritten by a follow, by either ×, and by a recompile that keeps the point.
/// So it warned "do NOT read it as Doug's context" after every edit-and-recompile,
/// about a file that was correct: an instruction to distrust the right file.
fn report_focus_file(
    out: &mut Vec<String>,
    focus_path: &Path,
    actions: &[Value],
    started: Option<u128>,
    now_ms: u128,
) {
    let Some(focus_time) = mtime_ms(focus_path) else {
        say(
            out,
            "no focus.json - nothing has ever been captured in this clone",
        );
        return;
    };
    let focus_age = format_age(seconds_between(focus_time, now_ms));
    let last_emit = last_action(actions, EMIT_KINDS);

    match last_emit {
        Some((emit, Some(_))) if text(&emit["detail"]).contains("NOT WRITTEN") => say(
            out,
            format!(
                "WARNING HRW's last emission FAILED ({}) - focus.json holds an OLDER context than the one below",
                text(&emit["detail"])
            ),
        ),
        Some((_, Some(emit_time))) => {
            // Same frame, so seconds of slack is generous.
            let skew = seconds_between(emit_time, focus_time).unsigned_abs();
            if skew > 5 {
                say(
                    out,
                    format!(
                        "WARNING focus.json ({focus_age}) was NOT written by HRW's last emission - something else touched it; do NOT read it as Doug's context"
                    ),
                );
            } else {
                say(
                    out,
                    format!(
                        "focus.json is HRW's latest emission ({focus_age}) - safe to open for node detail"
                    ),
                );
            }
        }
        _ if started.is_some_and(|s| focus_time < s) => say(
            out,
            format!(
                "focus.json ({focus_age}) PREDATES this HRW session - leftover from a previous run, not context"
            ),
        ),
        // No emission in the trail: either an HRW build from before 2026-11-07, which
        // did not record them, or a trail that scrolled. Say which cannot be told,
        // rather than guess — and do not warn, since there is nothing to compare.
        _ => say(
            out,
            format!(
                "focus.json written {focus_age} - cannot cross-check: no context-emitted in the trail (an HRW build from before 2026-11-07 does not record them; rebuild)"
            ),
        ),
    }
}

/// The point and the follow, each said even when absent.
fn report_point_and_follow(out: &mut Vec<String>, ctx: &Value) {
    let p = &ctx["pointing_at"];
    if present(p) {
        // **A null field is SAID, not left blank.** A lab passage has no stage, and the
        // first version interpolated the null straight in — "(tour passage in
        // connect-expansion, , Explain)". Two commas with nothing between them read as a
        // value that failed to render, not as a deliberate absence.
        let stage = if p["stage"].is_null() {
            "no stage - prose, not a phase".to_owned()
        } else {
            text(&p["stage"])
        };
        // **The verb is the user's message unless a gesture DECLARED one** — Doug's
        // ruling, 2026-11-07: "Unless I select something like a context menu item which
        // explicitly declares a verb, no verbs should be associated with my context
        // nouns." Every point once arrived tagged `Explain`, a verb nobody chose. Now
        // `request` is null unless the 🐞 debugger menu item declared it; an `Explain`
        // comes from an HRW build from before the fix and is named as presumed.
        let verb = match p["request"].as_str() {
            None => "no verb declared - the verb is the message".to_owned(),
            Some("Explain" | "explain") => {
                "no verb declared - \"Explain\" was PRESUMED by a pre-2026-11-07 HRW build; rebuild"
                    .to_owned()
            }
            Some(other) => format!("verb declared: {other}"),
        };
        say(
            out,
            format!(
                "pointing at: {}   ({}, {stage}, {verb})",
                text(&p["target"]),
                text(&p["kind"])
            ),
        );
    } else {
        say(out, "pointing at: nothing");
    }

    let f = &ctx["following"];
    if present(f) {
        // `mentions` is null until the tracking summary has been computed, which would
        // otherwise print "( mentions)".
        let mentions = if f["mentions"].is_null() {
            "not yet counted".to_owned()
        } else {
            format!("{} mentions", text(&f["mentions"]))
        };
        say(
            out,
            format!("following: {}   ({mentions})", text(&f["identifier"])),
        );
    } else {
        say(out, "following: nothing");
    }

    // A refusal to emit is context too: it means the bar is showing a point Claude does
    // not have, which is the one disagreement the Question Bar exists to make impossible.
    if present(&ctx["last_emission_error"]) {
        say(
            out,
            format!("NOT EMITTED - {}", text(&ctx["last_emission_error"])),
        );
    }
}

// ===========================================================================
// THE PROCESS PROBE
// ===========================================================================

/// Whether a process named exactly `hrw` is running, or `None` where there is no
/// probe for this OS.
///
/// Exact names on both, so neither this hook (`hrw-context`) nor a test binary
/// (`hrw-<hash>`) counts. On Linux `/proc/<pid>/comm` is the executable's name
/// (truncated at 15 bytes, which `hrw` is well inside). On Windows `tasklist` is the
/// equivalent of the script's `Get-Process -Name hrw`.
pub fn hrw_is_running() -> Option<bool> {
    #[cfg(target_os = "linux")]
    {
        let entries = std::fs::read_dir("/proc").ok()?;
        Some(entries.flatten().any(|e| {
            std::fs::read_to_string(e.path().join("comm")).is_ok_and(|c| c.trim_end() == "hrw")
        }))
    }
    #[cfg(windows)]
    {
        let out = std::process::Command::new("tasklist")
            .args(["/FI", "IMAGENAME eq hrw.exe", "/NH", "/FO", "CSV"])
            .output()
            .ok()?;
        Some(String::from_utf8_lossy(&out.stdout).contains("\"hrw.exe\""))
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// One fixture project, removed even if an assertion panics.
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            drop(std::fs::remove_dir_all(&self.0));
        }
    }
    impl Fixture {
        fn new(name: &str) -> Self {
            let root = std::env::temp_dir().join(format!(
                "hrw-hook-{}-{name}-{}",
                std::process::id(),
                millis(SystemTime::now())
            ));
            std::fs::create_dir_all(root.join("hrw/.hrw-bridge/diagnostics"))
                .expect("fixture dirs");
            Fixture(root)
        }
        fn bridge(&self) -> PathBuf {
            self.0.join("hrw").join(".hrw-bridge")
        }
        fn run(&self, prompt: Option<&str>, now: SystemTime) -> String {
            report(&Inputs {
                root: self.0.clone(),
                prompt: prompt.map(str::to_owned),
                now,
                hrw_running: Some(false),
            })
            .join("\n")
        }
    }

    #[test]
    fn ages_read_as_the_script_printed_them() {
        assert_eq!(format_age(42), "42s ago");
        assert_eq!(format_age(150), "2m ago", "2.5 rounds to even");
        assert_eq!(format_age(210), "4m ago", "3.5 rounds to even");
        assert_eq!(format_age(7200), "2h ago");
    }

    /// The editor's notes are stripped and nothing else is: an unclosed tag, a tag
    /// with no name, and ordinary angle brackets all survive.
    #[test]
    fn ide_notes_are_stripped_and_nothing_else_is() {
        assert_eq!(
            strip_ide_notes("<ide_opened_file>x</ide_opened_file>?"),
            "?"
        );
        assert_eq!(
            strip_ide_notes(
                "<ide_selection>a</ide_selection> <ide_opened_file>b</ide_opened_file>?"
            ),
            " ?"
        );
        assert_eq!(
            strip_ide_notes("a < b and <ide_x> unclosed"),
            "a < b and <ide_x> unclosed"
        );
        assert_eq!(
            strip_ide_notes("<ide_>no name</ide_>"),
            "<ide_>no name</ide_>"
        );
    }

    /// **The hook says something on an empty clone** — the must-fire floor: silence
    /// is what a hook that did not run looks like.
    #[test]
    fn an_empty_clone_still_gets_a_tagged_line() {
        let f = Fixture::new("empty");
        let out = f.run(None, SystemTime::now());
        assert!(out.starts_with(TAG), "{out}");
        assert!(out.contains("HRW has not run in this clone"), "{out}");
    }

    /// **The hook reports a cleared point as cleared, and never cries wolf over a
    /// correct `focus.json`** — against real-shaped states.
    ///
    /// # Why behaviour is tested, not lines
    ///
    /// The `[hrw-context]` hook is the reporter between HRW and every answer Claude
    /// gives. **It had three defects in two days** as a PowerShell script, and every one
    /// was a *behaviour* — which line it printed for which state:
    ///
    /// - 2026-11-06: it timed the capture by a different file's clock;
    /// - 2026-11-06, in the fix for that: it filtered on an action kind that is never
    ///   recorded (`doc_citations::the_hook_matches_real_action_kinds` covers that one);
    /// - 2026-11-07: its cross-check compared `focus.json` against the last POINT
    ///   gesture, so a follow or a recompile — both of which rewrite `focus.json` — drew
    ///   *"do NOT read it as Doug's context"* about a file that was correct. And after
    ///   the Question Bar's ×, it went on reporting the cleared point.
    ///
    /// This test ran the script through `powershell` and was `#[cfg(windows)]` until
    /// the 2026-11-26 Rust rewrite; it now calls [`report`] and runs on every OS.
    ///
    /// # The states, and what each must and must not print
    ///
    /// Each fixture is a `session.json` trail plus a `focus.json` with a set mtime,
    /// built with the trail's own timestamp formatter. **Three assert that a correct
    /// file draws no warning** — the false positive is the failure Doug hit, and a guard
    /// that only checked warnings fire would pass on a hook that warns always. **Two
    /// assert that a warning does fire**, so the hook cannot satisfy the first three by
    /// never warning.
    #[test]
    fn the_hook_reports_cleared_points_and_never_cries_wolf_on_a_correct_file() {
        let now = SystemTime::now();
        let now_ms = millis(now);
        let at = |secs_ago: u64| format_utc_millis(now_ms - u128::from(secs_ago) * 1000);
        let action = |secs_ago: u64, kind: &str, detail: &str| serde_json::json!({ "at": at(secs_ago), "kind": kind, "detail": detail });
        let a_point = serde_json::json!({
            "seq": 1, "target": "components.src.V", "kind": "a stage node",
            "stage": "Flatten", "request": null,
        });

        let run = |name: &str, actions: Vec<Value>, pointing_at: Value, focus_secs_ago: u64| {
            let f = Fixture::new(name);
            let session = serde_json::json!({
                "app": {
                    "ui_mode": "Lab",
                    "stage_tab": "Flatten",
                    "context": { "seq": 1, "pointing_at": pointing_at, "following": null },
                },
                "actions": actions,
            });
            std::fs::write(
                f.bridge().join("diagnostics").join("session.json"),
                session.to_string(),
            )
            .expect("write fixture session.json");
            let focus = f.bridge().join("focus.json");
            std::fs::write(&focus, "{}").expect("write fixture focus.json");
            std::fs::File::options()
                .write(true)
                .open(&focus)
                .and_then(|file| file.set_modified(now - Duration::from_secs(focus_secs_ago)))
                .expect("set fixture focus.json mtime");
            f.run(None, now)
        };

        // 1. The Question Bar's × cleared the point. Must say CLEARED, and must not
        //    report the age of the point that is gone.
        let out = run(
            "cleared",
            vec![
                action(600, "session", "HRW started"),
                action(500, "point-at-selection", "Answer"),
                action(500, "context-emitted", "focus.json seq 1"),
                action(100, "point-dropped", "components.src.V (cleared)"),
                action(100, "context-emitted", "focus.json seq 2"),
            ],
            Value::Null,
            100,
        );
        assert!(
            out.contains("the point was CLEARED"),
            "a cleared point is reported as cleared:\n{out}"
        );
        // The live-point line names its gesture ("pointed at 8m ago via …"). Checking
        // for that, not for "pointed at", which the CLEARED line correctly contains too
        // ("nothing is pointed at") — this assertion's first draft failed on exactly that.
        assert!(
            !out.contains("via point-at-selection"),
            "and not as a live point with an age:\n{out}",
        );
        assert!(
            !out.contains("WARNING"),
            "the file is correct, so no warning:\n{out}"
        );

        // 1b. The snapshot says no point, but the trail holds no drop — what an HRW build
        //     from before 2026-11-07 left after its ×, which recorded nothing. The point
        //     line must follow the SNAPSHOT.
        let out = run(
            "no-point-no-drop",
            vec![
                action(600, "session", "HRW started"),
                action(500, "point-at-selection", "Answer"),
                action(500, "context-emitted", "focus.json seq 1"),
            ],
            Value::Null,
            500,
        );
        assert!(
            out.contains("nothing is pointed at"),
            "no point in the snapshot is no point:\n{out}"
        );
        assert!(
            !out.contains("via point-at-selection"),
            "however recent the last point gesture in the trail:\n{out}",
        );

        // 2. A follow after the point rewrote focus.json — the false warning Doug hit.
        let out = run(
            "follow-after-point",
            vec![
                action(900, "session", "HRW started"),
                action(800, "point-at-selection", "Answer"),
                action(800, "context-emitted", "focus.json seq 1"),
                action(300, "follow", "follow h"),
                action(300, "context-emitted", "focus.json seq 2"),
            ],
            a_point.clone(),
            300,
        );
        assert!(
            out.contains("focus.json is HRW's latest emission"),
            "a follow's rewrite is HRW's own:\n{out}"
        );
        assert!(
            !out.contains("WARNING"),
            "and must not be called someone else's:\n{out}"
        );
        assert!(
            out.contains("pointed at 13m ago via point-at-selection"),
            "a live point reports its gesture and age:\n{out}"
        );

        // 3. A recompile kept the point and re-emitted it silently.
        let out = run(
            "recompile-keeps-point",
            vec![
                action(900, "session", "HRW started"),
                action(800, "point-at", "a node"),
                action(800, "context-emitted", "focus.json seq 1"),
                action(200, "compiled", "RcCircuit"),
                action(200, "context-emitted", "focus.json seq 1"),
            ],
            a_point.clone(),
            200,
        );
        assert!(
            !out.contains("WARNING"),
            "a recompile's re-emission is correct:\n{out}"
        );

        // 4. Must-fire: something other than HRW touched focus.json after its last emission.
        let out = run(
            "foreign-write",
            vec![
                action(900, "session", "HRW started"),
                action(800, "point-at", "a node"),
                action(800, "context-emitted", "focus.json seq 1"),
            ],
            a_point.clone(),
            10,
        );
        assert!(
            out.contains("NOT written by HRW's last emission"),
            "a write HRW did not make must be flagged:\n{out}",
        );

        // 5. Must-fire: HRW's last emission failed, so focus.json is older than the context.
        let out = run(
            "failed-emission",
            vec![
                action(900, "session", "HRW started"),
                action(800, "point-at", "a node"),
                action(
                    100,
                    "context-emitted",
                    "focus.json seq 2 NOT WRITTEN: denied",
                ),
            ],
            a_point,
            800,
        );
        assert!(
            out.contains("emission FAILED"),
            "a failed write must be said:\n{out}"
        );
    }

    /// **`?` answers the question asked in HRW, and a waiting question is never silent.**
    ///
    /// Doug's protocol, 2026-11-07: he types a question into HRW's Question Bar, HRW
    /// writes it with its context to `question.json`, and he types `?` here to have it
    /// answered. HRW cannot wake Claude, so the hook is the whole of the delivery — and
    /// like every reporter in this repository it gets a test that runs it.
    ///
    /// One fixture is driven through a **sequence**, because "seen" is state the hook
    /// keeps between prompts:
    ///
    /// 1. an ordinary prompt → *a question is waiting* (the trigger was forgotten);
    /// 2. `?` → the question, the instruction to ask when "this" is unclear (it printed
    ///    a resolved `this` until 2026-11-07), and the warning that the live context
    ///    below may differ from the one it was asked about;
    /// 3. an ordinary prompt → **no** reminder, since it has been shown;
    /// 4. `?` again → the same question, marked as shown before.
    ///
    /// Steps 1 and 3 are the must-fire pair: a hook that always printed the reminder
    /// would fail 3, and one that never did would fail 1. Like the test above, this
    /// spawned `powershell` and was `#[cfg(windows)]` until 2026-11-26.
    #[test]
    fn the_hook_delivers_an_hrw_question_on_question_mark_and_never_lets_one_wait_silently() {
        let now = SystemTime::now();
        let make = |name: &str| -> Fixture {
            let f = Fixture::new(name);
            let session = serde_json::json!({
                "app": { "ui_mode": "Lab", "stage_tab": "Flatten",
                         "context": { "seq": 3, "pointing_at": null, "following": null } },
                "actions": [],
            });
            std::fs::write(
                f.bridge().join("diagnostics").join("session.json"),
                session.to_string(),
            )
            .expect("fixture session.json");
            let question = serde_json::json!({
                "seq": 12,
                "question": "why is this singular?",
                "context": { "kind": "stage" },
            });
            std::fs::write(f.bridge().join("question.json"), question.to_string())
                .expect("fixture question.json");
            f
        };

        let asked = make("asked");

        // 1. The trigger was forgotten: the question must not sit unmentioned.
        let out = asked.run(Some("what does the matching do?"), now);
        assert!(
            out.contains("AN HRW QUESTION IS WAITING"),
            "a waiting question is said:\n{out}"
        );
        assert!(
            !out.contains("why is this singular?"),
            "but not delivered without ?:\n{out}"
        );

        // 2. `?` delivers it, with the instruction to ask about an unclear "this", and the
        //    live-context warning.
        let out = asked.run(Some("?"), now);
        assert!(
            out.contains("HRW QUESTION #12"),
            "? delivers the question:\n{out}"
        );
        assert!(
            out.contains("why is this singular?"),
            "with its text:\n{out}"
        );
        assert!(
            !out.contains("this ="),
            "the hook resolves \"this\"; Doug ruled that Claude asks instead:\n{out}"
        );
        assert!(
            out.contains("ASK Doug"),
            "and says to ask when \"this\" is unclear:\n{out}"
        );
        assert!(
            out.contains("LIVE context NOW"),
            "and says the live context may not be the one it was asked about:\n{out}",
        );

        // 3. Shown, so no reminder — the must-fire partner of step 1.
        let out = asked.run(Some("what does the matching do?"), now);
        assert!(
            !out.contains("WAITING"),
            "a shown question stops being announced:\n{out}"
        );

        // 4. `?` again repeats it, honestly marked.
        let out = asked.run(Some(" ? "), now);
        assert!(
            out.contains("shown before"),
            "a repeat says it is a repeat:\n{out}"
        );

        // 5. WHAT VS CODE ACTUALLY SENDS — found on the first real use, 2026-11-07. Doug
        //    had just opened a file, and the extension put its note in front of his `?`.
        //    Every step above sends a bare `?`, which the editor does not; this is the
        //    note copied from that real message.
        let note = "<ide_opened_file>The user opened the file \
                    c:\\Users\\dougd\\source\\repos\\rumoca\\hrw\\src\\doc_citations.rs in \
                    the IDE. This may or may not be related to the current \
                    task.</ide_opened_file>";
        let ide = make("ide-prefixed");
        let out = ide.run(Some(&format!("{note}?")), now);
        assert!(
            out.contains("HRW QUESTION #12"),
            "the editor's note in front of `?` must not hide the token:\n{out}",
        );
        assert!(
            !out.contains("has not typed ? yet"),
            "and the hook must never tell Claude that a `?` was not typed:\n{out}",
        );

        // 6. The must-not partner: a REAL question after the note is not the token.
        let real = make("ide-real-question");
        let out = real.run(Some(&format!("{note}what is this?")), now);
        assert!(
            !out.contains("HRW QUESTION #12"),
            "a question typed in the chat is not `?`, even with the note stripped:\n{out}",
        );
    }

    /// The process probe sees a process that is certainly running — this test's own,
    /// by the same mechanism — so a probe that silently always answers "not running"
    /// fails here rather than in a hook line nobody can check.
    #[cfg(target_os = "linux")]
    #[test]
    fn the_linux_probe_reads_real_process_names() {
        let own = std::fs::read_to_string("/proc/self/comm").expect("own comm");
        assert!(!own.trim().is_empty(), "comm names the executable");
        assert!(hrw_is_running().is_some(), "Linux has a probe");
    }
}
