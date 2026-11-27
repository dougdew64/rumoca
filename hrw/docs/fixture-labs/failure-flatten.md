# Failure lab — Flatten completes, and DAE construction checks the count

<!-- kind: failure -->

**Specimen:** `UnbalancedShaft` — `SingleInertia` with `tau` demoted from a parameter to an
unbound `Real` (and `J = 0.5` rather than `1.0`).

The question to hold: Modelica requires a model to have as many equations as unknowns
(MLS §4.9). That is arithmetic. Why is it not checked the moment the equations exist?

---

## Station 1 — The refusal

[Load UnbalancedShaft → DAE construction](hrw://load/UnbalancedShaft/Dae)

**Expected:** DAE construction has **Failed** (red), and its note begins
`unbalanced model: 2 equations, 3 unknowns (balance = -1)`, followed by a breakdown of the
unknowns and equations by kind (`state=2 algebraic=1 …`). The error code is `ED001`.

Three numbers, and the third is the useful one: balance = −1 means one more unknown than
equations. Negative is under-determined — the system does not pin down its own state. Positive
would be over-determined.

---

## Station 2 — Why it could not be checked sooner

[Resolve](hrw://load/UnbalancedShaft/Resolve)

**Expected:** Resolve is Ok. No flag, no error. So is Typecheck.

Nothing is wrong with any *name* or any *type* here. Every identifier resolves; every expression
typechecks. The model is only wrong as a system, and a system does not exist until the
component hierarchy is flattened into one equation set.

That is the answer to the question at the top. The count cannot be taken before flattening
because before flattening there is no single thing to count — the equations are scattered across
components, and connect statements have not yet been expanded into the equations they imply.
DAE construction, which turns the flat model into equations and unknowns, is the first phase
that holds both lists, and it counts them before anything else runs.

---

## Station 3 — What Flatten says

[Flatten](hrw://load/UnbalancedShaft/Flatten)

**Expected:** Flatten is not red and shows no tree. Its note reads
`flatten completed — the compile failed at DAE construction, whose result carries no flat
model. The DAE tab has the error.`

Flatten did its work; the compile's result simply carries no flat model to show, because the
call that runs both phases ended in DAE construction's error. So Flatten points you on rather
than repeating the error — exactly one tab is red, the one whose check refused.

*(Flatten once carried a second copy of this error, and the row read `Failed` twice.
`the_corpus_outcome_matrix_is_unchanged` now pins one `X`, at DAE construction.)*

[Simplify](hrw://load/UnbalancedShaft/Simplify)

**Expected:** Simplify, and every tab after it, reads `not reached (ToDae failed earlier)`.

---

## Station 4 — What one word did

The specimen is `SingleInertia` with `tau` changed from a `parameter` to an unbound `Real`.

[Load SingleInertia → DAE construction](hrw://load/SingleInertia/Dae)

**Expected:** SingleInertia's DAE tab has a tree, and its note reads
`2 state(s), 0 algebraic(s), 2 continuous equation(s)`.

A parameter is *given*; a variable must be *solved for*. Demoting one declaration added an
unknown — the `algebraic=1` in Station 1's breakdown — and nothing to determine it. The balance
check is the only thing standing between that edit and a solver that would have had to guess.

---

## What to bring back

- Does `balance = -1` tell you enough to find the offending declaration? It names no variable,
  though the breakdown says the extra unknown is algebraic.
- Flatten's note sends you to the DAE tab. Did you go, or did you read Flatten as the culprit?
