# Events — the equations that are not always true

Every tour so far treated a model as one fixed system of equations. **A hybrid model is not
that.** A bouncing ball obeys `der(v) = -g` right up to the instant it touches the floor, and at
that instant something happens which no differential equation describes: the velocity **reverses**.

This phase finds those instants, and separates *what is continuously true* from *what happens at
a moment*.

> Every count below is read from the specimens' generated traces.

---

## The problem this phase exists to solve

A numerical integrator assumes smoothness. It takes a step, estimates the error, and takes a
bigger one if it can. **A discontinuity destroys that assumption** — step across a bounce and the
error estimate is meaningless, because the function it was extrapolating stopped existing
partway.

So the solver must be told: *here is a scalar expression; watch its sign; when it crosses zero,
stop exactly there.* That is **zero-crossing detection**, and the compiler's job is to hand the
solver those expressions and say what to do when one fires.

**A `when` clause is therefore not an `if`.** An `if` chooses between values whenever it is
evaluated. A `when` fires *at the instant its condition becomes true* and not otherwise — it is a
statement about a moment, not about a region.

---

## Act 1 — A model with no events

[RcCircuit → Events](hrw://load/RcCircuit/Events)

**Expected:** `condition_equations` **0**, `relations` **0**, `discrete_real_updates` **0**,
`discrete_valued_updates` **0**.

No `when`, nothing to watch. **The system is one set of equations for all time**, which is what
every earlier tour quietly assumed. Worth seeing so that the next three models' entries are
visibly not routine.

---

## Act 2 — A state that jumps

[BouncingBall → Events](hrw://load/BouncingBall/Events)

```modelica
when h <= 0 then
  reinit(v, -e * pre(v));
end when;
```

**Expected:** `condition_equations` **1**, `relations` **1**, **`discrete_real_updates` 1**,
`discrete_valued_updates` **0**.

**Expected:** the condition is recorded as `c[1]`, and the real update's target is **`v`**.

Three separate things are in that one clause, and the trace separates them:

- **`h <= 0`** is a **relation** — the expression whose sign the solver watches. It becomes the
  zero-crossing function.
- **`c[1]`** is the **condition equation** — the compiler gives the condition a name and a slot,
  because the solver needs to evaluate it as data rather than as source text.
- **`reinit(v, …)`** is a **discrete real update** — an instruction to *overwrite a continuous
  state* at the event instant.

**`reinit` is the unusual one.** Everywhere else in Modelica you write equations that are always
true; `reinit` says a state's value is *discontinuous here*. That is why it counts as
`discrete_real_updates` rather than as an equation: it is not a constraint, it is an assignment
that happens once.

**And `pre(v)` is what makes it well-defined.** At the instant of the bounce there are two
velocities — the one arriving and the one leaving. `pre(v)` is the arriving one. Without it,
`reinit(v, -e * v)` would be a circular statement about a single instant.

---

## Act 3 — A mode that flips, with no state jumping

[MotorWithBrake → Events](hrw://load/MotorWithBrake/Events)

A DC motor with a `when`/`elsewhen` pair watching a speed threshold, toggling
`Boolean overSpeed`.

**Expected:** `condition_equations` **2**, `relations` **2**, **`discrete_real_updates` 0**,
**`discrete_valued_updates` 1**, `zero_crossing_conditions` **1**.

**Expected:** the valued update's target is `overSpeed`.

**Nothing continuous jumps here.** No `reinit`, so `discrete_real_updates` is 0 — the states
carry on smoothly across the event. What changes is a **discrete variable**, and the equations
that reference it now mean something different afterwards.

**Two condition equations for one `when`/`elsewhen` pair.** Each branch gets its own slot,
because the solver must be able to tell which fired.

[GearWithBrake → Events](hrw://load/GearWithBrake/Events) shows the same shape scaled up:
**4 condition equations, 4 relations, 1 valued update** on `braking`.

---

## Act 4 — The three families, and why they are counted separately

| the trace calls it | Rumoca's symbol | what it holds |
|---|---|---|
| `condition_equations` | **`f_c`** | the named conditions `c[i]` the solver evaluates |
| `real_updates` | **`f_z`** | `reinit` — continuous states that jump |
| `valued_updates` | **`f_m`** | discrete variables that take a new value |

**That split is the phase's real output.** A solver needs the three at different moments: `f_c`
continuously, to detect a crossing; `f_z` and `f_m` **only at an event**, and never during a
normal step.

**Recall where you last saw them.** [`solve-lowering.md`](solve-lowering.md) Act 4 found a
`discrete` section holding `runtime_assignment_rhs`, `pre_modes` and `observation_refresh` — that
is this phase's output, lowered into code. `pre_modes` is where `pre(v)` lives.

---

## Act 5 — What the solver actually does at a bounce

Worth spelling out, because it is the reason all of the above is separated:

1. Integrate normally, evaluating `f_c` at each step.
2. Notice `h <= 0` changed sign **between** two steps.
3. **Reject the step** and search for the crossing time — bisection or interpolation — until the
   event instant is located to tolerance.
4. Stop there. Apply `f_z` and `f_m`: set `v := -e * pre(v)`.
5. **Restart the integrator.** History is invalid — a multistep method's stored past now
   describes a trajectory the system has left.

**Step 5 is the expensive one**, and it is why event count matters for performance. Each bounce
costs a restart, and `BouncingBall` bounces infinitely often as it settles — the classic
*Zeno* problem, where event times accumulate and a naive integrator stalls.

**Step 3 is why the relation is stored separately from the condition.** The solver interpolates
the *expression* `h`, not the Boolean; a Boolean has no sign to bracket.

---

## What comes next

Simulation — running the thing. Every artifact the pipeline built is now in the integrator's
hands: the layout from solve lowering, the initial vector, the residual, the zero-crossing
functions from this phase.

That is also where the compilation tours end and the *simulation* questions begin — step size,
order selection, stiffness, and why `MotorWithBrake` needs BDF rather than an explicit method.

---

## What this tour cannot check

**One number I cannot explain.** `RcCircuit` reports `zero_crossing_conditions` **1** while having
no `when` clause, no relations and no condition equations. That may be a default entry, a
tolerance-related crossing the runtime always carries, or a reporting artifact. **I do not know**,
and Act 1 deliberately quotes only the four counts I can account for. If the Events tab explains
it, that is worth telling me.

**Whether the `f_c` / `f_z` / `f_m` vocabulary helps.** It is Rumoca's own naming and it maps
cleanly onto what a solver needs — but three symbols introduced at once, in a phase that already
distinguishes conditions from relations from updates, may be one distinction too many for a first
pass.

**Whether Act 5 belongs in a tour about the compiler at all.** It describes what the *solver*
does with this phase's output, not what the phase does. It is here because the separation in Act
4 looks arbitrary until you see who consumes each piece — but it is the section most likely to be
in the wrong document.
