//! The `[hrw-context]` hook's executable — Claude Code runs it on every prompt.
//!
//! All of the logic, and the account of why it is shaped as it is, lives in
//! `hrw::context_hook`; this file only gathers the inputs a test would want to fix
//! (the prompt, the clock, the process probe) and prints the report.
//!
//! **It always exits 0.** A `UserPromptSubmit` hook that fails hands Claude nothing,
//! and the report already says, as a tagged line, whatever it could not read.

use std::io::{IsTerminal, Read};
use std::path::PathBuf;
use std::time::SystemTime;

use hrw::context_hook::{Inputs, hrw_is_running, report};

fn main() {
    // `$CLAUDE_PROJECT_DIR` is set by the harness. Without it — run by hand for
    // debugging — use the checkout this binary was built from.
    let root = std::env::var_os("CLAUDE_PROJECT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            crate_dir.parent().map(PathBuf::from).unwrap_or(crate_dir)
        });

    // The prompt arrives on stdin as JSON from the harness. Read it only when stdin is
    // not a terminal, so running this by hand does not block waiting for input.
    let mut prompt = None;
    let mut stdin = std::io::stdin();
    if !stdin.is_terminal() {
        let mut raw = String::new();
        if stdin.read_to_string(&mut raw).is_ok() {
            prompt = serde_json::from_str::<serde_json::Value>(&raw)
                .ok()
                .and_then(|v| v["prompt"].as_str().map(str::to_owned));
        }
    }

    let inputs = Inputs {
        root,
        prompt,
        now: SystemTime::now(),
        hrw_running: hrw_is_running(),
    };
    for line in report(&inputs) {
        println!("{line}");
    }
}
