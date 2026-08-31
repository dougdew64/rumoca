# Fixture tours — tests you walk

**Purpose:** what a fixture tour is, how to walk one, and the rules for writing another.
**Status:** 👤 reference, written for a human.
**Read when:** about to walk a tour, or about to write one. **These are tests, not
explanations** — that distinction governs everything below.

## What a fixture tour is

**A short sequence of clickable stops through HRW's own views, each with an expectation that
can fail.** You pick one from the row of tours at the top of Tour mode and click through it.

They exist because of a gap nothing else covers: **Claude cannot see the rendered UI.** The
test suite checks HRW's logic; a fixture tour checks that clicking the thing does the thing.
That is the half of verification only a human can do, and these make it cheap to do.

**They are versioned and kept**, unlike an *ad hoc* tour (`.hrw-bridge/tour.md`, gitignored,
regenerated per question). The difference is not permanence for its own sake — it is that a
fixture tour has **pass/fail criteria** and an ad hoc tour has prose, and prose rots. This
project retired 1,632 lines of explanation for that reason, and deleted a 1,071-line tour that
described a 7×7 matrix on a tab showing 48 equations.

**Only justified because something runs them.** `fixture_tour_links_all_resolve` parses every
link in this directory on every test run, so a vocabulary change breaks the build rather than
breaking a document quietly. **A saved tour nobody runs is stored prose with extra steps.**

### And what a tour *is*, which is a different question from what it looks like

**A tour is a document that makes claims about what a program does.** A **fixture** tour makes them
*durably*, so they must be kept true; an **ad hoc** tour makes them *about the moment*, so they need
only be true when written.

*(Written 2026-08-22, after Doug noticed that `connect-expansion.md` states node sizes of 2, 2 and 3
as static text — true or false whether or not HRW is open and whether or not `RcCircuit` has ever
been compiled. The definition above is operational, and says nothing about this.)*

**THE SECOND HALF WAS FIRST WRITTEN AS *"while the program is not running"*, AND DOUG CORRECTED IT
THE SAME DAY:** ad hoc tours are authored *while HRW runs*, to explain what it is doing —
`tour::poll` picks up `.hrw-bridge/tour.md` within a second and auto-selects it. **He was right,
and the correction is worth more than the fix**, because "is the program running" was a proxy for
the thing that actually matters.

**What actually differs is the gap between when a claim is WRITTEN and when it is READ.**

| | fixture tour | ad hoc tour |
|---|---|---|
| written | once, against a compile | now, against what is on screen |
| read | months later, repeatedly | seconds later, once |
| relation to the program | a **copy** of what it did | an **observation** of what it is doing |
| failure mode | **staleness** — true when written, false later | **misreading** — wrong on arrival |
| defended by | checkers, markers, the gate | nothing, and nothing is needed |

**Duplicated truth is what rots, and an ad hoc tour duplicates nothing.** Its lifetime is seconds:
it is discarded before the world can move under it. That is why it is gitignored and unchecked —
not laxness, but that there is no gap for staleness to live in.

**And the two failure modes are genuinely different, which is the part to keep.** A fixture tour can
be perfectly written and *later* false. An ad hoc tour can never go stale — but it can be **wrong
on arrival**, if Claude misreads the bridge or invents what HRW did not say. No checker catches
that either; the difference is that Doug finds out in the next sentence rather than in three months.

**Everything in this directory manages the fixture side of that split**, which is where the rot is.

### Which channel is live, and when the other one becomes live

**Doug, 2026-08-22:** *"Right now while I'm beginner mode and just learning the basics, I'm entirely
using fixture tours. Eventually, after I've learned all that the fixture tours have to offer, I'll
begin using the ad hoc tours which you author to help answer my advanced questions."*

**So fixture tours are the whole channel today, and ad hoc tours are a capability held in reserve.**
Two consequences worth acting on:

- **Do not push an ad hoc tour at a question the fixture tours already cover.** Answering in the
  channel he is not yet using trades a durable, checked artifact for an ephemeral one.
- **`docs/ideas.md` #42's investment case is not yet due.** The ad hoc channel is built and works;
  it becomes the *primary* one only when the fixtures are spent. **The signal to watch is his
  questions outrunning the tours**, which `docs/question-ledger.md` is where to notice.

**So tour content sits in one of three tiers, and a writer should know which one a sentence is in:**

| tier | example in `connect-expansion.md` | kept true by |
|---|---|---|
| **checked against a real compile** | the five `<!-- pane-* -->` tables; the `2, 2, 3` set sizes | a slow test that compiles the specimen and compares |
| **checked structurally** | every `hrw://` link, the stop catalogue | fast tests — links resolve, `CATALOGUE.md` is current |
| **prose** | *which* connectors sit on node A; every explanation | **nothing. Only the walk.** |

**THE COUNTS ARE THE CHEAP PART TO KEEP TRUE, AND THE ARGUMENT IS THE EXPENSIVE PART.** A number
can be re-derived from a compile and compared. *"Nothing downstream ever groups connectors"* cannot
be checked by anything here — **and it is the sentence that actually teaches.** So the checkers
protect a tour's *facts* and leave its *reasoning* entirely to the walk, which is why Doug's
*"I couldn't have guessed that"* is worth more than any test in this repository.

**A MARKER IS NOT THE ONLY WAY A NUMBER IS CHECKED, so do not read an unmarked table as unchecked.**
The node table at the top of `connect-expansion.md` carries no marker, yet its sizes *are* verified —
`tour_node_sizes_match_the_connection_replay` asserts `potential == [2, 2, 3]` and
`flow == [2, 2, 3]` against the real connection frames, hard-coded in the test with a failure
message naming the stop. What that test does **not** check is the *mapping*: that node A is
`src.p, R.p` rather than some other pair. **Before trusting or editing a number, find what checks
it** — the marker, a named test, or nothing.

**AND THE LIMIT OF ALL OF IT, which the fidelity work is what makes it safe:** these checkers verify
that the **tour agrees with the pane**, never that the **pane agrees with the compiler**. If HRW
ever misrepresented what Rumoca did, a tour written against it would faithfully record the
misrepresentation and every checker here would go on passing. That is why accuracy in `worker.rs`
outranks everything, and why `--features notebook-check` — pane against a fresh compile — is the
instrument for the other half.

## Claude is the tour guide: drafting, walking, exploring

**Agreed with Doug 2026-08-23**, replacing an earlier numbering. **Claude writes the tours and
guides Doug through them**; the three activities are not stages of polish — **each measures
something the others cannot.**

| activity | what happens | what it measures | instrument |
|---|---|---|---|
| **drafting** | the guide writes the tour | correctness, and structural discipline | **Rumoca + HRW** — a real compile, the checkers |
| **walking** | Doug walks it, iterating with the guide | **effectiveness** — does it land | **Doug, and only Doug** |
| **exploring** | Doug leaves the route and asks what the tour never asked | **coverage** — what the guide failed to write at all | **Doug's questions** |

> **Why not "phases".** They were numbered 1/2/3 until 2026-08-23. **"Phase" already means something
> important here** — Rumoca has eleven compiler phases and the tours are *about* them — so "phase 2"
> named two unrelated things in one sentence. The numbers also implied a sequence that does not
> hold: a tour is walked and explored at the same time, and exploring produces prose that has to be
> walked. **Drafting, walking and exploring say what they are and collide with nothing.**

**The walk is the we-do of a textbook's I-do / we-do / you-do** — [`../vision.md`](../vision.md)
works out which of those three HRW should try to win, and which to concede.

### ⟶ THE WALK *IS* THE LEARNING — read this before anything else here

**Doug, 2026-08-22:** *"Most of my conceptual learning happens when iterating with you during
walks to improve the correctness and effectiveness of tours. Making the tour prose correct
and personally effective during those walks is my primary learning exercise right now."*

**So the tour is a byproduct, not the deliverable.** That is `working-with-doug.md`'s standing
principle — *the conversation is the instrument; code changes are a byproduct of understanding* —
applied to tours. **A finished tour is the residue of a learning session, not its purpose**, which
is why the three activities are worth keeping distinct and why three consequences bind:

- **A DRAFT IS NOT TRYING TO BE UNIMPROVABLE.** A draft with nothing left to iterate on would
  delete the exercise. **But the answer is not worse drafts** — it is drafts whose remaining
  weaknesses are **conceptual rather than mechanical.** Arguing about a wrong count, a dead link or
  a stop in the wrong order teaches Doug nothing about the compiler; arguing about *whether
  differentiating the constraint is the natural move* is the whole point. **Drafting's job is to
  spend the mechanical failure modes so the walk's iterations are all conceptual** — see
  *"Drafting aims at correct AND structurally disciplined"* below for the line that follows from it.
- **WHILE WALKING, ENGAGE — DO NOT PATCH.** The efficient reflex is: Doug says the prose is off, Claude
  rewrites it, both move on. **That reflex strips out the learning.** When he pushes back, say why
  it was written that way, what the alternative costs, and where the concept actually sits — and let
  him push again. **Slower on purpose**, because the dialogue is the instrument and the edit is the
  residue.
- **AND TOUR PROSE IS NOT WORKDAY WORK.** The two-mode split (`CLAUDE.md`) puts refactoring and bug
  hunting in Doug's workdays. It now has a reason beyond scheduling: **improving an explanation
  alone consumes the material his learning runs on.** Fixing a checker-caught number, a dead link or
  a stale citation is fine. **Rewriting an explanation unsupervised is not Claude's to do.**

### ⟶ WRITE TO PROVOKE QUESTIONS, NOT TO PRE-EMPT THEM — 2026-08-30

**Doug can select any sentence in the tour panel and ask about it** (the 🎯 capture). That changes
what exposition is for. His words: *"you can be more terse in your tour prose as I can always ask
you questions about your prose. The tours can focus more on being tours and less on being like
textbooks."* The pedagogical argument — that this **strengthens** we-do and you-do rather than
merely shortening the page — is in [`../vision.md`](../vision.md). The rules it produces:

1. **State the claim; do not argue it.** The argument is one question away. A paragraph that
   defends a claim nobody has yet doubted is spent attention.
2. **Naming a term is enough.** *"Rumoca computes them with union-find"* needs no paragraph on
   union-find. A term he does not know is a question he can now ask against the sentence that
   used it — which is a better teaching moment than a definition he did not ask for.
3. **Exposition serves the PREDICTION, not the concept.** Include what he needs to predict, and
   stop. Anything further front-loads the answer and weakens the step it precedes.
4. **Never terse in `**Expected:**`.** Those are the walk's test and must stay violable — rule 3
   of the three below. Terseness there destroys the falsifiability that makes a walk a walk.
5. **Fewer claims, never looser ones.** *"The system is singular"* is terse and checkable;
   *"things go wrong"* is neither. Vagueness is not brevity, and an unfalsifiable sentence fails
   the same rule an unfalsifiable expectation does.
6. **Depth goes to [`../compiler-phases/`](../compiler-phases/)**, which already exists for
   *"detail that is true but premature"* and only now becomes practical, because he can pull it
   on demand instead of meeting it uninvited.

**AND THE SAFETY ARGUMENT, which is why this is not a style preference.** An omitted explanation
costs one question; an over-reaching one costs a false belief he cannot detect. **Prose written to
pre-empt questions is prose written past what was verified.** Less exposition is less surface for
the failure this repository exists to prevent.

**Applied one tour at a time, as he walks it** — never as a campaign, for the reason the
conversion rule below already gives. `connect-expansion` is first, at his request: it is the tour
he has been using to get the format right.

### Correctness is Claude's job. Effectiveness is Doug's, and Claude cannot do it at all

**This corrects something Claude wrote on 2026-08-22** — that Doug is the instrument for whether
tour prose is *correct*. He is not, or not mainly. **Most prose claims are verifiable by reading
source**: *"nothing downstream ever groups connectors"* is checkable against
`rumoca-phase-flatten`, and a claim about phase order is checkable against the log. Slow for
Claude, but possible.

**What Claude is structurally unable to judge is whether prose works.** He knows what he meant and
cannot un-know it. That is not a care problem — it is the same shape as this repository's standing
finding that *Claude is a poor sensor for his own comprehension failures*. **Effectiveness is only
measurable by someone meeting the idea for the first time.**

**THE OPERATIONAL CONSEQUENCE, and it changes how a walk should be spent: Doug should not
fact-check.** The reports only he can produce are *"I couldn't have guessed that"*, *"I had to read
this twice"*, *"this arrived before I needed it"*, *"I don't know why you're telling me this yet."*
Those are unrecoverable if unsaid. Whether a number is right is Claude's problem and the checkers'.

**First encounter is somewhat renewable, and Claude over-weighted it.** He argued a tour can be
walked many times but *met* once, and that Doug had therefore partly spent his best measurement on
`index-reduction.md`. **Doug, 2026-08-22:** *"I often re-read articles and books, and sometimes
treat re-reads as first encounters. Let's not worry so much about that first encounter stuff."* So
it is a mild reason to make drafts good, not a resource to ration a walk around.

### Why a textbook is correct and ineffective, and why that is not a failure of effort

**Doug, 2026-08-22:** *"A textbook is typically correct, but also typically ineffective. Our
opportunity is to be correct and effective. And not merely generally effective, but personally
effective."*

**A textbook must work for a distribution of readers, and that forces three things on it:**
**completeness** (it cannot know what you already know, so it says everything), **fixed order** (it
cannot reorder for you), and **no feedback** (it cannot tell whether anything landed). **All three
constraints are lifted here.** The opportunity is not to try harder than a textbook — it is that we
are not solving the same problem.

**And there is a correctness axis a textbook cannot reach.** A textbook is right about *Modelica in
general*; a tour is right about **this compiler, on this model, in this run**, checked against a
real compile. That is what makes "correct *and* effective" a target rather than a boast.

### Tune to Doug's DURABLE profile, never to his transient state

**The danger in "personally effective", and it cuts against Doug's own goal for
`index-reduction.md`** — that it be *"the best demonstration of the value of this HRW project"* and
convince a PhD friend. Tune hard enough to one reader and nobody else can use the result.

- **Tune to the durable profile, which generalises.** Decades of C/C++/Java, new to Rust idiom and
  to Modelica compilers, basic calculus, top-down, problem before mechanism
  ([`../working-with-doug.md`](../working-with-doug.md)). That is a real archetype, not an
  idiosyncrasy, and a tour tuned to it works for a large class of engineers.
- **Never tune to transient state** — what he asked yesterday, what is on his screen now. That is
  genuinely personal and does not generalise.

**And that split is the fixture/ad hoc split**, which is a good sign both are drawn in the right
place: durable profile → fixture tour; transient state → ad hoc tour.

### Drafting aims at "correct AND structurally disciplined", not at "correct"

**Doug predicts drafts will be mostly correct and mostly ineffective. Treat that as a
prediction to fight, not a plan** — because first encounter is non-renewable, every avoidable
weakness in a draft spends a measurement that cannot be got back.

Claude cannot measure effectiveness, but three **structural proxies** are available without Doug:

- **`Predict` → look → `Expected`** forces a falsifiable expectation rather than an assertion.
- **Prose to the first prediction** bounds how much may be said before the reader tests something.
- **The tier discipline** names which sentences nothing will ever check — so they get the care.

**VERIFY THE ASSERTIONS; DO NOT PRE-EMPT THE OMISSIONS** *(2026-08-22, from what the connections
tour actually cost)*. The expensive part of that tour was never the iteration — it was **reading
Rumoca to check a claim**, writing two specimens, and running System Modeler. Those are Claude's
hours, not Doug's, which makes them the cheapest available lever on every remaining tour: **every
claim a draft makes should be checked against the source before Doug ever sees it.**

**The line, and it is easy to cross in the name of thoroughness:** verification targets *claims the
tour makes*, never *questions the tour does not raise*. Doug's connector-type question — **can a
voltage potential be connected to a mechanical one?** — was a real omission, and answering it in the
draft would have prevented him from asking it. This file already says a tour that answers a question
pre-emptively **spends his attention before he has a reason to want it.** Check what is written;
leave the gaps to be found.

### Exploring finds omissions, and its answers do NOT automatically become tour content

**Walking asks whether what is written lands. Exploring asks what was never written** — a question
the tour did not prompt is a gap the tour did not cover. It is the only one of the three that finds
omissions, which is why [`../question-ledger.md`](../question-ledger.md) says the real measure is
*the nature of the questions Doug asks*, and why **no questions at all is ambiguous and must not be
read as success.**

**Doug, 2026-08-22: *"we might or might not choose to improve tour content as the result of
[exploring] questions."*** Defend that. Such an answer routes one of three ways, and only one of
them edits the tour:

| the question is… | route |
|---|---|
| a gap at the tour's own depth | **into the tour** |
| true but premature — past this tour's level | **into [`../compiler-phases/`](../compiler-phases/)**, which exists for exactly this |
| answered and done | **nowhere.** Not every answer is tour content |

**Doug drew that line himself:** *"That distinction is past the level of useful detail for this
tour."* Routing everything into the tour is how a tour drifts to reference depth one good question
at a time.

#### The three INTERLEAVE — they describe a question, not a stage a tour is in

*(2026-08-22, from watching it happen. It is also why the numbering was dropped a day later.)*
Doug's connector-type question arrived during what looked like a walk, but it was **exploring**: a
detail the tour had never covered, asked by someone the tour had already worked on. Its answer
routed *into the tour*, which then needed ratifying on a walk — given five exchanges later.

**So `connect-expansion.md` is walked and still open to exploring at the same time**, and every
other tour will be too. There is no sequence of stages a tour passes through and finishes.

**The rule that falls out, and it is what keeps the walked markers honest:** when an answer found
by exploring routes *into the tour*, **the new prose enters UNMARKED, and becomes walked only when
Doug ratifies it.** It is Claude's draft until he has read it, whatever else in the file is already
protected.
That happened twice on 2026-08-22 — the type-claim section and the node clarification — and both
were marked only after *"the tour content looks good"*.

**The failure it prevents is the one no checker can catch:** prose written by Claude, sitting inside
a walked tour, carrying a provenance it never earned. See *Marking is Doug's ruling* above.

### The dangerous quadrant is EFFECTIVE BUT FALSE

Correctness and effectiveness come apart in both directions, and the two failures are not
symmetric:

- **True but ineffective** — the failure Doug has already corrected in Claude: accurate answers
  carrying three-graph comparison tables. It wastes a walk.
- **False but effective** — a clean analogy that lands beautifully and teaches something untrue.
  **Doug has no way to detect it precisely because it landed**, and everything he builds on it
  inherits the error.

**That is why accuracy outranks effectiveness when they conflict** (`CLAUDE.md`, and charter
Decision 7): an ineffective truth costs a walk; an effective falsehood corrupts what comes after.

**One consequence for the PhD friend, since he is the stated audience for `index-reduction.md`:**
he judges whether the claim is **credible**; he cannot measure whether it is **effective**. He reads
*"understandable with only basic calculus"* through a mind that already has the machinery and cannot
un-install it either. **He is the person to convince. He is not the instrument.**

## Walking one

1. Run HRW — `cargo run -p hrw` from the workspace root.
2. Open **Tour mode** and pick a tour from the row at the top.
3. Click each link in order and check the **Expected** line beneath it.

**Notices appear in the status bar**, along the bottom of the window. Several stops expect
one, and a reader who does not know where to look cannot check an expectation — which is a
real bug this suite has already produced.

**When something does not match, say so even if it looks minor.** Every off-stop finding so
far came from attention left spare by a short tour, which is why they stay short.

## The vocabulary — `tour`, `stop`, `observation`

**The top-level noun is `tour`, so the unit is a `stop`.** A tour has stops; that metaphor was
already chosen by the word "tour", and importing a second one (the units were briefly called
*acts*) put two metaphors on one job. The full reasoning, and the four name collisions it
uncovered, are in [`../tour-kinds-plan.md`](../tour-kinds-plan.md).

| word | what it is | whose it is |
|---|---|---|
| **tour** | a sequence of stops with one goal | the repository's |
| **stop** | a question, and something to look at | the document's |
| **observation** | what was found, and whether it matched | **Doug's** |
| **guide** | who answers what the document cannot | Claude's role |

**`stop` is a noun only for a tour stop.** The *verb* is free — *"the compile stops at Parse"*
cannot be misread. This matters because two other things in this project legitimately stop: a
**compile** (say *halts*, or *not reached*) and a **debugger** (say **break**, at an **anchor**).
`matching-live.md` is the one document where all three are in play, and it opens with a note
naming them.

## The kinds

**Every kind has stops. What varies is the activity at them** — Doug's model, 2026-08-17.

| kind | tours | the activity at a stop | goal |
|---|---|---|---|
| **concept** | 10 | prose → **Predict** → Look → confirm or reject | teach one step of the chain |
| **feature** | 3 | **do** the action → check what happened | verify one HRW capability |
| **failure** | 6 | **read** the diagnosis → check what it says | show what a broken model looks like |
| **adjudication** | 2 | **ask another implementation** | settle what HRW cannot settle |
| **hub** | 1 | none — a table of links | route into the concept tours |
| **ad hoc** | live | anything | answer the question just asked |
| **bug report** | none yet | narrate a failure for a recording | hand a maintainer a reproduction <!-- unbuilt: bug_report_tour --> |

**Each tour declares its kind machine-readably**, immediately under the H1:

```markdown
<!-- kind: concept -->
```

Greppable by a checker, and invisible in the pane — **but not because markdown hides it.**
`egui_commonmark` renders an HTML comment as *literal text*, so `TourState::poll` strips every
`<!-- … -->` span out of the tour before anything sees it, and the file on disk keeps the marker
for the checkers that read it there.

**That distinction is load-bearing**, and it is written here because the opposite was asserted
first: this line used to read *"invisible in the pane"* as though it were a property of the
format. It was an assumption, never checked, and Doug found the tag sitting under the title of
every tour. The pre-existing markers — `pane-groups`, `pane-origins`, `unbuilt:` — had been
rendering for weeks; they went unnoticed only because they sit beside tables mid-document.
`ui_tests::a_tour_renders_none_of_its_html_markers` now fails if any marker reaches the pane.
Without it, no check can tell *"a concept tour missing its predictions"* from *"a feature tour
correctly having none."*

### The invariant is `Expected`, not `Predict`

**Counted, not asserted:** `Predict` appears **zero** times in all 12 non-concept tours, and
**once per stop** in all 10 concept tours. No gradient, no partial cases.

So **`Expected` — a violable claim — is what every stop of every kind owes**, and it is what
makes a tour a *test* rather than an explanation. `Predict` is merely how a **concept** tour
earns its Expected. A feature tour earns the same claim by having you *do* the action; a failure
tour by having you *read* the diagnosis.

**This corrects a framing that was steering work.** The template below used to be presented as
the shape of *every* tour, "applied as tours are touched" — which read as *"the other twelve are
unconverted."* They are **differently designed**, and conversions stop at the concept tours.

### Feature tours — the subject is HRW

Each verifies one feature. A failed stop implicates exactly one thing.

| Tour | Verifies |
|---|---|
| [`node-pointing.md`](node-pointing.md) | pointing at a tree node, and following an identifier |
| [`frame-seeking.md`](frame-seeking.md) | stopping an animation on a given frame; addressing an equation |
| [`camera-aiming.md`](camera-aiming.md) | whether the canvas camera lands where a link says |

### Adjudication tours — the subject is a question HRW cannot settle

| Tour | Settles |
|---|---|
| [`structural-vs-numerical-rank.md`](structural-vs-numerical-rank.md) | full structural rank with numerical singularity — two stops in HRW, then a notebook |
| [`the-oracle.md`](the-oracle.md) | a model Rumoca accepts and System Modeler rejects |

**These mark every stop with the instrument it uses** — 📐 HRW, ⚙ System Modeler, 🧮 Wolfram — so
the activity varies *within* the tour, not only between tours. The convention was invented ad hoc
and is written down here because it turned out to be the clearest thing in the corpus.

### Concept tours — the subject is the compiler, and HRW is the instrument

Each teaches one step of
[`the-chain-of-problems.md`](../compiler-phases/the-chain-of-problems.md). **The prose is
load-bearing** (Doug, 2026-08-03): a stop is the explanation, and the pane is the evidence for
it. These are longer than a feature tour on purpose.

| Tour | Teaches |
|---|---|
| [`dae-construction.md`](dae-construction.md) | how the flat model becomes states/algebraics/parameters + residuals, why the equation count must equal the unknown count, and what an unbalanced model actually means — with excursions to Wolfram and System Modeler |
| [`matching.md`](matching.md) | **animation-based** — bipartite matching by augmenting-path search: greedy success, the moment the algorithm backs up and re-homes an earlier assignment, and a system that is square but structurally singular |

**An animation-based tour pauses on algorithm *steps*, not panes.** Its links are
`hrw://stage/<Stage>/<View>/frame/<n>`, and the frame numbers come from
`cargo run -p hrw --example frame_index -- <Model>`, which prints the ready-made link under each
step. **Do not transcribe the frame number by hand** — links are 1-based and the internal step
list is 0-based, and that tool spent a day telling authors otherwise.

**Why the "keep it narrow" rule below does not bind these.** That rule protects *attention per
expectation*, because a feature tour spends your surplus attention on finding off-stop bugs
in HRW. A concept tour is spending it on the concept instead. The rule it does keep is the
one that matters for both: **claims stay austere and trace-sourced, however long the prose
gets.** Length is bought with explanation, never with hedging.

#### A tour's job is to make the reader able to ask the next question, not to answer it

**Doug, 2026-08-16, having re-walked `connect-expansion.md` and then asked three detailed
questions from the panes:**

> *"You created a first draft of the connections tour, with the assumption that I knew nothing
> about connections. Then, I began walking the tour and iterating with you to improve that tour.
> And during those iterations, I gained the basic understanding… Now, I'm going back through the
> tour, am using HRW's panes to think of more detailed (not-so-basic) questions."*

**Three phases, and the third is what makes short tours correct:**

| phase | what happens | where the learning is |
|---|---|---|
| 1 | Claude drafts, assuming no knowledge | nowhere yet — a draft is a hypothesis |
| 2 | Doug walks it and iterates with Claude | **here** — the repair loop is the teaching (`vision.md`) |
| 3 | Doug re-walks, reads the panes, asks detailed questions | **here** — and the tour deliberately does not answer these |

**Walking tests the prose. Exploring tests the instrument** *(observed 2026-08-16, when
`connect-expansion.md` became the first tour to see all three)*. On a walk he follows the
tour, so the tour is what fails. Exploring, he leaves the route — clicking links out of order,
reading panes the prose never mentions, hovering things — so the **panes and the navigation** are
what fail. That day's exploring produced three teaching answers and, alongside them: a missing UI
explanation, a bridge that had stopped publishing what a pane drew, three dead scroll areas, tour
links that worked once per session, link navigation broken for nine of eleven stages, and a
divider that misremembered its width.

**None of those are connection-specific.** They are shared surfaces, so the expectation for the
*next* tour is that exploring finds far fewer — and if it does not, the finding is that exploring
exercises something the tests still cannot reach, which is worth more than the individual bugs.

#### The walk cost nothing the second time — first evidence that the template transfers

**`dae-construction.md`, walked 2026-08-17.** Doug: *"It works correctly. And, it is effective. It
seems to follow the tour template very well. Just enough instruction and no more."* **Zero
corrections.**

That is a different result from `connect-expansion.md`, whose walk took most of a day of
iteration — and the difference is not the subject matter. It is the first tour *written* under the
template, by an author who had already walked one and knew what the reader would
know. Which is the claim the template was making, now with one instance behind it.

**But do not read the absence of questions as success**, which `question-ledger.md` states as a
standing rule: *"No questions at all is ambiguous and must not be read as success."* What counts
here is his **explicit** report that it was effective — an assessment, not a silence. The
detailed questions come from exploring, which has not happened for this tour yet, and exploring is
also where the panes get stressed rather than merely followed.

**So the honest status is: walked clean, exploring outstanding.** One tour has seen all three
(`connect-expansion.md`); this one has been walked only.

**The test for "not too little, not too much" is therefore operational, not aesthetic:** *could
this question have been asked before the tour?* One of the three that morning — *why must an
unconnected flow variable get an equation when an unconnected potential need not?* — is only
**askable** by someone already holding the *n*−1 versus exactly-1 rule, because the asymmetry it
asks about **is** that rule. A tour that pre-emptively answered it would have spent his attention
before he had a reason to want it.

So **write to the point where the reader can generate the question, and stop.** The answer belongs
in the conversation, where it can be shaped by what he actually noticed —
charter Decision 8's split, arriving from the other direction: *the noun is assembled by mouse,
the verb is an unbounded utterance.*

The full account, including what made each of the three questions click, is in
[`question-ledger.md`](../question-ledger.md), 2026-08-16.

#### Two passes over every subject: the idea, then the code

**Agreed with Doug 2026-08-15**, generalising a split that had already happened by accident
between [`matching.md`](matching.md) and [`matching-live.md`](matching-live.md):

> *"The connections tour which we just completed is about the concepts, math and algorithms, but
> not about the rumoca code… We're going to make two walking passes through the tour subjects.
> During the first pass, we will focus on concepts, math and algorithms… During the second pass,
> we will focus on the rumoca code."*

| pass | subject | lab | example |
|---|---|---|---|
| **1** | concepts, mathematics, the algorithm as an idea | the HRW pane | `connect-expansion.md` |
| **2** | how Rumoca implements it | the source and the debugger | `matching-live.md` |

**Pass 2 is not defined by the debugger**, even though `matching-live.md` is a debugger tour.
Stepping is the sharpest instrument for reading an algorithm's *behaviour*; it is useless for
why a phase is organised as it is, why a type sits at one IR boundary and not another, or why an
origin is a `String` on one side of DAE construction and an enum on the other. Those are read,
not stepped. **So `-live` in a filename names the instrument, not the pass** — prefer a suffix
that names the pass for new ones, and leave `matching-live.md` alone rather than break its links.

**The template still applies, with the lab swapped.** *Predict → look → falsified if →
explanation after.* Predict what a function returns, which branch runs, what the union-find holds
at this step — then step, and check. A pass-2 tour that merely narrates source is prose competing
with the reader's own editor.

**What this fixes, and it is the reason to adopt it rather than a nicety.** The depth rule sends
premature detail to [`../compiler-phases/`](../compiler-phases/), which is raw, unsummarised, and
**read by nobody** — a graveyard dressed as a database. Under two passes that same material is
the *source* for a pass-2 tour, so exiling it is deferral rather than disposal. It also lets a
pass-1 tour be **ruthless** about excluding implementation detail, because there is somewhere
specific for it to go.

**Write pass 2 when he starts pass 2**, not alongside pass 1. The rule to write while you still
know what should happen is satisfied by capturing the material in `compiler-phases/` as it comes
up; composing it into a walk before it is wanted is building what nobody has asked to read.

Cross-platform tours may route through Wolfram Desktop or System Modeler when the point cannot
be made in HRW. Their notebooks are versioned in [`notebooks/`](notebooks/) — a *fixture*
notebook is kept for the same reason a fixture tour is, while an ad hoc notebook is ephemeral.
Claude evaluates every cell through the kernel first, then ships them for **you** to evaluate:
the stop that lands is the one you check yourself.

## Rules for writing one

### `index-reduction.md` CARRIES A HARDER BAR THAN THE OTHERS — Doug, 2026-08-21

**His words:** *"amongst all of the tours, my hope is for the index reduction tour to be the best.
Incredible, actually. That tour will serve as the best demonstration of the value of this HRW
project."* And the standard, which he has staked in public: *"I mentioned to a PhD Modelica friend
of mine that I am working with you to create an explanation of index reduction that anybody with
an understanding of only basic calculus can understand. I intend to prove to him that we can
accomplish that."*

**Treat "basic calculus only" as a CONSTRAINT, not an aspiration, because it is checkable.** It
names precisely what the tour may assume — derivatives, the chain rule, what integrating means —
and therefore what it may **not** assume without building it first:

- **DAE index as a formal object.** It is *defined* in the tour, as a distance, from what an
  integrator can be asked for.
- **Jacobian singularity, Newton iteration, structural rank.** The 2026-08-18 walk rebuilt the
  central argument on **matching**, which Doug had already walked — the same fact reached by
  counting rather than by linear algebra.
- **Pantelides by name**, or any algorithm invoked as an authority rather than shown.

**The bar is PREDICTION, not comprehension, and this is where a correct tour can still fail.** A
PhD reader will accept the current text as true. The test is harder: a reader who has never met a
DAE must be able to predict what the next pane shows *and be right*. Accuracy does not imply that,
and no checker in this repository can measure it — **only the walk can.**

**The three corrections the last walk produced are this constraint firing**, and they are the
worked examples of it: *solver* used where *integrator* was meant (wrong difficulty named); a
replacement that assumed Newton and Jacobians (rebuilt on matching); and backward references that
**retold** earlier results in prose, including a hand-written table duplicating the Incidence view.
Doug: *"HRW is your platform. Use it."*

### The rule the others now serve: prose to the first PREDICTION, then the pane

**Agreed with Doug 2026-08-12, and concept tours are to be written on this assumption.**
It came out of his own account of walking the first two: the tours felt like **books** —
gaps, and a struggle to read — while the conversation was unlike a **lecture**, because
questions get answered *and the tour gets fixed*. See
[`../vision.md`](../vision.md), "Why this beats books and lectures".

**The RHS is a laboratory, not an illustration.** Doug: *"the RHS will be partly helpful for
you to demonstrate what your tour prose attempts to explain, and the RHS will be mostly
helpful as a kind of lab for me to explore and test my expectations."*

**So the unit of a curriculum stop is a prediction the reader commits to before looking**, and
the prose before it exists only to make that prediction possible:

1. **Explain to the first point where a prediction is possible — then stop.** Not to
   comprehension. *"There should be three groups"* is crude, falsifiable and available after
   two sentences, and checking it is the fastest way to find out those two sentences were
   misread. **Prose that continues past the first prediction is prose competing with the
   lab.**
2. **State what would FALSIFY it, not only what to see.** A description invites agreement; a
   prediction invites a look. This is the existing violability rule with its purpose widened —
   it was written so a tour could **test HRW**, and it turns out to be how Doug learns, so it
   is now doing two jobs and gets stricter rather than looser.
3. **Explanation comes AFTER the look**, not before. Explaining first leaves nothing to be
   wrong about, which is comfortable and teaches less.
4. **A stop whose pane cannot falsify anything does not belong in a tour.** Move it to
   [`../compiler-phases/`](../compiler-phases/) as prose. A tour is for claims a pane can
   refute.

**And the reader audits the prose, which is the half Claude cannot do.** Every *count* in
these tours is read from a generated trace and is sound. Doug: *"if ever during that
learning process I find that the RHS does not agree with the prose, I will report that to
you."* So a stop should make disagreement easy to notice, which is the same demand as
rule 2.

**Rendering claims are no longer wholly unverified — as of 2026-08-13, for published
panes** *(this paragraph used to say every one of them was, and that is now false)*. HRW
publishes the pane on screen to `.hrw-bridge/view.json` as **the renderer's own input**,
and because that value is a pure function of a headless compile, a test can check a tour
against it. So:

- **A pane with a `to_bridge_json` gets a group table**, marked `<!-- pane-groups -->`, and
  `doc_citations::tour_group_tables_match_the_real_equation_sheet` verifies every label and
  count against a real compile. Today that is **Flatten → Equations** only.
- **Everything else is still unverified**, and so is everything about *pixels* on any pane —
  whether a `category` is drawn as a heading, whether rows are legible, whether something is
  scrolled out of view. The checker verifies **content, never rendering**.

**What this cost to learn:** Doug walked `connect-expansion.md` against the real pane on
2026-08-13 and found **six** disagreements in one sitting — wrong group headings, a layout
implied by a table that had no counterpart, and a claim that two renderings of an equation
lived in different panes when both are columns of the same row. Four of the six are now the
kind a test catches.

## The templates — one per kind

**Every kind's template ends the same way: a claim that can fail.** What differs is how the stop
earns it. Each template below is **derived from tours that already work**, not designed — read the
named exemplar before writing a new tour of that kind.

### Concept — `connect-expansion.md`

**Doug, after walking it: *"That is the template for all other tours."*** — and after the drafting
sweep: *"all of your phase 1 concept tours have been great. You have completely nailed that
format."* *(His words, 2026-08-17, before the numbering was retired; "phase 1" there is drafting.)*

**This template is frozen.** It is validated by Doug's walks, which is the one signal Claude cannot
generate, so it changes only on his report. The shape of every stop:

```markdown
## Stop N — <a question, not a topic>

<setup: the least that makes the prediction possible>

> **Predict.** <a question with a committed answer>

[Look — <Specimen> → <Stage> → <SubView>](hrw://load/…)

**Expected:** <the answer, exact>

**Falsified if** <what would refute it>

### What just happened

<the explanation, only now>
```

**Five things make it work, and four of them are not the format:**

1. **Stops chain.** Each prediction is answerable from the previous stop's *result* — nodes in
   Stop 1 become the input to Stop 2's equation count, which becomes Stop 3's row-pairing. **A tour
   whose stops could be reordered is a list of observations**, which is what this one was before.
   *(The chaining is a property of the content, not of the word: it survived the rename from "act"
   and must not be lost with it.)*
2. **Every term is defined at first use, and one word never does two jobs.** This tour needs three
   levels — **connector**, **node**, **connection set** — and conflating any two of them broke it
   three separate times. Fixing the wording was never enough; the levels had to be named.
3. **Say where a claim is *not* visible.** A flow set of *n* prints as one row naming all *n*; a
   potential set prints as *n* − 1 pairs and its size appears nowhere. Stating that turned the
   tour's most persistent confusion into its spine. **If a number you assert cannot be found on the
   screen, say so in the stop that asserts it.**
4. **Numbers are declared falsifiable up front.** The tour opens by saying its counts come from
   generated traces and asks to be told when one disagrees — which is what makes the reader an
   instrument rather than an audience.
5. **No historical asides.** See below.

**A `Stop 0` is legitimate and carries no prediction** — it is setup, with an expectation to check
and nothing to predict. `matching-live.md` and `frame-seeking.md` both have one.

### Feature — `node-pointing.md`

**No prediction, and that is correct.** You are being asked to *do* the action; there is nothing to
guess, because the point is whether clicking the thing does the thing.

```markdown
## Stop N — <the action, imperatively>

[<the link that performs it>](hrw://stage/Structural/Tree/node/…)

**Expected:** <what changes on screen, precisely enough to be wrong>
```

**Keep it narrow — one capability per tour.** The scarce resource is attention per expectation, and
a failed stop in a narrow tour implicates exactly one feature. And **say where to look**: several
stops expect a status-bar notice, and a reader who does not know that reports "nothing happened".

### Failure — `failure-parse.md`

**You read a diagnosis rather than predicting one.** The specimen is stated up front with the line
that breaks it, because the interest is in what the compiler *says* and how far it gets.

```markdown
**Specimen:** `<Model>` — <the one thing wrong with it>

**The question to hold:** <what the reader should be wondering>

## Stop N — <what this pane reveals>

[<load link>](hrw://load/…)

**Expected:** <the diagnosis, or "not reached", exactly>
```

**Close with `## What to bring back`** — open questions for Doug, since a failure tour's real
output is a design opinion about whether the diagnosis is actionable.

### Adjudication — `the-oracle.md`

**The stop's activity is asking a different implementation**, so every heading carries the
instrument as an emoji: 📐 HRW, ⚙ System Modeler, 🧮 Wolfram.

```markdown
## 📐 Stop N — <what HRW claims>

**Expected:** <HRW's answer>

## ⚙ Stop N+1 — Ask the other implementation

**Expected:** <what the other tool says, and whether they agree>
```

**Claude evaluates every notebook cell through the kernel first**, then ships it for Doug to
evaluate — the stop that lands is the one he checks himself. Fixture notebooks are versioned in
[`notebooks/`](notebooks/); ad hoc ones are ephemeral.

### Bug report — not built <!-- unbuilt: bug_report_tour -->

A tour that narrates the steps of a failure for a screen recording, to hand a Rumoca maintainer a
reproduction. **No template until an instance exists** — writing one from imagination is how a
convention becomes load-bearing before it is known to work.

**And it is the first kind whose reader is not Doug.** Its audience is maintainers, which flips who
judges it under the two-audience rule ([`../../DECISIONS.md`](../../DECISIONS.md)): the test becomes
*"did a maintainer act on it without asking Claude?"* — so **Doug's walk cannot validate one.**

### Keep the tour's history out of the tour

**A tour is written for Doug; a changelog is written for Claude.** No *"reworded after Doug
asked"*, no *"corrected 2026-08-13"*, no dated parentheticals. They accumulated to eight in one
file and made it read as a maintenance log.

That history is not lost, it is **filed where it belongs**: the decision and its reasoning in
[`../../DECISIONS.md`](../../DECISIONS.md), the question that prompted it in
[`../question-ledger.md`](../question-ledger.md), and the mechanism in a code comment or
[`../compiler-phases/`](../compiler-phases/). A tour states what is true now.

### A tour the overview links into must link back — with `hrw://`, twice

**Doug, 2026-08-17:** *"There's a top-level tour which links to subordinate tours. I really want
to be able to navigate backward from a subordinate tour to the top-level tour so that I can then
navigate downward to another subordinate tour."*

[`the-concepts.md`](the-concepts.md) is a **hub**: ten rows, each an `hrw://tour/<name>`
link into a phase tour. Those links ran one way only, so walking the chain meant reopening the
picker between every pair — with the hub sitting alphabetically among its own children, at
position 21 of 23.

**The convention is two back-links per tour**, and each placement answers a different moment:

```markdown
# Fixture tour — <phase>: <the idea>

[The chain overview](hrw://tour/the-concepts)
```

- **After the H1** — for *"wrong tour, take me back"*, before any reading has happened.
- **In the closing section** — `Or go back up: [The chain overview](hrw://tour/the-concepts)`
  — for the reader who finished and wants the next phase.

**AND THAT GOVERNS EVERY LINK IN A TOUR, NOT ONLY LINKS TO TOURS** — a rule stated three times
about whichever file type was in front of us, and evaded three times by the next one:

| to reach | write |
|---|---|
| another tour | `hrw://tour/the-concepts` |
| a doc under `hrw/docs/` | `hrw://doc/upstream-issues.md` — nesting allowed |
| a Wolfram notebook | `hrw://notebook/structural-vs-numerical-rank.nb` |
| a web page | an ordinary `https://` link — **the browser is right here** |

`doc_citations::no_tour_links_to_a_bare_file_path` fails on anything else, in the fast suite, on
the day it is written; **the three defects and the reasoning live on that test**, not here.

`doc_citations::every_tour_the_overview_links_to_links_back` derives the list from the overview's
own links and reports the two failures separately, because *no way back* and *a way back that
goes nowhere* look identical in a diff.

**And the hub sorts first in the picker**, with a separator beneath it
(`TourState::picker_order`). That was chosen over a dedicated "up" button because the transport
bar already sets the panel's width floor and another control raises it — and because a capability
tour has no parent to go up to, so the button would be dead most of the time.

### The rules this rests on

**One capability per tour, and keep it narrow.** The scarce resource is **attention per
expectation**, not the number of walks. A wide tour consumes the surplus that produces
off-stop findings rather than multiplying them, and a stop failure in a narrow tour implicates
exactly one feature.

**Every `**Expected:**` line must be violable.** Write what would be *different* if the
feature broke — a number, a named field, "nothing moves", "the counter goes down". *"Mostly
collapsed"* where the truth is **fully** collapsed tests nothing, and hedged expectations
teach the reader to skim, which defeats the point.

**An expectation must say WHERE to look**, not only what to look for. A stop was once
correctly refused with the reason on screen, and reported as "nothing happened", because the
tour never said notices live in the status bar.

**Write the tour while you still know what should happen.** Both the worst expectations ever
shipped here described behaviour Claude had *not* just built.

**Past ten or so fixtures this needs a selection principle** — walk whatever just changed,
plus one stale one — and **visible staleness**: nothing currently catches a tour whose
*expectations* rot, only its links. "Last walked" is derivable from the `tour-link` entries in
the action trail, and nobody has built it yet. <!-- unbuilt: last_walked -->

## Further reading

- 👤 [`../architecture.md`](../architecture.md) — how tour mode and the `hrw://` link
  vocabulary work
- 👤 [`../../README.md`](../../README.md) — what HRW is, and the capture plan that keeps
  screenshots honest by taking them at these stops
