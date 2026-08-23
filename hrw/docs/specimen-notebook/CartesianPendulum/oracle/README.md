# The pendulum's independent reference trajectory

**What this is:** what Wolfram System Modeler 15.0 computes for
[`CartesianPendulum.mo`](../../../../specimens/CartesianPendulum.mo), captured 2026-08-23 while a
machine with System Modeler was available.

**Why it exists:** it turns `docs/ideas.md` **#83** — *implement general Pantelides for Rumoca* —
from a project into a **red test**. Rumoca leaves the pendulum at four states and structurally
singular; System Modeler reduces it to two and simulates it. The day #83 lands, Rumoca's own
trajectory can be compared against this file and the answer is binary.

**Why now rather than then:** System Modeler is on one machine and the reduction was measured with
the session warm. Later it is a reinstall, a re-derivation, and a set of numbers nobody remembers.
**The data is cheap today and expensive on the day it is needed** — which is the whole argument for
capturing a continuation's acceptance criterion in advance.

## What is in the file

`system-modeler-15.0.json` — 101 samples at 0.1 s over 0 → 10 s of `x`, `y`, `vx`, `vy`, `lambda`,
plus provenance (System Modeler and WSMLink versions, solver, tolerance) and three invariants:

| invariant | value |
|---|---|
| states after reduction | **2** (dynamic state selection; `$dynState` `set0={vy}`, `set1={y}`) |
| `lambda` peak | **29.4293**, against a hand-computed *m*(*g* + *v*²/*L*) = **29.43** |
| max constraint residual, ǀ*x*²+*y*²−*L*²ǀ | **1.23 × 10⁻⁴** |

## How a test should use it — and the trap to avoid

**Do not demand pointwise agreement tight enough to encode System Modeler's particular numerical
choices as truth.** Its own constraint residual is 1.23 × 10⁻⁴; a different-but-correct reduction
will drift differently. A test that pins this trajectory to machine precision would fail a correct
implementation, which is worse than no test.

Charter §4.3 already fixes the protocol: **relative error on state trajectories**, at a stated
tolerance, with identical initial conditions.

**The strongest assertions are the ones no implementation choice can move:**

- **the state count is 2** — one degree of freedom, and Rumoca's four is the defect
- **`lambda` peaks at the bottom at *m*(*g* + *v*²/*L*)** — derivable by hand, independent of both tools
- **the constraint holds** to a stated tolerance throughout

Those three are worth more than the 101 samples, because they cannot be wrong. The samples are the
differential half, and `docs/upstream-strategy.md` rates differential testing against an independent
implementation as the rarest thing this project can bring to the Rumoca maintainers.
