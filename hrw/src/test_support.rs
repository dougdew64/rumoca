//! Shared helpers for tests that need a *real* compiled specimen.
//!
//! Most HRW tests are better off with hand-built frames: they are fast, they
//! pin exact wording, and they can construct situations no specimen produces.
//! But a view that reconstructs compiler state from a DAE — tearing runs the
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

/// The scratch probe's filename and source, shared so three tests agree on it.
///
/// `worker` asserts this model has exactly **one state**, which is a property of
/// *this source*; a test that wrote its own would drift from that assertion silently.
pub(crate) const SCRATCH_PROBE_NAME: &str = "ScratchProbe.mo";

/// Source for [`SCRATCH_PROBE_NAME`] — the smallest thing that shows a first-order lag.
pub(crate) const SCRATCH_PROBE_SOURCE: &str = "\
model ScratchProbe \"Smallest thing that shows a first-order lag\"
  // A scratch probe: written to answer one question, not part of the corpus.
  parameter Real tau = 0.5;
  Real x(start = 1);
equation
  tau * der(x) = -x;
end ScratchProbe;
";

/// A scratch specimen that exists for the duration of a test and **leaves
/// `.hrw-bridge/specimens/` exactly as it was found, including on a panic.**
///
/// # Why this exists rather than a write at the top and a remove at the bottom
///
/// Three tests used to depend on `ScratchProbe.mo` *happening* to be on disk and
/// returned early when it was not, so **in a clean checkout they asserted nothing and
/// said so to nobody** — the must-fire rule pointed at tests rather than at production
/// code. The obvious fix, writing the file in the test, is how the three
/// `.hrw-bridge/lab.md` defects happened (`CLAUDE.md`): *"a test that wrote its own
/// and **deleted Doug's** afterwards"*.
///
/// **That directory is live state.** Doug runs HRW from the working tree while the
/// suite runs, and Claude writes probes into it mid-conversation to answer questions —
/// two of them are sitting there as of 2026-08-22. So the contract is the one
/// [`crate::ui_tests::AdHocLab`] already proved for the lab file: **save what was
/// there, and put it back in `Drop`** so a failing assertion cannot poison the
/// directory for the next run or for the app Doug has open.
///
/// The shadow test is the sharp case: it writes `BouncingBall.mo`, and leaving that
/// behind would shadow a curated specimen — the *"makes Claude guess"* failure that
/// test exists to prevent.
pub(crate) struct ScratchSpecimen {
    path: PathBuf,
    saved: Option<String>,
}

impl ScratchSpecimen {
    /// The standard probe: [`SCRATCH_PROBE_NAME`] with [`SCRATCH_PROBE_SOURCE`].
    pub(crate) fn probe() -> Self {
        Self::with(SCRATCH_PROBE_NAME, SCRATCH_PROBE_SOURCE)
    }

    /// A scratch specimen named `name`, holding `source`, for this scope.
    ///
    /// Panics rather than skipping if the directory or the file cannot be written:
    /// **a test that cannot establish its precondition has failed, not passed**, which
    /// is the whole defect this type was introduced to remove.
    pub(crate) fn with(name: &str, source: &str) -> Self {
        let dir = PathBuf::from(crate::bridge::SCRATCH_SPECIMEN_DIR);
        std::fs::create_dir_all(&dir).expect("scratch specimen dir must be creatable");
        let path = dir.join(name);
        let saved = std::fs::read_to_string(&path).ok();
        std::fs::write(&path, source).expect("scratch specimen must be writable");
        Self { path, saved }
    }

    /// Where the specimen was written.
    pub(crate) fn path(&self) -> &std::path::Path {
        &self.path
    }
}

impl Drop for ScratchSpecimen {
    fn drop(&mut self) {
        match self.saved.take() {
            // Restore byte-for-byte: the file may be one Claude wrote to answer a
            // question, and Doug may be looking at it in the running app.
            Some(text) => {
                let _ = std::fs::write(&self.path, text);
            }
            None => {
                let _ = std::fs::remove_file(&self.path);
            }
        }
    }
}

/// **The guard's own job is restoration, so restoration is what gets tested.**
///
/// Without these, a guard that quietly failed to put a file back would leave every
/// test that uses it green while poisoning the directory — the exact shape of the
/// lab-file defects it was written to avoid.
#[cfg(test)]
mod tests_scratch_specimen {
    use super::{SCRATCH_PROBE_NAME, ScratchSpecimen};

    #[test]
    fn a_file_that_did_not_exist_is_removed_again() {
        let name = "GuardProbeAbsent.mo";
        let path = std::path::Path::new(crate::bridge::SCRATCH_SPECIMEN_DIR).join(name);
        let _ = std::fs::remove_file(&path);
        {
            let guard = ScratchSpecimen::with(name, "model G end G;\n");
            assert!(guard.path().exists(), "the guard must create the file");
        }
        assert!(
            !path.exists(),
            "and remove it again when it did not exist before"
        );
    }

    #[test]
    fn a_file_that_existed_is_restored_byte_for_byte() {
        let name = "GuardProbePresent.mo";
        let path = std::path::Path::new(crate::bridge::SCRATCH_SPECIMEN_DIR).join(name);
        std::fs::create_dir_all(path.parent().expect("has a parent")).expect("dir");
        let original = "model G \"the one Doug was looking at\" end G;\n";
        std::fs::write(&path, original).expect("seed the pre-existing file");
        {
            let _guard = ScratchSpecimen::with(name, "model G \"overwritten\" end G;\n");
            assert_ne!(
                std::fs::read_to_string(&path).expect("readable"),
                original,
                "the guard must actually overwrite, or the restore proves nothing",
            );
        }
        assert_eq!(
            std::fs::read_to_string(&path).expect("readable"),
            original,
            "a pre-existing scratch file must come back exactly as it was",
        );
        let _ = std::fs::remove_file(&path);
    }

    /// A panic inside the scope must still restore — `Drop` runs while unwinding, and
    /// that is the property the "remove at the end of the test" form did not have.
    #[test]
    fn a_panic_inside_the_scope_still_restores() {
        let path =
            std::path::Path::new(crate::bridge::SCRATCH_SPECIMEN_DIR).join(SCRATCH_PROBE_NAME);
        let before = std::fs::read_to_string(&path).ok();
        // **Recorded from inside the scope**, because "unchanged before and after" is
        // also true of a guard that never wrote anything — a hole this test had until
        // a deliberate break exposed it. Now the restore claim rests on the file having
        // genuinely existed while the guard was alive.
        let existed_inside = std::sync::atomic::AtomicBool::new(false);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let guard = ScratchSpecimen::probe();
            existed_inside.store(guard.path().exists(), std::sync::atomic::Ordering::SeqCst);
            panic!("a failing assertion, as far as Drop is concerned");
        }));
        assert!(result.is_err(), "the fixture must actually panic");
        assert!(
            existed_inside.load(std::sync::atomic::Ordering::SeqCst),
            "the guard must have created the file inside the scope, or restoring it \
             afterwards proves nothing",
        );
        assert_eq!(
            std::fs::read_to_string(&path).ok(),
            before,
            "unwinding must leave the directory as it was found",
        );
    }
}
