//! Equation sheet — the flat DAE rendered as readable math.
//!
//! Built from the typed `Dae` in the worker thread (where the typed data
//! lives), then sent to the UI for display. The sheet groups equations by
//! origin category and lists variables by classification. When the specimen
//! source is available, each equation is linked back to its source line(s)
//! via the `span` byte offsets carried through the pipeline.

use std::collections::HashMap;

use eframe::egui;
use rumoca_core::{SourceId, Span};
use rumoca_ir_dae as dae;


/// Per-role variable totals plus the equation count, tallied in one pass.
///
/// Replaces eight `dae.variables.<partition>.len()` reads with one iteration, so
/// these can no longer disagree with the variable rows the sheet lists.
#[derive(Debug, Default)]
struct RoleCounts {
    n_equations: usize,
    n_states: usize,
    n_algebraics: usize,
    n_parameters: usize,
    n_constants: usize,
    n_discrete: usize,
    n_inputs: usize,
    n_outputs: usize,
}

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
    /// An equation Rumoca synthesised whose kind has no group of its own here.
    ///
    /// **Added for the v0.10.0 port, and it exists to avoid a false claim.**
    /// Before 0.10.0 an equation carried `origin: String` and anything HRW did
    /// not recognise fell into [`Self::Component`] as a stated fallback. 0.10.0
    /// replaces that string with a typed [`dae::DaeGeneration`] of 24 variants,
    /// and most of them are *not* model equations —
    /// `IndexReduction`, `AliasElimination`, `SyntheticResidual`,
    /// `ArrayEquationProjection` and the rest are things the compiler made.
    /// Routing those to `Component`, whose heading says "Component equations"
    /// and whose description says *from component instances*, would assert the
    /// model's author wrote them.
    ///
    /// The group is deliberately coarse: the per-equation `origin` field carries
    /// Rumoca's own phrasing for the exact kind, so nothing is lost by not
    /// inventing a heading per variant.
    Generated,
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
            Self::Generated => "Compiler-generated equations",
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
            Self::Generated => {
                "Equations Rumoca synthesised; each row's origin names the exact kind"
            }
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
            Self::Generated => crate::colors::EQ_CAT_GENERATED,
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
/// # No text is parsed any more, and that closes a three-year workaround
///
/// Until 2026-08-13 this function tested prefixes itself —
/// `origin.starts_with("connection equation")`, `origin.contains("when")` — which is
/// **substring search deciding a classification**, forbidden outright by
/// `docs/identity-and-provenance.md`. That became a checked inverse of Rumoca's
/// `Display` (`EquationOriginKind::from_rendered`), which was better but still a
/// round-trip through rendered English.
///
/// **The reason it had to parse at all is gone.** The note below is kept verbatim
/// because it states the constraint exactly, and v0.10.0 removes it:
/// `DaeProvenance::origin()` carries a typed [`dae::DaeProvenanceOrigin`] —
/// `Source`, or `Generated` with one of 24 [`dae::DaeGeneration`] kinds — so the
/// typed origin now *does* survive the DAE boundary and this is a plain match.
/// The 532-site problem it describes was solved upstream by moving the
/// classification onto provenance rather than onto the equation.
///
/// > **Why parsing at all:** the typed origin does not survive the DAE boundary —
/// > `rumoca_ir_dae::Equation::origin` is a `String` — so a consumer downstream of DAE
/// > construction has only the rendered text. Carrying the type across would mean adding a
/// > field to a struct built at **532 sites across ten crates**, which is not the additive
/// > change the instrumentation rules require.
///
/// **There is no fallback arm now, and that is the point.** Every one of the 24
/// generations is named, so a kind added upstream fails to compile here and gets
/// a decision. The old `Unknown => Component` fallback could not have stayed
/// anyway: most generations are *not* model equations, and `Component equations`
/// means "from the model itself", so the fallback would have begun asserting
/// that Rumoca's synthesised residuals and index-reduction equations were
/// hand-written. Those go to [`EquationCategory::Generated`].
pub fn categorize_origin(origin: dae::DaeProvenanceOrigin) -> EquationCategory {
    use dae::DaeGeneration as G;

    let generation = match origin {
        // Not generated at all: the model's author wrote this equation.
        dae::DaeProvenanceOrigin::Source => return EquationCategory::Component,
        dae::DaeProvenanceOrigin::Generated(generation) => generation,
    };
    match generation {
        G::ConnectionEquation => EquationCategory::Connection,
        G::FlowBalanceEquation => EquationCategory::FlowSum,
        G::UnconnectedFlowEquation => EquationCategory::UnconnectedFlow,
        G::BindingEquation => EquationCategory::Binding,
        // The three ways a model's event behaviour becomes an equation: a
        // `when` body's assignment, a `reinit`, and the lowering of the
        // condition that guards them.
        G::DiscreteUpdate | G::EventActionLowering | G::ConditionLowering => {
            EquationCategory::Event
        }
        // An algorithm section is written by the author even though construction
        // marks it generated, so it groups with the model's own equations --
        // which is where the old string-based mapping put it too.
        G::AlgorithmEquation => EquationCategory::Component,
        // Everything else is the compiler's own work. Listed rather than left to
        // a wildcard so a generation added upstream fails to compile here and
        // gets a decision, instead of silently joining this group.
        G::SyntheticResidual
        | G::PreValueLowering
        | G::ClockLowering
        | G::DelayLowering
        | G::SemiLinearLowering
        | G::TerminalLowering
        | G::InitializationEquation
        | G::DefaultStart
        | G::ArrayEquationProjection
        | G::RecordEquationProjection
        | G::FunctionLoopLowering
        | G::FunctionConditionLowering
        | G::FunctionAggregateLowering
        | G::DerivedParameterLowering
        | G::IndexReduction
        | G::AliasElimination
        | G::RuntimeDiscontinuity => EquationCategory::Generated,
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

    // **One `inspect` for the whole sheet.** A 0.10.0 `Dae` is reachable only
    // through a `DaeView` under a higher-ranked lifetime, so nothing borrowed
    // from it escapes this closure — which is why the loop collects owned
    // `FormattedEquation`s rather than returning views.
    let equations: Vec<(usize, String, String, dae::DaeProvenanceOrigin, Span)> =
        dae.inspect(|view| {
            (0..view.continuous_equation_count())
                .filter_map(|index| {
                    let equation = view.continuous_equation(index)?;
                    let provenance = equation.provenance();
                    Some((
                        index,
                        crate::dae_expr_format::format_equation(view, equation),
                        // Rumoca's own phrasing for the kind, published rather
                        // than relabelled: "connection equation",
                        // "unconnected-flow equation", "source DAE object".
                        provenance.origin().to_string(),
                        provenance.origin(),
                        provenance.span(),
                    ))
                })
                .collect()
        });

    for (i, text, origin, provenance_origin, span) in equations {
        let category = categorize_origin(provenance_origin);

        // Try span-based matching first (direct specimen equations).
        let mut source_lines: Vec<u32> = match (specimen_sid, source_text) {
            (Some(sid), Some(src)) if span.source == sid => {
                vec![byte_offset_to_line(src, span.start.0)]
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
    /// Find the equation that differentiates `state`, if any.
    ///
    /// **Now an identity comparison rather than a name one.** 0.10.0's
    /// `expr_contains_der_of` takes a branded [`dae::StateId`] and matches
    /// `CoordinateView::Derivative(candidate) if candidate == state`, so the
    /// question "does this equation differentiate *this* variable" is answered
    /// by the arena's own identity instead of by asking whether a reference
    /// *refers to* a name. That removes the last way the `der(C.v1)` /
    /// `der(C.v)` confusion described above could ever have come back.
    fn derivative_evidence<'dae>(
        view: dae::DaeView<'dae>,
        state: dae::StateId<'dae>,
    ) -> Option<DerivativeEvidence> {
        (0..view.continuous_equation_count())
            .filter_map(|index| view.continuous_equation(index).map(|eq| (index, eq)))
            .find(|(_, eq)| {
                rumoca_ir_dae::expr_contains_der_of(view, eq.residual(), state)
            })
            .map(|(index, eq)| DerivativeEvidence {
                // Spelled exactly as this sheet spells it above, so the hover names
                // a row Doug can navigate to rather than a number he must trust.
                equation_id: format!("f_x[{index}]"),
                equation_text: crate::dae_expr_format::format_equation(view, eq),
            })
    }

    // **One iteration over roles, replacing eight partition reads.** The kind
    // strings are `identifier_index::role_label`'s, the single home for that
    // mapping — except that this sheet has always collapsed both discrete roles
    // to one `"discrete"` bucket, which is preserved below rather than silently
    // split by the shared labels.
    dae.inspect(|view| {
        for (_id, var) in view.variables() {
            let kind = match var.role() {
                dae::VariableRole::DiscreteReal | dae::VariableRole::DiscreteValue => "discrete",
                role => crate::identifier_index::role_label(role),
            };
            variables.push(ClassifiedVariable {
                name: var.name().to_string(),
                kind,
                unit: var
                    .unit()
                    .filter(|u| !u.is_empty())
                    .map(str::to_owned),
                description: var
                    .description()
                    .filter(|d| !d.is_empty())
                    .map(str::to_owned),
                start: var
                    .start()
                    .map(|start| crate::dae_expr_format::format_expr(view, start)),
                // Only states are searched: for every other kind the scan would
                // cost a run of `f_x` to learn nothing, since their reason is
                // either the absence of a match or the declaration itself.
                derivative_evidence: match var.identity() {
                    dae::VariableIdentity::State(state) => derivative_evidence(view, state),
                    _ => None,
                },
            });
        }
    });

    // **The per-role totals, tallied from the same iteration the sheet reports.**
    // These were eight `dae.variables.<partition>.len()` reads. Counting by role
    // here keeps them from being able to disagree with `variables` above: a role
    // this sheet buckets differently still lands in exactly one counter.
    let counts = dae.inspect(|view| {
        let mut counts = RoleCounts {
            n_equations: view.continuous_equation_count(),
            ..RoleCounts::default()
        };
        for (_id, var) in view.variables() {
            let counter = match var.role() {
                dae::VariableRole::State => &mut counts.n_states,
                dae::VariableRole::Algebraic => &mut counts.n_algebraics,
                dae::VariableRole::Parameter => &mut counts.n_parameters,
                dae::VariableRole::Constant => &mut counts.n_constants,
                dae::VariableRole::Input => &mut counts.n_inputs,
                dae::VariableRole::Output => &mut counts.n_outputs,
                // One bucket for both, as this sheet has always reported them.
                dae::VariableRole::DiscreteReal | dae::VariableRole::DiscreteValue => {
                    &mut counts.n_discrete
                }
            };
            *counter += 1;
        }
        counts
    });

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
        n_equations: counts.n_equations,
        groups,
        n_states: counts.n_states,
        n_algebraics: counts.n_algebraics,
        n_parameters: counts.n_parameters,
        n_constants: counts.n_constants,
        n_discrete: counts.n_discrete,
        n_inputs: counts.n_inputs,
        n_outputs: counts.n_outputs,
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
            Self::Generated => 6,
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

    /// Each generation lands in the group that states what it is.
    ///
    /// **No strings any more, and that is the change.** This test used to render
    /// `rumoca_ir_flat::EquationOrigin` through `Display` and feed the text to a
    /// parser — itself a repair of a worse version that asserted on invented
    /// strings `Display` never produces, and so validated the parser against
    /// fiction. v0.10.0 carries a typed [`dae::DaeProvenanceOrigin`] across the
    /// DAE boundary, so there is no text in the path at all.
    ///
    /// The flat-origin half of the mapping is now pinned where it belongs, in
    /// the crate that owns it: `rumoca-phase-dae`'s
    /// `an_unconnected_flow_is_distinct_from_a_flow_balance`. This half only has
    /// to get the generations into the right groups.
    #[test]
    fn each_generation_lands_in_the_group_that_states_what_it_is() {
        use dae::DaeGeneration as G;
        use dae::DaeProvenanceOrigin::{Generated, Source};

        // An equation the author wrote.
        assert_eq!(categorize_origin(Source), EquationCategory::Component);

        for (generation, expected) in [
            (G::ConnectionEquation, EquationCategory::Connection),
            (G::FlowBalanceEquation, EquationCategory::FlowSum),
            // Its own group, not folded into the flow sum: "no junction at all"
            // is a different statement from "these flows cancel at a junction".
            (G::UnconnectedFlowEquation, EquationCategory::UnconnectedFlow),
            (G::BindingEquation, EquationCategory::Binding),
            (G::DiscreteUpdate, EquationCategory::Event),
            (G::EventActionLowering, EquationCategory::Event),
            (G::ConditionLowering, EquationCategory::Event),
            // Written by the author, even though construction marks it generated.
            (G::AlgorithmEquation, EquationCategory::Component),
            (G::IndexReduction, EquationCategory::Generated),
            (G::SyntheticResidual, EquationCategory::Generated),
            (G::AliasElimination, EquationCategory::Generated),
            (G::InitializationEquation, EquationCategory::Generated),
        ] {
            assert_eq!(
                categorize_origin(Generated(generation)),
                expected,
                "{generation} landed in the wrong group",
            );
        }
    }

    /// Only an algorithm section may be grouped with the model's own equations.
    ///
    /// `Component`'s heading reads "Component equations" and its description
    /// says *from component instances*, so putting a compiler-made equation
    /// there asserts the author wrote it. The old mapping had a
    /// `Unknown => Component` fallback that would do exactly that once the
    /// origin became a 24-variant enum, most of whose members are the
    /// compiler's own work.
    ///
    /// The list is written out, so it can go stale if upstream adds a
    /// generation — but the exhaustive `match` in [`categorize_origin`] fails to
    /// compile in that case, which is the guard that cannot rot. This test
    /// catches the other direction: a careless edit re-routing one of these into
    /// `Component`.
    #[test]
    fn no_compiler_generation_claims_to_be_a_model_equation() {
        use dae::DaeGeneration as G;
        use dae::DaeProvenanceOrigin::Generated;

        for generation in [
            G::SyntheticResidual,
            G::BindingEquation,
            G::ConnectionEquation,
            G::FlowBalanceEquation,
            G::UnconnectedFlowEquation,
            G::DiscreteUpdate,
            G::ConditionLowering,
            G::PreValueLowering,
            G::ClockLowering,
            G::DelayLowering,
            G::SemiLinearLowering,
            G::TerminalLowering,
            G::EventActionLowering,
            G::InitializationEquation,
            G::DefaultStart,
            G::ArrayEquationProjection,
            G::RecordEquationProjection,
            G::FunctionLoopLowering,
            G::FunctionConditionLowering,
            G::FunctionAggregateLowering,
            G::DerivedParameterLowering,
            G::IndexReduction,
            G::AliasElimination,
            G::RuntimeDiscontinuity,
        ] {
            assert_ne!(
                categorize_origin(Generated(generation)),
                EquationCategory::Component,
                "{generation} would be presented as an equation the model's author wrote",
            );
        }
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
