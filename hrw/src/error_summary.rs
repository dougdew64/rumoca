//! **The structured error summary** — what a compiler stage says when it *refused* to
//! produce a result, rendered as headings, grids and diagnostics instead of raw JSON.
//!
//! Lifted out of `impl App` on 2026-08-19. See
//! [`docs/app-split-plan.md`](../docs/app-split-plan.md).
//!
//! # Why this one was free
//!
//! Every extraction before it had to establish *what state the pane touches* and then
//! design a signature around the answer. **These two functions touch none.** They sat
//! inside `impl App` as associated functions — `Self::generic_error_summary(ui, err,
//! stage)` — and never mentioned `self` in 228 lines. An associated function with no
//! `self` is a free function wearing an `impl` block, so the move is a rename of the
//! call sites and nothing else: no callback enum, no `&mut` parameter, no deferred press.
//!
//! **Found by a sweep, not by looking harder at the expensive ones.** The five iterations
//! before this went hunting for seams in methods with real coupling; one `awk` pass over
//! `impl App` for bodies containing no `self` found this pair immediately. The census is
//! in `app-split-plan.md` — three of the remaining `self`-free bodies are under 30 lines
//! and belong where they are.
//!
//! # What the two functions divide between them
//!
//! [`structural_singular_summary`] is the Structural stage's *entry* to the general
//! renderer: it digs `error` out of the stage value and states the absence itself when
//! there is none (`"(no structural error details)"`), rather than rendering an empty
//! summary. That is `CLAUDE.md`'s **absence is stated, never filled** in its smallest
//! form.
//!
//! [`generic_error_summary`] renders whatever the compiler actually put in the error
//! object, and its shape is **one optional block per key**. Nothing is synthesised: a
//! missing `guidance` prints no guidance, a missing `diagnostics` array prints no
//! diagnostics section. The blocks, in render order:
//!
//! | key | block |
//! |---|---|
//! | *(heading)* | `kind` × [`StageKind`] — "singular" is named per stage, everything else is `"<stage> error"` |
//! | `message` | suppressed for `singular`, where the grid below tells the story better |
//! | `error_code` | e.g. `EI001` from instantiate |
//! | `detail` | a clearer restatement |
//! | `state_name`/`row`/`reason` | the mass-matrix grid (solve lowering) |
//! | `context` | evaluation context (solve lowering) |
//! | `diagnostics` | flatten / typecheck, with severity colour and indented notes |
//! | `n_equations`/`n_unknowns`/`n_matched`/`rank_deficiency` | the singularity grid, plus the unmatched lists |
//! | `determinacy` | the initial-condition verdict (initialization) |
//! | `guidance` | last, and weak |
//!
//! **The four singularity counts are read as a tuple and rendered all-or-nothing.** A
//! partial grid would invite a reader to infer the missing count from the other three,
//! and a rank deficiency inferred by HRW is exactly the invented number the charter
//! forbids.

use eframe::egui;

use crate::worker::StageKind;

/// Render a structured summary for a singular Structural stage.
pub(crate) fn structural_singular_summary(ui: &mut egui::Ui, stage: &crate::worker::Stage) {
    let error_json = stage.value.as_ref().and_then(|v| v.get("error"));
    let Some(err) = error_json else {
        ui.weak("(no structural error details)");
        return;
    };
    generic_error_summary(ui, err, StageKind::Structural);
}

/// Render a structured error summary for any stage with error data.
pub(crate) fn generic_error_summary(
    ui: &mut egui::Ui,
    error: &serde_json::Value,
    stage: StageKind,
) {
    let kind = error
        .get("kind")
        .and_then(|k| k.as_str())
        .unwrap_or("error");
    let message = error
        .get("message")
        .and_then(|m| m.as_str())
        .unwrap_or("(unknown error)");

    let heading = match (kind, stage) {
        ("singular", StageKind::Structural) => "Structural singularity".to_owned(),
        ("singular", StageKind::Initialization) => "Initialization singularity".to_owned(),
        ("singular", StageKind::IndexReduction) => {
            "Still singular after index reduction".to_owned()
        }
        ("singular", _) => "Structural singularity".to_owned(),
        _ => format!("{} error", stage.name()),
    };

    ui.heading(heading);
    ui.add_space(4.0);

    // For singular errors the grid below tells the story — skip the raw
    // error string which is verbose and redundant.
    if kind != "singular" {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.label(egui::RichText::new(message).color(ui.visuals().error_fg_color));
        });
    }

    // Error code (e.g. EI001 from instantiate)
    if let Some(code) = error.get("error_code").and_then(|c| c.as_str()) {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.strong("Error code");
            ui.monospace(code);
        });
    }

    // Detail text (a clearer restatement of the error)
    if let Some(detail) = error.get("detail").and_then(|d| d.as_str()) {
        ui.add_space(4.0);
        ui.label(detail);
    }

    // Mass matrix details (solve lowering)
    if kind == "mass_matrix" {
        ui.add_space(8.0);
        egui::Grid::new("mass_matrix_grid")
            .num_columns(2)
            .spacing([12.0, 4.0])
            .show(ui, |ui| {
                if let Some(name) = error.get("state_name").and_then(|n| n.as_str()) {
                    ui.strong("State variable");
                    ui.monospace(name);
                    ui.end_row();
                }
                if let Some(row) = error.get("row").and_then(|r| r.as_u64()) {
                    ui.strong("Matrix row");
                    ui.label(format!("{row}"));
                    ui.end_row();
                }
                if let Some(reason) = error.get("reason").and_then(|r| r.as_str()) {
                    ui.strong("Reason");
                    ui.label(reason);
                    ui.end_row();
                }
            });
    }

    // Evaluation context (solve lowering)
    if kind == "evaluation"
        && let Some(ctx) = error.get("context").and_then(|c| c.as_str())
    {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.strong("Context");
            ui.label(ctx);
        });
    }

    // Diagnostics list (flatten / typecheck)
    if let Some(diags) = error.get("diagnostics").and_then(|d| d.as_array())
        && !diags.is_empty()
    {
        ui.add_space(8.0);
        ui.strong(format!("Diagnostics ({})", diags.len()));
        for d in diags {
            let severity = d
                .get("severity")
                .and_then(|s| s.as_str())
                .unwrap_or("Error");
            let code = d.get("code").and_then(|c| c.as_str());
            let msg = d.get("message").and_then(|m| m.as_str()).unwrap_or("");
            ui.horizontal(|ui| {
                let sev_color = if severity.contains("Error") {
                    ui.visuals().error_fg_color
                } else {
                    ui.visuals().warn_fg_color
                };
                ui.label(egui::RichText::new(severity).color(sev_color).strong());
                if let Some(c) = code {
                    ui.monospace(format!("[{c}]"));
                }
                ui.label(msg);
            });
            if let Some(notes) = d.get("notes").and_then(|n| n.as_array()) {
                for note in notes {
                    if let Some(text) = note.as_str() {
                        ui.horizontal(|ui| {
                            ui.add_space(16.0);
                            ui.weak(format!("note: {text}"));
                        });
                    }
                }
            }
        }
    }

    // Singularity details (structural/initialization errors)
    if kind == "singular"
        && let (Some(n_eq), Some(n_unk), Some(n_matched), Some(deficiency)) = (
            error["n_equations"].as_u64(),
            error["n_unknowns"].as_u64(),
            error["n_matched"].as_u64(),
            error["rank_deficiency"].as_u64(),
        )
    {
        ui.add_space(8.0);
        egui::Grid::new("singular_grid")
            .num_columns(2)
            .spacing([12.0, 4.0])
            .show(ui, |ui| {
                ui.strong("Equations");
                ui.label(format!("{n_eq}"));
                ui.end_row();
                ui.strong("Unknowns");
                ui.label(format!("{n_unk}"));
                ui.end_row();
                ui.strong("Matched");
                ui.label(format!("{n_matched}"));
                ui.end_row();
                ui.strong("Rank deficiency");
                ui.label(
                    egui::RichText::new(format!("{deficiency}"))
                        .color(crate::colors::ANIM_FAIL)
                        .strong(),
                );
                ui.end_row();
            });

        if let Some(eqs) = error["unmatched_equations"].as_array()
            && !eqs.is_empty()
        {
            ui.add_space(4.0);
            ui.strong("Unmatched equations");
            for eq in eqs {
                if let Some(name) = eq.as_str() {
                    ui.label(format!("  {name}"));
                }
            }
        }
        if let Some(unks) = error["unmatched_unknowns"].as_array()
            && !unks.is_empty()
        {
            ui.add_space(4.0);
            ui.strong("Unmatched unknowns");
            for unk in unks {
                if let Some(name) = unk.as_str() {
                    ui.label(format!("  {name}"));
                }
            }
        }
    }

    // Determinacy summary (initialization stage)
    if let Some(det) = error.get("determinacy") {
        ui.add_space(8.0);
        ui.strong("Initial condition determinacy");
        ui.add_space(2.0);
        egui::Grid::new("determinacy_grid")
            .num_columns(2)
            .spacing([12.0, 4.0])
            .show(ui, |ui| {
                if let Some(n) = det["states"].as_u64() {
                    ui.label("States");
                    ui.label(format!("{n}"));
                    ui.end_row();
                }
                if let Some(n) = det["initial_equations"].as_u64() {
                    ui.label("Initial equations");
                    ui.label(format!("{n}"));
                    ui.end_row();
                }
                if let Some(n) = det["fixed_start_states"].as_u64() {
                    ui.label("Fixed start states");
                    ui.label(format!("{n}"));
                    ui.end_row();
                }
                if let Some(n) = det["explicit_initial_conditions"].as_u64() {
                    ui.label("Explicit initial conditions");
                    ui.label(format!("{n}"));
                    ui.end_row();
                }
                if let Some(v) = det.get("verdict").and_then(|v| v.as_str()) {
                    ui.label("Verdict");
                    ui.label(v);
                    ui.end_row();
                }
            });
    }

    // Guidance
    if let Some(guidance) = error.get("guidance").and_then(|g| g.as_str()) {
        ui.add_space(12.0);
        ui.weak(guidance);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;
    use serde_json::json;

    /// **The test this extraction bought.** While these were private associated
    /// functions of `App`, the only way to reach them was to build an `App`, give it a
    /// worker, and drive a compile to a failing stage — so no test ever asserted what
    /// the summary *renders*, only that a failing stage reached the pane. Against a
    /// free function the error object is just an argument.
    fn draw(error: serde_json::Value, stage: StageKind) -> Harness<'static, ()> {
        let mut h = Harness::new_ui(move |ui| generic_error_summary(ui, &error, stage));
        h.run_steps(2);
        h
    }

    /// **Absence is stated, never filled** — the smallest instance of the charter rule.
    /// A Structural stage carrying no `error` object says so; it does not render an
    /// empty summary that a reader would take for a diagnosis.
    #[test]
    fn a_structural_stage_with_no_error_object_states_the_absence() {
        let stage = crate::worker::Stage::ok(json!({ "not_an_error": 1 }));
        let mut h = Harness::new_ui(move |ui| structural_singular_summary(ui, &stage));
        h.run_steps(2);

        assert!(
            h.query_by_label_contains("(no structural error details)")
                .is_some(),
            "the missing error object must be stated"
        );
        assert!(
            h.query_by_label_contains("Structural singularity")
                .is_none(),
            "a stage with no error object must not be given a singularity heading"
        );
    }

    /// The non-vacuity partner: the *same* entry point does render the heading when the
    /// compiler did supply an error. Without this, a `structural_singular_summary` that
    /// printed the absence unconditionally would pass the test above.
    #[test]
    fn a_structural_stage_with_an_error_object_renders_the_summary() {
        let stage = crate::worker::Stage::err_with_details(json!({ "kind": "singular" }), "boom");
        let mut h = Harness::new_ui(move |ui| structural_singular_summary(ui, &stage));
        h.run_steps(2);

        assert!(
            h.query_by_label_contains("Structural singularity")
                .is_some(),
            "an error object must reach the general summary"
        );
        assert!(
            h.query_by_label_contains("(no structural error details)")
                .is_none(),
            "absence must not be claimed when the details are present"
        );
    }

    /// **The singularity grid is all-or-nothing, and that is a correctness property
    /// rather than a layout one.** Three of the four counts on screen invite the reader
    /// to infer the fourth — and a rank deficiency inferred by HRW is precisely the
    /// invented number the charter forbids.
    #[test]
    fn the_singularity_grid_needs_all_four_counts() {
        let complete = json!({
            "kind": "singular",
            "message": "system is structurally singular",
            "n_equations": 5,
            "n_unknowns": 6,
            "n_matched": 5,
            "rank_deficiency": 1,
        });
        let h = draw(complete, StageKind::Structural);
        for label in ["Equations", "Unknowns", "Matched", "Rank deficiency"] {
            assert!(
                h.query_by_label_contains(label).is_some(),
                "the complete grid must render {label}"
            );
        }

        // Drop exactly one count. `query_by_label_contains`, not
        // `get_all_by_label_contains`: the latter PANICS when nothing matches, so it
        // cannot express absence at all (`app-split-plan.md`).
        let partial = json!({
            "kind": "singular",
            "message": "system is structurally singular",
            "n_equations": 5,
            "n_unknowns": 6,
            "n_matched": 5,
        });
        let h = draw(partial, StageKind::Structural);
        assert!(
            h.query_by_label_contains("Rank deficiency").is_none(),
            "a missing count must not be rendered"
        );
        assert!(
            h.query_by_label_contains("Equations").is_none(),
            "and the three counts that ARE present must be withheld with it, \
             or the reader infers the fourth"
        );
    }

    /// **Every block is optional and nothing is synthesised.** An error object carrying
    /// only a kind and a message must produce a heading and that message — and no
    /// guidance, no diagnostics section and no error code invented to fill the pane.
    #[test]
    fn a_minimal_error_renders_no_block_the_compiler_did_not_supply() {
        let h = draw(
            json!({ "kind": "typecheck", "message": "unknown component `x`" }),
            StageKind::Flatten,
        );

        assert!(
            h.query_by_label_contains("unknown component").is_some(),
            "a non-singular error shows its message"
        );
        for absent in ["Guidance", "Diagnostics", "Error code", "Reason", "Verdict"] {
            assert!(
                h.query_by_label_contains(absent).is_none(),
                "{absent} was not in the error object and must not appear"
            );
        }
    }

    /// **The same `kind` is named differently per stage**, because "singular" means a
    /// different failure in each: an unsolvable system, an unsolvable initial condition,
    /// and a reduction that did not fix it. Reading the heading is how a learner tells
    /// them apart.
    #[test]
    fn the_singular_heading_names_the_stage_that_failed() {
        for (stage, heading) in [
            (StageKind::Structural, "Structural singularity"),
            (StageKind::Initialization, "Initialization singularity"),
            (
                StageKind::IndexReduction,
                "Still singular after index reduction",
            ),
        ] {
            let h = draw(json!({ "kind": "singular" }), stage);
            assert!(
                h.query_by_label_contains(heading).is_some(),
                "{stage:?} must be headed {heading:?}"
            );
        }
    }
}
