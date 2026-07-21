//! Reusable pan/zoom canvas for custom-`Painter` views (Arc 3, charter §4.4).
//!
//! The tree inspector is one generic widget over every stage's IR; the Arc-3
//! structural views (BLT spy-plot now, more later) are custom-painted, and they
//! all need the same plumbing: allocate a drawing rect, let the user pan (drag)
//! and zoom (scroll about the pointer), and convert between *world* coordinates
//! (the view's own units — for the spy-plot, integer cell (col, row)) and screen
//! pixels. That shared scaffold lives here so each view only writes its drawing +
//! hit-testing, not its own camera.

use eframe::egui;

/// Persistent camera state for a canvas. `pan` is the world coordinate shown at
/// the canvas' top-left corner; `zoom` is pixels per world unit. Held in `App`
/// across frames so the user's pan/zoom sticks; `fit` requests a one-shot
/// fit-to-content on the next paint (set it when new data arrives).
pub struct Canvas {
    pan: egui::Vec2,
    zoom: f32,
    fit: bool,
}

impl Default for Canvas {
    fn default() -> Self {
        Canvas { pan: egui::Vec2::ZERO, zoom: 20.0, fit: true }
    }
}

/// Per-frame transform between world and screen coordinates, produced by
/// [`Canvas::show`]. Cheap to copy; valid only for the frame it was made in.
#[derive(Clone, Copy)]
pub struct View {
    rect: egui::Rect,
    pan: egui::Vec2,
    zoom: f32,
}

impl View {
    /// World point → screen pixel.
    pub fn to_screen(self, w: egui::Pos2) -> egui::Pos2 {
        self.rect.min + (w.to_vec2() - self.pan) * self.zoom
    }

    /// Screen pixel → world point.
    pub fn to_world(self, s: egui::Pos2) -> egui::Pos2 {
        (self.pan + (s - self.rect.min) / self.zoom).to_pos2()
    }

    /// The world-space rectangle `[min, max)` mapped to a screen rectangle.
    pub fn to_screen_rect(self, w: egui::Rect) -> egui::Rect {
        egui::Rect::from_min_max(self.to_screen(w.min), self.to_screen(w.max))
    }

    /// Pixels per world unit — a view uses this to decide when cells are large
    /// enough to label, etc.
    pub fn zoom(self) -> f32 {
        self.zoom
    }
}

impl Canvas {
    /// Request a fit-to-content on the next paint (call when the drawn data
    /// changes — e.g. a new specimen compiled).
    pub fn request_fit(&mut self) {
        self.fit = true;
    }

    /// Allocate the canvas over the remaining space, apply pan/zoom interaction,
    /// and return `(response, view, painter)`. `world_bounds` is the extent the
    /// drawing occupies in world units — used only to fit-to-content when a fit
    /// was requested. The returned painter is clipped to the canvas rect.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        world_bounds: egui::Rect,
    ) -> (egui::Response, View, egui::Painter) {
        let (rect, response) =
            ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());

        // One-shot fit: scale so the content fills ~92% of the rect, centered.
        if self.fit && world_bounds.width() > 0.0 && world_bounds.height() > 0.0 && rect.area() > 0.0
        {
            let zx = rect.width() / world_bounds.width();
            let zy = rect.height() / world_bounds.height();
            self.zoom = (zx.min(zy) * 0.92).clamp(1.0, 400.0);
            let pad = (rect.size() - world_bounds.size() * self.zoom) / 2.0;
            self.pan = world_bounds.min.to_vec2() - pad / self.zoom;
            self.fit = false;
        }

        // Pan by dragging.
        if response.dragged() {
            self.pan -= response.drag_delta() / self.zoom;
        }

        // Zoom about the pointer (keep the world point under the cursor fixed).
        if let Some(p) = response.hover_pos() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                let world_before = self.pan + (p - rect.min) / self.zoom;
                self.zoom = (self.zoom * (scroll * 0.002).exp()).clamp(1.0, 400.0);
                self.pan = world_before - (p - rect.min) / self.zoom;
            }
        }

        let view = View { rect, pan: self.pan, zoom: self.zoom };
        let painter = ui.painter_at(rect);
        (response, view, painter)
    }
}
