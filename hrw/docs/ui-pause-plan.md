# The UI Pause — plan

**Purpose:** what gets tested, what gets refactored, in what order, and the evidence for each.
**Status:** authority for this pause. **Claude's plan**, per `DECISIONS.md` *"Claude is the
primary consumer of HRW's code"* — Doug set the goal and the ordering, Claude picks the work.
**Read when:** starting or resuming the UI pause (2026-08-02).

Doug's goal, verbatim: *"My primary testing goal for this project is to provide the
verification which you declared earlier that you need to be effective."* His ordering: **tests
before any refactoring**, so regressions have something to hit.

---

## The evidence

Measured 2026-08-01/02. **Refactor where there is evidence of friction, never where code
merely looks large** — so each target below cites what it actually cost.

| Target | Size | Evidence it costs something |
|---|---|---|
| **Cache layer** | 20 `cached_*` fields, **40 invalidation sites**, `clear_specimen_state` hand-lists 24 in 54 lines, **no completeness test** | A missed invalidation renders **stale plausible data** — silently wrong and confident. Same disease as the "wire every new stage" rule. |
| **Left panel** (tours + model list, inline in `frame_ui`) | ~450 lines | Edited **three times on 2026-08-01** (corpus list, filter, dividers). Two of those edits shipped defects Doug caught: the corpus hidden, then vacuous tests. |
| **`central_panel_ui`** | 771 lines | The stage tabs and every stage view in one function. |
| **`App`** | **105 fields**, 53 `&mut self` methods | Blast radius is unpredictable: threading one field through `Compiled` meant hunting every construction site by hand. |
| **`app.rs` as a file** | 9,562 lines | **Caused defects directly.** Too large for targeted edits, so Claude fell back on generated scripts with string anchors — which produced attribute theft twice in one day (silently disabling a regression guard), leaked Rust escapes into comment text, and one blind global replace Doug rightly rejected. |

**The last row is the strongest and the least obvious.** It is not an aesthetic complaint: the
file's size is already manufacturing bugs through the mechanics of editing it.

---

## The ordering, and why it is not "all tests, then all refactoring"

Doug's rule is tests first, and it stands. But writing fifty tests up front has a failure mode
proven twice on 2026-08-01: **a test written against current code encodes current behaviour,
defects included.** The corpus test asserted the very hiding Doug then reported as a bug.

So the pause runs in two tiers:

1. **A baseline suite across the whole UI surface**, written once, up front. Its job is to
   catch *anything* that changes anywhere during the refactor. Broad and shallow.
2. **Per-region pinning tests written immediately before that region is touched.** Narrow and
   deep, and written while the region's behaviour is freshly understood.

**Every test earns its place by being seen to fail against a deliberately broken version.**
Twice on 2026-08-01 a UI test passed while checking nothing. At a hundred tests, vacuous ones
are *worse* than none — they manufacture the confidence that licenses the refactor.

---

## Step 1 — The baseline suite

**Goal: no pane changes what it renders without a test noticing.** Not coverage for its own
sake; a net under the refactor.

Per pane, the same three questions, which is what makes this writable at speed:

- **Given state X, is the expected content on screen?** (the reporting-pane rule)
- **Given empty state, does it say so** rather than rendering blank? "Nothing here" and
  "broken" must not look alike.
- **Does clicking it change what it should, and nothing it should not?**

Panes with no headless test today, in the priority order from `tech-debt.md`:

1. **Reporting panes first** — the status bar's notices, the log view, the equation sheet.
   Same shape as the Context Bar defect: they exist to say something, so a partial report is
   both plausible and unnoticeable.
2. **Panes whose emptiness is legitimate** — `specimen_source_ui`, the Purpose tab, the source
   map. `specimen_source_ui` was silently empty for library models until 2026-08-01.
3. **Cross-pane effects** — the stage tab row, the mode switch. A human checks the pane they
   clicked in; a test can check the other one.

**Known-correct behaviours to pin, from 2026-08-01's fixes** — these are not guesses about
intent, they are decisions Doug already made: the corpus visible unfiltered; HRW specimens
collapsed with MSL expanded; the background naming specimen *and* stage; a source scroll
landing at the left margin; an MSL model's source and clickable identifiers.

## Step 2 — The cache layer

**Do not test the invalidation. Make it unnecessary.**

Twenty `cached_*` fields move into one struct. Resetting becomes
`self.caches = StageCaches::default()`, which clears every field **by construction** — a new
cache field cannot be forgotten, because there is no list to forget it from. That removes the
bug class rather than detecting it, which is worth more than any test of the 54-line reset.

Guards that still earn their place:

- **After loading specimen B, nothing from A survives** — the property the 40 sites exist to
  maintain, asserted once at the level that matters.
- **A cache field count check**, so a field added *outside* the struct is caught. The must-fire
  rule applied to the mechanism itself.

Sequenced second because it removes 20 of 105 fields, which every later extraction would
otherwise have to thread.

## Step 3 — The left panel

The tours picker and the model list, ~450 lines lifted out of `frame_ui` into a new pane
module. **Highest edit frequency in the file**, and the region whose defects Doug caught three
times yesterday.

*(No path is named here on purpose. `doc_citations::every_documented_source_path_exists`
rejected the first draft for citing a file that does not exist yet — correctly: a plan that
writes proposals in the same form as facts is how a later session reads intent as history.)*

**Extract state, not just functions.** A function that still takes `&mut self` can reach any
of 105 fields, so nothing becomes independently testable and the next defect hides just as
well. The pane takes what it owns — the specimen list fields, the filter, the corpus — and
returns an action for the caller to apply.

## Step 4 — `central_panel_ui`

771 lines: the stage tab row and every stage view. Split by stage view, each taking its own
state. Likely the point where the day runs out, and a fine place to stop — the pause's value
does not depend on finishing it.

---

## What this pause will not do

- **No new features.** The corpus list with a filter (`ideas.md` #52) resumes after.
- **No change to what any stage means** — `DECISIONS.md`, Parse's meaning is not a
  performance knob.
- **No optimising HRW to widen test scope** — Doug's standing boundary, untouched.
- **No refactor without a test in front of it**, and no test kept without seeing it fail.

## Tonight

**Doug runs the owed large-scale fidelity sweep.** It exercises the compile path, not the UI,
so this pause does not disturb it — but **do not rebuild an example while the run holds its
binary**, and the standing long-run precautions in `CLAUDE.md` apply.
