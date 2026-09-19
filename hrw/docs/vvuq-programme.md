# The VVUQ programme — three gaps, five stages, and the standards they answer to

**Purpose:** the plan that takes Doug from where this project stands to competence in the
activities a senior VVUQ role requires — what to study, what to build, and in what order.
**Status:** plan. The destination it serves is CHARTER §1 and Decision 17 (v1.13).
**Read when:** choosing what to work on next, or deciding whether a proposal serves the
destination.

## Why this document exists

**Doug, 2026-09-19:** *"I aspire to a senior role in VVUQ and so intend to address the gaps
which you identified."* The destination is a career in **VVUQ**, specialising in **scenario
validation**, reached through a Purdue robotics MS and likely further study in autonomy. HRW and
the physically owned robot are **educational means to that professional end**.

**The one correction that produced this document.** Doug's initial framing was that scenario
validation is *"largely about analyzing large simulation campaigns."* That describes the
**downstream station**. Run-analysis is one link; the links before it decide whether the campaign
meant anything at all — operating-domain definition, scenario generation and selection,
acceptance criteria, coverage assessment, and the safety argument. **A million valid runs of the
wrong scenarios proves nothing**, so validity is necessary and nowhere near sufficient. Aim at
the chain; let run-analysis be the point of entry rather than the ceiling.

## The standards this work answers to

Doug asked for these explicitly. **Use the practitioners' term over a clearer home-grown one, and
say which standard it comes from** — that rule is on the mandatory reading path.

### Simulation credibility — what makes a computed result admissible

| standard | what it owns |
|---|---|
| **ASME VVUQ 1-2022** | **the terminology standard** for verification, validation and uncertainty quantification in computational modelling. The first authority for a disputed term. |
| **ASME V&V 10** | VVUQ practice for computational solid mechanics — the conceptual framework and the roles of verification, validation and UQ. |
| **ASME V&V 20** | CFD and heat transfer, and **the validation-uncertainty methodology**: the comparison error `E = S - D`, and `u_val` built from numerical, input-parameter and experimental contributions, with guidance on characterising **model form error**. The single most directly applicable document in this list. |
| **ASME V&V 40** | risk-informed credibility for medical devices. Included because its *risk-informed* framing — how much credibility this decision needs — generalises well beyond medicine. |
| **NASA-STD-7009** | **model-and-simulation credibility assessment**: verification, validation, **input pedigree**, uncertainty characterisation, robustness, use history. The vocabulary for saying how much weight a result carries. |
| **IEEE 1012** | system, software and hardware V&V processes — the software-engineering side of the same word. |

### Autonomy safety — what makes an argument about a system admissible

| standard | what it owns |
|---|---|
| **ISO 26262** | automotive functional safety — hazards from **malfunction**. The baseline the rest extend. |
| **ISO 21448 (SOTIF)** | hazards from **correct but inadequate** function — performance limitations with no fault present. The reason autonomy needs more than ISO 26262. |
| **ISO 34502** | **scenario-based safety evaluation** — the standard closest to what Doug called *"scenario validation"*. |
| **IEEE 2846** | formalising the **assumptions** a safety model rests on — the assumption layer made explicit and checkable. |
| **ISO/TR 4804** | safety and cybersecurity for automated driving systems; V&V guidance specific to ADS. |
| **UL 4600** | **the safety case** for autonomous products — the structured argument that evidence supports the claim. The document that decides whether everything above adds up. |

**Confidence note, and it matters because this list will be cited.** The two tables above were
assembled from search on 2026-09-19 and from ASME's own description of its VVUQ standards; the
autonomy roster (ISO 26262, ISO 21448, IEEE 2846, ISO 34502, ISO/TR 4804, UL 4600) came from a
published survey naming them as one assurance landscape. **`ISO 34503` — operating design domain
taxonomy — is believed relevant and was NOT verified**; check it before citing.
<!-- unbuilt: doc_citations::vvuq_standards_roster_is_verified -->

## The three gaps

Established 2026-09-19 by walking the full chain and asking what covers each step. **Verification
and validation are well served by this project. These three are not served at all**, and they are
the most senior parts of the job.

### Gap 1 — Uncertainty quantification

**The only gap that is a mathematical discipline rather than a practice**, so it is the one that
needs deliberate study; it cannot be picked up by doing. It is also what distinguishes a VVUQ
engineer from a test engineer.

Content, in dependency order: probability and estimation → **design of experiments** (factorial,
Latin hypercube, space-filling) → **uncertainty propagation** (Monte Carlo, then polynomial
chaos) → **global sensitivity analysis** (Sobol indices — variance decomposition, not local
derivatives) → **surrogate models** (Gaussian processes, for when Monte Carlo is unaffordable) →
**inverse UQ / calibration** — fitting parameters from measurement *with* uncertainty rather than
by least squares.

- **Ralph Smith, *Uncertainty Quantification: Theory, Implementation, and Applications* (SIAM)**
  — the standard graduate text, and the one to read if only one is read.
- **Saltelli et al., *Global Sensitivity Analysis: The Primer*** — for Sobol.
- **Kennedy & O'Hagan (2001), "Bayesian calibration of computer models"** — the canonical paper
  on **model discrepancy**, which formalises model form error.

**Check the Purdue catalogue before buying books.** UQ, design of experiments and Bayesian
statistics courses typically live in ME or AAE rather than STAT. Claude has not checked Purdue's
offerings and should not guess at them.

### Gap 2 — Coverage assessment

Less textbook, more standards and practice: the **functional → logical → concrete** scenario
hierarchy; **operating-domain specification**; **combinatorial coverage** (NIST's t-way
covering-array material by Kuhn and Kacker is free and excellent); and **critical scenario
identification** by falsification and adaptive sampling.

**The question that makes it hard — *how many runs is enough, and of what?* — is a rare-event
estimation problem**, which loops back to Gap 1. Coverage cannot be closed before UQ is started.

### Gap 3 — Safety-case argumentation

**Cheapest to close, and therefore scheduled early**: mostly reading and writing, weeks rather
than months. **It reframes everything else** — once you know what a safety argument must look
like, you know what the evidence is *for*, which changes how campaigns get designed.

- **Philip Koopman**, *How Safe Is Safe Enough?* plus his papers and blog — central to UL 4600
  and unusually readable.
- **The GSN Community Standard** (Goal Structuring Notation) — free, and the notation for
  structured arguments.
- The literature on **safety-argument fallacies**; knowing how arguments fail is most of the
  skill.

**The sub-problem worth owning: what makes simulation evidence admissible in a safety argument.**
That is the intersection of this entire programme, and it is not a settled question in the field.

## The five stages

Each is complete in itself and produces an artifact. **The first two need no hardware and no new
mathematics to begin.**

> **READY IS NOT NEXT** *(added 2026-09-19, the same day the stages were written)*. Stage A needs
> no hardware, which Claude reported as *"available now"* — and Doug then set the project order:
> Modelica compilation first, then the rebase, then simulation, and only after that the
> diagnostic and VVUQ work. **The whole of this programme sits behind that.** The ordering below
> is the order *within* the programme; the queue that decides when the programme starts at all is
> `../CLAUDE.md`'s Current work box. **Do not read this document as a schedule.**

### Stage A — UQ on a model already in the corpus

`BenchActuator` is a DC motor driving an inertia with parameters `R`, `L`, `k`, `J`. Treat them
as **distributions rather than numbers**. Monte Carlo the simulation; look at the distribution of
rise time; then compute **Sobol indices** to find which parameter actually drives it.

A complete UQ exercise on an existing model, and the project's first campaign-scale artifact.
**HRW needs a parameter sweep runner** — the repository sweeps *models* (`examples/fidelity_msl`)
but not *parameters of one model*.

### Stage B — Solution verification

Re-run at tightening tolerances and watch the answer converge; that yields the **numerical
contribution to `u_val`**. The ODE analogue of Richardson extrapolation. Small, and required
before any validation claim means anything.

### Stage C — First real validation, one servo

Identify parameters from step-response data. Form `E = S - D`. Assemble `u_val` from Stage A's
input uncertainty, Stage B's numerical uncertainty, and measurement uncertainty from repeated
trials. Deliver a verdict on model form error — **including the honest outcome `|E| <= u_val`,
which shows only that the model is not distinguishable from the data at your resolution.**

**HRW needs the comparison view**: measured data alongside simulated trajectories. Note this is
the *same* feature as the Rumoca-versus-System-Modeler gap already recorded in
[`diagnosing-models.md`](diagnosing-models.md). One feature, two purposes.

### Stage D — Coverage

Declare an operating domain for the servo — supply voltage, load inertia, command amplitude, duty
cycle, temperature if measurable. Build logical scenarios; generate concrete ones by covering
array or Latin hypercube; run the campaign; apply the validity monitors from
[`diagnosing-models.md`](diagnosing-models.md); **then assess coverage, which is a different
question from validity and the one most people skip.**

### Stage E — The safety case

A short **GSN** argument: *"this servo model is adequate for predicting step response to within X
over this operating domain"*, supported by the evidence from A–D and including an explicit
**confidence argument** about what is not covered. Ten pages.

## Two standing constraints on this programme

**One servo, until Stage E has been completed once.** A walking biped confounds contact,
friction, servo dynamics, timing and sensor noise simultaneously, and three references cannot
separate error sources that all move together. **A complete chain at trivial scale teaches the
thing; a fragment of a large chain does not.**

**Doug is the limiting factor, and he said so** *(2026-09-19: "I have a LOT of other stuff to
learn. Effectively, I am by far the limiting factor")*. This document exists so the plan can be
held on paper rather than in his attention. **Do not queue work against it faster than he can
absorb**, and do not treat a stage as blocked merely because a later one is.
