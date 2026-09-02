# Failure lab — Initialization, where too much information is the problem

<!-- kind: failure -->

**Specimens:** `OverInitRc` and `RotationalInertia`. Two ways the t=0 problem goes wrong, and
they are opposites.

The question to hold: every earlier lab showed something *missing* — a name, an equation, a
match. This phase can fail because there is too much.

---

## Station 1 — More conditions than states

[Load OverInitRc → Initialization](hrw://load/OverInitRc/Initialization)

**Expected:** flagged, and the note begins `OVER-DETERMINED initialization:` with counts of
explicit initial conditions against states, and a surplus.

Initialization solves a *different system* from the one that runs afterwards: at t=0 the
derivatives are unknown too, and the initial equations plus `fixed=true` start attributes have to
determine every state exactly once.

Give a state two conditions and they may disagree. Nothing checks that they agree — the count
is what is checkable, and a surplus is the signal.

---

## Station 2 — The other direction

[Load RotationalInertia → Initialization](hrw://load/RotationalInertia/Initialization)

**Expected:** flagged, with a note reading `IC planning failed: structurally singular system`.

Same phase, opposite complaint. Here the initialization system could not be *solved* — its own
matching is deficient, in the same sense `TwiceDefined` was in `failure-structural.md`, but
applied to the t=0 system rather than the running one.

This specimen is not marked DELIBERATELY BROKEN. It is a specimen we kept for other reasons
that turns out to exercise this path — which is worth knowing, because it means the condition
arises from ordinary modelling rather than from contrived breakage.

---

## Station 3 — Everything downstream still runs

[Solve lowering](hrw://load/OverInitRc/SolveLowering)

**Expected:** Solve lowering has content, for both specimens.

By now this should be unsurprising: initialization is flagged, not failed, and the pipeline
continues. The model would reach a solver, and the solver would start from an initial state
nobody verified.

That is the practical stake of this phase. A simulation that starts wrong produces a
trajectory that is smooth, plausible, and false — no error anywhere, just the wrong answer.

---

## Station 4 — The determinacy verdict

[Back to Initialization](hrw://load/OverInitRc/Initialization)

**Expected:** the stage tree contains a `determinacy` object beside the plan.

That is the compiler's own summary of whether the t=0 system is well-posed, and it is the field to
read first on any model you are unsure of. It is one node in a large tree, which is a fair
criticism of this pane — the most important fact is not the most prominent thing on screen.

---

## What to bring back

- Should `determinacy` be lifted out of the tree into the stage note, the way the balance is on
  the DAE tab?
- Both specimens are flagged. Given stop 3 — a false trajectory with no error — is flagging
  enough, or should an over-determined initialization stop the pipeline?
