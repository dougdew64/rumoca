//! Equation sheet — the flat DAE rendered as readable math.
//!
//! Built from the typed `Dae` in the worker thread (where the typed data
//! lives), then sent to the UI for display. The sheet groups equations by
//! origin category and lists variables by classification.

use rumoca_ir_dae as dae;

use crate::expr_format;

/// A single formatted equation with its origin and index.
#[derive(Debug, Clone)]
pub struct FormattedEquation {
    /// Index in the original DAE equation list.
    pub index: usize,
    /// Readable equation text (e.g. `der(w) = tau / J`).
    pub text: String,
    /// Human-readable origin (e.g. "equation from motor").
    pub origin: String,
    /// Origin category for grouping.
    pub category: EquationCategory,
}

/// Broad categories for grouping equations in the sheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EquationCategory {
    Component,
    Connection,
    FlowSum,
    Binding,
    Event,
}

impl EquationCategory {
    pub fn label(self) -> &'static str {
        match self {
            Self::Component => "Component equations",
            Self::Connection => "Connection equations",
            Self::FlowSum => "Flow conservation",
            Self::Binding => "Bindings",
            Self::Event => "Event equations",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Component => "Equations from component instances (their equation sections)",
            Self::Connection => "Equality constraints from connect() statements (potential variables)",
            Self::FlowSum => "Flow conservation: sum of signed flows = 0 at each connection node",
            Self::Binding => "Variable bindings from declarations (parameter values, fixed starts)",
            Self::Event => "Discrete assignments from when/elsewhen clauses and reinit",
        }
    }
}

/// A variable in the classification summary.
#[derive(Debug, Clone)]
pub struct ClassifiedVariable {
    pub name: String,
    pub kind: &'static str,
    pub unit: Option<String>,
    pub description: Option<String>,
    pub start: Option<String>,
}

/// The complete equation sheet, ready to render.
#[derive(Debug, Clone)]
pub struct EquationSheet {
    /// Equations grouped by category, in display order.
    pub groups: Vec<(EquationCategory, Vec<FormattedEquation>)>,
    /// Total equation count (continuous).
    pub n_equations: usize,
    /// Variable classification summary.
    pub variables: Vec<ClassifiedVariable>,
    /// Variable counts by kind.
    pub n_states: usize,
    pub n_algebraics: usize,
    pub n_parameters: usize,
    pub n_constants: usize,
    pub n_discrete: usize,
    pub n_inputs: usize,
    pub n_outputs: usize,
}

fn categorize_origin(origin: &str) -> EquationCategory {
    if origin.starts_with("connection equation") {
        EquationCategory::Connection
    } else if origin.starts_with("flow sum") || origin.starts_with("unconnected flow") {
        EquationCategory::FlowSum
    } else if origin.starts_with("binding") {
        EquationCategory::Binding
    } else if origin.contains("reinit") || origin.contains("when") {
        EquationCategory::Event
    } else {
        EquationCategory::Component
    }
}

/// Build an `EquationSheet` from a typed `Dae`.
pub fn build(dae: &dae::Dae) -> EquationSheet {
    let mut by_category: std::collections::BTreeMap<EquationCategory, Vec<FormattedEquation>> =
        std::collections::BTreeMap::new();

    for (i, eq) in dae.continuous.equations.iter().enumerate() {
        let text = expr_format::format_equation(eq);
        let origin = eq.origin.clone();
        let category = categorize_origin(&origin);
        by_category.entry(category).or_default().push(FormattedEquation {
            index: i,
            text,
            origin,
            category,
        });
    }

    let display_order = [
        EquationCategory::Component,
        EquationCategory::Connection,
        EquationCategory::FlowSum,
        EquationCategory::Binding,
        EquationCategory::Event,
    ];

    let groups: Vec<_> = display_order
        .into_iter()
        .filter_map(|cat| {
            by_category.remove(&cat).map(|eqs| (cat, eqs))
        })
        .collect();

    let mut variables = Vec::new();

    fn collect_vars(
        vars: &mut Vec<ClassifiedVariable>,
        iter: impl Iterator<Item = (String, dae::Variable)>,
        kind: &'static str,
    ) {
        for (name, v) in iter {
            vars.push(ClassifiedVariable {
                name,
                kind,
                unit: v.unit.clone().filter(|u| !u.is_empty()),
                description: v.description.clone().filter(|d| !d.is_empty()),
                start: v.start.as_ref().map(|e| expr_format::format_expr(e)),
            });
        }
    }

    macro_rules! collect_from {
        ($map:expr, $kind:expr) => {
            collect_vars(
                &mut variables,
                $map.iter().map(|(n, v)| (n.to_string(), v.clone())),
                $kind,
            )
        };
    }

    collect_from!(dae.variables.states, "state");
    collect_from!(dae.variables.algebraics, "algebraic");
    collect_from!(dae.variables.inputs, "input");
    collect_from!(dae.variables.outputs, "output");
    collect_from!(dae.variables.parameters, "parameter");
    collect_from!(dae.variables.constants, "constant");
    collect_from!(dae.variables.discrete_reals, "discrete");
    collect_from!(dae.variables.discrete_valued, "discrete");

    EquationSheet {
        n_equations: dae.continuous.equations.len(),
        groups,
        n_states: dae.variables.states.len(),
        n_algebraics: dae.variables.algebraics.len(),
        n_parameters: dae.variables.parameters.len(),
        n_constants: dae.variables.constants.len(),
        n_discrete: dae.variables.discrete_reals.len() + dae.variables.discrete_valued.len(),
        n_inputs: dae.variables.inputs.len(),
        n_outputs: dae.variables.outputs.len(),
        variables,
    }
}

impl EquationCategory {
    fn cmp_key(self) -> u8 {
        match self {
            Self::Component => 0,
            Self::Connection => 1,
            Self::FlowSum => 2,
            Self::Binding => 3,
            Self::Event => 4,
        }
    }
}

impl Ord for EquationCategory {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.cmp_key().cmp(&other.cmp_key())
    }
}

impl PartialOrd for EquationCategory {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(&other))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn categorize_origin_covers_all_variants() {
        assert_eq!(categorize_origin("equation from motor"), EquationCategory::Component);
        assert_eq!(categorize_origin("top-level model equation"), EquationCategory::Component);
        assert_eq!(categorize_origin("connection equation: a = b"), EquationCategory::Connection);
        assert_eq!(categorize_origin("flow sum: ..."), EquationCategory::FlowSum);
        assert_eq!(categorize_origin("unconnected flow x"), EquationCategory::FlowSum);
        assert_eq!(categorize_origin("binding for p"), EquationCategory::Binding);
        assert_eq!(categorize_origin("reinit of v"), EquationCategory::Event);
        assert_eq!(categorize_origin("when assignment x"), EquationCategory::Event);
    }

    #[test]
    fn category_labels_are_non_empty() {
        for cat in [
            EquationCategory::Component,
            EquationCategory::Connection,
            EquationCategory::FlowSum,
            EquationCategory::Binding,
            EquationCategory::Event,
        ] {
            assert!(!cat.label().is_empty());
            assert!(!cat.description().is_empty());
        }
    }

    #[test]
    fn build_on_real_specimen() {
        let specimen = std::path::PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/specimens/RotationalInertia.mo"
        ));
        let msl_base = concat!(env!("CARGO_MANIFEST_DIR"), "/vendor/msl");
        let libraries = vec![
            std::path::PathBuf::from(format!("{msl_base}/Modelica 4.1.0")),
            std::path::PathBuf::from(format!("{msl_base}/ModelicaServices 4.1.0")),
            std::path::PathBuf::from(format!("{msl_base}/Complex.mo")),
        ];
        let result = crate::worker::compile_specimen(&specimen, libraries)
            .expect("compile_specimen");
        let crate::worker::FromWorker::Compiled { equation_sheet, .. } = result else {
            panic!("expected Compiled");
        };
        let sheet = equation_sheet.expect("equation_sheet should be Some for a healthy specimen");

        assert!(sheet.n_equations > 0, "should have equations");
        assert!(sheet.n_states > 0, "should have state variables");
        assert!(!sheet.groups.is_empty(), "should have at least one group");
        assert!(!sheet.variables.is_empty(), "should have variables");

        for (_, eqs) in &sheet.groups {
            for eq in eqs {
                assert!(!eq.text.is_empty(), "equation text should not be empty");
                assert!(!eq.origin.is_empty(), "origin should not be empty");
            }
        }
    }
}
