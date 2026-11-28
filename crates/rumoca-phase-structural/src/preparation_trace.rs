//! Observation of structural preparation **as it runs inside a real solve**.
//!
//! Additive and observation-only: nothing here changes what preparation
//! computes. When no scope is open the cost is one thread-local read per
//! structural analysis and per reduction event.
//!
//! # Why this exists beside `inspect_prepare_for_solve`
//!
//! [`crate::inspect_prepare_for_solve`] and [`crate::inspect_structural_report`]
//! observe a call **the caller makes itself**. That answers "what would
//! preparation do to this DAE", but the solve path does not prepare the DAE it
//! is handed: `rumoca-phase-solve` first folds evaluable parameters, inlines
//! annotated calls, quotients aliases and folds constants, and then prepares
//! the result — and it analyses the system again for every reduction round,
//! alternate chart, loop-guard rebuild and formal quotient. A tool that calls
//! the inspect functions on the untransformed DAE therefore shows a different
//! execution from the one that produced the simulation, and for a model the
//! transforms change it can show different numbers.
//!
//! An opt-in thread-local buffer, in the shape `rumoca-phase-flatten`'s
//! connection trace already uses, lets a tool wrapping the real solve entry
//! observe **the preparation that actually happened**: every structural
//! analysis it ran, labelled by why, and every reduction record, in execution
//! order. No signature on the solve path moves and no second run exists — the
//! analyses below are the ones whose results preparation went on to use.
//!
//! # Fidelity
//!
//! A captured analysis runs the same `sort_from_incidence_with_observer` body
//! the uncaptured one does, with a recorder in place of the no-op observer;
//! the crate's structural seam already guarantees those are one execution.
//! Reduction records come from the same `record_of` conversion
//! [`crate::inspect_prepare_for_solve`] uses.
//!
//! Debug-build self-checks (`candidate_choice::check_deferred` and its holonomic
//! twin) re-attempt candidates to verify a bound. They are not decisions the
//! run makes and do not exist in release builds, so they are not recorded.
//!
//! # Cost
//!
//! While open, every analysis's matching, Tarjan and tearing steps are kept,
//! so a scope should wrap **one model's** preparation. [`take_capture`] both
//! drains and closes.

use std::cell::RefCell;

use rumoca_ir_dae as dae;

use crate::{
    AliasQuotientReport, AliasQuotientScope, Incidence, ReductionRecord, SortedDae,
    StructuralError, StructuralReport, StructuralTrace,
};

/// Why preparation ran one structural analysis.
///
/// Each variant names one call site; together they are every place structural
/// preparation analyses a system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalysisContext {
    /// The first analysis of the system preparation was given.
    Source,
    /// The first analysis after requested states (`StateSelect`) rebuilt it.
    RequestedStates,
    /// A direct-demotion candidate, tried on the rebuilt system.
    DirectCandidate,
    /// The opening analysis of a holonomic reduction round.
    HolonomicRound,
    /// The same round after tensor derivative aliases were normalized.
    NormalizedTensors,
    /// The base incidence a holonomic pass reuses for its candidates.
    HolonomicPass,
    /// A holonomic candidate, tried on the rebuilt system.
    HolonomicCandidate,
    /// A state candidate built from formal derivatives.
    StateCandidate,
    /// The candidate after `InlineAfterIndexReduction` calls were inlined.
    FormalInlinedCalls,
    /// The candidate after the formal-derivative alias quotient.
    FormalAliasQuotient,
    /// A system after derivative aliases were rewritten.
    DerivativeAliases,
    /// A system after loop-guarded relations gained event owners.
    LoopGuards,
}

/// The incidence one analysis matched over, detached from the system it was
/// built on.
///
/// Labels are this crate's own [`crate::equation_label`] and
/// [`crate::unknown_label`], the strings [`StructuralReport`] uses, so a
/// matching and its incidence correlate by name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedIncidence {
    /// Every unknown, in column order.
    pub unknowns: Vec<String>,
    /// Every scalar row, in row order.
    pub rows: Vec<CapturedIncidenceRow>,
}

/// One scalar row of a [`CapturedIncidence`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedIncidenceRow {
    /// The bare equation reference, `f_x[i]`.
    pub id: String,
    /// [`crate::equation_label`] of the row.
    pub label: String,
    /// The installed renderer's text for the equation, if one is installed
    /// (see [`set_equation_renderer`]).
    pub text: Option<String>,
    /// Column indices of the unknowns this row mentions, ascending.
    pub unknowns: Vec<usize>,
}

/// One observed step of structural preparation.
#[derive(Debug)]
pub enum PreparationEvent {
    /// One structural analysis — matching, BLT and tearing — and its result.
    StructuralAnalysis {
        /// Which analysis of the preparation this was.
        context: AnalysisContext,
        /// The incidence the analysis matched over; `None` when building it
        /// failed, which is then the analysis's error.
        incidence: Option<CapturedIncidence>,
        /// The report on success; the analysis's own error otherwise.
        outcome: Result<StructuralReport, StructuralError>,
        /// Every matching, Tarjan and tearing step this analysis took.
        trace: StructuralTrace,
    },
    /// One reduction record, exactly as [`crate::inspect_prepare_for_solve`]
    /// reports it.
    Reduction(ReductionRecord),
    /// An alias quotient ran: the classes it found, quotiented and unchanged.
    AliasQuotient {
        /// `Source` before state selection, `FormalDerivatives` after it.
        scope: AliasQuotientScope,
        /// Every class the quotient considered: those it collapsed, and those it
        /// left unchanged with the reason.
        report: AliasQuotientReport,
    },
}

thread_local! {
    /// Ambient capture buffer: `Some` while a capture scope is open.
    static CAPTURE: RefCell<Option<Vec<PreparationEvent>>> = const { RefCell::new(None) };
}

/// Renders one continuous equation of the system being analysed.
pub type EquationRenderer = Box<dyn Fn(dae::DaeView<'_>, usize) -> Option<String>>;

thread_local! {
    /// Optional renderer for captured incidence rows.
    static RENDERER: RefCell<Option<EquationRenderer>> = const { RefCell::new(None) };
}

/// **Begin capturing structural preparation on this thread.** Nothing is
/// recorded until this is called; a scope already open is restarted empty.
pub fn start_capture() {
    CAPTURE.with(|capture| *capture.borrow_mut() = Some(Vec::new()));
}

/// Install (or with `None`, remove) a renderer for captured incidence rows.
///
/// This crate renders no equations: the systems preparation analyses are
/// rebuilt DAEs that exist only during the run, so a tool wanting their text
/// must render them while they exist. The renderer is called with the analysed
/// system's view and a continuous-equation index, only while a scope is open,
/// and its result is stored as [`CapturedIncidenceRow::text`]. It observes and
/// cannot change the analysis.
pub fn set_equation_renderer(renderer: Option<EquationRenderer>) {
    RENDERER.with(|slot| *slot.borrow_mut() = renderer);
}

/// Take the captured events and close the scope.
///
/// Returns empty if no scope was open, which is the honest answer: nothing was
/// requested, so nothing was recorded.
pub fn take_capture() -> Vec<PreparationEvent> {
    CAPTURE
        .with(|capture| capture.borrow_mut().take())
        .unwrap_or_default()
}

/// Whether a scope is open, so a caller can skip building a record nobody
/// will read.
pub(crate) fn capturing() -> bool {
    CAPTURE.with(|capture| capture.borrow().is_some())
}

/// One step of preparation, handed to a live listener the moment it happens.
///
/// The captured [`PreparationEvent`]s arrive only when each analysis has
/// finished. A tool *watching* the run — paused at a breakpoint inside
/// matching, say — needs each step as it is taken, so this carries the same
/// records one at a time, with the boundaries that tell analyses apart.
#[derive(Debug)]
pub enum LiveStep<'a> {
    /// An analysis is starting; every [`LiveStep::Analysis`] until the next
    /// `AnalysisBegin` belongs to it.
    AnalysisBegin(AnalysisContext),
    /// One matching, Tarjan or tearing step of the current analysis.
    Analysis {
        context: AnalysisContext,
        record: &'a crate::StructuralRecord,
    },
    /// One reduction record, the same one the capture keeps.
    Reduction(&'a ReductionRecord),
}

/// Receives [`LiveStep`]s; see [`set_live_listener`].
pub type LiveListener = Box<dyn FnMut(LiveStep<'_>)>;

thread_local! {
    /// Optional live listener. Its callers in the analyses notify only while a
    /// capture scope is open; `notify` itself does not check.
    static LIVE: RefCell<Option<LiveListener>> = const { RefCell::new(None) };
}

/// Install (or with `None`, remove) a listener that receives every step of
/// preparation **as it happens**, on this thread.
///
/// It is called only while a capture scope is open ([`start_capture`]), because
/// the steps it receives are the ones the capture records: the listener sees
/// the run the capture describes, one step at a time, and cannot change it.
pub fn set_live_listener(listener: Option<LiveListener>) {
    LIVE.with(|slot| *slot.borrow_mut() = listener);
}

/// Hand one step to the live listener, if one is installed.
///
/// The listener is taken out of its slot for the call and put back afterwards,
/// so a listener that itself reaches this module cannot alias the slot. One
/// that installs a different listener from inside the call keeps the new one;
/// removing a listener is done from outside, between steps.
fn notify(step: LiveStep<'_>) {
    let Some(mut listener) = LIVE.with(|slot| slot.borrow_mut().take()) else {
        return;
    };
    listener(step);
    LIVE.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.is_none() {
            *slot = Some(listener);
        }
    });
}

pub(crate) fn notify_begin(context: AnalysisContext) {
    notify(LiveStep::AnalysisBegin(context));
}

pub(crate) fn notify_step(context: AnalysisContext, record: &crate::StructuralRecord) {
    notify(LiveStep::Analysis { context, record });
}

/// Append one event if a scope is open.
pub(crate) fn emit(event: PreparationEvent) {
    if let PreparationEvent::Reduction(record) = &event
        && capturing()
    {
        notify(LiveStep::Reduction(record));
    }
    CAPTURE.with(|capture| {
        if let Some(events) = capture.borrow_mut().as_mut() {
            events.push(event);
        }
    });
}

/// Record one finished analysis, rendering its report and incidence against
/// `view` while the sorted result is still branded to it.
pub(crate) fn emit_analysis<'dae>(
    context: AnalysisContext,
    view: dae::DaeView<'dae>,
    incidence: &Incidence<'dae>,
    sorted: &Result<SortedDae<'dae>, StructuralError>,
    trace: StructuralTrace,
) {
    let outcome = match sorted {
        Ok(sorted) => Ok(crate::structural_report_from_sorted(view, sorted)),
        Err(error) => Err(error.clone()),
    };
    emit(PreparationEvent::StructuralAnalysis {
        context,
        incidence: Some(capture_incidence(view, incidence)),
        outcome,
        trace,
    });
}

/// Record an analysis whose incidence could not be built.
pub(crate) fn emit_failed_incidence(context: AnalysisContext, error: &StructuralError) {
    emit(PreparationEvent::StructuralAnalysis {
        context,
        incidence: None,
        outcome: Err(error.clone()),
        trace: StructuralTrace::default(),
    });
}

fn capture_incidence<'dae>(
    view: dae::DaeView<'dae>,
    incidence: &Incidence<'dae>,
) -> CapturedIncidence {
    RENDERER.with(|renderer| {
        let renderer = renderer.borrow();
        let rows = incidence
            .eq_unknowns
            .iter()
            .zip(&incidence.equation_refs)
            .map(|(columns, equation)| {
                let mut unknowns = columns.to_vec();
                unknowns.sort_unstable();
                CapturedIncidenceRow {
                    id: equation.to_string(),
                    label: crate::equation_label(view, equation),
                    text: renderer
                        .as_ref()
                        .and_then(|render| render(view, equation.0)),
                    unknowns,
                }
            })
            .collect();
        CapturedIncidence {
            unknowns: incidence
                .unknowns
                .iter()
                .map(|unknown| crate::unknown_label(view, *unknown))
                .collect(),
            rows,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_quotient() -> PreparationEvent {
        PreparationEvent::AliasQuotient {
            scope: AliasQuotientScope::Source,
            report: AliasQuotientReport::default(),
        }
    }

    #[test]
    fn a_closed_scope_records_nothing_and_take_reports_empty() {
        assert!(take_capture().is_empty());
        emit(empty_quotient());
        assert!(
            take_capture().is_empty(),
            "an event emitted with no scope open was kept"
        );
    }

    #[test]
    fn the_live_listener_hears_each_step_and_stays_installed() {
        use std::rc::Rc;
        let heard = Rc::new(RefCell::new(Vec::new()));
        let sink = Rc::clone(&heard);
        set_live_listener(Some(Box::new(move |step| {
            if let LiveStep::AnalysisBegin(context) = step {
                sink.borrow_mut().push(context);
            }
        })));
        notify_begin(AnalysisContext::Source);
        notify_begin(AnalysisContext::DirectCandidate);
        set_live_listener(None);
        notify_begin(AnalysisContext::Source);
        assert_eq!(
            *heard.borrow(),
            vec![AnalysisContext::Source, AnalysisContext::DirectCandidate],
            "the listener must hear both steps, stay installed between them, and \
             hear nothing once removed",
        );
    }

    #[test]
    fn take_drains_and_closes_the_scope() {
        start_capture();
        emit(empty_quotient());
        assert_eq!(take_capture().len(), 1);
        assert!(!capturing(), "take_capture must close the scope");
        assert!(take_capture().is_empty());
    }
}
