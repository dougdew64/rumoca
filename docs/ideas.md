# Ideas — backlog for future implementation

Captured ideas not yet scheduled. **These are candidates, not commitments** — no
arc depends on them, and settled decisions live in [`DECISIONS.md`](../DECISIONS.md),
current work in [`CLAUDE.md`](../CLAUDE.md). Promote an item here into an arc /
decision when it's picked up.

---

## 1. Narratives for *simulation*, especially convergence-failure troubleshooting

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
