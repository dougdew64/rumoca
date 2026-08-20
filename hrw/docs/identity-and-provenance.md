# Identity and provenance — the no-heuristic-name-matching rule

**Purpose:** the bar any code must meet before it decides that two things in different
compiler phases are the same thing, and the inventory of what Rumoca already preserves to
make that possible.
**Status:** authority. Binds every tracking, highlighting and follow feature.
**Read when:** you are about to write code that asks *"is this the tracked variable?"* or
*"does this equation mention it?"* — or before adding a view that highlights anything.

Extracted 2026-08-01 from `source-tooling-plan.md`, where it was stated as that plan's
standing principle. It is broken out because it **outlives the plan**: six source files cite
it as the reason they do not take an easier route, and a rule that governs code should not
live inside a document whose phases are finishing.

Carried into that plan from `cross-stage-tracking-plan.md` (retired 2026-07-28), where it was
stated at the outset of the #10 work:

> **Accuracy and correctness required. No heuristic name-matching.** All mappings use
> Rumoca's typed provenance.

## The rule

**No substring search ever decides identity.**

It was violated for a while and is now honoured. `matches_tracked` — a whole-word substring
search — decided highlighting in every stage view until 2026-07-28. **Why it happened is the
part worth knowing**, because the same pressure will recur: the structural report Rumoca
emits carries names only (`"unknown": "src.n.i"`, no `def_id`), so the views genuinely had
nothing but strings to work with.

The resolution was not to abandon the principle but to separate two questions that one
function had been answering at once:

| Question | Means | Implementation |
|---|---|---|
| **Identity** — *is this the tracked variable?* | exact equality modulo one `der(…)` | `identifier_index::same_variable` |
| **Membership** — *does this equation mention it?* | structural where the data exists | `tarjan_anim::equation_mentions`, reading the incidence matrix's `rows[eq]` |
| **Membership**, text only | lexical, never substring | `source_view::mentions_identifier` — knows `height` is one token, and that string literals and comments are not code |

**Flat names are canonical. Being strings does not make them search terms.**

**Any future tracking work must meet this bar.** A new view deciding whether something *is*
the tracked variable uses identity; deciding whether something *refers* to it uses structure,
or the lexer — never a substring search.

### THE UNSTATED PRECONDITION: both sides must be the same MODEL

*(added 2026-08-20, after a defect that complied with every word above)*

**Exact equality decides identity only within one model.** Everything on this page was written
when the only names in play came from the specimen's own compilation, so *"the same string"*
and *"the same thing"* could not come apart. **"Go to definition" ended that**: it puts a
library class — `Modelica.Electrical.Analog.Basic.Resistor`, pulled from the resolved tree —
on screen beside indexes built from the specimen. A `Resistor` has an `R`; so does the
specimen. Both are exact matches, and they are different variables.

**So the rule gains a precondition rather than an exception:**

> Before comparing two names, establish that they are drawn from **one namespace**. Exact
> equality across two models is a collision wearing identity's clothes, and it is invisible
> here because the comparison itself is correct.

**Why this hid for a day and could have hidden for months.** `App::specimen_tree_options`
handed the *navigated* tree the specimen's `known_variables`, `variable_lines`,
`declaring_classes`, `path_lines` and `tracked` — five name-keyed indexes, all using exact
equality, **all compliant as written.** The failure is *presence substituted, not absence
filled*: a row of the Resistor offered *"declared at line 41"*, naming a line of the
specimen's file, with nothing on screen admitting where the number came from.

**The fix, and it is the shape to copy:** `nav_view_ui` stopped *taking* a `TreeOptions`
(2026-08-20) — the five plus `jump_to` and `highlight` are the whole struct, so removing the
parameter makes the cross-namespace comparison unrepresentable rather than merely absent.
Doug's ruling: **the navigated tree is annotated from the class, or not at all.**

**And the confirming detail belongs to this document rather than to that pane.** `def_index`
is per-`NavEntry` — the class's *own* DefId table, resolved structurally by the worker — and
it was **not** one of the five, so go-to-definition keeps working *through DefIds* while every
name-matched shortcut goes. **The structural route this page prescribes was the one that
survived**, which is the rule working rather than a coincidence.

**Blanking is the correct answer now, not the destination.** Nothing indexes an MSL class's
own variables, declaring positions or source lines, so there is no class-derived version to
substitute. If a navigated tree should ever be annotated, build the indexes **from the class**
— never re-derive them from the specimen.

## What provenance Rumoca already preserves

Relevant whenever a feature needs to know what identity information is available to it:

- Every flat/DAE **variable** carries `component_ref` (with `def_id`) and `source_span`
  pointing back to its declaration.
- Every flat/DAE **equation** carries `span` and a typed `EquationOrigin`.
- The flat model carries `symbol_ancestry` (`DefId → Arc<[DefId]>`).
- All `VarRef` expressions in equations carry structured `ComponentReference` objects, via the
  `structured_refs.rs` postprocess pass.

**No invasive Rumoca changes are needed to use any of it** — the provenance is already there;
HRW has to extract and index it.

## The gap, and the upstreamable fix

**The structural report does not surface `def_id`**, which is the entire reason views fall
back to names and the reason the violation above was tempting. Adding it would be additive,
observation-only instrumentation of exactly the kind this fork exists for — see
[`../CLAUDE.md`](../CLAUDE.md) on instrumentation discipline, and
[`upstream-strategy.md`](upstream-strategy.md) on flagging upstreamable work at planning time
rather than after it is built in a shape that cannot be handed over.

## Cited by

These files name this rule as the reason they take the harder route. **If this document
moves, update them** — the citations are the point:

- `src/identifier_index.rs`
- `src/modelica_lex.rs`
- `src/tarjan_anim.rs`
- `src/worker.rs`
