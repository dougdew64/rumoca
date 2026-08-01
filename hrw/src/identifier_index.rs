//! Cross-stage identifier index — maps source identifiers to their
//! representations across all pipeline stages.
//!
//! Built in the worker thread from the typed `Dae` alongside the equation
//! sheet. Each DAE variable that originates in the specimen source gets an
//! entry with its qualified flat name, source span, and classification.

use std::collections::HashMap;

use rumoca_core::{SourceId, VarName};
use rumoca_ir_dae as dae;

use crate::modelica_lex::TokenKind;
use crate::source_view::LineToken;

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
    /// Returns `(byte_start, byte_end, qualified_name)` tuples, in order,
    /// marking every place a DAE variable is named on this line.
    ///
    /// ## Driven by the lexer, not by searching
    ///
    /// `tokens` are the line's tokens from `SourceHighlight::line`. Scanning
    /// them instead of searching the raw text for each variable's leaf name
    /// fixes three things the previous approach got wrong:
    ///
    /// - **Every occurrence is found.** Searching returned only the first
    ///   position of each name, so `J = J + 1` had one clickable `J`.
    /// - **Comments and strings are excluded.** A variable named `h` made the
    ///   `h` in `// height of h` clickable, because a text search cannot tell
    ///   code from commentary. The lexer can.
    /// - **Qualified names disambiguate.** With `a.phi` and `b.phi` both on a
    ///   line, leaf matching linked both to whichever came first. The dotted
    ///   path ending at each identifier is matched longest-first, so each links
    ///   to its own variable.
    ///
    /// Identifiers with no matching variable are simply absent from the result
    /// — they may be class names, function names, modifier names, or names that
    /// never reached the DAE, and this index cannot tell those apart.
    pub fn clickable_spans(
        &self,
        line: u32,
        line_text: &str,
        tokens: &[LineToken],
    ) -> Vec<(usize, usize, String)> {
        let vars = self.variables_on_line(line);
        if vars.is_empty() {
            return Vec::new();
        }
        let mut spans = Vec::new();
        for (i, tok) in tokens.iter().enumerate() {
            if tok.kind != TokenKind::Identifier {
                continue;
            }
            let path = crate::source_view::dotted_path_ending_at(line_text, tokens, i);
            if let Some(var) = best_match(&vars, &path) {
                spans.push((tok.start, tok.end, var.name.clone()));
            }
        }
        spans
    }
}

/// Pick the variable a dotted path refers to, preferring the longest match.
///
/// `b.phi` is tried before `phi`, so a line mentioning both `a.phi` and `b.phi`
/// links each to its own variable rather than both to the first one found.
fn best_match<'a>(vars: &[&'a IndexedVariable], path: &str) -> Option<&'a IndexedVariable> {
    let parts: Vec<&str> = path.split('.').collect();
    for start in 0..parts.len() {
        let candidate = parts[start..].join(".");
        let suffix = format!(".{candidate}");
        if let Some(v) = vars
            .iter()
            .find(|v| v.name == candidate || v.name.ends_with(&suffix))
        {
            return Some(v);
        }
    }
    None
}

/// Reduce a derivative mention to the variable it differentiates.
///
/// Views name things as the DAE does, so an incidence column may read `der(h)`
/// where the source declares `h`. One canonical implementation — this used to
/// exist three times (`app.rs`, and twice inline in `tree.rs`).
///
/// Peels exactly **one** layer: `der(der(h))` yields `der(h)`, which is itself a
/// variable in a reduced system, so reducing further would name the wrong thing.
/// Input that merely looks the part is returned untouched — `der(a) + der(b)`
/// begins and ends correctly but its opening paren closes early.
pub fn strip_der(name: &str) -> &str {
    let Some(rest) = name.strip_prefix("der(") else { return name };
    let Some(inner) = rest.strip_suffix(')') else { return name };
    let mut depth = 0i32;
    for c in inner.chars() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth < 0 {
                    return name;
                }
            }
            // A top-level comma means this is not one variable's derivative.
            ',' if depth == 0 => return name,
            _ => {}
        }
    }
    if depth != 0 {
        return name;
    }
    inner.trim()
}

/// IR fields whose string values are prose written for a human, not code.
///
/// An identifier occurring in code *is* a mention; in prose it is a
/// coincidence. `Real h "height of h"` must not read as a use of `h`.
///
/// Note the fix is by **field**, not by content: these strings are not
/// Modelica, so lexing them would be a category error — `mentions_identifier`
/// would happily find `h` as a token in "height of h". What matters is where
/// the string came from.
///
/// Deliberately short: listing a field wrongly *hides* real matches, which is
/// the worse failure. `unit` and `quantity` are omitted on purpose — they hold
/// code-like values (`"N.m"`) that the lexer reads as one dotted reference.
///
/// Shared by the tree (highlighting) and the bridge (emission), which must
/// agree about what counts as a mention or the Context Bar would describe
/// something different from what the views show.
pub const PROSE_FIELDS: &[&str] = &["description", "comment", "file_name"];

pub fn is_prose_field(key: &str) -> bool {
    PROSE_FIELDS.contains(&key)
}

/// Whether two names refer to the same DAE variable.
///
/// **Exact comparison, modulo one `der(…)` wrapper on either side.**
///
/// This replaced a whole-word substring search (`matches_tracked`), which
/// `docs/identity-and-provenance.md` rules out as a standing principle: *"No
/// heuristic name-matching."* The substring version was buying exactly one
/// thing — letting a tracked `h` match an unknown named `der(h)` — and paying
/// for it with false positives wherever a name appeared inside other text.
///
/// Exact comparison is well-founded here because **flat names are canonical**:
/// `src.n.i` names precisely one variable in the DAE. A name is an identifier
/// that happens to be a string, not a search term. Every value that reaches
/// this function is a qualified flat name — `clickable_spans` and
/// `trackable_name` both yield them.
///
/// For *membership* questions — "does this equation mention the variable?" —
/// do not use this on rendered equation text. The incidence matrix answers that
/// structurally: `rows[i]` holds the columns equation `i` touches.
pub fn same_variable(a: &str, b: &str) -> bool {
    strip_der(a) == strip_der(b)
}

use crate::byte_offset_to_line;

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

    /// Build an index with the given `(qualified_name, kind)` variables all
    /// declared on `line`, and return the spans found in `text`.
    fn spans_for(line: u32, text: &str, vars: &[(&str, &'static str)]) -> Vec<(usize, usize, String)> {
        let mut idx = IdentifierIndex::default();
        for (name, kind) in vars {
            idx.variables.insert((*name).to_string(), IndexedVariable {
                name: (*name).to_string(),
                kind,
                source_byte_range: (0, 10),
                source_line: line,
                def_id: None,
                description: None,
            });
            idx.line_to_variables.entry(line).or_default().push((*name).to_string());
        }
        let hl = crate::source_view::SourceHighlight::new(text);
        idx.clickable_spans(line, text, hl.line(0))
    }

    /// The span texts, for assertions that care about what was linked rather
    /// than where.
    fn span_texts<'a>(text: &'a str, spans: &[(usize, usize, String)]) -> Vec<(&'a str, String)> {
        spans.iter().map(|(s, e, n)| (&text[*s..*e], n.clone())).collect()
    }

    #[test]
    fn clickable_spans_returns_empty_for_no_vars() {
        let idx = IdentifierIndex::default();
        let hl = crate::source_view::SourceHighlight::new("model Foo");
        assert!(idx.clickable_spans(1, "model Foo", hl.line(0)).is_empty());
    }

    #[test]
    fn clickable_spans_finds_leaf_name() {
        let text = "  parameter Real J = 1;";
        let spans = spans_for(3, text, &[("inertia.J", "parameter")]);
        assert_eq!(span_texts(text, &spans), vec![("J", "inertia.J".to_string())]);
    }

    /// Regression: the search-based implementation returned only the first
    /// position of each name, so later mentions were not clickable.
    #[test]
    fn every_occurrence_is_clickable() {
        let text = "  J = J + J;";
        let spans = spans_for(3, text, &[("inertia.J", "parameter")]);
        assert_eq!(spans.len(), 3, "all three mentions of J should be clickable");
        assert!(spans.iter().all(|s| s.2 == "inertia.J"));
    }

    /// Regression: a text search cannot tell code from commentary, so a
    /// variable named `h` made the `h` in a comment clickable.
    #[test]
    fn identifiers_in_comments_and_strings_are_not_clickable() {
        let text = "  Real h; // height of h";
        let spans = spans_for(2, text, &[("h", "state")]);
        assert_eq!(spans.len(), 1, "only the declaration, not the comment");
        assert!(spans[0].0 < text.find("//").unwrap());

        let text = "  Real h = 1 \"h in metres\";";
        let spans = spans_for(2, text, &[("h", "state")]);
        assert_eq!(spans.len(), 1, "the h inside the description string is not code");
    }

    /// Regression: leaf matching linked both mentions to whichever variable
    /// came first. The dotted path ending at each identifier disambiguates.
    #[test]
    fn same_leaf_on_one_line_links_to_the_right_variable() {
        let text = "  a.phi = b.phi;";
        let spans = spans_for(4, text, &[("m.a.phi", "state"), ("m.b.phi", "state")]);
        let linked: Vec<_> = span_texts(text, &spans)
            .into_iter()
            .filter(|(t, _)| *t == "phi")
            .map(|(_, n)| n)
            .collect();
        assert_eq!(linked, vec!["m.a.phi".to_string(), "m.b.phi".to_string()]);
    }

    #[test]
    fn duplicate_line_to_variables_entry_produces_single_span() {
        let text = "  Real h(start = 1.0) \"height\";";
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
        let hl = crate::source_view::SourceHighlight::new(text);
        let spans = idx.clickable_spans(2, text, hl.line(0));
        assert_eq!(spans.len(), 1, "duplicate line_to_variables entry must not duplicate the span");
    }

    #[test]
    fn pre_variable_same_leaf_produces_single_span() {
        let text = "  Real h(start = 1.0) \"height\";";
        let spans = spans_for(5, text, &[("h", "state"), ("__pre__.h", "parameter")]);
        assert_eq!(spans.len(), 1, "__pre__ variant must not add a second span at the same position");
        assert_eq!(spans[0].2, "h", "should prefer the non-prefixed variable");
    }

    /// Names that are not DAE variables — class names, function names, the
    /// modifier `start` — are simply not offered.
    #[test]
    fn non_variable_identifiers_are_not_linked() {
        let text = "  Real h(start = 1.0);";
        let spans = spans_for(2, text, &[("h", "state")]);
        assert_eq!(span_texts(text, &spans), vec![("h", "h".to_string())]);
    }

    /// Identity is exact, modulo one `der(...)` wrapper.
    ///
    /// Replaced a whole-word substring search. The substring version bought
    /// exactly one thing -- letting tracked `h` match an unknown `der(h)` --
    /// and paid for it with false positives wherever a name sat inside other
    /// text. `docs/identity-and-provenance.md` rules that out as a standing principle.
    #[test]
    fn same_variable_is_exact_modulo_der() {
        assert!(same_variable("h", "h"));
        assert!(same_variable("der(h)", "h"));
        assert!(same_variable("h", "der(h)"));
        assert!(same_variable("gear.flange_b.tau", "gear.flange_b.tau"));

        // A name inside another name is not the same variable.
        assert!(!same_variable("height", "h"));
        assert!(!same_variable("gear.h", "h"));
        assert!(!same_variable("inertia.J", "J"));
        assert!(!same_variable("gear.flange_b.tau", "tau"));
        // Rendered equation text is not a variable -- ask the lexer instead,
        // via `source_view::mentions_identifier`.
        assert!(!same_variable("der(h) - v", "h"));
    }

    #[test]
    fn strip_der_peels_one_layer() {
        assert_eq!(strip_der("der(h)"), "h");
        assert_eq!(strip_der("der(gear.phi)"), "gear.phi");
        assert_eq!(strip_der("h"), "h");
        assert_eq!(strip_der("order"), "order");
        // der(der(h)) is itself a variable in a reduced system.
        assert_eq!(strip_der("der(der(h))"), "der(h)");
        // Looks the part, but the opening paren closes early.
        assert_eq!(strip_der("der(a) + der(b)"), "der(a) + der(b)");
        assert_eq!(strip_der("der(a, b)"), "der(a, b)");
        assert_eq!(strip_der("der(h"), "der(h");
    }

    // --- IdentifierIndex::build ---

    /// Build an index from a minimal Dae with variables across two partitions
    /// and verify the resulting entries.
    #[test]
    fn build_index_from_dae() {
        use rumoca_core::{BytePos, SourceId, Span};

        // Source text: two declaration lines.
        let source_uri = "test://specimen.mo";
        let source_text = "Real h;\nReal v;\nparameter Real g = 9.81;\n";
        //                  ^0    ^6 ^8   ^14 ^16                   ^40
        let sid = SourceId::from_source_name(source_uri);

        let mut dae = dae::Dae::default();

        // h is a state on line 1, byte range 0..6.
        let h_var = dae::Variable {
            name: "h".into(),
            source_span: Span::new(sid, BytePos(0), BytePos(6)),
            ..dae::Variable::empty_with_span(Span::DUMMY)
        };
        dae.variables.states.insert("h".into(), h_var);

        // v is algebraic on line 2, byte range 8..14.
        let v_var = dae::Variable {
            name: "v".into(),
            source_span: Span::new(sid, BytePos(8), BytePos(14)),
            ..dae::Variable::empty_with_span(Span::DUMMY)
        };
        dae.variables.algebraics.insert("v".into(), v_var);

        // g is a parameter on line 3, byte range 16..40.
        let g_var = dae::Variable {
            name: "g".into(),
            source_span: Span::new(sid, BytePos(16), BytePos(40)),
            description: Some("gravitational accel".to_string()),
            ..dae::Variable::empty_with_span(Span::DUMMY)
        };
        dae.variables.parameters.insert("g".into(), g_var);

        let idx = IdentifierIndex::build(&dae, source_uri, source_text);

        // Three variables indexed.
        assert_eq!(idx.variables.len(), 3, "expected 3 variables");

        // Check kinds.
        assert_eq!(idx.variables["h"].kind, "state");
        assert_eq!(idx.variables["v"].kind, "algebraic");
        assert_eq!(idx.variables["g"].kind, "parameter");

        // Check source lines (1-based).
        assert_eq!(idx.variables["h"].source_line, 1);
        assert_eq!(idx.variables["v"].source_line, 2);
        assert_eq!(idx.variables["g"].source_line, 3);

        // Check description propagation.
        assert_eq!(idx.variables["g"].description.as_deref(), Some("gravitational accel"));
        assert!(idx.variables["h"].description.is_none());

        // Check line_to_variables reverse index.
        assert_eq!(idx.line_to_variables[&1], vec!["h"]);
        assert_eq!(idx.line_to_variables[&2], vec!["v"]);
        assert_eq!(idx.line_to_variables[&3], vec!["g"]);

        // variables_on_line returns the right entries.
        let line1 = idx.variables_on_line(1);
        assert_eq!(line1.len(), 1);
        assert_eq!(line1[0].name, "h");
        assert!(idx.variables_on_line(99).is_empty());
    }

    /// Variables from a non-matching source URI are excluded from the index.
    #[test]
    fn build_index_excludes_foreign_source() {
        use rumoca_core::{BytePos, SourceId, Span};

        let source_uri = "test://mine.mo";
        let source_text = "Real x;\n";
        let my_sid = SourceId::from_source_name(source_uri);
        let other_sid = SourceId::from_source_name("test://other.mo");

        let mut dae = dae::Dae::default();

        // x from the specimen source — should be indexed.
        let x_var = dae::Variable {
            name: "x".into(),
            source_span: Span::new(my_sid, BytePos(0), BytePos(6)),
            ..dae::Variable::empty_with_span(Span::DUMMY)
        };
        dae.variables.states.insert("x".into(), x_var);

        // y from a different source (e.g. library) — should be excluded.
        let y_var = dae::Variable {
            name: "y".into(),
            source_span: Span::new(other_sid, BytePos(0), BytePos(6)),
            ..dae::Variable::empty_with_span(Span::DUMMY)
        };
        dae.variables.algebraics.insert("y".into(), y_var);

        let idx = IdentifierIndex::build(&dae, source_uri, source_text);

        assert_eq!(idx.variables.len(), 1, "only specimen-source variables should be indexed");
        assert!(idx.variables.contains_key("x"));
        assert!(!idx.variables.contains_key("y"));
    }
}
