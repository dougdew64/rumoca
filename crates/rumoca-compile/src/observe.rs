//! **Observing a compile that is already running.**
//!
//! A tool wrapping this crate wants the intermediate artifacts a compile passes
//! through — the instantiated overlay, the typechecked overlay — and the session
//! API returns only the end of the pipeline. `CompilationResult` carries `flat`
//! and `dae`; neither overlay survives into it. Faced with that, such a tool
//! **compiles the model a second time**, phase by phase, to see the middle.
//!
//! That is not merely wasteful. The artifacts it shows then come from a
//! *different execution* than the one that produced the result, and the tool
//! must separately prove the two were configured identically. Options drift; the
//! copies stop matching; nothing fails, and the tool quietly displays a compile
//! that never happened.
//!
//! # Why a capture scope rather than parameters
//!
//! These artifacts are produced a dozen stack frames below the public entry
//! points, inside functions whose signatures have nothing to do with
//! observability. Threading an `Option<&dyn Fn(..)>` through all of them to
//! reach two assignment sites is a large, invasive change in service of an
//! optional feature, and it would perturb every caller to benefit one.
//!
//! So: an opt-in, thread-local buffer, in the shape `tracing` established for
//! the same problem — and the same shape as
//! [`rumoca_phase_flatten::connections::trace`]. **The distinction that module
//! draws applies here too**: ambient state is the *delivery* mechanism, not a
//! second code path. There is one instantiate and one typecheck, the recording
//! happens inside them, and a closed scope is a thread-local read — so what is
//! captured is always what the compile produced. That is a different situation
//! from `rumoca-phase-structural`'s searches, where an observer *parameter* was
//! the right answer because the alternative kept a second traced copy of the
//! algorithm.
//!
//! # Cost
//!
//! One thread-local read per compile when no scope is open. **While open, the
//! overlay is cloned twice**, which is why this is opt-in and why a scope should
//! wrap one model rather than a corpus sweep. [`take_typed_model_capture`] both
//! drains and closes, so one model's artifacts can never appear under the next.

use rumoca_core::Diagnostic;
use rumoca_ir_ast as ast;

/// What a compile passed through, for a caller that asked to watch.
///
/// Every field is optional because a compile can stop at any point: an
/// instantiate failure leaves both `None`, a typecheck failure leaves
/// `instantiated` set and `typechecked` empty with the diagnostics recorded.
/// **The shape of the capture is itself a report of how far the compile got.**
#[derive(Debug, Default, Clone)]
pub struct TypedModelCapture {
    /// The overlay as instantiation produced it, before typecheck annotated it.
    pub instantiated: Option<ast::InstanceOverlay>,
    /// The overlay after typecheck — the same object, with resolved types and
    /// evaluated dimensions written into it.
    pub typechecked: Option<ast::InstanceOverlay>,
    /// Diagnostics from a failed typecheck, empty when it succeeded.
    pub typecheck_diagnostics: Vec<Diagnostic>,
}

thread_local! {
    static CAPTURE: std::cell::RefCell<Option<TypedModelCapture>> =
        const { std::cell::RefCell::new(None) };
}

/// Begin capturing the intermediate artifacts of compiles on this thread.
///
/// Scope one model. [`take_typed_model_capture`] drains and closes.
pub fn start_typed_model_capture() {
    CAPTURE.with(|c| *c.borrow_mut() = Some(TypedModelCapture::default()));
}

/// Take what was captured and close the scope.
///
/// Returns the default when no scope was open — the honest answer, since nothing
/// was requested and so nothing was recorded.
pub fn take_typed_model_capture() -> TypedModelCapture {
    CAPTURE.with(|c| c.borrow_mut().take()).unwrap_or_default()
}

pub(crate) fn record_instantiated(overlay: &ast::InstanceOverlay) {
    CAPTURE.with(|c| {
        if let Some(cap) = c.borrow_mut().as_mut() {
            cap.instantiated = Some(overlay.clone());
        }
    });
}

pub(crate) fn record_typechecked(overlay: &ast::InstanceOverlay) {
    CAPTURE.with(|c| {
        if let Some(cap) = c.borrow_mut().as_mut() {
            cap.typechecked = Some(overlay.clone());
        }
    });
}

pub(crate) fn record_typecheck_diagnostics(diags: &[Diagnostic]) {
    CAPTURE.with(|c| {
        if let Some(cap) = c.borrow_mut().as_mut() {
            cap.typecheck_diagnostics = diags.to_vec();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **A scope records, and taking it closes.**
    ///
    /// The second half matters as much as the first: without it, one model's
    /// overlay would still be sitting in the buffer when the next model is
    /// compiled, and a tool would show the wrong model's artifacts with nothing
    /// on screen to say so.
    #[test]
    fn a_capture_scope_records_and_closes_on_take() {
        // Closed: recording is a no-op and taking yields the empty capture.
        record_instantiated(&ast::InstanceOverlay::default());
        let none = take_typed_model_capture();
        assert!(
            none.instantiated.is_none(),
            "no scope was open, so nothing was recorded"
        );

        start_typed_model_capture();
        record_instantiated(&ast::InstanceOverlay::default());
        record_typechecked(&ast::InstanceOverlay::default());
        let got = take_typed_model_capture();
        assert!(
            got.instantiated.is_some(),
            "the instantiated overlay was captured"
        );
        assert!(got.typechecked.is_some(), "and the typechecked one");

        // Closed again by the take.
        record_instantiated(&ast::InstanceOverlay::default());
        assert!(
            take_typed_model_capture().instantiated.is_none(),
            "capture continued after take \u{2014} one model's overlay would appear \
             under the next model's tabs",
        );
    }

    /// **The must-fire test: a real compile must record.**
    ///
    /// Everything else here calls `record_*` directly, so it all passes with
    /// both call sites in `session::compile_support` deleted — an observer that
    /// records nothing is indistinguishable from a compile that produced
    /// nothing. This one opens a scope around an actual compile.
    ///
    /// It also pins the property the two fields exist for: typecheck *mutates*
    /// the overlay it was handed, so `instantiated` and `typechecked` are two
    /// genuinely different artifacts rather than one object seen twice. If they
    /// were the same, a tool's Instantiate tab would be showing typecheck's
    /// output and nothing would say so.
    #[test]
    fn a_real_compile_records_both_overlays() {
        let source = r#"
            model Observed
              parameter Integer n = 3;
              Real x[n](each start = 1, each fixed = true);
            equation
              der(x) = -x;
            end Observed;
        "#;
        let mut session = crate::Session::default();
        session
            .add_document("observed.mo", source)
            .expect("document should parse");

        start_typed_model_capture();
        let report = session.compile_model_strict_reachable_uncached_with_recovery("Observed");
        let capture = take_typed_model_capture();
        assert!(
            report.requested_succeeded(),
            "the fixture must compile, or this test proves nothing: {:?}",
            report.requested_result
        );

        let instantiated = capture
            .instantiated
            .as_ref()
            .expect("instantiation ran, so its overlay must be recorded");
        let typechecked = capture
            .typechecked
            .as_ref()
            .expect("typecheck ran, so its overlay must be recorded");
        assert!(
            capture.typecheck_diagnostics.is_empty(),
            "a successful typecheck records no diagnostics: {:?}",
            capture.typecheck_diagnostics
        );
        // Both must describe the model that was asked for.
        assert!(
            !instantiated.classes.is_empty(),
            "the instantiated overlay must hold the model's classes"
        );
        // The two are captured either side of a mutation, so a tool showing
        // them as separate phases is telling the truth.
        // **The two overlays are genuinely different artifacts, and this is
        // where it shows.** `x[n]` carries its dimension as an unevaluated
        // expression out of instantiation; typecheck evaluates `n` and writes
        // `dims = [3]` into the same object. So the capture points bracket a
        // real mutation, and a tool showing an Instantiate tab beside a
        // Typecheck tab is showing two states rather than one object twice.
        //
        // This is what makes the test more than a presence check: recording
        // the *same* overlay at both points would satisfy every assertion
        // above and fail here.
        let dim_of = |o: &ast::InstanceOverlay, name: &str| -> Option<Vec<i64>> {
            o.components
                .values()
                .find(|c| c.qualified_name.to_string() == name)
                .map(|c| c.dims.clone())
        };
        assert_eq!(
            dim_of(instantiated, "x"),
            Some(Vec::new()),
            "out of instantiation, x[n]'s dimension is still an expression"
        );
        assert_eq!(
            dim_of(typechecked, "x"),
            Some(vec![3]),
            "typecheck evaluates n and writes the dimension in"
        );
    }

    /// A compile that stops early leaves a capture shaped like where it stopped.
    #[test]
    fn a_partial_compile_leaves_a_partial_capture() {
        start_typed_model_capture();
        record_instantiated(&ast::InstanceOverlay::default());
        // Typecheck failed: no typechecked overlay, but the diagnostics are kept.
        record_typecheck_diagnostics(&[]);
        let got = take_typed_model_capture();
        assert!(got.instantiated.is_some());
        assert!(
            got.typechecked.is_none(),
            "typecheck did not complete, and the capture must not imply that it did",
        );
    }
}
