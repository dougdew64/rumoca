# Source Tooling Plan — lexer, highlighting, identifier tracking, trees

One thread of work covering backlog items that turn out to share a single
foundation: **#36** (Modelica syntax highlighting), **#37** (reverse identifier
tracking), forward identifier debugging, the remaining reach of **#10**
(cross-stage identifier tracking), and — added once the earlier phases exposed
how weak the tree is as an instrument — **#11** (in-view search) as Phase 6.

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
- Identifiers that lex as names but have no DAE variable render normally and are
  not clickable. **The proposed "did not survive flattening" hover was dropped**:
  most such identifiers were never variables at all — class names, function
  names, modifier names like `start`, enumeration literals — and the index
  cannot distinguish those from a genuinely dropped variable. The hover would
  have been confidently wrong on the majority of what it labelled. Surfacing
  dropped variables is a real idea, but it needs a source of truth about what
  *was* a variable, which is a different piece of work.
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

## Phase 5 — The Context Bar (and forward identifier debugging)

**Goal.** Make the thin-emitter / thick-reasoner split visible: tracking emits a
compound capture, and a Context Bar renders what Claude will actually receive.
Breakpoint selection then follows from that context rather than from a table in
the app.

**Status: design settled 2026-07-27 — see [`context-assembly.md`](context-assembly.md).**

The design question below ("which phase function is the meaningful site?") is
**dissolved**, not answered. Doug's proposal — give tracking a second meaning, so
that as well as highlighting it performs a *compound capture* of everywhere it
highlighted — means HRW emits where the identifier lives at each stage and Claude
chooses the breakpoint sites per case, with the whole transformation in view.
Encoding a taxonomy in the app is exactly what the thin-emitter principle exists
to avoid. The original framing is kept below because the four candidate answers
are still the right things to reason *about* — they just belong to the reasoner.

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

### Deliverables

1. **Tracking emits a compound capture.** Every place tracking highlighted,
   across every stage, plus absences. `focus` (deliberate point) and `tracking`
   (ambient thread) as distinct sections of one file, so ambient browsing never
   destroys the context you meant to ask about.
2. **The Context Bar**, replacing the Tracking bar — a rendering of what will be
   emitted, including the standing context (stage IRs, DefId table, libraries)
   that the current UI never mentions at all.
3. **Rename the user-facing verbs** — see below.
4. **Retire the bridge's use of the status bar** — see below.

### Rename: "Capture" → "Point at", "Track" → "Follow"

Doug, 2026-07-27, on testing Phase 4. The bridge's own architecture note says the
loop is **point → ask → understand**, yet the menu says "Capture" — a word for
what the *app* does (writes a file) rather than what the *user* does. "Follow"
beats "Track" for being directional: tracking could mean recording or
monitoring; following implies the thing goes somewhere and you are going with
it. The pair also carries the point/thread distinction without anyone having to
explain it, and makes the menu and the Context Bar share one vocabulary.

Scope, deliberately partial:

- **Rename:** context-menu labels, the Context Bar's own wording, and the docs.
- **Leave:** the wire format and internal identifiers — `focus.json`,
  `emit_node_focus`, `AskRequest`, `Focus`. Renaming a protocol Claude already
  reads buys nothing and breaks continuity with recorded sessions.
- **Do update:** the self-describing `instructions` string inside `focus.json`
  (`bridge.rs`), so a user reading the file while dogfooding does not meet a
  third vocabulary — and add a one-line note in `bridge.rs` mapping the UI verbs
  to the code's nouns, so the two do not drift silently.

### Retire the bridge's use of the status bar

The Context Bar strictly dominates the *success* case: a persistent statement of
what is pointed at beats a transient "captured #3" that disappears.

**But `bridge_status` also reports failures, and those must not vanish with it.**
`bridge::write` returns `io::Result`. If the write fails — missing
`.hrw-bridge/`, bad permissions — a bar that cheerfully reads "Pointing at
`components.src.V`" would be claiming context Claude does not have; Claude would
still be holding the *previous* focus. That is precisely the lie the bar's
governing rule exists to prevent, and worse than today's behaviour because it is
confident.

So failure moves *into* the bar rather than staying on a second surface:

```
Context   MotorWithBrake · captured in Flatten
  Pointing at   components.src.V     ⚠ not emitted — permission denied
```

The bar's contract is "this is what Claude has"; saying so when emission fails
*is* that contract being honoured.

**Sequencing:** the removal happens only once the bar exists. Doing it first
would leave a window with no capture feedback at all.

---

## Phase 6 — Make the tree a real instrument (#11)

**Goal.** The IR tree is HRW's primary exploration widget and every stage tab
depends on it, but it is still a raw JSON dump with two ad-hoc affordances
bolted on. This phase reworks it deliberately rather than by accretion.

**Status: design first. Nothing here should be built before the questions below
are settled** — that is the lesson of the "Reveal identifiers" checkbox, which
was added quickly, then needed its trackability definition corrected, then its
expansion mechanism corrected, and still does not feel right.

### What prompted it

Doug, 2026-07-28: *"Little things don't feel right. For example, why is that
widget a checkbox instead of a button?"*

That question turns out to be the whole diagnosis. **A checkbox is a mode; a
button is an action.** "Reveal" was built as a mode, which forces every matching
node open for as long as it is ticked — and therefore takes the tree out of the
user's hands at exactly the moment they want to explore what was revealed.
Untick to regain control and the revealing is lost. If instead "reveal" is an
*action* that opens those paths once and then steps aside, the conflict
disappears, and so does the need for `force_open` in `Expansion`.

Three more things that are wrong in the same way:

- **The name over-claims.** "Reveal identifiers" reveals *variables of the
  compiled model*. With Phase 5's vocabulary it is really "reveal what you can
  follow". "Identifiers" is the loosest available word and the least true.
- **The count and the reveal are different magnitudes.** "(38 in this model)"
  counts distinct variables; the expansion opens a path to every *mention*,
  which is far more. The label sets an expectation the behaviour overshoots.
- **Expanding may be the wrong verb entirely.** Filtering *down* to matches
  shows a short list; expanding *open* shows a large tree held open. For "where
  is `h`?" the first is better, and it is what #11 describes.

### Candidate scope

Not commitments — the point of a design-first phase is to choose among these.

1. **In-view search (#11).** A find-as-you-type filter over the current tree.
   Filtering to matches rather than expanding to them, which subsumes "reveal"
   as one predicate among several.
2. **Reveal as an action.** A button that expands paths once and lets go.
3. **Noise suppression.** Real IR is dominated by provenance: every `name` in
   the Resolve tree carries a `location` with line/column/offsets/file plus
   `token_number` and `token_type` — roughly ten lines of scaffolding per
   identifier. A toggle that hides provenance would change readability more
   than anything else in this list.
4. **"What changed" filter.** The cross-stage diff already drives the green
   highlight; collapsing the tree to *only* changed nodes would answer "what did
   this phase actually do?" in one gesture. That is the core curriculum
   question, and the data for it already exists.
5. **Expansion controls.** Collapse-all, expand-one-level — the ordinary
   affordances of a tree widget, currently absent.

### Questions to settle first

- **Filter or expand?** They are different interactions and probably want
  different widgets. Deciding this first determines most of the rest.
- **What is the unit of a match** — a node, a row, a path? A filtered tree still
  has to show ancestors for context, or matches lose their meaning.
- **Does noise suppression belong to the tree, or to the emitted context too?**
  If provenance is hidden from the user, is it still worth sending to Claude?
  (Probably yes — span-ascent depends on it — but the answer should be
  deliberate.)
- **Does any of this change what is emitted?** If a filter narrows what the user
  is looking at, the Context Bar's rule says the emitted context should say so.
  Phase 5 and Phase 6 meet here.

## Sequencing

1 → 2 → 3 → 4 → 5 → 6. Each phase ships something usable on its own, and each
one's output is the next one's input. Phases 1–2 are self-contained and
low-risk; Phase 3 improves behaviour already in daily use; Phases 4–5 add
capability; Phase 6 reworks the widget all of them are rendered in.

Phases 1 and 2 can land as a single commit pair (crate-free — all HRW). Phase 3
touches existing tested behaviour and should be its own commit.

Phase 5 is no longer blocked on a design question — that was settled on
2026-07-27 by dissolving it (see [`context-assembly.md`](context-assembly.md)).
It is now the largest phase and the one that matters most to the project's
premise: HRW is not a standalone tool, and the Context Bar is where that stops
being an idea in a charter and becomes something visible on screen every time
you use it.

## Progress

- [x] **Phase 1 — Modelica lexer** ✅ 2026-07-27. `hrw/src/modelica_lex.rs`,
      13 tests. Two decisions worth knowing before Phase 2: quoted identifiers
      lex as `Identifier`, not `String` (so `'end of travel'` stays a clickable
      name), and tokens tile the input exactly including whitespace, so
      rendering can walk the list and emit every byte once.
- [x] **Phase 2 — Syntax highlighting (#36)** ✅ 2026-07-27.
      `hrw/src/source_view.rs` (line clipping + segment merge, 8 tests),
      `colors::syntax_color`, and the render loop in `app.rs`. Identifier
      linking preserved: colour and click targets now come from **one** pass, so
      they cannot disagree about where a run begins. Clickable and tracked
      colours deliberately outrank syntax colour. Note this already did part of
      Phase 3's job — `segments` is the merge point — so Phase 3 is now only
      about *which* identifiers `clickable_spans` offers, not about rendering.
- [x] **Phase 3 — Clickable identifiers re-based on the lexer** ✅ 2026-07-27.
      `clickable_spans` now takes the line's tokens and scans them instead of
      searching the text. Fixed three things, not the two planned: every
      occurrence is clickable; identifiers in **comments and strings** no longer
      are (a text search cannot tell code from commentary — this one was not
      anticipated); and same-leaf names on one line (`a.phi` vs `b.phi`) link to
      the right variable via longest-dotted-path matching. Dropped the plan's
      "hover explaining the name did not survive flattening" idea — see below.
- [~] **Phase 4 — Reverse identifier tracking (#37)** — 2026-07-27: scroll-to-line
      in the source view (armed on *change* only, via `scrolled_source_for`),
      `strip_der`, a single `set_tracked_identifier` entry point that toggles and
      reveals the source, and two entry points — the equation sheet's
      variable-classification grid, and "Track" in the IR tree's row menu. The
      tree is on every stage tab, so the gesture is now near-ambient rather than
      per-view. Tracking never answers with silence: the tracking bar states the
      declaring line, or says the name is not declared in this specimen.
      **Remaining:** incidence column headers, spy-plot blocks, reduction-view
      rows — all canvas-painted, so they need hit-testing rather than a widget
      response.
- [ ] **Phase 5 — The Context Bar** — four deliverables: tracking emits a compound
      capture; the Context Bar renders what will be emitted (including the standing
      context the UI never mentions today); "Capture"/"Track" become "Point
      at"/"Follow" in the UI and docs; and the status bar loses its bridge role,
      with emission *failures* moving into the bar. Design settled — see
      [`context-assembly.md`](context-assembly.md).
- [ ] **Phase 6 — Make the tree a real instrument (#11)** — design first. In-view
      search, "reveal" as an action rather than a mode, provenance noise
      suppression, a "what changed" filter over the existing diff data, and the
      ordinary expansion controls the tree still lacks.