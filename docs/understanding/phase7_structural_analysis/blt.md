# Drill-Down: BLT (Block-Lower-Triangular) Form

*Parent document: [structural_analysis.md](structural_analysis.md)*
*Source: [crates/rumoca-phase-structural/src/blt.rs](../../../crates/rumoca-phase-structural/src/blt.rs)*

---

## What Problem Does This Step Solve?

[Tarjan's algorithm](tarjan_scc.md) tells us *which* equations belong together
(the SCCs) and *what order* the SCCs must be solved in. BLT block construction
takes that information and packages it into a form the rest of the toolchain
— code generators, simulators, IC solvers — can consume directly:

- One `BltBlock` per SCC.
- Each block records its equations and the unknowns it is responsible for
  computing.
- The blocks are emitted in evaluation order, so a downstream consumer can
  simply iterate the list from first to last.

In short: BLT is the contract between the structural-analysis phase and
everything that comes after.

---

## Why "Block Lower Triangular"?

The name comes from the matrix view of the matched system. Take the equations
in matched order (each equation paired with its owned variable), and arrange
the incidence matrix so that variable $v_i$ is column $i$ and equation $e_i$
(matched to $v_i$) is row $i$. The matched cells lie on the diagonal.

If the system has no algebraic loops, the matched matrix can be permuted into
**lower triangular** form — every off-diagonal entry sits below the diagonal:

```
                 v0  v1  v2  v3
            e0 [  ●   .   .   . ]
            e1 [  ●   ●   .   . ]
            e2 [  .   ●   ●   . ]
            e3 [  ●   .   .   ● ]
```

Reading row $i$: equation $e_i$'s residual depends on $v_i$ (the diagonal,
which $e_i$ owns) and possibly on $v_0, \dots, v_{i-1}$ (already computed by
earlier rows). Solving top-to-bottom turns the implicit DAE into a sequence of
single-variable solves — back-substitution.

When algebraic loops are present, the loops form **diagonal blocks** that
cannot be resolved into pure triangular form:

```
                 v0  v1  v2  v3
            e0 [  ●   .   .   . ]   ← scalar block
            e1 [  .  ┌───────┐ ]
            e2 [  ●  │ ● ●   │ ]   ← 2×2 algebraic loop block
                    │ ● ●   │
            e3 [  ●  └───────┘● ]   ← scalar block
```

Diagonal blocks of size 1 are scalar; diagonal blocks of size $> 1$ are
algebraic loops. Outside the diagonal blocks, all entries are below — hence
"block lower triangular." This block structure is exactly the Tarjan SCC
partition arranged in reverse-topological order.

The mapping from SCC partition to BLT form is therefore *not a separate
algorithm* — it is just a relabelling of Tarjan's output:

| Tarjan output | BLT view |
|---------------|----------|
| SCC of size 1 | Scalar block |
| SCC of size $>1$ | Algebraic-loop block |
| Reverse-topological order | Top-to-bottom block ordering |

---

## The `BltBlock` Type

```rust
pub enum BltBlock {
    Scalar {
        equation: EquationRef,
        unknown:  UnknownId,
    },
    AlgebraicLoop {
        equations: Vec<EquationRef>,
        unknowns:  Vec<UnknownId>,
    },
}
```

The two variants directly mirror the two diagonal-block sizes:

- **`Scalar`** — one equation, one unknown. Evaluating this block means
  computing one variable's value. If the equation can be solved symbolically
  for the unknown (e.g. `0 = a*x + b` ⇒ `x = -b/a`), downstream phases use the
  symbolic form; otherwise a single-variable Newton iteration suffices.
- **`AlgebraicLoop`** — multiple equations, multiple unknowns. The unknowns
  must be determined jointly. This block is the input to [tearing](tearing.md):
  if a small set of "tear" variables can be identified, the loop reduces to a
  low-dimensional Newton-style iteration plus a sequence of causal evaluations;
  otherwise the whole block is solved by Levenberg-Marquardt.

The block records `EquationRef`s (not raw expression trees) so that
downstream code can look the equations up in the original DAE without needing
to clone them through the structural pipeline.

---

## Construction

The whole construction is surprisingly short — a thin wrapper over Tarjan:

```rust
pub(crate) fn build_blt_blocks(
    incidence: &Incidence,
    match_eq:  &[Option<usize>],
    adj:       &[Vec<usize>],
) -> Vec<BltBlock> {
    let sccs = tarjan_scc(incidence.n_eq, adj);
    sccs.into_iter()
        .map(|scc| scc_to_block(&scc, incidence, match_eq))
        .collect()
}
```

It calls Tarjan to get the SCCs in evaluation order, then converts each SCC
into a `BltBlock`. No reordering, no extra passes.

The conversion in `scc_to_block`:

```rust
fn scc_to_block(scc: &[usize], incidence: &Incidence, match_eq: &[Option<usize>]) -> BltBlock {
    if scc.len() == 1 {
        let eq_idx  = scc[0];
        let eq_ref  = incidence.equation_refs[eq_idx].clone();
        let unknown = match match_eq[eq_idx] {
            Some(var_idx) => incidence.unknown_names[var_idx].clone(),
            None          => UnknownId::Variable(VarName::from("???")),
        };
        BltBlock::Scalar { equation: eq_ref, unknown }
    } else {
        let equations: Vec<EquationRef> = scc
            .iter()
            .map(|&i| incidence.equation_refs[i].clone())
            .collect();
        let unknowns: Vec<UnknownId> = scc
            .iter()
            .filter_map(|&i| match_eq[i].map(|v| incidence.unknown_names[v].clone()))
            .collect();
        BltBlock::AlgebraicLoop { equations, unknowns }
    }
}
```

### Step by step

- **SCC of size 1.** Pull the single equation index out, look up the
  `EquationRef` and the matched unknown's `UnknownId`, and build a `Scalar`
  variant. The fallback `"???"` placeholder for an unmatched equation never
  fires in normal operation — by the time `scc_to_block` runs, the matching
  has already been validated to be perfect — but it keeps the function total.

- **SCC of size > 1.** Translate every equation index in the SCC to its
  `EquationRef`, and every matched unknown to its `UnknownId`. The `filter_map`
  on the unknowns is again defensive: it silently drops unmatched equations,
  but in practice no member of an SCC will be unmatched.

The block is emitted *in the order Tarjan returned the SCCs*. That order, as
discussed in [Tarjan's drill-down](tarjan_scc.md#why-the-output-order-is-already-reverse-topological),
is already correct for BLT — dependencies first. The downstream evaluator
walks the resulting `Vec<BltBlock>` from index 0 upward.

---

## Worked Example: Mixed Scalar + Loop

Adapted from
[`test_blt_mixed`](../../../crates/rumoca-phase-structural/src/blt.rs#L137-L169):

Three equations, three unknowns, with this incidence:

```
eq0 references {v0}
eq1 references {v0, v1, v2}
eq2 references {v1, v2}
```

Matching: `eq0 → v0`, `eq1 → v1`, `eq2 → v2`.

Dependency graph (consumer → producer):

```
adj[0] = []          // eq0 depends on nothing
adj[1] = [0, 2]      // eq1 references v0 (owned by eq0) and v2 (owned by eq2)
adj[2] = [1]         // eq2 references v1 (owned by eq1)
```

There is a 2-cycle between eq1 and eq2 (each references the other's owned
variable), and eq0 is a producer for eq1.

Tarjan walks the graph and emits two SCCs in reverse topological order:

1. `[0]` — the scalar producer.
2. `[1, 2]` (or `[2, 1]`) — the loop.

`build_blt_blocks` converts these to:

```rust
[
    BltBlock::Scalar       { equation: eq0_ref, unknown: v0 },
    BltBlock::AlgebraicLoop {
        equations: vec![eq1_ref, eq2_ref],
        unknowns:  vec![v1, v2],
    },
]
```

Reading the list as instructions: **first** evaluate eq0 to compute v0; **then**
solve the eq1/eq2 loop jointly to compute v1 and v2 (the loop block can use
v0 as a known input).

The matrix view:

```
                  v0     v1   v2
        eq0 [  ●     .    . ]    ← scalar block
        eq1 [  ●   ┌─────────┐
                  │ ●     ●  │   ← 2x2 algebraic loop
        eq2 [  .  │ ●     ●  │
                  └─────────┘
```

Lower-triangular outside the diagonal blocks; the loop is the 2×2 block on
the diagonal.

---

## Why "Reverse Topological" Is "Top to Bottom"

The directionality is easy to scramble. Two facts pin it down:

1. Edges in our dependency graph point *consumer → producer*. So the
   topological order on the condensation puts producers *later* than
   consumers, while the **reverse topological** order puts producers
   *earlier*.

2. Tarjan emits SCCs in reverse topological order of the condensation.

Combine: Tarjan emits **producers first, consumers last**. That is the order
the BLT block list is laid out, and that is the order downstream code
evaluates.

A different convention — edges pointing producer → consumer — would have
required reversing the SCC list. The code has chosen the convention that
needs no reversal.

---

## Tests

Three tests in
[blt.rs](../../../crates/rumoca-phase-structural/src/blt.rs#L73-L169) exercise the
three structural shapes:

- **Linear chain** (`test_blt_linear_chain`): `eq0 → eq1 → eq2`, no cycles.
  Expect three `Scalar` blocks.
- **Single 2×2 loop** (`test_blt_single_algebraic_loop`): `eq0 ↔ eq1`. Expect
  one `AlgebraicLoop` block of size 2.
- **Mixed** (`test_blt_mixed`): scalar producer `eq0`, plus loop `{eq1, eq2}`.
  Expect a `Scalar` followed by an `AlgebraicLoop`. The test asserts that the
  scalar comes *first* — this pins down the dependency direction and confirms
  Tarjan's emission order is being used unchanged.

---

## Summary

- BLT (block lower triangular) is the matrix view of an SCC-partitioned
  matched system: each diagonal block is one SCC, off-diagonal entries are
  all below the diagonal blocks.
- The two block flavours are `Scalar` (one equation, one unknown) and
  `AlgebraicLoop` (multiple equations, multiple unknowns coupled).
- Construction is a thin wrapper over Tarjan: each SCC becomes one block, in
  the order Tarjan emitted them.
- No reordering is needed because dependency edges point consumer → producer,
  so Tarjan's reverse-topological output is already evaluation order
  (dependencies first).
- The resulting `Vec<BltBlock>` is the contract for code generation,
  simulation, and the IC plan; downstream consumers simply iterate it from
  first to last.
