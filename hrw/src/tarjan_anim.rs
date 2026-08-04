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
use std::sync::Arc;
use std::thread;

use eframe::egui;

use rumoca_phase_structural::LiveTrace;
use rumoca_phase_structural::matching::maximum_matching_with_trace;
use rumoca_phase_structural::tarjan::{
    TarjanFrame, TarjanStep, tarjan_scc_with_trace,
};

use crate::canvas::Canvas;
use crate::playback::{Animated, Playback};
use crate::incidence_view::IncidenceMatrix;
use crate::truncate_label;

/// Animation state for Tarjan SCC discovery — supports recorded and live modes.
/// Where equation `i` sits in world space, for a graph of `n_nodes` equations.
///
/// **The single source of truth for this view's layout.** Drawing and camera aiming both
/// call it, so they cannot disagree about where an equation is — a link that aimed at a
/// position the renderer did not use would land near-but-wrong, which is worse than not
/// aiming at all because nothing on screen says it missed.
pub fn equation_world_pos(i: usize, n_nodes: usize) -> egui::Pos2 {
    let cols = grid_cols(n_nodes);
    egui::pos2((i % cols) as f32 + 0.5, (i / cols) as f32 + 0.5)
}

/// Columns in the square-ish grid the nodes are laid out on.
fn grid_cols(n_nodes: usize) -> usize {
    (n_nodes as f32).sqrt().ceil().max(1.0) as usize
}

/// Seconds between auto-advance frames.
const FRAME_INTERVAL: f64 = 0.5;

pub struct TarjanAnimation {
    /// Cursor, timing and live-session state — see [`Playback`].
    playback: Playback<TarjanFrame>,
    n_nodes: usize,
    node_names: Vec<String>,
    /// Which unknown columns each equation touches, and the column names.
    ///
    /// Kept so "is this node the tracked variable's equation?" is answered
    /// structurally. It used to be answered by substring-searching the
    /// pretty-printed equation text, which is exactly the heuristic
    /// name-matching `docs/identity-and-provenance.md` rules out.
    rows: Vec<Vec<usize>>,
    unknown_names: Vec<String>,
    adj: Vec<Vec<usize>>,
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
            if let Some(&Some(owner)) = match_var.get(col)
                && owner != eq && !adj[eq].contains(&owner) {
                    adj[eq].push(owner);
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
    /// Build from **frames captured during the compile**.
    ///
    /// Both searches come from the run that produced the BLT blocks on screen:
    /// `tarjan_frames` is the SCC search itself, and the dependency graph it ran over
    /// is rebuilt from the captured *matching*'s final state rather than by matching
    /// again. `adj` is a data structure the view needs in order to draw — deriving it
    /// is not the same as re-running the algorithm being animated.
    ///
    /// `match_var` is inverted from `match_eq` rather than captured: the two are
    /// exact inverses by construction, so storing both would be a second copy of one
    /// fact and a chance for them to disagree.
    ///
    /// **Returns `None` when either capture is empty.** *(Corrected 2026-08-04: this
    /// said "falls back to a faithful re-derivation", which was true until the
    /// fallback was removed that day for drawing blocks the compiler never built.
    /// The caller states the absence instead.)*
    pub fn from_captured_frames(
        mat: &IncidenceMatrix,
        matching_frames: &[rumoca_phase_structural::matching::MatchingFrame],
        tarjan_frames: &[rumoca_phase_structural::tarjan::TarjanFrame],
    ) -> Option<Self> {
        let n_eq = mat.n_eq();
        if n_eq == 0 {
            return None;
        }
        let (Some(last), false) = (matching_frames.last(), tarjan_frames.is_empty()) else {
            // No SCC search was captured — say so rather than running one now.
            return None;
        };
        // **The frames must describe this matrix.** Same hazard as
        // `MatchingAnimation::from_captured_frames`: this view also renders on the
        // Index Reduction tab, where `mat` is the reduced system while the capture is
        // from the raw one. `match_eq`'s length is the equation count of the system
        // that produced it, so it is the check.
        if last.match_eq.len() != n_eq {
            return None;
        }

        let match_eq = &last.match_eq;
        let mut match_var: Vec<Option<usize>> = vec![None; mat.n_var()];
        for (eq, var) in match_eq.iter().enumerate() {
            if let Some(v) = var
                && *v < match_var.len()
            {
                match_var[*v] = Some(eq);
            }
        }
        let adj = build_dep_graph(mat, match_eq, &match_var);
        Some(Self {
            playback: Playback::recorded(tarjan_frames.to_vec(), FRAME_INTERVAL),
            n_nodes: n_eq,
            node_names: mat.equation_texts().to_vec(),
            rows: mat.rows().to_vec(),
            unknown_names: mat.unknown_names().to_vec(),
            adj,
        })
    }

    /// **Re-runs matching AND Tarjan from scratch. Test-only, enforced by the
    /// compiler** — see [`crate::matching_anim::MatchingAnimation::from_incidence`]
    /// for why a `cfg` replaced a source-text grep on 2026-08-04.
    ///
    /// This is the constructor that drew a non-empty SCC animation for `CapacitorLoop`,
    /// a system whose compile produced no blocks at all.
    #[cfg(test)]
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
            playback: Playback::recorded(result.frames, FRAME_INTERVAL),
            n_nodes: n_eq,
            node_names: mat.equation_texts().to_vec(),
            rows: mat.rows().to_vec(),
            unknown_names: mat.unknown_names().to_vec(),
            adj,
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
                // Where HRW's `LiveTrace` meets the phase's observer callback.
                // The phase crate never learns `LiveTrace` exists — see
                // `rumoca_core::FrameObserver`.
                let observe = |f: &TarjanFrame| lt.push(f.clone());
                tarjan_scc_with_trace(n_eq, &adj_for_thread, Some(&observe));
                on_complete();
                done_for_thread.store(true, Ordering::Release);
            })
            .ok()?;

        Some(Self {
            playback: Playback::live(rx, done, FRAME_INTERVAL),
            n_nodes: n_eq,
            node_names: mat.equation_texts().to_vec(),
            rows: mat.rows().to_vec(),
            unknown_names: mat.unknown_names().to_vec(),
            adj,
        })
    }

    /// Where playback stands: `(cursor, frame count)`.
    ///
    /// Exists for the crash log (`diagnostics.rs`). "Which animation, at which
    /// frame" is one of the first things worth knowing about a crash in an
    /// animated view, and both fields are otherwise private.
    /// Aim this view's camera at equation `i`, if it exists.
    ///
    /// Out-of-range indices are ignored rather than clamped: a tour naming an equation
    /// this model does not have is a **bug in the tour**, and silently aiming somewhere
    /// plausible would hide it. Returns whether the aim was taken, so the caller can
    /// tell "aimed" from "that equation is not here".
    #[must_use]
    /// **The strongly connected components this animation ends on** — the
    /// blocks HRW re-derived, as opposed to the ones Rumoca reported.
    ///
    /// `docs/fidelity-plan.md` **F1**. Compare against
    /// [`IncidenceMatrix::reported_blocks`], as *sets*: Tarjan emits components
    /// in reverse topological order, and the report lists them in solve order,
    /// so the sequences legitimately differ while the partition must not.
    ///
    /// Collected from the `SccFound` steps, **not** from the last frame's
    /// `sccs_so_far`, which lags by one: `tarjan.rs` records the frame *before*
    /// pushing the component onto `self.sccs`, so the component a frame
    /// announces is precisely the one missing from its own snapshot. On a graph
    /// that is a single SCC — `ProportionalLoop` — reading the last frame
    /// therefore yields an empty partition, which is how this was found.
    pub fn final_sccs(&self) -> Vec<Vec<usize>> {
        self.playback
            .frames()
            .iter()
            .filter_map(|f| match &f.step {
                TarjanStep::SccFound { members, .. } => Some(members.clone()),
                _ => None,
            })
            .collect()
    }

    pub fn aim_at_equation(&self, canvas: &mut Canvas, i: usize) -> bool {
        if i >= self.n_nodes {
            return false;
        }
        canvas.request_center_on(equation_world_pos(i, self.n_nodes));
        true
    }

    pub fn is_empty(&self) -> bool {
        self.playback.is_empty()
    }

}

impl Animated for TarjanAnimation {
    fn which(&self) -> &'static str {
        "tarjan"
    }

    fn position(&self) -> (usize, usize) {
        self.playback.position()
    }

    fn live_state(&self, arming: bool) -> crate::LiveState {
        self.playback.live_state(arming)
    }

    /// The step description the view is drawing, plus how many strongly
    /// connected components have been closed so far.
    ///
    /// `sccs_so_far` is the frame's own snapshot, so it tracks the algorithm
    /// rather than the final block structure — Tarjan closes components one at a
    /// time as the stack unwinds, and *when* each closes is the thing worth
    /// watching. `step` is shared with the on-screen label.
    fn current_frame_context(&self) -> Option<serde_json::Value> {
        let frame = self.playback.current()?;
        let (_, desc) = step_description(&frame.step, &self.node_names);
        Some(serde_json::json!({
            "step": desc,
            "sccs_found_so_far": frame.sccs_so_far.len(),
            "stack_depth": frame.stack.len(),
            "n_nodes": self.n_nodes,
        }))
    }

    /// Seek is delegated to [`Playback`], so all eight views agree on what a frame
    /// index means and on refusing an out-of-range one.
    fn seek(&mut self, n: usize) -> bool {
        self.playback.seek(n)
    }
}

impl TarjanAnimation {
    /// What the graph is showing, in words.
    ///
    /// Companion to `matching_anim::render_running_state`, added for the same
    /// reason: the picture is only legible once you know what the algorithm is
    /// *for*, and where it has got to.
    ///
    /// Both numbers come from the frame's own snapshot, so they track the
    /// algorithm rather than the final block structure. The **stack depth** is
    /// the one worth watching: Tarjan closes a component only when the stack
    /// unwinds back to a node whose lowlink never fell below its own index, so
    /// a deep stack means "still inside something that might be one big block".
    fn render_running_state(&self, ui: &mut egui::Ui, frame: &TarjanFrame) {
        ui.add_space(2.0);
        ui.horizontal_wrapped(|ui| {
            ui.weak("Goal:");
            ui.weak(
                "find groups of equations that must be solved together. A group \u{2014} a \
                 strongly connected component \u{2014} is a set of equations each of which \
                 depends on the others, so none can be solved first. Everything else can be \
                 ordered and solved one at a time.",
            );
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(
                egui::RichText::new(format!(
                    "{} block{} closed of {} equations",
                    frame.sccs_so_far.len(),
                    if frame.sccs_so_far.len() == 1 { "" } else { "s" },
                    self.n_nodes,
                ))
                .strong()
                .color(crate::colors::ANIM_PATH_FOUND),
            );
            ui.weak(format!("\u{2014} {} on the stack", frame.stack.len()));
            // A block with more than one member is the interesting outcome: it
            // is a simultaneous system the solver cannot decompose further.
            if let Some(largest) = frame.sccs_so_far.iter().map(Vec::len).max()
                && largest > 1
            {
                ui.weak(format!("\u{00b7} largest block: {largest} equations"));
            }
        });
    }

    /// Whether equation `eq` references the tracked variable.
    ///
    /// Answered from the incidence matrix — `rows[eq]` holds exactly the
    /// columns that equation touches, which is the structural fact the
    /// structural phase computed. Previously this substring-searched the
    /// pretty-printed equation text, which could match a name occurring inside
    /// another name, inside a function call, or inside an origin label, and
    /// which `docs/identity-and-provenance.md` rules out as a standing principle.
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
        self.playback.sync_live();
        let live = self.playback.live_state(arming);

        // Nothing to show at all — no recorded frames and no live session.
        if self.playback.is_empty() && !arming {
            ui.label("No Tarjan trace available.");
            return false;
        }

        let dt = ui.input(|i| i.stable_dt) as f64;
        if self.playback.tick(dt, live) {
            ui.ctx().request_repaint();
        }

        // --- Controls ---
        let debug_clicked =
            crate::animation_controls(ui, self.playback.controls(), live, debug_enabled);

        // A session is starting or the debugger is parked at the startup gate,
        // so no frames have arrived. The controls above stay rendered (disabled)
        // rather than the whole row vanishing until the first Continue.
        if self.playback.frames().is_empty() {
            ui.add_space(4.0);
            ui.label("Waiting for first frame from debugger\u{2026}");
            ui.ctx().request_repaint();
            return debug_clicked;
        }

        // --- Step description ---
        if let Some(frame) = self.playback.current() {
            ui.horizontal(|ui| {
                let (icon, desc) = step_description(&frame.step, &self.node_names);
                ui.label(egui::RichText::new(icon).size(16.0));
                ui.label(desc);
            });
            self.render_running_state(ui, frame);
        }

        ui.add_space(4.0);

        // --- Dependency graph visualization ---
        self.draw_graph(ui, canvas, tracked);

        debug_clicked
    }

    fn draw_graph(&self, ui: &mut egui::Ui, canvas: &mut Canvas, tracked: Option<&str>) {
        // Lay out nodes in a grid arrangement.
        let cols = grid_cols(self.n_nodes);
        let grid_rows = self.n_nodes.div_ceil(cols);
        let bounds = egui::Rect::from_min_size(
            egui::pos2(-1.0, -1.0),
            egui::vec2(cols as f32 + 2.0, grid_rows as f32 + 2.0),
        );
        let (_response, view, painter) = canvas.show(ui, bounds);

        let visuals = ui.visuals();
        let bg = visuals.extreme_bg_color;
        painter.rect_filled(view.to_screen_rect(bounds), egui::CornerRadius::ZERO, bg);

        let Some(frame) = self.playback.current() else {
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

        // Same function the camera aims with — see `equation_world_pos`.
        let node_pos = |i: usize| equation_world_pos(i, self.n_nodes);

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
        // Iterate `in_scc` rather than `0..n_nodes`: it is built as
        // `vec![None; self.n_nodes]` directly above, so the bounds are the same
        // by construction — and indexing a second collection by a range index is
        // what `needless_range_loop` warns about.
        for (i, scc_of_node) in in_scc.iter().enumerate() {
            let center = view.to_screen(node_pos(i));
            let fill = if let Some(scc_idx) = *scc_of_node {
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

    /// Equations land on a square-ish grid, one world unit apart, centred in their cell.
    ///
    /// This is the arithmetic camera aiming depends on. Claude cannot see whether the
    /// view then *looks* right — that is what the fixture tour is for — but it can
    /// check that aiming and drawing compute the same place, which is the failure that
    /// would be near-invisible: a camera landing one cell off looks plausible.
    #[test]
    fn equation_positions_tile_a_square_grid() {
        // 9 nodes -> 3 columns.
        assert_eq!(grid_cols(9), 3);
        assert_eq!(equation_world_pos(0, 9), egui::pos2(0.5, 0.5));
        assert_eq!(equation_world_pos(2, 9), egui::pos2(2.5, 0.5));
        assert_eq!(equation_world_pos(3, 9), egui::pos2(0.5, 1.5));
        assert_eq!(equation_world_pos(8, 9), egui::pos2(2.5, 2.5));

        // 10 nodes -> 4 columns (ceil(sqrt(10)) = 4), so the grid is ragged.
        assert_eq!(grid_cols(10), 4);
        assert_eq!(equation_world_pos(4, 10), egui::pos2(0.5, 1.5));

        // A single node must not divide by zero.
        assert_eq!(grid_cols(1), 1);
        assert_eq!(equation_world_pos(0, 1), egui::pos2(0.5, 0.5));
    }

    /// Aiming past the end is refused, not clamped.
    ///
    /// A tour naming an equation the model does not have is a bug *in the tour*.
    /// Clamping would aim somewhere plausible and hide it; refusing surfaces it, and
    /// the caller turns the `false` into a visible notice.
    #[test]
    fn aiming_past_the_last_equation_is_refused() {
        let anim = TarjanAnimation {
            playback: Playback::recorded(Vec::new(), FRAME_INTERVAL),
            n_nodes: 4,
            node_names: vec!["a".into(), "b".into(), "c".into(), "d".into()],
            rows: vec![Vec::new(); 4],
            unknown_names: Vec::new(),
            adj: vec![Vec::new(); 4],
        };
        let mut canvas = Canvas::default();
        assert!(anim.aim_at_equation(&mut canvas, 3), "the last equation is aimable");
        assert!(!anim.aim_at_equation(&mut canvas, 4), "one past the end is not");
        assert!(!anim.aim_at_equation(&mut canvas, 999));
    }

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
        assert_eq!(
            anim.live_state(false),
            crate::LiveState::Idle,
            "a recorded animation has no live session running",
        );
    }

    #[test]
    fn tarjan_animation_starts_paused() {
        let mat = IncidenceMatrix::from_report(&sample_report()).unwrap();
        let anim = TarjanAnimation::from_incidence(&mat).unwrap();
        assert_eq!(anim.position().0, 0);
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
            if anim.live_state(false) == crate::LiveState::Finished { break; }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        anim.playback.sync_live();
        assert!(!anim.playback.frames().is_empty());
        assert_eq!(anim.live_state(false), crate::LiveState::Finished);
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
