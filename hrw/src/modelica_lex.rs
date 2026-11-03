//! Lexical scanner for Modelica source text.
//!
//! Answers one question — *what is at this byte offset?* — for any specimen.
//! Four features need that answer and none of them can get it today: syntax
//! highlighting (`docs/ideas.md` #36), clickable identifiers, reverse identifier
//! tracking (#37), and forward identifier debugging. See
//! `docs/identity-and-provenance.md`.
//!
//! ## Scope: lexical only
//!
//! This is a scanner, not a parser. It classifies bytes; it does not know that
//! `model` opens a class or that `end` closes one. That is deliberate — syntax
//! highlighting and identifier location both work at the token level, and a
//! parser would have to cope with incomplete or invalid source while the user
//! is looking at it. Rumoca's own `rumoca-phase-parse` does the real parsing.
//!
//! ## Two properties this guarantees
//!
//! 1. **It never fails.** Any byte sequence produces tokens. Unterminated
//!    strings and comments run to end-of-input; bytes that match nothing become
//!    [`TokenKind::Operator`]. A view rendering half-typed source must not
//!    panic or silently drop text.
//! 2. **Tokens tile the input exactly, on character boundaries.** Concatenating
//!    every token's byte range in order reproduces the source, with no gaps and
//!    no overlaps — whitespace included, which is why [`TokenKind::Whitespace`]
//!    exists. Rendering can therefore run the token list and emit every byte
//!    exactly once, rather than interleaving tokens with separately-tracked
//!    "the rest of the line". Every boundary falls between characters, so
//!    `source[tok.start..tok.end]` is always a legal slice — a caller cannot
//!    make this panic by handing it text with a non-ASCII character in it.
//!
//! Both are enforced by tests over every specimen in `specimens/`.
//!
//! ## Whole file, not line by line
//!
//! `tokenize` takes the entire source. Block comments span lines, so a per-line
//! scanner cannot classify `*/` on line 5 without knowing that line 3 opened a
//! comment. Callers that render per-line should tokenize once and map tokens to
//! lines, not tokenize each line.

/// What a run of bytes is.
///
/// Deliberately coarse: this is what a *reader* distinguishes at a glance, not
/// what a grammar distinguishes. `Keyword` and `Type` are separated because
/// Modelica's predefined types (`Real`, `Integer`, …) read as a distinct
/// category even though the grammar treats them as identifiers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TokenKind {
    /// A reserved word — `model`, `equation`, `when`, `end`, …
    Keyword,
    /// A predefined type name — `Real`, `Integer`, `Boolean`, `String`, …
    Type,
    /// A user-defined name, including quoted identifiers (`'foo bar'`).
    Identifier,
    /// A numeric literal, including any exponent.
    Number,
    /// A double-quoted string literal, including its quotes.
    String,
    /// A `//` line comment or a `/* */` block comment, including the markers.
    Comment,
    /// Punctuation and operators, and any byte that matches nothing else.
    Operator,
    /// Spaces, tabs, newlines, carriage returns.
    Whitespace,
}

/// One classified run of bytes. `end` is exclusive.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Token {
    pub kind: TokenKind,
    pub start: usize,
    pub end: usize,
}

impl Token {
    /// Slice this token's text out of the source it came from.
    pub fn text<'a>(&self, source: &'a str) -> &'a str {
        &source[self.start..self.end]
    }
}

/// Modelica reserved words (Modelica Language Specification 3.x, §2.3.3).
///
/// `der`, `initial`, and `pure` are reserved even though they read like
/// function names, so they are listed here rather than treated as identifiers.
const KEYWORDS: &[&str] = &[
    "algorithm",
    "and",
    "annotation",
    "block",
    "break",
    "class",
    "connect",
    "connector",
    "constant",
    "constrainedby",
    "der",
    "discrete",
    "each",
    "else",
    "elseif",
    "elsewhen",
    "encapsulated",
    "end",
    "enumeration",
    "equation",
    "expandable",
    "extends",
    "external",
    "false",
    "final",
    "flow",
    "for",
    "function",
    "if",
    "import",
    "impure",
    "in",
    "initial",
    "inner",
    "input",
    "loop",
    "model",
    "not",
    "operator",
    "or",
    "outer",
    "output",
    "package",
    "parameter",
    "partial",
    "protected",
    "public",
    "pure",
    "record",
    "redeclare",
    "replaceable",
    "return",
    "stream",
    "then",
    "true",
    "type",
    "when",
    "while",
    "within",
];

/// Predefined type names. Not reserved by the grammar, but they read as types
/// and colouring them as such is what makes a declaration scannable.
const TYPES: &[&str] = &[
    "Real",
    "Integer",
    "Boolean",
    "String",
    "StateSelect",
    "ExternalObject",
    "Clock",
];

/// Two-byte operators, checked before single bytes so `:=` does not lex as
/// `:` followed by `=`. The `.`-prefixed forms are Modelica's element-wise
/// arithmetic operators.
const TWO_BYTE_OPERATORS: &[&[u8; 2]] = &[
    b":=", b"<=", b">=", b"==", b"<>", b".+", b".-", b".*", b"./", b".^",
];

/// Scan `source` into tokens covering every byte exactly once.
///
/// Never panics and never returns an error — see the module docs for why both
/// matter. Unterminated strings and block comments extend to end-of-input.
pub fn tokenize(source: &str) -> Vec<Token> {
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let mut i = 0usize;

    // Byte-wise scanning is UTF-8 safe here: every delimiter this loop looks
    // for is ASCII, and no continuation byte of a multi-byte character can
    // equal an ASCII value. So a `"` inside a comment containing `é` is still
    // found correctly, and no index ever lands mid-character.
    while i < bytes.len() {
        let start = i;
        let kind = match bytes[i] {
            b if is_whitespace(b) => {
                while i < bytes.len() && is_whitespace(bytes[i]) {
                    i += 1;
                }
                TokenKind::Whitespace
            }
            b'/' if bytes.get(i + 1) == Some(&b'/') => {
                i += 2;
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
                TokenKind::Comment
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                i += 2;
                // Modelica block comments do not nest, so the first `*/` ends
                // it. Unterminated runs to end-of-input rather than failing.
                while i < bytes.len() {
                    if bytes[i] == b'*' && bytes.get(i + 1) == Some(&b'/') {
                        i += 2;
                        break;
                    }
                    i += 1;
                }
                TokenKind::Comment
            }
            b'"' => {
                i += 1;
                i = scan_delimited(bytes, i, b'"');
                TokenKind::String
            }
            b'\'' => {
                // A *quoted identifier* (`'foo bar'`), not a string. Modelica
                // allows any character inside, so this is the only way to write
                // a name containing a space, an operator, or a keyword. Lexing
                // it as a string would make such names uncolourable as names
                // and unclickable in the source view.
                i += 1;
                i = scan_delimited(bytes, i, b'\'');
                TokenKind::Identifier
            }
            b if b.is_ascii_digit() => {
                i = scan_number(bytes, i);
                TokenKind::Number
            }
            b if is_ident_start(b) => {
                while i < bytes.len() && is_ident_continue(bytes[i]) {
                    i += 1;
                }
                classify_word(&source[start..i])
            }
            _ => {
                let two = bytes.get(i..i + 2);
                if two.is_some_and(|t| TWO_BYTE_OPERATORS.iter().any(|op| op.as_slice() == t)) {
                    // Both bytes are ASCII, so this cannot start mid-character.
                    i += 2;
                } else {
                    // A whole character, not a byte. Every arm above matches
                    // ASCII only, so any non-ASCII character — an em dash in a
                    // description string, an accented letter in a comment —
                    // arrives here, and stepping one byte would put a token
                    // boundary *inside* it. Callers slice
                    // `source[tok.start..tok.end]`, which panics on such a
                    // boundary; that is exactly how HRW crashed when a followed
                    // identifier was searched for in a stage note containing an
                    // em dash.
                    i += utf8_char_len(bytes[i]);
                }
                TokenKind::Operator
            }
        };
        tokens.push(Token {
            kind,
            start,
            end: i,
        });
    }

    tokens
}

/// Consume bytes up to and including the closing `delim`, honouring `\` escapes.
/// Returns the index just past the closing delimiter, or end-of-input if there
/// is none.
fn scan_delimited(bytes: &[u8], mut i: usize, delim: u8) -> usize {
    while i < bytes.len() {
        match bytes[i] {
            b'\\' if i + 1 < bytes.len() => i += 2,
            b if b == delim => return i + 1,
            _ => i += 1,
        }
    }
    i
}

/// Consume a numeric literal: digits, an optional fractional part, and an
/// optional exponent (`1`, `1.5`, `1.5e-3`, `2E+10`).
///
/// The exponent sign is only consumed when a digit follows, so `1e` lexes as a
/// number followed by an identifier rather than swallowing the next token.
fn scan_number(bytes: &[u8], mut i: usize) -> usize {
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if bytes.get(i) == Some(&b'.') && bytes.get(i + 1).is_some_and(u8::is_ascii_digit) {
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
    } else if bytes.get(i) == Some(&b'.') && !bytes.get(i + 1).is_some_and(|&b| is_ident_start(b)) {
        // Trailing dot with no fraction: `1.` is a valid Modelica literal, but
        // `1.foo` is a component reference, so only consume the dot when what
        // follows cannot start a name.
        i += 1;
    }
    if matches!(bytes.get(i), Some(b'e' | b'E')) {
        let after_sign = if matches!(bytes.get(i + 1), Some(b'+' | b'-')) {
            i + 2
        } else {
            i + 1
        };
        if bytes.get(after_sign).is_some_and(u8::is_ascii_digit) {
            i = after_sign;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
        }
    }
    i
}

fn classify_word(word: &str) -> TokenKind {
    if KEYWORDS.contains(&word) {
        TokenKind::Keyword
    } else if TYPES.contains(&word) {
        TokenKind::Type
    } else {
        TokenKind::Identifier
    }
}

fn is_whitespace(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\r')
}

/// Byte length of the UTF-8 character whose leading byte is `lead`.
///
/// The input to `tokenize` is a `&str`, so it is valid UTF-8 and the scanner
/// only ever consults this at a character boundary. A continuation or invalid
/// byte therefore cannot reach here; 1 is returned for those so the scanner
/// still terminates and still tiles the input rather than looping or skipping.
fn utf8_char_len(lead: u8) -> usize {
    match lead {
        0x00..=0x7F => 1,
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        0xF0..=0xF7 => 4,
        _ => 1,
    }
}

fn is_ident_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_'
}

fn is_ident_continue(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Collect (kind, text) pairs, dropping whitespace — whitespace is asserted
    /// separately by the tiling test, and including it here would bury the
    /// interesting assertions.
    fn kinds(source: &str) -> Vec<(TokenKind, &str)> {
        tokenize(source)
            .into_iter()
            .filter(|t| t.kind != TokenKind::Whitespace)
            .map(|t| (t.kind, t.text(source)))
            .collect()
    }

    #[test]
    fn classifies_keywords_types_and_identifiers() {
        use TokenKind::*;
        assert_eq!(
            kinds("model M Real x; end M;"),
            vec![
                (Keyword, "model"),
                (Identifier, "M"),
                (Type, "Real"),
                (Identifier, "x"),
                (Operator, ";"),
                (Keyword, "end"),
                (Identifier, "M"),
                (Operator, ";"),
            ]
        );
    }

    #[test]
    fn line_comment_runs_to_newline_only() {
        use TokenKind::*;
        assert_eq!(
            kinds("x // note\ny"),
            vec![(Identifier, "x"), (Comment, "// note"), (Identifier, "y")]
        );
    }

    /// The reason `tokenize` takes the whole file: a per-line scanner cannot
    /// know that line 3's `*/` closes a comment opened on line 1.
    #[test]
    fn block_comment_spans_lines() {
        use TokenKind::*;
        let src = "a /* one\ntwo\nthree */ b";
        assert_eq!(
            kinds(src),
            vec![
                (Identifier, "a"),
                (Comment, "/* one\ntwo\nthree */"),
                (Identifier, "b")
            ]
        );
    }

    /// A quoted identifier is a *name*, not a string — it may contain spaces,
    /// operators, and words that would otherwise be keywords.
    #[test]
    fn quoted_identifier_is_a_name_not_a_string() {
        use TokenKind::*;
        assert_eq!(
            kinds("Real 'end of travel' = 1;"),
            vec![
                (Type, "Real"),
                (Identifier, "'end of travel'"),
                (Operator, "="),
                (Number, "1"),
                (Operator, ";"),
            ]
        );
    }

    #[test]
    fn string_literal_keeps_escaped_quotes() {
        use TokenKind::*;
        assert_eq!(
            kinds(r#"x = "a \" b";"#),
            vec![
                (Identifier, "x"),
                (Operator, "="),
                (String, r#""a \" b""#),
                (Operator, ";"),
            ]
        );
    }

    #[test]
    fn numeric_literals_with_exponents() {
        use TokenKind::*;
        assert_eq!(
            kinds("1 1.5 1.5e-3 2E+10 7."),
            vec![
                (Number, "1"),
                (Number, "1.5"),
                (Number, "1.5e-3"),
                (Number, "2E+10"),
                (Number, "7."),
            ]
        );
    }

    /// `1.foo` is a component reference, not a malformed literal, so the dot
    /// must not be swallowed into the number.
    #[test]
    fn number_does_not_swallow_a_following_component_reference() {
        use TokenKind::*;
        assert_eq!(
            kinds("a1.foo"),
            vec![(Identifier, "a1"), (Operator, "."), (Identifier, "foo")]
        );
    }

    /// `1e` is not an exponent — consuming the `e` would swallow the next token.
    #[test]
    fn bare_e_is_not_an_exponent() {
        use TokenKind::*;
        assert_eq!(kinds("1e"), vec![(Number, "1"), (Identifier, "e")]);
    }

    #[test]
    fn two_byte_operators_stay_together() {
        use TokenKind::*;
        assert_eq!(
            kinds("a := b .* c <= d"),
            vec![
                (Identifier, "a"),
                (Operator, ":="),
                (Identifier, "b"),
                (Operator, ".*"),
                (Identifier, "c"),
                (Operator, "<="),
                (Identifier, "d"),
            ]
        );
    }

    /// Unterminated constructs must not panic or drop text — the source view
    /// renders whatever is on disk, including half-written code.
    #[test]
    fn unterminated_constructs_run_to_end_of_input() {
        for src in [r#"x = "no close"#, "x /* no close", "x = 'no close"] {
            let toks = tokenize(src);
            let last = toks.last().expect("at least one token");
            assert_eq!(
                last.end,
                src.len(),
                "unterminated tail must reach EOF: {src:?}"
            );
            assert_tiles(src, &toks);
        }
    }

    #[test]
    fn non_ascii_inside_comments_and_strings_is_safe() {
        let src = "// °C and é\nx = \"naïve\";";
        assert_tiles(src, &tokenize(src));
    }

    /// Bare non-ASCII — not inside a comment or a string — reaches the
    /// catch-all arm, which used to advance **one byte**, putting token
    /// boundaries inside a character. This is not hypothetical Modelica: the
    /// lexer also runs over IR strings when deciding whether text mentions a
    /// followed identifier, and Rumoca's own structural note reads
    /// "…before it can be solved — see the Index Reduction tab." Slicing that
    /// em dash crashed HRW on a click.
    #[test]
    fn bare_non_ascii_lexes_on_character_boundaries() {
        for src in [
            "x — y",
            "requires index reduction — see the Index Reduction tab.",
            "α + β",
            "🎯",
        ] {
            let toks = tokenize(src);
            assert_tiles(src, &toks);
            // One token per character, not per byte.
            assert_eq!(
                toks.iter().filter(|t| t.text(src) == "—").count(),
                src.matches('—').count(),
                "each em dash must be exactly one token in {src:?}",
            );
        }
    }

    #[test]
    fn empty_source_produces_no_tokens() {
        assert!(tokenize("").is_empty());
    }

    /// Every byte accounted for, exactly once, in order. This is the property
    /// rendering depends on: run the tokens and you have emitted the file.
    fn assert_tiles(source: &str, tokens: &[Token]) {
        let mut expected_start = 0usize;
        for t in tokens {
            assert_eq!(
                t.start, expected_start,
                "gap or overlap before {t:?} in {source:?}"
            );
            assert!(t.end > t.start, "empty token {t:?} in {source:?}");
            // Checked explicitly, not left to `t.text()` panicking below: a
            // boundary inside a character is the failure that reached the app,
            // and it deserves a message that names itself.
            assert!(
                source.is_char_boundary(t.start) && source.is_char_boundary(t.end),
                "token boundary inside a character: {t:?} in {source:?}",
            );
            expected_start = t.end;
        }
        assert_eq!(
            expected_start,
            source.len(),
            "tokens end short of input in {source:?}"
        );
        let rebuilt: std::string::String = tokens.iter().map(|t| t.text(source)).collect();
        assert_eq!(
            rebuilt, source,
            "concatenated tokens must reproduce the source"
        );
    }

    /// The real corpus: every specimen must tokenize without panicking, and the
    /// tokens must tile it exactly.
    #[test]
    fn all_specimens_tokenize_and_tile_exactly() {
        let dir = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/specimens"));
        let mut checked = 0;
        for entry in std::fs::read_dir(dir).expect("specimens dir") {
            let path = entry.expect("dir entry").path();
            if path.extension().and_then(|e| e.to_str()) != Some("mo") {
                continue;
            }
            let src = std::fs::read_to_string(&path).expect("read specimen");
            assert_tiles(&src, &tokenize(&src));
            checked += 1;
        }
        assert!(
            checked >= 10,
            "expected the specimen corpus, found {checked} files"
        );
    }
}
