# Ideas — backlog for future implementation

**Purpose:** the numbered backlog. Every idea keeps its number forever, so `#43` means one
thing across every document and commit message.
**Status:** record. **Delivered items are cut down to a one-line entry** in *Delivered and
closed* at the foot of this file — a backlog is for what has not been built. **Their numbers
are never reused**, because 55 references outside this file depend on them resolving.
*Declined* items stay in full: their whole job is to stop a proposal coming back.
**Read when:** planning new work, or before proposing something that may already be here with
a decision attached. **Candidates, not commitments.**

Captured ideas not yet scheduled. **These are candidates, not commitments** — no
arc depends on them, and settled decisions live in [`DECISIONS.md`](../DECISIONS.md),
current work in [`CLAUDE.md`](../CLAUDE.md). Promote an item here into an arc /
decision when it's picked up.

## Prioritization model: guided tours drive feature priority

The `docs/compiler-phases/` documents are being re-envisioned as **guided tour
scripts** — each phase chapter becomes a walkthrough that leverages HRW and
specimens to teach the phase's concepts interactively (see idea #24). This
re-envisioning provides the **prioritization principle** for this backlog:

> **Build the feature that the next guided tour needs.**

The workflow: (1) design the guided tour for a phase — what concepts it teaches,
what specimens it uses, what the learner should *see* at each step; (2) identify
gaps — which "the learner should see X" moments require HRW enhancements that
don't exist yet; (3) build those enhancements (pulled from this backlog or newly
identified); (4) write the tour, now fully supported by the tool.

Items below can be **tour-linked** (needed by a specific guided tour) or
**generic** (supports the tool broadly). Tour-linked items carry a
`Tours: #phase` tag. When planning work, tour-linked items for the *current*
tour take priority over unlinked items of the same severity.

| Item | Tours |
|------|-------|
| #14 Rank deficiency visualization | Structural Analysis |
| #15 Matching-as-permutation view | Structural Analysis |
| #16 Animated BLT block discovery | Structural Analysis |
| #9 Animated algorithm stepping | Structural Analysis, Index Reduction |
| #17 Jacobian sparsity and conditioning | Solve Lowering, Simulation |
| #18 BDF step-size and order control | Simulation |
| #22 Exact event times + Newton convergence | Simulation |
| #19 Resolver process view | Parse / Resolve / Typecheck |
| #20 Flattening process view | Instantiate / Flatten |
| #21 Event lowering process | Events |
| #5 Four-bar linkage + planar library | Index Reduction |
| #7 Full init-system structural analysis | Initialization |
| #10 Cross-stage identifier tracking | (all tours) |
| #11 In-view search | (all tours) |
| #26 VS Code extension: Trace / Debug / Arm-it | (all tours) |
| #27 Equation sheet (readable DAE) | Flatten, Structural Analysis |
| #28 Source-to-equation traceability | Flatten (bridges Parse–Flatten) |
| #29 Solver stepping visualization | Simulation |
| #30 Live solver stepping (LiveTrace) | Simulation |
| #31 Revisit all simulator functionality | Simulation |
| #32 ~~In-app tour view~~ ✅ | (all tours) |
| #34 Sub-view / tree-node links | (all tours) |
| #35 Multiple tour documents + progress | (all tours) |
| #33 Comprehensive tooltips | (all tours) |
| #36 Modelica syntax highlighting | (all tours) |
| #37 Reverse identifier tracking | (all tours) |
| #38 Syntax highlighting for canvas axis labels | (all tours) |
| #1, #4, #13, #23 | generic |

---

## 4. Reconsider the arc close-out gates (differential test + debugger single-step)

Captured 2026-07-20 (Doug). The arc close-out ritual (CLAUDE.md) currently gates on (1) the specimen
passing the **differential test** in both toolchains (System Modeler vs Rumoca) and (3) Doug having
**single-stepped the phase** in the debugger. Arcs 1–3 all closed with these *accepted-as-deferred /
unconfirmed* rather than met. Doug is giving separate thought to whether they should remain **gates**
at all (his words: "we should probably eliminate those two items as gates"). Pending that decision,
treat them as satisfiable-by-acceptance, not hard blockers. If eliminated, update the ritual in
CLAUDE.md (and note it here as done). This is Doug's call — a charter/ritual change, not an
implementation task.

## 5. Four-bar linkage specimen + un-park the planar mechanics library (Arc 4 deferred)

Captured 2026-07-20 (Doug + finding). The charter's Arc-4 specimen is a four-bar / parallelogram
linkage (nonlinear loop-closure → index-3). It is **deferred**: Rumoca's Rust-path index reduction at
pin 8cdc7419 does not reduce nonlinear holonomic constraints (`x²+y²=L²`) — verified on the barest
Cartesian pendulum, not a library bug (see DECISIONS.md). The hand-built planar mechanics library
(`lib/PlanarMechanics.mo`) is drafted, complete, and parked. **Un-park when** either (a) Rumoca gains
nonlinear-constraint reduction (worth confirming against its own test suite / a possible upstream
contribution), or (b) we confirm the full private sim path / CasADi target handles it and expose a way
to drive it from HRW. Then: author `FourBarLinkage.mo` from the library, wire its trace + narrative,
and show index-3 → reduced. Arc 4's core (index reduction observed) is already met via Drivetrain.

## 7. Full initialization-system structural analysis (the rigorous form of #6)

Captured 2026-07-20 (Doug). Idea #6 (implemented) is a **count** heuristic — explicit
initial conditions vs states — which reliably catches gross *over*-determination
(`OverInitRc`) but is blind to a system that is count-balanced yet **structurally
singular** at initialization: e.g. two initial conditions pinning the *same*
variable while another state is left unpinned (net count zero, but ill-posed).

Enhancement: do for the **initialization system** what Arc 3 does for the continuous
system. Assemble the full init system — the continuous equations at t = 0 (with
`der(x)` as unknowns) + the user `initial equation`s + the fixed-`start` conditions —
build its incidence matrix, run maximum matching, and report **which initial
equations are redundant and which init unknowns are unpinned** (the same
unmatched-equations / unmatched-unknowns verdict the Structural tab gives, but for
t = 0). That turns "surplus +1" into a precise, per-equation diagnosis, and would
catch under-determination correctly too (a truly unpinned state — no condition and
no usable `start`), which the count heuristic deliberately does not.

Observatory shape: extend the **Initialization** stage's `determinacy` block (or a
sibling view) with the init-system matching result; a spy-plot of the init incidence
would reuse the Arc-3 canvas.

**Scout first:** whether Rumoca already assembles the init system anywhere reusable.
Pieces exist — `build_ic_relaxation_hint` already detects *singular initial algebraic
subsystems* (it drives the relaxation hint), and `build_ic_plan` walks the algebraic
init — but neither incorporates the user's `initial equation`s / fixed starts into one
matched system. If Rumoca does not expose it, reproducing the assembly risks a
subtly-wrong analysis (cf. the Arc-4 nonlinear-constraint reimplementation caution) —
weigh an upstream contribution instead.

## 9. Incremental / animated views of algorithms

**Largely implemented; one candidate left.** Seven animated views now exist:

| View | Kind | Live trace? |
| --- | --- | --- |
| Matching — augmenting paths (`matching_anim.rs`) | replay of a search | yes |
| BLT discovery — Tarjan SCC (`tarjan_anim.rs`) | replay of a search | yes |
| Index reduction — Pantelides / dummy derivatives (`reduction_anim.rs`) | replay of a search | yes |
| `pre()` lowering (`pre_lowering_anim.rs`, idea #40) | replay of a pass | yes |
| Tearing (`tearing_anim.rs`, 2026-07-29) | replay of a search | yes |
| Alias elimination (`alias_anim.rs`, 2026-07-29) | reveal of a list | no — nothing to trace |
| Initial-condition planning (`ic_plan_anim.rs`, 2026-07-29) | reveal of a list | no — nothing to trace |
| Connection expansion (`connection_anim.rs`, 2026-07-29) | replay of a pass | not yet — see below |

Trace infrastructure in `rumoca-phase-structural` (`maximum_matching_with_trace`,
`tarjan_scc_with_trace`, `tear_algebraic_loop_with_trace`, `block_local_incidence`)
`rumoca-phase-dae` (`to_dae_with_options_traced`) and `rumoca-phase-flatten`
(`flatten_ref_with_options_traced`, `connections::trace`).

The **replay / reveal** distinction is deliberate and worth preserving: only some
phases hide a search. Alias elimination walks a list and substitutes; the IC plan is
already computed when HRW sees it. Those two get a stepper for the *accumulation*
(the unknown count falling, the plan's shape emerging) but no Debug button, and their
module docs say why. See `DECISIONS.md` (2026-07-29).

**Open follow-up — a live path for connection expansion.** The phase is
instrumented and its recorded replay is complete, but there is no Debug button:
re-running flatten needs the resolved `ClassTree` (the whole MSL) and the
instance overlay, and shipping those to the UI thread to arm a breakpoint is a
bigger change than the view warranted on its own. The right fix is a
**worker-side live-debug path** — spawn the traced re-run on the worker, where
the tree already lives, and stream frames back over the existing channel. That
would also simplify the three views that currently clone a DAE into the app.

**Open follow-up — per-merge connection frames.** Frames are emitted per
connection *set*, not per union-find merge; the merges sit several call levels
below `process_connections`. Worth doing only if watching sets form turns out to
leave "why is this one set and not two?" unanswered.

**Remaining candidates:** **forward-mode AD lowering**
(`rumoca-phase-solve::ad`) — see below. Newton iteration / per-step convergence
is **deferred pending simulator maturity** (Doug, 2026-07-29 — see #22); it is
the last obvious replay-shaped algorithm, but instrumenting a phase we do not
trust would teach the wrong thing.

**Solve lowering: the phase does not qualify, but AD inside it might.** Asked
2026-07-29 (Doug) whether Solve Lowering was meant to get an animation. It was
not, and the reason is the replay/reveal test above. Two of the phase's three
jobs are translations with nothing hidden: `layout.rs` packs DAE variables into
the solver's `y`/`p` slots (a walk that assigns indices, and the result is
already readable as `problem.layout` / `problem.solve_layout` in the stage
tree), and `lower.rs` compiles each equation into a register-machine program,
one node at a time.

`ad.rs` is the exception. Forward-mode AD rewrites the primal program into a
J·v program by applying the chain rule per operation — `x*y` becomes
`x*dy + y*dx` — which *is* a rule-driven transformation with a reason at every
step. It is also where the **Jacobian comes from**, so it pairs with #17 (which
covers the Jacobian's values and conditioning but not the program that computes
them) and with the linear-algebra thread.

Deliberately **not** proposed as work: whether watching a JVP tape assemble
teaches more than reading `ad.rs` with a breakpoint in it is exactly the kind of
question that should come from Doug's reading, not from Claude's guess. Revisit
when the Jacobian becomes a live topic.

Captured 2026-07-21 (Doug). **Top-of-mind, long-running theme.** Educational
animations of challenging compiler algorithms — index reduction (Pantelides
iterating, SCC discovery, demotion), matching's augmenting paths, tearing — showing
each step incrementally so the algorithm's *process* is visible, not just its result.

- **Why it matters:** the current observatory shows the *output* of each phase (the
  IR before/after), but the hardest concepts to learn are the *algorithms themselves*
  — why Pantelides differentiates this constraint, why matching chose this augmenting
  path, why tearing picked this tear variable. A steppable animation of the algorithm
  in progress, synced with the data structures it's mutating, is the highest-value
  learning instrument HRW can offer.
- **Debugger integration:** the animations should be driveable from the VS Code
  debugger — single-step through the Rumoca phase code while watching the animation
  update in the HRW window. This ties directly to the instrumentation mission:
  the algorithm must emit step events that the observatory can render, and the
  debugger breakpoint flow ("debug") should be able to land inside an algorithm
  iteration, not just at the phase boundary.
- **Candidate algorithms:** Pantelides (index reduction), maximum matching
  (augmenting paths), BLT decomposition (SCC / Tarjan), tearing (selecting tear
  variables), Newton iteration (convergence per step). Each has a natural visual
  representation (bipartite graph, incidence matrix, BLT spy plot) that can
  animate incrementally.
- **When:** this is pass-two territory — it requires the internal instrumentation
  that the monorepo move enabled. Likely starts with matching (the bipartite graph
  view already exists) or Pantelides (the most pedagogically valuable).

## 10. Cross-stage identifier / equation tracking ("follow this through the pipeline")

Captured 2026-07-21 (Doug). Given a Modelica identifier (variable, parameter,
component) or equation from a specimen, **highlight every piece of information
associated with it across all stage views** — its declaration in Parse, its resolved
`def_id` in Resolve, its instantiated form in Instantiate/Typecheck, the flat
variable(s) it becomes in Flatten, which equation rows and matched unknowns it
appears in (Structural), whether it was differentiated or demoted (Index reduction),
its initial-condition plan entry (Initialization), any event conditions it
participates in (Events), and its solver variable slot (Solve lowering / Simulation).

- **Why it matters:** the pipeline transforms a single Modelica declaration through
  many representations — a variable named `v` becomes a flat unknown, gets a row in
  the incidence matrix, may be demoted by Pantelides, gets an IC plan entry, and
  ends up as a state in the solver. Understanding what happened to *one thing* across
  all phases is the core learning question, and today requires manually clicking
  through each tab and searching. A "follow this identifier" mode that highlights
  the relevant nodes/rows/cells across every stage view answers it in one action.
- **Sketch:** a search/select interaction — type or click an identifier, and every
  stage view gains highlights (tree nodes expanded + highlighted, incidence/spy-plot
  rows/columns lit up, simulation plot series selected). The cross-stage diff
  machinery already tracks `def_id` continuity; this extends it to a persistent
  visual filter. Applies to equations too (follow a `der(h) = v` through flatten →
  structural → index reduction → solve lowering).
- **When:** after the cross-stage diff infrastructure is solid (pass two). The
  `def_index` and `def_resolutions` already provide the identity backbone; the work
  is wiring highlights into each view.

## 11. In-view search for Modelica identifiers

**Half delivered 2026-07-28** — the *find-and-jump* half exists for the followed
identifier. The Context Bar's Following row shows `3 of 4 in Flatten` with prev
/ next arrows; each jump opens the collapsed ancestors and scrolls the match to
the centre of the tree. Matches come from `bridge::mention_paths`, the same walk
that produces `tracking.paths` in the emitted context, so the tree and Claude
cannot disagree about where an identifier appears.

What remains is the *query* half: a text field for finding something you are
**not** following. Note the ordering argument that produced this split — a search
box would have asked Doug to type an identifier the app was already displaying.
The plumbing that remains to be reused (ancestor expansion, scroll-to,
match cycling, count display) is the bulk of the work; adding a query swaps
"matches of the followed name" for "matches of typed text".

The original capture follows.


Captured 2026-07-21 (Doug). A search interaction within each stage view: type a
Modelica identifier (variable, parameter, component, equation label) and the view
scrolls to / expands / highlights where that identifier's information appears.

- **Why it matters:** the IR trees are deep and wide (especially after MSL expansion).
  Finding where `v` or `der(h)` lives in a 200-node Flatten tree, or which row of the
  incidence matrix corresponds to a particular equation, currently requires manual
  scrolling and visual scanning. A search box that jumps directly to the match removes
  that friction.
- **Relationship to #10:** this is the *within-view* complement to #10's *cross-stage*
  tracking. #10 highlights an identifier across all stages simultaneously; this idea
  is about finding it efficiently within a single view. Both share the need for an
  identifier-aware index of each view's content, but the UX is different: #10 is a
  persistent multi-view filter, this is a transient find-and-jump.
- **Sketch:** a Ctrl+F-style search bar (or a text field in the tab bar) that
  fuzzy-matches against qualified names in the current view — tree node
  keys, variable names in the flat model, equation labels, spy-plot row/column
  headers. Matching nodes auto-expand and scroll into view; matching matrix
  rows/columns highlight.
- **When:** can start independently of #10 — a single-view search is simpler and
  immediately useful.

## 13. Guided learning explorations through the Rumoca code

Captured 2026-07-21 (Doug). HRW is not merely an application — it is a **learning
instrument**. The goal is for Claude to provide guided explorations of the
mathematical and algorithmic foundations of continuous-system modeling and
simulation, using the actual Rumoca codebase as the teaching material.

- **Why it matters:** Doug is taking a **linear algebra applications** class at
  Purdue (Fall 2026, masters in robotics). The Rumoca codebase is rich with applied
  linear algebra — incidence matrices, maximum matching (transversals), BLT
  decomposition (Dulmage-Mendelsohn), index reduction (structural rank), Jacobian
  sparsity, Newton-based BDF integration (sparse LU). These are not abstract
  textbook exercises — they are algorithms running on real Modelica models, with
  observable inputs and outputs in HRW's views. A guided exploration ties
  coursework to working code: "here is the concept, here is the Rumoca function
  that implements it, here is the specimen that exercises it, here is what HRW
  shows you."
- **Initial focus (Fall 2026): linear algebra.** Candidate explorations:
  - The incidence matrix as a bipartite adjacency matrix — structural rank,
    matching, and transversals (`build_incidence`, `maximum_matching`)
  - BLT decomposition as Dulmage-Mendelsohn — permuting to block triangular form,
    SCCs in the dependency graph (`build_structural_report`)
  - Index reduction as structural rank repair — Pantelides' algorithm, augmenting
    paths, the dummy-derivative funnel (`dae_prepare::*`)
  - The Jacobian — sparsity structure, conditioning, what the solver sees
    (`build_solver_sparsity_triplets`, diffsol internals)
  - Newton iteration on coupled BLT blocks — solving J·Δx = −F each step
  - Initialization as a (possibly overdetermined) linear/nonlinear system at t=0
- **Later topics:** differential equations (ODE/DAE theory, BDF methods, stability,
  stiffness), numerical methods (step-size control, error estimation, event
  detection), and control theory (transfer functions, state-space, the specimens
  as control-system models).
- **Shape (to figure out):** the right delivery format — could be structured
  `docs/explorations/` walkthroughs, could be interactive sessions using HRW's
  capture→explain flow, could be annotated code tours, or a combination.
  The format should leverage Claude's ability to read the Rumoca source, point at
  specific functions and data structures, and connect them to textbook definitions.
  It should also leverage HRW's views — "open BouncingBall, click the Structural
  tab, look at the incidence matrix: this IS the bipartite adjacency matrix from
  your textbook."
- **The meta-point:** we are using Rumoca and HRW to enable Claude to help Doug
  **master the math and algorithms** of continuous-system modeling and simulation.
  Every feature decision should be evaluated against this learning mission.

## 15. Matching-as-permutation view — before and after the transversal

**Partially implemented 2026-07-22.** Matched-pair green circles now mark the
transversal diagonal on the incidence matrix. BLT block boundaries draw amber
outlines. Colors: `MATCHED_MARKER`, `BLT_BOUNDARY`. The full permuted-matrix
toggle (rows/columns reordered to put matched pairs on the diagonal) remains
deferred.

Captured 2026-07-21 (Claude, learning-driven). Show the incidence matrix **before
and after** the row/column permutation implied by maximum matching.

- **Why it matters (linear algebra):** maximum matching finds a **transversal** — a
  set of nonzeros, one per row and one per column, that defines a permutation
  matrix P. Applying P puts a nonzero on every diagonal entry. This is the
  same operation as the row/column permutation in textbook LU factorization:
  PAQ = LU. Seeing the matrix transform from scattered nonzeros to a diagonal
  makes the permutation concept concrete.
- **Sketch:** a toggle or side-by-side showing (a) the incidence matrix in its
  original equation/unknown ordering, and (b) the same matrix with rows and
  columns permuted so matched pairs sit on the diagonal. The diagonal entries
  (the transversal) are highlighted. Off-diagonal nonzeros become the "fill" that
  creates coupling — the visual seed for understanding why LU factorization cares
  about pivoting strategy.
- **Bonus:** once the matched matrix is permuted, the BLT block structure becomes
  visually obvious — the diagonal blocks in the spy-plot are the same blocks you'd
  see as dense sub-matrices along the diagonal of the permuted incidence matrix.
  This bridges the incidence view and the spy-plot view, showing they're two
  perspectives on the same decomposition.

## 17. Jacobian sparsity and conditioning view

Captured 2026-07-21 (Claude, learning-driven). Show the **Jacobian matrix** — the
actual partial-derivative matrix the solver uses — alongside the incidence matrix,
and report its conditioning.

- **Why it matters (linear algebra):** the incidence matrix is *structural* (does
  equation i reference unknown j? yes/no). The Jacobian is *numerical* (∂fᵢ/∂xⱼ —
  the actual partial derivative value). The incidence matrix can have full
  structural rank while the Jacobian is numerically singular (the nonzeros happen
  to cancel). Understanding the difference between structural and numerical rank
  is fundamental. The condition number κ(J) tells you how sensitive the solution
  is to perturbations — directly from the linear algebra course.
- **Sketch:** a spy-plot of the Jacobian's sparsity (same canvas as incidence, but
  showing ∂fᵢ/∂xⱼ patterns — it may differ from the incidence because the solver
  works in a different variable ordering). Optionally, for small systems, show the
  actual numerical values (as a heatmap or labeled matrix). Report the condition
  number for each coupled BLT block (where Newton iterates). On hover over a
  coupled block in the spy-plot: "this 3×3 block has condition number κ = 42 —
  well-conditioned" or "κ = 1e12 — nearly singular, Newton may struggle."
- **Rumoca entry point:** `build_solver_sparsity_triplets` (public) gives the
  Jacobian sparsity in solver-column order. The actual numerical Jacobian comes
  from diffsol during simulation — may require instrumentation.
- **Specimens:** ProportionalLoop (well-conditioned linear loop) vs NonlinearLoop
  (same structure, different conditioning) — same incidence, different Jacobian.

## 18. BDF step-size and order control visualization

Captured 2026-07-21 (Claude, learning-driven). During simulation, plot the BDF
integrator's **step size h(t)** and **method order k(t)** alongside the solution
trajectories.

- **Why it matters (differential equations / numerical methods):** BDF methods
  adapt both step size and order to maintain accuracy while taking the largest
  possible steps. Plotting h(t) reveals *where* the solver works hard (small steps
  = fast dynamics or events) and where it cruises (large steps = smooth solution).
  Plotting order k(t) shows the stability/accuracy trade-off the solver makes.
  For stiff problems (BenchActuator), you'd see BDF taking large steps through the
  stiff transient where RK45 would collapse to tiny steps — the visual proof of
  why implicit methods exist.
- **Sketch:** a secondary plot panel (below or beside the trajectory plot) showing
  h(t) and k(t) as time series. For comparison: a toggle to run the same model
  with RK45 and overlay its h(t) — the step-size collapse on a stiff problem is
  dramatic and immediately explains stiffness.
- **Rumoca entry point:** diffsol's `OdeSolverMethod` trait exposes step info, but
  `simulate_solve_model` returns only the resampled output grid. Instrumentation
  needed: either a callback during integration that logs (t, h, order) per step,
  or access to diffsol's `OdeSolverState`. This is a pass-two instrumentation
  target — the simulation loop is inside `rumoca-sim`.
- **Textbook link:** Hairer & Wanner, *Solving Ordinary Differential Equations II*
  (stiff problems), chapters on BDF order/step-size selection. Brenan, Campbell &
  Petzold, *Numerical Solution of Initial-Value Problems in DAEs*.

## 19. Resolver process view — scope/symbol tables and resolution steps

Captured 2026-07-22 (from pass-two-plan Arc 1). The Resolve tab today shows the
*result* — the resolved class with `def_id`s populated. Enhancement: surface the
**resolver's process** — the scope stack, symbol tables, and resolution steps that
produced those bindings.

- **Why it matters:** name resolution is one of the most complex compiler phases.
  Seeing *how* `flange_a` resolved to `def_id 27579` — which scopes were searched,
  what shadowing occurred, which imports were followed — teaches the Modelica
  scoping model and the Rumoca implementation simultaneously.
- **Rumoca entry point:** `rumoca-phase-resolve` / `rumoca-compile::Session::resolve()`.
  Scout the crate for scope/symbol-table data structures and resolution logic.
- **Also in Arc 1:** the typecheck's **dimension-evaluation steps** (how array
  dimensions are computed and checked) — scout `rumoca-phase-typecheck` for
  dimension-related internal state.

## 20. Flattening process view — connector expansion and flow-sum generation

Captured 2026-07-22 (from pass-two-plan Arc 2). The Flatten tab today shows the
flat model IR. Enhancement: surface the **flattening process** — connector expansion,
flow-sum equation generation, and modifier application as they happen.

- **Why it matters:** flattening transforms a hierarchical Modelica model into a
  flat system of equations. The key steps — expanding connectors into
  potential/flow variables, generating flow-sum equations at each connection node,
  applying modifications — are where Modelica's connect semantics become concrete
  equations. Showing this process (not just the result) explains *why* the flat
  model has the equations it does.
- **Rumoca entry point:** `rumoca-phase-flatten` (currently opaque — phases 5–9 run
  inside `compile_model_strict_reachable_with_recovery`). Scout the crate for
  connector-expansion and flow-sum-generation internals.

## 21. Event lowering process — `when` → zero-crossing construction

Captured 2026-07-22 (from pass-two-plan Arc 6). The Events tab today shows the
hybrid partitions (conditions + reinit actions). Enhancement: surface the
**lowering process** — how `when` clauses become zero-crossing functions (`f_z`)
and mode functions (`f_m`).

- **Why it matters:** the translation from Modelica's `when h <= 0` to the solver's
  zero-crossing function `f_z(x) = h(x)` with root-finding is where the hybrid
  semantics become numerical. Understanding this lowering explains event chattering,
  missed events, and the solver's event-detection machinery.
- **Rumoca entry point:** scout the DAE construction for `when`/`reinit` lowering
  and zero-crossing function assembly. The Events stage already reads the DAE's
  public fields; the process of *constructing* those fields is the target.

## 22. Exact event times and per-step Newton convergence from the solver

**DEFERRED 2026-07-29 (Doug) — pending simulator maturity.** *"So far as I can
tell, the simulator used by Rumoca is immature and not yet functional. So, I
don't want to waste time or effort on stuff like simulation animations now.
Ultimately, my greatest interest might be the simulation view as I have many
ideas there. But for now, I want to focus on stuff where we are confident that
the underlying Rumoca machinery works correctly and reliably."*

So: **not a candidate for instrumentation work, despite being the last obvious
replay-shaped algorithm.** Revisit when the simulator is trustworthy, and note
Doug expects this to become his *highest*-interest area eventually — the deferral
is about readiness, not value.

**How we will know when to revisit — measure it, do not estimate it.** What is
known today is narrow: `worker_simulate_runs_bouncing_ball` and
`single_inertia_simulates_to_a_correct_trajectory` pass, so simple specimens
work. That is entirely compatible with failing on stiff, high-index or
event-heavy models, which is where Doug's judgement comes from. #43 makes the
question answerable rather than arguable: **System Modeler simulates the same
specimen and the trajectories get diffed.** A standing differential test over the
specimen corpus would turn "so far as I can tell" into a number, and tell us the
month this becomes worth building. Cheap, and it is the same machinery #4 needs.

Captured 2026-07-22 (from pass-two-plan Arc 7). Two solver-internal data streams
not yet surfaced:

- **Exact event times:** `StepUntilOutcome::RootFound { t_root }` exists internally
  in diffsol/rumoca-sim. Surfacing it would replace the heuristic step-mode
  break-detection with exact event locations, and enable plotting event markers on
  the time axis.
- **Newton convergence per step:** for each implicit BDF step, the Newton solver
  iterates to convergence. Logging iteration count and residual norm per step
  reveals where the solver struggles — the same coupled BLT blocks that the
  structural view highlights.

These complement idea #18 (BDF step-size/order) — together they give a complete
picture of what the solver is doing at each time step.

- **Rumoca entry point:** `rumoca-sim::simulate_solve_model` and the diffsol
  integration loop. Instrumentation needed: a per-step callback or post-hoc
  log that records (t, h, order, newton_iters, event_detected).

## 23. Dedicated performance review cycle (when needed)

Captured 2026-07-22 (Doug + Claude). The tech-debt sweep already catches
performance items incidentally (it found per-frame `from_report` re-parsing,
per-frame `Path::exists()`, and per-frame `layout_no_wrap` — all fixed). A
**dedicated** performance review with profiling is not yet warranted: the app is
small, egui's 16ms frame budget has headroom, and compilation/simulation run on
the worker thread.

- **When to revisit:** (a) specimens grow large enough that the tree inspector or
  custom views lag visibly (likely trigger: MSL-heavy models with deep IR trees),
  (b) instrumentation hooks add measurable overhead to compilation (noticeable
  when comparing instrumented vs upstream Rumoca), or (c) simulation plotting
  with many variables or long time series causes frame drops.
- **What it would look like:** an occasional pass with
  `cargo flamegraph` or `perf` on a representative specimen, looking for hot
  spots in the UI thread. Focus areas: tree rendering (deep/wide JSON), canvas
  painting (large matrices), and channel throughput (many `CompileProgress`
  messages per compile).
- **For now:** let the phase-boundary tech-debt sweep catch performance issues as they
  surface — it has a good track record.

## 24. Re-envision compiler-phases docs as HRW-driven guided tours

Captured 2026-07-22 (Doug). The `docs/compiler-phases/` documents were written
before HRW existed — standalone theory explanations of each Rumoca phase with no
connection to the tool that can *show* the phase happening. Now that HRW can
render every phase's IR on real specimens, those docs should be re-envisioned as
**guided tour scripts**: the theory is preserved (and remains the explanation
layer), but the structure becomes a walkthrough keyed to HRW actions and
specimens.

- **Why it matters:** a guided tour unifies what was previously separate — "read
  the theory doc" and "click around in HRW" — into one experience: "open
  BouncingBall, click Structural, look at the incidence matrix — *this is* the
  bipartite adjacency matrix from the theory. Hover row 3 — that equation
  references two unknowns..." The theory explains what you're seeing, not what
  you might someday see.
- **The curriculum-aware teacher model:** Claude acts as a curriculum-aware
  teacher — designing each tour with explicit learning goals first, then
  identifying which HRW enhancements the tour needs (features pulled from this
  ideas backlog), building those features, and finally writing the tour. The
  tours **drive feature prioritization** for the entire backlog (see the
  prioritization model at the top of this file and `docs/vision.md`).
- **Shape:** enhanced markdown documents in `docs/compiler-phases/`, each
  structured as a sequence of steps: "open specimen X → click tab Y → observe Z
  → here's the theory that explains Z." The existing theory content is
  refactored into these steps, not discarded. Specimens are chosen for the
  phenomenon each step teaches (leveraging the `// purpose:` convention).
- **Sequencing:** start with the phases that have the richest visual/algorithmic
  content — **Structural Analysis** and **Index Reduction** — where the most
  backlog items cluster and the learning payoff is highest. Simpler phases
  (Parse, Resolve) can follow with lighter tours.
- **Relationship to idea #13 (guided learning explorations):** #13 proposed
  explorations organized by *mathematical topic* (linear algebra, ODEs, etc.)
  for Doug's coursework. The guided tours here are organized by *compiler
  phase*. They're complementary: a tour says "here's what Structural Analysis
  does," an exploration says "here's how maximum matching connects to your
  linear algebra class." Both draw on the same HRW features and specimens.
- **Design constraint:** features built to support tours must be general-purpose
  HRW enhancements, not one-off tour widgets. The tours are one way to
  experience the features; the features enrich HRW permanently.

## 26. VS Code extension integration: Trace / Debug / Debug-shortcut

Captured 2026-07-23 (Doug). **High-priority learning infrastructure.** Extend the
existing Rumoca VS Code extension (`packages/vscode`, publisher JamesGoppert) with
three capabilities that complete HRW's point → observe → understand → ask loop by
bridging from the Modelica source (where Doug thinks) to HRW (where Doug observes)
to the debugger (where Doug verifies understanding).

**This is one of the highest-value features on the backlog.** It directly serves the
learning mission (`docs/vision.md`) by maximizing both axes of the multiplicative
pair — context identification (right-click on a Modelica identifier is the most
natural starting point) × context-sensitive explanation (the traced/debugged state
is the richest possible context for Claude to explain).

### Capability 1: "Trace this identifier"

Right-click on a Modelica identifier in VS Code → select "Trace this identifier" →
HRW highlights that identifier's contribution to **every stage view** across the
entire pipeline:

- **Parse:** the AST node for the identifier
- **Resolve:** the `DefId` assigned to it
- **Instantiate:** the instance with modifications applied
- **Typecheck:** the `TypeId` assigned
- **Flatten:** the qualified name(s) it becomes, the equations it appears in
- **DAE:** the variable classification (state / algebraic / parameter / …)
- **Structural:** the row/column in the incidence matrix, the matching assignment
- **Index reduction:** whether it was differentiated or demoted
- **Initialization:** its IC plan entry
- **Events:** any event conditions it participates in
- **Solve lowering:** its solver variable slot
- **Simulation:** its time-series plot highlighted

This is the cross-stage identifier tracking (idea #10) initiated from the most
natural starting point — the Modelica source itself, not the HRW UI.

### Capability 2: "Debug this identifier"

Right-click on a Modelica identifier → select "Debug this identifier" → the
extension sets **conditional breakpoints** in Rumoca compiler phase code so the
debugger breaks when that specific identifier is being processed:

- Breakpoint in `rumoca-phase-resolve` conditioned on the identifier name
- Breakpoint in `rumoca-phase-flatten` conditioned on the qualified name
- Breakpoint in `rumoca-phase-structural` conditioned on the variable index
- (etc., one per phase where the identifier is transformed)

This lets Doug single-step through the compiler *while it processes a specific
variable*, not just at phase boundaries. Combined with HRW's live algorithm
animations, he sees the compiler's internal state update as it touches his
chosen identifier.

### Capability 3: "Debug" shortcut (from idea #25) — ✅ DELIVERED

**Delivered 2026-07-24** as the HRW Debugger Bridge extension (`hrw/vscode-extension/`).
After capturing an IR field in HRW, type `debug` in Claude chat → a conditional
breakpoint appears in the **already-running** debug session without restart.
Breakpoints accumulate per specimen. See idea #25 and `docs/debug-set-sites.md`.

### Technical approach

All three capabilities share the same infrastructure:

1. **IPC channel:** the VS Code extension watches a signal file (e.g.
   `.hrw-bridge/trace-request.json` or `.hrw-bridge/breakpoint-request.json`).
   HRW or Claude writes to the file; the extension reads it and acts.
2. **VS Code extension API:**
   - `vscode.debug.addBreakpoints()` for the debug shortcut and debug-this-identifier
   - Communication with HRW (via the same bridge file protocol) for trace-this
3. **Existing extension:** the Rumoca extension (`packages/vscode`) already
   provides Modelica language support via LSP. It has **no debugger integration
   today** — no `vscode.debug` usage, no `contributes.debuggers` in
   `package.json`. The extension is TypeScript, ~3860 lines in `extension.ts`.
   Adding context-menu items and debug API calls is straightforward VS Code
   extension development.

### The complete learning workflow

With all three capabilities:

1. **Read** the Modelica source in VS Code
2. **Trace** an identifier through the full pipeline (VS Code → HRW highlights)
3. **Debug** the compiler processing that identifier (VS Code sets conditional
   breakpoints, live-stepping with HRW animations)
4. **Ask** Claude what you're seeing (capture → explain flow)
5. **Debug** additional breakpoints without restarting (mid-session discovery)

This is the full realization of the vision's four-layer system: textbook theory
→ Rumoca code → HRW visualization → Claude explanation, all initiated from a
right-click on a Modelica identifier.

### Considerations

- **Upstream relationship:** this is James Goppert's extension. Extending it
  means either forking or proposing a PR — a different contribution relationship
  than the Rumoca compiler itself. The HRW-specific additions should be designed
  as general Rumoca tooling (useful to any Rumoca user, not just HRW), consistent
  with the upstreaming goal.
- **TypeScript:** the extension is TypeScript, a different language from the rest
  of HRW/Rumoca. Doug would need basic TypeScript familiarity to maintain it.
- **Sequencing:** the curriculum design should come first — it will clarify which
  identifiers, which specimens, and which phases are most important to trace/debug,
  informing the priority of which capabilities to build first.

### Related ideas

- **#10 Cross-stage identifier tracking** — trace-this-identifier is #10 initiated
  from the Modelica source instead of the HRW UI
- **#25 Live breakpoint arming** — the debug shortcut is capability 3, subsumed here
- **#9 Animated algorithm stepping** — debug-this-identifier syncs with live
  algorithm animations

## 30. Live solver stepping via LiveTrace — real-time solver animation

Captured 2026-07-24 (Claude, from Phase 3 planning). The Phase 3 solver stepping
implementation (#29) uses the simpler path: post-hoc data recorded in `SimResult`
and plotted after simulation completes. This idea captures the **richer alternative**:
streaming per-step data via the `LiveTrace` pattern (the same `Arc<Mutex<Vec<F>>>`
buffer used by matching and Tarjan animations) so the solver's progress is visible
**in real time** as the simulation runs.

- **Why it matters:** the matching and Tarjan animations are HRW's most powerful
  teaching tools — watching an algorithm step-by-step, seeing each decision as it
  happens, is qualitatively different from viewing the result after the fact. The
  same applies to the BDF solver: watching the step size shrink in real time as the
  solver hits a stiff region, seeing Newton iterations climb step by step, makes the
  adaptive control theory visceral in a way a post-hoc plot cannot.
- **What it enables beyond #29:**
  1. **Real-time animation:** the solver diagnostics plot updates live as the
     simulation runs, scrolling the time axis forward. For slow simulations
     (BenchActuator with tight tolerances), the learner watches the solver work.
  2. **Pause / step / resume:** using the LiveTrace frame-delay mechanism, the
     simulation can be paused at any step. The learner examines the current state
     (which BLT block is being solved, what the Jacobian looks like, what the
     residual norm is), then advances one step. This is the solver analogue of
     single-stepping through matching's augmenting paths.
  3. **Debugger synchronization:** with `live_trace_breakpoint()` called after each
     solver step, a VS Code breakpoint in the simulation loop would stop at each
     step while HRW shows the corresponding solver state — the same mechanism
     that makes the matching animation debugger-driveable.
  4. **Dual-solver comparison:** run the same model with BDF and RK45 simultaneously,
     streaming both step-size histories to overlaid live plots. The stiffness
     story (BDF cruising with large steps while RK45 collapses to tiny ones)
     unfolds in real time.
- **Implementation sketch:**
  1. Define `SolverStepFrame { t, h, order, newton_iters, residual_norm,
     event_detected, step_accepted }`.
  2. Add `live: Option<&LiveTrace<SolverStepFrame>>` parameter to
     `simulate_state_targets()` in `rumoca-eval-solve/src/sim_driver.rs`.
  3. After each `backend.step()`, push a frame. The frame-delay mechanism
     throttles the push rate for debugger-friendly stepping.
  4. In HRW, the worker thread creates a `LiveTrace`, passes it to the
     simulation, and the UI polls `snapshot()` to update the diagnostics plot
     each frame — the same pattern as `MatchingAnimation::start_live()`.
- **Why deferred:** the simpler post-hoc approach (#29 implementation) delivers
  the core learning value (seeing where the solver struggled) without the
  threading complexity of LiveTrace integration into the simulation path. The
  post-hoc plots are the right foundation — once they exist and prove
  pedagogically valuable, upgrading to live streaming is a well-scoped increment.
- **Depends on:** #29 (the post-hoc diagnostics data model and plots exist first).
- **Relates to:** #9 (animated algorithm stepping — this is the simulation-domain
  instance), #18 (BDF step-size/order — subsumed by #29 + this), #22 (Newton
  convergence — the per-step detail this provides).

## 31. Revisit all simulator functionality when Rumoca's simulator matures

Captured 2026-07-24 (Doug). Rumoca's simulator (diffsol BDF + RK45) is not yet
reliable enough for production use. The compiler pipeline is solid and productive
for learning, but the simulator needs upstream improvement before HRW's
simulation-related features can deliver their full value.

- **What to revisit:** all simulation-dependent functionality — the trajectory
  plot, solver diagnostics (#29), solver stepping (#30), convergence narratives
  (#1), event-time analysis (#22), Jacobian conditioning (#17), and the
  BDF step-size/order study (#18). The infrastructure is in place (instrumentation
  committed, diagnostics plot built) but the underlying solver results may not be
  trustworthy.
- **Interim discipline:** simulate specimens in Wolfram System Modeler as the
  ground-truth reference. Use System Modeler results for learning and trajectory
  comparison. Rumoca's simulation tab remains useful for studying Rumoca's solver
  *behavior* (what it does, even when wrong), but System Modeler is the
  authoritative tool until Rumoca catches up.
- **When:** revisit when upstream Rumoca improves its solver (diffsol upgrade,
  better event handling, or alternative backends). Monitor the upstream issue
  tracker and release notes.
- **Relates to:** #29 (solver stepping), #30 (live stepping), #1 (convergence
  narratives), #17 (Jacobian), #18 (BDF), #22 (events).

## 33. Comprehensive tooltips — surface contextual help across all HRW widgets

**Tours:** all tours

**Problem:** Generic field help is currently delivered as hover tooltips on tree
nodes (the doc strings from `field_help.json`) — instant, zero-click, at the point
of attention. But tooltips are only on tree items — many other HRW widgets would
benefit from the same treatment.

**Idea:** Extend tooltip coverage comprehensively across HRW:
- **Stage tabs** — tooltip explaining what each compiler phase does (one-liner from
  `docs/compiler-phases/` summaries).
- **Spy plot cells/blocks** — tooltip showing the BLT block type, equation/variable
  names, and whether the block is scalar or coupled.
- **Incidence matrix cells** — tooltip showing which equation depends on which
  variable and the nature of the dependence.
- **Equation sheet rows** — tooltip showing the original Modelica source line that
  produced each equation.
- **Simulation plot curves** — tooltip showing variable metadata (state vs output,
  initial value, units).
- **Toolbar buttons and menu items** — tooltip explaining what each action does,
  especially capture/debug actions that have non-obvious workflows.
- **Specimen list entries** — tooltip showing the full `// purpose:` comment and
  model description string.

**Principle:** Tooltips are the "fast tier" of help — instant, contextual, zero-click.
They complement the "explain" chat shortcut (deep, specimen-specific, multi-paragraph)
without replacing it. A tooltip answers "what is this?"; the chat shortcut answers
"why does this have this value in this specimen?"

**Relates to:** #32 (in-app tour view — tooltips handle generic field help, freeing
the UI for richer content like guided tours).

## 34. Sub-view and tree-node navigation links

Captured 2026-07-25. Extend the `hrw://` link scheme with finer-grained navigation:

- `hrw://view/<ViewName>` — switch to a specific sub-view within a stage. For
  example, `hrw://view/SpyPlot` on the Structural tab switches from Tree to SpyPlot.
  Useful for tour steps like "now look at the spy plot to see the BLT blocks."
- `hrw://node/<path>` — expand the JSON tree to a specific node path and scroll to
  it. For example, `hrw://node/classes/GearWithBrake/body/equations` would expand
  and scroll to the equations list. Enables tour steps like "expand equations and
  find the connect nodes."

**Why it matters:** the current `hrw://stage/X` links get the user to the right tab,
but the tour text still says "expand this, scroll to that." Finer-grained links would
make the entire walkthrough clickable — every "look at X" becomes a link that takes
you there.

**Relates to:** #32 (extends the link scheme), #33 (tooltips could use the same
node-path addressing).

## 35. Multiple tour documents and progress tracking

Captured 2026-07-25. Currently HRW embeds a single tour document (the end-to-end
tour). Extensions:

- **Multiple tours:** a tour selector (dropdown or list) to choose between the
  end-to-end tour and per-phase deep-dive tours (e.g. "Structural Analysis Tour",
  "Index Reduction Tour"). Tour documents live in `docs/compiler-phases/` as they
  do now; HRW discovers them by convention or a manifest.
- **Progress tracking:** persistent checkmarks on tour stops the user has visited,
  a "you are here" marker, and bookmarks. Stored per-user (e.g. in a local file
  or egui persistence). Helps the learner resume a partially-completed tour.
- **Tour table of contents:** a clickable outline of the current tour's stops,
  visible at the top of the tour panel. Clicking a stop scrolls to it (using
  `egui_commonmark`'s heading scroll-to feature).

**Why it matters:** as the curriculum grows, a single monolithic tour won't scale.
Phase-specific tours are natural complements to the end-to-end overview. Progress
tracking helps with multi-session learning — Doug may work through the Structural
tour over several days.

**Relates to:** #24 (guided tours as HRW-driven walkthroughs), #32 (the
infrastructure this extends).

## 36. Modelica syntax highlighting in the Specimen source view

Captured 2026-07-25. The Specimen mode LHS source view renders the specimen's
Modelica source as plain monospace text with line numbers. Adding syntax
highlighting (keywords, types, strings, comments, numbers in distinct colors)
would improve readability and match the visual quality of the rest of HRW.

- **Approach:** a simple Modelica tokenizer that classifies tokens into
  categories (keyword, type, identifier, number, string, comment, operator),
  rendered via `egui::LayoutJob` with per-token color. No full parser needed —
  lexical-level highlighting is sufficient.
- **Theme-aware:** colors should respect dark/light mode, consistent with
  HRW's existing palette.
- **When:** *(corrected 2026-07-27)* this entry originally said to do it after
  identifier clicking, assuming the tokenizer built for clickable identifiers
  could be extended to carry colour. **No such tokenizer exists.**
  `identifier_index::clickable_spans` is index-driven, not lexical — it searches
  each line for the leaf names of the DAE variables believed to be on it. The
  dependency runs the other way: the lexer comes *first*, and improves
  identifier clicking (which today finds only the first occurrence of a name per
  line, and only names that survived into the DAE).

**Relates to:** #10 and #37 — sequenced with them as one thread of work in
[`source-tooling-plan.md`](source-tooling-plan.md).

## 37. Reverse identifier tracking — click downstream, highlight source

Captured 2026-07-25 (Doug). The current identifier tracking (#10) flows one
direction: click an identifier in the Modelica source view → highlight all its
downstream mentions across stage views. **Reverse tracking** adds the opposite
direction: click a variable name in *any* downstream view → highlight the
corresponding identifier in the source view.

- **Why it matters:** when studying a stage view (e.g. the incidence matrix or
  equation sheet), the natural question is "where did this variable come from in
  the source?" Today the user must manually scan the source for the declaration.
  Reverse tracking answers that in one click.
- **Feasibility:** high. All highlighting is already driven by one field
  (`tracked_identifier: Option<String>`). Reverse tracking means adding click
  handlers in downstream views that set the same field. The source view already
  highlights the tracked identifier with a gold underline, so that direction is
  free. The `IdentifierIndex` already maps flat names back to source lines.
- **Wrinkle:** some views use derivative names like `"der(h)"` rather than the
  base flat name `"h"`. A small `strip_der()` helper would extract the base
  variable before setting `tracked_identifier`.
- **Highest-value entry points:** equation sheet (click a variable in an equation
  or classification row) and incidence matrix (click a column header). Spy plot
  blocks, tree leaves, and reduction view rows are secondary.
- **When:** after #10 step 4 is complete (all views wired for forward tracking).
  Delivered as Phase 4 of the [source tooling plan](source-tooling-plan.md).

**Relates to:** #10 (cross-stage identifier tracking — this is the bidirectional
extension).

## 38. Syntax highlighting for canvas-painted axis labels

Captured 2026-07-27 (Doug). HRW now renders Modelica text under one rule —
foreground carries syntax, background carries relationship — in the specimen
source view, the equation sheet, and both columns of the Flatten source map
(`source_view::ModelicaText`). Two places still render Modelica as flat,
uncoloured text and so break that rule:

- **Incidence / spy-plot row labels** — equation texts drawn as matrix axis
  labels (`draw_matrix_axis_labels` in `lib.rs`).
- **Tarjan node labels** — equation texts drawn on graph nodes
  (`tarjan_anim::draw_graph`).

- **Why it is not a quick change:** these are `painter.text` calls on a canvas,
  not egui widgets. Per-token colour means laying out each run, measuring it,
  and placing the pieces at computed x-offsets by hand — inside code that
  already handles zoom thresholds, label truncation, and (for column labels)
  -45° rotation. A `Painter::galley` built from a `LayoutJob` gets the colours
  for free and would probably be the way in, since a galley can be positioned
  and rotated as a unit.
- **Worth weighing first:** these labels are small and often truncated at low
  zoom. Colour may add less here than in the full-width views, and truncated
  fragments are exactly where syntax colouring is least meaningful. Consider
  applying it only above the existing label zoom threshold.

**Relates to:** the colour rule recorded in `DECISIONS.md` (2026-07-27) and
[`source-tooling-plan.md`](source-tooling-plan.md).

---

## 40a. Original proposal text for #40, retained for its rationale

*(Delivered — see #40 above. Kept because the reasoning is the record of why it was worth doing.)*

Captured 2026-07-28 (Doug), after a debugging session that failed for reasons
unrelated to the question being asked.

**The question that prompted it.** Following `__pre__.overSpeed` through
MotorWithBrake, the natural next question is *where does that variable come
from?* It appears in the Events IR, in Solve lowering's parameter vector, and
nowhere in the specimen — because `rumoca-phase-dae`'s `lower_pre_operator`
manufactures it. HRW can show the phase's **result** but not its **process**,
which is the exact gap the in-workspace move exists to close.

**Why a debugger is the wrong instrument for it.** Answering the question by
breakpoint took four wrong turns — a stale opt-level, a probe that silently
failed to compile, and finally the recorded windows-msvc finding that CodeLLDB's
PDB reader cannot bind breakpoints in path-dep `crates/rumoca-*` at all. Even
when it works, a debugger answers the question *once*, for one person, on one
machine, and leaves nothing behind. An instrumented phase answers it every time,
for anyone, with no adapter involved — and it is upstreamable, where a debugging
session is not.

**What to instrument.** `lower_pre_operator`
(`crates/rumoca-phase-dae/src/pre_lowering.rs`) has a clean four-beat structure
that maps directly onto animation frames:

1. **Discover** — `collect_pre_targets_from_*` walks equations, conditions,
   clocks, event actions and initialization, finding every `pre(x)`. Frame per
   target found, with the equation it was found in.
2. **Name** — `rumoca_core::pre_slot_name(base)` mints `__pre__.x`. One frame
   per target; this is the moment the variable begins to exist.
3. **Materialize** — `build_pre_parameter` creates it as a **parameter**,
   inheriting the base's shape and start value. Frame showing what was copied —
   this is *why* the slot lands in `P[…]` rather than among the unknowns.
4. **Substitute** — `rewrite_equations` / `rewrite_pre_expr` replace each
   `BuiltinCall{Pre}` with a `VarRef`. Frame per rewritten equation, before and
   after.

**The detail that makes it worth animating:** the pass **runs twice** — once from
`to_dae_with_options`, and again from `finalize_lowered_dae`.

**Corrected 2026-07-29 by the instrumentation itself.** An earlier version of
this paragraph claimed the second pass minted the `__pre__.c[…]` /
`__pre__.load.w` / `__pre__.maxSpeed` cluster. It does not. The real trace on
MotorWithBrake is 21 frames: pass 1 creates all three slots (`load.w`,
`maxSpeed`, `overSpeed`), and pass 2 creates **none**, re-substituting only. The
`__pre__.c[…]` companions come from
`condition_lowering::declare_condition_pre_parameter` — a different pass.

That error is the argument *for* this idea, not against it. It came from counting
`pre_slot_name` calls and **inferring** which pass they belonged to, then
recording the inference as though it were the measurement. A static view supports
exactly that kind of plausible-but-wrong reasoning; a trace does not. And "the
pass ran again and found nothing left to do" turns out to be a real fact about
the algorithm that no static view could have shown.

**Follow-up this turned up:** instrument
`condition_lowering::declare_condition_pre_parameter` too, since that is where
the `__pre__.c[…]` companions actually come from. Same shape, same observer type.

**Shape of the work.** Follow the existing pattern —
`rumoca-phase-structural`'s `LiveTrace<T>` plus a `*_with_trace` entry point,
additive and observation-only, with a `PreLoweringStep` enum and frames carrying
the DAE state. HRW renders it with the same `animation_controls` the matching,
Tarjan and reduction views use, so it inherits play/pause/step, the frame
counter, and live-trace debugging for free. Note `rumoca-phase-dae` already has
`[profile.dev.package] opt-level = 0` (added the same day), so single-stepping it
works once an adapter that can bind there is used.

**Also worth doing while in there:** this is the first *non-structural* crate to
be instrumented, so it will show whether `LiveTrace` genuinely generalises or
whether it has quietly grown structural-phase assumptions. That answer matters
for ideas #19–#22, which propose instrumenting resolve, flatten, event lowering
and the solver.

**Relates to:** `LiveTrace` in `rumoca-phase-structural`, the three animation
views, ideas #9 / #19–#22, and `DECISIONS.md` (2026-07-28) on where
`__pre__.overSpeed` is created.

---

## 41. Claude's teaching database — infrastructure for answering Doug's questions

Requested 2026-07-29 (Doug), after abandoning the end-to-end tour because its
prose was worse than the live conversation, and after establishing that **no part
of HRW needs to work without Claude**:

> Design and implement whatever you need in order to make use of those documents
> and any new documents however would best enable you to answer my questions so
> that I can learn.

**Unusual for this backlog:** every other item builds something Doug looks at.
This one builds something *Claude* reads. Doug consumes it only indirectly,
through better answers.

### The governing rule

`docs/compiler-phases/` is Claude's database (authorship corrected 2026-07-29 —
Claude wrote 100% of it; CLAUDE.md had wrongly credited Doug). What goes in
follows **store what cannot be regenerated**:

- **Store:** Doug's questions, the confusion behind them, what finally made a
  thing click, and decisions with rationale.
- **Do not store:** Claude's explanations of what a phase does. Those regenerate
  on demand, and writing them down builds an echo chamber a later session
  mistakes for an authoritative outside source. This is not hypothetical — the
  `end_to_end_tour.md` Stop 8 failure (describes a 7x7 incidence matrix on a tab
  that shows 48 equations) is exactly this, and Claude nearly adopted the same
  documents as trusted reference without noticing.

### Staging — deliberately incremental

The tour's mistake was building explanatory infrastructure ahead of real use.
Do not repeat it here. Build in this order, and only advance when the previous
stage has enough content to justify the next:

**(A) The question ledger — start immediately, costs nothing.** Per phase (or one
central file, decide when there is enough to retrieve from): date, the question
*verbatim*, the HRW context it was asked in, what unlocked it, concepts touched.
This is the irreplaceable content; everything below is machinery for reading it.

Capture the HRW context from `.hrw-bridge/focus.json`, which already holds the
assembled noun. Over months this answers the question Claude cannot otherwise
ask: *what was Doug looking at when he got stuck?* That is a far better feature
signal than "which tab was opened most".

**(B) A citation checker — cheap, mechanical, do it early.** The docs cite
`crates/**/*.rs` paths and named tests. An example binary (`cargo run -p hrw
--example check_doc_citations`) that verifies every cited path and symbol still
exists. An ad-hoc version of this run on 2026-07-29 found 16 of 17 paths resolve
and one broken, since fixed: a test file that had moved to 
`crates/rumoca-sim/src/solve_lowering/tests.rs`.
Catches the tour's failure mode mechanically, and is the "emitter correct,
reasoner supplements" discipline applied to Claude's own memory.

**(C) Provenance tags — a convention plus a lint.** Every claim marked `verified`
(checked against code/tools, with the file), `cellier` (with a citation), or
`inference`. Only the first two are trusted on re-read. Existing text is
`unverified` and upgrades **lazily**: when a real question sends Claude into the
source, the claims actually checked get promoted. No audit project — the database
becomes trustworthy exactly where it is used most. A lint that reports untagged
claims makes the coverage visible.

**(D) A generated index — defer until (A) has content.** Concept -> where
discussed -> question count -> last verified. **Generated, never hand-maintained**,
or it rots like everything else here has.

**(E) Repeat detection — falls out of (A) + (D).** A rising question count on one
concept is a signal, and it splits two ways that call for opposite responses:
the *concept* is hard (the earlier explanation failed — try a different angle),
or the *thing is not visible in HRW* (a feature request, and a better one than
Claude would invent).

### Deliberately not yet: usage telemetry

Doug raised `session.json`. Today's diagnostics log is a crash artifact with a
rotating ring buffer — built to survive a panic, not to accumulate months of
behaviour. Longitudinal usage would be a separate append-only artifact. Temper
the expectation too: "opened the incidence matrix 40 times" is weak evidence next
to "asked what a dummy derivative is three times". Questions are the signal;
clicks are corroboration at best. Build after (A) has proven what it is missing.

### Why this matters more than it looks

The arrangement makes Claude the author, maintainer, primary reader, *and* judge
of what is true enough to write down — four roles with no independent check.
The mitigations are (1) provenance tags, (2) the citation checker, and (3) most
importantly, **Cellier's problems**, which are the only part of the system where
something outside the loop gets to say Claude was wrong.

**Relates to:** the `hrw-works-with-claude-not-without` principle, `#9`
(animations, for the replay/reveal distinction), `#35` (tour progress tracking —
now largely superseded), `docs/context-assembly.md`, and the retirement of
`end_to_end_tour.md`'s explanatory prose.

---

## 42. Ad hoc tours — HRW as a channel for Claude's *answers*, not just its input

Requested 2026-07-29 (Doug), immediately after agreeing the phase docs are
Claude's database and noticing the specimen notebook has the same problem the
end-to-end tour did:

> Instead of using HRW's tour mode to enable a hard-coded end-to-end tour, I want
> to use tour mode for working through tours which you've created ad hoc in
> response to questions which I ask. […] Sometimes, HRW will provide you a MUCH
> more effective way to answer my questions than by merely emitting text here in
> this chat window.

### Why this is the missing half

The project already has a noun channel *inbound*: Doug assembles context with the
mouse, HRW emits `focus.json`, Claude reasons. But the **answer** has only ever
come back as chat text. Ad hoc tours give the return path the same shape as the
input — Claude can answer with a *sequence of HRW contexts*.

**Design principle: `hrw://` links should express any noun `focus.json` can
describe.** `focus.json` is the noun going out; `hrw://` is the noun coming back.
Same vocabulary, opposite direction.

### The rule that prevents this rotting like the tour did

**A tour is regenerable output; the question is the durable artifact.** So ad hoc
tours are **ephemeral by default**. If Doug wants one again months later, Claude
does not retrieve a stale file — it regenerates against the current tree, correct
by construction. What gets stored is the *question*, which #41's ledger already
covers. This is the same "store what cannot be regenerated" rule, third
application.

### Current state (verified 2026-07-29 by reading `app.rs`)

Tour mode is a left panel rendering **one markdown file `include_str!`'d into the
binary at compile time** (`end_to_end_tour.md`, `app.rs` ~3770), plus clickable
`hrw://` links parsed by `parse_hrw_link` with exactly three verbs:
`load/<Specimen>`, `stage/<Stage>`, `load/<Specimen>/<Stage>`.

So the *mechanism* is close to right. The failure was never the tour concept — it
was static content, bound at build time, with a link vocabulary too coarse to
point at anything interesting.

### Gaps, in order of size-to-value

1. ~~**Runtime loading.**~~ ✅ **DELIVERED 2026-07-29.** Tour mode now renders
   `.hrw-bridge/tour.md`, polled every 250 ms and re-read on mtime change; absence
   shows a short note rather than the retired `end_to_end_tour.md`. Living in the
   gitignored bridge directory makes the ephemerality rule structural rather than
   a discipline. `bridge::read_tour`, `App::poll_tour_file`, `App::no_tour_ui`.
2. **Link vocabulary.** ✅ **Sub-views delivered 2026-07-29.**
   `hrw://stage/<Stage>/<SubView>` and `hrw://load/<Specimen>/<Stage>/<SubView>`,
   with slugs that *are* the capture's own names (`SubView::from_slug` resolves them
   per stage). The parity principle is now enforced by
   `link_slugs_and_capture_names_are_the_same_vocabulary` rather than merely intended.

   ~~**Still below the reach of a link:** an animation frame position, the pointed-at
   node, the followed identifier.~~ ✅ **ALL THREE DELIVERED 2026-07-30.** *(Corrected
   2026-08-01 — this paragraph claimed they were unreachable for two days after they
   shipped, which is how a stale record turns into re-building what exists.)* The
   vocabulary now reaches every noun the capture can describe, **verified against the
   fixture tours that exercise each form**:

   | Noun | Link form | Fixture tour |
   |---|---|---|
   | animation frame | `hrw://stage/Structural/MatchingAnim/frame/41` | `frame-seeking.md` |
   | tree node | `hrw://stage/Structural/Tree/node/blocks[3].unknown` | `node-pointing.md` |
   | followed identifier | `hrw://follow/C.v` | `node-pointing.md` |
   | equation (canvas) | `hrw://stage/Structural/TarjanAnim/equation/11` | `frame-seeking.md` |
   | camera aim | — | `camera-aiming.md` |

   Each tour also drives an **out-of-range** case (`frame/99999`, `equation/999`), so a
   bad address fails visibly rather than silently doing nothing.
3. ~~**Ad hoc specimens — split, do not repurpose.**~~ ✅ **DELIVERED 2026-07-30**, and
   split exactly as recommended. Scratch models live in **`.hrw-bridge/specimens/`**,
   gitignored and therefore **ephemeral by construction** rather than by discipline;
   `App::scratch_specimens` marks them in the list and re-polls the directory, so a
   model Claude writes mid-conversation appears within a second with no restart. The
   curated `specimens/` corpus keeps its properties (portable subset, `// purpose:`
   comments, System Modeler round-trip intent) and is not repurposed. **A scratch name
   may not shadow a curated one** — the collision is reported and the file skipped,
   because silently loading a different model than the name says would have Claude
   reason confidently about source Doug is not looking at.

### The most valuable consequence

**Specimens became a medium of explanation.** "Here is the smallest model that
exhibits the thing you asked about" is what a good teacher does, and until 2026-07-30
it was impossible — Claude could only point at models that already existed. It is now
a normal move (gap 3 above). This feeds the Cellier loop directly: his problems often
specify a system that can be realised as a specimen and actually run through Rumoca,
so a claimed answer can be **checked rather than asserted** — which matters because
Claude being wrong is a false positive on the very test the loop relies on (**#57**).

### Specimen notebook conversion (authorised 2026-07-29) — ✅ DONE 2026-07-29

**All four items below landed.** 1,632 lines of `narrative.md` became 638 lines of
`purpose.md`; `trace/` was kept; the CLAUDE.md authorship claim was corrected. Recorded
here as the plan that was carried out, not as work outstanding.

Doug: *"I expressed the original design for the specimen notebook before I truly
understood the possibilities of this project."*

- **`trace/` — keep.** Generated, correct by construction, and what lets any
  number be checked. This half was always right.
- **`narrative.md` — retire the prose.** Claude's regenerable explanation with
  hand-transcribed numbers that nothing verifies: the same species as
  `end_to_end_tour.md` Stop 8 (describes a 7x7 matrix on a tab showing 48
  equations).
- **Replacement is not prose:** a short record of why the specimen exists (which
  phenomenon it triggers) and which of Doug's questions it was built for or
  answered. Links to the #41 ledger.
- Also fix the second stale authorship claim in `hrw/CLAUDE.md`, which still
  contrasts the notebook with "Doug's *generic* phase theory".

### What the 2026-07-29 animation work revealed about the address space

Doug asked for four more animations *before* this design work, deliberately: an
address vocabulary designed against three animations that all lived on the
Structural stage would have been over-fitted. It was the right call, and these
are the findings, all discovered while wiring them.

**Sub-views are not one thing.** The eight animations sit behind **four
dissimilar enums** — `StructuralView` (shared by the Structural *and* Index
Reduction stages), `EventsView`, `FlattenView`, and `InitView` (created
2026-07-29). A link that says "go to sub-view X" must paper over all four. With
three animations on one stage, the natural design would have assumed a single
enum.

**Not every animation is addressable the same way.** `matching_anim` and
`tarjan_anim` paint on a `Canvas` with pan/zoom; the four newer views are text
and grid panels. "Frame 7" is meaningful for both; **"node 25" is meaningful only
for the canvas ones.**

**Missing capability — camera aiming.** `Canvas` has `request_fit` and
drag-to-pan and *nothing that centres on a given node*. A tour stop saying "watch
what happens at node 25" currently cannot make Doug look at node 25. This is the
single biggest gap for canvas-view tours.

  *Warning attached:* the 2026-07-29 canvas bug (diagram sliding sideways because
  a line of text above it changed the height, and the fit is uniform-scale +
  horizontally centred) shows how fragile that camera is. A tour deliberately
  aiming it will be fighting the same fit logic — see `should_refit` in
  `canvas.rs` and its tests before building this.

**Live and recorded are not addressable alike.** `Animated::position()` returns
`(cursor, total)`, but a live session has no meaningful total — that was a real
bug Doug caught. So "frame 7 of 11" is well-defined only for recorded playback,
and the vocabulary must say so rather than pretend otherwise.

**The replay/reveal split has to reach the link.** A stop on the Tearing view can
legitimately offer to arm the debugger; one on the IC plan cannot, because there
is nothing to trace. If links cannot express that, a tour will show a Debug
button that does nothing — exactly the defect Doug found on the pre-lowering view.

**Do not over-fit to these eight either.** All eight are *compiler-phase*
animations addressing equations and variables. A front-end animation (#19-#22:
resolve, instantiate) would address **source spans and def_ids**. A solver
animation would address **time**, not a frame index. The address space needs at
least three shapes and only one is currently sampled — though see #22 on why the
solver shape is not urgent.

### The unplanned payoff: tours multiply user testing

Doug, 2026-07-29, after the first tour produced #44 on its first use:

> The surface area of HRW had already become more than I could effectively test by
> myself. This ad hoc tour feature mitigates that problem. […] By asking you
> questions, and you providing answers as ad hoc tours, we are multiplying the
> effective user testing of HRW.

**Why it works: different coverage profiles.** Doug navigates where he already
knows to go. A tour navigates where **the question** demands, which can be
somewhere neither party would have visited. #44 is the proof — the tour needed
`Matching ▶` on a singular system, a place Doug could never navigate to *because it
is not there*. **You cannot manually test the absence of a feature you do not know
should exist.**

**Holes in a tour are the signal, so never route silently around one.** The first
tour's Stop 3 is an admission rather than a stop: "I wanted to send you here and
cannot, and here is why that gating is wrong." Doug: *"holes in tours can be useful
ways to identify HRW functionality gaps or bugs."* A tour that hides its gap loses
the finding *and* leaves the reader wondering why the obvious next place went
unmentioned.

**The limit, which must not be over-trusted.** Claude does not *run* HRW — it reads
code and `.hrw-bridge/*.json` and reasons about what the UI will show. So it tests
the **logical** surface (missing sub-tabs, wrong gating, absent data, unparseable
links) and is **blind to the rendered** one (layout, legibility, truncation,
per-frame cost). The 2026-07-29 BLT sideways-drift bug — a diagram sliding because a
text line above it changed height — could not have been found this way; Doug found
it by watching. **Claude tests the logical surface; Doug tests the rendered one.**

**Doug's correction, same day — the rendered surface is covered too, indirectly.**
Claude's "blind to the rendered surface" claim was too pessimistic. Claude cannot
see it, but a tour **aims Doug's attention at it**:

> I'm more likely to experience that kind of bug while being led through one of your
> tours. […] I will always be in user acceptance testing mode while being led through
> your tours.

The mechanism: **a rendered bug is a violated expectation, and a tour supplies the
expectation.** Undirected use does not — a subtly wrong rendering just looks like how
the app is, which is presumably how the BLT sideways-drift survived as long as it
did. A stop that says "watch the stack depth as Tarjan descends" gives Doug something
specific to check, so a drifting diagram registers as *wrong* rather than as *normal*.

So the full loop: Claude composes (logical surface + aims attention) → Doug walks it
in acceptance mode (rendered surface) → bug reported → **fixed in flight if it is
degrading the tour**, rather than only logged → tour restarted clean.

**Boundary on "fix in flight":** yes when the fix is small and the bug is spoiling the
tour (the 2026-07-29 canvas fix was about an hour, including two wrong hypotheses).
No when the honest fix is structural — say so and log it rather than derail a
learning session into a refactor. State which it is and let Doug overrule.

**An unanticipated bonus: a tour's admissions are self-liquidating.** The first
tour's Stop 3 ("I wanted to send you to `Matching ▶` and cannot") exists only until
#44 lands. Fix it, regenerate, and the hole evaporates — because tours are
regenerated rather than stored. A *stored* tour would carry that apology forever and
eventually describe HRW as missing a feature it has.

**And the discipline that keeps it worth anything: the testing stays a byproduct.**
If tour stops get chosen to maximise coverage rather than to answer the question,
the tours get worse and the answers degrade. The coverage benefit is real *precisely
because* it is incidental.

### The discipline this needs from Claude

**A tour is for answers that are irreducibly sequential or spatial.** Most
questions still get two sentences of text. If "what is a dummy derivative?" starts
returning a nine-stop tour, this feature has made Claude worse, not better. The
new medium is not a licence for verbosity.

**Relates to:** #41 (the ledger stores the questions these tours answer), #35
(multiple tour documents + progress tracking — largely superseded by this), #9
(the animation views are the richest tour destinations), `docs/context-assembly.md`
(the noun vocabulary this must reach parity with), and the retirement of
`end_to_end_tour.md`'s explanatory prose.

---

## 43. Three platforms, three questions — Wolfram and System Modeler as answer channels

> **A TRACK, not a sequence step** *(2026-08-01, Doug)*. This was step 4 of the work sequence
> and gated the corpus list. It has been taken off the sequence entirely — see `../CLAUDE.md`.
>
> **Why it was never a dependency.** The conflation was mine: *#43* is mostly a **practice** —
> independence, the division of labour between the three platforms, and the standing *oracle
> first, then Rumoca* rule. What the sequence called "the oracle test" is a sub-part: a
> systematic Rumoca-vs-System-Modeler sweep producing a **report**. Once findings live in
> [`upstream-issues.md`](upstream-issues.md) and the corpus list merely shows a column, that
> sub-part's coupling to HRW feature work is **one column** — which the list handles by
> construction, since it already reads two reports.
>
> **The practice half is already in use** and needs no build: it settled `IncompatibleConnect`
> (upstream issue 2), and two fixture tours route through it.
>
> **Where the value actually is:** Doug's education — an independent implementation that can
> adjudicate, which is exactly why *oracle first* exists as a rule; it corrects Claude's bias
> toward blaming its own specimen. And **upstream**, where
> [`upstream-strategy.md`](upstream-strategy.md) calls differential testing against a commercial
> Modelica implementation *the rarest thing Doug brings*, and something a volunteer project
> cannot cheaply do for itself.
>
> **One constraint survives, because it is free:** *if* an oracle report is ever produced it must
> emit the same `name` join key as the survey and fidelity reports
> ([`reports.md`](reports.md)). That binds the **oracle's** design, not the list's, and
> retrofitting it later would cost the join.

Requested 2026-07-29 (Doug), extending #42 beyond HRW:

> I want you to view HRW as a platform for answering my questions, when you
> believe that your response would be best delivered as something like an ad hoc
> tour instead of as text here in this conversation. Also […] I want you to view
> Wolfram System Modeler and Wolfram desktop app as platforms for answering
> questions where the responses would be best delivered there.

### The point is independence, not extra channels

Claude flagged a structural weakness earlier the same day: under the #41
arrangement Claude is author, maintainer, primary reader **and** judge of what is
true — four roles with no outside check, and said Cellier's problems were the only
mitigation. That was too pessimistic. Two more exist, and both are already
installed:

- **Wolfram Desktop computes.** When Claude claims a block is well-conditioned,
  or a matrix has rank 6, or a Jacobian is singular where the incidence pattern
  says otherwise — that can be *computed* rather than asserted. "Emitter correct,
  reasoner supplements", finally applicable to Claude's own mathematical claims.
- **System Modeler is an independent implementation.** If Rumoca and System
  Modeler disagree about a specimen, one of them is wrong and neither Doug nor
  Claude gets a vote. That is the strongest check in the whole setup.

### The division of labour

| Platform | The question it answers |
| --- | --- |
| **Wolfram Desktop** | What *should* happen — the mathematics, computed exactly |
| **HRW / Rumoca** | How *this* compiler does it — the process, step by step |
| **System Modeler** | What a *mature independent* implementation actually gets |

Three different questions. Not redundancy.

### Verified capability status (2026-07-29 — run, not assumed)

Both are **live today** through the Wolfram MCP tools; this is not aspirational.

- Kernel: **Wolfram 15.0.0 for Microsoft Windows**, responding to
  `mcp__Wolfram__WolframLanguageEvaluator`. `MatrixRank`, `RowReduce`, `Det` all
  work — the linear algebra Cellier-style structural work needs.
- `mcp__Wolfram__WriteNotebook` writes a `.nb` to disk from markdown; `ReadNotebook`
  reads one back. So a notebook is a deliverable answer format.
- **System Modeler is reachable from the same kernel** — `SystemModel` is in the
  `System`` ` context and models load, compile and simulate.

**Working incantations, recorded because they cost four attempts to find.** The
obvious accessor forms fail with `SystemModelSimulationData::urvs`:

```wolfram
sim = SystemModelSimulate[
    SystemModel["Modelica.Electrical.Analog.Examples.CauerLowPassAnalog"], {0, 60}];
sim["StateVariables"]                  (* {"C1.v","C4.v","C5.v","L1.i","L2.i"} *)
v = sim["C1.v", TargetUnits -> None];  (* returns a Function of time *)
v[2.0]                                 (* 0.49473283090381587 *)
```

`sim["ValuesAtTime", t]["var"]` and `sim["ValuesAtTime"[t]]["var"]` both fail —
use `sim["var", TargetUnits -> None]` and apply the resulting function to a time.
Note also that a short horizon can return all zeros legitimately (that model's
step source starts late); check against a horizon where the response is
non-trivial before concluding anything is broken.

**Asking the oracle "does System Modeler accept this model?" (recorded 2026-07-29,
after four wrong attempts).**

```wolfram
Import["C:\...\hrw\specimens\IncompatibleConnect.mo"]  (* -> SystemModel["IncompatibleConnect", True] *)
SystemModelSimulate["IncompatibleConnect", {0, 1}]
(* Head is SystemModelSimulationData -> accepted.
   Head stays SystemModelSimulate + a ::bld message -> rejected, and the
   ::bldl message carries SM's reason. *)
```

Dead ends, so nobody repeats them: `SystemModel[sourceString]` fails (it takes a
model *name*, not source — `Import` is what registers a `.mo` file), and
`SystemModelValidate[name]` returns **unevaluated** rather than a verdict. The
practical oracle is "does it build", read off `SystemModelSimulate`'s head plus its
messages.

### Two consequences

**It unblocks the differential test.** The charter's System Modeler round-trip has
been deferred since Arcs 1-2 as a close-out chore nobody wanted (see #4). Under
this framing it stops being a chore: *"is Rumoca right about this?"* is a
**question**, and System Modeler is where the answer gets delivered. Chores get
deferred; answers do not. The comparison is now mechanical — simulate the same
specimen both ways and diff the state trajectories.

**It is where the linear algebra connects.** #17 wants Jacobian sparsity and
conditioning, and the structural-vs-numerical rank distinction — a matrix can have
full *structural* rank while being numerically singular. HRW can show the
incidence pattern; only Mathematica can show the rank actually collapsing on the
same small system. That connection is currently unmakeable, and it is the
explicit link to Doug's Fall 2026 linear algebra course.

### Disciplines Claude owes, extending #42's

1. **Text is the default.** The medium follows the question's nature. If "what is
   a dummy derivative?" starts returning a notebook, this has made Claude worse.
2. **Never hand over unevaluated Wolfram code as if it were a result.** Claude
   writes Wolfram Language less reliably than Rust, and a notebook on Doug's
   machine looks authoritative whether or not it is right. The evaluator exists —
   use it before delivering, every time. The four-attempt accessor hunt above is
   the argument for this rule, not against it.
3. **The #41 ledger records the medium.** "The tearing animation" and "the rank
   computation" are different facts about how Doug learns, and a ledger that
   drops the medium loses half of "what unlocked it".

### Standing practice: oracle first, then Rumoca

Set 2026-07-29 (Doug), after the failure-path audit produced two upstream bugs:

> Please make full use of that SystemModeler oracle in any way which might help you.
> Likewise for using Wolfram desktop.

**The reason that matters most is a bias, not a check.** Without an independent
implementation, Claude's default when an authored specimen behaves unexpectedly is to
**blame the specimen** — which reads as humility and *systematically destroys findings*,
because every Rumoca bug then looks like a bad specimen. `IncompatibleConnect` failed at
the wrong phase and the tempting move was "my specimen must be wrong, adjust it", which
would have deleted upstream issue #2. **The oracle corrects Claude's error attribution.**

1. **Before concluding anything from Rumoca's behaviour on an authored specimen**, ask
   System Modeler whether it accepts the model (recipe above).
2. **Read SM's message for *which kind* of error it is** — a second opinion on which phase
   *should* catch it. `UnderdeterminedShaft` was authored expecting a structural failure,
   failed at DAE construction, and SM was never asked where it thought the error was.
3. **Interpret the disagreement.** The readings are not interchangeable:

   | System Modeler | Rumoca | Reading |
   |---|---|---|
   | rejects | accepts | **Rumoca bug** — file it (`docs/upstream-issues.md`) |
   | accepts | rejects | **Rumoca bug**, other direction — a valid model refused |
   | accepts | accepts | the specimen is valid Modelica and **tests nothing** |
   | rejects | rejects | a **good failure specimen** — now compare the two diagnoses |

**Wolfram Desktop, same spirit — and one use not yet tried.** Beyond computing claims
instead of asserting them: **derive the ground truth symbolically and check Rumoca against
it.** For a constrained mechanism, work out the constraint equations and the
differentiation index in Mathematica, then see whether Pantelides arrives at the same
answer. That is an oracle for the *mathematics* rather than for another implementation, and
it aims straight at the robotics and linear-algebra goals. Pairs naturally with #5.

### Caveat

The Wolfram MCP connection is interactive-session-bound and may be absent in
headless or scheduled runs. Fine for working sessions; do not build anything
unattended that depends on it.

**Relates to:** #42 (ad hoc tours — same idea, HRW as the channel), #41 (the
ledger), #17 (Jacobian conditioning — the clearest Mathematica use), #4 (the
deferred differential test, now reframed), `user-wolfram-tools` and
`user-linear-algebra-learning` in Claude's memory.

---

## 45. Diagnostic mode — explaining why *Doug's own* model failed

Raised 2026-07-29 (Doug), after the rank-deficiency tour:

> Your ability to leverage HRW when answering my questions means that HRW could have
> production value. For example, I might attempt to compile a specimen which I had
> authored, see that the structural phase had failed for my specimen and then ask you
> to explain the failure in the structural phase.

### Why this is a different question shape

**Educational:** the specimen is known-good and authored to exhibit a phenomenon. The
question is *why does the compiler do this*, and the answer is explanatory. "`f_x[46]`
is unmatched" is a satisfying answer.

**Diagnostic:** Doug wrote the model, it failed, and the question is *what is wrong
with my model*. The answer has to be **actionable and point at his source** — "the
`connect()` on line 23 ties two positions together, so `phi` is over-determined" —
not at an IR node he did not write.

This use case will probably arrive **before** the Cellier problems do. Authoring
Modelica that fails to compile is the normal experience of writing Modelica, and a
robotics student modelling a mechanism will do it constantly.

### Audit of what HRW emits today (2026-07-29, quick pass — verify before relying)

- **Structural singular — ✅ AUDITED AND FIXED 2026-07-29.** *(Bullet corrected
  2026-08-01: it still read "the spans are dropped" while step 1 of the Sketch below
  recorded the fix. Two halves of one idea disagreeing is worse than either being
  wrong, because whichever is read first wins.)*

  `structural_error_to_json` emitted `n_equations`, `n_unknowns`, `n_matched`,
  `rank_deficiency`, `unmatched_equations`, `unmatched_unknowns` and `guidance`, but
  **dropped `unmatched_unknown_spans`** — which `StructuralError::Singular` carries
  precisely so the failure is traceable back to source. Rumoca handed over the
  traceability and HRW dropped it on the floor.

  **Now emitted** as `unmatched_unknown_locations` (line, column, excerpt, and the
  source `line_text`), turning "unknown `emf.p.v`" into "line N of your model". Full
  detail, including the wrong `rank_deficiency` the work uncovered, in **step 1 of the
  Sketch**.
- **DAE construction — ✅ AUDITED AND FIXED 2026-07-29.** It was the worst of the lot:
  the arm returned a bare `Stage::info("flatten succeeded; DAE construction failed
  (later arc)")` **while `error`, `error_code` and `diagnostics` sat in scope, unused.**
  Rumoca says *"unbalanced model: 2 equations, 3 unknowns (balance = -1)"* with code
  `rumoca::todae::ED001`, and HRW said none of it — making the **most common Modelica
  authoring error** the least informative failure in the pipeline.

  Now emits `kind: "dae_construction"` with the message verbatim, the error code, any
  diagnostics, and — parsed defensively — `n_equations`, `n_unknowns`, `balance`, plus a
  `reading` saying which *direction* the imbalance runs (the actionable half). Promoted
  from `info` to a real error, since `last_successful_stage` keys on `note_is_error` and
  flatten was otherwise still counting as the furthest good stage.

  **The counts are parsed from the display string**, because `rumoca-compile`
  stringifies the typed `ToDaeError::Unbalanced { equations, unknowns, balance }` at its
  boundary (`error: format!("{error}")`). The parse yields **absent, never wrong** — a
  reworded message loses the extras and cannot invent a number — and
  `an_unbalanced_model_reports_its_balance` fails loudly if the wording moves.
  **Upstream candidate:** preserve the typed error through that boundary. A compiler
  discarding its own structured error data is worse for every consumer, not just HRW
  (`project-engage-rumoca-community`).

  Test specimen: **`UnbalancedShaft.mo`**, marked DO NOT FIX per #46's convention.

- **Resolve / typecheck / flatten — ✅ AUDITED 2026-07-29.** Three specimens authored to
  break each path (`UndefinedRef`, `DimensionMismatch`, `IncompatibleConnect`, all marked
  DO NOT FIX). The audit found something far more serious than a missing span, plus the
  missing spans.

  **1. A broken specimen poisoned the next compile — FIXED.** Name resolution runs over
  the *whole session*, not the requested model, and a specimen that failed to resolve
  leaves errors in the session's resolved-state cache. Reproduced with a fresh session
  and MSL loaded: `CapacitorLoop` clean, then `UndefinedRef`, then `CapacitorLoop`
  again — and the third compile reported `unresolved component reference: 'missingGain'`,
  a name appearing **only in `UndefinedRef.mo`**, byte-identical to the second run's
  error. In the app: load a broken model, then a good one, and the good one looks broken
  *with the other file's error*. **That is the priority-1 failure** — it would have
  Claude diagnosing the wrong model.

  `remove_document` does **not** clear it, despite
  `apply_document_removal_at_revision` calling
  `invalidate_resolved_state(CacheInvalidationCause::DocumentRemoval)`. Rebuilding the
  session does. Mitigation: rebuild when the previous compile failed to resolve — the
  only mechanism *measured* to work, and guarded so the MSL reparse is paid only when it
  buys correctness. `a_broken_specimen_does_not_poison_the_next_compile` pins it with a
  fresh `WorkerState` so it cannot pass by accident of test ordering.

  **Upstream issue ([#1](upstream-issues.md)):** the root cause is inside Rumoca's resolved-state cache —
  `remove_document` invalidates and yet a stale resolve failure survives. Reproduction
  above; filable for `project-engage-rumoca-community`. Not guessed at here.

  **2. `Diagnostic::labels` was dropped by every diagnostic emitter — FIXED.**
  `rumoca_core::Diagnostic` carries `labels: Vec<Label>`, each a `Span` plus a message
  marking exactly where the error is ("equation assignment here"). Every emitter
  serialized `severity`, `code`, `message`, `notes` and **not `labels`.** One shared
  `diagnostics_to_json(diags, source)` now resolves them through `span_to_location`, used
  by resolve, typecheck, flatten *and* DAE construction — four emitters, one helper,
  three of which previously had their own copy of the serialization.

  A label pointing into a *library* file resolves to `null` rather than a wrong line,
  which is why MSL warnings carry no location and the model's own error does.

  **3. The resolve payload was ~99% MSL noise — FIXED.** `format!("{e:#}")` concatenated
  ~39 items, ~38 of them library deprecation warnings, the model's real error **last**:
  the signal was the final 2% of a 2000-character string.

  Now uses **`compile_model_diagnostics`** — a public, model-scoped API returning real
  `Diagnostic`s — partitioned by **`severity`**. Nothing is pattern-matched out of message
  text, so no wording change can filter away a real error. Measured on `UndefinedRef`:
  **34 diagnostics in, 1 error out** — `ER002 unresolved component reference:
  'missingGain'` at **line 9, column 7**, `y = missingGain * time;`. Warnings are kept but
  deduplicated: 33 collapse to 13 distinct. `message` is still emitted verbatim, so
  nothing is lost.

  `DimensionMismatch` likewise: **line 11**, `small = big;`, `ET002 array dimension
  mismatch: expected [2], found [3]`.

  **4. A specimen landed on the wrong phase — recorded, not tweaked away.**
  `IncompatibleConnect` was authored for the *flatten* path: `connect()` between
  connectors whose member sets differ (`PinA` has `v` and a flow `i`; `PinB` has only
  `v`), which MLS §9.3 makes a type-compatibility error. **Rumoca accepts it** and the
  model instead fails at *structural analysis* as singular.

  Per #46 that is a finding rather than a specimen to keep adjusting. But it is also
  exactly the case that cannot be adjudicated from inside HRW: either MLS permits this and
  the specimen is wrong, or **Rumoca is missing a validation** and this is an upstream
  bug. **System Modeler is the arbiter** — see #45 step 4.

  *(An earlier version of the specimen had the connectors at file scope, making three
  top-level classes, and the reachable-closure pipeline then returned **no result at
  all** — exercising nothing. Nesting them fixed that; the note is in the specimen.)*

### Audit policy (set 2026-07-29, after Doug asked which we were doing)

**Audit narrowly and fix immediately. Never stockpile findings.**

An audit is a mechanical scan of current code, so **it regenerates** — which by the
project's own storage rule (`hrw-works-with-claude-not-without`) means recording its
*conclusions* buys almost nothing and costs staleness. The durable artifact is the
**fix**. And the priority order in `docs/tech-debt.md` says these are *pre-emptive*: the
cheapest moment to close a gap is while nothing is blocked by it.

When the scope is genuinely too large to close in one pass, record **the scope**
("this path is unaudited") and **not the conclusions** ("this path lacks X"). A scope
note is honest about being unverified; a recorded conclusion pretends to knowledge and
is indistinguishable from a verified fact three months later.

**Priority note:** a missing span forces Claude to guess *where* in Doug's source the
problem is, which is priority 1 in `docs/tech-debt.md`'s ordering — above tour holes.

### The part that makes the System Modeler oracle load-bearing

When a hand-authored model fails, there are **two possible causes**: the model is
wrong, or **Rumoca cannot handle it.** Rumoca is not a production compiler. Those two
are indistinguishable from inside HRW, and telling Doug "your model is wrong" when the
truth is "Rumoca is incomplete" would be a confident wrong answer of the worst kind —
he would go and rewrite a correct model.

**#43's oracle is the only way to tell them apart.** Compile the same source in System
Modeler: if it succeeds there and fails here, the bug is Rumoca's. That elevates the
oracle from a nice-to-have to a **requirement of diagnostic mode**, and it gives the
long-deferred differential test (#4) its real motivation.

It is also the path by which HRW starts producing **upstream bug reports** for
CogniPilot, which is a stated goal — a Rumoca-only failure on a model System Modeler
accepts is exactly a filable issue.

### Sketch

1. ✅ **Emit `unmatched_unknown_spans`** — **DONE 2026-07-29.**
   `structural_error_to_json` now emits `unmatched_unknown_locations`: per unmatched
   unknown, its `line`, `column`, `excerpt`, and **`line_text`** — the source line,
   quotable straight back at Doug. `span_to_location` counts newlines and uses
   `from_utf8_lossy`, so a specimen containing non-ASCII cannot panic (an em-dash in a
   description string crashed the lexer this way on 2026-07-27).

   **Verified on `CapacitorLoop`:** unmatched unknown `gnd.p.i` resolves to **line 9,
   `connect(src.n, gnd.p);`** — the physically meaningful line, not merely a line. A
   capacitor straight across an ideal source leaves the ground branch current
   undetermined, and that is where the connect is written.

   **The test specimen is `CapacitorLoop`**, chosen because it fails structurally *and
   stays failed* after index reduction. `MotorWithBrake` and `Drivetrain` are also
   singular but get rescued, so neither is a diagnostic case — the distinction between
   "high-index" and "ill-posed" is exactly what a diagnosis has to make.

   **A wrong number found while doing it, and fixed.** `rank_deficiency` was computed
   from the *incidence passed in* rather than from the error's own counts, and
   `index_reduction_stage` passes the **raw** incidence while its error describes the
   **reduced** system. `CapacitorLoop` therefore reported a deficiency of **7** (14 raw
   equations minus 7 reduced matches) where the truth is 1. Claude would have read that
   and repeated it — the priority-1 failure mode, arriving with data behind it.
   `rank_deficiency_is_consistent_with_its_own_counts` now pins it.

   **Also learned, for step 2's audit:** a model with a genuinely *missing* equation
   never reaches structural analysis. Rumoca catches it at **DAE construction** ("flatten
   succeeded; DAE construction failed"), which is earlier and more specific — good
   compiler behaviour, but it means the most common authoring error of all lands on a
   failure path whose payload has *not* been audited yet. Start step 2 there.
2. **Audit the remaining failure payloads** for source location; add spans where Rumoca
   has them and widen visibility where it does not.

   **This is the only open item in #45, and it is narrower than it was written**
   *(scoped 2026-08-01)*. Five paths are already audited above, so what is left is:

   | Phase | Audited? |
   |---|---|
   | structural singular, DAE construction, resolve, typecheck, flatten | ✅ 2026-07-29 |
   | **parse, instantiate, index reduction, initialization, events, solve lowering, simulation** | **not yet** |

   **Do this through #46, not separately.** #46 authors a failure specimen per phase,
   and a payload cannot be audited without a model that reaches that failure — so the
   specimen is the prerequisite, and auditing the payload is what you do the moment it
   exists. Two tasks with one body of work.

   **Start at DAE construction's neighbours.** The 2026-07-29 audit found that a model
   with a genuinely *missing* equation never reaches structural analysis — Rumoca
   catches it earlier and more specifically at DAE construction. Good compiler
   behaviour, and a warning that the most common authoring errors land on early paths
   rather than the dramatic late ones.
2b. ✅ **Highlight the blamed source line** — **DONE 2026-07-29.** The specimen source
   view tints a blamed line and colours its line number, with a hover saying why. Plus
   `hrw://source[/<line>]`, so a tour can *point* at the line instead of quoting it.

   **The design condition is the interesting part: only a model index reduction cannot
   rescue gets its source blamed.** `MotorWithBrake` is structurally singular too, has
   an unmatched unknown, and has a source line for it — and it is a perfectly good
   model. Painting its `connect()` as a problem would teach the exact opposite of the
   lesson the Structural/IndexReduction contrast exists to teach.
   `only_an_unrescuable_model_gets_its_source_blamed` pins it, including the case where
   an unknown has no span at all (manufactured and solver-vector variables) and so
   contributes no blamed line rather than a bogus one.

   Rendering choices made *because Claude cannot see the result*: the highlight is
   painted **over** the row at low alpha rather than behind it (a `Frame` fill looks
   cleaner but adds margins), and a blamed line is marked by **colouring its line
   number** rather than adding a gutter glyph (which would widen the column and shift
   every line). Both alternatives risk a layout regression, which is precisely the class
   of defect Claude has no way to notice — see `project-tours-multiply-testing`.

3. ✅ **A "why did this fail?" capture** — **DONE 2026-07-29.** `focus.json` gains a
   `pipeline_failure` section carrying the failing stage, its summary, its full `error`
   payload, and the downstream stages that will read as "not reached".

   **The first failing stage, not the current one.** A failure cascades, so the earliest
   error is the cause and everything after it is a consequence. A capture naming
   whichever stage Doug happens to be looking at would routinely name a consequence —
   the wrong answer to "why doesn't this work?". Both askings of the CapacitorLoop
   question were answered by reading stage files directly, because the capture never
   mentioned that anything had failed; it worked only because Doug named the stage
   himself. Someone under deadline pressure says "it doesn't work".

   Absent rather than present-and-empty on a clean compile, so "nothing failed" cannot
   be confused with "the field was not populated".

   **Bug found and fixed while doing it:** `failure_context` walked `StageKind::ALL`,
   which ends with `Simulation` — a tab, not a compilation stage, and
   `StageBundle::get()` *panics* on it. Three existing tests caught it. There is now a
   `StageKind::COMPILATION` list with a comment on the trap, because it is easy to fall
   into and silent until something calls `get`.
4. ✅ **Oracle comparison on demand** — **DONE 2026-07-29**, and it settled a real
   question on its first use. Needed no HRW code, exactly as predicted; what it needed
   was the working incantation, now recorded in #43.

   **First verdict: a Rumoca bug.** `IncompatibleConnect` connects `PinA` (members `v`
   and a flow `i`) to `PinB` (member `v` only). MLS §9.3 requires connected connectors
   to be type-compatible.

   - **System Modeler: REJECTS.** `SystemModelSimulate` fails to build with
     `"Incompatible types. 'a ... 'b' has type 'PinB'."`
   - **Rumoca: ACCEPTS.** The model flattens, and the problem only surfaces later as a
     structural singularity — a misleading diagnosis for what is a type error at the
     `connect()`.

   So the specimen was right and Rumoca is wrong. **Note the validation already
   exists** — `validate_type_compatibility` in
   `crates/rumoca-phase-flatten/src/connections/mod.rs:671`, reached when
   `strict_connection_validation` is on, which HRW sets. So this is not a missing check
   but a check that **did not fire for this case**; the likely suspects are
   `get_validation_var_info` returning `None` for one side, or `canonical_type_id`
   collapsing the two connector types together.

   **Upstream issue [#2](upstream-issues.md)** (the first being the
   resolved-state cache not clearing on `remove_document`). Both were found by auditing,
   both adjudicated rather than guessed, and this one has an independent implementation
   as the witness — which is the strongest form of bug report available.

**Relates to:** #43 (the oracle, now load-bearing), #4 (the differential test, now
motivated), #42 (a diagnostic answer will often want a tour), the priority order in
`docs/tech-debt.md`, and `project-engage-rumoca-community` — Rumoca-only failures are
upstream issues.

---

## 46. A failure specimen + tour for every compiler phase

> ### ⟶ DO THIS BEFORE THE NEXT LARGE FIDELITY RUN
>
> **Doug, 2026-08-05**, on reading the run that finished that morning. It is a gate, not a
> preference, and the reason is measured rather than argued:
>
> **0 of 2,614 rows carried a failure message, and no MSL model produces an empty stage** — five
> partial/abstract classes were tested, one per kind, and every one populated all eleven. So
> **F10's absence clause, the only one of its three that is not near-tautological, had nothing
> to act on.** Its zero covers the provenance clauses and nothing else.
>
> **Absence is a property of failing compiles, and the corpus has none.** Re-running the same
> 2,626 models costs ~8.5 hours and re-confirms the same narrow zero. **This idea is what turns
> that zero into coverage**, and it is small-scale work that lands in the ~90-second pre-commit
> suite rather than needing the watchdog at all.
>
> ### ✅ The first one is done — `OverDeterminedShaft`, 2026-08-17
>
> **Found while converting `dae-construction.md`:** every unbalanced specimen in the corpus
> reported `balance = -1`, so the tour read the sign as informative while only the negative half
> was testable.
>
> **Built the same day.** `SingleInertia` plus `w = der(phi)` — nine lines, and it reports
> `unbalanced model: 3 equations, 2 unknowns (balance = 1)` with the reading *"more equations than
> unknowns — something is determined twice"*. It needed no new tour, only Stop 6 of
> `dae-construction.md`.
>
> **The surplus equation is deliberately consistent**, which is the design rather than an
> accident: it says exactly what an earlier equation said, so there is nothing to contradict — and
> the model is rejected anyway, because the balance check is arithmetic on counts and never asks
> whether the surplus agrees. A contradictory equation would blur *"one too many"* with *"these
> disagree"*, which are different diagnoses needing different fixes.
>
> **What this establishes for the rest of #46:** a failure specimen can be nine lines and can
> retire a whole class of untestable prose. The remaining phases are worth the same treatment.
>
> Recorded in [`fidelity-plan.md`](fidelity-plan.md) ("F10's first corpus run") and in the run
> policy in [`../CLAUDE.md`](../CLAUDE.md).

Requested 2026-07-29 (Doug), after the `CapacitorLoop`/`RcCircuit` contrast tour:

> I could imagine giving you a task of creating a bunch of new specimens and tours to
> demonstrate failure in each compiler phase. I'd bet that you would identify and fix
> gaps and bugs while completing that task.

### Why this works — the mechanism, stated exactly

**A bug is a violated expectation.** A tour supplies Doug's expectation, which is why
he catches *rendered* defects while being led (#42). **Authoring a specimen supplies
Claude's expectation**, which is why Claude catches *logical* ones: writing a model
means predicting where and how it should fail, and a prediction is falsifiable in a way
that reading code is not.

Evidence, all from 2026-07-29 and all found this way rather than by inspection:

| Finding | The expectation that exposed it |
| --- | --- |
| `rank_deficiency` reported **7** where the truth was **1** | an 8/8/7 system must be one short; the emitted number contradicted arithmetic |
| A missing-equation model fails at **DAE construction**, not structural analysis | `UnderdeterminedShaft` was authored expecting a structural failure |
| Claude's "single-nonzero column is a single point of failure" claim was **wrong** | the same column in `RcCircuit` has one nonzero and matches fine |

**Two of those three were Claude's errors, not HRW's.** That is the point rather than a
caveat: Claude's confident wrong answers are the failure mode Doug can least easily
catch, and this task attacks them.

### What to build

A specimen per failure mode, each with a `purpose.md` and a tour that walks its
diagnosis. Known coverage as of 2026-07-29:

| Phase | Failure to exhibit | Status |
|---|---|---|
| Parse | syntax error | to author (trivial) |
| Resolve | unresolvable name | to author |
| Instantiate | conditional / inner-outer problem | to author, shape unclear |
| Typecheck | dimension or unit mismatch | to author |
| Flatten | incompatible connectors (`strict_connection_validation`) | to author |
| DAE construction | unbalanced — a declared variable with no equation | **shape known** (see #45); no specimen yet |
| Structural | singular *and unrescuable* | ✅ `CapacitorLoop` |
| Index reduction | still singular after the funnel | ✅ `CapacitorLoop` (same specimen, later stage) |
| Initialization | over-determined initial conditions | ✅ `OverInitRc` |
| Events | ? | **may be impossible to fail deliberately — finding either way** |
| Solve lowering | ? | same |
| Simulation | divergence / convergence failure | **deferred** — simulator maturity (#22) |

"One per phase" is a coverage sketch, **not a quota.** A phase that turns out to have no
authorable failure mode is a *result*: it means either the phase cannot fail
independently, or its failures are always reported by a neighbour.

### Three things this task must get right

1. **A specimen landing on the wrong phase is a finding, not a failed attempt.**
   `UnderdeterminedShaft` was written to fail at structural analysis and failed at DAE
   construction instead — good compiler behaviour, and a fact about Rumoca that Claude
   would have guessed wrong. **Record it; do not silently retry until the specimen
   lands where predicted.** Silently retrying converts findings into wasted effort.

2. **Failure specimens are corpus liabilities and need marking.** A deliberately broken
   model in `specimens/` invites a future session to "fix" it, and breaks any
   "everything compiles" expectation. They belong in the **curated** set (they are
   durable and deliberate, not scratch — see #42's split), but each needs its
   `purpose.md` and its `// purpose:` line to say **DO NOT FIX** and name the phase it
   is meant to break. `UnderdeterminedShaft` was deleted on 2026-07-29 partly for want
   of this convention.

3. **#43's oracle is a prerequisite, not an extra — and it runs FIRST.** Deliberately
   writing invalid models raises the question every time: *is the model invalid for the
   reason intended, or is Rumoca simply rejecting something valid?* Claude cannot settle
   that from inside HRW, and its default when it cannot is to blame its own specimen —
   destroying exactly the findings this task exists to produce. See #43's "oracle first,
   then Rumoca" for the practice and the four readings of a disagreement.
   Doug: *"in case you doubt your expectations, you have available to you SystemModeler
   and Wolfram desktop to check your specimen."* Compile each specimen in System
   Modeler: a model **SM accepts and Rumoca rejects** is a Rumoca bug and a filable
   upstream issue (`project-engage-rumoca-community`); a model both reject is a good
   failure specimen. Do this **before** writing the tour, or the tour may teach a
   Rumoca defect as though it were Modelica semantics.

### Expected yield

Gaps in the failure payloads (#45 step 2), wrong or missing numbers of the
`rank_deficiency` species, absent source spans on the resolve/typecheck/flatten paths,
and tour holes wherever a diagnosis needs a view that does not exist. Log each as it
appears — in the tour-holes table for HRW gaps, and upstream for Rumoca ones.

**Relates to:** #45 (diagnostic mode — this is how it gets exercised), #43 (the oracle),
#42 (tours, and the curated/scratch split), #4 (the differential test), and
`docs/tech-debt.md`'s priority order.

---

## 47. Cross-platform tours — Wolfram Desktop and System Modeler as tour destinations

Requested 2026-07-29 (Doug), extending #42 and #43:

> Add to the ideas backlog a way to enable you to use Wolfram desktop and SystemModeler
> for tours. You should not be limited to HRW. I want very much to make HRW great. But,
> I do not want to duplicate in HRW functionality that is already in Wolfram desktop and
> that would always be best used in Wolfram desktop. For example, your answer to one of
> my questions might be entirely about a linear algebra concept and might be best
> explained in Wolfram desktop.

### The scoping test this gives HRW

**HRW should only hold what nothing else can hold.**

| Platform | What it uniquely has |
|---|---|
| **HRW / Rumoca** | *This compiler's process on this model.* Nothing else has it. |
| **Wolfram Desktop** | Mathematics — symbolic, exact, interactive, plotted. HRW must never compete. |
| **System Modeler** | An independent Modelica implementation. The oracle. |

**This retroactively rescopes #17** (Jacobian sparsity and conditioning). As written it
implies building a Jacobian heatmap and condition-number display *in HRW*. But
`MatrixRank`, `SingularValueList` and `MatrixPlot` already exist, are exact, and are
interactive, and the structural-vs-numerical rank distinction #17 exists to teach is a
**linear algebra** lesson, not a compiler one. HRW's contribution is the *sparsity
pattern from the real model*; Wolfram's is everything numerical done to it. Revisit #17
under this split before building any of it.

### A notebook already *is* a tour

A sequence of cells evaluated in order is structurally the same artifact as a sequence
of stops. `mcp__Wolfram__WriteNotebook` exists and was verified working 2026-07-29. So
the capability is **present**; what is missing is only that a tour cannot *span*
platforms.

### What to build (small)

1. **Per-stop medium in the tour format.** A stop declares where it happens — HRW,
   notebook, System Modeler — so a single tour can route across all three. Mostly a
   convention in the markdown plus a clear visual marker, not new machinery.
2. **A path a notebook can be handed over at.** Notebooks go in the **gitignored**
   bridge area (`.hrw-bridge/notebooks/`), *not* the repo. **Same ephemerality rule as
   tours** (#42): a stored notebook rots exactly like the retired specimen narratives
   did, and the durable artifact is the *question*, in `docs/question-ledger.md`. A
   plain path in the tour markdown is enough to start — Doug opens it.
3. **Later, optionally:** HRW launching a notebook, and a `wolfram://`-style return
   link. Neither is needed for the first cross-platform tour, so neither should be built
   before one exists.

### Disciplines — extending the medium rule rather than replacing it

- **Text first, always** (the medium rule, `docs/question-ledger.md`), and **one
  non-text medium at a time.** A three-platform answer to a small question is worse
  than a paragraph.
- **The bias risk grows with each platform.** Claude already noted that composing a tour
  is more interesting work than writing a sentence; building a Mathematica notebook is
  more interesting still. The mitigation is unchanged and it is Doug's: he asks.
- **Never hand over unevaluated Wolfram code** (#43). But deliver a notebook whose cells
  **Doug** evaluates — Claude evaluates first to know it works, then ships cells for him
  to run. The first tour's Stop 2 taught this: the stop that landed was the one where
  Doug verified something himself rather than being told it. Pre-evaluated output would
  throw that away.
- **A notebook must not silently become the answer to a compiler question.** If the
  question is about what Rumoca did, the notebook can only supplement — Wolfram has no
  access to Rumoca's internals, and a mathematically elegant answer to the wrong
  question is still wrong.

**Relates to:** #42 (tours), #43 (the platforms, and the verified capability), #17
(rescoped by this), #46 (System Modeler as arbiter for failure specimens),
`user-linear-algebra-learning`.

---

## 48. Get the full gate under one minute — REOPENED AND RESCOPED 2026-08-20

### ⟶ MEASURED 2026-08-21 — THE ANSWER, AND IT IS NOT WHERE THE METHOD LOOKED

**92 % of the gate is 72 compiles and 10 MSL loads. Every compile re-resolves the entire
MSL — 38,855 defs — because the compile path invalidates the session's resolution cache on
every call.** A two-equation specimen that references nothing from the MSL costs **3.5 s**;
the same specimen in a session with no MSL loaded costs **0.03 s**. The model's own work is a
rounding error.

**Doug's ruling on this measurement, 2026-08-21:** *"The 60 second goal is an arbitrary number
which I declared so that we could have a goal. The time reductions which you have identified
are significant and would be much appreciated. We're going to attempt A, B and C."* **So the
acceptance criterion below is a direction, not a contract** — the levers are authorised on
their own merits and the arc is not a failure if it lands at 90 s.

#### The confound that nearly became the finding — read this before trusting any timing here

The first measurement of the day gave `all_healthy_specimens_simulate` at **112.66 s** at
`t_end = 1.0` and **27.22 s** at `t_end = 0.1`, which reads as *integration dominates, 4×*. It
is false. **That was the first MSL load since boot, so ~75 s of it was cold OS page cache.**

**It was caught by a cross-check, not by suspicion:** the same test measures **31.3 s inside the
full suite**, and a test cannot cost 31 s in company if integration alone costs 85 s. Re-run
warm, the pair collapses (below).

**So the standing rule for this item: never compare a first-of-session run with a later one, and
sanity-check any isolated timing against the same test's in-suite figure.** The suite pays the
MSL's disk cost once; an isolated run pays it too, and attributes it to whatever test is running.

#### `t_end` IS DEAD — point 3 of the agreed method is spent

| `all_healthy_specimens_simulate`, warm | time |
|---|---:|
| `t_end = 1.0` | 37.75 s |
| `t_end = 0.1` | **37.33 s** |

**Integration is free.** Cutting `t_end` buys 0.4 s on the suite's largest simulating test.
Corroborated independently by the accounting: `simulate` averages **3.13 s** against
`compile_target`'s **3.50 s**, so a simulate costs *less* than a compile and the integration
inside it is invisible.

**This is the fifth lever to die on contact with a clock**, and the first that Doug and Claude
had *agreed on in advance*. The non-vacuity assertions point 3 asks for are therefore **not
owed** — there is nothing to pay for. Keep the rule itself (name the phenomenon a simulation
test needs) if `t_end` is ever cut for another reason; `BouncingBall`'s bounce is still the
case that would break silently.

#### Where the gate goes — instrumented counters, one full run

Measured by wrapping four call sites in a scope that accumulates count and wall time per
process. Run total **315.11 s**.

| bucket | calls | total | mean |
|---|---:|---:|---:|
| `compile_target` | 55 | **192.7 s** | 3.50 s |
| `simulate` (its own path — it does **not** call `compile_target`) | 17 | **53.2 s** | 3.13 s |
| `load_libraries` (a full MSL load) | 10 | **43.8 s** | 4.38 s |
| *of which* `strict_compile_resolved`, *inside `compile_target`* | 52 | *62.4 s* | *1.20 s* |

**72 compiles + 10 MSL loads = 289.7 s of 315.11 s.** The rest of the suite is noise: the
`--report-time` census puts **756 tests under 1 s sharing 14.3 s**, against **40 tests carrying
323.5 s**.

#### Why one compile of a two-equation model costs 3.5 s

HRW's own log stream, compiling `SingleInertia` (2 equations, zero MSL references):

| step | time |
|---|---:|
| Parse | 0.7 ms |
| **Resolve** | **1,600.2 ms** |
| **Rumoca compile — full pipeline** | **1,095.2 ms** |
| Structural → Index reduction → Initialization → Events → Solve lowering | ~20 ms |
| total | 2,736.6 ms |

**And the control that settles it: the same file, in a `WorkerState` that never loaded the MSL,
compiles in 0.03 s.** Both halves of the cost are proportional to the loaded library, not to
the model.

#### The mechanism, pinned by calling the resolver directly

| call | time | session state |
|---|---:|---|
| resolve #1 | 1.24 s | cold |
| resolve #2, #3 | **0.00 s** | nothing changed |
| resolve #4 | 1.61 s | after adding one workspace document |
| resolve #5 | **0.00 s** | unchanged again |
| resolve #6 | **1.59 s** | after `remove_document` + `update_document` of **byte-identical** text |

**Rumoca's resolution cache is correct and complete.** What costs the time is that **any**
workspace-document change invalidates the whole resolved library — and `compile_target`
deliberately performs `remove_document` + `update_document` on every compile, because
`update_document` short-circuits on identical source and the registration code would never
re-run. The comment at that site explains the *correctness* need honestly; nothing there was
wrong. **The interaction is the defect, and neither half looks like one on its own.**

**This is a candidate upstream question, not a defect claim** (`docs/upstream-issues.md`
discipline): *should a change to a workspace document invalidate resolution of durable external
source roots?* It reproduces in six lines and is exactly the shape `upstream-strategy.md` calls
a zero-cost gift. **Adjudicate before filing.**

#### The levers, with what each is measured to be worth

| # | lever | worth | status |
|---|---|---:|---|
| **A** | Stop invalidating the resolved MSL on every compile | ~1.6 s × 72 ≈ **115 s** | **authorised 2026-08-21** |
| **B** | Compile MSL-free specimens in a bare session | **49.5 s → 0.5 s** over 16 specimens | **authorised 2026-08-21** |
| **C** | Reduce the 10 full MSL loads | up to **44 s** | **authorised 2026-08-21** |
| **D** | Cut `t_end` | **0 s** | dead, see above |

**B carries a caveat that must not be lost.** 16 of the 24 specimens reference nothing from the
MSL. Compiling all 16 both ways and comparing every stage's JSON byte-for-byte: they **differ**,
but the *only* difference is **DefId numbering** — `def_id: 89` against `def_id: 86`, at
identical byte lengths (21,734 vs 21,734; 15,413 vs 15,413) — because an MSL-loaded session
allocates more DefIds before reaching the specimen. Semantically the same compile. **DefIds are
observable**, in the pane and in the committed notebook traces, so B is a trace-regenerating
change and `--features notebook-check` is part of its gate.

#### Run-to-run variance is large, so judge by counters and not by wall clock

The same suite measured **315 s, 338 s and 412 s** on one machine in one afternoon, with no
source change between two of them. **Any claimed improvement must clear ~15 % to mean anything.**
Prefer the instrumented counts (compiles, resolutions, MSL loads) — they are exact, and a lever
that removes 30 resolutions has demonstrably worked whatever the clock says.

#### How these numbers were produced, so they can be reproduced

Four temporary probes, none committed (`CLAUDE.md`: *a probe lives in the working tree until it
earns permanence*):

1. A scope guard accumulating `(count, nanos)` per bucket into a `BTreeMap`, rewriting a file on
   every sample, dropped into `compile_target`, `simulate`, `load_libraries` and around
   `strict_compile_resolved`.
2. A test timing MSL load, repeat compiles of the same specimen, and a compile in a session with
   **no** libraries loaded — the control that made the finding unambiguous.
3. The same test calling `session.strict_compile_resolved()` directly, with and without document
   churn between calls.
4. A test compiling every MSL-free specimen in both session kinds and diffing the stage JSON.

**`--report-time` needs `RUSTC_BOOTSTRAP=1`** on this stable toolchain — the harness gates it on
nightly, and without that the run dies after the build with a bare `error:` line.

**One caution for whoever adds a probe to `worker.rs`:** it changes the file's line count, so
`doc_citations::architecture_regions_are_current` fails for the duration. That failure is the
probe, not a defect — but do not let it mask a real one.

### ⟶ THE AGREED METHOD — Doug, 2026-08-21, superseded in part by the measurement above

**Points 1, 2 and 4 stand. POINT 3 IS DEAD** — see *`t_end` is dead* above. Kept unedited
because point 1 is what produced the answer, and because the method being *right* while its
named lever was *wrong* is the item's most transferable lesson.

**The `app.rs` split is finished, so this is the arc in flight.** Gate measured **~285–354 s**
on 2026-08-21. Doug's framing: *"that test time is subtracting from learning time."*

**1. MEASURE BEFORE COMMITTING TO A LEVER — the ten-minute experiment comes first.** Cut `t_end`
on a single simulation test and time it before and after. That settles whether **integration** or
**compilation** dominates, which is genuinely open: `all_healthy_specimens_simulate` (16 s)
compiles nine specimens before simulating any, and the two next-slowest tests
(`every_stage_serializes_without_panicking` 15 s, `a_rumoca_failure_is_represented_faithfully`
14 s) **do not simulate at all.** Four levers already died here after being proposed from
arithmetic over slow-looking test names — **a sum of names is not a measurement**, and one of the
four was proposed by Claude and withdrawn on measuring.

**2. COST REDUCTION RANKS ABOVE SELECTIVE EXECUTION.** The failure modes are not symmetric: a
test made cheaper still runs, while a test skipped by a wrong selection heuristic is a **silent
wrong negative** — the error this repository treats as the one nobody catches, because acting on
it means *not looking*. The safe forms of selection already exist (`slow-tests`, the FAST/FULL
table, `compile_specimen_shared`). Reach for more only where a test cannot be made cheap.

**3. CUT `t_end`, AND PAY FOR IT WITH A NON-VACUITY ASSERTION PER TEST.** Doug: for current
purposes 0.1 s of simulated time is as useful as more. **The exception is any test asserting a
PHENOMENON rather than that integration ran** — and `BouncingBall` is exactly that. A bounce is an
**event**, so `has_discontinuities`, `discontinuity_segments` and the *"discontinuities render as
discontinuities"* claim all require one to occur inside `t_end`. **Cut below the first bounce and
every one of them passes while checking nothing.** So: name the phenomenon each simulation test
depends on and assert it. That converts `t_end` from a number nobody dares touch into one anyone
can tune, because going too far then fails loudly by name.

**4. CHANGE `t_end` AT THE CALL SITE, NEVER IN A SPECIMEN'S `experiment` ANNOTATION.** Those
annotations are part of the System Modeler differential-test contract — identical solver
tolerances and initial conditions (charter §4.3). `t_end` is already a parameter to `simulate`,
so this costs nothing and keeps the protocol intact.

**A CANDIDATE COST NEITHER LEVER REACHES, and it is worth measuring early.** The suite is forced
to `--test-threads=1`, and the expensive tests serialise on a global `Mutex<WorkerState>`. **They
are serial precisely because they SHARE the expensive resource — the loaded MSL — and sharing is
what makes them individually cheap.** So the dominant cost may be MSL loading, which neither
`t_end` nor selective execution touches. **Measuring that is free. Acting on it is not:**
`CLAUDE.md`'s compile-path prohibition stands, revisable only on evidence **brought to Doug**, and
a session must not read its own measurement as authorisation. Splitting `worker.rs` into modules
is *not* a compile-path redesign; changing how the MSL session is loaded, cached or shared **is**.

---

**Status: LIVE, and it is the next arc after the `app.rs` split finishes.** Split out
2026-07-29 as *"memoize compiled specimens"*; **that sketch shipped** (see *Already
delivered* below), so this number now carries the goal rather than the mechanism. The
history is kept at the foot of the item because two of its conclusions are still binding
and one is now **wrong** — see *What the 2026-08-20 measurement overturns*.

### Doug's ruling, 2026-08-20 — this is a failure mode, not an optimisation

> *"As we've added more tests, our test run time has increased. At this point, I'm
> spending more time awaiting the completion of test runs than adding features or
> learning. So, that means that this project is in failure mode right now."*

**That is charter Decision 9 firing, and it outranks the item's old cost/benefit.** An
accurate instrument that costs attention to operate *spends the attention meant for
learning* — and Doug's education is the purpose the whole repository derives from. A gate
that consumes the session it is protecting has stopped being a safety net.

**THE 2026-07-29 RULING IS EXPLICITLY REVERSED.** This item used to read *"Doug ruled out
the concurrency work on that basis… **Do not revisit it**: the cost is high, the machine
has limited memory, and the return is two seconds."* He has reopened it in terms:
*"I'm very ready to reconsider #48."* The old ruling was correct **on the evidence it
had** — it priced concurrency against a two-second saving. It never priced a suite four
times longer, and it never priced the multi-hour deadlock that the same process-global
state caused on 2026-08-19 (twice) and 2026-08-20.

**Two motives now point at the same work**, which is the strongest argument this item has
ever had: **de-globalising `OutputCapture` and `focus.json` is simultaneously the
parallelism enabler and the permanent fix for the hang** that `DECISIONS.md` (2026-08-20)
currently papers over with `RUST_TEST_THREADS = "1"`. That setting is a seatbelt, not a
repair.

### The goal, stated as an acceptance criterion

**A full pre-commit gate in under 60 seconds**, where "full gate" is what `CLAUDE.md`
requires before a commit that touches `src/`:

```text
cargo test -p hrw --lib --features slow-tests
cargo test -p hrw --test msl_resolve --features slow-tests
cargo clippy -p hrw --all-targets
```

**Today that is about 277 seconds.** The target is a **4.6x cut**, and it is a number to
be measured after each step rather than argued about.

### Already delivered — do NOT re-implement these

| shipped | what it did |
|---|---|
| the `slow-tests` feature gate (2026-08-01) | took the *between-edits* loop from 183 s to 7.3 s |
| **specimen memoization** — `compile_specimen_shared` + `specimen_cache()` in `worker.rs` | this item's original sketch: compile each specimen once per test process, hand out clones |

**The memoization is live**, so today's cost is **not** "the same specimen compiled over
and over". Anyone reading the old sketch below and reaching for a `HashMap` is
re-implementing something that is already there.

### The measurement, 2026-08-20 (770 tests, single-threaded, this machine)

| run | time | tests |
|---|---|---|
| `--lib` (no `slow-tests`) — the between-edits loop | **24.9 s** | 680 passed, 90 ignored |
| `--lib --features slow-tests` | **253.5 s** | 769 passed, 1 ignored |
| `--test msl_resolve --features slow-tests` | 7.5 s | 2 passed |
| `cargo clippy -p hrw --all-targets` | 16.2 s | — |
| **full gate** | **~277 s** | |

**The concentration is extreme, and it is the most useful fact here:**

| | tests | time |
|---|---:|---:|
| tests taking **>= 1 s** | **41** | **316.4 s** |
| tests taking **< 1 s** | **728** | **12.9 s** |

*(Per-test totals sum to 329 s under `--report-time`, which adds its own overhead; the
clean wall-clock is 253 s. Use the ratios, not the absolute sums.)*

**Where it sits, by module:**

| module | time | tests |
|---|---:|---:|
| `worker` | **205.8 s** | 103 |
| `fidelity` | **62.2 s** | **7** |
| `equation_sheet` | 32.6 s | 16 |
| `doc_citations` | 14.8 s | 33 |
| `ui_tests` | 9.0 s | 53 |
| `app` | **0.4 s** | **140** |

**The five worst individual tests:**

| test | s |
|---|---:|
| `worker::all_healthy_specimens_simulate` | 33.2 |
| `fidelity::a_rumoca_failure_is_represented_faithfully` | 27.8 |
| `worker::a_broken_specimen_does_not_poison_the_next_compile` | 24.4 |
| `fidelity::f10s_absence_clause_is_exercised_by_a_partial_class` | 16.6 |
| `fidelity::every_stage_serializes_without_panicking` | 14.5 |

### What the 2026-08-20 measurement OVERTURNS

1. **"#48 stays scoped to the existing 12-specimen suite" is now WRONG.** `fidelity` is
   **seven tests holding 62 seconds** — 19% of the run — and this item explicitly scoped
   *away* from it (*"Do NOT reach for this for the fidelity sample"*). That note was about
   **memoization** being the wrong tool there, and it stands; but it was written when
   fidelity was small, and it cannot be read as "fidelity is out of scope for making the
   gate fast." **The harness shape that note recommends is exactly the lever those seven
   tests need**, and it is now worth ~62 s rather than a rounding error.
2. **The two worst offenders are not compiles at all.** `all_healthy_specimens_simulate`
   (33.2 s) and `an_msl_library_model_simulates` (13.2 s) are **simulations**. Compile
   memoization cannot touch them; nothing caches a simulate result today.
3. **About 35 seconds is deliberately un-memoized**, and it is the two tests this item
   itself named as opt-outs: `a_broken_specimen_does_not_poison_the_next_compile`
   (24.4 s — its whole subject is cross-compile contamination) and
   `compiling_a_specimen_twice_is_reproducible` (10.5 s — the mitigation the caveat below
   asked for). **They are anti-cache by design and must not be "optimised" into
   uselessness**; the question for them is *which gate they belong in*, not whether to
   cache them.

**And a fourth fact that is not a lever but sets expectations:** the `app` module's **140
tests cost 0.4 seconds total.** The `app.rs` split is free at runtime. Nothing about this
item argues against finishing it first.

### Levers as they stood BEFORE the 2026-08-21 measurement — kept for the reasoning, not the ranking

**Superseded by *The levers, with what each is measured to be worth* above.** Read this list
for what it says about parallelism and memory, which is still true; do **not** pick work from
it. Its ranking was written without knowing that a compile's cost is MSL re-resolution, and
lever 2 (cache simulation results, "worth ~46 s") is now known to be worth **nothing** — the
46 s it aimed at is compilation, not integration.

**No lever gets chosen before its number exists.** This item was once justified by
arithmetic over test names and the arithmetic was wrong; `CLAUDE.md` records the rule —
*a sum of slow-looking names is not a measurement.*

1. **Parallelism — the reopened one.** Blocked on three globals: `OutputCapture`'s
   process-wide fd 1/2 hijack, `focus.json`'s fixed path, and `shared_worker()`'s global
   `Mutex<WorkerState>` (Rumoca's `Session` is not thread-safe). The first two are also
   the hang. **Measure first: resident memory of one MSL-loaded `Session`.** That single
   number decides whether N workers is possible on this machine, and it is the number the
   2026-07-29 ruling leaned on without ever recording.
2. **Cache simulation results, not just compiles.** Worth ~46 s on the two simulate tests
   alone. **Measure first: how many distinct simulate calls the suite makes, and whether
   their outputs are `Clone` the way the compile payloads were.**
3. **Harness-shape the seven `fidelity` tests.** This item's own table already shows
   one-harness (compile once, apply every invariant, drop) beating per-test compiles on
   both time *and* memory. **Measure first: how many models those seven traverse and how
   much overlap there is.**
4. **A tier below `slow-tests`.** The deliberate opt-outs and the MSL-wide checks may not
   belong in *every* pre-commit gate. **This trades safety for speed and is Doug's call,
   not Claude's** — it is listed so it is considered, not assumed.
5. **Build and link time, which none of the above touches.** 277 s is test *execution*;
   the gate also pays compilation. **Measure first: a warm incremental gate's build share
   versus its run share.**

**A caution worth stating up front: parallelism alone probably cannot reach 60 s.** 316 s
of concentrated work across 4 threads is ~79 s before any coordination overhead, and 8
MSL-loaded sessions is exactly the memory the old ruling feared. **Expect to need work
*reduction* as well as work *spreading*** — which means levers 2 and 3 are not optional
extras behind lever 1.

### The caveats that survive, unchanged

**Memoizing weakens the suite.** The second test to ask for `Drivetrain` no longer
verifies that compiling it is *reproducible*. The mitigation shipped —
`compiling_a_specimen_twice_is_reproducible` — and **any new cache owes the same debt**:
keep one test that does the uncached thing and compares. This is the silent-coverage-loss
class that `project-tours-multiply-testing` warns about, a detector that quietly stops
detecting.

**The machine has limited memory**, which is why every lever above is paired with a
memory question rather than only a time one.

### First step for whoever picks this up — ✅ DONE 2026-08-21, and this is what it left

**The measuring phase is complete.** The three numbers this section asked for are answered or
retired: distinct compiles and MSL loads are counted above (72 and 10); the build-versus-run
split is answered by the counters (289.7 s of 315 s is run, and a warm rebuild is ~19 s); and
**resident memory of one MSL-loaded `Session` was never needed**, because parallelism is no
longer the lead lever — work *reduction* is, and it does not turn on that number.

**The order of work, Doug 2026-08-21: A, then B, then C.** Taken in that order because A is the
largest, is the one that makes every other compile cheaper, and is the one whose blast radius
must be understood before B changes which session a test uses.

- **A — do not restructure the compile path to get it.** `CLAUDE.md`'s prohibition is on a
  *redesign*; the change wanted here is narrow, and the honest form of it is *"do not invalidate
  when nothing changed"*, not *"re-plumb how specimens are registered"*. If the narrow form
  cannot be made correct — remember the site's comment names a real poisoned-cache failure — that
  is a finding to bring back, not a licence to widen.
- **B — its gate includes `--features notebook-check`**, because DefId renumbering is visible in
  committed traces. Budget the 157 s and the `gen_trace --all` regeneration that follows.
- **C — identify which of the 10 loads are deliberate.** At least two tests are anti-cache **by
  design** (`a_broken_specimen_does_not_poison_the_next_compile`,
  `compiling_a_specimen_twice_is_reproducible`) and must keep paying. The question for those is
  *which gate they belong in*, never whether to cache them.

**And the debt every one of these owes**, unchanged from *The caveats that survive*: a cache that
makes a test cheaper also stops it detecting what it used to detect. **Keep one test that does
the uncached thing and compares.**

---

### History — the 2026-07-29 framing, kept because two conclusions still bind

**What the original measurement said** (402 tests, before the suite grew to 770):

| | |
|---|---|
| 49 tests taking over 1s | **180.3s** |
| The other 353 tests | **~2.7s** |
| Of those 49, worker tests | **47** |

Every worker test acquires `shared_worker()` — a global `Mutex<WorkerState>`, needed
because Rumoca's `Session` is not thread-safe and because loading the MSL once is worth
a great deal. **So they serialize regardless of `--test-threads`.** Going parallel would
have taken 183s to roughly 181s. **That reasoning is still correct about the mechanism**
— it is the *conclusion drawn from it* that 2026-08-20 reopened.

**The original sketch, now shipped:** a `OnceLock<Mutex<HashMap<String, FromWorker>>>`
beside the shared worker; compile each specimen once per test process, hand out clones.
Tests needing a genuinely fresh compile opt out explicitly via the uncached path.
Estimated 180s to 60-70s at the time.

**STILL BINDING — do NOT reach for memoization for the fidelity sample (2026-07-31).**
Claude recommended it after F1 took 148s, then withdrew it the same hour. The 148s came
from *structure*, not from missing memoization: three F1 checks each looped over the same
ten specimens, so ten models cost thirty compiles. Memoization rescues that shape — but
it is a shape not worth choosing.

| | 9 checks x 50 models | memory held |
|---|---|---|
| Separate tests | 450 compiles | one model |
| Separate tests + #48 | 50 compiles | **all 50 at once** |
| One harness (compile once, apply every invariant, drop) | 50 compiles | one model |

The memoized payload is the *entire* compiled state — `Media.Examples.WaterIF97`'s
flatten stage alone was 3.2 MB. Trading memory for time at 40-60 MSL models runs the
trade backwards. **This is why lever 3 above says harness, not cache.**

**Relates to:** the `slow-tests` gate in `Cargo.toml`, `README.md`'s two test commands,
`shared_worker()` and `compile_specimen_shared()` in `worker.rs`, and the
`RUST_TEST_THREADS` note in `.cargo/config.toml` — the seatbelt this item is meant to
make unnecessary.

---

## 49. A narrow fixture tour per HRW feature

Doug's plan, 2026-07-30, after one fixture-tour walk produced four bugs:

> I'm the bottleneck for UI testing. In particular, my ability to focus on expected
> results during UI testing is the real limiting factor. Narrowly scoped tours are more
> aligned than broad tours with my human focus limitation. So, I envision us implementing
> a bunch of narrowly focused tours, each targeting an HRW feature.

### Why narrow, and why Claude was wrong about it

Claude proposed the opposite — *wider* tours — on noticing that **half** of that walk's
bugs came from outside the stops (Tour mode's wrong empty-state message, found by starting
HRW and clicking nothing; and the stage side not resetting between tours, which lives in
the gap *between* tours). That read the evidence backwards.

Those two were found **because the tour was short enough to leave attention to spare.** A
wider tour spends that surplus on more stops: it consumes what produced the off-stop
findings rather than multiplying them. **The scarce resource is Doug's attention per
expectation, not the number of walks.**

Two further arguments, less obvious:

- **A failed stop in a narrow tour implicates one feature.** In a wide one a stop can fail
  for reasons unrelated to its subject, and Doug ends up triaging instead of testing.
- **Claude authors a narrow tour while it still knows what should happen** — right after
  building the thing. Both tour errors found on 2026-07-30 ("mostly collapsed"; a
  highlight asserted before it existed) were written about behaviour Claude had *not* just
  built.

### The rationale for the whole scheme

These cover **what no test can reach**: every one of that walk's four bugs was HRW being
internally consistent and *wrong about what it should do*. A test encodes Claude's model of
correct behaviour — the same model that produced the bug — so it cannot find a fault in it.
A tour states the expectation in prose Doug reads against reality.

### Coverage today: 3 of roughly 20

Have one: **camera aiming**, **frame seeking**, **node pointing**.

Have none — a rough enumeration of the surface, to be firmed up when the work starts:

| Area | Features wanting a fixture |
|---|---|
| Modes | Tour, Specimen, Debug — and the transitions between them |
| Specimen view | source syntax highlighting, identifier click-to-follow, blamed-line highlight, the Purpose tab |
| Stage views | IR tree, stage diff highlighting, per-stage error summaries |
| Structural | incidence matrix, BLT spy plot |
| Animations (8) | matching, Tarjan, tearing, alias, reduction, `pre()` lowering, IC plan, connections — *and* their shared playback controls |
| Context Bar | point-at, follow, clearing, the composition of both |
| Other panes | log view, equation sheet, source map, simulation plot |
| Live trace | arming, stepping, the debugger handshake |
| Tour mode itself | the tour list, ad-hoc-vs-fixture, switching |

### What the suite needs past ten or so fixtures

Neither is worth building at three.

1. **A selection principle.** Doug cannot walk twenty. Suggested: *the tour for whatever
   just changed, plus one stale one* — regressions caught immediately, coverage swept
   slowly.
2. **Visible staleness, and this is the one that matters.** Nothing currently catches a
   tour whose **expectations** rot; `fixture_tour_links_all_resolve` checks only that its
   links parse. "Mostly collapsed" was wrong for weeks with every test passing. At three
   fixtures Claude can eyeball them; at twenty-five it cannot.

   **Nearly free already:** every `tour-link` click is in the action trail, so "last
   walked" is derivable from data HRW already writes. Showing it in the tour list makes
   "this covers a feature changed since it was last walked" visible at a glance.

**Relates to:** #42 (fixture tours as an artifact, and the ephemerality rule that exempts
them), `project-tours-multiply-testing` in Claude's memory, and `hrw/CLAUDE.md`'s
fixture-tour rules — including that every `**Expected:**` line must be **violable**.

---

## 50. ~~Measure test code coverage~~ — CONSIDERED AND DECLINED

Raised by Doug 2026-07-30, examined, and **not pursued**. Recorded so it is not proposed
afresh, and so the reasoning is available if circumstances change.

### The evidence that settled it

Fourteen defects found on 2026-07-30, classified by whether coverage could have found
them — that is, "code that exists, no test executes it, and it is wrong":

| Category | Count | Found by coverage? |
|---|---|---|
| Missing code — a guard or feature that did not exist | 5 | **No.** Coverage cannot measure absent code |
| Executed code doing the wrong thing | 5 | **No.** All ran every frame |
| Documentation and tour text | 2 | No |
| A `#[test]` attribute silently lost | 1 | No — the clippy count caught it |
| Claude's own new tool being wrong | 1 | No |

**Zero in the only category coverage detects.**

### Two reasons it is structurally misleading *here*

1. **`app.rs` is ~9,000 lines of egui paint closures Claude cannot drive.** Coverage would
   report them uncovered, correctly, and unactionably — their real testing mechanism is
   the fixture tours, which no coverage tool can see. The figure would be permanently
   depressed by design.
2. **Coverage measures execution, not verification.** A line run by a test that asserts
   nothing counts as covered. A coverage *target* would push toward the tests that raise
   it cheapest, which are exactly the vacuous ones this project keeps having to catch.

### The carve-out that defeated itself

Claude wanted one measurement to check whether its frequent *"test-verified"* claims are
honest. Coverage answers that **badly**, for the reason just given. The instrument that
actually answers it is **stating the verification boundary explicitly** — "this is
test-verified, that is not, and here is which" — which is already standard practice on
every commit. A percentage would be a worse answer to the one question it was wanted for.

### The honest limit of this decision

The sample is biased: 2026-07-30 was **new-feature work**, where missing-code bugs
dominate. Coverage's category — *old code, never exercised, silently wrong* — is a slow
failure mode a one-day sample cannot show, and there is some evidence it is live here (a
`#[test]` attribute vanished, the sibling of code quietly ceasing to be tested).

**Revisit if** the project shifts from building to maintaining, or if a bug is ever traced
to logic no test had run. Until then, Doug's own rule decides it: attention is the scarce
resource, and a coverage report competes directly with walking a tour — which has a
measured yield of nine bugs in a day.

**Relates to:** #49 (narrow fixture tours), `project-tours-multiply-testing` in Claude's
memory, and `docs/tech-debt.md`'s priority order.

---

## 51. The MSL example corpus — 1,656 known-good models, already vendored

Doug asked 2026-07-30 whether a public specimen source exists, so Claude need not author
every one. It does, and it is **already on the machine**: `hrw/vendor/msl/Modelica 4.1.0`
contains **618 example files** declaring roughly **1,656 models**, loaded by every test run
already.

### Why these are different from authored specimens

**They are known-good.** System Modeler compiles all of MSL; it is the reference library.
So a Rumoca failure on one is unambiguously a Rumoca limitation, **with no adjudication
step** — which is the expensive part of #46's authored-specimen loop (see #43's oracle).

But they carry **no expectation**. Authoring a specimen means predicting where it will
fail, and the *surprise* is the finding (#46). An MSL example predicts nothing, so a pass
teaches nothing. The two mechanisms therefore do different jobs and neither replaces the
other:

| | Authored specimen | MSL example |
|---|---|---|
| Cost | Claude writes it | free, already here |
| Yield | **insight** — the surprise corrects Claude's model of the compiler | **breadth** — reach across the input space |
| Needs the oracle? | Yes, to tell "my specimen is wrong" from "Rumoca is wrong" | **No** — known-good by construction |
| Count | 18 | ~1,656 |

### Measured reach: far better than expected

A 20-model sample, the second half chosen from domains Claude expected to *fail*:

- **14 compiled**, including `Clocked` (synchronous), `StateGraph`, `MultiBody.Pendulum`
  and `Media.WaterIF97` — every one of which Claude predicted would fail.
- **1 real diagnosed failure**: `Fluid.Examples.HeatExchanger.HeatExchangerSimulation` —
  *unbalanced model: 3848 equations, 3150 unknowns (balance = 698)*. A candidate upstream
  finding on its own.
- **5 "no result"**, needing triage: at least two are Claude's error (`Utilities.Examples.
  calculator` is a **function**, not a model), so the true failure rate is lower still.

**Claude's prior was badly wrong** — it expected most of MSL to fail and wrote a
recommendation around that ("a capability census, not a test suite") before measuring.
Twenty models overturned it. Record the prior alongside the result, because the same
instinct will recur.

### What to build

1. **A census run** — every example, recorded as compiles / fails-with-diagnosis /
   no-result, with failures **clustered by message**. Raw pass-fail over 1,656 is noise;
   clustered failures are a **capability map**: what Doug can model, and what upstream
   might build next.
2. **A baseline that must not regress.** Once the number is known, a drop means something
   broke — the one kind of regression the authored corpus is too small to catch.
3. **Triage "no result" first.** It currently conflates "not a model" with a real gap, and
   that ambiguity makes the whole census less useful than it should be.

**Cost:** far too slow for any existing suite — each compile is seconds against full MSL,
so ~1,656 is an hour or more. This is an **occasional run**, not part of `slow-tests`.

### What it tests in **HRW**, not just Rumoca

Asked directly by Doug 2026-07-30 — is there real value here, or only breadth?

**Yes, and the strongest HRW-specific reason had been understated: HRW's extraction code
has only ever run against 18 hand-picked specimens.** A four-model probe against MSL:

- `Media.Examples.WaterIF97` produced a **3.2 MB** flatten stage; `MultiBody.Pendulum`
  1.2 MB. **No panics.** Whether the JSON builders survive a model thirty times larger
  than anything they had seen was simply unknown before.
- All four reported `structural=ERR` *and* `initialization=ERR`, including
  `CauerLowPassAnalog` — which System Modeler simulates happily.

**And the probe demonstrated the cost in the same breath.** Those ERRs are uninterpretable
because the probe **omitted `index_reduction`**, the stage that resolves exactly the
structural singularity high-index models hit. Claude would have reported a scandal by
trusting its own output. **1,656 models will produce hundreds of ERRs and each needs a
judgement Claude has just shown it can get wrong.**

**The cause, found when Doug asked what "ERR" meant:** it was the probe's own label for
`Stage::note_is_error`, and **that flag is set by three constructors meaning different
things** — `err` (nothing produced), `err_with_details` (failed, with a diagnosis), and
`recovered` (**singular but produced a usable report anyway**). `recovered` is what the
structural stage uses for a *high-index* model, which is normal and is fixed by Index
Reduction next. The probe also printed `ERR` *instead of* the byte count, hiding that all
four stages still had values.

So a one-line formatting choice in a throwaway probe manufactured a finding that was not
there. **A census must classify these three apart** — anything reading `note_is_error`,
including `last_successful_stage`, currently treats "recovered" as "failed", which is
defensible for choosing a tab and wrong for counting outcomes.

That is the argument for clustering rather than a caveat about it: the census is worth
building **only if failures are grouped by message**, so one investigation explains fifty
models instead of one.

**Why this does not replace #46's authored specimens** — and not for the reason first
given ("a pass teaches nothing", which conflated *Claude's learning* with *testing
value*). The real reason: **an MSL failure costs triage and an authored failure does
not.** Authoring means already knowing what should happen, so a surprise is immediately
meaningful. With MSL, every failure is a small investigation.

### The bigger prize

Paired with #43's oracle this becomes the **differential test** deferred since Arcs 1–2
(#4): 1,656 known-good models, no authoring, and no adjudication ambiguity. "Compiles" is
not "correct" — only comparison with System Modeler establishes that, and this is the
corpus to do it on.

**Relates to:** #46 (authored specimens — complementary, not superseded), #43 (the oracle),
#4 (the differential test), #22 (simulation, deferred on maturity).

---

## 52. One corpus list with a filter — **NOT a Test mode** — CLOSED 2026-08-03, join deleted

**Closed 2026-08-03. The list shipped; the join was deleted rather than built, on evidence.**

The three items under *What to build instead* were delivered 2026-08-01. **The argument this
idea rests on was not**, and will not be: *"the reports do not want three views, they want to be
columns and filters over one row set."* The list reads `msl-survey.csv` and nothing else; no UI
code reads the fidelity report. <!-- unbuilt: SurveyRow::fidelity_verdict -->

**Doug asked the question that settled it**, before any code was written: *"If the fidelity
reports that there are zero violations, then there would be no value in adding all of that
functionality to the specimen list, correct?"*

**The sweep of 2026-08-02 answered yes, and it is the answer that counts** — the first run to
check Parse IR for real. Every previous sweep ran those checks against
`{"classes":{},"within":null}` and found nothing because there was nothing there, so the earlier
zero was vacuous. This one walked a real Modelica AST: mean peak memory rose 1,228 → 1,353 MB,
and F7 went from sampling roughly *two* nodes of the Parse stage to its 400-path cap.

**Result: 2,614 rows, `outcome=ok`, `n_violations=0`, no failed checks.**

So a fidelity column would read `ok` on every row and a fidelity predicate would match
everything or nothing. **Building it would produce a feature that looks finished and answers no
question**, which is worse than not building it.

### What would reopen this

**A report with a non-constant column.** The oracle (#43) is the live candidate: a
System Modeler differential run produces *findings*, which vary per model by construction. If
that report ever exists, the join becomes worth what this idea claimed — and the one constraint
already recorded still applies free: **it must emit the same `name` join key.**

**Or a fidelity sweep that stops being all-green.** The twelve models that exceed this machine's
limits are the standing reminder that "all-green" means "all-green among those checked".

### The distinction the join would have exposed, and how to get it cheaply

The survey has 2,626 rows and fidelity has 2,614, so joined data separates **"never checked"**
from **"checked and clean"**. That is a real axis and a twelve-row answer — obtainable by
diffing two CSVs, which is what `docs/reports/` is for.

**The lesson, recorded because it generalises:** this idea argued for the join *before the
fidelity data existed*. An argument written before its evidence arrives does not automatically
survive it, and a heading saying "BUILT" stopped anyone from re-checking. Doug asked the
question Claude should have asked itself.


**The mode was dropped before it was built.** Doug, 2026-08-01, questioning his own decision:

> More than once, we have both likened test mode to specimen mode. The more that I have
> thought about that, the more that I have questioned whether it makes sense to have a new
> test mode. After all we could simply add the MSL examples to the specimen list.

**He is right, and the reason is that "mode" is the wrong unit.** Every existing mode changes
the left panel **and the interaction loop** — Tour reads stops and clicks links, Specimen
picks from a list and inspects, Debug arms and steps. Test mode's loop is *pick from a list,
inspect the pipeline*, which **is Specimen mode's loop exactly**. What differs is the list's
source and its columns: a data question wearing a layout question's clothes. Two modes with
the same layout, the same right-hand side and the same gestures would read as important in a
plan and as noise in the app.

### The composition argues FOR merging, which is the part that changed my mind

[`reports.md`](reports.md) calls the three reports load-bearing — survey → *eligible*,
fidelity → *trustworthy*, oracle → *findings* — and that looked like justification for a
dedicated surface. It is the opposite.

**The interesting question is never "show me the fidelity report."** It is the **join**:
*models that are fidelity-green **and** oracle-mismatched*, because `reports.md` says a
mismatch is only an admissible upstream finding when the model is fidelity-green. Three
separate views make that join something you do in your head. **One list with filter predicates
makes it a query** — which is exactly the "query axes over the corpus" that #53 calls the
enabler for everything else.

So the reports do not want three views. **They want to be columns and filters over one row
set.**

### What to build instead

1. **One list widget, three visible sources** — curated `specimens/`, scratch
   `.hrw-bridge/specimens/`, and the 2,626 MSL rows.
2. **The filter is a prerequisite, not an enhancement.** Eighteen files need no filter; 2,644
   do. Without it the merged list is unusable — which is probably the real reason this felt
   like it needed its own mode.
3. **Keep the sources visibly distinct.** Curated specimens have properties MSL rows do not: a
   `purpose.md`, a generated trace, System Modeler round-trip intent. #53 already said *split,
   do not repurpose* about scratch specimens, and the same care applies. **Merge the widget,
   not the corpora.**

### Two things that must NOT go in the list

- **Oracle per-item state** (unfiled / filed / fixed upstream). That is not a property of a
  model, it is the status of *a finding you intend to file* — and filing state already has a
  home in [`upstream-issues.md`](upstream-issues.md). **Putting workflow state in a corpus
  browser is how a browser becomes a bug tracker.**
- **Anything that needs the three reports kept apart.** If a case turns up that genuinely
  cannot be expressed as a filter, **that is the evidence that something Test-mode-shaped was
  right after all** — and it should reopen this decision rather than be worked around.

### Delivered 2026-08-01

Built in the order the analysis implied, and **step 1 was the one that mattered**:

1. **Corpus addressability** — `ToWorker::CompileLibraryModel`, `App::open_library_model`, and
   `hrw://load/<qualified>`. The worker could already compile any of the 2,626 models
   (`compile_model_by_name`, built for the fidelity sweep) and **the UI had no way to ask**, so
   a tour could not link to one. That, not the filter, was what blocked just-in-time curricula.
2. **The corpus as a third list source** — a **collapsed header carrying the model count**,
   always visible, which opens when a filter is typed.

   **The first version showed it only while filtering, and that was wrong.** Doug started HRW,
   saw no MSL models and reported them "not showing" — exactly right from where he sat.
   **An absence you cannot see is indistinguishable from a feature that was never built.**
   Worse, the headless test had asserted the hidden behaviour, so **the test encoded the defect
   as a requirement** and a green suite said the feature worked. It did work; it was invisible.
   The collapsed header keeps the reason the first version existed — 2,626 rows must not bury
   18 curated specimens — while making the corpus's existence impossible to miss.
3. **The filter** — `survey::matches_filter`, case-insensitive, matching name *or* outcome,
   with whitespace-separated terms ANDed so adding a word narrows. Deliberately modest: Claude
   queries the CSV directly and is not its consumer.

**A real bug the headless test found**, and it was not a test artifact: the specimen list
returned early when `self.files` was empty, so **an empty or unscanned `specimens/` took the
whole corpus down with it** — a different source being empty made 2,626 models unreachable.
The guard predated the corpus and had come to guard too much. It now reports and continues.

**One verb, not two.** `hrw://load/` reaches all three sources: files resolve first so the
repo's own copy wins a name collision, a dotted name falls through to the library, and a bare
unknown name is reported as a typo rather than guessed at. A separate `hrw://model/` would
have split one gesture in two and needed merging later — the mistake Test mode was.

### Build the filter first regardless

#53 deferred this decision pending the filter, and that sequencing still holds — not because
the answer is unclear now, but because **the filter is required either way.** If the mode were
right it needs a filter; since the merge is right it needs a filter. It is the one piece no
decision changes, which makes it the safe thing to build while the decision is still cheap to
reverse.

**Everything below is the original 2026-07-31 proposal**, kept because the report-shape
reasoning in it is still exactly right — it is *what the columns are* that survives, not *what
the mode was*.

---

Doug, 2026-07-31, while the first full MSL survey was running:

> Ultimately, we're going to consider adding a new "Test" mode to HRW. The test mode will
> enable the user to load either the fidelity report or the oracle report. Once either
> report is loaded, it will be visible in the LHS of HRW. When a user clicks on a model in
> the LHS, we'll open the compiled model in the RHS for further investigation.

He raised it early **so the report formats are designed for consumption rather than
retrofitted** — `docs/upstream-strategy.md` planning rule 5. What follows is what that
implies, recorded before the formats harden.

### There are THREE reports, not one, and they should share a row shape

Easy to conflate, because the first one exists and the others do not:

| Report | Question | Produced by | Status |
|---|---|---|---|
| **Survey / capability map** | how much of MSL does *Rumoca* compile, and where does it stop | `examples/survey_msl.rs` | first full run 2026-07-31 |
| **Fidelity report** | does *HRW* agree with Rumoca, per model | F1-F9 harness (`src/fidelity.rs`) | emits nothing yet — only asserts |
| **Oracle report** | does *Rumoca* agree with *System Modeler*, per model | not built (`docs/ideas.md` #43) | not started |

**Give all three the same first four columns — `name`, `kind`, `outcome`, `message`** — so
Test mode has one loader and one list widget, and the report-specific columns simply extend
to the right. Diverging here costs a second implementation of everything.

**And `name` is a join key, not merely a label** — see `docs/reports.md`, which is the design
authority for how the three compose. A model that is oracle-mismatched is only an admissible
upstream finding if it is *fidelity-green*, and that judgement is computed by joining the
reports on the qualified model name.

**Same loader and layout, different default filter**, because their steady states differ:
survey **browses** (full list), fidelity shows **exceptions** (usually empty, and empty is
success), oracle is a **worklist** (unfiled first). A fidelity report rendering 2,600 green
rows would bury its own good news.

**The oracle report also carries state the run did not produce** — unfiled / filed / fixed
upstream / won't-file — so its regeneration must *merge* with a side file rather than
overwrite it. And clicking a mismatch **generates a draft, it does not file**; the reasoning
is in `docs/reports.md`.

### The gap that has to close before clicking a row can work

`WorkerState::compile` takes a **file path** and reads it as the specimen source. An MSL
model has no such file — it lives inside a package the library already loaded. So Test mode
needs a **compile-by-qualified-name** entry point in the worker.

The survey already proves the call works
(`session.compile_model_strict_reachable_uncached_with_recovery(name)`); what is missing is a
worker path that produces a full `FromWorker::Compiled` from a name rather than a path. That
is a prerequisite for Test mode and worth doing on its own — it would also let a scratch
question point at any MSL model, not only at specimens.

### Format: CSV stays, and HRW parses it

CSV is right for the **published** artifact — diffable in git, openable in a spreadsheet by
a maintainer who will not install anything. Emitting a second JSON copy for HRW would create
two sources of truth for one table.

So HRW gets a small RFC-4180 reader (~30 lines, quoted fields carry commas and quotes
freely). **No new dependency** — and adding one needs Doug's approval anyway.

Provenance rides in a **sidecar** `*.meta.json`, not in CSV comment lines, which strict
readers and spreadsheets both mishandle. Test mode uses it to caption a loaded report: which
Rumoca, which MSL, how many models, when. A report that cannot say what it describes is
neither reproducible nor safely readable months later.

### Layout, fonts and colours — conform, do not invent (Doug, 2026-07-31)

> The layout for test mode should be the same as for tour mode and specimen mode. And,
> other details such as font choices and colors should be the same.

Read against the code, that requirement mostly *determines* the design, which is the good
news: there is very little to decide.

`UiMode` today is `{ Tour, Specimen, Debug }` — **three** modes, not two. `Debug`
deliberately **hides** the LHS so the stage tabs fill the window with VS Code alongside, so
"the same as tour and specimen" means the two *panelled* modes.

**The structural observation: Test mode is Specimen mode with a different list source.**

| | Specimen mode | Test mode |
|---|---|---|
| LHS panel | `Panel::left`, `LEFT_PANEL_WIDTH_FRACTION` (0.4) | identical |
| Top third | specimen list (`SPECIMEN_LIST_HEIGHT_FRACTION` = 1/3) | report rows |
| Rest | purpose / source for the selected specimen | detail for the selected row |
| Click a row | loads that model into the RHS | **the same** |
| RHS | stage tabs | **unchanged** |

So the reuse is: `section_header` for the bars (it already pins weight and size 13.0 through
`section_style`), the same panel fractions, and `colors.rs` for every colour. **Outcome
colouring must come from the existing constants** — `ok_color(dark_mode)`, `WARN_AMBER`,
`ANIM_FAIL` — rather than a new palette, or Test mode's "failed" will not be the red the
rest of HRW already means by it.

That makes this a much smaller feature than it sounds. The genuinely new parts are only:
**loading and parsing a report**, **rendering a row**, and **compile-by-qualified-name**.

### ~~Wiring a new MODE — the checklist a new stage already has~~ — MOOT

*(Superseded 2026-08-01: no new mode is being built. Kept because the checklist is correct for
whenever a mode genuinely is added, and because it is a fair record of what the mode would
have cost — six wiring sites, two of which the compiler would not have caught.)*

`hrw-stage-diff-highlight-extend` records that a new *stage* must be wired into every
per-stage system. A new **mode** has its own list, and it is shorter but not empty:

- `UiMode` variant, and the **View menu** entry beside Tour / Specimen / Debug
- `view_context`'s `ui_mode` match — exhaustive, so the compiler catches this one
- `specimen_detail` and the other `ui_mode ==` guards, which are *not* exhaustive matches
  and therefore **will not** be caught: grep `ui_mode ==` and decide each site deliberately
- the idle-hint text (Tour mode's wrong hint was a bug Doug reported on 2026-07-30)
- `clear_specimen_state` / mode-switch reset — the second bug from that same walk was state
  surviving a switch
- a fixture tour for the mode, per `docs/ideas.md` #49

### What Test mode is really for

Note the sequence Doug describes: **a report finds something, and the observatory explains
it.** That is the same shape as the answer platform — the report is a *noun-finder* at a
scale no human browses, and HRW is the explanation. It also happens to be exactly the
workflow a Rumoca maintainer would want for triage, which is `upstream-strategy`'s overlap
argument arriving from a different direction.

---

## 53. The corpus shift — MSL as the example source, and what it does to specimens, lookup and curriculum

Agreed with Doug 2026-07-31, once the survey existed and it became clear what it enables.
**#51 predicted the corpus; this is what having it measured actually changes.**

The survey (`examples/survey_msl.rs`, `docs/reports/msl-survey.csv`) records outcome and IR shape for
all **2,626** MSL models, of which **557 reach a solvable system**. That turns the corpus from
"a pile of models we could compile" into **a catalogue searchable by the thing we care about**.

### Do specimens still belong in the repo? Yes — with a narrowed role

They are **no longer the breadth corpus**; MSL is. But four things MSL structurally cannot do:

- **Carry an expectation.** #51's insight, and it survives: an MSL example *predicts nothing*,
  so a pass teaches nothing. An authored specimen predicts where it will fail, and **the
  surprise is the finding**. Breadth and insight are different products.
- **Fail on purpose.** MSL is known-good and contains no model authored to fail at flatten
  with a connector mismatch. `IncompatibleConnect`, `UndefinedRef`, `DimensionMismatch` and
  `OverInitRc` are deliberate failures, and **F9 has no data without them**.
- **Be minimal.** `ProportionalLoop` has 3 equations. Learning tearing, a 3-equation loop is
  comprehensible; MSL's largest coupled block has **64 variables** and teaches nothing on
  first encounter. Pedagogical minimality is a property MSL does not offer at all.
- **Stay still.** A specimen does not change when MSL is upgraded, which matters for a
  reproducer attached to an upstream issue.

**So the policy is a burden of proof, not a purge.** A *new* specimen must answer *"why can
MSL not provide this?"*, and there are exactly three good answers: it must fail, it must be
minimal, or it is a bug reproducer. Existing specimens stay.

### Lookup: query the catalogue by IR shape

The columns already support it. Real answers from the first full survey:

| Want to study | The data answers |
|---|---|
| a large algebraic loop (tearing) | `Electrical.Machines.…SwitchingDcDc`, `PowerConverters.…HBridge_R` — 64-variable blocks |
| connection expansion at scale | `Electrical.Analog.Examples.Lines.SmoothStep` — **365** connection equations |
| the Pantelides funnel firing | 78 models; largest `DCMachines.DCPM_Drive` at 666 equations |
| a deep component hierarchy | `StateGraph.Examples.ControlledTanks` — depth 6 |

**This changes the working relationship with the corpus.** "Show me a real model where index
reduction does something" stops being a request Claude fulfils by *authoring* one, and becomes
a **query**. That is a large reduction in Claude's ability to bias the evidence by choosing
convenient examples.

### The specimen list and the example list are the same widget

#52 already noted Test mode is Specimen mode with a different list source. **Lookup makes that
literal:** a "specimen" stops being *a file in the repo* and becomes *a row you can open*,
which `WorkerState::compile_model_by_name` now makes possible.

One list widget, three sources:

1. `specimens/` — curated, minimal, some deliberately failing
2. `.hrw-bridge/specimens/` — Claude's scratch probes, ephemeral by construction
3. **the survey — 2,626 MSL models**, filterable by shape

~~Worth building the filter *before* deciding the two modes should merge: the merge may turn
out to be obvious once the list is shared, or obviously wrong.~~

**Decided 2026-08-01, and it was obvious: they merge.** Doug questioned whether Test mode
should exist at all, and the answer is no — *mode* is the wrong unit, because Test mode's
interaction loop **is** Specimen mode's. See #52, which now records the reasoning and what to
build instead. **The filter-first sequencing survives the decision**: the filter is required
either way, so it stays the thing to build while the rest is still cheap to reverse.

### A difficulty ladder, ordered by measured complexity

The most interesting consequence. For any phenomenon HRW has a view for, the survey can order
examples by how hard they are:

| Rung | Model | Largest coupled block |
|---|---|---|
| 1 | `ProportionalLoop` (authored, minimal) | 3 |
| … | MSL models, measured | 8, 15, 30 |
| top | `SwitchingDcDc` | 64 |

**Ordered by measured IR complexity rather than by Claude's guess about what is hard.** That is
the same shift as everywhere else in this work: from judgement to measurement.

**And the tension it must not cross.** `feedback-curriculum-emerges-from-reading` says do not
front-run the curriculum — it emerges from Doug reading Cellier and from Q&A finding friction.
A generated ladder does **not** violate that, and the distinction is worth keeping sharp:

> The ladder does not choose **what** to teach. It orders **examples** for a topic already
> chosen.

A syllabus generated from IR statistics would be exactly the front-running that rule forbids.
An ordering of candidate models, once Doug has decided he is studying tearing, is a lookup aid.
Keep it on the right side of that line.

### Ad hoc / just-in-time curricula (Doug, 2026-07-31)

The ladder above was one ordering — by measured complexity. Doug generalised it, and the
generalisation is the load-bearing part:

> There are so many potential curriculums, each depending upon my interest at the moment. For
> example, I might want to focus on CS and performance concerns and so request an ad hoc
> curriculum which explains and demonstrates what you highlighted in your "where does the time
> go" table. Ad hoc / just-in-time curriculums is a powerful idea.

**There is not one curriculum. There is one corpus and an unbounded number of paths through
it**, and which path is right depends on what Doug wants to understand *today*.

| Today's interest | The ordering it wants | The column that provides it |
|---|---|---|
| tearing | coupled-block size, ascending | `largest_coupled` |
| **CS and performance** | **cost against system size** | **per-phase timings (#54 Part B)** |
| hybrid systems | event conditions, smallest first | `n_event_conditions` |
| connection semantics | connection equations | `n_connect_eq` |
| how compilers fail | failure phase and cause | `outcome`, `message` |
| high-index DAEs | rescued-by-reduction, by size | `index_reduced`, `n_equations` |

### The design conclusion: do NOT build a curriculum feature

This is the part worth getting right, because the obvious move is wrong.

A curriculum is a **verb**, not a noun. It is an utterance — *"walk me from the simplest
algebraic loop to the hardest, explaining what tearing does at each"* — and HRW's whole
premise is that Claude supplies the verb while HRW supplies an exact noun
(`docs/context-assembly.md`, `hrw-works-with-claude-not-without`).

So the thing to build is **query surface**, not lesson storage:

- **Build:** filters and orderings over the corpus, so any axis is reachable by mouse.
- **Do not build:** a "curriculum" object, a lesson sequence, a progress tracker, stored
  narration per rung.

A *stored* curriculum is regenerable prose that nothing checks — the exact defect that
retired 1,632 lines of specimen narrative and the end-to-end tour's prose
(`feedback-learning-over-polish`, [[store what cannot be regenerated]]). An **ad hoc** one
cannot rot, because it does not persist. That is not a compromise; it is strictly better.

### Why this satisfies "curriculum emerges from reading" rather than violating it

`feedback-curriculum-emerges-from-reading` warns against front-running the curriculum. A
generated ladder could look like exactly that, so the line matters:

> **A curriculum generated on request is emergence, not front-running.** The request *is* the
> emergence — Doug's current interest is the input, not Claude's guess about what he should
> want next.

Front-running would be deciding in advance that lesson 3 is tearing. Answering "I want to
understand where the time goes" with an ordered set of real models is the opposite: it is the
curriculum arriving *because* a question did.

### What this needs that does not exist yet

1. ~~**Query axes over the corpus**~~ — **built 2026-08-01**: `survey::matches_filter`,
   case-insensitive over name and outcome, terms ANDed. **Sorting is still absent**, and no
   axis reaches beyond the survey's own columns. <!-- unbuilt: survey::sort_rows -->

   *(This entry read "does not exist yet" until 2026-08-02, and its tag passed the whole time
   — because the tag named `survey_filter` while the function shipped as `matches_filter`. **A
   tag that resolves nothing is indistinguishable from a claim that is still true**, so the
   checker was green on a stale claim. Name the symbol as it will actually be spelled, or the
   tag checks nothing.)*
2. **Per-phase timings**, for the performance curriculum specifically — #54 Part B, and the
   worker already logs them, so it is aggregation rather than instrumentation.
3. **Nothing else.** The curriculum itself is an utterance, and utterances need no schema.

### What to build (small, in this order)

1. **A filter/sort over the survey rows** in Test mode's list — no new data, just a view.
   **This is the enabler for every ad hoc curriculum**, not merely for lookup.
2. **A saved-query notion**, so "models with a coupled block between 8 and 20" is nameable and
   re-runnable rather than retyped.
3. **Per-phase timings** (#54 Part B) — the one axis the corpus does not yet carry, and the
   one a CS/performance curriculum needs.
4. **No curriculum object.** The ladder is a sort order; the teaching is an utterance.

**Relates to:** #51 (the corpus), #52 (Test mode), #46 (failure specimens),
`docs/reports.md`, and the specimen rules in `CLAUDE.md` that this narrows.

---

## 54. Where the time goes — a performance profile, for a maintainer AND for Doug

Doug, 2026-07-31, watching the survey and fidelity runs produce timing data as a byproduct:

> We're gathering lots of useful statistics which a rumoca maintainer might appreciate having
> in a performance report of some kind. […] Also, some of the statistics are interesting to me
> as a student of this stuff. I would be grateful to gain an understanding of where the
> performance costs are in the phases and their algorithms.

**Two reports from one measurement, and the difference is not the data — it is the framing.**

### The framing inversion, which is the whole reason this is one idea and not two

HRW deliberately calls `compile_model_strict_reachable_uncached_with_recovery`: an observatory
must actually *run* the phases, so the cache is off by design. That single fact points in
opposite directions for the two audiences:

| | Cold compile is… |
|---|---|
| **For a maintainer** | a **caveat** that must lead the report. Their users hit a warm cache; our numbers are worst case by construction, and a report that omits this deserves to be dismissed. |
| **For Doug** | the **point**. He wants to know what the *algorithm* costs, not what the cache saves. |

Same measurement, opposite emphasis. Neither report can quietly reuse the other's prose.

### How to frame it — settled 2026-08-01

**Not** *"here is what your compiler costs"*, which invites defensiveness. Instead:

> *"Here is what materialising every IR costs, measured across your standard library."*

Rumoca's README states as a design emphasis *"explicit compiler phases and IR boundaries"* and
*"reusable symbolic outputs rather than a single closed execution path"*. **A project that
chose that trade has already accepted that materialising IR costs something.** HRW pays it
maximally — ten stage trees per model — so the 50-170x measured below is *the price of looking
at everything*, a fact about the trade rather than a criticism.

For an interoperability layer, that number is **design data**: it says what a downstream
consumer pays to hold all the IR at once, which is what Julia/SciML, CasADi or a code
generator would be doing. See `docs/upstream-strategy.md` § "The alignment argument".

### Part A — the upstream performance profile (a zero-adoption-cost gift)

Three findings worth a maintainer's afternoon, per `docs/upstream-strategy.md`'s ordering:

- **The cost of full IR materialisation — NOT yet cleanly measured.** The obvious comparison
  (survey 5-15 s versus HRW ~900 s on the same models) is **contaminated**: the survey *caps
  index reduction above 800 equations and skips it*, while HRW runs the funnel
  unconditionally. So the gap mixes HRW's extraction overhead with a Rumoca phase the survey
  never ran. **Separating them needs either an uncapped survey pass on those models, or the
  `Index reduction` line from the per-phase log**, which now records phases individually.
  Until then there is no publishable ratio — see `docs/upstream-strategy.md`.
- **A performance cliff with a named cause.** The whole MSL compiles in ~38 minutes; adding
  index reduction made **four models consume 97 minutes** (the Spice3 four-bit-adder family,
  2,477-10,175 equations). Superlinear in system size, with the models to reproduce it. That
  is actionable in a way "Rumoca is slow" is not.
- **Session memory growth across compiles** — likely the most valuable, because nobody finds
  it without a batch run. One session reached **8.3 GB committed over 2,626 compiles**.
  `Session` is plainly built for language-server use (`update_document`,
  `namespace_index_query`, incremental caches), where growth across compiles matters a great
  deal. **Record it as an observation needing their confirmation, not a measured leak rate**:
  committed memory was watched, allocations were not.
- **Nondeterministic diagnostics** — already `docs/upstream-issues.md` #3.

**What to exclude:** stage IR sizes. `BalancingDelta` at 17.2 MB is *HRW's serialisation* of
their IR, not their cost. Including it would be measuring ourselves and billing them — the
same attribution discipline the capability map needs.

### HRW's costs are paid INSIDE the phase windows — so the phase log cannot attribute them

Established 2026-08-01, from Doug's question *"are the HRW costs paid at phase boundaries and
not during execution of phases?"*

**Inside.** Every stage's timing window encloses HRW's own work, not just Rumoca's:

```rust
log(LogLevel::StageStart, "Index reduction");
let t = Instant::now();
let (stage, frames) = index_reduction_stage(result, &source);   // <-- all of it
log(LogLevel::StageEnd, format!("Index reduction ({:.1}ms)", t.elapsed()...));
```

`index_reduction_stage` clones the DAE, runs Rumoca's
`index_reduce_for_structural_analysis`, builds the structural report, **serialises the whole
thing to JSON**, and records the animation frames — all before the clock stops. `Resolve` is
the same shape, enclosing `extract_class`, `build_def_index`, `instantiate_and_typecheck` and
`record_connection_frames`.

**Consequence, and it corrects a claim made an hour earlier:** `Index reduction: 500 s` means
*Pantelides plus HRW's serialisation of its result*, and nothing in the log tells them apart.
The per-phase log gives **phase attribution**, never **HRW-versus-Rumoca attribution**.

### The control group already exists: `survey_msl --only-skipped`

Built on 2026-07-31 to close the 2.7% coverage gap left by the reduction cap. It turns out to
be the missing experiment as well, because it runs **exactly the capped models, uncapped,
through `Session` directly with no HRW extraction**:

| Run | Measures |
|---|---|
| `survey_msl --only-skipped` | Rumoca's cost **including** index reduction |
| HRW's compile path on the same models | the same, **plus** HRW's extraction |
| **difference** | **HRW's overhead, cleanly attributed** |

**This is the number to have before publishing any ratio.** The retracted "50-170x" figure
compared a capped survey against an uncapped HRW path and called the difference overhead.

**Cost:** those 71 models uncapped include the Spice3 family, so it is the multi-hour run
already logged as optional part 2 of the survey. Its value has changed: it is no longer only a
coverage exercise, it is the control group for the whole performance question — which makes it
considerably more worth doing than when it was filed.

**Sequencing:** behind the fidelity retry. Completing the corpus matters more, and the two
compete for the same machine.

### Part B — the student's version: measured cost against theoretical complexity

The half Doug asked for, and the more interesting one. **HRW already logs per-stage
milliseconds** (`worker.rs`, `LogLevel::StageEnd`), so this is *aggregation*, not new
instrumentation — the numbers are being thrown away, not missing.

Per-phase cost across a corpus answers "where does the time go". But the sharper exercise is
one level down, comparing **what the algorithm is supposed to cost** against **what it does
cost**:

| Algorithm | Theoretical | What measuring it would teach |
|---|---|---|
| Matching (Kuhn) | O(V·E) worst case | how close real incidence matrices get to the worst case — usually nowhere near, and *why* is the lesson |
| Tarjan SCC | O(V+E) | should be flatly linear; a deviation means the graph build dominates, not the SCC |
| Tearing | greedy per coupled block | cost follows block size, and the largest block in the MSL is **64** — so tearing is cheap *because the blocks are small*, which is itself a fact about physical models |
| Pantelides / index reduction | iterative, superlinear here | the measured cliff. Why it is superlinear is a real question with a real answer |

**This is a way to learn the algorithms that reading cannot give**: a complexity bound is a
claim about behaviour, and the corpus is 2,626 chances to check it. It also sits correctly
with `feedback-curriculum-emerges-from-reading` — Doug asked the question, so it is emergent,
not front-run.

### What to build (small, and mostly already there)

1. **Aggregate the per-stage timings the worker already logs** into the survey row or a
   sibling report. No new instrumentation.
2. **Plot cost against a shape column** — `n_equations`, `largest_coupled`, `n_functions` —
   which is how a complexity claim becomes checkable.
3. **Separate the two reports** at the point of writing, never by editing one into the other.

**Relates to:** #51 (the corpus), #53 (lookup and the ladder — the same data, ordered for
teaching), `docs/upstream-strategy.md` (deliverable ordering, attribution),
`docs/upstream-issues.md` #3.

---

## 55. Size-aware batching for the fidelity sweep — measured, and deliberately deferred

Raised and then argued against by Claude on 2026-07-31; Doug agreed to defer. **Recorded with
the numbers so it can be picked up on evidence rather than re-derived from scratch.**

### The observation

The sweep runs **one model per process**, so it pays the MSL load — measured at **1.3 s** —
**2,626 times**. That is **57 minutes** of the ~4.3-hour serial run doing identical work.

Batching several *small* models per process recovers most of it. Failures and models under
200 equations peak around 1 GB, so several fit comfortably; models ≥200 equations stay
one-per-process.

| Batch size for small models | MSL load total | Saved |
|---|---|---|
| 1 (today) | 57 min | — |
| 5 | 17 min | 40 min |
| **10** | **12 min** | **45 min** |
| 20 | 9 min | 48 min |

**The saving is capped at 57 minutes** however large the batch, and flattens hard after 10 —
by then 90% of the redundant loads are already gone. Net effect at batch 10: **4.3 h → 3.5 h,
about 18%.**

Corpus split: **2,326 batchable** (failures + <200 equations), **300 must stay
one-per-process**.

### Why it was deferred

- **The run is overnight.** 3.5 h versus 4.3 h changes nothing about when results are seen.
  This is the same argument that stopped further survey optimisation (#48 and the survey's
  own "no further improvements" decision), and it applies more strongly here.
- **Batching is what broke the machine.** The 2026-07-31 crash was 53 models accumulating in
  one process. Size-aware batching is genuinely different — small models only, with
  `--rebuild-every` inside the batch — but it reintroduces the *shape* of that failure for
  18% on an **unattended** run. The current design's virtue is that its worst case is one
  model, which is easy to reason about at 3am.
- **It adds a scheduling layer that must be right**: partition by size, batch one partition
  and not the other, keep `--resume` working across both. More code to be wrong in than the
  thing it saves.

### When it becomes worth building

**If the sweep is re-run often.** The trigger policy has it running after a Rumoca rebase,
before an upstream PR, and whenever HRW changes how it emits or reads stage JSON — and that
third trigger could fire several times a week during active work on `worker.rs`. At that
cadence 45 minutes compounds into real time.

**Revisit with usage data**, not speculatively: count actual sweep runs over a month.

### Sketch, if picked up

- `scripts/measure-fidelity.ps1` partitions the model list using `docs/reports/msl-survey.csv`'s
  `n_equations` and `outcome` — data it already has.
- Small partition runs with `--max-models 10 --rebuild-every 5`; large partition unchanged at
  `--max-models 1`.
- `--resume` is unaffected: both write to the same report and skip settled rows.
- **The watchdog stays per-process and unchanged.** It is the safety property, and batching
  must not weaken it.

**Relates to:** `docs/long-runs.md` (the runbook), `docs/architecture.md` §11 (why the run is
bounded by process lifetime at all), and the standing boundary in `docs/fidelity-plan.md` —
note this is a **scheduling** change, not an optimisation of HRW, so it does not cross it.

---

## 56. A dedicated diagnostic run over the "hard models" stress corpus

Doug, 2026-08-01, while the fidelity retry was pending:

> Perhaps retry runs are opportunities to learn more about how various models stress HRW and
> how we might improve HRW. Should we consider retry runs to be useful stress tests of HRW?

**Yes to the framing, but not by loading the retry itself.** Deferred until after fidelity
testing, the oracle test and Test mode are done — Doug's explicit sequencing.

### The asymmetry that makes it worth doing

A retry operates on a **small set of pre-selected, known-difficult models** — 16, not 2,626.
That inverts the economics of instrumentation: something costing 10x per model is
unaffordable across the corpus and free across sixteen. And they are not arbitrary; they are
precisely the models that already broke something.

### Why it must NOT be the retry run

**A retry's job is to complete the corpus.** Its success criterion is "all 2,626 rows
present". Instrumentation that slows a model enough to push it past 900 s trades a data point
for a measurement — a bad trade, since the corpus result is the artifact and the measurement
is not.

| Run | Purpose | Instrumentation |
|---|---|---|
| Retry | complete the corpus | **only what is free** |
| **Diagnostic** | study the hard models | as heavy as useful |

The free part was done on 2026-08-01: the runner now captures the per-stage timings
`worker.rs` was already emitting into a discarded callback, and the watchdog narrates which
phase a slow model is sitting in. That cost nothing and is in the retry.

### The stress corpus

**Keep the hard models as a named, checked-in list.** They are deliberately extreme, already
identified, and reusable. Current members are the 16 that aborted in the full sweep — the
Spice3 family (up to 10,175 equations, 7.7 GB), the induction machines, and
`LightningSegmentedTransmissionLine`.

### What the diagnostic would measure

- **Where the memory goes.** 7.7 GB on a Spice3 model, and *which structure* is unknown. The
  ten stage `serde_json::Value` trees are the obvious suspect but that is inference. Peak RSS
  correlated against phase markers would localise it without a new dependency, since the
  watchdog already samples every 2 s and the phases are now timestamped.
- **Where the time goes, per phase**, on models where it matters — the free capture gives
  totals; a diagnostic could break down within a phase.
- **Whether cost is size-driven at all.** It is not, and that is unexplained:
  `FourInverters` is 282 equations and took 161 s; `TransformerTestbench` is 4,193 equations
  and took 31 s. Fifteen times the size, a fifth of the time.

### Diagnostics as teaching — and the trap in that idea (Doug, 2026-08-01)

> I could learn a lot from diagnosing problems experienced by those sixteen models. And we're
> building HRW to enable you to help me learn about stuff like those problems. So it seems
> that we should leverage HRW to enable you to teach me during diagnostics. For example,
> something like ad hoc diagnostic tours.

**The strongest version of this is stronger than stated.** A diagnostic is a *real question
with stakes*, and this project's thesis is that curriculum emerges from real friction rather
than from a syllabus (`feedback-curriculum-emerges-from-reading`). *"Why does this model take
900 seconds?"* pulls in model structure, incidence sparsity, index reduction, algebraic loops
and the cost of symbolic manipulation — not because someone decided those were lesson four,
but because the question requires them.

#### But hard-for-HRW is not the same as pedagogically rich

| Family | Why it is hard | What is there to learn |
|---|---|---|
| **Spice3** (up to 10,175 eq) | **scale** — a four-bit adder's worth of transistor models | mostly *"large systems are large"* |
| **Induction machines** (7 of the 10 timeouts) | **structure** — high index, coupled electromagnetics | genuinely rich |
| `LightningSegmentedTransmissionLine` (6,477 eq) | distributed parameters | interesting, physically motivated |

The Spice3 family dominates the *memory* story and teaches the least: its difficulty is a
fact about arithmetic, not about modelling. **Worth knowing before investing a session in it.**

#### And they are hard to open for exactly the reason they are hard to test

HRW's UI uses the same compile path the sweep does. A diagnostic tour of
`Spice3BenchmarkFourBitBinaryAdder` means waiting ~15 minutes and 7.7 GB for the first screen.
That is not a tour, it is an ordeal — and `docs/fidelity-plan.md`'s standing boundary says we
do **not** optimise HRW to fix it.

#### The synthesis: use the difficulty ladder in REVERSE

Rather than diagnosing the hardest model, find the **smallest** model exhibiting the same
phenomenon:

> *"Show me the smallest model where index reduction dominates the DAE pipeline."*

Study **that** interactively, where it opens in seconds and every view is usable — then
confirm the finding scales by checking the CSV across all 2,626 models. **The big model
becomes evidence, not the classroom.**

That is #53's query surface doing the work, and it removes the ordeal without touching HRW.

#### Ad hoc diagnostic tours are #53, pointed at a diagnostic

Same rule, so it does not need restating in code: **do not build a diagnostic-tour feature.**
The tour is an utterance Claude composes; HRW supplies the exact noun. What earns building is
**the query that finds the right model to tour**.

#### Label which kind of learning a diagnostic offers

Some of what is learnable here is about **HRW and Rust** — why materialising ten IR trees
costs 7 GB is an engineering lesson, not a modelling one. Doug is learning Rust and egui too
([[user_role]]), so that is not worthless. But **say which kind it is at the outset**, so a
week does not disappear into allocator behaviour when the goal was Pantelides.

### What is actually captured, and the one gap (2026-08-01)

Doug asked whether the narration is for monitoring or for later analysis, and whether its data
would be needed to pick the smaller models for study. Three things had been getting conflated:

| | What it is | Durable? |
|---|---|---|
| **Narration** | time-sampled console output, every 30 s past 60 s | **no** — monitoring only |
| **The `[phases]` log** | per-model phase *totals*, written on completion, beside the profile | **yes** |
| **The survey** | structural shape for all 2,626 models | yes, committed |

**Narration is monitoring only.** Nothing depends on it; if it vanished we would lose
visibility during a run and no data.

**Phase START/END events are not captured** — only per-phase totals, derived from what
`StageEnd` already reports. So there is no timeline, no nesting, no overlap detection. That is
why `TransformerTestbench`'s phase totals summed to 40.6 s against a 31.6 s wall clock and the
most that could be said was "the phases overlap or nest".

#### Selecting smaller models does NOT need it

The reverse-ladder selection runs off `docs/reports/msl-survey.csv`, which already carries
`n_equations`, `largest_coupled`, `index_reduced`, `n_event_conditions`, `n_coupled`,
`has_arrays`, `max_depth` and `n_functions` for **every** model. *"The smallest model with a
coupled block over 20"* is a query against existing columns.

#### The one criterion that does, and the gap under it

> *"Show me the smallest model where index reduction dominates the DAE pipeline."*

That is a phase-cost question, and **the full sweep ran before the phases log existed**. Phase
totals exist for **zero** of the 2,610 completed models, and for only the sixteen in the retry.
Backfilling means re-running the sweep — four-plus hours.

**Decision: do not backfill, and do not add start/end events.**

- Most selection criteria are structural, and the survey answers them today.
- Per-phase **totals** are sufficient to say which phase dominates. Timestamps would add a
  timeline, and no question currently needs one.
- The gap closes **incidentally**: the next full sweep after a Rumoca rebase captures phases
  for everything at no extra cost.

**Revisit only if a phase-cost query actually arises before the next rebase** — that is the
moment to weigh four hours against it, and not before.

### What it must not become

**Not a reason to optimise HRW.** `docs/fidelity-plan.md` carries Doug's standing boundary:
HRW is an education project, and the stage trees, equation sheet, identifier index and
animation frames *are the product*. The diagnostic is for **understanding** — Doug's
education, and the maintainer-facing performance profile in **#54** — not for making HRW
faster at extreme models.

**Relates to:** #54 (the performance profile — this is its measurement arm), `long-runs.md`,
`docs/architecture.md` §11.

---

## 57. Where to start reading Cellier — the chapters Rumoca fits best

**Extracted 2026-08-01** from `history/answer-platform-plan.md` (Phase 2), which is being
retired. This is the one part of that plan not already captured elsewhere, and it is a fact
about **Rumoca's coverage**, not a curriculum — which is why recording it does not violate
`curriculum-emerges-from-reading`.

### The recommendation

Start with the **structural-analysis chapters**: Cellier & Kofman, *Continuous System
Modeling*, **Ch. 9.3-9.5**.

**The reason is a control-of-variables argument, and it is the whole point.** Two things are
being tested the first time Doug works a textbook problem inside HRW:

1. **The loop** — read a narrative, work here, solve the problem, does the round trip hold
   together?
2. **The fit** — does HRW have anything to show for *this particular* piece of mathematics?

If the first problem is drawn from a chapter where Rumoca's coverage is thin, a failure is
uninterpretable: the loop and the fit failed together and nothing says which. **Ch. 9.3-9.5 is
where Rumoca's fit is best** — structural singularity, Pantelides, index reduction, tearing —
so a failure there is a failure of the loop, which is the thing actually under test.

Numerical-integration theory is the opposite end: largely pencil work, with little for an
observatory to show. **Expect a lopsided fit across chapters and do not design a uniform
process around it.**

### The risk this carries, stated in advance

**Claude being wrong is a false positive on the very test being relied upon.** The mitigation
is **#43**: prefer computation over assertion. Cellier says index 2 → *watch Pantelides reduce
it*. Claude says a block is well-conditioned → *compute the condition number*. An answer that
can be executed is worth more than one that can only be believed.

**Relates to:** #5 (four-bar — the closed kinematic chain is exactly the index-3 DAE these
chapters explain), #43, #53 (ad hoc curricula), `question-ledger.md`.

---

# Delivered and closed

**These items are done. Their planning prose was deleted 2026-08-01** — it described work that
now exists as code, and a backlog is for what has not been built.

**The numbers are kept, and that is the point.** They are cited **55 times** outside this file,
18 of those to #40 and 12 to #44, many from source-code comments. A deleted number turns every
one of those into a reference to nothing — the exact silent rot this cleanup exists to stop. So
each keeps one line saying what it was and where it landed; the reasoning is in git history,
`../DECISIONS.md`, and the code itself.

| # | What | Delivered | Where it landed |
|---|---|---|---|
| **#1** | Narratives for *simulation*, especially convergence-failure troubleshooting | 2026-07-21 | `gen_trace` runs simulation after compilation |
| **#2** | Specimen *purpose hints* — in the file and in the app UI | 2026-07-20 | the `// purpose:` convention; `read_purpose`, shown under the filename |
| **#3** | Directory naming / organization | 2026-07-20 | `docs/understanding/` → `docs/compiler-phases/` |
| **#6** | Initialization stage: detect over/under-determined *user* initialization | 2026-07-20 | over-determination only; the rigorous form is **#7**, still open |
| **#8** | Step-mode plotting for discontinuities | 2026-07-21 | `worker::discontinuity_segments`, gated on `SimData.has_discontinuities` |
| **#12** | HRW architecture document — how the code works | 2026-07-21 | `architecture.md` |
| **#14** | Rank deficiency visualization in the incidence matrix | 2026-07-22 | unmatched rows and columns get faint red bands |
| **#16** | Animated BLT block discovery (Tarjan’s SCC algorithm) | 2026-07-22 | `tarjan_anim.rs` |
| **#25** | Live breakpoint arming on an already-running debug session | 2026-07-24 | the HRW Debugger Bridge extension; protocol in `debug-set-sites.md` |
| **#27** | Equation sheet — the flat DAE in readable math notation | 2026-07-25 | `equation_sheet.rs` |
| **#28** | Source-to-equation traceability — bridging the OO/flat divide | 2026-07-25 | `source_map_ui()` in `app.rs` |
| **#29** | Solver stepping visualization — what the integrator does at each step | 2026-07-25 | solver diagnostics: step size, Newton iterations |
| **#32** | In-app tour view — tours rendered inside HRW with clickable navigation | 2026-07-25 | three UI modes; superseded in shape by **#42** ad hoc tours |
| **#39** | Crash and diagnostic log — troubleshootable without a live session | 2026-07-28 | `src/diagnostics.rs`, `examples/crash_probe.rs`; `architecture.md` §9 |
| **#40** | Instrument `pre()` lowering | 2026-07-29 | `pre_lowering_anim` on the Events stage. **The finding** — phases take an *observer callback*, not a `LiveTrace`, since that dependency would run backwards through the pipeline: `DECISIONS.md` 2026-07-29 |
| **#44** | Show `Matching ▶` when the Structural stage is singular | 2026-07-29 | one UI condition — the feature was *written and then gated out of reach*, and nothing tested it: `tech-debt.md` |

**Not listed here, and deliberately:**

- **#40a** — the original proposal text for #40, retained *for its rationale*, which is the one
  thing a tombstone cannot carry.
- **#50** — *declined*, not delivered. Its entire job is to stop test-coverage measurement being
  re-proposed, so deleting it would invite the proposal it exists to refuse.
- **#42** and **#45** — **partly** delivered, and still live. #42's ad hoc tours and #45's
  diagnostic audits have shipped sub-items marked in place; each still has open work.

---

## 58. A reading path for HRW, then a structural pass on `app.rs`

**Ported here 2026-08-01** when `current-work.md` was deleted — its work was done, but this
reasoning was future-facing and lived nowhere else.

### Why HRW now needs studying, like Rumoca does

**33,964 lines across 33 modules**, against Rumoca's 138,987 across 53 crates — about a
quarter, and no longer trivial. Doug, 2026-07-31: *"I'm definitely going to have to consider
HRW to be a subject of focused study, just like rumoca."*

But the complexity is **concentrated, not diffuse**: `app.rs` is 9,039 lines and `worker.rs`
5,668 — **43% of all HRW code in two files.** And unlike Rumoca's, much of it is *accidental*
rather than essential: a 9,000-line UI module is not inherent to what HRW does. **So part of
the answer is not "study it harder" but "make it smaller."**

### The gap a reading path fills

HRW has ~19,000 lines of documentation across 64 files — generous against 34k of code — but
[`architecture.md`](architecture.md) is a 1,500-line **reference**. It answers *"how does X
work"*, never *"where do I start"*. Rumoca has [`compiler-phases/`](compiler-phases/) for
exactly that; HRW has no equivalent.

### Both are deferred until after the corpus list, on purpose

The rule is one this project already holds — **skip debt a later phase will rewrite**
(`feedback-tech-debt-sweeps-serve-future-phases`). The corpus list touches `app.rs` heavily:
the specimen list becomes a filtered corpus browser. A structural pass or a reading path
written before it would be **partly obsolete on arrival**, and `CLAUDE.md` already defers
splitting `central_panel_ui` for the same reason.

**Relates to:** [`tech-debt.md`](tech-debt.md)'s `app.rs` entry (up 42% in three days, logged
rather than swept), #52.

## 59. A draggable LHS/RHS divider, with 40/60 as the opening default — BUILT 2026-08-02

**Delivered.** `SplitState` on `App`: both panels are `resizable`, clamped to 15–75 % of the
window, and reset to 40/60 on every mode switch.

**Who owns the width, which is the whole design.** egui does, while the reader drags — a
`Panel` remembers its width under its own id, so forcing a width every frame would fight the
drag. `SplitState` holds the last width *observed* (so the split is a number a test can read,
per H6) and a one-frame **reset request**. The reset must happen during rendering rather than
at the mode switch, because `Panel::exact_size` collapses the size range to a point and that is
what makes egui forget a dragged width.

**The split is a FRACTION of the window, not a stored pixel width** *(fixed 2026-08-03, and
confirmed working; five attempts)*. Doug: *"when HRW starts, too much horizontal space is given
to the LHS"* — it opened at 75 %, which is exactly `MAX_LEFT_FRACTION`, so the panel was being
**clamped at its maximum** rather than landing on any particular width.

Instrumenting settled in one restart what four theories had not:

```text
split: 0.400 of window (panel 2000px, available 5000px)
split: 0.750 of window (panel 1290px, available 1720px)
```

**The first frame reports a 5000 px window that does not exist.** 40 % of it is 2000 px, egui
stores that as an *absolute* width, and on the real 1720 px window it exceeds the maximum and
clamps to 0.75. The stored width is now rewritten whenever the available width moves, which
makes the fraction authoritative and gives window resizing the behaviour a reader expects.

**Four superseded attempts, kept because the sequence is the lesson**: a one-frame force, a
300 ms hold, unified panel ids, and clearing eframe's persisted `PanelState`. Each was a guess
about numbers nobody had looked at, and each cost a round trip through Doug. **Instrument
before theorising, once reproduction has failed even once** — `ui-findings.md` C15.

**Both edges are clamped, and that is not fussiness.** A divider draggable to zero hides a panel
*with no handle left to drag back*.

**The field-count ratchet fired**, which is the intended outcome rather than a nuisance: it
asked whether the field belongs on `App`, and the honest answer is yes — the split is window
layout, used by both panels and owned by neither, so there is no pane to push it into. Raised
57 → 58 with that reasoning in the same commit.

**What stays Doug's to judge**: whether the handle is findable and whether dragging feels
right. The tests cover the opening fraction and the reset; neither can tell you if a drag reads
as a drag.


**Doug, 2026-08-02.** The horizontal split between the left panel (tour text, or the specimen
list plus source/purpose) and the right panel (stages, log, animations) is **fixed**, and he
wants to drag it. **40/60 stays the default** when Specimen or Tour mode opens.

### The problem it solves

Every pane in HRW competes for the same width, and which one needs it **changes with the
question**. Reading MSL source wants the left; comparing an equation sheet against an
incidence matrix wants the right. Today neither can win, so the answer to "can I see enough
of this?" is always *no, and there is nothing you can do*.

This got sharper the moment MSL models became browsable. A library file is nested several
packages deep with long signatures — `Blocks/Continuous.mo` lines run well past what 40% of
the window shows — which is the same pressure that produced the horizontal-scroll defect Doug
reported on 2026-08-01.

### What is actually there now

`LEFT_PANEL_WIDTH_FRACTION = 0.4` in `app.rs`, applied at **two call sites** as
`ui.available_width() * LEFT_PANEL_WIDTH_FRACTION` — one for Tour mode, one for Specimen mode.
So the constant is already named and centralised; what is missing is that the width is
**recomputed from scratch every frame**, leaving nowhere for a drag to be remembered.

### Shape of the work

- The fraction becomes **state on `App` rather than a constant**, initialised to `0.4`.
- egui's `SidePanel` has `resizable(true)` and reports its width, which is the natural
  mechanism — but HRW draws this split with `available_width()` arithmetic, not a `SidePanel`.
  **Check which it is before assuming**; converting the layout is a larger change than adding
  a drag handle to one that is already a panel.
- **Reset to 0.4 on entering a mode**, per Doug's requirement. Note this makes the default
  *re-assert* on every mode switch, which is a deliberate choice and not the same as
  persisting one width for the session — worth confirming that is what he meant if the two
  ever feel different in use.
- **Clamp both sides.** A divider draggable to zero can hide a panel with no way back, which
  is a worse failure than a fixed split.

### Testable, and by which half

`source_scroll_offset` established the pattern (`ui-findings.md` H6): **geometry is checkable
when the app records it.** Here the fraction *becomes* app state by definition, so the drag's
effect is a number a headless test can read — that it changes, that it clamps, and that a mode
switch restores `0.4`. **What stays Doug's** is whether the handle is findable and whether
dragging it feels right.

**Relates to:** the UI pause ([`ui-pause-plan.md`](ui-pause-plan.md)) — this adds a field to
`App`, so it lands *after* the field-count ratchet exists, and is a fair early test of whether
the ratchet does its job: the honest resolution is that the fraction belongs to whatever owns
the layout, not to `App`.

---

## 60. Seeing how Doug uses HRW — the professor's pause, and what it needs to work

**Raised by Doug, 2026-08-03**, at the end of the day the first two curriculum tours landed.
**Not decided** — he was explicit that we lack the experience with the existing tours to choose
a design, and that we resume with this first.

### The model he described

> *"If you were a professor delivering a lecture, you might pause to ask me a question. My
> answer would be a signal to you about whether or not what you had presented right before my
> question had landed with me or not. And, if what you had presented had not landed, you might
> adjust your presentation. Or, in our HRW case, you might adjust the tour."*

That is a **feedback loop with three parts**, and only one of them exists today:

| Part | State |
|---|---|
| Doug's question is a signal about the material just presented | ✅ happens every session |
| Claude knows **what was just presented** | ❌ not reliably — see below |
| Claude **adjusts the tour** in response | ✅ possible, and cheap: tours are regenerable |

**The broken link is the middle one**, and it is what makes the loop lossy. Doug's governing
statement, and the reason this is not a small feature:

> *"I will probably be best served if you have more information about how I'm using HRW rather
> than less."*

### What Claude can see today, measured 2026-08-03

Nothing is pushed; everything requires going to look. Two files carry it.

- **`.hrw-bridge/diagnostics/session.json`** — a capped ring of recorded actions, including a
  **`tour-link` entry for every link clicked**, plus an `app` snapshot (`ui_mode`, `model`,
  `stage_tab`, `viewing_log`). Live example read that evening: `ui_mode: Tour`,
  `model: SingleInertia`, `stage_tab: DAE`, last action `tour-link load/SingleInertia/Dae`.
- **`.hrw-bridge/focus.json`** — the capture, written when Doug points at something and asks.
  Specimen, stage, view, node path.

### The specific gap, and why it is not a matter of trying harder

**Neither file records which tour is open, or which stop.** The tour is often inferable from
the specimen and stage — and inferring it is exactly the mistake
[`identity-and-provenance.md`](identity-and-provenance.md) forbids, because the inference is
**not sound**: `hrw://load/SingleInertia/Dae` occurs **three times** in `dae-construction.md`,
at three different stops. A bare link URI cannot name a position.

**Sequence alignment does better and is still not enough.** The ordered run of `tour-link`
actions can be matched against the tour text to locate a position by *order* rather than by
name, which is sound — but it degrades silently once the ring drops the earlier entries, and a
silent degradation to a guess is the failure mode this project treats as worst.

### The minimum that would close it

**Record the tour's identity with the link, and the stop index when a walk is playing.** During
autoplay the stop is known exactly (`Beat::stop`); a manual click knows the tour but not which
occurrence of the link it hit — so the honest emission is *tour + link + stop-if-known*, with
the absence stated rather than filled in. **The emitter must stay exact**
(`feedback-emitter-correct-reasoner-supplements`): a wrong stop is worse than a missing one,
because it would send Claude to adjust prose Doug never read.

### Why this became live now

`question-ledger.md` gained a section on 2026-08-03 recording Doug's grading criterion for the
curriculum tours — *"the real measure … will be the nature of the questions which I ask"* —
and the procedure it describes logs each question **against the stop that prompted it**. That
mapping is the whole mechanism, and today it depends on Doug saying where he was. **The ledger
entry and this idea are two halves of one loop**, written the same evening.

### What is deliberately open

- **How much more.** Doug's *"more rather than less"* is a direction, not a specification.
  Dwell time per stop, revisits, which stops are skipped, whether a tour was walked or played —
  each is a candidate and none is chosen.
- **Whether HRW should prompt.** The professor's pause is a *question asked of the student*,
  and nothing in HRW asks Doug anything. Whether a tour stop should be able to pose a question
  — and what would be done with the answer — is a genuinely different feature from observing
  behaviour, and should not be smuggled in with it.
- **The privacy-shaped question, stated once so it is not rediscovered:** this is instrumenting
  a person's study behaviour. Doug asked for it, it stays local to his machine, and the
  artifacts are already gitignored. Worth re-checking only if any of those three change.

**Do not design this before walking the existing tours.** Doug's reason is the right one: the
experience of using `dae-construction.md` and `matching.md` is what will say which signals
actually matter, and building to a guess would produce the wrong instrument confidently.

---

## 61. Quizzes — the same visualizations, run backwards

**Raised by Doug, 2026-08-03**, immediately after #60 and as its natural completion:

> *"Perhaps the same HRW visualizations which are being used now to deliver information can
> also be used for convenient quizzes. For example, there might be a quiz which requires that I
> correct on the next cell in a matrix that will be impacted by a matching algorithm. Along with
> the questions which I ask you here, my answers during those quizzes would be a measurement
> for you."*

**It makes sense, and "quiz" undersells it.** Three properties make this different in kind from
putting questions in an app.

### 1. HRW already holds the answer key, and it cannot rot

The matching animation's frames *are* the ground truth: "which cell next?" is literally the
content of frame *n+1*. So a question is **self-authoring** (any specimen with a trace
generates them), **self-grading** (an exact index comparison, no heuristic and no answer key to
maintain), and **incapable of going stale**, because the key is the algorithm rather than a
transcription of it. That last point is the one paper cannot match — and it is the same
property that makes the fixture tours' numbers trustworthy.

### 2. It inverts the composition primitive rather than adding one

Today Doug points at a thing to **ask** about it. Here he points at a thing to **answer**. Same
gesture, same noun, opposite direction — and the primitives are frozen
(`CLAUDE.md`), so **this must not become a third primitive.** It is the existing point-at with
a different consumer.

**The machinery is further along than it looks:** `incidence_view.rs` already hit-tests cells
(`hovered_cell`, `response.clicked()` — lines 308 and 443). What is missing is the *question*
and the *grading*, not the pointing.

### 3. It resolves the question #60 left open

#60 recorded *"whether HRW should prompt"* as deliberately undecided and warned against
smuggling it in with passive observation. **Doug has now answered it**: yes, and this is the
form. So #61 is not a separate feature — it is #60's active half, and the two should be read
together.

**The signals differ in kind, and that is the point.** Passive observation says what Doug
*looked at*. A wrong quiz answer says what he *believes*, localized to one algorithm step.
That is the more diagnostic of the two by a wide margin, and it is the only one that can be
wrong.

### Where this beats paper, and where it does not

**The dividing line is selection versus construction.** HRW wins where the answer is a *choice
over a structure already on screen*: which cell, which equation fails, which block is coupled,
which variable to tear, what order to evaluate. Paper still wins where the answer is *built*:
differentiate this constraint, write the residual, derive the Jacobian entry. **A quiz that
tries to accept a constructed answer would be re-implementing a worse text editor**, and the
line should be held.

### The risk worth designing against

**"Predict the next frame" can be passed by pattern-matching the animation.** A learner who has
watched enough matching runs can anticipate the mechanics without holding the concept — and
would score well while learning little, which is the worst outcome available here because it
would also mislead the measurement.

Two mitigations, both cheap: prefer questions whose mechanical answer *requires* the concept
(*"which equation will fail to find an augmenting path?"* cannot be answered by watching, only
by understanding rank deficiency), and ask for the **reason** alongside the click, which is a
question Claude can grade even though HRW cannot.

### Not before the tours are walked

Same constraint as #60, and Doug set it: the experience of using `dae-construction.md` and
`matching.md` is what will say which steps are worth asking about. **A quiz on a step nobody
found confusing measures nothing.**

---

## 62. Organizing the tours list

**Raised by Doug 2026-08-03** while the matching tour was being built, and **re-raised
2026-08-05** — *"I believe that we added an item to the ideas backlog to organize the growing
list of tours."* **We had not.** The thinking happened in conversation and scrolled away, which
is exactly the failure the scenario-1 rule in [`../CLAUDE.md`](../CLAUDE.md) exists to prevent:
*the rationale must live in the repository, not in the chat*. Recorded now, a day late, as its
own small evidence for that rule.

### Why it becomes live after #46

The list is **8 tours**, which is browsable. **#46 adds one failure tour per compiler phase**,
taking it to roughly **15** — and they are not peers of the existing ones. A learner opening the
picker would see `dae-construction` (a curriculum tour meant to teach), `frame-seeking` (a
capability test), and `parse-failure` (a demonstration that something breaks) with nothing
distinguishing them.

**Doug's stated value is expectation-setting**: *"per-phase tours enable me to set expectations
and focus, which makes good use of my scarce attention."* A flat list of 15 works against that
the moment the names stop being self-explaining.

### The shape sketched, not chosen

Front-matter in each fixture tour, three fields:

- **`kind`** — `curriculum` (teaches a concept), `capability` (exercises an HRW feature),
  `failure` (shows a phase refusing). This is the distinction the flat list loses.
- **`chain`** — which compiler phase it sits at, so the list can order by the pipeline rather
  than alphabetically. `the-chain-of-problems.md` already defines that order.
- **`requires`** — a tour that assumes another has been walked. Currently implicit and only in
  Claude's head.

**Deliberately not built** when first discussed, on the grounds that the picker had 8 entries and
sorting was speculative. **#46 is the evidence that changes it** — revisit immediately after,
per Doug 2026-08-05.

**What to check first**: whether `kind` alone is sufficient. Three groups in a picker may be all
the structure 15 tours need, and `chain`/`requires` may be solving a problem the grouping already
solves. Build the smallest of the three that works.

---

## 63. Answer from a tour that already exists

**Raised by Doug 2026-08-05**, immediately after the failure tours landed:

> *"Sometimes you might be able to answer a question by leveraging an already-existing tour, such
> as a failure tour."*

### The gap

Claude's answering repertoire today is **text**, then **an ad hoc tour** (`✨ Claude's answer`,
written to `.hrw-bridge/tour.md`). There is no third move for *"the answer is already on disk;
walk `failure-typecheck` stop 2."*

**So a question whose answer exists gets a freshly-written tour instead**, which costs a
regeneration, produces a second telling of something already told, and — worst — **loses the
expectations**. A fixture tour's `**Expected:**` lines are versioned and were checked; an ad hoc
retelling has whatever Claude remembers of them.

### Why it did not matter until now

**Fourteen tours is the threshold.** With three or four, Claude held them in mind. The failure set
alone added six, they are named by *phase* while Doug thinks in *specimens* — he went looking for
a "DimensionMismatch tour" and there is a `failure-typecheck` — and nothing in the repository
says what each one covers without opening it.

### What it needs

- **A catalogue Claude can read cheaply**: for each tour, the specimen(s), the phase, the kind,
  and the question it answers. **This is the same front-matter [#62](#62-organizing-the-tours-list)
  proposes for the picker** — which is the argument for doing #62 first and letting both consumers
  read one source. `feedback-claude-is-the-context-consumer` applies: design the front-matter for
  Claude's lookup, and let the picker use what is there.
- ~~**A way to say "start here"**~~ — ✅ **BUILT.** `hrw://tour/<name>/stop/<slug>` opens a
  fixture tour at a named stop, so an answer can be *"walk `failure-typecheck` from stop 2"*. The
  form existed from the start; the handler recorded a destination that **nothing consumed**, so
  every such link opened its tour and landed wherever the pane happened to be. Fixed 2026-08-17.
  **The `unbuilt:` tag here was false for two days and could never have fired**, because its
  target named a phrase rather than a symbol — `sweep-2026-08-19.md`, Finding 1.
- **A rule about when to reuse rather than write.** Reuse when the existing tour's expectations
  answer the question as asked. Write fresh when the question is about a *different* specimen or a
  narrower slice — a tour that nearly fits, walked as though it fits, is worse than a new one.

### The trap to avoid

**Do not point Doug at a tour without checking it still holds.** Tours go stale silently — nothing
compiles them, and `failure-typecheck` promised a tree that the pane did not show for the whole
time it existed. **Citing a tour is making its claims your own**, so the reuse path must include
re-reading it, not just naming it.

---

## 64. Promote "Claude's answer" to a fixture from the tour list

**Raised by Doug 2026-08-05**, in the same breath as [#63](#63-answer-from-a-tour-that-already-exists):

> *"Add an item to the ideas backlog to provide a convenient context menu item for the 'Claude's
> Answer' tour to promote that tour and its specimens to fixtures."*

### The problem

An ad hoc tour is **ephemeral by construction** — `.hrw-bridge/tour.md` is gitignored, as are the
scratch specimens in `.hrw-bridge/specimens/` it usually references. That is right: most answers
should not become artifacts.

**But some should**, and the ones that should are only identifiable *after* Doug has walked them.
Today that promotion is Claude editing files by hand at Doug's request, which means it happens
when Doug thinks to ask and not when he notices the tour was good.

**The moment of recognition is while walking it.** A right-click on `✨ Claude's answer` in the
tour list is where the decision belongs.

### What promotion actually involves — more than a file move

This is the part worth designing before building. A fixture tour has **obligations an ad hoc tour
does not**:

| | ad hoc | fixture |
|---|---|---|
| Location | `.hrw-bridge/tour.md` | `docs/fixture-tours/<name>.md` |
| Links | unchecked | **`fixture_tour_links_all_resolve` runs on every test** |
| Specimens | `.hrw-bridge/specimens/*.mo`, no rules | `specimens/*.mo` with a `// purpose:` comment |
| Per specimen | nothing | `docs/specimen-notebook/<Model>/purpose.md` **and** a generated trace |
| Naming | one file, overwritten | a name that will not collide, and does not shadow a curated specimen |

So the action is: **move the tour, move each referenced scratch specimen, generate the traces,
and stub the `purpose.md` files** — then the suite tells you what is still missing rather than the
promotion silently producing a fixture that fails the next test run.

### Design questions, unanswered

- **Who names it?** The ad hoc file has no name. A dialog is friction at the moment of
  recognition; a derived name (`answer-2026-08-05.md`) is checkable but meaningless later.
- **What if a scratch specimen shadows a curated one?** That collision is already reported and
  the file skipped (charter §4.3) — promotion must refuse rather than overwrite.
- **Should it promote, or stage?** Writing the files and leaving them uncommitted lets Doug and
  Claude finish the `purpose.md` prose together, which is the part a menu item cannot do.

**Depends on nothing; blocked by nothing.** Worth doing after [#62](#62-organizing-the-tours-list),
since a promoted tour needs the same front-matter a hand-written one does.

---

## 65. "Claude's answer" as the centre of the UI

**Raised by Doug 2026-08-05**, on watching the tour-citation mechanism work:

> *"I have a hunch that 'Claude's Answer' is going to become a focus of the HRW UI, if not THE
> focus. I have a hunch that we will ultimately find a better, more central location for
> 'Claude's Answer' than as an item in the tours list."*

And the larger framing, the same message:

> *"Whether we've noticed or not, we have been very quickly evolving HRW away from the pre-AI
> conventions for applications and UI to a contemporary vision which places Claude at the
> centre."*

### What is actually wrong with where it lives now

`✨ Claude's answer` is **one row in a list of fifteen**, sorted beside fourteen versioned
fixtures. That placement makes three claims that are all false:

1. **That it is a peer of the fixtures.** It is not. A fixture is durable, versioned and
   checked; the answer is *the response to the question just asked*, and it is overwritten by the
   next one. Those are different kinds of object sharing a widget.
2. **That it is chosen by browsing.** A list is a **menu of nouns you pick from** — the pre-AI
   convention. But the answer is not something Doug goes looking for; it is something that
   *arrives*, in response to a thing he said.
3. **That finding it is free.** Charter Decision 9 says otherwise: after asking a question, Doug
   switches to Tour mode, finds the row, clicks it. **Three actions to reach a thing that was
   produced for him seconds ago**, each of them friction spent on operating the instrument rather
   than learning.

### Why this follows from the charter rather than being a new direction

**Decision 8 — the noun is assembled by mouse, the verb is an unbounded utterance.** A list of
pre-written tours is *a menu of verbs*, which is exactly what that decision says cannot work. The
answer surface is where unbounded utterances and their responses live, so the thesis implies it.

**Decision 9 — minimize learning friction.** The answer is the single highest-traffic artifact in
the app and currently the most buried.

**So this is not a new idea. It is the first two decisions applied to the layout**, which is
probably why Doug reports it as a hunch rather than a proposal — the conclusion arrived before the
argument did.

### The reconciliation, which #63 already built

The obvious objection: **the fixture tours are valuable and Doug walks them independently.**
Making the answer central must not make them second-class.

**It does not, and the shape is already in place.** `hrw://tour/<name>/stop/<slug>` (built
2026-08-05) lets an answer *cite* a fixture at the exact stop that demonstrates the thing. So:

> **The answer is the index; the fixtures are the corpus.**

Doug does not browse tours and then read one. He asks, and the answer routes him into the durable
material — which he can then walk on his own, exactly as now. The tours list stops being the front
door and becomes what it always was: the shelf.

### Shapes, none chosen

- **A dedicated region** rather than a list row, always present, showing the latest answer.
- **The tour panel defaults to the answer** whenever it is newer than the last thing viewed —
  cheapest, and reversible.
- **The answer as the driving surface**: it composes prose, links and embedded views, and the
  stage tabs become a rendering target it points into. The most radical, and the closest to
  Doug's framing.

### Questions that decide it

- **What is the empty state?** Before any question is asked there is no answer, and the most
  central surface in the app would be blank. That is either the app's front door for asking, or a
  design failure — and which one is not obvious.
- **Does history matter?** The answer is overwritten by the next question. If it becomes central,
  is a walked-and-valued answer worth keeping? [#64](#64-promote-claudes-answer-to-a-fixture-from-the-tour-list)
  is one response (promote it); a session history is another.
- **What does it cost when Claude is wrong?** Decision 7 ranks accuracy above everything, and
  centrality raises the stakes: a wrong claim in a buried tour row misleads once, the same claim on
  the primary surface misleads by default. **This item should not be built without asking what
  makes an answer's claims checkable** — the tour-citation checker is a start, since a cited
  fixture's expectations were verified even when the surrounding prose was not.

### Not blocked, and deliberately not urgent

Nothing prevents starting. But **the evidence for the right shape is Doug using the mechanism
built today** — asking questions and being routed into fixtures — in the same way #64's open
questions are to be answered by watching #63 in use. **Build the cheap version (the panel
defaults to the answer), watch, then decide whether the radical one is warranted.**

**Confirmed in use, 2026-08-05.** The first real exercise of the mechanism — *"demonstrate how
Rumoca responds to a typecheck failure"* — worked as designed on the retrieval side: Claude read
`CATALOGUE.md` rather than recalling, re-verified the tour's claims with `failure_map` before
citing, and composed an answer linking into `failure-typecheck` at four stops.

**And it exposed a behavioural defect that is this idea's real subject.** Claude wrote the full
answer *twice* — once as `✨ Claude's answer` and once as prose in chat. Doug: *"Isn't your answer
going to be available as 'Claude's answer'? That is the least-friction solution."*

**Two copies of one answer is friction, not thoroughness.** It costs a second read, or the
attention spent deciding which is authoritative, and the two drift the moment either is edited.
**The substance belongs on the answer surface; chat gets a pointer plus what is not walkable** — a
caveat, what was verified and how, a question back. That is Charter Decision 9 applied to Claude's
output rather than to the UI, and it is a direct argument for **#65**: the answer is already the
place the content should live, which is exactly why its placement in a list row is wrong.

---

## 66. A curriculum tour for every phase and algorithm

**Stated by Doug 2026-08-05**, resuming the DAE tour:

> *"Eventually, we will implement tours like the DAE and matching tours, but for the other phases
> and algorithms."*

### What exists, and what this is not

Two **curriculum** tours: `dae-construction.md` and `matching.md`. They teach a concept, on a
specimen chosen so the concept is unavoidable — `SingleInertia` against `UnbalancedShaft`, one
line apart.

**Distinct from the six `failure-*` tours** built the same week under [#46](#46-a-failure-specimen--tour-for-every-compiler-phase).
Those show a phase *reporting trouble* and teach the `Failed`/`Flagged` distinction. A curriculum
tour shows a phase **working**, and teaches what it is *for*. Both are wanted; neither substitutes.

**The chain is already written**: `docs/compiler-phases/the-chain-of-problems.md` names the order,
and `CLAUDE.md` records index reduction on `Drivetrain` as the next link — *where a square system
is no longer enough because ideal gears make a state non-independent*.

### Why this is not simply "write eleven tours"

**The tours are what find the defects.** Doug, the same day: *"The testing of this tour has
yielded a great many bug fixes and feature enhancements."* That is the measured pattern, not a
hope — walking two curriculum tours produced, among others: the DAE tab that did not exist, five
tree-only stages that could not be pointed into, replays presented as the compilation, fabricated
BLT blocks, a pane that showed its error instead of its artifact, and the whole
source-provenance feature.

**So the rate limit is Doug's attention, not authoring effort.** A tour written faster than it can
be walked buys nothing, and the tours rule already says the scarce resource is *attention per
expectation*. **Write the next one when the previous has been walked**, which is also what keeps
each one honest — a tour written against an untested pane is a claim nobody has checked.

### What to do before each one

The pattern that has worked twice, and is worth following rather than rediscovering:

1. **Pick the specimen for the concept**, and prefer a *pair* one line apart. The contrast is what
   makes the concept unavoidable.
2. **Verify what the compiler actually does** — `failure_map`, the notebook trace — **before**
   writing a word. A first draft of `MissingComponentClass` asserted the wrong phase and was
   caught this way.
3. **Expect the composition to expose gaps**, and fix them rather than writing around them. Both
   existing curriculum tours did; that is the point of writing them.

### The phases are NOT equal candidates, because the goal is math and algorithms

**Doug, 2026-08-05:** *"The curriculum tours and the failure tours are pretty much the heart of my
effort to learn the parts of rumoca which I most care about now: math and algorithms."*

That is a **ranking**, and it had not been written down. Charter §1 says the pipeline is "a
physical enumeration of the subject", but the enumeration is uneven: some phases *are* the
mathematics and some are bookkeeping that has to happen first.

**Write these first — each is a named algorithm with a textbook behind it:**

| Phase | The mathematics |
|---|---|
| **Structural analysis** | maximum bipartite matching, Tarjan SCC, tearing as a Schur complement — and the incidence matrix is a sparsity pattern |
| **Index reduction** | Pantelides, dummy derivatives, DAE index theory |
| **Solve lowering** | residual programs, mass matrix, Jacobian sparsity |
| **Initialization** | the t=0 system, determinacy, over/under-determination |

**These later — they are real, and they are mostly bookkeeping:** Parse, Resolve, Instantiate,
Typecheck. Their *failure* tours already exist and carry most of what a learner needs from them,
which is a further reason not to spend curriculum effort there.

**Flatten, DAE construction and Events sit between.** DAE construction already has its tour;
Flatten's mathematics is connect expansion, which is graph-shaped and genuinely interesting;
Events is `when`-clause semantics more than analysis.

**One external constraint sharpens the order:** Doug's Purdue linear-algebra applications class
begins Fall 2026, and **structural analysis is the phase that is linear algebra wearing graph
clothing** — matching is a permutation, BLT is block triangularization, tearing is a Schur
complement. A curriculum tour there pays into the coursework directly
([[user-linear-algebra-learning]] in memory; `docs/vision.md`).

### Compilation is a MEANS, and the end is simulation

**Doug, 2026-08-05:** *"I'm learning the compilation stuff as a means to an end. That end is being
able to troubleshoot and improve simulations."*

**This does not change the ranking above, and that is the useful part.** Ranked by *mathematical
content*, the order is structural analysis, index reduction, solve lowering, initialization.
Ranked by *usefulness when a simulation misbehaves*, it is the same list for different reasons:

| Phase | What it buys at runtime |
|---|---|
| Structural analysis / BLT | the blocks are **what the solver actually solves**; their size is the cost |
| Tearing | sets how many variables Newton iterates on inside a coupled block |
| Index reduction | index > 1 is the classic source of a simulation that will not run |
| Initialization | non-convergence at t=0 is the most common practical failure |
| Solve lowering | Jacobian sparsity and residual programs — the inner loop |
| Events | discontinuities are what cause rejected steps and chattering |

**Two independent criteria agreeing is a stronger endorsement than either alone**, and it means
the plan does not need re-deriving. Parse, Resolve, Instantiate and Typecheck rank last on both.

### The three legs, and which one is thin

**Doug, 2026-08-05, on what he needs from a tour:** *"Problem statements, math and algorithms are
the most important for me to learn. And, gaining some understanding of how rumoca implements math
and algorithms to solve those problems."*

That is a **three-leg template**, and the first two are already the shape both curriculum tours
take — each opens with *"The problem this phase exists to solve"* before naming an algorithm,
which is `feedback-problem-before-solution` applied to prose.

**The third leg is the thin one.** `matching.md` shows the algorithm *running* — the animation is
the search, frame by frame — but never points into `matching.rs`. A reader finishes it knowing
what an augmenting path is and nothing about how Rumoca spells one.

**The material exists and is not reachable from a tour**: `docs/compiler-phases/*/guided-tour.md`
quotes line numbers, locals and enum variants, but its **audience is Claude**, and HRW's
*"Show this being set (debugger)"* verb arms a breakpoint without a tour ever suggesting it.

**So a curriculum tour should have an implementation stop**, and the cheapest honest form is a
stop that names the function and the debugger gesture — *"this is `augment` in `matching.rs`; arm
it and watch the recursion"* — rather than transcribing code into prose that will rot. **Nothing
compiles a tour**, and quoted code is the most rot-prone thing that can go in one
(`CLAUDE.md`'s standing rule about tours quoting line numbers).

### The one thing it does change: how a curriculum tour ends

A tour that closes with *"you now understand matching"* has taught the means and stopped. **Each
curriculum tour should end with what it buys when a simulation misbehaves** — one section, named
plainly, of the form *"when a solve fails/crawls, here is what this phase tells you."*

`matching.md`'s Stop 4 is half of this already: it ends on the permutation and what BLT needs it
for. What it does not yet say is that **the blocks are what Newton faces**, and that a large
coupled block is a slow simulation waiting to happen. That is the shape the section should take,
and it is cheap to add to a tour that already exists.

**Deliberately not written as speculation about the solver.** Until stage two instruments the
solve (`#68`), these sections say what the *structure* implies and stop short of claiming what
the numbers will do — `#69`'s caution, applied to tour prose rather than to a feature.

**Not scheduled.** The next is index reduction on `Drivetrain` when Doug reaches it.

---

## 67. Answering "where does this linear-algebra application appear in Rumoca?"

**Doug's stated workflow for the semester, 2026-08-05:**

> *"My linear algebra class is an applications class. Applications of linear algebra… As I'm going
> through my semester, I will mention to you which application of linear algebra is being
> discussed in my class. And then I will ask you if there's any relevance to that application
> within rumoca."*

**This is a question shape, not a feature request**, and it is the first one that runs *from* the
mathematics *into* the compiler rather than the other way round. Every tour so far starts at a
phase and explains its maths; this starts at a topic and asks where it lives.

### Why the existing machinery is not enough

[#63](#63-answer-from-a-tour-that-already-exists) makes tours findable, and `CATALOGUE.md` is
derived from them — so it answers *"which tour covers the DAE?"* and cannot answer *"which tour
covers **Schur complements**?"*, because no tour says that phrase. The vocabulary of the class and
the vocabulary of the pipeline do not overlap.

**The honest first answer is therefore a search of the source**, not a lookup. That is fine and
should be said out loud rather than papered over with a half-right index.

### The hazard, and the design rule it implies

**A concept map would be authored, not derived** — unlike the tour catalogue, nothing generates
*"tearing is a Schur complement"*. So it can be **wrong**, and a wrong entry does not merely fail
to help: it teaches Doug false mathematics, which Charter Decision 7 ranks as the worst outcome
this project can produce.

> **So an index may record WHERE TO LOOK, never WHAT THE MATHEMATICS IS.**
>
> A pointer (`matching.rs`, `failure-structural` stop 2, `docs/compiler-phases/phase7…`) is
> checkable and rots loudly. A claim (*"this is a Schur complement"*) is neither, and would be
> read as settled because it is written down.

**The mathematical framing gets made fresh each time**, against the code, with the oracle-first
discipline — and stated as reasoning Doug can check rather than as a fact retrieved from a file.

### Candidate correspondences, offered as leads and not as facts

**Untagged prose is a lead, not a fact** (`docs/provenance.md`), and every line here is a lead:

- **Permutation matrices** ← maximum matching assigns one unknown per equation
- **Block triangular form** ← Tarjan SCC over the dependency graph; the spy plot draws it
- **Schur complement** ← tearing, which picks iteration variables to shrink a coupled block
- **Sparsity patterns** ← the incidence matrix, and `structural rank` versus numerical rank
- **Rank deficiency** ← structural singularity, which `TwiceDefined` shows with an empty column
- **Jacobians** ← solve lowering's sparsity structure, and Newton on a torn block
- **Incidence / network laws** ← `connect` expansion generating Kirchhoff sums

**Each needs verifying against the source before being said to Doug.** `structural-vs-numerical-rank.md`
already exists as a fixture tour and is the one place some of this has been checked.

### What would actually help, if anything is built — AGREED by Doug, 2026-08-05

Not a map. **A worked answer, once**, for the first topic Doug raises — text plus a composed tour
citing the fixtures that demonstrate it. If a second and third look the same shape, *then* the
repeated part is worth extracting, and it will be extracted from things that were checked rather
than guessed.

---

## 68. Stage two — simulation, and why it needs a different instrument

**Doug, 2026-08-05**, looking past the compilation curriculum:

> *"After I learn all of the compilation stuff, I will begin looking forward to the second stage
> of this effort: simulation… I know from previous experience working at Caterpillar that figuring
> out how to solve simulation problems and how to improve simulation performance is what most
> people care about."*

**Already in scope, not a new direction.** Charter §1 names it: *"initialization and BDF
integration exercise numerical analysis, stiffness, and automatic differentiation."* Stage one is
the structural half of that sentence; this is the numerical half.

### What exists today, and it is thin

A Simulation tab, `simulate_specimen`, `simulate_library_model` (2026-08-05), and the solver
crates. **What does not exist is any instrumentation of the solve** — no capture scopes inside
BDF, no view of order selection, step rejection, Newton iterations or Jacobian reuse. The solver
is where HRW is today what it was for matching *before* the capture scopes: a result with no
visible process.

### The instrument is genuinely different, not just "more of the same"

| | Stage one — compilation | Stage two — simulation |
|---|---|---|
| Artifact | discrete, finite, inspectable | a trajectory over time |
| Determinism | same input, same IR | same input, same numbers **only to a tolerance** |
| "Correct" means | matches the compiler's own output | **agrees with another implementation** |
| The interesting event | a structure (a loop, a rank deficiency) | a *moment* (a rejected step, an event iteration, a stiffness onset) |

**Stage trees will not carry this.** The compilation views work because a phase produces a
structure worth freezing. A solve produces a *history*, and the analogue of the matching
animation is a step-by-step replay of the integrator — which is the same capture-scope pattern
pointed at `rumoca-sim` rather than `rumoca-phase-structural`.

### The consequence worth recording now

**The oracle stops being optional.**

Stage one can check itself: `docs/fidelity-plan.md` compares HRW's representation against
Rumoca's own IR, and a Rumoca bug faithfully rendered is a PASS. That works because a compiled
artifact is *checkable against its producer*.

**A trajectory is not.** Nothing inside Rumoca can say whether a state history is right — only
another implementation can. Charter **Decision 4** already built that rig (every specimen runs
through System Modeler and Rumoca, and disagreement is the most valuable event the setup can
produce), and `docs/ideas.md` **#43** has kept the oracle as a *track* rather than a step
precisely because stage one never needed it.

> **Stage two needs it. Plan the oracle before the simulation curriculum, not after.**

That also raises #43's priority for a reason it did not have before: it was *"an independent
adjudicator, useful for Doug's education"*; it becomes **the only way to know an answer is
right.**

### And a caution, from this project's own record

**Performance work is where wrong measurement is most seductive.** This week alone produced two
measurement errors that survived because they looked like conclusions: source spans declared
absent because a grep counted `"location"` and not `"span"`, and a `filter_map` audit that missed
nine sites hidden behind `str_vec`. A profiler produces numbers that feel like facts, and *"where
is the time going"* has the same shape as both mistakes.

**`docs/architecture.md` §11 and the fidelity sweep's per-check timing already exist** as the
pattern to follow: measure the thing, not a proxy for it, and say what was not measured.

**Not scheduled.** Stage one is not finished; this is recorded so the oracle's timing is a
decision rather than a discovery.

---

## 69. Tracing a simulation symptom back to a compilation cause

**Doug, 2026-08-05**, on what stage two is *for*:

> *"I will be interested in leveraging my knowledge of compilation to solve simulation problems.
> For example, if a simulation fails to converge, we'll want to trace backwards into compilation
> to identify causes. For example, if simulation runs too slowly, we'll want to consider how to
> instrument a model to identify opportunities to simplify the model."*

**This is the argument for the two stages being one project rather than two.** Stage one is not a
prerequisite to be got through; it is the vocabulary in which stage two's answers are written.

### The backward trace is the forward provenance, read in reverse — and its last link exists

The chain a non-convergence question would walk:

```
Newton fails on block B                      (solver — no instrumentation yet, #68)
  <- B is a coupled SCC                      (Tarjan, over the matched incidence)
    <- its members are f_x[i], f_x[j], ...   (the matching's permutation, matching.md Stop 4)
      <- each has a span                     (BUILT 2026-08-05)
        <- which is a line in the model      (BUILT 2026-08-05)
```

**The bottom two links shipped today** — the tooltip, the `📄 Show … in the Modelica source`
item, and the wash. They were built to answer *"where did this DAE node come from?"*, which turns
out to be the tail of every backward trace. The links above them are what #68 calls for.

**So the work has a direction it did not obviously have**: each provenance step is worth building
on its own for the tour it serves, *and* is a segment of this chain.

### The two questions want different things

- **"Why won't it converge?"** wants the chain above: symptom → block → equations → source. Mostly
  **structural**, and mostly already computable.
- **"Why is it slow, and what can I simplify?"** wants **cost attributed to structure** — which
  blocks dominate, how large the torn systems are, how often the Jacobian is refactorised — and
  then the same walk back to source. That needs per-block measurement the solver does not
  currently emit.

**Simplification is the more valuable and the harder one.** Knowing *which* equations to remove
is worth more than knowing why a solve failed, and it is exactly what Doug says practitioners
care about (`#68`, the Caterpillar observation).

### The caution, and it is the important part

**A structural story for a numerical failure is a fiction with a clean explanation.**

Non-convergence often has causes **no structural view can see**: bad scaling between states,
a poor initial guess, a genuinely ill-conditioned Jacobian, a discontinuity landing inside a
step. HRW will always be able to produce a *plausible* chain — this block is coupled, these
equations, this connect — because the chain exists whether or not it is the cause.

> **Presenting a plausible chain as a diagnosis is the same class of error as the fictions
> removed on 2026-08-04**, and worse, because it will be right often enough to be trusted.

The rules already written cover it and should be cited when this is built:
**Charter Decision 7** (accuracy outranks everything), **`#67`'s rule** (record *where to look*,
never *what the mathematics is*), and **`docs/provenance.md`** (untagged prose is a lead, not a
fact). The honest form of this feature says *"these are the structures involved"* and leaves
*"this is why"* to reasoning that can be checked — often against the oracle, which `#68` argues
becomes mandatory in stage two.

**Not scheduled.** Recorded because it tells stage one which provenance work pays twice.

---

## 70. Can Claude see where the debugger is stopped?

**Doug, 2026-08-05**, describing how he expects to learn Rumoca's code:

> *"When it's time for me to learn rumoca code details, then I will begin debugging rumoca code.
> Mostly, I will debug the algorithmic code. And most of the time, I will begin debugging that
> code by using the live trace feature… While live trace debugging, I might ask you about a line
> of rumoca code that I'm stopped at in the debugger. I don't believe that we've investigated
> whether or not you can query VS Code to determine which line of code I'm stopped."*

**He is right that it has never been investigated**, and it is worth knowing before the semester
rather than discovering mid-question.

### What is known, and what is not

**Known:** Claude runs inside the VS Code extension and **receives the editor's selection** —
the environment provides it explicitly, marked in the conversation context. That is why the
architecture puts Claude here rather than in a terminal, and why `docs/setup-windows.md` treats
the debugger as a first-class instrument.

**Not known:** whether **stopping at a breakpoint produces a selection.** A debugger stop
*reveals and highlights* a line, and a highlight is not necessarily a text selection. **These are
different mechanisms and the answer could be either way.**

**Do not assume it works.** A confident "I can see you are stopped at `matching.rs:214`" that is
actually Claude reading a stale selection is a wrong answer delivered with the tone of a right
one — the failure class this project spends most of its rules on.

### ✅ RUN 2026-08-07 — Claude cannot see the debugger; the click gesture works

Five trials, on Doug's Windows machine with live trace confirmed working:

| # | Setup | What Claude saw |
|---|---|---|
| 1 | Stopped, **nothing selected** | **nothing** — no location, no frame, no selection |
| 2 | Stopped, Doug reported selecting the line | **nothing** — see the correction below |
| 3 | **No debug session**, an ordinary line selected | the line, **verified against the file** |
| 4 | Stopped, **nothing selected** | **the FILE only** — an `ide_opened_file` event naming `live_trace.rs` |
| 5 | Stopped, **the stopped line selected** | **the line** — `live_trace.rs:173`, verified against the file |

**The answer to the title question is NO.** No trial ever produced a stopped location, a frame
or a call stack. Claude also searched the available tool surface: **nothing exposes editor or
debug-adapter state.** A debugger stop, by itself, tells Claude nothing about *where*.

**But the one-click gesture does work while stopped** (trial 5), which is what the entry
originally assumed and what trials 3 and 5 together now establish. `live_trace.rs:173` was read
back and checked against the file.

**A correction, because the wrong version was committed to this file first.** After trial 2 this
entry claimed *"the fallback did not work either"* and promoted the whole question from
convenience to capability gap. **Trial 5 contradicts it.** The trial-2 blank never reproduced
and is unexplained — most likely the selection did not register at that moment. **It is recorded
rather than deleted** so that a future session finding one silent blank knows it has been seen
before and is not the rule. The instinct that saved this was writing *"repeat trial 2 once
before building anything"* into the entry itself; it overturned the conclusion within the hour.

**Trial 4 found a channel this entry did not anticipate:** `ide_opened_file`. A stop reveals the
file, and that event does reach Claude — so **the file comes free, the line does not.** Claude
cannot distinguish "the debugger revealed this file" from "Doug opened it", so it is a hint, not
evidence.

**What Claude must NOT do, recorded because the temptation is concrete.** Claude can read
`.hrw-bridge/breakpoint-request.json` and recover the line HRW *asked* the extension to arm. At
the live-trace anchor that coincides with where Doug is stopped, so answering from it would look
like working debugger vision — until the day Doug sets his own breakpoint in `augment_traced`
and gets the anchor line back, stated with the same confidence as the dozen correct answers
before it. **Right often enough to be trusted is the failure mode**, not an approximation of
success.

### What to do, and it needs no code

**Select the line, then ask.** Verified in trial 5. One click, and Claude gets the exact text
plus the file, which is everything it needs.

**Or name the place, which never fails:** *"I'm stopped in `augment_traced` at the
`match match_var[var]`"*. Claude reads the file from there. Worth preferring when the selection
seems not to have landed — trial 2 happened once.

**`focus.json` stays UNBUILT, and the argument for it got weaker rather than stronger.** HRW
already writes `.hrw-bridge/focus.json` and the extension already watches that directory, so a
debugger stop could publish the same way — but that is a launch-configuration or extension
change for a problem one click already solves, and `#70`'s whole point was to find out before
building. It found out. **Do not build this without a new reason**, and a single unreproduced
blank is not one.

**The reopening condition, so this is a decision rather than a drift:** the click gesture
failing *repeatedly* in a real session. Then trial 2 was the rule and not the exception, and
this becomes real work.

### Why it matters more than it looks

The **third leg** of a curriculum tour is the thin one (`#66`): tours show an algorithm *running*
and never point into the code that runs it. Doug's stated remedy is to read the code in the
debugger and ask. **The quality of that exchange is what substitutes for tour prose that does not
exist yet** — so a friction of one click, repeated across a semester, is worth measuring rather
than assuming.

---

## 71. The Debug button says "armed" when nothing is listening

**Found 2026-08-07**, on the second Windows machine, because Doug asked whether the VS Code
extension had been built. It had not — and the more interesting half is what HRW would have
done about it.

### The defect

`App::live_debug_poll` ([`../src/app.rs`](../src/app.rs)) treats the two outcomes of the
handshake as one:

```rust
let acked = bridge::check_breakpoint_ack();
let timed_out = armed_at.elapsed() >= std::time::Duration::from_secs(3);
if acked || timed_out {
    self.pending_live_debug = None;
    self.live_breakpoint_armed = true;   // <-- true even when nothing acked
    return LiveDebugAction::SpawnLive;
}
```

**`live_breakpoint_armed = true` after a timeout is a claim that did not happen.** No ack means
either no extension installed, or an extension that did not process the request — in both cases
there is no breakpoint. HRW then spawns the algorithm thread, the animation runs to completion
without stopping, and **nothing on screen says why**.

### Why this belongs in the rules' own vocabulary, not the bug list

Two of this project's standing rules land on it directly:

- **The must-fire rule.** `check_breakpoint_ack` is a *reporter*, and its silence is currently a
  pass. The rule says silence must be a failure.
- **Nothing HRW shows may be invented.** `live_breakpoint_armed` is a state HRW asserts about
  the *outside world*, and after a timeout it asserts it without evidence. This is the same
  shape as the 2026-08-04 fictions — a plausible state, well-formed, and false.

**The three-second timeout itself is right and should stay.** A slow extension should not
deadlock the UI. What is wrong is that the fallback path is indistinguishable from success.

### What it would have cost

The failure is invisible in the direction that matters. A learner who presses **Debug** and
watches the animation run has no way to tell *"the bridge is not installed"* from *"this phase
has nothing to stop at"* — and the second is a plausible thing to believe about a compiler
phase. **`#70`'s experiment would have been run against a bridge that was not there**, and its
answer written into this file as fact.

### ✅ FIXED 2026-08-07, same day

1. **The timeout still spawns, but no longer passes for success.** `acked` proceeds silently — a
   notice on every Debug click would train the eye to ignore the one that matters — while
   `!acked` raises a status-bar notice naming the bridge and pointing at
   `vscode-extension/README.md`.
2. **`live_breakpoint_armed = acked`**, so HRW no longer asserts a breakpoint it has no evidence
   for, and the capture's `breakpoint_armed` key became true again in the literal sense.
3. **`LIVE_DEBUG_ACK_TIMEOUT`** now names the three seconds and is shared with the pre-warm,
   which had its own copy of the literal.
4. **`app::tests::a_timed_out_arm_claims_nothing_and_says_so`** covers both branches in one test
   (they share `.hrw-bridge/breakpoint-ack.json` and would otherwise race, the same reason
   `prewarm_arms_awaits_ack_then_removes` is combined). **Must-fire verified by reverting each
   half separately** — the armed assertion and the notice assertion were each shown to fail on
   the old code, since the first would otherwise mask the second.

   **UPDATE 2026-08-20 — it is four tests now, and the parenthesis above expired.** Giving
   `App::live_debug_poll` an `ack_path` parameter (`#74`'s ack-path seam) removed the shared
   file, so each verdict of `bridge::check_breakpoint_ack_at` drives a path nobody else reads:
   `an_armed_verdict_starts_the_run_and_stays_quiet` (`Armed`),
   `a_disabled_breakpoint_spawns_and_names_the_cause` (`NotArmed`),
   `a_stale_bridge_reply_claims_nothing_and_names_its_fix` (`Unreportable`), and this one
   (`Pending`). A failure now names the verdict rather than a line number.
   **Must-fire re-verified with three perturbations**: restoring `live_breakpoint_armed = true`
   fails the last three by name and leaves `Armed` green; making the `Armed` arm `notify` fails
   only the first; and blunting the `Unreportable` notice so it stops saying `npm run build`
   fails only the third. **The corrected sentence is kept above rather than deleted**, because
   the expiry is the finding: a comment explaining why a test cannot be split is a coupling
   measurement, and adding the seam it describes retires it silently.

**The accepted trade, recorded because it is a real regression in bookkeeping:** an ack arriving
*after* the timeout leaves a real breakpoint HRW no longer tracks. That is the honest side of
the trade — **HRW must not claim state it cannot see** — and `HRW: Clear Armed Breakpoints`
exists for exactly this. The previous tidiness was bought with a false statement.

**Done before `#70`**, since `#70` is an experiment whose result gets written down as fact, and
this defect is exactly what would have corrupted it.

---

## 72. Claude should see the debug session — location, stack, and values

**Doug, 2026-08-07**, immediately after `#70` settled what is available today:

> *"We will revisit this subject later as this capability is important for you to explain
> Rumoca's algorithmic code to me. You need to see what code I'm debug-stepping through, as well
> as stack traces and values. This is probably a very big bunch of functionality."*

**`#70` answered a smaller question and this is the real one.** That entry asked whether Claude
could see *where* the debugger stopped, found no, and found that one click closes the gap. Fine
for a single question. **It does not scale to stepping**, which is the actual use: Doug walks
through `augment_traced` or Pantelides, and asks what is happening *as it happens*. A click per
question, carrying one line of text, is a keyhole onto a running process.

**And the gap is not really the line — it is the state.** Claude can already read the source. The
questions worth asking during a step are *"what is `match_var` right now"*, *"how deep is this
recursion"*, *"which equation is this invocation working on"* — **none of which are in the
source.** Claude is currently reasoning about static code while Doug is looking at a running
program, and neither of them can see what the other sees.

### What is needed, in descending order of teaching value

1. **The call stack.** Sounds like the least interesting item and is almost certainly the most
   valuable, because **for `augment_traced` the call stack *is* the augmenting path.** Four
   nested frames means a four-edge alternating path, and each frame's `eq` is a node on it. The
   thing `matching.md` spends three acts animating is sitting in the debugger's stack pane,
   exactly and for free. The same holds for Tarjan's recursion.
2. **Variable values** — `match_eq`, `match_var`, `visited`, `eq`, `var`, `holder`. The partial
   permutation, mid-construction.
3. **The stop location** without a click. `#70`: the *file* already arrives via an
   `ide_opened_file` event; the line does not.
4. **Step events**, so Claude can follow rather than be told. The largest of the four and the
   least necessary.

**A slice of 1 + 3 alone would answer *"where am I and how did I get here"***, which is most of
the value for a fraction of the work. Do not start with values.

### Why this is big, and the standing rule that argues against it

The state lives in **VS Code's Debug Adapter Protocol**, in the extension host — reachable from
our own bridge extension (`vscode.debug.activeDebugSession` and `customRequest` for
`stackTrace` / `scopes` / `variables`), and reachable from nowhere else. Getting it to Claude
then needs a channel, and one already exists: `.hrw-bridge/` is a working file channel the
extension already watches, so the shape is *extension subscribes to debug events → writes
`.hrw-bridge/debug-state.json` → Claude reads it.*

**That is TypeScript and VS Code work, which `CLAUDE.md` says to avoid** — *"the deep-link effort
took five commits and several approaches and failed; new functionality defaults to the app
side."* **This is the strongest argument against, and it should be weighed rather than waved
past.** The difference worth noting: the deep-link work failed at *driving* VS Code's UI, while
this only *reads* documented API and writes a file — the same thing the breakpoint bridge already
does successfully. That is a reason to re-examine the rule here, not to ignore it.

### The accuracy rule this must carry from day one

**Every value Claude reports must be READ, never inferred, and a stale file must refuse to
answer.** A `debug-state.json` left over from the previous step would have Claude describe the
wrong `match_eq` with complete confidence — and Doug would have no way to tell that answer from
the correct ones. So the state file needs a **sequence number or timestamp that Claude checks**,
and "I cannot tell where you are" must remain a real answer.

This is the third time the same shape has appeared, which is why it is stated as a rule rather
than a caution: `#70`'s `breakpoint-request.json` (the armed line, which *coincides* with the
stopped line often enough to look like vision), `#71`'s `breakpoint_armed` (true because a
timeout expired), and now this. **Every one of them is a plausible substitute for a fact HRW
cannot observe.**

### First step is measurement, not code

`#70` earned this discipline the hard way — its own conclusion was overturned within the hour by
a repeat trial. **Before designing anything, find out what the adapter actually exposes:** the
launch config uses **`cppvsdbg`**, not CodeLLDB, and adapters differ substantially in what they
return for `variables` on Rust types. A `Vec<Option<usize>>` may come back as a readable list or
as an opaque pointer, and **that single fact decides whether item 2 is worth anything at all.**

### ✅ BUILT 2026-08-08 — the channel exists; what the adapter yields is now measurable

**Doug's call, overriding the standing preference against VS Code work**, which this entry had
recorded as the main argument against. The distinction that made it defensible: the deep-link
attempt failed at *driving* VS Code's UI, while this only **reads documented API and writes a
file** — exactly what the breakpoint bridge already does successfully.

**Shape, as designed above:** the bridge extension observes stops and writes
`.hrw-bridge/debug-state.json`; Claude reads it.

- **`src/debug_state.ts`** — payload assembly, **and it imports no `vscode`**, so `node --test`
  exercises it directly. This is the Rust-side `Plot::problems()` move applied to TypeScript:
  logic out of the untestable layer, thin wiring left behind. The existing
  `extension_surface.test.mjs` shows the alternative — several of its cases build a literal and
  assert its own fields.
- **`src/extension.ts`** — a `DebugAdapterTrackerFactory`, which is the only documented way to
  see a stop (**there is no `vscode.debug.onDidStop`**); `onDidSendMessage` carries the DAP
  `stopped` and `continued` events. On a stop it makes three round-trips — `stackTrace`, then
  `scopes` for the innermost frame, then `variables` for its most local scope.
- **11 new tests, and the invariants were verified must-fire** by breaking them: coercing
  `variables: null` to `[]` fails two tests, including the one that would otherwise let Claude
  report *"no locals"* about a frame it never managed to read.

**Three accuracy properties are load-bearing, not incidental:**

- **`null` ≠ `[]`.** `variables: null` plus `variablesError` means *not fetched*; `[]` means
  *fetched, none there*. Collapsing them is the fiction this feature is most likely to produce.
- **`continued` publishes too.** Without it the last stop stays on disk looking current, and
  Claude describes a position the program has left. A running payload blanks location, frames and
  values *even if a caller passes them* — tested.
- **Caps declare themselves.** `frameCount` is always the true total with `framesTruncated`
  alongside, because a shortened stack reads as a complete one — and depth is the whole point.
- **Staleness is the reader's duty.** Every write carries `seq` and `writtenAtMs`; `isStale`
  exists so a leftover payload cannot be mistaken for a current one. Nothing deletes the file on
  shutdown *by design*, since that check has to work anyway for a VS Code crash.

Written via temp-file + `rename` so a read can never tear.

### What remains, and it is now free to answer

**Item 4, step events, is deliberately not built.** Each stop already publishes, so Claude sees
every step it is told to look at; a push channel would only save the telling.

### ✅ MEASURED 2026-08-08 — the adapter gives us everything asked for

**The open question is answered: a `Vec<Option<usize>>` comes back READABLE, not opaque.** Once
expanded, `match_eq` reads `[0]=None`, `[1]=None`. So item 2 lives, and `#73` can lean on values
*as well as* the call stack.

Verified against a real breakpoint in `augment_traced` on a 2×2 system:

| Item (as ranked above) | Result |
|---|---|
| 1. Call stack | ✅ **20 frames**, innermost first, with paths and lines |
| 2. Variable values | ✅ **12 locals**, `Locals` scope, aggregates expanded one level |
| 3. Stop location | ✅ file + line + frame name |
| 4. Step events | ✅ every stop republishes, carrying `seq` and `writtenAtMs` |

**What one stop looked like** — line 189, `let can_augment = match match_var[var]`: `eq=0`,
`var=0`, `vars=[0]`, `visited=[true,false]`, `match_eq=[None,None]`, `match_var=[None,None]`,
`frames=[TryEquation, Explore]`. **That is enough to predict the algorithm's next four steps**,
and the stack showed **depth 1** — no displacement yet, Stop 1 territory rather than Stop 2. So
`#73`'s premise is confirmed rather than hoped for.

### Four findings, each bought with a wrong first attempt

**1. `levels: 0` returns NOTHING from `cppvsdbg`**, though DAP defines it as "all frames". The
first build sent exactly that, got an empty array, and could only say *"the adapter reported no
stack frames"* — true, useless, and indistinguishable from a thread that had none. **Fixed by
asking more than one way and publishing the tally**: `stackAttempts` records every shape tried
with the count it returned, `stackShape` names the winner (`levels=40`). **An empty result that
cannot say what was asked is not a measurement.**

**2. Aggregates arrive as summaries.** A slice renders `{ len=2 }`; elements live behind another
`variables` request keyed by `variablesReference`, which the first build discarded. **One level is
now expanded**, bounded by `CHILD_LIMIT` (64), truncation declared via `childrenTruncated`. This
is the field that matters most: **`match_eq`'s contents are Stop 4's partial permutation.**

**3. `cppvsdbg` MIXES SYNTHETIC CHILDREN IN WITH REAL ELEMENTS — the live trap.** `match_eq`
expands to `[len]=2`, `[0]=None`, `[1]=None`, `[Raw View]={data_ptr=0x…}`. **Only `[0]` and `[1]`
are elements**; the rest are MSVC visualizer artifacts, and a reader counting children would say
the slice has four. **Flagged rather than filtered on purpose:** a filter guessing which children
are synthetic would eventually drop a real struct field, and hiding data is the worse failure.
**Whoever consumes this must skip `[len]`, `[capacity]` and `[Raw View]`.**

**4. "Variable is optimized away and not available." is PROSE IN A VALUE FIELD**, and usually just
means *not live at this line*. At `for var in vars` (line 176) four of twelve locals read that
way — `var` unbound, `holder` in an unreached arm, `can_augment` assigned later, `iter` the
desugared iterator. **Stepping to 189 dropped it to two**, both correct. **The profile is
innocent**: `rumoca-phase-structural` is already `opt-level = 0`, and an early diagnosis blaming
it would have produced a pointless `Cargo.toml` change. Every local now carries
`available: boolean`, availability **recurses into children** (enforced by the type system, not
just a test), and `variablesUnavailable` is counted — `variableCount: 12` overstated what was
known by four.

### Operating notes for the next session

- **A breakpoint in `matching.rs` hits the ORDINARY COMPILE, not a live trace.** The stack ran
  `worker::structural_stage` → `build_structural_report` and `observer` was `None`. Live trace is a
  different path, entered from an animation's Debug button.
- **`npm run build` updates the installed extension in place** (the install is a junction), but a
  **window reload is still required** before new extension code runs.
- **A PULL DOES NOT INSTALL THE EXTENSION, and the failure is silent** *(found 2026-08-08 on the
  first machine, the day after this was built on the second)*. `git pull` brings `src/*.ts`; it
  does not run `tsc` and it does not create the junction. That machine had `out/extension.js` dated
  **2026-07-27** against a `src/debug_state.ts` dated **08-08**, and **nothing under
  `~/.vscode/extensions` matching `*hrw*`** — so the channel was dead while this entry, `#73` and
  `CLAUDE.md` all described it as working. **Per machine:** `npm install`, `npm run build`
  (`npm test` → 34 tests), the `New-Item -ItemType Junction` line from `setup-windows.md` §6, then
  reload the window; confirm with `code --list-extensions`. **The tell is the absence of
  `debug-state.json` after a stop** — which is indistinguishable from "no stop has happened yet",
  which is why it needs writing down rather than rediscovering.
- **Check freshness before trusting the payload** — `isStale`, `seq`, `writtenAtMs`. A payload from
  the previous stop describes the wrong state with equal confidence.
- Stop at a line where the locals are **live**: a loop head reports less than the body.

### What is left

- **Expansion is one level deep.** Enough for `match_eq`; a nested aggregate like `eq_vars`
  (`Vec<HashSet<usize>>`) shows its elements as `{ len=1 }`. **Do not deepen speculatively** — wait
  for a question it fails to answer.
- **`#73` is now unblocked**, and is the reason this was built.

---

## 73. Stop 5 should be a live-trace debugging session, not a map of the code

**Doug, 2026-08-08**, on reading Act 5: *"It seems that for Act 5, we have an opportunity to
accomplish something much more spectacular: live trace debugging."*

**He is right, and Stop 5 as shipped under-uses machinery that was built for exactly this.** It
names `maximum_matching_with_trace` and `augment_traced`, offers a breakpoint to set, and stops
there — ending the tour on *"go and read this"*, which is the homework failure the tour's own
*What this cannot check* section warns about.

### Three reasons this is the right shape

**1. The synchronization is designed, not lucky.** `LiveTrace::push` sends the frame, **sleeps
`frame_delay` so the UI can render it** — `matching_anim.rs` sets 20 ms — and *only then* calls
`live_trace_breakpoint`, where the debugger pauses all threads. So when execution stops, the
screen already shows the frame just produced.

Combine that with the emission order inside `augment_traced`: `Explore { eq, var }` is emitted
**immediately before `match match_var[var]`**. **At that stop the animation is showing the exact
edge the next line of code decides.** Screen and source in lockstep, by construction. A static
stop cannot do this at all.

**2. The call stack IS the augmenting path** — not an analogy. `augment_traced` recurses once per
displacement attempt, so N nested frames is an N-edge alternating path and each frame's `eq` is
a node on it. Stops 1-3 spend a dozen expectations animating that structure from outside; the
debugger holds it exactly, in the stack pane, for free. Plausibly the strongest single teaching
artifact in the tour.

**3. It turns the thinnest leg into the thickest.** `#66`'s three legs are problem, mathematics,
implementation. The third has been the weak one everywhere. This is what a strong one looks like.

### What must be settled first

**Two empirical unknowns, and they change how the stop is written** — neither is answerable
without walking it:

- **Do two breakpoints interleave cleanly?** With the anchor *and* one in `augment_traced`, does
  each frame give a tidy two-stop rhythm, or a confusing double-stop where the learner loses
  track of which one they are at? <!-- unverified -->
- **Does the screen stay legible while stopped?** The 20 ms delay guarantees the frame was sent
  and time was given — **not** that egui completed the paint. <!-- unverified -->

**And `#72` is load-bearing, not adjacent.** This stop can deliver the *seeing* today; the
*asking* — *"what is in `match_eq` now?"*, *"how deep are we?"* — is exactly what Claude is blind
to. **So `#72` comes first**, and its `cppvsdbg` measurement decides the design: if
`Vec<Option<usize>>` returns as an opaque pointer, item 2 of `#72` dies and this stop should lean
entirely on the call stack — which, per reason 2, is still most of the value.

### The risk, and the rule it produces

**A tour that promises synchronization and then drifts teaches something false**, which is worse
than homework — homework is merely unhelpful. **So every expectation here must be specific and
violable**: *"the stack shows 3 frames of `augment_traced`; the animation shows edge 1 → 0"*, never
*"the debugger and the animation stay in step"*. And it **must be walked before it is called
done**; Claude cannot verify any of it.

### Shape

**Keep the naming content — it becomes the setup rather than the payoff.** Function names are
what let a reader place a breakpoint at all. What changes is where the stop *ends*: at a decision
being made, with a question to ask, instead of at a reading list.

**This is the template for every algorithm tour's third leg** (Tarjan, index reduction, solve
lowering), so the shape is worth getting right once here rather than five times later.

### ✅ WALKED 2026-08-08 — both unknowns settled, and one claim above is wrong

Doug stepped `ProportionalLoop` through `augment_traced` with the anchor plus a breakpoint at
`matching.rs:189`, reading `debug-state.json` at every stop. **Twelve of twelve predictions about
the next stop held**, which is the evidence that the model Stop 5 will be written from is sound.

**Unknown 1 — do two breakpoints interleave cleanly? Neither answer above.** It is **not** a
two-stop rhythm, because the anchor fires for *every* frame while `189` fires only after
`Explore`. The real shape is: one startup gate, then one anchor stop per emitted frame, with a
`189` stop interleaved at each exploration. **It is legible only if each stop can be named**, and
the material for naming turned out not to be at the breakpoint at all:

| stop | how you know which it is |
|---|---|
| startup gate | `frame_index == usize::MAX`, `wait_for_debugger` on the stack |
| `TryEquation` | caller is `matching.rs:114` — **no `augment_traced` frame at all** |
| `Explore` | caller is `augment_traced:181` |
| `FoundFree` | caller is `augment_traced:191` |
| `TryDisplace` | caller is `augment_traced:202` |
| `DisplaceOk` / `DisplaceFail` | caller is `augment_traced:213` |
| `Assign` | caller is `augment_traced:233` |

**The discriminator is the CALLER's line number**, which exists only at runtime — no amount of
reading `live_trace.rs` would have produced this table, because at the anchor every stop looks
identical.

**The failure rows were added by the `TwiceDefined` walk below**, and the table is now complete
for the success *and* failure paths:

| stop | how you know which it is |
|---|---|
| `DisplaceOk` / `DisplaceFail` | caller is `augment_traced:213` — **the same line for both**, so this row names the site, not the outcome; only `frames.last()` distinguishes them |
| `EquationFailed` | caller is `maximum_matching_with_trace:133` |

**Unknown 2 — does the screen stay legible? Usually, but NOT reliably.** Read three times: in step
at `frame_index` 3 and 12, **one frame behind** at 11. The arithmetic explains it — `frame_delay`
is **20 ms** and a 60 Hz vsync interval is **16.7 ms**, so egui gets barely one frame period to
wake, drain the channel and complete a paint, starting from wherever it was in its own cycle.
**And the lag cannot recover while stopped**, because `cppvsdbg` freezes the UI thread too.

> **The first read was in step and was over-generalised into "Act 5 can promise
> synchronization."** One confirming observation is not a guarantee, and the correction cost
> nothing only because Doug read the screen a third time. **A tour that promised lockstep would
> have taught something false on roughly every third stop.**

**Fixed by the two-tier delay** (see `DECISIONS.md`, 2026-08-08): a stepped session gets a delay
comfortably longer than a vsync interval; a free-running one keeps 20 ms, because raising it
globally would make a thousand-frame `Drivetrain` trace sleep for minutes.

### The correction: "N nested frames is an N-edge alternating path" is WRONG

Observed at depth 2 — `augment_traced:181` over `augment_traced:210` — the path is
**eq1 → var0 → eq0 → var2**: two frames, **three** edges. The right statement:

- **N frames = N equation-nodes** on the path
- **N unmatched edges**, one per frame reaching for a variable
- **N − 1 matched edges**, one per displaced holder
- **2N − 1 edges total**

`maximum_bipartite_matching.md` already had this right (its
$e_0 - v_0 - e_1 - \dots - v_k$ form); only this entry was wrong. **The alternation is the
content**, and "N-edge" hides it — which matters directly for `#67`'s linear-algebra semester.

### Two instrument limits, both hit for real rather than anticipated

- **An anchor stop exposes only `frame_index`.** The innermost scope is `live_trace_breakpoint`,
  whose sole local it is; the algorithm's state is four frames up and the tracker fetches one
  scope. So **`173` tells you *which step*, `189` tells you *what the algorithm knows*** — the two
  breakpoints are not interchangeable and Stop 5 must say so.
- **`Option` payloads are invisible.** `match_var[0]` renders as `Some` with no holder, because
  `#72` expands one level and the payload is one deeper. On a 3×3 the holder is deducible from
  history; **on `Drivetrain` it would not be.** This is the concrete question `#72`'s "do not
  deepen speculatively" was waiting for.

**And the frame numbers disagree by one, permanently:** `frame_index` is 0-based (`push` uses
`fetch_add`, returning the pre-increment value) while the UI prints `cursor + 1`
(`lib.rs:230`). **`frame_index + 1` is the number on screen**, and a tour that quotes one while
the learner reads the other is a defect the learner will blame on themselves.

### The ledger, `ProportionalLoop`, every row read from a live stack

| idx | step | emit line | depth |
|---|---|---|---|
| 0 | `TryEquation(0)` | 114 | 0 |
| 1 | `Explore {0, 0}` | 181 | 1 |
| 2 | `FoundFree {0, 0}` | 191 | 1 |
| 3 | `Assign {0, 0}` | 233 | 1 |
| 4 | `TryEquation(1)` | 114 | 0 |
| 5 | `Explore {1, 0}` | 181 | 1 |
| 6 | `TryDisplace {1, 0, 0}` | 202 | 1 |
| 7 | `Explore {0, 2}` | 181 | **2** |
| 8 | `FoundFree {0, 2}` | 191 | **2** |
| 9 | `Assign {0, 2}` | 233 | **2** |
| 10 | `DisplaceOk {1, 0}` | 213 | 1 |
| 11 | `Assign {1, 0}` | 233 | 1 |

**Frames 5-11 are one augmenting path**, first reach to final commit. **The depth column is what
`matching.md` has never had** — without it the twelve rows are a list of steps; with it they have
a shape: two flat greedy assignments, a descent, a discovery, and an unwind committing two edges
on the way back. `frameCount` moves 19 → 20 → 19 across rows 6-10 and is the cheapest way to see
it.

**Stop 5 should be built on the depth column and the naming table, not on the value readouts.**

### ✅ THE FAILURE PATH WALKED 2026-08-08 — `TwiceDefined`, and Stop 5 gets a second specimen

**Doug: *"Comparisons between working models and models which don't work are very helpful for me.
So, I want a tour which compares and contrasts two models."*** The success path alone could not
supply `DisplaceFail` or `EquationFailed`, because `ProportionalLoop` succeeds.

**`CapacitorLoop` was the obvious candidate and is the wrong one. Measured before walking**, from
the generated notebook traces:

| | ProportionalLoop | **TwiceDefined** | CapacitorLoop |
|---|---|---|---|
| size | 3 × 3 | **2 × 2** | 14 × 14 |
| incidence entries | 9 | **2** | 42 |
| outcome | perfect | rank 1 of 2 | 13 of 14 |
| frames | 12 | **9** | ~114 |

**CapacitorLoop's failure is at its LAST equation**, so the interesting stops sit ~110 Continues
in — an ordeal, not a walk. `TwiceDefined` reaches both failure steps in nine frames. It stays the
right specimen for Stop 4's *physical* story (a capacitor across an ideal source is a real
modelling mistake); for learning the algorithm's failure path under a debugger, the synthetic 2×2
is strictly better. **Sizing a walk from the notebook trace before doing one is the reusable move
here.**

**Thirteen of fourteen predictions exact.** The ledger, every row read from a live stack:

| idx | step | emit line | depth | |
|---|---|---|---|---|
| 0 | `TryEquation(0)` | 114 | 0 | |
| 1 | `Explore {0, a}` | 181 | 1 | |
| 2 | `FoundFree {0, a}` | 191 | 1 | |
| 3 | `Assign {0, a}` | 233 | 1 | |
| 4 | `TryEquation(1)` | 114 | 0 | |
| 5 | `Explore {1, a}` | 181 | 1 | |
| 6 | `TryDisplace {1, a, 0}` | 202 | 1 | |
| — | *inner give-up* | **243** | **2** | **NO FRAME** |
| 7 | `DisplaceFail {1, a}` | 213 | 1 | |
| — | *outer give-up* | **243** | 1 | **NO FRAME** |
| 8 | `EquationFailed(1)` | **133** | 0 | |

### The two unnumbered rows are the argument for Stop 5

**Both `243` stops are real algorithm steps that never reach the frame stream.** The inner one is
equation 0 being asked to move and refusing: its only candidate is `a`, `visited[a]` is already
true, so line 177 skips it and the loop ends **without reaching a single emit**. The outer one is
equation 1 exhausting its one candidate.

So the animation runs `TryDisplace` → `DisplaceFail` → `EquationFailed` with nothing in between,
while the debugger shows two genuine decisions. **There are steps the animation structurally
cannot show and the debugger can** — which is the strongest case for the third leg that this
project has produced.

**Their signature is `var` and `iter` both reading `<unavailable>`.** At a `189` stop both are
live; at a `243` stop the `for` loop has *ended*, so the debugger reports them gone. That is the
adapter's "optimized away" prose doing real diagnostic work rather than being noise (`#72`'s
finding 4): **it distinguishes "returning from inside the loop" from "fell out of it."**

### The contrast, which is what the comparison tour is for

Both specimens produce a depth-2 stack. They are identical in shape and opposite in the one
property that decides everything:

| | ProportionalLoop | TwiceDefined |
|---|---|---|
| stack at depth 2 | `181 → 210 → 123` | `243 → 210 → 123` |
| path | eq1 → a → eq0 → **var2 (free)** | eq1 → a → eq0 → **dead end** |
| Berge's name | **augmenting** path | merely **alternating** path |
| the unwind | commits an edge per frame — the flip | **commits nothing** |
| `vars` at the collision | `[0, 1]`, `iter` has one left | `[0]`, `iter` **already exhausted** |

**An alternating path is cheap; you can always find one. It becomes *augmenting* only if it ends
at a free variable, and that terminal condition is the entire content of the theorem.** The
unwind is the flip in both runs — when the search fails, the flip is a no-op.

**And `vars`' length is where the mathematics meets the loop bound.** `TwiceDefined`'s incidence
has 2 entries in a 2×2, both in column `a`; **column `b` is empty**, so no permutation can place a
nonzero on its diagonal. That is **Hall's condition** violated by S = {eq0, eq1}, whose
neighbourhood N(S) = {a} has |N(S)| = 1 < 2 = |S|. The algorithm discovers this by exhausting the
search; Hall's theorem says why the search was doomed before it began. **`b` is never visited by
any frame** — `visited[1]` stays `false` for the whole run — so the unmatched *unknown* is
reported by absence, never by discovery.

**A failed search leaves the matching untouched.** `match_eq` and `match_var` are byte-identical
before and after equation 1's attempt; `visited` is the only thing mutated, and line 122 resets it
per equation. That is why Kuhn's outer loop never backtracks: the matching only grows.

### One line number was wrong, and the tag did not contain it

**`EquationFailed` is emitted at `matching.rs:133`, not 137.** Line 133 is the
`emit_matching_frame(` call; 137 is `step: MatchingStep::EquationFailed(eq)` inside the struct
literal argument, and **the stack reports the call site**. Every other row was taken from the call
line; this one row was read from where the variant is *named*.

**It was the only row never observed**, was tagged `<!-- unverified -->`, and was still stated as a
number beside twelve measured ones — where it read as equally solid. **The tag marked the risk and
did nothing to contain it.** It reached only conversation, never the repository, which was luck
rather than process. **When a table mixes measured and read-off values, the unmeasured ones need
the marking inside the cell, not in a sentence underneath.**

### Frame-delay evidence

Two screen readings this walk, **both in step** (`frame_index` 3 → "Frame 4", 8 → "Frame 9"),
against one lag in three at 20 ms. Consistent with the two-tier delay; still a small sample.

### ✅ AND THE DATA IS NOW GENERATED — `matching_ledger.rs`, 2026-08-08

Doug, on being told most of the walk had been *verification* rather than discovery: *"build the
ledger generator and the line number check. We need to figure out how to keep that data accurate
even as we make changes to the code being referenced by the tours."*

**Both tables above are now derived, not transcribed.**
`docs/compiler-phases/phase7_structural_analysis/matching-live-reference.md` is generated by
`cargo run -p hrw --example gen_matching_reference`, and
`matching_ledger::tests::the_generated_reference_is_current` fails the moment `matching.rs` moves
— naming the regeneration command **and the first line that changed**. Verified must-fire by
shifting `matching.rs` two lines: every emit site moved and the test caught it.

**Three things turned out to be derivable that had cost an hour of walking each:**

- **Emit sites** — scanned out of the source, attributed to the `emit_matching_frame(` **call**
  line rather than the line naming the variant. This is the check that would have caught the
  133/137 error automatically.
- **Depth** — recovered from the step sequence alone: `TryDisplace` descends,
  `DisplaceOk`/`DisplaceFail` return. **Pinned against both debugger walks**, which is what keeps
  it from being a derivation checked only against itself — the vacuous-test trap from the same day.
- **The ledger** — the real traced algorithm re-run over the specimen's *recorded* incidence from
  its notebook trace, so no compile and no MSL. It reproduces both walks exactly, and
  `ProportionalLoop`'s final matching agrees with the compile's recorded result.

**`maximum_bipartite_matching.md` no longer carries the numbers**, only the two facts about them
that are about the algorithm rather than about lines. A number written in two places goes stale in
one of them.

**What generation cannot replace, and this is the boundary that matters.** Everything the walks
found about the *instrument* — anchor stops exposing only `frame_index`, invisible `Option`
payloads, `var`/`iter` meaning the loop ended, the paint race — is not in the source and not
derivable. Neither are the two `augment_traced:243` give-ups, which **emit no frame and therefore
cannot appear in any generated ledger**; the reference says so in its own text. And neither is the
check that matters most: **whether a promised rhythm survives contact with a human.** Three
confident claims were falsified by Doug walking on 2026-08-08, and a test written from the same
wrong model would have agreed with every one of them.

---

## 74. Only the FIRST Debug press worked — cppvsdbg will not re-bind a removed line

**Doug, 2026-08-08**, walking `matching.md` in preparation for `#73`: *"the first time that I hit
the 'Debug' button, the breakpoint is correctly set and execution stops… when I then hit the
'Debug' button a second time, the breakpoint is again set, but only briefly. In fact, it only
shows an empty circle instead of a red circle."*

### ✅ FIXED 2026-08-08 — the teardown was an LLDB workaround, and Windows has no SIGSTOP

**The finding, and it is an operating fact worth keeping: `cppvsdbg` will not re-bind a
breakpoint at a location the extension REMOVED earlier in the same debug session.** VS Code
accepts the new breakpoint, lists it, and draws it **hollow** — unverified. Nothing errors. The
algorithm runs to completion and the animation reaches "Live (done)" looking like a successful
run that simply had nothing to stop at.

**Why HRW was removing it at all:** when a live session ends, the algorithm thread's
`on_complete` released the anchor, with a UI-side safety net behind it. Every rationale in the
repository is Unix-only — *"preventing a SIGSTOP signal when the algorithm thread exits"*
(`bridge.rs`), *"preventing SIGSTOP from LLDB"* (five `*_anim.rs` files), *"prevents LLDB from
delivering SIGSTOP/SIGCHLD"* (`architecture.md`). **Windows has no SIGSTOP and `cppvsdbg` is not
LLDB**, so the teardown bought nothing here and cost the entire feature after its first use.

**The fix is `RELEASE_ANCHOR_AT_SESSION_END` in `app.rs`** — `cfg!(not(windows))`, gating the
five `on_complete` closures and the safety net. **Three releases are deliberately NOT gated**,
because each ends the *reason* the breakpoint existed rather than merely pausing it: a session
that failed to spawn, a specimen change, and app exit. Leaving it armed between runs is safe:
`live_trace_breakpoint` is unreachable outside a live session, since its only callers are
`wait_for_debugger` and `push`, and `push` reaches it only with a `frame_delay` set — which only
`start_live` does. **An ordinary compile never touches it.**

### The finding is broader than "removed" — DISABLING does it too

*(Doug, 2026-08-08, testing `#75`.)* He disabled all breakpoints, pressed Debug, got the new
not-armed notice, then **re-enabled the anchor — and it never came back.** No filled marker, and
execution ran straight through. Restarting the debug session fixed it.

**So the rule generalises: `cppvsdbg` will not re-bind a location whose breakpoint has left the
adapter's active set within the same session, and disabling takes it out just as removing does.**
One-way door, either route.

**Two consequences, both load-bearing:**

- **`#75`'s advice was wrong on its first day.** The disabled-breakpoint reason said *"enable it,
  or use Enable All Breakpoints"* — a remedy measured not to work, which costs a debugging
  session before the reader stops believing it. It now says to start a new debug session, guarded
  by a test that fails if the old wording returns. **Advice that does not work is worse than no
  advice**, and this one shipped because it was written from the fix rather than from a walk.
- **No layer can detect it.** VS Code exposes **no `verified` field** on `vscode.Breakpoint`, so
  the extension cannot tell a bound breakpoint from a hollow one; a disabled-then-enabled anchor
  looks identical to a working one in `vscode.debug.breakpoints`. **`#75`'s `breakpointPresent`
  therefore means "an enabled breakpoint exists", never "execution will stop"** — a real
  improvement over *"I read your file"*, and still not proof. Recorded in `arm_verdict.ts` so a
  later reader does not quietly upgrade the claim.

**Operating rule for the tours:** do not disable the anchor mid-session. If you do, stop the
debugger and start a new session — nothing shorter recovers it.

### The gate is GONE — the LLDB teardown was deleted, not kept behind a `cfg`

*(Doug, 2026-08-08: "you mentioned some macOS cruft being in our code. Do we need that? If not,
eliminate it.")* `RELEASE_ANCHOR_AT_SESSION_END` lasted a few hours. What replaced it is nothing:
the session-end release is deleted outright, and with it the `on_complete` parameter on all five
`start_live` functions, whose **only** purpose was to call it.

**It is not macOS cruft; it is pre-migration cruft, and the distinction matters.** The SIGSTOP
work landed 2026-07-24 (`0270968a`) under **CodeLLDB**, before the 07-27 move to `cppvsdbg`.
**There is not one mention of macOS anywhere in this repository's docs.**

**Why deleted rather than gated**, since gating was the earlier decision and this reverses it:

- **Nothing tests the LLDB path**, so it was untested code for a configuration nobody runs —
  a claim of absence with no failing test behind it, which this repository has a rule about.
- **It is the mechanism that silently destroyed the feature.** Keeping a disabled copy of the
  code that caused the bug invites its return.
- **It removed a branch from the regression test too.** While the release was gated, the test had
  to branch — and its first draft branched on *the constant itself*, so forcing the gate took the
  other path and **passed**. Deleting the gate deletes that whole class of mistake; the test now
  asserts unconditionally. `live_debug_poll` also stopped taking a `LiveState`, which it read for
  nothing else.

**What survived the sweep, and why:** `OutputCapture`'s `#[cfg(unix)]` arms in `worker.rs` are
paired with `#[cfg(windows)]` arms and are three small functions — a portable abstraction, not
cruft. `main.rs`'s ~60 lines of **Linux** Wayland/X11 probing are flagged, not removed; they are
a real feature for a real platform, `#[cfg]`'d out here, and untested. <!-- unverified -->

### Two `hrw/` citations were sitting inside upstream-bound crates

Found while sweeping for the above, and worse than the cruft. `CLAUDE.md` requires the
instrumentation stay "separable from `hrw/` so an upstream PR is a clean cherry-pick":

- **`live_trace.rs` cited `hrw/docs/windows-migration.md` — a file deleted in `77754d61`.** A
  dangling cross-repo pointer, in code destined for CogniPilot, naming a directory upstream does
  not have. Replaced with the platform triple, which is the load-bearing part.
- **`pre_lowering.rs` cited `hrw/DECISIONS.md`.** Dropped; the paragraph already made its own
  argument.

**`doc_citations` could not have caught either** — it scans HRW's tree, not `crates/rumoca-*`.
Four `HRW`-by-name mentions remain in those crates and are left alone: each names the consumer
that exercises an observer API, which is ordinary context rather than a pointer into a directory
the reader lacks.

### How it was found, because three confident wrong answers came first

**Every one of them was eliminated by evidence rather than by reasoning**, and the order matters:

1. **`isDuplicate` skipping the arm** — the change from `armedBreakpoints` to all of
   `vscode.debug.breakpoints` (`1585432d`) had *just* gone live on this machine. Killed by the
   output channel: it said `Armed:`, never `Already armed: … skipped`.
2. **HRW tearing it down during the 500 ms `wait_for_debugger` sleep** — killed by reading
   `live_state`, which reports `Running` for the whole sleep, so the safety net cannot fire.
3. **The new `#72` tracker poisoning the adapter** — 27 `customRequest` round-trips across nine
   stops, and press 1 was the only arm that preceded any stop. **Killed by the control**: a
   hand-set breakpoint at the same line bound and hit on every press.

**The control is what turned a hypothesis into a diagnosis**, and it was free. A hand-set
breakpoint is never removed by the extension (`handleRemove` only touches what it armed), so it
isolates the remove/re-add cycle from the line, the anchor, the adapter and the session.

### The trap this left behind, caught only by trying to break the test

**The first version of the regression test asserted whatever the constant already said.** It
branched on `RELEASE_ANCHOR_AT_SESSION_END`, so forcing the gate to `true` took the *other*
branch and **passed**. It was rewritten to branch on `cfg!(windows)` — the platform, not the
value under test — and then verified must-fire by breaking the gate and watching it fail.

**A test that reads the value under test cannot fail**, and it is indistinguishable from a
passing one. Same shape as the deleted scroll-configuration tests and as a provenance tag that
resolves nothing. **The only thing that caught it was running the break.**

### Still open, and separate — now `#75`

**The ack means "I read your request", not "a breakpoint exists".** The extension writes
`breakpoint-ack.json` unconditionally, including when `handleAdd` skipped every entry as a
duplicate — and HRW does `live_breakpoint_armed = acked`. That is `#71`'s fiction one layer
down. It is not what caused this bug. **Fixed the same day — see `#75`.**

---

## 75. The ack answered the wrong question, and this fix made it the routine one

**Found 2026-08-08 while diagnosing `#74`, fixed the same day**, on Doug's standing rule: *"we will
pause feature work to fix bugs, especially bugs which cost accuracy."*

**The extension wrote `{"acked": true}` at the end of every request** — after arming nothing
because every entry was a duplicate, after a removal that matched nothing, after any request it
managed to read. HRW consumed it as `live_breakpoint_armed = acked`. **The file answered *"I read
your request"*; HRW read it as *"a breakpoint exists"*.**

**`#74`'s fix is what made this reachable rather than theoretical.** Leaving the anchor armed
between runs means every Debug press after the first correctly arms *nothing* and reports
`Already armed: … — skipped`. So the ack's least informative case became its **normal** case.

### The second bug, found while fixing the first

**`isDuplicate` never checked `bp.enabled`**, and disabled breakpoints stay in
`vscode.debug.breakpoints`. One click of VS Code's **Disable All Breakpoints** produced: the anchor
found and reported as covered, nothing armed, nothing enabled, `acked: true`, HRW announcing a
stepped session, and the algorithm running to completion **with no stop and no notice** — because
the `!acked` branch that exists to say so was unreachable. That is `#71` exactly, one layer down
and one toolbar click away.

### The contract

> **Does an ENABLED breakpoint now exist at every requested line?**

Not *"did I add one"* — an already-present enabled breakpoint is a perfectly good yes, and is what
a hand-set anchor or a repeat Debug press produces. **`isDuplicate` is deleted**; `findExisting`
returns the breakpoint so the caller can read the flag, which a `bool` could never express.

`BreakpointAck` has four variants where a `bool` used to be: **`replied()` ends the handshake,
`is_armed()` licenses the claim**, and only `Armed` does the latter. A partial success is a
failure — one dead line sinks a request that armed another — and an **empty** request reports
not-present, because "every one of zero lines is armed" is the vacuous truth this whole change
exists to remove.

### `Unreportable` — Doug's call, and it closes the *other* silent failure

*Doug, 2026-08-08: "honesty matters. Loud crashes are better than silent or dishonest bugs."*

A pre-`#75` `{"acked": true}` gets its own verdict rather than being guessed at. **Reading it as
armed reinstates the fiction; reading it as a plain failure blames the wrong thing** and silently
breaks live trace against a stale build. So HRW says: *the bridge replied in an old format and
cannot say whether it armed anything — rebuild it.*

**This is not a hypothetical branch.** It is exactly the state this machine was in for twelve days:
`out/extension.js` dated 2026-07-27 against sources from 08-08, because **`git pull` runs no
`tsc`** — the hazard recorded in `#72`'s operating notes and rediscovered the hard way. Under the
old ack that build was indistinguishable from a working one. Now it announces itself at the first
Debug press.

### Where the logic lives, and why

**`vscode-extension/src/arm_verdict.ts` imports no `vscode`**, so `node --test` exercises it
directly — the same move `debug_state.ts` made, for the same reason: `extension.ts` imports
`vscode` and cannot be tested at all. `extension.ts` keeps only the mapping from
`vscode.debug.breakpoints` into plain records and the VS Code calls.

**13 new TypeScript tests (49 total) and 3 new Rust ones, all verified must-fire** by breaking the
`enabled` check and watching four fail. `parse_breakpoint_ack` is split from the file read so the
verdicts are testable without touching disk.

**Still true, and worth keeping in view:** an ack cannot promise the breakpoint will *bind* —
`#74` is the case where VS Code held an unverified breakpoint and nothing stopped. The ack reports
what VS Code was asked to hold, not what `cppvsdbg` resolved. <!-- unverified -->


---

## 76. Debug-only features should say they need a debugger, not pretend they work

**Doug, 2026-08-08**, after the platform discussion: *"some of HRW's features only make sense if
HRW is launched as a debugged process. For example, the live trace 'Debug' buttons and related UI
features should not even be visible if HRW is not launched as a debugged process. And, relevant
to our upcoming Stop 5 effort, some tour links should be disabled if HRW is not launched as a
debugged process."*

**Deferred by Doug the same day**, alongside the platform question: *"neither the tech debt nor
the feature item are related to me making learning progress."* Recorded now because the design
was settled in conversation and would otherwise have to be re-derived.

### The problem, and it is not hypothetical

HRW currently offers a Debug button that looks identical whether or not it can possibly work.
**This machine ran that way for twelve days** (`#74`'s opening): an extension built 2026-07-27
against sources from 08-08, no junction, nothing listening — and the button was enabled the whole
time. `#71` and `#75` each removed one way of lying about the outcome *after* the press. This
entry is about the state *before* it.

### THE DESIGN DECISION: disable and explain, never hide

**Doug proposed hiding and then changed his mind on the argument below.** Recorded with the
reasoning, because "why is this a disabled button rather than no button" is exactly the question a
later session would re-open.

- **`lib.rs`'s `LiveState` already carries the rule:** *"Controls are enabled and disabled, never
  shown and hidden. A button that vanishes gives no clue that the action exists or why it is
  unavailable, and the row reflows under the pointer."*
- **Charter Decision 8:** fixed answers belong on screen, because a tooltip beats a question for
  latency. **Whether live trace is available is a fixed answer.**
- **And the mission argument, which is the decisive one: a hidden Debug button means a learner
  never discovers that live trace exists.** A disabled one reading *"launch HRW under the VS Code
  debugger (F5) to step this algorithm"* teaches the feature and its precondition in one glance.
  Hiding optimises for tidiness; **HRW optimises for visibility.**

### TWO preconditions, not one — and this is the part that is easy to get wrong

Gating on "am I being debugged?" alone would have shown an enabled button throughout those twelve
days, because a debugger *was* attached the whole time. What was missing was the other half.

| precondition | how HRW can tell |
|---|---|
| **a debugger is attached** | `IsDebuggerPresent()` on Windows — **no new dependency**, one `unsafe extern "system"` declaration. Answers *"attached right now"*, so poll it rather than snapshotting at startup: attaching later is normal. |
| **the bridge is alive and reports verdicts** | `#75`'s `BreakpointAck`. `Unreportable` means a stale extension, `NotArmed` carries a reason, and the startup pre-warm (`tick_prewarm`) already provides a channel to learn this at launch rather than at first click. |

**Name which one is missing.** A boolean "unavailable" throws away the whole diagnosis; *"the HRW
Bridge extension is not responding — see `vscode-extension/README.md`"* is the difference between
a dead end and a fix. This is the context-identification half of the observatory's north star, and
it moves *"why didn't it stop?"* from a question asked in chat to an answer on screen.

### Tour links declare their requirement rather than going quiet

For Stop 5 and every algorithm tour's third leg: a stop that needs a live session should **say so**,
and say what is missing when it is. **Absence is stated, never filled** — and the tours are
Markdown read outside HRW too, so a declared requirement is honest in both places. A stop that is
merely inert teaches the learner that the tour is broken.

### What this does NOT need

**Not a `cfg`-gated build.** The precondition is a runtime fact — a debugger can attach and detach
while HRW runs — so this is state, not configuration. See `tech-debt.md`'s platform entry for the
separate question of which platforms HRW supports at all; the two are independent, and conflating
them would put a runtime condition behind a compile-time flag.

---

## 77. A live tour needs THREE panes, and the layout only has two

**Doug, 2026-08-08**, walking `matching-live.md`: *"there's a basic UI problem: I need to have HRW
in tour mode, but then that makes the HRW RHS small when HRW is using only 50% of my screen and
VS Code is using the other 50% of my screen."*

**Doug is thinking about the solution; this entry is the problem and its constraints only.**
Recorded rather than designed, because the first idea is unlikely to be the right one and the
constraints below are the part that would otherwise be rediscovered.

### What actually changed

**Every earlier tour needed two surfaces: the tour text and HRW.** A live tour needs **three** —
the tour text, HRW's animation, *and* VS Code's call stack and variables. The stack is not
incidental: `#73`'s whole thesis is that the call stack **is** the augmenting path, so it is a
primary surface, not a reference.

So this is not "the right-hand side got small". **A two-pane split has nowhere to put a third
thing**, and squeezing is the only move it offers.

### The arithmetic, which is why squeezing runs out

HRW at half-width, tour panel at its 40 % default, leaves the observatory **30 % of the screen** —
and the matching animation is a matrix that wants width. Dragging to the 15 % floor buys back
17 points and makes the tour column too narrow to read prose in.

### What is already available, so a solution does not re-invent it

- **The divider is draggable, 15–75 %** (`SplitState`, `#59`). A scene where the reader mostly
  watches can run at 15/85 today.
- **`main.rs --half`** sizes HRW to half-width, full-height, and exists for exactly this
  side-by-side arrangement.
- **The tour is a Markdown file.** Reading it outside HRW is possible — but `hrw://` links stop
  working, which is precisely what `#73` and the breakpoint links just made load-bearing.

### The constraint any solution has to respect

**The tour's links are the reason the tour lives inside HRW.** Any layout that moves the prose
out of HRW must keep `hrw://load/…`, `hrw://breakpoint/…` and `hrw://stage/…` clickable, or it
trades one friction for a worse one. That rules out "just read it in VS Code" as-is, and it is
the question a second window, a collapsible drawer, or an overlay each has to answer.

**And whatever is chosen, controls are enabled and disabled, never shown and hidden** —
`lib.rs`'s `LiveState` rule. A layout that makes the tour vanish with no trace of how to bring it
back is the same defect in a new dress.

### The arithmetic is now measured, and it rules squeezing out entirely (2026-08-12)

Doug reached the same wall from the other direction — a 13" laptop with no external monitor, walking
the tours: *"there's not enough space on my small screen to display the tour and the RHS."*

**The number that settles it: HRW runs at `DEFAULT_ZOOM` = 2.0**, so a 13" 1280×720 screen gives it
**~640×360 points** of layout space, not 1280×720. Everything below is in points.

**The tour panel has an intrinsic minimum width of ~190–210 points**, set by its own content (the
tour-list rows and the autoplay controls) and **independent of window width** — measured across
1280, 640 and 500 point windows while fixing the divider bug the same day (`DECISIONS.md`,
2026-08-12). So at 640 points wide:

```text
tour panel minimum   ~210pt   =  33% of the window
left for the RHS     ~430pt   =  67%, for a matrix that wants width
```

**The 15 % floor is unreachable at this size** — it is 96 points, below what the content can render,
and trying to reach it was the defect: the divider stopped at the content minimum while the content
kept shrinking, opening a 112-point gap.

**At 640 points, squeezing is arithmetically finished** — a solution has to change the *container*,
not the fraction.

### But the 640 was self-inflicted, and it is fixed (2026-08-12, same day)

**`DEFAULT_ZOOM` was 2.0 and is now 1.0.** Zoom *multiplies* the display's own scaling
(`pixels_per_point = zoom_factor × native_pixels_per_point`), so at 150 % Windows scaling a 2.0 zoom
was an effective 3.0 — and it predates the WSL2 → native-Windows port, where a hi-dpi panel really
did report `native_pixels_per_point = 1.0` and the 2.0 *was* the DPI scaling. Native Windows reports
the real value, so the compensation had been double-counting since 2026-07-27.

**This changes the arithmetic above, and the honest version is much less dire.** The same 13" laptop
now gets **~1280×720 points**:

```text
tour panel minimum   ~210pt   =  16% of the window   (was 33%)
at the 40% default   ~512pt for prose, ~768pt for the stage view
```

which is the regime a large display was already in. **So this entry is no longer blocking**: a 13"
screen can now show a readable tour column and a usable stage view at the ordinary 40/60 split, and
the fix cost one constant rather than a new layout.

**What survives, and it is the part worth keeping:** the tour panel still cannot go below ~210
points, so the *three*-pane live-tour case this entry was opened for is improved but not solved —
HRW at half width is ~640 points again, and that is exactly the regime measured above. The stop
strip, drawer and alternating-mode options all remain on the table for `matching-live.md`; they are
simply no longer needed to walk the other eight tours.

**And the general lesson, which is bigger than the layout:** *a UI constant that compensates for a
platform quirk becomes a bug when the platform changes, and it does not announce itself.* Three weeks
of "HRW feels cramped" and one divider defect both trace to one number nobody re-derived after the
port.

## 78. Back / Forward for the RHS — an auto-navigation with no return path

**Doug, 2026-08-12, walking `connect-expansion.md`:** *"I'm in the Connect sub-tour, looking at the
Flatten stage, Equations sub tab. When I click on an equation, that correctly navigates me to the
Structural stage, Incidence sub-tab. Unfortunately, there is no way to navigate backward to the
Flatten stage, Equations sub tab that I had been at… It seems that I need buttons for navigating
backward and forward in the RHS."*

**The first walked finding from the nine tours, and it is about the instrument rather than a count.**

### He is not stuck, and that is a separate problem

**Clicking the `Flatten` tab returns him to the equation sheet.** The auto-switch
(`equation_sheet_ui`, `app.rs`) sets only `self.stage = Structural` and
`viewport.structural = Incidence`; `Viewport.flatten` is independent state and untouched, and its
initializer deliberately opens on `Equations`. *(Read from the source, not observed in the GUI.)*

**So the defect is not "no way back", it is "no way to know there is a way back."** Nothing on
screen says a jump happened, and nothing names where it came from. Those are two distinct misses,
and a Back button only fixes the first.

### Why a tour-authored return link is NOT the fix

The link scheme already reaches sub-views — `hrw://load/TwoLoops/Flatten/EquationSheet` is in this
very tour, so **#34 is largely built** — and the reflex is to add a return link at the stop. It
does not work here: **the stop Doug was on does not tell him to click an equation.** He clicked one
because the sheet invites it, which is the behaviour the sheet exists to produce. The jump is
triggered by *curiosity at an arbitrary moment*, so no authored link can be waiting for it. That is
precisely the case a general history covers and a scripted one cannot.

### What a location is

The state a Back must restore is more than `stage`:

- `stage`, and **that stage's** sub-view (`viewport.flatten` / `structural` / `init` / `events`)
- `viewing_log` — the Log button sits left of the tabs and is a peer of the stages
- `ui_mode` and `specimen_detail` — because **other jumps change those too**: clicking a variable in
  the equation sheet switches to Specimen mode and the Source detail, and `HrwLink::ShowSource` does
  the same. Any history that ignores them will fail on the second-most-common jump.

Deliberately **not** included: canvas pan/zoom and `highlighted_eq_row`. They are where the reader
was *looking*, not where they *were*, and restoring a camera is the kind of surprise a Back button
should not spring.

### The decision worth making deliberately, not drifting into

**HRW already has a Back.** `nav` — the go-to-definition stack — renders `← Back / ⌂ Specimen` with
a breadcrumb, and it means *"stop inspecting this library class."* A second arrow meaning *"go back
to where I was in the pipeline"* would put two Backs on screen with different scopes, which is worse
than having one. Three options, and this should be chosen rather than discovered:

1. **Two histories, visually distinct** — `nav`'s stays with the breadcrumb, the new one lives in the
   stage tab bar. Cheapest, and the scopes really are different: *what am I inspecting* vs *where in
   the pipeline am I looking*.
2. **One unified history** that records both kinds of move. Matches a browser, which is the model
   Doug named. But `nav` is a stack with breadcrumb semantics rather than a linear history, so this
   is a refactor of working code.
3. **Extend `nav`** to carry view state. Risks making the breadcrumb mean two things.

**Leaning (1)**, on the grounds that it is reversible and does not disturb `nav`; but (2) is what a
reader would predict, and "what a reader predicts" has beaten "what is cheap" every previous time in
this project.

### Requirements it must meet

- **Enabled and disabled, never shown and hidden** (`lib.rs`'s `LiveState` rule), with hover text
  naming the destination — *"Back to Flatten · Equations"* is worth more than an arrow, and it also
  fixes the second miss above by saying where you came from.
- **Do not record your own navigation.** The classic history bug: Back pushes a new entry and
  Forward becomes unreachable. Needs an explicit suppression flag, and a test for it.
- **Ships with a headless test.** This is behaviour, not config, and every part of it is reachable
  from `egui_kittest` — assert that a jump then Back restores stage *and* sub-view, that Forward
  returns, and that Back at the start is disabled rather than absent.

### The tour-navigation case is NOT this, and was solved without it (2026-08-17)

**Doug asked for tour-to-tour navigation and explicitly ring-fenced these buttons for the RHS:**
*"I don't want to use up those buttons for tours as I would probably prefer to have those buttons
for use in the RHS later on."* That reservation stands — **this entry is still open and still
unclaimed.**

What was built instead is a **back-link in the document plus picker ordering** (`DECISIONS.md`,
2026-08-17), and the distinction is worth keeping straight because it is the reason one problem
needed history and the other did not:

| | the tour hub | this entry |
|---|---|---|
| the move that needs undoing | **authored** — the overview's table sent him there | **unpredictable** — he clicked an equation out of curiosity |
| what "back" means | a fixed edge in the tour graph, same on every visit | wherever he happened to be last |
| so the fix is | a link in the document | a recorded history |

**The section above already says this**, from the other direction: *"the stop Doug was on does not
tell him to click an equation… so no authored link can be waiting for it."* The tour hub is the
exact complement — the overview *does* tell him which tour to open, so an authored link is waiting
for it, and history would be the wrong instrument.

---

## 79. `LoopWithInertia` becomes the final act of the tearing tour

**Doug, 2026-08-16, having asked whether the specimen deserved a tour of its own:** *"Eventually,
I will want very much to add LoopWithInertia to the tearing tour, as you've recommended. Please
ensure that we do that."*

**Not a tenth tour.** `README.md`'s first rule is one tour per capability, narrow — the scarce
resource is Doug's attention per expectation, and a tenth tour would spend it re-establishing
what a coupled block is, on a fourth loop specimen, to deliver one new idea. It is **one act**.

### The idea it adds, which the existing six acts cannot

Every specimen in `tearing.md` is **timeless**. `ProportionalLoop`, `TwoLoops` and `MixedLoop`
have no state, so each loop is torn and solved **once**. The tour therefore never confronts the
question its own subject raises:

> **What does a coupled block cost when time is advancing?**

`LoopWithInertia` is `ProportionalLoop` with the idealization removed — the same 3-cycle
`command → measurement → error → command`, now with `der(w)` beside it. The torn block is
re-solved **between every pair of integrator steps, for the whole run**. Tearing stops being a
compile-time tidy-up and becomes a decision about the inner loop of the simulation. That reframes
Stop 1 rather than repeating it.

### Why it is not written yet

Tours are converted to the Predict/Look/Falsified template **as Doug walks them**, because the
conversion is itself the teaching (`CLAUDE.md`, current work). He is walking in compiler-phase
order and is on Connections → DAE, so tearing is some way off. Writing the act now would convert
a tour he is not walking, which is the one thing that rule forbids.

### How the commitment survives until then

Two mechanisms, because a promise in a conversation does not survive the session:

- **A marked `## OWED` section at the head of `tearing.md`'s closing material**, so it is seen at
  the moment of use rather than found later.
- **`doc_citations::the_tearing_tour_gains_its_dynamic_loop_when_it_is_converted`**, which passes
  while the tour is unconverted, and fails the instant it gains two `**Predict.**` markers
  without an `hrw://load/LoopWithInertia` link. It also fails if the OWED note is simply deleted
  — abandoning the act is a decision to record, not a line to remove.

Verified must-fire on both paths 2026-08-16.

---

## 80. ~~The divider jumps to ~70 % when the window is MAXIMIZED~~ — FIXED 2026-08-16

**Doug, 2026-08-16**, correcting an earlier report he had made about startup: *"The vertical
divider is not positioned far to the right when HRW starts. Instead, the vertical divider bar
positions far to the right (~70%) when I maximize the HRW window from the normalized window
size."*

**Not diagnosed, and deliberately not theorised about.** Six blind tuning attempts on a
different layout defect the same day produced non-monotonic results and cost an hour; the thing
that resolved it was instrumenting and reading the numbers. This entry exists so the same
mistake is not repeated on this one.

### What is known

- It is **maximize**, not startup. The opening fraction is correct.
- ~70 % is suspiciously close to `MAX_LEFT_FRACTION` (75 %), which is where the
  **2026-08-03** bug landed too: a transient `avail` produced a stored pixel width that
  exceeded the maximum on the real window and clamped. `SplitState::configure` carries the
  full account of that one.
- `SplitState` means a **fraction**; egui stores a **width**. Every bug in this area so far
  has been a disagreement between those two at a moment when the window size was changing.

### The instrument now works past startup, which it did not

`observe` records `split: 0.400 of window (panel 2000px, available 5000px)` — exactly the line
that solved the 2026-08-03 bug after five wrong theories. It was **rationed to six
observations**, a budget sized for diagnosing startup and consumed by startup, and the
`reports_left == 0` check sat *above* the recording despite the comment beneath it saying
*"always to the diagnostics file, only anomalies to the log view."*

Fixed 2026-08-16: recording is unconditional, only the log-view message is rationed. So the
next maximize writes the numbers to `.hrw-bridge/diagnostics/session.json`.

### How to close this, cheaply

1. Doug maximizes the window once.
2. Claude reads the `split` actions in `diagnostics/session.json` and reads off the
   `(panel, available)` pair at the moment of the jump.
3. The pair says which of the two candidate mechanisms it is — a stale `avail` with a fresh
   panel width, or the reverse — and each has a different fix.

**Do not attempt step 3 without step 1.** That is the whole point of this entry.

### Closed the same day, by doing exactly that

Doug reproduced it, and the two recorded observations named the cause outright:

```text
split: 0.400 of window (panel 461px, available 1152px)   <- startup, correct
split: 0.750 of window (panel 200px, available  267px)   <- the jump
```

At `avail = 267` the legal range **collapses to a point**: the maximum is 267 × 0.75 = 200.25
and the 210pt floor sits above it, so the panel has exactly one legal width. **0.750 was
arithmetic, not a decision** — and `observe` stored it as a *proportion*, which then applied to
the maximized window.

**The floor is absolute and the memory is proportional, and that is the category error.**
`MIN_LEFT_POINTS` says "the content needs 210 points", a different claim at every window size, so
it must be re-derived per frame rather than remembered as a ratio.

Fix: `observe` no longer learns a fraction from a width the panel had no choice about, so the
reader's own split survives a trip through a narrow window. `last_rendered` was added beside
`fraction` because *what is on screen* and *what will be restored* became genuinely different
questions — collapsing them back into one field reintroduces this bug.

Five theories were wrong about the 2026-08-03 version of this before anyone read the numbers.
This one took **one** reading, because the instrument was fixed first.

## 81. A pendulum specimen for index reduction — pedagogy, no longer capability

**Doug, 2026-08-17, on the index-reduction tour:** *"index reduction is a large topic and so is not
well served by a short tour… the tour needs to assume that I have only a basic knowledge of
calculus."*

**The case for this changed mid-investigation, and the change is the point.** The original argument
was that no specimen in the corpus ever differentiates, so the tour named for Pantelides could not
show Pantelides. **That argument was wrong**: `Drivetrain` differentiates at least four times, and
HRW's captured frames record them (`DECISIONS.md`, 2026-08-17). The tour said zero because it read
a field whose name misled it.

**So this is no longer needed to see the phenomenon.** It is wanted because **97 equations and 88
algebraics cannot be done by hand**, and a reader meeting differentiation for the first time needs
a model small enough to work through on paper alongside the pane.

### What it should be

A pendulum in **Cartesian** coordinates — the canonical index-3 DAE that every treatment of
Pantelides opens with:

```text
der(x) = vx,   der(y) = vy
m*der(vx) = -lambda*x
m*der(vy) = -lambda*y - m*g
x^2 + y^2 = L^2          <- the constraint, and it is NONLINEAR
```

**Nonlinear is the load-bearing word.** Every constraint in the corpus today is an *alias* — one
variable equal to another times a constant — which substitution can remove. `x² + y² = L²` cannot
be substituted away, so differentiation is the only route, and the reader sees why rather than
being told.

Two states, one constraint, four equations: small enough to differentiate by hand and check the
pane against your own arithmetic. That is the whole reason to prefer it over `Drivetrain`.

### Unknowns to settle before writing prose around it

- **Whether Rumoca handles it at all.** Untried. It may fail, which is a finding rather than a
  setback — and possibly an upstream entry.
- **Whether MSL has a better one.** Doug's suggestion, worth keeping: HRW can compile any MSL model
  by name, so the corpus is searchable rather than guessable. The canonical high-index MSL examples
  live in **MultiBody**, which charter §4.3 excludes as a *specimen* dependency — using one as a
  diagnostic probe is a different act and probably fine, but it should be a deliberate call.
- **Which order to work in.** Specimen first, then read `dae_prepare` to find out what
  `reduce_constrained_dummy_derivatives` actually does, then rewrite the tour. Writing prose first
  is what produced the error being corrected.

---

## 82. The reduction passes should be expandable into their frames

**Owed since 2026-08-19, and filed here on 2026-08-21 because it had no home.** It had been
sitting under a *"STILL OWED"* line inside `CLAUDE.md`'s tour-transport-bar investigation — a box
about a completely unrelated layout bug — and would have been deleted with it. **An owed item
inside a closed arc's record is an owed item nobody will find.**

**The ask:** the reduction view lists the funnel's passes; each should expand into the frames that
pass actually produced, so a reader can see *what a step did* rather than only that it ran. That
is the same move `index-reduction.md` needed when it turned out `Drivetrain` differentiates six
times while the survivor list is empty (`#81`, `DECISIONS.md` 2026-08-17) — **a count with no way
to open it is exactly the shape that taught a tour the opposite of the truth.**

**Doug, on the length this implies:** *"my education is more important than strict adherence to
the template."* **Read that as permission, not as an exemption** — and the distinction matters,
because [`docs/tour-kinds-plan.md`](tour-kinds-plan.md) §4 freezes the concept template. **The
template constrains SHAPE, not LENGTH**, so a long tour needs no exemption in the first place;
what §4 forbids is moving setup → Predict → ▶ Look → Expected → Falsified if → *What just
happened*, and adding depth inside those beats does not touch it.

**Not started.** <!-- unbuilt: reduction_view::pass_frames -->
