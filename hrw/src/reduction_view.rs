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
    // Each funnel step: (step_name, outcome_description).
    // Example: ("demote_exact_alias_component_states", "1 demoted")
    steps: Vec<(String, String)>,
    // Equations manufactured by differentiating a constraint.
    differentiated_rows: Vec<DiffRow>,
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

        let steps: Vec<(String, String)> = parse_list(red, "steps", &mut unreadable, |s| {
            let step = s.get("step")?.as_str()?.to_owned();
            let outcome = s.get("outcome")?.as_str()?.to_owned();
            Some((step, outcome))
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
                    ui.colored_label(
                        ui.visuals().error_fg_color,
                        format!("\u{26a0} {problem}"),
                    );
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
            egui::RichText::new("funnel completed").color(
                crate::colors::ok_color(ui.visuals().dark_mode),
            )
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
        if !self.differentiated_rows.is_empty() {
            ui.label(format!(
                "{} equation{} manufactured by differentiation",
                self.differentiated_rows.len(),
                if self.differentiated_rows.len() == 1 { "" } else { "s" },
            ));
        }
        if !self.eliminations.is_empty() {
            ui.label(format!(
                "{} variable{} eliminated by substitution",
                self.eliminations.len(),
                if self.eliminations.len() == 1 { "" } else { "s" },
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

        egui::Grid::new("funnel_steps")
            .num_columns(2)
            .spacing([12.0, 4.0])
            .show(ui, |ui| {
                for (step, outcome) in &self.steps {
                    let short = step
                        .strip_prefix("demote_")
                        .or_else(|| step.strip_prefix("reduce_"))
                        .or_else(|| step.strip_prefix("index_reduce_"))
                        .unwrap_or(step);
                    ui.label(egui::RichText::new(short).monospace());

                    let is_err = outcome.starts_with("stopped");
                    let is_noop = outcome == "0 demoted" || outcome == "0 substituted";
                    let color = if is_err {
                        err_color
                    } else if is_noop {
                        neutral_color
                    } else {
                        ok_color
                    };
                    ui.label(egui::RichText::new(outcome).color(color));
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
            let is_tracked = tracked.is_some_and(|t| {
                crate::identifier_index::same_variable(name, t)
            });
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("\u{2022}")
                        .color(ui.visuals().warn_fg_color),
                );
                let mut rt = egui::RichText::new(name).monospace();
                if is_tracked {
                    rt = rt.strong().background_color(
                        crate::colors::TRACKED_FILL_MEDIUM,
                    );
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
                    let is_tracked = tracked.is_some_and(|t| {
                        crate::identifier_index::same_variable(&row.for_state, t)
                    });
                    let mut rt = egui::RichText::new(&row.for_state).monospace();
                    if is_tracked {
                        rt = rt.strong().background_color(
                            crate::colors::TRACKED_FILL_MEDIUM,
                        );
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
                    let is_tracked = tracked.is_some_and(|t| {
                        crate::identifier_index::same_variable(&elim.variable, t)
                    });
                    let mut rt = egui::RichText::new(&elim.variable).monospace();
                    if is_tracked {
                        rt = rt.strong().background_color(
                            crate::colors::TRACKED_FILL_MEDIUM,
                        );
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
                let lhs = bin.get("lhs").map(expr_to_short).unwrap_or_default();
                let rhs = bin.get("rhs").map(expr_to_short).unwrap_or_default();
                return format!("{lhs} {sym} {rhs}");
            }
            if let Some(unary) = v.get("Unary") {
                let op = unary.get("op").and_then(|o| o.as_str()).unwrap_or("?");
                let sym = unary_op_symbol(op);
                let rhs = unary.get("rhs").map(expr_to_short).unwrap_or_default();
                return format!("{sym}{rhs}");
            }
            if let Some(call) = v.get("BuiltinCall") {
                let func = call
                    .get("function")
                    .and_then(|f| f.as_str())
                    .unwrap_or("f");
                let args: Vec<String> = call
                    .get("args")
                    .and_then(Value::as_array)
                    .map(|a| a.iter().map(expr_to_short).collect())
                    .unwrap_or_default();
                return format!("{func}({})", args.join(", "));
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
        let bin = |op| format!(r#"{{"Binary":{{"op":"{op}","lhs":{{"VarRef":{{"name":"x"}}}},"rhs":{{"VarRef":{{"name":"y"}}}}}}}}"#);
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
            view.unreadable[0].contains("1 of 2") && view.unreadable[0].contains("differentiated_rows"),
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

    /// A list that is present but is not a list at all is a defect too — the shape
    /// changed under us, which is exactly what the fidelity suite exists to notice
    /// and what this view would otherwise render as "nothing happened".
    #[test]
    fn a_list_that_is_not_a_list_is_reported() {
        let report = report_with(serde_json::json!("eq[1], eq[2]"));
        let view = ReductionView::from_report(&report).expect("should parse");
        assert!(view.differentiated_rows.is_empty());
        assert_eq!(view.unreadable.len(), 1, "{:?}", view.unreadable);
        assert!(view.unreadable[0].contains("is not a list"), "{:?}", view.unreadable[0]);
    }
}
