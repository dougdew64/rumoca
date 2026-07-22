// Shared color constants for the HRW observatory.
//
// Centralizes colors that appear across multiple views so that palette
// changes are one-line edits and dark/light theme branching lives in one
// place instead of being copy-pasted at every call site.

use eframe::egui::Color32;

/// Success green — fixed (dark-mode) variant.
///
/// Used where a single color is needed regardless of theme (e.g. canvas
/// painters that draw over a controlled background, or the diff-highlight
/// in the tree inspector).
pub const OK_GREEN: Color32 = Color32::from_rgb(0x3F, 0xB9, 0x50);

/// Success green — theme-aware.
///
/// Returns a brighter green for dark backgrounds and a darker green for
/// light backgrounds so the text/icon contrast stays readable in both themes.
pub fn ok_color(dark_mode: bool) -> Color32 {
    if dark_mode {
        OK_GREEN
    } else {
        Color32::from_rgb(0x1A, 0x7F, 0x37)
    }
}

/// Stage-start blue — theme-aware (log view phase-start markers).
pub fn stage_start_color(dark_mode: bool) -> Color32 {
    if dark_mode {
        Color32::from_rgb(0x58, 0xA6, 0xFF)
    } else {
        Color32::from_rgb(0x0A, 0x5C, 0xC4)
    }
}

/// Warning amber — fixed color for log warnings.
pub const WARN_AMBER: Color32 = Color32::from_rgb(0xD2, 0x9E, 0x22);

/// Incidence matrix cell fill — the non-zero entry color.
pub const INCIDENCE_CELL: Color32 = Color32::from_rgb(0x42, 0x9E, 0xF5);

/// Incidence matrix hover highlight.
pub const INCIDENCE_HOVER: Color32 = Color32::from_rgb(0xFF, 0xC1, 0x07);

/// Coupled (algebraic-loop) block stroke color.
pub const COUPLED_STROKE: Color32 = Color32::from_rgb(0xF2, 0x8C, 0x28);

/// Coupled block semi-transparent fill (alpha 0x55).
pub fn coupled_fill() -> Color32 {
    Color32::from_rgba_unmultiplied(0xF2, 0x8C, 0x28, 0x55)
}

/// Grid line alpha multiplier for canvas matrix views.
pub const GRID_ALPHA: f32 = 0.3;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ok_color_differs_by_theme() {
        let dark = ok_color(true);
        let light = ok_color(false);
        assert_ne!(dark, light, "dark and light ok_color should differ");
        assert_eq!(dark, OK_GREEN, "dark ok_color should match OK_GREEN constant");
    }

    #[test]
    fn stage_start_color_differs_by_theme() {
        assert_ne!(
            stage_start_color(true),
            stage_start_color(false),
            "dark and light stage_start should differ",
        );
    }

    #[test]
    fn coupled_fill_is_semi_transparent() {
        let c = coupled_fill();
        assert!(c.a() > 0 && c.a() < 255, "coupled_fill should be semi-transparent");
    }

    #[test]
    fn grid_alpha_is_within_unit_range() {
        assert!(GRID_ALPHA > 0.0 && GRID_ALPHA <= 1.0);
    }
}
