# UI findings log

**Purpose:** the incidental discoveries made while testing and refactoring the UI — things
that were true but unrecorded, and would otherwise survive only in a commit message.
**Status:** running log, appended as findings appear. Not authority for anything; each entry
points at where the fact actually lives.
**Read when:** writing a UI test (the harness facts change how you write one), or looking for
work the pause turned up but did not do.

**Append a finding the moment it is found, not at the end of a chunk.** The ones worth having
are exactly the ones that feel too small to write down — a commit message records them where
nobody browses, and by the next session they are gone.

**One line per finding, pointing outward.** Facts live in the code, `tech-debt.md` or
`ideas.md`; this file is an index, not a second copy. A transcribed fact is a fact that will
disagree with its original.

---

## Harness facts — these change how a test is written

Discovered by writing tests that failed for reasons unrelated to the code under test. Costly
to rediscover, so they are also summarised in `src/ui_tests.rs`'s module documentation, which
is where someone writing a test will actually be looking.

| # | Fact | Cost of not knowing |
|---|---|---|
| H1 | **`query_by_label_contains` panics on multiple matches.** `"Flatten"` legitimately appears twice — a log entry and a stage tab. Use `get_all_…().next()` when a second match is none of the test's business. | Reads as "the feature is broken" when the screen is fine. |
| H2 | **The central panel does not draw its body without a loaded specimen.** The log view, stage views and equation sheet are all unreachable until `selected` is set. | A test that expects the right-hand side to render finds nothing. |
| H3 | **A widget laid out off-screen is queryable but not clickable.** Hence the harness's 1600×1200, not egui's 800×600 default. | A synthetic click lands on nothing. |
| H4 | **HRW never goes quiescent**, so `Harness::run` exhausts its budget and panics. `run_steps` is the right tool. | Looks like a hang in the app. |
| H5 | **State injected into `App` can be undone before the first frame.** `poll_scratch_specimens` fires on frame one and `rescan()`s an injected specimen list back to empty. `test_set_specimen_files` parks the poll for this reason. | **Silent vacuity** — the test passes while checking nothing. |
| H6 | **Geometry is reachable when the app records it.** The accessibility tree carries none, but a widget's own state is a number: `ScrollArea::show` returns its offset, so `app.rs` stores it and a test reads it back. | Layout defects wrongly assumed untestable and left to Doug. |
| H7 | **Only two surfaces are genuinely unreachable** — `incidence_view.rs` cell glyphs and `spyplot.rs`. The animations *are* testable; their controls and state labels are ordinary widgets. | Fixture tours spent re-walking panes a test could hold. |
| H8 | **Syntax-highlighted source renders one label per token.** `"model Resistor"` never matches however plainly it reads in the file; query a single token, and pick one that appears **only** in the body or the assertion proves nothing. | Reads as "the source did not render" when it rendered perfectly. |
| H13 | **`get_all_by_label_contains` panics when *nothing* matches**, so it cannot express absence — and a probe that calls `.count()` on it dies instead of reporting zero. Use `query_by_label_contains(..).is_none()`. | A diagnostic probe fails in a way that looks like the code under test failing. |
| H14 | **A test written from memory of a string tests a version that may not exist.** `"nothing assembled"` was deliberately removed from the Context Bar on 2026-08-01, by me, the previous day; the test asserting its *absence* then passed for the wrong reason. **Read the string out of the source.** | A **vacuous assertion** that reads as a real guard — the stale-negative class, inside the test suite itself. |
| H10 | **`Node::rect()` returns a widget's `egui::Rect` directly.** Geometry does not have to be recorded by the app to be checked. H6 still holds for values that are *not* widget rects — a `ScrollArea`'s offset is app state and has no node — but widget position and size were reachable all along. | Layout believed untestable; two days of defects left to Doug that a test could have held. |
| H11 | ~~`eprintln!` is swallowed in these tests.~~ **WRONG — corrected 2026-08-02, same day.** Nothing is swallowed. `cargo test` writes `test <name> ... ` **without a newline**, so a test's *first* line of output is appended to it and a `grep "^MARKER"` misses exactly one measurement. Drop the `^` anchor. The original entry blamed the fd-level `OutputCapture`, which was a confident causal claim on one observation — the same error this log exists to catch. | **A measurement appears to go missing**, and the gap looks like the code not running. Then the wrong cause gets written down and believed. |
| H15 | **The tab row wraps, and the wrapped line falls off the *bottom*.** Forcing the split to 93 % put `Solve lowering` at y 552..614 in a 600-px window. The failure a narrow central panel produces is **vertical**, not horizontal — which is not what one would predict from a horizontal divider. | A layout guard that checked only the x axis would have passed on it. |
| H12 | **"Every widget inside the viewport" is not a usable invariant.** At a healthy 1600×1200, **153 of 232** widgets lie outside it — scroll-area content extends past the clip rect by design. Shrinking to 800×700 moves that to 178, so the *signal* is ~25 widgets against a floor of 153. A layout guard must name the chrome that must **always** be visible (menu bar, tab row, status bar, mode buttons), not sweep every node. | A blanket assertion fails constantly on healthy layouts, and gets deleted rather than fixed. |
| H9 | **A test for an empty state must be given a genuinely empty one.** The Purpose tab was handed `RcCircuit`, which *has* a real `purpose.md`, so the pane correctly rendered the note and the test failed with nothing wrong. Real specimen names are live data. | Wrong diagnosis pointed at the pane instead of the fixture. |

## Code findings — things about HRW itself

**Disposition** is the column that matters: an entry with none is unfinished business.

| # | Finding | Disposition |
|---|---|---|
| C1 | **`equation_sheet_ui`'s `"(no equation sheet)"` branch is unreachable.** One call site, gated on `flatten_ready`, which is itself `cached_equation_sheet.is_some()`. | **Accepted.** Defensive, not wrong. Recorded in `src/ui_tests.rs` beside the test that would have asserted it — a test on that string would pass forever regardless of what the pane does. |
| C2 | **The Parse stage of a multi-class library file is ~4–5 MB of JSON.** `Blocks/Continuous.mo` is the measured case. | **Accepted by Doug, 2026-08-01** — `DECISIONS.md`, Parse's meaning is not a performance knob. Cost falls on the UI and on the fidelity sweep. |
| C3 | **The 20 `cached_*` fields are three families, not one.** Eleven stage-view caches shared a lifetime and were listed **by hand in two places**; the rest are compile *results* or self-keying memos that must not be cleared together. | **Fixed 2026-08-02** — `StageViewCaches::reset_for` assigns a whole `Self`. Verified by adding a cache mentioned in no invalidation site and watching it clear. The other nine move with their panes in steps 3–4. |
| C7 | **A test re-implemented the invalidation it was testing.** `report_cache_invalidated_on_stage_switch` cleared five fields inline and asserted they were clear, so it verified its own copy — the real block could have been deleted with the test still green. | **Fixed 2026-08-02**, now calls `reset_for`. **The shape is worth remembering**: a test that reproduces the logic under test is indistinguishable from one that exercises it. |
| C10 | **A field rename mangles any method whose name it prefixes.** `self.structural_view` → `self.viewport.structural` also rewrote `self.structural_view_available(..)` into `self.viewport.structural_available(..)`, at **16 call sites**. | Caught by the compiler, but only because the result was a *missing method*. A rename whose prefix landed on another **field** would compile and silently read the wrong thing. **Prefer the longest unique form** (`self.name.` or `self.name =`) over the bare `self.name`. |
| C9 | **A field-by-field rename misses multi-line chains, and one of them compiled.** `self\n    .scratch_polled_at` survived a `self.scratch_polled_at` rename **because the old field still existed** — it was declared 200 lines away from the block being removed. Two fields then held the same state: the test helper wrote one, the guard read the other, so the scratch poll fired every frame and `rescan()` wiped the injected list. | **Caught by the baseline suite on its first real outing** (`a_filter_opens_both_sections`), which is exactly what it was built for. The lesson: after a field move, **grep for the bare field name**, not just `self.name` — a rename that compiles is not a rename that finished. |
| C8 | **Inserting a struct between a doc comment and its item reassigns the doc.** Putting `StageViewCaches` above `pub struct App` silently made `App`'s twelve-line doc comment document the new struct instead. Caught by clippy, not by tests. | **Fixed 2026-08-02.** Same family as the `#[test]` attribute theft in `CLAUDE.md`: **anything between a doc block and its item is adopted by the wrong one.** |
| C4 | **105 fields on `App`; 91 of them touched ten times or fewer.** Only `stage`, `stages`, `tracked_identifier`, `selected` are genuinely shared. | **Scheduled** — the field-count ratchet in `ui-pause-plan.md`. |
| C5 | **`bridge::slice_source` tries `file_name` as a *relative* path.** Run from a directory holding a same-named file, it would slice the wrong one and emit a confident wrong excerpt. | **Mitigated 2026-08-01** by passing the full document URI, so the branch is no longer reached with a bare basename. **The relative test itself still stands** — open. |
| C6 | **The animations are testable and have no tests.** H7 corrects the earlier assumption that they were out of reach. | **Logged** in `tech-debt.md` under the UI testing debt, 2026-08-02. Deliberately not scheduled: off the refactor's path. |
| C7 | **`source_map_ui`'s `"(no source mapping available)"` is reachable, but only by a persisting sub-view.** The SourceMap tab is offered only when `has_source_map`, yet `flatten_view` survives a specimen change — sit on SourceMap, load a model without one, and the message appears. Its sibling `"(no equation sheet)"` is unreachable, like C1. | **Open** — reaching it needs a populated `EquationSheet`, so it belongs with the `slow-tests`. Recorded beside the deferral in `src/ui_tests.rs`. |

---

## How an entry leaves this file

An entry graduates when it acquires an owner elsewhere: `tech-debt.md` if it is work, `ideas.md`
if it is a feature, `DECISIONS.md` if it was decided. **It is not deleted here** — the
disposition column is updated to say where it went. A findings log that empties out loses the
record of what was looked at and consciously left alone, which is the half a later session most
needs.
