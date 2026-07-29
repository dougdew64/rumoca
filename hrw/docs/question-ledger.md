# Question Ledger

**Claude's record of what Doug asked, and what made it click.** Stage A of
`docs/ideas.md` #41. Read this before answering a question in a familiar area.

Doug reads this only if he wants to; its audience is Claude.

---

## Why this file exists

Everything else in `docs/` is regenerable. Claude can re-derive what Pantelides
does, what a coupled block is, how tearing picks a variable — which is exactly
why the specimen narratives and the end-to-end tour's prose were retired
(`docs/ideas.md` #42).

**Doug's questions are not regenerable.** Neither is the confusion behind one, nor
the specific thing that finally resolved it. That is the whole content of this
file, and it is the only artifact here that gets more valuable with time.

## How to use it

**Before answering**, scan for the concept. If it appears already:

- **Asked before, and the earlier explanation is recorded as having worked** —
  don't repeat it verbatim; check whether the question is actually a different
  question wearing the same words.
- **Asked before and the answer evidently did not stick** — the earlier
  explanation *failed*. Try a different angle, do not restate it louder. Two
  branches, and they call for opposite responses:
  - the **concept** is hard → a different explanation, probably a more concrete one
  - the **thing is not visible in HRW** → this is a feature request, and a better
    one than Claude would invent. Log it in `docs/ideas.md` and say so.

**After answering**, append an entry if something durable happened. Not every
turn — every-turn capture produces a log nobody can retrieve from. Say out loud
in the conversation when writing here, so Doug can veto an entry before it
calcifies.

## Entry format

- **Date** and the question **verbatim**. Not paraphrased: the wording is evidence.
- **Context** — what was on screen, from `.hrw-bridge/focus.json` when there is
  one. "Conversational" when Doug was not pointing at anything. Over months this
  answers a question Claude cannot otherwise ask: *what was he looking at when he
  got stuck?*
- **Medium** — text, HRW tour, animation, Wolfram notebook, System Modeler
  (`docs/ideas.md` #43). "The tearing animation" and "the rank computation" are
  different facts about how Doug learns.
- **What unlocked it** — the specific thing, not a summary of the answer.
- **Repeat?** — how many times this concept has come up.
- **Medium feedback** — when Doug comments on the *medium* rather than the answer.
  He committed on 2026-07-29 to saying so when a tour arrives where text would have
  done, and asked that it be recorded as a signal. Record it here, and treat two
  instances of the same kind as a standing correction, not two isolated notes.

## The medium rule (Doug, 2026-07-29)

**Lead with text, always. Write a tour only when Doug asks for one.**

His solution to a problem Claude had raised: the failure mode is asymmetric. Text
that should have been a tour costs one follow-up; a tour that should have been text
costs minutes of walking stops to reach a two-sentence answer. Leaving the choice to
Claude's judgement means Claude polices a bias it cannot feel — composing a tour will
always be the more interesting work.

One refinement Claude added and Doug accepted: **text first is not text silent.**
When a tour would genuinely add something, say so at the end of the text answer, so
Doug is accepting an offer rather than guessing which answers have a tour behind
them. Doug also expects to request follow-up tours on his own initiative.

---

## Entries

### 2026-07-29 — "Remind me again, what is the replay/reveal test?"

- **Context:** conversational — no HRW capture. Mid-discussion about which
  compiler phases deserve animations.
- **Medium:** text.
- **Repeat?** **Yes — second asking, roughly two exchanges after the first.**
- **Concepts:** replay vs reveal; which phases hide a search.

**What unlocked it:** a table with one row per animated phase and a single column
— *"what running it produces that the output doesn't."* Matching: the paths that
were tried and failed. Tearing: the appearance and competitor counts. Alias
elimination: *nothing*. IC planning: *nothing*.

**The lesson, and it is about Claude, not Doug.** Claude coined "the replay/reveal
test", used it as established shorthand two messages later, and it had not stuck.
Naming an abstraction is not teaching it. What worked was not a better definition
— it was **enumerating the instances** so the rule could be read off them.

- This is the *concept-is-hard* branch, not the *not-visible-in-HRW* branch. No
  feature request follows. Correctly diagnosing which branch a repeat belongs to
  is the skill this ledger is meant to build.
- **Standing correction for Claude:** do not introduce a coined term and then rely
  on it as shorthand. Either re-ground it at each use, or write it where it can be
  retrieved. It is now in `docs/ideas.md` #9 and `hrw/CLAUDE.md`, so the third
  asking should be answerable by pointing.

### 2026-07-29 — "So the Solve Lowering phase is not supposed to have an animation, is that correct?"

- **Context:** conversational, immediately after four new animations were
  delivered and Solve Lowering was visibly not among them.
- **Medium:** text, after reading `crates/rumoca-phase-solve/src/`.
- **Repeat?** First asking.
- **Concepts:** which phases hide a search; forward-mode AD; where the Jacobian
  comes from.

**What unlocked it:** splitting the phase into its three jobs and testing each
separately, rather than answering about "the phase". `layout.rs` packs variables
into solver slots — a walk, and its result is *already on screen* as
`problem.layout`. `lower.rs` compiles equations to a register machine —
mechanical. **`ad.rs` is the exception**: forward-mode AD applies the chain rule
per operation, which *is* a rule-driven transformation with a reason at every
step, and it is where the Jacobian comes from.

**Worth noting:** the question was framed as a yes/no scope check, and the honest
answer was "yes, but one third of that phase is a real candidate." Answering only
the yes/no would have been correct and useless. Logged as a candidate in
`docs/ideas.md` #9 rather than proposed as work, because whether watching a JVP
tape assemble beats a breakpoint in `ad.rs` is a question for Doug's reading.

### 2026-07-29 — "The Structural phase summary claims that the rank has a deficiency of 1. What does that mean?"

- **Context:** **first entry with a real HRW capture.** `focus.json` seq 1, `kind:
  stage`, `request: explain`, stage `Structural`, `stage_view: Summary`, specimen
  `MotorWithBrake.mo`, `ui_mode: Specimen`.
- **Medium:** text first (per the medium rule), then **the first ad hoc tour**, at
  Doug's request — "Write it."
- **Repeat?** First asking.
- **Concepts:** structural vs numerical rank; maximum matching as structural rank;
  hidden constraints; dummy-derivative demotion; degrees of freedom.

**What the answer rested on** — read from the bridge rather than recalled: 48
equations, 48 unknowns, 47 matched. Unmatched witnesses `f_x[46]`
(`emf.flange.phi - load.flange_a.phi`) and `emf.p.v`. Then the index-reduction
report: `reduce_constrained_dummy_derivatives` → 1 demoted, `emf.phi`, states 4 → 3,
`eliminate_trivial` → 41 eliminated.

**The framing that carried it:**

> The model was written with 4 independent states. The constraints permit only 3.
> The deficiency is the compiler discovering that, before any number is computed.

Reaching that required going *past* the definition to the cause: the unmatched
equation contains **no derivatives**, so it constrains positions directly, which is
the textbook hidden constraint. A rigid coupling removes a degree of freedom — which
lands on the robotics mathematics Doug is aiming at rather than staying a compiler
fact.

**Two things worth carrying forward:**

- **The unmatched pair is not a matched pair.** `f_x[46]` never mentions `emf.p.v`.
  Which equation and which unknown get stranded is *not unique* — a different
  maximum matching strands a different pair; only the count is invariant. Left
  unexplained this reads as a bug, so say it every time these witnesses come up.
- **Verification over assertion worked.** Every number above came from
  `.hrw-bridge/stages/*.json`, and the "no rows differentiated, it took the
  dummy-derivative path instead" detail would have been guessed wrong — the obvious
  assumption for a hidden constraint is that it gets differentiated.

**Feature request produced → `docs/ideas.md` #44.** Writing the tour surfaced that
`Matching ▶` is **hidden when Structural is singular**, so the one view that would
let Doug *watch* the deficiency happen is unavailable exactly when it matters. This
is the **not-visible-in-HRW** branch of the repeat signal, arriving without needing a
repeat — the first requirement the #42 mechanism produced, on its first use.

---

## Open observations

Not entries — patterns across too few data points to trust yet, kept so they can
be confirmed or killed later.

- ~~Both entries are conversational, with no HRW context.~~ **Retired 2026-07-29**
  by the rank-deficiency entry, which carried a full capture (stage, sub-view,
  specimen). #41's claim that the context field would matter is no longer untested:
  knowing Doug was on *Structural → Summary* for *MotorWithBrake* is what made the
  answer specific rather than a definition of rank deficiency in general.
- **The first tour produced a feature request immediately** (#44). One data point,
  but it is the data point the whole #42 argument predicted, so note whether it
  keeps happening — a mechanism that surfaces a real gap per use is worth far more
  than one that produces tours.
- **n = 3.** Still nothing that supports a generalisation. Resist reading trends
  into it.
