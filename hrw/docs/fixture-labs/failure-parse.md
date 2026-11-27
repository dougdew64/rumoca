# Failure lab — Parse, the earliest halt

<!-- kind: failure -->

**Specimen:** `UnclosedModel` — ten lines of valid Modelica with its `end` clause removed.

What this lab is for. The failure labs in this set show where a compile halts and how each
tab says so. This one is the earliest halt: a phase that fails, leaving the eleven stages
after it with nothing at all, and no simulation. Run it first — the others only make sense
against it.

The question to hold: when a pane has nothing to show, how do you tell *"the compiler
produced nothing"* from *"HRW failed to display something"*? That distinction is the whole
subject of this lab.

---

## Station 1 — The failure itself

[Load UnclosedModel → Parse](hrw://load/UnclosedModel/Parse)

**Expected:** the Parse tab shows an error, not a tree. The message begins `EP001` and says
`unexpected end of input`.

That code is Rumoca's, not HRW's. The parser reached the end of the file still expecting
`end UnclosedModel;` and had nowhere to recover to — there is no partial AST, because a
half-parsed class is not a class.

---

## Station 2 — What "stopped" costs

[Resolve](hrw://load/UnclosedModel/Resolve)

**Expected:** Resolve says it was not reached, and names Parse as the reason. It does not
show an empty tree.

An empty tree would be a claim: *this model resolved to nothing*. That is false — nothing was
attempted. The difference between those two sentences is the difference between a working
observatory and a misleading one, and it is why `Stage::info` exists as a distinct constructor
from `Stage::ok`.

Check two or three more tabs — DAE construction, Structural analysis. Each says the same
thing in its own words.

---

## Station 3 — The log agrees with the tabs

Click the Log toggle above the stage tabs. *(There is no lab-link form for the log. The
first draft of this station invented one and the link checker rejected it — which is the checker
working: a link form that does not exist must not sit in a lab looking clickable.)*

**Expected:** a `Parse` bracket that opens and closes, and no bracket for any later phase.
Not an empty `Resolve` bracket — no bracket at all.

A phase that did not run does not get a line saying it took 0.0ms. That was a real defect class
here: until 2026-08-04 the log carried a bracket named *"DAE pipeline"* that no phase
corresponded to. Brackets are now checked against `StageKind` at every emit, so an invented one
cannot be printed. *(And until 2026-11-27 this very specimen opened a `Resolve` bracket that
never closed — Resolve announced, never run. `a_parse_failure_logs_only_the_parse_bracket` now
holds this station's expectation.)*

---

## Station 4 — The distinction this specimen anchors

Now open a working model and compare.

[Load Drivetrain → Structural](hrw://load/Drivetrain/Structural)

**Expected:** the Structural tab is flagged — it says `singular` — and yet Index reduction,
Initialization and Solve lowering below it all have content.

`Drivetrain` is not broken. It is a healthy specimen whose simplified system is singular until
index reduction — six rounds of it — reduces it to three integrated state coordinates. The flag
is the compiler *narrating*, not refusing.

Hold that against `UnclosedModel`, where the tabs are empty because nothing ran:

| | `UnclosedModel` | `Drivetrain` |
|---|---|---|
| Parse | Failed | Ok |
| Structural | not reached | Flagged `singular` |
| Solve lowering | not reached | Ok |

So a flag and a halt are different things, and the tab colour says which: amber is Rumoca
reporting something while the work continues, red is where it stopped. A singularity that index
reduction repairs is carried on. A parse error, a resolve error, a typecheck diagnostic and an
unbalanced equation count all stop the compile — each failure lab here is one of those — and an
initial condition the model cannot satisfy stops the *run*, at its first instant. Parse is
simply the earliest stop, with nothing before it to show.

Measured with `cargo run -p hrw --example failure_map`, which lists where each specimen stops.
Its first version conflated these two and duly reported four healthy specimens as broken.

---

## What to bring back

- Did any tab show you something you could not tell the origin of? That is the defect this lab
  is hunting.
- Is *"not reached"* enough, or do you want the tab to say which phase stopped and why?
  Station 2 shows the current wording; it is HRW's, not Rumoca's, and it can change.
