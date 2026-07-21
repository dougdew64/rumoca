# Ideas — backlog for future implementation

Captured ideas not yet scheduled. **These are candidates, not commitments** — no
arc depends on them, and settled decisions live in [`DECISIONS.md`](../DECISIONS.md),
current work in [`CLAUDE.md`](../CLAUDE.md). Promote an item here into an arc /
decision when it's picked up.

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
  debugger breakpoint flow ("arm it") should be able to land inside an algorithm
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
- **Sketch:** a Ctrl+F-style search bar (or a text field in the tab bar / right
  panel) that fuzzy-matches against qualified names in the current view — tree node
  keys, variable names in the flat model, equation labels, spy-plot row/column
  headers. Matching nodes auto-expand and scroll into view; matching matrix
  rows/columns highlight.
- **When:** can start independently of #10 — a single-view search is simpler and
  immediately useful.

## 12. HRW architecture document — how the code works

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
