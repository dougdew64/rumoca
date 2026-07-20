# CLAUDE.md — HRW Observatory

Rust/egui observatory app for studying the Rumoca Modelica compiler. The project's purpose, binding
decisions, and curriculum are in `docs/CHARTER.md` (v1.1) — consult it for any design question; do
not re-litigate settled decisions in-session. Append any nontrivial implementation choice you make
to `DECISIONS.md` with a one-line rationale.

## Current arc

**Arc 3: Matching & BLT** (charter §4.2.3). Specimen: an ideal proportional feedback loop closed
around instantaneous relations (a servo inner loop, idealized) — yielding a genuine *simultaneous
algebraic block*. This arc studies **structural analysis** (Rumoca phase 7): the incidence matrix,
maximum bipartite matching (equation↔unknown), Tarjan SCC → **BLT blocks**, and **tearing** of
algebraic loops. Data comes from `build_structural_report(&dae)` over the pipeline's `CompilationResult.dae`
(needs the `rumoca-phase-structural` dep, added).

Scope: **this is the arc where custom views beyond the one generic tree arrive** — a **bipartite
incidence view** and a **BLT spy-plot**, built with a custom `egui::Painter` canvas (deliberately NOT
`egui_graphs` — chosen for maximum flexibility during Doug's dogfooding; see DECISIONS.md). Increment
plan: (1) structural report in a **Structural** generic-tree tab, (2) incidence canvas view,
(3) BLT spy-plot, (4) the feedback-loop specimen. Arc 1–2 (Parse … Flatten) + the bridge / help /
field-help / stage-diff systems are done. New pipeline stages must be wired into the stage-diff
highlight + stage-file publishing (see Claude's `hrw-stage-diff-highlight-extend` memory).

**Per-specimen lab notebook (`docs/notebook/`) — now active.** Each entry pairs a durable
**compilation trace** (`trace/` = the six stage IR files + a `manifest.json` stamping the Rumoca
rev + specimen hash, produced by `cargo run --example gen_trace -- <Model>`) with a Claude-written
**`narrative.md`** — the grounded story of *that specimen's* trip through the pipeline, foregrounding
the phenomenon the specimen was designed to trigger, citing specific trace locations, and linking to
`docs/understanding` chapters + external math references. The app's right panel has a **"Read:
specimen narrative"** button beside the generic-chapter button (visual channel → durable narrative).
`ProportionalLoop` is the pilot entry; regenerate traces + review narratives on a pin bump (see
`docs/updating-rumoca.md` step 5). The notebook is *specimen-specific* (Claude's synthesis); it is
distinct from `docs/understanding` (Doug's *generic* phase theory).

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
- **Every specimen carries a `// purpose:` comment** (one line, phenomenon-focused — the compiler
  feature it exercises, e.g. "high-index, structurally singular DAE"). The app scans it (`read_purpose`)
  and shows it under the filename in the specimen list; keep it distinct from the Modelica description
  string (which stays a faithful *model* description). Add one to each new specimen, and give it a
  `docs/notebook/<Model>/` trace + narrative (see the notebook README).

## Arc close-out ritual

An arc is done when: (1) the specimen passes the differential test in both toolchains;
(2) the arc's observatory pane renders the relevant IR; (3) Doug has single-stepped the phase
in the debugger on that specimen; (4) the trace log (IR before/after the phase) is captured;
(5) `CLAUDE.md`'s Current Arc section is advanced.
