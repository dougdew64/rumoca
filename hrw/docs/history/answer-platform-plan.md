# Plan — HRW as an answer platform

**Purpose:** the sequencing plan that carried out the 2026-07-29 reframe of the project.
**Status:** **HISTORICAL — do not follow its phases.** Retired 2026-08-01. Kept for its
reasoning, which is still load-bearing, and for the record of a correction Doug made that
governs how work is chosen.
**Read when:** you want to know *why* speculative features are permitted but stored prose is
not, or what the 2026-07-29 reframe actually changed. **Not** when deciding what to do next —
that is [`../../CLAUDE.md`](../../CLAUDE.md). *(It also named `current-work.md`, deleted 2026-08-01 when its work finished.)*

## Where its content went

Everything still live was moved out before retirement. Nothing here is the only copy:

| Content | Now lives in |
|---|---|
| The "features are experimentable" rule | `../../CLAUDE.md` (and Claude's memory) |
| Phase 2 — the first Cellier problems | `../ideas.md` **#57** |
| Phase 5 — four-bar linkage | `../ideas.md` **#5** |
| B4 — failure specimens per phase | `../ideas.md` **#46** |
| #41 stages D-E, #22's revisit trigger | `../ideas.md`, at those items |
| "The ledger is always in flight" | `../question-ledger.md` |
| The tech-debt trigger change | `../tech-debt.md` |
| Phases 6-7 of source tooling | `../source-tooling-plan.md`, still live there |

---

Written 2026-07-29, at the end of the session that reframed the project. Sequences
`docs/ideas.md` #41 (Claude's teaching database), #42 (Answers), #43 (Wolfram
and System Modeler as answer channels), #5 (four-bar linkage + planar mechanics),
and the tech-debt discipline.

Supersedes the "agreed work order (2026-07-28)" in `hrw/CLAUDE.md`, whose items 3–5
(attempt the lab, refactor `bridge.rs`, Phases 6–7) were written before the lab
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

The lab was worthless; the animations were the best part; both were speculative.
So speculativeness is not the discriminator. The discriminator is:

> **A feature you did not know you needed teaches you by being used. Prose you did
> not know you needed just rots.**

The lab's defect was never that it was built early — it was that it **stored
regenerable content that nothing checked**. Two different defects, collapsed into
one by the first version of this section.

**So the rule is narrower than "build only what a question asks for":**

- **Do not store regenerable explanation ahead of use.** This is the real rule, and
  it is what retired the narratives and the lab prose.
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
every lab needs a rebuild, labs cannot be experimented with at all. And Phase 2
is no longer "the requirements document for Phase 3" — one problem from one chapter
of a many-chaptered book is an anecdote, not a requirements process. Doug:
*"Working through the first Cellier problem might not be all that informative for
our decision making."* Build a first iteration of #42, **experiment with it for a
while**, and let a period of use rather than a single trial decide what comes next.

---

## Where things stand after 2026-07-29

**The day's real output was a change of project philosophy, not a batch of features.**

An earlier version of this section measured the day by code written and ledger entries
added, and concluded the instruments had run ahead of their use. Doug rejected that framing,
correctly:

> I would describe this day as a breakthrough change of project philosophy. Exhibit A is the
> concept of Answers which you create to answer questions. [...] I want very much to
> complete our paradigm change for HRW [...] while today's discussion is still fresh in my
> head.

**The wrong yardstick.** Counting code misses what actually changed, and what changed will
govern hundreds of future decisions:

- **HRW stopped being a tool Doug uses and became a medium both parties write to.** The
  inbound noun channel (Context Bar) gained a return path (#42).
- **Store what cannot be regenerated** — which retired 1,632 lines of narrative, re-aimed
  `docs/compiler-phases` at Claude, and killed the lab's prose.
- **Features are experimentable; stored prose is not** (Doug's correction of Claude's
  over-generalisation).
- **Labs multiply user testing**, and holes in labs are the signal.
- **Three platforms, three questions**, with System Modeler as an *adjudicator* that
  corrects Claude's bias toward blaming its own specimen.
- **Deadlines are real**, so gaps get fixed pre-emptively; and **audit narrowly, fix
  immediately.**

A second mismeasurement: "the ledger has no mathematics in it" narrowed the ledger to a
mathematics-only instrument. Its stated purpose is *what Doug asked and what made it click*,
and today taught him a great deal about what software can be when built to work with a
reasoner. That is learning.

**So the ordering below is by what the paradigm shift needs, not by yield.** Momentum on a
coherent design is real value: the design of #41-#47 is in Doug's head *now*, and deferring
half of it means re-deriving it worse in three weeks.

---

## Next batch — complete the paradigm shift  *(ordered)*

**B1–B3 delivered 2026-07-30**, and all five fixture labs run clean by Doug. The
answer channel now reaches every noun the capture can describe, spans three platforms,
and the teaching database is mechanically checked.

What that cost in bugs is the part worth remembering: **fourteen defects**, of which Doug
found nine by running labs and the tests and clippy found five. Not one was found by
Claude reading its own code. Several were in the artifacts *describing* HRW rather than in
HRW — a lab asserting a highlight that had never been built, a diagnostic file describing
the state before an action, two lab expectations that nothing could contradict.


### B1. Finish the answer channel (#42's remainder)  ✅ **DONE 2026-07-30**

Claude deferred these as "not needed yet", which was yield-thinking. Under the paradigm
framing they are **load-bearing**:

- **Ad hoc specimens.** *"Here is the smallest model that exhibits the thing you asked
  about"* is a core capability of the new paradigm — a teacher's basic move, and one Claude
  cannot currently make. **Split, do not repurpose** `specimens/`: the curated corpus has
  properties (portable subset, `// purpose:` comments, System Modeler round-trip intent) that
  scratch models would degrade. Note that 2026-07-29's four hand-authored specimens are
  *durable* and correctly belong in the curated set; the split is for disposable probes.
- **`Canvas` camera aiming.** A lab that cannot make Doug *look at* node 25 is an incomplete
  channel. Read `should_refit` and its tests first — the sideways-drift bug shows how fragile
  that camera is.
- **Animation frame addressing.** A stop can name a view but not the moment inside it, and
  the moment is where a replay's content lives.
- **`bridge.rs` decomposition (2365 lines)** — now *in scope* rather than deferred: it owns
  `focus.json`, so it owns half of noun parity. The tech debt gets paid **as** paradigm work.
- **`central_panel_ui` (664 lines)** — its four near-parallel sub-tab bars are exactly what
  frame addressing reworks. Same reasoning.

### B2. #47 — labs that span platforms  ✅ **DONE 2026-07-30**

Without this the paradigm is "HRW plus chat" rather than three platforms. Per-stop medium, a
gitignored path for notebooks, and Claude evaluating before delivering while **Doug**
evaluates to learn.

**The first one already has its question**, so do not design it in the abstract: the
`CapacitorLoop` lab ends by admitting HRW cannot show a matrix with full *structural* rank
that is numerically singular. Mathematica can, on a 3x3 Doug can perturb himself.

### B3. #41 stages B-C — make the database trustworthy  ✅ **DONE 2026-07-30**

- **B: the citation checker.** Mechanical, no dependencies, one known broken citation.
- **C: provenance tags**, upgrading lazily through use. Without them the database is Claude's
  own unverified prose, which is the echo chamber the arrangement exists to avoid.

Stages **D-E** (generated index, repeat detection) still wait: four ledger entries is not
enough to retrieve from. A content dependency, not a priority judgement.

### B4. #46 — failure specimens for the remaining phases

The testing loop, now with every channel available. Three specimens on 2026-07-29 produced
**four bugs** (two filable upstream), so the yield is established and oracle-first will
improve it. Parse, Instantiate, Events, Solve lowering, and a *working* flatten case remain.

### Then, in their own time

**#17** as a cross-platform lab rather than as HRW work. **#5** (four-bar) when there is
appetite for the rabbit hole. **The mathematics** — Cellier, or a model that will not
compile — when Doug is ready; that one only he can start.

**Doug's alone regardless:** filing the two entries in `docs/upstream-issues.md`.

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

## Phase 1 — Minimum viable Answer  ✅ **DONE 2026-07-29**

**Only one change: load the lab document from disk at runtime.** Today it is
`include_str!`'d into the binary, so a new lab needs a rebuild.

- A scratch path Claude writes to, picked up without restarting HRW.
- Lab-mode panel reads from there when a file is present, falls back to nothing
  when it is not.
- **Deliberately do not touch the link vocabulary.** The existing three verbs
  (`load`, `stage`, `load/stage`) are enough for a first lab, and the fourth verb
  should be chosen by a lab that needed it.

**Exit criterion — met.** Claude writes `.hrw-bridge/answer.md` mid-conversation and
Doug sees it without a rebuild. Delivered as `bridge::read_lab` +
`App::poll_lab_file`; the round trip and link parsing are covered by
`an_ad_hoc_lab_round_trips_through_the_bridge`.

**Two things the work turned up.** The old `lab_document_hrw_links_are_valid` test
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

Suggested starting material: the structural-analysis chapters the retired lab
already cited (Cellier & Kofman, *CSM* Ch. 9.3–9.5), where Rumoca's fit is best,
so the first attempt tests **the loop** rather than **the fit**.

**What this phase produces:**

- Ledger entries — the first real ones with HRW context attached.
- Evidence about what labs cannot yet express. **Evidence, not a requirements
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

**Reordered 2026-07-29 by evidence rather than guesswork.** The first lab hit two
holes, and that changes the ranking: sub-tab links degraded *four* navigation moments
in a single lab, which makes gap 2 the most-felt item rather than gap 1. See the
lab-holes table in `docs/tech-debt.md`.

1. **Link vocabulary parity with `focus.json`** — the design principle is that
   `hrw://` should express any noun `focus.json` can describe. Same vocabulary,
   opposite direction.
2. **Sub-view unification** — the four dissimilar enums (`StructuralView`, shared
   across two stages, plus `EventsView`, `FlattenView`, `InitView`). Deferred out of
   the Phase 0 sweep *deliberately*: this is the first design decision of the link
   vocabulary, not cleanup.
3. **`Canvas` camera aiming** — no way to centre on a node today. Read
   `should_refit` and its tests first: the 2026-07-29 sideways-drift bug shows how
   fragile that camera is, and a lab aiming it will fight the same fit logic.
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
to keep disciplined is the *storage* rule: a lab that gets built and used is fine
whether or not a question asked for it; a lab whose prose gets **saved** as a
durable artifact is not (see #42's ephemerality rule).

## Phase 4 — #41 stage B: the citation checker  *(small, can slot in anywhere)*

`cargo run -p hrw --example check_doc_citations` — verify every `crates/**/*.rs`
path and named test cited in `docs/` still resolves. An ad-hoc run on 2026-07-29
found 16 of 17 paths good and one broken
(a test file that had moved to `crates/rumoca-sim/src/solve_lowering/tests.rs`).

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

- **Every sweep starts from the lab-holes table** at the top of `docs/tech-debt.md`.
  A lab hole is a place HRW stopped Claude from answering a question, so it degrades
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
  not. Not scheduled — it should be cut down when a lab needs its spine, so the
  cut is informed by what a real lab wanted.
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
