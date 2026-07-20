# TwoLoops — a compilation narrative

*A per-specimen lab-notebook entry: the story of one specimen's trip through the
Rumoca pipeline, told against the committed [`trace/`](trace/) (ground truth) and
foregrounding what **this** specimen is designed to make interesting.*

> **Provenance.** Written against `trace/` from
> `cargo run --example gen_trace -- TwoLoops`, Rumoca `rev 8cdc74198` (v0.9.20),
> specimen fnv1a `8533e9ef74b20654` (see [`trace/manifest.json`](trace/manifest.json)).
> Regenerate on a specimen edit or pin bump, then re-read against the diff.

---

## Why this specimen exists

`TwoLoops` ([`specimens/TwoLoops.mo`](../../../specimens/TwoLoops.mo)) chains two
idealized proportional loops: the first loop's controller output drives the second
loop's setpoint.

```modelica
errorA   = reference - plantA * commandA;   // ┐ loop A
commandA = gainA * errorA;                  // ┘
errorB   = commandA - plantB * commandB;    // ┐ loop B  (driven by commandA)
commandB = gainB * errorB;                  // ┘
```

It exists to show that a system can contain **several independent simultaneous
blocks**. Loop A depends on nothing but the reference; loop B depends on loop A
but not vice versa. So structural analysis finds **two** strongly-connected
components — two coupled blocks — and schedules them in order. Where
[`ProportionalLoop`](../ProportionalLoop/narrative.md) is one orange box and
[`MixedLoop`](../MixedLoop/narrative.md) is one box flanked by scalars, this is
**two boxes**.

---

## The pipeline, stage by stage

Front stages are generic pass-throughs for a self-contained scalar model (see
[`docs/understanding`](../../understanding/)); the structure is the story.

### Flatten → [`trace/flatten.json`](trace/flatten.json)
Nine variables — five parameters and four unknowns (`errorA`, `commandA`,
`errorB`, `commandB`) — and four algebraic residuals (from the trace):

| slot | residual (`0 =`) | i.e. |
|------|------------------|------|
| `f_x[0]` | `errorA − (reference − plantA·commandA)` | `errorA = reference − plantA·commandA` |
| `f_x[1]` | `commandA − gainA·errorA` | `commandA = gainA·errorA` |
| `f_x[2]` | `errorB − (commandA − plantB·commandB)` | `errorB = commandA − plantB·commandB` |
| `f_x[3]` | `commandB − gainB·errorB` | `commandB = gainB·errorB` |

`{errorA, commandA}` reference only each other (and the reference parameter);
`{errorB, commandB}` reference each other **and** `commandA`.
[Phase 5](../../understanding/phase5_flatten/flatten.md).

### Structural → [`trace/structural.json`](trace/structural.json)
Tarjan finds two cycles — `errorA ↔ commandA` and `errorB ↔ commandB` — and,
because loop B reads `commandA` (matched inside loop A) but loop A reads nothing
from B, the dependency runs one way. So the BLT order is **loop A, then loop B**,
each a coupled block, each torn independently:

```
1.  COUPLED  {errorA, commandA}      tear = errorA · residual = f_x[0] · causal: commandA ← f_x[1]
2.  COUPLED  {errorB, commandB}      tear = errorB · residual = f_x[2] · causal: commandB ← f_x[3]
```

`coupled_block_count: 2`. In the spy-plot: **two orange 2×2 boxes down the
diagonal**, in solve order — the visual signature of a staged system of algebraic
loops. Each box is a separate 1-variable tear (guess the error, solve the command,
drive the residual to zero); the solver finishes box 1 before starting box 2,
because box 2 needs box 1's answer.

---

## Contrast across the notebook

- vs [`ProportionalLoop`](../ProportionalLoop/narrative.md): one coupled block vs
  two. Same tearing idea, applied twice, sequenced by the data dependency.
- vs [`MixedLoop`](../MixedLoop/narrative.md): both are "structured" BLTs, but
  MixedLoop interleaves *scalar* blocks with one loop, while TwoLoops sequences two
  *coupled* blocks — together they cover the arrangements the spy-plot can show.
- vs [`Drivetrain`](../Drivetrain/narrative.md): TwoLoops is fully matched (every
  block solvable); Drivetrain is structurally singular. Multiple coupled blocks are
  fine — it is *unmatched* equations that signal trouble.

## References
[Flatten](../../understanding/phase5_flatten/flatten.md) ·
[Structural analysis](../../understanding/phase7_structural_analysis/structural_analysis.md).
SCC decomposition → BLT ordering: R. E. Tarjan, "Depth-first search and linear
graph algorithms," *SIAM J. Comput.* 1(2):146–160, 1972
([doi:10.1137/0201010](https://epubs.siam.org/doi/10.1137/0201010)); tearing of the
individual blocks: F. E. Cellier & E. Kofman, *Continuous System Simulation*,
Springer, 2006 ([doi:10.1007/0-387-30260-3](https://link.springer.com/book/10.1007/0-387-30260-3)).
