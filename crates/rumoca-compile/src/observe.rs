//! **Observing a compile that is already running.**
//!
//! A tool wrapping this crate wants the intermediate artifacts a compile passes
//! through — the instantiated overlay, the typechecked overlay — and the session
//! API returns only the end of the pipeline. Faced with that, such a tool
//! **compiles the model a second time**, phase by phase, to see the middle.
//!
//! That is not merely wasteful. The artifacts it shows then come from a *different
//! execution* than the one that produced the result, and the tool must separately
//! prove the two were configured identically. Options drift; the copies stop
//! matching; nothing fails, and the tool quietly displays a compile that never
//! happened.
//!
//! # Why a capture scope rather than parameters
//!
//! These artifacts are produced a dozen stack frames below the public entry
//! points, inside functions whose signatures have nothing to do with
//! observability. Threading an `Option<&dyn Fn(..)>` through all of them to reach
//! two assignment sites is a large, invasive change in service of an optional
//! feature, and it would perturb every caller to benefit one.
//!
//! So: an opt-in, thread-local buffer, in the shape `tracing` established for the
//! same problem — and the same shape as the frame captures in
//! `rumoca-phase-flatten` and `rumoca-phase-dae`.
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
        assert!(none.instantiated.is_none(), "no scope was open, so nothing was recorded");

        start_typed_model_capture();
        record_instantiated(&ast::InstanceOverlay::default());
        record_typechecked(&ast::InstanceOverlay::default());
        let got = take_typed_model_capture();
        assert!(got.instantiated.is_some(), "the instantiated overlay was captured");
        assert!(got.typechecked.is_some(), "and the typechecked one");

        // Closed again by the take.
        record_instantiated(&ast::InstanceOverlay::default());
        assert!(
            take_typed_model_capture().instantiated.is_none(),
            "capture continued after take \u{2014} one model's overlay would appear \
             under the next model's tabs",
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
