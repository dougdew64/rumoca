# CLAUDE.md — HRW Observatory

Rust/egui observatory app for studying the Rumoca Modelica compiler. The project's purpose, binding
decisions, and curriculum are in `docs/CHARTER.md` (v1.1) — consult it for any design question; do
not re-litigate settled decisions in-session. Append any nontrivial implementation choice you make
to `DECISIONS.md` with a one-line rationale.

## Current arc

**Arc 5: Initialization & IC planning** (charter §4.2.5). This arc studies how Rumoca computes a
**consistent initial state at t = 0** — the initial-condition solve plan over the `initial` + continuous
equations and `start` attributes. Per the charter it is "the arc where the original [Rumoca] bug lived,
and where an early compiler most needs contributions." **Specimen: the RC/RL blow-up case** — a
resistor–capacitor (or –inductor) circuit whose initialization is non-trivial (implicitly-defined /
over-determined ICs), the resurrected 2025 case. (The linkage-from-static-equilibrium specimen §4.2.5
also lists is **deferred** — it needs the parked planar library + the nonlinear-constraint reduction
Arc 4 found missing.)

**Rumoca capability CONFIRMED — not blocked-on-upstream** (scouted 2026-07-20):
`rumoca-phase-structural::ic_plan` is **public** — `build_ic_plan(dae, n_x) -> Vec<IcBlock>` produces the
IC solve plan (`IcBlock` = `ScalarDirect` symbolic / `ScalarNewton` / `TornBlock` LM+causal / `CoupledLM`),
and `build_ic_relaxation_hint(dae, n_x)` gives the drop-equation / pin-unknown hint for structurally
singular initial subsystems. `n_x` = state count. The flat model already carries `initial_equations` /
`initial_structured_equations` / `initial_assert_equations` (visible in the Flatten trace). `IcBlock`
holds `rumoca_core::Expression`, so — like the structural report — build JSON rather than deriving Serialize.

Increment plan: (1) scout `build_ic_plan` / `build_ic_relaxation_hint` on an RC/RL specimen + decide the
observation (the IC plan + relaxation hint, JSON like the structural report); (2) wire an
**Initialization** stage/view into the observatory (full new-stage checklist: worker → tab → diff-highlight
→ stage files → field-help → trace → narrative); (3) author the RC/RL blow-up specimen and show the IC plan
(+ the failure mode the 2025 bug produced — the most likely upstream-contribution source, charter §5).

**Arc 4 closed (2026-07-20):** index reduction is observable on `Drivetrain` (Structural = singular →
Index reduction = solvable). Finding: Rumoca's Rust-path reduction handles *linear* high-index (gears) but
not *nonlinear* holonomic constraints; the four-bar linkage + planar library (`lib/PlanarMechanics.mo`)
are parked/deferred (`docs/ideas.md` #5). Arc 1–4 done (Parse … Index reduction + BLT spy-plot + the
7-specimen notebook). **New pipeline stages must be wired into the stage-diff highlight + stage-file
publishing AND the notebook trace/narrative** (see Claude's `hrw-stage-diff-highlight-extend` memory).

**Close-out gates under review:** Doug is separately weighing whether the differential test (System
Modeler round-trip) and the debugger single-step should remain arc close-out gates at all — Arcs 3 & 4
closed with both accepted (deferred / unconfirmed). Until he decides, treat them as satisfiable-by-acceptance,
not hard blockers (see `docs/ideas.md` #4).

**Per-specimen lab notebook (`docs/specimen-notebook/`) — now active.** Each entry pairs a durable
**compilation trace** (`trace/` = the six stage IR files + a `manifest.json` stamping the Rumoca
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

- Rumoca source: **git dependency on official Rumoca** (`github.com/CogniPilot/rumoca`) pinned to
  commit `8cdc7419` in `Cargo.toml`. The compiled source lives in Cargo's cache —
  `~/.cargo/git/checkouts/rumoca-*/8cdc7419/crates/...` — read it there (locate files with
  `find ~/.cargo/git/checkouts -path '*rumoca*/<file>'`). This is the authoritative source HRW
  builds against; a local `~/dev/rumoca` clone, if present, is only a personal reference and may
  differ from the pin. **Bumping this pin follows `docs/updating-rumoca.md`** (compiler + tests
  drive the code fixes; `cargo run --example gen_field_help` refreshes the generic field-help
  table; `docs/compiler-phases` is refreshed only by Doug).
- Doug's phase explanations: **`docs/compiler-phases/`** (in THIS repo) — top-level summary, one
  subdirectory per compiler phase containing a phase description, some with drill-down documents
  (e.g. Pantelides, tearing, BLT). These are Doug's own explanations, matching the pinned Rumoca
  commit; treat them as authoritative context. **Before working on code that touches a compiler
  phase, read that phase's description**; consult drill-downs when the work goes deep. (Distinct
  from `docs/specimen-notebook/` — the specimen-driven lab notebook.)
- Architectural invariants are in Rumoca's numbered SPEC files; comments cite Modelica Language
  Specification sections. Respect phase boundaries — IR crates are pure data.

## Architecture rules (from charter §4.4 and Decision 6)

- Rumoca is linked **as a library** (path/git dependency on the workspace — v0.8+ has no
  crates.io release). Never shell out to the Rumoca CLI. A load-IR-from-JSON import path is
  retained as a secondary mode only.
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
