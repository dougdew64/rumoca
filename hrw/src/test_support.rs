//! Shared helpers for tests that need a *real* compiled specimen.
//!
//! Most HRW tests are better off with hand-built frames: they are fast, they
//! pin exact wording, and they can construct situations no specimen produces.
//! But a view that reconstructs compiler state from a DAE — tearing walks the
//! BLT blocks, the IC-plan view reads the initialization plan — has a failure
//! mode hand-built frames cannot catch: the *reconstruction* can be wired up
//! wrongly (wrong index space, wrong matching direction) and every unit test
//! still passes. One end-to-end test per such view closes that gap.
//!
//! These helpers deliberately return `Option`: a checkout without the specimen
//! (or a specimen that stops compiling) should skip the test rather than fail
//! it, since the thing under test is the view, not the specimen.

use std::path::PathBuf;

use rumoca_compile::compile::PhaseResult;
use rumoca_compile::{Session, SessionConfig};

/// The specimen directory, resolved at compile time relative to this crate.
fn specimen_dir() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/specimens"))
}

/// Compile `specimens/<model>.mo` and return the model's DAE.
///
/// Returns `None` if the file is missing or the pipeline did not reach a full
/// success — see the module note on why that is a skip and not a failure.
///
/// Uses the **uncached** entry point for the same reason the worker does: the
/// cached one returns a previous `PhaseResult` without running the phases, and
/// a test whose subject is "the phase produced this" must actually run it.
pub fn dae_for(model: &str) -> Option<rumoca_ir_dae::Dae> {
    let path = specimen_dir().join(format!("{model}.mo"));
    let source = std::fs::read_to_string(&path).ok()?;

    let mut session = Session::new(SessionConfig::default());
    let uri = format!("file:///{}", path.display().to_string().replace('\\', "/"));
    session.update_document(&uri, &source);
    let qualified = session.qualify_model_name(&uri, model);
    let report = session.compile_model_strict_reachable_uncached_with_recovery(&qualified);

    match report.requested_result? {
        PhaseResult::Success(cr) => Some(cr.dae),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    /// The helper itself is load-bearing for other tests' meaning: if it
    /// silently returned `None` everywhere, those tests would pass by skipping.
    /// This one asserts a known-good specimen really does compile.
    #[test]
    fn a_known_specimen_compiles_to_a_dae() {
        let dae = super::dae_for("ProportionalLoop")
            .expect("ProportionalLoop is a checked-in specimen and must compile");
        assert!(!dae.continuous.equations.is_empty());
    }
}
