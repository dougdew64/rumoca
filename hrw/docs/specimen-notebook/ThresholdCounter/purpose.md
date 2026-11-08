# ThresholdCounter — `edge` and `change` are written in terms of `pre`

An undamped oscillator, `x = cos(10 t)`, and two counters driven by the same comparison:

```modelica
Boolean above "the position is above the threshold";
Integer rises(start = 0, fixed = true);
Integer crossings(start = 0, fixed = true);
equation
  der(x) = v;
  der(v) = -omega ^ 2 * x;
  above = x > threshold;
  when edge(above) then
    rises = pre(rises) + 1;
  end when;
  when change(above) then
    crossings = pre(crossings) + 1;
  end when;
```

## What it demonstrates

**`edge` and `change` are not primitives.** MLS 3.6 §3.7.5 defines both through `pre`, the
value a discrete variable had just before the current event:

| written | means | fires when |
|---|---|---|
| `edge(above)` | `above and not pre(above)` | `above` has just become true |
| `change(above)` | `above <> pre(above)` | `above` has just changed either way |

So the two `when` clauses differ *only* in the operator. One relation, `x > threshold`, produces
one zero-crossing root; both counters listen to it, and `rises` counts half the events
`crossings` counts.

## What to look at

The **Events** tab, which shows Rumoca's own condition algebra with the operators already
expanded:

```text
c[0]  x > threshold
c[1]  above and not pre(above)      <- edge(above)
c[2]  above <> pre(above)           <- change(above)
roots: r[0]  x > threshold          (one root, shared by both)
```

There is no `edge` and no `change` anywhere in it. That is the lesson: by the time the compiler
is done, they are `pre` and logic.

## What the run should show, and what Rumoca actually shows

`x` starts at 1, above the 0.5 threshold, so `above` is true from the start. Nothing has
changed at `t = 0`, and both counters start at 0. The crossings fall at `10 t = π/3 + 2πn`
(down) and `10 t = -π/3 + 2πn` (up):

| horizon | rises | crossings | source |
|---|---|---|---|
| `t = 1` | 1 | 3 | exact; **System Modeler 15.0 reproduces it**, stepping at 0.105, 0.524, 0.733 |
| `t = 2` (this trace) | 3 | 7 | exact |
| `t = 2`, **Rumoca 0.10.4** | **4** | **8** | the Simulation tab; both counters already **1 at `t = 0`** |

**Rumoca counts one extra event for each counter, at initialization.** Probes isolated it:
a `when x > 0.5` written directly on the relation is correct (0, then 3), while
`when above` on the Boolean variable fires at `t = 0` as `edge` and `change` do. Giving
`above` the declaration `start = true`, matching its initialized value, removes the extra count;
a `change` on a Boolean whose start differs from its initialized value keeps it. Every case fits
one reading: Rumoca takes `pre(b)` at the initial instant from `b`'s **declared start**, instead
of from the value initialization gives `b` — so here `pre(above)` is `false` while `above` is
`true`, and both `edge` and `change` see a change that did not happen. Recorded, with a
reproduction run on both tools, as [`upstream-issues.md`](../../upstream-issues.md) D3.

**Read the Events tab as right and the Simulation tab's counters as wrong.** The expansion is
exactly the MLS definition; it is the value of `pre(above)` at the first instant that is off.

## Why this is a specimen

Until this one, only `BouncingBall` used `pre`, and no curated specimen used `edge` or `change`.
The expansion the Events pane exists to show was covered only by a scratch model inside a test,
and it took the first curated model using them, checked against System Modeler, to find that the
initial instant was mishandled.

## Verified

Every phase produces IR — the corpus outcome matrix row is `OOOOOOOOOOO`. The Events expansion
is read from the committed trace. The System Modeler values are from a run to `t = 1`, its
oracle horizon; the Rumoca values are from this trace, which runs to `t = 2`.
