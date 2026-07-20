# ProportionalLoop — a compilation narrative

*A per-specimen lab-notebook entry: the story of one specimen's trip through the
Rumoca pipeline, told against the committed [`trace/`](trace/) (the ground truth)
and foregrounding what **this** specimen is designed to make interesting.*

> **Provenance.** Written against `trace/` produced by
> `cargo run --example gen_trace -- ProportionalLoop`, Rumoca `rev 8cdc74198`
> (v0.9.20), specimen hash + per-stage status in [`trace/manifest.json`](trace/manifest.json).
> If the specimen or the Rumoca pin changes, regenerate the trace and re-read
> this narrative against the diff — claims below cite specific trace locations, so
> a stale claim is a checkable one.

---

## Why this specimen exists

`ProportionalLoop` ([`specimens/ProportionalLoop.mo`](../../../specimens/ProportionalLoop.mo))
is an **idealized proportional servo inner loop** with every dynamic element
removed. A real inner loop integrates — the motor inertia is a *state*, so the
loop closes through `der(ω)` and the compiler sees an ODE. Here the plant is a
static gain, so **every relation is instantaneous (algebraic)** and the feedback
closes on itself with nothing to break it:

```modelica
error       = reference - measurement;   // summing junction
command     = controllerGain * error;    // proportional controller (Kp = 10)
measurement = plantGain * command;       // ideal static plant (gain = 2)
```

Substituting, `measurement = 2·10·(reference − measurement)`, i.e. `measurement`
appears on both sides: **an algebraic loop.** That is the single phenomenon this
specimen is built to trigger — the smallest model that forces Rumoca's structural
phase to report a genuine *simultaneous algebraic block* (a coupled BLT block)
and to *tear* it. It is not numerically hard (one linear solve, `measurement =
20/21`); it is *structurally* interesting, which is the whole point of Arc 3.

---

## The pipeline, stage by stage

Each stage below names the trace file to open and what is worth seeing in it. The
early stages are generic (covered by [`docs/understanding`](../../understanding/));
the action is at **Flatten → Structural**, so that boundary gets the deep read.

### 1 · Parse → [`trace/parse.json`](trace/parse.json)
The raw AST of the source: three `component_clause` declarations for the
parameters, three for the unknowns, and three equations in the `equations` list —
written exactly as typed, with all `def_id` fields still `null` (parsing assigns
no identities). Nothing specimen-specific yet; this is just faithful syntax.
See [Phase 1 · Parsing & AST](../../understanding/phase1_parsing_and_ast/parsing_and_ast.md).

### 2 · Resolve → [`trace/resolve.json`](trace/resolve.json)
Names become identities. Every declared component now carries a `def_id`
(`reference` → 89, `controllerGain` → 90, `plantGain` → 91, `error` → 92,
`command` → 93, `measurement` → 94), and each type reference `Real` resolves to
the builtin `def_id` 1. This is the *assign identities* half of scope resolution —
the exact assignment you can watch in the debugger at
`registration.rs` (see [docs/debug-set-sites.md](../../debug-set-sites.md)).
See [Phase 2 · Resolve & Scope](../../understanding/phase2_resolve_and_scope/resolve_and_scope.md).

### 3 · Instantiate → [`trace/instantiate.json`](trace/instantiate.json)
The resolved class is expanded into a concrete instance (the `InstanceOverlay`).
Because `ProportionalLoop` is flat already — no submodels, no connectors — this
stage is nearly a pass-through: no hierarchy to unfold, no `connect` to expand.
That *quietness* is itself the lesson — contrast it with `Drivetrain`, where
Instantiate does heavy lifting across electrical/rotational/translational
connectors. See [Phase 4 · Instantiate](../../understanding/phase4_instantiate/instantiate.md).

### 4 · Typecheck (instanced) → [`trace/typecheck.json`](trace/typecheck.json)
The instanced overlay is enriched in place: component `type_id`s are resolved and
dimensions evaluated (all scalar `Real` here). Diff this against
`instantiate.json` to see exactly what typecheck added — in the app that is the
green "changed vs previous stage" highlight. For a scalar model the delta is
small; the machinery matters more on array/dimensioned specimens.
See [Phase 3 · Typecheck & Dimensions](../../understanding/phase3_typecheck_and_dims/typecheck_and_dims.md).

### 5 · Flatten → [`trace/flatten.json`](trace/flatten.json)
The model becomes a flat **DAE** in residual form `f(x) = 0`. Two facts in this
file are the crux of the whole specimen:

- **`variables` has 6 entries, but only 3 are unknowns.** `reference`,
  `controllerGain`, `plantGain` are *parameters* (known); `error`, `command`,
  `measurement` are the unknowns. So it is a **3×3** algebraic system.
- **`equations` has 3 residuals, and every one is algebraic** — there is *no*
  `der(...)` anywhere in the file. Rendered from `trace/flatten.json`:

  | slot | residual (`0 =`) | i.e. |
  |------|------------------|------|
  | `f_x[0]` | `error − (reference − measurement)` | `error = reference − measurement` |
  | `f_x[1]` | `command − (controllerGain · error)` | `command = Kp · error` |
  | `f_x[2]` | `measurement − (plantGain · command)` | `measurement = plantGain · command` |

**Zero states, three algebraic unknowns.** That is the structural signature of an
idealized (integrator-free) loop, and it is what guarantees the next stage finds a
loop instead of an ODE. See [Phase 5 · Flatten](../../understanding/phase5_flatten/flatten.md).

### 6 · Structural → [`trace/structural.json`](trace/structural.json)
This is the arc's phase, and it does not rewrite the DAE — it **analyzes** it,
emitting a report (matching + BLT blocks + tearing). The deep read follows.

---

## The heart: matching → BLT → tearing

Structural analysis (Rumoca phase 7) turns the flat DAE into a solvable *schedule*
in three moves. Each is visible in [`trace/structural.json`](trace/structural.json).

### (a) Incidence + maximum matching
First, the **bipartite incidence**: which unknowns appear in which equation. Then
a **maximum matching** assigns each equation exactly one unknown it can be "solved
for" — a perfect matching if the system is structurally non-singular. The report's
`matching` records what Rumoca found:

```
f_x[0]  ↔  measurement
f_x[1]  ↔  error
f_x[2]  ↔  command
```

A subtlety worth pausing on: the matching is **structural, not algebraic** — it
only asks *does this unknown appear in this equation?*, not which side of the `=`
it was written on. Notice `f_x[0]` (`error = reference − measurement`) got matched
to **`measurement`**, not `error`. In a loop there are several valid perfect
matchings, and the algorithm picked this rotated one; the specific pairing inside
a coupled block is *not* the solve order (tearing decides that below). Maximum
bipartite matching for DAEs is classical — augmenting-path / transversal
algorithms; see Cellier & Kofman (below), Ch. on structural analysis.

### (b) Tarjan SCC → one coupled block
Rumoca builds a directed dependency graph over the *matched* equations (edge
`eq_a → eq_b` when `eq_a` references the variable matched to `eq_b`) and runs
Tarjan's strongly-connected-components algorithm. Each SCC is one **BLT block** in
evaluation order. Here the three equations form a single cycle
(`error → command → measurement → error`), so there is exactly **one SCC of size
3** — `coupled_block_count: 1`. That is the orange box the spy-plot draws: a
`3×3` block filling the whole matrix, with **no** scalar (diagonal-only) blocks,
because the entire system is the loop. Tarjan's linear-time SCC algorithm is
Tarjan (1972) — the same paper underlying BLT ordering.

### (c) Tearing: 3 unknowns → 1 iteration variable
A coupled block must be solved *simultaneously* — but tearing shrinks the
simultaneous part. Rumoca's greedy Cellier-style tearing (in Rumoca's
`tearing.rs :: tear_algebraic_loop`, the line the debugger breaks on — see the
[arc trace log note](../../debug-set-sites.md)) picks the fewest **tear
(iteration) variables** such that, once guessed, the rest solve causally. The
report's `tearing`:

```
tear var:            command
residual equation:   f_x[0]
causal sequence:     error       ← f_x[1]
                     measurement ← f_x[2]
```

Read it as an inner solve: **guess `command`**, then compute `error = command/Kp`
from `f_x[1]` and `measurement = plantGain·command` from `f_x[2]`; the leftover
equation `f_x[0]` (`error = reference − measurement`) becomes the **residual** the
iteration drives to zero. So a 3-dimensional algebraic loop collapses to a **1-D**
solve on `command`. That reduction — from block size to *tear* count — is exactly
what tearing buys, and why real tools tear before handing loops to a Newton
solver. See Cellier & Kofman, and (for how tearing composes with index reduction)
Pantelides below.

---

## Contrast, and an honest limit

- **Algebraic loop ≠ high index.** This specimen's coupled block comes from a
  *legitimate* simultaneous system that is fully matched (3/3). Compare
  [`Drivetrain`](../Drivetrain/narrative.md), which is *structurally singular*
  (93/97 matched) because its ideal gears impose position constraints — that is
  **high differential index**, and it needs index reduction (Pantelides / dummy
  derivatives), the subject of Arc 4, not tearing. Same phase, two very different
  verdicts; keeping both specimens side by side is the point.
- **The spy-plot shows the diagonal only.** The orange box is faithful (block
  membership + BLT order + tearing), but the *off-diagonal* incidence — e.g. that
  `f_x[0]` also reads `measurement`, the wiring that makes it a loop — is not
  drawn, because Rumoca's raw incidence matrix is `pub(crate)` and HRW draws only
  what the public report exposes (see [DECISIONS.md](../../../DECISIONS.md), Arc 3
  increment 2). For a 3×3 single block the box *is* the story; on larger systems,
  remember the between-block structure is omitted.
- **The rest of the family.** [`SingleInertia`](../SingleInertia/narrative.md) and
  [`RotationalInertia`](../RotationalInertia/narrative.md) are the opposite pole —
  pure index-1 ODEs that sort into scalar blocks only (no loop), because their
  states' integrators *are* the tears. This specimen removes the integrator on
  purpose, so the loop has nothing to break it.

---

## References

**Rumoca phase docs (in this repo):**
[Flatten](../../understanding/phase5_flatten/flatten.md) ·
[Structural analysis](../../understanding/phase7_structural_analysis/structural_analysis.md)
(matching, BLT, tearing drill-downs).

**External:**
- R. E. Tarjan, "Depth-first search and linear graph algorithms," *SIAM J.
  Comput.* 1(2):146–160, 1972. [doi:10.1137/0201010](https://epubs.siam.org/doi/10.1137/0201010)
  — the SCC algorithm behind BLT block ordering.
- F. E. Cellier & E. Kofman, *Continuous System Simulation*, Springer, 2006. ISBN
  978-0-387-26102-7. [doi:10.1007/0-387-30260-3](https://link.springer.com/book/10.1007/0-387-30260-3)
  — the standard treatment of structural analysis, BLT, and tearing of DAEs.
- C. C. Pantelides, "The consistent initialization of differential-algebraic
  systems," *SIAM J. Sci. Stat. Comput.* 9(2):213–231, 1988.
  [doi:10.1137/0909014](https://epubs.siam.org/doi/10.1137/0909014) — structural
  index reduction (why `Drivetrain` needs Arc 4, and this specimen does not).
- S. E. Mattsson & G. Söderlind, "Index reduction in differential-algebraic
  equations using dummy derivatives," *SIAM J. Sci. Comput.* 14(3):677–692, 1993.
  [doi:10.1137/0914043](https://epubs.siam.org/doi/10.1137/0914043) — the
  index-reduction method Arc 4 will study.
- Modelica Language Specification — [specification.modelica.org](https://specification.modelica.org/)
  (equation systems, `connect`, and the handling of algebraic loops).
