# HRW Observatory

**An instrument for understanding a Modelica compiler from the inside.**

HRW makes every phase of the [Rumoca](https://github.com/CogniPilot/rumoca) compiler
inspectable — not just its output, but its *process*. Pantelides iterating. Matching walking
augmenting paths. Tarjan discovering strongly connected components. Tearing choosing a
variable, and why. You can watch each algorithm run, step it in a debugger, and see the
animation advance in lockstep with the real code.

> <!-- CAPTURE 1 — hero loop, animated GIF, ~8s, no audio.
>      Source: the Tarjan animation on Drivetrain, stepping through SCC discovery.
>      This is the single image that has to earn the reader's attention. -->
> *[hero capture pending]*

It is a **study instrument, not a product.** There is no user but its author, and every design
choice follows from that: the stage trees, the equation sheet, the identifier index and the
animation frames *are* the point, not overhead to be optimised away.

HRW is a workspace member of [`dougdew64/rumoca`](https://github.com/dougdew64/rumoca) (a fork
of `CogniPilot/rumoca`) on the **`hrw` branch**, depending on the compiler crates via path deps
(`../crates/rumoca-*`). It links Rumoca **as a library** and never shells out to its CLI — so
what you see is the real compiler's real state.

---

## What it does that reading the source does not

**A compiler's phases are legible; its algorithms are not.** Rumoca's public API exposes each
phase's *result* — the IR after parsing, after flattening, after index reduction. What it
cannot expose is the *process*: the intermediate states that exist only mid-run and vanish
before any result is returned. **Those states are where the mathematics lives.**

That is why HRW moved *into* the Rumoca workspace. It adds observation hooks to the compiler
crates — **additive and semantics-preserving**, so HRW stays faithful to real Rumoca and the
work can be offered upstream.

| | What you get |
|---|---|
| **Eleven pipeline stages** | Parse → Resolve → Instantiate → Typecheck → Flatten → DAE → Index reduction → Initialization → Events → Solve lowering → Simulation, each with its IR inspectable and diffed against the previous stage |
| **Eight animated algorithms** | Matching, Tarjan/BLT, index reduction, tearing, alias elimination, IC planning, connection expansion, solver stepping |
| **Live debugger stepping** | The real algorithm pauses at each step; the animation follows. Not a re-implementation — the actual Rumoca code |
| **Source ↔ equation traceability** | Click a variable in Modelica source, see where it went; click a flat equation, see the line it came from |
| **A verified corpus** | F1-F9 fidelity checks over **2,614 of the 2,626 MSL models**, all green |

> <!-- CAPTURE 2 — still, PNG. The stage tabs with a cross-stage diff highlight visible.
>      Source: fixture tour `node-pointing.md`, stop 1. -->
> *[stage-view capture pending]*

## Does it tell the truth?

**This is the question an observatory has to answer**, because an observer fails silently: a
feature that breaks gets noticed, while a view that quietly misreports looks exactly like a
view that works.

So HRW checks itself against the compiler it observes. Nine invariants — **HRW must invent
nothing and omit nothing** — run at two scales: the curated specimens on every commit, and the
full Modelica Standard Library before anything is published.

- **2,614 of 2,626 MSL models, zero violations**
  ([`docs/msl-fidelity-report.csv`](docs/msl-fidelity-report.csv), with
  [provenance](docs/msl-fidelity-report.meta.json))
- The remaining 12 exceeded this machine's memory or the run's time limit, and the artifact
  **says so** rather than omitting them
- **It found two real bugs in HRW itself** — both weeks old, both introduced by ordinary work,
  neither suspected by anyone

**What it does not establish**, stated plainly because one visible overreach costs more than
several missing checks: that *Rumoca* is correct — only that HRW agrees with it — and nothing
about the rendered UI. See [`docs/fidelity-plan.md`](docs/fidelity-plan.md).

> <!-- CAPTURE 3 — still or short GIF: an animation stepping in the debugger, VS Code and
>      HRW side by side, breakpoint visibly hit.
>      This is the capability nothing else in this space has. -->
> *[live-trace capture pending]*

## Built to work *with* a reasoner

HRW is half of a pair. The other half is Claude.

The premise: **assembling a question's context is a mouse job; answering it is not.** So HRW
does the part a UI is good at — you point at a node, follow a variable, and it emits an exact
description of what you are looking at. The explanation comes from a reasoner that reads that
description. Neither half is asked to do the other's work, and **what HRW emits is exact rather
than approximate**: a missing fact is recoverable, a false one is not.

The channel runs both ways. Claude can compose a *tour* — a sequence of clickable stops through
HRW's own views — to answer a question that prose alone cannot.

---

## Getting started

**[`docs/setup-windows.md`](docs/setup-windows.md)** — from a bare Windows box to a running
app, and on to live-trace debugging. The short version:

```powershell
git clone https://github.com/dougdew64/rumoca.git && cd rumoca && git checkout hrw
# stage MSL 4.1.0 into hrw/vendor/msl/  — a fresh clone has none; see the setup guide
cargo run -p hrw
```

## Documentation

**[`docs/README.md`](docs/README.md) is the index** — every document, what it is for, and
whether it is live.

| Start here | For |
|---|---|
| [`docs/CHARTER.md`](docs/CHARTER.md) | Purpose, scope, and binding decisions |
| [`docs/vision.md`](docs/vision.md) | What this is ultimately for |
| [`docs/architecture.md`](docs/architecture.md) | How the code works |
| [`docs/compiler-phases/the-chain-of-problems.md`](docs/compiler-phases/the-chain-of-problems.md) | Why the pipeline has the shape it has |
| [`DECISIONS.md`](DECISIONS.md) | Every nontrivial implementation choice, with rationale |
| [`CLAUDE.md`](CLAUDE.md) | Working agreements for Claude |

## Layout

```
hrw/
├── src/               # The application
├── specimens/         # Modelica models, authored in Wolfram System Modeler
├── vendor/msl/        # Gitignored — staged MSL 4.1.0
├── vscode-extension/  # The HRW Debugger Bridge (out/ gitignored)
├── docs/              # Charter, architecture, compiler phases, specimen notebook
└── .hrw-bridge/       # Gitignored — runtime scratch for the Claude/debugger bridge
```

Build, run, and test from the **workspace root** with `-p hrw`, or from `hrw/` directly.

---

<!-- ============================================================================
     CAPTURE PLAN — read before shooting anything.

     Take captures AT FIXTURE-TOUR STOPS, not at arbitrary moments. A fixture
     tour already declares what should be on screen in violable terms, and
     `fixture_tour_links_all_resolve` runs over it on every test run. That makes
     a stale screenshot DETECTABLE: walking the tour is the check. A capture
     taken at an arbitrary moment has no such property — and screenshots are
     exactly the kind of regenerable-but-unchecked content this project keeps
     getting burned by. Claude cannot regenerate a screenshot.

     Available fixture tours (docs/fixture-tours/):
       camera-aiming.md                — canvas camera aiming
       frame-seeking.md                — stopping an animation on a given frame
       node-pointing.md                — pointing at a tree node, and following
       structural-vs-numerical-rank.md — cross-platform: HRW, then a notebook
       the-oracle.md                   — Rumoca vs System Modeler disagreeing

     GitHub's video rules, which are specific:
       - A committed .mp4 referenced by a RELATIVE PATH does NOT play inline;
         it renders as a link.
       - Inline video only works for assets uploaded through GitHub's own
         uploader: drag the file into an issue or PR comment, take the
         `user-attachments` URL it returns, and paste that URL here.
       - Animated GIF DOES play inline from a committed relative path, but has
         no audio, no seek bar, and gets large fast.
       - So: GIF for the short hero loop; uploaded MP4 for anything narrated or
         longer than ~10 seconds.

     AUDIENCE — decide deliberately, because the two want opposite openings:
       - A Rumoca maintainer asks "what does this show me about my compiler that
         I could not otherwise see?" -> incidence matrix, Pantelides replay, the
         2,614-model fidelity table.
       - A learner asks "will this teach me the algorithms?" -> the animations,
         the debugger sync, the specimen notebook.
     docs/upstream-strategy.md argues the maintainer framing should lead, since
     HRW is the one deliverable that asks for maintenance burden.
     ============================================================================ -->
