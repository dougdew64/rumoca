# HRW Vision

## The goal

Doug's top priority is **learning** — specifically, mastering the math and algorithms
of continuous-system modeling and simulation. HRW is the teaching instrument, not the
deliverable. Doug's understanding is the deliverable.

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

- The **end-to-end tour** is the spine
- The **phase tours** are the chapters
- The **specimen narratives** are the worked examples
- The **three-tier views** (snapshot / replay / live-stepping) are the labs
- Everything gets a **learning goal** and a **place in the sequence**

In detail:

1. **End-to-end guided tour** — the full story of a Modelica model becoming a running
   simulation, framed as a chain of problems and solutions. Each step's output creates
   the problem the next step solves:

   - Modelica's class hierarchy → *problem:* you can't do math on objects
   - Flat equations → *problem:* they're not in standard mathematical form
   - DAE in standard form → *problem:* the index may be too high for solvers
   - Index-reduced DAE → *problem:* hundreds of equations in no particular order
   - Computational order → *problem:* at t=0 the algebraic variables are unknown
   - Consistent initial conditions → *problem:* how to advance in time reliably
   - Numerical integration → *problem:* discrete events interrupt the continuous flow

2. **Phase-level guided tours** — each linked from the end-to-end tour, going deeper
   into one transformation's algorithms. These follow Cellier's chapter structure as
   inspiration but are grounded in Rumoca specimens and HRW views.

3. **Three-tier progression** — the delivery mechanism within each tour:
   - **Snapshot** shows the result (static IR view) — establishes what the algorithm produced
   - **Replay** shows the process (recorded animation) — reveals how the algorithm works
   - **Live-stepping** lets Doug test his understanding by predicting what happens next

Every guided tour has **explicit learning goals** — what Doug should understand by the
end. The three tiers are designed to achieve those goals, not as standalone features.

## Learning goals — end-to-end tour

By the end of the end-to-end tour (the spine), Doug should be able to:

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
  the context is established. The end-to-end tour is the entry point; phase tours are
  the deep dives.

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
