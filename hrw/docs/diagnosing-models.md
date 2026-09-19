# Diagnosing models — why a model does not work, and which instrument answers

**Purpose:** the four classes of model failure, the instrument each one needs, and why picking
the wrong instrument stalls a diagnosis — **across two axes**: one model with somebody looking,
and a large campaign with nobody looking.
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

## Vocabulary — align with V&V practice, not with this document

**Doug, 2026-09-19:** *"Going forward, I want to align with V&V vocabulary. I intend to focus my
Purdue robotics studies and my HRW studies on a career in V&V."*

**So the standard term wins, even where a home-grown one reads better**, because the audience for
this work now includes people who already have the terms. Name the source when using one.

| source | what it owns |
|---|---|
| **ASME VVUQ 1-2022** | the terminology standard for verification, validation and uncertainty quantification in computational modelling |
| **ASME V&V 10 / V&V 20** | VVUQ practice for computational solid mechanics / CFD and heat transfer |
| **NASA-STD-7009** | model-and-simulation **credibility**: verification, validation, input pedigree, uncertainty, robustness, use history |
| **ISO 21448 (SOTIF)** | hazards from correct-but-inadequate function, rather than from faults |
| **ISO 34502** | **scenario-based evaluation** — the closest standard to what Doug first called *"scenario validation"* |
| **UL 4600** | the overarching safety case for autonomous products |
| **ISO 26262 / IEEE 1012** | automotive functional safety / system and software V&V |

**The split that governs everything:** **verification** asks whether the equations are solved
right — numerical fidelity. **Validation** asks whether the right equations are being solved —
representation of the physics. ASME further divides verification into **code verification** (does
the software implement the equations) and **solution verification** (how large is the numerical
error in *this* calculation).

### The four classes ARE that split, which is the best evidence the taxonomy is sound

The classes were derived from this project's own failures before any standard was consulted, and
they land on the standard division almost exactly:

| class here | the standard name |
|---|---|
| 1 — the model says something you did not mean | **validation**, plus **input pedigree** when the scenario rather than the model is wrong |
| 2 — the model is ill-posed | well-posedness of the **conceptual model** — upstream of both |
| 3 — well-posed but numerically hard | **solution verification** — ASME V&V 20's whole subject |
| 4 — the tool is wrong | **code verification** |

**And one technique used here all month has a name.** Checking a simulator against a problem
whose exact answer is known independently is the **Method of Manufactured Solutions**, the
canonical code-verification technique. `BareRc` — an RC circuit checked against
`5(1 - e^{-t/0.1})` — is an MMS-style case, and so is `ThrownBall` checked against
`4.905t^2 + 5t - 1 = 0`. **Use the name from now on.**

### The third axis is the REFERENCE, and UQ is what moving along it costs

*(Corrected 2026-09-19, hours after being written. The first version called uncertainty
quantification "the axis this document does not yet have". That is imprecise: UQ is **absent**
from class 2 — a structural count has no uncertainty — and **coextensive** with class 3, since
solution verification simply is numerical uncertainty estimation. Something missing from one
class and identical to another is not a dimension crossing them.)*

**VVUQ** is **V**erification, **V**alidation and **U**ncertainty **Q**uantification; ASME's
committee took the third letter because a validation claim without an uncertainty statement is
not a claim. The dimension that actually crosses the other two is **what you compare against**:

| reference | uncertainty present | technique |
|---|---|---|
| an **exact answer** | negligible | **Method of Manufactured Solutions** |
| the **same model, refined** | numerical only | **Richardson extrapolation**, Grid Convergence Index |
| **another implementation** | two uncertain results, neither authoritative | the differential method |
| **measured data** | experimental + input + numerical | **ASME V&V 20** validation |

It crosses the class axis — class 2 needs no reference at all, and class 3's reference is itself
at a tighter tolerance — and it crosses the scale axis, since measured data is rarely available
per run, so a campaign falls back to self-consistency plus sampled cross-code.

**At the measured-data end, ASME V&V 20 carries this document's opening warning in its most
expensive form.** Form the comparison error `E = S - D` (simulation minus data) and a validation
uncertainty `u_val` combining numerical, input-parameter and experimental contributions. Then:

- **`|E| >> u_val`** — **model form error** is detected *and bounded*: the model is structurally
  wrong and you know by how much.
- **`|E| <= u_val`** — you have shown only that the model is **not distinguishable from the data
  at your resolution**. **You have not shown it is right.**

**Sloppy experiments and coarse numerics inflate `u_val`, which makes validation EASIER to
pass.** A worse experiment yields a better-looking result. That is *"an instrument that cannot
see a failure returns a clean result"* with a price attached — here the clean result can be
bought by measuring badly.

**Where this project sits: the top row, exclusively.** Every comparison in 2026-09 was against a
closed form or against System Modeler. **Nothing here has ever been validated against measured
data**, which is why uncertainty never arose — and why nothing in this repository prepares for
the place professional validation actually lives.
<!-- unbuilt: doc_citations::diagnosing_models_covers_validation_against_measured_data -->

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

## The second axis — by hand, or unattended at scale

**Raised by Doug 2026-09-19, from V&V practice:** among thousands or millions of scenario runs,
a safety engineer must decide which ones are *not valid evidence*. **Both axes matter to him**,
so the four classes above are only half of this document.

The classes do not change. **What changes is which of them matter, and what an instrument has to
be.** Three inversions, and each one reverses a habit that is correct by hand.

### A failure that announces itself becomes free; a plausible one becomes the whole problem

By hand, a crash is annoying and a wrong number is subtle. **Unattended, that reverses
completely.** A run that fails loudly costs nothing — it is filtered by exit status before anyone
sees it. **The runs that need a human are the ones that completed, exited zero, and produced a
believable curve.**

So at scale the classes reorder by *whether the failure is self-announcing*, not by how common
they are:

| class | unattended, what it looks like | the monitor that catches it |
|---|---|---|
| 1 — the scenario says something you did not mean | **silent**; a clean run of the wrong case | input fidelity: the run started and stayed where the scenario said, and inside each sub-model's fitted envelope |
| 2 — ill-posed | usually loud, and usually caught once at model level — **except per-scenario structural change** (a clutch locking, a constraint activating) which is silent and per-run | residual small throughout; structural re-check at mode switches |
| 3 — numerically hard | loud when it gives up, **silent when it degrades** | step-size collapse, order thrashing, event-count explosion, conditioning |
| 4 — the tool is wrong | **silent and systematic** — every run wrong the same way | cross-implementation agreement on a sampled subset |

**Class 4 changes character more than any other.** By hand it is the rarest case and the last
thing to suspect. At scale it is the most dangerous, because it is **not random**: a tool fault
corrupts every affected run identically, so it never looks like noise and averaging cannot find
it. Sampling can, cheaply, because you only need a few runs to disagree.

### The binding constraint becomes the false-positive rate, and it does not exist by hand

By hand a false alarm costs a minute. **Over a million runs a 1 % false-positive rate is ten
thousand human reviews**, which is not a validity process, it is a new backlog. So a monitor is
usable only if it is cheap per run *and* quiet, and that rules out instruments that are perfectly
good by hand — anything needing judgement, anything with a tunable threshold nobody has
calibrated, anything that fires on legitimate physics.

**This is the main reason the two axes cannot share one prescription**, and it is why Doug wants
both.

### The lesson `ThrownBall` teaches is the one to design around

`ThrownBall` exits zero, draws a smooth bouncing ball with the correct restitution, and
**conserves energy exactly between bounces**. It describes a ball that was dropped, when the
scenario said thrown at 5 m/s. It is a numerically excellent simulation of the wrong initial
condition.

**Every physical invariant you might monitor is satisfied**, because the run is internally
consistent. Conservation checks cannot see it. Residual checks cannot see it. Solver health is
perfect.

**The only check that catches it is the one that sounds too obvious to write down: read the first
sample back, and compare it against what the scenario asked for.** It is trivial, it is rarely
done, and it is exactly what failed here — silently, across 13 of 18 traced specimens.

**So the first validity criterion is not a physical invariant. It is that the inputs came back.**

### This repository already has an instrument on this axis

`examples/fidelity_msl` and `examples/survey_msl` are unattended sweeps over thousands of models
with a watchdog, one model per process, and results promoted with a provenance sidecar. The
design lessons in [`long-runs.md`](long-runs.md) are scale-axis lessons: bound every run, guard on
free RAM rather than process size, and never trust a sweep that cannot state what it did *not*
check.

**And its documented limit is a class boundary.** `../CLAUDE.md` records that the F1–F9 programme
verifies the **noun** — is this structure what Rumoca produced — and says nothing about the
**verb**: which phase ran, in what order, what it declined to do. **2,614 green rows answered a
class-2 question and were read as answering all four.** That is the scale-axis form of this
document's opening warning: an instrument that cannot see a failure returns a clean result, and at
corpus scale the clean result arrives with impressive authority.

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
- **Does the scale axis need its own vocabulary?** V&V practice has frameworks for this —
  NASA-STD-7009's credibility factors, ISO 21448 for functional insufficiency, ASAM
  OpenSCENARIO/OpenODD for the scenario description. **Claude is not confident which of these owns
  the phrase Doug used ("scenario validation") and has not checked**; adopting a standard's terms
  would make this document legible to people who already have them, and is worth one search
  before the prescription hardens.
- **What is the cheapest useful monitor set?** The `ThrownBall` lesson says input fidelity comes
  first, before any physical invariant. What follows it, and in what order, is the shape the
  prescriptive version needs — chosen on cost-per-run and false-positive rate, not on how
  informative each check is.
