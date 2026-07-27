# Design note — tracking as compound capture

Proposed by Doug, 2026-07-27, while planning Phase 5 of
[`source-tooling-plan.md`](source-tooling-plan.md). Not yet implemented; this
note records the reasoning so it survives the session.

## The proposal

Today HRW has two independent notions:

- **Capture** — deliberate. Click a node, and the bridge writes `focus.json`
  describing what you pointed at. Claude reads it and reasons: explains the
  node, or works out a useful breakpoint. This is the thin-emitter /
  thick-reasoner split, and it is why HRW carries no precomputed explanations
  and no hard-coded breakpoint table.
- **Tracking** — ambient. Set `tracked_identifier`, and every stage view
  highlights its mentions. Purely visual. It emits nothing.

The proposal: **give tracking a second meaning.** As well as highlighting, it
performs a *compound capture* — recording every place it highlighted, across
every stage — so Claude can be asked to explain the whole cross-stage story, or
to arm breakpoints at several of those places at once.

## Why this is the right shape

**It keeps the taxonomy out of HRW.** An earlier proposal (Claude's, same day)
had HRW classify a variable's origin into specimen / library / phase-generated
and produce a different kind of answer for each. That bakes a taxonomy into the
app — precisely the brittle precomputation the thin-emitter principle exists to
prevent. The compound capture needs no such concept: if a variable turns out to
be a Pantelides dummy, that is *visible in the captured set*, and the reasoner
says so. **The proposal subsumes the tiered design and is cleaner.**

**HRW already computes this and throws it away.** Every stage view runs
`matches_tracked` over its own data each frame to decide what to highlight. That
sweep *is* the answer to "where does this appear across the pipeline", and it is
discarded at the end of every paint. Capturing it is not new computation; it is
keeping something already paid for.

**The set says more than its members.** Eight separate captures are eight facts.
The assembled set is a *trajectory*: declared at line 12 → `def_id` 27579 in
Resolve → flat variable `emf.phi` → row 7 of the incidence matrix → demoted in
reduction round 2 → absent from the solver's state vector. The interesting part
is usually a *transition*, and a transition is only visible when both sides are
present. Without this, answering "what happened to `h`?" means Claude re-deriving
by hand what HRW had already found.

## The concern to design around

**Ambient tracking must not overwrite deliberate capture.**

Capture currently means "this is the thing I am asking about". Tracking is
exploratory — it gets toggled while browsing. If tracking rewrites `focus.json`,
then capturing a node and afterwards tracking something to look around silently
destroys the context you meant to ask about.

Suggested resolution: keep both in one file as **distinct sections** — `focus`
for the last explicit capture, `tracking` for the ambient compound set. One
channel, no ambiguity about which file is current, and both arrive together —
which is often exactly what is wanted ("you captured this equation, and you are
tracking `h`, which appears in it").

## Suggested shape

- Reuse the existing capture machinery — `build_node`, span-ascent, the
  cross-stage diff — once per hit, rather than inventing a second format.
- Per hit, record: the stage, what kind of thing it is (declaration, equation,
  incidence row, matched column, BLT block, solver slot), the enclosing node,
  and the key fields.
- **Emit absence as well as presence.** "Not present in Initialization or Solve
  lowering" is information: it is how a demoted or alias-eliminated variable
  announces itself. A capture that lists only hits cannot express disappearance,
  and disappearance is often the whole story.
- Refresh when tracking changes — a click, not a frame.

## What this does to Phase 5

Phase 5 was stated as a design question: *which phase function is the meaningful
breakpoint site for "where does this identifier get set?"* — the resolver
assigning a `def_id`, flattening emitting the flat variable, structural analysis
matching its row, index reduction demoting it. Four defensible answers, and
whichever HRW encoded would be wrong for some cases.

The compound capture dissolves the question. HRW emits where the identifier
lives at each stage; Claude chooses the sites per case, with the whole
transformation in view. The multi-breakpoint path is already plumbed — the
bridge request carries a list of `{path, line, condition}` entries and the
extension applies them all.

So Phase 5 becomes "emit richer context, then ask", rather than "design a
taxonomy". That is the architecture working as intended.

## Relationship to Phase 4

Phase 4 (reverse identifier tracking) stays as it is: the universal
click-anywhere gesture, tier-1 source navigation, and never answering with
silence. This note describes what tracking *additionally* emits, not a change to
what it highlights. Phase 4 does not depend on it, and it does not depend on
Phase 4 being finished — but every view Phase 4 wires up is one more source of
hits for the compound capture, so doing Phase 4 first makes this richer.
