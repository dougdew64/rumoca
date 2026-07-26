//! Cross-stage identifier index — maps source identifiers to their
//! representations across all pipeline stages.
//!
//! Built in the worker thread from the typed `Dae` alongside the equation
//! sheet. Each DAE variable that originates in the specimen source gets an
//! entry with its qualified flat name, source span, and classification.

use std::collections::HashMap;

use rumoca_core::{SourceId, VarName};
use rumoca_ir_dae as dae;

/// A single variable's cross-stage identity.
#[derive(Debug, Clone)]
pub struct IndexedVariable {
    /// Qualified flat name (e.g. "gear.flange_b.tau").
    pub name: String,
    /// Variable classification in the DAE (state, algebraic, parameter, etc.).
    pub kind: &'static str,
    /// Source byte range for the declaration (start, end).
    pub source_byte_range: (usize, usize),
    /// 1-based source line number of the declaration.
    pub source_line: u32,
    /// DefId from the component reference, if available.
    pub def_id: Option<u32>,
    /// Description string from the Modelica declaration.
    pub description: Option<String>,
}

/// Cross-stage identifier index for a compiled specimen.
#[derive(Debug, Clone, Default)]
pub struct IdentifierIndex {
    /// All indexed variables, keyed by qualified flat name.
    pub variables: HashMap<String, IndexedVariable>,
    /// Source line number → variable names declared on that line.
    pub line_to_variables: HashMap<u32, Vec<String>>,
}

impl IdentifierIndex {
    /// Build the index from a DAE and its source text.
    pub fn build(dae: &dae::Dae, source_uri: &str, source_text: &str) -> Self {
        let specimen_sid = SourceId::from_source_name(source_uri);
        let mut idx = Self::default();

        let v = &dae.variables;
        idx.add_partition("state", v.states.iter(), specimen_sid, source_text);
        idx.add_partition("algebraic", v.algebraics.iter(), specimen_sid, source_text);
        idx.add_partition("input", v.inputs.iter(), specimen_sid, source_text);
        idx.add_partition("output", v.outputs.iter(), specimen_sid, source_text);
        idx.add_partition("parameter", v.parameters.iter(), specimen_sid, source_text);
        idx.add_partition("constant", v.constants.iter(), specimen_sid, source_text);
        idx.add_partition("discrete real", v.discrete_reals.iter(), specimen_sid, source_text);
        idx.add_partition("discrete valued", v.discrete_valued.iter(), specimen_sid, source_text);

        idx
    }

    fn add_partition<'a>(
        &mut self,
        kind: &'static str,
        iter: impl Iterator<Item = (&'a VarName, &'a dae::Variable)>,
        specimen_sid: SourceId,
        source_text: &str,
    ) {
        for (var_name, var) in iter {
            if var.source_span.source != specimen_sid {
                continue;
            }
            let name = var_name.to_string();
            let line = byte_offset_to_line(source_text, var.source_span.start.0);
            let def_id = var.component_ref.as_ref().and_then(|cr| {
                cr.def_id.map(|id| id.0)
            });

            let entry = IndexedVariable {
                name: name.clone(),
                kind,
                source_byte_range: (var.source_span.start.0, var.source_span.end.0),
                source_line: line,
                def_id,
                description: var.description.clone(),
            };

            let names = self.line_to_variables.entry(line).or_default();
            if !names.contains(&name) {
                names.push(name.clone());
            }
            self.variables.insert(name, entry);
        }
    }

    /// Look up all variables declared on a given 1-based source line.
    pub fn variables_on_line(&self, line: u32) -> Vec<&IndexedVariable> {
        self.line_to_variables.get(&line)
            .map(|names| {
                names.iter().filter_map(|n| self.variables.get(n)).collect()
            })
            .unwrap_or_default()
    }

    /// Find clickable identifier spans within a source line.
    ///
    /// Returns a list of `(byte_start, byte_end, qualified_name)` tuples
    /// marking where leaf identifier names appear in the line text.
    /// Only matches whole identifiers (not substrings of longer names).
    pub fn clickable_spans(&self, line: u32, line_text: &str) -> Vec<(usize, usize, String)> {
        let vars = self.variables_on_line(line);
        if vars.is_empty() {
            return Vec::new();
        }
        let mut spans = Vec::new();
        let mut seen_positions = std::collections::HashSet::new();
        for var in &vars {
            let leaf = var.name.rsplit('.').next().unwrap_or(&var.name);
            if let Some(pos) = find_whole_identifier(line_text, leaf) {
                if seen_positions.insert(pos) {
                    spans.push((pos, pos + leaf.len(), var.name.clone()));
                }
            }
        }
        spans.sort_by_key(|s| s.0);
        spans
    }
}

fn find_whole_identifier(haystack: &str, needle: &str) -> Option<usize> {
    let mut start = 0;
    while let Some(pos) = haystack[start..].find(needle) {
        let abs = start + pos;
        let before_ok = abs == 0
            || !haystack.as_bytes()[abs - 1].is_ascii_alphanumeric()
                && haystack.as_bytes()[abs - 1] != b'_';
        let end = abs + needle.len();
        let after_ok = end >= haystack.len()
            || !haystack.as_bytes()[end].is_ascii_alphanumeric()
                && haystack.as_bytes()[end] != b'_';
        if before_ok && after_ok {
            return Some(abs);
        }
        start = abs + 1;
    }
    None
}

fn byte_offset_to_line(source: &str, byte_offset: usize) -> u32 {
    let clamped = byte_offset.min(source.len());
    source[..clamped].bytes().filter(|&b| b == b'\n').count() as u32 + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_offset_to_line_first_line() {
        assert_eq!(byte_offset_to_line("abc\ndef\n", 0), 1);
        assert_eq!(byte_offset_to_line("abc\ndef\n", 2), 1);
    }

    #[test]
    fn byte_offset_to_line_second_line() {
        assert_eq!(byte_offset_to_line("abc\ndef\n", 4), 2);
        assert_eq!(byte_offset_to_line("abc\ndef\n", 6), 2);
    }

    #[test]
    fn byte_offset_to_line_clamps() {
        assert_eq!(byte_offset_to_line("abc", 999), 1);
    }

    #[test]
    fn find_whole_identifier_basic() {
        assert_eq!(find_whole_identifier("  Real J;", "J"), Some(7));
    }

    #[test]
    fn find_whole_identifier_rejects_substring() {
        assert_eq!(find_whole_identifier("  Real JJ;", "J"), None);
    }

    #[test]
    fn find_whole_identifier_at_start() {
        assert_eq!(find_whole_identifier("J = 1;", "J"), Some(0));
    }

    #[test]
    fn find_whole_identifier_at_end() {
        assert_eq!(find_whole_identifier("x + J", "J"), Some(4));
    }

    #[test]
    fn clickable_spans_returns_empty_for_no_vars() {
        let idx = IdentifierIndex::default();
        assert!(idx.clickable_spans(1, "model Foo").is_empty());
    }

    #[test]
    fn clickable_spans_finds_leaf_name() {
        let mut idx = IdentifierIndex::default();
        idx.variables.insert("inertia.J".to_string(), IndexedVariable {
            name: "inertia.J".to_string(),
            kind: "parameter",
            source_byte_range: (0, 10),
            source_line: 3,
            def_id: None,
            description: None,
        });
        idx.line_to_variables.entry(3).or_default().push("inertia.J".to_string());
        let spans = idx.clickable_spans(3, "  parameter Real J = 1;");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].2, "inertia.J");
        assert_eq!(&"  parameter Real J = 1;"[spans[0].0..spans[0].1], "J");
    }

    #[test]
    fn duplicate_line_to_variables_entry_produces_single_span() {
        let mut idx = IdentifierIndex::default();
        idx.variables.insert("h".to_string(), IndexedVariable {
            name: "h".to_string(),
            kind: "state",
            source_byte_range: (0, 10),
            source_line: 2,
            def_id: None,
            description: None,
        });
        idx.line_to_variables.entry(2).or_default().push("h".to_string());
        idx.line_to_variables.entry(2).or_default().push("h".to_string());
        let spans = idx.clickable_spans(2, "  Real h(start = 1.0) \"height\";");
        assert_eq!(spans.len(), 1, "duplicate line_to_variables entry must not duplicate the span");
    }

    #[test]
    fn pre_variable_same_leaf_produces_single_span() {
        let mut idx = IdentifierIndex::default();
        let var = |name: &str, kind: &'static str| IndexedVariable {
            name: name.to_string(), kind,
            source_byte_range: (0, 10), source_line: 5,
            def_id: None, description: None,
        };
        idx.variables.insert("h".to_string(), var("h", "state"));
        idx.variables.insert("__pre__.h".to_string(), var("__pre__.h", "parameter"));
        idx.line_to_variables.entry(5).or_default().push("h".to_string());
        idx.line_to_variables.entry(5).or_default().push("__pre__.h".to_string());
        let spans = idx.clickable_spans(5, "  Real h(start = 1.0) \"height\";");
        assert_eq!(spans.len(), 1, "__pre__ variant must not add a second span at the same position");
        assert_eq!(spans[0].2, "h", "should prefer the non-prefixed variable");
    }
}
