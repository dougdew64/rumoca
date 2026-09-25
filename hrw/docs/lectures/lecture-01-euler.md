# Lecture 1 — Euler, 1768: getting a number when there is no formula

**Purpose:** the first rung of [`specimen-ladder.md`](../specimen-ladder.md). What problem numerical
integration solves, the single move Euler made, and what Rumoca does with it.
**Status:** lecture — a draft to be argued with. **Ask, and this file gets revised.**
**Read in VS Code's markdown preview**, which renders the math ($\LaTeX$ via KaTeX, on by default
as `markdown.math.enabled`). If the formulas below appear as literal `$` signs, that setting is off.
**Specimen:** `SingleInertia`. **Measured against Rumoca 0.9.20 on 2026-09-24.**

> **A notation convention, because two things share every name here.** Italic math — $\varphi$,
> $\omega$, $\tau$ — means the *mathematical* quantity. `Backticks` — `phi`, `w`, `tau` — mean the
> Modelica identifier and the number Rumoca reports for it. §1–§3 are about the first; §4 is where
> they meet.
>
> **Displayed equations are numbered by section — (1.1), (2.3) and so on — so you can name one
> instead of copying it.** Copying rendered math out of the preview pastes as a stack of lines,
> because KaTeX builds each formula from nested spans and the copy takes the visual layout. Say
> *"(2.3)"* and I will find it. The numbering is per section so that inserting an equation in §1
> never renumbers §4.
>
> **So inline math names a SYMBOL; a relation you might point at is displayed and numbered.** That
> is ordinary mathematical typesetting, and here it is also what keeps the document quotable: an
> equation buried mid-sentence can be neither numbered nor copied cleanly. Short values stay inline
> — $h = 0.25$, $t = 1$ — because naming them is never the problem. **If you find yourself wanting
> to quote a relation that is inline, that is a bug in the lecture, not in your copy-paste.**
>
> **And when you do need to quote prose containing inline math, drop the math.** *"And angle is what
> velocity accumulates"* is unique in this document; the symbols after it add nothing to the search
> and paste as a dozen lines.

---

## 1. The problem

You have a wheel. You know how hard you are pushing it, and you know it is sitting still right
now. **Where is it in one second?**

Write down what you actually know. A torque $\tau$ applied to an inertia $J$ produces an angular
acceleration — Newton's second law for rotation. And angle is what angular velocity accumulates:

$$\varphi(t) = \varphi(0) + \int_{0}^{t}\!\omega(s)\,ds \tag{1.1}$$

**That is what $\varphi$ *is*, and it is not a way of finding it.** To evaluate the integral you
need $\omega$ as a function of time across the whole interval — and all you have been given about
$\omega$ is *its* rate. (1.1) does not answer the question; it **defers** it to the same question
about $\omega$.

The **differential** form says the same thing, but locally — not *what has accumulated by $t$*, but
*how fast it is accumulating right now* — and that is what a model states:

$$\varphi' = \omega, \qquad J\,\omega' = \tau \tag{1.2}$$

**Hold on to the pairing, because the whole lecture turns on it.** (1.1) is what you want and
cannot evaluate. (1.2) is evaluable and is not what you want. Everything from §2 onward is about
getting from the second to the first.

*(For this wheel the deferral does terminate — $\omega$'s rate is a constant, so you can solve
$\omega$ first and then $\varphi$, which is what §1's next part does with a pencil. It terminates
because these two equations happen not to be tangled. Below, the general form shows what happens
when they are.)*

So:

```modelica
model SingleInertia
  parameter Real J = 1.0;
  parameter Real tau = 1.0;
  Real phi(start = 0.0);   // angle
  Real w(start = 0.0);     // angular velocity
equation
  der(phi) = w;
  J * der(w) = tau;
end SingleInertia;
```

Notice what you have and what you don't. **You know every *rate of change*, and you know where the
system is *now*.** You do not know where it will be. The model never says what $\varphi$ is at
$t = 1$; it says how fast $\varphi$ is changing, in terms of a quantity that is itself changing.

That is the general situation, and it has a name — an **initial value problem**:

$$z' = f(z, t), \qquad z(0) \ \text{given} \tag{1.3}$$

$z$ is the vector of things that carry the past. Here $z = (\varphi, \omega)$, and $f$ is the
right-hand side the model spells out:

$$z' = \begin{pmatrix} \varphi' \\ \omega' \end{pmatrix}
     = f(z,t) = \begin{pmatrix} \omega \\ \tau / J \end{pmatrix}
     = \begin{pmatrix} \omega \\ 1 \end{pmatrix} \tag{1.4}$$

**Every simulation in this project is this problem**, possibly with complications piled on top; the
rungs above this one are those complications, one at a time.

**And now (1.1) can be stated in general, which is where it gets genuinely circular:**

$$z(t) = z(0) + \int_{0}^{t}\!f\bigl(z(s),\, s\bigr)\,ds \tag{1.5}$$

**Look at where $z$ appears.** On the left, and again *inside its own integrand*. This is no longer
a deferral to some other quantity that might be solvable first — it is the unknown defined in terms
of itself. `SingleInertia` escapes it only because its $f$ ignores $\varphi$ entirely: the second
row of (1.4) is $\tau/J$, a constant, with no $\varphi$ and no $\omega$ in it. **Couple the
equations and the escape closes** — let the torque depend on the angle, $\tau = -\sin\varphi$, and
$\varphi$ is inside the integral that defines $\varphi$.

*(This is not a dead end in the mathematics — (1.5) is a Volterra integral equation and iterating
it is Picard's proof that a solution exists at all. It is a dead end for **computing**, which is
the subject here.)*

### Why this is hard

For this particular wheel it isn't. Integrate $\omega' = 1$ to get $\omega = t$, integrate
$\varphi' = t$ to get

$$\varphi(t) = \tfrac{1}{2} t^2 \tag{1.6}$$

and you are done — exactly, forever, for any $t$, with a pencil.

**That is the exception, not the rule.** Change one thing — let the torque depend on the angle, say
$\tau = -\sin\varphi$, which is a pendulum — and the pencil runs out.

**Be careful about what that claim is, because the loose version of it is false.** The pendulum
*does* have a closed-form solution. Separating $\varphi'' = -\sin\varphi$ and integrating once
leads to a quadrature, and that quadrature is an **elliptic integral**; inverting it gives

$$\sin\tfrac{\varphi}{2} = k \,\operatorname{sn}\!\bigl(K(k^2) - t,\; k^2\bigr),
  \qquad k = \sin\tfrac{\varphi_0}{2} \tag{1.7}$$

for a pendulum released from rest at $\varphi_0$, where $\operatorname{sn}$ is a Jacobi elliptic
function and $K$ the complete elliptic integral of the first kind. **Checked against a numerical
solve on 2026-09-25: the two agree to 1e−8 over eleven seconds**, which is the numerical solve's
own tolerance rather than any error in (1.7).

**The honest statement is narrower: no *elementary* formula** — no finite combination of
polynomials, exponentials, logarithms and trigonometric functions. That is a theorem rather than a
failure of effort; Liouville's results on integration in finite terms are what rule the elliptic
integral out, in the same way that $\int e^{-x^2}dx$ is ruled out.

**And the closed form (1.7) does not rescue you, which is the part worth taking from this.** It
buys less than it appears to:

- **It is still evaluated numerically.** $\operatorname{sn}$ and $K$ are computed by iteration, not
  read off. The formula relocates the arithmetic; it does not remove it.
- **It is brittle.** (1.7) describes an *undamped, unforced, single* pendulum released from rest.
  Add a damping term and it is gone. Add a driving torque and it is gone. Add a second link and the
  system is chaotic, with no useful closed form at any effort.

So the pencil does not fail here because the problem is exotic — it fails because the problem
stopped being idealised. **Most initial value problems worth simulating are in that category, and
every model you meet later in the ladder is.**

So the problem is: **produce numbers when no formula exists.**

---

## 2. Euler's move

Leonhard Euler published the answer in *Institutiones calculi integralis* (1768–70), and it is one
idea.

§1 left you holding both halves of a trade: an integral (1.1) that is the answer but cannot be
evaluated, and a derivative (1.2) that can be evaluated but is not the answer. **Euler's move is to
make that trade, repeatedly, over intervals short enough that the derivative barely changes across
one.**

Here is the same move from the other side, which is how it is usually taught. The derivative is
*defined* as a limit:

$$z'(t) = \lim_{h \to 0} \frac{z(t + h) - z(t)}{h} \tag{2.1}$$

**A machine cannot take a limit.** It can evaluate that quotient at some particular, finite $h$, and
that is all it can ever do. So Euler's move is: *stop the limit early and pretend.*

Rearrange the quotient before taking any limit:

$$z(t + h) \;\approx\; z(t) + h \, z'(t) \tag{2.2}$$

and $z'(t)$ is the one thing the model hands you — it is $f(z, t)$. So:

> **Forward Euler.** From where you are, ask the model how fast things are changing, believe that
> rate for a short time $h$, and step.
>
> $$z_{n+1} = z_n + h\, f(z_n, t_n) \tag{2.3}$$

That's the whole method. It is not an approximation *of* an algorithm; it **is** the algorithm, and
everything in lectures 2 and 3 is about the damage the word "pretend" does.

### What was actually purchased

You gave up exactness and bought *generality*. But it is worth being exact about which demand on
$f$ was dropped, because it is not the obvious one.

**The method must evaluate $f$.** Plainly — $f(z_n, t_n)$ is right there in the formula, and
without a way to compute it there is nothing to multiply by $h$. What Euler drops is the
requirement to **understand** $f$:

| | needs | fails when |
|---|---|---|
| **solving by hand** (§1) | a *formula* you can manipulate — an antiderivative you can name | $f$ has no elementary antiderivative, which is most of the time |
| **Euler** | a *procedure* you can call: hand it numbers, get numbers back | you cannot evaluate $f$ at all |

To integrate $\varphi' = t$ with a pencil you must know something **about** $t$ — that
$\tfrac{1}{2}t^2$ differentiates to it. Euler asks nothing about $t$; it asks *what is the rate,
here, now?* and gets a number.

**That is the whole trade.** $-\sin\varphi$ is exactly as easy as $1$ — not because the method
ignores which one it is, but because it only ever asks each of them the same question, and both can
answer it. Compare the two routes on the pendulum: solving it by hand needs an elliptic integral,
Liouville's theorem and the substitution (1.7), and all of that collapses the moment you add
damping. Evaluating $-\sin\varphi$ takes one call, and adding damping changes it to one slightly
longer call.

**Every model becomes computable, and the price is that every answer becomes wrong.** The rest of
the subject is about controlling how wrong.

> **This is the hinge the later lectures turn on, in both directions.**
>
> **Forward, into lecture 3:** evaluation is the *weakest* thing a method can demand, and forward
> Euler is the only one that gets away with it. Backward Euler puts the unknown on both sides:
>
> $$z_{n+1} = z_n + h\, f(z_{n+1}, t_{n+1}) \tag{2.4}$$
>
> Compare (2.3): the argument of $f$ changed, and that is the entire difference. But it turns each
> step into an *equation to solve* rather than a formula to apply, and solving it efficiently wants
> $\partial f / \partial z$. **A method that needs the Jacobian is asking about $f$'s structure
> again**, which is why lecture 3 costs more than this one.
>
> **Backward, into lecture 4:** if all a solver needs is something it can call, then somebody has to
> *build* that callable thing. A Modelica model is not it — `J * der(w) = tau` is an equation, not a
> procedure, and it does not even say which symbol to solve for. **Producing an evaluable $f$ from a
> model that never states one is Rumoca's entire job.**

---

## 3. Do it by hand

This is the part worth not skipping. Take $h = 0.25$ and step the wheel four times to $t = 1$.

State is $(\varphi, \omega)$, starting at $(0, 0)$. The rates are $\varphi' = \omega$ and
$\omega' = \tau/J = 1$.

| step | from $t$ | $\varphi$ | $\omega$ | rates $(\varphi', \omega')$ | $\varphi + h\varphi'$ | $\omega + h\omega'$ |
|---|---|---|---|---|---|---|
| 1 | 0.00 | 0.0000 | 0.00 | $(0.00,\ 1)$ | 0.0000 | 0.25 |
| 2 | 0.25 | 0.0000 | 0.25 | $(0.25,\ 1)$ | 0.0625 | 0.50 |
| 3 | 0.50 | 0.0625 | 0.50 | $(0.50,\ 1)$ | 0.1875 | 0.75 |
| 4 | 0.75 | 0.1875 | 0.75 | $(0.75,\ 1)$ | 0.3750 | 1.00 |

So Euler says $\varphi(1) = 0.375$ and $\omega(1) = 1$.

The truth is $\varphi(1) = \tfrac{1}{2}(1)^2 = 0.5$ and $\omega(1) = 1$.

**Two results, and they differ in kind.**

$\omega$ is **exact**. Not close — exact. Its rate is the constant $1$, so "believe the rate for a
short time" is not a pretence at all; it is true.

$\varphi$ is **25 % low**. Its rate is $\omega$, which is *changing during the step*. Euler used the
rate at the *start* of each step while $\omega$ grew throughout it, so every step under-counted. Look
at step 1: Euler moved $\varphi$ by $0.25 \times 0$ — not at all — while the true $\varphi$ had
already reached $\tfrac{1}{2}(0.25)^2 = 0.03125$.

**Read that against (1.1) and it stops being a quirk of the method.** Over one step, the truth is
an integral and Euler substitutes a rectangle:

$$\int_{t}^{t+h}\!\omega(s)\,ds \;\approx\; h\,\omega(t) \tag{3.1}$$

A rectangle whose height is the value at the *left* edge. For an $\omega$ that is rising, that
rectangle always falls short, and by an amount you can see: $\omega$ climbs by exactly $h$ across
the step, so the piece Euler misses is a triangle of base $h$ and height $h$ — area $h^2/2$, every
step, without fail. Over $n$ steps that is $n h^2 / 2$, which is (3.3) below, derived a second way.

**So "first-order" is not a property of Euler so much as of rectangles.** Lecture 2's better deal
comes from picking a smarter height than the left edge.

**That is the entire error mechanism, visible in one hand-worked table.** The method is exact when
the rate is constant across a step, and wrong in proportion to how much the rate moves.

### The lever

Halve the step and repeat with $h = 0.125$ (eight steps): $\varphi(1) = 0.4375$. The error was
$0.125$; now it is $0.0625$.

| $h$ | steps | $\varphi(1)$ | error $0.5 - \varphi(1)$ |
|---|---|---|---|
| $0.25$ | 4 | $0.375$ | $0.125$ |
| $0.125$ | 8 | $0.4375$ | $0.0625$ |
| $0.0625$ | 16 | $0.46875$ | $0.03125$ |

**Halving $h$ halves the error** — the error is $O(h)$, which is the definition of a **first-order**
method. It is a poor deal: work is $\propto 1/h$, so ten times the accuracy costs ten times the
work. Lecture 2 is about buying accuracy on better terms.

*Closed form for this case, if you want to check the table without stepping.* After $n$ steps of
size $h$, forward Euler gives

$$\varphi_n = h^2\,\frac{n(n-1)}{2}, \qquad\text{against the true}\qquad
  \varphi(nh) = \frac{(nh)^2}{2} \tag{3.2}$$

so the error is exactly

$$\varphi(nh) - \varphi_n = \frac{n h^2}{2} = \frac{h\,t}{2} \tag{3.3}$$

— linear in $h$ as claimed, and growing linearly in $t$.

---

## 4. What Rumoca does with it

Load `SingleInertia` and press Run. HRW plots `phi` rising as a parabola and `w` as a straight line,
which is the picture above. **But Rumoca is not running the method you just worked by hand**, and
the difference shows in its own numbers.

Rumoca's solver reports what it did on every step. For this model, $t_{\text{end}} = 1$:

```
   #      t            h        order
   0  0.00010000  1.000e-4  1
   1  0.00020000  2.000e-4  2
   2  0.00040000  2.000e-4  2
   ...                        (34 steps in total, none above order 2)
```

Three things there are not in §3.

**$h$ changes.** You chose $h$ once; the solver picks a new one every step, growing it while the
answer stays smooth. It went from $10^{-4}$ to $3.2 \times 10^{-3}$ within fifteen steps.

**The order changes.** Order 1 is §3's accuracy class — halve $h$, halve the error. Order 2 is a
different method with a better deal, and the solver switched to it after a single step. That is
lecture 2's subject arriving in the readout.

**It is an *implicit* method.** Rumoca runs BDF, which does **not** use the rate at the start of the
step. That is why it can take large steps on models where §3's method cannot, and it is lecture 3's
subject. For this wheel the distinction does not bite, which is part of why this is the right first
specimen.

### The measurement, and the thing worth seeing

Against the exact solution:

| | $t = 0.5$ | $t = 1.0$ |
|---|---|---|
| `w` error | $0$ | $0$ |
| `phi` error | $+9.801 \times 10^{-9}$ | $+9.801 \times 10^{-9}$ |

**`w` is exact and `phi` is not** — the same split as your hand table, for a related but not
identical reason.

$\omega$ solves $\omega' = 1$, whose solution is a straight line, and a first-order method
reproduces a straight line exactly. $\varphi$ solves $\varphi' = \omega = t$, whose solution is a
parabola, and **order 1 does not reproduce a parabola.**

Now look at where the error comes from. **Exactly one step ran at order 1** — step 0, with
$h_0 = 10^{-4}$. From step 1 on the order is 2, and a second-order method *does* reproduce a
parabola exactly. So no further error is ever added, and the error already present satisfies
$e' = 0$: it neither grows nor decays.

**One wrong step at the very start, frozen for the whole run.** The error is
$9.801 \times 10^{-9}$ at $t = 0.002$ and $9.801 \times 10^{-9}$ at $t = 1$. A constant, not a
drift — and that is the tell which says *startup*, not *accumulation*.

Tighten the tolerance, the solver takes a smaller first step, and the offset follows it:

| tolerance | first step $h_0$ | `phi` error at $t = 1$ |
|---|---|---|
| $10^{-4}$ | $1.000 \times 10^{-4}$ | $6.44 \times 10^{-7}$ |
| $10^{-6}$ | $1.000 \times 10^{-4}$ | $9.80 \times 10^{-9}$ |
| $10^{-9}$ | $3.761 \times 10^{-6}$ | $1.386 \times 10^{-11}$ |
| $10^{-12}$ | $1.189 \times 10^{-7}$ | $1.366 \times 10^{-14}$ |

$$\text{error} \;\approx\; h_0^2 \tag{4.1}$$

holding across eight orders of magnitude.

**One honest gap.** A single backward Euler step on $\varphi' = t$ predicts an error of
$h_0^2 / 2$. Measured is consistently about $1.96\times$ that. The scaling law is solid; the factor
of two is **not explained**, and no solver source was read for it. It is left visible rather than
rounded away.

---

## 5. What this rung cost to find out

Two things in §4 did not exist this morning.

**The `order` column is why any of this is explicable.** Without it, "`phi` is off by a constant" is
a curiosity. With it, the explanation is one line: one order-1 step, then order 2 forever.

**The tolerance table did not work at all until today.** HRW built its solver options as
`SimOptions { t_end, ..Default::default() }` and never read the model's
`experiment(Tolerance = …)`, so all four rows came back identical. Rumoca extracts the annotation and
three other consumers honour it; HRW was the one that dropped it. **The defect was found by trying
to teach this lecture**, which is the argument for writing lectures against a live instrument rather
than from memory.

---

## 6. Where this goes

- **Lecture 2 — order.** Halving $h$ for half the error is a bad deal. Runge (1895), Heun (1900) and
  Kutta (1901) bought a much better one. `SingleInertia` cannot show it — its solution is a
  polynomial that several methods get exactly — so `HarmonicOscillator`, whose solution is
  $\cos \omega t$, is the specimen.
- **Lecture 3 — stiffness.** There are systems where you cannot shrink $h$ for accuracy, because
  *stability* has already forced it far smaller. `StiffDecay`, and Curtiss & Hirschfelder's 1952
  paper that named it.
- **The compiler enters at lecture 4**, not before. Everything above treats $f(z, t)$ as
  **already evaluable** — §2's trade assumes something exists that you can hand numbers to. A
  Modelica model is not that thing, and Rumoca's entire job is building one from a model that
  never states it. [`the-pipeline`](../fixture-labs/the-pipeline.md) is the lab route through
  that.

---

## Questions this lecture has not answered

*Anything here you want pursued becomes a revision, not a chat reply.*

- Why $h_0^2$ and not $h_0^2/2$ — the factor of $1.96$.
- Why the solver starts at order 1 rather than the order it intends to use.
- What BDF actually computes per step, which §4 only gestures at. That is lecture 3's, unless you
  want it sooner.
