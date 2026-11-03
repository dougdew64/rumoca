# UndefinedRef — why this specimen exists

**Intent, not explanation.** *How* this specimen behaves is regenerated on demand from the
stage files. Prose about mechanism was retired 2026-07-29 (`docs/ideas.md` #42): Claude
reproduces it more accurately from the IR than from a stored copy nothing checks. What
**cannot** be regenerated is why someone made this file — the code never says what it is *for*.

**This specimen is deliberately broken. DO NOT FIX IT.** Repairing it deletes the test.

## Authored to trigger

A model referencing `missingGain`, which is never declared.

Authored for the **resolve** failure path. It is what proved the resolve payload was ~99% noise:
39 concatenated items, 38 of them MSL deprecation warnings, the model's own error last. Filtering
by diagnostic *severity* now reduces that to one error with a line number.

## Where it has been used

*No recorded question yet.* When one is answered using this specimen, add a line here linking to its entry — the entry itself lives in [`question-ledger.md`](../../question-ledger.md), **not** here.
