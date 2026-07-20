# CLAUDE.md — HRW Observatory

Rust/egui observatory app for studying the Rumoca Modelica compiler. The project's purpose, binding
decisions, and curriculum are in `docs/CHARTER.md` (v1.1) — consult it for any design question; do
not re-litigate settled decisions in-session. Append any nontrivial implementation choice you make
to `DECISIONS.md` with a one-line rationale.

## Current arc

**Arc 2: Instantiate → Flatten** (charter §4.2.2). Specimen: a motor–gearbox–link drivetrain
crossing electrical, rotational, and translational domains — connector expansion, flow-sum
generation, modifiers ("where object orientation dies and equations are born"); diff the flattened
output against a hand-flattened prediction. This arc also lands the model-scoped **typecheck**
deferred from Arc 1 (`typecheck_instanced`, post-instantiation — see DECISIONS.md).

Scope: add the instantiate → flatten (and typed) stages to the worker pipeline and point the ONE
generic serde tree at them (charter §4.4). **Do not build the bipartite / BLT / spy-plot or graph
views** — those belong to the Matching/BLT arc (§4.2.3) and later. If a task needs machinery from a
later arc, stop and ask. Arc 1 (Parse, Resolve) and the bridge/help system are done.

**Deferred — revisit after Doug's consideration (as of 2026-07-19):** the Arc-1 close-out ritual's
differential test (round-trip `RotationalInertia.mo` through System Modeler vs Rumoca) and the
per-specimen lab notebook (`docs/notebook/`). Doug is deliberately thinking through the
round-tripping + notebooking workflow and will return to them *without* blocking arc progress —
their absence is intentional, not an oversight, and this note advances the arc despite the open
ritual item #1.

## Reference documentation

- Rumoca source: **git dependency on official Rumoca** (`github.com/CogniPilot/rumoca`) pinned to
  commit `8cdc7419` in `Cargo.toml`. The compiled source lives in Cargo's cache —
  `~/.cargo/git/checkouts/rumoca-*/8cdc7419/crates/...` — read it there (locate files with
  `find ~/.cargo/git/checkouts -path '*rumoca*/<file>'`). This is the authoritative source HRW
  builds against; a local `~/dev/rumoca` clone, if present, is only a personal reference and may
  differ from the pin. **Bumping this pin follows `docs/updating-rumoca.md`** (compiler + tests
  drive the code fixes; `cargo run --example gen_field_help` refreshes the generic field-help
  table; `docs/understanding` is refreshed only by Doug).
- Doug's phase explanations: **`docs/understanding/`** (in THIS repo) — top-level summary, one
  subdirectory per compiler phase containing a phase description, some with drill-down documents
  (e.g. Pantelides, tearing, BLT). These are Doug's own explanations, matching the pinned Rumoca
  commit; treat them as authoritative context. **Before working on code that touches a compiler
  phase, read that phase's description**; consult drill-downs when the work goes deep. (Distinct
  from `docs/notebook/` — the specimen-driven lab notebook.)
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

## Arc close-out ritual

An arc is done when: (1) the specimen passes the differential test in both toolchains;
(2) the arc's observatory pane renders the relevant IR; (3) Doug has single-stepped the phase
in the debugger on that specimen; (4) the trace log (IR before/after the phase) is captured;
(5) `CLAUDE.md`'s Current Arc section is advanced.
