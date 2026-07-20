//! Build script: expose the *pinned Rumoca version* to the app at compile time,
//! read from `Cargo.lock`. The Help/About dialog shows it via `env!(...)`, so it
//! always reflects what HRW was actually built against and can never drift — no
//! manual step when the pin is bumped (see `docs/updating-rumoca.md`).

use std::fs;

fn main() {
    // Re-run whenever the lock changes (i.e. when the Rumoca pin is bumped).
    println!("cargo:rerun-if-changed=Cargo.lock");
    let (version, rev) = read_rumoca_pin();
    println!("cargo:rustc-env=HRW_RUMOCA_VERSION={version}");
    println!("cargo:rustc-env=HRW_RUMOCA_REV={rev}");
}

/// `rumoca-compile`'s resolved semver version and short git commit from
/// `Cargo.lock`. Falls back to "unknown" so `env!` in the app always resolves.
fn read_rumoca_pin() -> (String, String) {
    let lock = fs::read_to_string("Cargo.lock").unwrap_or_default();
    for block in lock.split("[[package]]") {
        if block.lines().any(|l| l.trim() == "name = \"rumoca-compile\"") {
            let mut version = String::from("unknown");
            let mut rev = String::from("unknown");
            for line in block.lines() {
                let line = line.trim();
                if let Some(v) = line.strip_prefix("version = \"").and_then(|s| s.strip_suffix('"')) {
                    version = v.to_owned();
                } else if let Some(src) = line.strip_prefix("source = \"").and_then(|s| s.strip_suffix('"')) {
                    // git+…?rev=<sha>#<sha> — the commit is after the '#'.
                    if let Some(sha) = src.rsplit('#').next().filter(|s| !s.is_empty()) {
                        rev = sha.chars().take(9).collect();
                    }
                }
            }
            return (version, rev);
        }
    }
    (String::from("unknown"), String::from("unknown"))
}
