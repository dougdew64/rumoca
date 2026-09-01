//! Equation sheet — the flat DAE rendered as readable math.
//!
//! Built from the typed `Dae` in the worker thread (where the typed data
//! lives), then sent to the UI for display. The sheet groups equations by
//! origin category and lists variables by classification. When the specimen
//! source is available, each equation is linked back to its source line(s)
//! via the `span` byte offsets carried through the pipeline.

use std::collections::HashMap;

use eframe::egui;
use rumoca_core::{SourceId, VarName};
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

impl EquationSheet {
    /// The sheet as the bridge publishes it — **the renderer's input, serialized.**
    ///
    /// # Why this exists
    ///
    /// Doug, 2026-08-13, mid-run: *"I was just about to describe to you with a bunch
    /// of text what the HRW Flatten → Equations view is showing when I realized that we
    /// should implement a way for you to 'see' what that view is showing."* Claude
    /// cannot see the GUI, so every question about a pane was costing a manual
    /// transcription — and a transcription can be wrong in ways neither party notices.
    ///
    /// # The rule this function has to obey
    ///
    /// **It serializes the struct the renderer runs. It does not describe what was
    /// drawn.** A second implementation that re-derived "what the pane shows" would be
    /// a fiction generator of exactly the kind `CLAUDE.md` bans: plausible, unfalsifiable
    /// from the outside, and wrong the moment the renderer changes. `equation_sheet_ui`
    /// reads *this* value and nothing else, so publishing it is publishing the pane's
    /// content.
    ///
    /// **The honest bound:** a field present here that the renderer chooses not to draw
    /// is still published. The converse cannot happen — the renderer has no other source
    /// — which is the direction that matters.
    ///
    /// # Identity, which is the point
    ///
    /// Every row carries `id`, spelled **`f_x[N]`** — the same key the structural report
    /// uses (`"equation": "f_x[0] (equation from src)"`), so a row here and a row in the
    /// incidence matrix or the matching are *the same named object*. That is what lets
    /// Doug say *"why is **this** equation…"* and have the noun resolve, rather than
    /// having to invent a name for it (his deixis requirement, and charter Decision 8:
    /// *the noun is assembled by mouse, the verb is an unbounded utterance*).
    #[must_use]
    pub fn to_bridge_json(&self) -> serde_json::Value {
        let groups: Vec<serde_json::Value> = self
            .groups
            .iter()
            .map(|(category, equations)| {
                serde_json::json!({
                    "category": category.label(),
                    // `null` for a top-level group. Published because the nesting is a
                    // claim about *why* these equations exist, and a flat list of
                    // labels cannot express it.
                    "family": category.family(),
                    "description": category.description(),
                    "n": equations.len(),
                    "equations": equations
                        .iter()
                        .map(|e| serde_json::json!({
                            "id": format!("f_x[{}]", e.index),
                            "index": e.index,
                            "text": e.text,
                            "origin": e.origin,
                            "source_lines": e.source_lines,
                        }))
                        .collect::<Vec<_>>(),
                })
            })
            .collect();

        let variables: Vec<serde_json::Value> = self
            .variables
            .iter()
            .map(|v| {
                serde_json::json!({
                    "id": v.name,
                    "kind": v.kind,
                    // **The evidence, not a restatement of it.** The Why column and
                    // its tooltip are both derived from this field, so publishing the
                    // field is publishing the column — and Claude can answer "why is
                    // this a state" from the bridge instead of asking Doug to read a
                    // tooltip aloud.
                    //
                    // Added 2026-08-16, the same hour the column shipped: without it
                    // this function's own promise — that the renderer has no source
                    // the bridge lacks — had quietly become false.
                    "derivative_evidence": v.derivative_evidence.as_ref().map(|e| {
                        serde_json::json!({
                            "equation_id": e.equation_id,
                            "equation_text": e.equation_text,
                        })
                    }),
                    "start": v.start,
                    "unit": v.unit,
                })
            })
            .collect();

        serde_json::json!({
            "n_equations": self.n_equations,
            "groups": groups,
            "counts": {
                "states": self.n_states,
                "algebraics": self.n_algebraics,
                "parameters": self.n_parameters,
                "constants": self.n_constants,
                "discrete": self.n_discrete,
                "inputs": self.n_inputs,
                "outputs": self.n_outputs,
            },
            "variables": variables,
        })
    }
}

#[cfg(test)]
mod bridge_json_tests {
    use super::*;

    fn sheet_with(indices: &[usize]) -> EquationSheet {
        EquationSheet {
            groups: vec![(
                EquationCategory::Connection,
                indices
                    .iter()
                    .map(|&index| FormattedEquation {
                        index,
                        text: format!("0 = a{index} - b{index}"),
                        origin: "connection equation".to_owned(),
                        category: EquationCategory::Connection,
                        source_lines: vec![7],
                    })
                    .collect(),
            )],
            n_equations: indices.len(),
            ..EquationSheet::default()
        }
    }

    /// **Every published row carries the identity the other views use.**
    ///
    /// This is the deixis requirement in a test. Doug, 2026-08-13: *"I will want to make
    /// use of deixis and ask you questions such as 'Why is this partial derivative value
    /// so high…'"* — for *"this"* to resolve, a row here and a row in the incidence
    /// matrix have to be **the same named object**, not two renderings that happen to
    /// look alike. `f_x[N]` is that name: the structural report writes
    /// `"equation": "f_x[0] (equation from src)"`, so the ids join.
    ///
    /// **Publishing the text alone would not do it.** Text is ambiguous — two equations
    /// can format identically — and matching by string is the heuristic name-matching
    /// `docs/identity-and-provenance.md` forbids outright.
    #[test]
    fn every_published_equation_carries_its_cross_view_id() {
        let json = sheet_with(&[0, 19]).to_bridge_json();
        let rows = json["groups"][0]["equations"].as_array().expect("rows");

        assert_eq!(rows[0]["id"], "f_x[0]");
        assert_eq!(rows[1]["id"], "f_x[19]");
        // The id is derived from the index, so the two can never disagree.
        for row in rows {
            let index = row["index"].as_u64().expect("index");
            assert_eq!(row["id"], format!("f_x[{index}]"));
        }
        assert_eq!(json["groups"][0]["category"], "Potential equality");
        assert_eq!(json["groups"][0]["family"], "Connector equations");
        assert_eq!(rows[1]["text"], "0 = a19 - b19");
        assert_eq!(json["n_equations"], 2);
    }

    /// An empty sheet publishes an empty sheet, not nothing.
    ///
    /// **Absence is stated, never filled.** A model whose flatten produced no equations
    /// must read as *"this pane has no rows"* rather than as a missing field that Claude
    /// would have to guess the meaning of.
    #[test]
    fn an_empty_sheet_still_publishes_its_shape() {
        let json = EquationSheet::default().to_bridge_json();
        assert_eq!(json["n_equations"], 0);
        assert!(json["groups"].as_array().expect("groups").is_empty());
        assert!(
            json["counts"]["states"].is_number(),
            "counts are always present"
        );
    }
}

/// Broad categories for grouping equations in the sheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EquationCategory {
    Component,
    Connection,
    FlowSum,
    /// A flow variable with nothing connected to it, set to zero (MLS §9.2).
    ///
    /// **Its own category since 2026-08-13.** It was folded into `FlowSum`, which is a
    /// different statement: a flow *sum* says several flows cancel at a junction, while
    /// this says one flow has no junction at all. Rumoca distinguishes them
    /// (`EquationOrigin::UnconnectedFlow`) and the pane was discarding that.
    UnconnectedFlow,
    Binding,
    Event,
}

impl EquationCategory {
    /// The heading this group is drawn under.
    ///
    /// **`Connection` is called "Potential equality", not "Connection equations".**
    /// Doug, 2026-08-13: *"the equations pane implies that only the potential variables
    /// of connectors yield connection equations. The flow variables are presented as
    /// though they create some other kind of equations which are not connection
    /// equations."* He was right, and it was the pane asserting something false: all
    /// three of these come from expanding `connect`, so giving the family's name to one
    /// child made the other two look unrelated to it. Each child is now named for what
    /// it *says*, and [`Self::family`] carries what they share.
    ///
    /// The three map one-to-one onto `rumoca_ir_flat::EquationOrigin`, which is what
    /// keeps them faithful rather than merely clearer:
    ///
    /// | this label | Rumoca's variant |
    /// |---|---|
    /// | Potential equality | `Connection { lhs, rhs }` |
    /// | Flow conservation | `FlowSum { description }` |
    /// | Unconnected flow | `UnconnectedFlow { variable }` |
    pub fn label(self) -> &'static str {
        match self {
            Self::Component => "Component equations",
            Self::Connection => "Potential equality",
            Self::FlowSum => "Flow conservation",
            Self::UnconnectedFlow => "Unconnected flow",
            Self::Binding => "Bindings",
            Self::Event => "Event equations",
        }
    }

    /// The parent heading this group belongs under, if any.
    ///
    /// Only the `connect`-derived kinds have one, and they share it because they share
    /// a cause: every equation in this family exists because two connectors were joined.
    /// Grouping them visually is the pane finally saying what Rumoca's
    /// `EquationOriginKind::is_connect_generated` already knew.
    #[must_use]
    pub fn family(self) -> Option<&'static str> {
        match self {
            Self::Connection | Self::FlowSum | Self::UnconnectedFlow => Some("Connector equations"),
            _ => None,
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Component => "Equations from component instances (their equation sections)",
            Self::Connection => {
                "One connection set's potential variables made equal: n-1 equations for n connectors"
            }
            Self::FlowSum => {
                "One connection set's flow variables summed to zero: exactly 1 equation per set"
            }
            Self::UnconnectedFlow => {
                "A flow variable with no connection at all, set to zero (MLS 9.2)"
            }
            Self::Binding => "Variable bindings from declarations (parameter values, fixed starts)",
            Self::Event => "Discrete assignments from when/elsewhen clauses and reinit",
        }
    }

    pub fn color(self) -> egui::Color32 {
        match self {
            Self::Component => crate::colors::EQ_CAT_COMPONENT,
            Self::Connection => crate::colors::EQ_CAT_CONNECTION,
            // Shares the flow-sum colour: both are about flow variables, and the
            // family heading is what distinguishes them structurally.
            Self::FlowSum | Self::UnconnectedFlow => crate::colors::EQ_CAT_FLOW_SUM,
            Self::Binding => crate::colors::EQ_CAT_BINDING,
            Self::Event => crate::colors::EQ_CAT_EVENT,
        }
    }
}

/// Why a variable is a state: the equation that puts a derivative on it.
///
/// Doug, 2026-08-16, following `C.v` on the equation sheet: *"There's no hint
/// provided in the HRW UI as to why this is a state instead of an algebraic."*
/// The pane asserted a classification and left the reason to a conversation.
///
/// **It belongs on screen by the charter's own test** (Decision 8: *is the answer
/// known in advance?*). The answer's shape never varies — *because `der(x)` appears
/// in equation N* — only `N` changes. That is a fixed answer with a lookup, not a
/// question needing a reasoner, and a tooltip beats a question for latency.
///
/// The `equation_id` is the **same `f_x[N]`** the sheet, the incidence matrix, the
/// matching and the BLT blocks use, so the hover names an object Doug can go and
/// look at rather than a claim he has to take on trust.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivativeEvidence {
    /// `f_x[14]` — the index into `dae.continuous.equations`, spelled the way every
    /// other pane spells it.
    pub equation_id: String,
    /// The equation as this sheet renders it, through the same formatter, so two
    /// panes cannot show different text for one equation.
    pub equation_text: String,
}

/// A variable in the classification summary.
#[derive(Debug, Clone)]
pub struct ClassifiedVariable {
    pub name: String,
    pub kind: &'static str,
    pub unit: Option<String>,
    pub description: Option<String>,
    pub start: Option<String>,
    /// For a **state**, the equation whose derivative made it one.
    ///
    /// `None` for every other kind, and for algebraic that is itself the answer:
    /// a variable is algebraic *because* no equation differentiates it, so there is
    /// no equation to name. [`ClassifiedVariable::kind_explanation`] says that in
    /// words rather than leaving the hover blank — absence is stated, never filled.
    ///
    /// A **state** with `None` here would be a genuine finding: Rumoca would have
    /// partitioned a variable into `x` with no `der()` anywhere in `f_x`. The hover
    /// reports that case as a discrepancy instead of hiding it, and
    /// `every_state_names_the_equation_that_made_it_one` fails on it.
    pub derivative_evidence: Option<DerivativeEvidence>,
}

impl ClassifiedVariable {
    /// The reason, short enough for a table cell.
    ///
    /// **This is the visible half, and it is the half that was missing.** The first
    /// version of this feature put the explanation behind a tooltip on the Kind
    /// cell, and Doug reported the same complaint that had prompted it: *"I don't
    /// see what you've added to enable me to understand why `h` is a state."*
    /// Nothing on screen suggested there was anything to hover, so the answer was
    /// present and undiscoverable — which is indistinguishable from absent.
    ///
    /// A state reads `der in f_x[14]`; everything else is blank, and **the blank is
    /// the contrast**: scanning the column shows at a glance that exactly one
    /// variable in `RcCircuit` earns its classification from an equation.
    #[must_use]
    pub fn why_short(&self) -> String {
        match (self.kind, &self.derivative_evidence) {
            ("state", Some(e)) => format!("der in {}", e.equation_id),
            // Loud, because it means Rumoca and HRW disagree about this variable.
            ("state", None) => "der not found".to_owned(),
            _ => String::new(),
        }
    }

    /// One hover's worth of text: the classification, and why it holds.
    ///
    /// `None` for kinds whose reason is not an equation — `parameter`, `input`,
    /// `constant` and the rest are decided by the **declaration**, and inventing an
    /// equation-shaped story for them would be exactly the kind of plausible fiction
    /// `CLAUDE.md` bans. Only the state/algebraic split is derived from `f_x`, and
    /// only it is explained here.
    #[must_use]
    pub fn kind_explanation(&self) -> Option<String> {
        match (self.kind, &self.derivative_evidence) {
            ("state", Some(e)) => Some(format!(
                "State \u{2014} its derivative appears in {}:\n    {}\n\nA state is \
                 integrated over time, so it carries the model's memory and needs an \
                 initial condition.",
                e.equation_id, e.equation_text,
            )),
            ("state", None) => Some(format!(
                "State, but no equation in this DAE contains der({}). HRW cannot show \
                 why, and that disagreement is worth reporting rather than guessing at.",
                self.name,
            )),
            ("algebraic", _) => Some(format!(
                "Algebraic \u{2014} no equation contains der({}), so it is solved at \
                 each instant rather than integrated, and needs no initial condition.",
                self.name,
            )),
            _ => None,
        }
    }
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
#[derive(Debug, Clone, Default)]
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
    /// **Stage-tree node path -> the source line that equation came from.**
    ///
    /// The equation half of "relate this tree node to the Modelica it came from",
    /// which Doug asked for on 2026-08-05 after the variable tooltip shipped.
    ///
    /// **Keyed by path rather than by index, because the tree is type-agnostic**
    /// (charter §4.4): it renders any `serde_json::Value` and must not learn that
    /// `f_x[3]` is an equation. So this side knows the DAE's shape — the continuous
    /// equations serialize as `f_x`, per MLS Appendix B — and the tree only looks up
    /// the path it is already carrying.
    ///
    /// Built with [`crate::bridge::describe_path`] rather than by formatting a string
    /// here, so the key format **agrees with the tree's by construction** instead of
    /// by two functions being kept in step.
    ///
    /// Empty when no source was provided, and missing an entry for any equation whose
    /// span points into a library file rather than the specimen — absent rather than
    /// guessed, so a node with no known origin says nothing instead of something wrong.
    pub node_lines: HashMap<String, u32>,
    /// The same thing for the **Flatten** tree: node path -> source line.
    ///
    /// Separate from [`node_lines`](Self::node_lines) rather than merged, because the
    /// two index different systems and their paths would collide in spirit even where
    /// they do not in text: `equations[3]` in the flat model and `f_x[3]` in the DAE
    /// are not the same equation, since DAE construction reorders, filters and
    /// synthesises. **One map keyed by two conventions would resolve confidently and
    /// wrongly**, which is the failure this whole feature exists to avoid.
    ///
    /// Flatten is the one other tree worth this: measured 2026-08-05 it carries
    /// **1,856 spans** on `Drivetrain`, and it is where `connect` expansion happens —
    /// so *"which connect statement produced this equation?"* is the question that
    /// tree exists to answer. Index reduction, initialization and structural carry no
    /// spans at all and get nothing.
    pub flat_node_lines: HashMap<String, u32>,
}

/// Build the Flatten tree's path -> source-line map from the flat model.
///
/// Mirrors what [`build`] does for the DAE, and deliberately **does not** reuse the
/// DAE's map: the two number their equations differently.
///
/// **Only equations whose span belongs to the specimen get an entry.** A span from a
/// library file resolved against the specimen's text would name a line that exists and
/// is not the right one — a confident wrong answer, which is worse than none. The
/// comparison is on `SourceId`, not on whether the offset happens to fit.
pub fn flat_node_lines(
    flat: &rumoca_ir_flat::Model,
    source_info: Option<(&str, &str)>,
) -> HashMap<String, u32> {
    let mut out = HashMap::new();
    let Some((uri, src)) = source_info else {
        return out;
    };
    let sid = SourceId::from_source_name(uri);
    for (i, eq) in flat.equations.iter().enumerate() {
        if eq.span.source != sid {
            continue;
        }
        let line = byte_offset_to_line(src, eq.span.start.0);
        let path = [
            crate::bridge::Seg::Key("equations".to_owned()),
            crate::bridge::Seg::Index(i),
        ];
        out.insert(crate::bridge::describe_path(&path), line);
    }
    out
}

/// Classify an equation by the **kind Rumoca gave it**, not by reading its prose.
///
/// # What this replaced, and why it was wrong
///
/// Until 2026-08-13 this function tested prefixes itself —
/// `origin.starts_with("connection equation")`, `origin.contains("when")`, and so on.
/// That is **substring search deciding a classification**, which
/// `docs/identity-and-provenance.md` forbids outright, and it was a private guess at
/// what `rumoca_ir_flat::EquationOrigin`'s `Display` produces, linked to it by nothing.
///
/// [`EquationOriginKind::from_rendered`] is now the one inverse of that `Display`, lives
/// beside it in `rumoca-ir-flat`, and is proven against **every variant** by
/// `rendered_origins_round_trip_to_their_kind`. The guess became a checked mapping.
///
/// **Why parsing at all:** the typed origin does not survive the DAE boundary —
/// `rumoca_ir_dae::Equation::origin` is a `String` — so a consumer downstream of DAE
/// construction has only the rendered text. Carrying the type across would mean adding a
/// field to a struct built at **532 sites across ten crates**, which is not the additive
/// change the instrumentation rules require.
///
/// `Unknown` maps to `Component` **as a stated fallback, not a guess**: an unrecognised
/// origin is some equation the model produced, and `Component equations` is the group
/// that means "from the model itself".
fn categorize_origin(origin: &str) -> EquationCategory {
    match rumoca_ir_flat::EquationOriginKind::from_rendered(origin) {
        rumoca_ir_flat::EquationOriginKind::Connection => EquationCategory::Connection,
        rumoca_ir_flat::EquationOriginKind::FlowSum => EquationCategory::FlowSum,
        rumoca_ir_flat::EquationOriginKind::UnconnectedFlow => EquationCategory::UnconnectedFlow,
        rumoca_ir_flat::EquationOriginKind::Binding => EquationCategory::Binding,
        rumoca_ir_flat::EquationOriginKind::Reinit
        | rumoca_ir_flat::EquationOriginKind::WhenAssignment => EquationCategory::Event,
        rumoca_ir_flat::EquationOriginKind::ComponentEquation
        | rumoca_ir_flat::EquationOriginKind::Algorithm
        | rumoca_ir_flat::EquationOriginKind::Unknown => EquationCategory::Component,
    }
}

use crate::byte_offset_to_line;

/// Scan the specimen source for `connect(A, B)` statements. Returns a vec
/// of `(line_number, component_a, component_b)` where the components are
/// the dot-paths named in the connect call (e.g. "motor.flange", "rotor.flange_a").
fn scan_connect_statements(source: &str) -> Vec<(u32, String, String)> {
    let mut result = Vec::new();
    for (i, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("connect(")
            && let Some(args) = rest.strip_suffix(");")
            && let Some((a, b)) = args.split_once(',')
        {
            result.push((i as u32 + 1, a.trim().to_owned(), b.trim().to_owned()));
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
fn match_connection_to_source(origin: &str, connects: &[(u32, String, String)]) -> Vec<u32> {
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
    let mut lines: Vec<u32> = connects
        .iter()
        .filter(|(_, a, b)| {
            vars.iter()
                .any(|v| v.starts_with(a.as_str()) || v.starts_with(b.as_str()))
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
    let connects = source_text.map(scan_connect_statements).unwrap_or_default();

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
                        source_text
                            .and_then(|src| {
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
                        source_text
                            .and_then(|src| {
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

        by_category
            .entry(category)
            .or_default()
            .push(FormattedEquation {
                index: i,
                text,
                origin,
                category,
                source_lines,
            });
    }

    // BTreeMap iterates in key order, which uses the Ord impl (cmp_key),
    // so the display order is derived from the single source of truth.
    let groups: Vec<_> = by_category.into_iter().collect();

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

    /// Find the equation that differentiates `var_name`, if any.
    ///
    /// **This runs the expression tree; it does not search text.**
    /// `expr_contains_der_of` is Rumoca's own structural query — it visits
    /// `BuiltinCall { function: Der, .. }` nodes and asks whether the argument
    /// *refers to* this variable. Searching the rendered string for `"der(C.v)"`
    /// would be heuristic name-matching, which `docs/identity-and-provenance.md`
    /// forbids outright, and it would be wrong in a way nobody would notice:
    /// `der(C.v1)` contains `der(C.v)` as a substring, so the hover would cite a
    /// real equation about a different variable.
    ///
    /// Returns the **first** match. A variable can be differentiated in more than
    /// one equation; one real citation answers *why it is a state* better than a
    /// summary of several.
    fn derivative_evidence(dae: &dae::Dae, var_name: &VarName) -> Option<DerivativeEvidence> {
        dae.continuous
            .equations
            .iter()
            .enumerate()
            .find(|(_, eq)| rumoca_ir_dae::expr_contains_der_of(&eq.rhs, var_name))
            .map(|(index, eq)| DerivativeEvidence {
                // Spelled exactly as this sheet spells it above, so the hover names
                // a row Doug can navigate to rather than a number he must trust.
                equation_id: format!("f_x[{index}]"),
                equation_text: expr_format::format_equation(eq),
            })
    }

    fn collect_vars(
        vars: &mut Vec<ClassifiedVariable>,
        iter: impl Iterator<Item = (VarName, dae::Variable)>,
        kind: &'static str,
        dae: &dae::Dae,
    ) {
        for (var_name, v) in iter {
            vars.push(ClassifiedVariable {
                name: var_name.to_string(),
                kind,
                unit: v.unit.clone().filter(|u| !u.is_empty()),
                description: v.description.clone().filter(|d| !d.is_empty()),
                start: v.start.as_ref().map(expr_format::format_expr),
                // Only states are searched: for every other kind the scan would
                // cost a run of `f_x` to learn nothing, since their reason is
                // either the absence of a match or the declaration itself.
                derivative_evidence: (kind == "state")
                    .then(|| derivative_evidence(dae, &var_name))
                    .flatten(),
            });
        }
    }

    macro_rules! collect_from {
        ($map:expr, $kind:expr) => {
            collect_vars(
                &mut variables,
                $map.iter().map(|(n, v)| (n.clone(), v.clone())),
                $kind,
                dae,
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

    // **Path -> line, for the stage tree.** Built from the same `source_lines` the
    // sheet already resolved, so the tree and the sheet cannot disagree about where
    // an equation came from. `f_x` is how the continuous equations serialize.
    let mut node_lines: HashMap<String, u32> = HashMap::new();
    for (_, eqs) in &groups {
        for eq in eqs {
            if let Some(line) = eq.source_lines.first() {
                let path = [
                    crate::bridge::Seg::Key("f_x".to_owned()),
                    crate::bridge::Seg::Index(eq.index),
                ];
                node_lines.insert(crate::bridge::describe_path(&path), *line);
            }
        }
    }

    EquationSheet {
        node_lines,
        // Filled by the caller, which has the flat model; `build` only sees the DAE.
        flat_node_lines: HashMap::new(),
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
            // The three connect-derived kinds sort adjacently, so the family heading
            // they share covers a contiguous run rather than an interleaved one.
            Self::Connection => 1,
            Self::FlowSum => 2,
            Self::UnconnectedFlow => 3,
            Self::Binding => 4,
            Self::Event => 5,
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
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn categorize_origin_covers_all_variants() {
        // **Every input here is RENDERED BY RUMOCA, not written by hand.**
        //
        // The previous version of this test asserted on invented strings —
        // `"flow sum: ..."`, `"unconnected flow x"`, `"binding for p"` — none of which
        // `EquationOrigin`'s `Display` ever produces. It passed because the old
        // hand-rolled prefixes (`starts_with("flow sum")`) were loose enough to catch
        // them, so **the parser was being validated against fiction**: it proved the
        // categoriser handled strings that cannot occur, and proved nothing about the
        // ones that do. Found 2026-08-13 when the categoriser was tightened to Rumoca's
        // real vocabulary and this test failed on three rows.
        use rumoca_ir_flat::EquationOrigin;
        let rendered = |o: &EquationOrigin| o.to_string();

        assert_eq!(
            categorize_origin(&rendered(&EquationOrigin::ComponentEquation {
                component: "motor".into()
            })),
            EquationCategory::Component
        );
        assert_eq!(
            categorize_origin(&rendered(&EquationOrigin::ComponentEquation {
                component: String::new()
            })),
            EquationCategory::Component
        );
        assert_eq!(
            categorize_origin(&rendered(&EquationOrigin::Connection {
                lhs: "a".into(),
                rhs: "b".into()
            })),
            EquationCategory::Connection
        );
        assert_eq!(
            categorize_origin(&rendered(&EquationOrigin::FlowSum {
                description: "a + b = 0".into()
            })),
            EquationCategory::FlowSum
        );
        // **Its own category now**, not folded into FlowSum: "no junction at all" is a
        // different statement from "these flows cancel at a junction".
        assert_eq!(
            categorize_origin(&rendered(&EquationOrigin::UnconnectedFlow {
                variable: "c.n.i".into()
            })),
            EquationCategory::UnconnectedFlow
        );
        assert_eq!(
            categorize_origin(&rendered(&EquationOrigin::Binding {
                variable: "p".into()
            })),
            EquationCategory::Binding
        );
        assert_eq!(
            categorize_origin(&rendered(&EquationOrigin::Reinit { state: "v".into() })),
            EquationCategory::Event
        );
        assert_eq!(
            categorize_origin(&rendered(&EquationOrigin::WhenAssignment {
                target: "x".into()
            })),
            EquationCategory::Event
        );
        // An origin Rumoca does not produce is not silently binned as something
        // plausible; it lands in the group that means "from the model itself".
        assert_eq!(
            categorize_origin("something nobody writes"),
            EquationCategory::Component
        );
    }

    #[test]
    fn category_labels_are_non_empty() {
        for cat in [
            EquationCategory::Component,
            EquationCategory::Connection,
            EquationCategory::FlowSum,
            EquationCategory::UnconnectedFlow,
            EquationCategory::Binding,
            EquationCategory::Event,
        ] {
            assert!(!cat.label().is_empty());
            assert!(!cat.description().is_empty());
            // The family is the nesting claim; only the connect-derived kinds have
            // one, and they must all agree on it or the heading would split.
            assert_eq!(
                cat.family().is_some(),
                matches!(
                    cat,
                    EquationCategory::Connection
                        | EquationCategory::FlowSum
                        | EquationCategory::UnconnectedFlow
                ),
                "{} has the wrong family",
                cat.label(),
            );
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
            match_connection_to_source(
                "connection equation: motor.flange.phi = rotor.flange_a.phi",
                &conns
            ),
            vec![10],
        );
        // Flow sum with two vars from one connect → that line.
        assert_eq!(
            match_connection_to_source(
                "flow sum equation: motor.flange.tau + rotor.flange_a.tau = 0",
                &conns
            ),
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
            (
                54,
                "brakeTorque.flange".to_owned(),
                "load.flange_b".to_owned(),
            ),
        ];

        // Direct connection: both vars from line 50 → single line.
        assert_eq!(
            match_connection_to_source(
                "connection equation: load.flange_b.phi = spring.flange_a.phi",
                &conns
            ),
            vec![50],
        );

        // Transitive connection: spring.flange_a (line 50) = brakeTorque.flange
        // (line 54). No single connect has both → returns both lines.
        assert_eq!(
            match_connection_to_source(
                "connection equation: spring.flange_a.phi = brakeTorque.flange.phi",
                &conns
            ),
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
    #[cfg_attr(
        not(feature = "slow-tests"),
        ignore = "compile-heavy; run with --features slow-tests"
    )]
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
        let result =
            crate::worker::compile_specimen(&specimen, libraries).expect("compile_specimen");
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
        let has_linked = sheet
            .source_lines
            .iter()
            .any(|sl| !sl.equation_indices.is_empty());
        assert!(
            has_linked,
            "at least one source line should link to an equation"
        );
    }

    #[test]
    #[cfg_attr(
        not(feature = "slow-tests"),
        ignore = "compile-heavy; run with --features slow-tests"
    )]
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
        let result =
            crate::worker::compile_specimen(&specimen, libraries).expect("compile_specimen");
        let crate::worker::FromWorker::Compiled { equation_sheet, .. } = result else {
            panic!("expected Compiled");
        };
        let sheet = equation_sheet.expect("equation_sheet");

        let linked: Vec<_> = sheet
            .groups
            .iter()
            .flat_map(|(_, eqs)| eqs)
            .filter(|eq| !eq.source_lines.is_empty())
            .collect();
        let total = sheet.n_equations;

        assert_eq!(
            linked.len(),
            total,
            "expected all {total} equations linked, got {}",
            linked.len(),
        );

        // Connection equations should point to connect() lines (45-54).
        let conn_lines: Vec<u32> = sheet
            .groups
            .iter()
            .filter(|(cat, _)| *cat == EquationCategory::Connection)
            .flat_map(|(_, eqs)| eqs)
            .flat_map(|eq| eq.source_lines.iter().copied())
            .collect();
        assert!(
            !conn_lines.is_empty(),
            "connection equations should have source lines"
        );
        assert!(
            conn_lines.iter().all(|&ln| (45..=54).contains(&ln)),
            "connection equations should point to connect() lines (45-54), got {:?}",
            conn_lines,
        );
    }

    /// **An equation's tree node resolves to the line it was written on.**
    ///
    /// Doug, 2026-08-05: the variable tooltip worked and he asked for the same on
    /// equations. The plumbing is a path-keyed map because the tree is type-agnostic
    /// by charter §4.4 — so the thing that can go wrong is the **key format**, and
    /// that is what this pins.
    ///
    /// Built with `describe_path` on both sides so the formats agree by construction;
    /// this asserts the agreement holds rather than trusting it, because a key that
    /// never matches produces **no tooltip and no error** — the silent-absence failure
    /// this project keeps finding.
    #[test]
    #[cfg_attr(
        not(feature = "slow-tests"),
        ignore = "compile-heavy; run with --features slow-tests"
    )]
    fn an_equation_node_path_resolves_to_its_source_line() {
        use crate::worker::FromWorker;

        let FromWorker::Compiled { dae, .. } =
            crate::worker::test_msl::compile_specimen_shared("SingleInertia")
        else {
            panic!("SingleInertia must compile");
        };
        let dae = dae.expect("SingleInertia produces a DAE");
        let uri = concat!(env!("CARGO_MANIFEST_DIR"), "/specimens/SingleInertia.mo");
        let src = std::fs::read_to_string(uri).expect("read specimen");

        let sheet = build(&dae, Some((uri, &src)));
        assert!(
            !sheet.node_lines.is_empty(),
            "no equation resolved to a source line \u{2014} either the spans stopped \
             arriving or the key format changed, and both are silent in the UI",
        );

        // Every key must be a path the tree would actually produce for an equation.
        for (path, line) in &sheet.node_lines {
            assert!(
                path.starts_with("f_x"),
                "keys must be tree paths under the continuous equations: {path}",
            );
            assert!(*line >= 1, "lines are 1-based: {path} -> {line}");
        }
    }

    /// **The Flatten tree's equations resolve too, under their own key space.**
    ///
    /// Added 2026-08-05 when Doug asked whether the feature covered every relevant
    /// tree. It did not — variables did, equations were DAE-only.
    ///
    /// **The assertion that matters is the last one.** The two maps must not be
    /// merged: `equations[3]` in the flat model and `f_x[3]` in the DAE are different
    /// equations, because DAE construction reorders, filters and synthesises. A single
    /// map keyed by both conventions would resolve **confidently and wrongly**, which
    /// is the exact failure this feature exists to prevent.
    #[test]
    #[cfg_attr(
        not(feature = "slow-tests"),
        ignore = "compile-heavy; run with --features slow-tests"
    )]
    fn the_flatten_tree_resolves_equations_under_its_own_paths() {
        use crate::worker::FromWorker;

        let FromWorker::Compiled { dae, flat, .. } =
            crate::worker::test_msl::compile_specimen_shared("SingleInertia")
        else {
            panic!("SingleInertia must compile");
        };
        let dae = dae.expect("a DAE");
        let flat = flat.expect("a flat model");
        let uri = concat!(env!("CARGO_MANIFEST_DIR"), "/specimens/SingleInertia.mo");
        let src = std::fs::read_to_string(uri).expect("read specimen");

        let flat_lines = flat_node_lines(&flat, Some((uri, &src)));
        assert!(
            !flat_lines.is_empty(),
            "no flat equation resolved \u{2014} either the spans stopped arriving or \
             the key format changed, and both are silent in the UI",
        );
        for (path, line) in &flat_lines {
            assert!(
                path.starts_with("equations"),
                "flat keys live under `equations`, not the DAE's `f_x`: {path}",
            );
            assert!(*line >= 1, "lines are 1-based: {path} -> {line}");
        }

        // **No key is shared between the two.** Not a style point: a shared key would
        // silently answer a Flatten question with a DAE line.
        let dae_lines = build(&dae, Some((uri, &src))).node_lines;
        for k in flat_lines.keys() {
            assert!(
                !dae_lines.contains_key(k),
                "`{k}` appears in both maps \u{2014} the two number different systems, \
                 so a shared key resolves to the wrong equation",
            );
        }
    }

    /// **The hover explains state and algebraic, and stays silent about the rest.**
    ///
    /// Silence is the load-bearing half. A `parameter` is a parameter because it was
    /// *declared* one, not because of anything in `f_x`, so an equation-shaped
    /// sentence about it would be a plausible fiction — the exact failure mode
    /// `CLAUDE.md` spends most of its rules on. This pins that the explanation is
    /// offered only where HRW actually derived something.
    #[test]
    fn kind_explanation_covers_state_and_algebraic_and_says_nothing_else() {
        let base = ClassifiedVariable {
            name: "C.v".to_owned(),
            kind: "state",
            unit: None,
            description: None,
            start: None,
            derivative_evidence: Some(DerivativeEvidence {
                equation_id: "f_x[14]".to_owned(),
                equation_text: "0 = C.i - C.C * der(C.v)".to_owned(),
            }),
        };

        // A state with evidence cites the equation, by the id every pane shares.
        let text = base.kind_explanation().expect("a state is explained");
        assert!(text.contains("f_x[14]"), "must name the equation: {text}");
        assert!(
            text.contains("0 = C.i - C.C * der(C.v)"),
            "must quote it, so the claim is checkable on screen: {text}"
        );
        assert!(
            text.contains("initial condition"),
            "must say what being a state costs the solver: {text}"
        );

        // A state WITHOUT evidence reports the discrepancy rather than inventing one.
        let orphan = ClassifiedVariable {
            derivative_evidence: None,
            ..base.clone()
        };
        let text = orphan.kind_explanation().expect("still explained");
        assert!(
            text.contains("no equation") && text.contains("reporting"),
            "an unjustifiable state must read as a discrepancy, not as an answer: {text}"
        );

        // Algebraic states its absence directly.
        let alg = ClassifiedVariable {
            kind: "algebraic",
            name: "R.v".to_owned(),
            derivative_evidence: None,
            ..base.clone()
        };
        let text = alg.kind_explanation().expect("algebraic is explained");
        assert!(
            text.contains("der(R.v)") && text.contains("no equation"),
            "absence is stated, never left blank: {text}"
        );

        // Every other kind is silent.
        for kind in ["parameter", "constant", "input", "output", "discrete"] {
            let other = ClassifiedVariable {
                kind,
                derivative_evidence: None,
                ..base.clone()
            };
            assert!(
                other.kind_explanation().is_none(),
                "{kind} has no equation-shaped reason, so HRW must not invent one",
            );
            assert!(
                other.why_short().is_empty(),
                "{kind} must leave the Why column blank rather than assert something",
            );
        }
    }

    /// **The reason is VISIBLE, not only hoverable.**
    ///
    /// Doug looked at the finished table and said *"I don't see what you've added to
    /// enable me to understand why `h` is a state."* The explanation was there, behind
    /// a tooltip, with nothing on screen to suggest hovering. **An answer that cannot
    /// be discovered is indistinguishable from one that was never added**, which is
    /// the same failure the feature was built to fix.
    ///
    /// So the short reason is a column, and this pins that it names the equation
    /// rather than restating the classification. A cell reading "state" again would
    /// pass a naive test and teach nothing.
    #[test]
    fn the_why_column_names_the_equation_rather_than_repeating_the_kind() {
        let state = ClassifiedVariable {
            name: "h".to_owned(),
            kind: "state",
            unit: None,
            description: None,
            start: None,
            derivative_evidence: Some(DerivativeEvidence {
                equation_id: "f_x[3]".to_owned(),
                equation_text: "0 = der(h) - v".to_owned(),
            }),
        };
        let cell = state.why_short();
        assert!(
            cell.contains("f_x[3]"),
            "the visible cell must name the equation: {cell:?}"
        );
        assert!(
            !cell.contains("state"),
            "restating the Kind column teaches nothing: {cell:?}"
        );

        // An unjustifiable state is loud in the column too, not merely in the hover.
        let orphan = ClassifiedVariable {
            derivative_evidence: None,
            ..state.clone()
        };
        assert!(
            orphan.why_short().contains("not found"),
            "a state HRW cannot justify must be visible as such: {:?}",
            orphan.why_short()
        );
    }

    /// **Whatever the pane draws, the bridge publishes.**
    ///
    /// `to_bridge_json`'s doc comment promises that a field the renderer draws cannot
    /// be missing here, *"because the renderer has no other source"*. Adding the Why
    /// column made that promise false for one hour: the column is drawn from
    /// `derivative_evidence`, which the bridge did not carry, so `view.json` described
    /// a pane that no longer existed — and Claude, asked about the Why column, would
    /// have had to invent one.
    ///
    /// **A promise in a doc comment is not a mechanism**, which is why this is a test
    /// rather than a firmer sentence.
    #[test]
    fn the_published_variables_carry_the_evidence_the_pane_draws() {
        let sheet = EquationSheet {
            variables: vec![
                ClassifiedVariable {
                    name: "h".to_owned(),
                    kind: "state",
                    unit: Some("m".to_owned()),
                    description: None,
                    start: Some("1.0".to_owned()),
                    derivative_evidence: Some(DerivativeEvidence {
                        equation_id: "f_x[3]".to_owned(),
                        equation_text: "0 = der(h) - v".to_owned(),
                    }),
                },
                ClassifiedVariable {
                    name: "g".to_owned(),
                    kind: "parameter",
                    unit: None,
                    description: None,
                    start: None,
                    derivative_evidence: None,
                },
            ],
            ..EquationSheet::default()
        };

        let json = sheet.to_bridge_json();
        let rows = json["variables"].as_array().expect("variables");

        assert_eq!(rows[0]["id"], "h");
        assert_eq!(
            rows[0]["derivative_evidence"]["equation_id"], "f_x[3]",
            "a state's evidence must reach the bridge, or the Why column is invisible \
             to anything reading view.json",
        );
        assert_eq!(
            rows[0]["derivative_evidence"]["equation_text"],
            "0 = der(h) - v"
        );
        assert!(
            rows[1]["derivative_evidence"].is_null(),
            "a parameter has no equation-shaped reason and must publish null rather \
             than an empty object that reads as one",
        );
    }

    /// **Every state names the equation that made it one, and that equation really
    /// differentiates it.**
    ///
    /// Doug, 2026-08-16: *"There's no hint provided in the HRW UI as to why this is a
    /// state instead of an algebraic."* This is the must-fire half of the fix: the
    /// hover is a *claim about Rumoca's partitioning*, and an unbacked claim is worse
    /// than the blank label it replaced.
    ///
    /// Two things are checked per state, on a real compile:
    ///
    /// 1. **Evidence exists.** A state with none would mean Rumoca put a variable in
    ///    `x` with no `der()` anywhere in `f_x` — a genuine finding about the compiler,
    ///    not a rendering bug.
    /// 2. **The cited equation is the right one.** The id is re-resolved against the
    ///    sheet's own equation list and required to differentiate *this* variable, so
    ///    an off-by-one in the `f_x[N]` spelling fails here rather than sending Doug to
    ///    a plausible neighbour.
    #[cfg_attr(
        not(feature = "slow-tests"),
        ignore = "compile-heavy; run with --features slow-tests"
    )]
    #[test]
    fn every_state_names_the_equation_that_made_it_one() {
        let mut states_checked = 0usize;

        for specimen in [
            "RcCircuit",
            "BouncingBall",
            "RotationalInertia",
            "Drivetrain",
        ] {
            let crate::worker::FromWorker::Compiled { equation_sheet, .. } =
                crate::worker::test_msl::compile_specimen_shared(specimen)
            else {
                panic!("{specimen} should compile");
            };
            let Some(sheet) = equation_sheet else {
                continue;
            };

            for v in sheet.variables.iter().filter(|v| v.kind == "state") {
                states_checked += 1;
                let e = v.derivative_evidence.as_ref().unwrap_or_else(|| {
                    panic!(
                        "{specimen}: `{}` is a state with no equation to justify it, so \
                         the hover cannot say why it is one",
                        v.name
                    )
                });

                // The id must resolve, and to an equation that mentions this
                // variable's derivative. `format_equation` is the same renderer the
                // sheet uses, so the text here is the text on screen.
                let index: usize = e
                    .equation_id
                    .trim_start_matches("f_x[")
                    .trim_end_matches(']')
                    .parse()
                    .unwrap_or_else(|_| panic!("{specimen}: malformed id {:?}", e.equation_id));
                let cited = sheet
                    .groups
                    .iter()
                    .flat_map(|(_, eqs)| eqs.iter())
                    .find(|q| q.index == index)
                    .unwrap_or_else(|| {
                        panic!(
                            "{specimen}: `{}` cites {} , which names no equation in the \
                             sheet \u{2014} the hover would send Doug nowhere",
                            v.name, e.equation_id
                        )
                    });
                assert_eq!(
                    cited.text, e.equation_text,
                    "{specimen}: the hover for `{}` quotes different text than the sheet \
                     shows for {}",
                    v.name, e.equation_id,
                );
                assert!(
                    e.equation_text.contains("der("),
                    "{specimen}: `{}` cites {} as its reason, but that equation has no \
                     derivative in it: {}",
                    v.name,
                    e.equation_id,
                    e.equation_text,
                );
            }
        }

        assert!(
            states_checked >= 8,
            "only {states_checked} states were checked across four specimens; this is \
             not exercising what it claims",
        );
    }
}
