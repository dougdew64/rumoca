# Failure lab — Typecheck, flagged, and the compile goes no further

<!-- kind: failure -->

**Specimen:** `DimensionMismatch` — a 2-vector assigned from a 3-vector.

```modelica
Real small[2];
Real big[3];
equation
  small = big;              // dimensionally inconsistent
  big = {1.0, 2.0, 3.0};
```

The question to hold: this model is *wrong*. It cannot be simulated, and no amount of later
work will fix it. Typecheck says so — and its tab is amber, not red. So what does that colour
promise, and what do the tabs after it say?

---

## Station 1 — The diagnosis

[Load DimensionMismatch → Typecheck](hrw://load/DimensionMismatch/Typecheck)

**Expected:** Typecheck is flagged (amber), the note says `typecheck: 1 diagnostic(s)`, and you
see both: a `⚠ What went wrong` panel with the diagnostic, and the instantiated overlay as a
tree below it — the last good state before the error. The panel is collapsible — fold it away
once read.

Typecheck evaluates dimensions across the instantiated model — it has to, because array sizes can
come from parameters and are not always literal. Here `small` has 2 elements and `big` has 3, and
`small = big` cannot hold.

> ### A defect this station found — and it is fixed, 2026-08-05
>
> This station originally promised a tree. Doug ran it and there was none.
>
> The stage value always contained one: 7.4 KB of instantiated overlay — `components`, `classes`,
> `type_roots` — plus an `error` key, assembled by the worker on purpose, its comment reading
> *"the instantiated overlay is the last good state to show beside them"*.
>
> The pane showed it instead of, not beside. `central_panel_ui` rendered the error summary in
> an `if` with the entire tree branch as the `else`, so the overlay was built on every compile of
> every flagged model and discarded at the last step — with nothing on screen saying content was
> withheld, which is the Context Bar defect's shape exactly.
>
> The fix was not "always show the tree." `note_is_error()` is true for both abnormal
> outcomes, and a failed stage carries only `{"error": …}` — a tree there would render the
> error payload as a tree beside the summary that already explains it. The condition is whether
> the value holds anything beyond `error`.
>
> Two tests now hold it: `a_flagged_stage_shows_its_artifact_beside_its_error` and
> `a_failed_stage_with_only_an_error_shows_no_tree`. The first was verified to fail on the old
> behaviour before being kept.
>
> The wrong expectation is recorded here rather than quietly rewritten, because how it was
> found is the lesson: a lab claimed something about a pane, and it took someone actually running
> it to notice. That is the second time this week.

---

## Station 2 — What every later tab says

[Solve lowering](hrw://load/DimensionMismatch/SolveLowering)

**Expected:** Solve lowering has no content; its note reads
`not reached (Typecheck failed earlier)`. Flatten, DAE construction, Simplify, Structural and
every tab between say the same. **Falsified if:** any tab after Typecheck shows a tree.

So a flagged Typecheck is where this compile ends. The amber colour is the outcome class HRW
gives a stage that produced its artifact *and* reported a problem — Typecheck did both — but it
does not promise the pipeline went on. Here nothing after it ran.

*(This station used to say the opposite — that a type error was carried all the way to the
solver's executable form. It was true of 0.9.20 and is not of the release HRW builds on; the
outcome matrix `the_corpus_outcome_matrix_is_unchanged` pins this row as Typecheck flagged and
every later stage not reached.)*

---

## Station 3 — Where the truth is kept

[Back to Typecheck](hrw://load/DimensionMismatch/Typecheck)

**Expected:** the diagnostic `ET002` names both dimensions — `expected [2], found [3]` — and
the equation's location, line 11, `small = big;`.

Every other tab tells you only that it was not reached and that Typecheck is why. The *reason*
is here and nowhere else.

---

## Station 4 — Compare where the compile halts

[Load UnclosedModel → Flatten](hrw://load/UnclosedModel/Flatten)

**Expected:** Flatten says it was not reached.

Three specimens, three shapes:

| | reports at | halts at |
|---|---|---|
| `UnclosedModel` | Parse | Parse |
| `UndefinedRef` | Resolve | Resolve |
| `DimensionMismatch` | Typecheck | Typecheck |

Each halts where it reports. What differs is the colour: Parse and Resolve are red (`Failed`),
Typecheck is amber (`Flagged`), because Typecheck's tab still has an artifact to show.

---

## What to bring back

- Should a flagged stage that ends the compile look different from a flagged stage the compile
  carries on past (Structural on `Drivetrain`, say)? Right now both are amber.
- Station 2's notes say *which* stage stopped the compile. Is that enough, or do you want the
  reason repeated on the later tabs?
