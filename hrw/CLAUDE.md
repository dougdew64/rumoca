# CLAUDE.md — HRW Observatory

Rust/egui observatory app for studying the Rumoca Modelica compiler. The project's purpose, binding
decisions, and curriculum are in `docs/CHARTER.md` (v1.1) — consult it for any design question; do
not re-litigate settled decisions in-session. Append any nontrivial implementation choice you make
to `DECISIONS.md` with a one-line rationale.

## Current arc

**Pass one (the public-API build of Arcs 1–7) is COMPLETE. Pass two is now the current work:
RE-IMPLEMENT Arcs 1–7 with internal Rumoca access, then build the log view.** Pass one built the
whole pipeline observatory (Parse → Resolve → Instantiate → Typecheck → Flatten → Structural → Index
reduction → Initialization → Events → Solve lowering → Simulation) under a self-imposed
**public-API-only** constraint. That constraint is **now lifted** — HRW lives in the Rumoca workspace
(`hrw/`) and may reach internal phase state. **Key mechanic:** across a crate boundary a phase's
`pub(crate)` internals aren't reachable, so "accessing non-public APIs" means **additively widening
visibility / adding observation hooks in the `../crates/rumoca-*` crates** — i.e. it *is* the
instrumentation, and it must stay **additive, observation-only, and upstreamable** (see
[`DECISIONS.md`](DECISIONS.md)).

**Completed initiative — End-to-end tour upgrade.** All three visualization features are delivered:
(1) equation sheet (#27), (2) source-to-equation traceability (#28), (3) solver stepping (#29). The
tour document is wired to these views. Only the manual verification stop ("verify every stop works
with a fresh MotorWithBrake load") remains open. *(Its plan document was retired 2026-07-28 —
see `DECISIONS.md`.)*

**Current initiative — source tooling** ([`docs/source-tooling-plan.md`](docs/source-tooling-plan.md)):
seven phases covering the Modelica lexer, syntax highlighting, identifier tracking, the Context Bar,
the tree rework, and the canvas views. **Phases 1–5 complete** (Phase 5 closed 2026-07-28 after Doug
tested the full loop end to end). The Context Bar, designed in
[`docs/context-assembly.md`](docs/context-assembly.md), is what makes the thin-emitter /
thick-reasoner split visible — the premise that HRW is an instrument for use with Claude rather than
a standalone tool.

**The composition primitives are frozen**: one point-at + one follow + background, unchanged until a
practical scenario demonstrates a need. Multiple `follow` items and a third "compare" primitive were
considered and deliberately not built — **do not re-propose them from first principles.**

**Current sequence (Doug, 2026-07-31) — each step's output is the next step's input:**

1. **The MSL survey** — `examples/survey_msl.rs`, first full run 2026-07-31. Rumoca's reach
   across all 2,626 MSL models, plus the IR-shape metrics that stratify the sample.
2. **Fidelity testing at scale** — F1-F9 (`src/fidelity.rs`, `worker.rs`) over that sample.
3. **Test mode + fidelity-report support** — load a report in the LHS, click a model, open
   it compiled in the RHS (`docs/ideas.md` **#52**).
4. **Design and run the oracle test** — Rumoca vs System Modeler (#43). **Constrained by
   [`docs/reports.md`](docs/reports.md):** it must emit the same `name` join key, because a
   mismatch is only an admissible upstream finding when that model is *fidelity-green*.
5. **Oracle-report support in Test mode**, including per-item state (unfiled / filed / fixed
   upstream) that regeneration merges rather than overwrites.

**[`docs/reports.md`](docs/reports.md) is the design authority for steps 3-5.** Its
load-bearing claim: **survey → eligible, fidelity → trustworthy, oracle → findings.** Read it
before building Test mode or designing the oracle test.

**Two dependencies this sequence hides, both worth knowing before starting step 2:**

- **Step 2 needs a compile-by-qualified-name path in the worker**, not just step 3.
  `WorkerState::compile` takes a *file path* and reads it as the specimen source, but an MSL
  model has no such file. #52 records this as a Test-mode prerequisite; it is really a
  step-2 one, because testing HRW's representation of an MSL model means compiling it
  *through HRW's own path*, which is the very thing under test.
- **Step 2 must EMIT a report, not only assert.** F1-F9 currently assert and produce
  nothing loadable. Step 3 has nothing to open unless step 2 writes a file — so the harness
  becomes *assert **and** emit*: the assertion keeps it a test, the emission makes it a
  deliverable (`docs/upstream-strategy.md` planning rule 5).

**One risk in the ordering, and its cheap insurance.** Test mode is built at step 3 with only
*one* real report to load, then asked to take a second at step 5 — an abstraction fitted to
n=1. Mitigation is already half-made: all three reports share the first four columns
(`name`, `kind`, `outcome`, `message`). The other half is to **sketch the oracle report's
columns during step 3**, without building the oracle, so the loader is shaped by two
consumers even though only one exists.

**Delivered 2026-07-29 — four more phase animations.** Tearing, alias elimination, initial-condition
planning and connection expansion now have animated views, bringing the total to eight. Building them
established a distinction to preserve: **not every phase hides a search.** Tearing and connection
expansion are real processes with reasons that exist only mid-run, so they are *replays*; alias
elimination and IC planning are lists computed before HRW sees them, so they are *reveals* with no
Debug button, and their module docs say why. Connection expansion is instrumented for a live trace
but has no Debug button *yet* — re-running flatten needs the whole MSL on the UI thread; the fix is a
worker-side live-debug path (`docs/ideas.md` #9). New Rumoca instrumentation: `rumoca-phase-structural`
(`pub mod blt`, `block_local_incidence`) and `rumoca-phase-flatten` (`connections::trace`,
`flatten_ref_with_options_traced`) — the first non-structural, non-DAE crate instrumented.

**Running HRW's tests — and always `--test-threads=1`:**

```text
cargo test -p hrw --lib -- --test-threads=1                        # ~7s,  353 tests — between edits
cargo test -p hrw --lib --features slow-tests -- --test-threads=1  # ~6min, 454 tests — before committing
cargo build -p hrw --bin hrw                                       # the binary — SEE BELOW
```

**`cargo test` does not build the binary, and that gap is not theoretical.** On 2026-07-31 a
`#[cfg(test)]` attribute was placed above the first of three lifted helpers — the attribute
applies to *one* item — so two of them compiled into `--bin hrw` referencing imports that
existed only under `cfg(test)`. Every test passed; **Doug's debugger launch failed.** Test
builds have everything, so they cannot see this class at all.

`cargo clippy -p hrw --all-targets` does cover the bin — **but check its exit code, not its
output.** The same breakage survived a clippy run that was piped to `grep -c "^warning: "`,
which counts warnings and silently ignores a compile error. `cargo clippy … ; echo $?` or
just let it print.

**Fidelity checks — when they run** (policy agreed with Doug 2026-07-31; full reasoning in
[`docs/fidelity-plan.md`](docs/fidelity-plan.md)). F1–F9 ask whether HRW faithfully
represents Rumoca, and they come in two scales answering *different* questions:

- **Small scale** (the 16 curated specimens, `src/fidelity.rs` + `worker.rs`) **stays in the
  pre-commit run**, costing ~90s of it. It answers *"did HRW drift from itself?"* — and that
  is not a rare event. **Both bugs found on 2026-07-31 were HRW-internal drift**, weeks old,
  introduced by ordinary work, and neither was suspected by anyone.
- **Large scale** (the stratified MSL sample) gets its **own feature gate** so it never runs
  by accident. It answers *"did Rumoca change; does an unseen IR shape break us?"*

Run the large suite: **(1)** after rebasing on upstream — a step in
`docs/updating-rumoca.md`, not a thing to remember; **(2)** before submitting a PR to
CogniPilot/rumoca; **(3)** when HRW changes how it **emits or reads stage JSON** — the
`*_to_json` functions in `worker.rs`, the path grammar in `bridge.rs`,
`IncidenceMatrix::from_report`, or any animation's re-derivation.

Trigger 3 is **code-shaped, not judgement-shaped**, deliberately: "when a change gives reason
to doubt fidelity" is exactly the judgement that already failed twice. A diff can be checked;
a suspicion has to occur to someone.

**Use the fast one between edits and the full one before every commit.** Measured 2026-07-29:
**49 of 402 tests accounted for 180 of the suite's 183 seconds**, nearly all compiling a specimen
against the MSL; the other 353 finish in seconds. The 49 are gated by the `slow-tests` feature, and
`cargo test` reports them as ignored *with a reason* rather than silently skipping them.

**This is not a parallelism fix, and parallelism is not the answer here.** Those 49 all acquire a
global `Mutex<WorkerState>` (Rumoca's `Session` is not thread-safe), so they serialize regardless of
`--test-threads` — dropping to parallel would save about *two seconds*. Doug ruled out the
concurrency work on 2026-07-29 after that measurement. `docs/ideas.md` **#48** is the fix that
actually shortens the full run: memoize compiled specimens, since 37 call sites cover only 12
distinct models.

`--test-threads=1` is still required regardless: two pre-existing tests race on process-global stdout
and on `focus.json`, and fail or hang under the default parallel harness on a clean tree.

**Current plan: [`docs/answer-platform-plan.md`](docs/answer-platform-plan.md)** (2026-07-29).
Five phases sequencing #41 (Claude's teaching database), #42 (ad hoc tours), #43 (Wolfram + System
Modeler as answer channels) and #5 (four-bar / planar mechanics), plus a change to the tech-debt
trigger — from weekly-by-calendar to **scoped by what the next phase touches** (adopted 2026-07-29;
see `docs/tech-debt.md` for the rules). `central_panel_ui` at 664 lines is logged and deliberately
not split before Phase 3 reworks the sub-tab bars — Doug: *"acknowledged"*.

Its spine — **corrected by Doug the same day**, and the corrected form is the load-bearing one:

> **Features are experimentable; stored prose is not.**

Claude's first version said nothing should be built ahead of a real question, generalising from the
tour's failure. The counter-example was already in the repo: the **animations were also speculative**
— nobody asked for a tearing replay — and they are the project's most educational output. Tour
worthless, animations excellent, both built ahead of any question. So speculativeness is not the
discriminator. *A feature you did not know you needed teaches you by being used; prose you did not
know you needed just rots.* The tour's real defect was storing **regenerable content that nothing
checked**.

So: **build speculative features freely** — in a domain nobody has mapped, feature-building *is* the
exploration method, and mistakes are cheap here. Keep only the narrow rule: do not *store*
regenerable explanation ahead of use. Runtime tour loading still goes first, as the **enabler of
experimentation** rather than as a hedge.

The plan **supersedes items 3-5 of the work order below** (attempt the tour, refactor `bridge.rs`,
Phases 6-7), which were written before the tour was attempted and found wanting. Items 1-2 are
delivered.

**Superseded work order (Doug, 2026-07-28) — retained for its reasoning:**

1. **Animation debt** — a trait over the three animation types, plus `animation_controls`'s 8
   positional parameters and the duplicated matrix-canvas boilerplate. First because idea #40 builds
   a *fourth* animation view; copying the pattern again would leave Phase 7 four near-duplicates.
   **Fold in** `current_frame_context()` on the trait, so `view.animation` in the capture carries
   *what* the user is looking at and not merely *where* they are (see `docs/tech-debt.md`).
2. **Idea #40** — instrument `lower_pre_operator` (`rumoca-phase-dae`) with `LiveTrace`. First
   non-structural crate instrumented, so it also tests whether `LiveTrace` generalises — which gates
   ideas #19–#22.
3. **End-to-end tour attempt** — moved *ahead* of Phases 6 and 7 deliberately. Attempting the tour
   is what generates requirements: every Phase 5 improvement came from Doug using the thing, none
   from planning. The tour will stress exactly the tree and canvas work those phases contain, so
   doing them first would be building on assumptions.
4. **Refactor `bridge.rs`** — 2342 lines at Phase 5 close; Phase 6 touches it.
5. **Phases 6 and 7** — the tree rework and the canvas views, **shaped by what the tour turns up**.
   Half of Phase 6's search work already landed as the jump-to-followed-identifier control, and
   incidence rows and spy-plot blocks are already clickable; Phase 7 adds axis labels, Tarjan nodes
   and reduction rows.

**The larger loop this sits inside** (Doug): attempt the tour → find what makes it tedious → fix
that → repeat, until the tour is *pleasurable*. Only then does he begin reading Cellier and using
HRW for its intended purpose, and only after that can he identify improvements to the phase
animations — *"For now, I'm unable to identify such improvements as I am too ignorant about the
algorithms which are being animated."* **Do not propose animation/pedagogy refinements before then;
log them in `ideas.md` instead.** The signal that the loop has converged is a change in the *kind*
of problem reported: from "this is broken or tedious" to "this doesn't teach me the thing well".

**Current work — Pass two, in this order:**
1. **Re-implement Arcs 1–7 with internal access**, arc by arc, delivering *richer* stage views than
   the public API allowed. Per arc: scout what state the phase holds (read the crate under
   `../crates/`), expose it additively, render it. Remaining per-arc instrumentation opportunities
   are captured in [`docs/ideas.md`](docs/ideas.md) (#19–#22). Clearest concrete win: the
   **incidence-matrix view** (Arc 3) — deferred in pass one *precisely because incidence was
   `pub(crate)`*; now reachable and delivered.
2. **The log view** — ✅ **delivered** — a pane streaming compilation + simulation log messages with
   **timestamps** and far more phase/solver detail than the public API could give (per-phase timing
   was impossible when phases 5–9 arrived from one opaque
   `compile_model_strict_reachable_with_recovery` call). The proof the migration was worthwhile (Doug).

Pass one is the **baseline to surpass, not discard** — its stage views, specimens, notebook, and tests
are the reference that pass two enriches. The pass-one arc record follows.

**Arc 7 closed (2026-07-21) — The simulation core** (charter §4.2.7), the biggest inflection (static IR →
live execution). Delivered: **Solve lowering** (phase 8 — DAE → `SolveModel`, via
`rumoca-phase-solve::lower_dae_to_solve_model`) as a stage tab, and **Simulation** (phase 9 — a
worker-thread runner calling `rumoca-sim::simulate_solve_model`, Auto solver = BDF-via-diffsol for stiff /
RK45 otherwise, plotted in an `egui_plot` pane; the UI never blocks, never shells out to the CLI). Ran
start-simple (`SingleInertia` → `BouncingBall` → the stiff `BenchActuator`). **Step-mode plotting** landed
(`worker::discontinuity_segments` breaks the line at reinit jumps, gated on `SimData.has_discontinuities` =
the DAE has a discrete update; `series_color` pins per-variable colour) — closes the Arc-6-deferred
"discontinuities render as discontinuities" and `docs/ideas.md` #8. Closed the "solve lowering not
instrumented" gap (Doug, 2026-07-20).

**Arc 6 closed (2026-07-20):** the compile-level hybrid structure is observable — the **Events** tab shows
`BouncingBall`'s condition (`h <= 0`) + discrete reinit; smooth models show "no events". **Arc 5 closed:**
initialization observable (`RcCircuit` IC plan + relaxation; `CapacitorLoop` structural + `OverInitRc`
init-determinacy blow-ups). **Arc 4 closed:** index reduction on `Drivetrain`; the nonlinear four-bar +
planar library (`lib/PlanarMechanics.mo`) parked/deferred (`docs/ideas.md` #5). Arc 1–7 done (Parse …
Simulation + BLT spy-plot + the 14-specimen notebook). **New pipeline stages must be wired into the
stage-diff highlight + stage-file publishing AND the notebook trace** (see Claude's
`hrw-stage-diff-highlight-extend` memory).

**Close-out gates under review:** Doug is separately weighing whether the differential test (System
Modeler round-trip) and the debugger single-step should remain arc close-out gates at all — Arcs 3 & 4
closed with both accepted (deferred / unconfirmed). Until he decides, treat them as satisfiable-by-acceptance,
not hard blockers (see `docs/ideas.md` #4).

**Per-specimen lab notebook (`docs/specimen-notebook/`).** Each entry has two parts:

- **`trace/`** — the durable per-stage IR plus a `manifest.json` stamping the Rumoca rev and
  specimen hash, produced by `cargo run --example gen_trace -- <Model>`. **Generated, therefore
  correct by construction.** Any number about a specimen is read from here.
- **`purpose.md`** — why the specimen exists (the phenomenon it was authored to trigger) and which
  of Doug's questions it has answered. HRW renders it as the **Purpose** tab of the specimen view.

**Converted 2026-07-29.** Each entry used to carry a `narrative.md` telling the story of that
specimen's trip through the pipeline. Retired, for the reason in `docs/ideas.md` #42: **Claude
regenerates that explanation on demand, so storing it buys nothing and costs staleness** — and the
staleness was real, not hypothetical (`end_to_end_tour.md` described a 7x7 incidence matrix on a tab
showing 48 equations, uncaught because nothing checks prose). 1,632 lines of narrative became 638
lines of purpose. It also removed the most expensive step of a Rumoca pin bump: there is no prose
left to re-verify.

**Both the notebook and `docs/compiler-phases` are written by Claude** — see the authorship
correction below.

**Deferred — revisit after Doug's consideration:** the Arc-1/2 close-out differential tests
(round-tripping specimens through System Modeler vs Rumoca). Doug is deliberately thinking through the
round-tripping workflow and will return to it *without* blocking arc progress — its absence is
intentional, not an oversight.

**Backlog:** unscheduled future-implementation ideas are captured in [`docs/ideas.md`](docs/ideas.md)
(e.g. simulation/convergence-failure narratives, specimen purpose hints in the UI, directory renames).
Candidates, not commitments — consult when planning new work; promote items into an arc/decision when picked up.

## Reference documentation

- Rumoca source: **HRW lives INSIDE a fork of the Rumoca workspace** — `hrw/` is a workspace member
  of `github.com/dougdew64/rumoca` (fork of `CogniPilot/rumoca`) on the `hrw` branch, depending on
  the Rumoca crates via **path deps** (`../crates/rumoca-*`). Read the source directly in the sibling
  `../crates/...` — it's the exact tree HRW builds against (no Cargo cache indirection). The `hrw`
  branch was cut from the former pin `8cdc7419` (v0.9.20); "updating Rumoca" now means **rebasing the
  `hrw` branch on upstream**, per `docs/updating-rumoca.md` (compiler + tests drive the code fixes;
  `cargo run -p hrw --example gen_field_help` refreshes the generic field-help table;
  `docs/compiler-phases` is maintained by Claude — see below). This in-workspace move exists to enable
  **instrumenting Rumoca internals** (the public API exposes phase *results*, not the algorithms'
  *process*); build/run/test from the workspace root with `-p hrw`, or `cd hrw/`. See `DECISIONS.md`.
- **`docs/compiler-phases/` — Claude's teaching database.** One subdirectory per compiler phase,
  with drill-downs (Pantelides, tearing, BLT, …). **Authorship corrected 2026-07-29: Claude wrote
  100% of these**, on Doug's request months ago. CLAUDE.md previously called them "Doug's own
  explanations… refreshed only by Doug", and that was wrong in a way that mattered — it made Claude's
  own months-old prose look like an authoritative outside source.

  **Audience: Claude, not Doug.** Doug reads them only indirectly, through answers. Their job is to
  make Claude a better teacher over months and years, so Claude maintains them and commits them.

  **What goes in** follows [[store what cannot be regenerated]]: Doug's *questions*, the confusion
  behind them, and what finally made a thing click. **Not** Claude's explanations — those are
  regenerable, and storing them builds an echo chamber that a later session mistakes for fact.
  A question asked repeatedly is a signal: either the earlier explanation failed (try a different
  angle) or the thing is not visible in HRW (a feature request, and a better one than Claude invents).

  **Provenance, and how it is enforced** (`docs/provenance.md`, `docs/ideas.md` #41 C).
  Two tests in `src/doc_citations.rs` run in the fast loop: every source path the docs cite
  must exist, and every `*Verified … against `<path>`*` tag must name a file that still
  does. **Upgrading is lazy on purpose** — no audit project; when a real question sends
  Claude into the source, the claims it actually checked get tagged on the way past. Low
  coverage is expected and does not fail; a *wrong* tag does, because a tag is a claim
  about trustworthiness. Coverage is printed each run (5 of 62 docs at 2026-07-30).

  **Every claim carries provenance** — `verified` (checked against code or tools, with the file),
  `cellier` (with a citation), or `inference`. Only the first two are trusted on re-read; `inference`
  gets re-checked. Text predating this rule is `unverified` by default and upgrades **lazily**: when
  a real question sends Claude into the source, the claims actually checked get promoted. The
  database becomes trustworthy exactly where it is used most, with no audit project.

  **Before working on code that touches a compiler phase, read that phase's description** — but treat
  untagged prose as a lead, not a fact. (Distinct from `docs/specimen-notebook/` — the specimen lab
  notebook, also Claude's.)
- **[`docs/upstream-strategy.md`](docs/upstream-strategy.md) — how engaging the Rumoca
  maintainers serves Doug's education, and what that implies for planning.** Doug asked
  (2026-07-31) to be reminded of this and for it to shape testing/implementation plans, so
  **consult it when planning work**, not only when preparing something to send.

  The short form: engagement is a **means**, not an end — their conversations are
  educational, they are busy, so the questions must be worth answering. The mechanism is to
  **make questions cheap to answer and interesting** ("here are 380 MSL models failing at
  flatten with the same error shape — expected?" beats "why does X?"), which is fidelity and
  survey work's real return beyond catching bugs.

  **Order deliverables by *their* cost to accept, not our effort.** Bug reports with System
  Modeler adjudication, an MSL capability map, and differential testing are zero-cost gifts;
  **HRW itself goes last**, because it is the only item asking for maintenance burden and
  review time. Gifts open conversations, proposals get scrutinized. The rarest thing Doug
  brings is *not* HRW — it is differential testing against a commercial Modelica
  implementation, which a volunteer project cannot cheaply do for itself.

  Planning rules that follow: prefer zero-adoption-cost artifacts; anything published must be
  **reproducible** (checked-in code *and* output, deterministic selection) and **honestly
  bounded** (a "what this does not establish" section, because one visible overreach costs
  more than several missing checks); and flag upstreamable work *at planning time*, not after
  it is built in a shape that cannot be handed over.
- **[`docs/upstream-issues.md`](docs/upstream-issues.md) — Rumoca bugs found through HRW,
  written ready to file.** Doug files them when the time is right; **Claude adds entries and
  never files them itself.** Only *reproduced* bugs go in, with reproduction, expected vs
  actual, and suspect code marked as unverified — a confident wrong diagnosis in a bug
  report wastes a maintainer's time and costs the credibility this project is building
  (`project-engage-rumoca-community`). Where an independent implementation can adjudicate,
  use it first: "System Modeler rejects this and you accept it" beats "I think the spec
  says…". Two entries as of 2026-07-29, both found by auditing failure paths.
- **[`docs/fixture-tours/`](docs/fixture-tours/) — tours that are *tests*, not
  explanations.** Kept and versioned, unlike an ad hoc tour (`.hrw-bridge/tour.md`,
  gitignored, regenerated per question). The ephemerality rule was never about tours; it was
  about **explanation**, which rots because nothing checks it. A fixture tour has expected
  outcomes, so it fails loudly instead. **Only justified because something runs it:**
  `fixture_tour_links_all_resolve` parses every link on every test run — a saved tour nobody
  runs is stored prose with extra steps. Pick one from the list at the top of Tour mode; the
  directory is hard-coded, not a setting. **One fixture tour per capability, not per change.**
  They cover what Claude cannot see: the rendered surface.

  **Cross-platform stops** (2026-07-30, ideas #47): a tour may route through Wolfram
  Desktop or System Modeler when the point cannot be made in HRW. Each stop marks its
  medium. A **fixture** notebook is versioned in `docs/fixture-tours/notebooks/` — an *ad
  hoc* notebook is ephemeral like an ad hoc tour, but a fixture has expected outcomes, so a
  test that vanishes on a fresh checkout is not a test. Claude evaluates every result
  through the kernel first, then ships cells for **Doug** to evaluate: the stop that lands
  is the one he checks himself.

  **Narrow, one feature each — deliberately** (Doug, 2026-07-30). Claude proposed *wider*
  tours after noticing that half of one walk's bugs came from outside the stops, and had
  the evidence backwards: those bugs were found *because* the tour was short enough to
  leave attention to spare. **The scarce resource is Doug's attention per expectation, not
  his walks**, so a wide tour consumes the surplus that produced the off-stop findings
  rather than multiplying them. Narrow also means a stop failure implicates one feature,
  and means Claude authors the tour while it still knows exactly what should happen —
  both "mostly collapsed" and the phantom highlight were written about behaviour Claude
  had *not* just built.

  Past ten or so fixtures this needs a **selection principle** (walk the tour for whatever
  just changed, plus one stale one) and **visible staleness** — nothing currently catches a
  tour whose *expectations* rot, only its links. "Last walked" is derivable from the
  `tour-link` entries in the action trail.

  **An expectation must say WHERE to look**, not only what to look for. Doug clicked a
  stop that was correctly refused, with the reason on screen, and reported that nothing
  happened — the tour said "a notice appears" and never said notices live in the status
  bar (2026-07-30). A reader who does not know where to look cannot check an expectation,
  so it is not violable in practice however precise its wording.

  **Every `**Expected:**` line must be violable.** Doug found "mostly collapsed" where the
  truth is *fully* collapsed (2026-07-30); an audit then found "nothing particularly
  centred" in a second fixture. Neither could be contradicted by anything, so neither
  tested anything — and hedged expectations teach Doug to read them loosely, which defeats
  the point. Write what would be *different* if the feature broke: a number, a named field,
  "nothing moves", "the counter goes down".
- **[`docs/question-ledger.md`](docs/question-ledger.md) — started 2026-07-29.** The questions
  themselves: verbatim wording, what was on screen, which medium answered, and what actually made it
  click. **Scan it before answering in a familiar area.** A repeated question is the signal, and it
  branches two ways that call for opposite responses — the concept is hard (try a different angle,
  don't restate louder), or the thing is not visible in HRW (a feature request, better than any
  Claude invents). The first entry is a repeat about Claude's own coined term, which is a lesson
  about Claude: naming an abstraction is not teaching it.
- Architectural invariants are in Rumoca's numbered SPEC files; comments cite Modelica Language
  Specification sections. Respect phase boundaries — IR crates are pure data.

## Architecture rules (from charter §4.4 and Decision 6)

- Rumoca is linked **as a library** — now via **path deps on the sibling `../crates/rumoca-*`**
  (HRW is an in-workspace member of the fork; the charter's "path dependency on the workspace" option).
  Never shell out to the Rumoca CLI. A load-IR-from-JSON import path is retained as a secondary mode only.
  **Instrumentation is permitted and intended** — additive, observation-only hooks in the Rumoca crates
  (semantics-preserving, so HRW stays faithful to real Rumoca), designed to be upstreamable. See `DECISIONS.md`.
- Compilation and simulation run on a **worker thread**, results returned over a channel. The
  egui `update()` loop never blocks and never calls into the compiler or solver directly.
- Native builds only. No WASM targets, no web deployment (charter Decision 5).
- One generic serde-value tree inspector, pointed at every pipeline stage's IR — not
  per-stage bespoke tree widgets. Graph views (egui_graphs) and custom-painter views
  (bipartite matching, BLT spy plot) arrive in their own arcs, not before.
- Ask before adding a new dependency; record accepted ones in `DECISIONS.md`.

## Debugging conventions

The VS Code debugger is a first-class learning instrument — structure code so that a breakpoint
can be set inside a Rumoca phase while it processes a specimen.

- Breakpoints belong in **actions** (button handlers, worker-thread tasks), never in the
  per-frame paint path. Keep compile/simulate logic out of rendering code.
- `[profile.dev.package]`: keep full debug info on all Rumoca crates; raise `opt-level` on
  numerical kernels only if debug-build simulation becomes painful.
- Debug stack: rust-analyzer + CodeLLDB. Rumoca pins its toolchain via `rust-toolchain.toml`.

## Specimen rules (from charter §4.3 and §4.1)

- Specimens live in `specimens/`, authored in Wolfram System Modeler, written to the
  **portable Modelica subset** — no Wolfram-flavored extensions. Definition of done: compiles
  and runs equivalently in System Modeler and Rumoca.
- **Scratch specimens (2026-07-29, ideas #42) live in `.hrw-bridge/specimens/` instead.** Claude
  writes them mid-conversation to answer a question — *"here is the smallest model that shows the
  thing you asked about"* — and HRW lists them (marked, in the explore colour) within a second, no
  restart. They are **not** held to the rules above and must never be promoted casually: the
  gitignored bridge directory makes them **ephemeral by construction**, the same rule as `tour.md`.
  A probe worth keeping gets moved into `specimens/` deliberately, with a `// purpose:` line and a
  notebook entry — which is the moment it stops being a probe.
  **A scratch name may not shadow a curated one:** the collision is reported in the panel and the
  scratch file skipped, because silently loading a different model than the name says would have
  Claude reason confidently about source Doug is not looking at.
- **No MSL MultiBody.** Mechanical components come from our own small planar (2D) mechanics
  library, hand-built in the portable subset (revolute joint, rigid link, ideal motor,
  friction, contact).
- Comparison protocol: identical solver tolerances, identical initial conditions, explicit
  `experiment` annotations, agreement metric = relative error on state trajectories and
  event-time differences.
- **Every specimen carries a `// purpose:` comment** (one line, phenomenon-focused — the compiler
  feature it exercises, e.g. "high-index, structurally singular DAE"). The app scans it (`read_purpose`)
  and shows it under the filename in the specimen list; keep it distinct from the Modelica description
  string (which stays a faithful *model* description). Add one to each new specimen, and give it a
  `docs/specimen-notebook/<Model>/` trace + `purpose.md` (see the notebook README).

## Arc close-out ritual

An arc is done when: (1) the specimen passes the differential test in both toolchains;
(2) the arc's observatory pane renders the relevant IR; (3) Doug has single-stepped the phase
in the debugger on that specimen; (4) the trace log (IR before/after the phase) is captured;
(5) `CLAUDE.md`'s Current Arc section is advanced.
