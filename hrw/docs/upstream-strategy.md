# Engaging the Rumoca maintainers — strategy

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
   UI — the fixture tours do that. Every published document needs its own *"what this does
   not establish"* section, for the same reason the bug-PR demo does: **one visible overreach
   costs more than several missing checks** (`docs/fidelity-plan.md`).
4. **Fidelity work is instrumental, not only defensive.** It is what makes the questions good.
   Weigh it accordingly when it competes with feature work.
5. **When a piece of work could produce something upstreamable, say so at planning time** —
   not after it is built in a shape that cannot be handed over.
