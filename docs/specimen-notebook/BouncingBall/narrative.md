# BouncingBall — a compilation narrative

*A per-specimen lab-notebook entry: the story of one specimen's trip through the
Rumoca pipeline, told against the committed [`trace/`](trace/) (ground truth) and
foregrounding what **this** specimen is designed to make interesting.*

> **Provenance.** Written against `trace/` from
> `cargo run --example gen_trace -- BouncingBall`, Rumoca `rev 8cdc74198` (v0.9.20);
> specimen hash + per-stage status in [`trace/manifest.json`](trace/manifest.json).
> Regenerate on a specimen edit or pin bump, then re-read against the diff.

---

## Why this specimen exists

`BouncingBall` ([`specimens/BouncingBall.mo`](../../../specimens/BouncingBall.mo))
is the archetypal **hybrid** model — a ball falling under gravity that bounces:

```modelica
Real h(start = 1.0), v(start = 0.0);
equation
  der(h) = v;
  der(v) = -g;
  when h <= 0 then
    reinit(v, -e * pre(v));   // reflect the velocity, losing energy (e = 0.8)
  end when;
```

It is the **Arc 6** (events & hybrid structure) specimen. Between bounces the model
is a plain smooth ODE; **at contact the equation structure changes** — a discrete
event fires and resets a state. This arc studies how Rumoca lowers that `when` /
`reinit` / `pre` into the DAE's event machinery. It is self-contained (portable
subset), chosen as the reframe for Arc 6 because the charter's stick-slip-friction
specimen needs the parked planar mechanics library, and an MSL `IdealDiode`
rectifier fails Rumoca's typecheck (see [DECISIONS.md](../../../DECISIONS.md)).

---

## The pipeline, stage by stage

- **Flatten … Initialization** — all clean and *smooth*. Two states (`h`, `v`),
  index-1, no algebraic loop, no index reduction, a trivial init. On every prior
  specimen the story ended here; BouncingBall is the first whose **Events** tab is
  not empty.
- **Structural / Index reduction / Initialization** — nothing hybrid shows up: those
  analyses see only the *continuous* part (`der(h) = v`, `der(v) = -g`). The event
  is invisible to them — which is exactly why Arc 6 needs its own view.

### Events → [`trace/events.json`](trace/events.json)  *(Arc 6)*
Here the `when` clause appears, lowered into the DAE's hybrid partitions. The
`summary`:

```
condition_equations      : 1
relations                : 1     → (h <= 0)
discrete_real_updates    : 1     → the reinit of v  (f_z)
discrete_valued_updates  : 0
zero_crossing_conditions : 0
scheduled_time_events    : 0
```

Read it as the two halves of a hybrid system:

- **The trigger** — a **relation** `h <= 0` (in `conditions.relations`). A solver
  watches this expression cross zero; the sign change *is* the event. This is a
  *state* event (its timing depends on the trajectory), not a scheduled time event.
- **The action** — one **discrete real update** (`discrete.real_updates`, the `f_z`
  partition): the `reinit(v, -e * pre(v))`. When the event fires, `v` is reset to
  `-e·pre(v)` — the pre-event velocity reflected and scaled by the restitution `e`.
  `pre(v)` is the value *just before* the event, the hallmark of discrete-time
  semantics.

So the flat model carries both a smooth continuous system (the ODE) **and** a
discrete overlay (this condition → update), and the Events tab is where that
overlay becomes visible. `zero_crossing_conditions` / `scheduled_time_events` are
empty here (the trigger lives in `conditions.relations`); a model with `sample(...)`
or an `after`-style schedule would populate `scheduled_time_events` instead.

**What Arc 6 does *not* show (yet).** The charter's "step-mode plotting" — running
the model and watching `h` bounce with discontinuous `v` — is a genuinely new
capability (a simulation runner + a plot pane) that belongs to **Arc 7 (the
simulation core)**. Arc 6 shows the *structure* of the hybrid model at compile time;
Arc 7 will run it.

---

## Contrast across the notebook

- vs every prior specimen: all of them are **smooth** — their Events tab reads "no
  events (this model is a smooth continuous system)". BouncingBall is the first with
  a real event partition, so it is the one that makes the tab worth opening.
- The event here is a *state* event (`h <= 0`); the natural next hybrid specimens
  (deferred with the planar library) are stick-slip friction and joint-limit stops,
  where the *continuous* equations themselves switch between modes — a richer change
  of structure than a single reinit.

## References
[DAE construction · events & hybrid structure](../../compiler-phases/phase6_dae_construction/dae_construction.md).
- **Modelica Language Specification** §8.3.5 (`when`-equations), §3.7.3 (`reinit`),
  §3.7.2 (`pre`) — [specification.modelica.org](https://specification.modelica.org/).
- F. E. Cellier & E. Kofman, *Continuous System Simulation*, Springer, 2006
  ([doi:10.1007/0-387-30260-3](https://link.springer.com/book/10.1007/0-387-30260-3))
  — state events, zero-crossing detection, and hybrid simulation.
