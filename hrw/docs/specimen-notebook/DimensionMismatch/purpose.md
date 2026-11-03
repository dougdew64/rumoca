# DimensionMismatch — why this specimen exists

**Intent, not explanation.** *How* this specimen behaves is regenerated on demand from the
stage files. Prose about mechanism was retired 2026-07-29 (`docs/ideas.md` #42): Claude
reproduces it more accurately from the IR than from a stored copy nothing checks. What
**cannot** be regenerated is why someone made this file — the code never says what it is *for*.

**This specimen is deliberately broken. DO NOT FIX IT.** Repairing it deletes the test.

## Authored to trigger

An equation assigning a `[3]` array to a `[2]` one.

Authored for the **typecheck** failure path, and the cleanest of the diagnostic specimens: it
produces exactly one diagnostic (`ET002`) with one label, on one line.

## Where it has been used

*No recorded question yet.* When one is answered using this specimen, add a line here linking to its entry — the entry itself lives in [`question-ledger.md`](../../question-ledger.md), **not** here.
