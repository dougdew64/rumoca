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
the tree rework, and the canvas views. Phases 1–4 complete. **Phase 5 is next: the Context Bar**,
designed in [`docs/context-assembly.md`](docs/context-assembly.md) — the feature that makes the
thin-emitter / thick-reasoner split visible, and the one that most matters to the premise that HRW
is an instrument for use with Claude rather than a standalone tool.

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
stage-diff highlight + stage-file publishing AND the notebook trace/narrative** (see Claude's
`hrw-stage-diff-highlight-extend` memory).

**Close-out gates under review:** Doug is separately weighing whether the differential test (System
Modeler round-trip) and the debugger single-step should remain arc close-out gates at all — Arcs 3 & 4
closed with both accepted (deferred / unconfirmed). Until he decides, treat them as satisfiable-by-acceptance,
not hard blockers (see `docs/ideas.md` #4).

**Per-specimen lab notebook (`docs/specimen-notebook/`) — now active.** Each entry pairs a durable
**compilation trace** (`trace/` = the per-stage IR files + a `manifest.json` stamping the Rumoca
rev + specimen hash, produced by `cargo run --example gen_trace -- <Model>`) with a Claude-written
**`narrative.md`** — the grounded story of *that specimen's* trip through the pipeline, foregrounding
the phenomenon the specimen was designed to trigger, citing specific trace locations, and linking to
`docs/compiler-phases` chapters + external math references. `ProportionalLoop` is the pilot entry;
regenerate traces + review narratives on a pin bump (see `docs/updating-rumoca.md` step 5). The
notebook is *specimen-specific* (Claude's synthesis); it is distinct from `docs/compiler-phases`
(Doug's *generic* phase theory).

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
  `docs/compiler-phases` is refreshed only by Doug). This in-workspace move exists to enable
  **instrumenting Rumoca internals** (the public API exposes phase *results*, not the algorithms'
  *process*); build/run/test from the workspace root with `-p hrw`, or `cd hrw/`. See `DECISIONS.md`.
- Doug's phase explanations: **`docs/compiler-phases/`** (in THIS repo) — top-level summary, one
  subdirectory per compiler phase containing a phase description, some with drill-down documents
  (e.g. Pantelides, tearing, BLT). These are Doug's own explanations, matching the pinned Rumoca
  commit; treat them as authoritative context. **Before working on code that touches a compiler
  phase, read that phase's description**; consult drill-downs when the work goes deep. (Distinct
  from `docs/specimen-notebook/` — the specimen-driven lab notebook.)
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
  `docs/specimen-notebook/<Model>/` trace + narrative (see the notebook README).

## Arc close-out ritual

An arc is done when: (1) the specimen passes the differential test in both toolchains;
(2) the arc's observatory pane renders the relevant IR; (3) Doug has single-stepped the phase
in the debugger on that specimen; (4) the trace log (IR before/after the phase) is captured;
(5) `CLAUDE.md`'s Current Arc section is advanced.
