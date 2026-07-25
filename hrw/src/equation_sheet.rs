//! Equation sheet — the flat DAE rendered as readable math.
//!
//! Built from the typed `Dae` in the worker thread (where the typed data
//! lives), then sent to the UI for display. The sheet groups equations by
//! origin category and lists variables by classification. When the specimen
//! source is available, each equation is linked back to its source line(s)
//! via the `span` byte offsets carried through the pipeline.

use eframe::egui;
use rumoca_core::SourceId;
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
    /// 1-based source line(s) where this equation originates. Most equations
    /// map to a single line, but connect-generated equations (transitive
    /// equalities, multi-connector flow sums) can trace to multiple connect()
    /// statements. Empty if the span points to a library file rather than
    /// the specimen.
    pub source_lines: Vec<u32>,
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

    pub fn color(self) -> egui::Color32 {
        match self {
            Self::Component => egui::Color32::from_rgb(100, 180, 255),
            Self::Connection => egui::Color32::from_rgb(255, 180, 80),
            Self::FlowSum => egui::Color32::from_rgb(255, 120, 80),
            Self::Binding => egui::Color32::from_rgb(160, 200, 120),
            Self::Event => egui::Color32::from_rgb(200, 140, 220),
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

/// One line of the specimen source with its equation associations.
#[derive(Debug, Clone)]
pub struct SourceLine {
    pub line_number: u32,
    pub text: String,
    /// Indices into the flat equation list of equations originating from this line.
    pub equation_indices: Vec<usize>,
    /// Category of the first associated equation (for color-coding).
    pub category: Option<EquationCategory>,
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
    /// Specimen source lines with equation associations (empty if source
    /// was not provided).
    pub source_lines: Vec<SourceLine>,
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

/// Convert a byte offset to a 1-based line number by counting newlines.
fn byte_offset_to_line(source: &str, byte_offset: usize) -> u32 {
    let clamped = byte_offset.min(source.len());
    source[..clamped].bytes().filter(|&b| b == b'\n').count() as u32 + 1
}

/// Scan the specimen source for `connect(A, B)` statements. Returns a vec
/// of `(line_number, component_a, component_b)` where the components are
/// the dot-paths named in the connect call (e.g. "motor.flange", "rotor.flange_a").
fn scan_connect_statements(source: &str) -> Vec<(u32, String, String)> {
    let mut result = Vec::new();
    for (i, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("connect(") {
            if let Some(args) = rest.strip_suffix(");") {
                if let Some((a, b)) = args.split_once(',') {
                    result.push((i as u32 + 1, a.trim().to_owned(), b.trim().to_owned()));
                }
            }
        }
    }
    result
}

/// Find the source line(s) for a connection/flow equation by matching its
/// origin against the specimen's connect() statements.
///
/// **Connection equations** (potential equalities like `A.phi = B.phi`):
/// if one connect() names both A and B, that's a direct match — return just
/// that line. Otherwise the equality is *transitive* (bridging two connect
/// statements that share a node), so return every connect() that mentions
/// either variable.
///
/// **Flow sums** (`A.tau + B.tau + C.tau = 0`): always return every connect()
/// that mentions any of the summed variables, because the conservation
/// equation is a property of the whole equivalence-class node.
fn match_connection_to_source(
    origin: &str,
    connects: &[(u32, String, String)],
) -> Vec<u32> {
    let is_flow;
    let vars: Vec<&str> = if let Some(rest) = origin.strip_prefix("connection equation: ") {
        is_flow = false;
        rest.split(" = ").collect()
    } else if let Some(rest) = origin.strip_prefix("flow sum equation: ") {
        is_flow = true;
        rest.strip_suffix(" = 0")
            .unwrap_or(rest)
            .split(" + ")
            .map(|v| v.trim().trim_start_matches('-'))
            .collect()
    } else {
        return Vec::new();
    };

    if !is_flow {
        // Connection equation: strict match first (both connectors in one connect).
        for (line, a, b) in connects {
            let both = vars.iter().any(|v| v.starts_with(a.as_str()))
                && vars.iter().any(|v| v.starts_with(b.as_str()));
            if both {
                return vec![*line];
            }
        }
    }

    // Flow sums always, connection equations when no strict match (transitive):
    // collect every connect() that mentions any variable.
    let mut lines: Vec<u32> = connects.iter()
        .filter(|(_, a, b)| {
            vars.iter().any(|v| v.starts_with(a.as_str()) || v.starts_with(b.as_str()))
        })
        .map(|(line, _, _)| *line)
        .collect();
    lines.dedup();
    lines
}

/// Build an `EquationSheet` from a typed `Dae`. When `source_info` is
/// provided, equations are linked to their source lines via span matching
/// (for direct specimen equations) and origin-based text matching (for
/// connect-generated equations whose spans point to library files).
pub fn build(dae: &dae::Dae, source_info: Option<(&str, &str)>) -> EquationSheet {
    let specimen_sid = source_info.map(|(uri, _)| SourceId::from_source_name(uri));
    let source_text = source_info.map(|(_, src)| src);
    let connects = source_text.map(|s| scan_connect_statements(s)).unwrap_or_default();

    let mut by_category: std::collections::BTreeMap<EquationCategory, Vec<FormattedEquation>> =
        std::collections::BTreeMap::new();

    // Flat list for building the reverse mapping (line → equations).
    let mut all_equations: Vec<(usize, EquationCategory, Vec<u32>)> = Vec::new();

    for (i, eq) in dae.continuous.equations.iter().enumerate() {
        let text = expr_format::format_equation(eq);
        let origin = eq.origin.clone();
        let category = categorize_origin(&origin);

        // Try span-based matching first (direct specimen equations).
        let mut source_lines: Vec<u32> = match (specimen_sid, source_text) {
            (Some(sid), Some(src)) if eq.span.source == sid => {
                vec![byte_offset_to_line(src, eq.span.start.0)]
            }
            _ => Vec::new(),
        };

        // Fall back to origin-based matching for connect-generated equations.
        if source_lines.is_empty() && !connects.is_empty() {
            source_lines = match category {
                EquationCategory::Connection | EquationCategory::FlowSum => {
                    match_connection_to_source(&origin, &connects)
                }
                EquationCategory::Component => {
                    if let Some(comp) = origin.strip_prefix("equation from ") {
                        source_text.and_then(|src| {
                            src.lines().enumerate().find_map(|(li, line)| {
                                let trimmed = line.trim().trim_end_matches(';');
                                if trimmed.contains(comp) && !trimmed.starts_with("//") {
                                    Some(li as u32 + 1)
                                } else {
                                    None
                                }
                            })
                        })
                        .into_iter()
                        .collect()
                    } else {
                        Vec::new()
                    }
                }
                EquationCategory::Binding => {
                    if let Some(var) = origin.strip_prefix("binding equation for ") {
                        source_text.and_then(|src| {
                            src.lines().enumerate().find_map(|(li, line)| {
                                let trimmed = line.trim();
                                if trimmed.contains(var) && !trimmed.starts_with("//") {
                                    Some(li as u32 + 1)
                                } else {
                                    None
                                }
                            })
                        })
                        .into_iter()
                        .collect()
                    } else {
                        Vec::new()
                    }
                }
                _ => Vec::new(),
            };
        }

        all_equations.push((i, category, source_lines.clone()));

        by_category.entry(category).or_default().push(FormattedEquation {
            index: i,
            text,
            origin,
            category,
            source_lines,
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

    // Build source lines with equation associations.
    let source_lines = if let Some(src) = source_text {
        let mut lines: Vec<SourceLine> = src
            .lines()
            .enumerate()
            .map(|(i, text)| SourceLine {
                line_number: i as u32 + 1,
                text: text.to_owned(),
                equation_indices: Vec::new(),
                category: None,
            })
            .collect();

        for (eq_idx, cat, eq_lines) in &all_equations {
            for &line in eq_lines {
                if let Some(sl) = lines.get_mut(line as usize - 1) {
                    sl.equation_indices.push(*eq_idx);
                    if sl.category.is_none() {
                        sl.category = Some(*cat);
                    }
                }
            }
        }

        lines
    } else {
        Vec::new()
    };

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
        source_lines,
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
    fn scan_connect_parses_specimen() {
        let src = "  connect(a.p, b.n);\n  connect( x , y );\n  x = 1;\n";
        let conns = scan_connect_statements(src);
        assert_eq!(conns.len(), 2);
        assert_eq!(conns[0], (1, "a.p".to_owned(), "b.n".to_owned()));
        assert_eq!(conns[1], (2, "x".to_owned(), "y".to_owned()));
    }

    #[test]
    fn match_connection_finds_connect_line() {
        let conns = vec![
            (10, "motor.flange".to_owned(), "rotor.flange_a".to_owned()),
            (11, "rotor.flange_b".to_owned(), "gear.flange_a".to_owned()),
        ];
        // Direct connection: both vars from one connect → single line.
        assert_eq!(
            match_connection_to_source("connection equation: motor.flange.phi = rotor.flange_a.phi", &conns),
            vec![10],
        );
        // Flow sum with two vars from one connect → that line.
        assert_eq!(
            match_connection_to_source("flow sum equation: motor.flange.tau + rotor.flange_a.tau = 0", &conns),
            vec![10],
        );
        // Non-connection origin → empty.
        assert_eq!(
            match_connection_to_source("equation from motor", &conns),
            Vec::<u32>::new(),
        );
    }

    #[test]
    fn match_connection_multi_connect_node() {
        // Three connectors share a node via two connect() statements:
        //   line 50: connect(load.flange_b, spring.flange_a)
        //   line 54: connect(brakeTorque.flange, load.flange_b)
        let conns = vec![
            (50, "load.flange_b".to_owned(), "spring.flange_a".to_owned()),
            (54, "brakeTorque.flange".to_owned(), "load.flange_b".to_owned()),
        ];

        // Direct connection: both vars from line 50 → single line.
        assert_eq!(
            match_connection_to_source(
                "connection equation: load.flange_b.phi = spring.flange_a.phi", &conns),
            vec![50],
        );

        // Transitive connection: spring.flange_a (line 50) = brakeTorque.flange
        // (line 54). No single connect has both → returns both lines.
        assert_eq!(
            match_connection_to_source(
                "connection equation: spring.flange_a.phi = brakeTorque.flange.phi", &conns),
            vec![50, 54],
        );

        // Multi-connector flow sum: all three connectors from two connect
        // statements → both lines.
        assert_eq!(
            match_connection_to_source(
                "flow sum equation: load.flange_b.tau + spring.flange_a.tau + brakeTorque.flange.tau = 0",
                &conns,
            ),
            vec![50, 54],
        );
    }

    #[test]
    fn byte_offset_to_line_basics() {
        let src = "line one\nline two\nline three\n";
        assert_eq!(byte_offset_to_line(src, 0), 1);
        assert_eq!(byte_offset_to_line(src, 5), 1);
        assert_eq!(byte_offset_to_line(src, 9), 2);
        assert_eq!(byte_offset_to_line(src, 18), 3);
        assert_eq!(byte_offset_to_line(src, 999), 4);
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

        assert!(!sheet.source_lines.is_empty(), "should have source lines");
        let has_linked = sheet.source_lines.iter().any(|sl| !sl.equation_indices.is_empty());
        assert!(has_linked, "at least one source line should link to an equation");
    }

    #[test]
    fn gear_with_brake_all_equations_linked_to_source() {
        let specimen = std::path::PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/specimens/GearWithBrake.mo"
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
        let sheet = equation_sheet.expect("equation_sheet");

        let linked: Vec<_> = sheet.groups.iter()
            .flat_map(|(_, eqs)| eqs)
            .filter(|eq| !eq.source_lines.is_empty())
            .collect();
        let total = sheet.n_equations;

        assert_eq!(
            linked.len(), total,
            "expected all {total} equations linked, got {}",
            linked.len(),
        );

        // Connection equations should point to connect() lines (45-54).
        let conn_lines: Vec<u32> = sheet.groups.iter()
            .filter(|(cat, _)| *cat == EquationCategory::Connection)
            .flat_map(|(_, eqs)| eqs)
            .flat_map(|eq| eq.source_lines.iter().copied())
            .collect();
        assert!(!conn_lines.is_empty(), "connection equations should have source lines");
        assert!(
            conn_lines.iter().all(|&ln| (45..=54).contains(&ln)),
            "connection equations should point to connect() lines (45-54), got {:?}",
            conn_lines,
        );
    }
}
