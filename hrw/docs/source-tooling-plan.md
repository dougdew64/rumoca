# Source Tooling Plan — lexer, highlighting, and identifier tracking

One thread of work covering four backlog items that turn out to share a single
foundation: **#36** (Modelica syntax highlighting), **#37** (reverse identifier
tracking), forward identifier debugging, and the remaining reach of **#10**
(cross-stage identifier tracking).

Created 2026-07-27.

## Why these are one piece of work

Each of them needs to answer the same question — *given a position in Modelica
source text, what is there?* — and today nothing in HRW can answer it. Building
that answer once, properly, is cheaper than four partial answers, and the
foundation is itself worth understanding: lexing is compiler front-end material,
directly comparable to how Rumoca's parse phase tokenizes via `parol` / `scnr2`.

## Correction to the recorded assumption

`docs/ideas.md` #36 says the tokenizer "built for clickable identifiers can be
extended to carry color information". **That tokenizer does not exist.**

`identifier_index.rs::clickable_spans` is *index-driven*, not lexical: it takes
the DAE variables believed to live on a line and searches the line for each
one's leaf name (`find_whole_identifier`). That was the right call when
identifier clicking was built — it avoided writing a lexer — but it means the
dependency runs the opposite way from what #36 assumed. The lexer comes first
and *improves* identifier clicking rather than falling out of it.

Two limitations of the current approach that Phase 3 removes:

- Only the **first** occurrence of a name on a line becomes clickable
  (`find_whole_identifier` returns the first match; `seen_positions` dedupes by
  position).
- Only identifiers that **reached the DAE** are found at all. Anything dropped
  during flattening is invisible in the source view, with no indication that it
  was ever a name.

---

## Phase 1 — The Modelica lexer

**Goal.** Answer "what token is at this byte offset?" for any specimen.

**Deliverable.** New `hrw/src/modelica_lex.rs`:

```rust
pub enum TokenKind {
    Keyword, Type, Identifier, Number, String, Comment, Operator, Whitespace,
}
pub struct Token { pub kind: TokenKind, pub start: usize, pub end: usize }
pub fn tokenize(source: &str) -> Vec<Token>;
```

**Design notes.**

- **Lex the whole file, not line by line.** Block comments `/* */` span lines, so
  a per-line lexer cannot classify correctly without carrying state. Whole-file
  plus a line map is simpler and caches naturally per specimen.
- Modelica specifics to handle: line comments `//`, block comments, **quoted
  identifiers** `'foo bar'` (a Modelica feature — these are identifiers, not
  strings), string literals with escapes, numeric literals with exponents
  (`1.5e-3`), and the `der`/`initial`/`when`/`end` keyword set.
- No parser. Lexical classification only — this must never fail on input, only
  classify it. Unknown bytes become `Operator`.
- Keep it free of `egui` so it stays testable and reusable.

**Done when.** A table-driven test over source snippets asserts kind and span
for each token, including a block comment spanning three lines, a quoted
identifier containing a space and a keyword, and a numeric literal with an
exponent. Every specimen in `specimens/` tokenizes without panicking and with no
byte unaccounted for (the token ranges tile the input exactly).

---

## Phase 2 — Syntax highlighting (#36)

**Goal.** The specimen source view reads like code.

**Deliverable.** Source rendered through `egui::LayoutJob` with per-token
colour; palette added to `colors.rs`, theme-aware for light and dark.

**Design notes.**

- Replaces the current per-line `RichText` / `ui.horizontal` split in
  `app.rs` (~2113–2140).
- Cache the token list per specimen; do not re-tokenize per frame.
- Colour choices should stay legible against the existing HRW palette rather
  than importing an editor theme wholesale. Comments recede; keywords and types
  lead.

**Done when.** Every specimen renders with correct colouring, light and dark
mode both check out, and scrolling a large specimen (`BenchActuator`) shows no
frame-rate regression.

---

## Phase 3 — Re-base clickable identifiers on the lexer

**Goal.** Every identifier occurrence is clickable, and colour and click targets
come from one pass instead of two mechanisms competing over the same line.

**Deliverable.** `clickable_spans` reimplemented: lex, take `Identifier` tokens,
ask `IdentifierIndex` which resolve to DAE variables.

**Design notes.**

- Fixes both limitations listed above.
- Decide what to do with identifiers that lex as names but have no DAE variable
  — most usefully, render them normally but not clickable, and consider a hover
  explaining that the name did not survive flattening. That absence is itself
  worth seeing.
- The existing `matches_tracked` whole-identifier logic stays; it is used by
  other views and is orthogonal.

**Done when.** A variable appearing twice on one line is clickable in both
places; existing identifier-tracking tests still pass; the tests that currently
pin `clickable_spans` behaviour are extended rather than replaced.

---

## Phase 4 — Reverse identifier tracking (#37)

**Goal.** Click a downstream mention, land on the source that produced it.

**Deliverable.** Click-to-track in the incidence view, spy plot, equation sheet,
and IR tree; the source view scrolls to and highlights the corresponding line.

**Design notes.**

- Downstream views already know which identifier each row, cell, or node
  represents, so most of this is wiring clicks to `tracked_identifier`.
- `app.rs` already computes a `tracked_line` (~1457) — check what it covers
  before adding anything new.
- The genuinely new piece is **scroll-to-line** in the source `ScrollArea`
  (`scroll_to_rect` / `scroll_to_cursor` against the tracked line's rect).
- Watch for feedback loops: setting `tracked_identifier` from a downstream click
  must not re-trigger a scroll every frame, only on change.

**Done when.** Clicking an incidence row scrolls the source view to the
declaring line and highlights it; clicking the same thing twice does not fight
the scrollbar.

---

## Phase 5 — Forward identifier debugging

**Goal.** From a tracked identifier, break in the Rumoca phase that determines it.

**Status: needs a design decision before implementation.**

The plumbing is already done. The bridge protocol carries arbitrary
`{ path, line, condition }` entries, the VS Code extension applies them
generically, and `AskRequest::DebugWhereSet` already exists. Nothing in the
transport needs work.

The open question is **semantic**: for a given identifier, which phase function
is the meaningful place to stop? Candidates — where the resolver assigns its
`def_id`, where flattening emits the flat variable, where structural analysis
matches its row, where index reduction demotes it. These are different questions
about the same name, and "debug this identifier" has to pick, or ask.

`docs/debug-set-sites.md` is the reference for which sites are already
considered meaningful. **Settle this in discussion before writing code** — it is
a curriculum question about what is worth stopping to look at, not a plumbing
problem.

---

## Sequencing

1 → 2 → 3 → 4 → 5. Each phase ships something usable on its own, and each one's
output is the next one's input. Phases 1–2 are self-contained and low-risk;
Phase 3 improves behaviour already in daily use; Phases 4–5 add capability.

Phases 1 and 2 can land as a single commit pair (crate-free — all HRW). Phase 3
touches existing tested behaviour and should be its own commit. Phase 5 may not
touch code at all until its design question is answered.

## Progress

- [ ] Phase 1 — Modelica lexer
- [ ] Phase 2 — Syntax highlighting (#36)
- [ ] Phase 3 — Clickable identifiers re-based on the lexer
- [ ] Phase 4 — Reverse identifier tracking (#37)
- [ ] Phase 5 — Forward identifier debugging *(design first)*
