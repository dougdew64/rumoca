# Failure lab — Resolve, where a name is looked up and the compile stops

<!-- kind: failure -->

**Specimens:** `UndefinedRef` and `MissingComponentClass`. Run them together; neither is worth
much alone.

Run the Parse failure lab first — or jump straight to the station that draws the distinction:
[failure-parse, station 4](hrw://lab/failure-parse/station/station-4-the-distinction-this-specimen-anchors).
It establishes `Failed` versus `Flagged`. This lab is the second `Failed`: a halt one phase
later than Parse's, for a different reason.

The question to hold: a name that does not exist is the simplest possible error. Where does the
compiler find it, and what does every tab after that point say?

---

## Station 1 — The error is found here, and the compile stops here

[Load UndefinedRef → Resolve](hrw://load/UndefinedRef/Resolve)

**Expected:** Resolve has **Failed** (red), and its note reads
`Resolve errors: unresolved component reference: 'missingGain'`. The error panel comes first,
and its diagnostic `ER002` points at line 9, `y = missingGain * time;`. Below it is a tree:
the class `UndefinedRef` as Rumoca resolved it.

**Why there is a tree at all, when Resolve failed.** The compile resolves in a *recovery* mode:
it keeps going past an error like this one, so the rest of the class still resolves, and it
reports the error rather than throwing the work away. The tree is that recovered class, from
the same resolve the compile makes. It is still a failure: the compile stops here, which is why
the tab is red and not amber. *(Until 2026-11-28 this station said there was no tree. HRW had
been trying to show one from a different resolve mode, which never existed in its session, so
the tab showed the error alone.)*

Rumoca found the problem, wrote it down with its location, and went no further. Compare
`UnclosedModel`, where Parse had nothing to hand on: here Parse succeeded, and Resolve is where
the model stopped being a model.

---

## Station 2 — What every later tab says

[Instantiate](hrw://load/UndefinedRef/Instantiate)

**Expected:** Instantiate says `not reached (the compile stopped before instantiate)`, and so
does Typecheck. Flatten and every tab after it say
`the reachable-closure pipeline produced no result for this model`. None of them is red.

So the phase that reports *is* the phase that stops, and the tab colours say so: one red tab,
at Resolve, and grey after it. **Falsified if:** any tab after Resolve is red, or shows a tree.

*(Until the v0.10.0 rebase this lab taught the opposite — Resolve merely flagged, and the
compile continuing until Flatten gave up. That was HRW painting a halt as a flag; the outcome
matrix `the_corpus_outcome_matrix_is_unchanged` now pins Resolve as the one `X` on this row.)*

---

## Station 3 — Which *kind* of name was missing

[Load MissingComponentClass → Resolve](hrw://load/MissingComponentClass/Resolve)

**Expected:** Failed again, but the note reads `unresolved type reference: 'NoSuchBlock'` —
type, not component.

The two specimens differ by one line:

```modelica
y = missingGain * time;   // UndefinedRef        — a missing VARIABLE
NoSuchBlock part;         // MissingComponentClass — a missing CLASS
```

Modelica looks up classes and components by different rules, so the compiler can say which kind
it failed to find. Everything else about the two compiles is identical — the same failed stage,
the same notes on every tab after it.

This lab exists because that was not obvious. The specimen's first draft asserted a missing
*class* would stop at Instantiate — a different phase — and that was wrong. It was caught by
`cargo run -p hrw --example failure_map` before this lab was written, which is the only reason
you are not reading a confident false claim right now.

---

## Station 4 — Read it in the log

Click the Log toggle above the stage tabs.

**Expected:** a `Parse` bracket, then a `Resolve` bracket that closes marked `— failed`. The
`Rumoca compile` bracket and the Instantiate, Typecheck, Flatten, DAE construction, Simplify and
Structural brackets inside and after it still appear — HRW reading out what the compile
produced, which here is nothing, so each encloses a stage that reports *not reached*. There is
no `Rumoca solve lowering` bracket: there was no DAE to prepare.

Contrast `UnclosedModel`, where no bracket after Parse appears at all, because there was no
model name to compile.

---

## What to bring back

- Does the Resolve tab's error panel make the location obvious enough — line 9, the reference
  itself — or did you have to hunt for it?
- Station 4's brackets around stages that did not run: are they honest (HRW did read the
  compile's result, and found nothing), or noise? That is a pane decision, and yours.
