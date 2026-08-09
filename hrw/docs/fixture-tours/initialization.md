# Initialization — the equations that only run once

Everything the earlier tours built describes how the system moves. **None of it says where it
starts.** At `t = 0` a solver needs a number for every state, and those numbers come from a
different system of equations that is solved exactly once and then never again.

**This tour is a pair one line apart.** `RcCircuit` initializes cleanly. `OverInitRc` is the same
circuit with two extra lines, and it is over-determined. The two lines are the lesson.

> Every number below is read from the specimens' generated traces.

---

## The problem this phase exists to solve

A state needs a starting value, and Modelica offers three ways to supply one:

- **a `start` attribute** — `Real x(start = 3)`, a *guess* unless marked `fixed = true`
- **an `initial equation` section** — equations that hold only at `t = 0`
- **conditions implied by the model**, such as a steady-state requirement

Count the states, count the conditions, and compare. **Too few and the solver guesses; too many
and the model contradicts itself.** The phase's job is to do that arithmetic and say which case
you are in — before the integrator wastes its time.

---

## Act 1 — Nothing specified, and that is fine

[BouncingBall → Initialization](hrw://load/BouncingBall/Initialization)

**Expected:** the determinacy record reads **2 states**, **0 initial equations**,
**surplus −2**, verdict *"well-posed (remaining states initialize from their start attributes)"*.

Height and velocity get their values from their `start` attributes. **A negative surplus is not
an error** — it means the model did not over-specify, and the unspecified states fall back to
their declared starts.

**But note what "well-posed" is not promising.** It says the arithmetic works out, not that the
starting values are *right*. A `start` attribute with no `fixed = true` is a guess, and a solver
is free to move it. That distinction matters the first time a simulation starts somewhere you
did not intend.

---

## Act 2 — A real circuit, initialized from one number

[RcCircuit → Initialization](hrw://load/RcCircuit/Initialization)

A 5 V source, a 100 Ω resistor, a 1 mF capacitor, a ground.

**Expected:** **1 state**, **0 initial equations**, **surplus −1**, verdict *well-posed*.

Twenty-three equations in the model, and exactly **one state**: the capacitor voltage `C.v`.
Everything else is algebraic — determined instantly once `C.v` is known.

**That ratio is the normal case.** Most equations in a component model are constitutive or
connection equations, and the state count is a much smaller number governed by how many
independent energy stores the physical system has. One capacitor, one state.

---

## Act 3 — The same circuit, over-determined

[OverInitRc → Initialization](hrw://load/OverInitRc/Initialization)

`OverInitRc` is `RcCircuit` with one section added:

```modelica
initial equation
  C.v = 0;
  der(C.v) = 0;
```

**Expected:** **1 state**, **2 initial equations**, **explicit initial conditions 2**,
**surplus +1**, verdict **"over-determined"**.

**Expected:** the block structure is otherwise identical to `RcCircuit` — 23 equations, 21
blocks, the same relaxation hint. Only the initial system differs.

**One state, two conditions.** The arithmetic is the whole diagnosis: you cannot impose two
independent requirements on one number and expect both to hold.

---

## Act 4 — Why those two lines fight

They look innocent, and each is individually reasonable:

- `C.v = 0` — *"the capacitor starts uncharged."*
- `der(C.v) = 0` — *"the circuit starts in steady state."*

**Now do the physics.** If `C.v = 0`, the full 5 V appears across the 100 Ω resistor, so 50 mA
flows into the capacitor. A capacitor's current *is* its voltage rate: `i = C · der(C.v)`. So
`der(C.v) = 0.05 / 0.001 = 50 V/s`, which is emphatically **not** zero.

The two conditions describe two different instants — the moment of switch-on, and the settled
state long after. **Requiring both asks the circuit to be uncharged and finished charging at the
same time.**

**This is the most common real initialization failure**, and it almost never looks like an error
in the source. Each line is defensible; only together are they wrong, and only the count reveals
it.

**Expected:** nothing in the *structural* stage complains about `OverInitRc` — the trouble is
confined to the initial system.

---

## Act 5 — The relaxation hint

**Expected:** both `RcCircuit` and `OverInitRc` report a relaxation hint naming **dropped
equation 17** and **pinned unknown `gnd.p.i`**.

The `t = 0` system is not automatically square, and this records what was done about it: one
equation set aside, one unknown fixed.

**`gnd.p.i` is the ground pin's current**, and pinning it is the electrical modeller's oldest
move. A ground is a *reference*: it defines where zero volts is, and the current flowing into it
is whatever the rest of the circuit sends there. Kirchhoff's current law across the whole circuit
is then redundant with the individual node equations — one equation too many, saying something
already implied.

**So the dropped equation is not information being discarded.** It is a redundancy being
recognised, and the hint is the compiler showing its work rather than silently making the system
square.

---

## Act 6 — The arithmetic, stated once

$$ \text{surplus} = (\text{initial equations} + \text{fixed starts}) - \text{states} $$

| surplus | meaning | what happens |
|---|---|---|
| **< 0** | fewer conditions than states | the rest come from `start` attributes — legal, and the values are guesses |
| **= 0** | exactly determined | the ideal case |
| **> 0** | more conditions than states | **over-determined** — the conditions may contradict |

| specimen | states | conditions | surplus | verdict |
|---|---|---|---|---|
| `BouncingBall` | 2 | 0 | −2 | well-posed |
| `RcCircuit` | 1 | 0 | −1 | well-posed |
| `OverInitRc` | 1 | 2 | **+1** | **over-determined** |

**A positive surplus is a rank statement, not a count of mistakes.** Two conditions on one state
are only a contradiction if they are *independent*; writing `C.v = 0` twice would also give
surplus +1 while being perfectly consistent. The count finds the candidates; whether they
genuinely conflict is a question about the equations, which is why the verdict is worth reading
alongside the numbers rather than instead of them.

---

## What comes next in the chain

Initialization and the running system are **two different problems over the same variables**, and
solve lowering emits both: an initial residual and a continuous one. That separation is visible
in the next tour's `problem` record, which carries `initialization` and `continuous` as siblings.

---

## What this tour cannot check

**Whether the determinacy record is easy to find on the Initialization tab.** Everything above
depends on those six fields being visible together; if they are buried in a tree, the tour is
describing data rather than a view.

**Whether Act 4's physics is the right depth.** Working through 50 mA and 50 V/s is the thing
that makes the conflict *obvious* rather than *asserted* — but it assumes the RC circuit is
familiar enough that arithmetic clarifies rather than distracts.

**Whether Act 5 belongs at all.** The relaxation hint is real and appears on both specimens, so
it is not part of the over-determination story — it may be a genuinely interesting aside, or a
distraction from a tour that was otherwise about one clean contrast.
