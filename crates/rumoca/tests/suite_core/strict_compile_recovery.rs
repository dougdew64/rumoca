//! `Session::strict_compile_resolved` returns a tree *and* its diagnostics, and
//! the `Ok` is not a success signal.
//!
//! Recovery mode (`ResolveBuildMode::StrictCompileRecovery`) is built to produce
//! a usable tree past errors that `Standard` treats as fatal. That is what makes
//! it the right mode for a caller that wants to inspect a resolution and then
//! compile from it — one resolution serves both — but it means an `Err` arrives
//! only when the library cannot be resolved *at all*.
//!
//! **A caller that reads the `Ok` as "resolved cleanly" stops detecting failures
//! entirely**, and silently: the tree it gets back is real and the model it
//! wanted may be missing from it. The diagnostics are the signal, and
//! [`StrictCompileTree::is_complete`] is the same question answered structurally.
//!
//! Both are asserted here rather than described, because the hazard is invisible
//! on a model that happens to resolve.

use rumoca_compile::compile::{Session, SessionConfig};
use rumoca_core::DiagnosticSeverity;

/// A model whose only defect is a reference to something that does not exist.
const UNRESOLVED: &str = "model U Real y; equation y = missingGain * time; end U;";

/// The same shape with nothing missing.
const CLEAN: &str = "model C Real y; equation y = 2 * time; end C;";

#[test]
fn recovery_returns_ok_and_reports_the_error_in_its_diagnostics() {
    let mut session = Session::new(SessionConfig::default());
    session
        .add_document("unresolved.mo", UNRESOLVED)
        .expect("fixture parses");

    let (tree, diagnostics) = session
        .strict_compile_resolved()
        .expect("recovery produces a tree even though the model does not resolve");

    // The `Ok` says only that a tree was built.
    assert!(
        !tree.is_complete(),
        "resolve did not complete, and `is_complete` is how a caller learns that \
         without reading diagnostics",
    );

    let errors: Vec<&str> = diagnostics
        .iter()
        .filter(|d| d.severity == DiagnosticSeverity::Error)
        .map(|d| d.message.as_str())
        .collect();
    assert!(
        errors.iter().any(|m| m.contains("missingGain")),
        "the unresolved reference must be reported in the diagnostics, since the \
         `Result` cannot carry it: {errors:?}",
    );
}

#[test]
fn a_clean_model_resolves_completely_and_reports_no_errors() {
    let mut session = Session::new(SessionConfig::default());
    session
        .add_document("clean.mo", CLEAN)
        .expect("fixture parses");

    let (tree, diagnostics) = session
        .strict_compile_resolved()
        .expect("a clean model resolves");

    assert!(
        tree.is_complete(),
        "nothing in this model is unresolved, so resolve completed",
    );
    let errors: Vec<&str> = diagnostics
        .iter()
        .filter(|d| d.severity == DiagnosticSeverity::Error)
        .map(|d| d.message.as_str())
        .collect();
    assert!(
        errors.is_empty(),
        "a clean model must report no errors, or the test above proves nothing: \
         {errors:?}",
    );
}

/// **Every error diagnostic says which source it is about.**
///
/// This is what lets a caller holding several documents attribute an error to
/// one of them. Without it, the only available question is "did anything in the
/// session fail", which is a different question and has a different answer: a
/// consumer that asked it charged a library's errors to whichever model it was
/// compiling, and reported that model's resolve as failed when it had not.
#[test]
fn an_error_names_the_source_it_came_from() {
    let mut session = Session::new(SessionConfig::default());
    session
        .add_document("unresolved.mo", UNRESOLVED)
        .expect("fixture parses");
    session
        .add_document("clean.mo", CLEAN)
        .expect("fixture parses");

    let (_tree, diagnostics) = session
        .strict_compile_resolved()
        .expect("recovery produces a tree");

    let placed = diagnostics
        .iter()
        .filter(|d| d.severity == DiagnosticSeverity::Error)
        .filter(|d| {
            d.labels
                .iter()
                .find(|l| l.primary)
                .or_else(|| d.labels.first())
                .is_some()
        })
        .count();
    let total = diagnostics
        .iter()
        .filter(|d| d.severity == DiagnosticSeverity::Error)
        .count();

    assert!(total > 0, "the unresolved model must produce an error");
    assert_eq!(
        placed, total,
        "every error must carry a label, since the label's span is the only thing \
         that says which document it belongs to",
    );
}
