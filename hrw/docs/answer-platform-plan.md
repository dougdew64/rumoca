# Plan — HRW as an answer platform

Written 2026-07-29, at the end of the session that reframed the project. Sequences
`docs/ideas.md` #41 (Claude's teaching database), #42 (ad hoc tours), #43 (Wolfram
and System Modeler as answer channels), #5 (four-bar linkage + planar mechanics),
and the tech-debt discipline.

Supersedes the "agreed work order (2026-07-28)" in `hrw/CLAUDE.md`, whose items 3–5
(attempt the tour, refactor `bridge.rs`, Phases 6–7) were written before the tour
was attempted and found wanting.

---

## Features are experimentable; stored prose is not

**This section was wrong in its first version (2026-07-29) and Doug corrected it
the same day.** The corrected form is the more useful one, so the error is recorded
rather than quietly replaced.

The wrong version said: building anything ahead of a real question is what ruined
`end_to_end_tour.md`, therefore #42 must wait for questions to justify it.

The counter-example is in this repository. **The animations were also built ahead
of any specific question** — nobody asked for a tearing replay — and by Doug's
account they are *the most educational thing the project has produced*:

> "my experiments with the features which you have built have been the most
> educational aspect of this entire project so far."

The tour was worthless; the animations were the best part; both were speculative.
So speculativeness is not the discriminator. The discriminator is:

> **A feature you did not know you needed teaches you by being used. Prose you did
> not know you needed just rots.**

The tour's defect was never that it was built early — it was that it **stored
regenerable content that nothing checked**. Two different defects, collapsed into
one by the first version of this section.

**So the rule is narrower than "build only what a question asks for":**

- **Do not store regenerable explanation ahead of use.** This is the real rule, and
  it is what retired the narratives and the tour prose.
- **Features are exempt, and speculative features are often correct.** In a domain
  nobody has mapped — Doug: *"I'm still trying to get my head around the
  possibilities of software in this new age of AI"* — **feature-building is the
  exploration method.** Requirements for something unprecedented cannot be derived;
  they are discovered by building a first iteration and pushing on it.
- **Mistakes are cheap here.** There is no product and no user but Doug. The project
  has already changed course several times as the possibilities became clearer, and
  each change was an improvement.

**What this changes about the sequencing below.** Phase 1 still comes first, but as
the *enabler of experimentation* rather than as a hedge against overbuilding: while
every tour needs a rebuild, tours cannot be experimented with at all. And Phase 2
is no longer "the requirements document for Phase 3" — one problem from one chapter
of a many-chaptered book is an anecdote, not a requirements process. Doug:
*"Working through the first Cellier problem might not be all that informative for
our decision making."* Build a first iteration of #42, **experiment with it for a
while**, and let a period of use rather than a single trial decide what comes next.

---

## Where things stand after 2026-07-29

**Delivered:** four phase animations; the canvas refit fix; `ui()` 982 → 325 via
`FrameIntent`; the notebook conversion; the question ledger (#41 stage A); runtime tour
loading and sub-view links (#42 gaps 1–2); #44; **all of #45** (source locations, the
DAE-construction payload, the `pipeline_failure` capture, the oracle); the front-end audit
with its contamination fix; `docs/upstream-issues.md`; and the tech-debt trigger change.

**The honest gap:** every one of those is *instrumentation*, and almost none of it has been
used for its purpose. The ledger's four entries are all about this project's own design —
**none about continuous-system mathematics.** That was the right call for one session
(features are experimentable, and the yield was two upstream Rumoca bugs, a cross-specimen
contamination bug, and a wrong number being displayed as fact). It is the wrong shape for
two sessions running. **The instruments are now well ahead of the use.**

So the next batch is deliberately **use-led with building attached**, and the ordering
below reflects that rather than backlog numbering.

---

## Next batch (ordered)

### A1. #46 continued — failure specimens, oracle-first  *(highest measured yield)*

Three specimens on 2026-07-29 produced **two filable upstream bugs plus two HRW bugs.**
That is the best yield-per-effort in the project, and the oracle-first practice (#43) makes
the next round better than the last.

Still unrepresented: **Parse**, **Instantiate**, **Events**, **Solve lowering**, and a
*working* flatten case (`IncompatibleConnect` is now a bug report rather than a flatten
specimen). Events and Solve lowering may have no authorable failure mode — a result either
way. Simulation stays deferred (#22).

This is testing wearing authoring's clothes, and it is **pre-emptive**: cheapest now, while
nothing is blocked on it.

### A2. #47 — the first cross-platform tour  *(small, target already identified)*

Do not design cross-platform tours in the abstract; **the first one already has its
question.** The `CapacitorLoop` tour ends by admitting HRW cannot show the one thing Stop 1
rests on: a matrix with **full structural rank that is numerically singular**. Mathematica
can, in a few lines, on a 3x3 Doug can perturb himself.

That single tour exercises the whole of #47 — per-stop medium, a notebook handed over at a
gitignored path, and Claude evaluating before delivering while Doug evaluates to learn. And
it lands on the linear-algebra thread rather than on the compiler.

### A3. #41 stage B — the citation checker  *(cheap, mechanical, slot anywhere)*

`cargo run -p hrw --example check_doc_citations`. Verifies every `crates/**/*.rs` path and
named test the docs cite. One broken citation is already known. No dependencies, so it
fills any gap.

### A4. Doug's alone: a real question

**Phase 2 — a Cellier problem, or a model Doug writes that will not compile.** Claude
cannot start this one; it needs Doug to read something and get stuck. It is also the only
item that puts a *mathematics* entry in the ledger.

Also Doug's alone: **filing the two upstream issues** (`docs/upstream-issues.md`).

### Deliberately NOT next, with reasons

- **#42's remainder** — animation *frame* addressing, `Canvas` camera aiming, and the
  curated/scratch specimen split. **None has been needed.** Four specimens were authored by
  hand on 2026-07-29 and the split never came up; no tour has yet wanted a specific node or
  frame. Build when a tour asks.
- **#41 stages D–E** (generated index, repeat detection) — four ledger entries is not enough
  to retrieve from. Wait for content.
- **#17 Jacobian** — rescoped by #47. When it happens it happens *as* a cross-platform tour,
  not as HRW work.
- **#5 four-bar** — large, and its gate is appetite for a known rabbit hole.
- **`central_panel_ui` (664 lines) and `bridge.rs` (2365)** — both wait on #42's remainder,
  which is not next. Per the sweep rule: skip what the next phase will rewrite.

### The one guard on "implement all of those"

No item above is wrong, and Doug is right to want them all. The risk is not a bad item — it
is **another full session of building that leaves the ledger with no mathematics in it.**
So: at least one real question answered per batch, and if a batch ends with no ledger entry
about continuous-system modelling, that is the signal to stop building and start reading.

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

## Phase 1 — Minimum viable ad hoc tour  ✅ **DONE 2026-07-29**

**Only one change: load the tour document from disk at runtime.** Today it is
`include_str!`'d into the binary, so a new tour needs a rebuild.

- A scratch path Claude writes to, picked up without restarting HRW.
- Tour-mode panel reads from there when a file is present, falls back to nothing
  when it is not.
- **Deliberately do not touch the link vocabulary.** The existing three verbs
  (`load`, `stage`, `load/stage`) are enough for a first tour, and the fourth verb
  should be chosen by a tour that needed it.

**Exit criterion — met.** Claude writes `.hrw-bridge/tour.md` mid-conversation and
Doug sees it without a rebuild. Delivered as `bridge::read_tour` +
`App::poll_tour_file`; the round trip and link parsing are covered by
`an_ad_hoc_tour_round_trips_through_the_bridge`.

**Two things the work turned up.** The old `tour_document_hrw_links_are_valid` test
had `end_to_end_tour.md` as its subject, a document HRW no longer shows — replaced.
And `narrative_hrw_links_are_valid` had started passing **vacuously**: the notebook
conversion renamed `narrative.md` to `purpose.md` and its `continue` swallowed every
directory, so it checked nothing. It now counts the files it checked and asserts the
count, because a silent-skip test is worse than no test.

**Why first:** it is the only thing standing between "Claude can compose an answer
in HRW" and "Claude cannot." Everything else in #42 is refinement.

## Phase 2 — First Cellier problems  *(start the real loop)*

Start the actual loop: read a narrative, work here, solve the problem. Use
**existing specimens** — do not build new ones in anticipation.

Suggested starting material: the structural-analysis chapters the retired tour
already cited (Cellier & Kofman, *CSM* Ch. 9.3–9.5), where Rumoca's fit is best,
so the first attempt tests **the loop** rather than **the fit**.

**What this phase produces:**

- Ledger entries — the first real ones with HRW context attached.
- Evidence about what tours cannot yet express. **Evidence, not a requirements
  document** — one problem is an anecdote. Expect to run several, over a period,
  before the pattern is trustworthy.
- Possibly a specimen or two, which starts to say whether ad hoc specimens matter.
- A first read on how much the fit varies by chapter.

**Exit criterion:** one Cellier problem solved, with the ledger recording what
unlocked it. Not "the loop feels good" — a solved problem. But this is a *start*,
not a gate: Phase 3 does not wait for a statistically respectable sample, and
building more of #42 to experiment with is a legitimate move at any point.

**Risks, stated in advance:**

- Claude being wrong is a *false positive on the test we are relying on*. Mitigation
  is #43: prefer computation over assertion. Cellier says index 2 → watch Pantelides
  reduce it. Claude says a block is well-conditioned → compute the condition number.
- Some chapters will not fit at all (numerical integration theory is pencil work).
  Expect a lopsided loop; do not design a uniform process around it.

## Phase 3 — #42 stage 2  *(medium; overlaps Phase 2 rather than following it)*

Deliberately **not gated on Phase 2 finishing.** Build a first iteration, experiment,
and let a period of use decide what to keep.

**Reordered 2026-07-29 by evidence rather than guesswork.** The first tour hit two
holes, and that changes the ranking: sub-tab links degraded *four* navigation moments
in a single tour, which makes gap 2 the most-felt item rather than gap 1. See the
tour-holes table in `docs/tech-debt.md`.

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
5. **Ad hoc specimens** — constructing the smallest model that exhibits a
   phenomenon is what a good teacher does, and Claude currently cannot. **Split, do
   not repurpose** `specimens/`: the curated corpus has properties (portable subset,
   `// purpose:` comments, System Modeler round-trip intent) that scratch models
   would degrade.

**Build a first iteration and experiment**, rather than waiting for each item to
be demanded. The order above is a guess at likely demand, not a gate. The one thing
to keep disciplined is the *storage* rule: a tour that gets built and used is fine
whether or not a question asked for it; a tour whose prose gets **saved** as a
durable artifact is not (see #42's ephemerality rule).

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

**Trigger-gated, but for a practical reason rather than a principled one.** #5 was
parked originally because the nonlinear four-bar turned into a rabbit hole — that
is a cost argument, not a "wait for a question" argument. Build it when the
mathematics of a constrained mechanism is what Doug wants to work on, which the
curriculum definition says is the target. It does not need a logged question first;
it needs an appetite for the rabbit hole.

---

## Tech-debt discipline — a change to flag

**The trigger moves from calendar to phase boundary.** The standing arrangement was
a weekly scan of `docs/tech-debt.md`. Phase 0's sweep was instead **scoped by what
#42 will touch**, which produced a better outcome: one item closed that had been
open across two sweeps, one closed as *obsolete*, one *corrected* as wrong, and
three deliberately pushed into #42 rather than swept.

**Adopted 2026-07-29** (Doug: *"Your tech debt proposal is spot on… We are entirely
agile for this project"*):

- **Every sweep starts from the tour-holes table** at the top of `docs/tech-debt.md`.
  A tour hole is a place HRW stopped Claude from answering a question, so it degrades
  the deliverable rather than costing future effort — and it arrives with the question
  it blocked as evidence. Doug: *"Fixing those gaps and bugs is high priority."* It is
  the only section of that file whose priority is not a judgement call.

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
depends on elapsed time.

Doug's correction *strengthens* this rather than weakening it. If experimenting
with features is what teaches — the project's strongest evidence to date — then
**the record of which experiment taught what** is the irreplaceable artifact. A
phase that ends with no ledger entries is not necessarily wasted work (the
animations had none while being built), but it does mean the learning went
unrecorded, and that is the one loss this project cannot absorb.
