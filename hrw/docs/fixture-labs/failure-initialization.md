# Failure lab — Initialization, where too much information is the problem

<!-- kind: failure -->

**Specimen:** `OverInitRc` — `RcCircuit` with an `initial equation` section of two lines that
cannot both hold:

```modelica
initial equation
  C.v = 0;
  der(C.v) = 0;
```

The question to hold: every earlier failure lab showed something *missing* — a name, an
equation, a match. This one has too much. Where does the compiler notice, and what does it do
about it?

---

## Station 1 — The plan Rumoca lowered

[Load OverInitRc → Initialization](hrw://load/OverInitRc/Initialization)

**Expected:** Initialization is **not** flagged. Its note reads
`2 initialization row(s), 1 surplus check(s)`, and the tree's `rows` holds two entries:

| row | `role` | `solves` | `at` |
|---|---|---|---|
| 0 | `Solved` | `C.v` | line 13, `C.v = 0;` |
| 1 | `SurplusAlgebraicCheck` | — | line 14, `der(C.v) = 0;` |

**Falsified if:** the stage is red or amber, or the surplus row is not row 1.

Initialization solves a *different system* from the one that runs afterwards: at t = 0 the
derivatives are unknown too, and the initial equations plus `fixed = true` start attributes have
to determine every state. One state, `C.v`, has two conditions. Rumoca does not refuse at the
count. It uses one row to *solve* `C.v`, and keeps the other as a **surplus check** — a row whose
every coordinate is already determined elsewhere, so it can only be evaluated, never solved
(the tree's `roles_note` says this in Rumoca's words; MLS 3.6 §8.6).

---

## Station 2 — The run evaluates the check, and refuses

[Simulation](hrw://load/OverInitRc/Simulation)

Press the tab row's Run button.

**Expected:** the run fails at its first instant. The error names
`owner=surplus-check(…the row is an MLS 3.6 §8.6 consistency check the model does not satisfy)`
and `value=5.000000e1`. No trajectory is drawn.

Work the 50 by hand. Row 0 sets `C.v = 0`. With no voltage on the capacitor, the full 5 V of the
source is across the 100 Ω resistor, so the current is 5 / 100 = 0.05 A, and
`der(C.v) = i / C = 0.05 / 1e-3 = 50` V/s. Row 1 demands 0. The residual is the difference: 50.

So the compile succeeds — every tab up to Solve lowering has content — and the *model* is still
wrong. The conflict is only visible once numbers exist, and the first numbers exist at t = 0.

---

## Station 3 — Where the two conditions are kept

[Back to Initialization](hrw://load/OverInitRc/Initialization)

**Expected:** each row's `at` names its source line (13 and 14), and the summary lists one
`Solved` and one `SurplusAlgebraicCheck`.

That is what lets you act on Station 2's error: it gives a row number, and this tab turns the row
number into the line you wrote.

---

## What to bring back

- The Initialization tab is green, and the model cannot run. Should a surplus check colour the
  tab, even though it is a correct plan for an over-specified model?
- Station 2's error quotes a row number and a residual. Was the link from row 1 to line 14 easy
  to make, or should the Simulation pane name the line itself?
