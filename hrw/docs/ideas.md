# Ideas — backlog for future implementation

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

## 1. Narratives for *simulation*, especially convergence-failure troubleshooting

**✅ Implemented 2026-07-21.** `gen_trace` now runs simulation after compilation for
specimens that compile through solve lowering, writing per-variable trajectory
summaries (`simulation.json`: name, is\_state, initial/final/min/max values) and
recording the simulation outcome in `manifest.json`. The notebook template and README
include a Simulation section. All 12 specimens have simulation traces. The deeper
convergence-failure diagnostics (solver logs, per-step residual history, Jacobian
conditioning) remain future work. Original capture below.

Captured 2026-07-20 (Doug). The Claude-authored [notebook narrative](specimen-notebook/README.md)
is powerful for *compilation*; it should extend to **simulation** — and is likely
*most* valuable when a simulation **fails to converge**.

- **Why it matters:** convergence failures (Newton divergence on a torn block,
  event-iteration chattering, step-size collapse, singular/ill-conditioned
  Jacobian) are exactly where a grounded, cited narrative earns its keep — the raw
  solver output is opaque, and the failure usually traces back to a *specific*
  structural feature (a particular BLT block, a bad tear choice, a high-index
  residual). A narrative can connect "the solver stalled here" to "this is the
  coupled block from the structural phase, torn on X."
- **Sketch:** a **simulation trace** analogous to the compilation trace — solver
  logs, per-step residual/error history, the failing block/tear, event log,
  Jacobian conditioning — captured durably, with a `narrative.md` that diagnoses
  the failure against it. Ties directly to the matching/BLT/tearing work: the
  structural report is the map a convergence post-mortem reads.
- **When:** aligns with the simulation arcs (charter §4.2, arcs 6–7). Revisit then;
  the trace+narrative machinery ([`examples/gen_trace.rs`](../examples/gen_trace.rs))
  is the pattern to extend.

## 2. Specimen *purpose hints* — in the file and in the app UI

**✅ Implemented 2026-07-20.** Convention: a `// purpose: <one-line>` comment in
each specimen (phenomenon-focused, distinct from the Modelica description string).
The app scans it at rescan (`read_purpose`, no compile) and shows it as weak
subtext + hover under each filename in the LHS list. All seven specimens carry one.
Original capture below.

Captured 2026-07-20 (Doug). The app's left-hand specimen list shows only filenames
— no hint of *why* each specimen exists (e.g. "demonstrates Pantelides' algorithm",
"a genuine algebraic loop"). Surface a one-line purpose both in the specimen file
and in the UI.

- **Source of truth:** every specimen already has a model description string
  (`model X "…"`), and the notebook's "Why this specimen exists" section states the
  phenomenon precisely — keep the hint consistent with both. Consider a lightweight
  convention (the description string, or a dedicated structured comment/annotation)
  so it's machine-readable.
- **UI:** show the hint in the LHS list (secondary text under the filename, or a
  hover tooltip). The compile already yields the model; the description is available
  from `parse.json` (or a cheap pre-compile scan of the `"…"` after the model name).
- **Payoff:** turns the specimen list into a navigable index of *what each teaches*
  — small change, directly serves "identify context conveniently."

## 3. Directory naming / organization

**✅ Implemented 2026-07-20.** Renamed `docs/understanding/` → `docs/compiler-phases/`
(says what it is — per-phase Rumoca explanations) and `docs/notebook/` →
`docs/specimen-notebook/` (shares the `specimen` stem with `specimens/`, signaling the
one-entry-per-specimen tie by name). `specimens/` left in place (its path is hard-coded
across the app and tests). `examples/` also kept — it's a **Cargo convention** (Cargo
discovers `examples/*.rs` as targets for `cargo run --example`), not a free-choice name,
so renaming would break the `gen_trace` / `gen_field_help` invocations. Original capture
below (its wording now uses the new names, since the rename swept this file too).

Captured 2026-07-20 (Doug). Two naming problems:

- **`specimens/` ↔ `docs/specimen-notebook/` coupling is invisible.** They are tightly
  related (one notebook entry per specimen) but the names don't say so.
  - *Options:* (a) lightest — rename `docs/specimen-notebook/` to something that signals the
    tie (e.g. `docs/specimen-lab/`) and lean on cross-links; (b) a shared parent;
    (c) per-specimen folders holding both the `.mo` and its notebook — the strongest
    signal but the biggest restructure (fights the app's flat `specimens/` scan and
    the many test paths that hard-code `specimens/<X>.mo`).
  - *Lean:* (a) — signal the relationship by name + cross-reference; defer the deep
    restructure unless it clearly pays off.
- **`docs/compiler-phases/` is poorly named** — "understanding of *what*?" It is
  Doug's canonical explanation of the **Rumoca compiler phases**.
  - *Candidate names:* `docs/phases/`, `docs/rumoca-phases/`, `docs/compiler-phases/`.
    *Lean:* `docs/rumoca-phases/` (says exactly what it is, and distinguishes it from
    the specimen-specific notebook).
  - *Impact (so a future rename is scoped, not surprising):* `src/field_help.rs`
    (`chapter_for_stage` paths), the notebook narratives' `../../compiler-phases/…`
    links, the app "Read: chapter" button, `.vscode/settings.json` editor
    associations glob, and references in `CLAUDE.md` / `docs/updating-rumoca.md`.
    Mechanical but wide — do it as one deliberate sweep.

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

## 6. Initialization stage: detect over/under-determined *user* initialization

**✅ Implemented 2026-07-20 (over-determination).** The Initialization stage now
reports a `determinacy` block — explicit initial conditions (initial equations +
fixed-start states) vs states — and flags **over-determined** init with a red note
(specimen `OverInitRc`: `C.v = 0` + `der(C.v) = 0` → surplus +1). Under-determination
is intentionally NOT flagged (states initialize from their `start` attributes, so a
deficit is normal — verified: `RcCircuit` surplus −1 is well-posed). A full
initialization-system structural analysis (Rumoca's `rumoca-phase-dae::balance` is
the *continuous* balance, not init-specific) remains a possible deepening. Original
capture below.


Captured 2026-07-20 (finding, Arc 5). The Initialization stage today renders
`build_ic_plan`, which plans the *algebraic subsystem* — it does NOT see the user's
`initial equation`s or `start`/`fixed` attributes. So a **pure initialization
blow-up** (e.g. conflicting initial equations like `C.v = 0` together with
`der(C.v) = 0` — the `OverInitRc` case, tested during Arc 5) shows **all-green** in
the observatory even though it's over-determined. Enhancement: have the
Initialization stage assemble the full initialization system (continuous eqs at
t=0 with `der` as unknowns + initial equations + fixed starts) and report its
determinacy (equations vs free init unknowns), flagging over/under-determination.
That would surface the class of blow-up `CapacitorLoop` cannot (its failure
surfaces structurally instead). Scout whether Rumoca exposes an initialization-
system assembly/consistency check first.

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

## 8. Step-mode plotting for discontinuities (Arc 7 refinement) — DONE (2026-07-21)

**Implemented** as Arc 7 #4. `worker::discontinuity_segments` breaks each trajectory
into segments at reinit jumps (threshold `max(range·0.08, 6·median|Δ|)`); the plot
draws one polyline per segment so the line never slopes through the jump. Gated on
`SimData.has_discontinuities` (the DAE has a `reinit`/`when` discrete update), so
smooth-but-stiff models like BenchActuator are never mis-broken. Landed differently
from the sketch below: a **break (gap)** at the jump rather than a fabricated vertical
riser, since the resampled `SimResult` doesn't carry exact event times. Original note:

Captured 2026-07-20. The Simulation pane (Arc 7 #3) plots each trajectory as a
straight-line `egui_plot::Line`. For a **hybrid** model like `BouncingBall`, the
velocity `v` jumps discontinuously at each bounce; with a fine time grid the jump
renders as a near-vertical segment, but a true **step-mode** render (hold the value
between samples / draw the jump as a vertical) would show discontinuities *as*
discontinuities — the charter's §4.2.6 "step-mode plotting so discontinuities render
as discontinuities". Needs: knowing which outputs are discrete/discontinuous (the
`SimResult.variable_meta` roles, or the first `n_states`), and using egui_plot's
line-style / a manual step polyline for those. Smooth specimens (BenchActuator) don't
need it; it's specifically for the event/hybrid ones.

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

## 12. HRW architecture document — how the code works

**Implemented 2026-07-21.** `docs/architecture.md` covers: crate structure,
data-flow diagram, worker-thread architecture (channel protocol, progressive
streaming, the Rumoca Session, compilation pipeline, simulation), the UI shell
(immediate-mode pattern, App struct, panel layout, tab bar, right-panel routing),
the generic tree inspector (path accumulation, cross-stage diff, DefId resolution),
custom-painted views (canvas scaffold, BLT spy-plot, incidence matrix, reduction
summary), the Claude bridge (thin-emitter/thick-reasoner, file protocol,
span-ascent, chat shortcuts), supporting modules (field help, log view), the
instrumentation surface (all Rumoca crate dependencies + discipline), build/run
commands, and key design decisions (why serde_json::Value, why simulation
re-compiles, why the funnel is replicated, why thin emitter, why egui). Original
capture below.

Captured 2026-07-21 (Doug). Claude has written 100% of the HRW code. Before any
upstream PR to the Rumoca repo, Doug needs to understand and be accountable for the
codebase. A dedicated architecture document that explains how HRW works — the module
structure, data flow, key abstractions, and design rationale — so Doug can read,
defend, and maintain the code he'd be submitting.

- **Why it matters:** submitting a PR means owning the code. Doug's learning mission
  is the Rumoca *compiler*, not the observatory's internals — but an upstream PR
  makes him the maintainer of both. A clear architecture doc bridges the gap between
  "I use HRW to study Rumoca" and "I can explain how HRW itself works."
- **Sketch:** a `docs/architecture.md` covering: module map (`app.rs`, `worker.rs`,
  `bridge.rs`, `tree.rs`, `log_view.rs`, `field_help.rs`), the worker-thread
  architecture (channel protocol, `ToWorker`/`FromWorker` messages, why the UI never
  blocks), the stage pipeline (how compilation results flow from worker → app state →
  views), the bridge (how captures reach Claude), the tree inspector (generic
  serde-value rendering, provenance, cross-stage diff), the spy-plot/incidence
  custom painters, and the instrumentation surface (what HRW touches in the Rumoca
  crates and why). Written for a reader who knows Rust and egui basics but hasn't
  read the HRW source.
- **When:** before preparing the upstream PR. Can be written incrementally — one
  section per module — and doubles as onboarding material if others contribute.

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

## 14. Rank deficiency visualization in the incidence matrix

**✅ Implemented 2026-07-22.** Unmatched rows and columns now have faint red bands
on the incidence matrix. Caption shows "N/M matched (rank deficiency D)" or
"(full rank)". Colors in `colors.rs`: `UNMATCHED_BAND`.

Captured 2026-07-21 (Claude, learning-driven). The incidence view currently reports
"93/97 matched" as text. Enhancement: **highlight unmatched rows and columns** in
the incidence matrix with a distinct color (red).

- **Why it matters (linear algebra):** the number of matched rows in a maximum
  matching equals the **structural rank** of the matrix. Unmatched rows are
  equations that cannot be assigned to any unknown — they represent rank deficiency.
  Highlighting them makes rank deficiency something you *see* spatially, not just
  a number. When Doug studies matrix rank in his linear algebra class, he can open
  Drivetrain's Structural tab and see: "these 4 red rows are why the system is
  singular — they correspond to constraint forces at the ideal gears."
- **Sketch:** the matching result already identifies unmatched equations and
  unknowns (they're the ones not in the transversal). Color unmatched equation rows
  with a red band and unmatched unknown columns with a red band. On hover, say
  "this equation is unmatched — the system has more constraints than unknowns for
  this variable." Could also annotate the caption: "93/97 matched (structural
  rank 93, rank deficiency 4)."
- **Specimens:** Drivetrain (singular, 4 unmatched) vs RotationalInertia (full
  rank, 0 unmatched) — the contrast teaches the concept.

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

## 16. Animated BLT block discovery (Tarjan's SCC algorithm)

**✅ Implemented 2026-07-22.** `tarjan_anim.rs` replays Tarjan's algorithm frame by
frame on the equation dependency graph. Trace recorded by `tarjan_scc_with_trace`
in `rumoca-phase-structural`. UI tab: "BLT ▶" in the structural view.

Captured 2026-07-21 (Claude, learning-driven). Animate the process of discovering
BLT blocks — Tarjan's algorithm finding strongly connected components (SCCs) in the
equation-variable dependency graph, then topological sorting.

- **Why it matters (linear algebra + graph theory):** the BLT decomposition is the
  structural analogue of permuting a matrix to block triangular form. Each block
  is an SCC: a set of equations that mutually depend on each other (an algebraic
  loop). The blocks are topologically ordered, so each block's inputs come from
  earlier (already-solved) blocks. Seeing the DFS stack grow, the low-link numbers
  update, and blocks pop off the stack one by one connects the graph algorithm to
  the matrix structure.
- **Sketch:** a step-by-step animation over the bipartite dependency graph (or the
  incidence matrix, with cells lighting up as the DFS visits them):
  1. DFS visits a node — highlight it, show discovery number
  2. Back edge found — show the low-link update, highlight the cycle
  3. SCC complete — outline the block, color it, add it to the BLT ordering
  4. Repeat until all nodes visited
  Synchronized with the spy-plot: as each SCC is discovered, the corresponding
  diagonal block appears in the BLT view.
- **Ties to idea #9** (animated algorithm views) — this is a concrete instance
  for one specific algorithm. Requires the instrumentation that emits per-step
  events from Tarjan's algorithm inside `rumoca-phase-structural`.
- **Textbook link:** Tarjan (1972), "Depth-first search and linear graph
  algorithms"; the Dulmage-Mendelsohn decomposition (Pothen & Fan, 1990).

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

## 25. ~~Live breakpoint arming on an already-running debug session~~ ✅ DELIVERED

Captured 2026-07-22 (Doug). **Delivered 2026-07-24.** Implemented via the **HRW
Debugger Bridge** VS Code extension (`hrw/vscode-extension/`). Claude writes
`.hrw-bridge/breakpoint-request.json` with conditional breakpoints keyed on the
captured item's identity; the extension calls `vscode.debug.addBreakpoints()` on
the running debug session. Breakpoints accumulate per specimen and are cleared
automatically when the specimen changes. The specimen list's context menu offers
**Recompile** to re-run compilation and hit armed breakpoints (the worker calls
`session.remove_document()` to bypass the session cache). See
`docs/debug-set-sites.md` for the protocol and `docs/architecture.md` §8 for the
full architecture.

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

## 27. Equation sheet — the flat DAE in readable math notation

**✅ Implemented 2026-07-25.** `equation_sheet.rs` renders the flat DAE as readable
equations with a precedence-aware pretty-printer (`expr_format.rs`).

Captured 2026-07-24 (Doug + Claude). The Flatten tab today shows the flat DAE as a
JSON tree. Enhancement: render the system of equations in **readable mathematical
notation** — one equation per line, variables and operators formatted as math, not
as nested JSON objects.

- **Why it matters:** the flat DAE is the central artifact of the entire compilation
  pipeline — everything before it (Parse through Typecheck) builds toward it, and
  everything after it (Structural through Simulation) operates on it. Yet the current
  view buries the equations inside a serde-value tree where `der(h) = v` reads as
  `{"lhs": {"Der": {"arg": {"ComponentRef": ...}}}, "rhs": ...}`. Rendering equations
  as math makes the "what does the solver actually see?" moment land immediately.
- **Foundation exists:** `expr_format.rs` already renders equation expressions as
  readable strings for the incidence matrix column labels (e.g. `der(h) = v`,
  `h + (-g) * t = 0`). The equation sheet extends this from single-line labels to a
  full formatted listing — variable classification (states, algebraic, parameters),
  equation grouping (by BLT block, by origin component), and residual vs explicit form.
- **Tour value:** the Flatten guided tour's central "aha" moment is "your Modelica
  `connect(a, b)` became *these* conservation equations." That moment needs a readable
  equation listing, not a JSON tree. The equation sheet would also improve the
  Structural Analysis tour (annotating which equations are in which BLT blocks) and the
  Index Reduction tour (showing which equations were differentiated).
- **Sketch:** a scrollable pane (tab or panel) listing each equation as formatted text,
  grouped by BLT block (if structural analysis has run) or by origin. Variable
  classification sidebar (state/algebraic/parameter). Click an equation to highlight
  its row in the incidence matrix; click a variable to highlight its column. Reuses
  `expr_format` for rendering, `incidence_view` for cross-linking.
- **Specimens:** RotationalInertia (small, readable system) → ProportionalLoop
  (algebraic loop visible in the equation grouping) → BouncingBall (hybrid equations
  with event conditions annotated).

## 28. Source-to-equation traceability — bridging the OO/flat divide

**✅ Implemented 2026-07-25.** `source_map_ui()` in `app.rs` renders a side-by-side
source-code / equation view with cross-highlighting.

Captured 2026-07-24 (Doug + Claude). A side-by-side or linked view showing which
Modelica source lines produced which equations in the flat DAE.

- **Why it matters:** the pipeline's biggest conceptual gap is between the
  object-oriented model (phases 1–4: Parse, Resolve, Instantiate, Typecheck) and the
  flat mathematical system (phases 5+: Flatten, Structural, Simulation). A `connect`
  statement in the source becomes conservation equations; a component's `equation`
  section becomes residual equations with qualified variable names; a `parameter`
  becomes a numeric constant. Without a visual bridge, these two worlds feel
  disconnected — the learner can't answer "where did equation 7 come from?" or
  "what happened to my `connect(flange_a, flange_b)`?"
- **Foundation exists:** Rumoca's IR carries `location` spans (byte offsets into the
  source file) through the pipeline. The bridge module's `ascend_provenance` already
  traces a node back to its source line. The equation sheet (#27) would give equations
  readable labels. Combining these: each equation in the flat DAE links back to the
  source line(s) that produced it.
- **Sketch:** two panes — Modelica source on the left, equation sheet on the right.
  Click a source line → highlight the equations it generated. Click an equation →
  highlight the source line(s) that produced it. Color-code by origin type: `connect`
  equations (flow sums, potential equalities), component equations, parameter bindings.
  For `connect`: show "these two flow variables sum to zero because of this connect
  statement."
- **Tour value:** the Flatten tour's story arc is "OO model → flat math." This view
  *is* that story arc, made visual. The guided tour can literally say "click on
  `connect(flange_a, flange_b)` and see the two equations it generated."
- **Relationship to #10 and #26:** cross-stage identifier tracking (#10) follows a
  *variable* through the pipeline; this follows an *equation* back to its *source*.
  Complementary: #10 answers "what happened to variable `v`?", this answers "where
  did equation 7 come from?" The VS Code integration (#26) could initiate this from
  a right-click on a `connect` statement.

## 29. Solver stepping visualization — what the integrator does at each time step

**✅ Implemented 2026-07-25.** Solver diagnostics (step size, Newton iterations,
convergence) are plotted in `simulation_pane()` alongside trajectory plots.

Captured 2026-07-24 (Doug + Claude). During simulation, visualize the solver's
internal decisions at each time step: step size adaptation, Newton iteration counts,
convergence behavior, and (for BDF) order changes.

- **Why it matters:** the simulation plot shows *results* but not *process*. The
  learner sees that `BenchActuator` converges and `BouncingBall` bounces, but not
  *why* the solver chose those step sizes, *why* it needed 4 Newton iterations at
  one point and 1 at another, or *why* BDF order 2 was selected over order 1. These
  are the core questions of numerical methods for DAEs — and they're invisible today.
  Seeing the solver struggle at a stiff transient (step size shrinking, Newton
  iterations climbing) and then recover (step size growing, iterations dropping)
  makes the theory of stiffness, implicit methods, and adaptive control concrete.
- **Complements #18 and #22:** idea #18 (BDF step-size and order) covers the
  integrator's macro-level decisions; idea #22 (Newton convergence) covers the
  per-step micro-level. This idea unifies both into a single solver-stepping view
  that shows the complete picture: at time t, the solver took a step of size h,
  at BDF order k, requiring n Newton iterations with final residual r. Together
  they answer "what is the solver actually doing?" at every level of detail.
- **Sketch:** a secondary plot panel synchronized with the trajectory plot's time
  axis. Three time-series sub-plots stacked vertically:
  1. **Step size h(t)** — log scale, showing adaptation. Dramatic shrinkage at
     events or stiff transients; smooth growth in quiet regions.
  2. **Newton iterations per step** — integer values, typically 1–6. Spikes
     indicate coupling or near-singularity in the BLT blocks.
  3. **BDF order k(t)** — integer 1–5, showing the stability/accuracy trade-off.
  Click a time point → see the details: which BLT block required the most Newton
  iterations, what the residual norm was, whether an event was detected nearby.
- **The stiffness story:** run `BenchActuator` with both BDF and RK45. Overlay
  the step-size plots. BDF takes ~100 large steps through the stiff actuator
  transient; RK45 takes ~10,000 tiny steps (stability-limited). The visual
  contrast *is* the explanation of stiffness — no textbook definition needed.
- **Rumoca entry point:** `rumoca-sim::simulate_solve_model` currently returns only
  the resampled output grid. Instrumentation needed: a per-step callback or a
  post-hoc log recording `(t, h, order, newton_iters, residual_norm,
  event_detected)` from diffsol's integration loop. This is pass-two
  instrumentation territory — the simulation loop is inside `rumoca-sim`.
- **Textbook link:** Hairer & Wanner, *Solving ODEs II* (BDF step/order control);
  Brenan, Campbell & Petzold, *Numerical Solution of Initial-Value Problems in
  DAEs* (Newton convergence on DAE systems).

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

## 32. In-app tour view — render guided tours inside HRW with clickable navigation

**✅ Implemented 2026-07-25.** Three UI modes (Tour/Specimen/Debug) with
`egui_commonmark` rendering markdown in the left panel. Tour mode renders the
end-to-end tour document with clickable `hrw://` navigation links that load
specimens and switch stage tabs. Specimen mode shows the specimen list (top ⅓)
and narrative (bottom ⅔). Debug mode hides the left panel for side-by-side with
VS Code. The `hrw://` link scheme (`hrw://load/<Specimen>`,
`hrw://stage/<StageName>`, `hrw://load/<Specimen>/<StageName>`) works in both
tour and narrative documents via `egui_commonmark`'s link hooks API. Styled with
blue section headers consistent with the RHS palette. Window launches maximized
by default; `--half` flag for debug layout. Original idea below.

**Problem:** The end-to-end guided tour (`docs/compiler-phases/end_to_end_tour.md`)
lives in a markdown file that the user reads in VS Code. The tour references HRW
stage views ("click the Parse tab", "expand equations") but has no way to actually
*drive* HRW from those references. Multiple approaches to making deep links work in
VS Code's markdown preview failed (DocumentLinkProvider, editor decorations,
`vscode://` URI handler, `command:` URIs) — the VS Code extension environment in
Remote WSL proved too opaque to debug effectively. See commit `100dab9f` (revert).

**Remaining extensions:**
- **Sub-view links:** `hrw://view/SpyPlot`, `hrw://view/Incidence` — switch to a
  specific sub-view within a stage (e.g. Structural → SpyPlot vs Incidence vs Tree).
- **Tree-node links:** `hrw://node/<path>` — expand the tree to a specific node and
  scroll to it. Useful for tour steps like "expand equations → GearWithBrake → body."
- **Multiple tour documents:** currently only the end-to-end tour; future phase-specific
  tours could be selectable from a dropdown.
- **Tour progress tracking:** checkmarks, "you are here" indicator, bookmarks.

**Relates to:** #24 (guided tours as HRW-driven walkthroughs), #9 (animated
algorithm stepping — the tour could embed step controls).

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

## 39. ~~Crash and diagnostic log — make HRW troubleshootable without a live session~~ ✅ DELIVERED

**Delivered 2026-07-28** (`src/diagnostics.rs`, `examples/crash_probe.rs`) — see
`architecture.md` § 9 *Crash and diagnostic log*. Built essentially as captured:
panic hook + per-frame app snapshot + action ring buffer + log tail + build
identity, in `.hrw-bridge/diagnostics/`. The `session.json` half covers deaths
that run no hook, and `Help ▸ Write diagnostic snapshot` covers problems that do
not kill the app. The original capture follows.

Captured 2026-07-28 (Doug), after HRW crashed instantly on left-clicking an
identifier in the specimen source view:

> So far, you have done an amazing job of troubleshooting problems. But, that
> might not always be possible. Today's crash … reminded me that we have skipped
> a step in creating HRW.

**The problem.** When HRW dies, the evidence dies with it. Today's crash was
diagnosed only because the failing path could be *re-created headlessly* — a
test compiled `MotorWithBrake` and called `summarize_tracking`, which reproduced
the panic with its message and location. That worked because the crash lived in
pure logic reachable from a test. A crash in the paint path, in a drag, in
frame-timing, or one that depends on GPU or window state would not have been
reproducible that way, and there would have been nothing to reason from but
Doug's description of what he clicked.

The gap is structural, not incidental: HRW is a **windowed** app. A Rust panic
prints to stderr, and when the app is launched from the VS Code debugger or from
Explorer there is often no stderr anyone reads. HRW has already lost evidence
twice this way — today's panic, and the earlier `exit code 101` from
egui-wgpu's staging-buffer failure during long debugger pauses, which took
several rounds of guessing before Doug happened to capture the message.

**What it should capture.** A panic message and a backtrace are the easy half
and the less useful half. Location says *where* the process died; it rarely says
*why the app was there*. The HRW-specific half is the **application state at the
moment of death**:

- Selected specimen, model name, current stage, current detail view.
- What was pointed at and what was being followed — the assembled noun
  (`PointedAt`, `tracked_identifier`, the sequence counters).
- Live/animation state: which animation, frame index of total, `LiveState`.
- The tail of the existing `log_entries` buffer (`worker::LogEntry`), which
  already carries timestamped per-phase detail — the log view's data, persisted.
- Build identity: git rev, Rumoca rev, `wgpu` backend in use.

Today, every one of those bullets was reconstructed from Doug's sentence
describing what he did. All of them are already in `HrwApp`.

**Design notes.**

- **Claude is the consumer** (see `DECISIONS.md`, 2026-07-28) — this file is not
  a user-facing error report. Optimise it for *diagnosis*, not readability, and
  do not summarise away detail.
- A `std::panic::set_hook` that writes a timestamped file (say
  `.hrw-bridge/crashes/<timestamp>.json`) covers panics. Reaching `HrwApp` state
  from inside the hook is the design problem worth thinking about — likely a
  small `Mutex`/`ArcSwap` snapshot the app refreshes each frame, so the hook
  reads a plain value and never touches the app's borrow graph.
- **Not all deaths are panics.** GPU device loss, an `abort`, or a hard kill
  leave no hook to run. A rolling "last frame state" written cheaply (or an
  atomic frame counter plus periodic flush) would still say what HRW was doing.
- Worth considering: keep a short ring buffer of recent *user actions* (clicks,
  stage changes, follows) rather than only final state — a crash's cause is
  usually the action before last, not the state after.
- Consider surfacing existing crash files in the UI, so Doug can hand one over
  without hunting for a path.

**Why it is worth doing before it is needed.** The value shows up only on the
day something is *not* reproducible from a description — and on that day it
cannot be added retroactively. Cheap to build, and it converts "HRW crashed when
I clicked something" from a guessing game into a file.

**Relates to:** `log_view` and `worker::LogEntry` (the infrastructure already
exists; this persists it), the Context Bar's `PointedAt` state, and the
`architecture.md` § Live trace debugging notes on the egui-wgpu device-loss
failure.

---

## 40. ~~Instrument `pre()` lowering~~ ✅ DELIVERED

**Delivered 2026-07-29.** `rumoca-phase-dae` gained `to_dae_with_options_traced`
and `lower_pre_operator_with_trace`; HRW gained `pre_lowering_anim`, a sub-tab on
the **Events** stage. Four beats replay: discover → name → materialize →
substitute.

**Two findings the work produced.**

1. **`LiveTrace` did not need to generalise — the phases do not need it.** The
   question this idea was written to answer had a better answer than expected:
   `rumoca-phase-dae` takes an **observer callback**, not a `LiveTrace`, because
   `LiveTrace` lives in `rumoca-phase-structural` and that dependency would run
   backwards through the pipeline. HRW owns the `LiveTrace` and passes a closure.
   More upstreamable, and the existing three phases could migrate to it.
2. **The instrumentation immediately falsified a documented claim** — see the
   correction below, which was the first thing it produced.

What *did* generalise is `Playback<T>`: the new view declares no cursor, no
timing, no channel, and compiled first try. That is the payoff from sequencing
the animation debt ahead of this.

The original capture follows.

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
and one broken: `crates/rumoca-sim/src/diffsol/tests/scalarization_regressions.rs`.
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

   **Still below the reach of a link:** an animation **frame position**, the
   pointed-at node, the followed identifier. A tour can now say "open the Tearing
   view" but not "…and go to frame 7, where `command` won". Frame addressing is the
   next natural increment; nothing has needed it yet, so it waits for a tour that
   does.
3. **Ad hoc specimens — split, do not repurpose.** Doug offered `specimens/` for
   repurposing; recommend against. The curated corpus has real properties
   (portable Modelica subset, `// purpose:` comments, System Modeler round-trip
   intent) that scratch models would quietly degrade. Two directories: curated,
   and generated-for-a-question. Generated ones are ephemeral on the same rule.

### The most valuable consequence

**Specimens become a medium of explanation.** "Here is the smallest model that
exhibits the thing you asked about" is what a good teacher does, and it is
currently impossible — Claude can only point at the models that already exist.
This also feeds the Cellier loop directly: his problems often specify a system
that could be realised as a specimen and actually run through Rumoca, so a
claimed answer can be *checked* rather than asserted.

### Specimen notebook conversion (authorised 2026-07-29)

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

## 44. ~~Show `Matching ▶` when the Structural stage is singular~~ ✅ DELIVERED

**Found 2026-07-29 by writing the first ad hoc tour** — the first requirement the
#42 mechanism produced, on its first use. **Fixed the same day**, pre-emptively: no
question was waiting on it, and the cheapest moment to fix a hole is while nothing is
blocked by it (see the priority order in `docs/tech-debt.md`).

**The fix was one UI condition.** Nothing had to be built: the trace already emitted
`MatchingStep::EquationFailed` and `matching_anim` already painted the failed row red
with "has no augmenting path — unmatched (rank deficiency)". The feature was **written
and then gated out of reach**, and nothing tested it, which is how it stayed hidden.
`a_singular_report_still_animates_and_ends_on_the_failure` now pins it against real
`MotorWithBrake` data: exactly one failure, 47 of 48 matched.

**The `Tearing ▶` / `Spy-plot` half of this idea was deliberately NOT done, and the
original suggestion was wrong.** Showing them with an explanatory message sounded
kinder than absence, but a BLT decomposition or a tearing result computed from a
*partial* matching would be plausible-looking and meaningless — a "makes Claude
guess" hazard, which the priority order ranks above tour holes. Absence is correct
there. The distinction is that `Matching ▶` replays a *search*, so its failure is the
content; the other three consume a *result* that does not exist yet.

Doug asked what a rank deficiency of 1 means. The best available answer is to watch
Kuhn's algorithm exhaust its augmenting paths and give up on the 48th equation.
**That view is hidden.** `app.rs` gates four sub-tabs on `!is_singular ||
is_index_reduction`:

```rust
if !is_singular || is_index_reduction {
    ui.selectable_value(.., StructuralView::SpyPlot, "Spy-plot");
}
ui.selectable_value(.., StructuralView::Incidence, "Incidence");
if !is_singular || is_index_reduction {
    ui.selectable_value(.., StructuralView::MatchingAnim, "Matching ▶");
    ui.selectable_value(.., StructuralView::TarjanAnim, "BLT ▶");
    ui.selectable_value(.., StructuralView::TearingAnim, "Tearing ▶");
}
```

**The gating is right for three of the four and backwards for the fourth.** A spy
plot, a BLT decomposition and tearing all require a *complete* matching before they
mean anything — hiding them on a singular system is correct. But the matching
**animation** is a replay of the *search*, and the search failing is the most
instructive thing on that tab. It is hidden exactly when it would teach the most.

**What it should do instead.** `matching_anim` already builds from
`IncidenceMatrix::from_report`, and a singular Structural report *does* carry an
`incidence` and a partial `matching` (`partial_matching_to_json`), so the data is
present. The animation should run and **end on the failure**: the last frame is an
augmenting-path search that finds no path, and the running-state panel already says
"Matched 47 of 48 — still unmatched: …", which is precisely the sentence the
question needs. Check what `maximum_matching_with_trace` emits when a search fails —
whether there is a distinct terminal step or whether the frame stream simply stops —
and if there is no explicit "gave up" step, that is a small `rumoca-phase-structural`
addition of the same shape as the tearing `NoProgress` variant.

**Also worth reconsidering:** `Tearing ▶` on a singular Structural tab could
legitimately say "no blocks to tear — there is no matching to decompose", which is
more informative than the tab being absent. Absence reads as "this feature does not
exist here"; a message reads as "here is why there is nothing to show." The same
argument applies to the spy plot.

**Relates to:** #42 (produced this), #9 (the animation set), the question ledger
entry for "what does a rank deficiency of 1 mean", and `docs/answer-platform-plan.md`
Phase 3 — this is a concrete item for it, with a real question behind it rather than
a guess at likely demand.

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

- **Structural singular — well emitted, but the spans are dropped.**
  `structural_error_to_json` emits `n_equations`, `n_unknowns`, `n_matched`,
  `rank_deficiency`, `unmatched_equations`, `unmatched_unknowns`, `guidance` — enough
  for today's answer. But `StructuralError::Singular` *also* carries
  **`unmatched_unknown_spans`**, whose own doc comment says it exists "so the failure
  is traceable back to source", and **HRW does not emit it.** Rumoca hands over the
  source traceability and HRW drops it on the floor. That is the single cheapest fix
  here and the one that turns "unknown `emf.p.v`" into "line N of your model".
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
2. **Audit the other failure payloads** for source location; add spans where Rumoca
   has them and widen visibility where it does not.
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

## 48. Memoize compiled specimens across tests

Split out 2026-07-29, when Doug asked how to run the test suite in parallel and the
measurement said parallelism was the wrong lever.

### What the measurement said

| | |
|---|---|
| 49 tests taking over 1s | **180.3s** |
| The other 353 tests | **~2.7s** |
| Of those 49, worker tests | **47** |

Every worker test acquires `shared_worker()` — a global `Mutex<WorkerState>`, needed
because Rumoca's `Session` is not thread-safe and because loading the MSL once is worth
a great deal. **So they serialize regardless of `--test-threads`.** Going parallel would
have taken 183s to roughly 181s.

**Doug ruled out the concurrency work on that basis** (per-test bridge directories,
fixing the process-global stdout capture, per-thread workers each loading the MSL). Do
not revisit it: the cost is high, the machine has limited memory, and the return is two
seconds.

**Delivered instead:** the `slow-tests` feature gate (#1 of that discussion), which took
the between-edits loop from 183s to **7.3s** across 353 tests. That solved the *inner
loop*. This item is what shortens the *full* run.

### The actual cost: the same specimens, compiled over and over

**37 `compile_specimen_shared` call sites cover only 12 distinct specimens**, plus two
"all healthy specimens" tests that compile ten each. `Drivetrain` is compiled from
scratch five or six times per run. And each compile is deliberately **uncached**
(`compile_model_strict_reachable_uncached_with_recovery`) because HRW is an observatory
and the phases must actually run — right for the app, expensive for a test suite.

### Sketch

A `OnceLock<Mutex<HashMap<String, FromWorker>>>` beside the existing shared worker:
compile each specimen once per test process, hand out clones. The payload types
(`StageBundle`, `DefInfo`, `EquationSheet`, `IdentifierIndex`, `Dae`, flat `Model`) are
all `Clone` — checked 2026-07-29.

Tests that need a genuinely fresh compile **opt out explicitly** via the existing
uncached path: `a_broken_specimen_does_not_poison_the_next_compile` (whose entire subject
is cross-compile contamination), and anything arming a breakpoint.

Estimated 180s → 60-70s.

### The caveat worth building in

Memoizing **weakens the suite**: the second test to ask for `Drivetrain` no longer
verifies that compiling it is *reproducible*. Cheap mitigation — keep one test that
compiles a specimen fresh and compares against the memoized result. That checks precisely
the property memoization could hide, and it is the kind of silent coverage loss that
`project-tours-multiply-testing` warns about (a detector that quietly stops detecting).

**Relates to:** the `slow-tests` gate in `Cargo.toml`, `README.md`'s two test commands,
and `shared_worker()` in `worker.rs`.

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
