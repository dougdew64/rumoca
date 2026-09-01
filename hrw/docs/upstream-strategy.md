# Engaging the Rumoca maintainers — strategy

**Purpose:** why engaging CogniPilot/rumoca serves Doug's education, and the five planning
rules that follow from it.
**Status:** authority — shapes planning, not just outreach.
**Read when:** planning testing or implementation work, not only when preparing something to
send. Deliverables are ordered by *their* cost to accept; HRW itself goes last.

Agreed with Doug 2026-07-31. **This document is meant to shape testing and implementation
plans**, not to sit and be admired; the last section says how.

## The actual goal, which is not what it looks like

Doug's goal for this project remains **his education**. Engaging the CogniPilot/Rumoca
maintainers is a *means* to that, arrived at by reasoning rather than ambition:

> Conversations with those folks are likely to be very educational. Now, they are busy
> people. So, in order to motivate them to answer my questions (I tend to ask a LOT of
> questions) I need to ask interesting questions and perhaps deliver something to them
> which they might value.

So the mechanism is not "give them things so they owe me answers." It is:

**Make the questions cheap to answer and interesting to think about.**

*"Why does X?"* is a tax on a maintainer's afternoon. *"Here are 380 MSL models failing at
flatten with the same error shape — is that expected?"* is a gift with a question attached.
The fidelity and survey work is what converts the first kind into the second. That is its
real return, over and above catching bugs.

## The insight this rests on

**"How Rumoca Works" and "a Rumoca bug-investigation tool" have overlapping requirements.**
Both need the same thing: *what did the compiler decide on this input, phase by phase, and
why*. A maintainer does not need that to learn the compiler. They need it to see what it did
on the report they are triaging.

Doug's own framing — *"the maintainers might already know how rumoca works, but I'd bet they
would welcome a bug investigation tool."* There is a stronger version worth carrying too:
**HRW lowers the cost of a new contributor understanding the pipeline**, and contributor
onboarding is a perennial maintainer problem. That is a better pitch than debugging, because
maintainers can already debug.

## Order deliverables by THEIR cost to accept, not by our effort

Doug's implicit list led with HRW. HRW should be **last**, because it is the only item that
asks them to take on cost.

| Deliverable | Their cost to accept | Status |
|---|---|---|
| A bug report with System Modeler adjudication | **zero** — it is simply a good report | 2 ready, `docs/upstream-issues.md` |
| The MSL reach report (capability map) | **zero** — a web page, nothing to install | falls out of the survey |
| Differential testing at scale | zero, and it is ongoing | Doug has the tools |
| HRW itself | **maintenance burden, review time, a GUI dep, Windows-tested** | the long game |

The first three are gifts; the fourth is a proposal. **Gifts open conversations, proposals get
scrutinized** — and the conversations are the point.

### The highest-value item is the one that was not on the list

Doug has **System Modeler and Wolfram Desktop locally**, so he can differential-test Rumoca
against a commercial Modelica implementation at scale. That is expensive for a volunteer
project to do for itself, and it is exactly what turned `docs/upstream-issues.md` #2 from *"I
think the spec says…"* into *"System Modeler rejects this and you accept it."*

**A rarer contribution than a GUI.** See `feedback-oracle-first-for-specimens`.

## The alignment argument — read Rumoca's own README first

Noticed by Doug 2026-08-01, and it is a better opening than anything Claude had drafted.

Rumoca's README does not merely tolerate what HRW does. It **states as a design emphasis** the
exact property HRW depends on:

> Rumoca emphasizes:
> - **explicit compiler phases and IR boundaries**
> - strong structural analysis and DAE lowering
> - **reusable symbolic outputs rather than a single closed execution path**

And on why that is unusual, in their words: traditional Modelica compilers *"primarily focus
on simulation, FMU export, and tool-specific execution pipelines."*

### So the pitch is not "HRW is useful"

**HRW is a demonstration of a property they designed for.** An observatory that renders every
phase's IR, and lets a reader click from one to the next, is *evidence* that the phase
boundaries really are explicit and the symbolic outputs really are reusable. You cannot build
one on a compiler that does not have those properties — which is the point.

That is a far better first sentence than any claim about usefulness, because it is a claim
about **their** work rather than about ours, and it is checkable by running the thing.

### It also disposes of the cost objection, which was the worry

Measured 2026-08-01 during the full fidelity sweep: models the survey handles in **5-15
seconds** take **~900 seconds** through HRW's path.

**That ratio is NOT HRW overhead, and an earlier version of this document said it was.** The
survey **caps index reduction above 800 equations and skips it**; HRW's `index_reduction_stage`
runs the funnel **unconditionally**. So for exactly these models the two runs are not doing the
same work — a large part of the gap is HRW performing a phase the survey declined to perform,
and the phase in question is *Rumoca's*, not HRW's serialisation.

**Do not publish the ratio until the two sides are made comparable** — either by running the
survey uncapped on those models, or by subtracting the `Index reduction` figure the per-phase
log now records. A number that misattributes a compiler's own cost to a tool measuring it is
precisely the overreach that costs credibility permanently.

That looks alarming until it is framed correctly. **A project whose stated goal is "reusable
symbolic outputs rather than a single closed execution path" has already accepted that
materialising IR costs something** — that is the trade it chose over a closed pipeline. HRW
pays it maximally: ten stage trees per model rather than one throughput-optimised path.

**50-170x is the price of looking at everything.** It is a fact about the trade, not a
criticism of the compiler.

### Which reframes the performance profile (`docs/ideas.md` #54)

Not *"here is what your compiler costs"*, which invites defensiveness. Instead:

> *"Here is what materialising every IR costs, measured across your standard library."*

For a project positioning itself as an **interoperability layer**, that is useful design data:
it tells them what a downstream consumer pays to hold all the IR at once — which is precisely
what Julia/SciML, CasADi or a code generator would be doing.

### Two limits, so the inference is not over-read

- **The interoperability framing names computational consumers**: Julia/SciML, Python/JAX/
  CasADi/PyTorch, embedded targets, WASM. **Not GUIs.** So mission alignment is real, and it
  does *not* by itself argue for merging an egui app into their repository. **The deliverable
  ordering above stands** — HRW remains the item with a maintenance cost attached.
- Alignment of *mission* is not agreement about *scope*. It makes the conversation easy to
  start; it does not pre-decide where it ends.

### One practical detail worth acting on

The README says, verbatim:

> **Project status:** Rumoca is in active development. You should expect bugs and rough
> edges; **please file issues** at https://github.com/cognipilot/rumoca/issues.

That is an explicit invitation, which makes the entries in `docs/upstream-issues.md`
lower-friction than assumed. A bug report is not an imposition on a project that asked for
them.

## Two cautions

### A reach report can read as a scorecard

"Here is everything your compiler fails at", from a stranger, lands badly however true it is.
Frame it as a **capability map**. Be scrupulous about attribution: a model failing because it
uses MultiBody (out of scope) is *not* the same as a bug, and lumping the two together is the
class of overstatement that costs credibility permanently.

**The report must be more careful explaining non-failures than failures.**

### Do not arrive with everything at once

A busy maintainer facing a tool *plus* a report *plus* a question list will process none of
it. One well-made bug report, then another, then the report when someone asks what Doug is
building. Slower, and it works.

## What this means when planning

Concrete implications for testing and implementation decisions:

1. **Prefer work that yields zero-adoption-cost artifacts.** A report, a reproducer, a data
   set — these can be handed over without anyone approving anything.
2. **Anything published must be reproducible.** Checked-in survey code *and* its output, and
   deterministic selection, so a maintainer can regenerate and diff it. A number nobody can
   re-derive is worth less than no number.
3. **Anything published must be honestly bounded.** F1-F9 establish that HRW agrees with
   Rumoca. They do **not** establish that Rumoca is right, and they do not test the rendered
   UI — the fixture labs do that. Every published document needs its own *"what this does
   not establish"* section, for the same reason the bug-PR demo does: **one visible overreach
   costs more than several missing checks** (`docs/fidelity-plan.md`).
4. **Fidelity work is instrumental, not only defensive.** It is what makes the questions good.
   Weigh it accordingly when it competes with feature work. **And it is more instrumental
   than that**: `docs/reports.md` establishes that an oracle mismatch is only an admissible
   upstream finding when the same model is *fidelity-green* — otherwise the mismatch may be
   HRW lying rather than Rumoca erring. Fidelity is what makes oracle findings cheap enough
   to be worth having.
5. **When a piece of work could produce something upstreamable, say so at planning time** —
   not after it is built in a shape that cannot be handed over.
