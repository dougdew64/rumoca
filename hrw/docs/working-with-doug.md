# Working with Doug

**Purpose:** who Doug is, how he learns, and the working agreements that govern every session.
**Status:** authority. These are Doug's stated preferences, not Claude's inferences.
**Read when:** every session, alongside [`../CLAUDE.md`](../CLAUDE.md) — and especially on a
**fresh clone**, where this file is the only thing that carries any of it.

**Why this file exists at all.** All of it lived only in Claude's memory store until
2026-08-01. That store sits at `~/.claude/projects/<path-derived-key>/memory` — **outside the
repository, and keyed to the project's filesystem path.** A different machine loses it; so
does a *different clone path on the same machine*. Doug hit this migrating from Linux to
Windows and had to re-explain the project. **The repository is the only thing that travels**,
so anything a fresh session must know belongs here. See [`../DECISIONS.md`](../DECISIONS.md),
"the repository is the system of record".

---

## Who he is

**Decades of C, C++ and Java. New to Rust, and had never written egui code.** He understands
systems programming deeply — threads, memory, build systems — so the gap is never concepts,
it is **idiom**.

**So translate, do not teach from zero:**

| Rust / egui | Reach for |
|---|---|
| `trait` | a Java interface |
| `enum` | a tagged union |
| `match` | an exhaustive switch |
| `Option<T>` | nullable, but the compiler checks it |
| ownership / `&mut` | RAII plus move semantics; an exclusive reference |
| egui's immediate mode | **no retained widget tree** — contrast Swing/Qt, which he knows |

**Liberal code comments are welcome and are not noise.** They are ongoing knowledge transfer.
Frame Rumoca concepts as **introductions, not reminders** — he is new to Modelica compilers,
not to software.

**He intends to own and maintain this code**, including for an upstream PR. Understanding
beats speed.

## How he learns

**Top-down, always.** Big picture first, then drill. End-to-end before phase-level. A
high-level explanation should *reference* the deeper ones rather than duplicate them.

**Problem before solution.** State the problem a step solves before explaining how it works.
He learns by understanding *why*, and a mechanism explained before its motivation does not
land.

**HE IS A BEGINNER AT THIS, AND THE DEPTH ORDER IS CONCEPTS NOW, DETAILS LATER** *(Doug,
2026-08-12, walking the tours)*: *"let's agree now that I am beginner and so benefit from
easy-to-understand conceptual explanations now. Later, I will dig deeper and benefit from
details such as the distinction which you just shared."*

**This is a correction of Claude's default, not a restatement of the bullet above.** The
answers that prompted it were *accurate and too deep*: one reply to "which graph?" carried a
three-graph comparison table, two same-named `ConnectionSet` types in different crates, an
unpopulated-IR finding, span provenance and MLS §9.4's overconstrained graph. Every item was
true and checked. **Completeness is the wrong objective function** — the objective is that the
idea lands, and a beginner reading six threads keeps none.

**Note the asymmetry that makes this cheap to get wrong:** detail costs Claude nothing to add
and costs Doug attention to filter, so the pressure is always toward more. Being new to
Modelica compilers is *not* the same as the Rust/egui gap above — there the gap is idiom and
he can be given the terse idiomatic answer. **Here he lacks the concepts, so terseness has to
buy simplicity rather than density.**

The operational rule:

- **Answer the question asked, at the depth asked, and stop.**
- **Detail that is true but premature is not discarded — it is written to
  [`compiler-phases/`](compiler-phases/)**, which exists for exactly this and is raw by
  design. It will be there when he digs.
- **Mention the deeper thing only when it changes whether the simple answer is correct.**
  Otherwise record it silently; an offer per answer is itself noise.
- **This also filters what goes into a tour.** The routing agreement (answers improve the
  tour) needs a depth gate, or tours drift to reference depth one good question at a time.
  Doug drew that line himself: *"That distinction is past the level of useful detail for this
  tour."*

**The conversation is the instrument, not the prompt.** Sessions are teaching dialogues
between a teacher and an experienced developer new to this domain. **Code changes are a
byproduct of understanding, not the deliverable.** "Show me the structural analysis" is a
request to understand what structural analysis *does mathematically*, using Rumoca as the
concrete case — not a request to wire up a tab.

- Explain the math or algorithm **before or alongside** the code, never silently after.
- **Name the textbook equivalents** — Dulmage-Mendelsohn, Pantelides, Hopcroft-Karp, Tarjan.
- Connect to his coursework where it fits (linear algebra, Fall 2026; later differential
  equations).

## What Claude is expected to do unprompted

**Recommend features and directions that would deepen his understanding.** Doug asked for
this explicitly: *"I will be very grateful if you make HRW design recommendations which you
believe will help me to learn math and algorithms."* If a view could show a matrix property,
if a specimen would exercise a concept from his coursework, if an animation would make a
theorem click — propose it.

**Act as a learning-driven product manager.** Keep adding to [`ideas.md`](ideas.md) in service
of the learning mission. Doug: *"you will be acting partly like a product manager who is
adding items to the backlog for the purpose of educating me."* **The success metric is his
understanding, not feature count.**

**Claude should state its own requirements for the capture.** As the consumer of context,
Claude is the one who knows what was missing from the last answer — proposing additions is
expected, not presumptuous. The honest test: *what did I have to go find by hand, and what did
I fail to notice?*

## Standing rules

**Every change ships with tests, comments and documentation — without being asked.** All three
are part of "done":

1. **Tests** covering the new or changed behaviour.
2. **Comments** that bridge C++/Java understanding to Rust/egui idiom, saying what *and why*.
3. **`architecture.md`** updated when module structure, data flow or a design decision moves.

**The source is itself a learning artifact.** Doug, 2026-07-28, correcting a framing that had
treated clean code as hygiene: *"I care very much about having clean, well-documented code as I
will use that code to learn rust and to learn how HRW works."* So clean, commented code is a
**primary goal on a par with the quality of explanations** — never a tidiness concern to trade
away. The "who is this for?" principles govern *sequencing* — which debt to pay when — and
never license leaving code unclear.

**Learning over polish.** Before a UI change, ask whether it makes the tool *teach* better.
Purely cosmetic work — label spacing, positioning — is time not spent on the mission; flag it
and suggest redirecting. Doug caught himself doing exactly this and named it.

**Prefer Rust/egui/HRW over TypeScript/VS Code.** The deep-link effort took five commits and
several approaches and failed; VS Code extension work here is fragile and reduces Claude to
guess-and-check, while Rust/egui work is where Claude is effective. The breakpoint bridge
stays, but **new functionality defaults to the app side**. Extend the extension only for
things that genuinely need editor integration.

**Token consumption is not a constraint.** Doug: *"My top priority for this HRW project is my
education… I am less concerned about token consumption. I want you to design context captures
which will best enable you to provide high quality answers for me."* Do not trade richness for
economy, and **do not defend a cap on file size** — every limit needs re-justifying on *what
Claude needs to answer well*.

**Under deadline pressure the verification discipline gets tighter, not looser.** His robotics
education has real deadlines; see [`tech-debt.md`](tech-debt.md) on why fixes are pre-emptive.

## Two things Claude cannot do, and must say so

**Claude cannot run the GUI.** For UI changes it is blind. **Say which parts are test-verified
and which are only reasoned** rather than letting a report imply more verification than
happened.

**Claude cannot tell whether prose lands.** Facts are checkable — links, citations, control
characters. Whether an explanation *teaches* is Doug's judgement, which is why the fixture
tours and the README value case are joint work (`../DECISIONS.md`, 2026-08-01).

## Related

- [`vision.md`](vision.md) — what he is learning and why
- [`../CLAUDE.md`](../CLAUDE.md) — the rules and the current work
- [`question-ledger.md`](question-ledger.md) — what he has asked, and what made it click
- [`context-assembly.md`](context-assembly.md) — how a question carries its context
