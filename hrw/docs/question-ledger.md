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

---

## Open observations

Not entries — patterns across too few data points to trust yet, kept so they can
be confirmed or killed later.

- **Both entries are conversational, with no HRW context.** Today's questions were
  about the project's design, not about IR. The `focus.json` context field is
  therefore unexercised, and #41's claim that it will answer "what was he looking
  at when he got stuck?" is so far untested. Expect this to change once the
  Cellier work starts, and be suspicious if it doesn't.
- **n = 2.** Nothing here supports a generalisation yet. Resist reading trends
  into it.
