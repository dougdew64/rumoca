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

**Partially implemented 2026-07-22.** Matching (augmenting paths) and BLT discovery
(Tarjan SCC) now have animated steppers — see `matching_anim.rs` and `tarjan_anim.rs`.
Trace infrastructure added to `rumoca-phase-structural` (`maximum_matching_with_trace`,
`tarjan_scc_with_trace`). Remaining candidates: Pantelides, tearing, Newton iteration.

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

Captured 2026-07-22 (Doug + Claude). The weekly tech-debt scan already catches
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
- **What it would look like:** a periodic (monthly, not weekly) pass with
  `cargo flamegraph` or `perf` on a representative specimen, looking for hot
  spots in the UI thread. Focus areas: tree rendering (deep/wide JSON), canvas
  painting (large matrices), and channel throughput (many `CompileProgress`
  messages per compile).
- **For now:** let the weekly tech-debt scan catch performance issues as they
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
  This is step 5 in the [cross-stage tracking plan](cross-stage-tracking-plan.md).

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
