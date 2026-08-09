# Index reduction — when nine states are really three

**Walk [`matching.md`](matching.md) first**, and Act 3 of it especially. That act showed a
*structurally singular* system as a **failure**: `CapacitorLoop` could not be matched, and the
compiler reported rank deficiency.

**This tour shows the same failure as an ordinary, expected thing** — and shows what fixes it.
`Drivetrain` is structurally singular too. It is also a perfectly good model of a perfectly
ordinary machine.

> Every number below is read from the specimens' generated traces.

---

## The problem this phase exists to solve

A modeller writes `der(x)` and means *"x is a state"*. Nine `der`s means nine states, and a
solver wants nine initial conditions and nine equations of motion.

**Physics disagrees.** Bolt two shafts together through an ideal gear and their angles are not
independent — one is a fixed multiple of the other. You wrote `der(rotor.phi)` and
`der(load.phi)`, but the machine has one degree of freedom there, not two.

That relationship, `rotor.phi = ratio * load.phi`, is a **constraint hiding among the
equations**. A DAE carrying such constraints has **index greater than 1**, and a plain ODE solver
cannot integrate it: it would need the constraint force — the torque the gear teeth exert — and
no equation gives it directly.

**Index reduction finds the constraints and removes the redundant states.**

---

## Act 1 — A model that needs nothing

[BouncingBall → Index Reduction → Summary](hrw://load/BouncingBall/IndexReduction/Summary)

Height and velocity, `der(h) = v` and `der(v) = -g`.

**Expected:** states **2 before, 2 after**. Nothing demoted.

**Expected:** every step of the reduction reports `0 demoted`.

Two `der`s, two genuine degrees of freedom, no constraint between them. **This is index 1**, and
the phase correctly does nothing. Worth seeing first so the later models' activity is visibly
*not* routine.

---

## Act 2 — One state that was not a state

[BenchActuator → Index Reduction → Summary](hrw://load/BenchActuator/IndexReduction/Summary)

A motor driving a load through an inductor and an EMF coupling.

**Expected:** states **4 before, 3 after**, with exactly one demoted: **`emf.phi`**.

The electromotive-force element's angle is rigidly tied to the rotor it sits on. It appeared as a
state because someone wrote its derivative; it is not one, because knowing the rotor's angle
determines it.

**Expected:** the step that did it is `reduce_constrained_dummy_derivatives`, reporting
`1 demoted`, and every other step reports `0`.

---

## Act 3 — Nine states, three degrees of freedom

[Drivetrain → Index Reduction → Summary](hrw://load/Drivetrain/IndexReduction/Summary)

A motor, a gear, a shaft, a translating load and a compliant mount.

**Expected:** states **9 before, 3 after**. Six demoted:

`emf.phi`, `rotor.phi`, `rotor.w`, `shaft.phi`, `load.s`, `load.v`

**Expected:** the three survivors are `L.i`, `shaft.w`, `mount.s_rel`.

**Read that list as physics.** What survived is the inductor current, the shaft speed, and the
mount's deflection — one electrical state, one rotational state, one compliance. Everything else
in the drivetrain is *rigidly geared to* one of those three. The machine has three independent
ways to store energy, and six of the nine `der`s were describing the same three.

**Notice `rotor.phi` and `rotor.w` were both demoted.** A position *and* its velocity went
together, which is what an ideal gear does: it removes a whole second-order degree of freedom,
not half of one.

---

## Act 4 — Why the previous phase failed, and why that was correct

Here is `Drivetrain` **before** reduction:

[Drivetrain → Structural → Summary](hrw://load/Drivetrain/Structural/Summary)

**Expected:** a structural error reading *"structurally singular system: 93 matched out of 97
equations and 97 unknowns"* — a rank deficiency of **4**.

**Expected:** the unmatched unknowns are `emf.p.v`, `shaft.flange_a.tau`, `load.flange_a.f`,
`wall.flange.f`.

**Look at what those four are.** A voltage at a pin, a torque at a flange, a force at a flange, a
force at a wall. **They are the constraint forces** — precisely the quantities a constrained
mechanism cannot solve for algebraically. That is not a coincidence and it is not a modelling
error; it is the signature of a high-index DAE, and the same signature every textbook uses.

Now the same model **after** reduction: **20 equations, 20 unknowns, 12 blocks, no singularity.**

| | equations | unknowns | matched | outcome |
|---|---|---|---|---|
| before | 97 | 97 | 93 | **singular**, deficiency 4 |
| after | 20 | 20 | 20 | 12 blocks, 1 coupled |

**So `matching.md` Act 3's failure has two completely different meanings**, and telling them
apart is the point of this tour:

- **`CapacitorLoop`** is singular because the *model* is wrong — a capacitor across an ideal
  source over-constrains its own voltage. No reduction saves it.
- **`Drivetrain`** is singular because the *DAE has index > 1* — and index reduction is the
  standard, expected cure.

**A rank deficiency is a question, not a verdict.**

---

## Act 5 — What Rumoca actually does, which is not what the textbook name suggests

**A correction worth carrying, and it was found by reading the traces rather than the
literature.**

The textbook algorithms here are **Pantelides** — repeatedly differentiate constraint equations
until the system is index 1 — and **dummy derivatives**, which selects which differentiated
variables become algebraic. Any course will teach it that way.

**Rumoca's traces show `differentiated_rows` empty on all 21 specimens.** The reduction is
achieved by **demoting states**, not by differentiating equations, at least on every model in
this repository.

The phase is a **funnel of ten named steps**, each a different reason a state might turn out not
to be one:

| step | what it looks for |
|---|---|
| `demote_exact_alias_component_states` | a state that is literally an alias of another |
| `demote_direct_assigned_states` | a state given directly by an equation |
| **`reduce_constrained_dummy_derivatives`** | **states tied by a constraint — the one that fires here** |
| `index_reduce_missing_state_derivatives` | a state whose derivative never appears |
| `demote_states_without_assignable_derivative_rows` | a `der` with no equation that can produce it |
| `eliminate_derivative_aliases` | duplicate derivative references |
| `demote_states_without_retained_derivative_rows` | derivatives dropped by earlier steps |
| `expand_compound_derivatives` | `der` of an expression |
| `substitute_standalone_state_derivatives_in_non_ode_rows` | `der(x)` used as an ordinary term |
| `eliminate_trivial` | equations reduced to nothing |

**Expected:** on all three specimens above, the only step reporting a demotion is
`reduce_constrained_dummy_derivatives` — 1, 5 and 6 respectively for `BenchActuator`,
`GearWithBrake` and `Drivetrain`.

**Expected:** `eliminate_trivial` reports large numbers — 41, 33 and 77 — because collapsing the
connection equations is what takes 97 equations down to 20.

**And `funnel_completed` is `true` with `stopped_at` null**, which is the phase saying it ran
every step rather than bailing early. A model that defeated it would say so there.

---

## Act 6 — The linear algebra, in one paragraph

The nine apparent states are a vector; the constraints say that vector is confined to a
**3-dimensional subspace**. Six of the nine coordinates are determined by the other three, so the
constraint Jacobian has rank 6 and its null space has dimension 3.

**The three surviving states are a basis for that null space** — a choice of coordinates on the
manifold the system actually moves in. `L.i`, `shaft.w`, `mount.s_rel` are not *the* answer;
they are *an* answer, and a different valid choice would give a different, equally correct model.

That "any basis will do, but you must pick one" structure is exactly what makes the dummy
derivative method a *selection* problem, and it is why two compilers can reduce the same model to
two different state vectors and both be right.

---

## What comes next in the chain

Reduction produced a 20-equation system with one coupled block. Everything you learned in
[`blt-ordering.md`](blt-ordering.md) and [`tearing.md`](tearing.md) now applies to it — and
`Drivetrain`'s post-reduction blocks are the realistic version of the tiny examples those tours
used.

---

## What this tour cannot check

**Whether Act 4 lands as the reframe it is meant to be.** "The same error means two opposite
things" is the most useful idea here, and it depends on `matching.md` Act 3 being fresh in mind.
If it isn't, this reads as a list of numbers.

**Whether Act 5's correction is welcome or disorienting.** Being told the textbook name and then
told the implementation does something else may be exactly the honesty that helps, or may be
confusing before the textbook version is understood at all. It is placed fifth for that reason,
and might belong last — or in a footnote.

**Whether the Index Reduction tab shows the before/after split usefully.** `architecture.md` says
comparative views render Before/After there, which is the natural way to see 97 equations become
20 — but whether that comparison is legible for a model this size is exactly what no test reaches.
