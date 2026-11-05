# CartesianPendulum — the index-3 DAE, and how Rumoca reduces it

**A point mass on a rigid rod, in Cartesian coordinates.** Five equations, five unknowns, and
the canonical example every treatment of index reduction opens with.

```modelica
der(x) = vx;
der(y) = vy;
m * der(vx) = -lambda * x;
m * der(vy) = -lambda * y - m * g;
x ^ 2 + y ^ 2 = L ^ 2;          // the constraint
```

## Why this specimen exists

**Every other constraint in the corpus is an *alias*** — one variable equal to another times a
constant — which substitution removes without differentiating anything. `Drivetrain`'s nine
states fall to three that way, and the lab written from it could not show differentiation
because the model never needs it.

**`x² + y² = L²` cannot be substituted away.** It is nonlinear in two states at once, so the only
route to a solvable system is to differentiate it — twice — until the constraint speaks about
accelerations, which is where `lambda` finally appears. That is the whole idea of index
reduction, and this is the smallest model that forces it.

Four states and one constraint: small enough to differentiate by hand and check the pane against
your own arithmetic, which `Drivetrain` at 97 equations is not.

## What actually happens — one phase declines and the next succeeds

**Measured on v0.10.4, 2026-11-04.** Through v0.10.0 this section read *"Rumoca does not reduce
it"*, and that was true: the model failed solve lowering and did not run. It runs now, and the
route is worth following, because **two different components give two different answers and both
are correct.**

**Structural analysis reports the deficiency, and names the physics exactly:**

```text
structurally singular system: 4 matched out of 5 equations and 5 unknowns;
unmatched equations: f_x[4]; unmatched unknowns: lambda
```

`f_x[4]` **is** the constraint and `lambda` **is** its force. The constraint mentions no
derivative and no `lambda`, so nothing can pair with it; `lambda` appears only in the two force
equations, which are already matched to `der(vx)` and `der(vy)`.

**The reducer then finds the constraint, tries it, and refuses:**

```text
candidates  direct    direct_admissible    0 discovered
candidates  direct    direct_conditional   0 discovered
candidates  holonomic holonomic            1 discovered
attempt     holonomic                      would_create_vacuous_residual
outcome: singular, admitted false, residue 2
```

**`would_create_vacuous_residual` is the whole of why this model is hard.** Differentiate
`x² + y² = L²` and substitute it back and you are left with `0 = 0` — a row that constrains
nothing. The Direct lane finds nothing in either proof group, correctly: a nonlinear constraint in
two variables is not a direct definition of anything.

So the **Index Reduction payload is a refusal, and the model simulates anyway.** That is not a
contradiction and it is the most instructive thing this specimen now does.

*The tab itself says so as of 2026-11-05: its note reads* "this reducer's search stayed singular;
solve lowering selected 2 independent coordinate(s) on the constraint manifold and lowered the
model", *and the stage's outcome is `Ok`. The rounds and the refusal are still there in the pane —
what changed is that the stage no longer passes one phase's refusal off as the compiler's verdict.*

**Solve lowering carries it, by changing coordinates instead of differentiating in place.**
`rumoca-phase-solve`'s state selection — *"static independent coordinates for structurally
constrained state manifolds"* — constructs formal derivatives and picks a chart on the manifold.
The lowered system has exactly **one** `State` declaration:

```text
$state_coordinates     State      shape [2]     <- the two integrated scalars
  [1] = y                                       <- and this is what they ARE
  [2] = der(y)
x, y, vx, vy, lambda   Algebraic                <- all reconstructed from the chart
$formal_derivative.2.x, $formal_derivative.2.y  <- the constraint, differentiated twice
```

**The chart is `{y, der(y)}`** — read from the solver's own `state_coordinate` metadata, not
inferred. System Modeler selects `$dynState` sets over `{y}` and `{vy}`, which is the same pair
under the other tool's spelling. **Two independent implementations, given a nonlinear constraint
and no hint, pick the same two coordinates.**

**Read that table slowly, because it is the answer to "what is index reduction".** The solver does
not integrate `x` and `y` at all. It integrates two abstract coordinates on the circle of radius
`L`, and reconstructs the Cartesian positions, the velocities and the rod tension from them. The
order-**2** formal derivatives are the constraint differentiated twice — the step that takes index
3 to index 1, visible in the lowered layout.

**And it is exact, not stabilized.** `x² + y² = L²` holds to **4.6e-12** at the final sample after
a full swing, and `lambda` peaks at **29.4303**, against the hand-computed rod tension
*m*(*g* + *v*²/*L*) = 3*mg* = **29.43** for a mass released from horizontal.

## What this teaches, and it is not "Rumoca is broken"

**A phase can report failure while the compiler succeeds.** The reducer's refusal is a true
statement about one search, and reading it as the compiler's verdict is a mistake HRW itself made
— its Index Reduction banner called this model *Singular* in red on the day the model began
simulating correctly. If a pane and a trajectory disagree, the trajectory is the arbiter.

**And it made the same mistake twice, which is the sharper lesson.** The banner was fixed on
2026-11-04; the **stage outcome underneath it was not**, so for a day this specimen showed *Reduced
by state selection* in green under a tab painted red. Doug found it by loading the model. One fact
had two homes — the pane and the label — and fixing the home you are looking at feels like fixing
the fact. Both now read the same `state_scalar_count` from solve lowering, which is the only
arrangement in which they cannot drift apart again.

**Reduction is not the only way down from high index.** Differentiate-and-substitute is the
textbook route and it is the one that fails here. Choosing independent coordinates on the
constraint manifold reaches index 1 without ever rewriting the constraint — the same destination
by a different road, and the road a commercial tool takes too.

So the corpus now spans the full range:

| specimen | constraint kind | what reduction does |
|---|---|---|
| `BouncingBall` | none | nothing, correctly — already index-1 |
| `BenchActuator` | alias | 1 differentiation, 4 states → 3 |
| `Drivetrain` | aliases at scale | 6 differentiations, 9 states → 3 |
| **`CartesianPendulum`** | **nonlinear** | **the reducer refuses; state selection takes a chart, 4 states → 2** |

**That last row is the one that makes the phase legible**, because it is the only specimen where
the two routes separate: every other row is rescued by the reducer, and this one is not rescued by
it at all.

## Provenance

Counts read from `trace/index_reduction.json`, `trace/solve_lowering.json` and
`trace/simulation.json`, regenerated against **v0.10.4** on 2026-11-04. The lowered roles come from
`solve_layout.variable_declarations`; the constraint residual and the `lambda` peak from the
simulation's own samples.

**Round-tripped through System Modeler 2026-08-22, and the two tools now AGREE.** Wolfram System
Modeler **15.0** loads this file unmodified, simulates it 0 → 10 s cleanly, and reduces it to
**two** states by dynamic state selection (`$dynState` sets over `{vy}` and `{y}`). A planar
pendulum has one degree of freedom, so two is the right answer — and v0.10.4 reaches the same two,
by the same kind of route **and on the same coordinates**: its chart is `{y, der(y)}`, against
System Modeler's `{y}` and `{vy}`.

*(Through v0.10.0 this paragraph ended "against Rumoca's four". That was the disagreement this
specimen was authored to hold, and it is closed.)*

The oracle's `lambda` peaks at **29.4293** at the bottom of the swing, against a hand-computed rod
tension *m*(*g* + *v*²/*L*) = **29.43** — so the comparison rests on a number that can be checked
without either tool. The full record, the version string, and the three things this run does *not*
establish are in [`../../upstream-issues.md`](../../upstream-issues.md).

**"Rumoca implements a narrower strategy" was an adjudicated fact, and v0.10.4 retired it.** The
reducer *is* still narrower — it refuses this constraint — but the compiler is not, because state
selection reaches index 1 without it.

**Charter §4.3 now passes for this specimen, and that is a change of status worth recording.** It
was authored to hold a disagreement: it ran in one toolchain and not the other. Both run it now,
both reduce to two states, and both put the rod tension at 29.43. **What the specimen is for has
changed with it** — it is no longer evidence of a gap, it is the corpus's worked example of index
reduction succeeding by a route the textbook does not take first.
