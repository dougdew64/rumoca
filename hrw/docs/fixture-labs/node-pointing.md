# Fixture lab — pointing at a tree node, and following

<!-- kind: feature -->

**This is a test, not an explanation.** It verifies the last two verbs of the answer
channel: pointing at a node inside a stage tree, and setting the follow.

Every path below was read from `docs/specimen-notebook/RcCircuit/trace/structural.json`,
not invented — a fixture lab with a made-up path is a broken test that looks fine.

Pick it from the Labs list.
Notices appear in the status bar, along the bottom of the HRW window. Several stops below expect one; that is where to look.


---

## Station 1 — Open the tree

[RcCircuit → Structural → Tree](hrw://load/RcCircuit/Structural/Tree)

**Expected:** the Structural IR tree with every header collapsed — `blocks`,
`incidence` and `matching` all show a twisty and no contents. Nothing opens by default:
headers open only for a followed identifier, for Reveal identifiers, or for a jump
target, and none of those is active yet.

That is what the next stops have to get through.

## Station 2 — Point at a shallow node

[Point at `coupled_block_count`](hrw://stage/Structural/Tree/node/coupled_block_count)

**Expected:** the tree scrolls to `coupled_block_count`, centred, and its row carries a
cyan wash — distinct from the gold of a *followed* identifier, because a jump target
is one row for one link rather than a thread through every stage. RcCircuit has no
coupled blocks, so the value reads `0`.

The wash stays until you click a row or load something else. It answers "which row did
that link mean?", and that question is open until you move on.

## Station 3 — Point at something nested

[Point at `incidence.rows[0].equation_text`](hrw://stage/Structural/Tree/node/incidence.rows[0].equation_text)

**Expected:** `incidence`, `rows` and `rows[0]` all expand, and the view scrolls to
`equation_text`, whose value is `0 - (src.p.i + src.n.i)` — the first equation's text.

This is the discriminating stop. Getting there by hand is four clicks and a scroll; if it
lands without them, the verb works.

## Station 4 — Point somewhere deeper still

[Point at `blocks[3].unknown`](hrw://stage/Structural/Tree/node/blocks[3].unknown)

**Expected:** the fourth BLT block expands and its `unknown` field is in view.

## Station 5 — A path that is not there

[Point at `error.unmatched_unknowns[0]`](hrw://stage/Structural/Tree/node/error.unmatched_unknowns[0])

That path is real — but it belongs to `CapacitorLoop`, which *fails* structurally.
RcCircuit succeeds, so it has no `error` at all.

**Expected:** nothing moves, and a notice in the status bar says there is no node at that path. It must
not expand partway and stop, which would read as "it opened something" rather than
"that path is wrong."

## Station 5b — A view this model does not have

[Structural → Summary](hrw://stage/Structural/Summary)

The Summary tab exists only when a model is *singular*. `RcCircuit` compiles cleanly,
so it has no Summary — look at the sub-tab row and you will not find one.

**Expected:** nothing changes, and a notice in the status bar says this stage has no Summary view for this
model. It must not select the tab silently and render the singular-system summary for
a model that is not singular.

*(That is precisely what happened before 2026-07-30: the cross-platform lab linked here
and HRW obliged. The slug is valid for the stage — availability depends on what the
compile produced, which only the panel knows.)*

## Station 6 — Follow an identifier

[Follow `C.v`](hrw://follow/C.v)

**Expected:** `C.v` — the capacitor voltage, the model's one state — becomes the followed
identifier. The Context Bar shows it, and the tree marks where it appears.

The stage must not change. Following and pointing are independent primitives; a stop
may set either without disturbing the other.

## Station 7 — Follow, then point, and see both

[Point at `matching[0].unknown`](hrw://stage/Structural/Tree/node/matching[0].unknown)

**Expected:** the tree jumps to the first matched unknown while `C.v` stays followed —
the Context Bar shows a point *and* a thread together, which is the composition the
capture was designed around.

---

## What this cannot check

Whether the highlight is legible, whether the scroll lands somewhere comfortable to read,
and whether an expanded tree is navigable afterwards or leaves you lost. All of that
needs eyes.
