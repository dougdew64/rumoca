# IncompatibleConnect — why this specimen exists

**Intent, not explanation.** *How* this specimen behaves is regenerated on demand from the
stage files. Prose about mechanism was retired 2026-07-29 (`docs/ideas.md` #42): Claude
reproduces it more accurately from the IR than from a stored copy nothing checks. What
**cannot** be regenerated is why someone made this file — the code never says what it is *for*.

**This specimen is deliberately broken. DO NOT FIX IT.** Repairing it deletes the test.

## Authored to trigger

`connect()` between two connectors whose member sets differ — `PinA` has `v` and a flow `i`,
`PinB` has only `v`.

Authored for the **flatten** failure path, since MLS §9.3 makes connecting type-incompatible
connectors an error.

**It does not land there, and that is now the point.** Rumoca accepts the `connect` and the model
fails later at structural analysis as singular — a misleading diagnosis for what is a wiring
error. System Modeler 15.0 **rejects** the same source (*"Incompatible types"*), so the specimen
is right and Rumoca has a bug: [`docs/upstream-issues.md`](../../upstream-issues.md) #2.

**Keep it exactly as it is.** When Rumoca is fixed it should start failing at flatten, and that
transition is the test.

## Where it has been used

*No recorded question yet.* When one is answered using this specimen, add a line here linking to its entry — the entry itself lives in [`question-ledger.md`](../../question-ledger.md), **not** here.
