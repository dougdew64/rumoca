# Upstream issues — Rumoca bugs found through HRW

**Purpose:** reproduced Rumoca bugs, each written to be filable with a copy-paste plus a
sentence.
**Status:** record.
**Read when:** a bug is reproduced and needs recording. **Claude adds entries and never files
them** — filing is Doug's. Only *reproduced* bugs go in, and suspect code is marked
unverified: a confident wrong diagnosis costs the credibility this project is building.

**Ready to file with [CogniPilot/rumoca](https://github.com/CogniPilot/rumoca).** Doug files
these when the time is right; Claude adds entries as they are found and never files them
itself.

Each entry is written to be **filable with a copy-paste plus a sentence** — reproduction,
expected vs actual, evidence, and where the suspect code lives — so nothing has to be
re-investigated months later. That is the whole point of the file: an investigation
regenerates only at the cost of doing it again.

**Baseline for everything below:** Rumoca `0.9.20`, `hrw` branch cut from upstream
`8cdc7419`. Verify against current upstream before filing — a bug fixed in the meantime
should be struck out here, not reported.

**Why this file exists:** bugs found through HRW are opportunities to build the maintainer
relationship, not just to work around (`project-engage-rumoca-community`). Both entries
below were found by *auditing failure paths* (`docs/ideas.md` #45), and the second was
adjudicated by an independent Modelica implementation rather than argued from the spec.

---

## Which to file first

**Issue 2 (connector validation).** Doug's plan (2026-07-30) is to open one bug PR with a
screen-capture video of a self-playing HRW tour attached — no campaigning, just something
likely to prompt a reviewer to ask what it is.

Issue 2 suits that far better than issue 1:

- **The reproduction is one 20-line model**, where issue 1 needs three compiles in a
  particular order within one session.
- **It is independently adjudicated.** System Modeler rejects the same source, so there is
  nothing to argue about.
- **The narrative is visual and short**: flatten *succeeds*, structural then fails as a
  *singularity*, System Modeler says "Incompatible types". A misleading diagnosis is
  exactly the thing a phase-by-phase view makes obvious.
- **HRW's usefulness is the point of the story rather than an aside**, so nobody has to
  claim it.

`docs/fixture-tours/the-oracle.md` already walks this narrative and would want tightening
for a recording — a demo tour is a third kind after ad hoc and fixture: few stops, no
scrolling, deterministic start, nothing needing a second read.

---

## 1. `Session::remove_document` leaves a stale resolve failure in the resolved-state cache

**Severity:** high for any multi-document consumer. A model that resolves cleanly can be
reported as failing, **with a different file's error**.

**Found:** 2026-07-29, auditing HRW's front-end failure payloads.

### Reproduction

One `Session`, MSL loaded as a durable source root, then three compiles:

```rust
let mut session = Session::new(SessionConfig::default());
// ... load MSL via replace_parsed_source_set(.., SourceRootKind::DurableExternal, ..) ...

// 1. A model that resolves cleanly.
compile("CapacitorLoop.mo");     // resolve: OK

// 2. A model with an undefined reference.
compile("UndefinedRef.mo");      // resolve: "unresolved component reference: 'missingGain'"

// 3. The SAME clean model again.
compile("CapacitorLoop.mo");     // resolve: FAILS, reporting 'missingGain'
```

Each `compile` does `session.remove_document(uri); session.update_document(uri, src);` for
the file being compiled, then `session.resolved()`.

`UndefinedRef.mo` is the only file containing the identifier `missingGain`.

### Expected

Step 3 resolves cleanly, as step 1 did.

### Actual

Step 3 fails, reporting `unresolved component reference: 'missingGain'` — **byte-identical
to step 2's error**, including the ~33 accompanying MSL warnings. The identical text
suggests a cached result is being returned rather than a fresh resolution.

Removing the previous document does **not** help. Rebuilding the `Session` from scratch
does.

### Why this is surprising

`remove_document` → `apply_document_removal_at_revision`
(`crates/rumoca-compile/src/session/session_impl_inputs.rs:139`) **does** call
`invalidate_resolved_state(CacheInvalidationCause::DocumentRemoval)`. So invalidation is
attempted and something survives it.

Suspects, unverified: the `query_state.resolved.builds` cache read in
`build_resolved_with_diagnostics_inner`
(`crates/rumoca-compile/src/session/session_impl.rs:373`) — the `Standard`-mode branch
returns a cached tree and `record_standard_resolved_cache_hit()`; and
`restore_detached_source_root_document`, called during removal, which may put back a
document the caller meant to drop.

### Impact on consumers

Any tool that compiles several models in one session — an IDE, a language server, a batch
compiler, HRW — will attribute one file's resolve error to another. There is no way for the
consumer to tell, because the returned error looks exactly like a genuine failure of the
model it asked about.

### What the workaround costs — measured 2026-08-26

**1.95 s per occurrence.** The rebuild reloads every library root into a fresh `Session`;
it is cheaper than a cold start only because the *parse* is memoised per process, so the
cost is re-loading ASTs and re-resolving, not re-reading `.mo` files.

Measured by `worker::tests::the_stale_cache_workaround_reports_what_it_costs`, which drives
the exact sequence the workaround fires on — healthy model, a model that fails to resolve,
then a *different* healthy model — and reads a counter wrapped around the rebuild.

**How often it fires is order-dependent and is NOT claimed here.** It triggers whenever a
compile follows a resolve failure of a different file, so a consumer that interleaves broken
and working models pays it repeatedly, and one that does not may never pay it at all. For a
test suite carrying a handful of deliberately-broken specimens the total is single-digit
seconds; for an IDE with a file open that does not resolve, it would be every compile.

**A hypothesis this measurement killed, recorded so it is not re-proposed:** HRW's log tests
cost 13.7 s in isolation and 79.3 s inside the full suite, and this workaround was the
leading explanation for that gap. At 1.95 s per occurrence it cannot be — the arithmetic is
an order of magnitude short. The in-suite cost of a compile is dominated by something else,
still unidentified.

### Workaround in HRW

Rebuild the session when the previous compile failed to resolve. Guarded so the library
reparse is only paid after an actual failure. See `WorkerState::last_resolve_failed` and
the regression test `a_broken_specimen_does_not_poison_the_next_compile`.

---

## 2. `validate_type_compatibility` does not fire for connectors with differing member sets

**Severity:** medium. An invalid model is accepted, and the resulting failure is reported
at the wrong phase with a misleading diagnosis.

**Found:** 2026-07-29, authoring a specimen to exercise the flatten failure path.
**Adjudicated by System Modeler**, not argued from the spec.

### Reproduction

```modelica
model IncompatibleConnect
  connector PinA
    Real v;
    flow Real i;
  end PinA;

  connector PinB
    Real v;
  end PinB;

  PinA a;
  PinB b;
equation
  connect(a, b);
end IncompatibleConnect;
```

Compile with `FlattenOptions { strict_connection_validation: true, .. }` — the setting
`rumoca_compile`'s own `flatten_options_for_tree()` uses.

### Expected

Rejected at flatten as a connector type-compatibility error. **MLS §9.3** requires
connected connectors to be type-compatible, and `PinA` and `PinB` have different member
sets.

### Actual

Flatten **succeeds**. The model then fails at structural analysis as *structurally
singular* — a misleading diagnosis for what is a type error at the `connect()`. A user is
sent to look at their equations when the problem is one line of wiring.

### Independent confirmation

Wolfram System Modeler 15.0 **rejects** the same source:

```
SystemModelSimulate::bld:  Failed to build model "IncompatibleConnect".
SystemModelSimulate::bldl: "Error": "Incompatible types. 'a ...  'b' has type 'PinB'."
```

So the model is genuinely invalid and Rumoca is the outlier.

### Note: the check exists and did not fire

`validate_type_compatibility` is at
`crates/rumoca-phase-flatten/src/connections/mod.rs:671`, and `validate_connections`
reaches it when `strict_connection_validation` is on. So this is **not a missing
validation** — it is one that did not trigger for this input.

### TRACED 2026-08-22 — and neither suspect was the cause

**The two suspects previously recorded here — `get_validation_var_info` returning `None`, or
`canonical_type_id` collapsing both connector types to one root — are both wrong.** The cause is
structural and is visible in `validate_expanded_connector_connection`:

```rust
for sub_a in subs_a {
    let Some((suffix_a, ..)) = extract_suffix(sub_a.as_str(), ctx.path_a) else { continue };
    let Some(var_b_match) = find_matching_var_b_indexed(&suffix_a, .., &sub_match_index) else { continue };
    validate_flow_consistency(..)?;   // and type, dimension, quantity
}
```

**Validation runs per matched member PAIR, members are paired by NAME, and an unmatched member is
silently `continue`d.** Nothing anywhere compares the connector *types* themselves, and nothing
compares the member *sets*. For `PinA{v, i}` against `PinB{v}`: `v` pairs and validates cleanly,
`i` finds no partner and is skipped, so the connection is accepted.

**The same hole has a worse instance, confirmed the same day.** Connect an MSL electrical
`Pin{v, i}` to a translational `Flange{s, f}` — **no member name matches at all**, so nothing is
validated *and no connection equations are generated*. Flatten reports success; the model surfaces
three phases later as `still singular after index reduction: empty system: no equations or
unknowns`, which names nothing about the wiring. **System Modeler 15.0 rejects it**:
*"Incompatible types. 'g … Interfaces.Flange_b'."*

```modelica
model ConnectAcrossDomains "no member name matches — accepted, and silently inert"
  Modelica.Electrical.Analog.Basic.Ground gnd;
  Modelica.Mechanics.Translational.Components.Fixed fixed;
equation
  connect(gnd.p, fixed.flange);
end ConnectAcrossDomains;
```

**And the control that shows the validators themselves are sound.** Same member *names*, different
quantities — every member pairs, so `validate_quantity_compatibility` fires and Flatten refuses it,
naming both sides: `incompatible connector types in connection: e.v (quantity: ElectricPotential)
and m.v (quantity: Force)`.

```modelica
model ConnectSameNamesDifferentQuantity "every member pairs — correctly refused"
  connector ElectricalPort
    Modelica.Units.SI.Voltage v;
    flow Modelica.Units.SI.Current i;
  end ElectricalPort;
  connector MechanicalPort
    Modelica.Units.SI.Force v;
    flow Modelica.Units.SI.Velocity i;
  end MechanicalPort;
  ElectricalPort e;
  MechanicalPort m;
equation
  connect(e, m);
end ConnectSameNamesDifferentQuantity;
```

**Both were written as scratch specimens, which are gitignored and do not survive a machine
change** — so the sources are inlined here. This file's own rule is that anything published must be
reproducible, and a maintainer cannot run a model that exists only on one laptop.

So the report should be framed as **a missing connector-level check, not a validator that failed to
fire**: the per-member validators work correctly and precisely — a same-named pair with mismatched
quantities is refused at Flatten with both quantities named — but nothing establishes that the two
connectors are the same type before their members are paired.

---

## 3. Cyclic-dependency diagnostics are nondeterministic across runs

**Severity:** low for a user, **real for CI**. A diagnostic whose text changes between
identical runs breaks golden-file tests and makes log diffs noisy.

**Found:** 2026-07-31, by running the full MSL survey twice and diffing. Not the kind of
thing a single run reveals.

### Reproduction

Compile any of these twice, in separate processes, and compare the error text:

- `Modelica.StateGraph.PartialCompositeStep`
- `Modelica.StateGraph.Examples.Utilities.CompositeStep`
- `Modelica.StateGraph.Examples.Utilities.CompositeStep2`
- `Modelica.StateGraph.Examples.Utilities.MakeProduct`
- `Modelica.Fluid.Examples.ControlledTankSystem.Utilities.NormalOperation`

### Expected

The same diagnostic text on every run.

### Actual

The **outcome is stable** (`failed:ToDae` every time, same error class) but the cycle is
reported **starting from a different member each run**:

```text
run 1: ... cyclic dependency: outerState.subgraphStatePort.suspend -> ...
run 2: ... cyclic dependency: stateGraphRoot.subgraphStatePort.suspend -> ...
```

```text
run 1: ... cyclic dependency: fillTank2.outerStatePort.subgraphStatePort.suspend -> ...
run 2: ... cyclic dependency: wait2.outerStatePort.subgraphStatePort.suspend -> ...
```

### Why it matters

Only 5 of 2,626 models are affected, so the practical impact on a user is small. But a
consumer that stores diagnostics — a golden-file test, a CI log diff, or a checked-in
capability report like `docs/reports/msl-survey.csv` — sees spurious changes with no behaviour
change behind them, which is exactly the noise that trains people to ignore diffs.

**Suspect, unverified:** the cycle is detected over a hash-ordered collection, so the
traversal entry point varies while the cycle's membership does not. A deterministic entry
point — the lexicographically smallest member, say — would fix the text without changing
the analysis.

### Reproduced independently, 2026-08-01

Re-running the whole survey at a **different shard count** (4 against the committed 6) and
diffing column by column reproduced it without looking for it — which is worth stating in the
report, because it means the variation does not need a deliberate repeat run to surface, only
a re-run.

**6 models this time, not 5**, so the count is a floor rather than a fixed set:

```
Modelica.Fluid.Examples.ControlledTankSystem.Utilities.NormalOperation
Modelica.StateGraph.Examples.Utilities.CompositeStep
Modelica.StateGraph.Examples.Utilities.CompositeStep1
Modelica.StateGraph.Examples.Utilities.CompositeStep2
Modelica.StateGraph.Examples.Utilities.MakeProduct
Modelica.StateGraph.PartialCompositeStep
```

**Every one is `StateGraph` or a `StateGraph` user**, which narrows where to look and is
consistent with the hash-ordering suspicion above. The `message` column was the **only**
non-timing column to differ across the two runs — the model set, every structural metric and
every outcome matched exactly.

---

## Index reduction's demoted states are still counted as states downstream

**Found 2026-08-08** while writing `docs/fixture-tours/index-reduction.md` from generated
notebook traces. **Reproduced on two specimens, same direction both times.**

### Reproduction

`cargo run -p hrw --example gen_trace -- Drivetrain`, then read the trace:

| specimen | `index_reduction` `n_states_after` | `solve_lowering` `state_scalar_count` |
|---|---|---|
| `Drivetrain` | **3** | **9** |
| `GearWithBrake` | **2** | **7** |

`Drivetrain`'s index reduction demotes six states — `emf.phi`, `rotor.phi`, `rotor.w`,
`shaft.phi`, `load.s`, `load.v` — leaving `L.i`, `shaft.w`, `mount.s_rel`. Its
`solve_layout.solver_maps.names` nevertheless begins with all nine, the six demoted ones
included.

### Expected vs actual

**Expected:** the solver layout carries the states index reduction left behind — three for
`Drivetrain`, two for `GearWithBrake`.

**Actual:** it carries the pre-reduction count.

### The corroborating symptom

`Drivetrain`'s **initialization is structurally singular**: *"80 matched out of 88 equations and
88 unknowns"*, with `determinacy.states` also reading **9**. That is exactly the outcome expected
if the reduction had not been applied — a high-index system is singular until its constraints are
removed, which is what `index_reduction` reports having done.

### Two readings, and this is where it stops being reproduced fact

**Unverified.** Either

1. index reduction's result is not propagated to the downstream stages, or
2. `state_scalar_count` deliberately counts every variable that ever carried a `der`, demoted or
   not, and the initialization singularity has an unrelated cause.

**Nothing in the traces distinguishes these**, and no suspect source location is offered for the
same reason. Reading (1) would be a real bug; reading (2) would make this a naming problem in the
report. **Adjudicate before filing** — `docs/ideas.md` #43's System Modeler recipe applies:
`Drivetrain` either simulates there or does not, and that answers it.

---

---

## `zero_crossing_conditions` counts a condition the event partition does not contain

**Found 2026-08-16** while writing `docs/fixture-tours/events.md`, which needed to explain a
number Doug can see on screen. Reproduced across five specimens.

### Reproduction

```bash
cargo run -p hrw --example gen_trace -- RcCircuit
```

Then read `docs/specimen-notebook/RcCircuit/trace/events.json`. `RcCircuit` is a plain
source–resistor–capacitor–ground circuit with **no `when` clause anywhere** in the specimen.

### Expected

A smooth continuous model reports no events, and the summary counts agree with the collections
they summarise.

### Actual

```json
"summary": { "condition_equations": 0, "relations": 0,
             "discrete_real_updates": 0, "discrete_valued_updates": 0,
             "zero_crossing_conditions": 1, "scheduled_time_events": 0 },
"conditions": { "equations_f_c": [], "relations": [] }
```

**`zero_crossing_conditions` is 1 while both `equations_f_c` and `relations` are empty.** The
count reports a condition that is not in the partition, so a consumer cannot inspect the thing
being counted.

### The pattern, across the corpus

| specimen | `zero_crossing_conditions` | `equations_f_c` | `relations` | MSL `Resistor`? |
|---|---|---|---|---|
| `RcCircuit` | **1** | 0 | 0 | yes |
| `OverInitRc` | **1** | 0 | 0 | yes |
| `BenchActuator` | **1** | 0 | 0 | yes |
| `Drivetrain` | **1** | 0 | 0 | yes |
| `MotorWithBrake` | **1** | 2 | 2 | yes |
| `GearWithBrake` | 0 | 4 | 4 | no |
| `BouncingBall` | 0 | 1 | 1 | no |

**Every specimen containing `Modelica.Electrical.Analog.Basic.Resistor` reports exactly 1, and no
specimen without one reports any.** `GearWithBrake` has four real condition equations and reports
**0**, so the field is not counting condition equations at all.

### Suspect mechanism — UNVERIFIED

`Resistor.mo:15` is the component's only relation, and it is inside an `assert`:

```modelica
assert((1 + alpha*(T_heatPort - T_ref)) >= Modelica.Constants.eps,
       "Temperature outside scope of model!");
```

**Hypothesis:** the assert's relation is registered as a zero crossing — which is arguably
*correct*, since an assert has to be monitored — but is not then emitted into `equations_f_c` or
`relations`, so the count and the collections diverge.

**Not verified.** The correlation is exact across seven specimens and the mechanism is a guess
from reading one MSL file. Before filing, confirm by compiling a one-line model whose only
content is an `assert` with a relation, and by checking whether Rumoca's event partitioning walks
assert conditions.

### Which bug this is

Two readings, and they need different reports:

1. **The count is right and the partition is incomplete** — asserts must be monitored, and the
   condition should appear in `equations_f_c` so a consumer can see it. This is the more likely
   reading and the more useful fix.
2. **The count is wrong** — assert conditions are not zero crossings and should not be counted.

Either way the current state is inconsistent, which is what makes it filable without settling
which reading is intended.

### Impact on consumers

HRW's Events pane reports the summary counts. On a smooth model it therefore says *"1 zero
crossing"* and can show nothing behind it — a number with no evidence, which is the one thing
`hrw/CLAUDE.md` forbids a pane from doing. `events.md` Stop 2 currently names the anomaly and
points here rather than reasoning around it.

**A one-line fix in either direction removes the need for that paragraph.**

## The index-reduction funnel is not observable, so HRW mirrors its step order

**This is a feature request rather than a bug**, and it is the clearest candidate in this file for
the observability API `hrw/CLAUDE.md` describes as the intended shape of an upstream contribution.

### What HRW does today, and why

`rumoca-sim` index-reduces the DAE inside `solve_lowering/structural_lowering.rs`. **Nothing in the
public surface reports what that funnel did** — which steps ran, what each demoted, which rows were
differentiated. The result is observable; the process is not.

So HRW clones the compiled DAE and **re-runs the funnel itself**, calling
`rumoca_phase_structural::dae_prepare`'s steps in an order it maintains by hand:

```text
demote_exact_alias_component_states, demote_direct_assigned_states,
reduce_constrained_dummy_derivatives, index_reduce_missing_state_derivatives,
demote_states_without_assignable_derivative_rows, eliminate_derivative_aliases,
demote_states_without_retained_derivative_rows, expand_compound_derivatives,
substitute_standalone_state_derivatives_in_non_ode_rows, eliminate_trivial
```

**The steps and the data are Rumoca's; only the order is HRW's** — and that order is a copy of
rumoca-sim's internals, kept in sync by a human on every rebase.

### Why it matters to a maintainer

**A reordering upstream is invisible to the compiler.** Nothing breaks, nothing fails to build, and
HRW's Index Reduction tab keeps rendering a plausible funnel that no longer matches what
`rumoca-sim` does. It is a silent divergence by construction, and the only guard is a step in a
rebase checklist.

**And the same gap produced a documented teaching error here.** HRW's summary
(`differentiated_rows`) reported zero for a model whose captured frames recorded four
differentiations, because the summary scans the *final* DAE while the differentiated rows are
removed by a later elimination step. A tour taught the opposite of what the compiler did for its
whole existence (`DECISIONS.md`, 2026-08-17).

### The ask — ✅ BUILT 2026-08-18, and ready to offer

An **additive, observation-only** hook on the real funnel — a callback or a returned report naming
each step and its outcome, in the order `rumoca-sim` actually ran them. Semantics-preserving; no
behaviour change; the existing entry point keeps working untouched.

**Implemented as `prepare_dae_for_structural_analysis_observed`**, with
`prepare_dae_for_structural_analysis` delegating to it — one implementation, so the two cannot
drift. `FunnelStepFrame` carries the step name, the system's size on either side of it, and a
`FunnelStepOutcome` of `Completed` / `Demoted(n)` / `Failed(_)`; delivery is
`rumoca_core::FrameObserver`, the contract every other traced phase here already uses.

**The `Failed` variant is what makes it a diagnostic rather than a report** — it is emitted before
the error propagates, so a funnel that stops has a location instead of one error at the top naming
none of ten steps.

**And it justified itself on the first run.** The test that pins the step order was drafted from
HRW's mirrored copy of the funnel and omitted `scalarize_equations`, which the real funnel runs
first under default options. **HRW's mirror has never had that step.** The drift this API exists to
remove was already there, and one run of the API found it.

Commit `cc821f07`, `crates/` only, so the cherry-pick is clean.

That would let HRW **delete its mirrored copy**, which is the outcome worth having on both sides:
one fewer place for the order to drift, and a compiler that can explain its own index reduction to
anyone, not only to HRW.

**Unverified:** whether a hook is better placed on the funnel as a whole or on each
`dae_prepare` step. That is a maintainer's call about the crate's shape, and the reason this is
written as a request rather than a patch.

## Index reduction does not reduce the canonical index-3 DAE

**Reproduced 2026-08-18** with `hrw/specimens/CartesianPendulum.mo` — a point mass on a rigid rod
in Cartesian coordinates, the example every treatment of index reduction opens with.

### Reproduction

```modelica
der(x) = vx;
der(y) = vy;
m * der(vx) = -lambda * x;
m * der(vy) = -lambda * y - m * g;
x ^ 2 + y ^ 2 = L ^ 2;
```

Five equations, five unknowns (`x`, `y`, `vx`, `vy`, `lambda`).

### Expected vs actual

**Expected:** the constraint is differentiated twice — introducing first velocities, then
accelerations, at which point `lambda` enters it and the system becomes index-1 and solvable.

**Actual:** every step of the preparation funnel reports zero. States stay at 4, nothing is
demoted, `differentiated_rows` is empty, and the system is left structurally singular:

```text
structurally singular system: 4 matched out of 5 equations and 5 unknowns;
unmatched equations: f_x[4]; unmatched unknowns: lambda
```

Simulation then fails as an unreduced high-index system does, with a step-size message rather
than an index one:

```text
BDF step: ODE solver error: Step size is too small at time = 0.00004774281227423659
```

### The reading, and it is a design observation rather than a defect claim

**Rumoca's index reduction appears to be a set of pattern-based demotions rather than general
Pantelides.** The step names say so: `demote_exact_alias_component_states`,
`demote_direct_assigned_states`, `reduce_constrained_dummy_derivatives`,
`index_reduce_missing_state_derivatives`. Each targets a shape.

The pendulum matches none of them — all four states *have* derivative rows, and the constraint is
nonlinear in two states at once, so no substitution removes it.

**Every constraint in our 24-specimen corpus other than this one is an alias**, which is why this
went unnoticed: `Drivetrain` reduces 9 states to 3 with 6 differentiations and never needs the
general algorithm.

**Unverified, and it is the question for a maintainer:** whether general index reduction for
nonlinear constraints is intended and missing, intended and deferred, or deliberately out of
scope. All three are reasonable answers and the diagnostics would differ for each — at minimum, a
model that cannot be reduced could say so where it is diagnosed, rather than surfacing as a
step-size failure four decimal places into the simulation.

**ADJUDICATED AGAINST SYSTEM MODELER 2026-08-22 — outcome 1, the reading holds.** It simulates
cleanly, and it reduces the system to **two** states by dynamic state selection. The reading above
is no longer an inference; see *The adjudication* below.

### How to adjudicate it — for whichever machine has System Modeler

**Doug, 2026-08-18:** *"I don't have System Modeler on this machine. When I get home I will use
my other machine to test the pendulum specimen with System Modeler."* Written here rather than
left in a conversation, because **the memory store does not travel between machines** and this
file does.

1. Open `hrw/specimens/CartesianPendulum.mo`. It is portable Modelica with no Wolfram
   extensions and no MSL dependency, so it should load as-is.
2. **Simulate it**, 0 → 10 s, default solver. That is the whole test.

**What each outcome means, decided in advance so the result is not read to taste:**

| System Modeler | what it establishes |
|---|---|
| **simulates cleanly** | An independent implementation reduces this system. Rumoca's index reduction is then **narrower than a mainstream compiler's**, and the upstream entry above becomes a well-evidenced gap rather than an open question. This is the expected outcome — it is the textbook example. |
| **fails the same way** | The reading is wrong, or the model is. Check the model first: a bad initial condition (`x=1, y=0` must satisfy `x²+y²=L²`, and it does at `L=1`) would fail in both. **This outcome retires the entry** rather than weakening it. |
| **rejects it at compile time** | The most interesting answer. Read the diagnostic — a compiler that *declines* a model it cannot reduce, and says so, is the behaviour the entry above argues Rumoca should have. |

**Record the result here**, with the version of System Modeler, and update
[`specimen-notebook/CartesianPendulum/purpose.md`](specimen-notebook/CartesianPendulum/purpose.md),
whose Provenance section currently says the round-trip has not been done. **`the-oracle.md` is the
tour for this gesture** and its stop 3 is the worked example of asking the other implementation.

**And it settles charter §4.3 for this specimen**, which requires *"compiles and runs equivalently
in both"* — a bar `CartesianPendulum` currently fails on the Rumoca side, deliberately.

### The adjudication — run 2026-08-22. OUTCOME 1: it simulates cleanly

**Tool:** Wolfram System Modeler **15.0**, via `WSMLink` *"15.0.0 (build ID 7, installer build ID
2) created on Wed 6 May 2026 07:49:21"*, driven from Wolfram Language 15.0.0 for Microsoft Windows
(64-bit), 19 May 2026. Three System Modeler versions are installed on this machine (14.2, 14.3,
15.0); **the build directory confirms 15.0 is the one that ran.**

```wl
Import["…/hrw/specimens/CartesianPendulum.mo", "MO"]
SystemModelSimulate["CartesianPendulum", {0, 10}]
```

**It loaded as-is** — no edit, no MSL, no Wolfram extension — **and simulated the full interval.**
`ExitCode` 0, `SimulationInterval` `{0., 10.}`, 2001 output points, no warning of any kind.

**AND IT REDUCED THE SYSTEM TO TWO STATES, WHICH IS THE PART WORTH MORE THAN "IT RAN".** With
`SystemModelSimulate::ddss` and `::dinit` enabled:

```text
At time 0. s: Dynamic state set no. 0 selection at start {vy}.
At time 0. s: Dynamic state set no. 1 selection at start {y}.
At time 0. s: Initialization of states:
dollar_dynState.set0.x[1] = 0
dollar_dynState.set1.x[1] = 0
```

**Two states, chosen at runtime, against Rumoca's four.** A planar pendulum on a rigid rod has one
degree of freedom, so two states is the right answer and four is the unreduced count. The
`$dynState` sets are the **dummy-derivative method with dynamic state selection** — the
differentiated constraints are kept and, at each point, two of the four candidates are demoted to
algebraic, the choice switching because no single pair stays non-singular through a full swing.

**The physics checks out, so "it produced numbers" is not the whole claim:**

| check | value |
|---|---|
| `lambda` peak | **29.4293** at *t* = 1.775 s, where (*x*, *y*) = (−0.0039, −0.99999) — the bottom |
| analytic rod tension there, *m*(*g* + *v*²/*L*) | **29.43** |
| constraint drift, max &#124;*x*² + *y*² − *L*²&#124; over 0–10 s | **1.23 × 10⁻⁴** at tolerance 1e-6 |

The peak matches to four significant figures at the point the pendulum is lowest, which is where
an independent hand calculation is available. The constraint drift is small and is the residual
one expects from integrating a *differentiated* constraint rather than enforcing the original.

### What this establishes, and what it does not

**Establishes** — the outcome-1 row above, without qualification: an independent, mainstream
implementation reduces this system, so **Rumoca's index reduction is narrower than a mainstream
compiler's**, and the entry above is a well-evidenced gap rather than an open reading. The
corroborating detail is stronger than the bare outcome asked for: the oracle does not merely
succeed, it demotes exactly the two states the physics says are redundant.

**Does not establish**, and these are the over-claims to avoid:

- **Not that Rumoca *should* do this.** Whether general nonlinear-constraint reduction is intended
  and missing, intended and deferred, or deliberately out of scope remains the maintainers'
  question — which is the whole point of filing this as a question. The adjudication removes the
  *"maybe the model is wrong"* branch, nothing more.
- **Not that System Modeler runs Pantelides.** `$dynState` names the **dummy-derivative** method,
  which is what consumes a differentiated system; the messages do **not** name the algorithm that
  decided *what* to differentiate. Do not cite this run as evidence about Pantelides specifically.
- **Nothing about Rumoca's private sim path or CasADi target**, which is `docs/ideas.md` #5's
  separate un-park condition (b).
- **The trace records two events, at 0 s and 10 s** — the interval endpoints. So the output does
  **not** show a mid-run state-set switch, even though the selection mechanism is dynamic. Whether
  switching is reported as an event was not determined.

**Charter §4.3 is now settled for this specimen and still fails**: it runs in one toolchain and
not the other, which is the disagreement the specimen was authored to hold.

## PERFORMANCE QUESTIONS -- not defect claims

**These two are different in kind from everything above, and the distinction is deliberate.**
Nothing here produces a wrong answer. Both are cases where Rumoca does far more work than the
input requires, and in both the current behaviour has an obvious correctness motivation -- so
each is written as a **question to a maintainer**, not as a bug report. `upstream-strategy.md`
calls this shape a zero-cost gift: a reproduction in a few lines, a measurement, and no demand.

**Both were measured 2026-08-21 while working `docs/ideas.md` #48**, where 92 % of HRW's test
gate turned out to be 72 compiles and 10 MSL loads.

---

## P1. A workspace-document change invalidates resolution of durable external source roots

**The question:** should adding, changing or removing one workspace document discard the
resolved tree of a `DurableExternal` source root that did not change?

**Why it matters here:** it is the single largest cost in HRW's test suite. A two-equation
specimen that references nothing from the MSL costs **3.5 s** to compile in a session with the
MSL loaded, and **0.03 s** in a session with no libraries -- a factor of ~100, none of which is
the model's own work.

### Reproduction

Six calls against one `Session` with the MSL loaded as a durable external source root:

| call | time | session state |
|---|---:|---|
| `strict_compile_resolved()` #1 | 1.24 s | cold |
| #2, #3 | **0.00 s** | nothing changed |
| #4 | 1.61 s | after adding one workspace document |
| #5 | **0.00 s** | unchanged again |
| #6 | **1.59 s** | after `remove_document` + `update_document` of **byte-identical** text |

### Expected

A change to a workspace document invalidates that document, and whatever depends on it. The
MSL's 38,855 defs do not depend on it.

### Actual

The whole resolved tree is rebuilt. `invalidate_resolved_state` calls
`ResolvedArtifactState::clear()`, which clears `builds` and `dependency_fingerprints` outright
(`crates/rumoca-compile/src/session.rs`). The per-source-set `source_set_aggregates` that
*do* survive carry only `model_names` and `dependency_fingerprints` -- not the resolved tree --
so `restore_resolved_inputs_from_source_root_aggregates` cannot avoid the rebuild.

**Unverified:** whether a durable external root's resolved tree *can* be kept across a
workspace mutation depends on whether workspace documents can shadow or extend library classes.
If they can, the current behaviour may be the only correct one, and that is exactly what the
question is asking.

### Impact on consumers

Any LSP-shaped consumer -- edit a file, re-resolve -- pays a full library resolution per
keystroke-batch. HRW pays it 72 times per test run.

---

## P2. An artifact-cache miss costs ~21 s of pruning regardless of the input

**The question:** should `maybe_prune_cache_after_write` run on every cache miss, synchronously,
when its cost is a function of the whole cache rather than of what was just written?

### Reproduction

Parse a **five-line** source root that the artifact cache has not seen, three times, each with
fresh content, and read `ParsedSourceRoot::timing`:

| pass | `parse_source_root_with_cache` wall | of which the parse | cache status |
|---|---:|---:|---|
| 1 | **58,243 ms** | 6 ms | Miss |
| 2 | **21,635 ms** | 2 ms | Miss |
| 3 | **21,245 ms** | 2 ms | Miss |

### Expected

Parsing a five-line model costs about as long as parsing a five-line model.

### Actual

~21 s, about **3,500x** the work being cached. None of it appears in `SourceRootCacheTiming` --
`collect_files_ms`, `hash_inputs_ms`, `cache_deserialize_ms`, `parse_files_ms`,
`validate_layout_ms` and `cache_write_ms` together account for a handful of milliseconds. The
only uninstrumented step on a miss is `maybe_prune_cache_after_write`, which walks and prunes
the entire shared cache.

**Unverified:** that the prune is the whole 21 s. It is the only uninstrumented step on the miss
path, but the cost was measured by subtraction, not by timing the prune directly. **Time it
before filing.**

### Why this is surprising

The cost is invisible in the timing struct the same call returns, so a consumer profiling with
Rumoca's own instrumentation sees a 2 ms parse and a 21 s wall clock and has nothing to attribute
it to. Adding a `prune_ms` field would make it self-describing whether or not the prune is
changed.

### Impact on consumers

It makes **cache misses the dominant cost of a cold build**, and it falls hardest on the case a
cache should serve best: many small roots. In HRW it made a single must-fire test cost 39 s, which
is how it was found.

---

## Adding to this file

One entry per bug, and only for bugs **reproduced**, not suspected. Include the
reproduction, expected vs actual, and the suspect code location — but mark suspicions as
unverified, because a confident wrong diagnosis in a bug report wastes a maintainer's time
and costs credibility that this project is trying to build.

Where an independent implementation can adjudicate, **use it before filing** (see
`docs/ideas.md` #43 for the System Modeler recipe). "System Modeler rejects this and you
accept it" is a far stronger report than "I think the spec says…".
