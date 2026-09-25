# Lecture 1 — Euler, 1768: getting a number when there is no formula

**Purpose:** the first rung of [`specimen-ladder.md`](specimen-ladder.md). What problem numerical
integration solves, the single move Euler made, and what Rumoca does with it.
**Status:** lecture — a draft to be argued with. **Ask, and this file gets revised.**
**Read before:** any lab downstream of `Simulation`. There isn't one yet, and this lecture is
part of working out what it should check.
**Specimen:** `SingleInertia`. **Measured against Rumoca 0.9.20 on 2026-09-24.**

---

## 1. The problem

You have a wheel. You know how hard you are pushing it, and you know it is sitting still right
now. **Where is it in one second?**

Write down what you actually know. A torque `τ` applied to an inertia `J` produces an angular
acceleration — that is Newton's second law for rotation, `J·ω′ = τ`. And angle is what velocity
accumulates: `φ′ = ω`. So:

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
system is *now*.** You do not know where it will be. The model never says what `φ` is at `t = 1`;
it says how fast `φ` is changing, in terms of a quantity that is itself changing.

That is the general situation, and it has a name — an **initial value problem**:

```
z′ = f(z, t),    z(0) given
```

`z` is the vector of things that carry the past. Here `z = (φ, ω)`, and `f` is the right-hand
side the model spells out. **Every simulation in this project is this problem**, possibly with
complications piled on top; the rungs above this one are those complications, one at a time.

### Why this is hard

For this particular wheel it isn't. Integrate `ω′ = 1` to get `ω = t`, integrate `φ′ = t` to get
`φ = t²/2`, and you are done — exactly, forever, for any `t`, with a pencil.

**That is the exception, not the rule.** Change one thing — let the torque depend on the angle,
say `τ = −sin φ`, which is a pendulum — and there is no elementary formula for `φ(t)` at all. Not
"nobody has found one"; there isn't one. Most initial value problems worth simulating are in that
category, and every model you will meet later in the ladder is.

So the problem is: **produce numbers when no formula exists.**

---

## 2. Euler's move

Leonhard Euler published the answer in *Institutiones calculi integralis* (1768–70), and it is one
idea.

The derivative is defined as a limit:

```
z′(t) = lim   [ z(t + h) − z(t) ] / h
        h→0
```

**A machine cannot take a limit.** It can compute that quotient at some particular, finite `h`, and
that is all it can ever do. So Euler's move is: *stop the limit early and pretend.*

Rearrange the quotient before taking any limit, and it is an exact statement about nothing in
particular:

```
z(t + h) ≈ z(t) + h · z′(t)
```

and `z′(t)` is the one thing the model hands you — it is `f(z, t)`. So:

> **Forward Euler.** From where you are, ask the model how fast things are changing, believe that
> rate for a short time `h`, and step.
>
> ```
> z_{n+1} = z_n + h · f(z_n, t_n)
> ```

That's the whole method. It is not an approximation *of* an algorithm; it *is* the algorithm, and
everything in lectures 2 and 3 is about the damage the word "pretend" does.

**What was actually purchased.** You gave up exactness and bought *generality*. The formula above
never asks what `f` is. `−sin φ` is as easy as `1`. **Every model becomes computable, and the
price is that every answer becomes wrong.** The rest of the subject is about controlling how
wrong.

---

## 3. Do it by hand

This is the part worth not skipping. Take `h = 0.25` and step the wheel four times to `t = 1`.

State is `(φ, ω)`, starting at `(0, 0)`. The rates are `φ′ = ω` and `ω′ = τ/J = 1`.

| step | from `t` | `φ` | `ω` | rates `(φ′, ω′)` | `φ + h·φ′` | `ω + h·ω′` |
|---|---|---|---|---|---|---|
| 1 | 0.00 | 0.0000 | 0.00 | (0.00, 1) | 0.0000 | 0.25 |
| 2 | 0.25 | 0.0000 | 0.25 | (0.25, 1) | 0.0625 | 0.50 |
| 3 | 0.50 | 0.0625 | 0.50 | (0.50, 1) | 0.1875 | 0.75 |
| 4 | 0.75 | 0.1875 | 0.75 | (0.75, 1) | 0.3750 | 1.00 |

So Euler says `φ(1) = 0.375` and `ω(1) = 1.0`.

The truth is `φ(1) = 1²/2 = 0.5` and `ω(1) = 1`.

**Two results, and they are different in kind.**

`ω` is **exact**. Not close — exact. Its rate is the constant `1`, so "believe the rate for a short
time" is not a pretence at all; it is true.

`φ` is **25 % low**. Its rate is `ω`, which is *changing during the step*. Euler used the rate at
the *start* of each step and `ω` grew throughout it, so every step under-counted. Look at step 1:
Euler moved `φ` by `0.25 × 0` — not at all — while the true `φ` had already reached `0.03125`.

**That is the entire error mechanism, visible in one hand-worked table.** The method is exact when
the rate is constant across a step and wrong in proportion to how much the rate moves.

### The lever

Halve the step and repeat with `h = 0.125` (eight steps) and you get `φ(1) = 0.4375`. Error was
`0.125`, now `0.0625`.

| `h` | `φ(1)` | error |
|---|---|---|
| 0.25 | 0.375 | 0.125 |
| 0.125 | 0.4375 | 0.0625 |
| 0.0625 | 0.46875 | 0.03125 |

**Halving `h` halves the error.** That is the definition of a **first-order** method, and it is a
poor deal: ten times the accuracy costs ten times the work. Lecture 2 is about buying accuracy on
better terms.

---

## 4. What Rumoca does with it

Load `SingleInertia` and press Run. HRW plots `φ` rising as a parabola and `ω` as a straight line,
which is the picture above. **But Rumoca is not running the method you just worked by hand**, and
the difference is visible in its own numbers.

Rumoca's solver reports what it did on every step. For this model, `t_end = 1`:

```
   #      t            h        order
   0  0.00010000  1.000e-4  1
   1  0.00020000  2.000e-4  2
   2  0.00040000  2.000e-4  2
   ...                        (34 steps in total, none above order 2)
```

Three things there are not in §3.

**`h` changes.** You chose `h` once; the solver picks a new one every step, growing it while the
answer stays smooth. It went from `1e-4` to `3.2e-3` within fifteen steps.

**`order` changes.** Order 1 is the accuracy class of §3's method — halve `h`, halve the error.
Order 2 is a different method with a better deal, and the solver switched to it after one step.
That is lecture 2's subject arriving in the readout.

**It is an *implicit* method.** Rumoca runs BDF, which does not use the rate at the start of the
step. It is why the solver can take large steps without blowing up on models where §3's method
cannot, and it is lecture 3's subject. For this wheel the distinction does not bite, which is part
of why this is the right first specimen.

### The measurement, and the thing worth seeing

Against the exact solution:

| | `t = 0.5` | `t = 1.0` |
|---|---|---|
| `w` error | 0 | 0 |
| `phi` error | +9.801e−9 | +9.801e−9 |

**`ω` is exact, and `φ` is not.** The same split as your hand table — and for a related reason,
though not the same one.

`ω` solves `ω′ = 1`, whose solution is a straight line. A first-order method reproduces a straight
line exactly. `φ` solves `φ′ = ω = t`, whose solution is a parabola, and **order 1 does not
reproduce a parabola.**

Now look at where the error comes from. **Exactly one step ran at order 1** — step 0, with
`h = 1e-4`. From step 1 on, the order is 2, and a second-order method *does* reproduce a parabola
exactly. So no further error is ever added, and the error already present obeys `e′ = 0`: it
neither grows nor decays.

**One wrong step at the very start, frozen for the whole run.** The error is `9.801e−9` at
`t = 0.002` and `9.801e−9` at `t = 1`. A constant, not a drift — which is the tell that says
"startup", not "accumulation".

Tighten the tolerance and the solver takes a smaller first step, and the offset follows it:

| tolerance | first `h` | `phi` error at `t = 1` |
|---|---|---|
| 1e−4 | 1.000e−4 | 6.44e−7 |
| 1e−6 | 1.000e−4 | 9.80e−9 |
| 1e−9 | 3.761e−6 | 1.386e−11 |
| 1e−12 | 1.189e−7 | 1.366e−14 |

`err ≈ h₀²`, holding across eight orders of magnitude.

**One honest gap.** A single backward Euler step on `φ′ = t` predicts an error of `h₀²/2`. Measured
is consistently about **1.96×** that. The scaling law is solid; the factor of two is not explained,
and no solver source was read for it. It is left visible rather than rounded away.

---

## 5. What this rung cost to find out

Two things in §4 did not exist this morning.

**The `order` column is why any of this is explicable.** Without it, "`φ` is off by a constant" is
a curiosity. With it, the explanation is one line: one order-1 step, then order 2 forever.

**The tolerance table did not work at all until today.** HRW built its solver options as
`SimOptions { t_end, ..Default::default() }` and never read the model's
`experiment(Tolerance = …)`, so all four rows of that table came back identical. Rumoca extracts
the annotation and three other consumers honour it; HRW was the one that dropped it. **The defect
was found by trying to teach this lecture**, which is the argument for writing lectures against a
live instrument rather than from memory.

---

## 6. Where this goes

- **Lecture 2 — order.** Halving `h` for half the error is a bad deal. Runge (1895), Heun (1900)
  and Kutta (1901) bought a much better one. `SingleInertia` cannot show it, because its solution
  is a polynomial that several methods get exactly; `HarmonicOscillator` can.
- **Lecture 3 — stiffness.** There are systems where you cannot shrink `h` for accuracy, because
  stability has already forced it far smaller. `StiffDecay`, and Curtiss & Hirschfelder's 1952
  paper that named it.
- **The compiler enters at lecture 4**, not before. Everything above treats `f(z, t)` as given.
  Rumoca's entire job is producing `f` from a model that does not state it — and
  [`the-pipeline`](fixture-labs/the-pipeline.md) is the lab route through that.

---

## Questions this lecture has not answered

*Anything here that you want pursued becomes a revision, not a chat reply.*

- Why `h₀²` and not `h₀²/2` — the factor of 1.96.
- Why the solver starts at order 1 rather than the order it intends to use.
- What BDF actually computes per step, which §4 waves at. That is lecture 3's, unless you want it
  sooner.
