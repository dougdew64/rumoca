//! Index-reduction process view — a structured summary of the dummy-derivative
//! funnel's transformations.
//!
//! ## What is index reduction?
//!
//! A **differential-algebraic equation (DAE)** system has an "index" that
//! measures how far it is from being an ordinary differential equation (ODE).
//! A DAE of index > 1 cannot be solved directly by standard numerical solvers
//! (like BDF or Runge-Kutta) — it must first be reduced to index 1.
//!
//! **Example:** A mechanical system with constraints (like two gears meshing)
//! produces a high-index DAE because the constraint equations relate positions
//! (not velocities or accelerations). The constraint must be differentiated to
//! introduce velocity-level equations, and some "state" variables must be
//! demoted to "algebraic" variables (they are no longer independently integrated
//! but are instead determined by the constraints).
//!
//! Rumoca uses the **Pantelides algorithm** combined with **dummy derivatives**
//! to perform this reduction. The process runs through a "funnel" of steps:
//!
//! 1. **Demotion steps:** identify state variables that are actually determined
//!    by constraints (aliases, directly assigned, etc.) and demote them to
//!    algebraic. Each step targets a specific pattern (e.g., `demote_exact_alias`
//!    catches `x = y` constraints).
//!
//! 2. **Differentiation:** for states that lost their derivative equation to
//!    demotion, manufacture a new equation by differentiating a constraint.
//!
//! 3. **Elimination:** remove trivially determined variables by substitution
//!    (e.g., if `z = y`, eliminate `z` everywhere and replace it with `y`).
//!
//! ## Why this is a panel, not a canvas
//!
//! Unlike the spy-plot and incidence views (spatial, custom-painted on a
//! pan/zoom canvas), the index-reduction story is *sequential*: a pipeline of
//! named steps with outcomes. A scrollable text panel with tables is the
//! natural fit — no coordinate transforms or hit-testing needed.

use eframe::egui;
use serde_json::Value;

use crate::json_read::parse_list;

use crate::str_vec;

/// Parsed index-reduction report, ready for rendering.
///
/// Built from the `reduction` sub-object of the structural report JSON.
/// The fields mirror the JSON structure but are strongly typed.
pub struct ReductionView {
    // Did the funnel complete successfully (all steps ran without error)?
    funnel_completed: bool,
    // If the funnel stopped early, which step it stopped at.
    stopped_at: Option<String>,
    // Number of state (differential) variables before and after reduction.
    // The difference = number of demoted states.
    n_states_before: usize,
    n_states_after: usize,
    // Names of variables that were demoted from state to algebraic.
    demoted_states: Vec<String>,
    // Each funnel step, with the system's shape either side of it.
    steps: Vec<StepRow>,
    // Equations manufactured by differentiating a constraint **and still present at
    // the end**. Empty does NOT mean no differentiation happened — see below.
    differentiated_rows: Vec<DiffRow>,
    /// **Differentiations the funnel actually performed**, from its own frames.
    ///
    /// The pane reported differentiation only when `differentiated_rows` was
    /// non-empty, so on `Drivetrain` — six differentiations, none surviving
    /// `eliminate_trivial` — it said **nothing at all**. A lab read that silence as
    /// zero and taught the opposite of the truth for its whole existence.
    n_differentiations: usize,
    // Variables removed by symbolic substitution.
    eliminations: Vec<Elimination>,
    /// **What the report contained that this view could not read.**
    ///
    /// Added 2026-08-04 by the tech-debt sweep. Every list above used to be built
    /// with `filter_map`, so an entry the parser could not understand was **silently
    /// dropped** — the pane then showed four differentiated rows where the compiler
    /// had produced five, with nothing on screen where the fifth had been.
    ///
    /// **A partial report leaves no gap where the missing part was**, which is the
    /// same defect the Context Bar had, and the reason `CLAUDE.md` requires a pane to
    /// ship with a test. It matters most here of anywhere in HRW: `differentiated_rows`
    /// *is* the Pantelides output, and the tech-debt file's own priority-1 example is
    /// a case where reasoning about that list being empty would have been confidently
    /// wrong.
    unreadable: Vec<String>,
}

/// One funnel step, with the system's shape on either side of it.
///
/// **The shapes are what let the pane say whether a step acted.** An outcome string
/// reports that a step ran; only the pair of shapes reports what it did — and
/// `CartesianPendulum` runs every step to completion while moving nothing, which
/// looked identical to quiet work until these numbers arrived.
struct StepRow {
    name: String,
    outcome: String,
    states_before: usize,
    states_after: usize,
    equations_before: usize,
    equations_after: usize,
}

// An equation that was created by differentiating an existing constraint,
// to provide a derivative row for a state that needed one after demotion
// changed the equation structure.
struct DiffRow {
    // Which constraint was differentiated (e.g., "index_reduction:d_dt_for_x").
    equation_origin: String,
    // Which state variable this new equation serves.
    for_state: String,
}

// A variable that was eliminated by symbolic substitution.
// Example: z was replaced everywhere by y (because the system had z = y).
struct Elimination {
    variable: String,
    // The replacement expression, pre-rendered to a human-readable string at
    // construction time (avoiding per-frame JSON re-parsing).
    display: String,
}

impl ReductionView {
    /// Parse the `reduction` sub-object from a structural report JSON.
    ///
    /// Returns `None` if the report has no reduction data (e.g., the model
    /// is already index-1 and no reduction was needed, or the structural
    /// phase failed before reaching index reduction).
    ///
    /// # Missing is not the same as unreadable
    ///
    /// *(Rewritten 2026-08-04.)* This used to say the parsing was *"defensive: each
    /// field uses `?` or `.unwrap_or_default()` to handle missing/malformed data
    /// **gracefully**"* — and graceful meant **silent**. The two cases were collapsed:
    ///
    /// - **Absent** — the compiler produced no such list. Legitimate, extremely
    ///   common (an already-index-1 model has no eliminations), and correctly shown
    ///   as nothing.
    /// - **Present but unreadable** — the list is there and this parser could not
    ///   understand it, or one entry in it. That is a *defect*, and rendering it as
    ///   an empty list tells the reader the compiler did nothing.
    ///
    /// They now go different ways: absent yields an empty list, unreadable is
    /// recorded in [`unreadable`](Self::unreadable) and shown in the pane. **No entry
    /// is ever dropped without a count of it appearing on screen.**
    pub fn from_report(report: &Value) -> Option<ReductionView> {
        let red = report.get("reduction")?;

        let funnel_completed = red.get("funnel_completed")?.as_bool()?;
        let stopped_at = red
            .get("stopped_at")
            .and_then(|v| v.as_str())
            .map(str::to_owned);
        let n_states_before = red.get("n_states_before")?.as_u64()? as usize;
        let n_states_after = red.get("n_states_after")?.as_u64()? as usize;

        let demoted_states = str_vec(red.get("demoted_states"));

        let mut unreadable = Vec::new();

        let steps: Vec<StepRow> = parse_list(red, "steps", &mut unreadable, |s| {
            let num = |k: &str| s.get(k).and_then(serde_json::Value::as_u64).unwrap_or(0) as usize;
            Some(StepRow {
                name: s.get("step")?.as_str()?.to_owned(),
                outcome: s.get("outcome")?.as_str()?.to_owned(),
                states_before: num("states_before"),
                states_after: num("states_after"),
                equations_before: num("equations_before"),
                equations_after: num("equations_after"),
            })
        });

        let differentiated_rows: Vec<DiffRow> =
            parse_list(red, "differentiated_rows", &mut unreadable, |r| {
                Some(DiffRow {
                    equation_origin: r.get("equation_origin")?.as_str()?.to_owned(),
                    for_state: r.get("for_state")?.as_str()?.to_owned(),
                })
            });

        let eliminations: Vec<Elimination> =
            parse_list(red, "eliminations", &mut unreadable, |e| {
                let replacement = e.get("replacement")?.as_str()?;
                Some(Elimination {
                    variable: e.get("variable")?.as_str()?.to_owned(),
                    display: abbreviate_expr(replacement),
                })
            });

        Some(ReductionView {
            funnel_completed,
            stopped_at,
            n_states_before,
            n_states_after,
            demoted_states,
            steps,
            differentiated_rows,
            n_differentiations: red
                .get("n_differentiations")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0) as usize,
            eliminations,
            unreadable,
        })
    }

    /// Render the full reduction view as a scrollable panel.
    ///
    /// The view is organized into sections, each conditionally shown:
    /// 1. Summary (always) — completion status, state counts
    /// 2. Funnel steps (always) — the step-by-step pipeline with outcomes
    /// 3. Demoted states — which variables were demoted (if any)
    /// 4. Differentiated equations — manufactured equations (if any)
    /// 5. Trivial eliminations — substituted-away variables (if any)
    pub fn ui(&self, ui: &mut egui::Ui, tracked: Option<&str>) {
        egui::ScrollArea::both()
            .id_salt("reduction_view")
            .auto_shrink(false)
            .show(ui, |ui| {
                // **First, and in the error colour**: anything the report contained
                // that this view could not read. It goes above the summary because a
                // reader who scrolls past it would be reading an incomplete list
                // believing it complete — which is the failure this records.
                for problem in &self.unreadable {
                    ui.colored_label(ui.visuals().error_fg_color, format!("\u{26a0} {problem}"));
                }
                if !self.unreadable.is_empty() {
                    ui.add_space(8.0);
                }
                self.summary(ui);
                ui.add_space(8.0);
                self.funnel_steps(ui);
                if !self.demoted_states.is_empty() {
                    ui.add_space(8.0);
                    self.demoted_section(ui, tracked);
                }
                if !self.differentiated_rows.is_empty() {
                    ui.add_space(8.0);
                    self.differentiated_section(ui, tracked);
                }
                if !self.eliminations.is_empty() {
                    ui.add_space(8.0);
                    self.elimination_section(ui, tracked);
                }
            });
    }

    // Top-level summary: funnel completion status and state variable counts.
    fn summary(&self, ui: &mut egui::Ui) {
        ui.heading("Index Reduction Summary");
        ui.add_space(4.0);

        let status = if self.funnel_completed {
            egui::RichText::new("funnel completed")
                .color(crate::colors::ok_color(ui.visuals().dark_mode))
        } else {
            let step = self.stopped_at.as_deref().unwrap_or("unknown");
            egui::RichText::new(format!("stopped at {step}")).color(ui.visuals().error_fg_color)
        };
        ui.label(status);

        let n_demoted = self.demoted_states.len();
        ui.label(format!(
            "{} state{} before \u{2192} {} after ({} demoted)",
            self.n_states_before,
            if self.n_states_before == 1 { "" } else { "s" },
            self.n_states_after,
            n_demoted,
        ));
        // **Reported whenever the funnel differentiated, not only when rows survived.**
        // Keyed on `n_differentiations`, because the survivor list is empty on every
        // specimen in the corpus that differentiates at all — so keying on it meant the
        // pane was silent exactly when it had the most to say.
        if self.n_differentiations > 0 {
            let survivors = self.differentiated_rows.len();
            ui.label(format!(
                "{} differentiation{} performed \u{2014} {} of the manufactured \
                 equation{} survive{} to the end",
                self.n_differentiations,
                if self.n_differentiations == 1 {
                    ""
                } else {
                    "s"
                },
                survivors,
                if survivors == 1 { "" } else { "s" },
                if survivors == 1 { "s" } else { "" },
            ));
            if survivors == 0 {
                // The gap is the interesting part, so it is explained where it is seen
                // rather than left as two numbers that appear to contradict each other.
                ui.label(
                    egui::RichText::new(
                        "the rows they produced were removed by a later elimination step",
                    )
                    .weak(),
                );
            }
        } else if self.n_states_before == self.n_states_after && self.n_states_before > 0 {
            // **A funnel that did nothing says so.** `CartesianPendulum` runs every step
            // to completion and changes nothing; an unannotated pane of zeroes reads as
            // "reduction happened and was small" rather than "reduction did not act".
            ui.label(
                egui::RichText::new(
                    "no differentiation and no demotion \u{2014} this funnel did not act \
                     on the system",
                )
                .weak(),
            );
        }
        if !self.eliminations.is_empty() {
            ui.label(format!(
                "{} variable{} eliminated by substitution",
                self.eliminations.len(),
                if self.eliminations.len() == 1 {
                    ""
                } else {
                    "s"
                },
            ));
        }
    }

    // Render the funnel step table: each row is a named step with its outcome,
    // color-coded: green for successful reductions, red for stops, dim for no-ops.
    // Step names have their common prefix stripped (e.g., "demote_" is removed)
    // for readability.
    fn funnel_steps(&self, ui: &mut egui::Ui) {
        ui.strong("Funnel steps");
        ui.add_space(2.0);

        let ok_color = crate::colors::ok_color(ui.visuals().dark_mode);
        let err_color = ui.visuals().error_fg_color;
        let neutral_color = ui.visuals().weak_text_color();

        // **Three columns, because the outcome string alone cannot say whether a step
        // acted.** An outcome of "ok" reports that a step ran; the shape either side
        // reports what it did. `CartesianPendulum` runs every step to completion and
        // moves nothing, and until these columns existed it looked exactly like a
        // funnel doing quiet work.
        egui::Grid::new("funnel_steps")
            .num_columns(3)
            .spacing([12.0, 4.0])
            .show(ui, |ui| {
                for row in &self.steps {
                    let short = row
                        .name
                        .strip_prefix("demote_")
                        .or_else(|| row.name.strip_prefix("reduce_"))
                        .or_else(|| row.name.strip_prefix("index_reduce_"))
                        .unwrap_or(&row.name);
                    ui.label(egui::RichText::new(short).monospace());

                    let is_err = row.outcome.starts_with("stopped");
                    let acted = row.states_before != row.states_after
                        || row.equations_before != row.equations_after;
                    let color = if is_err {
                        err_color
                    } else if acted {
                        ok_color
                    } else {
                        // **Keyed on the shape, not on the outcome text.** The old
                        // version tested for the literal strings "0 demoted" and
                        // "0 substituted", so a step reporting "ok" while changing
                        // nothing was coloured as though it had worked — and the
                        // wording changed under it twice this week.
                        neutral_color
                    };
                    ui.label(egui::RichText::new(&row.outcome).color(color));

                    // Shown only where something moved. A column of unchanged pairs
                    // beside every row is noise that hides the two rows that matter.
                    if acted {
                        let mut parts: Vec<String> = Vec::new();
                        if row.states_before != row.states_after {
                            parts.push(format!(
                                "states {}\u{2192}{}",
                                row.states_before, row.states_after
                            ));
                        }
                        if row.equations_before != row.equations_after {
                            parts.push(format!(
                                "eqs {}\u{2192}{}",
                                row.equations_before, row.equations_after
                            ));
                        }
                        ui.label(egui::RichText::new(parts.join("  ")).weak().monospace());
                    } else {
                        ui.label("");
                    }
                    ui.end_row();
                }
            });
    }

    // List of demoted state variables — these were differential variables in the
    // original DAE but were reclassified as algebraic by the reduction funnel.
    fn demoted_section(&self, ui: &mut egui::Ui, tracked: Option<&str>) {
        ui.strong(format!("Demoted states ({})", self.demoted_states.len()));
        ui.add_space(2.0);
        ui.label(
            egui::RichText::new(
                "These variables were state (differential) variables in the raw DAE; \
                 the reduction funnel demoted them to algebraic.",
            )
            .weak(),
        );
        ui.add_space(2.0);
        for name in &self.demoted_states {
            let is_tracked =
                tracked.is_some_and(|t| crate::identifier_index::same_variable(name, t));
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("\u{2022}").color(ui.visuals().warn_fg_color));
                let mut rt = egui::RichText::new(name).monospace();
                if is_tracked {
                    rt = rt
                        .strong()
                        .background_color(crate::colors::TRACKED_FILL_MEDIUM);
                }
                ui.label(rt);
            });
        }
    }

    // Table of equations that were manufactured by differentiating existing
    // constraints. When a state variable's original derivative equation is
    // disrupted by demotion, the compiler differentiates a constraint to
    // produce a replacement.
    fn differentiated_section(&self, ui: &mut egui::Ui, tracked: Option<&str>) {
        ui.strong(format!(
            "Differentiated equations ({})",
            self.differentiated_rows.len()
        ));
        ui.add_space(2.0);
        ui.label(
            egui::RichText::new(
                "Equations manufactured by differentiating a constraint to produce \
                 a derivative row for a state that lacked one.",
            )
            .weak(),
        );
        ui.add_space(2.0);
        egui::Grid::new("diff_rows")
            .num_columns(2)
            .spacing([12.0, 4.0])
            .show(ui, |ui| {
                ui.label(egui::RichText::new("for state").weak());
                ui.label(egui::RichText::new("equation origin").weak());
                ui.end_row();
                for row in &self.differentiated_rows {
                    let is_tracked = tracked
                        .is_some_and(|t| crate::identifier_index::same_variable(&row.for_state, t));
                    let mut rt = egui::RichText::new(&row.for_state).monospace();
                    if is_tracked {
                        rt = rt
                            .strong()
                            .background_color(crate::colors::TRACKED_FILL_MEDIUM);
                    }
                    ui.label(rt);
                    ui.label(egui::RichText::new(&row.equation_origin).monospace().weak());
                    ui.end_row();
                }
            });
    }

    // Table of trivially eliminated variables. When the system contains
    // equations like `z = y` (a single-unknown row or an alias), the variable
    // `z` can be replaced everywhere by `y`, reducing the system size.
    fn elimination_section(&self, ui: &mut egui::Ui, tracked: Option<&str>) {
        ui.strong(format!(
            "Trivial eliminations ({})",
            self.eliminations.len()
        ));
        ui.add_space(2.0);
        ui.label(
            egui::RichText::new(
                "Variables removed by symbolic substitution (aliases, single-unknown rows).",
            )
            .weak(),
        );
        ui.add_space(2.0);
        egui::Grid::new("eliminations")
            .num_columns(2)
            .spacing([12.0, 4.0])
            .show(ui, |ui| {
                ui.label(egui::RichText::new("variable").weak());
                ui.label(egui::RichText::new("replaced by").weak());
                ui.end_row();
                for elim in &self.eliminations {
                    let is_tracked = tracked
                        .is_some_and(|t| crate::identifier_index::same_variable(&elim.variable, t));
                    let mut rt = egui::RichText::new(&elim.variable).monospace();
                    if is_tracked {
                        rt = rt
                            .strong()
                            .background_color(crate::colors::TRACKED_FILL_MEDIUM);
                    }
                    ui.label(rt);
                    ui.label(egui::RichText::new(&elim.display).monospace().weak());
                    ui.end_row();
                }
            });
    }
}

// Convert a JSON-encoded Rumoca expression into a short human-readable string.
//
// The `replacement` field in eliminations stores the replacement expression as a
// JSON string (it's a serialized Rumoca IR expression). Showing raw JSON in the
// UI would be unreadable, so this function pattern-matches on common expression
// kinds and renders them concisely:
//   - VarRef -> just the variable name (e.g., "omega")
//   - Literal -> the literal value (e.g., "3.14", "true")
//   - Binary -> "lhs op rhs" (e.g., "x + y")
//   - Unary -> "op rhs" (e.g., "-x")
//   - BuiltinCall -> "func(args)" (e.g., "sin(theta)")
//   - anything else -> "(expr)" as a fallback
pub(crate) fn abbreviate_expr(json_expr: &str) -> String {
    if let Ok(v) = serde_json::from_str::<Value>(json_expr) {
        expr_to_short(&v)
    } else {
        // Not valid JSON — show the raw string (shouldn't happen in practice).
        json_expr.to_owned()
    }
}

fn binary_op_symbol(variant: &str) -> &str {
    match variant {
        "Add" => "+",
        "Sub" => "-",
        "Mul" => "*",
        "Div" => "/",
        "Eq" => "==",
        "Neq" => "<>",
        "Lt" => "<",
        "Le" => "<=",
        "Gt" => ">",
        "Ge" => ">=",
        "And" => "and",
        "Or" => "or",
        "Exp" => "^",
        "ExpElem" => ".^",
        "AddElem" => ".+",
        "SubElem" => ".-",
        "MulElem" => ".*",
        "DivElem" => "./",
        "Assign" => "=",
        _ => variant,
    }
}

fn unary_op_symbol(variant: &str) -> &str {
    match variant {
        "Minus" => "-",
        "Plus" => "+",
        "DotMinus" => ".-",
        "DotPlus" => ".+",
        "Not" => "not ",
        _ => variant,
    }
}

/// Stands in for a subexpression this renderer could not find.
///
/// **It has to be visible, and it has to not look like Modelica.** Until 2026-08-04
/// a missing operand rendered as the empty string, so `a * 2` with an unreadable
/// left side became `" * 2"` and `x + y` became `"x + "` — **a different equation,
/// displayed as the model's equation**, which is the same class of defect as the
/// incidence matrix showing a dependency that is not there.
///
/// Distinct from `"(expr)"`, which this function already used and which means
/// something else: *an expression shape this renderer does not know how to print*.
/// That one is a limit of HRW; this one is a hole in the data.
const MISSING: &str = "(missing)";

// Recursive expression-to-string renderer. Each branch pattern-matches on the
// Rumoca IR expression enum variant (serialized as a JSON object with one key).
pub(crate) fn expr_to_short(v: &Value) -> String {
    match v.get("VarRef").or_else(|| v.get("Literal")) {
        Some(inner) => {
            if let Some(name) = inner.get("name").and_then(|n| n.as_str()) {
                return name.to_owned();
            }
            if let Some(val) = inner.get("value") {
                if let Some(r) = val.get("Real").and_then(|r| r.as_f64()) {
                    return format!("{r}");
                }
                if let Some(i) = val.get("Integer").and_then(|i| i.as_i64()) {
                    return format!("{i}");
                }
                if let Some(b) = val.get("Bool").and_then(|b| b.as_bool()) {
                    return format!("{b}");
                }
                if let Some(s) = val.get("String").and_then(|s| s.as_str()) {
                    return format!("\"{s}\"");
                }
            }
            "(expr)".to_owned()
        }
        None => {
            if let Some(bin) = v.get("Binary") {
                let op = bin.get("op").and_then(|o| o.as_str()).unwrap_or("?");
                let sym = binary_op_symbol(op);
                let lhs = bin
                    .get("lhs")
                    .map_or_else(|| MISSING.to_owned(), expr_to_short);
                let rhs = bin
                    .get("rhs")
                    .map_or_else(|| MISSING.to_owned(), expr_to_short);
                return format!("{lhs} {sym} {rhs}");
            }
            if let Some(unary) = v.get("Unary") {
                let op = unary.get("op").and_then(|o| o.as_str()).unwrap_or("?");
                let sym = unary_op_symbol(op);
                let rhs = unary
                    .get("rhs")
                    .map_or_else(|| MISSING.to_owned(), expr_to_short);
                return format!("{sym}{rhs}");
            }
            if let Some(call) = v.get("BuiltinCall") {
                // **`"f"` was the worst substitution in this function.** An unreadable
                // function name rendered as a call to a function literally named `f`,
                // which is a plausible Modelica identifier — so `sin(x)` with an
                // unreadable name became `f(x)`, and nothing said otherwise.
                let func = call
                    .get("function")
                    .and_then(|f| f.as_str())
                    .unwrap_or("(unknown fn)");
                // Absent args and an empty args list are different expressions:
                // `time()` really takes none, while a missing `args` key means this
                // parser could not find them. `unwrap_or_default` rendered both as
                // a zero-argument call.
                let args: String = match call.get("args").and_then(Value::as_array) {
                    Some(a) => a.iter().map(expr_to_short).collect::<Vec<_>>().join(", "),
                    None => MISSING.to_owned(),
                };
                return format!("{func}({args})");
            }
            "(expr)".to_owned()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_reduction_report() {
        let report = json!({
            "reduction": {
                "funnel_completed": true,
                "stopped_at": null,
                "n_states_before": 4,
                "n_states_after": 2,
                "states_before": ["a", "b", "c", "d"],
                "states_after": ["a", "c"],
                "demoted_states": ["b", "d"],
                "steps": [
                    { "step": "demote_exact_alias_component_states", "outcome": "1 demoted" },
                    { "step": "demote_direct_assigned_states", "outcome": "1 demoted" },
                ],
                "differentiated_rows": [
                    { "equation_origin": "index_reduction:d_dt_for_x", "for_state": "x" }
                ],
                "eliminations": [
                    { "variable": "z", "replacement": "{\"VarRef\":{\"name\":\"y\"}}" }
                ],
            }
        });
        let view = ReductionView::from_report(&report).expect("should parse");
        assert!(view.funnel_completed);
        assert_eq!(view.n_states_before, 4);
        assert_eq!(view.n_states_after, 2);
        assert_eq!(view.demoted_states, vec!["b", "d"]);
        assert_eq!(view.steps.len(), 2);
        assert_eq!(view.differentiated_rows.len(), 1);
        assert_eq!(view.eliminations.len(), 1);
    }

    #[test]
    fn missing_reduction_returns_none() {
        assert!(ReductionView::from_report(&json!({})).is_none());
        assert!(ReductionView::from_report(&json!({"blocks": []})).is_none());
    }

    #[test]
    fn abbreviate_renders_var_ref() {
        let expr = r#"{"VarRef":{"name":"omega","subscripts":[],"span":{"lo":0,"hi":0}}}"#;
        assert_eq!(abbreviate_expr(expr), "omega");
    }

    #[test]
    fn abbreviate_renders_literal_real() {
        let expr = r#"{"Literal":{"value":{"Real":3.14}}}"#;
        assert_eq!(abbreviate_expr(expr), "3.14");
    }

    #[test]
    fn abbreviate_renders_literal_integer() {
        let expr = r#"{"Literal":{"value":{"Integer":42}}}"#;
        assert_eq!(abbreviate_expr(expr), "42");
    }

    #[test]
    fn abbreviate_renders_literal_bool() {
        let expr = r#"{"Literal":{"value":{"Bool":true}}}"#;
        assert_eq!(abbreviate_expr(expr), "true");
    }

    #[test]
    fn abbreviate_renders_literal_string() {
        let expr = r#"{"Literal":{"value":{"String":"hello"}}}"#;
        assert_eq!(abbreviate_expr(expr), "\"hello\"");
    }

    #[test]
    fn abbreviate_renders_binary() {
        let expr = r#"{"Binary":{"op":"Add","lhs":{"VarRef":{"name":"x"}},"rhs":{"VarRef":{"name":"y"}}}}"#;
        assert_eq!(abbreviate_expr(expr), "x + y");
    }

    #[test]
    fn abbreviate_renders_binary_operators() {
        let bin = |op| {
            format!(
                r#"{{"Binary":{{"op":"{op}","lhs":{{"VarRef":{{"name":"x"}}}},"rhs":{{"VarRef":{{"name":"y"}}}}}}}}"#
            )
        };
        assert_eq!(abbreviate_expr(&bin("Sub")), "x - y");
        assert_eq!(abbreviate_expr(&bin("Mul")), "x * y");
        assert_eq!(abbreviate_expr(&bin("Div")), "x / y");
        assert_eq!(abbreviate_expr(&bin("Exp")), "x ^ y");
        assert_eq!(abbreviate_expr(&bin("Ge")), "x >= y");
        assert_eq!(abbreviate_expr(&bin("And")), "x and y");
    }

    #[test]
    fn abbreviate_renders_unary() {
        let expr = r#"{"Unary":{"op":"Minus","rhs":{"VarRef":{"name":"x"}}}}"#;
        assert_eq!(abbreviate_expr(expr), "-x");
    }

    #[test]
    fn abbreviate_renders_unary_not() {
        let expr = r#"{"Unary":{"op":"Not","rhs":{"VarRef":{"name":"b"}}}}"#;
        assert_eq!(abbreviate_expr(expr), "not b");
    }

    #[test]
    fn abbreviate_renders_builtin_call() {
        let expr = r#"{"BuiltinCall":{"function":"sin","args":[{"VarRef":{"name":"theta"}}]}}"#;
        assert_eq!(abbreviate_expr(expr), "sin(theta)");
    }

    #[test]
    fn abbreviate_renders_builtin_call_multiple_args() {
        let expr = r#"{"BuiltinCall":{"function":"atan2","args":[{"VarRef":{"name":"y"}},{"VarRef":{"name":"x"}}]}}"#;
        assert_eq!(abbreviate_expr(expr), "atan2(y, x)");
    }

    #[test]
    fn abbreviate_nested_binary_in_builtin() {
        let expr = r#"{"BuiltinCall":{"function":"abs","args":[{"Binary":{"op":"Sub","lhs":{"VarRef":{"name":"a"}},"rhs":{"VarRef":{"name":"b"}}}}]}}"#;
        assert_eq!(abbreviate_expr(expr), "abs(a - b)");
    }

    #[test]
    fn abbreviate_unknown_variant_falls_back() {
        let expr = r#"{"FunctionCall":{"name":"custom","args":[]}}"#;
        assert_eq!(abbreviate_expr(expr), "(expr)");
    }

    #[test]
    fn abbreviate_invalid_json_returns_raw() {
        assert_eq!(abbreviate_expr("not json"), "not json");
    }

    #[test]
    fn elimination_display_uses_abbreviate() {
        let report = json!({
            "reduction": {
                "funnel_completed": true,
                "n_states_before": 2,
                "n_states_after": 1,
                "demoted_states": ["z"],
                "steps": [],
                "differentiated_rows": [],
                "eliminations": [
                    {
                        "variable": "z",
                        "replacement": "{\"Binary\":{\"op\":\"Mul\",\"lhs\":{\"VarRef\":{\"name\":\"a\"}},\"rhs\":{\"Literal\":{\"value\":{\"Real\":2.0}}}}}"
                    }
                ],
            }
        });
        let view = ReductionView::from_report(&report).expect("should parse");
        assert_eq!(view.eliminations[0].variable, "z");
        assert_eq!(view.eliminations[0].display, "a * 2");
    }

    /// A minimal report with a well-formed `reduction` object, for the two tests
    /// below to perturb. Kept tiny so the perturbation is the only difference.
    fn report_with(rows: serde_json::Value) -> Value {
        serde_json::json!({
            "reduction": {
                "funnel_completed": true,
                "n_states_before": 2,
                "n_states_after": 1,
                "differentiated_rows": rows,
            }
        })
    }

    /// **A row the parser cannot read is reported, not dropped.**
    ///
    /// This is the defect the 2026-08-04 sweep found: every list here was built with
    /// `filter_map`, so an entry missing a field vanished and the pane showed a
    /// shorter list with **nothing where the missing row had been**. A partial report
    /// leaves no gap, so nothing prompts a second look.
    ///
    /// It matters most on this view of any in HRW. `differentiated_rows` *is* the
    /// Pantelides output, and `tech-debt.md`'s priority-1 example is a case where
    /// reasoning from that list would have been confidently wrong.
    #[test]
    fn an_unreadable_row_is_counted_rather_than_silently_dropped() {
        let report = report_with(serde_json::json!([
            { "equation_origin": "eq[1]", "for_state": "phi" },
            { "equation_origin": "eq[2]" },
        ]));
        let view = ReductionView::from_report(&report).expect("should parse");

        assert_eq!(
            view.differentiated_rows.len(),
            1,
            "the malformed row has nothing to render, so it is still excluded",
        );
        assert_eq!(
            view.unreadable.len(),
            1,
            "but its absence must be REPORTED. Dropping it silently shows one \
             differentiated equation where the compiler produced two, and the pane \
             gives the reader no reason to doubt it",
        );
        assert!(
            view.unreadable[0].contains("1 of 2")
                && view.unreadable[0].contains("differentiated_rows"),
            "the notice must say how many and which list: {:?}",
            view.unreadable[0],
        );
    }

    /// **Absent is not unreadable.** The other half, and the one that keeps the
    /// notice meaningful: an already-index-1 model has no differentiated rows at all,
    /// and saying so in red would train the reader to ignore the warning.
    #[test]
    fn a_list_the_compiler_never_emitted_is_not_a_problem() {
        let report = serde_json::json!({
            "reduction": {
                "funnel_completed": true,
                "n_states_before": 1,
                "n_states_after": 1,
            }
        });
        let view = ReductionView::from_report(&report).expect("should parse");
        assert!(view.differentiated_rows.is_empty());
        assert!(
            view.unreadable.is_empty(),
            "a list the compiler never produced is not a parse failure: {:?}",
            view.unreadable,
        );

        // And a well-formed list is likewise silent.
        let clean = report_with(serde_json::json!([
            { "equation_origin": "eq[1]", "for_state": "phi" },
        ]));
        let view = ReductionView::from_report(&clean).expect("should parse");
        assert_eq!(view.differentiated_rows.len(), 1);
        assert!(view.unreadable.is_empty(), "{:?}", view.unreadable);
    }

    /// **A missing operand is visible, not blank.** The sweep's finding, 2026-08-04.
    ///
    /// `expr_to_short` rendered an absent `lhs`/`rhs` as the empty string, so the
    /// pane displayed a *different equation* as the model's equation — with correct
    /// spacing and plausible syntax, giving a reader no reason to doubt it. Equations
    /// are what Doug is learning from; a wrong one is as damaging here as a wrong
    /// incidence matrix.
    #[test]
    fn a_missing_operand_renders_visibly_rather_than_as_nothing() {
        // `a * <missing>` — the shape that used to render as "a *".
        let half = json!({ "Binary": {
            "op": "Mul",
            "lhs": { "VarRef": { "name": "a" } },
        }});
        let out = expr_to_short(&half);
        assert!(
            out.contains("(missing)"),
            "a missing operand must be visible: {out:?}",
        );
        assert!(
            !out.ends_with("* "),
            "and must not read as a well-formed expression: {out:?}"
        );

        // A unary with no operand used to render as a bare "-".
        let unary = expr_to_short(&json!({ "Unary": { "op": "Neg" } }));
        assert!(unary.contains("(missing)"), "{unary:?}");
    }

    /// **An unreadable function name does not become a plausible one.**
    ///
    /// It rendered as `"f"` — a legal Modelica identifier — so `sin(x)` with an
    /// unreadable name became `f(x)`, indistinguishable from a real call to a
    /// function named `f`. The substitution was not just lossy, it was *convincing*.
    #[test]
    fn an_unreadable_function_name_is_not_replaced_by_a_plausible_one() {
        let call = expr_to_short(&json!({ "BuiltinCall": {
            "args": [{ "VarRef": { "name": "x" } }],
        }}));
        assert!(
            !call.starts_with("f("),
            "a plausible fake name is worse than none: {call:?}"
        );
        assert!(call.contains("unknown fn"), "{call:?}");

        // Absent args and an empty args list are different expressions.
        let no_args = expr_to_short(&json!({ "BuiltinCall": { "function": "sin" } }));
        assert!(
            no_args.contains("(missing)"),
            "absent args must say so: {no_args:?}"
        );
        let empty_args =
            expr_to_short(&json!({ "BuiltinCall": { "function": "time", "args": [] }}));
        assert_eq!(
            empty_args, "time()",
            "a genuine zero-argument call is unchanged"
        );
    }

    /// A well-formed expression is untouched by all of the above.
    #[test]
    fn a_complete_expression_still_renders_normally() {
        let e = json!({ "Binary": {
            "op": "Add",
            "lhs": { "VarRef": { "name": "x" } },
            "rhs": { "VarRef": { "name": "y" } },
        }});
        assert_eq!(expr_to_short(&e), "x + y");
    }

    /// A list that is present but is not a list at all is a defect too — the shape
    /// changed under us, which is exactly what the fidelity suite exists to notice
    /// and what this view would otherwise render as "nothing happened".
    #[test]
    fn a_list_that_is_not_a_list_is_reported() {
        let report = report_with(serde_json::json!("eq[1], eq[2]"));
        let view = ReductionView::from_report(&report).expect("should parse");
        assert!(view.differentiated_rows.is_empty());
        assert_eq!(view.unreadable.len(), 1, "{:?}", view.unreadable);
        assert!(
            view.unreadable[0].contains("is not a list"),
            "{:?}",
            view.unreadable[0]
        );
    }
}

#[cfg(test)]
mod tests_differentiation_count {
    use super::*;
    use serde_json::json;

    fn report(n_differentiations: u64, survivors: usize) -> Value {
        json!({
            "reduction": {
                "funnel_completed": true,
                "stopped_at": null,
                "n_states_before": 9,
                "n_states_after": 3,
                "states_before": [], "states_after": [],
                "demoted_states": [],
                "steps": [],
                "n_differentiations": n_differentiations,
                "differentiated_rows": (0..survivors)
                    .map(|i| json!({
                        "equation_origin": format!("index_reduction:d_dt_for_x{i}"),
                        "for_state": format!("x{i}"),
                    }))
                    .collect::<Vec<_>>(),
                "eliminations": [],
            }
        })
    }

    /// **The pane knows how many differentiations happened, not just how many rows
    /// survived.**
    ///
    /// `Drivetrain` differentiates six times and retains none, and the pane reported
    /// differentiation *only* when the survivor list was non-empty — so it was silent
    /// exactly when it had the most to say. A lab read that silence as zero and taught
    /// the opposite of the truth for its whole existence (`DECISIONS.md`, 2026-08-17).
    #[test]
    fn the_view_carries_differentiations_performed_separately_from_survivors() {
        let view = ReductionView::from_report(&report(6, 0)).expect("the report parses");
        assert_eq!(
            view.n_differentiations, 6,
            "the count of differentiations performed must survive into the view; it is \
             the number whose absence caused the defect",
        );
        assert!(
            view.differentiated_rows.is_empty(),
            "and it must stay distinct from the survivor list, which is the whole point \
             \u{2014} equal counts would collapse two different facts into one",
        );
    }

    /// **An older report without the field reads as zero, not as garbage.**
    ///
    /// Traces committed before 2026-08-18 have no `n_differentiations`. A missing field
    /// must degrade to "nothing to say" rather than to a panic, because
    /// `from_report` returning `None` would blank the whole pane over one absent key.
    #[test]
    fn a_report_predating_the_field_still_parses() {
        let mut old = report(0, 0);
        old["reduction"]
            .as_object_mut()
            .expect("object")
            .remove("n_differentiations");

        let view = ReductionView::from_report(&old).expect("an older report must still parse");
        assert_eq!(view.n_differentiations, 0);
    }
}

#[cfg(test)]
mod tests_step_shapes {
    use super::*;
    use serde_json::json;

    /// **A step carries the system's shape either side of it, so the pane can say
    /// whether it acted.**
    ///
    /// The funnel table showed a name and an outcome string, and `"ok"` reports that a
    /// step *ran* while saying nothing about what it *did*. `CartesianPendulum` runs
    /// all eleven steps to completion and moves nothing — which looked exactly like a
    /// funnel doing quiet work.
    #[test]
    fn a_step_that_moved_nothing_is_distinguishable_from_one_that_did() {
        let report = json!({
            "reduction": {
                "funnel_completed": true, "stopped_at": null,
                "n_states_before": 4, "n_states_after": 3,
                "states_before": [], "states_after": [], "demoted_states": [],
                "n_differentiations": 1,
                "differentiated_rows": [], "eliminations": [],
                "steps": [
                    { "step": "expand_compound_derivatives", "outcome": "ok",
                      "states_before": 4, "states_after": 4,
                      "equations_before": 48, "equations_after": 48 },
                    { "step": "reduce_constrained_dummy_derivatives", "outcome": "1 demoted",
                      "states_before": 4, "states_after": 3,
                      "equations_before": 48, "equations_after": 48 },
                ],
            }
        });

        let view = ReductionView::from_report(&report).expect("parses");
        let inert = &view.steps[0];
        let acted = &view.steps[1];

        assert_eq!(
            (inert.states_before, inert.states_after),
            (4, 4),
            "an inert step's shapes must survive parsing; without them the pane is back \
             to guessing from the outcome text",
        );
        assert_eq!((acted.states_before, acted.states_after), (4, 3));
        assert!(
            inert.states_before == inert.states_after
                && inert.equations_before == inert.equations_after,
            "the inert step must be recognisable as inert from its shapes alone \u{2014} \
             its outcome string says \"ok\", which is exactly the case the old \
             literal-string test could not colour correctly",
        );
    }

    /// **An older trace without the shape fields still parses.**
    ///
    /// Every committed trace predating 2026-08-18 has bare `{step, outcome}` rows. A
    /// missing number must read as zero rather than blanking the pane over one absent
    /// key — the same rule the differentiation count follows.
    #[test]
    fn a_step_row_predating_the_shape_fields_still_parses() {
        let report = json!({
            "reduction": {
                "funnel_completed": true, "stopped_at": null,
                "n_states_before": 2, "n_states_after": 2,
                "states_before": [], "states_after": [], "demoted_states": [],
                "differentiated_rows": [], "eliminations": [],
                "steps": [{ "step": "eliminate_trivial", "outcome": "0 eliminated" }],
            }
        });

        let view = ReductionView::from_report(&report).expect("an older trace must parse");
        assert_eq!(view.steps.len(), 1, "the row survives");
        assert_eq!(view.steps[0].states_before, 0);
    }
}
