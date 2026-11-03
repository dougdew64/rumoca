//! Proves the panic path writes a crash file. Run: `cargo run -p hrw --example crash_probe`
//!
//! Not a unit test: a test that panics on purpose is caught by the harness,
//! which installs its *own* panic hook and reports a failure — so the one thing
//! worth verifying (that a real, process-killing panic leaves a file behind)
//! cannot be checked from inside `cargo test`. This example panics for real.
fn main() {
    hrw::diagnostics::init();
    hrw::diagnostics::record_action("specimen", "MotorWithBrake.mo");
    hrw::diagnostics::record_action("follow", "follow overSpeed (in Resolve)");
    hrw::diagnostics::record_log(
        "Error",
        "note containing an em dash \u{2014} like the real one",
    );
    hrw::diagnostics::set_snapshot(serde_json::json!({
        "model": "MotorWithBrake",
        "stage_tab": "Resolve",
        "context": { "following": { "identifier": "__pre__.overSpeed" } },
    }));
    eprintln!("crash files land in {}", hrw::diagnostics::DIAGNOSTICS_DIR);
    panic!("deliberate crash probe");
}
