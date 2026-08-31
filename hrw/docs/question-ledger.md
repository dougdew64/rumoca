# Question Ledger

**Purpose:** what Doug asked, verbatim, and the specific thing that made it click.
**Status:** record — append-only, never rewritten.
**Read when:** before answering a question in an area that has come up before. A repeat is a
signal, and it branches two ways demanding opposite responses.

Stage A of `docs/ideas.md` #41. Doug reads this only if he wants to; its audience is Claude.

---

## Always in flight

**Every phase of work appends here, and this is the only artifact whose value depends on
elapsed time.** *(Carried forward 2026-08-01 from the retired answer-platform plan, whose
closing section this was.)*

If experimenting with features is what teaches — the project's strongest evidence to date —
then **the record of which experiment taught what is the irreplaceable artifact.** A stretch
of work that ends with no entries here is not necessarily wasted (the animations produced none
while being built), but it does mean the learning went unrecorded, and **that is the one loss
this project cannot absorb.**

---

## This is how the curriculum tours get graded

**Doug, 2026-08-03, on finishing the first two:** *"The real measure of whether the tours are
good enough will be the nature of the questions which I ask you while and after I work through
the tours. If it seems that I have learned what you attempted to explain, then the tours will
have been at least partly successful."*

That criterion has no other instrument. A tour's link checker proves its links resolve and its
`**Expected:**` lines prove its claims are violable — **neither can tell whether the lesson
landed.** This file can, because it already records the one thing that reveals it.

**So a question arriving during or after a curriculum tour gets logged with the stop it traces
back to.** The mapping is the whole point: without it there is a pile of questions and a pile
of tours and no way to connect them.

Four shapes to watch for, because they call for opposite responses:

- **A question the tour set up and did not answer** — the tour worked. It built enough
  scaffolding for the next question to be askable. This is the target.
- **A question the tour already answered** — the explanation did not land. Note *which stop*,
  and try a different angle rather than repeating the prose louder.
- **A question revealing a misconception the tour created** — the most valuable signal here and
  the most expensive to miss. A wrong idea installed confidently is worse than no idea, and
  only Doug can surface it. Fix the tour, and record what the wrong wording was.
- **No questions at all** — ambiguous, and **do not read it as success.** It means either the
  tour was complete or it provoked nothing, and those are opposite outcomes. Ask.

**The tours are regenerable; this record is not.** Same rule as the rest of the file — which is
why a tour rewritten in response to a question should say so in its own text, as
`dae-construction.md` does.

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
- **Asked again to RE-TEST a capability that just landed** — added 2026-07-29, because
  the two branches above did not cover it and a naive reading of them gives the *wrong*
  response. Doug re-asked "why did the structural phase fail for CapacitorLoop?"
  immediately after source-line highlighting shipped, having asked it before source
  spans existed at all. The explanation had not failed; the *answer channel* had
  changed.

  **Tell it apart by what shipped between the two askings.** If a relevant capability
  landed in between, the question is a re-test. Then: **keep the explanation, lead with
  what is newly possible, and be shorter** — Doug has heard the reasoning and is
  checking the delivery. Hunting for a fresh angle would waste the turn and imply he
  had failed to understand something he understood the first time.

**If composing a tour hits an HRW gap** — you cannot get somewhere, or you work
around it with prose ("same tab → now click X") — record it in the **tour holes**
table at the top of [`tech-debt.md`](tech-debt.md), not only here. Doug's ruling:
those outrank all other tech debt, because they degrade the deliverable rather than
costing future effort. **Prose workarounds count**: the first tour logged its loud
hole (#44) and silently accepted a quiet one at four separate stops, which went
unnoticed until Doug asked whether holes were being tracked.

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

### 2026-07-29 (second asking) — "Why did the structural phase fail for the CapacitorLoop specimen?"

- **Context:** `focus.json` seq 1, stage `Structural`, `stage_view: Summary`,
  `specimen_detail: Source`, specimen `CapacitorLoop.mo` — **he had the source view
  open**, which is itself the evidence for the re-test reading.
- **Medium:** text, short. No tour: one already exists for this question and nothing
  about it needed regenerating.
- **Repeat?** **Third asking of this concept, and a *re-test*** — the first was before
  source spans existed, the second produced the contrast tour, this one followed
  source-line highlighting shipping.
- **Concepts:** structural singularity; high-index vs ill-posed; where blame belongs.

**What made the difference this time:** nothing about the explanation. The answer that
had been *"reported at line 9"* became *"line 9 should be tinted right now."* Same
diagnosis, delivered by pointing instead of quoting.

**The ledger gained a category because of this entry.** "Repeat" had two branches —
concept-is-hard and not-visible-in-HRW — and neither fits. Following either would have
had Claude change an explanation that worked. See the third bullet under *How to use it*.

**Still worth saying every time these witnesses come up:** `gnd.p.i` is the visible
casualty, not the cause. The cause is lines 7–8 pinning the capacitor voltage; the
shortage surfaces at `gnd.p.i` because it has the least slack in the matching. Claude
got this wrong once already (called it "a single point of failure") and had to correct
it after checking `RcCircuit`, where the same one-mark column matches fine.

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

**Outcome — the tour delivered.** Doug walked it and reported: *"I found row 46 and
have concluded that the ad hoc tour feature is working brilliantly."*

**So what unlocked it was Stop 2** — finding the one row in a 48x48 matrix with no
matching marker on it. Everything else in the tour is Claude asserting things; that
stop is Doug verifying one himself, which is why it was the stop that had to work.
**Generalise this when composing tours: at least one stop should be something Doug
can check rather than be told.**

Also a small confirmed data point about the rendered surface, which Claude had
flagged as the risk in this tour: **the incidence view's matching markers are legible
enough at 48x48 to spot a single missing one.** That was not obvious in advance.

**Two tour holes, not one — and only the loud one got logged at first.**

1. **Loud:** `Matching ▶` hidden when Structural is singular → `docs/ideas.md` #44.
   Noticed immediately because it forced an admission into the tour.
2. **Quiet:** `hrw://` cannot address a sub-tab, so Stops 2, 5 and 6 degrade to
   "same tab → click **Incidence** / **Reduction ▶** / **Aliases ▶** / **Matching ▶**
   yourself." The tour has **2 working links and 4 prose hand-offs.** This one went
   unrecorded until Doug asked whether tour holes were being tracked — which is
   exactly why quiet holes need the same discipline as loud ones. Both are now rows
   in `tech-debt.md`'s tour-holes table.

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

---

## 2026-08-12 — `connect-expansion.md`, the tour's own opening sentence

**The first question from the nine-tour walk, and it is about wording rather than about a count.**

**Doug, verbatim:** *"In the connection tour, you wrote: 'it is one edge in a graph, and the graph is
solved before any equation exists.' Which graph are you referencing?"*

**Traces back to:** the tour's lead paragraph, before Stop 1 — so the very first sentence of prose in
the tour, not a stop.

### Which shape of question this is

**A question revealing imprecise wording, not a misconception.** Doug did not misunderstand the
phase; he correctly detected that the sentence names neither the graph nor the operation and asked
which one was meant. That is the cheapest of the four shapes to fix and the easiest to have missed —
**nothing checks prose for underspecification**, and the sentence reads fluently, which is exactly
what let it ship.

**Two things were wrong with it:**

1. **"a graph" was ambiguous** across the tour set. Three graphs appear in the nine tours, and this
   sentence sits in the first of them — so the ambiguity is worst exactly where the reader has the
   least context to resolve it.
2. **"solved" is not an operation on a graph.** What happens is that the **connected components** are
   computed. The word was doing rhetorical work and carrying no meaning, which is the failure mode
   `CLAUDE.md` names for log lines — *"a claim that reads nicely is still a claim"* — pointed at
   prose.

### What the answer had to contain

The useful answer was not just "the connection graph" but the **disambiguation his question
implies**, because the distinction is load-bearing for the two tours he walks next:

| graph | where | vertices | edges | question asked | algorithm |
|---|---|---|---|---|---|
| connection graph | Flatten | connector variables | the `connect` statements | connected components | union-find |
| incidence graph | Structural | equations ∪ unknowns (bipartite) | "this equation mentions this unknown" | maximum matching | augmenting paths |
| dependency digraph | Structural | equations | derived from the matching, **directed** | strongly connected components | Tarjan |

**The insight worth keeping from this exchange:** rows 1 and 3 are both *"find the components"*, and
the only difference is **undirected versus directed** — which is precisely why one is union-find and
the other is Tarjan. That single contrast connects Flatten to BLT across four tours, and it existed
nowhere in the tour set before this question.

### Verified against the source, not against prose

`crates/rumoca-phase-flatten/src/connections/mod.rs` — `struct UnionFind` with path compression and
union-by-rank, over `VarName` indices. **Two of them**, `potential_uf` and `stream_uf`
(`connections/mod.rs`, the builder's parameters), and `ConnectionSet` carries
`kind: {Flow, Potential, Stream}`. *Inferred, not traced: that a Flow set shares its membership with
the Potential set rather than having a union-find of its own. The per-set arithmetic (n−1 potential +
1 flow) is trace-verified; that implementation detail is not.*

### What was changed

The lead paragraph now names the graph, its vertices and its edges, says **components are computed**
rather than "solved", and adds the three-graph note so the ambiguity cannot recur for the next
reader. Per this file's standing rule, the tour says it was reworded and why.

**What this says about the tours' grading criterion:** the question is evidence the tour is *being
read closely* — it came from the lead paragraph, which a skimmer skips entirely. That is a better
signal than agreement would have been.

---

## 2026-08-16 — three questions from a *re-walk*, and a fifth question shape

**Doug, verbatim, in one morning:**

1. *"There's no hint provided in the HRW UI as to why this is a state instead of an algebraic."*
2. *"When creating connection equations, why is a zero-flow equation attempted?"*
3. *"Why is it ok for no equation to be created for an unconnected potential variable, yet an
   equation must be created for an unconnected flow variable?"*

**Traces back to:** none of them to a stop. All three came from **panes**, during a second pass
through `connect-expansion.md` — the variable grid, the connections replay's `frame[14]`, and the
asymmetry between two frame kinds.

### The new shape, and why it needed its own entry

The four shapes recorded above are all ways a tour can be **deficient** — imprecise wording, a
missing definition, a concept with no counterpart on screen, a hidden feature. These three are
none of those. **They are questions the tour deliberately does not answer, asked by someone the
tour has already succeeded on.**

Question 3 is the clearest case. It is only *askable* by someone who already holds the *n*−1
versus exactly-1 rule, because the asymmetry it asks about **is** that rule. The tour taught the
premise; the question is the reader operating on it.

**So the grading criterion gains a positive signal it did not have.** `CLAUDE.md` records that
*"no questions at all is ambiguous and must not be read as success"*. The converse was never
specified. It now is: **detailed questions arriving from the panes after a completed walk are the
success signal**, and they are distinguishable from the deficiency shapes by a test — *could this
question have been asked before the tour?* If no, the tour worked.

### What made each click

1. **The state question:** naming the *mechanism* rather than the category — a variable is a state
   exactly when some equation differentiates it — and then citing `f_x[14]` so he could go read it.
   The physical gloss did the rest: energy storage is what puts a derivative in an equation.
2. **Zero-flow:** the counting argument. A connector costs two unknowns; connection equations pay
   for them; an unconnected one still costs and must be paid another way.
3. **The asymmetry:** two answers, and the second was the one that landed. First, that the
   potential *already has* an equation — from the component's own physics — verified by showing the
   matching, `inertia.flange_b.phi ← f_x[1] (equation from inertia)`. Second, that the rule is not
   special at all: **(n−1) + 1 evaluated at n = 1 gives 0 potential equations and 1 flow equation.**
   The thing he was asking about was the rule he already knew, at its smallest case.

### Doug's own account of the loop, which is the reason this entry exists

> *"You created a first draft of the connections tour, with the assumption that I knew nothing
> about connections. Then, I began walking the tour and iterating with you to improve that tour.
> And during those iterations, I gained the basic understanding of connections which was the goal
> of the tour. Now, I'm going back through the tour, am using HRW's panes to think of more detailed
> (not-so-basic) questions, and am asking those questions here."*

**Three phases, and the third had not been described before.** The repair loop as teaching was
recorded 2026-08-15 (`vision.md`); this adds what comes *after* a tour is finished, and it is the
phase that justifies the tours being short. A tour that answered question 3 pre-emptively would
have spent his attention before he had a reason to want it.

**The transferable rule for tour authoring:** *a tour's job is to make the reader able to ask the
next question, not to answer it.* That is the operational meaning of "not too little, not too
much", and it gives the boundary a test rather than a feeling.

---

## 2026-08-22 — the SAME sentence as 2026-08-12, because the repair did not survive a rewrite

**Doug, verbatim:** *"In the overview, there's an assertion that the connect statement is an edge in
a graph. That seems wrong. Are the number of graphs which are involved determined by the number of
the types of connector variables which are involved? In other words, for this example, isn't there a
graph of voltage variables and a graph of current variables?"*

**Traces back to:** `connect-expansion.md`'s lead paragraph — **the same sentence as the 2026-08-12
entry above** — and to `the-concepts.md`'s three-graphs table, which carried the same flaw in its
*edges* column. **Medium:** HRW tour, first question of the re-walk. **Repeat: second asking of one
sentence, ten days apart.**

### This is the repeat case, and the cause is NOT that the concept is hard

`CLAUDE.md` says a repeat branches two ways — the concept is hard, or the thing is not visible.
**Neither applies.** Doug did not merely detect the imprecision this time; he proposed the correct
model himself and was essentially right. The concept landed. **The prose regressed.**

**The regression is in the commit history, not inferred:**

| commit | date | the lead paragraph |
|---|---|---|
| `bd84defb` | 08-08 | *"one edge in a graph, and the graph is **solved** before any equation exists"* |
| `21f7cbb0` | 08-12 | **the repair** — a block quote naming the graph, its vertices *"one per member of every connector instance"*, its edges, and its undirectedness |
| `2dfbb504` | **08-13** | **the repair is gone.** Back to *"one edge in a graph … until that graph has been **solved**"* |

**The 2026-08-12 fix survived one day.** The rewrite that made this tour "the template for all other
tours" reintroduced almost the original sentence, *including* the word "solved" that the fix had
specifically removed. Nothing failed, because **no checker reads prose for meaning** — the link
checker, the group-table checkers and the catalogue check were all green across that rewrite.

**The rule this buys, and it generalises past tours:** *a repair with no checker is undone by the
next rewrite of the file it lives in, and the loss is silent.* The repo already knows this shape for
code (the must-fire rule) and for claims of absence (the `<!-- unbuilt: -->` tag). **Prose repairs
have no such mechanism**, and this is the first measured instance of one being lost.

### What the answer had to contain — and where Doug's model needed one adjustment

**He was right that there is more than one graph, and right about which two, for this example.** The
adjustment: the count is the number of connector **members**, not the number of **kinds**. Give a
connector two potential members and a flow member and there are **three** graphs, two of them
potential-kind. The kind never splits a graph — it decides *what equation the finished component
generates*: *n* − 1 equalities for potential, exactly 1 sum for flow.

`Pin` has two members, one of each kind, so **"members" and "kinds" coincide in `RcCircuit` and the
example cannot discriminate them.** That is why the imprecise sentence survived two readings.

### Verified against the source — and it SETTLES the inference flagged on 2026-08-12

The 2026-08-12 entry closed with an explicit *"Inferred, not traced: that a Flow set shares its
membership with the Potential set rather than having a union-find of its own."* **That inference was
wrong, and it is now traced.** This file is append-only, so the correction is recorded here rather
than in that entry.

`crates/rumoca-phase-flatten/src/connections/mod.rs` builds **three** independent union-finds, one
per kind, and the flow one is *not* derived from the potential one:

- `potential_uf` — **global**, with the comment that merging or splitting scopes is equivalent
  because *"N-1 for N variables either way"*.
- `stream_uf` — global.
- **flow** — `connect_primitive_vars` pushes flow pairs into a `flow_pairs` vector rather than
  unioning them, and each **scope** then builds a fresh `scope_uf` from its own pairs. **Flow sets
  get their own connected-components run, under a different scoping rule from potential.**

And the reason no graph ever mixes members: `connect(a, b)` enumerates `a`'s sub-variables, takes
each one's **suffix**, and pairs it with the variable in `b` having the matching suffix
(`find_matching_var_b_indexed`). **No operation in the phase can produce a `.v — .i` edge.**

### What was changed

The lead paragraph now says *"an edge in each of several graphs, one per member of the connector"*
and restores **connected components are computed** in place of "solved". `the-concepts.md`'s table
row now says *"one edge per connector member, per `connect`"* — the vertices column was already
right, and only the edges column undercounted.

`connect-expansion.md` Act 1 has stated the correct version throughout, including on 2026-08-13
while the intro contradicted it — **so the tour disagreed with itself for nine days and nothing
could say so.**

### Doug reclassified it, and the reclassification is the reason the rest of this entry exists

**Doug, verbatim, 2026-08-22:** *"My biggest learning investment during the early stages of this
project is my phase 2 walks of tours. During those phase 2 walks, my learnings are captured in the
form of corrected prose in the tours. That corrected prose serves two purposes: 1. You are able to
use that as a measurement of what I've learned and know. 2. I'm able to use the tour as a trusted
reference during phase 3 walks. Losing the phase 2 prose is a seriously bad regression."*

*(Quoted verbatim. The phase numbering was retired 2026-08-23 for colliding with Rumoca's own
compiler phases — "phase 2" here is **walking**, "phase 3" is **exploring**.)*

**Claude had filed this as imprecise wording — the cheapest of the four shapes.** It is not. Corrected
tour prose **is the artifact a walk produces**, and it is load-bearing twice over: it is the
only measurement of what Doug knows, and it is what exploring treats as trustworthy. **A lost correction
is lost learning, not a lost sentence.**

### The audit that reclassification prompted — one confirmed loss, not a systemic one

**The dangerous pattern is specific: a whole-document rewrite landing *after* a walk.** Found by
listing every tour commit deleting ≥ 20 lines. Four tours had that exposure:

| tour | rewrite | walked before it? | verdict |
|---|---|---|---|
| **connect-expansion** | `2dfbb504` **−213** (08-13) | corrected **08-12** | **LOST** — this entry |
| index-reduction | `404779ee` −139, `3bb69db0` −131 | corrections landed **08-18, after** | safe |
| dae-construction | `41f90923` **−286** (08-16) | walked 08-03 | substance survived |
| matching | `117effa0` **−390** (08-17) | earlier walk | substance survived |

**Both large conversions are clean.** `dae-construction` lost the *wording* of its states passage and
kept the mechanism — *"a variable is a state exactly when some equation **differentiates** it"*.
`matching` went **466 → 195** lines and still teaches both threads that looked dropped:
equations-are-not-assignments in five places, and *"a rank deficiency of 1 means exactly one such
pair"* — the answer to the 2026-07-29 rank question, intact. **The template conversions compressed;
they did not delete.**

### The audit's own weakness IS the finding

Verdicts above were reached by grepping distinctive phrases and judging "substance survived" by eye.
**Nothing in the repository marks which prose came out of a walk**, so Claude cannot reliably separate
his own draft from Doug's correction on a walk — and a rewrite sees uniform prose and treats it
as uniformly Claude's to replace. That is exactly what happened on 08-13.

**Both of Doug's stated purposes rest on that missing mark:**

- **Measurement** is not merely at risk, it has **never been available**. Reading the tours today
  measures Claude's drafts back to himself.
- **Trusted reference** needs the page to say which sentences Doug validated, and no page says.

**So marking a walk's prose is not only protection — it is the missing index of the learning**, and it
is the prerequisite for purpose 1 rather than an improvement to it.

### What was done about it

**Doug authorised the checker the same day.** `<!-- walked: -->` regions, over the existing
`tests_guarded_regions` machinery, which already diffs a marked region against `HEAD` and fails by
name in the FAST suite. **It does not forbid changing walked prose** — Doug rewrites his own prose
constantly and that must stay cheap. It forbids changing it *silently*, the
`app_does_not_regrow_its_field_count` shape.

**Which passages get marked is Doug's ruling, not Claude's.** Marking a draft as walked would defeat
purpose 1 quietly, which is the failure this whole entry is about. Agreed approach: **mark during the
walk, when a correction is made**, and backfill only the ledger-recorded ones.

## 2026-08-30 — the ten walked regions of `connect-expansion` are superseded, on Doug's instruction

**Recorded here because `walked_prose_never_changes_silently` requires it.** That check treats a
deleted `walked:` region as a regression — *"that prose is a record of what Doug learned"* — and
allows exactly one alternative: **agree with Doug that it is superseded, and say so here.**

**His instruction, the day tour prose became pointable:** *"I want you to disregard the edits that
I made to the connections tour and re-write the tour using our new pedagogical agreement. The edits
which I made no longer make sense."*

### Why they no longer make sense, which is the part worth keeping

The 2026-08-22 walk produced corrections to prose written under the **old** constraint: a tour
could not be asked questions, so exposition had to pre-empt them. **Most of those corrections were
improvements to explanation** — a clearer account of nodes versus connection sets, a fuller
statement of the type-compatibility gap, the residual form spelled out. Under
[`../DECISIONS.md`](../DECISIONS.md), 2026-08-30, that whole category of prose is now written to
**provoke** questions instead. Corrections that made an explanation *more complete* are corrections
to a thing the tour no longer tries to be.

### What was NOT superseded, and was preserved verbatim

- every `**Predict.**`, `**Expected:**` and `**Falsified if**` — the walk's tests;
- all five guarded tables (`pane-frames`, two `pane-groups`, two `pane-origins`), which are
  machine-checked against a real compile;
- the factual corrections his walk produced, as opposed to the expository ones — notably that
  **"node" is the reader's bookkeeping and appears nowhere in Rumoca or HRW**, which the old prose
  originally got wrong and which survives in the rewrite as three sentences rather than a section.

### The ten regions retired, named individually because that is what licenses each one

`walked_prose_never_changes_silently` accepts a deletion only when the slug appears here — **the
slug, not the tour**. A blanket *"I rewrote this tour"* must not license removing regions nobody
noticed were there, so naming each forces the author to look at what is being retired. That
mechanism was added the same day, having been promised by the check's own message and never built.

- `opening-what-connect-is`
- `the-type-claim`
- `node-is-the-readers-bookkeeping`
- `stop1-nodes-versus-sets`
- `stop2-potential-versus-flow`
- `stop2-what-the-two-kinds-produce`
- `stop3-the-same-asymmetry-other-side`
- `stop4-a-resistor-is-seven-equations`
- `stop4-the-why-column-is-the-definition`
- `stop5-no-connect-no-expansion`

### The markers came off rather than being re-dated

**Re-dating would have asserted he had walked prose he has never seen.** A `walked:` marker means
*this passage was walked and corrected*; the rewrite has been neither. New markers get added when
he walks it. That is the honest reading of the mechanism, and the alternative — a marker carrying
today's date on text written today by Claude — is precisely the false claim the marker exists to
prevent.

**451 lines to 341.** The tour is first because he asked for it first: *"I've been focused on that
tour, trying to get this tour format right."*
