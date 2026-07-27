//! Bridges the lexer to the specimen source view.
//!
//! Two problems sit between `modelica_lex::tokenize` and rendering:
//!
//! 1. **Tokens are absolute and may span lines; the view draws one line at a
//!    time.** A block comment is a single token covering three lines, but line 2
//!    has to be drawn on its own. [`SourceHighlight`] splits tokens at line
//!    boundaries once per specimen, so rendering never re-tokenizes.
//! 2. **Colour and clickability come from different sources and overlap.**
//!    Token kinds come from the lexer; clickable identifier spans come from
//!    `IdentifierIndex`, which knows which names reached the DAE. A line must be
//!    cut so that every piece is uniformly coloured *and* uniformly clickable.
//!    [`segments`] does that merge.
//!
//! Keeping both here leaves `modelica_lex` free of any notion of lines, links,
//! or `egui`, and keeps the merge testable without a UI.

use crate::modelica_lex::{Token, TokenKind, tokenize};

/// A token clipped to one line, with **line-relative** byte offsets.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LineToken {
    pub kind: TokenKind,
    pub start: usize,
    pub end: usize,
}

/// Per-line token classification for one source file.
///
/// Built once when a specimen is loaded and cached — tokenizing every frame
/// would be wasteful, and the source does not change while it is displayed.
pub struct SourceHighlight {
    lines: Vec<Vec<LineToken>>,
}

impl SourceHighlight {
    /// Tokenize `source` and slice the result into lines.
    ///
    /// Line indexing matches [`str::lines`] exactly — split on `\n`, with a
    /// trailing `\r` excluded — so callers can zip this against
    /// `text.lines().enumerate()` without an off-by-one.
    pub fn new(source: &str) -> Self {
        let tokens = tokenize(source);
        let mut lines = Vec::new();
        // Tokens and lines are both sorted, so a single advancing cursor is
        // enough. It cannot simply skip past a consumed token, though: a
        // multi-line token has to appear, clipped, on every line it covers.
        let mut first = 0usize;
        for (line_start, line_end) in line_ranges(source) {
            while first < tokens.len() && tokens[first].end <= line_start {
                first += 1;
            }
            lines.push(clip_tokens(&tokens, first, line_start, line_end));
        }
        Self { lines }
    }

    /// Tokens on a 0-based line, or empty if the line is out of range.
    pub fn line(&self, index: usize) -> &[LineToken] {
        self.lines.get(index).map_or(&[], Vec::as_slice)
    }

    /// Number of lines classified.
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }
}

/// Byte ranges of each line, matching [`str::lines`] semantics.
fn line_ranges(source: &str) -> Vec<(usize, usize)> {
    let bytes = source.as_bytes();
    let mut ranges = Vec::new();
    let mut start = 0usize;
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'\n' {
            let mut end = i;
            if end > start && bytes[end - 1] == b'\r' {
                end -= 1;
            }
            ranges.push((start, end));
            start = i + 1;
        }
    }
    // A trailing newline does not produce a final empty line, matching `lines()`.
    if start < bytes.len() {
        let mut end = bytes.len();
        if end > start && bytes[end - 1] == b'\r' {
            end -= 1;
        }
        ranges.push((start, end));
    }
    ranges
}

/// Clip tokens overlapping `[line_start, line_end)` to line-relative offsets.
fn clip_tokens(tokens: &[Token], from: usize, line_start: usize, line_end: usize) -> Vec<LineToken> {
    let mut out = Vec::new();
    let mut j = from;
    while j < tokens.len() && tokens[j].start < line_end {
        let start = tokens[j].start.max(line_start) - line_start;
        let end = tokens[j].end.min(line_end) - line_start;
        if end > start {
            out.push(LineToken { kind: tokens[j].kind, start, end });
        }
        j += 1;
    }
    out
}

/// A run of text that is uniformly coloured and uniformly clickable.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Segment<'a> {
    pub text: &'a str,
    pub kind: TokenKind,
    /// The qualified variable name when this run is a clickable identifier.
    pub link: Option<&'a str>,
}

/// Merge token colouring with clickable identifier spans for one line.
///
/// The two overlap arbitrarily — a clickable span may cover part of a token, and
/// `IdentifierIndex` derives its spans independently of the lexer — so the line
/// is cut at the union of both sets of boundaries. Every resulting segment is
/// therefore inside exactly one token and either wholly inside a clickable span
/// or wholly outside one.
///
/// Segments tile the line exactly, so rendering emits every byte once.
///
/// `clickable` is `(start, end, qualified_name)` with line-relative offsets, as
/// produced by `IdentifierIndex::clickable_spans`. Entries that are out of
/// range or land mid-character are ignored rather than panicking: they come
/// from an index built against a possibly-stale copy of the source.
pub fn segments<'a>(
    line: &'a str,
    tokens: &[LineToken],
    clickable: &'a [(usize, usize, String)],
) -> Vec<Segment<'a>> {
    if line.is_empty() {
        return Vec::new();
    }

    let usable = |p: usize| p <= line.len() && line.is_char_boundary(p);

    let mut cuts = vec![0usize, line.len()];
    for t in tokens {
        cuts.push(t.start);
        cuts.push(t.end);
    }
    for (start, end, _) in clickable {
        cuts.push(*start);
        cuts.push(*end);
    }
    cuts.retain(|&p| usable(p));
    cuts.sort_unstable();
    cuts.dedup();

    let mut out = Vec::new();
    for pair in cuts.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let kind = tokens
            .iter()
            .find(|t| t.start <= a && t.end >= b)
            // No covering token means no highlight information for this run
            // (an empty `tokens`, say). `Operator` renders in the default text
            // colour, which is the right neutral fallback.
            .map_or(TokenKind::Operator, |t| t.kind);
        let link = clickable
            .iter()
            .find(|(start, end, _)| *start <= a && *end >= b && usable(*start) && usable(*end))
            .map(|(_, _, name)| name.as_str());
        out.push(Segment { text: &line[a..b], kind, link });
    }
    out
}

/// Lays out Modelica text with syntax colouring, wherever HRW shows it.
///
/// The specimen source view is not the only place: the equation sheet and the
/// Flatten source map both render `expr_format` output, which is
/// Modelica-shaped too. Routing them all through this gives the app one visual
/// language, so a keyword or a literal looks the same everywhere.
///
/// ## The colour rule this enforces
///
/// **Foreground carries syntax; background carries relationship.** Views that
/// need to say "this belongs to your selection" or "this is the tracked
/// identifier" do it with a background tint, leaving the foreground channel to
/// mean one thing consistently. Before this, the source map's equation column
/// used *foreground* colour for line-linkage, which would have collided
/// head-on with syntax colouring — the same channel carrying both "keyword" and
/// "selected", with the selection signal losing.
///
/// A builder rather than a function with six positional parameters, two of them
/// adjacent colours that a caller could transpose silently.
pub struct ModelicaText<'a> {
    font: eframe::egui::FontId,
    dark: bool,
    default_color: eframe::egui::Color32,
    tracked: Option<(&'a str, eframe::egui::Color32)>,
    background: Option<eframe::egui::Color32>,
}

impl<'a> ModelicaText<'a> {
    /// Take font, theme, and default text colour from the current style.
    pub fn new(ui: &eframe::egui::Ui) -> Self {
        Self {
            font: eframe::egui::TextStyle::Monospace.resolve(ui.style()),
            dark: ui.visuals().dark_mode,
            default_color: ui.visuals().text_color(),
            tracked: None,
            background: None,
        }
    }

    /// Highlight the tracked identifier, **per identifier token**.
    ///
    /// Not by searching the string: `contains("h")` matches `height`, and tints
    /// the whole run rather than the mention inside it.
    pub fn tracked(mut self, tracked: Option<(&'a str, eframe::egui::Color32)>) -> Self {
        self.tracked = tracked;
        self
    }

    /// Tint the whole run — the relationship channel (selected, line-linked).
    pub fn background(mut self, background: Option<eframe::egui::Color32>) -> Self {
        self.background = background;
        self
    }

    /// Lay out `text` as Modelica.
    pub fn job(&self, text: &str) -> eframe::egui::text::LayoutJob {
        let mut job = eframe::egui::text::LayoutJob::default();
        self.append(&mut job, text);
        job
    }

    /// Append Modelica `text` to an existing job, for runs that follow
    /// non-Modelica content such as a line-number gutter.
    pub fn append(&self, job: &mut eframe::egui::text::LayoutJob, text: &str) {
        for token in tokenize(text) {
            let slice = &text[token.start..token.end];
            let mut format = eframe::egui::TextFormat {
                font_id: self.font.clone(),
                color: crate::colors::syntax_color(token.kind, self.dark)
                    .unwrap_or(self.default_color),
                ..Default::default()
            };
            if let Some(background) = self.background {
                format.background = background;
            }
            // The tracked highlight outranks a whole-run tint: it is the more
            // specific statement about this particular token.
            if let Some((needle, background)) = self.tracked {
                if token.kind == TokenKind::Identifier && identifier_is(slice, needle) {
                    format.background = background;
                }
            }
            job.append(slice, 0.0, format);
        }
    }

    /// Append text that is *not* Modelica — a line-number gutter — so it is
    /// never coloured as though it were code.
    pub fn append_plain(
        &self,
        job: &mut eframe::egui::text::LayoutJob,
        text: &str,
        color: eframe::egui::Color32,
    ) {
        let mut format = eframe::egui::TextFormat {
            font_id: self.font.clone(),
            color,
            ..Default::default()
        };
        if let Some(background) = self.background {
            format.background = background;
        }
        job.append(text, 0.0, format);
    }
}

/// Whether an identifier token names the tracked variable.
///
/// Matches the bare name, and the last component of a qualified one, so
/// tracking `gear.phi` highlights the `phi` in `gear.phi` without also
/// highlighting an unrelated `phi`… which it would if only leaves were compared.
pub(crate) fn identifier_is(token: &str, tracked: &str) -> bool {
    token == tracked || tracked.ends_with(&format!(".{token}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line_of(hl: &SourceHighlight, source: &str, index: usize) -> Vec<(TokenKind, String)> {
        let line = source.lines().nth(index).unwrap();
        hl.line(index)
            .iter()
            .map(|t| (t.kind, line[t.start..t.end].to_owned()))
            .collect()
    }

    #[test]
    fn line_indexing_matches_str_lines() {
        for src in ["", "a", "a\n", "a\nb", "a\n\nb", "a\r\nb\r\n"] {
            assert_eq!(
                SourceHighlight::new(src).len(),
                src.lines().count(),
                "line count mismatch for {src:?}"
            );
        }
    }

    /// The whole reason this module exists: one token, three lines.
    #[test]
    fn block_comment_is_clipped_onto_every_line_it_covers() {
        use TokenKind::*;
        let src = "a /* one\ntwo\nthree */ b";
        let hl = SourceHighlight::new(src);
        assert_eq!(
            line_of(&hl, src, 0),
            vec![(Identifier, "a".into()), (Whitespace, " ".into()), (Comment, "/* one".into())]
        );
        assert_eq!(line_of(&hl, src, 1), vec![(Comment, "two".into())]);
        assert_eq!(
            line_of(&hl, src, 2),
            vec![(Comment, "three */".into()), (Whitespace, " ".into()), (Identifier, "b".into())]
        );
    }

    /// Each line's tokens must cover it exactly, or rendering drops or repeats
    /// text. `\r` is excluded from the line, so it must not leak in either.
    #[test]
    fn line_tokens_tile_each_line() {
        let src = "model M\r\n  Real x = 1.5; // c\r\n  /* a\nb */\nend M;";
        let hl = SourceHighlight::new(src);
        for (i, line) in src.lines().enumerate() {
            let mut pos = 0;
            for t in hl.line(i) {
                assert_eq!(t.start, pos, "gap on line {i}: {line:?}");
                pos = t.end;
            }
            assert_eq!(pos, line.len(), "line {i} not fully covered: {line:?}");
        }
    }

    #[test]
    fn segments_tile_the_line_and_carry_links() {
        let line = "  Real x = y;";
        let hl = SourceHighlight::new(line);
        let x = line.find('x').unwrap();
        let y = line.find('y').unwrap();
        let clickable = vec![
            (x, x + 1, "m.x".to_owned()),
            (y, y + 1, "m.y".to_owned()),
        ];
        let segs = segments(line, hl.line(0), &clickable);

        let rebuilt: String = segs.iter().map(|s| s.text).collect();
        assert_eq!(rebuilt, line, "segments must reproduce the line");

        let linked: Vec<_> = segs.iter().filter_map(|s| s.link.map(|l| (s.text, l))).collect();
        assert_eq!(linked, vec![("x", "m.x"), ("y", "m.y")]);
        // The type name is still coloured as a type, not swallowed by a link.
        assert!(segs.iter().any(|s| s.text == "Real" && s.kind == TokenKind::Type));
    }

    /// A quoted identifier contains spaces and a keyword; it must stay one
    /// clickable identifier rather than being cut into pieces.
    #[test]
    fn quoted_identifier_stays_one_linked_segment() {
        let line = "Real 'end of travel' = 1;";
        let hl = SourceHighlight::new(line);
        let start = line.find('\'').unwrap();
        let end = line.rfind('\'').unwrap() + 1;
        let clickable = vec![(start, end, "m.'end of travel'".to_owned())];
        let segs = segments(line, hl.line(0), &clickable);

        let linked: Vec<_> = segs.iter().filter(|s| s.link.is_some()).collect();
        assert_eq!(linked.len(), 1);
        assert_eq!(linked[0].text, "'end of travel'");
        assert_eq!(linked[0].kind, TokenKind::Identifier);
    }

    /// Spans from a stale index must not panic or corrupt the output.
    #[test]
    fn out_of_range_clickable_spans_are_ignored() {
        let line = "Real x;";
        let hl = SourceHighlight::new(line);
        let clickable = vec![(100, 200, "gone".to_owned())];
        let segs = segments(line, hl.line(0), &clickable);
        let rebuilt: String = segs.iter().map(|s| s.text).collect();
        assert_eq!(rebuilt, line);
        assert!(segs.iter().all(|s| s.link.is_none()));
    }

    /// With no highlight information the line still renders, in one neutral run.
    #[test]
    fn segments_without_tokens_still_tile() {
        let line = "anything at all";
        let segs = segments(line, &[], &[]);
        let rebuilt: String = segs.iter().map(|s| s.text).collect();
        assert_eq!(rebuilt, line);
        assert!(segs.iter().all(|s| s.kind == TokenKind::Operator));
    }

    #[test]
    fn empty_line_produces_no_segments() {
        assert!(segments("", &[], &[]).is_empty());
    }

    /// Regression for the equation sheet, which used `text.contains(tracked)`:
    /// tracking `h` shaded every equation mentioning `height`.
    #[test]
    fn tracked_identifier_matching_is_whole_token() {
        assert!(identifier_is("h", "h"));
        assert!(!identifier_is("height", "h"), "substring is not a mention");
        assert!(!identifier_is("h", "height"));
    }

    /// Tracking a qualified name highlights its final component, which is how
    /// the name appears inside an equation.
    #[test]
    fn tracked_qualified_name_matches_its_leaf() {
        assert!(identifier_is("phi", "gear.phi"));
        assert!(identifier_is("gear.phi", "gear.phi"));
        // But not an unrelated leaf that merely ends the same way.
        assert!(!identifier_is("hi", "gear.phi"));
    }
}
