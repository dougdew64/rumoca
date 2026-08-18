# CartesianPendulum — the index-3 DAE Rumoca does not reduce

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
states fall to three that way, and the tour written from it could not show differentiation
because the model never needs it.

**`x² + y² = L²` cannot be substituted away.** It is nonlinear in two states at once, so the only
route to a solvable system is to differentiate it — twice — until the constraint speaks about
accelerations, which is where `lambda` finally appears. That is the whole idea of index
reduction, and this is the smallest model that forces it.

Four states and one constraint: small enough to differentiate by hand and check the pane against
your own arithmetic, which `Drivetrain` at 97 equations is not.

## What actually happens — Rumoca does not reduce it

**Every funnel step reports zero.** Nothing demoted, nothing differentiated, states 4 → 4:

```text
scalarize_equations                                 ok
demote_exact_alias_component_states                 0 demoted
demote_direct_assigned_states                       0 demoted
reduce_constrained_dummy_derivatives                0 demoted
index_reduce_missing_state_derivatives              0 demoted
demote_states_without_assignable_derivative_rows    0 demoted
eliminate_derivative_aliases                        ok
demote_states_without_retained_derivative_rows      0 demoted
expand_compound_derivatives                         ok
substitute_standalone_state_derivatives_in_non_ode_rows  0 rewritten
eliminate_trivial                                   0 eliminated
```

**And the system is left structurally singular**, with the diagnosis naming the physics exactly:

```text
structurally singular system: 4 matched out of 5 equations and 5 unknowns;
unmatched equations: f_x[4]; unmatched unknowns: lambda
```

`f_x[4]` **is** the constraint and `lambda` **is** its force. The constraint mentions no
derivative and no `lambda`, so nothing can pair with it; `lambda` appears only in the two force
equations, which are already matched to `der(vx)` and `der(vy)`.

Simulation then fails as an unreduced high-index DAE does — *"Step size is too small at time =
0.0000477"* — rather than with a message about index.

## What this teaches, and it is not "Rumoca is broken"

**Rumoca's index reduction is a set of pattern-based demotions, not general Pantelides.** Read
the step names: exact aliases, direct assignments, constrained dummy derivatives, states missing
a derivative row. Each targets a shape. The pendulum's constraint is none of those — all four
states *have* derivative rows, and the constraint is nonlinear in two of them at once.

So the corpus now spans the full range:

| specimen | constraint kind | what reduction does |
|---|---|---|
| `BouncingBall` | none | nothing, correctly — already index-1 |
| `BenchActuator` | alias | 1 differentiation, 4 states → 3 |
| `Drivetrain` | aliases at scale | 6 differentiations, 9 states → 3 |
| **`CartesianPendulum`** | **nonlinear** | **nothing — and the model does not run** |

**That last row is the one that makes the phase legible**, because it shows what the phase is
*for* by showing a system it cannot rescue.

## Provenance

Counts read from `trace/index_reduction.json` and `trace/structural.json`, generated
2026-08-18. The upstream question is filed in [`../../upstream-issues.md`](../../upstream-issues.md).

**Not yet round-tripped through System Modeler** — which is the obvious next check, since an
independent implementation reducing this model is what would turn "Rumoca implements a narrower
strategy" from a reading into an adjudicated fact.
