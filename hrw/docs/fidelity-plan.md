# Plan — does HRW faithfully represent Rumoca?

Written 2026-07-30, from Doug's concern:

> We have not sufficiently tested that the HRW code correctly represents Rumoca's
> understanding or decisions about the specimens which it compiles.

He is right, and nothing in the 444 tests addresses it. They check HRW's **own** logic
(link parsing, playback, layout) and that specimens **compile**. Neither asks whether what
HRW *shows* is what Rumoca *decided*.

## Why this is the dangerous gap

HRW does two different things with Rumoca's output, and only one of them is safe:

| | Risk |
|---|---|
| **Reads** a phase result and renders it | distortion — a dropped field, a transposed index, a mislabelled column |
| **Re-derives** a result by re-running the algorithm | **divergence** — HRW computes a *different answer* and presents it as the compiler's |

The second is the one that can teach Doug something false. `tearing_anim::walk_blocks`
re-runs tearing on every coupled block to produce its animation; `matching_anim` and
`tarjan_anim` re-run matching and Tarjan from the incidence JSON. **None of these has ever
been compared against what Rumoca's own report says.**

They agree on the 18 hand-picked specimens by assumption, not by test.

## The design idea: invariants, not expected answers

The obstacle raised by `docs/ideas.md` #51 is **triage cost** — with 1,656 unfamiliar
models, every failure is an investigation, and Claude has already twice reported a finding
that was not there.

**Invariant checks dodge that entirely.** They do not require knowing the right answer for
a given model; they assert a property that must hold for *every* model. A violation is
definitionally a bug, with nothing to adjudicate. That is what makes this affordable at
scale where a compile-census is not.

## The checks

Ordered by how much they would hurt if they failed.

### F1. Re-derivation matches the report  *(the centrepiece)*

For every model with a coupled block: run `tearing_anim::walk_blocks` and compare its
result against the `tearing` field of the structural report — same tear variables, same
residual equations, same causal sequence, in the same order.

Both sides exist today. The report says
`{tear_vars, residual_equations, causal_sequence}`; the animation produces `TearingFrame`s
plus `BlockNames` to resolve local indices back to names.

Same shape for `matching_anim` (final matching vs the report's `matching`) and
`tarjan_anim` (its SCCs vs the report's `blocks`).

**If this fails, an animation is teaching a decision the compiler did not make.**

### F2. Incidence is faithful

HRW's `incidence` JSON against a fresh `build_incidence(&dae)`: same `n_eq`, same `n_var`,
same `unknown_names` in the same order, and every row's index set identical. Catches a
transposition or an off-by-one that would make every downstream view subtly wrong while
looking plausible.

### F3. Structural counts agree with themselves

`n_equations`, `n_unknowns`, `matching.len()`, `rank_deficiency` — mutually consistent, and
consistent with `incidence`. The `rank_deficiency: 7` bug (2026-07-29, true value 1) is
exactly this class and was found by eye.

### F4. BLT blocks partition the equations

Every equation appears in exactly one block; block sizes sum to `n_equations`. A
partition is easy to check and impossible to satisfy accidentally.

### F5. Matching is a matching

Injective: no equation matched to two unknowns, no unknown claimed twice, every index in
range.

### F6. Derived views cover their source

The equation sheet names every continuous equation; the identifier index's variables are a
subset of the DAE's; every `def_id` referenced in a stage resolves in `def_index`.

### F7. Every capture noun works on real IR

For a sample of node paths from each stage: `describe_path` → `parse_path` → `navigate`
returns the same subtree. The capture vocabulary has only ever been exercised on 18
models, and it is the thing every question depends on.

### F8. No stage panics, and sizes are recorded

The stress test, as a **byproduct** rather than the goal. `Media.Examples.WaterIF97`
produced a 3.2 MB flatten stage without panicking; that was previously unknown. Record
size and time per stage per model so a regression in either is visible.

## The sample

**Not all 1,656.** A stratified sample of **40–60**, chosen to cover:

- every MSL top-level package (Electrical, Mechanics, Thermal, Fluid, Media, Magnetic,
  Blocks, Clocked, StateGraph, Math, Utilities)
- the phenomena HRW has views for: high index, algebraic loops, events, arrays, functions,
  clocked/synchronous
- the extremes — the largest models available, and the smallest

Stratification matters more than count: 50 models spread across packages will find more
than 500 from one.

## Sequencing

1. **Fix outcome classification first.** `Stage::note_is_error` is set by three
   constructors meaning "nothing produced", "failed with a diagnosis", and **"singular but
   perfectly usable"**. Anything counting outcomes on that boolean will miscount, and it
   already produced one false finding (see #51). A three-way `Stage::outcome()` comes
   before any harness reads it.
2. **F1 on the existing 18 specimens.** If the re-derivations already disagree with the
   report on models we know, that is the bug of the day and the MSL work can wait.
3. **The sample list**, checked in, with why each model was chosen.
4. **F2–F7 as invariants**, run over the sample.
5. **F8 recorded**, with a baseline that must not regress.

Step 2 is deliberately first and deliberately small: it is the cheapest possible test of
the plan's central hypothesis, and if it passes on 18 models the same harness runs on 50
with no new code.

## Cost, and where it runs

Each model is a full uncached compile against MSL — seconds. 50 models is minutes, not the
hour a full census would take, which is why the sample is stratified rather than
exhaustive. **Behind `slow-tests`**, not in the 7-second loop.

## What this does not establish

That Rumoca is *correct*. Every check here asks whether HRW agrees with Rumoca, so a
compiler bug faithfully rendered passes every one of them. Correctness needs the
independent implementation — `docs/ideas.md` #43 and the differential test in #4, on the
same corpus.

**Two different questions, and this plan answers only the first:**

- *Does HRW tell the truth about Rumoca?* — this plan.
- *Does Rumoca tell the truth about Modelica?* — System Modeler.
