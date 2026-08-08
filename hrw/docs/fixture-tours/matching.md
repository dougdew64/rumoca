# Fixture tour — Matching: when greed is not enough

**A curriculum tour**, and the second in the chain. `dae-construction.md` ended with DAE
construction promising a **square system** and me saying that counting is a *necessary*
condition, not a sufficient one. This is the phase that consumes the promise — and the third
act is where square turns out not to be enough.

**This is the first tour built on animations.** Its stops do not point at a pane; they point at
a **step of an algorithm**, paused. That is the only way to see the thing worth seeing here,
because what makes matching interesting is not its answer but the moment it **changes its
mind**.

Every frame number below came from `cargo run -p hrw --example frame_index -- <Model>`, read
from the committed traces. **Frame numbers in that tool's output are 0-based and the links are
1-based** — it prints the ready-made link precisely so nobody does that arithmetic by hand,
after it spent a day telling authors the wrong number.

**The animation shows its step in words**, above the matrix. Every `**Expected:**` line below
quotes that text, so each is checkable without interpreting a picture.

---

## The problem this phase exists to solve

You have a square system: *n* equations, *n* unknowns. You could hand the whole thing to a
nonlinear solver and ask it to find all *n* values at once. For a real model that is a terrible
idea — a 4,000-equation Newton iteration is slow, fragile, and tells you nothing when it fails.

Almost always, you do not have to. Most equations determine **one** unknown given values you
already have, so the system can be solved in an **order**: this equation gives that variable,
which lets the next equation give another, and so on. Only the genuinely circular parts need to
be solved simultaneously, and those parts are usually small.

**Finding that order starts with deciding which equation is responsible for which unknown.**
That is *matching*: pair each equation with exactly one unknown, using each unknown exactly
once. It is a **maximum bipartite matching** — equations on one side, unknowns on the other, an
edge wherever an equation mentions an unknown — and the standard algorithm is an
augmenting-path search.

Two things make this worth watching rather than reading about.

**First, a greedy pass is not enough**, and the algorithm's recovery is the interesting part.
**Second, matching does not respect how you wrote your equations** — and Act 2 makes that
concrete in a way no explanation has.

---

## Act 1 — When greed works

[BouncingBall → Structural → Matching animation](hrw://load/BouncingBall/Structural/MatchingAnim)

`BouncingBall` has two equations and two unknowns:

```modelica
der(h) = v;      // f_x[0]
der(v) = -g;     // f_x[1]
```

The unknowns are `der(h)` and `der(v)` — the derivatives, as always at this point in the chain.
Eight frames, and no surprises in any of them. That is the point of starting here.

[Frame 1 — start the search](hrw://stage/Structural/MatchingAnim/frame/1)

**Expected:** `Starting augmenting-path search for equation 0: der(h) - v`.

Even the easy case is *called* an augmenting-path search. The algorithm has one procedure and
runs it every time; the easy case is just the procedure terminating immediately.

[Frame 3 — a free variable](hrw://stage/Structural/MatchingAnim/frame/3)

**Expected:** `Variable 0 (der(h)) is free — augmenting path found for eq 0`.

**"Free" means no equation has claimed it yet.** `f_x[0]` mentions `der(h)`, nobody else holds
it, so the search ends on its first step. An augmenting path of length one.

[Frame 4 — the assignment](hrw://stage/Structural/MatchingAnim/frame/4)

**Expected:** `Matched: equation 0 (der(h) - v) ↔ variable 0 (der(h))`.

[Frame 8 — and again for the second equation](hrw://stage/Structural/MatchingAnim/frame/8)

**Expected:** `Matched: equation 1 (der(v) - -g) ↔ variable 1 (der(v))`.

Four frames per equation — try, explore, found free, assign — twice, and done. **This is what
greedy success looks like**: every equation finds an unclaimed unknown on its first look, and
nothing is ever revisited.

If every model looked like this, matching would be a loop with no cleverness in it. The next
act is why it is not.

---

## Act 2 — When greed fails, and the algorithm backs up

[ProportionalLoop → Structural → Matching animation](hrw://load/ProportionalLoop/Structural/MatchingAnim)

`ProportionalLoop` is an idealised servo loop with no integrator anywhere — every relation is
instantaneous, so the feedback closes on itself:

```modelica
error       = reference - measurement;      // f_x[0]
command     = controllerGain * error;       // f_x[1]
measurement = plantGain * command;          // f_x[2]
```

Three equations, three unknowns (`error`, `command`, `measurement`), and **sixteen frames**
instead of twelve. The extra four are the whole lesson.

[Frame 4 — the greedy start](hrw://stage/Structural/MatchingAnim/frame/4)

**Expected:** `Matched: equation 0 (error - (reference - measurement)) ↔ variable 0 (error)`.

`f_x[0]` mentions both `error` and `measurement`. It looks at `error` first, finds it free, and
takes it. **A perfectly reasonable choice that is about to turn out wrong.**

[Frame 6 — and now the collision](hrw://stage/Structural/MatchingAnim/frame/6)

**Expected:** `Equation 1 (command - controllerGain * error) exploring variable 0 (error)`.

`f_x[1]` is `command = controllerGain * error`. It mentions `error` and `command`; it tries
`error` first, and `error` is taken. **A greedy algorithm is now stuck.** Every option `f_x[1]`
has is either claimed or not mentioned, and the naive response — give up and declare the system
singular — would be *wrong*, because a perfect matching does exist.

[**Frame 7 — the moment worth pausing on**](hrw://stage/Structural/MatchingAnim/frame/7)

**Expected:** `Variable 0 (error) held by eq 0 (error - (reference - measurement)). Can eq 0 find an
alternative?`

**This is the augmenting path, and this question is the entire algorithm.**

Rather than give up, the search asks the *current holder* to move. Not "is `error` free?" but
"is `error` **freeable**?" — which is a different and much better question, because it is
recursive. `f_x[0]` is now asked to redo its own search, under the constraint that it may not
use `error`.

The name makes sense from here. There is a path — `f_x[1] → error → f_x[0] → ?` — alternating
between edges *not* in the matching and edges that *are*. If it ends at a free variable, you
flip every edge along it, and the matching grows by exactly one. **Augmenting** means growing
the matching by one, and the path is the route by which it grows.

[Frame 9 — the recursive call succeeds](hrw://stage/Structural/MatchingAnim/frame/9)

**Expected:** `Variable 2 (measurement) is free — augmenting path found for eq 0`.

`f_x[0]` had a second option all along. `error = reference - measurement` mentions
`measurement`, nothing holds it, and so the path terminates. Length three: `f_x[1] → error →
f_x[0] → measurement`.

[Frame 10 — the holder re-homes](hrw://stage/Structural/MatchingAnim/frame/10)

**Expected:** `Matched: equation 0 (error - (reference - measurement)) ↔ variable 2 (measurement)`.

[Frame 11 — which frees what was wanted](hrw://stage/Structural/MatchingAnim/frame/11)

**Expected:** `Displacement succeeded — eq 1 can take variable 0 (error)`.

[Frame 12 — and the original request completes](hrw://stage/Structural/MatchingAnim/frame/12)

**Expected:** `Matched: equation 1 (command - controllerGain * error) ↔ variable 0 (error)`.

Read frames 7 through 12 as a **call and its return**: 7 asks the question, 9–10 are the
recursive call finding an alternative, 11–12 are the return unwinding it. On a bigger model
that recursion nests — `RcCircuit` has 78 displacement steps in 233 frames — and it is the
reason matching is not simply a loop.

### The result, which is stranger than the process

[Point at the finished matching](hrw://stage/Structural/Tree/node/matching)

**Expected:** three pairs —

| Equation, as you wrote it | is used to solve for |
|---|---|
| `error = reference - measurement` | **measurement** |
| `command = controllerGain * error` | **error** |
| `measurement = plantGain * command` | **command** |

**Not one equation is matched to the variable on its left-hand side.**

This is the part worth sitting with. Every Modelica introduction says that `=` is an
**equation, not an assignment** — that the language is *acausal*, that you state relationships
and the compiler decides direction. It is easy to nod at and hard to actually believe, because
every line you write still *looks* like an assignment.

Here is the compiler doing it. You wrote `command = controllerGain * error` as though it
defines `command`. Matching decided that equation determines **`error`** — it will be evaluated
as `error := command / controllerGain`. The causality you appeared to specify was never
binding; **matching is where the real direction is chosen**, and it chose differently for all
three.

That is not a quirk of this model. It is what acausal modelling *means*, and this is the phase
where the word stops being an adjective and becomes an algorithm.

### The linear-algebra reading

Build a matrix with one row per equation and one column per unknown, putting a mark wherever
the equation mentions the unknown. **A matching is a choice of one mark per row and per
column** — equivalently, a permutation of the columns that puts a mark everywhere on the
diagonal.

A perfect matching exists exactly when that matrix has full **structural rank**: the rank it
would have for generic nonzero values, determined by the *pattern* alone. Matching computes
structural rank, and the augmenting-path search is how.

**This is the same rank thread `dae-construction.md` opened.** There, `balance = -(nullity)`
counted a deficiency in the *shape* of the system. Here the shape is square and the deficiency,
if any, is in its **connectivity**. Act 3 is a system that passes the first test and fails this
one.

---

## Act 3 — When no augmenting path exists

[CapacitorLoop → Structural → Matching animation](hrw://load/CapacitorLoop/Structural/MatchingAnim)

```modelica
Modelica.Electrical.Analog.Sources.ConstantVoltage src(V = 5);
Modelica.Electrical.Analog.Basic.Capacitor C(C = 1e-3);
Modelica.Electrical.Analog.Basic.Ground gnd;
equation
  connect(src.p, C.p);
  connect(src.n, C.n);
  connect(src.n, gnd.p);
```

A capacitor wired directly across an ideal voltage source. **Fourteen equations, fourteen
unknowns** — it passes DAE construction's balance check without complaint, which is exactly why
it is the right counterexample. Everything the previous tour could check is fine.

114 frames, with **34 displacement steps**. The algorithm works hard here, and that effort is
itself informative: it does not fail quickly, it fails *exhaustively*.

[Frame 111 — one more attempt](hrw://stage/Structural/MatchingAnim/frame/111)

**Expected:** `Equation 13 (C.n.v - gnd.p.v) exploring variable 12 (gnd.p.v)`.

[Frame 112 — ask the holder to move, as in Act 2](hrw://stage/Structural/MatchingAnim/frame/112)

**Expected:** `Variable 12 (gnd.p.v) held by eq 8 (gnd.p.v - 0). Can eq 8 find an alternative?`

Exactly the question from frame 7 of Act 2 — the same procedure, applied for the 38th time.

[Frame 113 — but this time the answer is no](hrw://stage/Structural/MatchingAnim/frame/113)

**Expected:** `Displacement failed — variable 12 (gnd.p.v) cannot be freed for eq 13`.

[**Frame 114 — the algorithm gives up**](hrw://stage/Structural/MatchingAnim/frame/114)

**Expected:** `Equation 13 (C.n.v - gnd.p.v) has no augmenting path — unmatched (rank
deficiency)`.

**"No augmenting path" is a much stronger statement than "I could not find one."** The search
is exhaustive: it explored every alternating path from `f_x[13]`, and every one of them dead-ended
on a variable whose holder had nowhere else to go. When that search completes without reaching
a free variable, **no perfect matching exists** — not "not by this route", but at all. That is a
theorem (Berge's), and it is why the algorithm is entitled to stop rather than keep trying.

[Point at the failure](hrw://stage/Structural/Tree/node/error)

**Expected:** `n_matched` is **13** of 14, `rank_deficiency` is **1**, the unmatched equation is
`f_x[13]`, and the unmatched unknown is **`gnd.p.i`** — the current through the ground pin.

Two things to take from that.

**The diagnosis is specific.** It does not say "this model is broken". It names the one equation
and the one variable that could not be paired, which is what makes the message actionable.
`gnd.p.i` is the ground's current, and nothing in the model determines it — the capacitor
voltage is fixed by the source, so the current that would charge it is unconstrained.

**And `rank_deficiency: 1` is the number from the last tour, in its second form.** There,
`balance = -1` meant one unknown too many. Here the counts are equal and the *rank* falls one
short. Same deficiency of one, found by a different test — because counting and connectivity are
different questions and a system can pass either while failing the other.

[Show where the model says it](hrw://source/9)

**Expected:** line 9, `connect(src.n, gnd.p);`.

The physical reading: a capacitor across an ideal voltage source is over-constrained. The
source insists on 5 V; the capacitor's voltage is a *state*, which wants its own initial value
and its own dynamics. Both cannot hold. **The model is not a typo — it is a bad idealisation**,
and this phase is where an idealisation that cannot be simulated is caught.

---

## Act 4 — The thing you have been building is a permutation

Everything above described matching as a *search*: try an equation, explore, back up, assign.
That is how it runs. **It is not what it produces.**

[ProportionalLoop → Structural → Incidence](hrw://load/ProportionalLoop/Structural/Incidence)

**Expected:** the incidence matrix, with the matched cells marked, and the caption reading
`3/3 matched (full rank)`.

Now read the marks rather than the search. **Exactly one per row. Exactly one per column.**

That is a **permutation matrix** — the 0/1 matrix `P` with a single 1 in each row and column.
Matching does not merely pair things off; it constructs `P`, and everything downstream is what
`P` buys.

### Why a compiler wants one

The incidence matrix `A` says which unknowns each equation *mentions*. It is a sparsity pattern:
1 where equation *i* involves unknown *j*, 0 otherwise. Nothing in `A` says which unknown an
equation should be **solved for**.

Applying the permutation — reordering the columns so each equation's matched unknown lands on the
diagonal — gives a matrix whose **diagonal is entirely non-zero**. That is the precondition for
everything after:

- **Tarjan** (Act 3's sequel) finds strongly connected components in the *permuted* matrix, and
  the blocks it returns are the **block triangular form**. Without a full diagonal there are no
  blocks to find.
- **Solving** a scalar block means "solve equation *i* for unknown *j*" — and `P` is what says
  which *j*.

**So the search you watched is a constructive proof that `P` exists.** An augmenting path is the
step that fixes a partial permutation into a larger one.

### And rank deficiency is the permutation failing to exist

Return to `CapacitorLoop` from Act 3.

[CapacitorLoop → Structural → Incidence](hrw://load/CapacitorLoop/Structural/Incidence)

**Expected:** the caption reports **fewer matched than the system's size**, and names a rank
deficiency.

That is the same statement in two vocabularies. *"No augmenting path exists"* and *"no permutation
matrix exists"* are the same fact, and **`structural rank` is the size of the largest partial
permutation `A`'s sparsity pattern admits.**

**Structural rank is an upper bound on numerical rank, never the other way round.** A pattern can
admit a permutation while the numbers still make the system singular — cancellation the sparsity
cannot see. `structural-vs-numerical-rank.md` is the tour for that distinction; this stop only
establishes that the two are different questions.

### If you are reading this alongside a linear algebra course

The correspondences worth carrying, each visible on the pane above rather than asserted here:

| In class | On this screen |
|---|---|
| sparsity pattern | the incidence matrix |
| permutation matrix `P` | the matched cells, one per row and column |
| `PA` with non-zero diagonal | the reordering matching makes possible |
| block triangular form | the BLT blocks Tarjan finds in the permuted matrix |
| rank of a pattern | `structural rank`, and the deficiency `CapacitorLoop` reports |

**Ask about any row of that table.** The mathematics is stated here as reasoning you can check
against the pane, not as a fact retrieved from a file — see `docs/ideas.md` #67 for why that
distinction is a rule rather than a preference.

## Act 5 — How Rumoca spells it

Acts 1-3 showed the algorithm **running**. Act 4 showed what it **builds**. The third question
is the one this tour has never answered: **what does the code look like, and where is it?**

Everything below names things you can go and read. **Nothing here transcribes code** — quoted
source is the most rot-prone thing a tour can carry, and nothing compiles a tour.

### Two functions, one file

Both live in `crates/rumoca-phase-structural/src/matching.rs`.

- **`maximum_matching_with_trace`** — the outer loop. One attempt per equation, in index order.
  Every *"Starting augmenting-path search for equation i"* you read in Act 1 is one turn of it.
- **`augment_traced`** — the search for a single equation, and **it calls itself**. The entirety
  of Act 2 — the exploring, the displacement, the backing up — happens inside one invocation of
  this function and its recursive descendants.

**Two arrays are the whole state:** `match_eq` and `match_var`, each an `Option<usize>` per
entry, pointing at each other. **`match_eq` *is* Act 4's permutation** — one entry per row, each
holding the column it was matched to. The permutation matrix is not built at the end; it is
this array, filled in as the search succeeds.

### The log lines are enum variants

The narration you have been reading all tour is not prose HRW invents. Each line is one
`MatchingStep`, declared beside the algorithm and rendered in `hrw/src/matching_anim.rs`:

| The line you read | `MatchingStep` | Emitted when |
|---|---|---|
| `Starting augmenting-path search for equation i` | `TryEquation` | the outer loop begins an equation |
| `Equation i exploring variable j` | `Explore` | the search reaches a variable it has not visited |
| `Variable j is free — augmenting path found` | `FoundFree` | that variable has no holder |
| `Variable j held by eq h. Can eq h find an alternative?` | `TryDisplace` | it does have a holder, and the recursive call is about to happen |
| `Displacement succeeded` / `failed` | `DisplaceOk` / `DisplaceFail` | that recursive call returned |
| `Matched: equation i ↔ variable j` | `Assign` | both arrays are written |
| `Equation i has no augmenting path — unmatched` | `EquationFailed` | the outer loop's attempt returned false |

**So the animation is not an illustration of the algorithm — it is the algorithm reporting
itself.** Those frames were recorded during the compile you loaded.

### The textbook name, and a correction worth carrying

This is **Kuhn's algorithm** — the Hungarian augmenting-path method for bipartite matching.
Depth-first, one augmenting path per equation, O(V·E).

**It is not Hopcroft-Karp**, which a course is more likely to name. Hopcroft-Karp uses BFS to
find many *vertex-disjoint* augmenting paths per phase and runs in O(E√V). **Same idea, different
schedule** — and worth knowing, because the two produce different frame counts and different
intermediate matchings for the same input. What you watched in Act 2 is specifically Kuhn's
backing-up, and Hopcroft-Karp would not have shown it that way.

### Why the animation is reproducible

`eq_vars` is a `HashSet`, whose iteration order is not stable — so `augment_traced` **sorts the
variables before exploring them**. Without that sort, Act 1's *"variable 0 first"* would be a
coin flip, and every `**Expected:**` line in this tour would be *sometimes* true, which is worse
than being wrong. Pinned by `test_maximum_matching_is_deterministic_under_ties` in the same file.

### Stand inside it

[ProportionalLoop → Structural → Matching](hrw://load/ProportionalLoop/Structural/MatchingAnim)

**Expected:** the matching animation, unstarted.

Click **Debug**. Execution stops at the live-trace anchor *before any algorithm work*, showing
`frame_index` as `usize::MAX` — the startup gate.

**Expected:** VS Code stops, and the Debug Console shows that frame index rather than `0`.

**F5 advances one algorithm step**, and the animation follows each press.

**Expected:** one press moves the animation forward exactly one frame.

To stand *inside* the algorithm rather than beside it, set your own breakpoint on the
`match match_var[var]` expression in `augment_traced`. **That one branch is the entire
free-versus-displace decision** — Act 1 is what it looks like when the first arm keeps being
taken, and Act 2 is what it looks like when the second one is.

> **When you ask Claude about the line you are stopped at, select it first.**
> Measured 2026-08-07 (`docs/ideas.md` #70): **Claude cannot see a debugger stop at all** — no
> location, no frame, no call stack. Stopping reveals the *file*, and that much does reach
> Claude, but never the line. **Selecting the line does reach it**, verified against the source.
> So the gesture is one click, and without it Claude knows only which file you are in.
>
> If a selection ever seems not to land, **name the place instead** — *"I'm stopped in
> `augment_traced` at the `match`"* — which always works, because Claude then reads the file.
> This stop is where the tour hands off to a conversation, so it is worth knowing which gesture
> carries and which does not.

## What this tour cannot check

Whether the **matrix view** makes the search legible — whether the moving highlight reads as
"the algorithm is exploring here" or as flicker. That is the half `egui_kittest` cannot reach
(`incidence_view.rs` cells are painted, not widgets), so it is exactly what a walked tour is
for.

Whether 114 frames is watchable in Act 3, or whether the honest thing is to seek straight to
frame 111 and let the earlier 110 stay unwatched. **The Play button makes this an empirical
question for the first time**, and it is worth running once at 90 seconds and once at three
minutes before deciding.

And whether Act 2's punchline — no equation matched to its own left-hand side — lands as
*revelation* or as *arbitrary*. It is the strongest claim in the tour and the one I am least
able to judge from this side.

**And Act 5 most of all, because it is the newest and the least like the others.** It names
functions instead of showing them, on the theory that a name plus a debugger beats transcribed
code that rots. **If it instead reads as homework** — a list of things to go look up rather than
something that closes the loop — then the theory is wrong and the stop should carry a small
amount of real code after all. That judgement is yours; I cannot make it from here.

## What comes next in the chain

Matching says *which* equation solves *which* unknown. It does not say **in what order** to
evaluate them — and for `ProportionalLoop` there is no valid order at all: `error` needs
`measurement`, which needs `command`, which needs `error`. A genuine cycle.

Finding those cycles is **Tarjan's strongly-connected-components algorithm**, it turns the
matched system into blocks that must be solved simultaneously, and it has its own animation
(`TarjanAnim`). `ProportionalLoop` is the smallest model that produces a coupled block, which
makes it the specimen for that tour too — the same three equations, one question later.
