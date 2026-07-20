# <Model> — a compilation narrative

*A per-specimen lab-notebook entry: the story of one specimen's trip through the
Rumoca pipeline, told against the committed [`trace/`](trace/) (ground truth) and
foregrounding what **this** specimen is designed to make interesting.*

> **Provenance.** Written against `trace/` from
> `cargo run --example gen_trace -- <Model>`, Rumoca `rev <rev>` (see
> [`trace/manifest.json`](trace/manifest.json)). Regenerate on a specimen edit or
> Rumoca pin bump, then re-read this narrative against the diff — claims below cite
> specific trace locations, so a stale claim is a checkable one.

---

## Why this specimen exists

<The one phenomenon this specimen is authored to trigger — the Rumoca feature or
mathematical structure it exercises. State it crisply, and how it relates to the
sibling specimens (link them).>

---

## The pipeline, stage by stage

<Early stages briefly (they are generic — link `docs/compiler-phases`); expand the
boundary where this specimen gets interesting. Cite the trace file at each step.>

- **Parse → [`trace/parse.json`](trace/parse.json)** — …
- **Resolve → [`trace/resolve.json`](trace/resolve.json)** — …
- **Instantiate / Typecheck → [`trace/instantiate.json`](trace/instantiate.json),
  [`trace/typecheck.json`](trace/typecheck.json)** — …

### Flatten → [`trace/flatten.json`](trace/flatten.json)
<The flat DAE: variables (parameters vs states vs algebraic unknowns) and the
residual equations that matter, rendered from the trace.>

### Structural → [`trace/structural.json`](trace/structural.json)
<Matching → BLT blocks → tearing, grounded in the report. If the specimen is
structurally singular, note that `structural.json` is absent and read the verdict
from `trace/manifest.json`.>

---

## Contrast across the notebook

- vs [`<Other>`](../<Other>/narrative.md): …

## References

<`docs/compiler-phases` chapters for the phases in focus, plus durable external
citations (textbook + section, DOIs, the Modelica spec) — verify links when added.>
