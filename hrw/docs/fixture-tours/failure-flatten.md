# Failure tour — Flatten, where the count is checked

**Specimen:** `UnbalancedShaft` — `SingleInertia` with one line changed.

**The question to hold:** Modelica requires a model to have as many equations as unknowns
(MLS §4.9). That is arithmetic. Why is it not checked the moment the equations exist?

---

## Stop 1 — The refusal

[Load UnbalancedShaft → Flatten](hrw://load/UnbalancedShaft/Flatten)

**Expected:** Flatten has **Failed**, and the note reads
`unbalanced model: 2 equations, 3 unknowns (balance = -1)`.

Three numbers, and the third is the useful one: **balance = −1** means one more unknown than
equations. Negative is under-determined — the system does not pin down its own state. Positive
would be over-determined.

---

## Stop 2 — Why it could not be checked sooner

[Resolve](hrw://load/UnbalancedShaft/Resolve)

**Expected:** Resolve is **Ok**. No flag, no error. So is Typecheck.

Nothing is wrong with any *name* or any *type* here. Every identifier resolves; every expression
typechecks. The model is only wrong **as a system**, and a system does not exist until the
component hierarchy is flattened into one equation set.

**That is the answer to the question at the top.** The count cannot be taken before flattening
because before flattening there is no single thing to count — the equations are scattered across
components, and connect statements have not yet been expanded into the equations they imply.

---

## Stop 3 — The phase that owns the error, and the phase that reports it

[DAE construction](hrw://load/UnbalancedShaft/Dae)

**Expected:** the DAE tab **also** shows this failure, in its own words, rather than being blank.

The balance check belongs to DAE construction — its error code is `rumoca::todae::ED001`. But
Flatten reports it too, because `flatten_stage` has carried the `ToDae` error since before the
DAE tab existed.

**That duplication is deliberate**, decided 2026-08-03: a learner who opens the DAE tab of a model
with no DAE and finds nothing has hit a dead end. Two tabs explaining the same stop is redundant;
one tab silently blank is worse.

**Notice what this means for reading the failure map:** `UnbalancedShaft` reports `Failed` at two
stages. The first is not necessarily the culprit.

---

## Stop 4 — What one line did

The specimen is `SingleInertia` with `tau` changed from a `parameter` to an unbound `Real`.

[Load SingleInertia → Flatten](hrw://load/SingleInertia/Flatten)

**Expected:** SingleInertia's Flatten tab has a **tree**, and its DAE tab reports
`1 state(s), 1 algebraic(s), 2 continuous equation(s)`.

A parameter is *given*; a variable must be *solved for*. Promoting one word added an unknown and
nothing to determine it. **The balance check is the only thing standing between that edit and a
solver that would have had to guess.**

---

## What to bring back

- Does `balance = -1` tell you enough to find the offending declaration? It names no variable.
- Two tabs report this failure. Is the duplication helpful, or did you read it as two problems?
