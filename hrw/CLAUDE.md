# CLAUDE.md — HRW Observatory

Rust/egui observatory app for studying the Rumoca Modelica compiler. The project's purpose, binding
decisions, and curriculum are in `docs/CHARTER.md` (v1.1) — consult it for any design question; do
not re-litigate settled decisions in-session. Append any nontrivial implementation choice you make
to `DECISIONS.md` with a one-line rationale.

## Current arc

**The seven-arc curriculum is COMPLETE (Arc 7 closed 2026-07-21).** The charter's curriculum (§4.2) is
seven arcs, one per phase; the observatory now instruments the whole pipeline **Parse → Resolve →
Instantiate → Typecheck → Flatten → Structural → Index reduction → Initialization → Events → Solve
lowering → Simulation** — from static IR inspection through live execution. There is **no Arc 8 in the
charter**: further work comes from the backlog (`docs/ideas.md`), the deferred items below, or a new
charter decision — not a pre-planned next arc. Pick the next thrust *with Doug*; don't assume one.

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
Simulation + BLT spy-plot + the 12-specimen notebook). **New pipeline stages must be wired into the
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
`docs/compiler-phases` chapters + external math references. The app's right panel has a **"Read:
specimen narrative"** button beside the generic-chapter button (visual channel → durable narrative).
`ProportionalLoop` is the pilot entry; regenerate traces + review narratives on a pin bump (see
`docs/updating-rumoca.md` step 5). The notebook is *specimen-specific* (Claude's synthesis); it is
distinct from `docs/compiler-phases` (Doug's *generic* phase theory).

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
