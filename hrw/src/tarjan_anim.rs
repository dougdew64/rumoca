//! Animated Tarjan SCC stepper — replays Tarjan's strongly connected
//! component algorithm frame by frame on the equation dependency graph.
//!
//! The animation uses `TarjanFrame`s recorded by the traced Tarjan
//! in `rumoca_phase_structural::tarjan`. Each frame captures one
//! algorithmic decision (visit a node, explore/tree/back edge, pop SCC)
//! plus a snapshot of the DFS stack and discovered SCCs.
//!
//! The stepper renders a node-per-equation grid with:
//! - DFS stack: highlighted nodes currently on the Tarjan stack
//! - Discovered SCCs: colored blocks for each discovered component
//! - Current edge: directional highlight showing the exploration
//!
//! Controls: play/pause, step forward/back, reset, speed slider.

use std::collections::HashSet;

use eframe::egui;

use rumoca_phase_structural::matching::maximum_matching_with_trace;
use rumoca_phase_structural::tarjan::{
    TarjanFrame, TarjanStep, tarjan_scc_with_trace,
};

use crate::canvas::Canvas;
use crate::incidence_view::IncidenceMatrix;

/// Animation state for Tarjan SCC discovery.
pub struct TarjanAnimation {
    frames: Vec<TarjanFrame>,
    n_nodes: usize,
    node_names: Vec<String>,
    adj: Vec<Vec<usize>>,
    cursor: usize,
    playing: bool,
    interval: f64,
    elapsed: f64,
}

impl TarjanAnimation {
    /// Build the Tarjan trace from a parsed incidence matrix.
    ///
    /// First runs matching to build the dependency graph (equation A
    /// depends on equation B if A references a variable matched to B),
    /// then traces Tarjan's SCC algorithm on that graph.
    pub fn from_incidence(mat: &IncidenceMatrix) -> Option<Self> {
        let n_eq = mat.n_eq();
        let n_var = mat.n_var();
        if n_eq == 0 {
            return None;
        }

        let eq_vars: Vec<HashSet<usize>> = mat
            .rows()
            .iter()
            .map(|cols| cols.iter().copied().collect())
            .collect();
        let trace = maximum_matching_with_trace(n_eq, n_var, &eq_vars);

        // Build the dependency graph: for each equation, the equations
        // whose matched variable it references.
        let mut adj = vec![Vec::new(); n_eq];
        for (eq, cols) in mat.rows().iter().enumerate() {
            for &col in cols {
                if trace.match_eq[eq] == Some(col) {
                    continue; // skip the matched variable (self-dep)
                }
                if let Some(&Some(owner)) = trace.match_var.get(col) {
                    if owner != eq && !adj[eq].contains(&owner) {
                        adj[eq].push(owner);
                    }
                }
            }
        }
        for deps in &mut adj {
            deps.sort_unstable();
        }

        let result = tarjan_scc_with_trace(n_eq, &adj);
        Some(Self {
            frames: result.frames,
            n_nodes: n_eq,
            node_names: mat.equation_names().to_vec(),
            adj,
            cursor: 0,
            playing: false,
            interval: 0.5,
            elapsed: 0.0,
        })
    }

    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    fn current_frame(&self) -> Option<&TarjanFrame> {
        self.frames.get(self.cursor)
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, canvas: &mut Canvas) {
        if self.frames.is_empty() {
            ui.label("No Tarjan trace available.");
            return;
        }

        let dt = ui.input(|i| i.stable_dt) as f64;
        if self.playing {
            self.elapsed += dt;
            if self.elapsed >= self.interval {
                self.elapsed = 0.0;
                if self.cursor + 1 < self.frames.len() {
                    self.cursor += 1;
                } else {
                    self.playing = false;
                }
            }
            ui.ctx().request_repaint();
        }

        // --- Controls ---
        ui.horizontal(|ui| {
            if self.playing {
                if ui.button("\u{23f8} Pause").clicked() {
                    self.playing = false;
                }
            } else if ui.button("\u{25b6} Play").clicked() {
                if self.cursor + 1 >= self.frames.len() {
                    self.cursor = 0;
                }
                self.playing = true;
                self.elapsed = 0.0;
            }

            if ui.button("\u{23ee} Reset").clicked() {
                self.cursor = 0;
                self.playing = false;
            }

            ui.add_enabled_ui(!self.playing, |ui| {
                if ui
                    .add_enabled(self.cursor > 0, egui::Button::new("\u{25c0} Back"))
                    .clicked()
                {
                    self.cursor = self.cursor.saturating_sub(1);
                }
                if ui
                    .add_enabled(
                        self.cursor + 1 < self.frames.len(),
                        egui::Button::new("Step \u{25b6}"),
                    )
                    .clicked()
                {
                    self.cursor += 1;
                }
            });

            ui.separator();
            ui.label(format!(
                "Frame {}/{}",
                self.cursor + 1,
                self.frames.len()
            ));

            ui.separator();
            ui.label("Speed:");
            let mut speed_ms = (self.interval * 1000.0) as i32;
            if ui
                .add(egui::Slider::new(&mut speed_ms, 50..=2000).suffix("ms"))
                .changed()
            {
                self.interval = speed_ms as f64 / 1000.0;
            }
        });

        // --- Step description ---
        if let Some(frame) = self.current_frame() {
            ui.horizontal(|ui| {
                let (icon, desc) = step_description(&frame.step, &self.node_names);
                ui.label(egui::RichText::new(icon).size(16.0));
                ui.label(desc);
            });
            // SCC summary.
            if !frame.sccs_so_far.is_empty() {
                ui.horizontal(|ui| {
                    ui.weak(format!(
                        "{} SCC{} discovered so far",
                        frame.sccs_so_far.len(),
                        if frame.sccs_so_far.len() == 1 { "" } else { "s" },
                    ));
                });
            }
        }

        ui.add_space(4.0);

        // --- Dependency graph visualization ---
        self.draw_graph(ui, canvas);
    }

    fn draw_graph(&self, ui: &mut egui::Ui, canvas: &mut Canvas) {
        // Lay out nodes in a grid arrangement.
        let cols = (self.n_nodes as f32).sqrt().ceil() as usize;
        let grid_rows = (self.n_nodes + cols - 1) / cols;
        let bounds = egui::Rect::from_min_size(
            egui::pos2(-1.0, -1.0),
            egui::vec2(cols as f32 + 2.0, grid_rows as f32 + 2.0),
        );
        let (_response, view, painter) = canvas.show(ui, bounds);

        let visuals = ui.visuals();
        let bg = visuals.extreme_bg_color;
        painter.rect_filled(view.to_screen_rect(bounds), egui::CornerRadius::ZERO, bg);

        let Some(frame) = self.current_frame() else {
            return;
        };

        let on_stack: HashSet<usize> = frame.stack.iter().copied().collect();
        let in_scc: Vec<Option<usize>> = {
            let mut map = vec![None; self.n_nodes];
            for (i, scc) in frame.sccs_so_far.iter().enumerate() {
                for &node in scc {
                    map[node] = Some(i);
                }
            }
            map
        };

        let scc_colors = [
            egui::Color32::from_rgb(0x42, 0xA5, 0xF5), // blue
            egui::Color32::from_rgb(0xAB, 0x47, 0xBC), // purple
            egui::Color32::from_rgb(0x26, 0xA6, 0x9A), // teal
            egui::Color32::from_rgb(0xFF, 0x70, 0x43), // deep orange
            egui::Color32::from_rgb(0x78, 0x90, 0x9C), // blue-grey
            egui::Color32::from_rgb(0xEC, 0x40, 0x7A), // pink
        ];

        let node_pos = |i: usize| -> egui::Pos2 {
            let col = i % cols;
            let row = i / cols;
            egui::pos2(col as f32 + 0.5, row as f32 + 0.5)
        };

        // Draw edges (dependency arrows).
        let edge_color = visuals.weak_text_color().gamma_multiply(0.3);
        for (from, deps) in self.adj.iter().enumerate() {
            for &to in deps {
                let p1 = view.to_screen(node_pos(from));
                let p2 = view.to_screen(node_pos(to));
                painter.line_segment([p1, p2], egui::Stroke::new(1.0, edge_color));
            }
        }

        // Highlight the current step's edge.
        match &frame.step {
            TarjanStep::ExploreEdge { from, to }
            | TarjanStep::TreeEdge { from, to }
            | TarjanStep::BackEdge { from, to }
            | TarjanStep::Return { from, to } => {
                let p1 = view.to_screen(node_pos(*from));
                let p2 = view.to_screen(node_pos(*to));
                let color = match &frame.step {
                    TarjanStep::BackEdge { .. } => crate::colors::ANIM_FAIL,
                    TarjanStep::TreeEdge { .. } => crate::colors::ANIM_PATH_FOUND,
                    _ => crate::colors::ANIM_EXPLORE,
                };
                painter.line_segment([p1, p2], egui::Stroke::new(2.5, color));
            }
            _ => {}
        }

        // Draw nodes.
        let node_radius = view.zoom() * 0.3;
        let font = egui::FontId::proportional(view.zoom() * 0.2);
        for i in 0..self.n_nodes {
            let center = view.to_screen(node_pos(i));
            let fill = if let Some(scc_idx) = in_scc[i] {
                scc_colors[scc_idx % scc_colors.len()]
            } else if on_stack.contains(&i) {
                crate::colors::ANIM_EXPLORE.gamma_multiply(0.7)
            } else {
                visuals.widgets.inactive.bg_fill
            };
            let stroke_color = match &frame.step {
                TarjanStep::Visit(v) if *v == i => crate::colors::ANIM_PATH_FOUND,
                _ => visuals.widgets.inactive.fg_stroke.color,
            };
            painter.circle(center, node_radius, fill, egui::Stroke::new(1.5, stroke_color));
            if view.zoom() >= 10.0 {
                let label = self
                    .node_names
                    .get(i)
                    .map(|n| truncate_label(n, 12))
                    .unwrap_or("?");
                painter.text(
                    center + egui::vec2(0.0, node_radius + view.zoom() * 0.12),
                    egui::Align2::CENTER_TOP,
                    label,
                    font.clone(),
                    visuals.text_color().gamma_multiply(0.8),
                );
            }
        }
    }
}

fn step_description(step: &TarjanStep, names: &[String]) -> (&'static str, String) {
    let name = |i: usize| names.get(i).map(String::as_str).unwrap_or("?");
    match step {
        TarjanStep::Visit(v) => (
            "\u{1f50d}",
            format!("Visiting node {} ({}): assigned index/lowlink", v, name(*v)),
        ),
        TarjanStep::ExploreEdge { from, to } => (
            "\u{1f449}",
            format!("Exploring edge {} ({}) \u{2192} {} ({})", from, name(*from), to, name(*to)),
        ),
        TarjanStep::TreeEdge { from, to } => (
            "\u{1f332}",
            format!("Tree edge: {} ({}) \u{2192} {} ({}) \u{2014} unvisited, recursing", from, name(*from), to, name(*to)),
        ),
        TarjanStep::BackEdge { from, to } => (
            "\u{1f519}",
            format!("Back edge: {} ({}) \u{2192} {} ({}) \u{2014} on stack, cycle detected!", from, name(*from), to, name(*to)),
        ),
        TarjanStep::Return { from, to } => (
            "\u{21a9}",
            format!("Returning from {} ({}) to {} ({}): updating lowlink", to, name(*to), from, name(*from)),
        ),
        TarjanStep::SccFound { root, members } => {
            let member_names: Vec<&str> = members.iter().map(|&m| name(m)).collect();
            (
                "\u{1f3af}",
                format!(
                    "SCC found! Root: {} ({}). Members ({}): [{}]",
                    root, name(*root), members.len(),
                    member_names.join(", "),
                ),
            )
        }
    }
}

fn truncate_label(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        s.get(..max).unwrap_or(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_report() -> serde_json::Value {
        json!({
            "matching": [
                { "equation": "f_x[0]", "unknown": "der(x)" },
                { "equation": "f_x[1]", "unknown": "y" },
                { "equation": "f_x[2]", "unknown": "z" },
            ],
            "blocks": [],
            "incidence": {
                "n_eq": 3,
                "n_var": 3,
                "unknown_names": ["der(x)", "y", "z"],
                "rows": [
                    { "equation": "f_x[0]", "unknowns": [0, 1] },
                    { "equation": "f_x[1]", "unknowns": [1, 2] },
                    { "equation": "f_x[2]", "unknowns": [0, 2] },
                ],
            }
        })
    }

    #[test]
    fn tarjan_animation_from_incidence_produces_frames() {
        let mat = IncidenceMatrix::from_report(&sample_report()).unwrap();
        let anim = TarjanAnimation::from_incidence(&mat).unwrap();
        assert!(!anim.is_empty());
    }

    #[test]
    fn tarjan_animation_starts_paused() {
        let mat = IncidenceMatrix::from_report(&sample_report()).unwrap();
        let anim = TarjanAnimation::from_incidence(&mat).unwrap();
        assert_eq!(anim.cursor, 0);
        assert!(!anim.playing);
    }

    #[test]
    fn step_description_produces_text_for_all_variants() {
        let names = vec!["eq0".to_string(), "eq1".to_string()];
        let steps = vec![
            TarjanStep::Visit(0),
            TarjanStep::ExploreEdge { from: 0, to: 1 },
            TarjanStep::TreeEdge { from: 0, to: 1 },
            TarjanStep::BackEdge { from: 1, to: 0 },
            TarjanStep::Return { from: 0, to: 1 },
            TarjanStep::SccFound { root: 0, members: vec![1, 0] },
        ];
        for step in &steps {
            let (icon, desc) = step_description(step, &names);
            assert!(!icon.is_empty());
            assert!(!desc.is_empty());
        }
    }
}
