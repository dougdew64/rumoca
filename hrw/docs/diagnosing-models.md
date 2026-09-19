# Diagnosing models — why a model does not work, and which instrument answers

**Purpose:** the four classes of model failure, the instrument each one needs, and why picking
the wrong instrument stalls a diagnosis.
**Status:** **DRAFT TAXONOMY — it does not yet prescribe a method.** Raised by Doug 2026-09-19
as the next phase of the project; to be iterated with him until it does prescribe one.
**Read when:** a model does not do what it should, and you are about to reach for a tool.

## What this document is, and what it is not yet

**Doug, 2026-09-19:** *"After I've learned how Rumoca works, I intend to continue by using this
project to learn why some models do not work."*

**This is phase two of the project**, and it changes what HRW is for. Phase one asks *what did
Rumoca do*; phase two asks *why is this model wrong*, which is a different question with
different instruments.

**Right now this file only classifies.** It says which instrument answers which failure. It does
not yet say *do this, then this*, and it should not be cited as though it does.
<!-- unbuilt: doc_citations::diagnosing_models_prescribes_a_procedure -->

**Nor is it yet held to the 👤 standard** (`README.md`, *Two audiences*) — that a reader must
finish without asking Claude. It is being written *with* Doug rather than *for* him, and the
mark goes on when the prescription lands and a reader can follow it alone.

## The mistake this exists to prevent

**An instrument that cannot see a failure returns a clean result, and a clean result reads as
evidence.** That is the whole reason to classify before reaching.

The sharpest case: for a model that says something you did not mean, **a second tool agreeing
with the first is exactly what you should expect** — both are faithfully simulating your
mistake. The agreement feels like confirmation and is not. A month of this project's work
(`upstream-issues.md`) ran the other way, where disagreement was the finding, which makes the
trap easy to walk into from habit.

## The four classes

| # | the failure | is the oracle useful? | the instrument |
|---|---|---|---|
| 1 | the model says something you did not mean | **no — it agrees** | an independent derivation |
| 2 | the model is ill-posed | it concurs, separately | structural diagnostics |
| 3 | well-posed, numerically hard | it may succeed where Rumoca fails | solver diagnostics |
| 4 | the tool is wrong | **yes — this is its case** | the differential method |

### 1. The model says something you did not mean

Wrong sign, wrong parameter, a unit slip, a connector wired to the wrong port. **The most common
failure, and the one the differential method cannot see at all.**

The instrument is an **independent derivation** — something that does not come from any
simulator:

- a **closed-form solution**, where the model is linear and low-order;
- a **conservation law** — energy, charge, momentum — which holds for nonlinear models too;
- a **limiting case** you already know: `t → 0`, `t → ∞`, a parameter driven to zero or infinity;
- a **dimensional check**, which costs nothing and catches a whole class of slips.

**Worked example.** `BareRc`'s answer was checked against `5(1 - e^{-t/0.1})` before any tool was
trusted, and `ThrownBall`'s first bounce came from solving `4.905t^2 + 5t - 1 = 0` by hand —
`0.171236` predicted, `0.1715` measured. Neither needed a second implementation.

### 2. The model is ill-posed

Structurally singular, over- or under-determined, high-index without reduction, inconsistent
initial conditions. **The mathematics is defective regardless of tool**, so every tool objects —
the question is only what it tells you.

**This is where HRW is worth more than a commercial tool**, and it is the argument for phase two
happening here. Dymola or System Modeler hands you a verdict; HRW shows you the structure that
produced it — the matching, the unmatched variable, the BLT blocks, the differentiation steps.

**Worked example, and note the two verdicts are independent.** `OrphanConnector` declares a bare
`Pin` with no component behind it. Rumoca's index reduction says *"structurally singular system:
1 matched out of 1 equations and 2 unknowns; unmatched unknowns: p.v"*. System Modeler says
*"Unbalanced model"* and *"Variables not solvable in any equation: 'p.v'"*. **Same variable,
reached separately** — which is what makes it a fact rather than one tool's opinion.

And the near-miss that teaches the class: `DanglingPin` leaves a resistor pin unconnected and is
**fine**, because the unconnected-flow pass gives `R.n.i = 0` and the resistor's own equation
then determines `R.n.v`. MLS requires a component to be locally balanced, so an absent connection
is not an absent equation. **Counting is the instrument, not intuition.**

### 3. Well-posed but numerically hard

Stiff, chattering at events, badly scaled, a near-singular Jacobian. **Nothing is wrong with the
model**; the solver is struggling, and the symptom is a failure to converge, an absurd runtime,
or a step size collapsing.

The instrument is **solver diagnostics**, and HRW already plots them: step size against time, and
integrator order, from `SimData::solver_steps`.

**A symptom worth recognising**, because it was misread once: *"BDF step: Step size is too small
at time = 0"* is not a stiffness problem. A step size that collapses **at `t = 0`** means the
*initialization* never solved, which is class 2 wearing class 3's clothes. Collapse at `t = 0.4`
is the real thing.

### 4. The tool is wrong

**The rarest case, and the one this project spent 2026-09 on.** The instrument is the
**differential method**: the same model through two independent implementations, where a
disagreement localises the fault to the implementation rather than the model.

Its prerequisite is that classes 1 and 2 are already ruled out. A model that is wrong, or
ill-posed, will make two tools disagree for reasons that have nothing to do with either tool.

## The principle underneath all four

**Hold an independent source of truth, and vary one thing at a time.**

Only the *source* changes between classes — analytic solution, structural count, solver trace,
second implementation. What never changes is the second half, and it is the half that gets
skipped.

**Varying one thing is what turns an observation into a finding.** `BareRc` differs from
`RcCircuit` in two ways at once — no connectors *and* no library — so it could establish that the
flat-line was not universal but not *why*. `ConnRc` changed exactly one of them: the same
topology with hand-written connectors and no library. It flat-lined, which **ruled the MSL out
and ruled connectors in**, in about ten minutes and after a week of reading the compiler had not.

**Corollary, paid for three times in 2026-09:** a correlation across many models is not a
mechanism. Eleven specimens fitting a pattern produced a confident causal claim that a twelfth
model refuted. **Design the twelfth model on purpose.**

## What HRW gives you, per class

| class | HRW today | gap |
|---|---|---|
| 1 | nothing, correctly — an independent derivation is not the tool's job | — |
| 2 | the strongest surface: matching, BLT, index reduction, IC plan, `stage_outcomes` | — |
| 3 | step size and order plotted from `solver_steps` | no Jacobian conditioning, no event-chatter view |
| 4 | `hrw://systemmodeler` opens a specimen there | **no side-by-side trajectory comparison** |

**The class-4 gap is the one that costs time.** Comparing Rumoca against System Modeler is
currently done by hand — export, read numbers, compare in conversation. For the instrument that
*defines* class 4, that is the slowest part of the loop.

Checked against the frozen composition primitives (`../CLAUDE.md`): that freeze covers point-at
and follow for context capture, **not** a plot overlay, so a comparison view is not a
re-proposal of the declined *compare* primitive.

## Open, for the iterations that make this prescriptive

- **What is the entry procedure?** A decision tree keyed on the symptom is the obvious shape —
  but a symptom rarely names its class, as the `t = 0` step-size case shows.
- **How is class 1 ruled out cheaply?** It is the most common and has the weakest instrument,
  because "derive it independently" is real work.
- **Where do Doug's own models enter?** Every worked example here is a specimen authored to
  provoke a known failure. The phase-two case is a model whose failure nobody designed.
- **What does the corpus owe?** Eight `DO NOT FIX` failure specimens exist, and
  [`ideas.md`](ideas.md) #46 wants one per phase. This taxonomy suggests a second axis: one per
  *class*, which is not the same cut.
