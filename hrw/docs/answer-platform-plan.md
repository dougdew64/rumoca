# Plan — HRW as an answer platform

Written 2026-07-29, at the end of the session that reframed the project. Sequences
`docs/ideas.md` #41 (Claude's teaching database), #42 (ad hoc tours), #43 (Wolfram
and System Modeler as answer channels), #5 (four-bar linkage + planar mechanics),
and the tech-debt discipline.

Supersedes the "agreed work order (2026-07-28)" in `hrw/CLAUDE.md`, whose items 3–5
(attempt the tour, refactor `bridge.rs`, Phases 6–7) were written before the tour
was attempted and found wanting.

---

## The tension this plan has to resolve

Today established that **building explanatory infrastructure ahead of real use** is
what made `end_to_end_tour.md` worthless, and that features should be
**traceable to a real question** Claude answered badly or could not answer.

#42 is a large build with **zero logged questions demanding it.** By the rule just
established, that is suspect.

The resolution, and the spine of this plan:

> **#42's *premise* is evidenced; its *scale* is not.**

Doug attempted the tour, found the prose worse than the conversation, and
identified ad hoc tours as what he actually wanted. That is real friction from real
use — not something Claude imagined. What is *not* evidenced is the size: link
vocabulary parity, camera aiming, sub-view unification, ad hoc specimen
generation. None of those has a question behind it yet.

So #42 is built in two widely separated pieces: the **smallest possible unlock**
now, and everything else **only after real questions ask for it**. If that second
piece never gets built because tours turn out to be rarely the right medium, the
plan has succeeded, not failed.

---

## Phase 0 — done 2026-07-29

- Scoped tech-debt sweep: `ui()` 982 → 325 via `FrameIntent`; `central_panel_ui`
  extracted; three dead locals removed; test-race entry corrected; register
  re-measured.
- Specimen notebook converted: 1,632 lines of narrative → 630 of `purpose.md`.
- Question ledger started (`docs/question-ledger.md`), #41 stage A.
- Backlog captured: #41, #42, #43; #22 deferred; #9 updated; authorship corrected
  in `hrw/CLAUDE.md`.

---

## Phase 1 — Minimum viable ad hoc tour  *(small)*

**Only one change: load the tour document from disk at runtime.** Today it is
`include_str!`'d into the binary, so a new tour needs a rebuild.

- A scratch path Claude writes to, picked up without restarting HRW.
- Tour-mode panel reads from there when a file is present, falls back to nothing
  when it is not.
- **Deliberately do not touch the link vocabulary.** The existing three verbs
  (`load`, `stage`, `load/stage`) are enough for a first tour, and the fourth verb
  should be chosen by a tour that needed it.

**Exit criterion:** Claude writes a tour file mid-conversation and Doug opens it
without a rebuild.

**Why first:** it is the only thing standing between "Claude can compose an answer
in HRW" and "Claude cannot." Everything else in #42 is refinement.

## Phase 2 — First Cellier problem  *(the forcing function)*

Start the actual loop: read a narrative, work here, solve the problem. Use
**existing specimens** — do not build new ones in anticipation.

Suggested starting material: the structural-analysis chapters the retired tour
already cited (Cellier & Kofman, *CSM* Ch. 9.3–9.5), where Rumoca's fit is best,
so the first attempt tests **the loop** rather than **the fit**.

**What this phase produces, which is the point:**

- Ledger entries — the first real ones with HRW context attached.
- A list of what a tour could not express. *This is Phase 3's requirements
  document.*
- Probably a specimen or two, which tells us whether ad hoc specimens matter.
- An answer to whether the fit varies by chapter as much as expected.

**Exit criterion:** one Cellier problem solved, with the ledger recording what
unlocked it. Not "the loop feels good" — a solved problem.

**Risks, stated in advance:**

- Claude being wrong is a *false positive on the test we are relying on*. Mitigation
  is #43: prefer computation over assertion. Cellier says index 2 → watch Pantelides
  reduce it. Claude says a block is well-conditioned → compute the condition number.
- Some chapters will not fit at all (numerical integration theory is pencil work).
  Expect a lopsided loop; do not design a uniform process around it.

## Phase 3 — #42 stage 2, driven by Phase 2  *(medium, scope TBD by Phase 2)*

Now with requirements. Expected, in likely order of demand:

1. **Link vocabulary parity with `focus.json`** — the design principle is that
   `hrw://` should express any noun `focus.json` can describe. Same vocabulary,
   opposite direction.
2. **Sub-view unification** — the four dissimilar enums (`StructuralView`, shared
   across two stages, plus `EventsView`, `FlattenView`, `InitView`). Deferred out of
   the Phase 0 sweep *deliberately*: this is the first design decision of the link
   vocabulary, not cleanup.
3. **`Canvas` camera aiming** — no way to centre on a node today. Read
   `should_refit` and its tests first: the 2026-07-29 sideways-drift bug shows how
   fragile that camera is, and a tour aiming it will fight the same fit logic.
4. **`bridge.rs` decomposition** (2,365 lines) — it owns `focus.json`, so it owns
   half of vocabulary parity. Sequenced *here* rather than earlier for exactly that
   reason.
5. **Ad hoc specimens** — *only if* Phase 2 showed a need. **Split, do not
   repurpose** `specimens/`: the curated corpus has properties (portable subset,
   `// purpose:` comments, System Modeler round-trip intent) that scratch models
   would degrade.

**Build only what Phase 2 asked for.** An item with no question behind it stays in
the backlog.

## Phase 4 — #41 stage B: the citation checker  *(small, can slot in anywhere)*

`cargo run -p hrw --example check_doc_citations` — verify every `crates/**/*.rs`
path and named test cited in `docs/` still resolves. An ad-hoc run on 2026-07-29
found 16 of 17 paths good and one broken
(`crates/rumoca-sim/src/diffsol/tests/scalarization_regressions.rs`).

Cheap, mechanical, and it is the "emitter correct, reasoner supplements" discipline
applied to Claude's own memory. **Slot it in whenever there is a gap** — it has no
dependencies and it protects the database from the failure mode that produced Stop
8. Provenance tags (#41 stage C) upgrade lazily through use and need no phase of
their own; index and repeat-detection (D, E) wait until the ledger has content.

## Phase 5 — #5: four-bar linkage + planar mechanics  *(large, trigger-gated)*

Promoted from parked-since-Arc-4 to central by the 2026-07-29 curriculum
definition: the target is **the mathematics of robotics**, and a closed kinematic
chain produces exactly the high-index DAE that Pantelides exists to fix.
Constrained mechanisms *are* index-3 DAEs.

**Trigger, not a slot.** Build it when a Cellier question or a robotics question
needs a constrained mechanism that no existing specimen provides. Building it
because "robotics sounds right" is the mistake this plan is organised against —
and #5 was parked originally because the nonlinear four-bar was a rabbit hole, so
it deserves a real reason before re-entry.

---

## Tech-debt discipline — a change to flag

**The trigger moves from calendar to phase boundary.** The standing arrangement was
a weekly scan of `docs/tech-debt.md`. Phase 0's sweep was instead **scoped by what
#42 will touch**, which produced a better outcome: one item closed that had been
open across two sweeps, one closed as *obsolete*, one *corrected* as wrong, and
three deliberately pushed into #42 rather than swept.

Proposed standing rule, for Doug to accept or reject:

- **Sweep at each phase boundary, scoped to what the next phase touches.**
- **Measure, never re-estimate.** Phase 0 found `compile()` had grown 327 → 363 and
  `app.rs` ~5,900 → 6,375; the previous sweep found `ui()` had grown 385 lines in a
  day unnoticed.
- **Three outcomes are all legitimate:** fixed, *closed as obsolete* (the batch
  narrative workflow — the narratives went away), or *deferred into the next phase*
  when sweeping it would mean designing an abstraction without its requirements.
- **Skip what the next phase will rewrite.** This is the existing principle; Phase
  0 applied it three times.

Known outstanding debt, with dispositions: `central_panel_ui` 664 lines (do not
split before Phase 3 reworks the sub-tab bars); `compile()` 363 lines (nothing
forces it; growth is one line per artifact); test races (two causes, mitigated by
`--test-threads=1`, worth fixing before any CI since the default harness *hangs*);
`Expansion::force_open` (waits on a Phase 6 that may not survive); 63 pre-existing
clippy warnings in HRW (the Rumoca crates are clean and denied, HRW is not).

---

## What is deliberately *not* in this plan

- **Simulation animations / #22.** Deferred at Doug's direction pending simulator
  maturity — *"I want to focus on stuff where we are confident that the underlying
  Rumoca machinery works correctly and reliably."* Expected to become his highest-
  interest area eventually, so the deferral is about readiness, not value. The
  revisit trigger is measurable now rather than arguable: simulate a specimen in
  both toolchains via #43 and diff the trajectories.
- **The retired `end_to_end_tour.md` prose.** Its chain-of-problems spine and its
  Cellier/Hairer citations are worth keeping; the stage-by-stage walkthroughs are
  not. Not scheduled — it should be cut down when a tour needs its spine, so the
  cut is informed by what a real tour wanted.
- **Phases 6–7 of `source-tooling-plan.md`** (tree rework, canvas views). Half of
  Phase 6's search work already landed as the jump-to-followed-identifier control.
  What remains should be re-derived from real questions, not from a plan written
  before the reframe.
- **Usage telemetry.** `session.json` is a crash artifact with a rotating buffer,
  not a longitudinal record. And questions are the signal; clicks are corroboration
  at best.

## The one thing that is always in flight

**The ledger.** Every phase appends to it, and it is the only artifact whose value
depends on elapsed time. If a phase ends with no ledger entries, that phase was
infrastructure work with no questions behind it — which is precisely the failure
mode this plan exists to prevent.
