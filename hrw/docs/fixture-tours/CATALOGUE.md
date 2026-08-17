# Tour catalogue

**Generated — do not edit.** `cargo run -p hrw --example gen_tour_catalogue`.

**Audience: Claude.** This exists so a question can be answered by citing a tour that already demonstrates the thing, rather than by writing a new one that retells it without its checked expectations (`docs/ideas.md` #63). Cite a stop with `hrw://tour/<name>/stop/<slug>`.

**Re-read a tour before citing it.** Everything below is derived and current; whether a tour's *claims* still hold is not something a catalogue can know, and a tour promised a tree the pane did not show for its whole existence.

## `blt-ordering`

**Fixture tour — BLT: finding an order, and finding out there isn't one**

A curriculum tour. Walk [`matching.md`](matching.md) first — it answers *which* equation

- **Specimens:** `RcCircuit`, `ProportionalLoop`, `TwoLoops`
- **Stages:** `Structural`
- **Stops:**
  - `fixture-tour-blt-finding-an-order-and-finding-out-there-isn-t-one` — Fixture tour — BLT: finding an order, and finding out there isn't one
  - `the-problem-this-phase-exists-to-solve` — The problem this phase exists to solve
  - `act-1-when-an-order-exists` — Act 1 — When an order exists
  - `act-2-when-no-order-exists` — Act 2 — When no order exists
  - `act-3-when-the-system-splits` — Act 3 — When the system splits
  - `act-4-what-you-have-been-building-is-a-block-triangular-form` — Act 4 — What you have been building is a block triangular form
  - `what-this-tour-cannot-check` — What this tour cannot check
  - `what-comes-next-in-the-chain` — What comes next in the chain

## `camera-aiming`

**Fixture tour — camera aiming**

This is a test, not an explanation. It exists so Doug can verify the half of camera

- **Specimens:** `RcCircuit`
- **Stages:** `Structural`
- **Stops:**
  - `fixture-tour-camera-aiming` — Fixture tour — camera aiming
  - `stop-1-load-and-note-where-the-camera-starts` — Stop 1 — Load, and note where the camera starts
  - `stop-2-aim-at-the-first-equation` — Stop 2 — Aim at the first equation
  - `stop-3-aim-at-the-far-corner` — Stop 3 — Aim at the far corner
  - `stop-4-aim-at-something-that-is-not-there` — Stop 4 — Aim at something that is not there
  - `stop-5-aiming-survives-a-resize` — Stop 5 — Aiming survives a resize
  - `what-this-cannot-check` — What this cannot check

## `connect-expansion`

**Flatten — what `connect` actually means**

This tour counts. `RcCircuit` has four `connect` statements and twenty-three equations, and every

- **Specimens:** `RcCircuit`, `TwoLoops`
- **Stages:** `Flatten`
- **Stops:**
  - `flatten-what-connect-actually-means` — Flatten — what `connect` actually means
  - `act-1-how-many-nodes` — Act 1 — How many nodes?
  - `act-2-how-many-equations-do-three-nodes-make` — Act 2 — How many equations do three nodes make?
  - `act-3-which-rows-belong-to-the-same-node` — Act 3 — Which rows belong to the same node?
  - `act-4-how-big-is-a-four-component-circuit` — Act 4 — How big is a four-component circuit?
  - `act-5-what-if-there-are-no-connectors-at-all` — Act 5 — What if there are no connectors at all?
  - `what-comes-next-in-the-chain` — What comes next in the chain
  - `what-this-tour-cannot-check` — What this tour cannot check

## `dae-construction`

**Fixture tour — DAE construction: the count that decides everything**

A curriculum tour. It teaches a step of the chain

- **Specimens:** `SingleInertia`, `UnbalancedShaft`
- **Stages:** `Dae`, `Flatten`, `Structural`
- **Stops:**
  - `fixture-tour-dae-construction-the-count-that-decides-everything` — Fixture tour — DAE construction: the count that decides everything
  - `the-problem-this-phase-exists-to-solve` — The problem this phase exists to solve
  - `act-1-which-declarations-carry-the-past` — Act 1 — Which declarations carry the past?
  - `act-2-what-makes-a-variable-a-state` — Act 2 — What makes a variable a state?
  - `act-3-what-is-the-solver-actually-solving-for` — Act 3 — What is the solver actually solving for?
  - `act-4-the-claim` — Act 4 — The claim
  - `act-5-what-the-compiler-says-when-the-claim-fails` — Act 5 — What the compiler says when the claim fails
  - `act-6-where-it-fails-and-why-that-is-the-right-place` — Act 6 — Where it fails, and why that is the right place
  - `two-excursions-if-you-want-them` — Two excursions, if you want them
  - `what-this-tour-cannot-check` — What this tour cannot check
  - `what-comes-next-in-the-chain` — What comes next in the chain

## `events`

**Fixture tour — Events: the equations that are not always true**

A curriculum tour. Walk [`initialization.md`](initialization.md) first. Everything so far has

- **Specimens:** `BouncingBall`, `RcCircuit`, `GearWithBrake`
- **Stages:** `Events`
- **Stops:**
  - `fixture-tour-events-the-equations-that-are-not-always-true` — Fixture tour — Events: the equations that are not always true
  - `the-problem-this-phase-exists-to-solve` — The problem this phase exists to solve
  - `act-1-a-model-with-a-real-event` — Act 1 — A model with a real event
  - `act-2-a-model-with-none-and-what-the-pane-says` — Act 2 — A model with none, and what the pane says
  - `act-3-a-model-with-several` — Act 3 — A model with several
  - `what-this-tour-cannot-check` — What this tour cannot check
  - `what-comes-next-in-the-chain` — What comes next in the chain

## `failure-flatten`

**Failure tour — Flatten, where the count is checked**

Specimen: `UnbalancedShaft` — `SingleInertia` with one line changed.

- **Specimens:** `UnbalancedShaft`, `SingleInertia`
- **Stages:** `Dae`, `Flatten`, `Resolve`
- **Stops:**
  - `failure-tour-flatten-where-the-count-is-checked` — Failure tour — Flatten, where the count is checked
  - `stop-1-the-refusal` — Stop 1 — The refusal
  - `stop-2-why-it-could-not-be-checked-sooner` — Stop 2 — Why it could not be checked sooner
  - `stop-3-the-phase-that-owns-the-error-and-the-phase-that-reports-it` — Stop 3 — The phase that owns the error, and the phase that reports it
  - `stop-4-what-one-line-did` — Stop 4 — What one line did
  - `what-to-bring-back` — What to bring back

## `failure-initialization`

**Failure tour — Initialization, where too much information is the problem**

Specimens: `OverInitRc` and `RotationalInertia`. Two ways the t=0 problem goes wrong, and

- **Specimens:** `OverInitRc`, `RotationalInertia`
- **Stages:** `Initialization`, `SolveLowering`
- **Stops:**
  - `failure-tour-initialization-where-too-much-information-is-the-problem` — Failure tour — Initialization, where too much information is the problem
  - `stop-1-more-conditions-than-states` — Stop 1 — More conditions than states
  - `stop-2-the-other-direction` — Stop 2 — The other direction
  - `stop-3-everything-downstream-still-runs` — Stop 3 — Everything downstream still runs
  - `stop-4-the-determinacy-verdict` — Stop 4 — The determinacy verdict
  - `what-to-bring-back` — What to bring back

## `failure-parse`

**Failure tour — Parse, the only phase that truly stops**

Specimen: `UnclosedModel` — ten lines of valid Modelica with its `end` clause removed.

- **Specimens:** `UnclosedModel`, `Drivetrain`
- **Stages:** `Parse`, `Resolve`, `Structural`
- **Stops:**
  - `failure-tour-parse-the-only-phase-that-truly-stops` — Failure tour — Parse, the only phase that truly stops
  - `stop-1-the-failure-itself` — Stop 1 — The failure itself
  - `stop-2-what-stopped-costs` — Stop 2 — What "stopped" costs
  - `stop-3-the-log-agrees-with-the-tabs` — Stop 3 — The log agrees with the tabs
  - `stop-4-the-distinction-this-specimen-anchors` — Stop 4 — The distinction this specimen anchors
  - `what-to-bring-back` — What to bring back

## `failure-resolve`

**Failure tour — Resolve, where a name is looked up and the answer is recorded**

Specimens: `UndefinedRef` and `MissingComponentClass`. Walk them together; neither is worth

- **Specimens:** `UndefinedRef`, `MissingComponentClass`
- **Stages:** `Flatten`, `Resolve`
- **Stops:**
  - `failure-tour-resolve-where-a-name-is-looked-up-and-the-answer-is-recorded` — Failure tour — Resolve, where a name is looked up and the answer is recorded
  - `stop-1-the-error-is-found-here` — Stop 1 — The error is found here
  - `stop-2-and-the-compile-stops-somewhere-else` — Stop 2 — And the compile stops somewhere else
  - `stop-3-which-kind-of-name-was-missing` — Stop 3 — Which *kind* of name was missing
  - `stop-4-read-it-in-the-log` — Stop 4 — Read it in the log
  - `what-to-bring-back` — What to bring back

## `failure-structural`

**Failure tour — Structural analysis, where counting stops being enough**

Specimens: `TwiceDefined` and `CapacitorLoop`. Both are flagged `singular`. They are not

- **Specimens:** `TwiceDefined`, `CapacitorLoop`
- **Stages:** `Dae`, `Structural`
- **Stops:**
  - `failure-tour-structural-analysis-where-counting-stops-being-enough` — Failure tour — Structural analysis, where counting stops being enough
  - `stop-1-square-and-singular-anyway` — Stop 1 — Square, and singular anyway
  - `stop-2-what-matching-finds` — Stop 2 — What matching finds
  - `stop-3-the-same-flag-a-different-cause` — Stop 3 — The same flag, a different cause
  - `stop-4-what-no-blocks-means` — Stop 4 — What "no blocks" means
  - `what-to-bring-back` — What to bring back

## `failure-typecheck`

**Failure tour — Typecheck, which reports and does not stop at all**

Specimen: `DimensionMismatch` — a 2-vector assigned from a 3-vector.

- **Specimens:** `DimensionMismatch`, `UnclosedModel`
- **Stages:** `Flatten`, `SolveLowering`, `Typecheck`
- **Stops:**
  - `failure-tour-typecheck-which-reports-and-does-not-stop-at-all` — Failure tour — Typecheck, which reports and does not stop at all
  - `stop-1-the-diagnosis` — Stop 1 — The diagnosis
  - `stop-2-the-surprise` — Stop 2 — The surprise
  - `stop-3-where-the-truth-is-kept` — Stop 3 — Where the truth is kept
  - `stop-4-compare-with-a-stop` — Stop 4 — Compare with a stop
  - `what-to-bring-back` — What to bring back

## `frame-seeking`

**Fixture tour — seeking to a frame**

This is a test, not an explanation. It verifies that a link can stop an animation on

- **Specimens:** `MotorWithBrake`
- **Stages:** `Structural`
- **Stops:**
  - `fixture-tour-seeking-to-a-frame` — Fixture tour — seeking to a frame
  - `stop-0-a-stop-clicked-out-of-order` — Stop 0 — A stop clicked out of order
  - `stop-1-a-replay-unstarted` — Stop 1 — A replay, unstarted
  - `stop-2-jump-into-the-middle` — Stop 2 — Jump into the middle
  - `stop-3-jump-backwards` — Stop 3 — Jump backwards
  - `stop-4-seek-past-the-end` — Stop 4 — Seek past the end
  - `stop-5-seek-in-a-different-animation` — Stop 5 — Seek in a different animation
  - `stop-6-seek-a-view-that-has-no-animation` — Stop 6 — Seek a view that has no animation
  - `what-this-cannot-check` — What this cannot check

## `index-reduction`

**Fixture tour — Index reduction: more states than freedoms**

A curriculum tour. Walk [`blt-ordering.md`](blt-ordering.md) and

- **Specimens:** `BouncingBall`, `Drivetrain`
- **Stages:** `IndexReduction`, `Structural`
- **Stops:**
  - `fixture-tour-index-reduction-more-states-than-freedoms` — Fixture tour — Index reduction: more states than freedoms
  - `the-problem-this-phase-exists-to-solve` — The problem this phase exists to solve
  - `act-1-the-case-that-needs-nothing` — Act 1 — The case that needs nothing
  - `act-2-the-case-that-does` — Act 2 — The case that does
  - `act-3-what-actually-happened-to-the-surplus` — Act 3 — What actually happened to the surplus
  - `act-4-what-index-counts-and-why-1-is-the-target` — Act 4 — What "index" counts, and why 1 is the target
  - `what-this-tour-cannot-check` — What this tour cannot check
  - `a-number-downstream-disagrees-with-this-tour-and-it-is-a-real-open-question` — A number downstream disagrees with this tour, and it is a real open question
  - `what-comes-next-in-the-chain` — What comes next in the chain

## `initialization`

**Fixture tour — Initialization: the values at t = 0**

A curriculum tour. Walk [`index-reduction.md`](index-reduction.md) first — this phase runs on

- **Specimens:** `BouncingBall`, `RcCircuit`, `OverInitRc`, `RotationalInertia`
- **Stages:** `Initialization`
- **Stops:**
  - `fixture-tour-initialization-the-values-at-t-0` — Fixture tour — Initialization: the values at t = 0
  - `the-problem-this-phase-exists-to-solve` — The problem this phase exists to solve
  - `act-1-the-case-with-nothing-to-solve` — Act 1 — The case with nothing to solve
  - `act-2-the-case-with-a-real-initialization-system` — Act 2 — The case with a real initialization system
  - `act-3-the-case-that-specifies-too-much` — Act 3 — The case that specifies too much
  - `act-4-square-and-still-singular-again` — Act 4 — Square and still singular, again
  - `what-this-tour-cannot-check` — What this tour cannot check
  - `what-comes-next-in-the-chain` — What comes next in the chain

## `matching-live`

**Fixture tour — Matching, live: the call stack is the augmenting path**

A pass-two tour. [`matching.md`](matching.md) taught the idea; this one is about Rumoca's

- **Specimens:** `ProportionalLoop`, `TwiceDefined`
- **Stages:** `Structural`
- **Stops:**
  - `fixture-tour-matching-live-the-call-stack-is-the-augmenting-path` — Fixture tour — Matching, live: the call stack is the augmenting path
  - `scene-0-two-things-must-be-true-before-any-of-this-works` — Scene 0 — Two things must be true before any of this works
  - `act-1-arm-an-anchor-and-learn-what-a-stop-is-named` — Act 1 — Arm an anchor, and learn what a stop is named
  - `act-2-the-call-stack-is-the-augmenting-path` — Act 2 — The call stack is the augmenting path
  - `act-3-the-same-machinery-refusing` — Act 3 — The same machinery, refusing
  - `act-4-what-this-instrument-can-and-cannot-show-you` — Act 4 — What this instrument can and cannot show you
  - `what-this-tour-cannot-check` — What this tour cannot check
  - `what-comes-next` — What comes next

## `matching`

**Fixture tour — Matching: which equation solves which unknown**

A curriculum tour. It teaches a step of the chain and uses HRW as the instrument. It is still

- **Specimens:** `BouncingBall`, `ProportionalLoop`, `CapacitorLoop`
- **Stages:** `Structural`
- **Stops:**
  - `fixture-tour-matching-which-equation-solves-which-unknown` — Fixture tour — Matching: which equation solves which unknown
  - `the-problem-this-phase-exists-to-solve` — The problem this phase exists to solve
  - `act-1-the-case-where-it-is-obvious` — Act 1 — The case where it is obvious
  - `act-2-the-case-that-is-not-obvious-at-all` — Act 2 — The case that is not obvious at all
  - `act-3-the-case-with-no-answer` — Act 3 — The case with no answer
  - `act-4-what-this-is-called-and-why-the-name-helps` — Act 4 — What this is called, and why the name helps
  - `what-this-tour-cannot-check` — What this tour cannot check
  - `what-comes-next-in-the-chain` — What comes next in the chain

## `node-pointing`

**Fixture tour — pointing at a tree node, and following**

This is a test, not an explanation. It verifies the last two verbs of the answer

- **Specimens:** `RcCircuit`
- **Stages:** `Structural`
- **Stops:**
  - `fixture-tour-pointing-at-a-tree-node-and-following` — Fixture tour — pointing at a tree node, and following
  - `stop-1-open-the-tree` — Stop 1 — Open the tree
  - `stop-2-point-at-a-shallow-node` — Stop 2 — Point at a shallow node
  - `stop-3-point-at-something-nested` — Stop 3 — Point at something nested
  - `stop-4-point-somewhere-deeper-still` — Stop 4 — Point somewhere deeper still
  - `stop-5-a-path-that-is-not-there` — Stop 5 — A path that is not there
  - `stop-5b-a-view-this-model-does-not-have` — Stop 5b — A view this model does not have
  - `stop-6-follow-an-identifier` — Stop 6 — Follow an identifier
  - `stop-7-follow-then-point-and-see-both` — Stop 7 — Follow, then point, and see both
  - `what-this-cannot-check` — What this cannot check

## `solve-lowering`

**Fixture tour — Solve lowering: names become indices**

A curriculum tour. The last phase before simulation. Walk [`events.md`](events.md) first.

- **Specimens:** `BouncingBall`, `RcCircuit`
- **Stages:** `SolveLowering`
- **Stops:**
  - `fixture-tour-solve-lowering-names-become-indices` — Fixture tour — Solve lowering: names become indices
  - `the-problem-this-phase-exists-to-solve` — The problem this phase exists to solve
  - `act-1-where-your-variables-went` — Act 1 — Where your variables went
  - `act-2-what-else-is-in-the-arrays` — Act 2 — What else is in the arrays
  - `act-3-the-same-mapping-at-scale` — Act 3 — The same mapping at scale
  - `what-this-tour-cannot-check` — What this tour cannot check
  - `one-count-here-contradicts-the-index-reduction-tour` — One count here contradicts the index-reduction tour
  - `what-comes-next-in-the-chain` — What comes next in the chain

## `structural-vs-numerical-rank`

**Structural rank vs numerical rank**

The first cross-platform tour. Two stops in HRW, then a notebook — because the point

- **Specimens:** `ProportionalLoop`
- **Stages:** `Structural`
- **Stops:**
  - `structural-rank-vs-numerical-rank` — Structural rank vs numerical rank
  - `stop-1-the-pattern-from-a-real-model` — 📐 Stop 1 — The pattern, from a real model
  - `stop-2-what-hrw-concludes-from-it` — 📐 Stop 2 — What HRW concludes from it
  - `stop-3-where-the-values-go-in` — 🧮 Stop 3 — Where the values go in
  - `stop-4-back-to-hrw-and-what-it-would-say` — 📐 Stop 4 — Back to HRW, and what it would say
  - `what-each-side-uniquely-holds` — What each side uniquely holds
  - `what-this-cannot-check` — What this cannot check

## `tearing`

**Fixture tour — Tearing: guess one number, get the rest for free**

A curriculum tour. Walk [`blt-ordering.md`](blt-ordering.md) first — it produces the coupled

- **Specimens:** `ProportionalLoop`, `TwoLoops`, `MixedLoop`, `LoopWithInertia`
- **Stages:** `Structural`
- **Stops:**
  - `fixture-tour-tearing-guess-one-number-get-the-rest-for-free` — Fixture tour — Tearing: guess one number, get the rest for free
  - `the-problem-this-phase-exists-to-solve` — The problem this phase exists to solve
  - `act-1-guess-one-number-and-the-rest-falls-out` — Act 1 — Guess one number and the rest falls out
  - `act-2-watch-the-choice-being-made` — Act 2 — Watch the choice being made
  - `act-3-two-blocks-torn-independently` — Act 3 — Two blocks, torn independently
  - `act-4-all-three-kinds-of-block-in-one-model` — Act 4 — All three kinds of block in one model
  - `act-5-what-it-costs-once-time-is-moving` — Act 5 — What it costs once time is moving
  - `what-this-tour-cannot-check` — What this tour cannot check
  - `what-comes-next-in-the-chain` — What comes next in the chain

## `the-mathematics`

**The mathematics — a week's walk through the pipeline**

Start here. This tour walks nothing itself; it is the map for the nine that do, in the order

- **Stops:**
  - `the-mathematics-a-week-s-walk-through-the-pipeline` — The mathematics — a week's walk through the pipeline
  - `the-route` — The route
  - `the-four-numbers-that-connect-the-tours` — The four numbers that connect the tours
  - `the-one-structural-idea-the-whole-pipeline-turns-on` — The one structural idea the whole pipeline turns on
  - `three-graphs-three-classical-questions` — Three graphs, three classical questions
  - `three-things-worth-knowing-before-you-start` — Three things worth knowing before you start
  - `two-open-questions-you-may-hit` — Two open questions you may hit
  - `what-to-tell-me-afterwards` — What to tell me afterwards

## `the-oracle`

**The oracle — when Rumoca and System Modeler disagree**

A tour that leaves HRW to settle a question HRW cannot settle. Rumoca accepts a model

- **Specimens:** `IncompatibleConnect`
- **Stages:** `Flatten`
- **Stops:**
  - `the-oracle-when-rumoca-and-system-modeler-disagree` — The oracle — when Rumoca and System Modeler disagree
  - `stop-1-a-specimen-built-to-fail-at-flatten` — 📐 Stop 1 — A specimen built to fail at flatten
  - `stop-2-where-it-actually-fails` — 📐 Stop 2 — Where it actually fails
  - `stop-3-ask-the-other-implementation` — ⚙ Stop 3 — Ask the other implementation
  - `stop-4-why-this-could-not-be-settled-inside-hrw` — 📐 Stop 4 — Why this could not be settled inside HRW
  - `the-rule-this-tour-exists-to-make-concrete` — The rule this tour exists to make concrete
  - `what-this-cannot-check` — What this cannot check

