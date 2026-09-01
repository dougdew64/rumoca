# Flatten — what `connect` actually means

<!-- kind: concept -->

[The chain overview](hrw://lab/the-concepts)

`connect(src.p, R.p)` looks like wiring two things together. In Rumoca it is neither an assignment
nor an equation — it is an instruction to **merge sets of variables**, one merge per member the two
sides share, and no equation exists until every merge is done.

Which sets? A `connect` names **connectors, not variables** — `src.p` is a `Pin`. Expansion derives
the members and pairs them by name first
([`expand_connector_connection`](hrw://src/crates/rumoca-phase-flatten/src/connections/mod.rs#expand_connector_connection)),
and each pair goes to
[`union(a, b)`](hrw://src/crates/rumoca-phase-flatten/src/connections/mod.rs#UnionFind), which
joins **whichever sets currently hold `a` and `b`**.
Nothing else ever puts a variable into the structure, so a pair it has not seen before creates both
and merges them in the same call: **every set starts at two and only grows.** A variable no
`connect` touches is not a set of one — it is **absent**, which is why an unconnected flow variable
needs a pass of its own
([`generate_unconnected_flow_equations`](hrw://src/crates/rumoca-phase-flatten/src/connections/equation_generation.rs#generate_unconnected_flow_equations)).
Order cannot change which variables end up together, and a `connect` whose two ends are already in
one set does nothing at all — `union` compares roots before it merges.

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

**This lab counts.** `RcCircuit` has four `connect` statements and twenty-three equations, and every
step from one number to the other is something you can predict before you look.

Each stop asks you to **commit to an answer**, then sends you to the pane that settles it. The
answers are read from generated compiler traces, so if a count disagrees with your screen, the lab
is wrong and I want to know.

Rumoca does check that claim — and the check is **strongest where you need it least, absent where
you need it most.** `RcCircuit` cannot show you why: every `connect` here pairs cleanly, so nothing
below trips it. Ask me, or read [`upstream-issues.md`](hrw://doc/upstream-issues.md).

---

## Station 1 — How many connection sets?

Here is every `connect` in `RcCircuit`:

```modelica
connect(src.p, R.p);
connect(R.n, C.p);
connect(C.n, src.n);
connect(src.n, gnd.p);
```

A `Pin` has two members, so each statement pairs by name into **two merges** — one joining `.v` to
`.v`, one joining `.i` to `.i`. Four statements, eight merges.

Eight merges do not mean eight sets. Watch `src.n`: it is named by `connect(C.n, src.n)` **and** by
`connect(src.n, gnd.p)`, so by the time the second one runs, `src.n.v` is already in a set — and
`union` joins that whole set to the one holding `gnd.p.v`, rather than making a new one.

> **Predict.** How many connection sets come out of those eight merges, and how many equations do
> they produce?

[Look — RcCircuit → Flatten → Connections](hrw://load/RcCircuit/Flatten/Connections)

**Expected:** the replay's last frame declaring **6 connection sets** producing **7 equations**.

**Falsified if** that last frame says anything other than 6 and 7.

### What just happened

**Eight merges, six sets**, because two of them landed in sets that already existed. `src.n` is
named twice, so `C.n.v`, `src.n.v` and `gnd.p.v` finish in **one set of three** — and their `.i`
counterparts in another. The other two statements make sets of two.

So: three sets over the `.v` members, of sizes 2, 2 and 3, and three over the `.i` with exactly the
same membership. Six.

**Pairing by name is why the two kinds never mix.** No merge joins a `.v` to an `.i`, because no
statement ever pairs them — which is why the sets come out **matched**, one `.v` set for each `.i`
set. Station 6 is where that stops being true.

**They arrive in two runs of three** — the flow sets first, then the potential ones — which is why
flow equations end up with *lower* indices than potential ones.

**One frame does nothing.** An `unconnected flow` step reports **0 equations added**: MLS §9.2
gives a flow variable in no connection set `f = 0`, and every one of `RcCircuit`'s is in a set.

**The order is forced, not tidy.** No equation can be written until no set can still grow —
`connect(src.n, gnd.p)` changes what the earlier `connect(C.n, src.n)` is worth.

---

## Station 2 — How many equations does a set make?

Station 1 left six sets — three of `.v`, three of `.i`. They do **not** produce equations the same
way:

| variable | kind | what it means across a set |
|---|---|---|
| `v` — voltage | **potential** | measured *across*; all of them are **equal** |
| `i` — current | **flow** | measured *through*; they **sum to zero** |

Making *n* voltages equal takes **n − 1** equations. Making *n* currents sum to zero takes exactly
**1**, whatever *n* is. That is Kirchhoff's current law, and the same statement for heat, torque or
mass flow.

*(Modelica has a third kind, `stream`, for fluid connectors. `Pin` has none.)*

**You can watch the asymmetry on the two size-3 sets in two frames**, each set followed immediately
by the equations it generated:

- [frame 7](hrw://stage/Flatten/Connections/frame/7) — the **flow** set of size 3 → **1** equation
- [frame 13](hrw://stage/Flatten/Connections/frame/13) — the **potential** set of size 3 → **2**

<!-- pane-frames: RcCircuit -->

| frame | step | kind | set size | equations |
|---|---|---|---|---|
| `7` | `EquationsGenerated` | `flow` | `3` | `1` |
| `13` | `EquationsGenerated` | `potential` | `3` | `2` |

Same three members, different arithmetic. Everything below is that, once per set.

> **Predict.** Potential sets of 2, 2 and 3 `.v`, and flow sets of 2, 2 and 3 `.i`. How many
> equations in total, and how do they split between the two kinds?

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

| set size | potential → rows | flow → rows |
|---|---|---|
| 2 | 1 | 1 |
| 2 | 1 | 1 |
| 3 | **2** | 1 |
| | **4** | **3** |

**The last two columns behave completely differently.** A flow set of size *n* becomes **one row
naming all *n* variables**, so the sizes are there to be counted:

```
0 = src.p.i + R.p.i
0 = R.n.i + C.p.i
0 = C.n.i + src.n.i + gnd.p.i
```

A potential set of size *n* becomes ***n* − 1 rows, each naming only a pair**, so **its size is
never printed anywhere**. You have to reassemble the sets yourself — that is Station 3.

**The equations are residuals**: `src.p.v = R.p.v` is stored as `0 = src.p.v - R.p.v`. The readable
form is in each row's **origin** column.

**Why *n* − 1 and not every pair:** every pair would be redundant, and redundant equations make a
system structurally singular. *n* − 1 is the fewest that still forces all *n* equal.

---

## Station 3 — Which rows belong to the same set?

The sheet groups equations by *kind*, not by set. The six sets are nowhere on screen — but they are
recoverable, and the two kinds give them up differently.

> **Predict.** Of the four `Potential equality` rows, which **two** came from the same set — and
> which single `Flow conservation` row is that same set's `.i` twin, seen whole?

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

Rows three and four are the set of three: they **chain** through `src.n.v`. The third flow row is
that set's `.i` twin, and it names all three members at once.

**Falsified if** no two potential rows share a variable, or if the three-term flow row's members are
not the `.i` counterparts of the variables in that chain.

### What just happened

**The flow row is the only place a set appears whole; the potential rows *are* the spanning tree,
drawn one edge at a time.** Station 2's asymmetry, seen from the other side.

---

## Station 4 — How big is a four-component circuit?

`RcCircuit` is a voltage source, a resistor, a capacitor and a ground. Seven of its equations came
from the six connection sets.

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

## Station 5 — What if there are no connectors at all?

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

With no `connect`, nothing merges, no set exists and this phase contributes nothing. `TwoLoops`'
equation indices map straight onto its source, which is why `blt-ordering.md` uses it.

---

## Station 6 — Do the sets still come out matched?

Station 1's sets came out **matched**: three of `.v` and three of `.i`, same membership, because
pairing by name never lets a merge cross between members. That pairing is real. **What is not a law
is the matching**, and `RcCircuit` cannot show you why, because all four of its connects sit at
root scope.

`ScopedConnect` puts a resistor behind two pins inside a `Segment`, wires it **there**, and wires
the segments to each other and to a source at the **top level**:

```modelica
model Segment
  Pin p; Pin n; Resistor R(R = 50);
equation
  connect(p, R.p);        // declared at scope `seg1` / `seg2`
  connect(R.n, n);
end Segment;
// ...and at root: src.p—seg1.p, seg1.n—seg2.p, seg2.n—src.n, src.n—gnd.p
```

Three junctions, the same as `RcCircuit`. Eight `connect` statements instead of four.

> **Predict.** How many **potential** sets and how many **flow** sets? The rule from Station 1 says
> the two counts are equal. Commit to that, or to a reason it breaks.

[Look — ScopedConnect → Flatten → Equations](hrw://load/ScopedConnect/Flatten/EquationSheet)

**Expected:** they are **not** equal — **8** potential equality rows and **7** flow conservation
rows:

<!-- pane-groups: ScopedConnect -->

| group | rows |
|---|---|
| `Component equations` | 19 |
| `Potential equality` | 8 |
| `Flow conservation` | 7 |

**Falsified if** the two connector groups hold the same number of rows, or if the potential rows
are not 8 — which is *n* − 1 over sets of 3, 4 and 4.

### What just happened

**Three junctions produced three potential sets and seven flow sets.** The rule broke, and the
introduction said why before you got here: potential merges in **one global** union-find, flow in
**one per scope**.

Follow a single junction — `src.p`, `seg1.p`, `seg1.R.p` — through both:

**As potential — one set of three, spanning both scopes:**

```text
seg1.p.v = seg1.R.p.v          (wired inside Segment)
seg1.R.p.v = src.p.v           (wired at root)
```

**As flow — two sums, one per scope:**

```text
-seg1.p.i + seg1.R.p.i = 0     (scope seg1)
src.p.i + seg1.p.i = 0         (root)
```

**So `seg1.p.i` is in two connection sets at once, and that is correct.** The inner sum is the
segment's own current balance; the outer is the balance where the segment meets the circuit. A
single sum across both would say that current entering `seg1` through its pin vanishes — which is
the thing Kirchhoff's law exists to forbid. A single potential *equality* across both is exactly
right, because a junction is at one voltage no matter which file wired it.

**And that is the intro's asymmetry made visible:** merging potential across scopes changes
nothing, since *n* − 1 equalities come out the same whether the sets are split or merged. Merging
*flow* across scopes changes the physics.

---

## What comes next in the chain

The flat model is a pile of equations with no order and no classification. **DAE construction**
partitions it — states, algebraics, and which equations are which kind — and that is
[dae-construction](hrw://lab/dae-construction). After that, `matching.md` asks which equation
solves which unknown.

## What this lab cannot check

- **How the `Connections` replay presents its sets.** The counts come from a trace and are checked;
  whether the frames *read* as a phase building sets one at a time is your report and nothing
  else's.
- **Whether a connection is legal.** Rumoca checks that *paired* variables agree, but nothing
  checks that both connectors have the **same member set**. That gap has its own lab:
  [the-oracle](hrw://lab/the-oracle).
- **Stream connectors.** Named in Station 2 and exercised by no specimen here.

Or go back up: [The chain overview](hrw://lab/the-concepts)
