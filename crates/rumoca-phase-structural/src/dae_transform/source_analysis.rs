//! The source system's structural analysis, built once per preparation.

use rumoca_ir_dae as dae;

use super::{PreparedDae, SourceRound, analyse_structure_capturing, prepare_with_source_round};
use crate::StructuralError;
use crate::preparation_trace::AnalysisContext;

/// The structural analysis (scalar incidence, matching, singularity, BLT) of
/// one source system. State selection decides an early loop-closure reduction
/// from its singularity verdict and then hands it to the reducer as its first
/// round, so the source system is analyzed once (SPEC_0053 §1).
pub struct SourceStructuralAnalysis<'source> {
    model: &'source dae::Dae,
    round: SourceRound,
}

impl<'source> SourceStructuralAnalysis<'source> {
    /// Analyze `model` once.
    #[must_use]
    pub fn of(model: &'source dae::Dae) -> Self {
        Self {
            model,
            round: analyse_structure_capturing(model, None, None, AnalysisContext::Source),
        }
    }

    /// The analyzed source system.
    #[must_use]
    pub const fn model(&self) -> &'source dae::Dae {
        self.model
    }

    /// Whether the source system is structurally singular.
    #[must_use]
    pub fn is_singular(&self) -> bool {
        matches!(self.round.0, Err(StructuralError::Singular { .. }))
    }
}

/// [`super::prepare_for_solve`] of the analyzed source, reusing its analysis
/// as the reducer's first round.
///
/// This is the solve path's reduction, so its records go to an open
/// [`crate::preparation_trace`] scope.
pub fn prepare_for_solve_from_source(
    source: SourceStructuralAnalysis<'_>,
) -> Result<PreparedDae<'_>, StructuralError> {
    prepare_with_source_round(
        source.model,
        Some(source.round),
        &mut super::observation::AmbientReduction,
    )
}
