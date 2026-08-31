# Flatten — what `connect` actually means

<!-- kind: concept -->

[The chain overview](hrw://tour/the-concepts)

`connect(src.p, R.p)` looks like wiring two things together. In Rumoca it is neither an assignment
nor an equation — it is an instruction to **merge sets of variables**, one merge per member the two
sides share, and no equation exists until every merge is done.

Which sets? A `connect` names **connectors, not variables** — `src.p` is a `Pin`. Expansion derives
the members and pairs them by name first
([`expand_connector_connection`](hrw://src/crates/rumoca-phase-flatten/src/connections/mod.rs#expand_connector_connection)),
and each pair goes to `union(a, b)`, which joins **whichever sets currently hold `a` and `b`**.
Nothing else ever puts a variable into the structure, so a pair it has not seen before creates both
and merges them in the same call: **every set starts at two and only grows.** A variable no
`connect` touches is not a set of one — it is **absent**, which is why an unconnected flow variable
needs a pass of its own
([`generate_unconnected_flow_equations`](hrw://src/crates/rumoca-phase-flatten/src/connections/equation_generation.rs#generate_unconnected_flow_equations)).
Order cannot change which variables end up together, and a `connect` whose two ends are already in
one set does nothing at all — `union` compares roots before it merges.

The textbook picture is a graph: variables as vertices, an edge per `connect`, and the answer is
each graph's **connected components**. Rumoca computes those components and **never builds the
graph.** [`connections/mod.rs`](hrw://src/crates/rumoca-phase-flatten/src/connections/mod.rs) uses **[union-find](hrw://src/crates/rumoca-phase-flatten/src/connections/mod.rs#UnionFind)** — one parent index per variable it has touched,
no edges stored anywhere, answering only *"same set?"*. Draw the graph anyway, to predict with;
just hold it the way you are about to hold *nodes* in Stop 1, as your bookkeeping rather than the
compiler's.

[`connect_primitive_vars`](hrw://src/crates/rumoca-phase-flatten/src/connections/mod.rs#connect_primitive_vars) is where a statement becomes merges. It pairs the two connectors'
members **by name**, then routes each pair by that member's prefix:

| the member is | the pair goes to |
|---|---|
| `flow` | `flow_pairs` — a plain `Vec`, not merged yet |
| `stream` | `stream_uf` |
| neither, so potential | `potential_uf` |

A member with **no counterpart on the other side is routed nowhere at all.** Wire a `Pin` `{v, i}`
to a `Flange` `{s, f}` and every pairing fails, so nothing merges and nothing is checked — which is
why `connect` is also a **compatibility claim**, and why the language requires a compiler to test it.

Notice what is not symmetric there. `potential_uf` and `stream_uf` exist, empty, before the first
statement is read; flow is a **list that becomes a union-find per scope, afterwards.** That is a
rule about equations wearing a data structure's clothes: potential merging can be global, because
n − 1 equalities come out the same whether sets are split or merged, while a flow sum must be
scoped or it conserves the wrong thing.

What comes out is a `Vec<`[`ConnectionSet`](hrw://src/crates/rumoca-phase-flatten/src/connections/mod.rs#ConnectionSet)`>`, each carrying `variables`, `kind` and `scope` — so a
connection set is **a set of variables of one kind**, never a set of connectors. `kind` picks the
generator: `Potential` calls [`generate_equality_equations`](hrw://src/crates/rumoca-phase-flatten/src/connections/equation_generation.rs#generate_equality_equations),
`Flow` calls [`generate_flow_equation`](hrw://src/crates/rumoca-phase-flatten/src/connections/equation_generation.rs#generate_flow_equation).
The replay you are about to step through is that pair of acts, once per set — [`SetFormed`](hrw://src/crates/rumoca-phase-flatten/src/connections/trace.rs#SetFormed), then
[`EquationsGenerated`](hrw://src/crates/rumoca-phase-flatten/src/connections/trace.rs#EquationsGenerated).

**This tour counts.** `RcCircuit` has four `connect` statements and twenty-three equations, and every
step from one number to the other is something you can predict before you look.

Each stop asks you to **commit to an answer**, then sends you to the pane that settles it. The
answers are read from generated compiler traces, so if a count disagrees with your screen, the tour
is wrong and I want to know.

Rumoca does check that claim — and the check is **strongest where you need it least, absent where
you need it most.** `RcCircuit` cannot show you why: every `connect` here pairs cleanly, so nothing
below trips it. Ask me, or read [`upstream-issues.md`](hrw://doc/upstream-issues.md).

---

## Stop 1 — How many nodes?

Here is every `connect` in `RcCircuit`:

```modelica
connect(src.p, R.p);
connect(R.n, C.p);
connect(C.n, src.n);
connect(src.n, gnd.p);
```

Two connectors are on the same **node** if you can walk from one to the other along `connect`
statements — so joining `a` to `b` and `b` to `c` puts all three on one node. That is
**transitivity**.

**"Node" is yours, not the compiler's.** The word appears nowhere in Rumoca and nowhere in HRW. It
is bookkeeping *you* do on paper to predict what the compiler will build. What it actually builds
is **connection sets**, and this stop is the gap between the two.

> **Predict.** How many nodes do these four statements make, and how many connectors are on the
> largest one? Then a second number, and expect it to disagree with the first: **how many
> *connection sets* will the replay say it built?**

[Look — RcCircuit → Flatten → Connections](hrw://load/RcCircuit/Flatten/Connections)

**On your paper** — and only there, because the pane has no opinion about nodes and never will:
**three** nodes, of sizes **2, 2 and 3** connectors.

**Expected:** the replay's last frame declaring **6 connection sets** producing **7 equations**.
That is the whole of what the screen can settle for you.

**Falsified if** that last frame says anything other than 6 and 7. Or, on your paper, if you
counted four nodes, made all three the same size, or put one connector on two of them — and the
two halves have to agree, so **the set count must come out twice the node count.**

### What just happened

Four statements, three nodes — because **`src.n` appears twice**, so `connect(C.n, src.n)` and
`connect(src.n, gnd.p)` are one node with three connectors on it.

| node | connectors | size |
|---|---|---|
| A | `src.p`, `R.p` | 2 |
| B | `R.n`, `C.p` | 2 |
| C | `C.n`, `src.n`, `gnd.p` | **3** |

*That table is your paper written out. Nothing in HRW draws it.*

**Nothing downstream ever groups connectors.** Pairing by name means a merge never crosses from one
member to another, so what you drew comes out as one graph per member:

| graph | vertices | edges from `connect(src.p, R.p)` |
|---|---|---|
| **potential** | every `.v` | `src.p.v — R.p.v` |
| **flow** | every `.i` | `src.p.i — R.p.i` |

Each graph yields three **connected components**, of sizes 2, 2 and 3. Modelica calls one component
a **connection set** (MLS §9.2), which is the `ConnectionSet` the opening named.

**So six is three nodes × two kinds.** The replay never counts nodes, because the compiler never
forms them. Step through and they arrive in two runs of three — **the flow sets first, then the
potential sets** — which is why flow equations end up with *lower* indices than potential ones.

**One frame does nothing.** An `unconnected flow` step reports **0 equations added**: MLS §9.2
requires a flow variable in no connection set to get `f = 0`, and `RcCircuit` has none.

**The order is forced, not tidy.** No equation can be written until no node can still grow —
`connect(src.n, gnd.p)` changes what the earlier `connect(C.n, src.n)` is worth.

---

## Stop 2 — How many equations do three nodes make?

Stop 1 left two sets per node — the `.v` and the `.i`. They do **not** produce equations the same
way:

| variable | kind | what it means on a node |
|---|---|---|
| `v` — voltage | **potential** | measured *across*; all of them are **equal** |
| `i` — current | **flow** | measured *through*; they **sum to zero** |

Making *n* voltages equal takes **n − 1** equations. Making *n* currents sum to zero takes exactly
**1**, whatever *n* is. That is Kirchhoff's current law, and the same statement for heat, torque or
mass flow.

*(Modelica has a third kind, `stream`, for fluid connectors. `Pin` has none.)*

**You can watch the asymmetry on one node in two frames**, each set followed immediately by the
equations it generated:

- [frame 7](hrw://stage/Flatten/Connections/frame/7) — the **flow** set of size 3 → **1** equation
- [frame 13](hrw://stage/Flatten/Connections/frame/13) — the **potential** set of size 3 → **2**

<!-- pane-frames: RcCircuit -->

| frame | step | kind | set size | equations |
|---|---|---|---|---|
| `7` | `EquationsGenerated` | `flow` | `3` | `1` |
| `13` | `EquationsGenerated` | `potential` | `3` | `2` |

Same three connectors, different arithmetic. Everything below is that multiplied by three nodes.

> **Predict.** Nodes of 2, 2 and 3 connectors — so potential sets of 2, 2 and 3 `.v`, and flow sets
> of 2, 2 and 3 `.i`. How many equations in total, and how do they split between the two kinds?

[Look — RcCircuit → Flatten → Equations](hrw://load/RcCircuit/Flatten/EquationSheet)

**Expected:** **7** — four potential and three flow. `Component equations` stands alone, and a
`Connector equations` heading covers two sub-groups:

<!-- pane-groups: RcCircuit -->

| group | rows |
|---|---|
| `Component equations` | 16 |
| `Potential equality` | 4 |
| `Flow conservation` | 3 |

**Falsified if** the two connector sub-groups hold four and three of anything other than voltages
and currents respectively, or if their total is not seven.

### What just happened

| node | connectors | potential set | → rows | flow set | → rows |
|---|---|---|---|---|---|
| A | 2 | 2 × `.v` | 1 | 2 × `.i` | 1 |
| B | 2 | 2 × `.v` | 1 | 2 × `.i` | 1 |
| C | 3 | 3 × `.v` | **2** | 3 × `.i` | 1 |
| | | | **4** | | **3** |

**The last two columns behave completely differently.** A flow set of size *n* becomes **one row
naming all *n* variables**, so the sizes are there to be counted:

```
0 = src.p.i + R.p.i
0 = R.n.i + C.p.i
0 = C.n.i + src.n.i + gnd.p.i
```

A potential set of size *n* becomes ***n* − 1 rows, each naming only a pair**, so **its size is
never printed anywhere**. You have to reassemble the sets yourself — that is Stop 3.

**The equations are residuals**: `src.p.v = R.p.v` is stored as `0 = src.p.v - R.p.v`. The readable
form is in each row's **origin** column.

**Why *n* − 1 and not every pair:** redundant equations make a system structurally singular. The
phase produces a **spanning tree** of each potential set, never its complete graph.

---

## Stop 3 — Which rows belong to the same node?

The sheet groups equations by *kind*, not by node. Nodes A, B and C are nowhere on screen — but
they are recoverable, and the two kinds give them up differently.

> **Predict.** Of the four `Potential equality` rows, which **two** belong to the same node —
> and which single `Flow conservation` row is that same node seen whole?

[Look — RcCircuit → Flatten → Equations](hrw://load/RcCircuit/Flatten/EquationSheet)

**Expected:**

```
Connector equations
 └ Potential equality
  0 = src.p.v - R.p.v
  0 = R.n.v - C.p.v
  0 = C.n.v - src.n.v
  0 = src.n.v - gnd.p.v

 └ Flow conservation
  0 = src.p.i + R.p.i
  0 = R.n.i + C.p.i
  0 = C.n.i + src.n.i + gnd.p.i
```

Rows three and four are node C: they **chain** through `src.n.v`. The third flow row is that
same node, and it names all three members at once.

**Falsified if** no two potential rows share a variable, or if the three-term flow row's members are
not the union of the connectors in that chain.

### What just happened

**The flow row is the only place a node appears whole; the potential rows *are* the spanning tree,
drawn one edge at a time.** Stop 2's asymmetry, seen from the other side.

---

## Stop 4 — How big is a four-component circuit?

`RcCircuit` is a voltage source, a resistor, a capacitor and a ground. Seven of its equations come
from the connect graph.

> **Predict.** How many equations does the whole model have — and of the rest, how many belong to
> the resistor alone?

[Look — RcCircuit → Flatten → Equations](hrw://load/RcCircuit/Flatten/EquationSheet)

**Expected:** **23 equations**, of which **16** are `Component equations`, split by origin:

<!-- pane-origins: RcCircuit -->

| origin text | rows |
|---|---|
| `equation from R` | 7 |
| `equation from src` | 4 |
| `equation from C` | 4 |
| `equation from gnd` | 1 |

**Falsified if** the total is not 23, or if the resistor contributes fewer than the source and the
capacitor.

### What just happened

**A resistor is seven equations.** Four of them:

```
0 = R.T_heatPort - R.T
0 = R.R_actual - R.R * (1 + R.alpha * (R.T_heatPort - R.T_ref))
0 = R.v - R.R_actual * R.i
0 = R.LossPower - R.v * R.i
```

Ohm's law here is `v = R_actual·i`, against a resistance computed from a **temperature**. Nobody
asked for thermal modelling; MSL's `Resistor` has it, so the model has it.

Below the equations the pane lists **30 variables**: **1 state, 22 algebraic, 7 parameters**.

> **Predict.** Of those 30 variables, how many will the **Why** column say anything about?

**Expected:** exactly **one** — `C.v`, reading `der in f_x[14]`. Every other row is blank.

**Falsified if:** two or more rows carry a `der in …`, or the one that does names a variable other
than `C.v`.

*What just happened.* **A variable is a state exactly when some equation differentiates it**, so the
Why column is the definition, not decoration. A resistor's law is instantaneous; a capacitor's is a
rate law, so `C.v` is the one quantity carrying the past into the present.

**Flattening is mostly copying.** Sixteen of twenty-three equations are each component's own,
instantiated with a prefix. Only seven are new — and those seven determine the system's structure.

---

## Stop 5 — What if there are no connectors at all?

`TwoLoops` writes its equations directly, with no components and no `connect`.

> **Predict.** How many groups will the equation sheet show?

[Look — TwoLoops → Flatten → Equations](hrw://load/TwoLoops/Flatten/EquationSheet)

**Expected:** one group only — **no** `Connector equations` heading at all:

<!-- pane-groups: TwoLoops -->

| group | rows |
|---|---|
| `Component equations` | 4 |

<!-- pane-origins: TwoLoops -->

| origin text | rows |
|---|---|
| `top-level model equation` | 4 |

**Falsified if** a connector group appears, or if any equation's origin names a component.

### What just happened

With no `connect`, the connection graph is empty and this phase contributes nothing. `TwoLoops`'
equation indices map straight onto its source, which is why `blt-ordering.md` uses it.

---

## What comes next in the chain

The flat model is a pile of equations with no order and no classification. **DAE construction**
partitions it — states, algebraics, and which equations are which kind — and that is
[dae-construction](hrw://tour/dae-construction). After that, `matching.md` asks which equation
solves which unknown.

## What this tour cannot check

- **How the `Connections` replay presents its sets.** The counts come from a trace and are checked;
  whether the frames *read* as a phase building sets one at a time is your report and nothing
  else's. Nothing in HRW represents a node.
- **Whether a connection is legal.** Rumoca checks that *paired* variables agree, but nothing
  checks that both connectors have the **same member set**. That gap has its own tour:
  [the-oracle](hrw://tour/the-oracle).
- **Stream connectors.** Named in Stop 2 and exercised by no specimen here.

Or go back up: [The chain overview](hrw://tour/the-concepts)
