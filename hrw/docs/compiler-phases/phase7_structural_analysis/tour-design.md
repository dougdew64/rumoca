# Structural Analysis Guided Tour — Design

Working document for the first HRW-driven guided tour (idea #24). This tour
proves the concept: theory + HRW actions + specimens = interactive learning.

## Progression — five lessons, simple to complex

| Lesson | Specimen | Concepts | What the learner sees |
|--------|----------|----------|----------------------|
| 1. The incidence matrix | SingleInertia | Eq-unknown dependency, sparsity, bipartite graph | Incidence view: small sparse matrix, hover to identify pairs |
| 2. Maximum matching | SingleInertia → ProportionalLoop | Transversal, structural rank, assigning unknowns to equations | Matched-pair overlay on incidence matrix + animated matching stepper |
| 3. BLT decomposition | ProportionalLoop, MixedLoop | SCCs, scalar vs coupled blocks, topological order, algebraic loops | Spy-plot + BLT block boundaries on incidence matrix + animated Tarjan stepper |
| 4. Tearing | ProportionalLoop, TwoLoops | Tear vars, residuals, causal sequence, Newton dimension reduction | Spy-plot tooltip (tear/residual info) |
| 5. Structural singularity | Drivetrain | Unmatched rows/columns, rank deficiency, why index reduction needed | Unmatched row/column highlighting |

## Gap analysis — HRW enhancements needed

### Essential (v1 tour blockers) — ALL DONE

1. ~~**Matched-pair indicators on incidence matrix**~~ — DONE. Green circles on
   transversal diagonal cells; caption shows matching count and rank info.

2. ~~**BLT block boundaries on incidence matrix**~~ — DONE. Amber outlines on the
   incidence view, thicker for coupled blocks.

3. ~~**Unmatched row/column highlighting**~~ — DONE. Faint red bands on unmatched
   rows and columns.

### Stretch (animated algorithm stepping) — ALL DONE

4. ~~**Animated matching**~~ — DONE. `matching_anim.rs` replays Kuhn's algorithm
   frame by frame on the incidence matrix. Trace recorded by
   `maximum_matching_with_trace` in `rumoca-phase-structural`. Tab: "Matching ▶".

5. ~~**Animated BLT discovery**~~ — DONE. `tarjan_anim.rs` replays Tarjan's SCC
   algorithm frame by frame on the dependency graph. Trace recorded by
   `tarjan_scc_with_trace` in `rumoca-phase-structural`. Tab: "BLT ▶".

### Deferred (not needed for this tour)

- Full permuted-matrix toggle (#15 full version)

## Implementation order

1. ~~Build the three essential enhancements (incidence view additions)~~ — DONE
2. ~~Build animated stepping (matching, then Tarjan)~~ — DONE
3. Write the five-lesson tour document — NEXT

## Instrumentation added to Rumoca crates

These additions are additive, observation-only, and upstreamable:

- `crates/rumoca-phase-structural/src/matching.rs` — `MatchingStep`, `MatchingFrame`,
  `MatchingTraceResult`, `maximum_matching_with_trace()`. The `matching` module
  is now `pub` (was `mod`).
- `crates/rumoca-phase-structural/src/tarjan.rs` — `TarjanStep`, `TarjanFrame`,
  `TarjanTraceResult`, `tarjan_scc_with_trace()`. The `tarjan` module is now
  `pub` (was `mod`).
