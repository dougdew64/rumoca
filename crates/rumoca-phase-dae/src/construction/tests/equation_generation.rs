//! Every flat equation origin keeps its own DAE generation across the boundary.

use rumoca_ir_dae as dae;
use rumoca_ir_flat as flat;

use crate::construction::equation_generation;

/// An unconnected flow is not reported as a flow balance.
///
/// MLS §9.2 constrains a flow variable with nothing connected to it to zero,
/// which states something different from a flow balance: a balance says several
/// flows cancel at a junction, this says one flow has no junction at all. The
/// flat IR separates them, and construction used to map both onto
/// `FlowBalanceEquation`, so the distinction did not survive into the DAE and no
/// consumer could recover it.
///
/// **The two assertions are the point, not either one alone**: the first proves
/// the new classification fires, the second proves it did not capture the case
/// it must stay distinct from. Testing only the new variant would pass equally
/// if `FlowSum` had been swept into it.
#[test]
fn an_unconnected_flow_is_distinct_from_a_flow_balance() {
    assert_eq!(
        equation_generation(&flat::EquationOrigin::UnconnectedFlow {
            variable: "resistor.p.i".to_string(),
        }),
        Some(dae::DaeGeneration::UnconnectedFlowEquation),
    );
    assert_eq!(
        equation_generation(&flat::EquationOrigin::FlowSum {
            description: "node n".to_string(),
        }),
        Some(dae::DaeGeneration::FlowBalanceEquation),
    );
}

/// A source-written equation stays unclassified, so its provenance is `Source`.
///
/// `equation_generation` returning `None` is what makes
/// `equation_owner_provenance` build a source provenance rather than a generated
/// one. A generation added here by mistake would silently relabel every
/// hand-written equation in every model as compiler-generated.
#[test]
fn a_component_equation_names_no_generation() {
    assert_eq!(
        equation_generation(&flat::EquationOrigin::ComponentEquation {
            component: "resistor".to_string(),
        }),
        None,
    );
}
