# The oracle — when Rumoca and System Modeler disagree

<!-- kind: calibration -->

**A lab that leaves HRW to settle a question HRW cannot settle.** Rumoca accepts a model
that System Modeler rejects, and the disagreement is the finding.

📐 = HRW · ⚙ = System Modeler

---

## 📐 Station 1 — A specimen built to fail at flatten

[IncompatibleConnect → Flatten](hrw://load/IncompatibleConnect/Flatten/Tree)

`connect(a, b)` between two connectors with **different member sets**: `PinA` has `v` and
a flow `i`, `PinB` has only `v`. MLS §9.3 makes that a type error.

**Expected:** flatten **succeeds**. The tab is not red, and the flat model exists.

## 📐 Station 2 — Where it actually fails

[Structural → Summary](hrw://stage/Structural/Summary)

**Expected:** *Singular*. The model fails here instead, reported as a structural
singularity — which is a misleading diagnosis for what is a wiring error. A reader
following it would go and study their equations when the problem is one `connect`.

## ⚙ Station 3 — Ask the other implementation

[Open IncompatibleConnect in System Modeler](hrw://systemmodeler/IncompatibleConnect)

**Expected:** System Modeler opens the file — **not** a text editor. Build the model.

**Expected result:** it is **rejected**, with a message naming the real problem:

```
Incompatible types. 'a' ...  'b' has type 'PinB'.
```

**That settles it.** The specimen is genuinely invalid, so Rumoca is the outlier: it has
a validation that did not fire (`validate_type_compatibility` in
`rumoca-phase-flatten/src/connections/mod.rs`). Recorded as
[`docs/upstream-issues.md`](hrw://doc/upstream-issues.md) #2.

## 📐 Station 4 — Why this could not be settled inside HRW

[Structural → Incidence](hrw://stage/Structural/Incidence)

**Expected:** a perfectly ordinary incidence matrix, one row short of a full matching.

Nothing here says *why*. From inside HRW there are two indistinguishable explanations —
the model is wrong, or Rumoca is wrong — and choosing between them is not a judgement the
tool can make about itself. **An independent implementation is the only arbiter**, which
is why `ideas.md` #43 calls the oracle a *requirement* of diagnostic mode rather than a
convenience.

---

## The rule this lab exists to make concrete

When an authored specimen behaves unexpectedly, the tempting move is to assume the
specimen is wrong. That reads as humility and **systematically destroys findings**: every
Rumoca bug then looks like a bad specimen. Ask the oracle first.

| System Modeler | Rumoca | Reading |
|---|---|---|
| rejects | accepts | **Rumoca bug** — file it |
| accepts | rejects | **Rumoca bug**, the other way — a valid model refused |
| accepts | accepts | the specimen is valid and **tests nothing** |
| rejects | rejects | a **good failure specimen** — compare the two diagnoses |

## What this cannot check

Whether System Modeler is installed, and whether its build message still reads as quoted
above. `.mo` is associated with `ModelCenter.exe` by the System Modeler installer
(verified 2026-07-30), so HRW hands the file over and never learns where it lives.
