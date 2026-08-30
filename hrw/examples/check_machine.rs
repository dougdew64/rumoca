//! **Verify this machine is ready to work on HRW.** Run it after switching machines.
//!
//! ```text
//! cargo run -p hrw --example check_machine
//! ```
//!
//! # Why this is Rust and not a PowerShell script
//!
//! It was `hrw/scripts/check-machine.ps1` for about two hours. Doug switched
//! machines, ran it, and got *"the script is not digitally signed"* — the other
//! machine's execution policy is stricter than this one's. **The script written to
//! catch per-machine differences was blocked by a per-machine difference**, on the
//! first switch it was written for.
//!
//! `cargo run --example` has no execution policy, so the whole class is gone rather
//! than the instance. It is also the move this repository already made once:
//! `promote-run.ps1` became `examples/promote_run.rs`, and `docs/tech-debt.md`
//! carries the standing *"move it where the toolchain can check it"* trigger.
//!
//! **The build cost is a feature.** On a machine that has just been switched to, this
//! compiles `hrw` first — which proves the toolchain works, and *that* is one more
//! thing a `git pull` does not bring.
//!
//! # What it checks, and why these
//!
//! Only things that do **not** travel with a `git pull`. Everything here has cost a
//! real failure:
//!
//! - **The project settings and the context hook** — `.claude/settings.json` carries
//!   the permission allowlist *and* the `UserPromptSubmit` hook that reports HRW's
//!   assembled context on every prompt. **It used to be the archetype of this file's
//!   purpose**: `.claude/` is gitignored by upstream, so it was per machine, and
//!   without it every Bash call prompts — indistinguishable from a hang during an
//!   unattended run. Since 2026-08-30 `.gitignore` re-includes it, so it travels, and
//!   what is checked here is that its *contents* are intact rather than that it exists.
//! - **A locked `hrw.exe`** — the gate cannot relink while HRW runs, and after a
//!   `clippy --all-targets` that failure is permanent rather than transient.
//! - **The parsed-artifact cache** — per machine and keyed on a fingerprint of
//!   `crates/`, so a first compile re-parses the whole MSL. Advisory: it predicts a
//!   slow gate, which should not be diagnosed as a hang.
//! - **The VS Code bridge extension** — built and junctioned per machine. Advisory:
//!   only `matching-live.md` needs it.
//!
//! Blocking problems exit non-zero and name their fix. Advisory ones report and do
//! not, because a check that fails for things that are merely worth knowing trains
//! the reader to ignore it.

use hrw::machine_policy::{SleepRuling, fix_for, rule};
use std::path::{Path, PathBuf};
use std::process::Command;

/// `hrw/`, from which the repository root is the parent.
fn hrw_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn repo_root() -> PathBuf {
    hrw_dir()
        .parent()
        .expect("hrw/ lives inside the workspace")
        .to_path_buf()
}

#[derive(PartialEq)]
enum Verdict {
    Pass,
    Fail,
    Warn,
}

struct Report {
    blocking: usize,
    advisory: usize,
}

impl Report {
    fn line(&mut self, verdict: Verdict, name: &str, detail: &str, fix: &str) {
        let tag = match verdict {
            Verdict::Pass => "PASS",
            Verdict::Fail => "FAIL",
            Verdict::Warn => "WARN",
        };
        println!("  {tag:<5} {name}");
        if !detail.is_empty() {
            println!("        {detail}");
        }
        if verdict != Verdict::Pass && !fix.is_empty() {
            println!("        fix: {fix}");
        }
        match verdict {
            Verdict::Fail => self.blocking += 1,
            Verdict::Warn => self.advisory += 1,
            Verdict::Pass => {}
        }
    }
}

/// Is `hrw.exe` held open by a running HRW?
///
/// **Tests the actual failure rather than a proxy for it.** Asking whether a process
/// named `hrw` exists would need `tasklist` parsing and would still not answer the
/// question that matters, which is whether cargo can replace the binary. Opening it
/// for write without truncating does answer that: Windows refuses while it is
/// mapped. An absent binary is not a failure — there is nothing to lock.
fn binary_is_locked(exe: &Path) -> bool {
    if !exe.exists() {
        return false;
    }
    std::fs::OpenOptions::new().write(true).open(exe).is_err()
}

/// Ask `powercfg` whether this machine will suspend, and rule on the answer.
///
/// **The full path is deliberate.** Under MSYS a bare `powercfg /q …` had `/q`
/// rewritten into a Windows path and came back `Invalid Parameters`; invoking the
/// executable directly from Rust avoids that shell entirely. A failure to run at
/// all lands on the same `Unreadable` arm as unrecognised output, because both mean
/// the same thing here: this machine's settings are unknown, not confirmed safe.
fn sleep_ruling() -> SleepRuling {
    let out = Command::new("powercfg")
        .args(["/q", "SCHEME_CURRENT", "SUB_SLEEP"])
        .output();
    match out {
        Ok(o) => rule(&String::from_utf8_lossy(&o.stdout)),
        Err(_) => SleepRuling::Unreadable,
    }
}

fn main() {
    let repo = repo_root();
    let mut r = Report {
        blocking: 0,
        advisory: 0,
    };

    println!("\nHRW machine check  --  {}\n", repo.display());

    // ------------------------------------------------------------- blocking --

    // **This checked a file that no longer travels per machine** — `.gitignore` now
    // re-includes `.claude/settings.json` (2026-08-30), so a `git pull` brings the
    // permission allowlist AND the UserPromptSubmit hook with it. What was the most
    // frequent blocking failure here is now a `git checkout` away from impossible.
    //
    // **It is kept, and it checks CONTENT rather than existence.** A present file that
    // has lost its hook is the case that matters now, and it is exactly the silent one:
    // the hook reports HRW's assembled context on every prompt, so without it Claude
    // answers "what is this?" about whatever he last read instead of what Doug just
    // captured — confidently, and with nothing on screen to say the channel is dead.
    let settings = repo.join(".claude").join("settings.json");
    match std::fs::read_to_string(&settings) {
        Err(_) => r.line(
            Verdict::Fail,
            "project settings",
            "missing, so every Bash call prompts and the context hook cannot run",
            "it is checked in now: `git checkout .claude/settings.json`",
        ),
        Ok(text) if !text.contains("hrw-context.ps1") => r.line(
            Verdict::Fail,
            "project settings",
            "present, but the UserPromptSubmit context hook is gone",
            "restore it: `git checkout .claude/settings.json` (hrw/docs/setup-windows.md \u{00a7}8)",
        ),
        Ok(_) => r.line(
            Verdict::Pass,
            "project settings",
            "allowlist and context hook both present",
            "",
        ),
    }

    // The hook's own script, which the settings file points at. Checked separately
    // because the two rot independently: a rename here leaves the settings pointing at
    // nothing, and PowerShell's failure would arrive as an empty prompt rather than an
    // error Doug sees.
    let hook = repo.join("hrw").join("scripts").join("hrw-context.ps1");
    if hook.exists() {
        r.line(Verdict::Pass, "context hook script", "present", "");
    } else {
        r.line(
            Verdict::Fail,
            "context hook script",
            "hrw/scripts/hrw-context.ps1 is missing; every prompt loses HRW's context",
            "`git checkout hrw/scripts/hrw-context.ps1`",
        );
    }

    let exe = repo.join("target").join("debug").join("hrw.exe");
    if binary_is_locked(&exe) {
        r.line(
            Verdict::Fail,
            "hrw.exe not locked",
            "HRW is running and holds the binary; the gate cannot relink it",
            "close HRW, or run the two gate targets separately (CLAUDE.md, Running things)",
        );
    } else {
        r.line(
            Verdict::Pass,
            "hrw.exe not locked",
            "the full gate can build the binary",
            "",
        );
    }

    match sleep_ruling() {
        SleepRuling::Never => r.line(
            Verdict::Pass,
            "stays awake",
            "neither sleep nor hibernate will suspend this machine",
            "",
        ),
        ruling @ SleepRuling::SuspendsOnAc { what, after_secs } => r.line(
            Verdict::Fail,
            "stays awake",
            &format!(
                "{what} after {}s while plugged in; a long run suspends mid-step and every \
                 timing it reports is wall-clock, so it will read as a hang",
                after_secs
            ),
            &fix_for(ruling).unwrap_or_default(),
        ),
        ruling @ SleepRuling::SuspendsOnBatteryOnly { after_secs } => r.line(
            Verdict::Warn,
            "stays awake",
            &format!("safe plugged in; on battery it sleeps after {after_secs}s"),
            &fix_for(ruling).unwrap_or_default(),
        ),
        SleepRuling::Unreadable => r.line(
            Verdict::Warn,
            "stays awake",
            "powercfg did not answer, so this machine's sleep settings are UNKNOWN -- \
             not confirmed safe",
            "run: powercfg /q SCHEME_CURRENT SUB_SLEEP",
        ),
    }

    // ------------------------------------------------------------- advisory --

    let cache = std::env::var("LOCALAPPDATA").map(|p| {
        PathBuf::from(p)
            .join("Rumoca")
            .join("source-roots")
            .join("parsed-files")
    });
    match cache {
        Ok(p) if p.exists() => r.line(
            Verdict::Pass,
            "parsed-artifact cache",
            "MSL parses are cached for this compiler fingerprint",
            "",
        ),
        _ => r.line(
            Verdict::Warn,
            "parsed-artifact cache",
            "absent, so the first compile re-parses the whole MSL",
            "nothing to do -- expect a slow first gate, and do not diagnose it as a hang",
        ),
    }

    let ext = std::env::var("USERPROFILE").map(|p| {
        PathBuf::from(p)
            .join(".vscode")
            .join("extensions")
            .join("dougdew64.hrw-debugger-bridge-0.1.0")
    });
    match ext {
        Ok(p) if p.exists() => r.line(
            Verdict::Pass,
            "VS Code bridge extension",
            "junction present",
            "",
        ),
        _ => r.line(
            Verdict::Warn,
            "VS Code bridge extension",
            "only matching-live.md needs it; the other tours run from HRW alone",
            "hrw/docs/setup-windows.md section 6 -- npm install, npm run build, then the junction",
        ),
    }

    let dirty = Command::new("git")
        .args([
            "-C",
            &repo.to_string_lossy(),
            "status",
            "--porcelain",
            "--untracked-files=all",
        ])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).lines().count());
    match dirty {
        Some(0) => r.line(Verdict::Pass, "working tree clean", "", ""),
        Some(n) => r.line(
            Verdict::Warn,
            "working tree clean",
            &format!("{n} uncommitted change(s)"),
            "commit or stash before an unattended run",
        ),
        None => r.line(
            Verdict::Warn,
            "working tree clean",
            "git could not answer -- not a checkout, or git is not on PATH",
            "",
        ),
    }

    // -------------------------------------------------------------- verdict --

    println!();
    if r.blocking > 0 {
        println!(
            "{} blocking problem(s). Fix before working, and before any unattended run.",
            r.blocking
        );
        std::process::exit(1);
    }
    if r.advisory > 0 {
        println!("Ready. {} advisory note(s) above.", r.advisory);
    } else {
        println!("Ready.");
    }
}
