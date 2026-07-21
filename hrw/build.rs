//! Build script: expose the Rumoca version + commit to the app at compile time.
//! HRW now lives *inside* the Rumoca workspace (path deps), so the authoritative
//! commit is the workspace's own git HEAD — the exact Rumoca source being built.
//! The version comes from `rumoca-compile`'s entry in `Cargo.lock`. The Help/About
//! dialog shows both via `env!(...)`, so they can't drift. (Before the in-workspace
//! move, the rev came from the `git+…#<sha>` source line of the pinned dependency.)

use std::fs;
use std::process::Command;

fn main() {
    // Re-run when the lock changes (version) or HEAD moves (commit). In a
    // workspace only the ROOT Cargo.lock is maintained, so read that (../).
    println!("cargo:rerun-if-changed=../Cargo.lock");
    println!("cargo:rerun-if-changed=../.git/HEAD");
    let version = read_rumoca_version();
    let rev = git_head_short().unwrap_or_else(|| String::from("unknown"));
    println!("cargo:rustc-env=HRW_RUMOCA_VERSION={version}");
    println!("cargo:rustc-env=HRW_RUMOCA_REV={rev}");
}

/// `rumoca-compile`'s resolved semver version from the workspace-root `Cargo.lock`.
/// Falls back to "unknown" so `env!` in the app always resolves.
fn read_rumoca_version() -> String {
    let lock = fs::read_to_string("../Cargo.lock").unwrap_or_default();
    for block in lock.split("[[package]]") {
        if block.lines().any(|l| l.trim() == "name = \"rumoca-compile\"") {
            for line in block.lines() {
                if let Some(v) =
                    line.trim().strip_prefix("version = \"").and_then(|s| s.strip_suffix('"'))
                {
                    return v.to_owned();
                }
            }
        }
    }
    String::from("unknown")
}

/// Short commit of the workspace HEAD — the Rumoca source HRW is built against,
/// now that HRW is an in-workspace member. `None` outside a git checkout.
fn git_head_short() -> Option<String> {
    let out = Command::new("git").args(["rev-parse", "--short", "HEAD"]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let rev = String::from_utf8(out.stdout).ok()?.trim().to_owned();
    (!rev.is_empty()).then_some(rev)
}
