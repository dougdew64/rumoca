# Design note — context assembly and the Context Bar

**Purpose:** the design of the capture — how a question carries its context to Claude, and
why the noun must be self-sufficient.
**Status:** reference. **The design it describes is DELIVERED** (source-tooling Phase 5,
closed 2026-07-28); it is kept for the reasoning, which still governs every new emission
point.
**Read when:** adding anything that emits context, or adding a new place a user can point
from. **The composition primitives are frozen** — one point-at, one follow, background —
until a practical scenario proves otherwise. Do not re-propose multi-follow or a third
"compare" primitive from first principles.

Renamed from `tracking-as-capture.md` on 2026-07-27. The original note covered
one proposal (tracking should also capture); the discussion that followed showed
it was half of a larger idea, so this note now covers context assembly as a
whole. Proposed by Doug.

*(Header corrected 2026-08-01: this said "not yet implemented" for four days after it was
implemented. A design note that outlives its implementation has to say so, or a later session
plans to build what already exists.)*

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

And it sits alongside the three-tier progression as a load-bearing idea, not a
convenience (Doug, 2026-07-28):

> Along with our three-tier snapshot / replay / live trace, I believe that the
> context bar concept will be of central importance.

The two answer different halves of the same mission. The three tiers make an
algorithm's *process* observable — static result, recorded replay, live-stepped
code. The Context Bar makes the *conversation about it* legible: what has been
assembled, what Claude can therefore see, and what a question will have behind
it. Observation without a way to ask is a picture; asking without knowing what
was observed is guesswork.

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

### The payload must keep point and thread apart

Doug, 2026-07-27: keeping one wire format is right — it is all captured context
— **but the payload must still say which part is pointed at and which is
followed.** The distinction drives different behaviour, so losing it silently
degrades both request types:

**For `explain`,** the point is the *subject* and the thread is the *lens*.
"Pointed at `components.src.V`, following `src.V`" asks for that node explained
as part of that variable's story. "Pointed at an equation, following `h`" asks
about the equation with attention to how `h` participates. Flatten the two and
the answer addresses the wrong thing — a cross-stage narrative when one field was
asked about, or one field when the trajectory was wanted.

**For `debug-where-set`, it decides how many breakpoints to arm:**

| | breakpoints |
|---|---|
| a **point** — one node, one stage | **one** site: where that value is set |
| a **thread** — one identifier across stages | **several**: resolved, flattened, matched, demoted |

Conflating them arms one breakpoint when the trajectory was wanted, or scatters
them across the pipeline when a single field was. The multi-entry bridge request
exists precisely for the second case.

Requirements that follow:

- **Structural separation, not a flag.** Two named sections, so the distinction
  cannot be lost by a reader or flattened by a future edit.
- **Each independently absent.** Point without thread, thread without point, or
  both — all are normal states and the payload must express them.
- **Per-section recency.** If both are present and the request is ambiguous,
  whichever was acted on *last* is almost certainly the subject. One shared
  `seq` cannot express this; each section needs its own, so the reasoner can
  tell "pointed at this, then went following" from "was following, then pointed
  at this" — which mean different questions.

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

### Vocabulary: point at, follow

**Delivered 2026-07-28.** The menus now read "Point at" and "Follow …"; the bar
already read "Pointing at" and "Following". One vocabulary throughout.

The menu used to say "Capture" and "Track" while the bar said "Pointing at" and
"Following" — two vocabularies for one concept. Doug's call (2026-07-27) was
that the bar's verbs are the better ones and should win everywhere:

- **"Capture" describes what the app does** — writes a file. **"Point at"
  describes what the user does.** The bridge's own architecture note already
  says the loop is *point → ask → understand*; the menu was using a different
  word for the first step of its own model.
- **"Follow" is directional where "Track" is not.** Tracking could mean
  recording, monitoring, or logging. Following implies the thing goes somewhere
  and you are going with it.
- They pair, and the pairing carries the point/thread distinction for free.

Renaming covers the **UI labels, the Context Bar, and the docs**. The wire
format and internal identifiers (`focus.json`, `emit_node_focus`, `AskRequest`,
`Focus`, `TreeActions::capture`/`track`) stay: renaming a protocol Claude
already reads buys nothing and breaks continuity with recorded sessions. The
self-describing `instructions` string inside `focus.json` *is* updated, so
someone reading that file while dogfooding does not meet a third vocabulary.

One incidental decision worth recording: the glyphs. Only codepoints HRW already
renders may be used — egui ships far less than the whole of Unicode, and an
unproven one shows as a tofu box (U+2715 did exactly that in this bar). So
"Point at" takes 🎯 and "Follow" takes 🔎, the two already in the menu. The
magnifier landing on "Follow" is a better fit than it was on "Capture" anyway:
following *is* a search across every stage, and where the identifier is absent
counts as much as where it is found.

### The status bar loses its bridge role

**Delivered 2026-07-28.** `status_line` now returns `Option<String>` and a
successful point returns `None`; the field is renamed `bridge_status` →
`notice`, since what remains is genuinely transient and belongs nowhere else
("specimen not found", "diagnostic written to …", a stage-file write failure).

`bridge_status` used to show a transient line after each capture. The Context
Bar dominates that: persistent beats transient, and "what is pointed at" beats
"something was captured a moment ago". Worse than redundant, in fact — the
status line stated the point once and then went stale, so two surfaces claiming
to describe what Claude has could disagree. That is the failure this design
keeps running into, and the weaker surface is the one to drop.

Two exceptions survive, and both are things the bar *cannot* say:

- **Failures.** `bridge::write` returns `io::Result`, and if it fails the bar
  must say so — otherwise it reads "Pointing at X" while Claude is still holding
  the previous focus, which is exactly the confident lie the governing rule
  forbids. Failure moved *into* the bar (`point_error`), and the status line
  keeps a second, transient copy for the moment it happens.
- **The debugger request.** `debug-where-set` still speaks on success, because
  it asks the user to do something next — say "debug" in the chat. An
  instruction is not a confirmation.

The removal was sequenced after the bar existed, so there was never a window
without capture feedback.

### Which verb a left-click carries, and why the hover says so

**Added 2026-07-28**, after Doug met the inconsistency in testing.

Left-click means different things on different surfaces:

| Left-click -> **follow** | Left-click -> **point at** |
|---|---|
| Specimen source view (identifiers) | IR tree rows |
| Equation sheet's variable grid | Stage tabs, incidence rows, spy-plot blocks |

**The rule: the surface determines the verb, because the surface determines what
is clickable.** Where every clickable thing is a *name*, following is the only
thing a name affords. Where clickable things are *nodes*, most of them are not
names at all (`op: "Add"`, `causality: "None"`), so following is not generally
available. And there is a hard constraint underneath: **a source token has no IR
address.** The specimen text is not IR, so there is no key-path to emit and
"point at this token" is not expressible - following really is the only verb the
source view can offer.

That rule is coherent, but it was **emergent rather than designed** - the
source-view click came from Phase 3/4's reverse tracking (idea #37), before
"point at" and "follow" were named as the two primitives, so the vocabulary that
would have forced the question did not exist yet. It is written down here now.

### Why not rename "point at" to "select"

Doug's observation, same session: *"A mere point-at doesn't really suggest a side
effect, whereas a select suggests that there might be a side effect. Our left
clicks always cause side effects."* Correct - every left-click writes
`focus.json`, which changes what an external process has.

But "select" misdescribes it in the other direction. In every GUI anyone has
used, selection is **free, local and private**; ours is none of those. A reader
who learns "select" would be *more* surprised by the publish, not less.

**Why the publish is eager at all is structural, not sloppiness.** In classic
noun-verb, selection can be free precisely because the app *sees the verb*. Here
the verb is typed in another process - HRW never learns that a question was
asked - so the noun must be published speculatively on every change. Eager
publication is forced by the very split that makes the paradigm work.

Taking "select" seriously would therefore mean *changing the behaviour*, not the
label: select freely, then press **Send to Claude**. That was rejected - it costs
a click on every question, and it reintroduces the exact failure this design has
already fixed twice, where the bar and the file disagree because something was
selected and never sent. Eager publish makes that impossible by construction.

**So the verbs stay, and the gesture says what it will do.** `follow_hover` and
`POINT_AT_HOVER` (in `lib.rs`, shared so the wording cannot drift between
surfaces) name the verb *and* admit the send - "Follow `emf.phi` - sends it to
Claude now, and highlights it in every stage". The Context Bar already answered
"which did I just do?" afterwards; the hover answers it beforehand, which is
where the ambiguity actually lived. Tree hovers **append** to the field's Rumoca
documentation rather than replacing it: field help is the fast, no-AI tier, and
burying a real answer under directions for asking one would be a bad trade.

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
