# The chain of problems — why the pipeline has the shape it has

**Purpose:** the spine of the whole compiler — each phase stated as a *response to a specific
insufficiency* in what came before — plus the structural/numerical distinction and the
reading-list map.
**Status:** reference, and the safest kind: it describes **mathematics and pipeline shape**,
never HRW screen state, so it does not rot the way the document it came from did.
**Read when:** framing any explanation of why a phase exists. Lead with the problem, then the
solution — Doug learns by understanding why.

**Salvaged 2026-08-01 from `end_to_end_tour.md`**, which was deleted the same day. That
document was 1,071 lines: twelve stop-by-stop walkthroughs asserting specific numbers on
specific tabs, plus these conceptual sections. **The walkthroughs had rotted** — Station 8
described a 7×7 incidence matrix on a tab that shows 48 equations, and nothing caught it
because nothing checks prose. HRW stopped showing the document on 2026-07-29, but the file
stayed inside the teaching database, where a later session would read it as authoritative.
These sections were the part worth keeping, and they are kept because they make **no claim
about what is on screen**.

---

## The chain

Every step exists because the previous step's output is *insufficient* for what comes next.

```
Modelica text
  └─ Problem: can't compute on text
     → Parse to AST

AST
  └─ Problem: names are unresolved strings
     → Resolve names to definitions

Resolved AST
  └─ Problem: component internals are opaque
     → Instantiate (expand class hierarchy)

Instance tree
  └─ Problem: types unchecked, dims unresolved
     → Typecheck

Typed instance tree
  └─ Problem: still hierarchical, no global equations
     → Flatten (expand connects, qualify names)

Flat equation system
  └─ Problem: unstructured — no variable classification
     → DAE construction (MLS Appendix B partition)

High-index DAE
  └─ Problem: index > 1, states without derivative equations
     → Index reduction (demote constrained states)

Index-1 DAE
  └─ Problem: no evaluation order
     → Structural analysis (matching, BLT, tearing)

Structurally sorted DAE
  └─ Problem: no consistent initial values
     → IC plan (initialization recipe)

Initialized DAE
  └─ Problem: discontinuities will corrupt integration
     → Event extraction (zero-crossing functions)

Event-aware DAE
  └─ Problem: mathematical description, not executable
     → Solve lowering (compile to tensor compute graph)

SolveProblem
  └─ Problem: need to actually advance in time
     → Simulation (BDF integration + event handling)

Time-series output ✓
```

**This chain is not a Rumoca design choice.** It is inherent in the gap between what Modelica
expresses — a declarative, hierarchical, equation-based model of a physical system — and what
a computer needs to produce a solution: an executable, sequential, numerically-sound
computation. **Every Modelica tool must traverse this chain in some form**, which is what
makes learning it transferable rather than Rumoca trivia.

---

## Structural vs. numerical — two kinds of reasoning

The most important distinction in the pipeline is between analysis that works on the
*pattern* of variables in equations and analysis that works on *numerical values*.

**Structural analysis** examines the incidence matrix — which variables appear in which
equations — without evaluating a single expression. The matching algorithm does not know that
`J = 0.01`; it only knows that equation 3 mentions variables 2, 6 and 7. That reasoning is:

- **Cheap** — polynomial in the number of equations, independent of numerical difficulty.
- **Exact** — a matching is either perfect or it is not; there is no approximation.
- **Model-independent** — changing parameter values does not change the analysis, only the
  coefficients.

**Numerical analysis** works with actual floating-point values: Newton's method for
initialization, BDF stepping, Jacobian evaluation, event bisection. All are subject to
convergence failure, roundoff and stiffness.

**The power of the structural-then-numerical ordering** is that cheap, exact structural work
dramatically reduces what the expensive, approximate numerical phase must do.

> **Cellier & Kofman, *CSM*, Ch. 9:** the structural-analysis chapter is entirely about
> exploiting structure to reduce numerical work. This is the central idea.

---

## Where to go deeper

Each link below goes to the phase's own drill-down — the same problem-before-solution
structure, at the level of individual algorithms. *(All twelve verified to resolve
2026-08-01.)*

| Topic | Phase lab | Key algorithm |
|-------|-----------|---------------|
| Parsing | [Phase 1](phase1_parsing_and_ast/parsing_and_ast.md) | Recursive descent |
| Name resolution | [Phase 2](phase2_resolve_and_scope/resolve_and_scope.md) | Scope chain lookup |
| Type checking | [Phase 3](phase3_typecheck_and_dims/typecheck_and_dims.md) | Type inference, unit checking |
| Instantiation | [Phase 4](phase4_instantiate/instantiate.md) | Modification application |
| Flattening | [Phase 5](phase5_flatten/flatten.md) | Connection expansion (MLS §9.2) |
| DAE construction | [Phase 6](phase6_dae_construction/dae_construction.md) | Variable classification, when expansion |
| Index reduction | [Phase 6 drill-down](phase6_dae_construction/index_reduction.md) | Dummy derivative demotion, symbolic d/dt |
| Structural analysis | [Phase 7](phase7_structural_analysis/structural_analysis.md) | Matching, Tarjan, BLT, tearing |
| Initialization | [Phase 7 drill-down](phase7_structural_analysis/ic_plan.md) | IC plan (ScalarDirect/Newton/Torn/CoupledLM) |
| Solve lowering | [Phase 8](phase8_solve_lowering/solve_lowering.md) | Variable layout, AD Jacobian |
| Simulation | [Phase 9](phase9_simulation/simulation.md) | BDF/ESDIRK, event detection |

The [structural-analysis guided lab](phase7_structural_analysis/guided-lab.md) is the
deepest of these — five lessons with animated replays and live-stepped debugging.

**For actual numbers about a specimen, read its trace**, never prose: the
[`MotorWithBrake` trace](../specimen-notebook/MotorWithBrake/trace/) holds the full IR at
every stage, generated and therefore correct by construction. *That distinction is exactly
what the deleted document got wrong.*

---

## Reading list

| Book | What it covers |
|------|---------------|
| Cellier & Kofman, *Continuous System Modeling* (2006) | Modeling philosophy, DAE systems, structural analysis, index concepts |
| Cellier & Kofman, *Continuous System Simulation* (2010) | Numerical integration, stiffness, discontinuity handling |
| Hairer & Wanner, *Solving ODEs II* (1996) | BDF, implicit Runge-Kutta, stiff systems, stability theory |
| Hairer, Nørsett & Wanner, *Solving ODEs I* (1993) | Event location (Ch. II.6) |
| Brenan, Campbell & Petzold (1996) | DAE theory, index, initialization, BDF for DAEs |
| Modelica Language Specification | Connection semantics (§9.2), Appendix B DAE form, event semantics (§8.5-8.6), variability (§4.9) |

**Where to start reading Cellier is a decided question** — `../ideas.md` **#57**: Ch. 9.3-9.5,
because Rumoca's fit is best there, so a first attempt tests the *loop* rather than the *fit*.
