# Flatten — what `connect` actually means

`connect(src.p, R.p)` looks like wiring two things together, and in a drawing that is all it is.
**In the equations it is neither an assignment nor an equality** — it is one edge in a graph, and
the graph is solved before any equation exists.

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

[RcCircuit → Flatten → Connections](hrw://load/RcCircuit/Flatten/Connections)

The source says:

```modelica
connect(src.p, R.p);
connect(R.n, C.p);
connect(C.n, src.n);
connect(src.n, gnd.p);
```

**Four statements. But `src.n` appears in two of them**, so the last two describe *one* junction
with three things attached. Connection is **transitive**: joining `C.n` to `src.n`, and `src.n`
to `gnd.p`, joins all three.

**The sets are therefore:**

| set | connectors | size |
|---|---|---|
| A | `src.p`, `R.p` | 2 |
| B | `R.n`, `C.p` | 2 |
| C | `C.n`, `src.n`, `gnd.p` | **3** |

**Expected:** three connection sets, not four.

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
