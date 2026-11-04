//! **Photograph every canvas sub-view of a specimen, in a running HRW.**
//!
//! ```text
//! cargo run -p hrw --example capture_suite                   # Drivetrain, pane crop
//! cargo run -p hrw --example capture_suite -- CapacitorLoop  # another specimen
//! cargo run -p hrw --example capture_suite -- Drivetrain window
//! ```
//!
//! **HRW must already be running.** This talks to it through the screenshot channel
//! (`src/bridge.rs`), the same files a hand-written request uses — it starts nothing,
//! so the window it photographs is a real session with a real GPU behind it.
//!
//! The reasoning for what is photographed, what is checked here and what is left for
//! the reader is in [`hrw::capture_suite`]. This file is the part that talks to the
//! filesystem and waits: everything with a judgement in it lives in the library, where
//! a test can reach it. That is the same split `examples/gate.rs` makes, and for the
//! same reason — **an example's code is reachable by no test**.
//!
//! On exit: `0` if every shot either landed or was refused by the app, `1` if any
//! failed. A refusal is the app answering, not a defect.

use std::io::Write as _;
use std::time::{Duration, Instant};

use hrw::bridge::{self, ShotRegion};
use hrw::capture_suite::{self, Outcome, Row};

/// How long one shot may take: a cold specimen compiles against the MSL, which is tens
/// of seconds on the large models, and the first shot of a run pays for all of it.
const SHOT_TIMEOUT: Duration = Duration::from_secs(180);

/// How often to look for the reply. The app answers in a frame or two once it has
/// settled; this only has to be short enough not to dominate the wait.
const POLL: Duration = Duration::from_millis(250);

fn main() -> std::io::Result<()> {
    let mut args = std::env::args().skip(1);
    let specimen = args.next().unwrap_or_else(|| "Drivetrain".to_string());
    let region = match args.next().as_deref() {
        None | Some("pane") => ShotRegion::Pane,
        Some("window") => ShotRegion::Window,
        Some(other) => {
            eprintln!("second argument must be `pane` or `window`, not `{other}`");
            std::process::exit(2);
        }
    };

    let plan = capture_suite::plan();
    // Start above whatever the channel has already answered, so this run cannot
    // collide with a request written by hand earlier in the same session.
    let first_seq = bridge::last_answered_screenshot().unwrap_or(0) + 1;
    println!(
        "{} shots of {specimen}, region {}, starting at seq {first_seq}",
        plan.len(),
        region.as_str()
    );
    println!("(HRW must be running; a cold compile makes the first shot the slow one)");

    let mut rows = Vec::with_capacity(plan.len());
    for (seq, shot) in (first_seq..).zip(plan) {
        let label = shot.label(&specimen);
        print!("  seq {seq:>4}  {label:<46}");
        let _ = std::io::stdout().flush();
        write_request(seq, &shot.link(&specimen), region, &label)?;
        let outcome = match await_reply(seq) {
            Ok(reply) => capture_suite::classify(&shot, &specimen, &reply),
            Err(error) => Outcome::Failed { error },
        };
        println!("{}", describe(&outcome));
        rows.push(Row { shot, seq, outcome });
    }

    let index = capture_suite::index_markdown(&specimen, region, &rows);
    let path = std::path::Path::new(bridge::BRIDGE_DIR).join("capture-suite.md");
    std::fs::write(&path, &index)?;
    println!("\nindex: {}", path.display());

    let failed = rows
        .iter()
        .filter(|r| matches!(r.outcome, Outcome::Failed { .. }))
        .count();
    if failed > 0 {
        eprintln!("{failed} shot(s) failed");
        std::process::exit(1);
    }
    Ok(())
}

/// One line of status per shot, short enough to read as a column.
fn describe(outcome: &Outcome) -> String {
    match outcome {
        Outcome::Landed { size, .. } => format!("landed {}x{}", size.0, size.1),
        Outcome::Refused { notice } => format!("refused ({notice})"),
        Outcome::Failed { error } => format!("FAILED ({error})"),
    }
}

/// Write one request and leave it on disk for HRW's poll to find.
fn write_request(seq: u64, link: &str, region: ShotRegion, why: &str) -> std::io::Result<()> {
    let request = serde_json::json!({
        "seq": seq,
        "why": format!("capture suite: {why}"),
        "follow": [link],
        "region": region.as_str(),
    });
    std::fs::create_dir_all(bridge::BRIDGE_DIR)?;
    std::fs::write(
        bridge::SCREENSHOT_REQUEST_FILE,
        serde_json::to_string_pretty(&request).unwrap_or_default(),
    )
}

/// Wait for the reply **to this `seq`**, and nothing else.
///
/// Matching on `seq` rather than on the file changing is what makes a slow shot
/// distinguishable from a stale reply: the previous request's result sits on disk the
/// whole time, and a poll that merely read the file would return it instantly and call
/// it this shot's answer.
fn await_reply(seq: u64) -> Result<serde_json::Value, String> {
    let deadline = Instant::now() + SHOT_TIMEOUT;
    while Instant::now() < deadline {
        std::thread::sleep(POLL);
        let Ok(text) = std::fs::read_to_string(bridge::SCREENSHOT_RESULT_FILE) else {
            continue;
        };
        let Ok(reply) = serde_json::from_str::<serde_json::Value>(&text) else {
            // A half-written file: HRW is writing it as we read. Try again.
            continue;
        };
        if reply.get("seq").and_then(serde_json::Value::as_u64) == Some(seq) {
            return Ok(reply);
        }
    }
    Err(format!(
        "no reply to seq {seq} within {}s \u{2014} is HRW running?",
        SHOT_TIMEOUT.as_secs()
    ))
}
