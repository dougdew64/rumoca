# Failure tour — Resolve, where a name is looked up and the answer is recorded

**Specimens:** `UndefinedRef` and `MissingComponentClass`. Walk them together; neither is worth
much alone.

**Walk the Parse failure tour first** — or jump straight to the stop that draws the distinction:
[failure-parse, stop 4](hrw://tour/failure-parse/stop/stop-4-the-distinction-this-specimen-anchors).
It establishes `Failed` versus `Flagged`, and this tour is the
first case of the second kind.

**The question to hold:** a name that does not exist is the simplest possible error. Why does it
take *two* phases to deal with it?

---

## Stop 1 — The error is found here

[Load UndefinedRef → Resolve](hrw://load/UndefinedRef/Resolve)

**Expected:** Resolve is **flagged**, and its note contains `unresolved component reference`. The
stage still has a **tree below the note** — resolution produced something.

That is the whole lesson of this tour in one pane. Rumoca found the problem, wrote it down, and
kept the partial result. Compare `UnclosedModel`, where Parse had nothing to hand on.

---

## Stop 2 — And the compile stops somewhere else

[Flatten](hrw://load/UndefinedRef/Flatten)

**Expected:** Flatten has **Failed** — no tree — and says the reachable-closure pipeline produced
no model.

**So the phase that reports is not the phase that stops.** Resolve knew at stop 1; the pipeline
continued through Instantiate and Typecheck anyway, and gave up when flattening needed a name
that was never bound.

This is worth sitting with, because it is the shape of most errors in this compiler: **the
diagnosis and the halt are in different places**, and the log is what connects them.

---

## Stop 3 — Which *kind* of name was missing

[Load MissingComponentClass → Resolve](hrw://load/MissingComponentClass/Resolve)

**Expected:** flagged again, but the note reads `unresolved type reference` — **type**, not
**component**.

The two specimens differ by one line:

```modelica
y = missingGain * time;   // UndefinedRef        — a missing VARIABLE
NoSuchBlock part;         // MissingComponentClass — a missing CLASS
```

Modelica looks up classes and components by different rules, so the compiler can say which kind
it failed to find. **Everything else about the two compiles is identical** — same flagged stage,
same failing stage, same message at Flatten.

**This tour exists because that was not obvious.** The specimen's first draft asserted a missing
*class* would stop at Instantiate — a different phase — and that was wrong. It was caught by
`cargo run -p hrw --example failure_map` before this tour was written, which is the only reason
you are not reading a confident false claim right now.

---

## Stop 4 — Read it in the log

Click the **Log** toggle above the stage tabs.

**Expected:** brackets for `Parse`, `Resolve`, and `Rumoca compile` all opening and closing
normally, with the failure appearing inside the compile. **No bracket is missing** — every phase
up to the stop actually ran.

Contrast `UnclosedModel`, where later brackets do not appear at all.

---

## What to bring back

- Does the Resolve tab make it clear that its tree is **partial**? It is, and the pane may not
  say so loudly enough.
- Would you rather the *stop* be attributed to Resolve, since that is where the cause is? That
  is a real design question and the answer is not obvious — the compiler's own structure says
  Flatten.
