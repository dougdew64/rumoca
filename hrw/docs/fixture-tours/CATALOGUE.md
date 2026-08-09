# Tour catalogue

**Generated — do not edit.** `cargo run -p hrw --example gen_tour_catalogue`.

**Audience: Claude.** This exists so a question can be answered by citing a tour that already demonstrates the thing, rather than by writing a new one that retells it without its checked expectations (`docs/ideas.md` #63). Cite a stop with `hrw://tour/<name>/stop/<slug>`.

**Re-read a tour before citing it.** Everything below is derived and current; whether a tour's *claims* still hold is not something a catalogue can know, and a tour promised a tree the pane did not show for its whole existence.

## `blt-ordering`

**BLT — finding an order, and finding out there isn't one**

Walk [`matching.md`](matching.md) first. Matching answered *which* equation solves *which*

- **Specimens:** `RcCircuit`, `ProportionalLoop`, `TwoLoops`
- **Stages:** `Structural`
- **Stops:**
  - `blt-finding-an-order-and-finding-out-there-isn-t-one` — BLT — finding an order, and finding out there isn't one
  - `the-problem-this-step-exists-to-solve` — The problem this step exists to solve
  - `act-1-when-an-order-exists` — Act 1 — When an order exists
  - `act-2-when-no-order-exists` — Act 2 — When no order exists
  - `act-3-when-the-system-splits` — Act 3 — When the system splits
  - `act-4-what-you-have-been-building-is-a-block-triangular-form` — Act 4 — What you have been building is a block triangular form
  - `act-5-how-rumoca-spells-it` — Act 5 — How Rumoca spells it
  - `what-comes-next-in-the-chain` — What comes next in the chain
  - `what-this-tour-cannot-check` — What this tour cannot check

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

## `dae-construction`

**Fixture tour — DAE construction: the count that decides everything**

A curriculum tour. Most tours here verify an HRW capability; this one teaches a step of the

- **Specimens:** `SingleInertia`, `UnbalancedShaft`
- **Stages:** `Dae`, `Structural`
- **Stops:**
  - `fixture-tour-dae-construction-the-count-that-decides-everything` — Fixture tour — DAE construction: the count that decides everything
  - `the-problem-this-phase-exists-to-solve` — The problem this phase exists to solve
  - `stop-1-the-dae-itself` — Stop 1 — The DAE itself
  - `stop-2-what-the-solver-is-actually-solving-for` — Stop 2 — What the solver is actually solving for
  - `stop-3-the-equations-and-the-claim` — Stop 3 — The equations, and the claim
  - `stop-4-the-counterexample-and-what-the-compiler-says` — Stop 4 — The counterexample, and what the compiler says
  - `stop-5-why-2-equations-3-unknowns-is-not-no-solution` — Stop 5 — Why "2 equations, 3 unknowns" is not "no solution"
  - `stop-6-what-state-looks-like-when-it-runs` — Stop 6 — What "state" looks like when it runs
  - `stop-7-what-the-dae-does-not-tell-you` — Stop 7 — What the DAE does not tell you
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

**Index reduction — when nine states are really three**

Walk [`matching.md`](matching.md) first, and Act 3 of it especially. That act showed a

- **Specimens:** `BouncingBall`, `BenchActuator`, `Drivetrain`
- **Stages:** `IndexReduction`, `Structural`
- **Stops:**
  - `index-reduction-when-nine-states-are-really-three` — Index reduction — when nine states are really three
  - `the-problem-this-phase-exists-to-solve` — The problem this phase exists to solve
  - `act-1-a-model-that-needs-nothing` — Act 1 — A model that needs nothing
  - `act-2-one-state-that-was-not-a-state` — Act 2 — One state that was not a state
  - `act-3-nine-states-three-degrees-of-freedom` — Act 3 — Nine states, three degrees of freedom
  - `act-4-why-the-previous-phase-failed-and-why-that-was-correct` — Act 4 — Why the previous phase failed, and why that was correct
  - `act-5-what-rumoca-actually-does-which-is-not-what-the-textbook-name-suggests` — Act 5 — What Rumoca actually does, which is not what the textbook name suggests
  - `act-6-the-linear-algebra-in-one-paragraph` — Act 6 — The linear algebra, in one paragraph
  - `what-comes-next-in-the-chain` — What comes next in the chain
  - `what-this-tour-cannot-check` — What this tour cannot check

## `initialization`

**Initialization — the equations that only run once**

This tour is a pair one line apart. `RcCircuit` initializes cleanly. `OverInitRc` is the same

- **Specimens:** `BouncingBall`, `RcCircuit`, `OverInitRc`
- **Stages:** `Initialization`
- **Stops:**
  - `initialization-the-equations-that-only-run-once` — Initialization — the equations that only run once
  - `the-problem-this-phase-exists-to-solve` — The problem this phase exists to solve
  - `act-1-nothing-specified-and-that-is-fine` — Act 1 — Nothing specified, and that is fine
  - `act-2-a-real-circuit-initialized-from-one-number` — Act 2 — A real circuit, initialized from one number
  - `act-3-the-same-circuit-over-determined` — Act 3 — The same circuit, over-determined
  - `act-4-why-those-two-lines-fight` — Act 4 — Why those two lines fight
  - `act-5-the-relaxation-hint` — Act 5 — The relaxation hint
  - `act-6-the-arithmetic-stated-once` — Act 6 — The arithmetic, stated once
  - `what-comes-next-in-the-chain` — What comes next in the chain
  - `what-this-tour-cannot-check` — What this tour cannot check

## `matching-live`

**Matching, live — standing inside the search**

Walk [`matching.md`](matching.md) first. That tour shows the algorithm running, what it

- **Specimens:** `ProportionalLoop`, `TwiceDefined`
- **Stages:** `Structural`
- **Stops:**
  - `matching-live-standing-inside-the-search` — Matching, live — standing inside the search
  - `scene-0-two-things-must-be-true-before-any-of-this-works` — Scene 0 — Two things must be true before any of this works
  - `scene-1-arm-it-and-learn-to-name-a-stop` — Scene 1 — Arm it, and learn to name a stop
  - `scene-2-the-call-stack-is-the-augmenting-path` — Scene 2 — The call stack *is* the augmenting path
  - `scene-3-the-same-machinery-refusing` — Scene 3 — The same machinery, refusing
  - `scene-4-what-the-two-runs-say-together` — Scene 4 — What the two runs say together
  - `scene-5-what-this-instrument-can-and-cannot-show-you` — Scene 5 — What this instrument can and cannot show you
  - `what-this-tour-cannot-check` — What this tour cannot check

## `matching`

**Fixture tour — Matching: when greed is not enough**

A curriculum tour, and the second in the chain. `dae-construction.md` ended with DAE

- **Specimens:** `BouncingBall`, `ProportionalLoop`, `CapacitorLoop`
- **Stages:** `Structural`
- **Stops:**
  - `fixture-tour-matching-when-greed-is-not-enough` — Fixture tour — Matching: when greed is not enough
  - `the-problem-this-phase-exists-to-solve` — The problem this phase exists to solve
  - `act-1-when-greed-works` — Act 1 — When greed works
  - `act-2-when-greed-fails-and-the-algorithm-backs-up` — Act 2 — When greed fails, and the algorithm backs up
  - `act-3-when-no-augmenting-path-exists` — Act 3 — When no augmenting path exists
  - `act-4-the-thing-you-have-been-building-is-a-permutation` — Act 4 — The thing you have been building is a permutation
  - `act-5-how-rumoca-spells-it` — Act 5 — How Rumoca spells it
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

**Solve lowering — where names become numbers**

This is the last compilation phase, and the one where the model stops being a model.

- **Specimens:** `BouncingBall`, `ProportionalLoop`, `RcCircuit`
- **Stages:** `SolveLowering`
- **Stops:**
  - `solve-lowering-where-names-become-numbers` — Solve lowering — where names become numbers
  - `the-problem-this-phase-exists-to-solve` — The problem this phase exists to solve
  - `act-1-two-states-five-parameters-and-a-starting-vector` — Act 1 — Two states, five parameters, and a starting vector
  - `act-2-a-model-with-no-dynamics-at-all` — Act 2 — A model with no dynamics at all
  - `act-3-one-state-carrying-twenty-two-algebraic-variables` — Act 3 — One state carrying twenty-two algebraic variables
  - `act-4-three-problems-not-one` — Act 4 — Three problems, not one
  - `act-5-why-the-layout-is-frozen-at-compile-time` — Act 5 — Why the layout is frozen at compile time
  - `what-comes-next-in-the-chain` — What comes next in the chain
  - `what-this-tour-cannot-check` — What this tour cannot check

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

**Tearing — turning a 3×3 solve into a 1×1 one**

Walk [`blt-ordering.md`](blt-ordering.md) first. It ended with a coupled block of three

- **Specimens:** `ProportionalLoop`, `TwoLoops`, `MixedLoop`
- **Stages:** `Structural`
- **Stops:**
  - `tearing-turning-a-3-3-solve-into-a-1-1-one` — Tearing — turning a 3×3 solve into a 1×1 one
  - `the-problem-this-step-exists-to-solve` — The problem this step exists to solve
  - `act-1-guess-one-number-and-the-rest-falls-out` — Act 1 — Guess one number and the rest falls out
  - `act-2-watch-the-choice-being-made` — Act 2 — Watch the choice being made
  - `act-3-two-blocks-torn-independently` — Act 3 — Two blocks, torn independently
  - `act-4-all-three-kinds-of-block-in-one-model` — Act 4 — All three kinds of block in one model
  - `act-5-the-linear-algebra-this-is-a-schur-complement` — Act 5 — The linear algebra: this is a Schur complement
  - `act-6-greedy-and-what-greedy-costs` — Act 6 — Greedy, and what greedy costs
  - `what-comes-next-in-the-chain` — What comes next in the chain
  - `what-this-tour-cannot-check` — What this tour cannot check

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

