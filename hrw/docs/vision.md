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
