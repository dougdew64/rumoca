# Failure tour — Typecheck, which reports and does not stop at all

**Specimen:** `DimensionMismatch` — a 2-vector assigned from a 3-vector.

```modelica
Real small[2];
Real big[3];
equation
  small = big;              // dimensionally inconsistent
  big = {1.0, 2.0, 3.0};
```

**The question to hold:** this model is *wrong*. It cannot be simulated, and no amount of later
work will fix it. So why does the compiler keep going?

---

## Stop 1 — The diagnosis

[Load DimensionMismatch → Typecheck](hrw://load/DimensionMismatch/Typecheck)

**Expected:** Typecheck is **flagged**, and the note says `typecheck: 1 diagnostic(s)`. You see an
**error summary and no tree**.

Typecheck evaluates dimensions across the instantiated model — it has to, because array sizes can
come from parameters and are not always literal. Here `small` has 2 elements and `big` has 3, and
`small = big` cannot hold.

> ### A defect this stop found, 2026-08-05
>
> **This tour originally said "there is a tree below it", and Doug found there was not.**
>
> The stage value *does* contain one. Measured: 7.4 KB carrying `components`, `classes`,
> `type_roots` and the rest of the instantiated overlay, **plus** an `error` key. The worker
> assembles it deliberately — its comment reads *"the instantiated overlay is the last good state
> to show **beside** them"*.
>
> **The pane shows it instead of, not beside.** `App::central_panel_ui` tests
> `note_is_error() && value["error"].is_some()` and, when true, renders the error summary in place
> of the tree. So the overlay is built on every compile of every flagged model and discarded at
> the last step.
>
> **Nothing on screen says content is being withheld** — which is the Context Bar defect's shape
> exactly: a partial report leaves no gap where the missing part was. Logged in
> [`../tech-debt.md`](../tech-debt.md).
>
> **Keep reading with the tour as written.** What follows is unaffected, and the wrong expectation
> is left visible here on purpose: it is the second time this week a claim about a pane survived
> only because nobody looked.

---

## Stop 2 — The surprise

[Solve lowering](hrw://load/DimensionMismatch/SolveLowering)

**Expected:** Solve lowering has **content**. So do Flatten, DAE construction and Structural
analysis. Nothing after Typecheck refused to run.

**This is the most surprising pane in the failure set.** A model with a known type error was
carried all the way to the solver's executable form.

That is deliberate, and it is what "recovering compiler" means: a diagnostic is a *report*, not a
gate. The value is that one bad equation does not blind you to everything else about the model —
you can still see its flat form, its incidence matrix, its BLT blocks.

**The cost is that a green-looking tab does not mean a correct model**, and that is the thing
worth carrying away from this tour.

---

## Stop 3 — Where the truth is kept

[Back to Typecheck](hrw://load/DimensionMismatch/Typecheck)

**Expected:** the diagnostic names the equation and both dimensions. It is the **only** place in
the eleven tabs that says this model is wrong.

*(And per stop 1, it is currently the only thing this tab shows — the typechecked overlay beside
it is built and then withheld by the pane.)*

Ten tabs will show you a plausible-looking model. One tab says otherwise. **If you skip it, every
other tab lies to you by omission** — not because HRW is hiding anything, but because a flat model
of a bad model is still a flat model.

---

## Stop 4 — Compare with a stop

[Load UnclosedModel → Flatten](hrw://load/UnclosedModel/Flatten)

**Expected:** Flatten says it was **not reached**.

Three specimens, three shapes:

| | reports at | stops at |
|---|---|---|
| `UnclosedModel` | Parse | **Parse** |
| `UndefinedRef` | Resolve | **Flatten** |
| `DimensionMismatch` | Typecheck | **nowhere** |

The third row is the one nobody guesses.

---

## What to bring back

- Should a flagged Typecheck be *visible from the other tabs*? Right now the only signal is on
  the Typecheck tab itself.
- Is there a class of model where continuing past a type error is actively useful to you? If not,
  that is an argument to make the flag much louder.
