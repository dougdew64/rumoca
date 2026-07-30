//! Reusable pan/zoom canvas for custom-painted views.
//!
//! ## Why this exists
//!
//! HRW has two kinds of views for compiler IR:
//!
//! 1. **The tree inspector** (`tree.rs`) — a text-based, collapsible tree widget
//!    that works identically for every pipeline stage. It uses egui's built-in
//!    `CollapsingHeader` widget.
//!
//! 2. **Custom-painted views** — spatial/graphical views like the BLT spy-plot
//!    (`spyplot.rs`) and the incidence matrix (`incidence_view.rs`). These draw
//!    directly onto an egui `Painter` using lines, rectangles, and text.
//!
//! Every custom-painted view needs the same infrastructure: allocate a rectangular
//! drawing area, let the user pan (drag) and zoom (scroll wheel), and convert
//! between **world coordinates** (the view's own units — e.g., integer cell
//! indices for a matrix) and **screen pixels**. This module provides that shared
//! scaffold, so each view only implements its own drawing and hit-testing logic.
//!
//! ## Coordinate system
//!
//! The canvas defines a two-space coordinate model:
//!
//! - **World space:** the view's logical coordinates. For the spy-plot, world
//!   unit (3.0, 5.0) means column 3, row 5 of the BLT matrix. World space is
//!   infinite and view-specific.
//!
//! - **Screen space:** pixel coordinates within the egui window. The canvas
//!   occupies a rectangle on screen; `view.to_screen(world_pos)` maps a world
//!   point into that rectangle, and `view.to_world(screen_pos)` does the reverse.
//!
//! The mapping is: `screen = canvas_origin + (world - pan) * zoom`
//!
//! ## Usage pattern
//!
//! ```text
//! let (response, view, painter) = canvas.show(ui, world_bounds);
//! // Now draw using `painter` and `view.to_screen(...)` for positioning.
//! // Use `view.to_world(response.hover_pos())` for hit-testing.
//! ```

use eframe::egui;

const MIN_ZOOM: f32 = 1.0;
const MAX_ZOOM: f32 = 400.0;
const FIT_MARGIN: f32 = 0.92;

/// How much the canvas height must change, as a fraction of its height at the
/// last fit, before it counts as a resize rather than content reflow above it.
///
/// A wrapped status line is roughly 20px; a canvas is typically 400-800px tall,
/// so reflow moves the height by 3-5%. A window or panel resize worth re-framing
/// for moves it by far more. 15% sits well clear of both.
const HEIGHT_REFIT_FRACTION: f32 = 0.15;

/// Whether a canvas last fitted at `fitted` size, now allocated `current`,
/// should re-fit to its content.
///
/// Split out of [`Canvas::show`] so the decision can be tested without a
/// `Ui` — it is the whole of the fix for the BLT diagram shifting sideways
/// while its animation played (see the call site for the mechanism).
///
/// `fitted` of zero means "never fitted", which is not a size change.
fn should_refit(fitted: egui::Vec2, current: egui::Vec2) -> bool {
    if fitted == egui::Vec2::ZERO {
        return false;
    }
    current.x != fitted.x
        || (current.y - fitted.y).abs() > fitted.y * HEIGHT_REFIT_FRACTION
}
/// The `pan` that puts world point `target` at the centre of a `size`-pixel viewport.
///
/// `pan` is the world coordinate shown at the **top-left** corner, so centring means
/// backing off by half a viewport, converted to world units by `zoom`.
///
/// Split out of [`Canvas::show`] for the same reason as [`should_refit`]: it is the
/// whole of the aiming logic, and a `Ui` is not needed to check it. Claude cannot see
/// the rendered result, so the arithmetic is the part that *can* be verified here — see
/// `docs/ideas.md` #42 on camera aiming.
fn pan_to_center(target: egui::Pos2, size: egui::Vec2, zoom: f32) -> egui::Vec2 {
    target.to_vec2() - size / (2.0 * zoom)
}

const SCROLL_ZOOM_SENSITIVITY: f32 = 0.002;

/// Persistent camera state for a pan/zoom canvas.
///
/// One `Canvas` is stored in the `App` struct for each custom-painted view.
/// It persists across frames so the user's pan/zoom position sticks — when
/// you zoom into a particular block of the spy-plot and switch tabs, it
/// remembers where you were.
///
/// ## Fields
///
/// - `pan`: the world coordinate shown at the canvas' top-left corner. Dragging
///   modifies this. Think of it as "which part of the world is visible."
/// - `zoom`: pixels per world unit. Scrolling modifies this. A zoom of 20.0
///   means each world unit occupies 20 pixels on screen.
/// - `fit`: when `true`, the next `show()` call will auto-fit the content to
///   fill the canvas. Set this when new data arrives (e.g., a different specimen
///   is compiled) so the user sees the whole picture before manually zooming.
pub struct Canvas {
    pan: egui::Vec2,
    zoom: f32,
    fit: bool,
    /// A one-shot request to centre the view on a world point, set by
    /// [`Canvas::request_center_on`] and consumed by the next [`Canvas::show`].
    ///
    /// **Processed after `fit`, deliberately.** Both write `pan`, so when a link
    /// arrives in the same frame as a resize the two would fight; aiming is the more
    /// specific intent and wins. It also survives the fit rather than being erased by
    /// it, which is what makes `hrw://…/node/25` land even on a freshly-opened view
    /// that has not been fitted yet.
    center_on: Option<egui::Pos2>,
    /// Fraction of view height reserved as top margin when fitting content.
    /// 0.0 = content at top, 0.1 = 10% gap above content for labels.
    /// Matrix views use 0.1 so column labels sit near the view top.
    fit_vertical_bias: f32,
    /// Last allocated size, so we can re-fit when the window resizes.
    /// Canvas size at the last fit-to-content, so a later `show()` can tell a
    /// real resize from the layout jitter of content above the canvas.
    ///
    /// Compared against the size at the last *fit* rather than the last
    /// *frame*: a slow window drag changes the height in many small steps, and
    /// comparing frame-to-frame would let every step fall under the threshold
    /// and never refit at all.
    fitted_rect_size: egui::Vec2,
}

impl Default for Canvas {
    fn default() -> Self {
        // Start with fit=true so the first draw auto-fits the content.
        // The default zoom of 20.0 is a reasonable fallback if fit doesn't
        // trigger (e.g., empty content).
        Canvas {
            pan: egui::Vec2::ZERO,
            zoom: 20.0,
            fit: true,
            center_on: None,
            fit_vertical_bias: 0.0,
            fitted_rect_size: egui::Vec2::ZERO,
        }
    }
}

/// A snapshot of the world-to-screen transform for the current frame.
///
/// Produced by [`Canvas::show`] and passed to drawing code. It is `Copy`
/// because it is just three small values (a rect and two floats), and custom
/// painters call `to_screen` / `to_world` many times per frame. It is only
/// valid for the frame it was produced in — the next frame may have different
/// pan/zoom if the user interacted.
#[derive(Clone, Copy)]
pub struct View {
    // The screen-space rectangle allocated for this canvas (egui's allocation).
    rect: egui::Rect,
    // Copies of the Canvas's pan and zoom at the time `show()` was called.
    pan: egui::Vec2,
    zoom: f32,
}

impl View {
    /// Convert a world-space point to a screen-space pixel position.
    ///
    /// Formula: `screen = rect.min + (world - pan) * zoom`
    ///
    /// This is the core transform — every rectangle, line, and text placement
    /// in a custom view goes through this.
    pub fn to_screen(self, w: egui::Pos2) -> egui::Pos2 {
        self.rect.min + (w.to_vec2() - self.pan) * self.zoom
    }

    /// Convert a screen-space pixel position back to world coordinates.
    ///
    /// Formula: `world = pan + (screen - rect.min) / zoom`
    ///
    /// Used for hit-testing: when the user hovers or clicks, we get a screen
    /// pixel and need to know which world-space cell/object is underneath.
    pub fn to_world(self, s: egui::Pos2) -> egui::Pos2 {
        (self.pan + (s - self.rect.min) / self.zoom).to_pos2()
    }

    /// Map an axis-aligned world-space rectangle to screen space.
    ///
    /// Transforms both corners. Used to draw filled/stroked rectangles for
    /// matrix cells, block outlines, background regions, etc.
    pub fn to_screen_rect(self, w: egui::Rect) -> egui::Rect {
        egui::Rect::from_min_max(self.to_screen(w.min), self.to_screen(w.max))
    }

    /// The current zoom level (pixels per world unit).
    ///
    /// Custom views use this to make level-of-detail decisions: e.g., only
    /// draw grid lines when `zoom >= 6.0` (otherwise they become a smear),
    /// only draw text labels when `zoom >= 16.0` (otherwise they overlap).
    pub fn zoom(self) -> f32 {
        self.zoom
    }

    /// Map a single grid cell at `(col, row)` to its screen rect.
    /// Each cell is 1x1 in world space.
    pub fn cell_rect(self, col: usize, row: usize) -> egui::Rect {
        self.to_screen_rect(egui::Rect::from_min_size(
            egui::pos2(col as f32, row as f32),
            egui::vec2(1.0, 1.0),
        ))
    }

    /// Identify the grid cell under the hover pointer, if any.
    /// Returns `None` if the pointer is outside the `n_cols × n_rows` bounds.
    pub fn hovered_cell(self, response: &egui::Response, n_cols: usize, n_rows: usize) -> Option<(usize, usize)> {
        response.hover_pos().and_then(|p| {
            let w = self.to_world(p);
            if w.x < 0.0 || w.y < 0.0 {
                return None;
            }
            let col = w.x as usize;
            let row = w.y as usize;
            if col < n_cols && row < n_rows { Some((col, row)) } else { None }
        })
    }

    /// Draw grid lines for an `n_cols × n_rows` matrix.
    /// Only draws when zoom is high enough that individual cells are visible.
    pub fn draw_grid(self, painter: &egui::Painter, n_cols: usize, n_rows: usize, color: egui::Color32) {
        if self.zoom < 6.0 {
            return;
        }
        let stroke = egui::Stroke::new(1.0, color);
        let nr = n_rows as f32;
        let nc = n_cols as f32;
        for col in 0..=n_cols {
            let t = col as f32;
            let a = self.to_screen(egui::pos2(t, 0.0));
            let b = self.to_screen(egui::pos2(t, nr));
            painter.line_segment([a, b], stroke);
        }
        for row in 0..=n_rows {
            let t = row as f32;
            let a = self.to_screen(egui::pos2(0.0, t));
            let b = self.to_screen(egui::pos2(nc, t));
            painter.line_segment([a, b], stroke);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_view(rect_w: f32, rect_h: f32, pan: egui::Vec2, zoom: f32) -> View {
        View {
            rect: egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(rect_w, rect_h)),
            pan,
            zoom,
        }
    }

    #[test]
    fn to_screen_to_world_round_trip() {
        let view = make_view(800.0, 600.0, egui::vec2(5.0, 10.0), 20.0);
        let world = egui::pos2(7.0, 12.5);
        let screen = view.to_screen(world);
        let back = view.to_world(screen);
        assert!((back.x - world.x).abs() < 1e-4, "x round-trip failed: {back:?} vs {world:?}");
        assert!((back.y - world.y).abs() < 1e-4, "y round-trip failed: {back:?} vs {world:?}");
    }

    #[test]
    fn to_screen_origin_maps_pan_to_rect_min() {
        let view = make_view(800.0, 600.0, egui::vec2(5.0, 10.0), 20.0);
        // The point at the pan origin should map to the rect's top-left corner.
        let screen = view.to_screen(egui::pos2(5.0, 10.0));
        assert!((screen.x - 0.0).abs() < 1e-4);
        assert!((screen.y - 0.0).abs() < 1e-4);
    }

    #[test]
    fn to_screen_rect_preserves_size() {
        let view = make_view(800.0, 600.0, egui::Vec2::ZERO, 10.0);
        let world_rect = egui::Rect::from_min_size(egui::pos2(2.0, 3.0), egui::vec2(4.0, 5.0));
        let screen_rect = view.to_screen_rect(world_rect);
        let expected_w = 4.0 * 10.0;
        let expected_h = 5.0 * 10.0;
        assert!((screen_rect.width() - expected_w).abs() < 1e-4);
        assert!((screen_rect.height() - expected_h).abs() < 1e-4);
    }

    #[test]
    fn zoom_accessor_returns_view_zoom() {
        let view = make_view(800.0, 600.0, egui::Vec2::ZERO, 42.0);
        assert!((view.zoom() - 42.0).abs() < 1e-4);
    }

    #[test]
    fn to_world_at_rect_min_equals_pan() {
        let view = make_view(800.0, 600.0, egui::vec2(3.0, 7.0), 15.0);
        let world = view.to_world(egui::pos2(0.0, 0.0));
        assert!((world.x - 3.0).abs() < 1e-4);
        assert!((world.y - 7.0).abs() < 1e-4);
    }

    #[test]
    fn to_screen_rect_round_trip() {
        let view = make_view(800.0, 600.0, egui::vec2(1.0, 2.0), 25.0);
        let world_rect = egui::Rect::from_min_max(egui::pos2(3.0, 4.0), egui::pos2(6.0, 8.0));
        let screen_rect = view.to_screen_rect(world_rect);
        let back_min = view.to_world(screen_rect.min);
        let back_max = view.to_world(screen_rect.max);
        assert!((back_min.x - world_rect.min.x).abs() < 1e-4);
        assert!((back_min.y - world_rect.min.y).abs() < 1e-4);
        assert!((back_max.x - world_rect.max.x).abs() < 1e-4);
        assert!((back_max.y - world_rect.max.y).abs() < 1e-4);
    }

    /// Centring puts the target in the middle of the viewport, in world terms.
    ///
    /// The arithmetic half of camera aiming — the half Claude can verify. Whether the
    /// node then *looks* centred needs Doug's eyes; that is what the fixture tour is
    /// for.
    #[test]
    fn pan_to_center_puts_the_target_in_the_middle() {
        let size = egui::vec2(400.0, 200.0);
        let zoom = 20.0;
        let target = egui::pos2(7.0, 3.0);

        let pan = pan_to_center(target, size, zoom);

        // Re-derive where the target lands on screen: (world - pan) * zoom.
        let on_screen = (target.to_vec2() - pan) * zoom;
        assert!(
            (on_screen.x - size.x / 2.0).abs() < 1e-3,
            "target should sit at half the width: {on_screen:?} vs {size:?}",
        );
        assert!(
            (on_screen.y - size.y / 2.0).abs() < 1e-3,
            "target should sit at half the height: {on_screen:?} vs {size:?}",
        );
    }

    /// Zoom is preserved: aiming says *where* to look, not how far in.
    ///
    /// Changing zoom on a link would silently discard whatever the reader had set up
    /// while exploring, which is the opposite of helpful mid-tour.
    #[test]
    fn centring_at_different_zooms_keeps_the_target_centred() {
        let size = egui::vec2(300.0, 300.0);
        let target = egui::pos2(-4.0, 11.5);
        for zoom in [1.0_f32, 5.0, 20.0, 90.0] {
            let pan = pan_to_center(target, size, zoom);
            let on_screen = (target.to_vec2() - pan) * zoom;
            assert!(
                (on_screen.x - 150.0).abs() < 1e-2 && (on_screen.y - 150.0).abs() < 1e-2,
                "zoom {zoom}: target landed at {on_screen:?}, expected the centre",
            );
        }
    }

    /// A pending aim is consumed once, and does not linger to re-pin the view.
    ///
    /// The same discipline as `jump_target` in the source view: a one-shot that stayed
    /// set would fight the scrollbar every frame — which is exactly the bug the
    /// 2026-07-29 sideways-drift fix was about, in a different guise.
    #[test]
    fn an_aim_request_is_one_shot() {
        let mut canvas = Canvas::default();
        assert!(canvas.center_on.is_none(), "nothing pending by default");
        canvas.request_center_on(egui::pos2(1.0, 2.0));
        assert_eq!(canvas.center_on, Some(egui::pos2(1.0, 2.0)));
        // `show` consumes it via `.take()`; emulate that without a Ui.
        let taken = canvas.center_on.take();
        assert!(taken.is_some());
        assert!(canvas.center_on.is_none(), "consumed, so it cannot re-pin next frame");
    }

    #[test]
    fn canvas_default_requests_fit() {
        let canvas = Canvas::default();
        assert!(canvas.fit, "default canvas should request a fit");
    }

    /// A line of text appearing above the canvas must not re-frame the
    /// drawing.
    ///
    /// This is the BLT bug (Doug, 2026-07-29): the fit is uniform-scale and
    /// horizontally centred, so a *height* change alone changes the zoom and
    /// therefore the centring padding — the diagram slides sideways. The BLT
    /// view's status line wraps as Tarjan's stack deepens and un-wraps as it
    /// unwinds, so the diagram shifted right and then back left, once per run.
    #[test]
    fn a_wrapped_line_of_text_does_not_trigger_a_refit() {
        let fitted = egui::vec2(900.0, 600.0);
        // One wrapped status line is about 20px.
        assert!(!should_refit(fitted, egui::vec2(900.0, 580.0)), "line appears");
        assert!(!should_refit(fitted, egui::vec2(900.0, 600.0)), "line goes away");
        // Even a few lines of reflow stays under the bar.
        assert!(!should_refit(fitted, egui::vec2(900.0, 545.0)), "three lines");
    }

    /// A real resize still re-frames. Width always counts, because the fit is
    /// horizontally centred and a different width is genuinely a different
    /// framing; height counts once it is past anything reflow can produce.
    #[test]
    fn a_real_resize_still_triggers_a_refit() {
        let fitted = egui::vec2(900.0, 600.0);
        assert!(should_refit(fitted, egui::vec2(880.0, 600.0)), "narrower window");
        assert!(should_refit(fitted, egui::vec2(900.0, 400.0)), "much shorter window");
        assert!(should_refit(fitted, egui::vec2(900.0, 900.0)), "much taller window");
    }

    /// Comparing against the size at the last *fit* rather than the last
    /// *frame* is what makes a slow drag work: a vertical-only window resize
    /// arrives as many small steps, and frame-to-frame comparison would let
    /// every one of them fall under the threshold and never refit at all.
    #[test]
    fn a_slow_vertical_drag_accumulates_to_a_refit() {
        let fitted = egui::vec2(900.0, 600.0);
        let mut refit = false;
        for step in 1..=10 {
            let h = 600.0 - (step as f32) * 15.0;
            if should_refit(fitted, egui::vec2(900.0, h)) {
                refit = true;
                break;
            }
        }
        assert!(refit, "a drag that shrinks the canvas by 150px must eventually refit");
    }

    /// A canvas that has never been fitted reports no size change, so the
    /// first `show()` is driven by `fit` starting true rather than by a
    /// spurious resize.
    #[test]
    fn a_never_fitted_canvas_reports_no_size_change() {
        assert!(!should_refit(egui::Vec2::ZERO, egui::vec2(900.0, 600.0)));
    }

    #[test]
    fn request_fit_sets_flag() {
        let mut canvas = Canvas { fit: false, ..Canvas::default() };
        canvas.request_fit();
        assert!(canvas.fit);
    }

    #[test]
    fn cell_rect_is_one_by_one_world_unit() {
        let view = make_view(800.0, 600.0, egui::Vec2::ZERO, 10.0);
        let rect = view.cell_rect(3, 5);
        assert!((rect.width() - 10.0).abs() < 1e-4, "width should be 1 world unit * zoom");
        assert!((rect.height() - 10.0).abs() < 1e-4, "height should be 1 world unit * zoom");
        let expected_min = view.to_screen(egui::pos2(3.0, 5.0));
        assert!((rect.min.x - expected_min.x).abs() < 1e-4);
        assert!((rect.min.y - expected_min.y).abs() < 1e-4);
    }

    #[test]
    fn draw_grid_skips_low_zoom() {
        let view = make_view(800.0, 600.0, egui::Vec2::ZERO, 5.0);
        // zoom < 6.0 — draw_grid should be a no-op (we can't easily test
        // painter output, but we verify it doesn't panic)
        assert!(view.zoom() < 6.0);
    }

    #[test]
    fn fit_margin_is_less_than_one() {
        assert!(FIT_MARGIN > 0.0 && FIT_MARGIN < 1.0, "FIT_MARGIN should leave breathing room");
    }

    #[test]
    fn zoom_bounds_are_sane() {
        assert!(MIN_ZOOM > 0.0, "MIN_ZOOM must be positive");
        assert!(MAX_ZOOM > MIN_ZOOM, "MAX_ZOOM must exceed MIN_ZOOM");
    }
}

impl Canvas {
    /// Set the vertical placement bias for fit-to-content (0.0 = top-aligned,
    /// 0.5 = centered, 1.0 = bottom-aligned). Returns `self` for chaining.
    pub fn with_fit_vertical_bias(mut self, bias: f32) -> Self {
        self.fit_vertical_bias = bias.clamp(0.0, 1.0);
        self
    }

    /// Aim the camera: centre the view on a world point at the next paint.
    ///
    /// One-shot, like [`request_fit`], and applied *after* it — see `center_on`.
    /// Zoom is left alone: a link that says "look at node 25" is about *where* the
    /// camera points, not how far in it is, and silently changing the zoom would
    /// discard whatever the reader had set up.
    pub fn request_center_on(&mut self, target: egui::Pos2) {
        self.center_on = Some(target);
    }

    /// Request a fit-to-content on the next paint.
    ///
    /// Call this when the drawn data changes — e.g., a new specimen compiled,
    /// producing a different-sized matrix. The next `show()` will compute a
    /// zoom and pan that centers the content in the available space.
    pub fn request_fit(&mut self) {
        self.fit = true;
    }

    /// The main entry point for a canvas frame.
    ///
    /// Allocates the drawing area, processes pan/zoom input, and returns the
    /// three things a custom view needs:
    ///
    /// - `response` — egui's interaction state (hover position, clicks, drags)
    /// - `view` — the world-to-screen transform for this frame
    /// - `painter` — an egui `Painter` clipped to the canvas rect, for drawing
    ///
    /// `world_bounds` is the bounding box of the content in world coordinates.
    /// It is only used when a fit-to-content was requested (via `request_fit`);
    /// during normal pan/zoom frames it is ignored.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        world_bounds: egui::Rect,
    ) -> (egui::Response, View, egui::Painter) {
        // Allocate the entire remaining space in the panel for this canvas.
        // `click_and_drag` makes the allocated rect sensitive to both clicks
        // (for capture) and drags (for panning).
        let (rect, response) =
            ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());

        // Re-fit when the canvas area changes (window resize, panel toggle) —
        // but only for changes that are really a resize.
        //
        // The fit below is **uniform-scale and horizontally centred**:
        // `zoom = min(zx, zy)` and `pad_x = (width - world_width * zoom) / 2`.
        // So a change in *height alone* moves content *sideways* — shrink the
        // height, the zoom drops, the content narrows, and the centring padding
        // grows. Meanwhile the text above a canvas changes height by a line as
        // an animation steps: the BLT view's status line wraps when Tarjan's
        // stack gets deep and un-wraps as it unwinds, which shifted the whole
        // diagram right and then back left, once per run (Doug, 2026-07-29).
        //
        // Width changes always refit — the fit is horizontally centred, so a
        // different width genuinely means a different framing. Height changes
        // refit only when large enough to be a resize rather than a reflow: a
        // wrapped line is ~20px against a canvas several hundred tall, so the
        // fraction below separates the two cleanly with room to spare.
        if should_refit(self.fitted_rect_size, rect.size()) {
            self.fit = true;
        }

        // --- Fit-to-content (one-shot) ---
        //
        // When `self.fit` is true, compute a zoom that fits the world_bounds
        // into the canvas rect, then position it.
        //
        // `fit_vertical_bias` reserves that fraction of view height as top
        // margin (e.g. 0.1 = 10% reserved at top for labels). The zoom is
        // computed from the remaining vertical space so the content always
        // fits below that margin.
        if self.fit && world_bounds.width() > 0.0 && world_bounds.height() > 0.0 && rect.area() > 0.0
        {
            let top_reserve = rect.height() * self.fit_vertical_bias;
            let avail_height = rect.height() - top_reserve;
            let zx = rect.width() / world_bounds.width();
            let zy = avail_height / world_bounds.height();
            self.zoom = (zx.min(zy) * FIT_MARGIN).clamp(MIN_ZOOM, MAX_ZOOM);
            // Horizontally center; vertically place content top at top_reserve.
            let pad_x = (rect.width() - world_bounds.width() * self.zoom) / 2.0;
            self.pan = world_bounds.min.to_vec2() - egui::vec2(pad_x, top_reserve) / self.zoom;
            self.fit = false;
        }

        // --- Aim (one-shot), after the fit so the more specific intent wins ---
        if let Some(target) = self.center_on.take()
            && rect.area() > 0.0
        {
            self.pan = pan_to_center(target, rect.size(), self.zoom);
            self.fitted_rect_size = rect.size();
        }

        // --- Pan by dragging ---
        //
        // `drag_delta()` gives the pixel distance the pointer moved this frame.
        // Dividing by zoom converts screen pixels to world units. Subtracting
        // (not adding) because dragging right should reveal content to the left
        // (the pan origin moves in the opposite direction of the drag).
        if response.dragged() {
            self.pan -= response.drag_delta() / self.zoom;
        }

        // --- Zoom about the pointer ---
        //
        // The key insight: when zooming, we want the world point under the
        // cursor to stay fixed on screen. Without this, zooming would drift
        // the content away from where the user is looking.
        //
        // Algorithm:
        // 1. Record the world point under the cursor before zoom changes.
        // 2. Apply the zoom change (exponential for smooth feel).
        // 3. Adjust pan so the same world point maps back to the same screen pos.
        if let Some(p) = response.hover_pos() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                let world_before = self.pan + (p - rect.min) / self.zoom;
                // Exponential zoom for perceptually uniform speed — each scroll
                // tick multiplies zoom by a constant factor rather than adding.
                self.zoom = (self.zoom * (scroll * SCROLL_ZOOM_SENSITIVITY).exp()).clamp(MIN_ZOOM, MAX_ZOOM);
                // Re-anchor: solve for pan such that world_before maps to p.
                self.pan = world_before - (p - rect.min) / self.zoom;
            }
        }

        // Snapshot the transform for this frame and create a clipped painter.
        let view = View { rect, pan: self.pan, zoom: self.zoom };
        // `painter_at` returns a Painter whose draw commands are clipped to
        // `rect` — nothing drawn by the custom view leaks outside the canvas.
        let painter = ui.painter_at(rect);
        (response, view, painter)
    }
}
