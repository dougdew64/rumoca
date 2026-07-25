# End-to-End Guided Tour: From Modelica Model to Running Simulation

*The spine of the HRW curriculum — an interactive walk-through of what a Modelica
compiler must do and why, grounded in the GearWithBrake specimen and driven by
HRW's stage views.*

**Specimen:** `GearWithBrake.mo`
([source](../../specimens/GearWithBrake.mo))
— a geared oscillator with an automatic speed-limiting brake (MSL rotational
components, index > 1, discrete events, stiff dynamics).
Load it in HRW: hrw://load/GearWithBrake

**Prerequisites:** HRW built and running (`cargo run -p hrw` from the workspace
root). Click the load link above to open `GearWithBrake` — it compiles through
all stages automatically. This tour complements the textbooks listed in
[`vision.md`](../vision.md). It references specific chapters and sections;
consult the originals for proofs and formal development. This tour provides
what the books cannot: a concrete specimen and a real compiler to ground
the theory.

**Note on simulation (2026-07-24):** Rumoca's simulator is not yet reliable
enough for production use. The compiler pipeline (Stops 1–11) is solid; the
Simulation stop (12) shows what the solver *attempts*, but for ground-truth
trajectories, simulate specimens in Wolfram System Modeler.

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

Open `GearWithBrake.mo`. Read it as an engineer would — it describes a
physical system:

```
Motor (constant torque)
  → Rotor (small inertia, J = 0.01)
    → Ideal gear (ratio 5:1)
      → Load (large inertia, J = 0.5)
        → Spring-damper (c = 100, d = 2) anchored to a fixed frame

When the load spins too fast (> 5 rad/s), a brake engages.
When it slows down (< 2.5 rad/s), the brake releases.
```

The model is 69 lines of Modelica. It uses seven MSL rotational components
connected by `connect()` statements, plus a `when`/`elsewhen` clause for
brake logic and an `if` expression for braking torque direction.

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

GearWithBrake provides none of these directly. It provides:

- A hierarchy of MSL classes (ConstantTorque, Inertia, IdealGear, …)
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

**What happens to GearWithBrake:** The parser reads the 69 lines and produces
a tree of nodes: one `ClassDefinition` for `GearWithBrake`, containing 10
component declarations, 7 `connect` equations, a `when`/`elsewhen` clause, an
`if` expression, and an `annotation`. At this point every identifier is just a
string — `Modelica.Mechanics.Rotational.Components.Inertia` is a name, not a
resolved reference to any specific class.

**The mathematical form:** None yet. This is pure syntax — the tree captures
the *shape* of the source text, not its meaning.

**What's insufficient:** The AST is a faithful mirror of what the programmer
typed, but it knows nothing about what the names mean. `rotor.flange_b`
could be anything — a variable, a class, an error. The type system hasn't
been consulted; no equation has been expanded; nothing has been checked.

**In HRW:** Click the **Parse** tab (hrw://Parse). The JSON tree shows the raw
AST — expand `classes → GearWithBrake → equations`
(hrw://Parse/classes/GearWithBrake/equations)
to find the 7 `connect` nodes and the `when` clause. Click any node to inspect
it; notice that identifiers like `Modelica.Mechanics.Rotational.Components.Inertia`
are plain strings with no `def_id` — resolution hasn't happened yet.

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

**What happens to GearWithBrake:** The resolver annotates every component
reference with its definition. `motor` (def_id 89) is an instance of
`ConstantTorque` (type_def_id 28206). `rotor` (def_id 90) is an instance
of `Inertia` (type_def_id 27586). The `connect(motor.flange, rotor.flange_a)`
statement's two arguments are now linked to specific port definitions inside
those MSL classes.

**The mathematical form:** Still none. Resolution establishes the *semantic
graph* — who refers to whom — but does not yet produce equations.

**What's insufficient:** Names are resolved, but the MSL component internals
are still opaque. We know that `rotor` is an `Inertia`, but we haven't
expanded what that means — what variables it declares, what equations it
contributes, what parameter values it carries. The model is still a
hierarchical description, not a flat system of equations.

**In HRW:** Click the **Resolve** tab (hrw://Resolve). The tree now has
`def_id` and `type_def_id` annotations on every identifier — hover one to see
the resolved class name (e.g. `type_def_id: 27586 → model Modelica.Mechanics
.Rotational.Components.Inertia`). Expand `components`
(hrw://Resolve/components) to see the 10 component declarations
with their resolved type references. Right-click a `type_def_id` →
**"↪ Go to Inertia"** to navigate into the MSL class and read its internal
structure. Use **← Back** to return.

> **Phase tour:** [Resolve and Scope](phase2_resolve_and_scope/resolve_and_scope.md)

---

## Stop 3: Instantiation — Hierarchy to Instances

**The problem:** The model uses seven MSL components, each of which is itself
a class with internal variables, equations, parameters, and possibly further
sub-components. The `Inertia` class declares `phi`, `w`, `a`,
`flange_a.phi`, `flange_a.tau`, `flange_b.phi`, `flange_b.tau`, and the
equation `J * a = flange_a.tau + flange_b.tau` — none of this is visible yet.

**The solution:** Instantiate every component: apply the parameter
modifications from the top-level model (`J = 0.01`, `ratio = 5`, `c = 100`,
…) and recursively expand each component's internal structure.

**What happens to GearWithBrake:** The instance tree grows from 10
declarations to hundreds of entries. Each MSL component unfolds: `rotor`
brings `rotor.J`, `rotor.phi`, `rotor.w`, `rotor.a`, `rotor.flange_a.phi`,
`rotor.flange_a.tau`, `rotor.flange_b.phi`, `rotor.flange_b.tau`. The gear
brings its `ratio`, `phi_a`, `phi_b`, `phi_support`, constraint equations.
The spring-damper brings `phi_rel`, `w_rel`, `a_rel`, `tau`, `c`, `d`,
`phi_rel0`, `tau_c`, `tau_d`, `lossPower`.

Every parameter modification is applied: `rotor.J = 0.01` (from the
top-level `Inertia rotor(J = 0.01)` declaration) overrides the MSL default.

**The mathematical form:** Still no equations in usable form — but the raw
material is now all present. Every variable that will eventually appear in
the equation system is now declared somewhere in the instance tree.

**In HRW:** Click the **Instantiate** tab (hrw://Instantiate). The tree is
much larger — expand `components` (hrw://Instantiate/components) to see the
fully instantiated component hierarchy. Each MSL component has been expanded
with its internal variables, equations, and parameter modifications applied.

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

**What happens to GearWithBrake:** Every variable receives its resolved
type. `rotor.phi` is `Angle` (rad), `rotor.w` is `AngularVelocity` (rad/s),
`rotor.J` is `MomentOfInertia` (kg·m²). The equation `J * a = flange_a.tau
+ flange_b.tau` type-checks: `MomentOfInertia × AngularAcceleration =
Torque + Torque`. The brake parameters `maxSpeed` and `brakeForce` are
`Real` (no SI annotation in the user model, but dimensionally consistent
within their usage context).

**The mathematical form:** Still hierarchical, still no global equation
system. But the semantic contract is now verified — every expression is
well-typed, every array dimension is resolved, every variability constraint
is satisfied.

**In HRW:** Click the **Typecheck** tab (hrw://Typecheck). The tree is
structurally similar to Instantiate, but every expression now carries resolved
type information. Look for `type_specifier` fields on variable declarations.

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

**What happens to GearWithBrake:** The seven `connect()` statements produce
dozens of equations:

- **Potential equalities.** `connect(motor.flange, rotor.flange_a)` generates
  `0 = motor.flange.phi - rotor.flange_a.phi` — the two ports share the
  same angular position (Kirchhoff's voltage-law analogue for rotational
  mechanics).

- **Flow-sum equations.** At every connection node, the flow variables
  (torques) must sum to zero: `0 = motor.flange.tau + rotor.flange_a.tau`.
  This is Kirchhoff's current-law analogue — torques balance at a junction,
  just as currents balance at a node.

- **Component equations** from each MSL class are included with qualified
  names. The inertia's `J * a = flange_a.tau + flange_b.tau` becomes
  `0 = rotor.J * rotor.a - (rotor.flange_a.tau + rotor.flange_b.tau)` in
  residual form.

The flat model has 60+ variables and 44+ equations, all in **residual form**:
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

**In HRW:** Click the **Flatten** tab (hrw://Flatten). You have three sub-views:

1. **Equations** — the equation sheet. This is the single biggest upgrade over
   the JSON tree: all 44 equations rendered in readable mathematical notation,
   grouped by origin (component equations, connect-generated equalities,
   connect-generated flow sums, initial equations). Read them as a mathematician
   would — these are the equations the solver will actually see. The variable
   classification table at the top lists every variable with its role (state,
   algebraic, parameter), start value, and unit.

2. **Source Map** — the source-to-equation traceability view. The left pane shows
   `GearWithBrake.mo` source code; the right pane shows the equation sheet.
   **Click a source line** (e.g. line 50, `connect(load.flange_b,
   spring.flange_a)`) → the equations it generated highlight in the right pane.
   **Click an equation** → the source line(s) that produced it highlight in the
   left pane. This is the bridge between the OO world the engineer wrote and the
   flat equation world the compiler produced.

3. **Tree** — the raw JSON tree (the original view, still available for deep
   inspection).

**What's insufficient:** The flat system is correct but unstructured. We have
a soup of 44 equations and 60+ variables. We don't know which variables are
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

**What happens to GearWithBrake:** Every variable is classified:

| Partition | Symbol | GearWithBrake examples | Count |
|-----------|--------|------------------------|-------|
| States | x(t) | `motor.phi`, `rotor.phi`, `rotor.w`, `load.phi`, `load.w`, `spring.phi_rel`, `spring.w_rel` | 7 |
| Derivatives | ẋ(t) | `der(spring.phi_rel)`, `der(spring.w_rel)`, … | 7 |
| Algebraics | y(t) | `motor.flange.tau`, `gear.flange_a.tau`, `spring.a_rel`, `load.flange_a.tau`, … | many |
| Parameters | p | `rotor.J`, `gear.ratio`, `spring.c`, `spring.d`, `maxSpeed`, `brakeForce` | many |
| Discrete-valued | m(tₑ) | `braking` (Boolean) | 1 |

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
braking := if (load.w > maxSpeed AND NOT pre(load.w > maxSpeed)) then true
           elseif (load.w < 0.5*maxSpeed AND NOT pre(…)) then false
           else pre(braking)
```

This goes into f_m. The brake conditions (`load.w > maxSpeed`, `load.w <
0.5*maxSpeed`) are extracted as zero-crossing relations for the event
detector.

**The mathematical form:** The canonical hybrid DAE of MLS Appendix B.
This is the form that Cellier, Hairer & Wanner, and Brenan, Campbell &
Petzold analyze. For the first time, the system is in a standard notation
that a textbook reader would recognize.

**In HRW:** Click the **Flatten** tab (hrw://Flatten). The equation sheet now shows the classified
system — equations grouped into the four MLS Appendix B partitions (continuous
f_x, discrete real f_z, discrete-valued f_m, conditions f_c). The variable
table shows the partition column (state, algebraic, parameter, discrete).
Compare this with the Flatten tab's equation sheet: the equations are the same,
but now classified.

**What's insufficient:** The DAE is in standard form, but its **differential
index may be greater than 1**. The ideal gear constraint (`phi_a = ratio *
phi_b`) is a position-level algebraic equation that couples state variables.
Standard DAE solvers — BDF, ESDIRK, TR-BDF2 — require an index-1 system
where every state has a computable derivative at each time step. As written,
five of the seven states have no usable derivative equation.

> **Phase tour:** [DAE Construction](phase6_dae_construction/dae_construction.md)
>
> **MLS Appendix B:** The formal definition of the hybrid DAE form. This is
> the mathematical contract between the Modelica language and its solvers.
>
> **Cellier & Kofman, *CSM*, Ch. 9:** DAE systems and the challenges they
> pose for numerical methods. The index concept is introduced here.

---

## Stop 7: Index Reduction — Eliminating Hidden Constraints

**The problem:** GearWithBrake has 7 state variables but only 2 independent
degrees of freedom. The ideal gear constraint `phi_a = ratio * phi_b`
(position-level) plus the connect-generated equalities chain the five
rotational positions together. Five states are redundant — their values are
determined by the constraint, not by independent differential equations.

A BDF integrator trying to advance `motor.phi` would look for an equation
giving `der(motor.phi)` and find... nothing. The position is constrained by
the gear ratio, not by a force balance. The integrator gets stuck.

This is the **high-index problem** — the DAE's differential index is greater
than 1 because algebraic constraints implicitly determine state derivatives.

**The solution:** Identify constrained states and demote them to algebraic
variables, eliminating the redundant degrees of freedom. If necessary,
differentiate algebraic constraints with respect to time to manufacture the
missing derivative equations.

**What happens to GearWithBrake:** The index reduction pipeline runs 10
steps:

1. **Constrained dummy derivative demotion** demotes 5 states. The gear
   constraint couples `motor.phi`, `rotor.phi`, `load.phi` through the gear
   ratio, and their velocities through its derivative. After identifying
   these constraints, the reducer demotes `motor.phi`, `rotor.phi`,
   `rotor.w`, `load.phi`, and `load.w` from states to algebraic variables
   (or eliminates them outright).

2. **Trivial elimination** removes 33 variables whose values are determined
   by a single equation. For example:
   - `motor.phi_support → 0.0` (fixed support)
   - `motor.tau → motor.tau_constant` (constant torque source)
   - `rotor.w → -spring.w_rel * 0.2 / 0.04` (velocity through gear ratio)
   - `motor.flange.phi → rotor.phi` (connection equality)
   - `spring.tau → c * (phi_rel - phi_rel0) + d * w_rel` (constitutive law)

The result: **7 states → 2 states** (`spring.phi_rel` and `spring.w_rel`).
The original 44-equation system collapses to 11 equations and 11 unknowns.

These two surviving states have clear physical meaning. `spring.phi_rel` is
the angular deflection of the spring, and `spring.w_rel` is its rate of
change. They are the system's two true degrees of freedom — one spring
storing energy as position, one as velocity. Everything else is determined
by the constraints.

**The mathematical form:** An index-1 DAE — every remaining state has a
usable derivative equation. The system's two ODE-like equations are:

```
der(spring.phi_rel) = spring.w_rel
der(spring.w_rel)   = spring.a_rel    (computed from force balance)
```

The remaining 9 equations determine the algebraic unknowns: the internal
torques (`motor.flange.tau`, `gear.flange_a.tau`, `gear.flange_b.tau`,
`load.flange_a.tau`, `rotor.flange_b.tau`), the braking torque
(`brakeTorque.tau`), the angular acceleration (`spring.a_rel`), load speed
(`load.w`), and a boundary position (`spring.flange_b.phi`).

**In HRW:** Click the **Index Reduction** tab (hrw://Index_Reduction). The reduction report shows the
10-step pipeline: constrained dummy derivative demotion (5 states demoted),
trivial eliminations (33 variables removed), and the final equation count
(11 equations, 11 unknowns). The equation sheet here shows the *reduced*
system — compare it with the DAE tab to see which equations and variables
were eliminated.

**What's insufficient:** We have 11 equations and 11 unknowns. But in what
order do we evaluate them? Some equations depend on the results of others.
And there may be circular dependencies — algebraic loops — where equations
cannot be ordered at all and must be solved simultaneously. Without a
solution order, a solver would have to solve all 11 equations jointly at
every time step, forming and factoring an 11×11 Jacobian. For this small
system that's feasible, but for industrial models with thousands of
equations, it's prohibitively expensive.

> **Phase tour:**
> [Index Reduction and State Demotion](phase6_dae_construction/index_reduction.md)
>
> **Cellier & Kofman, *CSM*, Ch. 9:** Index reduction and the Pantelides
> algorithm. The GearWithBrake demonstrates the same fundamental issue
> Cellier discusses: position-level algebraic constraints that couple states,
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

**What happens to GearWithBrake:**

**Step 1 — Incidence matrix.** The 11×11 matrix records which unknowns
appear in which equations. Crucially, the *columns* are the unknowns the
solver must determine: state derivatives (`der(spring.phi_rel)`,
`der(spring.w_rel)`) and algebraic variables (`spring.a_rel`,
`brakeTorque.tau`, `motor.flange.tau`, `rotor.flange_b.tau`,
`gear.flange_a.tau`, `gear.flange_b.tau`, `load.flange_a.tau`,
`spring.flange_b.phi`, `load.w`). State *values* are not columns — they
are known inputs to the integrator at each step.

**Step 2 — Maximum matching.** Kuhn's augmenting-path algorithm pairs each
equation with exactly one unknown. The matching is *perfect* (all 11
equations matched) — confirming the system is structurally non-singular
after index reduction. Example pairings:

| Equation | Matched unknown |
|----------|-----------------|
| motor torque balance | `motor.flange.tau` |
| rotor Newton's 2nd law | `spring.a_rel` |
| gear ratio constraint | `gear.flange_a.tau` |
| load Newton's 2nd law | `load.w` |
| spring kinematics | `der(spring.phi_rel)` |
| spring dynamics | `der(spring.w_rel)` |
| brake torque | `brakeTorque.tau` |

**Step 3 — Dependency graph and Tarjan's SCCs.** From the matching, build
a directed graph: equation A depends on equation B if A references the
variable that B is matched to. Run Tarjan's algorithm to find strongly
connected components — cycles where equations depend on each other
mutually.

GearWithBrake has **one algebraic loop** of size 5: the gear's force
balance, the flow-sum equations at the rotor-gear and gear-load junctions,
and the rotor and load Newton's-law equations are mutually coupled. The
constraint torques `load.flange_a.tau`, `gear.flange_b.tau`,
`gear.flange_a.tau`, `rotor.flange_b.tau`, and `spring.a_rel` form a
circular dependency — you can't compute any one without the others.

**Step 4 — Tearing.** The 5×5 algebraic loop is expensive to solve naively
(5×5 Jacobian every step). Cellier tearing identifies `load.flange_a.tau`
as a single **tear variable** — if we guess its value, the other four can
be computed one at a time in sequence. Only one variable needs to be
iterated, and a single residual equation checks convergence. The cost
drops from 5×5 to 1×1 iteration.

**Step 5 — BLT assembly.** The final evaluation plan has 7 blocks:

```
Block 1:  Scalar — motor.flange.tau         (from motor torque balance)
Block 2:  Scalar — brakeTorque.tau           (from brake expression)
Block 3:  Coupled, size 5 — the gear torque loop (torn: iterate on
          load.flange_a.tau, compute gear.flange_b.tau → spring.a_rel →
          gear.flange_a.tau → rotor.flange_b.tau, residual check)
Block 4:  Scalar — load.w                   (from load equation)
Block 5:  Scalar — der(spring.phi_rel)       (= spring.w_rel)
Block 6:  Scalar — der(spring.w_rel)         (= spring.a_rel)
Block 7:  Scalar — spring.flange_b.phi       (boundary)
```

The solver evaluates these blocks **in order**, top to bottom. Each scalar
block is one assignment; the coupled block is a small Newton iteration.
This is far cheaper than solving 11 equations jointly.

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

**In HRW:** Click the **Structural** tab (hrw://Structural) — this is HRW's richest view. Four
sub-views:

1. **Incidence** — the 11×11 incidence matrix with the matching overlay (green
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
   the SCC pops. The spy-plot view shows the final block structure.

4. **Spy Plot** — the BLT spy plot: the block-diagonal structure of the sorted
   system. The one coupled block (size 5) is visible as a filled square on the
   diagonal.

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

For GearWithBrake, the states have `start` values (typically 0), but the
algebraic unknowns — the internal torques, the angular acceleration — must
be computed from these start values by solving the algebraic equations.

**The solution:** Build an **IC (initial condition) plan** — a precomputed
recipe that tells the runtime exactly how to compute consistent initial
values. The IC plan reuses the matching/BLT/tearing machinery from
structural analysis, but applied to the algebraic-only subsystem (treating
states as known constants at their start values).

**What happens to GearWithBrake:** Rumoca constructs an IC plan that
sequences the algebraic variable initialization. The plan assigns each
algebraic unknown to a block type:

- **ScalarDirect:** the equation can be symbolically rearranged for the
  unknown — cheapest, just evaluate an expression.
- **ScalarNewton:** no symbolic solution, use single-variable Newton
  iteration.
- **TornBlock:** an algebraic loop, reduced by tearing.
- **CoupledLM:** a loop where tearing fails — full Levenberg-Marquardt.

For GearWithBrake, at t = 0 with all start values at zero:
`spring.phi_rel = 0`, `spring.w_rel = 0`, so `spring.a_rel = 0` (no spring
force, no damping force), and the torques cascade through the gear train.

**Note:** GearWithBrake's initialization currently fails in Rumoca (33/37
matched — a structurally singular init subsystem caused by MSL support-flange
variables). This is a known Rumoca limitation, not a fundamental problem. The
model still simulates because the solver falls back to a relaxed IC solve.
This failure is itself educational: it shows that initialization is a hard
problem in practice, not just in theory.

**In HRW:** Click the **Initialization** tab (hrw://Initialization). The determinacy view shows
the IC plan's matching result — how many of the initialization unknowns were
matched (33/37 for GearWithBrake). The unmatched variables are the ones
causing the structural singularity. For a simpler example, load `RcCircuit`
to see a fully matched IC plan with ScalarDirect and TornBlock assignments.

**The mathematical form:** A nonlinear system F(y) = 0 at t = 0, where
the states x(0) are fixed and the algebraics y(0) are the unknowns.
This is a **root-finding problem**, typically solved by Newton's method or
Levenberg-Marquardt.

**What's insufficient:** We can now start the integrator. But what happens
when `load.w` crosses 5 rad/s and the brake engages? The continuous
equations change discontinuously — the braking torque jumps from 0 to
-20 N·m. A standard integrator, which assumes smooth dynamics, will produce
garbage if it steps blindly across this discontinuity.

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

**The problem:** GearWithBrake has discontinuous dynamics. The `when`
clause switches `braking` from false to true when `load.w > maxSpeed`,
and back to false when `load.w < 0.5 * maxSpeed`. At the switching instant,
`brakeTorque.tau` jumps from 0 to ±20 N·m. A numerical integrator that
steps across this discontinuity without detecting it will lose accuracy
catastrophically — the integrator's error estimator assumes smooth
derivatives, and a jump invalidates that assumption.

**The solution:** Extract the relational conditions as **zero-crossing
functions** and use an event-detection mechanism to locate the exact
instants when conditions change. At each event:

1. The integrator stops.
2. Discrete variables are updated (the `f_m` equations fire).
3. The continuous equations are re-evaluated with the new discrete state.
4. The integrator restarts with fresh initial conditions.

**What happens to GearWithBrake:** The event structure has 4 conditions and
1 discrete-valued update:

| Condition | Expression | Purpose |
|-----------|------------|---------|
| c[1] | `braking` | Current brake state |
| c[2] | `load.w > 0` | Sign of load velocity (for torque direction) |
| c[3] | `load.w > maxSpeed` | Brake engagement trigger |
| c[4] | `load.w < maxSpeed * 0.5` | Brake release trigger |

The discrete update is the expanded `when`/`elsewhen`:

```
braking := if (c[3] AND NOT pre(c[3])) then true
           elseif (c[4] AND NOT pre(c[4])) then false
           else pre(braking)
```

The zero-crossing functions `load.w - maxSpeed` and `load.w - 0.5*maxSpeed`
guide the event locator. The solver uses bisection (or a faster root-finding
method) on these functions to pinpoint the crossing time to within the
specified tolerance.

**The mathematical form:** A **hybrid automaton** — continuous dynamics
between events, with discrete transitions triggered by zero-crossings of
guard functions. The MLS Appendix B formalization captures this as the
interplay of f_x (continuous), f_m (discrete updates), and f_c (conditions).

**In HRW:** Click the **Events** tab (hrw://Events). The event listing shows the 4 conditions
and the discrete-valued update for `braking`. For contrast, load `SingleInertia`
(a smooth model) — the Events tab shows "no events," confirming that the event
structure is specific to hybrid models.

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

**What happens to GearWithBrake:** The 49 variables are assigned specific
slots in a flat state vector:

- `motor.phi → Y[0]`, `rotor.phi → Y[1]`, …, `spring.phi_rel → Y[5]`,
  `spring.w_rel → Y[6]` (states and their derivatives)
- Algebraic unknowns fill the next slots
- `braking → Y[44]`, conditions `c[1]–c[4] → Y[45]–Y[48]`

The continuous residual `F(t, y, ẏ) = 0` is compiled into a `ComputeBlock`
— a sequence of operations the runtime dispatches. A forward-mode automatic
differentiation pass produces the Jacobian `∂F/∂y` symbolically during
lowering, so the solver never approximates it with finite differences.

**The mathematical form:** The same DAE, but encoded as a dispatch-ready
compute graph rather than symbolic expression trees. The SolveProblem is
schema-versioned and serializable — it can cross process boundaries (JSON,
binary) for codegen targets.

**In HRW:** Click the **Solve Lowering** tab (hrw://Solve_Lowering). The JSON tree shows the
SolveProblem: expand `variable_layout` to see the flat slot assignments
(Y[0] through Y[48]), `compute_blocks` for the compiled residual operations,
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
stiff systems like GearWithBrake) with event detection.

**What happens to GearWithBrake:**

1. **Initialization.** The IC plan solves for consistent algebraic values
   at t = 0. All initial velocities are zero, the spring is undeflected,
   the brake is off.

2. **Time stepping.** The BDF integrator advances the two true states
   (`spring.phi_rel`, `spring.w_rel`). At each step it evaluates the
   residual F(t, y, ẏ) = 0 and the Jacobian ∂F/∂y, then solves the
   resulting nonlinear system via Newton's method. The BLT ordering from
   structural analysis makes this efficient — the 5-equation algebraic loop
   is solved with 1-variable tearing, not 5×5 factorization.

3. **Event detection.** The solver monitors the zero-crossing functions
   `load.w - maxSpeed` and `load.w - 0.5*maxSpeed`. When a sign change
   is detected between steps, the solver bisects to find the crossing time,
   updates the discrete state (`braking`), re-initializes, and restarts
   integration.

4. **The physics.** The motor accelerates the system through the gear.
   The load speeds up, oscillating due to the spring. When `load.w`
   exceeds 5 rad/s, the brake engages (-20 N·m opposing motion). The load
   decelerates. When it drops below 2.5 rad/s, the brake releases. This
   produces a limit cycle with discrete braking events — exactly the
   behavior a BDF solver with event detection is designed to handle.

**Result:** 49 variable trajectories over 501 time points (t = 0 to 2.0),
with discontinuity segments at the braking events.

**In HRW:** Click the **Simulation** tab (hrw://Simulation) and press **▶ Run**. Two plots appear:

1. **Trajectory plot** — state variables vs time. Look for the limit cycle:
   the load accelerates, the brake engages (velocity drops), the brake
   releases, the load re-accelerates. Discontinuity segments (vertical
   breaks in the line) mark the braking events.

2. **Solver diagnostics** (below the trajectory) — step size h(t) and BDF
   order k(t) vs time, with a synchronized time axis. Look for step-size
   shrinkage at braking events (the solver tightens its steps near
   discontinuities) and order changes (the BDF order drops back to 1 after
   a restart and climbs as the solver gains confidence in the smooth
   inter-event interval).

**Caveat (2026-07-24):** Rumoca's simulator is not yet reliable — the
trajectory may differ from the true solution. For ground-truth results,
simulate GearWithBrake in Wolfram System Modeler with identical solver
tolerances and initial conditions. The diagnostics plot remains valuable
for studying what the solver *does*, even when the results are imperfect.

**The mathematical form:** Numerical integration of the index-1 DAE with
event handling. The BDF method is an implicit multistep method — at each
step it solves a nonlinear system of the form:

```
G(yₙ) = F(tₙ, yₙ, (yₙ - Σ αⱼyₙ₋ⱼ) / (hβ₀)) = 0
```

where αⱼ and β₀ are the BDF coefficients and h is the step size. For stiff
systems (and GearWithBrake is stiff — the spring constant of 100 with a
small rotor inertia of 0.01 creates a wide spread of eigenvalue magnitudes),
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
([`GearWithBrake trace`](../specimen-notebook/GearWithBrake/trace/))
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
