# MeasuredOscillator — three states, three ways to know where they start

A mass on a spring, its velocity read by a tachometer, the reading smoothed by a first-order
filter:

```modelica
Real x(start = 0.5, fixed = true) "position, fixed by its own declaration";
Real v "velocity, with no start of its own";
Real v_meas(start = 1.0, fixed = true) "tachometer reading";
Real v_filt(start = 0.0) "filtered reading, whose start is only a guess";
equation
  der(x) = v;
  m * der(v) = -k * x;
  v_meas = v;
  T * der(v_filt) = v_meas - v_filt;
```

## What it demonstrates

**The difference between a start value that is an assertion and one that is a guess, and the
case where the assertion is written on a different variable from the state it settles.**

The model has three states, `x`, `v` and `v_filt`. Each one's initial value is settled a
different way:

| state | what the model says | how it is settled |
|---|---|---|
| `x` | `start = 0.5, fixed = true` on `x` itself | **given** — taken as it is written |
| `v` | nothing on `v`; `fixed = true` on `v_meas`, and `v_meas = v` | **solved**: the plan solves for `v` from the value stated on the reading |
| `v_filt` | `start = 0.0` only | **not settled by the plan** — a bare `start` is only a guess |

`fixed = true` on a variable that is not a state is legal Modelica: it adds the initial
equation `v_meas = 1.0`. What makes this case interesting is that the initial equation is
about `v_meas`, while the thing that needs an initial value is the state `v`.

## What to look at

The **Initialization** tab — the initialization plan Rumoca lowered for the run.

```text
given_states: [x]
unknowns:     [v]
rows:         row 0  Solved  solves v   at MeasuredOscillator.mo:20
                                        Real v_meas(start = 1.0, fixed = true)
note:         1 initialization row(s), 0 surplus check(s)
```

**Read the rows against `given_states`, because they disagree on purpose.** Two states are
settled, but there is only one row. `x`'s own fixed start is given and needs no solving, so it
has no row. A pane that showed only the rows would report one settled state out of three, and
the right answer is two. The row's location is the other half of the lesson: the row *solves*
`v` and points at `v_meas`'s declaration, which you could not predict from `v`'s own — `v`
declares nothing at all.

`v_filt` appears in neither list: the plan neither takes it as given nor solves for it. Here its
guess stands: System Modeler and Rumoca both start `v_filt` at 0.

## Why this is a specimen

**Until this one, no curated specimen declared `fixed = true`.** Every specimen stated bare
`start` values, so the Initialization pane was empty across the whole corpus and only a scratch
model inside a test exercised it. That is the same shape of gap that let F4 go
blind for two releases (see `ArrayFamily`): a pane the corpus never reaches is a pane nothing
checks.

## Verified

**System Modeler 15.0 simulates it** and chooses the same three states. At `t = 0` it reports
`x = 0.5`, `v = 1`, `v_meas = 1`, `v_filt = 0`; Rumoca's trace reports the same four values.
The exact solution is `x(t) = 0.5 cos 2t + 0.5 sin 2t`: System Modeler gives `x(1) = 0.24658`
and Rumoca's trace gives `x(2) = -0.70522`, both matching it. Every phase produces IR — the
corpus outcome matrix row is `OOOOOOOOOOOO`.
