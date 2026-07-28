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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;

use eframe::egui;

use rumoca_phase_structural::LiveTrace;
use rumoca_phase_structural::matching::maximum_matching_with_trace;
use rumoca_phase_structural::tarjan::{
    TarjanFrame, TarjanStep, tarjan_scc_with_trace,
};

use crate::canvas::Canvas;
use crate::incidence_view::IncidenceMatrix;
use crate::truncate_label;

/// Animation state for Tarjan SCC discovery — supports recorded and live modes.
pub struct TarjanAnimation {
    frames: Vec<TarjanFrame>,
    n_nodes: usize,
    node_names: Vec<String>,
    /// Which unknown columns each equation touches, and the column names.
    ///
    /// Kept so "is this node the tracked variable's equation?" is answered
    /// structurally. It used to be answered by substring-searching the
    /// pretty-printed equation text, which is exactly the heuristic
    /// name-matching `docs/source-tooling-plan.md` rules out.
    rows: Vec<Vec<usize>>,
    unknown_names: Vec<String>,
    adj: Vec<Vec<usize>>,
    cursor: usize,
    playing: bool,
    interval: f64,
    elapsed: f64,
    live_rx: Option<mpsc::Receiver<TarjanFrame>>,
    live_done: Arc<AtomicBool>,
}

/// Build the equation dependency graph from a matching result.
/// Factored out so both `from_incidence` and `start_live` can use it.
fn build_dep_graph(
    mat: &IncidenceMatrix,
    match_eq: &[Option<usize>],
    match_var: &[Option<usize>],
) -> Vec<Vec<usize>> {
    let n_eq = mat.n_eq();
    let mut adj = vec![Vec::new(); n_eq];
    for (eq, cols) in mat.rows().iter().enumerate() {
        for &col in cols {
            if match_eq[eq] == Some(col) {
                continue;
            }
            if let Some(&Some(owner)) = match_var.get(col) {
                if owner != eq && !adj[eq].contains(&owner) {
                    adj[eq].push(owner);
                }
            }
        }
    }
    for deps in &mut adj {
        deps.sort_unstable();
    }
    adj
}

impl TarjanAnimation {
    /// Build the Tarjan trace from a parsed incidence matrix (recorded mode).
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
        let trace = maximum_matching_with_trace(n_eq, n_var, &eq_vars, None);
        let adj = build_dep_graph(mat, &trace.match_eq, &trace.match_var);
        let result = tarjan_scc_with_trace(n_eq, &adj, None);
        Some(Self {
            frames: result.frames,
            n_nodes: n_eq,
            node_names: mat.equation_texts().to_vec(),
            rows: mat.rows().to_vec(),
            unknown_names: mat.unknown_names().to_vec(),
            adj,
            cursor: 0,
            playing: false,
            interval: 0.5,
            elapsed: 0.0,
            live_rx: None,
            // `true`, not `false` — see the note in `MatchingAnimation::from_incidence`:
            // a recorded animation has no live session running, and
            // `live_debug_lifecycle` relies on that to release a stray armed
            // breakpoint.
            live_done: Arc::new(AtomicBool::new(true)),
        })
    }

    /// Start a live debug session for Tarjan's algorithm. Runs matching
    /// first (non-live) to build the dependency graph, then spawns a thread
    /// for Tarjan's SCC with a `LiveTrace` producer.
    ///
    /// `on_complete` runs inside the algorithm thread after the last frame
    /// but before the thread exits — the caller uses this to remove the
    /// armed breakpoint via the bridge, preventing SIGSTOP from LLDB when
    /// the thread terminates.
    pub fn start_live(mat: &IncidenceMatrix, on_complete: impl FnOnce() + Send + 'static) -> Option<Self> {
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
        let trace = maximum_matching_with_trace(n_eq, n_var, &eq_vars, None);
        let adj = build_dep_graph(mat, &trace.match_eq, &trace.match_var);

        let (lt, rx) = LiveTrace::new();
        let lt = lt.with_frame_delay(std::time::Duration::from_millis(20));
        let done = Arc::new(AtomicBool::new(false));
        let done_for_thread = Arc::clone(&done);
        let adj_for_thread = adj.clone();

        thread::Builder::new()
            .name("tarjan-debug".to_owned())
            .spawn(move || {
                lt.wait_for_debugger();
                tarjan_scc_with_trace(n_eq, &adj_for_thread, Some(&lt));
                on_complete();
                done_for_thread.store(true, Ordering::Release);
            })
            .ok()?;

        Some(Self {
            frames: Vec::new(),
            n_nodes: n_eq,
            node_names: mat.equation_texts().to_vec(),
            rows: mat.rows().to_vec(),
            unknown_names: mat.unknown_names().to_vec(),
            adj,
            cursor: 0,
            playing: false,
            interval: 0.5,
            elapsed: 0.0,
            live_rx: Some(rx),
            live_done: done,
        })
    }

    /// Where playback stands: `(cursor, frame count)`.
    ///
    /// Exists for the crash log (`diagnostics.rs`). "Which animation, at which
    /// frame" is one of the first things worth knowing about a crash in an
    /// animated view, and both fields are otherwise private.
    pub fn position(&self) -> (usize, usize) {
        (self.cursor, self.frames.len())
    }

    pub fn is_live(&self) -> bool {
        self.live_rx.is_some()
    }

    pub fn is_empty(&self) -> bool {
        self.frames.is_empty() && self.live_rx.is_none()
    }

    fn current_frame(&self) -> Option<&TarjanFrame> {
        self.frames.get(self.cursor)
    }

    fn sync_live(&mut self) {
        if let Some(rx) = &self.live_rx {
            let before = self.frames.len();
            self.frames.extend(rx.try_iter());
            if self.frames.len() > before {
                self.cursor = self.frames.len().saturating_sub(1);
            }
        }
    }

    pub fn live_finished(&self) -> bool {
        self.live_done.load(Ordering::Acquire)
    }

    /// Whether equation `eq` references the tracked variable.
    ///
    /// Answered from the incidence matrix — `rows[eq]` holds exactly the
    /// columns that equation touches, which is the structural fact the
    /// structural phase computed. Previously this substring-searched the
    /// pretty-printed equation text, which could match a name occurring inside
    /// another name, inside a function call, or inside an origin label, and
    /// which `docs/source-tooling-plan.md` rules out as a standing principle.
    fn equation_mentions(&self, eq: usize, tracked: Option<&str>) -> bool {
        let Some(tracked) = tracked else { return false };
        let Some(col) = self
            .unknown_names
            .iter()
            .position(|n| crate::identifier_index::same_variable(n, tracked))
        else {
            return false;
        };
        self.rows.get(eq).is_some_and(|cols| cols.contains(&col))
    }

    /// Map the raw live-session flags onto the shared [`crate::LiveState`].
    ///
    /// `arming` comes from the app: the Debug button's breakpoint handshake
    /// takes several frames, and throughout them this view is still showing the
    /// *recorded* animation — so the animation alone cannot tell that a session
    /// is starting, and its controls would stay enabled after the click.
    pub fn live_state(&self, arming: bool) -> crate::LiveState {
        if self.is_live() {
            if self.live_finished() {
                crate::LiveState::Finished
            } else {
                crate::LiveState::Running
            }
        } else if arming {
            crate::LiveState::Arming
        } else {
            crate::LiveState::Idle
        }
    }

    /// Returns `true` on the frame the Debug button is clicked — the caller
    /// owns the bridge state needed to actually arm a session.
    #[must_use]
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        canvas: &mut Canvas,
        tracked: Option<&str>,
        arming: bool,
        debug_enabled: bool,
    ) -> bool {
        self.sync_live();
        let live = self.live_state(arming);

        // Nothing to show at all — no recorded frames and no live session.
        if self.frames.is_empty() && !self.is_live() && !arming {
            ui.label("No Tarjan trace available.");
            return false;
        }

        // A live session takes the cursor; drop any timed playback that was
        // running when the user clicked Debug, so it cannot resume when the
        // session finishes and re-enables the controls.
        if live.is_busy() {
            self.playing = false;
        }

        let dt = ui.input(|i| i.stable_dt) as f64;
        if self.playing && !live.is_busy() {
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
        if live.is_busy() {
            ui.ctx().request_repaint();
        }

        // --- Controls ---
        let n_frames = self.frames.len();
        let debug_clicked = crate::animation_controls(
            ui,
            &mut self.cursor,
            &mut self.playing,
            &mut self.elapsed,
            &mut self.interval,
            n_frames,
            live,
            debug_enabled,
        );

        // A session is starting or the debugger is parked at the startup gate,
        // so no frames have arrived. The controls above stay rendered (disabled)
        // rather than the whole row vanishing until the first Continue.
        if self.frames.is_empty() {
            ui.add_space(4.0);
            ui.label("Waiting for first frame from debugger\u{2026}");
            ui.ctx().request_repaint();
            return debug_clicked;
        }

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
        self.draw_graph(ui, canvas, tracked);

        debug_clicked
    }

    fn draw_graph(&self, ui: &mut egui::Ui, canvas: &mut Canvas, tracked: Option<&str>) {
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

        let scc_colors = crate::colors::SCC_PALETTE;

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
        let font = egui::FontId::proportional((view.zoom() * 0.2).min(14.0));
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
            let is_tracked_node = self.equation_mentions(i, tracked);
            if is_tracked_node {
                painter.circle_stroke(
                    center,
                    node_radius + 2.0,
                    egui::Stroke::new(2.5, crate::colors::TRACKED_GOLD),
                );
            }
            if view.zoom() >= crate::NODE_LABEL_ZOOM_THRESHOLD {
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
                    if is_tracked_node {
                        crate::colors::TRACKED_GOLD
                    } else {
                        visuals.text_color().gamma_multiply(0.8)
                    },
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

    /// A recorded animation must report that no live session is running — see
    /// `recorded_animation_reports_no_live_session` in `matching_anim` for why
    /// `live_debug_lifecycle` depends on this.
    #[test]
    fn recorded_animation_reports_no_live_session() {
        let mat = IncidenceMatrix::from_report(&sample_report()).unwrap();
        let anim = TarjanAnimation::from_incidence(&mat).unwrap();
        assert!(!anim.is_live());
        assert!(anim.live_finished(), "a recorded animation has no live session running");
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

    #[test]
    fn live_mode_receives_all_frames() {
        let mat = IncidenceMatrix::from_report(&sample_report()).unwrap();
        let mut anim = TarjanAnimation::start_live(&mat, || {}).unwrap();
        for _ in 0..100 {
            if anim.live_finished() { break; }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        anim.sync_live();
        assert!(!anim.frames.is_empty());
        assert!(anim.is_live());
        assert!(anim.live_finished());
    }

    #[test]
    fn build_dep_graph_constructs_adjacency() {
        // 3 equations, 3 unknowns:
        //   eq0 references vars {0, 1}, matched to var 0
        //   eq1 references vars {1, 2}, matched to var 1
        //   eq2 references vars {0, 2}, matched to var 2
        //
        // Matching: eq0→var0, eq1→var1, eq2→var2
        //
        // Dependencies (off-diagonal references through matching):
        //   eq0 refs var1 (not its match) → var1 owned by eq1 → eq0 depends on eq1
        //   eq1 refs var2 (not its match) → var2 owned by eq2 → eq1 depends on eq2
        //   eq2 refs var0 (not its match) → var0 owned by eq0 → eq2 depends on eq0
        let mat = IncidenceMatrix::from_report(&sample_report()).unwrap();
        let match_eq = vec![Some(0), Some(1), Some(2)];
        let match_var = vec![Some(0), Some(1), Some(2)];

        let adj = build_dep_graph(&mat, &match_eq, &match_var);
        assert_eq!(adj.len(), 3);
        assert_eq!(adj[0], vec![1], "eq0 should depend on eq1");
        assert_eq!(adj[1], vec![2], "eq1 should depend on eq2");
        assert_eq!(adj[2], vec![0], "eq2 should depend on eq0");
    }

    #[test]
    fn build_dep_graph_no_self_loops() {
        // An equation that references its own matched variable should not
        // produce a self-loop in the dependency graph.
        let report = json!({
            "matching": [
                { "equation": "e0", "unknown": "u0" },
            ],
            "blocks": [],
            "incidence": {
                "n_eq": 1,
                "n_var": 1,
                "unknown_names": ["u0"],
                "rows": [
                    { "equation": "e0", "unknowns": [0] },
                ],
            }
        });
        let mat = IncidenceMatrix::from_report(&report).unwrap();
        let match_eq = vec![Some(0)];
        let match_var = vec![Some(0)];

        let adj = build_dep_graph(&mat, &match_eq, &match_var);
        assert!(adj[0].is_empty(), "self-reference through match should not create a dependency");
    }

    #[test]
    fn build_dep_graph_unmatched_var_ignored() {
        // A variable that isn't matched to any equation should not create
        // dependencies (it has no owner equation to point to).
        let report = json!({
            "matching": [
                { "equation": "e0", "unknown": "u0" },
            ],
            "blocks": [],
            "incidence": {
                "n_eq": 1,
                "n_var": 2,
                "unknown_names": ["u0", "u1"],
                "rows": [
                    { "equation": "e0", "unknowns": [0, 1] },
                ],
            }
        });
        let mat = IncidenceMatrix::from_report(&report).unwrap();
        let match_eq = vec![Some(0)];
        let match_var = vec![Some(0), None];

        let adj = build_dep_graph(&mat, &match_eq, &match_var);
        assert!(adj[0].is_empty(), "unmatched variable should not create a dependency");
    }
}
