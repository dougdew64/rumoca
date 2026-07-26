# End-to-End Guided Tour: From Modelica Model to Running Simulation

*The spine of the HRW curriculum — an interactive walk-through of what a Modelica
compiler must do and why, grounded in the MotorWithBrake specimen and driven by
HRW's stage views.*

**Specimen:** [Load MotorWithBrake](hrw://load/MotorWithBrake)
— a DC motor driving an inertial load, with a speed-limit event (MSL electrical
+ rotational components, index > 1 from the EMF coupling, discrete events, stiff
dynamics from fast electrical / slow mechanical time constants).

**Prerequisites:** HRW built and running (`cargo run -p hrw` from the workspace
root). [Load MotorWithBrake](hrw://load/MotorWithBrake) — it compiles through all
stages automatically. This tour complements the textbooks listed in
[`vision.md`](../vision.md). It references specific chapters and sections;
consult the originals for proofs and formal development. This tour provides
what the books cannot: a concrete specimen and a real compiler to ground
the theory.

**Note on simulation (2026-07-26):** Rumoca's simulator handles this model
well — the pipeline (Stops 1–11) is solid and the simulation (Stop 12)
produces dynamic trajectories with correct qualitative behavior (motor spin-up
to back-EMF-limited speed). For ground-truth trajectories, simulate specimens
in Wolfram System Modeler with identical solver tolerances.

---

## Learning Goals

By the end of this tour, you should be able to:

1. **Explain why** a Modelica model cannot be directly simulated.
2. **Trace the chain of problems** — articulate each transformation as a
   response to a specific insufficiency in what came before.
3. **Identify the mathematical form** at each major stage.
4. **Distinguish structural from numerical** analysis.
5. **Explain what a solver needs** to start and to advance.
6. **Recognize these transformations as universal** — not Rumoca-specific.
7. **Read the equations** the solver actually sees (not JSON) and trace them
   back to their Modelica source lines.
8. **Use HRW's stage views** to inspect the IR at each stop — navigate the
   JSON tree, read the equation sheet, click through source-to-equation
   links, and interpret the structural analysis visualizations.
9. **Know where to go deeper** — which phase tour to read next.

---

## The Model

Open `MotorWithBrake.mo`. Read it as an engineer would — it describes a
physical system:

```
DC voltage source (V = 12)
  → Resistor (R = 1.0 Ω)
    → Inductor (L = 0.1 mH)
      → Rotational EMF (k = 0.1 V·s/rad)
        → Inertial load (J = 0.05 kg·m²)

When the load spins too fast (> 30 rad/s), set overSpeed = true.
When it slows below 15 rad/s, set overSpeed = false.
```

The model is 56 lines of Modelica. It uses six MSL components — five
electrical (`ConstantVoltage`, `Resistor`, `Inductor`, `RotationalEMF`,
`Ground`) and one rotational (`Inertia`) — connected by `connect()`
statements, plus a `when`/`elsewhen` clause for speed-limit detection.

Here is the central question of this tour: **what must happen between this
description and a plot of load speed vs time?**

The answer is a chain of nine transformations. Each one solves a specific
problem — and each one's output creates the problem the next one must solve.

---

## Stop 0: Why Can't We Just Simulate This?

Before walking the pipeline, it is worth pausing on why a pipeline exists at
all. A Modelica model is *not* a program. It has no control flow, no
execution order, no state vector. It is a set of **declarations** and
**equations** organized into a class hierarchy.

Consider what a numerical integrator needs:

- A **state vector** x(t) — a flat array of real numbers
- A **function** ẋ = f(t, x) — or, for DAEs, F(t, x, ẋ) = 0
- **Consistent initial values** x(0) that satisfy all constraints
- An **event detection** mechanism for discontinuities

MotorWithBrake provides none of these directly. It provides:

- A hierarchy of MSL classes (ConstantVoltage, Resistor, Inductor, RotationalEMF, Inertia, …)
- Equations scattered across those classes, written in terms of local variables
- `connect()` statements that imply equations but don't spell them out
- A `when` clause that is not an equation at all — it is an event specification
- No explicit state vector, no residual function, no Jacobian

The gap between "what the engineer wrote" and "what the solver needs" is the
compiler pipeline. Every Modelica tool — Rumoca, Dymola, OpenModelica, System
Modeler — must bridge this gap. The algorithms differ in detail, but the
*problems* are universal.

> **Cellier & Kofman, *Continuous System Modeling*, Ch. 1:** The distinction
> between a *model* (a description of a system's structure and behavior) and a
> *simulation* (the numerical solution of the model's equations) is
> fundamental. The model is a mathematical artifact; the simulation is a
> computational one. A compiler bridges the two.

---

## Stop 1: Parsing — Text to Structure

**The problem:** The model is a text file. Nothing can be computed from text.

**The solution:** Parse the Modelica source into an Abstract Syntax Tree (AST).

**What happens to MotorWithBrake:** The parser reads the 56 lines and produces
a tree of nodes: one `ClassDefinition` for `MotorWithBrake`, containing 8
component declarations (6 MSL components + 1 parameter + 1 Boolean), 6
`connect` equations, a `when`/`elsewhen` clause, and an `annotation`. At
this point every identifier is just a string —
`Modelica.Electrical.Analog.Basic.RotationalEMF` is a name, not a resolved
reference to any specific class.

**The mathematical form:** None yet. This is pure syntax — the tree captures
the *shape* of the source text, not its meaning.

**What's insufficient:** The AST is a faithful mirror of what the programmer
typed, but it knows nothing about what the names mean. `rotor.flange_b`
could be anything — a variable, a class, an error. The type system hasn't
been consulted; no equation has been expanded; nothing has been checked.

**In HRW:** [Open the Parse tab](hrw://stage/Parse). The JSON tree shows the raw AST — expand
`classes` → `MotorWithBrake` → `body` → `equations` to find the 6 `connect`
nodes and the `when` clause. Click any node to inspect it; notice that
identifiers like `Modelica.Electrical.Analog.Basic.RotationalEMF` are plain
strings with no `def_id` — resolution hasn't happened yet.

> **Phase tour:** [Parsing and AST](phase1_parsing_and_ast/parsing_and_ast.md)

---

## Stop 2: Resolution — Names to Definitions

**The problem:** Identifiers are unresolved. `Modelica.Mechanics.Rotational
.Components.Inertia` is a chain of strings — the compiler doesn't yet know
which class definition it points to, whether it exists, or whether the
reference is legal.

**The solution:** Walk the AST and resolve every name against the scope chain.
Each identifier gets a `def_id` — a unique integer pointing to its definition
in the global symbol table.

**What happens to MotorWithBrake:** The resolver annotates every component
reference with its definition. `src` is an instance of `ConstantVoltage`,
`R` is a `Resistor`, `L` is an `Inductor`, `emf` is a `RotationalEMF`,
`load` is an `Inertia` — each with a unique `def_id` and `type_def_id`
pointing to its MSL class definition. The `connect(src.p, R.p)` statement's
two arguments are now linked to specific port definitions inside those MSL
classes.

**The mathematical form:** Still none. Resolution establishes the *semantic
graph* — who refers to whom — but does not yet produce equations.

**What's insufficient:** Names are resolved, but the MSL component internals
are still opaque. We know that `rotor` is an `Inertia`, but we haven't
expanded what that means — what variables it declares, what equations it
contributes, what parameter values it carries. The model is still a
hierarchical description, not a flat system of equations.

**In HRW:** [Open the Resolve tab](hrw://stage/Resolve). The tree now has `def_id` and
`type_def_id` annotations on every identifier — hover one to see the resolved
class name (e.g. `type_def_id → model Modelica.Electrical.Analog.Basic
.RotationalEMF`). Right-click a `type_def_id` → **"↪ Go to RotationalEMF"** to
navigate into the MSL class and read its internal structure. Use **← Back** to
return.

> **Phase tour:** [Resolve and Scope](phase2_resolve_and_scope/resolve_and_scope.md)

---

## Stop 3: Instantiation — Hierarchy to Instances

**The problem:** The model uses six MSL components, each of which is itself
a class with internal variables, equations, parameters, and possibly further
sub-components. The `Inertia` class declares `phi`, `w`, `a`,
`flange_a.phi`, `flange_a.tau`, `flange_b.phi`, `flange_b.tau`, and the
equation `J * a = flange_a.tau + flange_b.tau` — none of this is visible yet.

**The solution:** Instantiate every component: apply the parameter
modifications from the top-level model (`R = 1.0`, `L = 1e-4`, `k = 0.1`,
`J = 0.05`, `V = 12`, …) and recursively expand each component's internal
structure.

**What happens to MotorWithBrake:** The instance tree grows from 8
declarations to 84 entries. Each MSL component unfolds: `R` (Resistor)
brings `R.R`, `R.v`, `R.i`, `R.p.v`, `R.p.i`, `R.n.v`, `R.n.i`,
`R.LossPower`, `R.T_heatPort`, `R.R_actual`. `emf` (RotationalEMF) brings
`emf.k`, `emf.v`, `emf.i`, `emf.phi`, `emf.w`, plus internal support
flange variables. `load` (Inertia) brings `load.J`, `load.phi`, `load.w`,
`load.a`, `load.flange_a.phi`, `load.flange_a.tau`, `load.flange_b.phi`,
`load.flange_b.tau`.

Every parameter modification is applied: `load.J = 0.05` (from the
top-level `Inertia load(J = 0.05)` declaration) overrides the MSL default.

**The mathematical form:** Still no equations in usable form — but the raw
material is now all present. Every variable that will eventually appear in
the equation system is now declared somewhere in the instance tree.

**In HRW:** [Open the Instantiate tab](hrw://stage/Instantiate). The tree is much larger — expand
`components` to see the fully instantiated component hierarchy. Each MSL
component has been expanded with its internal variables, equations, and
parameter modifications applied.

**What's insufficient:** The instance tree is still hierarchical. Variables
have local names (`phi`, `w`) within their containing class. Equations
reference variables through their class's local scope. No global naming
scheme exists, and `connect()` statements have not been expanded into
equations.

> **Cellier & Kofman, *CSM*, Ch. 6:** Modelica's object-oriented
> decomposition allows an engineer to build a model from reusable components
> (an inertia, a gear, a spring) without thinking about the global equation
> system. This is the power of the language — and the source of the compiler's
> work.

---

## Stop 4: Type Checking — Dimensional Consistency

**The problem:** Variables have been declared but not yet verified for type
consistency. Does `rotor.J * rotor.a` make physical sense? Are array
dimensions compatible?

**The solution:** Assign each expression a type, verify dimensional
consistency (SI units where annotated), evaluate array dimensions, and check
variability constraints (a `parameter` must not depend on time-varying
quantities).

**What happens to MotorWithBrake:** Every variable receives its resolved
type. `L.i` is `Current` (A), `src.V` is `Voltage` (V), `R.R` is
`Resistance` (Ω), `load.phi` is `Angle` (rad), `load.w` is
`AngularVelocity` (rad/s), `load.J` is `MomentOfInertia` (kg·m²). The
EMF equation `v = k * w` type-checks: `Voltage = (V·s/rad) × (rad/s)`.
The load equation `J * a = flange_a.tau + flange_b.tau` type-checks:
`MomentOfInertia × AngularAcceleration = Torque + Torque`. The speed
parameter `maxSpeed` is `Real` (no SI annotation, but dimensionally
consistent within its usage context).

**The mathematical form:** Still hierarchical, still no global equation
system. But the semantic contract is now verified — every expression is
well-typed, every array dimension is resolved, every variability constraint
is satisfied.

**In HRW:** [Open the Typecheck tab](hrw://stage/Typecheck). The tree is structurally similar to
Instantiate, but every expression now carries resolved type information. Look
for `type_specifier` fields on variable declarations.

**What's insufficient:** Type-checked variables still live inside a class
hierarchy with local names. No flat equation system exists. The seven
`connect()` statements have still not been expanded.

> **Phase tour:** [Typecheck and Dimensions](phase3_typecheck_and_dims/typecheck_and_dims.md)
>
> **MLS §4.9:** Variability classification rules — constant, parameter,
> discrete, continuous. These are the rules the typechecker enforces.

---

## Stop 5: Flattening — From Objects to a Flat Equation System

**The problem:** The model is a tree of objects with local names and
`connect()` statements. A mathematical analysis tool needs a single, flat
system of equations over globally-named variables.

**The solution:** Walk the instance tree, build globally-qualified names
(e.g. `rotor.flange_b.tau`), and expand every `connect()` into explicit
equations.

**What happens to MotorWithBrake:** The six `connect()` statements produce
dozens of equations:

- **Potential equalities.** `connect(src.p, R.p)` generates
  `0 = src.p.v - R.p.v` — the two pins share the same voltage (Kirchhoff's
  voltage law). `connect(emf.flange, load.flange_a)` generates
  `0 = emf.flange.phi - load.flange_a.phi` — shared angular position.

- **Flow-sum equations.** At every connection node, the flow variables
  (currents or torques) must sum to zero: `0 = src.p.i + R.p.i` for
  currents, `0 = emf.flange.tau + load.flange_a.tau` for torques. This is
  Kirchhoff's current law — currents balance at a node, torques balance at
  a mechanical junction.

- **Component equations** from each MSL class are included with qualified
  names. The inertia's `J * a = flange_a.tau + flange_b.tau` becomes
  `0 = load.J * load.a - (load.flange_a.tau + load.flange_b.tau)` in
  residual form. The EMF's `v = k * w` and `tau = -k * i` couple the
  electrical and mechanical domains.

The flat model has 61 variables and 47 equations, all in **residual form**:
`0 = expression`. The class hierarchy is gone. Connect statements are gone —
replaced by the equality and flow-sum equations they imply.

**The mathematical form:** A system of algebraic and differential equations
over a flat variable set:

```
0 = f₁(v₁, v₂, …, der(vₖ), …)
0 = f₂(…)
⋮
0 = fₙ(…)
```

This is the first point at which we have something recognizable as
*mathematics* — a system of equations. But it is not yet in any standard
form that a textbook would recognize.

**In HRW:** [Open the Flatten tab](hrw://stage/Flatten). You have three sub-views:

1. **Equations** — the equation sheet. This is the single biggest upgrade over
   the JSON tree: all 47 equations rendered in readable mathematical notation,
   grouped by origin (component equations, connect-generated equalities,
   connect-generated flow sums, initial equations). Read them as a mathematician
   would — these are the equations the solver will actually see. The variable
   classification table at the top lists every variable with its role (state,
   algebraic, parameter), start value, and unit.

2. **Source Map** — the source-to-equation traceability view. The left pane shows
   `MotorWithBrake.mo` source code; the right pane shows the equation sheet.
   **Click a source line** (e.g. line 34, `connect(emf.flange, load.flange_a)`)
   → the equations it generated highlight in the right pane. **Click an
   equation** → the source line(s) that produced it highlight in the left pane.
   This is the bridge between the OO world the engineer wrote and the flat
   equation world the compiler produced.

3. **Tree** — the raw JSON tree (the original view, still available for deep
   inspection).

**What's insufficient:** The flat system is correct but unstructured. We have
a soup of 47 equations and 61 variables. We don't know which variables are
states, which are algebraic, which are parameters. We don't know the DAE
index. We have no evaluation order. The `when` clause is still embedded as
a structured object, not yet converted to the event-handling form a solver
needs.

> **Phase tour:** [Flattening](phase5_flatten/flatten.md)
>
> **MLS §9.2:** Connection semantics — potential equalities and flow sums.
> The rules that generate equations from `connect()` statements.

---

## Stop 6: DAE Construction — From Equation Soup to Standard Mathematical Form

**The problem:** We have a flat system of equations, but they are not
classified. A numerical solver needs to know: which variables change with
time (states)? Which are determined at each instant by the equations
(algebraics)? Which are fixed before simulation starts (parameters)? Which
change only at discrete events?

**The solution:** Classify every variable into the MLS Appendix B partition
and route every equation to the appropriate group.

**What happens to MotorWithBrake:** Every variable is classified:

| Partition | Symbol | MotorWithBrake examples | Count |
|-----------|--------|------------------------|-------|
| States | x(t) | `L.i`, `emf.phi`, `load.phi`, `load.w` | 4 |
| Derivatives | ẋ(t) | `der(L.i)`, `der(emf.phi)`, `der(load.phi)`, `der(load.w)` | 4 |
| Algebraics | y(t) | `emf.w`, `emf.v`, `load.a`, `src.i`, `R.v`, `load.flange_a.tau`, … | many |
| Parameters | p | `src.V`, `R.R`, `L.L`, `emf.k`, `load.J`, `maxSpeed` | many |
| Discrete-valued | m(tₑ) | `overSpeed` (Boolean) | 1 |

The equations are routed into four groups:

```
Continuous (between events):
  0 = f_x(x, ẋ, y, p, t)                    [B.1a — the implicit DAE]

Discrete real updates (at events):
  z := f_z(x, y, p, t, pre(z), pre(m))      [B.1b]

Discrete-valued updates (at events):
  m := f_m(x, y, p, t, pre(z), pre(m))      [B.1c]

Condition equations:
  c := f_c(relation(x, y, p, t))             [B.1d]
```

The `when`/`elsewhen` clause is expanded:

```
overSpeed := if (load.w > maxSpeed AND NOT pre(load.w > maxSpeed)) then true
             elseif (load.w < 0.5*maxSpeed AND NOT pre(…)) then false
             else pre(overSpeed)
```

This goes into f_m. The speed-limit conditions (`load.w > maxSpeed`, `load.w <
0.5*maxSpeed`) are extracted as zero-crossing relations for the event
detector.

**The mathematical form:** The canonical hybrid DAE of MLS Appendix B.
This is the form that Cellier, Hairer & Wanner, and Brenan, Campbell &
Petzold analyze. For the first time, the system is in a standard notation
that a textbook reader would recognize.

**In HRW:** [Open the Flatten tab](hrw://stage/Flatten). The equation sheet now shows the classified
system — equations grouped into the four MLS Appendix B partitions (continuous
f_x, discrete real f_z, discrete-valued f_m, conditions f_c). The variable
table shows the partition column (state, algebraic, parameter, discrete).
Compare this with the Flatten tab's equation sheet: the equations are the same,
but now classified.

**What's insufficient:** The DAE is in standard form, but its **differential
index may be greater than 1**. The EMF's internal support creates a
position-level constraint coupling `emf.phi` to a fixed reference. Standard
DAE solvers — BDF, ESDIRK, TR-BDF2 — require an index-1 system where every
state has a computable derivative at each time step. One of the four states
has no usable derivative equation — it is constrained by the EMF coupling.

> **Phase tour:** [DAE Construction](phase6_dae_construction/dae_construction.md)
>
> **MLS Appendix B:** The formal definition of the hybrid DAE form. This is
> the mathematical contract between the Modelica language and its solvers.
>
> **Cellier & Kofman, *CSM*, Ch. 9:** DAE systems and the challenges they
> pose for numerical methods. The index concept is introduced here.

---

## Stop 7: Index Reduction — Eliminating Hidden Constraints

**The problem:** MotorWithBrake has 4 state variables, but one of them —
`emf.phi` — is constrained by the EMF's internal support mechanism. The
RotationalEMF component has a `fixed` internal support that anchors its
reference frame, creating a position-level constraint:
`emf.internalSupport.flange.phi = emf.fixed.flange.phi`. This connects
`emf.phi` to a fixed reference without providing a derivative equation.

A BDF integrator trying to advance `emf.phi` would look for an equation
giving `der(emf.phi)` and find... nothing. The position is constrained by
the fixed support, not by a force balance. The integrator gets stuck.

This is the **high-index problem** — the DAE's differential index is greater
than 1 because an algebraic constraint implicitly determines a state's
derivative.

**The solution:** Identify constrained states and demote them to algebraic
variables, eliminating the redundant degrees of freedom. If necessary,
differentiate algebraic constraints with respect to time to manufacture the
missing derivative equations.

**What happens to MotorWithBrake:** The index reduction pipeline runs 10
steps:

1. **Constrained dummy derivative demotion** demotes 1 state. The EMF's
   fixed-support constraint makes `emf.phi` redundant — its value is
   determined by the connection equality `emf.flange.phi = load.flange_a.phi`,
   not by an independent differential equation. The reducer demotes
   `emf.phi` from state to algebraic.

2. **Trivial elimination** removes 41 variables whose values are determined
   by a single equation. For example:
   - `src.v → src.V` (constant voltage source)
   - `R.T_heatPort → R.T` (thermal port)
   - `L.p.i → L.i` (connection equality)
   - `emf.w → load.w` (EMF-to-load connection)
   - `R.R_actual → R.R * (1 + R.alpha * (R.T_heatPort - R.T_ref))` (thermal coefficient)

The result: **4 states → 3 states** (`L.i`, `load.phi`, `load.w`).
The original 47-equation system collapses to 7 equations and 7 unknowns.

These three surviving states have clear physical meaning. `L.i` is the
inductor current (electrical energy storage), `load.phi` is the load
angular position, and `load.w` is the load angular velocity (mechanical
energy storage). Everything else is determined by the circuit laws and
mechanical constraints.

**The mathematical form:** An index-1 DAE — every remaining state has a
usable derivative equation. The system's three ODE-like equations are:

```
der(L.i)      = (src.V - R.R*L.i - emf.k*load.w) / L.L    (circuit KVL)
der(load.phi) = load.w                                      (kinematics)
der(load.w)   = load.a                                      (Newton's 2nd law)
```

The remaining 4 equations determine the algebraic unknowns: the load
acceleration (`load.a`), the connection voltage (`src.n.v`), the EMF
support position, and a connection equality.

**In HRW:** [Open the Index Reduction tab](hrw://stage/IndexReduction). The reduction report shows the
10-step pipeline: constrained dummy derivative demotion (1 state demoted),
trivial eliminations (41 variables removed), and the final equation count
(7 equations, 7 unknowns). The equation sheet here shows the *reduced*
system — compare it with the DAE tab to see which equations and variables
were eliminated.

**What's insufficient:** We have 7 equations and 7 unknowns. But in what
order do we evaluate them? Some equations depend on the results of others.
And there may be circular dependencies — algebraic loops — where equations
cannot be ordered at all and must be solved simultaneously. Without a
solution order, a solver would have to solve all 7 equations jointly at
every time step, forming and factoring a 7×7 Jacobian. For this small
system that's feasible, but for industrial models with thousands of
equations, it's prohibitively expensive.

> **Phase tour:**
> [Index Reduction and State Demotion](phase6_dae_construction/index_reduction.md)
>
> **Cellier & Kofman, *CSM*, Ch. 9:** Index reduction and the Pantelides
> algorithm. MotorWithBrake demonstrates the same fundamental issue Cellier
> discusses: position-level algebraic constraints that couple states,
> requiring differentiation or demotion to recover an index-1 system.
>
> **Brenan, Campbell & Petzold, Ch. 1–2:** The formal definition of
> differential index and its implications for numerical methods. Why standard
> ODE solvers fail on high-index DAEs.

---

## Stop 8: Structural Analysis — Finding the Solution Order

**The problem:** We have 11 equations and 11 unknowns, but no evaluation
order. Which equation should be computed first? Which depends on which?
Are there circular dependencies?

**The solution:** Build an **incidence matrix**, find a **maximum matching**
(pairing each equation with one unknown), detect cycles via **Tarjan's SCC
algorithm**, and arrange the result into **BLT (block lower-triangular)
form** — a sequenced recipe for evaluation.

**What happens to MotorWithBrake:**

**Step 1 — Incidence matrix.** The 7×7 matrix records which unknowns
appear in which equations. Crucially, the *columns* are the unknowns the
solver must determine: state derivatives (`der(L.i)`, `der(load.phi)`,
`der(load.w)`) and algebraic variables (`load.a`, `src.n.v`, the EMF
support position, and a connection equality). State *values* are not
columns — they are known inputs to the integrator at each step.

**Step 2 — Maximum matching.** Kuhn's augmenting-path algorithm pairs each
equation with exactly one unknown. The matching is *perfect* (all 7
equations matched) — confirming the system is structurally non-singular
after index reduction. Example pairings:

| Equation | Matched unknown |
|----------|-----------------|
| inductor V = L·di/dt | `der(L.i)` |
| load kinematics | `der(load.phi)` |
| load Newton's 2nd law | `der(load.w)` |
| load acceleration | `load.a` |
| ground connection | `src.n.v` |
| EMF support constraint | EMF support position |
| EMF-load connection | connection equality |

**Step 3 — Dependency graph and Tarjan's SCCs.** From the matching, build
a directed graph: equation A depends on equation B if A references the
variable that B is matched to. Run Tarjan's algorithm to find strongly
connected components — cycles where equations depend on each other
mutually.

MotorWithBrake has **no algebraic loops** — all 7 blocks are scalar. This
means every equation can be evaluated independently in sequence, with no
iteration needed. The trivial elimination pass in index reduction did most
of the work: by substituting 41 variables, it broke the circular
dependencies that the original 47-equation system had.

**Step 4 — BLT assembly.** The final evaluation plan has 7 scalar blocks:

```
Block 1:  Scalar — src.n.v                  (ground connection)
Block 2:  Scalar — der(L.i)                 (inductor equation)
Block 3:  Scalar — der(load.phi)            (= load.w)
Block 4:  Scalar — der(load.w)              (= load.a)
Block 5:  Scalar — load.a                   (Newton's 2nd law)
Block 6:  Scalar — EMF support position     (fixed constraint)
Block 7:  Scalar — EMF-load connection      (equality)
```

The solver evaluates these blocks **in order**, top to bottom. Every block
is a single assignment — no Newton iteration needed. This is as cheap as
evaluation gets.

**The mathematical form:** A **block lower-triangular system** — the same
form a textbook presents as the output of Gaussian elimination applied to
the system's dependency structure, but discovered through the incidence
matrix rather than through numerical values.

This is a critical distinction: structural analysis works on the *pattern*
of which variables appear where, not on numerical values. It is graph
theory, not linear algebra. The matching is a combinatorial problem (find a
perfect matching in a bipartite graph). The BLT ordering is topological
sorting of the condensation DAG. None of this requires evaluating a single
floating-point number.

**In HRW:** [Open the Structural tab](hrw://stage/Structural) — this is HRW's richest view. Four
sub-views:

1. **Incidence** — the 7×7 incidence matrix with the matching overlay (green
   circles on matched cells). Hover a cell to see the equation↔unknown pair.
   Zoom in (scroll wheel, zoom ≥ 16) for full labels. Click an equation row →
   the corresponding equation highlights in the Flatten tab's equation sheet.

2. **Matching ▶** — an animated replay of Kuhn's augmenting-path algorithm
   finding the maximum matching, step by step. Use Play/Pause/Step to watch
   each augmenting path discovered. This is the
   [three-tier progression](phase7_structural_analysis/guided-tour.md): static
   snapshot (the matching overlay), recorded replay (this animation), and
   live-stepped execution (set a breakpoint in the Rust code via the Debug
   shortcut).

3. **BLT ▶** — an animated replay of Tarjan's SCC algorithm discovering the
   7-block BLT decomposition. Watch the DFS stack, the low-link updates, and
   the SCC pops. All 7 blocks are scalar (no algebraic loops).

4. **Spy Plot** — the BLT spy plot: the block-diagonal structure of the sorted
   system. All blocks are 1×1 — a clean diagonal with no coupled blocks.
   (For a model *with* algebraic loops, load `ProportionalLoop` or
   `GearWithBrake` to see coupled blocks as filled squares on the diagonal.)

For the full interactive experience, see the
[Structural Analysis guided tour](phase7_structural_analysis/guided-tour.md)
— a five-lesson walkthrough with the three-tier progression.

**What's insufficient:** We have a solution order for the continuous-time
equations. But at t = 0, the solver hasn't taken a step yet — it has no
"previous state" to feed into the equations. We need consistent initial
values that satisfy all the algebraic constraints.

> **Phase tour:**
> [Structural Analysis](phase7_structural_analysis/structural_analysis.md)
> (and drill-downs on
> [incidence matrices](phase7_structural_analysis/incidence_matrix.md),
> [matching](phase7_structural_analysis/maximum_bipartite_matching.md),
> [Tarjan SCCs](phase7_structural_analysis/tarjan_scc.md),
> [BLT form](phase7_structural_analysis/blt.md),
> [tearing](phase7_structural_analysis/tearing.md))
>
> **Cellier & Kofman, *CSM*, Ch. 9.3–9.5:** Structural analysis of DAE
> systems — incidence matrices, maximum matching ("structurally admissible
> assignment"), BLT decomposition. The algorithms Rumoca implements are
> the same ones Cellier presents.
>
> **Hairer & Wanner, *SODEs II*, Ch. VIII.2:** Structural aspects of
> implicit systems. The BLT form as a decomposition strategy.

---

## Stop 9: Initialization — Making the First Step Possible

**The problem:** At t = 0, the integrator needs consistent values for every
variable: the states x(0), the algebraics y(0), and the derivatives ẋ(0)
must all satisfy the continuous equations simultaneously. This is itself a
(potentially nonlinear) system of equations that must be solved before
integration begins.

For MotorWithBrake, the states have `start` values (`L.i = 0`, `load.phi = 0`,
`load.w = 0`), but the algebraic unknowns — the EMF voltage, the load
acceleration, the connection equalities — must be computed from these start
values by solving the algebraic equations.

**The solution:** Build an **IC (initial condition) plan** — a precomputed
recipe that tells the runtime exactly how to compute consistent initial
values. The IC plan reuses the matching/BLT/tearing machinery from
structural analysis, but applied to the algebraic-only subsystem (treating
states as known constants at their start values).

**What happens to MotorWithBrake:** Rumoca constructs an IC plan that
sequences the algebraic variable initialization. The plan assigns each
algebraic unknown to a block type:

- **ScalarDirect:** the equation can be symbolically rearranged for the
  unknown — cheapest, just evaluate an expression.
- **ScalarNewton:** no symbolic solution, use single-variable Newton
  iteration.
- **TornBlock:** an algebraic loop, reduced by tearing.
- **CoupledLM:** a loop where tearing fails — full Levenberg-Marquardt.

For MotorWithBrake, at t = 0 with all start values at zero:
`L.i = 0` (no initial current), `load.w = 0` (load at rest), so the
back-EMF is zero and the full voltage drives the initial current ramp.

**Note:** MotorWithBrake's initialization currently fails in Rumoca (41/44
matched — a structurally singular init subsystem caused by MSL support-flange
variables in the EMF). This is a known Rumoca limitation, not a fundamental
problem. The model still simulates successfully because the solver falls back
to a relaxed IC solve, and the zero start values happen to be physically
consistent. This failure is itself educational: it shows that initialization
is a hard problem in practice, not just in theory.

**In HRW:** [Open the Initialization tab](hrw://stage/Initialization). The determinacy view shows
the IC plan's matching result — how many of the initialization unknowns were
matched (41/44 for MotorWithBrake). The unmatched variables are the ones
causing the structural singularity. For a simpler example, load `RcCircuit`
to see a fully matched IC plan with ScalarDirect and TornBlock assignments.

**The mathematical form:** A nonlinear system F(y) = 0 at t = 0, where
the states x(0) are fixed and the algebraics y(0) are the unknowns.
This is a **root-finding problem**, typically solved by Newton's method or
Levenberg-Marquardt.

**What's insufficient:** We can now start the integrator. But what happens
when `load.w` crosses 30 rad/s and the speed-limit triggers? The discrete
variable `overSpeed` changes from false to true. While this particular event
doesn't change the continuous dynamics, in general, events can change the
equations — and a standard integrator, which assumes smooth dynamics, needs
to detect these transitions precisely.

> **Phase tour:**
> [IC Plan](phase7_structural_analysis/ic_plan.md)
>
> **Brenan, Campbell & Petzold, Ch. 2.4:** Consistent initialization of
> DAE systems. Why it is harder than ODE initialization and what can go
> wrong.
>
> **Cellier & Kofman, *CSS*, Ch. 4–5:** Numerical methods for initial-value
> problems. The requirement for consistent initial conditions.

---

## Stop 10: Event Handling — Discrete Jumps in Continuous Flow

**The problem:** MotorWithBrake has discrete dynamics. The `when` clause
switches `overSpeed` from false to true when `load.w > maxSpeed`, and back
to false when `load.w < 0.5 * maxSpeed`. While this particular event doesn't
alter the continuous equations, the compiler must handle it as a general
event — in more complex models, events trigger torque changes, reinitialize
states, or switch equation sets. A numerical integrator that ignores events
will produce incorrect discrete state trajectories.

**The solution:** Extract the relational conditions as **zero-crossing
functions** and use an event-detection mechanism to locate the exact
instants when conditions change. At each event:

1. The integrator stops.
2. Discrete variables are updated (the `f_m` equations fire).
3. The continuous equations are re-evaluated with the new discrete state.
4. The integrator restarts with fresh initial conditions.

**What happens to MotorWithBrake:** The event structure has 2 conditions and
1 discrete-valued update:

| Condition | Expression | Purpose |
|-----------|------------|---------|
| c[1] | `load.w > maxSpeed` | Speed-limit trigger |
| c[2] | `load.w < maxSpeed * 0.5` | Speed-limit release |

The discrete update is the expanded `when`/`elsewhen`:

```
overSpeed := if (c[1] AND NOT pre(c[1])) then true
             elseif (c[2] AND NOT pre(c[2])) then false
             else pre(overSpeed)
```

The zero-crossing functions `load.w - maxSpeed` and `load.w - 0.5*maxSpeed`
guide the event locator. The solver uses bisection (or a faster root-finding
method) on these functions to pinpoint the crossing time to within the
specified tolerance.

**The mathematical form:** A **hybrid automaton** — continuous dynamics
between events, with discrete transitions triggered by zero-crossings of
guard functions. The MLS Appendix B formalization captures this as the
interplay of f_x (continuous), f_m (discrete updates), and f_c (conditions).

**In HRW:** [Open the Events tab](hrw://stage/Events). The event listing shows the 2 conditions
and the discrete-valued update for `overSpeed`. For contrast, load
`SingleInertia` (a smooth model) — the Events tab shows "no events,"
confirming that the event structure is specific to hybrid models. For a
richer event example, load `BouncingBall` — it has `reinit` (state
reinitialization at events), not just Boolean tracking.

**What's insufficient:** The DAE is index-1, structurally decomposed,
initialized, and event-aware. But it is still a *mathematical description*.
A runtime solver needs a *computational artifact* — a compiled residual
function, a Jacobian, a variable layout, and event dispatch logic. The
mathematical DAE must be lowered to execution-ready form.

> **MLS §8.5–8.6:** Events and hybrid modeling. The formal semantics of
> `when`, `pre()`, `reinit`, and zero-crossing detection.
>
> **Cellier & Kofman, *CSS*, Ch. 10:** Discontinuity handling in
> continuous-system simulation. State events, time events, and the
> challenges of locating event times accurately.
>
> **Hairer, Nørsett & Wanner, *SODEs I*, Ch. II.6:** Event location
> techniques for ODE solvers — the same principles apply to DAE solvers.

---

## Stop 11: Solve Lowering — From Mathematics to Compute Graph

**The problem:** The DAE IR (Appendix B form) is excellent for mathematical
reasoning but not for execution. Every consumer — BDF integrator, Jacobian
evaluator, code generator — would otherwise re-walk expression trees,
re-build variable layouts, and re-compute Jacobians independently.

**The solution:** Lower the DAE to a **SolveProblem** — a tensor compute
graph with a flat variable layout, compiled residual blocks, a symbolically-
derived Jacobian, and pre-sequenced initialization, event, and discrete
partitions.

**What happens to MotorWithBrake:** The 51 variables are assigned specific
slots in a flat state vector:

- `L.i → Y[0]`, `emf.phi → Y[1]`, `load.phi → Y[2]`, `load.w → Y[3]`
  (states and their derivatives)
- Algebraic unknowns fill the next slots
- `overSpeed → Y[48]`, conditions `c[1]–c[2] → Y[49]–Y[50]`

The continuous residual `F(t, y, ẏ) = 0` is compiled into a `ComputeBlock`
— a sequence of operations the runtime dispatches. A forward-mode automatic
differentiation pass produces the Jacobian `∂F/∂y` symbolically during
lowering, so the solver never approximates it with finite differences.

**The mathematical form:** The same DAE, but encoded as a dispatch-ready
compute graph rather than symbolic expression trees. The SolveProblem is
schema-versioned and serializable — it can cross process boundaries (JSON,
binary) for codegen targets.

**In HRW:** [Open the Solve Lowering tab](hrw://stage/SolveLowering). The JSON tree shows the
SolveProblem: expand `variable_layout` to see the flat slot assignments
(Y[0] through Y[50]), `compute_blocks` for the compiled residual operations,
and `jacobian` for the symbolically-derived ∂F/∂y. This is the last
compiler output — the next stop is execution.

**What's insufficient:** We have an execution-ready artifact. Now we
actually have to *run* it — choose a solver, feed it the residual and
Jacobian, step forward in time, handle events, and produce output.

> **Phase tour:** [Solve Lowering](phase8_solve_lowering/solve_lowering.md)

---

## Stop 12: Simulation — Running the Model

**The problem:** We have a compiled SolveProblem. Now we need a numerical
solver to integrate the DAE forward in time, locate events, handle
discontinuities, and produce trajectories.

**The solution:** Feed the SolveProblem to an implicit DAE solver (BDF for
stiff systems like MotorWithBrake) with event detection.

**What happens to MotorWithBrake:**

1. **Initialization.** The IC plan solves for consistent algebraic values
   at t = 0. All initial values are zero: no current, load at rest,
   overSpeed is false.

2. **Time stepping.** The BDF integrator advances the three true states
   (`L.i`, `load.phi`, `load.w`). At each step it evaluates the
   residual F(t, y, ẏ) = 0 and the Jacobian ∂F/∂y, then solves the
   resulting nonlinear system via Newton's method. The BLT ordering from
   structural analysis makes this efficient — all 7 blocks are scalar
   assignments, no Newton iteration needed for the algebraic part.

3. **Event detection.** The solver monitors the zero-crossing functions
   `load.w - maxSpeed` and `load.w - 0.5*maxSpeed`. When a sign change
   is detected between steps, the solver bisects to find the crossing time,
   updates the discrete state (`overSpeed`), and continues integration.

4. **The physics.** The constant voltage drives current through the
   circuit. The EMF converts electrical energy to mechanical torque,
   spinning up the load. The current `L.i` drops from 12 A toward ~8 A
   as the back-EMF `k·w` increases, opposing the source voltage. The load
   accelerates from rest toward the back-EMF-limited steady-state speed
   of V/k = 120 rad/s, reaching ~40 rad/s in the 0.5 s simulation window.
   The speed crosses 30 rad/s (maxSpeed), triggering the overSpeed event.

**Result:** 51 variable trajectories over 501 time points (t = 0 to 2.0),
with discontinuity segments at the speed-limit event.

**In HRW:** [Open the Simulation tab](hrw://stage/Simulation) and press **▶ Run**. Two plots appear:

1. **Trajectory plot** — state variables vs time. Watch `load.w` accelerate
   from 0 toward ~40 rad/s (a concave curve as back-EMF brakes the
   acceleration), while `L.i` drops from 12 A as the motor loads up.
   The two curves tell the story of electromechanical energy conversion.

2. **Solver diagnostics** (below the trajectory) — step size h(t) and BDF
   order k(t) vs time, with a synchronized time axis. The step size starts
   tiny (the fast L/R electrical time constant of ~0.1 ms forces small
   initial steps), then grows as the solver recognizes the smooth mechanical
   dynamics. The BDF order climbs from 1 to higher orders as the solver
   gains confidence. This is stiffness in action: the electrical and
   mechanical time constants differ by a factor of ~50,000.

**The mathematical form:** Numerical integration of the index-1 DAE with
event handling. The BDF method is an implicit multistep method — at each
step it solves a nonlinear system of the form:

```
G(yₙ) = F(tₙ, yₙ, (yₙ - Σ αⱼyₙ₋ⱼ) / (hβ₀)) = 0
```

where αⱼ and β₀ are the BDF coefficients and h is the step size. For stiff
systems (and MotorWithBrake is stiff — the electrical time constant L/R =
0.1 ms is ~50,000× faster than the mechanical time constant J·R/k² ≈ 5 s),
implicit methods are essential.

> **Phase tour:** [Simulation](phase9_simulation/simulation.md)
>
> **Hairer & Wanner, *Solving ODEs II*:** The definitive reference on stiff
> integration methods — BDF, ESDIRK, Radau, their stability regions, order
> barriers, and convergence theory. Ch. V covers BDF.
>
> **Brenan, Campbell & Petzold, Ch. 3–5:** Numerical solution of DAEs.
> How BDF methods extend from ODEs to DAEs, what index-1 buys you (the
> same convergence theory applies), and what higher index breaks.
>
> **Cellier & Kofman, *CSS*, Ch. 3–8:** A comprehensive treatment of
> numerical integration — from Euler to BDF to implicit Runge-Kutta —
> progressing from simple ODEs to stiff systems to DAEs.

---

## The Chain of Problems

Looking back across all twelve stops, a single pattern emerges. Every step
exists because the previous step's output is *insufficient* for what comes
next:

```
Modelica text
  └─ Problem: can't compute on text
     → Parse to AST

AST
  └─ Problem: names are unresolved strings
     → Resolve names to definitions

Resolved AST
  └─ Problem: component internals are opaque
     → Instantiate (expand class hierarchy)

Instance tree
  └─ Problem: types unchecked, dims unresolved
     → Typecheck

Typed instance tree
  └─ Problem: still hierarchical, no global equations
     → Flatten (expand connects, qualify names)

Flat equation system
  └─ Problem: unstructured — no variable classification
     → DAE construction (MLS Appendix B partition)

High-index DAE
  └─ Problem: index > 1, states without derivative equations
     → Index reduction (demote constrained states)

Index-1 DAE
  └─ Problem: no evaluation order
     → Structural analysis (matching, BLT, tearing)

Structurally sorted DAE
  └─ Problem: no consistent initial values
     → IC plan (initialization recipe)

Initialized DAE
  └─ Problem: discontinuities will corrupt integration
     → Event extraction (zero-crossing functions)

Event-aware DAE
  └─ Problem: mathematical description, not executable
     → Solve lowering (compile to tensor compute graph)

SolveProblem
  └─ Problem: need to actually advance in time
     → Simulation (BDF integration + event handling)

Time-series output ✓
```

This chain is not a Rumoca design choice. It is inherent in the gap between
what Modelica expresses (a declarative, hierarchical, equation-based model
of a physical system) and what a computer needs to produce a solution
(an executable, sequential, numerically-sound computation). Every Modelica
tool must traverse this chain in some form.

---

## Structural vs. Numerical: Two Kinds of Reasoning

One of the most important distinctions in this pipeline is between analysis
that works on the *pattern* of variables in equations (structural) and
analysis that works on *numerical values* (numerical).

**Structural analysis** (Stops 8–9) examines the incidence matrix — which
variables appear in which equations — without evaluating a single
expression. The matching algorithm doesn't know that `J = 0.01`; it only
knows that equation 3 mentions variables 2, 6, and 7. This graph-theoretic
reasoning is:

- **Cheap** — polynomial in the number of equations, independent of
  numerical difficulty
- **Exact** — the matching is either perfect or it isn't; there is no
  approximation
- **Model-independent** — changing parameter values doesn't change the
  structural analysis (only the coefficient values change)

**Numerical analysis** (Stops 9, 12) works with actual floating-point
values. Newton's method for initialization, BDF stepping, Jacobian
evaluation, event bisection — these all compute with real numbers and are
subject to convergence failures, roundoff error, and stiffness.

The power of the structural-then-numerical ordering is that cheap, exact
structural analysis dramatically reduces the work the expensive, approximate
numerical phase must do. Without BLT decomposition, the solver would face
an 11×11 coupled system; with it, the solver faces 6 scalar assignments
and one 1-variable iteration.

> **Cellier & Kofman, *CSM*, Ch. 9:** The structural analysis chapter is
> entirely about exploiting structure to reduce numerical work. This is the
> central idea.

---

## Where to Go Deeper

Each stop in this tour has a corresponding **phase tour** that goes deeper
into the algorithms. The phase tours follow the same problem-before-solution
structure but at the level of individual algorithms: how does the augmenting-
path algorithm work? What does the Tarjan DFS stack look like? How does the
chain-rule differentiator produce a new equation?

| Topic | Phase tour | Key algorithm |
|-------|-----------|---------------|
| Parsing | [Phase 1](phase1_parsing_and_ast/parsing_and_ast.md) | Recursive descent |
| Name resolution | [Phase 2](phase2_resolve_and_scope/resolve_and_scope.md) | Scope chain lookup |
| Type checking | [Phase 3](phase3_typecheck_and_dims/typecheck_and_dims.md) | Type inference, unit checking |
| Instantiation | [Phase 4](phase4_instantiate/instantiate.md) | Modification application |
| Flattening | [Phase 5](phase5_flatten/flatten.md) | Connection expansion (MLS §9.2) |
| DAE construction | [Phase 6](phase6_dae_construction/dae_construction.md) | Variable classification, when expansion |
| Index reduction | [Phase 6 drill-down](phase6_dae_construction/index_reduction.md) | Dummy derivative demotion, symbolic d/dt |
| Structural analysis | [Phase 7](phase7_structural_analysis/structural_analysis.md) | Matching, Tarjan, BLT, tearing |
| Initialization | [Phase 7 drill-down](phase7_structural_analysis/ic_plan.md) | IC plan (ScalarDirect/Newton/Torn/CoupledLM) |
| Solve lowering | [Phase 8](phase8_solve_lowering/solve_lowering.md) | Variable layout, AD Jacobian |
| Simulation | [Phase 9](phase9_simulation/simulation.md) | BDF/ESDIRK, event detection |

The **specimen notebook**
([`MotorWithBrake trace`](../specimen-notebook/MotorWithBrake/trace/))
contains the full IR at every stage — the raw data this tour narrates.

---

## Reading List Connections

This tour references the following textbooks. Each stop above links to
specific chapters; here is the high-level mapping:

| Book | What it covers | Tour stops |
|------|---------------|------------|
| Cellier & Kofman, *Continuous System Modeling* (2006) | Modeling philosophy, DAE systems, structural analysis, index concepts | 0, 3, 6, 7, 8 |
| Cellier & Kofman, *Continuous System Simulation* (2010) | Numerical integration, stiffness, discontinuity handling | 9, 10, 12 |
| Hairer & Wanner, *Solving ODEs II* (1996) | BDF, implicit Runge-Kutta, stiff systems, stability theory | 12 |
| Brenan, Campbell & Petzold (1996) | DAE theory, index, initialization, BDF for DAEs | 7, 9, 12 |
| Modelica Language Specification | Connection semantics, Appendix B DAE form, event semantics | 4, 5, 6, 10 |

---

*This tour is interactive: every stop has an "In HRW" block telling you
exactly what to click, what to look for, and what to compare. For the
deepest interactive experience, see the
[Structural Analysis guided tour](phase7_structural_analysis/guided-tour.md)
— a five-lesson walkthrough with animated algorithm replays and live-stepped
debugging. That tour is the model for how individual phase tours will evolve
as HRW's visualization capabilities grow.*
