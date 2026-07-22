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
        Color32::from_rgb(0x3F, 0xB9, 0x50)
    } else {
        Color32::from_rgb(0x1A, 0x7F, 0x37)
    }
}

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
}
