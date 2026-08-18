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

**Expected:** Typecheck is **flagged**, the note says `typecheck: 1 diagnostic(s)`, and you see
**both**: a `⚠ What went wrong` panel with the diagnostic, and the typechecked overlay as a tree
below it. The panel is collapsible — fold it away once read.

Typecheck evaluates dimensions across the instantiated model — it has to, because array sizes can
come from parameters and are not always literal. Here `small` has 2 elements and `big` has 3, and
`small = big` cannot hold.

> ### A defect this stop found — and it is fixed, 2026-08-05
>
> **This stop originally promised a tree. Doug walked it and there was none.**
>
> The stage value always contained one: 7.4 KB of instantiated overlay — `components`, `classes`,
> `type_roots` — **plus** an `error` key, assembled by the worker on purpose, its comment reading
> *"the instantiated overlay is the last good state to show **beside** them"*.
>
> **The pane showed it instead of, not beside.** `central_panel_ui` rendered the error summary in
> an `if` with the entire tree branch as the `else`, so the overlay was built on every compile of
> every flagged model and discarded at the last step — **with nothing on screen saying content was
> withheld**, which is the Context Bar defect's shape exactly.
>
> **The fix was not "always show the tree."** `note_is_error()` is true for both abnormal
> outcomes, and a **failed** stage carries only `{"error": …}` — a tree there would render the
> error payload as a tree beside the summary that already explains it. The condition is whether
> the value holds anything **beyond** `error`.
>
> Two tests now hold it: `a_flagged_stage_shows_its_artifact_beside_its_error` and
> `a_failed_stage_with_only_an_error_shows_no_tree`. The first was verified to **fail** on the old
> behaviour before being kept.
>
> **The wrong expectation is recorded here rather than quietly rewritten**, because how it was
> found is the lesson: a tour claimed something about a pane, and it took someone actually walking
> it to notice. That is the second time this week.

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

*(Since 2026-08-05 the typechecked overlay sits below it rather than being withheld — see stop 1.
The diagnostic is still the only place in eleven tabs that says the model is wrong.)*

Ten tabs will show you a plausible-looking model. One tab says otherwise. **If you skip it, every
other tab lies to you by omission** — not because HRW is hiding anything, but because a flat model
of a bad model is still a flat model.

---

## Stop 4 — Compare where the compile halts

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
