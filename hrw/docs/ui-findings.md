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

## Code findings — things about HRW itself

**Disposition** is the column that matters: an entry with none is unfinished business.

| # | Finding | Disposition |
|---|---|---|
| C1 | **`equation_sheet_ui`'s `"(no equation sheet)"` branch is unreachable.** One call site, gated on `flatten_ready`, which is itself `cached_equation_sheet.is_some()`. | **Accepted.** Defensive, not wrong. Recorded in `src/ui_tests.rs` beside the test that would have asserted it — a test on that string would pass forever regardless of what the pane does. |
| C2 | **The Parse stage of a multi-class library file is ~4–5 MB of JSON.** `Blocks/Continuous.mo` is the measured case. | **Accepted by Doug, 2026-08-01** — `DECISIONS.md`, Parse's meaning is not a performance knob. Cost falls on the UI and on the fidelity sweep. |
| C3 | **20 `cached_*` fields, 40 invalidation sites, no completeness guard.** A missed invalidation renders stale plausible data. | **Scheduled** — `ui-pause-plan.md` step 2 removes the bug class rather than testing for it. |
| C4 | **105 fields on `App`; 91 of them touched ten times or fewer.** Only `stage`, `stages`, `tracked_identifier`, `selected` are genuinely shared. | **Scheduled** — the field-count ratchet in `ui-pause-plan.md`. |
| C5 | **`bridge::slice_source` tries `file_name` as a *relative* path.** Run from a directory holding a same-named file, it would slice the wrong one and emit a confident wrong excerpt. | **Mitigated 2026-08-01** by passing the full document URI, so the branch is no longer reached with a bare basename. **The relative test itself still stands** — open. |
| C6 | **The animations are testable and have no tests.** H7 corrects the earlier assumption that they were out of reach. | **Open** — not in the pause's four steps. Belongs in `tech-debt.md`'s UI testing debt. |

---

## How an entry leaves this file

An entry graduates when it acquires an owner elsewhere: `tech-debt.md` if it is work, `ideas.md`
if it is a feature, `DECISIONS.md` if it was decided. **It is not deleted here** — the
disposition column is updated to say where it went. A findings log that empties out loses the
record of what was looked at and consciously left alone, which is the half a later session most
needs.
