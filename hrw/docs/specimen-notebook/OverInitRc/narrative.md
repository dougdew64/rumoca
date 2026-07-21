# OverInitRc — a compilation narrative

*A per-specimen lab-notebook entry: the story of one specimen's trip through the
Rumoca pipeline, told against the committed [`trace/`](trace/) (ground truth) and
foregrounding what **this** specimen is designed to make interesting.*

> **Provenance.** Written against `trace/` from
> `cargo run --example gen_trace -- OverInitRc`, Rumoca `rev 8cdc74198` (v0.9.20);
> specimen hash + per-stage status in [`trace/manifest.json`](trace/manifest.json).
> Regenerate on a specimen edit or pin bump, then re-read against the diff.

---

## Why this specimen exists

`OverInitRc` ([`specimens/OverInitRc.mo`](../../../specimens/OverInitRc.mo)) is the
clean [`RcCircuit`](../RcCircuit/narrative.md) with **two conflicting initial
conditions** bolted on:

```modelica
initial equation
  C.v = 0;       // fix the capacitor voltage at 0
  der(C.v) = 0;  // AND demand steady state (which forces C.v = 5)
```

The continuous model is perfectly well-posed (index-1, same as `RcCircuit`). But
the *initialization* is **over-determined**: the capacitor has **one** state
(`C.v`), yet the user pins it **twice** — once to 0, once (via `der(C.v) = 0`) to
the steady-state value 5 V. No initial state satisfies both; a solver handed this
diverges. It is the pure **initialization blow-up** — a failure entirely at t = 0,
with nothing structurally wrong with the DAE itself.

This specimen exists to exercise the Initialization stage's **determinacy check**
([`docs/ideas.md`](../../ideas.md) #6): the observatory must catch this even though
`build_ic_plan` — which plans only the algebraic subsystem — does not see the
user's initial equations at all.

---

## The pipeline — an all-green model with a red *initialization*

- **Flatten / Structural / Index reduction** — all clean, identical to `RcCircuit`
  (index-1, well-matched). Nothing here signals a problem; the DAE is fine.
- **Initialization → [`trace/initialization.json`](trace/initialization.json)** —
  this is where it fails. `build_ic_plan` still emits a plan (it plans the
  algebraic subsystem regardless), but the stage's **`determinacy`** block reports
  the real story, rendered from the DAE (`initialization.equations` + fixed-start
  states vs states):

  ```
  states                       : 1
  initial_equations            : 2      (C.v = 0, der(C.v) = 0)
  fixed_start_states           : 0
  explicit_initial_conditions  : 2
  surplus_over_states          : +1     → OVER-DETERMINED
  ```

  The stage note flags it red: *"OVER-DETERMINED initialization: 2 explicit initial
  condition(s) for 1 state — 1 too many; conflicting / redundant ICs."* That +1
  surplus is the blow-up, named and quantified.

**Why this is only over-determination, not under-determination.** The check flags a
*surplus* of explicit conditions; it does **not** flag a deficit, because a state
with no explicit condition still initializes from its `start` attribute (default
init). `RcCircuit` has `surplus = −1` and is well-posed — correctly *not* flagged.
Over-specification is the unambiguous, actionable signal.

---

## Contrast across the notebook

- vs [`RcCircuit`](../RcCircuit/narrative.md): the *same* clean circuit; the only
  difference is the two conflicting `initial equation`s. One is well-posed, the
  other blows up — and the difference is visible *only* in the Initialization tab's
  determinacy verdict, nowhere upstream.
- vs [`CapacitorLoop`](../CapacitorLoop/narrative.md): both are "blow-ups,"
  but of **different kinds**. CapacitorLoop's DAE is *structurally* ill-posed (it
  fails at Structural / Index reduction); OverInitRc's DAE is *structurally fine*
  and fails only in the **initialization determinacy**. Together they cover the two
  ways initialization goes wrong: a bad system, and a bad set of initial conditions.

## References
[Structural analysis · IC planning](../../compiler-phases/phase7_structural_analysis/ic_plan.md).
- C. C. Pantelides, "The consistent initialization of differential-algebraic
  systems," *SIAM J. Sci. Stat. Comput.* 9(2):213–231, 1988
  ([doi:10.1137/0909014](https://epubs.siam.org/doi/10.1137/0909014)) — consistent
  (and, here, *inconsistent* / over-specified) initial conditions.
- **Modelica Language Specification** §8.6 (initialization, `initial equation`,
  `fixed`/`start`) — [specification.modelica.org](https://specification.modelica.org/).
