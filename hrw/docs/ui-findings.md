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
| H10 | **`Node::rect()` returns a widget's `egui::Rect` directly.** Geometry does not have to be recorded by the app to be checked. H6 still holds for values that are *not* widget rects — a `ScrollArea`'s offset is app state and has no node — but widget position and size were reachable all along. | Layout believed untestable; two days of defects left to Doug that a test could have held. |
| H11 | **`eprintln!` is swallowed in these tests; use `println!`.** Measured 2026-08-02: a three-iteration probe printed only iterations 1 and 2 via `eprintln!`, and all three via `println!`. Same family as the fd-level `OutputCapture` bug in the must-fire rule. | **A measurement silently goes missing**, and the gap looks like the code not running. |
| H12 | **"Every widget inside the viewport" is not a usable invariant.** At a healthy 1600×1200, **153 of 232** widgets lie outside it — scroll-area content extends past the clip rect by design. Shrinking to 800×700 moves that to 178, so the *signal* is ~25 widgets against a floor of 153. A layout guard must name the chrome that must **always** be visible (menu bar, tab row, status bar, mode buttons), not sweep every node. | A blanket assertion fails constantly on healthy layouts, and gets deleted rather than fixed. |
| H9 | **A test for an empty state must be given a genuinely empty one.** The Purpose tab was handed `RcCircuit`, which *has* a real `purpose.md`, so the pane correctly rendered the note and the test failed with nothing wrong. Real specimen names are live data. | Wrong diagnosis pointed at the pane instead of the fixture. |

## Code findings — things about HRW itself

**Disposition** is the column that matters: an entry with none is unfinished business.

| # | Finding | Disposition |
|---|---|---|
| C1 | **`equation_sheet_ui`'s `"(no equation sheet)"` branch is unreachable.** One call site, gated on `flatten_ready`, which is itself `cached_equation_sheet.is_some()`. | **Accepted.** Defensive, not wrong. Recorded in `src/ui_tests.rs` beside the test that would have asserted it — a test on that string would pass forever regardless of what the pane does. |
| C2 | **The Parse stage of a multi-class library file is ~4–5 MB of JSON.** `Blocks/Continuous.mo` is the measured case. | **Accepted by Doug, 2026-08-01** — `DECISIONS.md`, Parse's meaning is not a performance knob. Cost falls on the UI and on the fidelity sweep. |
| C3 | **20 `cached_*` fields, 40 invalidation sites, no completeness guard.** A missed invalidation renders stale plausible data. | **Scheduled** — `ui-pause-plan.md` step 2 removes the bug class rather than testing for it. |
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
