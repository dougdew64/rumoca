# Fixture labs — tests you run

**Purpose:** what a fixture lab is, how to run one, and the rules for writing another.
**Status:** 👤 reference, written for a human.
**Read when:** about to run a lab, or about to write one. **These are tests, not
explanations** — that distinction governs everything below.

> ### ⚠ THIS FILE IS MID-REVISION UNDER CHARTER DECISION 14 — read that first
>
> **A walk is a lab session, not a reading.** Doug's 🎯 capture made the conversational loop part
> of the run, and Decision 14 (September 1, 2026) **retires the model most of this file was
> written for** — that a lab's prose must stand alone, explain every term before use, and
> pre-empt every question.
>
> **The lab now supplies the route, the checkpoints and the machine-checked claims; Claude
> supplies the explanation on demand.** Claude holds two distinct roles: **lab guide** before the
> run, **lab instructor** during it.
>
> **Fifteen sections below carry rulings older than 2026-08-30 and assume the retired model.**
> Until this file is revised, **read any rule requiring self-sufficient prose as superseded** —
> do not attempt to reconcile it with Decision 14. Everything about *accuracy* still binds without
> exception; Decision 14 loosens what prose must carry, never what it may claim.

## What a fixture lab is

**A short sequence of clickable stops through HRW's own views, each with an expectation that
can fail.** You pick one from the row of labs at the top of Lab mode and click through it.

They exist because of a gap nothing else covers: **Claude cannot see the rendered UI.** The
test suite checks HRW's logic; a fixture lab checks that clicking the thing does the thing.
That is the half of verification only a human can do, and these make it cheap to do.

**They are versioned and kept**, unlike an *ad hoc* lab (`.hrw-bridge/lab.md`, gitignored,
regenerated per question). The difference is not permanence for its own sake — it is that a
fixture lab has **pass/fail criteria** and an ad hoc lab has prose, and prose rots. This
project retired 1,632 lines of explanation for that reason, and deleted a 1,071-line lab that
described a 7×7 matrix on a tab showing 48 equations.

**Only justified because something checks them and someone runs them — and those are different
halves.** `fixture_lab_links_all_resolve` parses every link in this directory on every test run,
so a vocabulary change breaks the build rather than breaking a document quietly. **That guards the
plumbing, not the claims.** The expectations are executed by **Doug running the lab**, which is
the half of verification only a human can do. **A saved lab that is neither checked nor run is
stored prose with extra steps.**

### And what a lab *is*, which is a different question from what it looks like

**A lab is a document that makes claims about what a program does.** A **fixture** lab makes them
*durably*, so they must be kept true; an **ad hoc** lab makes them *about the moment*, so they need
only be true when written.

*(Written 2026-08-22, after Doug noticed that `connect-expansion.md` states set sizes of 2, 2 and 3
as static text — true or false whether or not HRW is open and whether or not `RcCircuit` has ever
been compiled. The definition above is operational, and says nothing about this.)*

**THE SECOND HALF WAS FIRST WRITTEN AS *"while the program is not running"*, AND DOUG CORRECTED IT
THE SAME DAY:** ad hoc labs are authored *while HRW runs*, to explain what it is doing —
`lab::poll` picks up `.hrw-bridge/lab.md` within a second and auto-selects it. **He was right,
and the correction is worth more than the fix**, because "is the program running" was a proxy for
the thing that actually matters.

**What actually differs is the gap between when a claim is WRITTEN and when it is READ.**

| | fixture lab | ad hoc lab |
|---|---|---|
| written | once, against a compile | now, against what is on screen |
| read | months later, repeatedly | seconds later, once |
| relation to the program | a **copy** of what it did | an **observation** of what it is doing |
| failure mode | **staleness** — true when written, false later | **misreading** — wrong on arrival |
| defended by | checkers and the gate | nothing, and nothing is needed |

**Duplicated truth is what rots, and an ad hoc lab duplicates nothing.** Its lifetime is seconds:
it is discarded before the world can move under it. That is why it is gitignored and unchecked —
not laxness, but that there is no gap for staleness to live in.

**And the two failure modes are genuinely different, which is the part to keep.** A fixture lab can
be perfectly written and *later* false. An ad hoc lab can never go stale — but it can be **wrong
on arrival**, if Claude misreads the bridge or invents what HRW did not say. No checker catches
that either; the difference is that Doug finds out in the next sentence rather than in three months.

**Everything in this directory manages the fixture side of that split**, which is where the rot is.

### Which channel is live, and when the other one becomes live

**Doug, 2026-08-22:** *"Right now while I'm beginner mode and just learning the basics, I'm entirely
using fixture labs. Eventually, after I've learned all that the fixture labs have to offer, I'll
begin using the ad hoc labs which you author to help answer my advanced questions."*

**So fixture labs are the whole channel today, and ad hoc labs are a capability held in reserve.**
The consequence worth acting on:

- **Do not AUTHOR a second document where a fixture lab already covers the ground.** Writing a
  whole ad hoc lab over material a fixture lab already carries trades a durable, checked artifact
  for an ephemeral one. **The waste is the duplicate document, never the answer** — answering
  Doug's question mid-run *is* the run (charter Decision 14), and produces no artifact to
  duplicate.
**So lab content sits in one of three tiers, and a writer should know which one a sentence is in:**

| tier | example in `connect-expansion.md` | kept true by |
|---|---|---|
| **checked against a real compile** | the five `<!-- pane-* -->` tables; the `2, 2, 3` set sizes | a slow test that compiles the specimen and compares |
| **checked structurally** | every `hrw://` link, the stop catalogue | fast tests — links resolve, `CATALOGUE.md` is current |
| **prose** | *which* members are in which set; every explanation | **nothing. Only the run.** |
| **the conversation** | whatever Claude says when Doug presses 🎯 | **nothing. Only Doug, in the moment.** |

**THE FOURTH ROW IS NEW, AND IT IS WHERE THE EXPOSURE MOVED** *(added 2026-09-01 with charter
Decision 14)*. Decision 14 shifted the teaching out of the prose and into the conversation, which
shrinks tier 3's volume without changing its status — and adds a tier that is **unchecked,
unversioned, and delivered at the moment Doug is most receptive.** That is exactly the *effective
but false* quadrant he cannot detect. **Claude being present to answer is not a substitute for a
checker; it is the reason one is needed.**

**THIS IS THE STANDING LOOP TARGET** *(Doug's ruling, 2026-09-01)*. When effort is available for a
new feedback loop, it goes to the bottom two rows, because that is where the teaching now happens
and where nothing currently looks. **The lever already exists and is `MATH-INSPIRED,
CODE-GROUNDED` below**: a claim naming `generate_equality_equations` can be wired into
`doc_citations`, and a claim about "graphs" can never be. **So the way to check prose and
conversation is not to check prose and conversation — it is to require that they name code**, and
then check the names. Grounding is the mechanism, not the manners.

**THE COUNTS ARE THE CHEAP PART TO KEEP TRUE, AND THE ARGUMENT IS THE EXPENSIVE PART.** A number
can be re-derived from a compile and compared. *"Nothing downstream ever groups connectors"* cannot
be checked by anything here — **and it is the sentence that actually teaches.** So the checkers
protect a lab's *facts* and leave its *reasoning* entirely to the run, which is why Doug's
*"I couldn't have guessed that"* is worth more than any test in this repository.

**A MARKER IS NOT THE ONLY WAY A NUMBER IS CHECKED, so do not read an unmarked table as unchecked.**
Station 1's set sizes in `connect-expansion.md` carry no marker, yet they *are* verified —
`lab_set_sizes_match_the_connection_replay` asserts `potential == [2, 2, 3]` and
`flow == [2, 2, 3]` against the real connection frames, hard-coded in the test with a failure
message naming the stop. What that test does **not** check is the *mapping*: that the first set is
`src.p.v, R.p.v` rather than some other pair. **Before trusting or editing a number, find what checks
it** — the marker, a named test, or nothing.

**AND THE LIMIT OF ALL OF IT, which the fidelity work is what makes it safe:** these checkers verify
that the **lab agrees with the pane**, never that the **pane agrees with the compiler**. If HRW
ever misrepresented what Rumoca did, a lab written against it would faithfully record the
misrepresentation and every checker here would go on passing. That is why accuracy in `worker.rs`
outranks everything, and why `--features notebook-check` — pane against a fresh compile — is the
instrument for the other half.

## Claude is the LAB INSTRUCTOR: drafting, running, exploring

**Charter Decision 15 renamed this role, and it was a correction rather than a relabelling.** A
lab guide talks continuously, is never surprised, and is followed rather than consulted — none of
which is the job. **A lab instructor's job is specific: the student is at the bench, the apparatus
did something unexpected, and the instructor helps find out why.** It also says what Claude is
*not*: Cellier is the textbook author, and the lecture lives elsewhere.

**Agreed with Doug 2026-08-23** as three activities, **reframed 2026-09-01** under Decisions 14 and
15. They are not stages of polish — **each measures something the others cannot.**

| activity | what happens | what it measures | instrument |
|---|---|---|---|
| **drafting** | the instructor writes the lab | correctness, and structural discipline | **Rumoca + HRW** — a real compile, the checkers |
| **running** | Doug works the apparatus, iterating with the instructor | **effectiveness** — does it land | **Doug, and only Doug** |
| **exploring** | Doug asks what the protocol never asked | **coverage** *and* **the instrument** — what the instructor never wrote, and what the panes do off the route | **Doug's questions — the 🎯 capture** |

**EXPLORING CHANGED STATUS, AND THAT IS THE SUBSTANTIVE EDIT HERE.** It used to mean *leaving the
route* — a bonus, and slightly a failure of the route to be complete. **Under Decision 14 the route
exists in order to provoke it**, so exploring is the intent rather than the departure, and a
protocol nobody leaves is a protocol that pre-empted its own questions. Note that this row named
its instrument *"Doug's questions"* on 2026-08-23, a week before the 🎯 capture existed. **The row
predicted the mechanism.**

**A NOTE ON VOCABULARY WHILE THE RENAME IS PENDING** *(2026-09-01)*. Decision 15 binds the
sequence: reimagine now, rename atomically later. So **role and activity names are corrected here
immediately, because they were wrong** — Claude is not a lab guide — **while the artifact is still
spelled `lab` throughout this file and in every `hrw://lab/<name>` link**, and changes in one
atomic pass. That mixture is deliberate and bounded, not drift.

> **Why not "phases".** They were numbered 1/2/3 until 2026-08-23. **"Phase" already means something
> important here** — Rumoca has eleven compiler phases and the labs are *about* them — so "phase 2"
> named two unrelated things in one sentence. The numbers also implied a sequence that does not
> hold: a lab is run and explored at the same time, and exploring produces prose that has to be
> run. **Drafting, running and exploring say what they are and collide with nothing.**

**The run is the we-do of a textbook's I-do / we-do / you-do** — [`../vision.md`](../vision.md)
works out which of those three HRW should try to win, and which to concede.

### ⟶ RUNNING A LAB *IS* THE LEARNING — read this before anything else here

**Doug, 2026-08-22:** *"Most of my conceptual learning happens when iterating with you during
runs to improve the correctness and effectiveness of labs. Making the lab prose correct
and personally effective during those runs is my primary learning exercise right now."*

**So the lab is a byproduct, not the deliverable.** That is `working-with-doug.md`'s standing
principle — *the conversation is the instrument; code changes are a byproduct of understanding* —
applied to labs. **A finished lab is the residue of a learning session, not its purpose**, which
is why the three activities are worth keeping distinct and why three consequences bind:

- **A DRAFT IS NOT TRYING TO BE UNIMPROVABLE — DRAFTING IS THE INSTRUCTOR'S PREP.** A draft with
  nothing left to iterate on would delete the exercise. **But the answer is not worse drafts** — it
  is drafts whose remaining weaknesses are **conceptual rather than mechanical.** **This is what a
  lab instructor does before class: check the apparatus, so the session is spent on the physics and
  not on a broken meter.** Arguing about a wrong count, a dead link or a stop in the wrong order
  teaches Doug nothing about the compiler; arguing about *whether differentiating the constraint is
  the natural move* is the whole point. **Drafting's job is to spend the mechanical failure modes so
  the live iterations are all conceptual** — see *"Drafting aims at correct AND structurally
  disciplined"* below for the line that follows from it.
- **WHILE RUNNING ONE, ENGAGE — DO NOT PATCH.** The efficient reflex is: Doug says the prose is off, Claude
  rewrites it, both move on. **That reflex strips out the learning.** When he pushes back, say why
  it was written that way, what the alternative costs, and where the concept actually sits — and let
  him push again. **Slower on purpose**, because the dialogue is the instrument and the edit is the
  residue.
- **AND LAB PROSE IS NOT WORKDAY WORK.** The two-mode split (`CLAUDE.md`) puts refactoring and bug
  hunting in Doug's workdays. It now has a reason beyond scheduling: **improving an explanation
  alone consumes the material his learning runs on.** Fixing a checker-caught number, a dead link or
  a stale citation is fine. **Rewriting an explanation unsupervised is not Claude's to do.**

  **THE LINE IS REDRAWN IN LAB TERMS** *(Doug, 2026-09-01, in light of Decision 14)*. The old
  wording listed three permitted repairs and forbade one act, which left everything unlisted
  undecided. **The test is now the one this repository already uses for unsupervised document work:
  RESTORE, NEVER CHOOSE.**

  | | unsupervised | why |
  |---|---|---|
  | a number, link, citation, table, vocabulary slip | **free** | a checker caught it or could; there is one right answer to restore |
  | a route broken by something that moved — a dead pane, a renamed stage | **free** | restoring apparatus, not designing an experiment |
  | **the explanation** | **Doug's** | it is the material his learning runs on |
  | **the predictions and `Expected:` lines** | **Doug's** | **Decision 14 makes these the pedagogical core**, not the prose around them |
  | **which specimen, which panes, in what order** | **Doug's** | that is designing the experiment |

  **Decision 14 moved the explanation into the conversation, which shrinks how much of a lab is
  prose — it does not license editing what remains.** If anything it *narrows* the free column,
  because a prediction now carries the teaching that a paragraph used to. **When the answer is
  "choose", it is Doug's; when it is "restore", it is Claude's.**

### ⟶ WRITE TO PROVOKE QUESTIONS, NOT TO PRE-EMPT THEM — 2026-08-30

**THIS IS HOW CHARTER DECISION 14 IS EXECUTED, NOT A PREFERENCE FOR SHORTER PROSE.** Written
2026-08-30 as guidance layered on the old model; Decision 14 made it the definition of what a lab's
prose is *for*. The prose does not teach — **it provokes the exchange that teaches**, and the six
rules below are what that requires of a sentence.

**Doug can select any sentence in the lab panel and ask about it** (the 🎯 capture). That changes
what exposition is for. His words: *"you can be more terse in your lab prose as I can always ask
you questions about your prose. The labs can focus more on being labs and less on being like
textbooks."* The pedagogical argument — that this **strengthens** we-do and you-do rather than
merely shortening the page — is in [`../vision.md`](../vision.md). The rules it produces:

1. **State the claim; do not argue it.** The argument is one question away. A paragraph that
   defends a claim nobody has yet doubted is spent attention.
2. **Naming a term is enough.** *"Rumoca computes them with union-find"* needs no paragraph on
   union-find. A term he does not know is a question he can now ask against the sentence that
   used it — which is a better teaching moment than a definition he did not ask for.
3. **Exposition serves the PREDICTION, not the concept.** Include what he needs to predict, and
   stop. Anything further front-loads the answer and weakens the step it precedes.
4. **Never terse in `**Expected:**`.** Terseness there destroys the falsifiability that makes a lab
   a test — **the violability rule is stated in full under "The rules this rests on"** and is not
   repeated here. **Decision 14 raises it rather than relaxing it:** with the explanation moved
   into conversation, the checkpoint carries the teaching that a paragraph used to, which is also
   why rule 7's redrawn line makes these Doug's and not Claude's.
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

**Applied one lab at a time, as he runs it** — never as a campaign, for the reason the
conversion rule below already gives.

### ⟶ MATH-INSPIRED, CODE-GROUNDED — the rules for what a lab may claim

**Four rulings from 2026-08-30/31.** The rule is here; **the account of each — Doug's words, the
evidence, what it cost — is in [`../../DECISIONS.md`](../../DECISIONS.md) under those dates.** Do
not restate it here: this file is read before every lab edit and that one is not.

**1. Ground every claim in Rumoca's code; let the textbook supply the QUESTION, not the answer.**
An abstract claim has nothing to be wrong against, so no reading and no checker can refute it —
only a collision months later. A grounded claim is refutable in minutes, **and Doug can refute it
himself by opening the file.**

**AND IT APPLIES TO WHAT CLAUDE SAYS, NOT ONLY TO WHAT HE WRITES** *(2026-09-01)*. Decision 14 moved
most explanation into the conversation, which is the tier **nothing checks** — no checker can see
what Claude says at the bench. **Grounding is the only defence that transfers there**, because Doug
can open the file the claim named. So rule 1 is not a drafting rule that also happens to help live;
**it is the safety mechanism for the one tier that has no other.**

**2. An abstraction must pass BOTH tests before it appears at all.**

1. **Does the code already have a noun for it?** If so, use the code's noun. An abstraction is
   only ever a stand-in for something unnamed.
2. **Does it predict something the code does?** One predicting nothing is dead weight however
   honestly it is labelled.

| abstraction | verdict |
|---|---|
| **the singleton set** | Out — predicts nothing. Rumoca's union-find never has one. |
| **node** | Out — duplicates `ConnectionSet`, *and* its prediction is false at two scopes. |
| **the graph** | Out — duplicates what union-find already computes; predict from merges. |

**3. The LABEL goes in the lab; the COMPARISON goes in the conversation.** A few words stop an
abstraction impersonating the compiler. What the code stores *instead*, and what the textbook
version keeps that is never used, is a comparison — it pays only when he pulls it. **The tell is
that a clause needs a conversation to land**: that is not a gap to fill with more prose, it is
prose that should have been a name.

**4. The introduction builds the mental model; the stops reinforce or break it.** So the intro's
abstractions and the stops' predictions are **the same list seen twice** — an abstraction with no
stop is untested confidence, and a stop tracing back to no abstraction is trivia. **Select for both
jobs**: only-reinforce manufactures false confidence, only-challenge never consolidates.

**In lab terms this is the PRE-LAB BRIEFING and the EXPERIMENT**, which is what makes the
matched-lists requirement obvious rather than arbitrary: the briefing establishes what Doug expects,
and the bench confirms or refutes it. **An abstraction with no stop is a briefing for an experiment
nobody ran**; a stop with no abstraction is an experiment testing nothing anyone predicted.

**Two failure modes worth naming, because both look like knowledge.** *True, checkable and
useless* — naming a type teaches nothing; the contrast and the rationale are what land. And *the
textbook model of a data structure is not that structure's behaviour in a given program* —
recognising the algorithm is not reading it. **Read the callers.**

**The mechanical payoff, which is why this outranks style.** `doc_citations` already checks that
cited paths exist and symbols resolve, so a claim naming `generate_equality_equations` can be
wired into the gate. A claim about "graphs" can never be. This is
[`../../CLAUDE.md`](../../CLAUDE.md)'s *bias to checkable output*, applied to prose.

### Correctness is Claude's job. Effectiveness is Doug's, and Claude cannot do it at all

**This corrects something Claude wrote on 2026-08-22** — that Doug is the instrument for whether
lab prose is *correct*. He is not, or not mainly. **Most prose claims are verifiable by reading
source**: *"nothing downstream ever groups connectors"* is checkable against
`rumoca-phase-flatten`, and a claim about phase order is checkable against the log. Slow for
Claude, but possible.

**What Claude is structurally unable to judge is whether prose works.** He knows what he meant and
cannot un-know it. That is not a care problem — it is the same shape as this repository's standing
finding that *Claude is a poor sensor for his own comprehension failures*. **Effectiveness is only
measurable by someone meeting the idea for the first time.**

**THE OPERATIONAL CONSEQUENCE IS A RELEASE, NOT A DUTY: Doug should not fact-check.** Whether a
number is right is Claude's problem and the checkers'. **The things only he can see** are *"I
couldn't have guessed that"*, *"I had to read this twice"*, *"this arrived before I needed it"*,
*"I don't know why you're telling me this yet."*

*(This read "those are unrecoverable if unsaid", which made a duty roster of it. Reworded
2026-09-01 with the running discipline Doug removed: **it is a description of what is useful, not a
list of reports he owes.** Nobody learns well while filing observations.)*

**AND IT GOVERNS THE CONVERSATION, NOT ONLY THE PROSE** *(2026-09-01)*. This was written about
drafted paragraphs, but Decision 14 moved most explanation to the bench — where **both halves of
the asymmetry get worse.** Claude still cannot judge whether a live answer landed, *and* nothing
checks it. So the four reports above are owed for an answer exactly as for a paragraph: **"I had to
read that twice" is as useful said out loud at a stop as it is written against a draft.**

**First encounter is somewhat renewable**, so it is a mild reason to make drafts good, **not a
resource to ration a run around** — Doug, 2026-08-22: *"I often re-read articles and books, and
sometimes treat re-reads as first encounters."*

### Tune to Doug's DURABLE profile, never to his transient state

*(Why a textbook is correct and ineffective — HRW's whole positioning against textbooks — is in
[`../vision.md`](../vision.md). This is the rule it produces.)*

**The danger in "personally effective", and it cuts against Doug's own goal for
`index-reduction.md`** — that it be *"the best demonstration of the value of this HRW project"* and
convince a PhD friend. Tune hard enough to one reader and nobody else can use the result.

- **Tune to the durable profile, which generalises.** Decades of C/C++/Java, new to Rust idiom and
  to Modelica compilers, basic calculus, top-down, problem before mechanism
  ([`../working-with-doug.md`](../working-with-doug.md)). That is a real archetype, not an
  idiosyncrasy, and a lab tuned to it works for a large class of engineers.
- **Never tune to transient state** — what he asked yesterday, what is on his screen now. That is
  genuinely personal and does not generalise.

**THE SPLIT IS BY ARTIFACT LIFETIME, AND THERE ARE NOW THREE CHANNELS** *(corrected 2026-09-01)*.
It read *"durable profile → fixture lab; transient state → ad hoc lab"*, which was a clean
mapping while those were the only two. Decision 14 added a third — **the conversation** — and it
lands on the side this rule says never to tune to.

| channel | lifetime | tune to |
|---|---|---|
| a fixture lab — its route, checkpoints and claims | months, re-read | **the durable profile** |
| an ad hoc lab | seconds, discarded | the moment |
| **an answer at the bench, after 🎯** | the exchange | **the moment — and that is the point** |

**So the rule governs what OUTLIVES the moment, not what Claude may say in it.** What is on Doug's
screen is precisely what a captured passage is *for*; tuning an answer to it is correct. **What must
never absorb transient state is the artifact**, because it is read months later by someone — Doug
included — who no longer has that screen.

### Drafting aims at "correct AND structurally disciplined", not at "correct"

**Doug predicts drafts will be mostly correct and mostly ineffective. Treat that as a
prediction to fight, not a plan** — because an avoidable weakness spends a run on something a
checker could have caught, and the run is the scarce resource.

*(This said "first encounter is non-renewable", which Doug overturned the same day and which the
section above now records correctly: he often treats re-reads as first encounters. Corrected
2026-09-01 — the two paragraphs were one section apart and said opposite things.)*

Claude cannot measure effectiveness, but three **structural proxies** are available without Doug:

- **`Predict` → look → `Expected`** forces a falsifiable expectation rather than an assertion.
- **Prose to the first prediction** bounds how much may be said before the reader tests something.
- **The tier discipline** names which sentences nothing will ever check — so they get the care.

**VERIFY THE ASSERTIONS; DO NOT PRE-EMPT THE OMISSIONS** *(2026-08-22, from what the connections
lab actually cost)*. The expensive part of that lab was never the iteration — it was **reading
Rumoca to check a claim**, writing two specimens, and running System Modeler. Those are Claude's
hours, not Doug's, which makes them the cheapest available lever on every remaining lab: **every
claim a draft makes should be checked against the source before Doug ever sees it.**

**AND UNDER DECISION 14 IT BUYS A SECOND THING: THE ANSWER AT THE BENCH.** A claim verified while
drafting is a claim Claude can *answer from* when Doug captures the sentence and asks — grounded,
rather than reconstructed on the spot in front of him. **Pre-draft verification is therefore the
only preparation the conversation gets**, since nothing checks it live.

**The line, and it is easy to cross in the name of thoroughness:** verification targets *claims the
lab makes*, never *questions the lab does not raise*. Doug's connector-type question — **can a
voltage potential be connected to a mechanical one?** — was a real omission, and answering it in the
draft would have prevented him from asking it. This file already says a lab that answers a question
pre-emptively **spends his attention before he has a reason to want it.** Check what is written;
leave the gaps to be found.

### Exploring finds omissions, and its answers do NOT automatically become lab content

**Running a lab asks whether what is written lands. Exploring asks what was never written** — a question
the lab did not prompt is a gap the lab did not cover. It is the only one of the three that finds
omissions, which is why [`../question-ledger.md`](../question-ledger.md) says the real measure is
*the nature of the questions Doug asks*, and why **no questions at all is ambiguous and must not be
read as success.**

**Doug, 2026-08-22: *"we might or might not choose to improve lab content as the result of
[exploring] questions."*** Defend that. Such an answer routes one of three ways, and only one of
them edits the lab:

| the question is… | route |
|---|---|
| a gap at the lab's own depth | **into the lab** |
| anything else — premature, or simply answered | **nowhere.** Not every answer is lab content |
| *and separately:* it revealed something about **how Doug learns** | the **question** to [`../question-ledger.md`](../question-ledger.md) — never the answer |

**THE MIDDLE ROW USED TO SAY `../compiler-phases/`, AND THAT HOME NO LONGER ACCEPTS THE DELIVERY**
*(corrected 2026-09-01)*. That directory was described as a store for detail deferred from a lab;
it is not. **It is the closest thing that exists to Rumoca documentation** — written before HRW
existed and refreshed at the version bump — and posting lab overflow into it would corrupt the one
job it does. **Decision 14 removed the need for the deferral store anyway:** premature detail
arrives on demand when Doug asks, so it does not have to be written down first. Three routes became
two, and the ledger takes the *question*, not the explanation.

**Doug drew that line himself:** *"That distinction is past the level of useful detail for this
lab."* Routing everything into the lab is how a lab drifts to reference depth one good question
at a time.

#### The three INTERLEAVE — they describe a question, not a stage a lab is in

*(2026-08-22, from watching it happen. It is also why the numbering was dropped a day later.)*
Doug's connector-type question arrived during what looked like a run, but it was **exploring**: a
detail the lab had never covered, asked by someone the lab had already worked on. Its answer
routed *into the lab*, which then needed ratifying on a run — given five exchanges later.

**So `connect-expansion.md` is run and still open to exploring at the same time**, and every
other lab will be too. There is no sequence of stages a lab passes through and finishes.

**The rule that falls out:** when an answer found by exploring routes *into the lab*, **it is
Claude's draft until Doug has read it** — whatever else in the file he has already run. Nothing
records that distinction since the `run:` markers were retired on 2026-08-31, so **judge it from
the conversation.**

**The failure it guards against is one no checker can catch:** prose written by Claude, sitting
inside a lab Doug has run, carrying a standing it never earned.

### The dangerous quadrant is EFFECTIVE BUT FALSE

Correctness and effectiveness come apart in both directions, and the two failures are not
symmetric:

- **True but ineffective** — the failure Doug has already corrected in Claude: accurate answers
  carrying three-graph comparison tables. It wastes a run.
- **False but effective** — a clean analogy that lands beautifully and teaches something untrue.
  **Doug has no way to detect it precisely because it landed**, and everything he builds on it
  inherits the error.

**That is why accuracy outranks effectiveness when they conflict** (`CLAUDE.md`, and charter
Decision 7): an ineffective truth costs a run; an effective falsehood corrupts what comes after.
**Decision 7 states the ranking; this states why it holds** — the two failures differ in
*detectability*, not just in cost. An ineffective truth announces itself, because Doug notices he
is lost or bored. An effective falsehood is silent by construction.

**AND IT IS WORSE AT THE BENCH THAN ON THE PAGE** *(2026-09-01)*. Charter Decision 14 records this
quadrant as its binding hazard, and this section is its description: a live answer is **more
fluent, tailored to the question just asked, and arrives with no checker between it and Doug.**
Every property that makes conversation good teaching also makes a wrong answer land better. **So
the rule that governs drafted prose governs an answer at a stop, and more strictly.**

**One consequence for any EXPERT reader** — the PhD friend is the standing example, but this holds
for every credentialed reader of any lab: **he judges whether the claim is credible; he cannot
measure whether it is effective.** He reads *"understandable with only basic calculus"* through a
mind that already has the machinery and cannot un-install it either. **He is the person to convince.
He is not the instrument.**

## Running one

**Run HRW (`cargo run -p hrw`), open Lab mode, pick one.** That is the whole of it.

**THERE IS NO SESSION DISCIPLINE, AND THERE ARE NO OBLIGATIONS ON DOUG HERE** *(his ruling,
2026-09-01)*: *"that discipline was turning education into a chore, including frequent pesters from
you about the need to walk labs."* **This section used to prescribe an order to click in and
require him to report every mismatch however minor.** Both are gone. **He runs a lab when he wants
to, in whatever order he likes.**

**What he reports is unowed, not unimportant** — and the distinction matters, because the first
version of this paragraph said he *"reports whatever he feels like reporting"*, which is looser
than what Doug ruled. He removed the **obligation**. He did not say the reports stop mattering:
*"I couldn't have guessed that"* and *"I had to read that twice"* remain **the one signal Claude
cannot generate**, and nothing replaces them.

**The one thing worth knowing is apparatus, not procedure: notices appear in the STATUS BAR** along
the bottom of the window. A reader who does not know where the readout is cannot read it — a real
defect this suite has already produced. **That is the lab instructor's job, not a rule for Doug:
point at the readout before asking anyone to read it.**

**And nothing tracks or reports what has been run.** The `run:` markers were retired
2026-08-31 because *"that bookkeeping doesn't yield enough value"*, and the practice survived in
prose anyway — hand-maintained backlogs of which stops had no reader yet. **Do not reintroduce it in
either form**; judge from the conversation.

## The vocabulary — `lab`, `station`, `observation`

**SETTLED 2026-09-01 under charter Decision 15.** The old vocabulary was `lab` / `stop` /
`observation` / `guide`. **Two of those four were already lab words and did not move**, which is
the strongest evidence the metaphor fits rather than being imposed on it.

| word | what it is | whose it is | changed? |
|---|---|---|---|
| **lab** | a sequence of stations with one goal | the repository's | was `lab` |
| **station** | a question, and something to look at | the document's | was `stop` |
| **observation** | what was found, and whether it matched | **Doug's** | **already lab-native** |
| **instructor** | who answers what the document cannot | Claude's role | was `guide` |

`Predict` and `Expected` were already lab-native too. The full reasoning behind the original
choice, and the four name collisions it uncovered, are in
[`../lab-kinds-plan.md`](../lab-kinds-plan.md).

**THE VERB IS `run`, AND THE SESSION IS A `session`** *(Doug's ruling, 2026-09-01, completing the
table above)*. **`run` was a tour word and the first rename missed it** — Decision 15 settled the
nouns and never touched the verb, leaving ~350 occurrences in `docs/` and ~230 in `src/`. Charter
Decision 14 had already named the replacement: *"a walk is a **lab session**, not a reading."*

| was | is |
|---|---|
| *run a tour* | **run a lab** |
| *a run* | **a lab session** |
| `a_finished_session_returns_to_the_mode_it_started_in` | `a_finished_session_…` |

**AND `run` COLLIDES EXACTLY AS `stop` DID, so the rename is surgical, never blind.** Two senses
live in `src/`: the **lab session** (`test_set_session_state`, *"a self-running run, as the Play
button does"*) and **traversal**, which must survive untouched — `walk_modules()`, `fn walk(dir:
&Path)`, *"it walks the alias equations"*, *"walking into library class IR"*.

**WHY `station` AND NOT `step`, so nobody re-proposes it.** This section exists because `stop`
*collided*: a compile stops and a debugger stops. Any replacement had to survive that test, and
**`step` fails it far worse — 346 uses in `src/` alone**, including `fn step`, `match step`, and
single-stepping in the debugger. `run` fails the same way (586). **`station` was the only
lab-native candidate genuinely unused here: 0 in `src/`, 1 in `docs/`.**

**And the rename retires the collision rather than managing it.** With the unit called a
`station`, **`stop` becomes free for its natural senses** — a compile halting, a debugger
breaking — so `matching-live.md` no longer needs the opening note that disambiguated three
meanings. That is a gain the rename buys beyond the metaphor.

**The artifact is still spelled `lab` in this file, in `hrw://lab/<name>` links and in `src/`.**
Decision 15 binds the sequence — reimagine first, rename **atomically** afterwards — and forbids a
mechanical substitution. **This table is the target vocabulary, not a description of the tree.**

## The kinds

**Every kind has stations. What varies is the activity at them** — Doug's model, 2026-08-17.

**ONLY ONE KIND IS RENAMED UNDER DECISION 15: `adjudication` → `calibration`.** It is the one whose
old name explained nothing, and *calibration against a reference standard* is exactly what asking
System Modeler is. **`concept`, `feature` and `failure` keep their names**, because Decision 15
requires renaming what carries the *lab* metaphor and those three never did — they read correctly
in a lab already. Three lab-native alternatives were proposed and **all three collided**:
`experiment` is a **Modelica annotation**, `orientation` is rotational mechanics (184 in `crates/`),
and `diagnosis` is Rumoca's compiler diagnostics (804). The charter carries the account under
Decision 15. **Collision-check any candidate against `src/`, `crates/` and `specimens/` before
proposing it.**

| kind | the activity at a stop | goal |
|---|---|---|
| **concept** | prose → **Predict** → Look → confirm or reject | teach one step of the chain |
| **feature** | **do** the action → check what happened | verify one HRW capability |
| **failure** | **read** the diagnosis → check what it says | show what a broken model looks like |
| **calibration** *(was `adjudication`)* | **ask a reference implementation** | settle what HRW cannot settle |
| **hub** | none — a table of links | route into the concept labs |
| **ad hoc** | anything | answer the question just asked |
| **bug report** | narrate a failure for a recording | hand a maintainer a reproduction <!-- unbuilt: bug_report_lab --> |

*(The per-kind lab counts were dropped on 2026-09-01. They were hand-maintained beside the labs
they counted, and `concept` had already drifted from 10 to 11 with nothing to notice — the failure
`CLAUDE.md` names as the cheapest in this repository to leave stale. `CATALOGUE.md` is generated
and carries the current membership.)*

**Each lab declares its kind machine-readably**, immediately under the H1:

```markdown
<!-- kind: concept -->
```

Greppable by a checker, and invisible in the pane — **but not because markdown hides it.**
`egui_commonmark` renders an HTML comment as *literal text*, so `LabState::poll` strips every
`<!-- … -->` span out of the lab before anything sees it, and the file on disk keeps the marker
for the checkers that read it there.

**That distinction is load-bearing**, and it is written here because the opposite was asserted
first: this line used to read *"invisible in the pane"* as though it were a property of the
format. It was an assumption, never checked, and Doug found the tag sitting under the title of
every lab. The pre-existing markers — `pane-groups`, `pane-origins`, `unbuilt:` — had been
rendering for weeks; they went unnoticed only because they sit beside tables mid-document.
`ui_tests::a_lab_renders_none_of_its_html_markers` now fails if any marker reaches the pane.
Without it, no check can tell *"a concept lab missing its predictions"* from *"a feature lab
correctly having none."*

### The invariant is `Expected`, not `Predict`

**Counted on every test run, not asserted here:**
`doc_citations::a_lab_predicts_if_and_only_if_its_kind_says_so` enforces it — `Predict` appears
once per station in a concept lab and nowhere else. No gradient, no partial cases.

*(This paragraph used to quote the counts by hand — "all 12 non-concept labs … all 10 concept
labs". There are **11** concept labs, and the section above already explains that per-kind counts
were dropped on 2026-09-01 because `concept` drifted 10 → 11 with nothing to notice. The stale
number was sitting twenty lines below the note describing it. **A checker retires the prose it
replaces**; the reasoning lives on the test.)*

So **`Expected` — a violable claim — is what every station of every kind owes**, and it is what
makes a lab a *test* rather than an explanation. `Predict` is merely how a **concept** lab
earns its Expected. A feature lab earns the same claim by having you *do* the action; a failure
lab by having you *read* the diagnosis.

**This corrects a framing that was steering work.** The template below used to be presented as
the shape of *every* lab, "applied as labs are touched" — which read as *"the other twelve are
unconverted."* They are **differently designed**, and conversions stop at the concept labs.

**WHICH LABS EXIST IS IN [`CATALOGUE.md`](CATALOGUE.md), WHICH IS GENERATED** by
`cargo run -p hrw --example gen_lab_catalogue` and kept honest by
`app::tests::lab_catalogue_is_current`. *(Three per-kind rosters lived here until 2026-09-01 and
were deleted rather than corrected: the concept one listed **2** labs when there were **11**,
omitting `connect-expansion` — the lab in active use. A generated roster already existed. What
follows is what each kind **is**, which is a rule; membership is data.)*

### Feature labs — the subject is HRW

**Each verifies one feature. A failed station implicates exactly one thing.**

### Calibration labs *(was: adjudication)* — the subject is a question HRW cannot settle

**These mark every station with the instrument it uses** — 📐 HRW, ⚙ System Modeler, 🧮 Wolfram —
so the activity varies *within* the lab, not only between labs. The convention was invented ad hoc
and is written down because it turned out to be the clearest thing in the corpus. **Decision 15
makes it obviously right rather than merely useful:** a calibration lab moves between instruments,
so saying which one you are standing at is the point of the kind.

### Concept labs — the subject is the compiler, and HRW is the instrument

Each teaches one step of
[`the-chain-of-problems.md`](../compiler-phases/the-chain-of-problems.md). **The prose is
load-bearing** (Doug, 2026-08-03): a station is the explanation, and the pane is the evidence for
it. These are longer than a feature lab on purpose.

**DECISION 14 QUALIFIES THAT, and the qualification is narrow.** The prose is load-bearing **for
the prediction** — it must carry enough that Doug can commit to an answer before looking. The
*explanation* now arrives at the bench when he asks. So "longer on purpose" survives as a
statement about what a prediction costs to set up, **not as licence to explain in advance**;
rules 8 and 12 govern the difference.

**An animation-based lab pauses on algorithm *steps*, not panes.** Its links are
`hrw://stage/<Stage>/<View>/frame/<n>`, and the frame numbers come from
`cargo run -p hrw --example frame_index -- <Model>`, which prints the ready-made link under each
step. **Do not transcribe the frame number by hand** — links are 1-based and the internal step
list is 0-based, and that tool spent a day telling authors otherwise.

**Why the "keep it narrow" rule below does not bind these.** That rule protects *attention per
expectation*, because a feature lab spends your surplus attention on finding off-stop bugs
in HRW. A concept lab is spending it on the concept instead. The rule it does keep is the
one that matters for both: **claims stay austere and trace-sourced, however long the prose
gets.** Length is bought with explanation, never with hedging.

#### A lab's job is to make the reader able to ask the next question, not to answer it

**Doug, 2026-08-16**, re-running `connect-expansion.md` and then asking three detailed questions
from the panes: *"Now, I'm going back through the lab, am using HRW's panes to think of more
detailed (not-so-basic) questions."*

**THE OPERATIONAL TEST FOR "NOT TOO LITTLE, NOT TOO MUCH": could this question have been asked
BEFORE the lab?** One of his three — *why must an unconnected flow variable get an equation when
an unconnected potential need not?* — is only **askable** by someone already holding the *n* − 1
versus exactly-1 rule, because the asymmetry it asks about **is** that rule. A lab that answered
it pre-emptively would have spent his attention before he had a reason to want it.

**So write to the point where the reader can generate the question, and stop.** The answer belongs
in the conversation, where it can be shaped by what he actually noticed.

**RUNNING AND EXPLORING STRESS DIFFERENT SURFACES.** On a run he follows the route, so **the
prose** is what fails. Exploring, he leaves it — clicking links out of order, reading panes the
prose never mentions — so **the lab's coverage and HRW's instrument** both fail, and that day
produced three teaching answers alongside a bridge that had stopped publishing what a pane drew,
three dead scroll areas, links that worked once per session, and a divider that misremembered its
width. **None of those were connection-specific**, so the expectation for the next lab is that
exploring finds fewer — and if it does not, the finding is that exploring reaches something the
tests still cannot, which is worth more than the individual bugs.

**Do not read the absence of questions as success** — `question-ledger.md` states it as standing:
*"No questions at all is ambiguous."* What counts is an **explicit** report that a lab landed, as
`dae-construction.md` got on 2026-08-17 with zero corrections — the first evidence that the
template transfers to a lab written by an author who had already run one.


#### `matching-live.md` is a DEBUGGER lab, and that is an instrument, not a stage

*(This replaced a two-pass model on 2026-09-01. Doug had agreed on 2026-08-15 to run each subject
twice — concepts first, Rumoca's code second — and then ruled the split away: **"Let us eliminate
entirely the notion of passes through the labs. What matters most for labs is that their content
is math-inspired, code grounded."** Code-grounding applies from the first sentence of every lab,
so there is no first pass to defer code out of, and no second pass to defer it into.)*

**Stepping is the sharpest instrument for reading an algorithm's *behaviour*, and useless for
everything else** — why a phase is organised as it is, why a type sits at one IR boundary and not
another, why an origin is a `String` on one side of DAE construction and an enum on the other.
Those are read, not stepped. **So `-live` in a filename names the instrument**; leave
`matching-live.md` alone rather than break its links, and do not read the suffix as a stage.

**The template is unchanged when the lab is the debugger.** *Predict → look → falsified if →
explanation after.* Predict what a function returns, which branch runs, what the union-find holds
at this step — then step, and check. **A lab that merely narrates source is prose competing with
the reader's own editor.**

**THAT OPEN QUESTION WAS CLOSED 2026-09-01, AND BOTH HALVES OF IT DISSOLVED.** It read: the depth
rule exiles premature detail to `../compiler-phases/`, two passes made that material the source for
a later lab, and without passes the exile is disposal — so what becomes of the directory is an open
question for Doug.

**Neither premise survived.** Doug ruled that `compiler-phases/` is **Rumoca reference
documentation** — written before HRW existed, refreshed at the version bump, and kept — so it was
never the deferral store this paragraph assumed. And **Decision 14 removed the need for one**:
premature detail is not exiled, it is simply not said yet, and Doug pulls it by asking. **Nothing
is disposed of, because nothing is stored.**


Cross-platform labs may route through Wolfram Desktop or System Modeler when the point cannot
be made in HRW. Their notebooks are versioned in [`notebooks/`](notebooks/) — a *fixture*
notebook is kept for the same reason a fixture lab is, while an ad hoc notebook is ephemeral.
Claude evaluates every cell through the kernel first, then ships them for **you** to evaluate:
the stop that lands is the one you check yourself.

## Rules for writing one

### `index-reduction.md` CARRIES A HARDER BAR THAN THE OTHERS — Doug, 2026-08-21

**His words:** *"amongst all of the labs, my hope is for the index reduction lab to be the best.
Incredible, actually. That lab will serve as the best demonstration of the value of this HRW
project."* And the standard, which he has staked in public: *"I mentioned to a PhD Modelica friend
of mine that I am working with you to create an explanation of index reduction that anybody with
an understanding of only basic calculus can understand. I intend to prove to him that we can
accomplish that."*

**Treat "basic calculus only" as a CONSTRAINT, not an aspiration, because it is checkable.** It
names precisely what the lab may assume — derivatives, the chain rule, what integrating means —
and therefore what it may **not** assume without building it first:

- **DAE index as a formal object.** It is *defined* in the lab, as a distance, from what an
  integrator can be asked for.
- **Jacobian singularity, Newton iteration, structural rank.** The 2026-08-18 run rebuilt the
  central argument on **matching**, which Doug had already run — the same fact reached by
  counting rather than by linear algebra.
- **Pantelides by name**, or any algorithm invoked as an authority rather than shown.

**The bar is PREDICTION, not comprehension, and this is where a correct lab can still fail.** A
PhD reader will accept the current text as true. The test is harder: a reader who has never met a
DAE must be able to predict what the next pane shows *and be right*. Accuracy does not imply that,
and no checker in this repository can measure it — **only running the lab can.**

**AND DECISION 14 PUT THAT MEASUREMENT AT RISK, WHICH IS WORTH NAMING HERE** *(2026-09-01)*. With
the instructor present, **a failed prediction can be rescued in conversation and neither party
notices the lab is broken.** Doug cannot predict, asks, gets a good answer, understands — and the
station that should have carried him is still defective. The explanation patched the gap
invisibly.

**So: a station whose prediction only worked after Claude explained it is a DEFECT in the lab, not
a successful exchange.** It is the one place where the conversation, which Decision 14 makes the
teaching, actively hides the measurement this bar depends on. **Say so at the bench when it
happens** — that report is the instrument, and it costs nothing to make.

**The three corrections the last run produced are this constraint firing**, and they are the
worked examples of it: *solver* used where *integrator* was meant (wrong difficulty named); a
replacement that assumed Newton and Jacobians (rebuilt on matching); and backward references that
**retold** earlier results in prose, including a hand-written table duplicating the Incidence view.
Doug: *"HRW is your platform. Use it."*

### The rule the others now serve: prose to the first PREDICTION, then the pane

**Agreed with Doug 2026-08-12, and concept labs are to be written on this assumption.**
It came out of his own account of running the first two: the labs felt like **books** —
gaps, and a struggle to read — while the conversation was unlike a **lecture**, because
questions get answered *and the lab gets fixed*. See
[`../vision.md`](../vision.md), "Why this beats books and lectures".

**The RHS is a laboratory, not an illustration.** Doug: *"the RHS will be partly helpful for
you to demonstrate what your lab prose attempts to explain, and the RHS will be mostly
helpful as a kind of lab for me to explore and test my expectations."*

**THAT SENTENCE IS WHERE CHARTER DECISION 15 CAME FROM, AND IT IS DATED 2026-08-12** — three weeks
before the decision was written. **Doug called it a lab first**, the word reached
[`../vision.md`](../vision.md), and the only thing that happened on 2026-09-01 is that it finally
reached the labs themselves. The metaphor was never imposed on this project; it had been sitting in
one half of it, unapplied to the other.

**So the unit of a curriculum station is a prediction the reader commits to before looking**, and
the prose before it exists only to make that prediction possible:

1. **Explain to the first point where a prediction is possible — then stop.** Not to
   comprehension. *"There should be three groups"* is crude, falsifiable and available after
   two sentences, and checking it is the fastest way to find out those two sentences were
   misread. **Prose that continues past the first prediction is prose competing with the
   lab.**
2. **State what would FALSIFY it, not only what to see.** A description invites agreement; a
   prediction invites a look. This is the existing violability rule with its purpose widened —
   it was written so a lab could **test HRW**, and it turns out to be how Doug learns, so it
   is now doing two jobs and gets stricter rather than looser.
3. **Explanation comes AFTER the look**, not before. Explaining first leaves nothing to be
   wrong about, which is comfortable and teaches less. **Decision 14 MECHANISED this rather than
   changing it** — the 🎯 capture makes look-then-ask the default path, where it used to be a
   discipline Claude had to remember. The rule and the mechanism now agree, which is a reason to
   keep the rule stated, not to drop it as handled.
4. **A station whose pane cannot falsify anything does not belong in a lab.** **It needs no other
   home: leave it out, and let Doug ask.** *(This said "move it to `../compiler-phases/` as
   prose". That directory is Rumoca reference documentation, not an overflow store — and under
   Decision 14 unfalsifiable material does not need storing anywhere, because it arrives on demand.
   Corrected 2026-09-01; see the routing table under "Exploring finds omissions".)* A lab is for
   claims a pane can refute.

**And the reader audits the prose, which is the half Claude cannot do.** Every *count* in
these labs is read from a generated trace and is sound. Doug: *"if ever during that
learning process I find that the RHS does not agree with the prose, I will report that to
you."* So a stop should make disagreement easy to notice, which is the same demand as
rule 2.

**Rendering claims are no longer wholly unverified — as of 2026-08-13, for published
panes** *(this paragraph used to say every one of them was, and that is now false)*. HRW
publishes the pane on screen to `.hrw-bridge/view.json` as **the renderer's own input**,
and because that value is a pure function of a headless compile, a test can check a lab
against it. So:

- **A pane with a `to_bridge_json` gets a group table**, marked `<!-- pane-groups -->`, and
  `doc_citations::lab_group_tables_match_the_real_equation_sheet` verifies every label and
  count against a real compile. Today that is **Flatten → Equations** only.
- **Everything else is still unverified**, and so is everything about *pixels* on any pane —
  whether a `category` is drawn as a heading, whether rows are legible, whether something is
  scrolled out of view. The checker verifies **content, never rendering**.

**What this cost to learn:** Doug run `connect-expansion.md` against the real pane on
2026-08-13 and found **six** disagreements in one sitting — wrong group headings, a layout
implied by a table that had no counterpart, and a claim that two renderings of an equation
lived in different panes when both are columns of the same row. Four of the six are now the
kind a test catches.

### A lab the overview links into must link back — with `hrw://`, twice

**Doug, 2026-08-17:** *"There's a top-level lab which links to subordinate labs. I really want
to be able to navigate backward from a subordinate lab to the top-level lab so that I can then
navigate downward to another subordinate lab."*

[`the-concepts.md`](the-concepts.md) is a **hub**: ten rows, each an `hrw://lab/<name>`
link into a phase lab. Those links ran one way only, so running the chain meant reopening the
picker between every pair — with the hub sitting alphabetically among its own children, at
position 21 of 23.

**The convention is two back-links per lab**, and each placement answers a different moment:

```markdown
# Fixture lab — <phase>: <the idea>

[The chain overview](hrw://lab/the-concepts)
```

- **After the H1** — for *"wrong lab, take me back"*, before any reading has happened.
- **In the closing section** — `Or go back up: [The chain overview](hrw://lab/the-concepts)`
  — for the reader who finished and wants the next phase.

**AND THAT GOVERNS EVERY LINK IN A LAB, NOT ONLY LINKS TO LABS** — a rule stated three times
about whichever file type was in front of us, and evaded three times by the next one:

**This table defines the `hrw://` verbs, so it is where the rename lands.** `hrw://lab/<name>`
becomes `hrw://lab/<name>` in the atomic pass — **every occurrence rewritten, no alias** (Doug,
2026-09-01: *"we are replacing the concept of tours with the concept of labs"*).
`fixture_lab_links_all_resolve` makes that safe: a missed link fails by name rather than rotting.
The other four verbs are unaffected.

| to reach | write |
|---|---|
| another lab | `hrw://lab/the-concepts` → **`hrw://lab/the-concepts`** at the rename |
| a doc under `hrw/docs/` | `hrw://doc/upstream-issues.md` — nesting allowed |
| **a source file, at a symbol** | `hrw://src/hrw/src/bridge.rs#resolve_source` |
| a Wolfram notebook | `hrw://notebook/structural-vs-numerical-rank.nb` |
| a web page | an ordinary `https://` link — **the browser is right here** |

**Every code name a grounded lab states should be an `hrw://src` link** *(2026-08-31)* — Doug,
pointing at *"`connections/mod.rs` uses union-find"*: *"This reference and others like it would be
much more helpful as links to the code files in VS Code."* A name he cannot reach is a citation;
one he can click is the source. **Name the symbol, never a line** — the line is computed at click
time, so a link that resolves is right by construction, while `tech-debt.md`'s `worker.rs:3434`
rotted inside a day. Functions, types, enum variants and struct fields all resolve.

**This is the half that pays for grounding:** `fixture_labs_reference_files_that_exist` resolves
every one in the FAST suite, so a renamed symbol fails a test rather than a run.

**AND IT IS WHAT MAKES THE CONVERSATION SAFE, NOT ONLY THE PROSE** *(2026-09-01)*. Rule 9 records
that grounding is the only defence that transfers to the bench, because no checker can see what
Claude says there. **An `hrw://src` link is that defence made operational:** Doug can open the file
and refute the answer while it is being given. A cited symbol he cannot reach is a claim he must
take on trust.

**SYMBOL-NOT-LINE IS THE BEST DESIGN IN THIS DOCUMENT, and it is worth copying rather than merely
obeying.** Charter Decision 13 says perishable specifics do not belong in durable text. This rule
does something stronger: **it makes the perishable specific unrepresentable.** There is no way to
write a rotting line number into an `hrw://src` link, because the line is computed at click time
and a checker resolves the symbol. `tech-debt.md`'s hand-written `worker.rs:3434` rotted inside a
day; this cannot. **When building the loop target from the tier table, aim here — the best loop
does not catch the error, it removes the way to make it.**

`doc_citations::no_lab_links_to_a_bare_file_path` fails on anything else, in the fast suite, on
the day it is written; **the three defects and the reasoning live on that test**, not here.

`doc_citations::every_lab_the_overview_links_to_links_back` derives the list from the overview's
own links and reports the two failures separately, because *no way back* and *a way back that
goes nowhere* look identical in a diff.

**And the hub sorts first in the picker**, with a separator beneath it
(`LabState::picker_order`). That was chosen over a dedicated "up" button because the transport
bar already sets the panel's width floor and another control raises it — and because a capability
lab has no parent to go up to, so the button would be dead most of the time.

### The rules this rests on

**One capability per lab, and keep it narrow.** The scarce resource is **attention per
expectation**, not the number of runs. A wide lab consumes the surplus that produces
off-stop findings rather than multiplying them, and a stop failure in a narrow lab implicates
exactly one feature.

**Every `**Expected:**` line must be violable** — write what would be *different* if the feature
broke: a number, a named field, "nothing moves", "the counter goes down". *"Mostly collapsed"*
where the truth is **fully** collapsed tests nothing, and hedged expectations teach the reader to
skim, which defeats the point. **This is the one home for that rule**; the provoke-questions
section's fourth point cites it rather than restating it.

**An expectation must say WHERE to look**, not only what to look for. A stop was once
correctly refused with the reason on screen, and reported as "nothing happened", because the
lab never said notices live in the status bar.

**Write the lab while you still know what should happen.** Both the worst expectations ever
shipped here described behaviour Claude had *not* just built.

**A GAP STATED HONESTLY, WITH NO WORK QUEUED BEHIND IT: nothing catches a lab whose EXPECTATIONS
rot — only its links.** A renamed symbol fails a test; an expectation that quietly stopped being
true does not. Grounding (rule 9) narrows this, because a claim naming code can be checked, but it
does not close it.

*(Until 2026-09-01 this paragraph also prescribed a selection principle — "run whatever just
changed, plus one stale one" — and queued a `last_walked` feature derived from the action trail.
**Both were removed as running discipline**, which Doug eliminated that day: the `run:` markers
had already gone on 2026-08-31, and this was a plan to rebuild the same tracking from a different
source. The `unbuilt:` tag went with it, since an absence tag on work that must not be done reads
as a to-do a later session may pick up in good faith.)*

## The templates — one per kind

**Every kind's template ends the same way: a claim that can fail.** What differs is how the station
earns it. Each template below is **derived from labs that already work**, not designed — read the
named exemplar before writing a new lab of that kind.

> **THE SKELETONS BELOW STILL SAY `Station N`, AND THAT IS DELIBERATE UNTIL THE ATOMIC RENAME.** Rule
> 16 settled the vocabulary — the unit is a **station** — but these skeletons are *copied* when a
> new lab is written. Updating them now would produce new labs saying `Station` while all 23
> existing labs say `Stop`, which charter Decision 15 calls **worse than no rename**: two words for
> one thing, and no way to tell which is current. **Do not "fix" them individually.** They change
> in the one atomic pass, with every lab and every `hrw://lab/<name>` link.

### Concept — `connect-expansion.md`

**Doug, after running it: *"That is the template for all other labs."*** — and after the drafting
sweep: *"all of your phase 1 concept labs have been great. You have completely nailed that
format."* *(His words, 2026-08-17, before the numbering was retired; "phase 1" there is drafting.)*

**This template is frozen.** It is validated by Doug's runs, which is the one signal Claude cannot
generate, so it changes only on his report. The shape of every stop:

```markdown
## Station N — <a question, not a topic>

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
   Station 1 become the input to Station 2's equation count, which becomes Station 3's row-pairing. **A lab
   whose stops could be reordered is a list of observations**, which is what this one was before.
   *(The chaining is a property of the content, not of the word: it survived the rename from "act"
   and must not be lost with it.)*
2. **Every term is defined at first use, and one word never does two jobs.** This lab needs three
   levels — **connector**, **node**, **connection set** — and conflating any two of them broke it
   three separate times. Fixing the wording was never enough; the levels had to be named.
3. **Say where a claim is *not* visible.** A flow set of *n* prints as one row naming all *n*; a
   potential set prints as *n* − 1 pairs and its size appears nowhere. Stating that turned the
   lab's most persistent confusion into its spine. **If a number you assert cannot be found on the
   screen, say so in the stop that asserts it.**
4. **Numbers are declared falsifiable up front.** The lab opens by saying its counts come from
   generated traces and asks to be told when one disagrees — which is what makes the reader an
   instrument rather than an audience.
5. **No historical asides.** See below.

**A `Station 0` is legitimate and carries no prediction** — it is setup, with an expectation to check
and nothing to predict. `matching-live.md` and `frame-seeking.md` both have one.

### Feature — `node-pointing.md`

**No prediction, and that is correct.** You are being asked to *do* the action; there is nothing to
guess, because the point is whether clicking the thing does the thing.

```markdown
## Station N — <the action, imperatively>

[<the link that performs it>](hrw://stage/Structural/Tree/node/…)

**Expected:** <what changes on screen, precisely enough to be wrong>
```

**Keep it narrow — one capability per lab.** The scarce resource is attention per expectation, and
a failed stop in a narrow lab implicates exactly one feature. And **say where to look**: several
stops expect a status-bar notice, and a reader who does not know that reports "nothing happened".

### Failure — `failure-parse.md`

**You read a diagnosis rather than predicting one.** The specimen is stated up front with the line
that breaks it, because the interest is in what the compiler *says* and how far it gets.

```markdown
**Specimen:** `<Model>` — <the one thing wrong with it>

**The question to hold:** <what the reader should be wondering>

## Station N — <what this pane reveals>

[<load link>](hrw://load/…)

**Expected:** <the diagnosis, or "not reached", exactly>
```

**Close with `## What to bring back`** — open questions for Doug, since a failure lab's real
output is a design opinion about whether the diagnosis is actionable.

### Calibration *(was: adjudication)* — `the-oracle.md`

**The station's activity is asking a reference implementation**, so every heading carries the
instrument as an emoji: 📐 HRW, ⚙ System Modeler, 🧮 Wolfram.

```markdown
## 📐 Station N — <what HRW claims>

**Expected:** <HRW's answer>

## ⚙ Station N+1 — Ask the other implementation

**Expected:** <what the other tool says, and whether they agree>
```

**Claude evaluates every notebook cell through the kernel first**, then ships it for Doug to
evaluate — the stop that lands is the one he checks himself. Fixture notebooks are versioned in
[`notebooks/`](notebooks/); ad hoc ones are ephemeral.

### Bug report — not built <!-- unbuilt: bug_report_lab -->

A lab that narrates the steps of a failure for a screen recording, to hand a Rumoca maintainer a
reproduction. **No template until an instance exists** — writing one from imagination is how a
convention becomes load-bearing before it is known to work.

**And it is the first kind whose reader is not Doug.** Its audience is maintainers, which flips who
judges it under the two-audience rule ([`../../DECISIONS.md`](../../DECISIONS.md)): the test becomes
*"did a maintainer act on it without asking Claude?"* — so **Doug's run cannot validate one.**

### Keep the lab's history out of the lab

**A lab is written for Doug; a changelog is written for Claude.** No *"reworded after Doug
asked"*, no *"corrected 2026-08-13"*, no dated parentheticals. They accumulated to eight in one
file and made it read as a maintenance log.

That history is not lost, it is **filed where it belongs**: the decision and its reasoning in
[`../../DECISIONS.md`](../../DECISIONS.md), the question that prompted it in
[`../question-ledger.md`](../question-ledger.md), and the mechanism in **a code comment or on the
test that enforces it**. A lab states what is true now.

*(That last route said `../compiler-phases/` until 2026-09-01 — the sixth site sending HRW material
into what turned out to be **Rumoca reference documentation**. An HRW mechanism belongs beside the
HRW code that implements it.)*

**THE SCOPE IS A LAB, AND RULES FILES DELIBERATELY DO THE OPPOSITE — including this one.** A lab is
read to **learn**, so history sits between the reader and the idea. A rules file is read to
**decide**, and the failure it must prevent is a session re-deriving a rule that was retired.

**That is not hypothetical here.** On 2026-09-01 alone, retired rules came back five separate ways:
running discipline returned as a queued `last_walked` feature, `compiler-phases/` was described as
a deferral store in six places, and the `Predict` counts outlived the checker that superseded them.
**Each would have been prevented by one dated line saying what changed.** So this file carries its
corrections and the labs carry none — the same test decides both: *if this note were gone, would
the rule become easier to get wrong?*

**A dated note earns its place by preventing a re-derivation, and stops earning it when nobody is
tempting.** Those added during this sweep are load-bearing now and will not be forever; the
discriminator for removing one is whether anyone has tried to re-derive that rule since.

## Running a lab edit — the loop, and the two gate traps

*(Moved here from `CLAUDE.md`'s Current work on 2026-09-01. These are needed **while editing a
lab**, which is exactly when this file is read; they were filed under "what is in flight", which
they never were.)*

**THE LAB ITERATION LOOP, and it is not the gate:**

```text
cargo test -p hrw --lib -- --test-threads=1 doc_citations lab   # 6.1s -- while editing
cargo run -q -p hrw --example gen_lab_catalogue                 # 9.9s -- see the trigger below
cargo run -p hrw --example gate                                  # before the commit
```

**The third line is the RUNNER, not the plain fast suite** *(corrected 2026-08-31)*. It said
`cargo test -p hrw --lib` — which for a lab edit is the one gate that **cannot see the
change**, because the tests verifying guarded tables against a real compile are slow-gated off.
The runner picks LAB for a lab edit (11.1 s) and FAST otherwise.

**And the generator's trigger is not only a `##` heading** — that comment said `ONLY`, which the
blurb trap twenty lines below has contradicted since 2026-08-22. Regenerate if a `##` heading
**or the lab's first bolded line** moved; when in doubt run the checkers first and let
`lab_catalogue_is_current` tell you.

**TWO GATE TRAPS, both of which have cost the full gate before:**

- **`connect-expansion.md` is the one expensive lab — but only its five guarded tables are.**
  It is the only lab carrying `<!-- pane-groups -->` / `pane-origins` / `pane-frames` tables,
  which slow-gated tests verify against a real compile. **Editing one of those tables means the
  LAB gate** — 11.1 s, not FULL's ~101 — whatever the diff-grep says; **editing its prose does
  not, and never did.**

  **YOU NO LONGER HAVE TO REMEMBER THAT** *(built 2026-08-22)*.
  `doc_citations::editing_a_guarded_lab_table_needs_the_full_gate` compares every guarded region
  against `HEAD` and **fails by name in the FAST suite** if one changed, naming the marker and
  printing the FULL command. It is gated *off* under `slow-tests` — in a FULL run the real
  checkers are executing, so **the cheap gate is the only place the warning is useful.**

  **The gain is assurance, not permission.** Prose edits were always FAST; what was missing was
  any check that an edit believed to be prose actually was one. The filtered iteration line
  catches it, so a green `doc_citations lab` run now means FAST was genuinely the right gate.
- **Any `##` heading edit changes `CATALOGUE.md`.** Forget `gen_lab_catalogue` and
  `lab_catalogue_is_current` fails. The order is `cargo fmt` → generators → checks, and getting
  it backwards has cost the whole gate four times.

  **AND THAT IS NOT THE ONLY TRIGGER — the blurb is the lab's FIRST BOLDED LINE** *(found
  2026-08-22)*. `lab::catalogue` takes each lab's summary from the first line starting with
  `**`, so **inserting any bolded paragraph above the existing one silently replaces the
  catalogue's summary** — in this case with a mid-sentence fragment, *"MLS §9.3 requires connected
  connectors to be type-compatible, and Rumoca does check. Four"*. Nothing about headings was
  involved. **A new intro section goes BELOW the lab's opening bold line**, or the catalogue's
  description of that lab changes with it.

## Further reading

- 👤 [`../architecture.md`](../architecture.md) — how lab mode and the `hrw://` link
  vocabulary work
- 👤 [`../../README.md`](../../README.md) — what HRW is, and the capture plan that keeps
  screenshots honest by taking them at these stations
