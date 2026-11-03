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

/// Matched-pair marker on the incidence matrix (the transversal diagonal).
pub const MATCHED_MARKER: Color32 = Color32::from_rgb(0x3F, 0xB9, 0x50);

/// Unmatched row/column band — rank deficiency indicator.
pub const UNMATCHED_BAND: Color32 = Color32::from_rgb(0xE5, 0x39, 0x35);

/// BLT block boundary stroke on the incidence matrix.
pub const BLT_BOUNDARY: Color32 = Color32::from_rgb(0xF2, 0x8C, 0x28);

/// Currently-explored edge in the augmenting-path animation.
pub const ANIM_EXPLORE: Color32 = Color32::from_rgb(0xFF, 0xD5, 0x4F);

/// Augmenting-path found — the successful path highlight.
pub const ANIM_PATH_FOUND: Color32 = Color32::from_rgb(0x66, 0xBB, 0x6A);

/// Failed exploration — dead-end backtrack.
pub const ANIM_FAIL: Color32 = Color32::from_rgb(0xEF, 0x53, 0x50);

/// Tracked-identifier highlight — opaque gold for text/strokes.
pub const TRACKED_GOLD: Color32 = Color32::from_rgb(0xFF, 0xD5, 0x4F);

/// The Context Bar's **Always** row — specimen, stage, open lab, stage IRs, DefIds.
///
/// One of three colours, one per category of context (Doug, 2026-08-30: *"make the
/// context bar contents easier to read by using different text colors for each of the
/// three categories"*). The other two are [`CONTEXT_POINT`] and [`TRACKED_GOLD`].
///
/// **Named for the word on screen, not for the word in the design docs.**
/// `context-assembly.md` calls this the *background*; the bar has always labelled the
/// row `Always`, and a reader matching a colour to a category reads the label. It was
/// `CONTEXT_BACKGROUND` for one commit, before the two halves of this category were
/// found to be rendering under different names.
///
/// **Quiet on purpose, and the quietest of the three.** This is what is true whatever
/// you have clicked; the other two are what *you* assembled, and should win the eye. It
/// replaced `ui.weak`, which was theme-adaptive — a deliberate trade for being
/// distinguishable from the two categories beside it, which is what was asked.
pub const CONTEXT_ALWAYS: Color32 = Color32::from_rgb(0x8A, 0x9E, 0xA8);

/// The Context Bar's **point** — the one node you are pointing at.
///
/// **The opaque twin of [`JUMP_FILL`]**, exactly as [`TRACKED_GOLD`] is the opaque twin
/// of [`TRACKED_FILL`]. That pairing is the whole reason this value and not another:
/// `JUMP_FILL`'s own note already fixes the vocabulary — *"Cyan, deliberately not gold.
/// Gold means followed — a thread through every stage — and a jump target is a
/// different thing: one row, one link, this moment."* A point is that second thing, so
/// the bar says it in the colour the panes already say it in.
pub const CONTEXT_POINT: Color32 = Color32::from_rgb(0x42, 0xC5, 0xF5);

/// A **scratch specimen** in the model list — a probe Claude wrote to answer one
/// question, living in the gitignored bridge directory rather than the curated corpus.
///
/// **Its own constant since 2026-08-30, on Doug's instruction.** The row had borrowed
/// [`ANIM_EXPLORE`], whose documented meaning is *"currently-explored edge in the
/// augmenting-path animation"* — a scratch file is not an explored edge, and a palette
/// entry read by two unrelated features cannot be retuned for either.
///
/// **The value deliberately matches what was on screen before**, so this is a naming
/// change and not a visual one. That it coincides with [`ANIM_EXPLORE`] and
/// [`TRACKED_GOLD`] is now a coincidence the three are free to break independently,
/// which is the whole point of separating them.
pub const SCRATCH_SPECIMEN: Color32 = Color32::from_rgb(0xFF, 0xD5, 0x4F);

/// A translucent tint: a normal RGB colour at `alpha`, premultiplied correctly.
///
/// `Color32::from_rgba_premultiplied` requires every channel to be **less than
/// or equal to** the alpha; passing full-strength RGB with a low alpha does not
/// produce a faint tint but a near-opaque additive wash. Three constants below
/// did exactly that until 2026-07-27. It went unnoticed while the text
/// underneath was uncoloured — once syntax colouring landed, the wash buried it
/// completely and no syntax colour was distinguishable.
///
/// `from_rgba_unmultiplied` does this conversion but is not `const` in this egui
/// version, hence doing it here so the call sites stay readable as
/// "this colour, this faint".
const fn tint(r: u8, g: u8, b: u8, alpha: u8) -> Color32 {
    let a = alpha as u32;
    Color32::from_rgba_premultiplied(
        ((r as u32 * a) / 255) as u8,
        ((g as u32 * a) / 255) as u8,
        ((b as u32 * a) / 255) as u8,
        alpha,
    )
}

/// Background wash on the row a `hrw://…/node/<path>` link pointed at.
///
/// **Cyan, deliberately not gold.** Gold means *followed* — a thread through every
/// stage — and a jump target is a different thing: one row, one link, this moment.
/// Reusing gold would make a lab stop look like it had set a follow.
///
/// Exists because scrolling a row to the centre of a screen full of near-identical
/// rows, without marking it, leaves the reader guessing which one was the target.
/// The node-pointing fixture lab asserted this highlight before it was built —
/// Doug run the lab and found the claim false (2026-07-30).
pub const JUMP_FILL: Color32 = tint(0x42, 0xC5, 0xF5, 0x45);

/// Tracked-identifier background fill (subtle, alpha 0x30 ≈ 19%).
pub const TRACKED_FILL: Color32 = tint(0xFF, 0xD5, 0x4F, 0x30);

/// Tracked-identifier background fill (medium, alpha 0x40 ≈ 25%).
pub const TRACKED_FILL_MEDIUM: Color32 = tint(0xFF, 0xD5, 0x4F, 0x40);

/// SCC palette — distinct colors for coloring strongly connected components
/// in the Tarjan animation graph view.
pub const SCC_PALETTE: [Color32; 6] = [
    Color32::from_rgb(0x42, 0xA5, 0xF5), // blue
    Color32::from_rgb(0xAB, 0x47, 0xBC), // purple
    Color32::from_rgb(0x26, 0xA6, 0x9A), // teal
    Color32::from_rgb(0xFF, 0x70, 0x43), // deep orange
    Color32::from_rgb(0x78, 0x90, 0x9C), // blue-grey
    Color32::from_rgb(0xEC, 0x40, 0x7A), // pink
];

/// Equation category colors — used in the equation sheet to distinguish
/// equation origins (component, connection, flow-sum, binding, event).
pub const EQ_CAT_COMPONENT: Color32 = Color32::from_rgb(100, 180, 255);
pub const EQ_CAT_CONNECTION: Color32 = Color32::from_rgb(255, 180, 80);
pub const EQ_CAT_FLOW_SUM: Color32 = Color32::from_rgb(255, 120, 80);
pub const EQ_CAT_BINDING: Color32 = Color32::from_rgb(160, 200, 120);
pub const EQ_CAT_EVENT: Color32 = Color32::from_rgb(200, 140, 220);

/// Solver diagnostics plot — step size (h) line.
pub const SOLVER_STEP_SIZE: Color32 = Color32::from_rgb(70, 130, 230);

/// Solver diagnostics plot — BDF order (k) line.
pub const SOLVER_BDF_ORDER: Color32 = Color32::from_rgb(230, 130, 70);

/// Source-map equation-linked line highlight (light blue, alpha 40 ≈ 16%).
///
/// Sits behind syntax-coloured Modelica text, so it must shift the surface
/// without competing with the glyphs — see [`tint`].
pub const SOURCE_MAP_LINK: Color32 = tint(100, 180, 255, 40);

/// Clickable identifier text in the source view (light blue).
pub const CLICKABLE_IDENT: Color32 = Color32::from_rgb(0x64, 0xB5, 0xF6);

/// Syntax colour for a Modelica token, or `None` to use the default text colour.
///
/// Deliberately restrained. This is a *reading* surface inside an observatory,
/// not an editor: the eye needs to find declarations and skip commentary, while
/// clickable identifiers (`CLICKABLE_IDENT`) and the tracked identifier
/// (`TRACKED_GOLD`) must still stand out against everything else. So keywords
/// and types lead, comments recede, literals are marked — and identifiers and
/// operators stay default, leaving the two interactive colours unrivalled.
pub fn syntax_color(kind: crate::modelica_lex::TokenKind, dark_mode: bool) -> Option<Color32> {
    use crate::modelica_lex::TokenKind as K;
    Some(match (kind, dark_mode) {
        (K::Keyword, true) => Color32::from_rgb(0xC5, 0x92, 0xE8),
        (K::Keyword, false) => Color32::from_rgb(0x7B, 0x2C, 0xBF),
        (K::Type, true) => Color32::from_rgb(0x4E, 0xC9, 0xB0),
        (K::Type, false) => Color32::from_rgb(0x0E, 0x70, 0x60),
        (K::Number, true) => Color32::from_rgb(0xD1, 0x9A, 0x66),
        (K::Number, false) => Color32::from_rgb(0x8A, 0x51, 0x00),
        (K::String, true) => Color32::from_rgb(0xCE, 0x91, 0x78),
        (K::String, false) => Color32::from_rgb(0x9B, 0x2C, 0x2C),
        (K::Comment, true) => Color32::from_rgb(0x6A, 0x9B, 0x6A),
        (K::Comment, false) => Color32::from_rgb(0x4A, 0x7A, 0x4A),
        // Identifiers and operators keep the default text colour so the
        // clickable and tracked highlights remain the loudest thing on screen.
        (K::Identifier | K::Operator | K::Whitespace, _) => return None,
    })
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
        assert_eq!(
            dark, OK_GREEN,
            "dark ok_color should match OK_GREEN constant"
        );
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
        assert!(
            c.a() > 0 && c.a() < 255,
            "coupled_fill should be semi-transparent"
        );
    }

    /// Checked at **compile time** rather than as a `#[test]` — see the note on the
    /// equivalent block in `canvas.rs`. A constant's range cannot be wrong only
    /// when the tests happen to run.
    const _: () = {
        assert!(
            GRID_ALPHA > 0.0 && GRID_ALPHA <= 1.0,
            "GRID_ALPHA is an alpha fraction"
        );
    };
}
