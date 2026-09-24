# The Cartesian Pendulum as a DAE: From Equations to a Newton Step

**Purpose:** what a DAE is, why the Cartesian pendulum is one, and what a solver actually does with
it — fourteen steps from the physical system to a Newton step worked by hand, plus the context that
is usually missing.
**Status:** reference, and **incomplete by intent** — *"Connections to coursework"* grows as Doug's
courses reach the material, and Step 14 leaves the third iteration unworked.
**Read when:** learning what *index* means, before or alongside
[`fixture-labs/index-reduction.md`](fixture-labs/index-reduction.md), or when a lab's claim about
constraints needs the mathematics standing behind it.

*Written with Doug on 2026-09-24. **This file is the source of truth**; the claude.ai artifact is
published from it and is downstream. Edit here and republish — a second editable copy is how the
committed notebook went stale for 25 days with nothing in the toolchain able to notice
([`tech-debt.md`](tech-debt.md)).*

---

## Step 1: The physical system

A mass m hangs on a rigid rod of length L. The rod pivots at the origin. Gravity g pulls downward. Put the pendulum in ordinary x-y coordinates, with y pointing up, so the mass hangs at negative y.

The angle θ would be the natural coordinate, and it gives you a plain ODE with one equation and no constraint. We're deliberately not using it. Cartesian coordinates force a constraint into the problem, and the constraint is what makes this a DAE. That's the thing we want to study.

### Aside 1a: What a plain ODE is

A plain ODE is a system where every unknown has its own rate equation. Written out, it has the form z′ = f(z, t). Hand it the current state, and it hands back every rate directly. There is nothing to solve and no rule to enforce. The integrator just reads off the rates and steps forward.

A DAE breaks that pattern in two ways. Some unknowns, like λ, have no rate equation at all. Some equations, like the rod constraint, give no rate at all. You can't read off the rates. You have to solve for them, and the answer has to respect the constraint.

Step 6 shows this directly. For a plain ODE, the ∂F/∂z′ matrix can be made the identity: one derivative per row, one row per unknown. For the Cartesian pendulum, that matrix has a zero row and a zero column. That singularity is exactly "not a plain ODE."

### Aside 1b: Why the angle gives a plain ODE

The angle θ builds the rod into the coordinate itself:

```
x = L·sin θ
y = −L·cos θ
```

Pick any θ. The mass lands exactly L from the pivot. There is no value of θ that breaks the rod. So the constraint x² + y² = L² doesn't need to be stated as an equation. It is satisfied automatically.

Here's a mechanical picture. Cartesian coordinates are like tracking a train by its map position and adding the rule "stay on the track." The angle is like tracking the train by distance along the track. In the second description, leaving the track isn't forbidden. It simply can't be expressed.

The tension drops out too. The rod pulls straight along itself, and the mass moves along the circle, at right angles to the rod. A force perpendicular to the motion can't speed the mass up or slow it down along its path. So when you write Newton's law along the circle, only gravity appears:

```
θ′ = ω
ω′ = −(g/L)·sin θ
```

That is two unknowns with two rate equations, and each gives its rate directly. It's a plain ODE.

The pendulum has one physical degree of freedom. The angle description uses exactly one coordinate for it, plus its rate. The Cartesian description uses two position coordinates for one degree of freedom, so it needs an extra equation to remove the extra freedom. It also needs an extra unknown, λ, to enforce that equation. Those extras are what turn it into a DAE.

If you still want the tension, you can compute it afterward from θ and ω. It is an output, not something you have to solve for.

### Aside 1c: Why anyone uses the Cartesian form

For a pendulum, you wouldn't. But finding "track coordinates" by hand stops being practical in bigger systems: closed-loop linkages, robot arms touching things, and Modelica models built by connecting components. Each connection naturally produces a constraint. Tools like Modelica compilers therefore accept the DAE as written and deal with the constraint mechanically. That is the problem the rest of this walkthrough is building toward.

### Aside 1d: Deriving ω′ = −(g/L)·sin θ

The equation comes from Newton's law applied along the direction the mass can actually move. There are three ingredients.

**1. Which way the mass can move.** The mass is stuck on the circle. At any instant, it can only move along the tangent, the direction at right angles to the rod. So only forces along the tangent can change its speed.

**2. How much of gravity points along the tangent.** Gravity is mg, straight down. Measure θ from straight down. Then the angle between gravity and the rod is θ, and the part of gravity along the tangent is mg·sin θ.

Check the extremes. Hanging straight down, θ = 0: gravity points straight along the rod, the rod absorbs it all, and the tangential part is zero. Rod horizontal, θ = 90°: gravity points entirely along the tangent, so the full mg drives the motion. sin θ blends smoothly between those two cases.

The minus sign says the pull always points back toward the bottom. Swing right (θ positive), and gravity pushes left.

**3. How tangential acceleration relates to θ.** Distance along the arc is s = L·θ. So arc acceleration is L·θ″.

Now apply Newton's law along the tangent: mass times acceleration equals force.

```
m·L·θ″ = −m·g·sin θ
θ″ = −(g/L)·sin θ
```

The mass cancels, which is why a pendulum's swing doesn't depend on how heavy the bob is. Then split the second derivative into two first-order equations by naming ω = θ′. That gives θ′ = ω and ω′ = −(g/L)·sin θ.

#### The same result from the Cartesian equations

You can also get it mechanically from the equations in Step 3, which shows the two descriptions really are the same system.

Substitute x = L·sin θ and y = −L·cos θ, and differentiate twice:

```
x″ = L·(cos θ·θ″ − sin θ·θ′²)
y″ = L·(sin θ·θ″ + cos θ·θ′²)
```

The tangent direction is (cos θ, sin θ). "Project onto the tangent" means: multiply the x-equation by cos θ, multiply the y-equation by sin θ, and add.

On the acceleration side, the θ′² terms cancel, and cos²θ + sin²θ = 1 leaves L·θ″.

On the force side, the two λ terms cancel exactly. That's the algebra saying the rod force has no tangential component. Gravity leaves −g·sin θ.

The result is L·θ″ = −g·sin θ, the same equation.

That projection is the whole trick of minimal coordinates. You choose directions in which the constraint force has no component, so λ drops out and the constraint disappears with it.

## Step 2: The unknowns

There are five:

- x, y: position of the mass
- u, v: velocity of the mass (u = dx/dt, v = dy/dt)
- λ: the rod's pull, scaled. The rod's force on the mass points back toward the pivot, along −(x, y). λ is the size of that force per unit length. Physically, it's tension divided by L.

The first four appear with time derivatives. They are the differential states. λ never appears differentiated. It is the algebraic variable. Nothing tells you how λ changes over time. It is whatever it must be, at each instant, to keep the rod rigid.

### Aside 2a: Why λ is a scaled tension

It's because of the arrow λ is multiplied by.

The rod pulls the mass toward the pivot with tension T. As a vector, that force is T times a unit arrow pointing from the mass to the pivot. The position vector (x, y) points from the pivot to the mass, and its length is L. So the unit arrow toward the pivot is −(x, y)/L, and the force is:

```
F_rod = −T·(x, y)/L
```

The equations in Step 3 write the same force as −λ·(x, y). Match the two and you get λ = T/L.

The vector (x, y) is a ready-made direction arrow. It already points along the rod, but it's L long instead of 1 long. λ absorbs that length. Think of (x, y) as a ruler lying along the rod: λ says how hard to pull per unit of ruler.

**Why write it this way instead of using T directly?** It keeps the equations simple. Using T means dividing by L, or by √(x² + y²) if you don't want to assume the length. Using λ makes every equation a plain polynomial: products and sums, no division, no square roots. That keeps the Jacobian entries simple too. In Step 6, the λ column is just x and y.

**It's also how Lagrange multipliers come out.** In constrained mechanics, the constraint force is always a multiplier times the gradient of the constraint. The constraint here is x² + y² − L², and its gradient is (2x, 2y). That's the same direction, along the rod, just with a factor of 2. Textbooks differ on whether they fold in the 2, the mass, or both. So "λ" in one book may be twice or m times "λ" in another. The physics is identical; only the bookkeeping differs.

**A mechanical picture.** A force of −λ·(x, y) is exactly what a spring anchored at the pivot would exert if its rest length were zero and its stiffness were λ. λ even has spring units: force per length. So λ is a spring whose stiffness adjusts itself, instant by instant, to exactly the value that keeps the mass at distance L.

**The sign carries meaning.** Positive λ pulls inward, so the rod is in tension. That's what appears in Step 13. Negative λ pushes outward, so the rod is in compression. A rigid rod can do that. A string couldn't, which is why a string pendulum is a harder problem: the constraint switches off whenever the string goes slack.

## Step 3: The equations

Five unknowns need five equations:

```
x' = u                      velocity definition
y' = v                      velocity definition
m·u' = −λ·x                 Newton's law, x direction
m·v' = −λ·y − m·g           Newton's law, y direction
x² + y² = L²                the rod is rigid
```

The first four are ordinary dynamics. The fifth has no derivatives at all. It's a pure constraint: at every instant, the mass must be exactly L from the pivot.

## Step 4: Residual form

A solver wants every equation written as "something = 0." Move everything to one side:

```
r1 = x' − u
r2 = y' − v
r3 = m·u' + λ·x
r4 = m·v' + λ·y + m·g
r5 = x² + y² − L²
```

Think of each residual as a feeler gauge. Give it a guess for the unknowns, and it reports how big the gap is. Zero means that equation is satisfied. The solver's job is to adjust the guesses until all five gauges read zero at once.

This is exactly what your residual function computes: plug in current values, get back five gaps.

One thing to notice now, because it will matter later: λ appears in r3 and r4, but not in r5. The constraint equation doesn't mention the variable that's supposed to enforce it. That gap is the root of why this problem is hard.

## Step 5: How the integrator removes the derivatives

The residuals contain both unknowns (x, y, u, v, λ) and their derivatives (x′, y′, u′, v′). Newton's method can only solve for one set of unknowns. So the integrator gets rid of the derivatives by writing each one in terms of the unknowns.

The simplest way is backward Euler, the first-order BDF method. You are at time t_n with known values. You want the values one step h later. Replace each derivative with a difference over that step:

```
x′ ≈ (x − x_prev) / h
```

Here x is the new, unknown value and x_prev is the known old one. Do this for y, u, and v too.

After the substitution, the five residuals depend only on the five new unknowns. The problem is now five nonlinear equations in five unknowns, and it has to be solved once per time step. That is Newton's job.

The substitution has one consequence worth seeing clearly. If you nudge the new x, the implied x′ moves by 1/h times as much. The integrator has linked position and rate through a lever with ratio 1/h. That lever is where the 1/h in the iteration matrix comes from.

### Aside 5a: Newton's method

Newton's method solves "make these gauges read zero" by repeating one move. At the current guess, replace the curved problem with its straight-line approximation. Solve the straight-line problem exactly. Jump to that answer, and repeat.

It is the "calculus flattens the curve, linear algebra solves the flat problem" idea, run in a loop.

#### One unknown first

Say you want the z where f(z) = z² − 2 equals zero, which is √2.

Start with a guess, z = 1. There f = −1, so the gauge is off by −1. The slope there is f′ = 2z = 2. Near the guess, the curve looks like a straight line with that slope. A line with slope 2 that sits at −1 crosses zero 0.5 to the right. So jump to z = 1.5.

Repeat:

```
guess       f(z)       slope     step        new guess
1           −1         2         +0.5        1.5
1.5         +0.25      3         −0.0833     1.41667
1.41667     +0.00694   2.8333    −0.00245    1.414216
```

The true value is 1.4142136. The number of correct digits roughly doubles each round: 1, then 3, then 6. That's called quadratic convergence. The reason is simple. The error the straight line makes is proportional to the square of how far you jumped. Small jumps leave tiny errors.

#### Many unknowns

With five unknowns and five residuals, the idea is identical. Only the bookkeeping grows.

Each residual has a slope with respect to each unknown. Those slopes, stacked up, are the Jacobian: row i holds residual i's slopes, and column j holds the slopes with respect to unknown j. Together they give a linear model of how all five gauges respond to any combination of nudges:

```
change in r  ≈  J · Δ
```

You want the nudge that brings every gauge to zero at once. So set the predicted change equal to −r, and solve:

```
J · Δ = −r
```

That's a linear system: five equations and five unknown nudges. Linear algebra solves it exactly, usually by LU factorization. Add Δ to the guess, recompute r and J, and repeat.

Each residual alone is a curved surface over the five unknowns. Its linear model is a flat tangent plane. Solving J·Δ = −r finds the one point where all five tangent planes pass through zero simultaneously. That's what Step 11 does by hand.

#### What "one set of unknowns" means

The sentence above, "Newton's method can only solve for one set of unknowns," is loosely worded. The real requirement is that Newton needs a square system: as many equations as unknowns, with every equation written in terms of those same unknowns.

The raw pendulum residuals fail that test. They contain nine quantities: the five unknowns plus the four derivatives x′, y′, u′, v′. There are only five equations. Nine unknowns and five equations can't be solved; there are infinitely many answers.

The integrator supplies the missing link. Backward Euler says x′ = (x − x_prev)/h, and likewise for the others. That ties each derivative to its unknown, so the nine quantities collapse to five. Now it's five equations in five unknowns, and Newton can go to work.

#### When Newton struggles

Newton needs two things. First, a starting guess close enough that the straight-line model is trustworthy. Start too far away, and the tangent can point somewhere useless. That's one reason solvers use predictors (see Aside 9a). Second, a Jacobian that isn't singular or nearly singular. If J can't be solved cleanly, the jump is meaningless or enormous. The pendulum's index-3 structure pushes J toward that problem as h shrinks.

The modified Newton from Step 14 bends one rule. It reuses an old J instead of rebuilding it each round. The tangent planes are then slightly wrong, so convergence slows from digit-doubling to steady improvement. But each round is much cheaper.

## Step 6: The two partial-derivative matrices

Order the unknowns as columns: x, y, u, v, λ. The rows are the residuals r1 through r5.

First, how each residual responds to the derivatives:

```
∂F/∂z′      x′   y′   u′   v′   λ′
r1           1    0    0    0    0
r2           0    1    0    0    0
r3           0    0    m    0    0
r4           0    0    0    m    0
r5           0    0    0    0    0
```

Two things stand out. The last column is all zeros, because λ′ never appears. The last row is all zeros, because r5 has no derivatives. So this matrix is singular. That is the signature of a DAE. If this matrix were invertible, you could solve for the derivatives directly and you would have an ODE in disguise.

Second, how each residual responds to the unknowns themselves:

```
∂F/∂z       x    y    u    v    λ
r1           0    0   −1    0    0
r2           0    0    0   −1    0
r3           λ    0    0    0    x
r4           0    λ    0    0    y
r5          2x   2y    0    0    0
```

Read row r3 as an example. r3 = m·u′ + λ·x. Nudge x and r3 changes by λ. Nudge λ and r3 changes by x. Nothing else in the row matters.

## Step 7: The iteration matrix

Combine the two matrices through the 1/h lever:

```
J = ∂F/∂z + (1/h)·∂F/∂z′
```

```
J           x     y     u     v     λ
r1         1/h    0    −1     0     0
r2          0    1/h    0    −1     0
r3          λ     0    m/h    0     x
r4          0     λ     0    m/h    y
r5         2x    2y     0     0     0
```

This is the matrix Newton factors at each iteration. Rows are residuals. Columns are the new unknowns. Entry (i, j) answers the question: nudge unknown j, and how much does gauge i move?

The α/h I mentioned earlier generalizes this. Backward Euler has α = 1. Higher-order BDF methods use more past points, and α changes, but the structure stays the same.

One more foreshadowing note. Look at the r5 row and the λ column. The r5 row has a zero in the λ column, and the λ column has zeros in the u and v rows. λ reaches the constraint only indirectly: through r3 and r4 into u and v, then through r1 and r2 into x and y, and only then into r5. Two differential equations sit between the constraint and λ. For layered systems like this one, the index is that count plus one, so the pendulum is index 3. Aside 7a gives the official definition. This indirect path is also why this matrix gets badly conditioned as h shrinks.

### Aside 7a: What "index" officially means

#### The official definition

The standard version is the differentiation index. It is the minimum number of times you must differentiate the equations (all or some of them) with respect to time until you can solve for every derivative, including λ′, as a function of the unknowns. In other words, it counts the differentiations needed to reach a plain ODE.

On that scale, a plain ODE has index 0, since no differentiation is needed. An index-1 DAE is one differentiation away from an ODE.

#### Counting for the pendulum, step by step

Start with the rod constraint:

```
x² + y² − L² = 0
```

**First differentiation.** Differentiate with respect to time. L is a constant, so its derivative is zero. By the chain rule, the derivative of x² is 2x·x′, and likewise for y:

```
2x·x′ + 2y·y′ = 0
```

Substitute the velocity definitions x′ = u and y′ = v, then divide by 2:

```
x·u + y·v = 0
```

This is the velocity constraint from Aside 8a. It says the velocity is at right angles to the rod. λ still doesn't appear.

**Second differentiation.** Differentiate x·u + y·v = 0. Each term is a product, so use the product rule: the derivative of x·u is x′·u + x·u′.

```
x′·u + x·u′ + y′·v + y·v′ = 0
```

Substitute x′ = u and y′ = v:

```
u² + x·u′ + v² + y·v′ = 0
```

Now the accelerations u′ and v′ appear, and Newton's law supplies them. From Step 3, u′ = −λ·x/m and v′ = (−λ·y − m·g)/m. Substitute:

```
u² + v² − λ·x²/m − λ·y²/m − g·y = 0
```

Group the λ terms: −λ·(x² + y²)/m. The rod constraint says x² + y² = L², so:

```
u² + v² − λ·L²/m − g·y = 0
```

Multiply through by m:

```
m·(u² + v²) − λ·L² − m·g·y = 0
```

Now λ appears, and you can solve for it:

```
λ = m·(u² + v² − g·y) / L²
```

This is where the system becomes index 1. Every unknown is either a state with a rate equation or can be solved for algebraically.

**Third differentiation.** Differentiate the λ equation. m, g, and L are constants. The derivative of u² is 2u·u′, and the derivative of y is v:

```
λ′ = m·(2u·u′ + 2v·v′ − g·v) / L²
```

Substitute Newton's law for u′ and v′ again:

```
2u·u′ + 2v·v′ = −2λ·(x·u + y·v)/m − 2g·v
```

The velocity constraint says x·u + y·v = 0, so the λ term vanishes. What's left is −2g·v, and adding the −g·v already in the bracket gives −3g·v:

```
λ′ = −3·m·g·v / L²
```

Now every unknown has a rate equation. That's a plain ODE. Three differentiations reached it, so the index is 3.

#### Counting to "usable" instead of to ODE

A different count is also common: the differentiations needed to reach index 1, which is what a BDF solver can handle. That count is always one less than the index. For the pendulum, two differentiations make it usable. The third is unnecessary in practice, because BDF can solve for λ alongside the states without ever needing its rate.

Some authors formalize this count. Kunkel and Mehrmann's strangeness index measures the distance to a form that's solver-ready. For systems like this one, it equals the differentiation index minus one. So the "two differentiations to usable" count is essentially the pendulum's strangeness index. That is a real, established measure; it just isn't the one people mean when they say "index 3."

This is also what a Modelica compiler does. The Pantelides algorithm finds which equations to differentiate, and how many times, to reach index 1. For the pendulum, it differentiates the constraint twice. The dummy derivative method then chooses which variables to treat as algebraic, so the solver still enforces the original position constraint. That prevents the mass from slowly drifting off the circle, which would happen if you kept only the twice-differentiated version.

#### The chain picture

The chain in Step 7 is an intuition, not a definition. Between the constraint and λ sit two differential equations. x′ = u links position to velocity, and u′ = −λx/m links velocity to λ. Each differentiation of the constraint pulls in one more link. After two, λ appears, which is index 1. One more gives λ′, which is index 3 overall.

So the rule of thumb is: index = (differential equations between the constraint and its multiplier) + 1. That holds for systems with this layered structure, called Hessenberg form, which covers most constrained mechanical systems. It isn't a general definition, and it can mislead for other structures.

Other definitions exist too: perturbation index, tractability index, and the structural index used by Pantelides. They agree for well-behaved systems like the pendulum and can disagree in edge cases.

## Step 8: Pick numbers

Use round values so the arithmetic stays visible:

- m = 1, L = 1, g = 10 (not 9.81, to keep the arithmetic clean)
- step size h = 0.1, so 1/h = 10
- starting state: rod horizontal, mass at rest. x = 1, y = 0, u = 0, v = 0.

At rest with the rod horizontal, the rod carries no load, so λ = 0. The old state is fully known: (1, 0, 0, 0, 0).

The goal is the state one step later, at t = 0.1.

### Aside 8a: Where the starting state comes from

The old state (1, 0, 0, 0, 0) was chosen, but not all five values were free to choose.

The pendulum has one degree of freedom. That means only two things can be chosen: where the mass is along the circle, and how fast it's moving along the circle. The other three values are forced.

**Position.** Choose x, and x² + y² = L² fixes y, up to above or below the pivot.

**Velocity.** The velocity has to be tangent to the circle. If it had any component along the rod, the rod would be stretching. You get this by differentiating the constraint once:

```
x·u + y·v = 0
```

**λ.** Differentiate again. Then substitute Newton's law for u′ and v′:

```
u² + v² + x·u′ + y·v′ = 0
m·(u² + v²) − λ·L² − m·g·y = 0
λ = m·(u² + v² − g·y) / L²
```

At our start, the mass is at rest (u = v = 0) at the height of the pivot (y = 0), so λ = 0. That formula confirms what physical intuition says above: a horizontal rod holding a stationary mass carries no load.

Read the formula physically. The term u² + v² is the pull needed to curve the mass's path into a circle. The −g·y term is gravity's share along the rod, since y is negative when the mass is below the pivot.

A starting state that satisfies all three conditions is called consistent. If you hand a solver an inconsistent one, such as velocity not tangent to the circle or the wrong λ, the first step either jerks violently to fix it or Newton fails. Finding consistent values is its own problem, called consistent initialization. IDA has a routine for it, IDACalcIC. Modelica tools build and solve a separate initialization system from the model's `initial equation` sections and the `fixed = true` attributes.

The differentiated constraints are called hidden constraints. The model never states them, but every valid state must obey them. Notice the counting. Two differentiations of the rod constraint produced an equation that determines λ. One more would produce an equation for λ′. That count of three is exactly the index 3 from Step 7.

## Step 9: The first guess and its residuals

Newton needs a starting guess for the new unknowns. The simplest guess is "nothing changed": use the old values. So the guess is z₀ = (1, 0, 0, 0, 0).

Compute the implied derivatives with backward Euler. Every new value equals its old value, so all four derivatives are zero.

Now read the five gauges:

```
r1 = x′ − u               = 0 − 0          = 0
r2 = y′ − v               = 0 − 0          = 0
r3 = m·u′ + λ·x           = 0 + 0          = 0
r4 = m·v′ + λ·y + m·g     = 0 + 0 + 10     = 10
r5 = x² + y² − L²         = 1 + 0 − 1      = 0
```

Only r4 is off. It is saying: gravity is pulling, and nothing in your guess is responding. The guess has the mass floating.

### Aside 9a: Where Newton's first guess comes from

Newton needs somewhere to start before it can take its first jump. That starting point is z₀.

This step uses the simplest choice: assume nothing changed, so the guess for the new values is the old values. That works, but it's crude. It has the mass floating, and Newton has to discover gravity from scratch.

Real BDF solvers use a predictor. They keep the last few accepted states and fit a polynomial through them, then extrapolate that polynomial forward by h. If the mass has been swinging along a smooth arc, the predictor extends the arc. The guess lands very close to the answer, often close enough that Newton converges in one or two iterations.

On the very first step, there is only one past point, so there's nothing to extrapolate from. The solver falls back to something like our "nothing changed" guess, or to a straight-line guess using the initial derivatives if it knows them. That's also why variable-order BDF codes start at order 1 with a small step and build up order as history accumulates.

## Step 10: The Jacobian at the guess

Take the iteration matrix from Step 7 and substitute numbers into it. Two kinds of values go in.

**Constants, fixed for the whole step.** h = 0.1, so 1/h = 10. m = 1, so m/h = 1/0.1 = 10. These two happen to be equal only because m = 1. With m = 2, the m/h entries would be 20.

**The current guess.** From z₀: x = 1, y = 0, λ = 0. The guess also has u = 0 and v = 0, but those never appear in J. Every residual is linear in u and v, so their slopes are plain constants.

Here is every entry, with its formula from Step 7 and the substitution:

```
entry      formula    substitution    value
r1, x      1/h        1/0.1           10
r1, u      −1         constant        −1
r2, y      1/h        1/0.1           10
r2, v      −1         constant        −1
r3, x      λ          λ = 0            0
r3, u      m/h        1/0.1           10
r3, λ      x          x = 1            1
r4, y      λ          λ = 0            0
r4, v      m/h        1/0.1           10
r4, λ      y          y = 0            0
r5, x      2x         2·1              2
r5, y      2y         2·0              0
```

Every entry not listed is zero in Step 7's matrix and stays zero.

The result:

```
J           x     y     u     v     λ
r1         10     0    −1     0     0
r2          0    10     0    −1     0
r3          0     0    10     0     1
r4          0     0     0    10     0
r5          2     0     0     0     0
```

## Step 11: Solve for the correction

Newton says: find the nudge Δ that makes the linear model's gauges read zero. That is, solve J·Δ = −r.

Here −r = (0, 0, 0, −10, 0). Each row of the Step 10 matrix, multiplied by Δ = (Δx, Δy, Δu, Δv, Δλ), must equal that row's entry of −r.

The rows can't be solved top to bottom. r1 contains two unknowns, Δx and Δu, and neither is known yet. Instead, start with a row that has only one unknown, solve it, and substitute the result into the rows that remain. This is back substitution. Once the rows are put in the right order, the system is triangular, and each row yields exactly one new unknown.

In the order they must be solved:

- r4: 10·Δv = −10, so Δv = −1.
- r5: 2·Δx = 0, so Δx = 0.
- r1: 10·Δx − Δu = 0. With Δx = 0, this is −Δu = 0, so Δu = 0.
- r2: 10·Δy − Δv = 0. With Δv = −1, this is 10·Δy + 1 = 0, so Δy = −0.1.
- r3: 10·Δu + Δλ = 0. With Δu = 0, this is Δλ = 0.

Put back in unknown order, the correction is:

```
Δx = 0    Δy = −0.1    Δu = 0    Δv = −1    Δλ = 0
```

Finding that solve order is the same job a BLT sort does in a Modelica compiler. It reorders the equations so each one solves for exactly one new unknown, which turns the matrix triangular. Sparse linear solvers do something similar when they choose a pivot order.

This first iteration is lucky. The zeros from y = 0 and λ = 0 break every loop in the matrix, so pure back substitution works. Step 13 isn't so lucky.

The new guess is z₁ = (1, −0.1, 0, −1, 0).

Read it physically. The mass fell straight down. Velocity reached −g·h = −1, and position dropped 0.1. The rod did nothing.

Why? At a horizontal rod, the r5 row is (2, 0, 0, 0, 0). It is blind to vertical motion. That is true to first order: move a point on a circle's rightmost edge straight down, and its distance from the center barely changes at first. The linear model believes falling straight down keeps the rod length. So it sees no reason for λ to act.

## Step 12: Check the gauges again

At z₁, the implied derivatives are x′ = 0, y′ = −1, u′ = 0, v′ = −10.

```
r1 = 0 − 0                  = 0
r2 = −1 − (−1)              = 0
r3 = 0 + 0                  = 0
r4 = −10 + 0 + 10           = 0
r5 = 1 + 0.01 − 1           = 0.01
```

The worst gauge went from 10 to 0.01. What remains is exactly the curvature the linear model ignored. The mass is now 1.005 from the pivot, so the rod has "stretched."

## Step 13: The second iteration

Rebuild J at z₁ = (1, −0.1, 0, −1, 0). The constant entries (1/h, m/h, and −1) never change during a step, so only the entries that depend on the guess need rechecking:

```
entry      formula    substitution    value    changed?
r3, x      λ          λ = 0            0        no
r3, λ      x          x = 1            1        no
r4, y      λ          λ = 0            0        no
r4, λ      y          y = −0.1        −0.1      yes (was 0)
r5, x      2x         2·1              2        no
r5, y      2y         2·(−0.1)        −0.2      yes (was 0)
```

The first iteration moved y and v. v never appears in J, so only the two entries that depend on y change:

```
J           x     y     u     v     λ
r1         10     0    −1     0     0
r2          0    10     0    −1     0
r3          0     0    10     0     1
r4          0     0     0    10   −0.1
r5          2   −0.2    0     0     0
```

The right side is −r = (0, 0, 0, 0, −0.01).

This time, back substitution alone won't work. Every row now has at least two unknowns, so there's no row to start from. The two new nonzero entries closed a loop. Follow the rows: r5 links Δx and Δy, r2 links Δy and Δv, r4 links Δv and Δλ, r3 links Δλ and Δu, and r1 links Δu back to Δx. It's a cycle through all five unknowns.

The way out is to pick one unknown, treat it as if it were known, and express everything else in terms of it. Choose Δx. Walk around the loop:

- r1: 10·Δx − Δu = 0, so Δu = 10·Δx.
- r3: 10·Δu + Δλ = 0, so Δλ = −10·Δu = −100·Δx.
- r4: 10·Δv − 0.1·Δλ = 0, so Δv = 0.01·Δλ = −Δx.
- r2: 10·Δy − Δv = 0, so Δy = 0.1·Δv = −0.1·Δx.

One row is left, r5. Substitute Δy = −0.1·Δx into it:

```
2·Δx − 0.2·Δy        = −0.01
2·Δx − 0.2·(−0.1·Δx) = −0.01
2.02·Δx              = −0.01
Δx                   = −0.00495
```

Now back substitution finishes the job. Each expression above gives one unknown from Δx:

- Δu = 10·(−0.00495) = −0.0495
- Δλ = −100·(−0.00495) = +0.495
- Δv = −(−0.00495) = +0.00495
- Δy = −0.1·(−0.00495) = +0.000495

In unknown order, to four figures:

```
Δx = −0.00495    Δy = +0.000495    Δu = −0.0495    Δv = +0.00495    Δλ = +0.495
```

Choosing one unknown to break a loop is called tearing, and Δx here is the tearing variable. It's the same technique a Modelica compiler uses on algebraic loops, mentioned in Part II. The loop shrinks to one equation in one unknown, and everything else follows by substitution.

Now the rod has come alive. The r5 row can see y, so the solver knows the mass drifted off the circle. The only way to pull it back is through λ. λ acts on u and v through r3 and r4, and u and v act on x and y through r1 and r2. The mass is pulled slightly inward, toward the pivot, and λ turns positive: the rod is in tension.

Look at the size of Δλ compared to Δx: 100 times larger, which is 1/h². That ratio is the chain from Step 7 showing up in numbers. A small position error requires a λ correction amplified by 1/h at each link, two links before it reaches position. Shrink h, and this amplification grows. Keep that in mind for the index discussion.

## Step 14: Where it stands

After two iterations, the worst gauge is about 0.0025. The leftover comes from the products λ·x and λ·y in r3 and r4. Both factors changed at once, and the linear model can't capture a product of two changes. A third iteration would bring the residuals down to around 10⁻⁶. The solver then compares against its tolerance, accepts the step, and moves on to t = 0.2.

Two practical notes on what real solvers do differently. They usually start from a predictor that extrapolates from past steps, not from "nothing changed," so the first guess is much closer. And they often reuse one factored J across several iterations and even several steps. This is called modified Newton. It trades slower convergence for skipping the expensive refactorization.

---

# Part II: Context

These sections sit outside the main walkthrough. They don't advance the pendulum example, but they explain where it fits in the wider world.

## Why DAEs are rarely taught

Most engineering and computer science students take a numerical methods course that covers ODEs, and there are many books on solving ODEs. Few students are required to learn DAEs, and far fewer books cover them. Step 6 points at why this matters: if ∂F/∂z′ were invertible, the system would be an ODE in disguise. Real systems often aren't. There are four main reasons for the gap.

**History.** ODE numerics are old. Euler's method dates from the 1760s, and Runge-Kutta from around 1900. Serious DAE numerics started in 1971, when Bill Gear showed that BDF methods could handle algebraic equations directly. The workhorse code, Linda Petzold's DASSL, appeared in 1982. The theory of index and its consequences was worked out through the 1980s and 90s. Curricula were set long before the subject matured, and they haven't made room since.

**The textbook habit of reducing by hand.** Textbooks pick problems where a clever coordinate choice removes the constraint, like θ for the pendulum. Students learn to write ODEs, then solve ODEs. The DAE never appears because a human eliminated it before the numerics started. That works for textbook problems and breaks down for real systems, as Aside 1c describes.

**Tools that hide it.** Most engineers who solve DAEs every day don't know it. SPICE is a DAE solver: Kirchhoff's laws are algebraic constraints sitting next to capacitor and inductor rate equations. Chemical process simulators, multibody packages, and Modelica tools are DAE solvers too. A Modelica compiler does the reduction mechanically: matching, BLT sorting, index reduction, tearing. It usually hands the integrator something that is effectively an ODE with algebraic solves embedded. So the ODE-in-disguise idea is literally what those tools produce, and the user never sees the step.

**It's harder, and it builds on ODEs.** You can't learn DAE methods without first knowing implicit ODE methods, Newton, and stiffness. Then you add ideas with no ODE counterpart: index, hidden constraints, consistent initialization, and drift off the constraint. A lot of ODE methods also fail outright. Explicit methods like classic RK4 have no way to handle an equation with no rate in it, so you need implicit methods from the start. That makes it a second course, and most programs only have room for one.

The consequence is a mismatch. Industrial simulation is heavily DAE-based, while the curriculum is ODE-based. The people who understand the gap are mostly the ones who build the tools.

## Where DAEs show up in robotics

Robotics sits squarely inside this. Closed kinematic chains, legged robots with feet on the ground, and grasping all add constraints, and constrained multibody dynamics is a DAE problem. Most robotics courses either choose minimal coordinates or hand the problem to a physics engine. So a compiler-side view of DAEs is an uncommon angle in that field.

## Further reading

The books that exist are good, and there are only a handful:

- **Ascher & Petzold, *Computer Methods for Ordinary Differential Equations and Differential-Algebraic Equations* (SIAM, 1998).** It's the most readable. It builds DAEs on top of ODEs in one book.
- **Brenan, Campbell & Petzold, *Numerical Solution of Initial-Value Problems in Differential-Algebraic Equations* (SIAM).** This is the DASSL book, written from the solver side.
- **Hairer & Wanner, *Solving Ordinary Differential Equations II: Stiff and Differential-Algebraic Problems*.** It's the rigorous reference and dense.
- **Kunkel & Mehrmann, *Differential-Algebraic Equations: Analysis and Numerical Solution*.** It's mathematical and theory-first.
- **Cellier & Kofman, *Continuous System Simulation*.** It's written from the modeling and compiler side: sorting, tearing, and the Pantelides algorithm for index reduction. It's the closest to how a Modelica compiler builder thinks.

## Connections to coursework

This section tracks places where ideas from courses show up directly in the walkthrough. New connections get added as they come up.

### Graduate linear algebra (Purdue, Fall 2026)

**Rank and null space.** ∂F/∂z′ in Step 6 is 5×5 with rank 4. Its null space is the λ direction: nudge λ′ and nothing responds, because λ′ appears nowhere. Its left null space is the r5 direction: no combination of derivative nudges moves the constraint gauge. The rank deficiency, one, equals the number of constraint equations. For a DAE, the gap between size and rank counts the algebraic part.

**A singular matrix inside an invertible one.** Step 7's J is ∂F/∂z plus (1/h) times the singular ∂F/∂z′. Adding a well-chosen second matrix repairs the missing rank, so J is invertible even though one of its parts isn't. A pair of matrices combined as A + s·B is called a matrix pencil, the same object that appears in generalized eigenvalue problems. For linear DAEs, the index can be read directly from the pencil's Kronecker canonical form.

**Conditioning.** Invertible isn't the same as well-behaved. Step 13 shows Δλ amplified by 1/h² relative to Δx. For index-3 problems, J's condition number grows roughly like 1/h³ as h shrinks. The matrix stays invertible in exact arithmetic but becomes numerically fragile. Singular values make this precise: the smallest one heads toward zero.

**Factorization.** Every Newton iteration in Aside 5a solves J·Δ = −r, and solvers do it with LU factorization. Modified Newton works by reusing a factorization, and sparse LU on huge Jacobians is where much of a real simulator's time goes.
