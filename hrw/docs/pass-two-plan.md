# Pass two — re-implementing Arcs 1–7 with internal Rumoca access

**Status: the plan for the current work (set 2026-07-21, just after HRW moved into the Rumoca
workspace).** Pass one built the whole pipeline observatory under a self-imposed *public-API-only*
constraint. Pass two revisits each arc now that HRW is a workspace member and can reach internal
phase state — to deliver **richer stage views** — and culminates in a **log view** that reports far
more compilation/simulation detail than the public API allowed. See `CLAUDE.md` (Current arc) and
`DECISIONS.md`.

## The mechanic, the method, the discipline

- **Mechanic.** Across a crate boundary, a phase's `pub(crate)` internals are *not* reachable from
  `hrw`. So "accessing non-public APIs" means **additively changing the `../crates/rumoca-*` crates**:
  widen a visibility (`pub(crate)` → `pub`), add a small accessor, or add an observation hook. That
  *is* the instrumentation.
- **Method, per arc.** (1) **Scout** — read the phase crate under `../crates/` to find the internal
  state worth showing. (2) **Expose** it additively. (3) **Render** it in HRW, re-using the generic
  serde tree where it fits and a custom painter where it doesn't (à la the BLT spy-plot). (4) **Re-wire
  into every per-stage system** (see the `hrw-stage-diff-highlight-extend` memory) and refresh the
  specimen notebook trace + narrative.
- **Discipline (non-negotiable).** Hooks stay **additive & observation-only** (semantics-preserving →
  HRW stays faithful to real Rumoca, upstream rebases stay clean) and **upstreamable** (shaped as a
  general observability API, kept separable from `hrw/` for a clean cherry-pick). Pass one is the
  **baseline to surpass, not discard**: its views/specimens/tests are the reference.

## Per-arc opportunities (candidates — confirm by reading the crate in-window)

Confidence tags reflect what earlier scouting already established vs. what still needs a look.

### Arc 1 — Parse → Resolve → Typecheck
- Pass one: raw AST, the *resolved* class extracted from the aggregate, the instanced typecheck, and
  a DefId→identity `def_index`.
- Internal to surface: the **resolver's scope/symbol tables and resolution steps**; full diagnostics
  with spans; the typecheck's **dimension-evaluation** steps. (`rumoca-phase-resolve`,
  `rumoca-phase-typecheck`.) → *Scout.*
- Richer view: show name resolution as a *process* (scope stack, what bound what), not just the result.

### Arc 2 — Instantiate → Flatten
- Pass one: instantiate + flatten IR (we already call `rumoca-phase-instantiate` directly).
- Internal: **connector expansion, flow-sum generation, modifier application** — the flattening
  *process* inside `rumoca-phase-flatten`. → *Scout.*
- Richer view: the connector graph + the flow/potential equations *as they're generated*.

### Arc 3 — Matching & BLT  **(clearest concrete win)**
- Pass one: `build_structural_report` → matching + BLT + tearing (JSON + BLT spy-plot). The
  **incidence-matrix view was deferred *because incidence was `pub(crate)`*** (`docs/ideas.md`).
- Internal, now reachable: the **incidence matrix** (equation↔unknown bipartite), the **matching
  steps** (Hopcroft–Karp augmenting paths), **Tarjan's SCC** stack forming the blocks, tearing
  internals. → **High confidence** (incidence known `pub(crate)`).
- Richer view: the deferred **incidence-matrix custom-painter view**; steppable matching / SCC
  formation — process, not just the BLT result.

### Arc 4 — Index reduction (Pantelides)  **(high-value)**
- Pass one: HRW *replicated* the reduction funnel from the **public `dae_prepare` building blocks** —
  handles linear high-index (`Drivetrain`) but **not** nonlinear holonomic constraints (the pendulum
  `x²+y²=L²`).
- Internal: `rumoca-sim`'s **private `prepare_dae_for_structural_analysis`** (+ `eliminate_trivial`,
  `apply_elimination_substitutions_to_dae`), the actual **Pantelides iterations** and
  **dummy-derivative selection** (Mattsson–Söderlind). → **High confidence** (the funnel was a public
  re-implementation of this private code).
- Richer view: the *real* reduction process (which equations differentiated, which states demoted),
  step by step — and it should handle the **nonlinear four-bar / planar library** parked in pass one
  (`docs/ideas.md` #5), potentially reviving it.

### Arc 5 — Initialization & IC planning
- Pass one: `build_ic_plan` + relaxation hint + an over-/under-determinacy heuristic.
- Internal: the **initial-condition solve internals** (the actual solve, homotopy/relaxation path) and
  the **full init-system structural analysis** deferred in pass one (`docs/ideas.md` #7). → *Scout.*
- Richer view: the IC solve as a process; the full init-system incidence/matching.

### Arc 6 — Events & hybrid structure
- Pass one: the DAE's event partitions (public fields).
- Internal: the **`when`→`f_z`/`f_m` lowering steps** and **zero-crossing function construction**. →
  *Scout.*
- Richer view: how the hybrid structure is *lowered*, not just its final partitions.

### Arc 7 — Solve lowering & Simulation  **(high-value; feeds the log view)**
- Pass one: `lower_dae_to_solve_model` + `simulate_solve_model`. Known gaps hit: `SimResult` carries
  **no event times** and a **uniform output grid** (forced the heuristic step-mode plotting).
- Internal: **residual/Jacobian construction, mass matrix, sparsity**; and the **solver internals**
  (`rumoca-solver` / diffsol) — **BDF order history, step-size control, Newton iterations per step,
  and event root-finding** (`StepUntilOutcome::RootFound { t_root }` exists internally). → **High
  confidence** on event times + per-step solver data.
- Richer view: **exact event times** (makes step-mode plotting exact, not heuristic), per-step
  order/step-size plots, Newton convergence, a Jacobian-sparsity view.

## The culmination — the log view

A pane streaming **compilation + simulation log messages with timestamps**:
- **Compilation:** per-phase *entered / did-substep / left* with **timing** — which requires hooks
  that separate the phases Rumoca currently runs together (phases 5–9 arrive from one opaque
  `compile_model_strict_reachable_with_recovery` call, so per-phase timing was *impossible* in pass
  one). This is the direct payoff of the incremental-progress work (`FromWorker::CompileProgress`),
  now with real granularity.
- **Simulation:** per-step solver log — order, step size, event detections + exact times, Newton
  iterations.

Doug's framing: the log view is the deliberate **demonstration that migrating was worthwhile** — it
reports *much* more detail than the public API ever could. Build it **after** the Arc 1–7
re-implementation, once the hooks it draws on exist.

## First moves in the new window
1. Confirm the workspace still builds/tests green from `~/dev/rumoca` (`cargo test -p hrw`).
2. Start with **Arc 3 / incidence** (highest-confidence unlock) or **Arc 4 / Pantelides** (highest
   learning value) — scout the crate, expose additively, render, re-wire, update the notebook.
3. Keep each exposed hook a candidate upstream PR (separable, observation-only).
