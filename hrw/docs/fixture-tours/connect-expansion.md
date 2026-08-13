# Flatten — what `connect` actually means

`connect(src.p, R.p)` looks like wiring two things together, and in a drawing that is all it is.
**In the equations it is neither an assignment nor an equality.**

**It is one edge in a graph.** Define that graph, because everything below is arithmetic over it:

> **The connection graph.** Its **vertices** are connector variables — `src.p.v`, `R.p.v`,
> `src.n.i`, one per member of every connector instance. Its **edges** are the `connect` statements
> themselves, one edge each, and they are **undirected**: `connect(a, b)` says *a and b are the same
> node*, which is symmetric.
>
> The only question asked of it is **which connectors belong together**. Nothing else about the
> graph is used.

**"Computing the components" means sorting the connectors into groups, and the groups are the
output.** The procedure is as plain as it sounds: pick any connector, walk along every `connect` you
can reach from it, and everything you reach is one group; repeat with whatever is left over until
nothing is. **Act 1's table of sets A, B and C is that output** — three groups, one per node in the
circuit.

**Two names for one thing, and you will meet both.** Graph theory calls a group like that a
**connected component**; Modelica calls it a **connection set** (MLS §9.2). Same object.

**The grouping happens first, and only then does any equation exist** — and that ordering is forced
rather than tidy. A set of *n* connectors yields *n − 1* equations plus one flow equation, so the
*count* depends on group **size**. Emitting an equation early would be a guess:
`connect(src.n, gnd.p)` retroactively changes how many equations the earlier `connect(C.n, src.n)`
is responsible for. You cannot know until no group can still grow.

Rumoca does the grouping with **union-find** (`crates/rumoca-phase-flatten/src/connections/mod.rs`),
the standard tool for exactly this question — it is the walking procedure above, made fast.

*(Lead rewritten 2026-08-12, twice, both times because Doug asked. First version said "the graph is
solved", naming neither the graph nor the operation. Second version named the connection graph
without defining it — the definition lived only in the answer, which is the failure this paragraph
now exists to prevent. **The three-graph contrast that came out of the same question is in
[`the-mathematics.md`](the-mathematics.md)**, since it spans four tours and belongs to none of
them.)*

---

## What a connector holds, and where its variables come from

**A connector is not a wire. It is a small bundle of variables**, and `RcCircuit`'s connectors are
all `Pin`, which holds exactly two: a voltage `v` and a current `i`.

**Those variables are discovered one phase earlier than you might expect.** Instantiate expands the
class hierarchy, so by the time flattening starts it already knows that `src` has a connector `p`,
and that `p` contains `v` and `i` — 53 component instances for this four-component circuit. Flatten's
job is to give each one a single flat name: `src.p.v`. So **Instantiate discovers the connector
variables; Flatten names them.** The graph cannot be built before Instantiate, because `connect`
names *connectors* and the graph's vertices are their *members*.

**This is the bridge to the arithmetic below.** The graph is drawn on **connectors**, but equations
come out **per member** — so four `connect` statements over `Pin`s produce equations about `v` *and*
equations about `i`, and they are different kinds of equation.

### Three kinds of connector variable

Modelica gives each member of a connector one of three roles, and **the role decides what happens
when connectors are joined**:

| kind | written | the physical question | at a junction | example |
|---|---|---|---|---|
| **potential** | *(no prefix)* | measured **across** two points | all of them are **equal** | voltage, temperature, angle |
| **flow** | `flow` | measured **through** a point | they **sum to zero** | current, heat flow, torque |
| **stream** | `stream` | **carried along** by a flow | depends on flow *direction* | specific enthalpy in a fluid |

**Potential and flow are the pair that makes a junction work.** Equal potentials say *"this is one
node"*; flows summing to zero says *"nothing accumulates here"* — Kirchhoff's current law, and the
same statement for heat, torque or mass. **That asymmetry is the whole reason a set of *n* connectors
gives *n − 1* equations and 1, rather than *n* and *n*** — the arithmetic in Act 2.

**Stream exists for fluids** and does not appear in this tour: a mixing junction cannot simply
average enthalpy, because what arrives depends on which way each stream is flowing. `Pin` has no
stream variable, and neither does any specimen here. It is named so the set of three is complete.

**Expected:** in the equation sheet, every `connection equation` is about a `.v` and every
`flow sum equation` is about a `.i`. Nothing mixes the two.

### What stops you connecting the wrong things

Rumoca checks each **pair** of variables it is about to join, and refuses the connection if they
disagree on any of:

- **flow versus non-flow** — a current cannot be joined to a voltage
- **primitive type** — `Real` to `Real`, not `Real` to `Boolean`
- **quantity** — a variable declared as `"ElectricPotential"` will not join one declared `"Length"`
- **array shape** — scalars to scalars, arrays to arrays of the same size

**But note what that list is: checks on the pairs that *were* made.** Nothing checks that every
member found a partner — so connecting a connector holding `{v, i}` to one holding only `{v}` passes
every check above, because `v` pairs with `v` and `i` is simply never looked at. The model is invalid
and Rumoca accepts it. That gap has its own tour: [▶ the-oracle](hrw://tour/the-oracle), where
System Modeler is asked to adjudicate and rejects the same model.

This tour counts. `RcCircuit` has **four `connect` statements** and they produce **seven
equations** — and the reason it is seven rather than four or eight is the whole content of the
phase.

> Every count below is read from the specimens' generated traces.

---

## The problem this phase exists to solve

A connector carries two kinds of variable, and they obey opposite rules:

- a **potential** variable — voltage, pressure, angle. Joined connectors are all **equal**.
- a **flow** variable — current, mass flow, torque. Joined connectors **sum to zero**.

That is Kirchhoff's two laws, stated once and reused for every physical domain. Modelica's
`connect` is the general form, which is why the same construct wires a circuit, a pipe network
and a gearbox.

**And `connect` is not directional.** Nothing says which side is input; `src.p` and `R.p` are
peers. That is what makes Modelica *acausal*, and it is why matching had a job to do at all —
the causality every later phase computes was never written down by the modeller.

---

## Act 1 — Four statements, three sets

**Do the grouping yourself before opening anything.** `RcCircuit`'s entire connect section is
four lines:

```modelica
connect(src.p, R.p);
connect(R.n, C.p);
connect(C.n, src.n);
connect(src.n, gnd.p);
```

Apply the walking procedure from the lead: start anywhere, follow every `connect` you can reach,
that is one group.

> **Predict, and commit to a number before you look.** How many groups are there, and what is the
> size of the largest?
>
> **Four statements, so four groups, is the obvious answer and it is wrong.** If you got four, the
> reason is worth finding before you read on.

[▶ Now look — RcCircuit → Flatten → Connections](hrw://load/RcCircuit/Flatten/Connections)

**Expected:** **three** groups, of sizes **2, 2 and 3**.

**Falsified if** you see four groups, or three groups all of size 2, or any group containing a
connector from two different junctions. Any of those means either the grouping is wrong or this
tour is.

### Why three and not four

**`src.n` appears in two statements.** That is the whole answer. `connect(C.n, src.n)` and
`connect(src.n, gnd.p)` are not two junctions — they describe *one* junction with three things
attached, because grouping is **transitive**: if `C.n` is joined to `src.n`, and `src.n` to
`gnd.p`, then all three are one node. The walk from `C.n` reaches `gnd.p` without ever meeting a
`connect` that names them together.

| set | connectors | size |
|---|---|---|
| A | `src.p`, `R.p` | 2 |
| B | `R.n`, `C.p` | 2 |
| C | `C.n`, `src.n`, `gnd.p` | **3** |

**This is the fact Act 2's arithmetic runs on**, and it is why counting `connect` statements can
never give you the equation count.

---

## Act 2 — The arithmetic, and it is exact

A set of *n* connectors produces:

- **n − 1 potential equations** — enough to make all *n* voltages equal, and no more
- **exactly 1 flow equation** — the sum of all *n* currents is zero

| set | size | potential eqs | flow eqs |
|---|---|---|---|
| A | 2 | 1 | 1 |
| B | 2 | 1 | 1 |
| C | 3 | **2** | 1 |
| | | **4** | **3** |

[RcCircuit → Flatten → Equations](hrw://load/RcCircuit/Flatten/EquationSheet)

**Expected:** the sheet groups the equations by origin, with exactly **4** under
`connection equation` and **3** under `flow sum equation`.

**Expected:** those seven read as residuals — `0 = <expression>` — not as equalities:

```
connection equation
  0 = src.p.v - R.p.v
  0 = R.n.v - C.p.v
  0 = C.n.v - src.n.v
  0 = src.n.v - gnd.p.v

flow sum equation
  0 = src.p.i + R.p.i
  0 = R.n.i + C.p.i
  0 = C.n.i + src.n.i + gnd.p.i
```

**Expected:** the third flow row sums **three** currents while the other two sum two — that single
row is set C, and it is the visible consequence of the transitive join.

**Why residuals, and why the same equation looks different in two panes.** Rumoca stores every
continuous equation as an expression that must equal zero, so `src.p.v = R.p.v` is kept as
`src.p.v - R.p.v`. The equation sheet prints that form. **The structural report labels the same
equation differently** — over on Structural → Tree it appears as
`f_x[19] (connection equation: src.p.v = R.p.v)`, because a *label* is written for a human reading
a matching, while the *sheet* shows the mathematics as stored. Two renderings of one equation, and
neither is a rounding of the other.

**Why n − 1 and not n.** Writing all *n(n−1)/2* pairwise equalities would be redundant: equality
is transitive, and the extra equations would make the system structurally singular — exactly the
rank deficiency `matching.md` Act 3 diagnoses. **The phase must produce a spanning tree of the
set, not its complete graph.**

---

## Act 3 — Where the other sixteen equations came from

**Expected:** the 23 equations break down as:

| origin | count |
|---|---|
| equation from `R` | 7 |
| equation from `src` | 4 |
| equation from `C` | 4 |
| equation from `gnd` | 1 |
| **connection equations** | **4** |
| **flow sum equations** | **3** |

**Sixteen of the twenty-three are constitutive** — Ohm's law, the capacitor's `i = C·der(v)`,
the source's voltage definition, the ground's `v = 0`, and each component's own internal pin
equations. The connect graph contributes seven.

**That ratio is worth carrying.** Flattening is mostly *copying* — instantiating each component's
equations with its own prefix — and only a minority of the output is genuinely new. But that
minority is the part that could not have been written by hand at model scale, and it is the part
that determines the system's structure.

---

## Act 4 — A model with no connectors at all

[TwoLoops → Flatten → Equations](hrw://load/TwoLoops/Flatten/EquationSheet)

**Expected:** **4 equations**, all described as `top-level model equation`, and **no** connection
or flow-sum equations.

`TwoLoops` writes its equations directly, with no components and no `connect`. **This is why it
was the clean specimen for `blt-ordering.md`** — its equation indices map straight onto what the
source says, with no expansion in between.

**Compare the two models' variable counts.** `RcCircuit` flattens to **30 variables** for a
circuit a person would describe as having one interesting quantity. Twenty-nine of them are pin
voltages, pin currents and component internals that no one typed.

---

## Act 5 — Why this phase is where a model's size explodes

Every later tour's numbers begin here:

| model | after flattening |
|---|---|
| `RcCircuit` | 30 variables, 23 equations |
| `Drivetrain` | **97 equations** before index reduction |

`Drivetrain`'s source is a few dozen lines. The 97 equations are connectors expanded, components
instantiated, and every flange's force and position given its own name.

**So the whole downstream pipeline exists because this phase is generous.** Matching, BLT,
tearing and index reduction are all, in part, machinery for undoing the size that flattening
necessarily creates — 97 equations become 20 after reduction, and those 20 become 12 blocks of
which 11 are trivial.

**None of that is waste.** Flattening's job is to be *complete and mechanical*, so that nothing
depends on a modeller having simplified by hand. Making it small again is somebody else's job,
and you have now met all of them.

---

## What comes next in the chain

DAE construction, then everything you have already walked. If you are reading these in pipeline
order, this is the earliest one — and [`dae-construction.md`](dae-construction.md) is the next
link.

---

## What this tour cannot check

**Whether the Connections view shows sets or statements.** Act 1's entire claim is that four
statements make three sets, and it assumes the view groups connectors by set. If it lists the
four `connect` statements instead, the tour is asserting something the screen contradicts, and
Act 1 should be rewritten around the structural equation names — which is where the evidence
actually is.

**Whether the spanning-tree point in Act 2 is convincing without a demonstration.** "Pairwise
equalities would be redundant" is stated, not shown. A specimen that *did* over-connect would
show it, and none exists — that may be a specimen worth adding.

**Whether Act 5 belongs here or in an overview.** It is the argument that ties the whole pipeline
together, and it is placed in the first phase's tour, which a reader may not reach first.
