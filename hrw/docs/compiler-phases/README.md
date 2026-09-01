# How Rumoca works — the phase-by-phase notes

**Purpose:** the entry point to Rumoca's compiler pipeline: where to start, what each phase
does, and which drill-down answers which question.
**Status:** 👤 reference — **the closest thing that exists to Rumoca documentation.** Upstream
has none of this. Claude wrote these files for Doug **before the HRW project existed**, and Doug
copied them in.
**Read when:** you want to understand a compiler phase, or you are about to change code that
touches one.

**Maintenance: these are refreshed at the Rumoca version bump**, not at every change — steps 6
and 7 of [`../updating-rumoca.md`](../updating-rumoca.md), which own the procedure. Nothing else
imposes an obligation on them, deliberately: a per-change tax on files no checker reads is how
they would come to be resented rather than kept.

**How to read them between refreshes.** They describe Rumoca **as of the last refresh** and carry
no provenance tags, so treat them as **a map, not a verified claim — the source is the arbiter.**
Their 41 `crates/` citations *are* checked by `doc_citations`, so a rebase surfaces what moved;
prose around a still-valid path can be stale without anything saying so. The 18 places quoting a
source **line number** are the first thing to distrust.

## Start here

**[`the-chain-of-problems.md`](the-chain-of-problems.md)** — why the pipeline has the shape it
has. Every phase stated as a response to a *specific insufficiency* in what came before, plus
the structural-vs-numerical distinction and the reading list.

**Read it first even if you only care about one phase.** A phase in isolation looks arbitrary;
a phase as an answer to the previous phase's shortfall does not. And **the chain is not a
Rumoca design choice** — every Modelica tool traverses it in some form, which is what makes
learning it transferable rather than trivia about one compiler.

Then **[`high_level_overview.md`](high_level_overview.md)** for the component inventory: what
Rumoca is, and what each phase produces.

## The phases

| # | Phase | Turns | Drill-downs |
|---|---|---|---|
| 1 | [Parsing](phase1_parsing_and_ast/parsing_and_ast.md) | text → AST | |
| 2 | [Resolve](phase2_resolve_and_scope/resolve_and_scope.md) | names → definitions | |
| 3 | [Typecheck](phase3_typecheck_and_dims/typecheck_and_dims.md) | types and dimensions | |
| 4 | [Instantiate](phase4_instantiate/instantiate.md) | classes → instances | |
| 5 | [Flatten](phase5_flatten/flatten.md) | hierarchy → one equation system | |
| 6 | [DAE construction](phase6_dae_construction/dae_construction.md) | equations → the standard DAE form | [index reduction](phase6_dae_construction/index_reduction.md) |
| 7 | [Structural analysis](phase7_structural_analysis/structural_analysis.md) | an unordered system → an evaluation order | [incidence matrix](phase7_structural_analysis/incidence_matrix.md), [matching](phase7_structural_analysis/maximum_bipartite_matching.md), [Tarjan SCC](phase7_structural_analysis/tarjan_scc.md), [BLT](phase7_structural_analysis/blt.md), [tearing](phase7_structural_analysis/tearing.md), [IC plan](phase7_structural_analysis/ic_plan.md) |
| 8 | [Solve lowering](phase8_solve_lowering/solve_lowering.md) | mathematics → a compute graph | |
| 9 | [Simulation](phase9_simulation/simulation.md) | a compute graph → trajectories | |
| 10 | [Codegen templates](phase10_codegen_templates/codegen_templates.md) | IR → emitted source | |

**Phase 7 has six drill-downs and the others have none or one.** That is not neglect — it is
where the interesting algorithms live, and where HRW's animated views point.
[A five-lesson guided lab](phase7_structural_analysis/guided-lab.md) walks it with animated
replays and live-stepped debugging.

## Two things to know before trusting a page here

**These are Claude's notes, written by Claude.** They are not an outside authority, and a page
months old was written by a session with no memory of writing it. That is exactly the
arrangement that can become an echo chamber, so:

**Every claim should carry provenance** — `verified` (checked against code, naming the file),
`cellier` (with a citation), or `inference`. **Untagged prose is a lead, not a fact.** Tags
are upgraded lazily: when a real question sends Claude into the source, the claims it actually
checked get tagged on the way past, so the database becomes trustworthy exactly where it is
used most. See [`../provenance.md`](../provenance.md).

**Numbers about a specimen come from its trace, never from prose here.** The generated
[`../specimen-notebook/`](../specimen-notebook/) entries are correct by construction. This is
not a hypothetical caution: a 1,071-line lab in this directory was deleted on 2026-08-01 for
asserting a 7×7 incidence matrix on a tab that shows 48 equations, uncaught for weeks because
nothing checks prose.

**What is checked**: `src/doc_citations.rs` verifies on every test run that every source path
cited here still exists, that every provenance tag is well formed, and that no document
carries stray control characters. **What is not checked is whether the prose around a citation
is still true** — which is why the provenance tags exist and why untagged text is a lead.

## Further reading

- 👤 [`../architecture.md`](../architecture.md) — how HRW renders each of these phases
- 👤 [`../../specimens/README.md`](../../specimens/README.md) — the small models authored to
  trigger one phenomenon each, which is how most of these pages were checked
- The Rumoca source itself, in `../../../crates/rumoca-*` — the exact tree HRW builds against,
  with numbered SPEC files carrying the architectural invariants
