# Lab catalogue

**Generated — do not edit.** `cargo run -p hrw --example gen_lab_catalogue`.

**Audience: Claude.** This exists so a question can be answered by citing a lab that already demonstrates the thing, rather than by writing a new one that retells it without its checked expectations (`docs/ideas.md` #63). Cite a stop with `hrw://lab/<name>/station/<slug>`.

**Re-read a lab before citing it.** Everything below is derived and current; whether a lab's *claims* still hold is not something a catalogue can know, and a lab promised a tree the pane did not show for its whole existence.

## `blt-ordering`

**Fixture lab — BLT: finding an order, and finding out there isn't one**

A concept lab. Run [matching](hrw://lab/matching) first — it answers *which* equation

- **Specimens:** `RcCircuit`, `ProportionalLoop`, `TwoLoops`
- **Stages:** `Structural`
- **Stations:**
  - `fixture-lab-blt-finding-an-order-and-finding-out-there-isn-t-one` — Fixture lab — BLT: finding an order, and finding out there isn't one
  - `the-problem-this-phase-exists-to-solve` — The problem this phase exists to solve
  - `station-1-when-an-order-exists` — Station 1 — When an order exists
  - `station-2-when-no-order-exists` — Station 2 — When no order exists
  - `station-3-when-the-system-splits` — Station 3 — When the system splits
  - `station-4-what-you-have-been-building-is-a-block-triangular-form` — Station 4 — What you have been building is a block triangular form
  - `what-this-lab-cannot-check` — What this lab cannot check
  - `what-comes-next-in-the-chain` — What comes next in the chain

## `camera-aiming`

**Fixture lab — camera aiming**

This is a test, not an explanation. It exists so Doug can verify the half of camera

- **Specimens:** `RcCircuit`
- **Stages:** `Structural`
- **Stations:**
  - `fixture-lab-camera-aiming` — Fixture lab — camera aiming
  - `station-1-load-and-note-where-the-camera-starts` — Station 1 — Load, and note where the camera starts
  - `station-2-aim-at-the-first-equation` — Station 2 — Aim at the first equation
  - `station-3-aim-at-the-far-corner` — Station 3 — Aim at the far corner
  - `station-4-aim-at-something-that-is-not-there` — Station 4 — Aim at something that is not there
  - `station-5-aiming-survives-a-resize` — Station 5 — Aiming survives a resize
  - `what-this-cannot-check` — What this cannot check

## `connect-expansion`

**Flatten — what `connect` actually means**

This lab counts. `RcCircuit` has four `connect` statements and twenty-three equations, and every

- **Specimens:** `RcCircuit`, `TwoLoops`, `ScopedConnect`
- **Stages:** `Flatten`
- **Stations:**
  - `flatten-what-connect-actually-means` — Flatten — what `connect` actually means
  - `station-1-how-many-connection-sets` — Station 1 — How many connection sets?
  - `station-2-how-many-equations-does-a-set-make` — Station 2 — How many equations does a set make?
  - `station-3-which-rows-belong-to-the-same-set` — Station 3 — Which rows belong to the same set?
  - `station-4-how-big-is-a-four-component-circuit` — Station 4 — How big is a four-component circuit?
  - `station-5-what-if-there-are-no-connectors-at-all` — Station 5 — What if there are no connectors at all?
  - `station-6-do-the-sets-still-come-out-matched` — Station 6 — Do the sets still come out matched?
  - `what-comes-next-in-the-chain` — What comes next in the chain
  - `what-this-lab-cannot-check` — What this lab cannot check

## `dae-construction`

**Fixture lab — DAE construction: the count that decides everything**

A concept lab. It teaches a step of the chain

- **Specimens:** `SingleInertia`, `UnbalancedShaft`, `OverDeterminedShaft`
- **Stages:** `Dae`, `Flatten`, `Structural`
- **Stations:**
  - `fixture-lab-dae-construction-the-count-that-decides-everything` — Fixture lab — DAE construction: the count that decides everything
  - `the-problem-this-phase-exists-to-solve` — The problem this phase exists to solve
  - `station-1-which-declarations-carry-the-past` — Station 1 — Which declarations carry the past?
  - `station-2-what-makes-a-variable-a-state` — Station 2 — What makes a variable a state?
  - `station-3-what-is-the-solver-actually-solving-for` — Station 3 — What is the solver actually solving for?
  - `station-4-the-claim` — Station 4 — The claim
  - `station-5-what-the-compiler-says-when-the-claim-fails` — Station 5 — What the compiler says when the claim fails
  - `station-6-the-other-sign` — Station 6 — The other sign
  - `station-7-where-it-fails-and-why-that-is-the-right-place` — Station 7 — Where it fails, and why that is the right place
  - `two-excursions-if-you-want-them` — Two excursions, if you want them
  - `what-this-lab-cannot-check` — What this lab cannot check
  - `what-comes-next-in-the-chain` — What comes next in the chain

## `events`

**Fixture lab — Events: the equations that are not always true**

A concept lab. Run [initialization](hrw://lab/initialization) first. Everything so far has

- **Specimens:** `BouncingBall`, `RcCircuit`, `GearWithBrake`
- **Stages:** `Events`
- **Stations:**
  - `fixture-lab-events-the-equations-that-are-not-always-true` — Fixture lab — Events: the equations that are not always true
  - `the-problem-this-phase-exists-to-solve` — The problem this phase exists to solve
  - `station-1-a-model-with-a-real-event` — Station 1 — A model with a real event
  - `station-2-a-model-with-none-and-what-the-pane-says` — Station 2 — A model with none, and what the pane says
  - `station-3-a-model-with-several` — Station 3 — A model with several
  - `what-this-lab-cannot-check` — What this lab cannot check
  - `what-comes-next-in-the-chain` — What comes next in the chain

## `failure-flatten`

**Failure lab — Flatten, where the count is checked**

Specimen: `UnbalancedShaft` — `SingleInertia` with one line changed.

- **Specimens:** `UnbalancedShaft`, `SingleInertia`
- **Stages:** `Dae`, `Flatten`, `Resolve`
- **Stations:**
  - `failure-lab-flatten-where-the-count-is-checked` — Failure lab — Flatten, where the count is checked
  - `station-1-the-refusal` — Station 1 — The refusal
  - `station-2-why-it-could-not-be-checked-sooner` — Station 2 — Why it could not be checked sooner
  - `station-3-the-phase-that-owns-the-error-and-the-phase-that-reports-it` — Station 3 — The phase that owns the error, and the phase that reports it
  - `station-4-what-one-line-did` — Station 4 — What one line did
  - `what-to-bring-back` — What to bring back

## `failure-initialization`

**Failure lab — Initialization, where too much information is the problem**

Specimens: `OverInitRc` and `RotationalInertia`. Two ways the t=0 problem goes wrong, and

- **Specimens:** `OverInitRc`, `RotationalInertia`
- **Stages:** `Initialization`, `SolveLowering`
- **Stations:**
  - `failure-lab-initialization-where-too-much-information-is-the-problem` — Failure lab — Initialization, where too much information is the problem
  - `station-1-more-conditions-than-states` — Station 1 — More conditions than states
  - `station-2-the-other-direction` — Station 2 — The other direction
  - `station-3-everything-downstream-still-runs` — Station 3 — Everything downstream still runs
  - `station-4-the-determinacy-verdict` — Station 4 — The determinacy verdict
  - `what-to-bring-back` — What to bring back

## `failure-parse`

**Failure lab — Parse, the only phase that truly stops**

Specimen: `UnclosedModel` — ten lines of valid Modelica with its `end` clause removed.

- **Specimens:** `UnclosedModel`, `Drivetrain`
- **Stages:** `Parse`, `Resolve`, `Structural`
- **Stations:**
  - `failure-lab-parse-the-only-phase-that-truly-stops` — Failure lab — Parse, the only phase that truly stops
  - `station-1-the-failure-itself` — Station 1 — The failure itself
  - `station-2-what-stopped-costs` — Station 2 — What "stopped" costs
  - `station-3-the-log-agrees-with-the-tabs` — Station 3 — The log agrees with the tabs
  - `station-4-the-distinction-this-specimen-anchors` — Station 4 — The distinction this specimen anchors
  - `what-to-bring-back` — What to bring back

## `failure-resolve`

**Failure lab — Resolve, where a name is looked up and the answer is recorded**

Specimens: `UndefinedRef` and `MissingComponentClass`. Run them together; neither is worth

- **Specimens:** `UndefinedRef`, `MissingComponentClass`
- **Stages:** `Flatten`, `Resolve`
- **Stations:**
  - `failure-lab-resolve-where-a-name-is-looked-up-and-the-answer-is-recorded` — Failure lab — Resolve, where a name is looked up and the answer is recorded
  - `station-1-the-error-is-found-here` — Station 1 — The error is found here
  - `station-2-and-the-compile-stops-somewhere-else` — Station 2 — And the compile stops somewhere else
  - `station-3-which-kind-of-name-was-missing` — Station 3 — Which *kind* of name was missing
  - `station-4-read-it-in-the-log` — Station 4 — Read it in the log
  - `what-to-bring-back` — What to bring back

## `failure-structural`

**Failure lab — Structural analysis, where counting stops being enough**

Specimens: `TwiceDefined` and `CapacitorLoop`. Both are flagged `singular`. They are not

- **Specimens:** `TwiceDefined`, `CapacitorLoop`
- **Stages:** `Dae`, `Structural`
- **Stations:**
  - `failure-lab-structural-analysis-where-counting-stops-being-enough` — Failure lab — Structural analysis, where counting stops being enough
  - `station-1-square-and-singular-anyway` — Station 1 — Square, and singular anyway
  - `station-2-what-matching-finds` — Station 2 — What matching finds
  - `station-3-the-same-flag-a-different-cause` — Station 3 — The same flag, a different cause
  - `station-4-what-no-blocks-means` — Station 4 — What "no blocks" means
  - `what-to-bring-back` — What to bring back

## `failure-typecheck`

**Failure lab — Typecheck, which reports and does not stop at all**

Specimen: `DimensionMismatch` — a 2-vector assigned from a 3-vector.

- **Specimens:** `DimensionMismatch`, `UnclosedModel`
- **Stages:** `Flatten`, `SolveLowering`, `Typecheck`
- **Stations:**
  - `failure-lab-typecheck-which-reports-and-does-not-stop-at-all` — Failure lab — Typecheck, which reports and does not stop at all
  - `station-1-the-diagnosis` — Station 1 — The diagnosis
  - `station-2-the-surprise` — Station 2 — The surprise
  - `station-3-where-the-truth-is-kept` — Station 3 — Where the truth is kept
  - `station-4-compare-where-the-compile-halts` — Station 4 — Compare where the compile halts
  - `what-to-bring-back` — What to bring back

## `frame-seeking`

**Fixture lab — seeking to a frame**

This is a test, not an explanation. It verifies that a link can stop an animation on

- **Specimens:** `MotorWithBrake`
- **Stages:** `Structural`
- **Stations:**
  - `fixture-lab-seeking-to-a-frame` — Fixture lab — seeking to a frame
  - `station-0-a-stop-clicked-out-of-order` — Station 0 — A stop clicked out of order
  - `station-1-a-replay-unstarted` — Station 1 — A replay, unstarted
  - `station-2-jump-into-the-middle` — Station 2 — Jump into the middle
  - `station-3-jump-backwards` — Station 3 — Jump backwards
  - `station-4-seek-past-the-end` — Station 4 — Seek past the end
  - `station-5-seek-in-a-different-animation` — Station 5 — Seek in a different animation
  - `station-6-seek-a-view-that-has-no-animation` — Station 6 — Seek a view that has no animation
  - `what-this-cannot-check` — What this cannot check

## `index-reduction`

**Fixture lab — Index reduction: when differentiating is the only way out**

A concept lab. Run [blt-ordering](hrw://lab/blt-ordering) and

- **Specimens:** `CartesianPendulum`, `BouncingBall`, `BenchActuator`, `Drivetrain`
- **Stages:** `IndexReduction`, `Structural`
- **Stations:**
  - `fixture-lab-index-reduction-when-differentiating-is-the-only-way-out` — Fixture lab — Index reduction: when differentiating is the only way out
  - `the-problem-this-phase-exists-to-solve` — The problem this phase exists to solve
  - `station-1-the-case-that-needs-nothing` — Station 1 — The case that needs nothing
  - `station-2-the-smallest-model-that-needs-something` — Station 2 — The smallest model that needs something
  - `station-3-the-same-idea-at-a-scale-you-could-not-do-by-hand` — Station 3 — The same idea, at a scale you could not do by hand
  - `station-4-what-the-compiler-actually-reached-for` — Station 4 — What the compiler actually reached for
  - `station-5-the-model-rumoca-cannot-reduce` — Station 5 — The model Rumoca cannot reduce
  - `what-this-lab-cannot-check` — What this lab cannot check
  - `what-comes-next-in-the-chain` — What comes next in the chain

## `initialization`

**Fixture lab — Initialization: the values at t = 0**

A concept lab. Run [index-reduction](hrw://lab/index-reduction) first — this phase runs on

- **Specimens:** `BouncingBall`, `RcCircuit`, `OverInitRc`, `RotationalInertia`
- **Stages:** `Initialization`
- **Stations:**
  - `fixture-lab-initialization-the-values-at-t-0` — Fixture lab — Initialization: the values at t = 0
  - `the-problem-this-phase-exists-to-solve` — The problem this phase exists to solve
  - `station-1-the-case-with-nothing-to-solve` — Station 1 — The case with nothing to solve
  - `station-2-the-case-with-a-real-initialization-system` — Station 2 — The case with a real initialization system
  - `station-3-the-case-that-specifies-too-much` — Station 3 — The case that specifies too much
  - `station-4-square-and-still-singular-again` — Station 4 — Square and still singular, again
  - `what-this-lab-cannot-check` — What this lab cannot check
  - `what-comes-next-in-the-chain` — What comes next in the chain

## `matching-live`

**Fixture lab — Matching, live: the call stack is the augmenting path**

A concept lab, pass two. [matching](hrw://lab/matching) taught the idea; this one is about

- **Specimens:** `ProportionalLoop`, `TwiceDefined`
- **Stages:** `Structural`
- **Stations:**
  - `fixture-lab-matching-live-the-call-stack-is-the-augmenting-path` — Fixture lab — Matching, live: the call stack is the augmenting path
  - `station-0-two-things-must-be-true-before-any-of-this-works` — Station 0 — Two things must be true before any of this works
  - `station-1-arm-an-anchor-and-learn-what-an-anchor-is-named` — Station 1 — Arm an anchor, and learn what an anchor is named
  - `station-2-the-call-stack-is-the-augmenting-path` — Station 2 — The call stack is the augmenting path
  - `station-3-the-same-machinery-refusing` — Station 3 — The same machinery, refusing
  - `station-4-what-this-instrument-can-and-cannot-show-you` — Station 4 — What this instrument can and cannot show you
  - `what-this-lab-cannot-check` — What this lab cannot check
  - `what-comes-next` — What comes next

## `matching`

**Fixture lab — Matching: which equation solves which unknown**

A concept lab. It teaches a step of the chain and uses HRW as the instrument. It is still

- **Specimens:** `BouncingBall`, `ProportionalLoop`, `CapacitorLoop`
- **Stages:** `Structural`
- **Stations:**
  - `fixture-lab-matching-which-equation-solves-which-unknown` — Fixture lab — Matching: which equation solves which unknown
  - `the-problem-this-phase-exists-to-solve` — The problem this phase exists to solve
  - `station-1-the-case-where-it-is-obvious` — Station 1 — The case where it is obvious
  - `station-2-the-case-that-is-not-obvious-at-all` — Station 2 — The case that is not obvious at all
  - `station-3-the-case-with-no-answer` — Station 3 — The case with no answer
  - `station-4-what-this-is-called-and-why-the-name-helps` — Station 4 — What this is called, and why the name helps
  - `what-this-lab-cannot-check` — What this lab cannot check
  - `what-comes-next-in-the-chain` — What comes next in the chain

## `node-pointing`

**Fixture lab — pointing at a tree node, and following**

This is a test, not an explanation. It verifies the last two verbs of the answer

- **Specimens:** `RcCircuit`
- **Stages:** `Structural`
- **Stations:**
  - `fixture-lab-pointing-at-a-tree-node-and-following` — Fixture lab — pointing at a tree node, and following
  - `station-1-open-the-tree` — Station 1 — Open the tree
  - `station-2-point-at-a-shallow-node` — Station 2 — Point at a shallow node
  - `station-3-point-at-something-nested` — Station 3 — Point at something nested
  - `station-4-point-somewhere-deeper-still` — Station 4 — Point somewhere deeper still
  - `station-5-a-path-that-is-not-there` — Station 5 — A path that is not there
  - `station-5b-a-view-this-model-does-not-have` — Station 5b — A view this model does not have
  - `station-6-follow-an-identifier` — Station 6 — Follow an identifier
  - `station-7-follow-then-point-and-see-both` — Station 7 — Follow, then point, and see both
  - `what-this-cannot-check` — What this cannot check

## `solve-lowering`

**Fixture lab — Solve lowering: names become indices**

A concept lab. The last phase before simulation. Run [events](hrw://lab/events) first.

- **Specimens:** `BouncingBall`, `RcCircuit`
- **Stages:** `SolveLowering`
- **Stations:**
  - `fixture-lab-solve-lowering-names-become-indices` — Fixture lab — Solve lowering: names become indices
  - `the-problem-this-phase-exists-to-solve` — The problem this phase exists to solve
  - `station-1-where-your-variables-went` — Station 1 — Where your variables went
  - `station-2-what-else-is-in-the-arrays` — Station 2 — What else is in the arrays
  - `station-3-the-same-mapping-at-scale` — Station 3 — The same mapping at scale
  - `what-this-lab-cannot-check` — What this lab cannot check
  - `one-count-here-contradicts-the-index-reduction-lab` — One count here contradicts the index-reduction lab
  - `what-comes-next-in-the-chain` — What comes next in the chain

## `structural-vs-numerical-rank`

**Structural rank vs numerical rank**

The first cross-platform lab. Two stops in HRW, then a notebook — because the point

- **Specimens:** `ProportionalLoop`
- **Stages:** `Structural`
- **Stations:**
  - `structural-rank-vs-numerical-rank` — Structural rank vs numerical rank
  - `station-1-the-pattern-from-a-real-model` — 📐 Station 1 — The pattern, from a real model
  - `station-2-what-hrw-concludes-from-it` — 📐 Station 2 — What HRW concludes from it
  - `station-3-where-the-values-go-in` — 🧮 Station 3 — Where the values go in
  - `station-4-back-to-hrw-and-what-it-would-say` — 📐 Station 4 — Back to HRW, and what it would say
  - `what-each-side-uniquely-holds` — What each side uniquely holds
  - `what-this-cannot-check` — What this cannot check

## `tearing`

**Fixture lab — Tearing: guess one number, get the rest for free**

A concept lab. Run [blt-ordering](hrw://lab/blt-ordering) first — it produces the coupled

- **Specimens:** `ProportionalLoop`, `TwoLoops`, `MixedLoop`, `LoopWithInertia`
- **Stages:** `Structural`
- **Stations:**
  - `fixture-lab-tearing-guess-one-number-get-the-rest-for-free` — Fixture lab — Tearing: guess one number, get the rest for free
  - `the-problem-this-phase-exists-to-solve` — The problem this phase exists to solve
  - `station-1-guess-one-number-and-the-rest-falls-out` — Station 1 — Guess one number and the rest falls out
  - `station-2-watch-the-choice-being-made` — Station 2 — Watch the choice being made
  - `station-3-two-blocks-torn-independently` — Station 3 — Two blocks, torn independently
  - `station-4-all-three-kinds-of-block-in-one-model` — Station 4 — All three kinds of block in one model
  - `station-5-what-it-costs-once-time-is-moving` — Station 5 — What it costs once time is moving
  - `what-this-lab-cannot-check` — What this lab cannot check
  - `what-comes-next-in-the-chain` — What comes next in the chain

## `the-concepts`

**The concepts — a week's run through the pipeline**

Start here. This is the map for the labs of the compiler phases, in the order

- **Stations:**
  - `the-concepts-a-week-s-run-through-the-pipeline` — The concepts — a week's run through the pipeline
  - `the-route` — The route
  - `the-four-numbers-that-connect-the-labs` — The four numbers that connect the labs
  - `the-one-structural-idea-the-whole-pipeline-turns-on` — The one structural idea the whole pipeline turns on
  - `three-graphs-three-classical-questions` — Three graphs, three classical questions
  - `three-things-worth-knowing-before-you-start` — Three things worth knowing before you start
  - `two-open-questions-you-may-hit` — Two open questions you may hit
  - `what-to-tell-me-afterwards` — What to tell me afterwards

## `the-oracle`

**The oracle — when Rumoca and System Modeler disagree**

A lab that leaves HRW to settle a question HRW cannot settle. Rumoca accepts a model

- **Specimens:** `IncompatibleConnect`
- **Stages:** `Flatten`
- **Stations:**
  - `the-oracle-when-rumoca-and-system-modeler-disagree` — The oracle — when Rumoca and System Modeler disagree
  - `station-1-a-specimen-built-to-fail-at-flatten` — 📐 Station 1 — A specimen built to fail at flatten
  - `station-2-where-it-actually-fails` — 📐 Station 2 — Where it actually fails
  - `station-3-ask-the-other-implementation` — ⚙ Station 3 — Ask the other implementation
  - `station-4-why-this-could-not-be-settled-inside-hrw` — 📐 Station 4 — Why this could not be settled inside HRW
  - `the-rule-this-lab-exists-to-make-concrete` — The rule this lab exists to make concrete
  - `what-this-cannot-check` — What this cannot check

