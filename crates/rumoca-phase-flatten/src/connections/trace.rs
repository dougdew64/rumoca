//! Observation hooks for connection expansion (MLS §9).
//!
//! Additive and observation-only: nothing here changes what the phase
//! computes. When no observer is attached the cost is one `Option` check per
//! connection set.
//!
//! # What the frames are for
//!
//! The flat model's equation count is far larger than the number of equations
//! anyone wrote, and connection expansion is where most of the difference comes
//! from. The *result* — a `flat::Model` full of `EquationOrigin::Connection`
//! and `EquationOrigin::FlowSum` rows — shows the equations but not the rule
//! that produced them, and the rule is the interesting part:
//!
//! - a **potential** set of *n* variables yields *n − 1* equality equations
//!   (`v1 = v2 = … = vn` written as a chain), while
//! - a **flow** set of the same *n* variables yields exactly **one** equation,
//!   the sum-to-zero (MLS §9.2 — Kirchhoff's current law).
//!
//! That asymmetry is the single most useful thing to see here, and it is
//! visible only if you can watch each set produce its equations. The frames
//! also expose that connection *sets* are transitive: `connect(a, b)` and
//! `connect(b, c)` form one set of three, not two sets of two, because the sets
//! are built by union-find.
//!
//! # Granularity
//!
//! Frames are emitted per connection **set**, not per union-find merge. The
//! merges happen several call levels below `process_connections`
//! (`process_connection` → `connect_primitive_vars` → …), and threading an
//! observer down there would touch six functions to show a data structure that
//! the set membership already reports. Per-merge frames remain a possible
//! refinement if the set view turns out to leave a question unanswered.

use rumoca_core::FrameObserver;

/// One step of connection expansion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionStep {
    /// The pass is starting, with the number of `connect()` statements it
    /// collected (after skipping ones involving disabled components).
    Start { connect_statements: usize },
    /// A connection set has been built: these variables are connected to one
    /// another, transitively, at this scope.
    SetFormed {
        /// `"flow"`, `"potential"` or `"stream"`.
        kind: &'static str,
        /// The hierarchy level the `connect()` was declared at; empty = root.
        scope: String,
        variables: Vec<String>,
    },
    /// Equations were generated for the set just formed.
    ///
    /// `equations_added` is measured, not predicted — the difference in the
    /// model's equation count across the generating call — so it stays honest
    /// if array scalarization makes one logical equation into several.
    EquationsGenerated {
        kind: &'static str,
        set_size: usize,
        equations_added: usize,
    },
    /// A flow variable in no connection set: MLS §9.2 gives it `f = 0`.
    UnconnectedFlow { equations_added: usize },
    /// The pass is finished.
    Complete {
        sets: usize,
        /// Every equation this pass added to the model.
        equations_added: usize,
    },
}

/// One frame of the connection-expansion trace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionFrame {
    pub step: ConnectionStep,
    /// Connection sets completed so far — the running count.
    pub sets_so_far: usize,
    /// Equations this pass has added so far.
    pub equations_so_far: usize,
}

/// Emit one frame if an observer is attached.
///
/// Free function rather than a method so the call sites read as a single line
/// and the `Option` check stays visible at each one.
pub(crate) fn emit(
    observer: Option<FrameObserver<'_, ConnectionFrame>>,
    step: ConnectionStep,
    sets_so_far: usize,
    equations_so_far: usize,
) {
    if let Some(obs) = observer {
        obs(&ConnectionFrame { step, sets_so_far, equations_so_far });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// With no observer the emit call is a no-op; with one, the frame arrives
    /// carrying the running counts. Those counts are the reason the frame type
    /// exists — a step alone cannot say how far the pass has got.
    #[test]
    fn emit_is_a_no_op_without_an_observer_and_delivers_with_one() {
        emit(None, ConnectionStep::Start { connect_statements: 4 }, 0, 0);

        let seen = RefCell::new(Vec::new());
        let sink = |f: &ConnectionFrame| seen.borrow_mut().push(f.clone());
        emit(
            Some(&sink),
            ConnectionStep::EquationsGenerated {
                kind: "potential",
                set_size: 3,
                equations_added: 2,
            },
            1,
            2,
        );

        let seen = seen.into_inner();
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].sets_so_far, 1);
        assert_eq!(seen[0].equations_so_far, 2);
        assert!(matches!(
            seen[0].step,
            ConnectionStep::EquationsGenerated { set_size: 3, equations_added: 2, .. },
        ));
    }
}
