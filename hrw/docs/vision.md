# HRW Vision

**Purpose:** the north star — what Doug is actually trying to learn, and the platform HRW is
becoming around that.
**Status:** authority.
**Read when:** weighing whether a piece of work serves the goal, or when a plan starts
optimising for something that is not Doug's understanding.

## The goal

Doug's top priority is **learning** — specifically, mastering the math and algorithms
of continuous-system modeling and simulation. HRW is the teaching instrument, not the
deliverable. Doug's understanding is the deliverable.

### The widening: the target is the mathematics of *robotics*

**Continuous-system modeling and simulation is an *example* of the target, not the whole of
it.** Doug, 2026-07-29:

> what I'm trying to learn most right now is the mathematics of robotics. For example, the
> mathematics of continuous system modeling and simulation.

**The bridge is specific and mathematical, not a slogan.** A **closed kinematic chain produces
exactly the high-index DAE that Pantelides exists to fix** — constrained mechanisms *are*
index-3 DAEs. Index reduction is not an abstract compiler topic; it is what makes a robot with
a loop simulable at all. Rumoca's upstream is **CogniPilot**, a robotics/autopilot
organisation, so the fork's purpose and the learning goal already coincide.

**Concrete consequence, and it is a scheduling one:** [`ideas.md`](ideas.md) **#5** (four-bar
linkage specimen + un-park the planar mechanics library) is *central*, not parked. And the
charter's "no MSL MultiBody — build our own planar mechanics" was a **robotics decision made
before anyone said so.**

**The destination.** This is undertaken *in preparation for and alongside the **Robotics MS at
Purdue, beginning Fall 2026*** — [`CHARTER.md`](CHARTER.md) §1 and Decision 1 carry the binding
form. CSM is the **deterministic substrate** the robotics curriculum is built on, which is why
the charter defers stochastic methods, SO(3)/SE(3) geometry and optimization as
*prerequisite-building rather than deferral*.

*(Added 2026-08-01, and the gap is the point. Doug stated the widening on 2026-07-29 and it
was recorded in Claude's memory — which names **this file** as the authority for curriculum
decisions — but never written here. So the authority lacked what the note about it contained,
and a session without that memory would read this page, answer "continuous-system simulation",
and never learn what it is **for**. Found 2026-08-01 by Doug auditing Claude's answer to
"what is my top priority?")*

**The operational definition of success is the charter's, not a separate one:** complete
understanding of Rumoca on the specimen set. The bet is a proxy claim — understand the
pipeline as the specimens exercise it, and you thereby understand the necessary math and CS.

The target understanding is **product-independent**: a mental flowchart of the
mathematical and algorithmic transformations that any Modelica compiler and simulator
must perform, and the problems that motivate each transformation. This understanding
should transfer equally to Rumoca, Wolfram System Modeler, OpenModelica, Dymola, or
any other tool.


## Why this beats books and lectures — Doug's model, after the first tour walk

*(Doug, 2026-08-12, having walked `connect-expansion.md` and started `dae-construction.md`. His
words; the refinements after them are Claude's, and one of them contradicts him.)*

> *"I began this effort because I believed that it would be possible to create a learning tool and
> process which would be more effective for me than books and attending lectures. I believe that we
> are on the right track. But, after just a bit of tour walking, it is striking to me how similar the
> tours are initially to the books, yet how different our conversation is from the lectures. At least
> initially, the tours have gaps and I struggle to understand what is written in the tours. That
> experience is very much like my experience with books. However, during our conversation, you answer
> questions and improve the tours. I don't get to ask questions during lectures and the lecturer
> doesn't improve the books."*

**The tours are books. The difference is not the prose, it is the repair loop.** This is a design
finding rather than a complaint, and it was measured within an hour: three questions produced three
tour defects, every one of them **a term used before it was defined** — "the graph is solved", "the
connection graph", "the components are computed". Prose fails the same way in a tour as in a
textbook. What a textbook lacks is a reader who can make it change.

**And the compounding property is the thing neither medium has.** A lecture is synchronous but lossy;
a book is durable but static. Here a question is answered synchronously *and* the answer is routed
into the repository — the tour, [`compiler-phases/`](compiler-phases/),
[`question-ledger.md`](question-ledger.md). **Asking permanently improves the artifact**, including
for future-Doug, which is why the routing discipline in
[`working-with-doug.md`](working-with-doug.md) is load-bearing rather than tidy.

### A COMPLEMENT to textbooks, doing the thing textbooks structurally cannot — 2026-08-31

**Doug, after four failed passes on one sentence:**

> *"Textbooks are typically frustratingly useless because they make claims which cannot be verified
> against code such as Rumoca's. Moreover, the claims made by textbooks very often just don't make
> sense to a reader who is attempting to think through a possible implementation of the abstractions
> described in textbooks. … This HRW project is a complement to textbooks and accomplishes what
> textbooks cannot."*

**This narrows the section above rather than repeating it.** *"The tours are books; the difference
is the repair loop"* was about **who can fix the prose**. This is about **what the prose is allowed
to claim** — and it is the stronger statement, because it says the tours should not be books at all
in their content, only in their form.

**The mechanism, and it is not a preference.** A textbook's claims are unfalsifiable *in principle*
for its reader: there is no artifact to check them against, so a claim that is vague, over-general
or simply wrong reads exactly like one that is right. Here there is Rumoca. **Claude can check tour
prose against it, and so can Doug** — and the four-pass sentence is the demonstration in both
directions. Every pass stated a textbook abstraction (union-find "starts with singletons"); every
objection Doug raised was an *implementer's* question (*"under what circumstances can that happen?"*);
and the resolution came from reading the callers of `get_or_insert_idx`. **No textbook could have
been wrong in a way that was detectable, and none could have been corrected.**

**The consequence for content:** a textbook-style abstraction that cannot be mapped to Rumoca's code
does not belong in a tour — Doug can get it from a textbook, where it is cheaper and no worse. What
HRW owes him is the part a textbook cannot supply. The operational test — *an abstraction earns its
place by predicting something the code does* — is in
[`fixture-tours/README.md`](fixture-tours/README.md), with the worked cases.

### Why a textbook is correct and INEFFECTIVE, and why that is not a failure of effort

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

*(Moved here from `fixture-tours/README.md` on 2026-09-01. The textbook case was being
argued in three places by then; this file is where HRW's positioning belongs, and the README keeps
only the authoring rule it produces.)*

### The three surfaces have distinct jobs, and the RHS is a lab

Doug's model, in his words:

> *"I haven't yet used the RHS features of HRW to learn. My sense is that I will begin to use those
> features after I have gained a very basic conceptual understanding… my very basic conceptual
> understanding will enable me to form expectations for what I should see in the RHS features of HRW,
> so that I will use those RHS features to test my expectations. In other words, the RHS will be
> partly helpful for you to demonstrate what your tour prose attempts to explain, and the RHS will be
> mostly helpful as a kind of lab for me to explore and test my expectations."*

**Accept this, and note what it settles.** It gives a criterion for whether a pane is worth building:
**a pane that only illustrates what the prose said is redundant; a pane that can falsify an
expectation is not.** That is the same rule `fixture-tours/README.md` already imposes for a different
reason — *every `**Expected:**` line must be violable* — which was written to make the tours **test
HRW**. Doug has arrived at it from the other side: violable expectations are also how **he** learns.
One mechanism, two purposes, and the second one is the reason to keep it strict.

### Two refinements, one of which contradicts the ordering above

**1. The threshold is a PREDICTION, not understanding — so the lab enters much earlier than Doug
expects.** He proposes prose first, lab after "very basic conceptual understanding". The trap is that
"later" never arrives: if the RHS waits for understanding, the prose has to carry the whole
explanation, and the tours drift to reference depth one good question at a time — the failure already
identified. **A prediction is far cheaper than understanding.** *"There should be three groups"* is a
crude, testable, falsifiable prediction available after two sentences, and checking it is the
fastest way to find out the two sentences were misread. So: **prose to the first prediction, then the
pane.** Not prose to comprehension.

**2. The lab checks CLAUDE, not only Doug — and this is the stronger argument for using it early.**
Doug framed the RHS as testing *his* expectations. It also tests the prose. Every tour count is read
from a generated trace and is therefore sound; **every rendering claim is unverified**, and Claude
cannot see the GUI. Twice on 2026-08-12 Claude asserted something it had not checked and was caught —
once by Doug, once by a checker. So each time Doug compares a pane against a tour claim he is
auditing the tour, and that audit exists nowhere else. **The lab is the only instrument that can
falsify Claude.**

### The repair loop may BE the teaching, not the thing that improves the teaching

*(Doug, 2026-08-15, after `connect-expansion.md` was finished.)*

> *"My most important observation is that working together to improve that tour was
> educational. In fact, at least so far, more educational than actually walking the completed
> tour."*

**This inverts the assumed order.** The model above treats the tour as the artifact and the
conversation as its repair mechanism. This says the repair is where the learning happened, and the
finished tour is closer to a by-product.

**Three mechanisms, and they are identifiable rather than mysterious:**

1. **He had to hold a model precise enough to detect a mismatch.** *"This does not agree with the
   pane"* is impossible without a prediction already in hand. Every disagreement was a
   prediction error **he generated himself**, and walking a correct tour produces agreement,
   which produces nothing.
2. **The disagreements were conceptual, never typographical.** Nodes versus connection sets;
   potential and flow as siblings versus as kinds of one thing; three versus six; spanning tree
   versus roster. **The friction was the curriculum**, and it clustered precisely where Claude
   had asserted something it could not check.
3. **He chose the questions.** A tour explains what *Claude* predicts will confuse him. *"Which
   graph?"* and *"what does 'computed' mean?"* came from real confusion about phrases Claude had
   used believing them clear — and could not have been anticipated.

**The confound, stated because it is large.** The tour he walked at the end was one **he had
co-written**, so of course it taught less; he already knew it. The honest comparison is walking a
tour he had no hand in, which has not been run. It may be that improving beats walking, or merely
that *first contact* beats *second contact* with the same material. **The next tour is that
test**, and it should be treated as one.

**The design consequence, if it survives the test.** It reads as an argument for shipping
under-polished tours, and is not. The disagreements that taught were concentrated in **claims
Claude could not verify** — not in claims it had been lazy about. So: get every checkable thing
right, and be *loud* about what remains unchecked, because that is where his attention pays.

That is what `**Falsified if**` and *"What this tour cannot check"* already are. **So the
unverified half is the valuable half, and it should stop being written about as a regrettable
gap.** A tour that could be fully machine-checked would teach less, not more.

### What this predicts about what to build

- **Phrase expectations as predictions to check, not descriptions to read**, and say what would
  falsify them. A description invites agreement; a prediction invites a look.
- **[`ideas.md`](ideas.md) #78 (Back/Forward for the RHS) rises in value**, because lab work is
  constant round-tripping between prose and pane. Doug hit that friction before adopting the lab
  framing, which is corroboration rather than coincidence.
- **A tour whose panes cannot falsify anything is a tour that should be prose in
  `compiler-phases/`** instead of a walk.

### Which teaching job HRW should try to win — I-do / we-do / you-do

*(Doug, 2026-08-23. The frame is his; the four-way split below is a refinement he asked for.)*

**The project's origin is a comparison with textbooks.** Before Claude, textbooks were the only
route to the mathematics of continuous-system modelling and simulation. With Claude, a project like
this becomes feasible — and the question worth asking is not *"is HRW better than a book"* but
**which part of teaching it should try to be better at.** Rumoca is the base because it is far
better suited to this than the alternatives; OpenModelica would have made the same project much
harder.

Effective textbooks run **I-do → we-do → you-do**: the author works an example, then guides an
attempt, then sets an independent project. Mapping HRW onto it is clarifying, and **the honest
conclusion is that HRW should not contest all three.**

| | who should own it | why |
|---|---|---|
| **I-do — general theory** | **the textbook** | Written by experts who have taught it for years and know **where learners fail**. That is precisely what Claude structurally lacks: he knows what he meant and cannot un-know it, so he cannot judge whether an explanation lands. Cellier's worked derivations are not something to compete with. |
| **I-do — this implementation** | **HRW, uncontested** | No textbook covers what *Rumoca* does to *`Drivetrain`*. But this is a different I-do: it shows **what happened**, not how to reason. The traces are the only source there is. |
| **we-do** | **HRW** | Predict → Look → Expected → Falsified is guided practice, with feedback a page cannot give. |
| **you-do** | **HRW** | The continuations — with an acceptance test, which is what textbook projects lack. |

**We-do is where the instrument changes the pedagogy rather than the medium.** A textbook must work
an example *before* asking you to try, because it cannot respond. HRW can invert that — predict
first, then look — and **the inversion is only affordable because feedback is immediate and cheap.**
That is a capability of having an instrument, not of writing better prose.

**You-do is where the gap is widest, in both directions.** A textbook's end-of-chapter project has
**no feedback loop** — you build it and cannot tell whether you succeeded, which is why most go
undone — and it is **contrived**, existing to exercise the chapter. A continuation here has an
executable acceptance criterion and is real work with a real upstream audience.

#### ⟶ TERSE EXPOSITION IS A PEDAGOGICAL CHOICE, NOT AN ECONOMY — 2026-08-30

*(Doug, on the day tour prose became pointable: he is the tourist, Claude is the guide.)*

**Tours may now be written to PROVOKE questions rather than to pre-empt them**, because he can
select any sentence in the tour panel and ask about it (`CLAUDE.md`, the 🎯 capture). His words:
*"you can be more terse in your tour prose as I can always ask you questions about your prose.
The tours can focus more on being tours and less on being like textbooks."*

**Why the prose was long is the whole argument.** A tour could not be asked questions, so every
confusion it might provoke had to be answered inline — which is *the textbook's constraint*, the
same one the we-do row above says an instrument removes. The tours inherited it for **prose** long
after HRW had removed it for **panes**. Exposition was doing two jobs: being the walk, and
pre-empting every question. The second is now unnecessary.

**It strengthens we-do rather than merely shortening it.** A prediction is only worth making about
something not yet explained, so **long exposition front-loads the answer and weakens the very step
it precedes.** Terser prose leaves more genuinely unknown at the moment of predicting, which is the
thing this row exists to measure.

**And it strengthens you-do**, in two ways. A dense paragraph is a wall to replace; a claim plus a
pointer is something Doug can rewrite in his own words — the third continuation below. And the
questions he asks *are* the raw material: what he had to ask about is exactly what his own version
should say.

**The asymmetry that makes it safe**, and it is the charter's first rule in a new dress: an omitted
explanation costs one question; an over-reaching one costs a false belief he cannot detect. **Prose
written to pre-empt questions is prose written past what was verified** — which is where
`differentiated_rows` and the fabricated BLT blocks came from. Less exposition is less surface for
that, and the capture makes the trade cheap.

**The boundary: fewer claims, never looser ones.** Terseness applies to *exposition*. It must not
touch a `**Expected:**` line, which is the walk's test and must stay violable — and it must not
become vagueness. *"The system is singular"* is terse and checkable; *"things go wrong"* is neither.
The writing rules are in [`fixture-tours/README.md`](fixture-tours/README.md).

#### The continuations, and what protects each

*(Named by Doug across 2026-08-22 and 2026-08-23. He asks that each be **protected and enabled
before it is needed** — the acceptance criterion captured while it is cheap, not on the day it is
wanted.)*

| | the continuation | what protects it |
|---|---|---|
| 1 | **Implement Pantelides** — [`ideas.md`](ideas.md) #83 | `pantelides_ladder`: five rungs, rung 1 green and four verified red, against a committed System Modeler oracle |
| 2 | **Author the visualizations himself** | [`../CLAUDE.md`](../CLAUDE.md)'s two-tier comprehension policy, and its standing rule to *move a computation out before adding one in* whenever one of the five view files is touched |

**⟶ THE THIRD WAS WITHDRAWN, 2026-08-31.** It read *"replace Claude's tour content with his
own"*, and Doug retired it outright: *"I have concluded that my goal of replacing Claude's tour
content with my own is a bad idea. I am completely withdrawing that goal."*

**What replaced it is a better instrument for the same end.** Tour prose became *pointable* the
day before — select a passage, press the target button, ask — so the loop is now *"I will ask you
questions and then you can make edits to the tour files."* Authoring the prose was a way to test
whether he understood it; **asking the question that exposes a gap tests the same thing and costs
a sentence instead of a rewrite.**

**Its whole apparatus went with it**, and that is the point rather than a side effect: the
`<!-- authored: -->` marker, `<!-- walked: -->` (also withdrawn — *"keeping track of what tours I
have walked doesn't yield enough value to justify the bookkeeping"*), both checkers, and the
supersession ceremony built for them. **Doug's own verdict on the request that produced them:**
*"we have been spending a lot of time running checkers and such because I requested a way for me
to edit tour files. I think that my request now seems like a bad idea."*

**So you-do now rests on the two heavier continuations**, #1 and #2 — which were always the
substantial ones. That is a real change to the balance of this frame, made deliberately and not
by drift.

**But the textbook wins one thing, and it is worth borrowing: its projects are SCOPED AND GRADED.**
A binary acceptance test says when you are *done*; it never says whether you are *on track*. That is
why `#83`'s test is being written as a ladder of five rungs rather than one red test.

### The operational filter this gives

**When weighing a piece of work: is it trying to beat a textbook at exposition?** If so it is aimed
at the one job HRW should concede. Build the thing a book cannot do — respond, falsify, and be
finished by a test — and leave the canonical derivation to the people who have taught it for
decades.

**This is a working hypothesis, not a settled decision** — hence its home here rather than in
[`CHARTER.md`](CHARTER.md), which holds what is amended deliberately and never drifted from.

## The opportunity

Books like Cellier's *Continuous System Modeling* and *Continuous System Simulation*
are the gold standard for the theory, but they are abstract. A reader hits a wall when
the math gets dense — reading about index reduction or BLT decomposition and thinking
"I understand the words, but I can't see it happening."

The missing piece has always been a concrete, inspectable implementation paired with a
guide who can bridge between the theory and the code. That is exactly what we have:

| Layer | Role | Example |
|-------|------|---------|
| **Cellier / textbooks** | The *what and why* — mathematical problems, theoretical framework, motivation for each transformation | "A DAE of index > 1 cannot be solved by standard BDF methods because..." |
| **Rumoca** | The *how, concretely* — a real, working implementation that can be read, debugged, and instrumented | `crates/rumoca-phase-structural/src/matching.rs` |
| **HRW** | The *show me* — visual evidence at every level of abstraction, from end-to-end pipeline to individual algorithm steps | Incidence matrix with augmenting paths highlighted |
| **Claude** | The *bridge* — connecting Cellier's theorem to a specific line of Rust to the green cell in the matrix you're looking at | "That cell turning green is Cellier §9.3's 'structurally admissible assignment' — here's why it matters" |

This combination — textbook theory grounded in a real compiler, visualized in an
interactive observatory, with an AI teacher bridging all three — is an opportunity for
learning that has not existed before.

## HRW is never finished, and that is the design

**HRW is not a product with a completion date.** It is an instrument that evolves alongside
Doug's reading. The loop, established 2026-07-24:

1. Doug reads a textbook — Cellier, Hairer & Wanner, Petzold, Strang.
2. He hits **friction** — a passage that does not land.
3. Q&A identifies the specific educational problem.
4. Claude answers it with some mix of explanation, specimen, document and HRW feature.

**Build a feature when it makes that loop more effective** — not when a curriculum is
"complete", because it never will be. Doug, 2026-07-28: *"HRW is like fashion: it will never
be done."*

**Two consequences that govern how work is judged:**

- **"Correct" means trustworthy, not complete.** The bar for foundational work is that its
  invariants hold, so later changes never re-litigate them.
- **Change cost dominates build cost.** On a multi-year artifact under permanent revision,
  that is the practical reason clean commented code and a maintained `DECISIONS.md` are
  load-bearing rather than polish — and why **any process proposed around HRW must be
  near-zero-friction, or it will not survive contact with the timescale.**

**The tension this resolves.** The curriculum cannot be designed top-down before the reading,
because the whole point is to address *specific* friction. But features should not be built
speculatively either. The rule that separates them: **build what has obvious benefit to the
reading-and-Q&A workflow.** Feature ideas come from three places — friction points (primary),
existing entries in [`ideas.md`](ideas.md) with clear Q&A benefit, and what HRW can already
do.

*(Written here 2026-08-01; it had lived only in Claude's memory. See
[`working-with-doug.md`](working-with-doug.md) on why that matters.)*

## The curriculum

The curriculum is **top-down**, matching Doug's learning style. Its structure:

- The **chain of problems** is the spine
- The **phase drill-downs** are the chapters
- The **specimen traces** are the worked examples
- The **three-tier views** (snapshot / replay / live-stepping) are the labs
- Everything gets a **learning goal** and a **place in the sequence**

**Two of these were stored documents and are not any more** *(corrected 2026-08-01; this
section still named both)*:

| Was | Is now | Why it changed |
|---|---|---|
| the end-to-end tour document, as spine | [`compiler-phases/the-chain-of-problems.md`](compiler-phases/the-chain-of-problems.md) for the *reasoning*, and an **ad hoc tour** for the *walk* | The stored tour rotted — it asserted a 7×7 incidence matrix on a tab showing 48 equations. Deleted 2026-08-01; its conceptual half, which makes no claim about what is on screen, was kept. |
| specimen narratives, as worked examples | a generated [`specimen-notebook/<Model>/trace/`](specimen-notebook/) plus a short hand-written `purpose.md` | 1,632 lines of narrative became 638 of purpose on 2026-07-29. **Numbers are read from the trace, which is correct by construction**; Claude regenerates the explanation on demand. |

**The pattern behind both is the project's governing rule:** *store what cannot be
regenerated.* An explanation is regenerable and rots; a **generated** trace and a *why this
exists* note are not. The curriculum did not shrink — its vehicle stopped being a file nobody
checked.

In detail:

1. **The chain of problems** — the full story of a Modelica model becoming a running
   simulation, framed as problems and solutions. Each step's output creates
   the problem the next step solves:

   - Modelica's class hierarchy → *problem:* you can't do math on objects
   - Flat equations → *problem:* they're not in standard mathematical form
   - DAE in standard form → *problem:* the index may be too high for solvers
   - Index-reduced DAE → *problem:* hundreds of equations in no particular order
   - Computational order → *problem:* at t=0 the algebraic variables are unknown
   - Consistent initial conditions → *problem:* how to advance in time reliably
   - Numerical integration → *problem:* discrete events interrupt the continuous flow

2. **Phase-level drill-downs** — each linked from the chain of problems, going deeper
   into one transformation's algorithms. These follow Cellier's chapter structure as
   inspiration but are grounded in Rumoca specimens and HRW views. They live in
   [`compiler-phases/`](compiler-phases/), one directory per phase; **Phase 7 has six of
   them**, which is where the interesting algorithms are.

3. **Three-tier progression** — the delivery mechanism within each tour:
   - **Snapshot** shows the result (static IR view) — establishes what the algorithm produced
   - **Replay** shows the process (recorded animation) — reveals how the algorithm works
   - **Live-stepping** lets Doug test his understanding by predicting what happens next

Every guided tour has **explicit learning goals** — what Doug should understand by the
end. The three tiers are designed to achieve those goals, not as standalone features.

## Learning goals — the spine

**These are the goals, and they outlived their vehicle** *(re-titled 2026-08-01; they were
headed "end-to-end tour", a document since deleted)*. Nothing here depended on that file —
each goal is a statement about **Doug's understanding**, which is the deliverable, so the
list stands unchanged while the way it is reached has moved to the chain of problems, the
phase drill-downs and ad hoc tours.

Doug should be able to:

1. **Explain why** a Modelica model cannot be directly simulated — why the
   hierarchical, object-oriented, equation-based description must be transformed
   before any numerical solver can touch it.

2. **Trace the chain of problems** — articulate each transformation as a response
   to a specific insufficiency in what came before (the problem-chain above).

3. **Identify the mathematical form** at each major stage — "at this point we have
   a flat system of equations; at this point we have F(t, x, x', y) = 0; at this
   point we have a block-triangular computational plan."

4. **Distinguish structural from numerical** — understand that some analysis
   (matching, BLT, index) works on the *pattern* of which variables appear where,
   independent of numerical values, and why that distinction matters.

5. **Explain what a solver needs** to start and to advance — consistent initial
   conditions, a residual function, a Jacobian, and event detection — and point to
   where each is produced in the pipeline.

6. **Recognize these transformations as universal** — not Rumoca-specific, but the
   same chain that System Modeler, Dymola, and OpenModelica all must implement in
   some form.

7. **Know where to go deeper** — for each stage, know which phase tour (chapter) to
   read and which specimen best illustrates the concept.

These goals are revisable as Doug's understanding deepens.

## The platform — the general shape this is becoming

Recorded 2026-07-31, from Doug:

> When reading Cellier or other textbooks, I might encounter a topic which baffles me and so
> turn to hrw for help. […] You are going to be a just-in-time author's assistant for whichever
> textbook I might be struggling with. I might ask you a thermodynamics question one day, a
> linear algebra question the next day and a CS question the following day. HRW, Wolfram
> Desktop, System Modeler will be part of your platform for teaching me. And, because you are
> able to consume other tools via MCP, if I need to make other tools available to you so that
> you can best help me, I will.

**The near-term focus does not change.** Doug's words the same day: *"My initial focus is
understanding how rumoca works. That doesn't change. But, we are building something more
general."* Everything above this section stands. What follows describes the shape the work is
taking, not a change of direction.

### What actually generalises: the subject

Until now the **subject** was Rumoca and HRW was the instrument. In the general form, the
subject is *whatever Doug is reading*, and Rumoca becomes one instrument among several —
sharply valuable when a question has a computational instantiation, irrelevant when it does
not.

That makes **instrument selection** a primary skill rather than an afterthought. A linear
algebra question wants Wolfram; a thermodynamics question wants an MSL model and System
Modeler; a performance question wants HRW's phase timings. Choosing wrong wastes Doug's time
in a way that being slightly wrong about content does not.

### The MSL is a physics library, not only a test corpus

The most consequential consequence, and it took three reframes in one day to see:

| Stage | What the 2,626 models were |
|---|---|
| morning | a test corpus for fidelity checking |
| after the survey | a searchable catalogue of specimens by IR shape |
| **now** | **a reference library of physics that runs** |

`Modelica.Thermal`, `Modelica.Media`, `Modelica.Fluid`, `Modelica.Magnetic`,
`Modelica.Mechanics` are peer-reviewed formulations of physics, expressed as equations, that
compile and simulate. So a thermodynamics question is **not** off-topic for HRW: *"here is
`Modelica.Thermal.HeatTransfer`, here are its actual equations, here is what System Modeler
does with it"* is an answer a textbook alone cannot give.

### The discipline that has to transfer, and the safety net that does not

For Rumoca questions there is an **oracle**: System Modeler adjudicates, which is what turned
`docs/upstream-issues.md` #2 from an opinion into evidence, and what has repeatedly caught
Claude being confidently wrong.

**Cross-domain, that net is gone.** The adjudicator for a thermodynamics claim is the
textbook, and Claude cannot read it unless Doug shows it. So the provenance rule
(`docs/provenance.md`) sharpens into a working rule for teaching:

> **Prefer claims a tool can demonstrate over claims Claude can only assert — and when only
> assertion is available, say what would check it.**

*"Here is the entropy relation"* is an assertion. *"Here is `Modelica.Media.Water`, here is
the equation it uses, here is a simulation of the behaviour, and here is where it would
disagree with your textbook's assumption"* is a demonstration. **Converting assertions into
demonstrations is what the platform is for.**

### What must NOT be built

The same rule as the ad hoc curriculum (`docs/ideas.md` #53), one level up:

- **No domain-specific features.** No thermodynamics view, no linear-algebra view. The
  platform stays general — query surface, exact context, composable instruments.
- **No stored lessons, per domain or otherwise.** Domain knowledge is Claude's and is
  regenerable; a stored version rots, which is what retired 1,632 lines of specimen narrative
  and the end-to-end tour's prose.

What *is* worth building is whatever widens the query surface or sharpens the context: the
filters of #53, the timings of #54, better nouns.

### Admitting a new instrument

Doug will add tools via MCP as needs appear. The bar a new instrument must clear is the one
`focus.json` already meets:

**It must emit exact context, never approximate.** A tool that hands Claude plausible-looking
summaries is *worse than no tool*, because Claude will reason confidently over them and the
error is unrecoverable downstream (`feedback-emitter-correct-reasoner-supplements`). Exactness
is the admission criterion; convenience is not.

### The north star, restated for the general case

`observatory-goal-context-and-explanation` says HRW maximises **convenient
context-identification × context-sensitive explanation**, a multiplicative pair. That survives
intact; only the sources widen. Context may now come from any admitted instrument, and the
explanation is still Claude's — which is why the platform grows by adding *nouns*, never by
adding stored *verbs*.

## Principles

- **Problem before solution.** Every explanation, tour stop, and narrative leads with
  the problem before presenting the solution. What would go wrong without this step?
  What is insufficient about what we have so far?

- **Product-independent understanding.** The problems are universal; every Modelica
  tool faces them. Rumoca's specific code organization is the concrete grounding, not
  the thing being learned.

- **Learning over polish.** Time spent on visual improvements that don't deepen
  understanding is time not spent on the mission.

- **Claude is a curriculum-aware teacher.** Not a product manager, not just an
  implementer. The ideas backlog is a curriculum backlog ordered by learning value.
  Features are proposed based on what will deepen Doug's understanding next.

- **Top-down, then drill.** Start with the big picture. Drill into details only after
  the context is established. [`the-chain-of-problems.md`](compiler-phases/the-chain-of-problems.md)
  is the entry point; the per-phase drill-downs are the deep dives.

## Assumptions

The curriculum **complements** the textbooks — it does not replace them. The books
provide the theory, definitions, and proofs. The curriculum provides what the books
cannot: "now open HRW, load this specimen, and *see* what you just read about."

Doug will read and work to understand the following (and others as topics arise):

- Cellier & Kofman, *Continuous System Modeling* (2006)
- Cellier & Kofman, *Continuous System Simulation* (2010)
- Hairer & Wanner, *Solving Ordinary Differential Equations II* (stiff problems, BDF/ESDIRK theory)
- Brenan, Campbell & Petzold, *Numerical Solution of Initial-Value Problems in Differential-Algebraic Equations*
- Modelica Language Specification (MLS) — especially Appendix B (DAE formulation)

The curriculum may freely reference specific chapters, sections, examples, and
theorems from these books. Claude's role is to bridge the textbook theory to the
concrete implementation — not to serve as an alternative author of textbooks.

Additional context: Doug's Purdue linear algebra applications course (Fall 2026)
connects Rumoca's algorithms to their matrix and linear algebra foundations.
