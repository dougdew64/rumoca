# Design note — context assembly and the Context Bar

Renamed from `tracking-as-capture.md` on 2026-07-27. The original note covered
one proposal (tracking should also capture); the discussion that followed showed
it was half of a larger idea, so this note now covers context assembly as a
whole. Proposed by Doug; not yet implemented.

## The thing this is really about

**HRW is not a standalone tool. It is an instrument for use with Claude.**

Every observatory feature ultimately serves one loop: *point → ask →
understand*. HRW's job is to make the pointing convenient and to emit what was
pointed at; the reasoning happens in the Claude Code session, over the specimen
source, the staged IR, the Rumoca phase code, and `docs/compiler-phases`. That
is the thin-emitter / thick-reasoner split, and it is why HRW carries no
precomputed explanations and no hard-coded breakpoint table.

The Context Bar below is the part of the UI that makes that split **visible**.
Doug's framing: it is "a constant reminder of the role which HRW plays and which
Claude plays in our overall design."

## Two shapes of context

HRW has grown two ways to assemble context, and until now they looked unrelated:

- **Capture** is a **point** — this node, at this key-path, in this stage, with
  its source provenance and cross-stage diff. Deliberate; one thing.
- **Tracking** is a **thread** — this identifier, everywhere it appears, across
  every stage. Ambient; persistent; spans the pipeline.

Nothing else distinguishes them. "What is the difference between captured and
tracked?" has no better answer than *point versus thread*.

The UI, however, treated them asymmetrically, and the asymmetry was arbitrary:

| | actual state | what the UI showed |
|---|---|---|
| Capture | persistent — `focus.json` stays until the next capture | a **transient** status line, then nothing |
| Tracking | persistent | a **permanent** "Tracking" bar |

So the one thing genuinely in the emitted context was the one you could not see,
and the one you could see was not emitted at all.

## Part 1 — Tracking should emit (compound capture)

As well as highlighting, tracking should perform a **compound capture**:
recording every place it highlighted, across every stage, so Claude can be asked
to explain the whole cross-stage story, or to arm breakpoints at several of
those places at once.

**It keeps the taxonomy out of HRW.** An earlier proposal (Claude's) had HRW
classify a variable's origin into specimen / library / phase-generated and
produce a different answer for each. That bakes a taxonomy into the app —
precisely the brittle precomputation the thin-emitter principle exists to
prevent. The compound capture needs no such concept: if a variable turns out to
be a Pantelides dummy, that is *visible in the captured set*, and the reasoner
says so.

`build_declaring_classes` (2026-07-27) has since made this sharper. HRW now
reports a source line, or a declaring class, or neither — and the reasoner draws
the categories:

| what HRW emits | what Claude concludes |
|---|---|
| source line present | declared in the specimen |
| declaring class present | came from a library |
| neither | a compiler phase created it |

HRW never has a concept called "phase-generated". It reports facts and absences.

**HRW already computes this and throws it away.** Every stage view runs
`matches_tracked` over its own data each frame to decide what to highlight. That
sweep *is* the answer to "where does this appear across the pipeline", and it is
discarded at the end of every paint. Capturing it is not new computation.

**The set says more than its members.** Eight separate captures are eight facts.
The assembled set is a *trajectory*: declared at line 12 → `def_id` 27579 in
Resolve → flat variable `emf.phi` → row 7 of the incidence matrix → demoted in
reduction round 2 → absent from the solver's state vector. The interesting part
is usually a *transition*, and a transition is only visible when both sides are
present.

### Shape

- Reuse the existing capture machinery — `build_node`, span-ascent, the
  cross-stage diff — once per hit, rather than inventing a second format.
- Per hit record: the stage, what kind of thing it is (declaration, equation,
  incidence row, matched column, BLT block, solver slot), the enclosing node,
  and the key fields.
- **Emit absence as well as presence.** "Not present in Initialization or Solve
  lowering" is information: it is how a demoted or alias-eliminated variable
  announces itself. A capture listing only hits cannot express disappearance,
  and disappearance is often the whole story.
- Refresh when tracking changes — a click, not a frame.

### The concern to design around

**Ambient tracking must not overwrite deliberate capture.** Capture means "this
is the thing I am asking about"; tracking is exploratory and gets toggled while
browsing. If tracking rewrites `focus.json`, then capturing a node and
afterwards tracking something to look around silently destroys the context you
meant to ask about.

Resolution: keep both in one file as **distinct sections** — `focus` for the
last explicit capture, `tracking` for the ambient compound set. One channel, no
ambiguity about which file is current, and both arrive together, which is often
exactly what is wanted ("you captured this equation, and you are tracking `h`,
which appears in it").

## Part 2 — The Context Bar

Replace the "Tracking" bar with a **Context Bar**: a persistent summary of what
context is currently available for Claude to answer questions with, updated as
you capture, track, un-capture, and un-track.

### The governing principle

**The Context Bar is a rendering of what will be emitted — not a separate UI
concept.**

If it shows something Claude does not receive, or omits something Claude does,
it lies about Claude's knowledge, and questions get calibrated against a
fiction. Built as a view of the payload, it cannot drift, because there is
nothing to drift from.

This has a sharp consequence: **an honest Context Bar cannot exist until
tracking emits.** Today tracking writes nothing, so a bar listing it would claim
context that does not exist. Part 1 and Part 2 are therefore not adjacent ideas
but one piece of work seen from two sides — the emission makes the bar true, the
bar makes the emission legible.

### Sketch

```
Context   MotorWithBrake · Flatten
  Pointing at   components.src.V                  [x]
  Following     src.V — in …ConstantVoltage       [x]   12 mentions across 7 stages
```

- Specimen and stage are always context, so they are always shown.
- "Pointing at" and "Following" keep the point/thread distinction in the
  vocabulary rather than hiding it.
- The mention count says how much a question about `src.V` actually has behind
  it — the difference between a rich answer and a thin one, visible *before* the
  question is asked.
- Showing both rows also settles the overwrite concern by making coexistence
  visible instead of leaving it a hidden hazard.

## What this does to Phase 5

Phase 5 of [`source-tooling-plan.md`](source-tooling-plan.md) was stated as a
design question: *which phase function is the meaningful breakpoint site for
"where does this identifier get set?"* — the resolver assigning a `def_id`,
flattening emitting the flat variable, structural analysis matching its row,
index reduction demoting it. Four defensible answers, and whichever HRW encoded
would be wrong for some cases.

The compound capture dissolves the question. HRW emits where the identifier
lives at each stage; Claude chooses the sites per case, with the whole
transformation in view. The multi-breakpoint path is already plumbed — the
bridge request carries a list of `{path, line, condition}` entries and the
extension applies them all.

So **Phase 5 is now: bring the Context Bar to life.** Make tracking emit, render
the bar as a view of that emission, and let breakpoint selection follow from the
context rather than from a table in the app.

## Relationship to Phase 4

Phase 4 (reverse identifier tracking) stands on its own: the ambient
click-anywhere gesture, source navigation, the declaring-class answer, and never
replying with silence. Nothing here changes what tracking *highlights*. But every
view Phase 4 wires up is one more source of hits for the compound capture, so
finishing Phase 4 first makes the Context Bar richer when it arrives.
