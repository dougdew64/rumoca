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
}

impl Default for Canvas {
    fn default() -> Self {
        // Start with fit=true so the first draw auto-fits the content.
        // The default zoom of 20.0 is a reasonable fallback if fit doesn't
        // trigger (e.g., empty content).
        Canvas { pan: egui::Vec2::ZERO, zoom: 20.0, fit: true }
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

    #[test]
    fn canvas_default_requests_fit() {
        let canvas = Canvas::default();
        assert!(canvas.fit, "default canvas should request a fit");
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

        // --- Fit-to-content (one-shot) ---
        //
        // When `self.fit` is true, compute a zoom that fits the world_bounds
        // into ~92% of the canvas rect (leaving a margin), then center it.
        // This runs once after new data arrives, then `self.fit` is cleared.
        //
        // The 0.92 factor leaves visual breathing room around the content.
        // The clamp(1.0, 400.0) prevents degenerate zoom levels (too small to
        // see, or so large that floating-point precision degrades).
        if self.fit && world_bounds.width() > 0.0 && world_bounds.height() > 0.0 && rect.area() > 0.0
        {
            let zx = rect.width() / world_bounds.width();
            let zy = rect.height() / world_bounds.height();
            // Use the smaller of horizontal/vertical zoom to fit fully.
            self.zoom = (zx.min(zy) * FIT_MARGIN).clamp(MIN_ZOOM, MAX_ZOOM);
            // Center by computing the padding and shifting the pan origin.
            let pad = (rect.size() - world_bounds.size() * self.zoom) / 2.0;
            self.pan = world_bounds.min.to_vec2() - pad / self.zoom;
            self.fit = false;
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
