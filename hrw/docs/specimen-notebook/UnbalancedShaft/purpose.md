# UnbalancedShaft — why this specimen exists

**Intent, not explanation.** *How* this specimen behaves is regenerated on demand from the
stage files. Prose about mechanism was retired 2026-07-29 (`docs/ideas.md` #42): Claude
reproduces it more accurately from the IR than from a stored copy nothing checks. What
**cannot** be regenerated is why someone made this file — the code never says what it is *for*.

**This specimen is deliberately broken. DO NOT FIX IT.** Repairing it deletes the test.

## Authored to trigger

A shaft with `tau` declared and no equation to determine it.

Authored for the **DAE-construction** failure path — the most common Modelica authoring error
there is: declare a variable, forget its equation. It exists because auditing that path
(`docs/ideas.md` #45) needed a model that reaches it.

**Landed where it was aimed**, and taught something in passing: Rumoca's balance check catches a
missing equation *before* structural analysis runs, which is earlier and more specific than a
structural singularity. Claude had expected a structural failure and was wrong.

## Where it has been used

*No recorded question yet.* When one is answered using this specimen, add a line here linking to its entry — the entry itself lives in [`question-ledger.md`](../../question-ledger.md), **not** here.
