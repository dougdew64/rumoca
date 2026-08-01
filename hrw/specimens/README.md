# Specimens — the curated Modelica corpus

**Purpose:** what a specimen is, why each one exists, and the rules a new one must meet.
**Status:** 👤 reference, written for a human — Doug, or a Rumoca maintainer reading the fork.
**Read when:** adding a specimen, or wondering why HRW ships eighteen small models when the
Modelica Standard Library is right there.

## What these are

**Small Modelica models, each authored to trigger one phenomenon in the compiler.** They are
the models HRW is developed and tested against, and they exist because the MSL does not
contain what a *study* instrument needs: a minimal case per behaviour, with the reason it was
written recorded next to it.

The MSL is here too — 2,626 models, vendored and surveyed — and it is the better corpus for
*scale* questions ("does an unseen IR shape break us?"). These eighteen are the better corpus
for *shape* questions ("what does index reduction actually do?"), because each isolates one
thing and nothing else.

## The corpus

**Every specimen carries a `// purpose:` line**, and HRW shows it under the filename in the
specimen list. This table is generated from those lines — if it disagrees with a file, the
file wins.

| Specimen | Purpose |
|---|---|
| `SingleInertia` | Minimal index-1 ODE — two states, all scalar BLT blocks (the baseline) |
| `RotationalInertia` | Same physics via MSL connectors — connector expansion; still index-1 |
| `Drivetrain` | Ideal gears → **high-index, structurally singular** DAE; needs index reduction |
| `MotorWithBrake` | The end-to-end specimen — every phase: MSL connectors, index reduction (EMF coupling), events, stiff dynamics |
| `GearWithBrake` | The other end-to-end specimen, same intent |
| `ProportionalLoop` | Idealized algebraic feedback → one coupled BLT block (tearing) |
| `NonlinearLoop` | Same *structure*, but Newton to solve — **structure ≠ numerics** |
| `TwoLoops` | Two algebraic loops in series → two coupled blocks, sequenced |
| `MixedLoop` | Loop bracketed by scalar solves — BLT ordering made visible |
| `RcCircuit` | Initialization / IC planning — a well-posed RC |
| `CapacitorLoop` | RC initialization blow-up — a capacitor across an ideal source |
| `OverInitRc` | Conflicting initial equations over-determine the capacitor state |
| `BouncingBall` | Events / hybrid structure — a state event with `reinit` |
| `BenchActuator` | Stiffness — fast L/R coupled to a slow rotor. **BDF's reason to exist** |

### The failure specimens — marked DO NOT FIX

Four models are **deliberately broken**, one per compiler failure path, so the diagnosis HRW
produces can be examined. Fixing one destroys what it is for; each says so in its own header.

| Specimen | Fails at |
|---|---|
| `UndefinedRef` | resolve |
| `DimensionMismatch` | typecheck |
| `IncompatibleConnect` | flatten — and **System Modeler rejects it while Rumoca accepts it**, which is upstream issue 2 |
| `UnbalancedShaft` | DAE construction |

## Rules for a new specimen

1. **Portable Modelica only.** Authored in Wolfram System Modeler, but no Wolfram-flavoured
   extensions — *definition of done is that it compiles and runs equivalently in System
   Modeler and in Rumoca*, because a specimen that only one tool accepts cannot adjudicate
   anything.
2. **A `// purpose:` line**, one sentence, phenomenon-focused — the compiler feature it
   exercises. Keep it distinct from the Modelica `description` string, which stays a faithful
   description of the *model*.
3. **A notebook entry** — `../docs/specimen-notebook/<Model>/` with a generated `trace/` and a
   hand-written `purpose.md`. See that directory's README.
4. **No MSL MultiBody.** Mechanical components come from our own small planar library; the
   reasoning is in `../docs/CHARTER.md`.
5. **Prefer standards.** MSL components and portable Modelica over anything custom — what is
   learned here should transfer to any Modelica tool.

## Scratch specimens are a different thing

Claude writes throwaway models mid-conversation — *"here is the smallest model that shows the
thing you asked about"* — into **`../.hrw-bridge/specimens/`**, which is gitignored. HRW lists
them within a second, marked, with no restart.

**Those are not held to the rules above**, and being in a gitignored directory makes them
ephemeral by construction rather than by discipline. A probe worth keeping gets *moved* here
deliberately, with a `// purpose:` line and a notebook entry — which is the moment it stops
being a probe.

**A scratch name may not shadow a curated one.** The collision is reported and the scratch
file skipped, because silently loading a different model than the name says is the kind of
error nobody catches.

## Further reading

- 👤 [`../docs/CHARTER.md`](../docs/CHARTER.md) §4.3 — the specimen rules as adopted
- 👤 [`../docs/specimen-notebook/`](../docs/specimen-notebook/) — per-specimen traces and purposes
- 👤 [`../docs/compiler-phases/the-chain-of-problems.md`](../docs/compiler-phases/the-chain-of-problems.md) — what each phase is *for*, which is what these specimens probe
