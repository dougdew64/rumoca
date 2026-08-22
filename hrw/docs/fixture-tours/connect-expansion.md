# Flatten — what `connect` actually means

<!-- kind: concept -->

[▲ The chain overview](hrw://tour/the-concepts)

`connect(src.p, R.p)` looks like wiring two things together. In the equations it is **neither an
assignment nor an equality** — it is **an edge in each of several graphs, one per member of the
connector** — and the equations do not exist until each graph's **connected components** have been
computed.

**This tour counts.** `RcCircuit` has four `connect` statements and twenty-three equations, and every
step from one number to the other is something you can predict before you look.

Each stop asks you to **commit to an answer**, then sends you to the pane that settles it. The
answers are read from generated compiler traces, so if a count disagrees with your screen, the tour
is wrong and I want to know.

---

## Stop 1 — How many nodes?

Here is every `connect` in `RcCircuit`:

```modelica
connect(src.p, R.p);
connect(R.n, C.p);
connect(C.n, src.n);
connect(src.n, gnd.p);
```

**A connector is not a value — it is a bundle of variables.** `src.p` is a `Pin`, and a `Pin` holds
a voltage `v` and a current `i`. So `connect(src.p, R.p)` does not relate `src.p` to `R.p`; it
relates their **members, pairwise**:

```text
connect(src.p, R.p)   →   src.p.v — R.p.v      (the voltages)
                          src.p.i — R.p.i      (the currents)
```

Two connectors form the same **node** if you can walk from one to the other along `connect`
statements — so joining `a` to `b` and `b` to `c` puts all three on one node, even though no
`connect` statement names `a` and `c` together. That is **transitivity**, and it is the only
property you need here.

> **Predict.** How many nodes do these four statements make, and how many connectors are on the
> largest one? Then a second number, and expect it to disagree with the first: **how many
> *connection sets* will the replay say it built?**

[▶ Look — RcCircuit → Flatten → Connections](hrw://load/RcCircuit/Flatten/Connections)

**Expected:** **three** nodes, of sizes **2, 2 and 3** connectors — and the replay's last frame
declaring **6 connection sets** producing **7 equations**.

**Falsified if** you count four nodes, or if all three are the same size, or if any connector
appears on two nodes — or if the set count is anything but **twice** the node count.

**Three and six are both right**, and the gap between them is the point of the rest of this stop.

### What just happened

Four statements, three nodes — because **`src.n` appears twice**. `connect(C.n, src.n)` and
`connect(src.n, gnd.p)` are not two nodes; they are one node with three connectors on it.

| node | connectors | size |
|---|---|---|
| A | `src.p`, `R.p` | 2 |
| B | `R.n`, `C.p` | 2 |
| C | `C.n`, `src.n`, `gnd.p` | **3** |

### And this is where the counting has to get precise

**Nothing downstream ever groups connectors.** Because each `connect` expands per member, what
actually gets grouped are **variables**, in **two separate graphs that share no vertices**:

| graph | vertices | edges from `connect(src.p, R.p)` |
|---|---|---|
| **potential** | every `.v` | `src.p.v — R.p.v` |
| **flow** | every `.i` | `src.p.i — R.p.i` |

Each graph is asked the same question — *which vertices are reachable from which*, the **connected
components** — and each yields three components of sizes 2, 2 and 3, mirroring the nodes above
because the same statements built both. Modelica calls one such component a **connection set**
(MLS §9.2), and a connection set is a set of **variables of one kind**, never a set of connectors.
Rumoca computes them with **union-find**, in
`crates/rumoca-phase-flatten/src/connections/mod.rs`, and keeps the two kinds apart throughout.

**So "a node of size 3" means three connectors, hence a potential set of three `.v` and a flow set
of three `.i`** — six variables, in two sets that never mix.

**That is where six comes from: three nodes × two kinds.** The replay never counts nodes, because
the compiler never forms them; it counts the sets it actually built. Step through and you can watch
them arrive in two runs of three — **the flow sets first, then the potential sets.** That ordering
is not cosmetic: it is why the flow equations end up with *lower* indices than the potential ones,
which you will meet again in Stop 3.

**One frame does nothing, and it is worth knowing why.** Near the end, an `unconnected flow` step
reports **0 equations added**. MLS §9.2 requires a flow variable in no connection set to be given
`f = 0`; `RcCircuit` has none, so the step fires and adds nothing. That is also why the equation
sheet has no `Unconnected flow` group later — the category exists, and this model is empty of it.

**The order is forced, not tidy.** The number of equations depends on how *big* each node is, so
no equation can be written until no node can still grow. `connect(src.n, gnd.p)` changes what
the earlier `connect(C.n, src.n)` is worth.

---

## Stop 2 — How many equations do three nodes make?

Stop 1 left you with two sets per node — the `.v` and the `.i`. They do **not** produce equations the
same way, and `Pin`'s two members are why:

| variable | kind | what it means on a node |
|---|---|---|
| `v` — voltage | **potential** | measured *across*; all of them are **equal** |
| `i` — current | **flow** | measured *through*; they **sum to zero** |

*(Modelica has a third kind, `stream`, for fluid connectors carrying enthalpy. `Pin` has none, and
neither does any specimen in this tour.)*

That asymmetry decides everything. Making *n* voltages equal takes **n − 1** equations — a chain is
enough, and more would be redundant. Making *n* currents sum to zero takes exactly **1**, whatever
*n* is. That is Kirchhoff's current law, and the same statement for heat, torque or mass flow.

**You can watch the asymmetry happen on one model in two frames.** Back in the replay, the
three-connector node produces its two kinds of set at different times, and each set is followed
immediately by the equations it generated:

- [▶ frame 7](hrw://stage/Flatten/Connections/frame/7) — the **flow** set of size 3 → **1** equation
- [▶ frame 13](hrw://stage/Flatten/Connections/frame/13) — the **potential** set of size 3 → **2**

<!-- pane-frames: RcCircuit -->

| frame | step | kind | set size | equations |
|---|---|---|---|---|
| `7` | `EquationsGenerated` | `flow` | `3` | `1` |
| `13` | `EquationsGenerated` | `potential` | `3` | `2` |

Same three connectors, same instant in the pass, different arithmetic. Everything below is that one
observation multiplied by three nodes.

> **Predict.** Nodes of 2, 2 and 3 connectors — so potential sets of 2, 2 and 3 `.v`, and flow sets
> of 2, 2 and 3 `.i`. How many equations in total, and how do they split between the two kinds?

[▶ Look — RcCircuit → Flatten → Equations](hrw://load/RcCircuit/Flatten/EquationSheet)

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

**Read the last two columns against the screen, because they behave completely differently.** A flow
set of size *n* becomes **one row naming all *n* variables**, so the sizes 2, 2 and 3 are literally
there to be counted:

```
0 = src.p.i + R.p.i
0 = R.n.i + C.p.i
0 = C.n.i + src.n.i + gnd.p.i
```

A potential set of size *n* becomes ***n* − 1 rows, each naming only a pair**, so a set of three
arrives as two rows and **its size is never printed anywhere**. If you go looking for "2, 2, 3"
among the potential rows you will not find it — you will find four pairs, and you have to
reassemble the sets yourself. That is Stop 3.

Both sub-groups sit under **`Connector equations`** because both exist for the same reason: two
connectors were joined. A flow sum is every bit as connection-derived as a potential equality.

**The equations are residuals.** Rumoca stores every continuous equation as an expression that must
equal zero, so `src.p.v = R.p.v` is kept as:

```
0 = src.p.v - R.p.v
```

The readable form has not been lost — each row's **origin** carries it, reading
`connection equation: src.p.v = R.p.v`. Two renderings of one equation, in two columns of the same
row.

**Why *n* − 1 and not every pair.** Writing all pairwise equalities would be redundant, and
redundant equations make a system structurally singular — the rank deficiency `matching.md` Stop 3
diagnoses. The phase produces a **spanning tree** of each potential set, never its complete graph.

---

## Stop 3 — Which rows belong to the same node?

The sheet groups equations by *kind*, not by node. Nodes A, B and C are nowhere on the
screen — but they are still recoverable, and the two kinds give them up differently.

> **Predict.** Of the four `Potential equality` rows, which **two** belong to the same node —
> and which single `Flow conservation` row is that same node seen whole?

[▶ Look — RcCircuit → Flatten → Equations](hrw://load/RcCircuit/Flatten/EquationSheet)

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

**A potential set of *n* arrives as *n* − 1 rows naming pairs; a flow set of *n* arrives as one row
naming all *n*.** So the flow row is the only place a node appears whole, and the potential rows
*are* the spanning tree — drawn one edge at a time.

That is the same asymmetry as Stop 2's arithmetic, seen from the other side.

---

## Stop 4 — How big is a four-component circuit?

`RcCircuit` is a voltage source, a resistor, a capacitor and a ground. Seven of its equations come
from the connect graph.

> **Predict.** How many equations does the whole model have — and of the rest, how many belong to
> the resistor alone?

[▶ Look — RcCircuit → Flatten → Equations](hrw://load/RcCircuit/Flatten/EquationSheet)

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

**A resistor is seven equations.** Look at four of them:

```
0 = R.T_heatPort - R.T
0 = R.R_actual - R.R * (1 + R.alpha * (R.T_heatPort - R.T_ref))
0 = R.v - R.R_actual * R.i
0 = R.LossPower - R.v * R.i
```

Ohm's law here is not `v = R·i`. It is `v = R_actual·i`, against a resistance computed from a
**temperature**, with a heat port and dissipated power alongside. Nobody asked for thermal
modelling; MSL's `Resistor` has it, so the model has it.

**Below the equations the pane lists every variable** — 30 of them, with kind, **why**, start value
and unit: **1 state, 22 algebraic, 7 parameters**. One interesting quantity, twenty-nine bookkeeping
ones.

> **Predict.** Of those 30 variables, how many will the **Why** column say anything about?

**Expected:** exactly **one** — `C.v`, reading `der in f_x[14]`. Every other row is blank.

**Falsified if:** two or more rows carry a `der in …`, or the one that does names a variable other
than `C.v`.

*What just happened.* The Why column is not decoration; it is the definition. **A variable is a
state exactly when some equation differentiates it**, so the column shows the equation that did it
and the blanks are the rest of the model saying *nothing differentiates me*. Hover the cell for the
equation itself.

That single row is why this circuit is interesting at all. A resistor's law is instantaneous —
`v = R·i`, no memory. A capacitor's is a rate law — `i = C·dv/dt` — so `C.v` is the one quantity
that carries the past into the present. **Energy storage is what puts a derivative in an equation**,
and the state count is the number of independent things the system has to remember. Count the
energy-storing components and you have usually counted the states before the compiler has.

**Flattening is mostly copying.** Sixteen of twenty-three equations are each component's own,
instantiated with a prefix. Only seven are new. But those seven are the ones that could not have
been written by hand at model scale, and they are what determines the system's structure.

---

## Stop 5 — What if there are no connectors at all?

`TwoLoops` writes its equations directly, with no components and no `connect`.

> **Predict.** How many groups will the equation sheet show?

[▶ Look — TwoLoops → Flatten → Equations](hrw://load/TwoLoops/Flatten/EquationSheet)

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

With no `connect`, the connection graph is empty, and a phase that spends most of its effort on
connection sets contributes nothing. `TwoLoops`' equation indices map straight onto what its source
says, with no expansion in between — which is exactly why `blt-ordering.md` uses it.

---

## What comes next in the chain

The flat model is a pile of equations with no order and no classification. **DAE construction**
partitions it — which variables are states, which are algebraic, which equations are which kind —
and that is [▶ dae-construction](hrw://tour/dae-construction). After that, `matching.md` asks which equation
solves which unknown.

## What this tour cannot check

- **That the `Connections` replay in Stop 1 shows nodes the way the prose implies.** The counts
  come from a trace; how the replay presents them is unverified.
- **Whether a connection is legal.** Rumoca checks that *paired* variables agree — flow with flow,
  `Real` with `Real`, matching array shapes — but nothing checks that both connectors have the
  **same member set**, so joining a `{v, i}` connector to a `{v}` connector is accepted. That gap
  has its own tour: [▶ the-oracle](hrw://tour/the-oracle).
- **Stream connectors.** Named in Stop 2 and exercised by no specimen here.

Or go back up: [▲ The chain overview](hrw://tour/the-concepts)
