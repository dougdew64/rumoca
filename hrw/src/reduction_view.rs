//! Index-reduction process view (Pass two) — a structured summary of
//! what the dummy-derivative funnel did to transform a high-index DAE into a
//! matchable, index-1 system.
//!
//! Unlike the spy-plot and incidence views (spatial, canvas-based), this is a
//! scrollable panel: the reduction is a sequential pipeline story — which steps
//! ran, which states were demoted, which equations were differentiated, which
//! variables were eliminated.

use eframe::egui;
use serde_json::Value;

pub struct ReductionView {
    funnel_completed: bool,
    stopped_at: Option<String>,
    n_states_before: usize,
    n_states_after: usize,
    demoted_states: Vec<String>,
    steps: Vec<(String, String)>,
    differentiated_rows: Vec<DiffRow>,
    eliminations: Vec<Elimination>,
}

struct DiffRow {
    equation_origin: String,
    for_state: String,
}

struct Elimination {
    variable: String,
    replacement: String,
}

impl ReductionView {
    pub fn from_report(report: &Value) -> Option<ReductionView> {
        let red = report.get("reduction")?;

        let funnel_completed = red.get("funnel_completed")?.as_bool()?;
        let stopped_at = red
            .get("stopped_at")
            .and_then(|v| v.as_str())
            .map(str::to_owned);
        let n_states_before = red.get("n_states_before")?.as_u64()? as usize;
        let n_states_after = red.get("n_states_after")?.as_u64()? as usize;

        let demoted_states: Vec<String> = red
            .get("demoted_states")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect())
            .unwrap_or_default();

        let steps: Vec<(String, String)> = red
            .get("steps")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|s| {
                        let step = s.get("step")?.as_str()?.to_owned();
                        let outcome = s.get("outcome")?.as_str()?.to_owned();
                        Some((step, outcome))
                    })
                    .collect()
            })
            .unwrap_or_default();

        let differentiated_rows: Vec<DiffRow> = red
            .get("differentiated_rows")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|r| {
                        Some(DiffRow {
                            equation_origin: r.get("equation_origin")?.as_str()?.to_owned(),
                            for_state: r.get("for_state")?.as_str()?.to_owned(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        let eliminations: Vec<Elimination> = red
            .get("eliminations")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|e| {
                        Some(Elimination {
                            variable: e.get("variable")?.as_str()?.to_owned(),
                            replacement: e.get("replacement")?.as_str()?.to_owned(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Some(ReductionView {
            funnel_completed,
            stopped_at,
            n_states_before,
            n_states_after,
            demoted_states,
            steps,
            differentiated_rows,
            eliminations,
        })
    }

    pub fn ui(&self, ui: &mut egui::Ui) {
        egui::ScrollArea::both()
            .id_salt("reduction_view")
            .auto_shrink(false)
            .show(ui, |ui| {
                self.summary(ui);
                ui.add_space(8.0);
                self.funnel_steps(ui);
                if !self.demoted_states.is_empty() {
                    ui.add_space(8.0);
                    self.demoted_section(ui);
                }
                if !self.differentiated_rows.is_empty() {
                    ui.add_space(8.0);
                    self.differentiated_section(ui);
                }
                if !self.eliminations.is_empty() {
                    ui.add_space(8.0);
                    self.elimination_section(ui);
                }
            });
    }

    fn summary(&self, ui: &mut egui::Ui) {
        ui.heading("Index Reduction Summary");
        ui.add_space(4.0);

        let status = if self.funnel_completed {
            egui::RichText::new("funnel completed").color(
                if ui.visuals().dark_mode {
                    egui::Color32::from_rgb(0x3f, 0xb9, 0x50)
                } else {
                    egui::Color32::from_rgb(0x1a, 0x7f, 0x37)
                },
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

    fn funnel_steps(&self, ui: &mut egui::Ui) {
        ui.strong("Funnel steps");
        ui.add_space(2.0);

        let ok_color = if ui.visuals().dark_mode {
            egui::Color32::from_rgb(0x3f, 0xb9, 0x50)
        } else {
            egui::Color32::from_rgb(0x1a, 0x7f, 0x37)
        };
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

    fn demoted_section(&self, ui: &mut egui::Ui) {
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
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("\u{2022}")
                        .color(ui.visuals().warn_fg_color),
                );
                ui.label(egui::RichText::new(name).monospace());
            });
        }
    }

    fn differentiated_section(&self, ui: &mut egui::Ui) {
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
                    ui.label(egui::RichText::new(&row.for_state).monospace());
                    ui.label(egui::RichText::new(&row.equation_origin).monospace().weak());
                    ui.end_row();
                }
            });
    }

    fn elimination_section(&self, ui: &mut egui::Ui) {
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
                    ui.label(egui::RichText::new(&elim.variable).monospace());
                    let display = abbreviate_expr(&elim.replacement);
                    ui.label(egui::RichText::new(display).monospace().weak());
                    ui.end_row();
                }
            });
    }
}

fn abbreviate_expr(json_expr: &str) -> String {
    if let Ok(v) = serde_json::from_str::<Value>(json_expr) {
        expr_to_short(&v)
    } else {
        json_expr.to_owned()
    }
}

fn expr_to_short(v: &Value) -> String {
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
                let lhs = bin.get("lhs").map(expr_to_short).unwrap_or_default();
                let rhs = bin.get("rhs").map(expr_to_short).unwrap_or_default();
                return format!("{lhs} {op} {rhs}");
            }
            if let Some(unary) = v.get("Unary") {
                let op = unary.get("op").and_then(|o| o.as_str()).unwrap_or("?");
                let rhs = unary.get("rhs").map(expr_to_short).unwrap_or_default();
                return format!("{op}{rhs}");
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
}
