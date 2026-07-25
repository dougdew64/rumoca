# Cross-Stage Identifier Tracking Plan (#10)

Trace a Modelica identifier across all 11 pipeline stage views. Click an
identifier in the specimen source → highlights propagate to every stage.

**Principle:** accuracy and correctness required. No heuristic name-matching.
All mappings use Rumoca's typed provenance (`def_id`, `component_ref`,
`source_span`, `symbol_ancestry`).

---

## Feasibility (confirmed 2026-07-25)

Rumoca preserves rich identity through flattening:

- Every flat/DAE **variable** carries `component_ref` (with `def_id`) and
  `source_span` pointing back to its source declaration.
- Every flat/DAE **equation** carries `span` and typed `EquationOrigin`.
- The flat model carries `symbol_ancestry` (`DefId → Arc<[DefId]>`).
- All `VarRef` expressions in equations carry structured `ComponentReference`
  objects (via the `structured_refs.rs` postprocess pass).

The `def_id` → qualified flat name mapping can be built trivially by iterating
flat variables and reading `component_ref.def_id`. No invasive Rumoca changes
needed — the provenance is already there; HRW just needs to extract and index it.

---

## Design decisions

### Entry point: Modelica source in the Specimen mode LHS

The source view lives in the Specimen mode left panel (bottom two-thirds),
toggleable with the existing narrative view. Default: source view. This puts
the source text next to *any* stage view on the RHS — not buried inside
the Flatten tab's Source Map.

### All interactions in HRW, not VS Code

The VS Code extension path was evaluated and rejected. Context selection
happens in HRW — consistent with every other HRW interaction.

### Incremental, one stage at a time

Each stage has different data structures and highlighting semantics. Build
and test one stage view at a time, with incremental commits. Reverts should
be cheap.

---

## Implementation steps

### Step 1: Source view toggle in Specimen mode LHS ✓

Add a toggle in the Specimen mode section header: "Source" | "Narrative".
Default to Source when a specimen is selected. Load and render the specimen's
Modelica source text in the bottom two-thirds of the LHS panel. No
identifier clicking yet — just readable source text.

**Files:** `app.rs` (new enum, toggle UI, source rendering)

### Step 2: Build the IdentifierIndex in the worker ✓

After compilation, build an `IdentifierIndex` mapping each source identifier
(by `def_id` and/or `source_span`) to its representations in post-flatten
stages: flat variable name(s), equation indices, incidence matrix column(s),
variable classification row, solver slot, simulation series name.

Send the index from worker → UI alongside the existing `Compiled` message.

**Files:** `identifier_index.rs` (new module), `worker.rs` (index construction),
`app.rs` (store the index)

### Step 3: Make source identifiers clickable ✓

For each source line, look up variables declared on that line via the
`IdentifierIndex`. Find the leaf name (last dot-segment of the qualified
flat name) in the line text and render it as a clickable, underlined,
blue-tinted label. On click, toggle `tracked_identifier` on `App`.
Hover shows the full qualified flat name. Active tracking uses gold highlight.

**Files:** `identifier_index.rs` (`clickable_spans`, `find_whole_identifier`),
`app.rs` (`tracked_identifier` field, source view rendering)

### Step 4: Wire highlighting — one stage at a time ✓ (6 of 11)

Gold highlight (rgba `0xFF, 0xD5, 0x4F`) used consistently across all views.
A tracking indicator bar ("Tracking: name ✕") appears above the stage content.

| Order | Stage view | Highlight mechanism | Status |
|-------|-----------|-------------------|--------|
| 4a | Equation Sheet | Background-highlight equations containing the variable; bold+highlight its row in the classification grid | ✓ |
| 4b | Incidence Matrix | Persistent gold column band for the matched unknown (via `column_index` + `highlighted_col`) | ✓ |
| 4c | Simulation plot | Gold color + 3× line width for the matching time series | ✓ |
| 4d | Spy plot | Gold outline stroke around the BLT block containing the variable | ✓ |
| 4e | Source Map (Flatten) | Gold background on the source line declaring the tracked variable | ✓ |
| 4f | Tree inspector (Parse) | Deferred — tree-walk highlighting requires threading `def_id` through recursive renderer |  |
| 4g | Tree inspector (Resolve) | Deferred (same as 4f) |  |
| 4h | Tree inspector (Instantiate) | Deferred (same as 4f) |  |
| 4i | Tree inspector (Typecheck) | Deferred (same as 4f) |  |
| 4j | Reduction view | Gold background on demoted states, differentiated-equation rows, and eliminated variables | ✓ |
| 4k | Remaining views (Init, Events, Solve Lowering) | Deferred — low data density for identifier tracking |  |

### Step 5: Bidirectional highlighting

Click in *any* stage view (not just the source) to set the tracked identifier.
Click an incidence column → source highlights. Click a tree node with a
`def_id` → all other views highlight. The source view is the first entry
point; bidirectional makes every view an entry point.

---

## Sequencing notes

- Steps 1–3 are sequential (each depends on the prior).
- Step 4 sub-steps are independent of each other (any order works).
- Step 5 is a polish pass after step 4.
- Step 1 is pure UI with no cross-stage logic — a clean first commit.
