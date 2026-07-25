# End-to-End Tour Plan

The plan to upgrade the end-to-end guided tour from a reading tour
(`docs/compiler-phases/end_to_end_tour.md`) to an interactive, HRW-driven
walkthrough. Grounded in the GearWithBrake specimen.

**Principle:** infrastructure features (what to make visible) are built now,
driven by the tour. Pedagogical narrative (what to teach deeply) is refined
later, driven by Doug's Cellier reading. The two concerns are orthogonal.

## Current state

The existing end-to-end tour is a 13-stop text document — a first draft
reading tour with learning goals and concrete GearWithBrake data, but no
interactive HRW integration. The Structural Analysis phase tour
(`phase7_structural_analysis/guided-tour.md`) is the model for what
"interactive" looks like: three-tier progression (snapshot, replay,
live-stepped), lesson-structured, with HRW view references.

### Tour stop readiness

| Stop | Phase | Visual today | Gap |
|------|-------|-------------|-----|
| 0 | Why can't we simulate? | Text only | Adequate (framing) |
| 1–4 | Parse → Typecheck | JSON tree inspector | Adequate for generic tour |
| **5** | **Flatten** | **JSON tree** | **Worst stop — equations buried in nested JSON** |
| **5→6** | **OO → flat bridge** | **Nothing** | **The conceptual crux is invisible** |
| 6 | DAE / variable classification | JSON tree | Improved by equation sheet |
| 7 | Index reduction | Reduction report | Adequate; Pantelides animation later |
| 8 | Structural analysis | Incidence, matching anim, BLT, Tarjan | **Already strong** |
| 9 | Initialization | Determinacy view | Adequate |
| 10 | Events | Event listing | Adequate |
| 11 | Solve lowering | JSON tree | Tolerable; equation sheet helps |
| **12** | **Simulation** | **Trajectory plot only** | **Results without solver process** |

Three dead spots: Flatten, the OO-to-flat bridge, Simulation.

## Essential features (backlog ideas)

Three features fix the three dead spots. Dependencies form a critical path:

```
#27 Equation sheet ──► #28 Source-to-equation traceability
        │
        │  (independent)
        │
#29 Solver stepping visualization
```

- **#27 → #28:** traceability needs readable equations to link to.
- **#29** is independent — can be built in parallel with #27/#28.

### Deferred (not required for the generic tour)

- **#10 Cross-stage identifier tracking** — valuable but expensive; depends
  on #27 and #28; revisit after the tour works without it.
- **#26 Rumoca VS Code extension integration** — the polish/convenience layer;
  revisit after HRW-native views prove out.
- **#9 Pantelides animation** — the Index Reduction stop is adequate with the
  existing reduction report; animate later.

### Decision: Rumoca VS Code extension

**Not using it for this plan.** The Rumoca extension (`packages/vscode`)
provides language support (LSP, syntax highlighting) but has no debug
integration. The three essential features (#27, #28, #29) are all
HRW-native — no VS Code extension changes needed. The VS Code integration
(#26: right-click in source → trace/debug) is a convenience upgrade to
consider *after* the core views exist. Avoids: TypeScript development,
upstream coordination with James Goppert's extension, and a second
deployment target.

---

## Phases

### Phase 1: Equation sheet (#27)

The single highest-leverage feature. Transforms the weakest tour stop
(Flatten) from "look at JSON" to "read the equations the solver sees."

- [x] Extend `expr_format.rs` from single-equation labels to a full
  equation listing renderer
- [x] Add an "Equations" sub-tab to the Flatten stage view (alongside the
  existing JSON tree)
- [x] Group equations by origin (component equations, connect-generated,
  initial)
- [x] Show variable classification table (states, algebraic, parameters,
  with start values and units)
- [x] Click an equation → highlight its incidence matrix row (cross-link
  to Structural tab)

**Milestone:** open GearWithBrake → Flatten → Equations → see all 44
equations in readable math instead of JSON.

### Phase 2: Source-to-equation traceability (#28)

Bridges the OO/flat divide — the conceptual crux of the tour.
Depends on Phase 1 (needs readable equations to link to).

- [x] Use Rumoca's `span` byte offsets to trace each flattened equation
  back to its Modelica source line
- [x] Add a split-pane linked view: Modelica source ↔ equation sheet
  ("Source Map" sub-tab on Flatten)
- [x] Click a source line → highlight equations it generated; click an
  equation → highlight source line(s)
- [x] Color-code by origin type: `connect` equations, component equations,
  parameter bindings (category colors on group headers and source gutter)

**Milestone:** click `connect(gear.flange_b, load.flange_a)` in the source
pane → see the two conservation equations it generated highlighted in the
equation sheet.

### Phase 3: Solver stepping (#29)

Completes the Simulation stop. Independent of Phases 1–2; can overlap.

- [ ] Instrument `rumoca-sim` to emit per-step data (t, h, order,
  newton_iters) via a callback or shared buffer (same pattern as `LiveTrace`)
- [ ] Add a secondary plot panel below the trajectory plot: step size h(t),
  Newton iterations, BDF order
- [ ] Synchronized time axis with the trajectory plot

**Milestone:** run GearWithBrake → see step-size shrinkage at brake events
and Newton iteration spikes at the coupled block, alongside the velocity
trajectory.

### Phase 4: Wire into the tour

Upgrade the end-to-end tour document from reading tour to interactive tour.

- [ ] Rewrite each stop to reference the new views: "click Flatten →
  Equations" instead of "examine the JSON"
- [ ] Add the three-tier structure from the Structural Analysis tour
  (snapshot → replay → live-stepped) where applicable
- [ ] Update learning goals to include the new visual capabilities
- [ ] Verify every stop works with a fresh GearWithBrake load in HRW

**Milestone:** a learner can follow the tour document with HRW open and
see meaningful visuals at every stop — no JSON-tree dead spots.

---

## Sequencing notes

- Phases 1 and 3 can overlap (independent).
- Phase 2 depends on Phase 1.
- Phase 4 depends on all three.
- Each phase is a natural commit/push boundary.
- The instrumentation in Phase 3 (changes to `rumoca-sim`) should be
  committed separately from HRW code per the split-instrumentation rule.
