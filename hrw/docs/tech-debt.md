# Tech Debt — HRW Observatory

**Purpose:** the two sweep triggers, the priority order, the lab holes, and the outstanding
debt with its dispositions.
**Status:** authority for when a sweep fires and what it prioritises; record for the debt list
itself.
**Read when:** at a phase boundary, or after noticing that a defect was caught by Doug rather
than by the toolchain. Every sweep starts from the lab holes, then re-measures.

Quality improvements identified by code review, grouped by theme and ordered by severity within
each group.

## The v0.10.0 rebase disabled five surfaces, and one is still owed a re-model

**This is the largest outstanding item and it is not ordinary debt** — it is work Doug ruled into a
specific order (`DECISIONS.md`, 2026-11-02). 0.10.0 re-shaped these phases' data models, so HRW's
panes could not be ported; they need re-modelling onto the new vocabulary. Ports reached the build;
these land afterwards, **one at a time, where each can be watched running** — because a pane
re-modelled blind is how one ends up correct-looking and wrong.

Each row's tag names the symbol its re-model will introduce, so
`doc_citations::claims_of_absence_are_still_true` **fails the moment that symbol exists** and forces
the row to be retired. That is what keeps "temporarily disabled" from becoming permanent quietly.

### ~~Index reduction — the funnel is gone, and the replacement is richer~~

**Re-modelled 2026-11-04**, the last of the five the rebase recruited; emptying
`worker::AWAITING_REMODEL`. The account stays because what the re-model *refused* is the useful
part.

HRW animated a **ten-step dummy-derivative funnel** mutating a `&mut Dae` in place. 0.10.0 deleted
`dae_prepare`, made `Dae` immutable and non-`Clone`, and reduces in **two fixed-point lanes** —
Direct and Holonomic — through a candidate / attempt / selection protocol. Nothing survived the
change, and the pane was *also* re-running reduction on a cloned DAE rather than observing the
compile.

**The replacement is `rumoca_phase_structural::inspect_prepare_for_solve`**, published by
`worker::index_reduction_from_observation`: per-round residue, unmatched equations and unknowns,
candidate counts per proof boundary, every attempt with its outcome **including why it was
rejected**, each selection with residue before and after, pristine retries, the stop reason, and a
snapshot of the stalled DAE when preparation fails. Strictly more than the funnel frames carried.

**One call, two stages.** `inspect_prepare_for_solve` reduces the system *and* returns the initial
value pins, so Index reduction and Initialization now read one observation instead of calling
preparation twice. The second call was not wrong — the function is pure — but nothing held the two
runs together, and `no_stage_reports_an_hrw_sequencing_fault` now asserts they agree about admission.

**What the re-model refused to restore, which is the part worth keeping:**

- **The frame-consuming sub-views stay unavailable on this tab** — matching, Tarjan and tearing
  animations. The reduced system's *report* is observable; its *search* is not, because the
  reduction records rounds rather than the matching passes inside each rebuild. And the existing
  guard would not have caught a mistake here: `TearingAnimation::from_captured` compares
  `blocks.len()` against `segments.len()` and ordinals that run `0..n` in **both** systems, so a
  structural-report-shaped payload would have let the raw system's recorded frames pair silently
  with reduced block names.
- **No name is attached to a candidate's ordinals.** `state_ordinal` and friends index the rebuilt
  system of their own round, so resolving one against the raw DAE gives a confident wrong answer
  from round 2 onward. The span is the stable identity; `declared_at` carries it, and
  `the_reduction_search_log_is_the_compiles_own_run` fails if a later session adds the convenience.

So the three companions resolve rather than wait: `app::structural_frames_for_stage` needs no
`IndexReduction` branch, `app::tearing_dae` needs no special case, and `StructuralView::Animate`
becomes real for the views that do have frames.

**Measured on the corpus, 2026-11-04** — 13 specimens `borrowed` (already index-1), 5 `sorted` after
real rounds, 1 `sorted` with **zero** rounds, 3 `singular`:

| specimen | rounds | states | note |
|---|---|---|---|
| `Drivetrain` | 6 | 9 → 5 | both lanes; round 3 selects a `held` candidate and round 4 then succeeds |
| `GearWithBrake` | 5 | 7 → 2 | 40 records, Direct lane only |
| `CapacitorLoop` | 2 | 1 → 1 | **counts unchanged**; Direct finds 0 candidates, Holonomic sorts it |
| `StabilizedPendulum` | 0 | 4 → 4 | 1 algebraic and 5 equations → **3 and 7**: normalization, no round |
| `CartesianPendulum` | 3 | — | singular; **0 candidates in every lane**, so nothing is attempted |

Two of those rows are the teaching material. **`CapacitorLoop` shows that reduction is not a size
reduction** — a pane reporting only before/after counts would say nothing happened. And
**`StabilizedPendulum` shows `Sorted` with zero rounds**, which is the observable signature of
`derivative_aliases::normalize`: `borrowed_with_observer` reports `Borrowed` when normalization
leaves the system alone and `Sorted` when it rebuilds it. That pass is `pub(super)`, and this is how
its work is visible without instrumenting it.

**`Drivetrain` demotes 9 states to 5, not to 3** as `docs/upstream-issues.md`'s open state-count
entry records. That entry needs re-measuring against 0.10.0 before anything is concluded from it.

### ~~The Index Reduction tab's two displays still speak the funnel's vocabulary~~

**Both re-modelled 2026-11-04**, in the commit after the stage's own. The record is kept for one
reason: **the `unbuilt:` tags on this row named a symbol that was never written.**

They read `reduction_view::ReductionView::from_search` and
`reduction_anim::ReductionAnimation::from_search`. Both implementations are called `from_report`, so
neither tag ever resolved — and `doc_citations::claims_of_absence_are_still_true` **fails only when a
target DOES resolve**, so a tag naming nothing is indistinguishable from a claim that is still true.
`CLAUDE.md` states that exactly — *"a misspelled target is silently permanent"* — and this is an
instance of it produced by the same session that quoted the rule. The mechanism did not fail; the
tag was aimed at a symbol that did not get written.

What landed:

1. **`reduction_view.rs` is a reader for the search log** rather than the funnel report: the
   outcome and its own `meaning`, a before/after size grid with the manifold count, and the rounds
   grouped back out of the flat log, each collapsible with its candidate groups, attempts and
   selection. The grouping **announces anything it could not place** — a record before any round, an
   unknown kind, a record with no kind at all — because the lines around it are positive claims
   about the model that a silent loss would falsify.
2. **`reduction_anim.rs` steps the log**, one observed record at a time, with an opening frame that
   is the system before the search.

**Three renderings are deliberate, each a measured misreading the plainer table invites:** `held` is
glossed as progress (`Drivetrain`'s round 3 moves nothing and round 4 then succeeds); identical
before/after counts get a sentence saying reduction is not a size reduction (`CapacitorLoop`); and a
candidate group of zero reads *nothing to try*, never a refusal (`CartesianPendulum`, which is
`ideas.md` #83 stated precisely).

**The expression-formatter question dissolved instead of being answered.** `abbreviate_expr` and
`expr_to_short` lived in `reduction_view.rs` with about twenty tests, and `alias_anim` and
`ic_plan_anim` were their only consumers — so deleting those two views removed the question of where
an expression formatter belongs. The new panel needs none: the search log carries ordinals, spans
and names, not serialized expressions, and `dae_expr_format` already renders expressions from a live
`DaeView`, which is the better input anyway.

**Each reader has one test that compares it against the writer**
(`the_panel_reads_a_real_compile`, `the_stepper_reads_a_real_compile`). Every other test in both
modules builds its own payload, so all of them would pass against a shape
`worker::index_reduction_from_observation` no longer publishes — they agree with each other and with
nothing else.

### ~~Live reduction stepping needs a public observer, and that is an upstream API question~~

**Closed 2026-11-04, in two commits.** The seam landed in `rumoca-phase-structural` (`4e54fd1d`,
`crates/` only, a clean cherry-pick) and HRW wired it the same day. Kept because the *reasoning* is
what generalizes, and because it is the clearest example this project has of the rule that when HRW
cannot observe something, the answer is to instrument Rumoca.

**What was missing, and why no HRW-side workaround was acceptable.** `inspect_prepare_for_solve`
returns an owned `ReductionReport` *after the run completes*. HRW's live stepping paces a real
algorithm against a breakpoint, so feeding a finished report into the frame channel on a timer would
have looked identical on screen and been **a recording wearing a live label** — the fiction this
repository has already removed twice. So the pane said so and offered nothing, which was the honest
state but not the useful one.

**`CLAUDE.md` is what made this load-bearing rather than optional.**
`PendingLiveDebug::Reduction` is named there as one of the four live paths that re-run a phase which
still exists and *must not be "fixed"* by deletion — and `start_live` returned `None`
unconditionally, so that path was an affordance which armed a breakpoint and reported nothing: the
exact shape that forced the PreLowering variant's deletion. **Either the seam became public or that
path had to go**, and only one of those serves the learning.

**What landed is not what this entry predicted**, which is the part worth keeping. The prediction
was *"export `ReductionObserver` and `ReductionEvent`, and make `prepare_for_solve_with_observer`
public"*. That would have exported the **borrowed callback protocol** — `Identity<'a>`,
`AttemptOutcome<'a>`, `Lane`, `CandidateGroup` — and `observation.rs`'s own module doc draws exactly
that boundary: *"consumers receive only extracted data, never the reduction protocol."* Taking the
prediction would have fought the design upstream chose, for nothing: a watcher needs the record, not
the event that produced it.

So the shape is a `ReductionWatcher` trait receiving the **already-owned** `ReductionRecord`, plus
`watch_prepare_for_solve`. The borrowed types stay private, the owned conversion was extracted to
one `record_of` that both the batch recorder and the streaming forwarder call, and the new public
surface is one trait and one function.

*Read that as a caution about this document rather than about the seam: an entry that names the
implementation is predicting, and this one predicted wrong while being right about the need.*

**A stalled intermediate is deliberately not streamed.** `ReductionReport::stalled` transfers an
owned `dae::Dae` out of the reducer at the discard point — a transfer of ownership, not an
observation — so a watcher handed one could keep the discarded model alive past the run.
`inspect_prepare_for_solve` remains the way to get it.

*Note what this was **not**: it was not `derivative_aliases::normalize`. That pass extends an
implicit derivative system rather than eliminating alias equations, and its work is already
observable — `Stopped::Sorted` with zero rounds, per `worker::reduction_outcome_summary`.*

**VERIFIED BY DOUG 2026-11-04, and the verification earned its keep.** The live path spawns a
thread that waits at `LiveTrace::wait_for_debugger` and then runs a real reduction, so a breakpoint
firing inside the reduction with a candidate half-attempted is the thing being bought — and
`CLAUDE.md`'s standing limit applied: there was no test to write instead.

He pressed Debug and reported *"live debugging is not working… the UI is not updating when I step in
the debugger."* `.hrw-bridge/debug-state.json` settled which half was wrong: stopped at
`live_trace.rs:186` in `live_trace_breakpoint`, with a 21-frame stack running
`watch_prepare_for_solve` → `prepare_for_solve_with_observer` →
`reduce_for_solve_with_observer` → `observation::observe` → `WatchFn::record` →
`LiveTrace::push`. **The seam worked on the first attempt; the pane did not.**

**It found two defects, both in `ReductionAnimation::ui` and both documented verbatim one file
over** in `connection_anim::ui`: no `Playback::sync_live` call, so the channel filled while the pane
redrew its first frame; and a bail-out on `is_empty()`, which is the state a live session *begins*
in. Fixed, and the first is a contract now
(`playback::tests_animated_contract::every_live_capable_view_drains_its_channel`), because it is the
second view to make that exact mistake. Doug confirmed the fix the same day.

**The lesson is about where a rule lives.** Both rules existed, correct and well argued, as comments
beside the code they governed — and so were read only by someone already in that file. Writing a
new animation is precisely the case where nobody is.

### ~~Resolve failure is inferred from diagnostic severities where 0.10.0 states it structurally~~

**CLOSED 2026-11-03, and the answer is that the switch must not happen.** `is_complete()` asks a
**session-wide** question — `ResolutionPlanningTree` is `Complete(_)` or `Incomplete(_)` for the
whole build — so with the 0.10.0 MSL emitting ~75 resolve errors it reads `Incomplete` for *every*
model, including one that resolves perfectly. That is the identical contamination the severity
predicate had, which is what made every specimen's Resolve stage read `Failed`.

**So `is_complete()` is not the structural form of the same question; it is the structural form of
a different one**, and this row proposed replacing a per-model answer with a per-session one.
`strict_compile_resolved`'s own doc comment says they are equivalent — that sentence is wrong in
the same way and is corrected there.

**What replaced both** is the per-model subset: `diagnostic_source` attributes each diagnostic to
the file its primary label names, using Rumoca's own precedence, and only this model's errors count
as this model's failure. That is neither of the two options above.

**The row's instinct was right and worth keeping**: it said the two should be *compared before one
replaces the other*, because a disagreement on a corpus model would be the interesting finding.
They disagree on all 25, and the disagreement found the bug.

*`is_complete()` is still the right call for what it does answer* — whether the session's resolve
completed — and the new `crates/rumoca/tests/suite_core/strict_compile_recovery.rs` pins it in that
role.

*The original row follows, unedited.*

### Resolve failure is inferred from diagnostic severities where 0.10.0 states it structurally

`worker.rs` decides whether a strict compile's resolve actually succeeded with
`resolve_diagnostics_indicate_failure`, which scans `CommonDiagnostics` for any
`DiagnosticSeverity::Error`. It exists because recovery **succeeds past errors `Standard` treats
as fatal**, so an `Ok` return does not mean the model resolved.

**0.10.0 answers the same question at the type level.** The session's planning tree is
`Complete(Arc<ResolvedTree>)` or `Incomplete(Arc<ClassTree>)` — a completed Resolve proof against
a recovery tree — and `rumoca_compile::StrictCompileTree::is_complete()` exposes it. A structural
answer cannot drift if the severity taxonomy changes, which a predicate over severities can.

**Not switched during the rebase, deliberately.** That call sits in `worker.rs`'s compile path,
which charter Decision 12(a) says is extracted *around*, never restructured — so this is a change
to propose and watch, not one to fold into a rebase whose job was to make the old behaviour
compile. The two should also be **compared before one replaces the other**: if they ever disagree
on a corpus model, the disagreement is the interesting finding, and whichever way it goes one of
them is currently wrong about a model Doug can load.

### ~~Solver per-step diagnostics — our instrumentation, not an upstream API~~

**RE-MODELLED 2026-11-03, and it is now an upstream API rather than ours.** The plots have data
again: one row per accepted step, `t` the time the step reached, `h` derived from its endpoints,
`order` the local continuous-extension order.

**The recruitment mechanism did its job, visibly.** This row's `unbuilt:` tag named
`worker::solver_steps_from_integrator`, and `claims_of_absence_are_still_true` failed the moment
that function existed — *"claims `worker::solver_steps_from_integrator` is unbuilt — it exists"* —
which is what forced this retirement rather than leaving a stale claim of absence behind. The
function is named for the tag on purpose.

**What was built, and where.** `rumoca_solver::fmi_me::step_observation`: a borrowed
`MeStepEvent`, an `MeStepObserver` whose `()` implementation is empty, an owned `MeStepRecord`, and
`MeSimulationSession::observe_steps` to install one. `rumoca_sim::simulate_with_optional_step_records`
is the entry HRW calls.

**Three decisions worth keeping:**

- **`observe` returns `()`,** which is load-bearing rather than convenient. The session's
  accepted-step path has no rollback proof from the plugin's `advance` onward, so a hook that could
  fail would be a new way to lose a live session.
- **Emitted from the proved `MeStepProposal`, not the plugin's candidate.** A candidate that fails
  its SPEC_0044 §6 obligations never happened as far as the trajectory is concerned; reporting one
  would draw a step the solver did not take.
- **`Option<Vec<_>>`, not `Vec<_>`.** `None` is *this backend reports no steps* — rk45 never enters
  the ME session — and `Some(empty)` would be *it took none*. Collapsing them would tell a reader
  the false one, which is this file's recurring lesson.

Covered by `suite_core::step_observation` (the steps partition the interval; an observed run is
**bit-identical** to an unobserved one) and
`worker::tests::the_solver_diagnostics_plots_receive_the_steps_the_integrator_took`, which uses
`StiffDecay` and requires the step size to **vary** — a constant series would make the plot a flat
line and hide exactly what it exists to show.

*The original row follows, unedited, because its account of why the plots were kept is still the
reason they matter.*

`rumoca_solver::SolverStepRecord` was **HRW's own instrumentation** of the integrator rather than
an upstream type, so there is nothing to follow to: 0.10.0's `SimResult` carries `times`, `names`,
`data`, `n_states`, `variable_meta` and `termination`, and no per-step record of any kind. This is
the only place in the rebase where HRW loses an observation Rumoca never exposed.

**The plots are kept, not deleted**, and `SimData::solver_steps` keeps its shape — retyped to HRW's
own `plot_labels::StepRow`, which is what they consumed anyway, so re-instrumentation only has to
fill the vector. They were built for a reported failure (Doug, 2026-09-25: he could not verify one
of lecture 1's solver claims, because egui_plot's three-place default rendered a 1e-4 step as
`0.000`), and the step size is on a log axis because measurement showed zoom could not reach it.
Deleting that to save a type parameter would cost real pedagogical work.

**The Simulation pane states the absence** rather than omitting the section, because a missing
"Solver diagnostics" heading reads as a run that took no steps worth reporting — a false claim
about a solver that integrated normally.

The re-model is a solver observation seam, and the shape is already settled by precedent:
`rumoca-phase-structural`'s `observation.rs` threads an observer through one implementation with
`()` as the production observer. A BDF integrator reporting `(t, h, order)` per accepted step is
that pattern applied to a loop rather than a search.

### ~~`pre()` lowering — the pass became one arm of construction~~

**RE-MODELLED 2026-11-03, and it needed no instrumentation — which both this row and
`pre_lowering_anim`'s module doc predicted it would.** That prediction is the interesting part of
this entry, so it is kept rather than quietly corrected: *"the replacement reads construction"*, and
*"one seam there can show `pre`, `edge`, `change` and `previous` being lowered together"*. A seam
would have worked. It was not needed, because **construction's work survives into the artifact**:

- every node the history-operator lowering synthesises carries
  `DaeGeneration::PreValueLowering`, while a `pre()` the model wrote is `Source`
- the owner span on a generated node is the call that produced it, so grouping by span
  reassembles each expansion and the label is read from the span rather than matched by name

So the right first move on a re-model is to **check what the finished IR already carries**, before
pricing a `crates/` change. `worker::pre_lowering_from_construction` is HRW-side only, and
`claims_of_absence_are_still_true` forced this retirement by failing the moment it existed.

**What the pane shows, measured on a four-case probe.** `edge` and `change` are *not* primitives —
MLS §3.7.5 defines them in terms of `pre`, and the IR carries the expansion:

| site | nodes construction created, innermost first | |
|---|---|---|
| `edge(b)` | `pre(b)`, `not pre(b)`, **`b and not pre(b)`** | synthesised |
| `change(b)` | `pre(b)`, **`b <> pre(b)`** | synthesised |
| `pre(counted)` | `pre(counted)` | the model's own |
| `pre(x)` | `pre(x)` | the model's own |

**The old pass could only ever show `pre` alone**, so this is strictly richer, exactly as predicted.
`edge_and_change_expand_into_pre_and_the_ir_shows_it` pins it, and
`the_pre_lowering_sites_are_only_the_models_own` is the must-fire half — a walk that over-collected
would find something in `SingleInertia`, which has no `when` clause at all.

**And the lane is a semantic claim rather than a label.** `pre()` resolves to one of
`PreDiscreteReal`, `PreDiscreteValue`, `PreState` or `PreAlgebraic`; the last two exist because a
continuous coordinate gets its own event-entry snapshot lane, which is what makes
`reinit(x, pre(x) + 1)` read the `x` from event entry rather than the one the reinit assigned.
`BouncingBall`'s `pre(v)` is a `PreState`, and that is why its bounce can set `v` from the `v` it
had on contact.

#### Live stepping is what did NOT come back, and the affordance went with it

`pre()`-lowering live stepping is not built. <!-- unbuilt: worker::pre_lowering_live_trace -->

*(The tag named `pre_lowering_anim::start_live` first and fired immediately: the checker matches a
symbol **tree-wide**, and five other views define a `start_live`. A module-qualified target does not
narrow it. So an `unbuilt:` tag has to name a symbol whose spelling is unique in the tree, or it
reports a claim of absence as false the moment any module defines that name — the mirror of the
misspelling failure the tag's own rule warns about. This one names the HRW entry point the seam
would feed.)*

The old view re-ran `to_dae_with_options_traced`; that function is gone, and re-running
`Dae::construct` would report nothing without a tracing seam inside `lower_history_operator`. **So
`start_live` and `app.rs`'s live-debug gate around it were deleted rather than left returning
`None`**, and `PendingLiveDebug::PreLowering` went with them — a live-debug request no code path can
originate.

`playback::tests_animated_contract::only_the_views_with_a_live_path_report_a_live_session` is what
forced the choice: a view `app.rs` hands a live path **must** delegate `live_state` to its
`Playback`, and a view that delegates while holding only recorded frames answers `Arming` whenever
any session is armed anywhere. There was no third option in which the button stayed and told the
truth.

#### A second specimen gap, found the same way as the `fixed = true` one

**No curated specimen uses `edge` or `change`**, and only `BouncingBall` uses `pre` at all. So the
corpus exercises one of the four cases, and `edge_and_change_expand_into_pre_and_the_ir_shows_it`
has to author a scratch model to cover the other three. This is a *specimen* gap rather than a code
one, the same shape as the `fixed = true` row below — and the same remedy: a curated model with an
`edge`, authored through System Modeler, would cover the expansion the pane exists to show.

### ~~Events — a condition is an algebra now, not an equation~~

**RE-MODELLED 2026-11-03, and the lecture point it promised is now a test.** The pane publishes
Rumoca's own condition algebra: `ConditionOperation` rendered as a term, relations, roots as the
`{relation, activation}` pair they are, and event actions by operation. No instrumentation was
required — `DaeView` already hands out an accessor and an ordered walk per arena.
`claims_of_absence_are_still_true` forced this retirement by failing the moment
`worker::events_from_condition_algebra` existed.

**MLS 3.6 §8.3.5.1 is observable, and `a_vector_when_is_not_a_disjunction` holds it there.** A
vector `when` activates on `edge(b1) or … or edge(bn)`, which is *not* the edge of the disjunction.
Two scratch specimens measured it:

| model | `c[3]` renders as | fires |
|---|---|---|
| `when {u, not u}` | `anyRise(u, not (u))` | at every switch of `u` |
| `when u or not u` | `(u or not (u))` | never |

**Their summaries are identical** — four conditions, one time event, one discrete real equation,
one discrete value definition. **So this is a case where no count could have distinguished two
models with opposite behaviour**, which is the whole argument for rendering the algebra instead of
counting it, and the reason `AnyRise` is published under Rumoca's name rather than expanded into
the `or` form MLS defines it by.

**Three records went with the re-model.** `the_events_summary_uses_rumocas_own_names` pins the
summary keys to `DaeView`'s accessor names *and* each count to the arena it names — checking the
keys alone would pass a summary whose `roots` carried the relation count.
`bouncing_ball_has_events_smooth_model_has_none` was re-keyed onto `event_actions` and now looks
for the `reinitialize` **operation**, because BenchActuator has one event action too and it is an
`assert`. And `events_ir_has_expected_summary_keys` was **retired rather than restored**: all six
keys it asserted were 0.9.20's, and re-keying it would have made a weaker second copy of the first
test.

**One predecessor could not be re-keyed at all, and the reason is worth keeping.**
`every_events_rendering_lands_on_a_real_node` resolved each key of `Stage::rendered` against the
IR, because 0.9.20 attached renderings as a side map where a non-matching key meant *no tooltip,
indistinguishable from a node with nothing renderable*. The re-model has no side map — formatted
text goes into the node — so there are no keys left to resolve. **The silence it guarded survived
the change in a new shape**: every formatter has a fallback (`<relation 3>`, `<unresolved
relation>`, `<state>`), each meaning HRW could not follow a reference Rumoca holds, and each a
plausible-looking string in a pane full of them. `no_events_term_is_a_placeholder` is the
descendant, and it detects `<` followed by a letter — a marker no real Modelica relation can
produce, since every use of `<` in an expression is `<=` or `<` plus a space.

### ~~Every `dae.json` embeds the whole vendored MSL — 320 MiB of library text and 56,232 home paths~~

**Found and fixed 2026-11-04**, while fixing the span resolver next door (see the entry below). It
was a pre-existing condition, not a regression, and it had two independent costs.

`Stage::from_ser(&dae)` serializes the whole `Dae`, and a `Dae` owns its `SourceMap` — which holds
`(id, name, content)` for **every source the compile registered**, the entire vendored MSL
included. So each specimen's `dae.json` carried a copy of the MSL source tree.

**Measured:** 2556 registered files and **15.25 MiB of embedded text per trace**; **342.6 MiB
across 24 `dae.json` files**, of which **320.0 MiB (93.4 %)** was that text; the notebook was
359 MiB in all. The account name of the machine that generated them appeared **2556 times per file,
56,232 times in the notebook**.

**The second cost is the one that outranked size.** This repository is public, and an absolute path
under a home directory carries the account name of whoever ran the compile.

**There was a tension to settle, not just a fix to apply.** `examples/gen_trace.rs` records a
decision of **2026-08-16** declining to rewrite these paths repo-relative, on the grounds that
*"that changes which file the string names"* — an accuracy argument, made about Rumoca's own
rendered strings. **Doug's call, 2026-11-04: drop the text and publish workspace-relative names.**
The 2026-08-16 decision survives where it was actually made — Rumoca's rendered messages are
untouched, and the two that name the root are now a named baseline rather than a blanket exemption.

**What landed, and why not the obvious shape.** `worker::replace_source_map_with_a_portable_index`
swaps Rumoca's node for an id-to-file index. Not Rumoca's wire format with the names edited:
deserializing a `SourceMap` runs `checked_source_indexes`, which **requires
`source_id_for_name(name) == id`** (SPEC_0029 §3a) — a source id *is* a hash of its name — so a
portable name beside the real id is a triple Rumoca would reject, and recomputing the id to match
would detach it from every span in the document. Publishing an HRW-shaped node **that says so in
its own text** is honest where a Rumoca-shaped node that cannot round-trip would not be.

**And the index is the referenced subset, which the measurement earned:** 2556 files are registered
but only **1 to 28 are referenced** by any span in a given DAE, with **nothing unresolvable**.
`SingleInertia` names exactly one file — its own. Publishing all 2556 would restate the MSL's
directory listing 22 times and explain no span that 28 entries do not.

**Result: the notebook went 359 MiB → 18 MiB, `dae.json` 342.6 MiB → 1.7 MiB (203×), and
workspace-root mentions 56,232 → 2** — both survivors being the miette-rendered parse error the
2026-08-16 decision protects.

**The *first* correction this entry owes is to itself.** It said `BouncingBall/trace/dae.json` held
**2863** absolute paths. The measured count is **2556**, one per registered source; 2863 was a raw
text count and is not reproducible. It changed nothing about the decision, which is exactly why it
is worth recording — a number that does not move a decision is a number nobody re-checks.

**Two checks, because the first one could not see this.**
`no_committed_trace_names_a_path_outside_the_workspace` had `dae.json` excluded, and **the
exclusion was not the only reason it would have passed**: Rumoca's source map serializes its
entries as `(id, name, text)` *arrays*, so there was no `file` key for that walk to find. A green
check and a false claim — the right thing measured in the wrong unit, which is this repository's
most frequent defect shape. So `no_committed_trace_mentions_the_workspace_root` asks the privacy
question directly, over every string in every trace regardless of key or nesting, with the needle
**derived from `CARGO_MANIFEST_DIR`** rather than written down — a test that hardcoded the account
name would be the thing it checks for. It was proved to fail against the pre-fix notebook, naming
all 22 files.

### A span was resolved against whichever file the caller happened to hold, and four sites published the result

**Found and fixed 2026-11-04.** Kept here because the *shape* is the lesson and it recurs: a
documented safety property that nothing checked, holding up four publishers.

`span_to_location` took one source text and sliced it. Its callers' comments claimed the safe
behaviour — *"a label whose span points into a library file resolves to `null` … it refuses rather
than inventing a line"* — and the only guard was `end > bytes.len()`. A span into an MSL file whose
byte range happened to fit inside a short specimen resolved against the wrong file and reported a
line with full confidence.

**Live, across most of the corpus.** `Drivetrain`'s four unmatched unknowns were published as lines
2, 6, 8 and 10 of `Drivetrain.mo` — **four of its own comments**. The truth is
`SI.AngularVelocity w`, `SI.AngularAcceleration a`, `SI.Acceleration a` and `SI.Force f`, in four
different MSL files. `CapacitorLoop` named `connect(src.n, gnd.p);` for a span that is
`flow SI.Current i` in `Electrical/Analog/Interfaces/Pin.mo`.

**Two checks were complicit rather than absent**, which is what makes this worth keeping:

- `a_structural_failure_names_the_source_line` **asserted the wrong answer** and passed. It pinned
  `line == 9` plus the `connect` line text for `gnd.p.i`. The `Pin.mo` declaration is *also* on
  line 9, so the line check agreed by coincidence while naming a different file — and the text
  check was reading the same wrong file. **Two consistent wrong readings look exactly like one
  right one.**
- `fidelity::a_rumoca_failure_is_represented_faithfully` verified each location's `line_text`
  against **the specimen**, and so certified every one of these as *"source locations verified"*.
  The checker embodied the same assumption as the code it was checking.

`Dae::source_map()` and `SourceMap::get_source()` were already public, so the fix needed no Rumoca
change: resolve a span in the file its own `SourceId` names, and publish `file` beside `line`,
because a line number means nothing until the reader knows which file it counts lines in. The
fidelity checker now reads the named file, so it would catch this rather than bless it, and a
location with no `file` is a violation in itself — which is the shape the old resolver produced.

**The remaining gap is narrow and stated.** The diagnostics path is reached from
`PhaseResult::Failed`, where no `Dae` exists, so it is told the specimen and *refuses* every
foreign span — `null` rather than a wrong line. `ModelDiagnostics` carries a
`source_map: Option<SourceMap>`, so resolving a library label in its own file there is available
and not yet done.

### No curated specimen declares `fixed = true`, so the Initialization pane is empty across the corpus

**Found 2026-11-03** while re-modelling that pane. Every curated specimen states `start = …` and
none states `fixed = true`, so no model in the corpus produces a single initial-value pin. The pane
is correct and informative about it — `SingleInertia` reports `only_guessed: ["phi", "w"]` — but
nothing in the corpus exercises the pin rendering, and
`the_initialization_pane_shows_each_pin_and_its_role` has to use a scratch specimen to cover it.

**This is the same shape as F10's absence clause**, which went uncovered for three months for
exactly this reason, and it is a *specimen* gap rather than a code one.

**It is also not an accident, and that is the interesting part.** A bare `start` is a guess the
initialization solve may move; `fixed = true` is an assertion it must satisfy. The corpus states
guesses throughout, which is why the initialization defect in `docs/upstream-issues.md` could
affect so much of it: with nothing pinned, every reported initial value came out of the solve, and
on 0.9.20 the solve returned the value that zeroes the derivative.

**What would close it:** one curated specimen in the arc-1 style whose states are genuinely pinned,
and one where a value is *transferred* onto a state through a proved equality — the second being
the case the pane's `source` column exists for, and the one a reader is least likely to predict.
Both want the full curated ritual (System Modeler authoring, `// purpose:`, a notebook trace,
`purpose.md`, the corpus baseline), so this is a specimen errand rather than a code change.

### ~~Initialization — `ic_plan` became `initial_pins`~~

**RE-MODELLED 2026-11-03.** The pane publishes Rumoca's own
[`InitialValuePin`]s — each an MLS 3.6 §8.6 initial equation rewritten onto the coordinate it is
about — plus a determinacy summary. `claims_of_absence_are_still_true` forced this retirement by
failing the moment `worker::initialization_from_initial_pins` existed.

**What the pane shows that the old one could not: the role.** A `Definition` determines a state's
initial value and stays an equation in the joint solve; a `Check` stays a residual so the
initialization instant answers it with numbers. A sequence of `IcBlock` solve steps said *how* the
values are computed; the roles say **which stated values are load-bearing and which are
consistency conditions the model must satisfy.**

**Two things were measured rather than assumed, and the first one corrected a wrong design.**

1. **A pin is not a state's own `fixed = true` start.** The first version counted those and
   reported every corpus model as having no pins — correctly, but it also reported
   `determined = 0` for models whose states *are* determined. A pin records an initial equation
   that had to be **proved**: both producing paths in `initial_pins.rs` skip `role() == State`.
   Measured on a two-line probe: `Real y(start = 2.0, fixed = true)` with `y = x` produces a
   `Definition` pin **on `x`**, sourced from `y`. So the determinacy count reads *both*
   mechanisms, and `only_guessed` names the states left over.
2. **No curated specimen declares `fixed = true`.** Every one states `start = …`, which is a guess
   the solve may move. So the pane is empty across the corpus — honestly — and the test uses a
   scratch specimen covering all three mechanisms at once. **That is a specimen gap worth its own
   row**, and it explains the shape of the initialization defect `upstream-issues.md` carries:
   with nothing pinned, those models' initial values come out of the solve, which is where 0.9.20
   took the derivative-zero answer instead of the start. `SingleInertia` reports
   `only_guessed: ["phi", "w"]` — that list *is* the blast radius.

**Provenance is `Compiler`, not `Hrw`.** `prepare_for_solve` is Rumoca's function over the
compile's own immutable `Dae` and HRW invents no value — the same relationship `structural_stage`
has to `inspect_structural_report`. Solve lowering prepares internally and returns nothing, so
this second preparation is the only available route; unlike the old funnel it cannot disagree with
the compile's, because `Dae` is immutable and `prepare_for_solve` takes `&Dae`.

*When the index-reduction pane lands it will want `inspect_prepare_for_solve`, which does this same
work and returns the `ReductionReport` too — one call should then serve both stages.*

*The original row's account follows unedited; only its claim-of-absence sentence is gone, because
it is no longer true.*

`ic_plan::{IcBlock, build_ic_plan, build_ic_relaxation_hint}` are gone, replaced by
`initial_pins::{InitialValuePin, InitialValueRole, PinTerm}`. The determinacy check HRW computed
alongside the plan read `v.fixed == Some(true)` over a partition that no longer exists;
`VariableView::fixed_uniform()` and `fixed_scalar()` are its 0.10.0 equivalents, and they are
richer — MLS §4.8 allows `fixed` to differ per element, which the old single `Option<bool>` could
not express.

**A closed item is cut down to one line in *Closed debt*, not left checked in place**
*(2026-08-01, matching `ideas.md`)*. A debt file is for what has not been paid; a list of
ticked boxes reads like outstanding work and hides the items that are.

**Trigger: each phase boundary, scoped to what the next phase touches** — adopted
2026-07-29, replacing a weekly calendar scan. Doug, on the change: *"my original
tech debt schedule is a prime example of old fashioned software development
mentality where teams have to agree upon weekly schedules and other such stuff
which absolutely do not matter for this project. We are entirely agile."*

**A THIRD TRIGGER, ADDED 2026-08-19: SCHEDULED AND COMPREHENSIVE, TO COVER OUR BLIND SPOTS.**

**Doug:** *"it's possible and perhaps likely that both of us has blind spots and is not
recognizing that we are building new code atop defective code… we are going to protect the code
from our blindness by also performing scheduled comprehensive tech debt sweeps."*

**This is not the weekly scan rejected in July.** That was refused on ceremony grounds — *"old
fashioned software development mentality… we are entirely agile"* — and this argument is
different in kind, which is why it is added rather than a reversal. **A forward-looking sweep is
structurally incapable of finding what neither of us anticipates**, because its scope is drawn
from what we expect to touch.

**2026-08-19 demonstrated it.** The lab transport bar's non-monotonic width had `MIN_LEFT_POINTS`,
two divider-test thresholds, `MAX_LAB_CHROME` and the picker's adaptive width all tuned *around*
it. **It was never on anyone's list** — nobody planned to touch that bar until a Back button
needed a slot — so no targeted sweep would have scoped it in. It was found by running into it, at
the cost of an evening and two dead theories.

**During a comprehensive sweep, general software-engineering principle is admissible grounds** —
including refactoring for size, which the targeted triggers deliberately refuse.

**With one guard, because the failure mode of a scheduled sweep is churn that looks like
progress:** `format-and-app-plan.md`'s rule applies to every change made on principle alone —
**no extraction lands without a test that could not have been written before it.** A refactor
that enables no test needs a justification other than the principle.

**Why it matters more than tidiness — FIXABILITY DECAY, and this file is its home.** The cost of a
deferred fix scales with **how much has come to depend on the broken behaviour**, and a blind spot
is precisely where dependants accumulate unobserved. **So the expensive defect is not the severe
one, it is the early one**: severity is fixed at discovery, while dependants keep accruing.

*(Re-homed 2026-09-24. This sentence cited `hrw/CLAUDE.md`'s "fixability decay" — a rule the
2026-09-01 compression cut, leaving the citation pointing at nothing and the reasoning surviving
only as this paraphrase of it. **Two checkers would each have missed it**:
`qualified_citations_resolve` reads backtick citations to `crates/`/`src/` paths, and
`every_markdown_link_in_a_governing_document_resolves` reads `[text](path)` links. **This was
neither — a prose citation to a NAMED RULE inside another document**, which nothing resolves. It
is recorded under "who caught it?" below.)*

**Doug named the extension on 2026-09-24**: the 2026-08-19 argument is not only a case for
*scheduled* sweeps but for **opportunistic hunting during ordinary work** — *"it is good for this
project when you invest extra time and effort in identifying and fixing defects."* The instance
that day: the specimen ladder had been written the previous day citing `LoopWithInertia` as *"4 of
4 series move"*, which was true and meant nothing, because the state sat flat at its steady value
and moved by 1e-8 of drift. **Curriculum had already been built atop it**, which is exactly the
shape 2026-08-19 predicted.

The rules, in full:

- **Measure, never re-estimate.** This rule keeps paying. The 2026-07-28 sweep found
  `ui()` had grown 385 lines in a day, unnoticed. The 2026-07-29 sweep found `compile()`
  at 363 where 327 was logged. **The 2026-08-01 measurement found `app.rs` up 42% in three
  days and `compile()` refactored out of existence** — a logged item resolved by work
  nobody connected to it. Estimates would have missed all three.
- **Three outcomes are all legitimate:** *fixed*; *closed as obsolete* (the batch
  narrative workflow — the narratives it served were retired); or *deferred into the
  phase that will rewrite it*, when sweeping would mean designing an abstraction
  before its requirements exist.
- **Skip what the next phase will rewrite.** Applied three times on 2026-07-29.
- **Start from the lab holes.** They are the only items that arrive with evidence that
  they blocked a real answer, which makes theirs the only priority that is not a judgement
  call. **All four logged holes are currently closed** — so a sweep starting there today
  starts empty, which is the correct outcome and not a reason to skip the step. The
  section's *recording discipline* is the part that stays live.

Previous cycles: 48 items fixed across two passes (2026-07-22), plus 22 in the
2026-07-25 cycle, 25 in the 2026-07-25 sweep, and 20 in the 2026-07-26 sweep.
See git history for details.

### A second trigger: code that costs mistakes rather than code the next phase needs

Added 2026-08-01, from Doug:

> We did not add to our tech debt log an item for asking ourselves whether our current code is
> causing too many mistakes or is reducing our feature velocity.

The existing trigger looks **forward** — what does the next phase touch? This one looks
**backward** — what has actually been costing us? They are complementary, and the second was
missing.

#### Ask "who caught it?", not "did it feel hard?"

**"Too many mistakes" is not measurable and would become a vibe.** The observable version is:

> **Did the toolchain catch this defect, or did a human?**

If `cargo`, `clippy` or a test caught it, the environment is doing its job and there is nothing
to sweep. If **Doug** caught it — by noticing a missing line, a frozen number, a claim that did
not match the data — then the code lives somewhere nothing checks, and *that* is the debt.

Evidence from 2026-08-01, which is why this trigger exists. Every silent defect that day was
at or inside a **language boundary**: an array argument collapsed by `powershell -File`, a
formfeed injected into a path by a Python escape, an `eprintln!` swallowed by HRW's own
fd-level `OutputCapture`, a rate limiter that gated its own first fire, an announcement that
stayed silent when models were pending by absence. **None was in `fidelity.rs`**, which is
Rust, tested and clippy-clean.

#### The property is verifiability, not Rust

Rust is the proxy, not the point — a Rust program with no tests is no better than a shell
script. What actually differs:

| | Verifying environment | Not |
|---|---|---|
| type errors | compiler | nothing |
| logic errors | the test suite | nothing |
| silent no-ops | non-vacuity guards | nothing |
| who verifies | **the toolchain, in seconds, unattended** | **a human, watching output** |

So the sweep question is *"can the toolchain check this?"* — and **converting to Rust is only
one of the available answers.** Often cheaper and sufficient: add a test, add a non-vacuity
guard, or make the thing fail loudly instead of silently.

#### When it should fire — all three, not any one

1. The code is **re-run repeatedly** (a runbook step, not a one-off).
2. It can **fail silently** — producing nothing looks like having nothing to report.
3. It has **already produced a defect only a human caught**.

One or two of these is not enough. A five-line shell command that runs once and fails loudly is
fine forever.

---

## Rework rate is NOT measurable from this history — two attempts, 2026-08-23

**Proposed as a code-wall instrument and abandoned on measurement.** Recorded so it is not
re-proposed: the reasoning is appealing, and both classifiers fail for different reasons.

**Attempt 1 — classify commits by message.** *"A fix-shaped commit touching a file changed in the
previous 7 days."* It produced a clean-looking trend: **0 % rework before week 33, then 40 %.**

**It was measuring the commit-message convention.** This project wrote `hrw: …` for 319 commits
before adopting `fix(hrw): …`, so weeks 30–32 showed **144, 302 and 147 commits with zero fixes** —
which is not plausible and is the tell. Presented without that check it would have read as a
dramatic regression beginning in week 33.

**Attempt 2 — read no messages at all.** *"A commit touching a file that ≥ 2 other commits touched
in the preceding 7 days."* Convention-independent, and **it saturates: 94 % of all commits qualify**
(88–96 % every single week).

**The cause is tempo, not coupling.** 701 `hrw/` commits in five weeks — **288 in one week** — into
about twenty files. Structural co-occurrence carries no information at that rate, and a column that
reads the same on every row is the failure `docs/ideas.md` #52 already records.

### The conclusion, and it redirects rather than just closing

**The divider/transport-bar arc *is* real rework** — but it was identifiable only from what the
messages *said*: *"second theory dead"*, *"passes by coincidence — five measurements"*, *"unfixed"*.
**Rework in this repository is a semantic property, not a structural one**, so the only instrument
that can see it is a curated one.

**That instrument already exists: the "who caught it?" ledger below.** No new metric is needed, and
the earlier recommendation to *"keep rework rate on the list"* was wrong — this is the evidence.

**What would reopen it:** a sustained drop in commit tempo. At a few commits a day into a larger
file set, attempt 2 would discriminate again.

## The "who caught it?" ledger

**Doug, 2026-08-16, asked the question this exists to answer:** *"Are you confident that we will
now experience less drift and fewer mistakes?"* The honest reply was that nobody could know,
because **the trigger above was never tallied** — it decided individual sweeps and left no
record, so "are we improving?" stayed a matter of opinion.

**Append one row per defect, as it is found.** Cheap, and it converts a feeling into a number.

**Read the ratio, not the rows.** If it moves toward the toolchain, the checks are working. If
Doug keeps finding several a day, the checks are decorating a process that has not changed —
and that is a result worth having either way.

**Three columns, because two would hide the interesting case.** A defect Claude *introduced and
caught in the same hour* is not the same as one that survived a week: the first says the
verification loop is tight, the second says something is unwatched.

| date | defect | caught by | note |
|---|---|---|---|
| 08-15 | `f_x[N]` ids were decorated labels; deixis resolved to nothing | **Doug** | pointing at a cell |
| 08-15 | the id fix covered one of three writers | **Doug** | "I rebuilt before running HRW" |
| 08-15 | ids missing from `matching`/`blocks` too | **Doug** | "Did you fix it for all RHS panes?" |
| 08-15 | `architecture.md` stale ×3 | toolchain | `architecture_regions_are_current` |
| 08-15 | notebook 25 days stale | Claude | found while regenerating for the id fix |
| 08-15 | coupled-block assertions never executed | Claude | writing the id checker |
| 08-16 | no hint why `C.v` is a state | **Doug** | running the lab |
| 08-16 | the Why explanation was hover-only, undiscoverable | **Doug** | "I don't see what you've added" |
| 08-16 | `view.json` stopped publishing what the pane drew | Claude | prompted by Doug's question about the labs |
| 08-16 | `tests/msl_resolve.rs` unrun since ≥08-05 | **Doug** | "Do all of the checkers run?" |
| 08-16 | connections pane nested three scroll areas | **Doug** | lists not using vertical space |
| 08-16 | lab link hooks never cleared; first click masked the rest | **Doug** | frame 7 and 13 |
| 08-16 | link navigation dead for 9 of 11 stages | **Doug** | *"still not causing navigation"* — after a wrong fix |
| 08-16 | divider learned a fraction from a pinned width | **Doug** | maximize from normalized |
| 08-16 | tests treated the live Answer as scratch (×3) | Claude | writing one for Doug to test |
| 08-16 | picker layout regressed the divider | toolchain | `the_left_panel_content_never_detaches…` |
| 08-16 | duplicate `#[test]`; stolen `#[test]` | toolchain | `no_function_has_two_test_attributes`, `dead_code` |
| 09-24 | ladder judged rungs by "series that move"; `LoopWithInertia` + `BenchActuator` flat-lined while recorded healthy | **Claude** | went looking; curriculum already built atop one |
| 09-24 | `upstream-issues.md` said an isolation "rules connectors in"; connectors are neither necessary nor sufficient | **Claude** | fell out of the row above |
| 09-24 | HRW ignored the model's `experiment(Tolerance)`; three other in-tree consumers honour it | **Claude** | probing a thing Doug correctly said was not a defect |
| 09-24 | four false greens in one UI test — a vacuous assertion, plus two helpers that panic on 2+ and on 0 matches | **Claude** | verified each by swapping the defaults |
| 09-24 | `tech-debt.md` cited `hrw/CLAUDE.md`'s "fixability decay"; the 09-01 compression cut it | **Doug** | correcting Claude's framing of his own preference |
| 09-25 | plot tooltips rounded every solver value to `0.000` — egui_plot's `{:.3}` default | **Doug** | could not verify one claim in lecture 1 |
| 09-25 | step size and BDF order shared a linear y axis; the h line got 0.06 % of the height | **Doug** | asked for zoom; zoom already existed and could not reach it |
| 09-25 | the three plots' x axes did not align — BDF steps 2.36 % past `t_end` | **Doug** | **two wrong hypotheses first**; see below |
| 09-25 | `experiment(Tolerance)` was never read by HRW | Claude | probing a thing Doug had correctly called a non-defect |
| 09-25 | the BDF order series was drawn as a LINE, asserting fractional orders the solver never ran | **Doug** | asked whether he needed a step table; the answer was no, and this fell out |
| 09-25 | `SolverStepRecord::h` is the NEXT step, not the one taken — 11 of 34 records disagree with `t_n - t_(n-1)`; its doc says *"Step size used"* | Claude | checking which interval each staircase value belongs to |
| 09-25 | lecture 1's (4.1) `error ~ h_0^2` was **refuted by the table printed under it** — two rows share `h_0` and differ 66x | Claude | auditing section 4 to answer Doug's table question |

**08-15 and 08-16: Doug 10, toolchain 5, Claude 5.** The Doug column is dominated by **things that
are true on screen** — a pane that shows no reason, a link that does nothing, a divider in the
wrong place. `CLAUDE.md` already records that `egui_kittest` cannot see layout, and half of these
confirm it. **The toolchain column is dominated by Claude's own fresh mistakes**, caught within
minutes, which is the loop working as designed.

**09-24: Claude 4, Doug 1 — and the shape is new enough to be worth naming.** All four of Claude's
came from **going looking rather than from something looking wrong**: three of the four were
*green* beforehand, and one had a ✅ beside it in a document written the previous day. **The
toolchain column is empty**, which is the honest reading — no check existed for any of them, and
for two of them none could have, since a movement count cannot distinguish a trajectory from drift
around a wrong constant and nothing resolves a prose citation to a named rule in another document.

**Doug's one row is the interesting one**: he caught it by *correcting Claude's framing of Doug's
own preference*, which is not a review activity at all. It is the second instance of a defect
surfacing from a conversation about something else.


### 09-25, the x-axis misalignment — **the detail Doug volunteered was the whole diagnosis**

**Three hypotheses, and his one extra sentence killed the first two.** He reported that the
1-second ticks did not line up, and added, unasked: *"Interestingly, the 0-second ticks for all
three plots are aligned."*

| # | hypothesis | what it predicted | outcome |
|---|---|---|---|
| 1 | differing y-axis widths | left edges differ | **excluded by his sentence** — aligned zeros mean the left edges already matched |
| 2 | differing allocated widths | same left edge, different pixel spans | plausible; fixed it anyway, **and the ticks still did not align** |
| 3 | differing x DATA ranges | same left edge, same span, different mapping | **correct** — solver steps reach `t = 1.0236` against a trajectory ending at `1` |

**Aligned zeros were the constraint that mattered**, because both ranges start at zero and only the
far end could diverge. Claude used it to exclude (1) and then failed to see that it equally
*implied* (3); it took the second fix failing to force the reading.

**And the cause was not a layout bug at all.** BDF steps past `t_end` and interpolates the output
back, so the step records outrun the last trajectory sample by one step — 2.36 % on
`SingleInertia`, 0 % on `BouncingBall`, whose steps never exceed `3e-3`. **A UI symptom whose cause
was solver behaviour**, which is why reading egui_plot's rect derivation — it reserves nothing on
the right — was the step that made further layout hypotheses pointless.

**What it cost and what it bought.** Two commits that did not fix the reported defect, both kept:
the equal-width change is a latent version of the same bug, and the y-axis floor was genuinely
needed. **And the overshoot is now a sentence in lecture 1 §4**, which had listed three ways
Rumoca differs from hand-worked Euler and should have listed four.

**The rule this sharpens** — `CLAUDE.md`'s *mine the report for its constraining noun*: Doug's
volunteered details are not colour. Here one clause eliminated a hypothesis and pointed at the
answer, and the cost of half-using it was two wrong fixes.

### 09-25 → 11-03, the v0.10.0 rebase — **the ratio inverts when the subject is documents**

**Appended 2026-11-03.** One long arc rather than a run of days, and it splits cleanly by *what
was wrong*. Against code, the checkers did their job. Against **prose**, almost nothing was caught
by a test, because what broke was a claim's truth rather than its form.

**Caught by the toolchain or by a test (code):**

| what | what caught it |
|---|---|
| `worker.rs` produced `tearing_passes()` where every consumer expects block segments | my own `a_real_algebraic_loop_is_traced_end_to_end` |
| a test written against `Underdetermined`, which balance checking rejects before structural analysis ever runs — **it would have passed vacuously forever** | probing the early-return paths with deliberate panics |
| seven observation tests that would all still pass with **every `emit` deleted** | mutation testing, three mutations, each failing exactly one test |
| `inspect_sort` published with no caller inside `crates/` | `public_api_surface` |
| a helper inserted between `#[test]` and its `fn` | a `Termination` trait error — the documented anchor hazard, again |
| `ClaimedPreference` / `PreferenceUnavailable` emitted through a variable, so the generated reference silently omitted both | the generator's own output, read back |
| the Station 2 rewrite dropping the live lab from five cited source lines to three | `every_line_the_live_lab_cites_is_a_real_anchor`'s non-vacuity guard |
| `"walked"` in brand-new lab prose | `no_lab_uses_retired_vocabulary`, within a minute of writing it |
| a **second opening summary** in `derivative_evidence`, whose live summary still named a parameter the 0.10.0 port had removed | `no_doc_block_gains_a_second_summary` |

**Caught by reading, because nothing could check it (documents):**

| what | why no check existed |
|---|---|
| `matching-live.md` Station 2 asserting *"the main thread's 8 MB stack"* and *"a 500,000-node chain is a real MSL shape"* — **both invented**; the source names no size and no length, and the figure is 1 MB on Doug's platform | nothing compares a number in prose against a source that carries no number |
| `debug-set-sites.md`'s entire breakpoint table naming `emit_matching_frame` and `TracedTarjanState::record` — **the duplicate traced implementations this rebase deleted** | the paths resolved; the function names were never checked |
| `architecture.md` explaining *why HRW replicates the index-reduction funnel*, work HRW no longer does at all | a section can describe a removed capability and stay well-formed |
| `flatten.md`'s thesis — *"only one of the two connection graphs is confined to this phase"* — **inverted** by the overconstrained check moving into Flatten | the claim is about where a check runs, which no citation encodes |
| `rust-for-cpp-readers.md` reporting `unsafe` in three `dae_prepare/` files; the read path now has **none at all**, across nine crates | a stale measurement reads exactly like a current one |
| CJK characters written into the doc comment of the very test that forbids them | caught by running that test — the one case where the mechanism did reach prose |

**And one gap in the instrument itself, found by reading its output rather than by its failing:**
`every_documented_source_path_exists` recognised `crates/<crate>/src/…` but **not** the shorter
`<crate>/src/…`, which these documents use just as often. **Eight dead paths were invisible**,
including `dae_construction.md`'s entire module table. Widening the extractor took the run from 11
dead citations to 49. *Who caught it: reading, which is the trigger — the second spelling lived
somewhere nothing resolved.*

**The entry worth keeping, though, is Doug's, and it was not a bug report.** Told that whether a
rewritten lab station *reads* was his to judge, he asked: *"So am I supposed to run HRW now to
verify that a lab station works?"* The answer was no twice over — it is a live-debugger lab on a
tree with failing tests, and the sentence was the lab-progress pestering `CLAUDE.md` forbids,
wearing humility as a disguise. **Checking why it was wrong is what found the two invented numbers
above.** A push-back on *process* located a defect in *content*, which no amount of running HRW
would have surfaced.

**So the ratio, honestly stated:** nine code defects caught by mechanisms, six prose defects caught
only by someone reading, and one checker gap that hid eight more. **The instrument is in good
shape where it points at code and is nearly blind where it points at claims** — which is the same
boundary as *effectiveness is Doug's*, reached from the maintenance side.

<!-- ledger-through: 2026-11-03 -->

### 08-17 → 08-22, backfilled 2026-08-23 — and the ledger had gone dark

**Seventeen rows, all from two days, then nothing for a week** — through the transport-bar arc,
two defects on 08-21, a full day on 08-22 and a night of unattended work. **The instrument did not
fail; the habit did**, which is why appending is now checked by
`doc_citations::the_who_caught_it_ledger_keeps_up_with_the_work` rather than remembered.

Rows marked *(inferred)* were attributed from the commit body rather than from a contemporaneous
note; the rest quote the commit.

| date | defect | caught by | note |
|---|---|---|---|
| 08-17 | a new lab opened partway down, not at its top | **Doug** | *"When I click a subordinate lab link in the-concepts hub…"* |
| 08-17 | a stop link did not land at its stop | **Doug** | same run *(inferred)* |
| 08-17 | lab `kind` markers were visible in the pane | **Doug** | *"is now visible in the…"* |
| 08-17 | a stage summary claimed more than its frames recorded | Claude | following Doug's *"way, way too short"* |
| 08-19 | eighteen lab references opened a browser | **Doug** | *"I found a broken link in blt-ordering"* |
| 08-19 | the time selector took egui's default width | **Doug** | *"Why is the combobox so wide?"* |
| 08-19 | the alias view stranded on a stage with no alias tab | Claude | found by a pure function an extraction had produced |
| 08-20 | a replay restarted after passing through a report stage | Claude | cache-lifetime split *(inferred)* |
| 08-20 | the navigated tree was annotated from the specimen | Claude | app.rs extraction *(inferred)* |
| 08-20 | the module-size scan had the failure it was written to prevent | Claude | a floor only ever sees the row above it |
| 08-21 | the Flatten sub-view row stranded | Claude | column read |
| 08-21 | a test named `…matches_worker` never touched the worker | Claude | reading `worker.rs` for #48 |
| 08-22 | Look links recompiled an already-loaded specimen | **Doug** | *"even if those specimens are already loaded"* |
| 08-22 | the lab implied HRW represents a "node"; so did a doc comment | **Doug** | re-running connections |
| 08-22 | `connect` called one edge, is one edge **per member** | **Doug** | *"That seems wrong"* |
| 08-22 | the 08-12 lab repair was lost to the 08-13 rewrite | Claude | investigating Doug's question |
| 08-22 | the mandatory reading path was growing unchecked | Claude | answering Doug's question about the arrangement |
| 08-22 | three scratch-specimen tests passed vacuously | Claude | while fixing an unrelated assertion |
| 08-22 | a `##`-heading edit silently rewrote the catalogue blurb | toolchain | `lab_catalogue_is_current` |
| 08-22 | an `**Expected:**` marker was removed | toolchain | `every_station_of_every_lab_owes_an_expected` |
| 08-22 | `describe`→`parse` had no round-trip guard | Claude | night 1, column read |
| 08-22 | a stage with no tab failed nothing; a doc comment claimed otherwise | Claude | night 1 |
| 08-22 | `…shows_every_fixture…` checked 9 of 22 | Claude | night 1 |
| 08-23 | `architecture.md` described a re-run **deleted 2026-08-04** — the replay fiction | Claude | extending the citation checker to a file it did not cover |
| 08-23 | the symbol resolver could not see enum variants — 3 of 6 flags were false | Claude | same run; `egui_plot` was a 4th, external |
| 08-23 | `alias_anim` and `ic_plan_anim` nested a scroll area and capped it, same defect Doug found on 08-16 | toolchain | night 2; generalising `connection_anim`'s per-file check caught both on its first run |
| 08-23 | `symbol_is_defined` matched a **prefix** — a new `pantelides_ladder` module silently retired the `pantelides` claim of absence | toolchain | night 2; `claims_of_absence_are_still_true` fired the moment the module landed |
| 08-23 | `architecture.md` cited `mem::take`, which resolved only via that loose match | toolchain | night 2; exposed by tightening the resolver — it names std, so it now reads `std::mem::take` |
| 08-23 | `alias_anim` and `ic_plan_anim` opened on frame 1 with a step **already applied** — no opening frame, since birth | **Doug** | running the two views night 2 had just touched; the same pair again, and the column read had missed it by reading the trait and not the frame semantics |
| 08-23 | `drain_worker`'s `DefTree` arm cleared the loading indicator for a request **still in flight** — the one arm of six with no staleness guard | Claude | night 3, column read of the router's six arms |
| 08-23 | `requires_specimen`'s doc said *"the three that do not need one"* while its list held **six** | Claude | night 3, column read of `dispatch_hrw_link`'s twelve arms |
| 08-24 | a recompile drew the **previous** compile's matrix over the current compile's report — `CompileProgress` invalidated no view cache | Claude | night 3 found it; **Doug ruled it a fiction rather than a trade**, on the principles |
| 08-24 | `ic_plan_anim`'s empty-plan message states a **cause** HRW infers — *"every unknown comes from a start attribute"* — beside a header showing Rumoca's own verdict | Claude | night 4; true today by agreement, not derivation. `ui-findings.md` C19, awaiting Doug |
| 08-24 | 6 of 16 absence messages were named by no test, including 2 of 3 in one file where the third was tested | Claude | night 4; the survey that found them is now a standing checker |
| 08-24 | `ideas.md` recorded lever B's status **twice and differently** — the final ledger said *declined*, an earlier table still said *blocked on a ruling* | Claude | Doug asked which MSL ruling he owed; there was none, and the stale row is what said otherwise |
| 08-31 | *"token cost is not a constraint — never trade richness for economy"* against *"answer at the depth asked and stop"* and *"thoroughness had been treated as free and is not"*, the last **400 lines away in the same file** | **Doug** | he asked whether his own recorded declarations might contradict; his words were scoped to *context captures* and two rounds of summarising dropped the scope |
| 08-31 | `CLAUDE.md` said **night 6 was queued and unrun**; the run log recorded it closed | Claude | pre-push handoff check. A session starting that night would have queued a spent lens and never reached Doug's night-7 instruction |
| 08-31 | three documents named the **budget ratchet** retired hours earlier, one of them inside a no-go rule | Claude | night 7, pass 3 |
| 08-31 | the gate has **three** verdicts; `CLAUDE.md`, `unattended-runs.md` and `examples/gate.rs`'s own header all said two | Claude | night 7, pass 4. The runner's header being wrong is the sharpest: it is what a reader checks when prose and table disagree |
| 08-31 | `CLAUDE.md` charged **FULL** for a guarded-table edit in three places, hours after the LAB gate made it 11.1 s | Claude | night 7. The very friction Doug ruled a bug, still being *instructed* by the file read before committing |
| 08-31 | the lab iteration loop named the plain fast suite — **the one gate that cannot see a lab edit** — and said the catalogue regenerates `ONLY` on a `##` heading, contradicted twenty lines below | Claude | night 7. The `ONLY` cost three catalogue surprises in one day |

**Seven days: Doug 8, Claude 13, toolchain 3.** Against the first two days' **Doug 10, toolchain 5,
Claude 5**, the rate that matters moved: **Doug from ~5.0/day to ~1.1/day**, while Claude's own
finding rate held at ~1.9/day. **The ratio moved toward the toolchain and Claude**, which is what
the ledger was built to detect.

### 08-25 — and half of these are not in the code at all

| date | defect | caught by | note |
|---|---|---|---|
| 08-25 | the not-run wording had **five** authors, three of them rebuilt by hand — and `a_stage_that_says_it_never_ran_shows_no_ir` finds a never-ran stage by matching that text | Claude | proposing a `worker.rs` improvement; a guard whose premise is hand-maintained in five places has a slow leak |
| 08-25 | a helper's doc comment adopted `not_reached_stage`'s summary — the doc-insertion bug, **4th time**, an hour after writing the rule against it | toolchain | `no_doc_block_gains_a_second_summary` — **but only because `worker.rs` sat at 20/20.** One under budget and it is silent; the ratchet counts, it does not detect |
| 08-25 | a 27,668 s gate step was confidently diagnosed as build contention. **The machine had slept.** | **Doug** | *"I think that my machine went to sleep"* — `CLAUDE.md`'s hung-vs-slow test is two-way and complete-looking, the same shape as `fmt` missing from a two-gate rule |
| 08-25 | the notebook step's cost was inferred at ~256 s by subtracting two gate runs that had **different source changes**; measured, it is 101 s and the documented 109 s was right all along | Claude | flagged as unpinned before it was written down; Doug then asked for a real number |

| 08-25 | the only pane-level test of an **empty** IC plan fed `{"blocks": []}` with no `determinacy` — a report **neither real specimen produces**. `BouncingBall` and `SingleInertia` both carry a verdict, so the one test covering that pane exercised an arm that never occurs in the corpus | toolchain | fell out of C19's fix: gating the gloss on the verdict made the arms distinguishable, and the fixture landed on the wrong one |

| 08-25 | **`flatten_stage` still adopted DAE construction's error** — an arrangement from before DAE had a tab of its own (given one 2026-08-03), so BOTH stages rendered the same payload and the committed notebook held `flatten.json` byte-identical to `dae.json`. Flatten never failed: reaching ToDae requires it to have succeeded | Claude | fell out of C20's one-stop rule; `OverDeterminedShaft`/`UnbalancedShaft` read `OOOOXX.....`, and two failures in one row cannot both be where the pipeline stopped |
| 08-25 | **a failed parse set SEVEN downstream stages to `Stage::err`, plus an eighth for resolve** — eight claims that the pipeline stopped, for one stop at parse. None was matched by `a_stage_that_says_it_never_ran_shows_no_ir`, which keys on "not reached" | Claude | found only because `UnclosedModel` did **not** move when the first two C20 sites were fixed and I had predicted it would — a wrong prediction being more informative than a right one |
| 08-25 | **one pipeline condition renders as a FAILURE in six stage tabs and a NON-failure in three** — a pipeline that returns no result at all is `Stage::info` (`Ok`, neutral) in Instantiate/Typecheck/DAE and `Stage::err` (`Failed`, red) in the other six | Claude | `the_corpus_outcome_matrix_is_unchanged` on its FIRST run; a `.` interleaved with `!` in three rows is what made it visible. `ui-findings.md` C20, awaiting Doug |

| 08-25 | **the DAE tab was wholly blank** for a compile with no pipeline result (`MissingComponentClass`, `UndefinedRef`) while its six siblings all carried the no-result note. Deliberate, on the reasoning that *"Flatten already reports it"* — which is the adoption argument `dae_absent_stage`'s own doc comment was written to reject, in the same file about the same two stages | Claude | found while checking whether the not-reached tail was contiguous. **Absence is stated, never filled, and a NEIGHBOURING tab stating it does not satisfy that** |
| 08-25 | **Instantiate and Typecheck rendered blank tabs** when parse produced no model — `let mut instantiate = Stage::default()` was never written to on that path, so two stages said nothing while every other stage in the compile explained itself | toolchain | `the_not_reached_tail_is_contiguous_and_agrees_on_its_cause` on its first run, naming `UnclosedModel` |
| 08-25 | **the notebook check does not compare manifest NOTES** — only stage IR files and the roster. `UnclosedModel`'s committed resolve note stayed pre-C20 through a green 109.9 s notebook run, and surfaced only when an unrelated fix forced a regeneration | Claude | reading a regeneration diff. A stage's note is a claim about the model; nothing checks that the committed one still matches |

| 08-25 | **HRW plotted a non-finite trajectory in silence.** A simulation can return `Ok` with an infinity in it, and `hrw/src/` contained no finiteness check of any kind — so the pane drew it, `egui_plot`'s bounds did whatever they do, and nothing said why the picture looked wrong | Claude | three probes: the first two died on the solver's projection guard at init, and the third — a singularity independent of any state — got through. **The guard is not "the solver rejects non-finite values"**; it does, in the paths it watches, and an algebraic output is not one |
| 08-25 | **HRW called the plain simulate entry point while the CLI called the NaN-tracing one**, so on a diverging model the CLI named the offending variable and the learning instrument reported "step size too small" | Claude | reading `rumoca-sim`'s facade while measuring the above. The wrapper's own doc says it is *"intended for scheduled single-model use"* — which is what pressing Run is |

| 08-25 | **the panic path's log entry claims `elapsed_secs: 0.0`** — a fabricated timestamp. It is built by hand rather than through `make_log`, because `Worker::spawn` holds no compile clock, so it reports the panic as having happened at t=0 of a compile that had been running | Claude | night 5, item 1, while sweeping the log machinery. **Recorded, not fixed**: a log line's time is a claim, and changing one unattended is what hard rule 6 forbids improvising |

| 09-01 | `experiment` proposed as a lab-kind name — it is a **Modelica annotation** in 14 specimens | **Doug** | Claude had recorded the collision-check discipline one commit earlier and not run it |
| 09-01 | `compiler-phases/` described as a deferral store; Claude proposed deleting 8,650 lines on that premise | **Doug** | only he knew the files predated HRW |
| 09-01 | the tour → lab rename **missed the verb** entirely — ~580 `walk` occurrences | **Doug** | he read a heading |
| 09-01 | five broken `CLAUDE.md` links after the rename; the FULL gate passed | grep | nothing resolved this file's markdown links — checker built the same day |
| 09-01 | prose *about* the rename rewritten by it — Decision 14's title, two of Doug's quotations | grep | third instance of this class |
| 09-01 | `end_to_end_tour.md`, a deleted file's real name, falsified in 18 places | Claude, same hour | read-back after a generator script |
| 09-01 | two duplicates created **while conducting the duplicate sweep** | Claude, same day | both from restating a rule being cut rather than pointing at it |
| 09-01 | doc comment stranded by an insertion above it — **three times in one day** | `no_item_loses_its_doc_comment` | the checker caught every one; knowing the rule did not |
| 09-01 | `sed` used to edit a file, hours after defending the rule against it | Claude, same hour | no mechanism; self-reported |
| 09-01 | a link clicked in an early lab segment dispatched nothing — `prepare_show` wipes every hook | `a_link_far_down_a_long_lab_still_dispatches` | **a wiped hook is indistinguishable from a link nobody pressed**; the test predated the change |
| 09-01 | `a_code_span_mention_is_not_extracted_as_a_link` was **misnamed against its own assertion** | Claude | found while changing the extractor it tested |
| 09-01 | the per-stage-system rule listed **three** systems and there are more | Claude | the one rule whose own text says its failure is silent |
| 09-02 | an Answer quoted six counts with **no route to any of them** | **Doug** | *"where is this reported?"* — the labs' own rule is that an expectation must say where to look |
| 09-02 | the same Answer said *"worth asking System Modeler"* when `hrw://systemmodeler/` exists | **Doug** | describing an action HRW can perform |

**Fourteen rows, and the pattern differs from every section above: two are defects in Claude's
REASONING, not in a file.** A wrong cause asserted for a timing anomaly, and a cost derived by
arithmetic instead of measured — **no checker could have caught either, because neither lived
anywhere a checker looks.** Doug caught the first; the second was caught only because he asked for
a real number, which is the same intervention twice.

**That is the ledger reporting on its own blind spot.** Every mechanism in this repository verifies
a claim written in a file. **A claim made only in conversation is unguarded**, and both of these
were one step from being written down as fact — the 256 s figure would have replaced a correct
number in `CLAUDE.md`, and the contention diagnosis would have left the real cause unfixed for a
third occurrence. The countermeasure that worked both times was **saying the number was not pinned
down before using it**, which costs one sentence.

**One row also argued for a real checker, and it was built the same day.** The doc-insertion bug
has now happened four times, and the thing that caught it is a *budget*, not a detector: it noticed
a count crossing a threshold, not a doc comment losing its item.
`doc_citations::no_item_loses_its_doc_comment` now watches the other symptom — **an item that was
documented at `HEAD` and no longer is.**

**The design turned on measuring the obvious alternative first.** Asserting that every item is
documented would need a ratchet over **264 undocumented column-0 items across 56 files**, which has
precisely the blind spot being fixed. But losing a doc comment is a **transition**, and git holds
the before-state — so the check needs no heuristic, no allowance and no triage, and its budget is
zero by construction.

**The two checks now split stock from flow**, which is worth stating because neither covers both:
the budgeted `no_doc_block_gains_a_second_summary` watches the standing population loosely at ~29 %
precision, and the new one watches everything that changed since `HEAD` exactly. **A defect
committed and never touched again is invisible to the second** — that is what the first is for.

### 09-03 → 09-12 — the Doug column is questions now, not screenshots

| date | defect | caught by | note |
|---|---|---|---|
| 09-03 | an Answer pointed at **three absent things** in Solve lowering | **Doug** | *"I don't see this"*, three times, then *"I still don't see 'Y'"* |
| 09-03 | `c[1]` cited as a tree node and not present | **Doug** | *"Do I need to expand some nodes?"* |
| 09-03 | bold used for emphasis in an Answer, against a standing agreement | **Doug** | he had corrected the same thing before |
| 09-03 | autoplay clicked only the **first** link on a line | **Doug** | *"You missed many of multiple links per line"* — after a fix that addressed one link |
| 09-03 | the beat-increment flash blamed on id collisions; his report said *"a different scroll position"* | **Doug** | a mechanism chased past the report's own constraining noun; cost a commit that had to say it had not fixed the reported thing |
| 09-03 | *"the simulation plot does not honour the follow"* — it did | Claude | a filtered grep needing both terms on one line, against `let tracked = self.tracked_identifier;` |
| 09-03 | doc comment stranded by an insertion above it | `no_item_loses_its_doc_comment` | bitten eight times total, five of them on 09-04 |
| 09-04 | **simulation was the one pane whose data the bridge never published** | **Doug** | *"C.v is flat at 5, I think"* — a hedged reading of the only artifact under suspicion, after it had already cost one wrong diagnosis |
| 09-04 | *"declared starts never reach the state vector"* — they reach `initial_y` intact | Claude | reading `initial_y`'s own doc, which calls itself the check for exactly that |
| 09-04 | the mechanism in a **public** upstream entry: *"the steady state could not be found"* implies a failed attempt; none is made | **Doug** | *"why does BouncingBall simulate correctly?"* — the equation that would fail is filtered out before the solve |
| 09-04 | `view_published` reports a publish that did not happen | Claude | `last_published_view` is a dedup key and a claim under one name |
| 09-04 | `experiment(StopTime)` silently ignored in favour of the slider | Claude | defensible and undocumented; `RcCircuit` asks for 1 s and gets 2 |
| 09-04 | a **stale copy of `CLAUDE.md` in a parent directory**, injected as authority, untracked, 4 days old | Claude | a grep for a path that should have been there and was not |
| 09-04 | `updating-rumoca.md` step 4 named **one** harvested crate; there are five, and the table is keyed by stage | Claude | verifying the procedure instead of reading it |
| 09-04 | the same file said *"nothing else depends on"* the committed traces; **eight source files read them** | Claude | including `answer_check`'s fallback and a gate |
| 09-04 | `git fetch upstream` — **there is no `upstream` remote**; the rebase plan was blocked at its first command | Claude | prompted by Doug asking which Rumoca version HRW uses |
| 09-12 | the lab answered the **flow** half of unconnected connectors and left the potential half hanging | **Doug** | *"which compiler phase fails?"* — a reader following the passage carefully reaches the question |

**Seventeen rows: Doug 8, Claude 8, toolchain 1.** The ratio has moved, and not because Doug is
finding less — because **what he finds has changed kind**. Nine of his ten rows in the 08-16 section
were things visibly wrong on screen. Here, five of eight are **questions**: *why does BouncingBall
work*, *which phase fails*, *which version are we on*. None of those is a bug report. Each one
turned out to sit on top of something wrong.

**That is a better instrument than the screenshots were, and it is worth saying why.** A question
tests whether Claude's account *holds together*, and three of these found accounts that did not —
including a wrong mechanism already written into a public entry with Doug's name on it. **No checker
in this repository verifies a mechanism**, only that claims resolve; `git fetch upstream` is
well-formed, names a real project, and had never been run.

**The Claude column is dominated by one activity: verifying a document instead of reading it.** Five
rows come from checking `updating-rumoca.md`'s claims against the code — crate counts, dependency
claims, a remote that does not exist. **The procedure had been carrying an untested first command
since HRW moved in-workspace**, and nothing could notice, because a procedure is prose and prose is
only checked for whether its citations resolve.

**The toolchain column is one row — and reading that as a finding is a mistake, corrected here the
same day Doug asked what the argument was.** *"Only one of seventeen"* sounds like evidence the
checkers are weak. It is not, because **this column is undercounted by construction**: a clippy
error, a failing test or a rustfmt diff is fixed in the same minute and never becomes a row. What
the ratio actually says is that the defects *worth recording* skew toward the ones checkers cannot
see, which is nearly a tautology. **Read the ratio for Doug-versus-Claude, which is what it was
built for, and not for the toolchain's effectiveness, which it cannot measure.**

**What does survive is one nameable class, and it is rows 14, 15 and 16: a document claim that
names real code, resolves, and is false.** Every mechanism here verifies **resolution** — the path
exists, the symbol is found, the link parses. None verifies **correspondence** — whether the prose's
claim about that code is true. *"`gen_field_help` re-extracts from `rumoca-ir-ast`"* cites a real
crate and resolves perfectly; there are five. *"`git fetch upstream`"* is well-formed and names a
real project; there is no such remote. **`qualified_citations_resolve` was green throughout, and
correctly so.**

**But the remedy is a REHEARSAL, not a checker**, and that distinction is the useful part. All three
are in one document, and all three were found by one activity: *verifying* `updating-rumoca.md`
rather than reading it. A checker pinning crate counts against `IR_CRATES` would have caught 14 and
neither 15 nor 16. **What would have caught all three is running the procedure once** — it had never
been executed since HRW moved in-workspace, so the defects were not subtle, they were
**unexercised**. <!-- unbuilt: gate_policy::procedure_docs_are_rehearsed -->

**The transferable rule: a procedure nobody has run is not documentation, it is a hypothesis.**
This repository already applies that to code — the must-fire rule exists because an unexercised
reporter is indistinguishable from a working one. `updating-rumoca.md` is the same shape, and it sat
unexercised for weeks while every checker over it stayed green.

**SECOND INSTANCE, 2026-09-19 — nine days later, and it is the same author making the same
mistake.** `upstream-issues.md` gained the line *"promote `ConnRc` when the rebase lands"*, written
the same morning. Loading it once — which did not happen until the afternoon — showed it could not
be carried out: **HRW compiles the FIRST class in a specimen file**, `ConnRc.mo` led with
`connector Pin`, so HRW had been compiling a connector while eight stages reported *"the
reachable-closure pipeline produced no result"*. The plan resolved, read fine, and had never been
executed.

**Two instances in nine days is what makes this worth a mechanism rather than a resolution.** The
first was a procedure document; the second was a single sentence in a different document. **What
they share is not their shape but their author's habit** — writing a next step without taking it
once. So a checker aimed at procedure docs would have caught the first and missed the second, which
is the argument for the *practice* over the tag above, and against over-fitting a mechanism to the
first example.

**And the second instance carries a sharper warning than the first.** `Import[…, "MO"]` takes the
**LAST** class, so System Modeler simulated `Gnd` off the same file and returned an ordinary
`SystemModelSimulationData` — **a wrong-model adjudication is indistinguishable from a right one**,
and the oracle is this project's court of last resort. The guard is now one line in
[`ideas.md`](ideas.md) #43: check `sim["ModelName"]` whenever a file holds more than one class.

**Not counted above, deliberately: the initialization defect itself** — Rumoca's, not ours. It is in
[`upstream-issues.md`](upstream-issues.md), adjudicated against System Modeler, and it was found by
Doug asking for a specimen that plots well. Counting another project's defect here would flatter the
ratio this ledger exists to keep honest.

### 09-13 → 09-20 — the defects moved out of the code and into the record

| date | defect | caught by | note |
|---|---|---|---|
| 09-13 | the entry's causal mechanism — "the state sits among the unknowns with nothing pinning it" — refuted by a model that has every blamed condition and works | Claude | `BareRc`, measured; written up six days later because the session ended mid-build |
| 09-19 | the same entry's headline generalised past its evidence: *"a state's initial value comes from der = 0"* describes several models, not Rumoca | Claude | the title was renamed to name a symptom |
| 09-19 | *"promote `ConnRc` when the rebase lands"* — written that morning, **not executable**: HRW compiles the FIRST class and the file led with `connector Pin` | Claude | found by loading it once; the unexercised-claim failure's second instance |
| 09-19 | System Modeler simulated `Gnd` off the same file — `Import[…, "MO"]` takes the LAST class — and returned a normal result object | Claude | a wrong-model adjudication looks identical to a right one; guard added to the recipe |
| 09-19 | a class named `Res` drew four resolve flags for shadowing | Claude | cosmetic; renamed |
| 09-19 | *"UQ is the third axis"* — imprecise: UQ is absent from class 2 and coextensive with class 3, so it crosses nothing | **Doug** | *"what is the third axis?"* — the reference is the axis; UQ is its cost |
| 09-19 | the gap list omitted domain physics with **no reason written**, so a reader could conclude it was judged peripheral | **Doug** | his inference from the writing's emphasis exposed the absence |
| 09-19 | *"frame compiler concepts as introductions, not reminders"* — half wrong for eight weeks; he built the back half of a Modelica compiler in 2011 | **Doug** | by telling Claude what he had built; nothing in the repository could have known |
| 09-19 | a former employer's internal situation, written and **pushed** to a public repository | **Doug** | the attribution test did not exist; it does now |
| 09-19 | a verbatim quote of his own words that reads as self-doubt once the conversation is gone, in a public file | **Doug** | accurate, exact, and the mechanism of the problem — a paraphrase carried every finding |
| 09-19 | a charter clause recording him changing his mind about his own motives | **Doug** | ratified text edited only on his instruction |
| 09-19 | the reading path pointed at the taxonomy and not at the programme that says what to do next | Claude | caught by the pre-push handoff question |
| 09-19 | "Stage A is available now" — available is not next; his sequence puts the whole programme behind step 3 | Claude | corrected the same day it was written |
| 09-20 | Doug's Rust plumbing questions graded as drift when they were the prerequisite for the criterion Claude had just given him | **Doug** | *"I am ignorant about Rust"* — the working agreement already said to weave idiom in on contact |

**Fourteen rows: Doug 7, Claude 7, toolchain 0.** And the zero is correct rather than alarming
this time, because **not one of the fourteen is a code defect.** Eleven are claims in documents;
three are working-agreement errors about Doug himself. There was no code to check.

**What the block shows, and it is different from the last one.** The 09-03 block found that Doug's
catches had become *questions*. This block finds that **his catches are now things only he could
know**: what he built in 2011, whether a passage reads as self-doubt, what his employer would not
want public, why he asks plumbing questions. Nothing in the repository, and no possible checker, has
access to those facts. **The asymmetry is not that Doug sees defects Claude misses. It is that Doug
holds information Claude cannot obtain except by being told**, and the defects were Claude writing
as though he had it.

**Three of Doug's seven are the same shape, and it has a rule now.** Accurate, well-sourced,
public, and wrong to publish — the three tests in the private memory (stranger, attribution,
quote-versus-paraphrase). The instructive part is *why* the existing rule failed: Claude's attention
was on **accuracy**, which this project trains hardest, and accuracy is orthogonal to discretion. A
true, exactly-sourced sentence passed every check that existed and should not have been written.

**Not counted, deliberately: the 0.10.0 confirmation and the three flat-line specimens.** Those are
findings about Rumoca, and good ones — the branch fixes the defect; connectors are the trigger — but
they are not *defects in this project*, which is what this ledger counts.

### 09-21 — one environment defect, and two wrong diagnoses of it

| date | defect | caught by | note |
|---|---|---|---|
| 09-21 | rust-analyzer had no cross-crate dependency graph — Go to Definition did nothing, silently — since the nightly pin | **Doug** | *"nothing happens"*; no checker in the project rules on the analyzer, and the analyzer's own status icon was the only tell |
| 09-21 | first diagnosis: an analyzer/toolchain version mismatch. Prescribed a toolchain-matched analyzer; failed identically | Claude | the log's `--lockfile-path` line, pasted by Doug after restart 2, was the fact; the prescription came before it |
| 09-21 | second diagnosis: the sysroot query. Prescribed `cargo.sysroot: null`; failed identically, because the flag is sent for the workspace query too | Claude | `cargo metadata --help \| grep lockfile` on both toolchains, and `analysis-stats`, settled it in two minutes — and should have been the FIRST move, not the third |

**Three rows: Doug 1, Claude 2, toolchain 0.** The environment defect is real and is now both fixed
(`.vscode/settings.json`, with the account) and ruled on (`check_machine` warns when stable's
`rust-src` is missing — the one piece of the fix that does not travel with a pull).

**The two Claude rows are one shape, and it has a name already:** the unexercised claim, third
instance. Two prescriptions were written from a plausible reading of a symptom, each cost Doug a
restart and an index, and each was refuted by the next restart. The third prescription was proved
from the command line *before* the restart, and worked. **The rule the first two instances produced
— run a procedure once before trusting it — applies to diagnoses as well as procedures:** measure
the thing named in the diagnosis before prescribing on it.

### 09-22 — a rule applied to its own author, and half-applied

| date | defect | caught by | note |
|---|---|---|---|
| 09-22 | the overview rule's third row was applied as "file the station" and **not** as "move the claim out", so `connect-expansion`'s overview came out 72 → 67 lines | **Doug** | *"The connect-expansion lab overview seems as long as it had been before."* Claude had written the row that morning and reported the small delta as an honest result rather than as a failure to follow it |
| 09-22 | moving `generate_unconnected_flow_equations` to Station 5 left the overview's next paragraph opening *"There is no **such** pass…"* pointing at nothing | Claude | found while re-reading the overview after Doug's catch; removed with the paragraph |
| 09-22 | a new test's doc comment was spliced into the middle of the neighbouring test's doc comment | toolchain | `no_doc_block_gains_a_second_summary`, exactly what it exists for. Fixed by moving the block, not by raising the budget |

| 09-22 | `connect-expansion`'s overview explained union-find, pair routing and `ConnectionSet`'s fields, and **never said what a connector is** — in a lab named for what `connect` means | **Doug** | found in one reading, after two rounds of Claude's own triage had rewritten that overview twice without noticing |
| 09-22 | the same overview gave away Station 6's prediction — it explained why potential merges globally and flow per scope, which is exactly what Station 6 asks the reader to commit to | Claude | found while rewriting from empty; it had survived both triage passes and violates the standing *provoke, do not pre-empt* rule |

**Five rows: Doug 2, Claude 2, toolchain 1**, and Doug's second is the one that changes a rule.

**Why triage could not have found the missing connector.** Triage starts from existing prose and
asks of each sentence whether it may stay. It can only remove, and it rationalises whatever
survives — **it never asks what is absent.** Two passes cut that overview 72 → 50 and left the
central concept of the language undefined. Doug's sequence inverts it: write the stations, start
the overview **empty**, add only what a station cannot be attempted without. The first question an
empty overview asks is *what must the reader know*, which is the question that surfaces an absence.
Rewriting from empty produced the connector introduction and the Station 6 spoiler in one pass.

**And the spoiler is the second thing an absence-blind method misses.** A sentence can be true,
grounded, stationed, and still wrong to keep — because the station it belongs to has nothing left
to ask once the reader has read it.

**What Doug's catch actually exposes.** The rule had a `keep`, a `move` and a `file`, and *only the
first two named a destination.* Faced with prose that was correct, recent and deliberate, the
disposition with no destination collapsed into "leave it where it is and write a backlog entry" —
which changes nothing a reader experiences. **A rule that names an action for two cases and only a
judgement for the third will be obeyed in the two cases.** The row now names the destination
(`What this lab cannot check`), and re-applying it took the overview 67 → 50.

**And the caution that produced it was real, which is why this is not simply carelessness.** Two of
the three claims answered questions Doug asked this month, one of them two days earlier, and the
question ledger records that passage as *his* to rewrite. Every one of those is a reason not to
*delete*. None of them is a reason not to *move*. Conflating the two is the actual error.

### 09-22, second pass — what applying the rule to all 21 labs turned up

| date | defect | caught by | note |
|---|---|---|---|
| 09-22 | **every** lab opened with its GENRE, not its subject — *"A concept lab. It teaches a step of the chain"* — and that line is also the catalogue blurb, so the catalogue described lab kinds | Claude | 11 rewritten to open with the lab's own claim; the blurb pins followed the prose, per the standing rule that a pin guards a sentence and not its emphasis |
| 09-22 | four spoilers, each reading as a useful abstraction: `tearing` (3×3 → 1×1, Station 1), `index-reduction` (Station 5's unmatched pair, verbatim), `connect-expansion` (Station 6's asymmetry), `the-concepts` (a whole section of cross-lab numbers pre-announcing **four stations across four labs**) | Claude | found only by reading each overview against its own stations; no checker looks at this |
| 09-22 | `initialization`'s table of contents said *"Three stations"* and it has four | Claude | a hand-maintained count with nothing able to notice; all nine TOCs were cut rather than corrected |
| 09-22 | `failure-structural` opened a bold span in its overview and never closed it, and Station 4 carried a stray closing `**` with no opener | Claude | **survived every gate**: nothing looked at rendering and nothing looked at markup. A span crossing a blank line renders as literal asterisks and inverts the emphasis after it |
| 09-22 | three feature labs carried their own history in the overview — 0-based frame numbers, a parenthetical about an off-by-one, how camera-aiming was selected before in-app selection existed | Claude | `writing-a-lab.md` already says *keep the lab's history out of the lab*; the rule existed and nothing checked it |

**Five rows, all Claude, and the honest reading is that Doug caused all five.** They were found
because he supplied a method — write the stations, start the overview empty — and every one had
survived however many passes came before under *"is this sentence true and does it belong?"*

**Only the last two had a pre-existing rule.** The history rule was written and unenforced; the
markup defect had no rule at all and now has
`every_bold_span_in_a_lab_closes_in_its_own_paragraph`, proven in both directions. **The other
three are the ones worth noticing**, because the genre-blurb defect was in *every single lab* and
had therefore stopped looking like a defect at all — it looked like the house style.

**The pass introduced a false claim, and Doug caught it the same day.** `connect-expansion`'s
rewritten overview said of connection expansion *"that is the whole of the phase"* — **flatten
also crushes the hierarchy into one namespace of globally unique names, copies in each
component's own equations, and rewrites everything into residual form**, per the crate's own
module doc. The lab's own Station 4 already measured the contradiction: 16 of `RcCircuit`'s 23
equations are the components', only 7 are the connections'.

**The mechanism is specific to rewriting from empty, so it is worth naming.** Cutting an overview
to what the stations need produces a document about *the lab's slice of a phase*, and the shortest
way to end that paragraph is a sentence saying the slice is the thing. Triage never produced this
error, because triage never wrote a new summary sentence. **A method that removes prose cannot
introduce a falsehood; a method that rewrites it can** — so the from-empty rule buys accuracy at
the station level and costs a new failure mode at the summary level.

A sweep for the same shape found one more, milder: `dae-construction` listed the phase's jobs as
though the two the lab follows were all of them, where it also lowers algorithm sections, `when`
conditions and `pre()`. Both now name what the lab does not cover. `matching` and `tearing` say
*"X is the phase that does Y"*, which is focus rather than a claim of exhaustiveness, and were
left alone.

**And the rule's own end point, reached on `index-reduction` the same day.** Its overview stayed
at 112 lines after the pass, because the ~90 lines left were a *derivation* — the pendulum's five
unknowns, the constraint that mentions none of them, the chain rule applied twice — and Doug's
standard for that lab is that index reduction be followable with basic calculus alone. **Two of
his rules pointed opposite ways**: stations-first says a derivation is not a prerequisite for any
station's prediction, and the harder bar says the derivation is the point. Put to him rather than
settled, and he chose the resolution that satisfies both: **make the derivation a station.**

It already had a station's shape — predict that five equations in five unknowns is enough, open
the Incidence view, find the constraint's row empty. It is now Station 1, the old five renumber to
2-6, and Station 6 is its payoff: *you differentiated it twice by hand; the compiler performs
zero.* The overview is 26 lines. **The general lesson is that an overview which cannot shrink is
usually one holding an activity, not a briefing** — and the test is whether it could carry an
`**Expected:**`.

### 09-22, third pass — a false claim in the emitted context, and the bar was innocent

| date | defect | caught by | note |
|---|---|---|---|
| 09-22 | the 🎯 capture labelled a **pane selection** as lab prose and emitted `file: hrw/docs/fixture-labs/connect-expansion.md`, a file the text is not in | **Doug** | he captured `flow sum equation: src.p.i + R.p.i = 0` and asked whether Claude could see it; the string is generated by `connection_anim` and written in no lab |
| 09-22 | Claude's first report blamed the capture broadly | **Doug** | *"The context bar's Pointing at item correctly identifies what I captured."* The copy path and the bar were both right, which **localised the defect to the attribution alone** and made the fix a checked flag instead of a new capture shape |

**The mechanism**, and it is the kind that looks correct in every reading of the code:
`arm_lab_passage_capture` pushes an `egui::Event::Copy`, which takes **whatever is selected
anywhere in the window**, and then labels it with whichever lab is open. The comment above the
call said the lab's label is taken *"so the capture names the document the passage can be found
in"* — a sentence that states the invariant the code does not enforce.

**Fixed by checking rather than asserting.** `PointKind::LabPassage` and `Focus::LabPassage` carry
`in_source`, computed at capture against the lab's own text; when false the emitted section drops
`file` entirely, renames `lab` to `lab_open`, and says the text came from a pane and must not be
looked for in the markdown. The bar's noun changes with it.

**The normalisation is the load-bearing half, not the containment check.** egui returns what the
pane *displays*, so a genuine passage arrives with its markdown gone — a raw `contains` against
the source would call nearly every real capture foreign and fire the warning constantly, which
would have been worse than the defect. Both sides are stripped of emphasis and code marks, link
targets dropped, whitespace collapsed. `a_pane_selection_is_not_mistaken_for_lab_prose` asserts
both arms, the positive one on a sentence whose source is thick with markup.

**It errs toward "in source"**: with no lab text loaded there is nothing to contradict, and
claiming prose is foreign would be its own false statement.

**Two more fields were still lying, found by Doug using the fixed button.** He captured
`flow · 1 set(s)` — a lane header from the Connections replay — and the document
still said `kind: "lab_passage"` and `stage: "(lab prose, not a compile phase)"` **beside a
section stating the text is in no lab.** Contradictory claims in one document, and the second
field's own comment records it being introduced to fix exactly that failure once before. Both now
key on `in_source`: the kind becomes `selection`, and the sentinel says the text came from a pane
and points at `view`. `a_pane_selection_emits_selection_and_says_it_is_not_prose` pins both.

**The reframe is the finding, and it is Doug's:** *"The capture button is MUCH more useful now as
it can be used for anything which I can drag-select in the HRW window."* **The fix added no
capability** — the button always copied the whole window's selection. What it added was an
honest label, and a capability nobody could rely on became one they could. **A false claim does
not only mislead; it suppresses the feature it misdescribes.**

**What that reframe leaves unfinished, stated rather than queued silently.** The button lives only
in the lab panel's transport bar, so general capture is still gated behind having a lab open, and
its refusal message reads *"no lab is open, so there is no passage to point at"* — the old
framing. And `view.stage_view` reports what was on screen rather than where the selection came
from; for a pane capture those are almost certainly the same, and *almost certainly* is what this
whole entry is about.

**The spoiler is the defect this corpus is now most exposed to**, and it has no checker and
probably cannot have one: it requires knowing what a station asks. The check is per station — for
each `**Predict.**`, is its answer stated anywhere the reader passes first? A hub is the worst
case, since everything it spoils is a lab it is sending the reader to.

### 09-22, the pointing arc — eight runs, seven links, three tests that lied

| date | defect | caught by | note |
|---|---|---|---|
| 09-22 | a right-click **collapses** the text selection it was meant to act on (`any_pressed()` where egui means primary) | **Doug**, by running it | filed as **E2** in `upstream-issues.md`; the mechanism changed to capturing at selection time |
| 09-22 | the copy was pushed after the labels drew, so nothing ever saw it | **Doug**, by running it | `got_copy_event` is read *while each label paints*; the 🎯 button only worked by accident of being drawn above the prose |
| 09-22 | the drag/click predicate read `press_origin()` on the release frame, where egui has already cleared it | **Doug**, by running it | never fired once; `button_clicked` is the surviving form of the same answer |
| 09-22 | clicking the menu item is itself a primary press, so it invalidated the text it was about to use | **Doug**, by running it | the item re-rendered disabled and a disabled button reports no click |
| 09-22 | the guard for *that* read `Context::any_popup_open()` (**pass state**, empty at the top of a frame) instead of `Popup::is_any_open()` (**memory**) | **Doug**, by running it | the trail said `enabled=false … inside_item=true`, which named it outright |

**Five rows, all Doug, and none of them findable by reading.** Every fix was correct and exposed
the next link. What made the arc tractable was not analysis — it was that each repair shipped with
an *instrument*, so the next run localised the next defect instead of restarting the argument.

**The part worth carrying: three tests passed while the code they vouched for was broken.**

| test | why it lied |
|---|---|
| step 0's gesture test | the harness never reported the label as hovered, so it could not reproduce the collapse it was written to catch |
| a scratch popup probe | clicked the item rect captured on the popup's **first** frame — a popup has no size then and moves on the next |
| the popup-open test | called the predicate **after** `Popup::show`; the code calls it **before** |

**One shape: each verified the right ingredient at the wrong moment.** A predicate asserted at a
different moment than it is used is not asserted at all. **For UI behaviour a synthetic harness
checks logic, not interaction** — so the order is instrument the running program, read the trail,
then write the test that pins what the trail proved. The tests kept from this arc are the ones
that pin *egui facts* (`press_origin` is gone at release; a popup stays open across the press),
because those are what a future egui bump will break.

### The standing prediction, checked — 3 of 4, and the fourth is the interesting one

It said the mechanised classes *"should stop appearing in the Doug column"*, and that a **new**
class appearing instead would mean point-fixes are not converging.

| mechanised class | still in Doug's column? |
|---|---|
| equation identity | **no** — none since |
| notebook currency | **no** |
| gate coverage | **no** |
| link resolution and dispatch | **yes, twice** — 08-17 and 08-19 |

**The fourth did not recur in its mechanised form and did recur in adjacent ones.**
`fixture_lab_links_all_resolve` checks that a link *resolves*; the 08-19 defect was eighteen links
that resolved and **opened a browser**, and 08-17's was a link that resolved, dispatched, and
scrolled to the wrong place. **The checker was true the whole time and the feature was broken** —
which is this project's recurring shape, a claim narrower than the guarantee a reader takes from
it.

**So the honest reading: mechanisation works on the exact class it names and does not generalise to
the surface around it.** The remedy is not more mechanisation of link *resolution*; it is that the
lab-link surface has three failure modes and only one is checked.

### And the surface Doug's column keeps landing on is UI LAYOUT

Of his eight, three are layout or on-screen behaviour, and the whole 08-12 → 08-19 divider and
transport-bar arc — eight commits, *"second theory dead"*, *"passes by coincidence"*, *"0 for 2"* —
is the single largest cluster of rework in the project's history.

**Doug, 2026-08-23:** *"that UI layout stuff was so problematic that I had made a mental note to
minimize or even entirely avoid making any future UI layout changes."* That is a design
preference rather than a rule, and it is recorded here rather than in `CLAUDE.md` because the
mandatory reading path is budgeted and this is not a thing a session must know before acting.
**The measurement supports him:** layout is the one surface where `egui_kittest` cannot help,
so it is the one place a change is verified only by Doug looking at it.

**The prediction worth checking next week:** the classes now mechanised — equation identity, link
resolution and dispatch, notebook currency, gate coverage — should stop appearing in the Doug
column. If a *new* class appears there instead, that is the pattern continuing and the honest
reading is that point-fixes are not converging.

#### The counter-argument, which is real

**Scripts are editable without a rebuild.** On 2026-08-01 that mattered: the fidelity binary
was locked by a running sweep, and fixing the watchdog in PowerShell required no rebuild and
did not disturb the run. Converting everything to Rust would have cost that.

So the honest form is *"move it where the toolchain can check it, unless being editable
mid-run is why it exists"* — and say which, rather than converting on principle.

#### Standing candidate

~~`measure-fidelity.ps1` and `promote-run.ps1` meet all three conditions.~~ **Resolved
2026-08-01 by splitting them** (`verification-plan.md` item 3). `promote-run` became
`examples/promote_run.rs` with its guards tested in `src/promote.rs` — it writes a **published
claim** and needed no crate. **`measure-fidelity.ps1` stays in PowerShell**, with the reason
recorded: it needs process memory sampling, and being editable while a sweep holds the binary
is why it exists in that form.

**And one condition weakened, which is worth noticing about the trigger itself.** "Re-run
repeatedly" was true while sweeps were daily; the corpus is green now and the suite runs a few
times a year. A trigger whose conditions can lapse is working as intended — but it means the
standing candidates list needs re-reading, not just appending to.

### 11-04 — the column Doug has owned alone now has a second occupant

| date | defect | caught by | note |
|---|---|---|---|
| 11-04 | **the screenshot channel never fired at all**: `frame_ui` runs the poll, and egui does not paint an idle window | Claude, **by running the feature** | the first live request sat on disk unanswered; `session.json`'s last frame was stamped fourteen seconds *before* it arrived. Fixed with a waker thread that `stat`s the request file and wakes the UI only on a change |
| 11-04 | the poll's 250 ms rate limiter **raced the waker it could not see** | Claude, reasoning about the fix just made | a wake arriving just after a poll would find the next one "not due" and let the app sleep again, with nothing to wake it until the file changed a *second* time. The limiter was removed rather than coordinated: one `stat` per painted frame is cheaper than the session file the same frame already writes |

**Two rows, both from one afternoon's feature, and the first one is the point.** Every unit test
passed before the feature worked at all — because they test the protocol and the clock, and the
defect was that nothing ever turned the clock. **It was found by using the thing**, which is the
method this ledger has recorded as Doug's for eight months.

**The standing claim one section up now needs qualifying.** *"Layout is the one surface where
`egui_kittest` cannot help, so it is the one place a change is verified only by Doug looking at
it"* was true when written and is **no longer the whole truth**: `screenshot.rs` lets Claude look
too. On its first navigated run it confirmed three of that day's fixes — the tab row offering five
tabs rather than eight, the banner reading its payload rather than a note, and the constraint
excerpts a lab station expects — none of which the suite could show directly.

**What it does not change is Doug's judgement.** A photograph tells Claude *what is drawn*; it does
not say whether the drawing teaches. His 2026-08-23 preference for minimising layout churn stands
on its own evidence and is not softened by this.

### 11-04, later — the capture suite's first run, and what one specimen cost to look at

Doug asked whether capture tests aligned with the headless ones made sense. The answer was: not as
a mirror — fifty-odd of those assert facts the accessibility tree can see, and photographing them
would be the same fact twice — but as the complement of **one**,
`every_offered_sub_view_has_something_to_draw`, whose own doc had said the day before:

> It is **not** a claim about pixels; `egui_kittest` cannot reach a painted cell, so a tab whose
> data parses can still look wrong, and **only Doug can see that**.

`examples/capture_suite.rs` is that sentence's falsification. Twelve shots of `Drivetrain` — two
canvas stages x six canvas views, availability **asked of the app, never computed** — five landed,
seven refused, nothing failed. Three defects in the five:

| date | defect | caught by | note |
|---|---|---|---|
| 11-04 | the screenshot reply reported **whatever notice was on screen**, not the one its shot caused | the suite's first run | three consecutive shots came back carrying an earlier request's refusal — true about the screen, false about the shot. Fixed with a counter and a floor recorded at pickup. **Closed a second gap on the way:** four sites assigned `self.notice` directly and were therefore never recorded in the session log that `notify`'s own doc gives as its reason to exist |
| 11-04 | **the After incidence matrix was drawn a third of the way off its column** | the suite's first run, confirmed at full window | `viewport.incidence` is one camera serving two layouts; `Canvas::fitted_rect_size` was written only by the *aim* branch, so `should_refit` was asked `should_refit(ZERO, …)` forever and answered "no" by design. The comment above it promising *"width changes always refit"* described something the code could not do |
| 11-04 | *"still unmatched: &lt;six names&gt;"* on a frame where **97 equations are unmatched** | the suite's first run | `.take(6)` with no sign of truncation. Load-bearing, not cosmetic: the code's own comment says this line is how a reader sees that a system is structurally singular, so anyone counting the names to get the rank deficiency got six |

**The middle one is the one worth carrying, for two reasons.**

*It was order-dependent.* The camera is only wrong after the full-width view has been visited, so
a lab that opens Index Reduction directly never sees it, and neither did two months of fixture
runs. The suite hit it because it walks the product rather than a path.

*And it was invisible to the test that was watching.* `should_refit` is correct and has four tests.
What was broken is that **nothing ever fed it** — the predicate was right and its input was never
set. That is this project's recurring shape, a claim narrower than the guarantee a reader takes
from it, and no test of the predicate could have caught it. The regression test added
(`a_canvas_refits_when_its_column_narrows`) asserts the **outcome** — where the matrix lands in
pixels — and reproduces the photograph numerically: without the fix a 97x97 matrix fitted to a
2048 px canvas lands at 748..1300 inside a 1024 px column.

**One method note, because the first version of that test was wrong.** It asserted on the *zoom*,
which does not move: a 97-unit square in a 600 px-tall pane is height-limited, so `min(zx, zy)` is
identical at both widths. What the width changes is the **centring padding**. A test asserting
zoom would have passed while the matrix sat off the edge — *the right ingredient at the wrong
moment*, which is the 09-22 arc's lesson arriving again in a new costume.

**The widening to all four specimens is `ideas.md` #91, with the costs measured.** Three of the six
canvas views have still never been photographed by anything, because `Drivetrain` refuses them.

### 11-04, the second specimen — `SingleInertia`, and the rate held

Doug asked whether to add it. No code was needed — the runner already takes the specimen as an
argument — so the answer was to run it rather than to build anything. It was the strongest
remaining case: the three views `Drivetrain` refuses are the three nothing had ever photographed.

Two more defects, both in those views:

| date | defect | caught by | note |
|---|---|---|---|
| 11-04 | the BLT graph labelled its nodes **`0 = der(phi)`** and **`0 = J*der(w)`** | the second capture run | the residuals are `0 = der(phi) - w` and `0 = J*der(w) - tau`. `truncate_label(n, 12)` cut them with **no marker**, and both results are well-formed equations that are *false* — the first says the angle never changes. The graph has **no hover**, so the label is all a node says |
| 11-04 | `TearingAnim` offered with a **>** over *"No tearing was recorded for this model"* | the second capture run | two scalar blocks, nothing to tear. The availability rule gates on singularity, which cannot see whether the compile captured anything. **This is Doug's own 11-04 report in a second place** — *"there are triangles in the view labels… but none of those views offer animations"* — and takes the same answer he accepted for Index Reduction: refuse the tab |

**The first is the worse one, and it is a new instance of the project's worst class.** Not a label
that looks cut off — a label that reads as a complete, correct statement and is wrong. `0 = der(phi)`
is exactly the sort of thing that gets read once, believed, and never questioned. The fix is in
`truncate_label` itself, so no call site can shorten silently; the same change turned up a latent
bug on the way, since the old form compared **bytes** and sliced with `get(..max)`, which returns
`None` on a char boundary — so a multi-byte label was silently *not truncated at all*, and the
surviving test asserted that fallback as if it were the intent.

**The second found a test measuring the wrong unit, which is the durable part.**
`every_offered_sub_view_has_something_to_draw` checked, for all three animations, whether an
**incidence matrix** could be parsed — the backdrop they draw on, not the records they replay. So
the test and the app were looking at the same wrong thing and agreed. Corrected, it reproduced the
photograph exactly: one entry, `SingleInertia / Structural / TearingAnim`, across all four
specimens.

**And the corrected test deliberately does not ask what the app asks.** The app gates on *is the
capture non-empty*, which is cheap enough to run every frame; the test asks the panes' own
constructors, which additionally refuse frames that do not **address** this matrix. Had both asked
the same question the walk could never fail — the tautology trap this suite exists upstream of.

**One cross-confirmation worth recording**, because it is the kind that only a photograph gives:
the same specimen's incidence matrix renders its row labels **in full** — `0 = der(phi) - w` — two
tabs away from the graph that was truncating them. The two panes disagreed on screen about the same
equation, and no test compared them.

### 11-04, the third capture run — `CartesianPendulum`, and the defects moved into the suite itself

**Three landed, nine refused, nothing failed — and every one of the nine refusals is correct.**
This specimen is the corpus's index-3 case: structurally singular, *"still singular after
reduction"*, no initialization IR, no simulation. Four consecutive stages whose honest answer is
an absence, which is the kind of pane nothing had photographed before. The photograph of the
Structural tab row — `Summary · Incidence · Matching ▶ · Tree`, four tabs and no more — is what
confirms the refusals are the app answering rather than the app hiding.

| date | defect | caught by | note |
|---|---|---|---|
| 11-04 | `CANVAS_VIEWS` was documented as *"the sub-views drawn with `Painter` calls, and so invisible to `egui_kittest`"* — **false for a third of its entries** | the third capture run | `tearing_anim.rs` and `reduction_anim.rs` contain **zero** painter calls; both are built entirely from `ui.label`. The parity test could not notice: it checks only that each view is named in the *union* of the two rosters, so a view in the wrong list passes exactly as one in the right list does |
| 11-04 | `reduction_anim.rs`'s module doc asserted **three times** that live stepping does not exist | reading the file the photograph sent me into | *"It is not live yet"*, *"`start_live` still returns `None`"*, and that the gap *"is an upstream API question"*. All three were true until `watch_prepare_for_solve` landed the day before; `start_live` now spawns `spawn_reduction_live`, `DISABLED_VIEWS` is empty, and Doug stepped the live session by hand on 11-04 |
| 11-04 | the runner's own status column read `Incidencerefused` | the third capture run's output | the label width was a hard-coded `46`, which `Drivetrain`'s labels fit and `CartesianPendulum / IndexReduction / Incidence` (49) does not. Now measured from the plan before the first shot |

**The roster defect is the one worth keeping, and it is the wrong-negative class again.** The
false half was a claim of *impossibility* — that these two panes cannot be checked headlessly —
and `CLAUDE.md` already names why that is the error nobody catches: acting on it means not
looking, so nothing ever contradicts it. `matrix_panes.rs` spent eight days filed as untestable
with six tests available, on precisely this misreading. A corrected sentence would have rotted the
same way, so the classification is **data** now (`PhotographedView::painted`) and
`the_painted_flag_matches_the_module` reads each module and fails by name in either direction.
Verified failing before being trusted: flipping `Animate` back to `painted: true` fails that test
while `every_structural_view_is_either_photographed_or_excused` passes in the same run, which is
the gap stated as a measurement.

**Both label-only views stay in the suite**, because the suite's second job is to ask availability
rather than re-derive it, and a tab offered over an empty pane is a defect a photograph catches
and a query does not. Both of the previous run's defects were exactly that.

**A limit of the suite this run exposed and nothing yet closes:** it photographs **frame 1 only**,
and `Outcome::Landed` is decided by image dimensions. So an animation that draws nothing lands
identically to one that draws a matrix — the Reduction animation's empty canvas was indistinguish-
able from a full one until I stepped it to frame 6 by hand through the screenshot channel. <!-- unbuilt: capture_suite::is_blank -->

**And a legibility question left open rather than guessed at**, the same shape as
`tarjan_anim::NODE_LABEL_MAX_CHARS`: the matching animation truncates `0 = m*der(vx) - -(l…`
while the incidence matrix one tab over renders the same equation in full, and ~540 px of blank
pane sits immediately left of the truncated label. Both are *marked* as truncated, so neither
reads as a false equation — this is budget, not accuracy, and nobody has measured what the budget
should be.

## Owed sweeps — small `src/` fixes that must not buy a gate of their own

**Doug, 2026-08-31**, asked whether a five-second imprecision should be swept up next time
something else took Claude into `src/`: *"Yes."* **That is the rule these live under.** Each is
small, none is urgent, and each alone would cost a ~101 s FULL gate — so they ride along with the
next `src/` errand instead. **Do them when you are already there; do not schedule one.**

They sit here rather than in `CLAUDE.md`'s Current work because that section is on the mandatory
reading path and this file is not. A pointer stays there.

> ### ✅ ALL FOUR DONE 2026-09-01 — the table below is history
>
> **The rule bent once and correctly.** *"Do them when you are already there"* assumes one item;
> **four together justified a deliberate errand**, because they share a single ~130 s FULL gate.
> Doug ruled it: *"Let's add the item and then do the work now."*
>
> **What the fourth found, which is the argument for having done it at all.**
> `no_lab_uses_retired_vocabulary` immediately caught residue **both** renames had missed — all of
> it capitalised past case-sensitive regexes. `Walk [matching]` in seven labs, and `**Stops:**`
> emitted by the catalogue **generator**, so editing the markdown would have done nothing and the
> word would have returned on the next regeneration.
>
> **And two pre-existing defects surfaced.** `a_code_span_mention_is_not_extracted_as_a_link` was
> misnamed against its own assertion — it asserted the mention *was* extracted, proving only the
> backtick terminator. And `no_item_loses_its_doc_comment` caught `blurb_of` being inserted above
> `specimens_in`'s doc comment, stranding it: **the exact defect that rule exists for, committed
> while adding a checker.**
>
> **The guards, all revert-and-checked:** `every_pinned_lab_claim_holds` (the `blurb` verb, 22
> pins), `extract_hrw_links_skips_placeholders_and_bare_verbs`, `no_lab_uses_retired_vocabulary`,
> and two new assertions in `a_lab_edit_selects_the_lab_gate_and_a_source_edit_still_selects_full`.

### Owed again, noticed 2026-09-04 while publishing the simulation to the bridge

| owed | what to do | why it waits |
|---|---|---|
| **`view_published` overclaims** | `App::publish_current_view` sets `last_published_view` to the key even when `body` is `None`, so `session.json` reports `view_published: "Simulation"` while `view.json` does not exist. Report the key only when something was written — the field is a **dedup key** and a **claim**, and it cannot honestly be both under one name | It is the absence rule pointed at a diagnostic: a claim that a publish happened when it did not. Found by reading, not by a test, and it misled nobody yet because the stage files answered the question instead |
| **`experiment(StopTime)` is ignored** | HRW simulates to its own `sim_t_end` (default 2.0) and never reads the specimen's `experiment` annotation. `RcCircuit` asks for 1 s and gets 2; `BenchActuator` asks for 0.5 s and gets 2 | Defensible — the slider is deliberate and `SimData::to_bridge_json` publishes the *requested* `t_end` beside the achieved `t_final` for exactly this reason. But it is **undocumented**, so a reader comparing a specimen's annotation with a trace finds a discrepancy carrying no information. Either honour it as the initial slider value or say in one comment why not |

| owed | what to do | why it waits |
|---|---|---|
| **Blurb pinning** | Pin each lab's catalogue blurb to its expected first bolded line | bit 3× on 2026-08-31 |
| **Placeholder extraction** | Teach `extract_hrw_links` to skip targets containing `<` | bit 2× the same day |
| **Retired vocabulary** | Pin the count of retired words (`tour`, `stop`-as-a-unit, `walk`-as-a-session) with an allow-list of the deliberate historical uses | both renames were caught by **Doug**, not a test |
| **Lab-region predicate** | Exclude `README.md` and `CATALOGUE.md` from `gate_policy::touches_a_verified_lab_region` | 5 s per README edit |

**The first two are the same session's self-inflicted wounds, and the first is the expensive one.**
A bolded paragraph inserted above a lab's opening bold line **silently becomes the catalogue's
blurb**, and nothing reports it — which is why the reflex became regenerating the catalogue every
turn, ~10 s per prose edit in the mode where Doug feels every second. A metavariable
(`hrw://src/<workspace path>`, or a bare `hrw://src/` in prose) extracts as a real link and fails
the reference checker, which at least fails loudly.

**The third is an over-broad predicate, not a defect.** `touches_a_verified_lab_region` matches any
`.md` under `fixture-labs/`, so editing that directory's README or its generated catalogue selects
the LAB gate (11 s) where FAST (6 s) would do. Neither file has a `pane-*` marker, so neither needs
a compile.

## Priority order — read this before choosing what to fix

Set 2026-07-29, when Doug named the real operating constraint: **his robotics
education has deadlines.**

> Try to imagine me as a Purdue robotics student who is under time pressure to
> complete an assignment, is having a difficulty understanding a concept, needs to get
> an answer from you, but cannot get that answer because we procrastinated
> implementing a bug fix that would require hours to implement. In short, my robotics
> education is not merely going to be for entertainment, on a leisurely schedule of my
> choosing.

**So "high priority" is not enough — fixes here are PRE-EMPTIVE.** High priority means
"first in the next sweep", which still leaves the gap open when the deadline lands, and
the hours a fix needs are hours Doug will not have. **Fix while there is slack.**

Feature *experimentation* stays cheap. **Unavailability does not.** Building the wrong
feature costs some tokens; a missing capability the night before an assignment costs
Doug the assignment.

0. **Anything that shows Doug something FALSE** *(inserted 2026-08-04, above the previous
   rank 1, which it outranks)*. A fabricated pane, a log line describing work that did not
   happen, a view derived by HRW and presented as the compiler's. **A gap is recoverable and
   a fiction is not**: a gap sends Doug to ask, while a fiction sends him away satisfied and
   wrong, with nothing to prompt a second look. He named this his top priority — *"in order
   for me to learn about Rumoca, HRW must accurately represent Rumoca"* — and authorised the
   cost: *"we will pause and fix code as often as necessary in order to deliver accuracy."*
   **This rank does not wait for a sweep and does not wait for slack.** See the accuracy rule
   at the top of [`../CLAUDE.md`](../CLAUDE.md).
1. **Anything that forces Claude to guess instead of verify** — a phase not emitting
   its data, a broken bridge, a claim that cannot be checked. **This is the
   catastrophic case, not a missing lab.** On 2026-07-29 the textbook shape of a
   hidden constraint says Pantelides *differentiates* it; the actual report showed
   `differentiated_rows` empty and `emf.phi` demoted via the dummy-derivative path.
   Recall would have been confidently wrong, and under deadline pressure Doug would
   have had no reason to doubt it.
2. **Anything that makes HRW unavailable** — crashes, hangs, failure to build. Note
   the test suite *hangs* under the default harness; that class of failure bites worst
   at the worst time.
3. **Lab holes** — below. These usually *degrade* an answer rather than block it, since a
   text answer remains available. **All four logged holes are closed as of 2026-07-30.**
4. **Ordinary debt** — everything further down this file.

---

## Lab holes

**A lab hole is a place where HRW stopped Claude from answering a question.** Doug's
ruling, 2026-07-29:

> When attempting to deliver to me the thing which I value most (answers), I want very
> much for you to have available all of the HRW functionality which you need. Fixing
> those gaps and bugs is high priority.

**These outrank all ordinary debt in this file** (but see the priority order above —
anything that makes Claude *guess*, or makes HRW *unavailable*, comes first), including
items that have been open across several sweeps. Ordinary debt costs *future* effort; a lab hole degrades the
*deliverable*, and it arrives with evidence attached — a real question it got in the
way of. **Every sweep starts here.**

Two kinds, and both count:

- **Loud holes** — Claude cannot get there at all, and has to say so mid-lab. These
  get noticed because they are embarrassing.
- **Quiet holes** — Claude works around it with prose ("same tab → now click X") and
  the lab is a little worse at several points. **These are the dangerous ones**: they
  accumulate unnoticed, and the first lab produced one that went unlogged until Doug
  asked whether holes were being tracked.

**All four logged holes are closed.** They are recorded as one line each rather than as a
live table, because a table of closed rows reads like open work *(condensed 2026-08-01)*.

| Hole | Closed | How |
|---|---|---|
| `Matching ▶` hidden when Structural is singular — the view that shows *why* a rank deficiency exists, unavailable exactly when it matters | 2026-07-29 | **No new code.** `MatchingStep::EquationFailed` was already emitted and `matching_anim` already painted the failed row red. The feature was **built and then gated out of reach** — one UI condition — and nothing tested it, which is how it stayed hidden. `a_singular_report_still_animates_and_ends_on_the_failure` pins it. |
| `hrw://` cannot address a **sub-tab**, so every animation and custom view sat one level below what a link could reach | 2026-07-29 | `SubView::from_slug` resolves slugs *per stage*, and the slugs **are** the capture's own names. `link_slugs_and_capture_names_are_the_same_vocabulary` asserts the two lists cannot drift. |
| `hrw://` cannot point at a **source line**, so a lab had to *quote* one | 2026-07-29 | `hrw://source[/<line>]`, plus a tinted blamed line — ideas #45 step 2b. |
| `Canvas` cannot centre on a node | 2026-07-30 | `App::aim_at_equation` → `anim.aim_at_equation(canvas, target)`, with `camera-aiming.md` as its fixture lab. *(This row read "open (unconfirmed)" until 2026-08-01, two days after it shipped — the stale-negative class that `verification-plan.md` item 0b exists to catch.)* |

**Measured effect on the lab that exposed the first three:** regenerated, it went from
**2 working links and 4 prose hand-offs to 9 links and none**, and Station 3 turned from an
apology into the best stop in the lab.

**Under-logged twice, and the pattern is the lesson.** The source-line row was added only
after Doug asked whether it was time to build highlighting — two labs had already worked
around it in prose and Claude had logged neither. That was the *second* quiet hole missed this
way. **Loud holes get logged because they are embarrassing; quiet ones get absorbed.** When
writing a lab, treat every "click this yourself" and every quoted-instead-of-linked reference
as a row here, mechanically, without judging whether it feels worth mentioning.

**Recording discipline.** When a lab hits a hole: add a row here *and* note it in the
[question ledger](question-ledger.md) entry for the question that exposed it. The
ledger says which question suffered; this table is what a sweep reads. A hole worked
around in prose still gets a row — that is the whole point of the quiet/loud split.

---

## Sweep history

**Condensed 2026-08-01.** Three sections recorded past sweeps blow by blow. What survives is
what a future sweep can use: what was fixed, and the lesson. Detail is in git history.

| Sweep | Scope | Fixed |
|---|---|---|
| **2026-07-28** comprehensive | before source-tooling Phases 5-7 | `collect_tracked_ancestors` short-circuited on `any`, so the tree opened a path to the **first** mention only — now folds. `ui()` 1272 → 880. The specimen source view's private copy of the tracking toggle now calls `set_tracked_identifier` like every other entry point. `tree.rs` module docs refreshed. |
| **2026-07-29** scoped to #42 | before Answers | `ui()` 982 → 325 via `FrameIntent`, closing an item open across two sweeps and *blocked* the previous time — extraction needed seven out-parameters threaded through, because egui panel closures borrow `self`. Three dead locals removed (`node_ask`, `debug_ask`, `nav_to` — folded in with `.or(…)` where the fold was always the other operand). Batch narrative regeneration **closed as obsolete**. Test-race entry **corrected**: it blamed `bridge.rs` alone, but `worker`'s stdout redirect races too, and both reproduce on a clean tree. |

**Three items were deferred into #42 rather than swept**, and the reasoning generalises:
unifying the four sub-view enums, decomposing `bridge.rs`, and aiming the canvas were all
**design decisions of the link vocabulary wearing cleanup's clothes**. Sweeping them would
have meant designing the abstraction before knowing what it must express — the mistake that
made `end_to_end_tour.md` worthless. All three have since landed as #42 work.

---

## Measured 2026-08-01

**Measure, never re-estimate** — this file's own first rule, applied to itself. Three days of
drift, and it is not small:

| | logged 2026-07-29 | **measured 2026-08-01** | drift |
|---|---|---|---|
| `app.rs` | 6,375 | **9,039** | **+2,664** |
| `bridge.rs` | 2,365 | **2,855** | +490 |
| `worker.rs` | — | **5,668** | new baseline |
| `ui()` | 325 | **410** | +85 |
| `central_panel_ui()` | 664 | **756** | +92 |
| `compile()` | 363 | **3** | **refactored away** |
| `compile_target()` | — | **454** | the body `compile()` used to hold |
| `source_map_ui()` | 245 | 245 | — |
| `generic_error_summary()` | 201 | 201 | — |

**`app.rs` grew 42% in three days** and is now over 9,000 lines. That is the fidelity, survey
and report work landing, and it is logged rather than swept because the next sweep is scoped by
what the next phase touches — Test mode (#52) and the verification pause both touch `app.rs`
heavily, and `egui_kittest` may change how its UI is structured for testability.

**`compile()` closed itself.** It is now a 3-line wrapper over `compile_target`, which took the
454-line body — so the logged item below is resolved by a refactor nobody logged. The bulk did
not go away; it moved and gained a second caller.

---

## Closed debt

**Cut down to one line each 2026-08-01**, the same treatment `ideas.md` got: a debt file is for
what has not been paid. Full reasoning is in git history and in the sweep table above.

| Item | Closed | Resolution |
|---|---|---|
| Parse stage empty for every MSL model | 2026-08-01 | Fixed — it parsed the empty string Rumoca keeps in place of source-root text; now parses the declaring file. Display-only, so the compile path is unchanged. All 2,553 MSL files verified to parse standalone. |
| **`ui()` was 1,272 lines** | 2026-07-29 | → 325 via `FrameIntent`, which bundles the ~8 intent locals that egui's borrow rules force `ui()` to act on after its panel closures end. Now **410** and growing again — logged below, not re-opened. |
| **The three animation views were near-identical** | 2026-07-29 | `playback::Playback<T>` — a **generic struct, not a trait**, deliberately: a trait would share the behaviour and leave the state declared three times, so it could still drift. `playback::Animated` is the small trait on top for the one thing that cannot be generic — what the current frame *means*. `animation_controls` went 8 positional parameters → 4. |
| **No batch narrative regeneration** | 2026-07-29 | **Closed as obsolete.** The 14 `narrative.md` files it would have driven were retired. Debt that evaporated rather than got paid — one of the three legitimate outcomes. |
| **`compile()` was 363 lines with an inlined `macro_rules!`** | *(discovered closed 2026-08-01)* | Now a 3-line wrapper over `compile_target` (454 lines), which gained a second caller — `compile_model_by_name`. **Resolved by a refactor nobody logged**, which is why the sweep rule is *measure*. |
| **67 clippy warnings in HRW, drifting upward unwatched** | 2026-08-01 | Cleared 75 (it had reached 75 — this session added 8) and set `[lints.clippy] all = "deny"`, so a new one is now a build failure. Three were not style: four constant assertions became compile-time `const` blocks, `--fix` relocated `impl Canvas` above `mod tests` (the lint for the debugger-launch bug shape), and a doc block turned out to have been documenting the wrong function. |
| **Crash files were never pruned, and `cargo test` failures wrote them** | 2026-08-01 | The accumulation was the symptom; **the panic hook is process-global**, so every failing assertion left a `crash-*.json` looking like an app crash — all five found that day were test failures. Now: no full file under `cargo test`, newest 3 kept, and a `crashes.log` digest appended forever so pruning loses no recurrence history. |

---

## Code quality / duplication

- [ ] **`app.rs` is 9,039 lines** *(measured 2026-08-01, up from 6,375 on 2026-07-29 — **+42% in
  three days**).* The growth is the fidelity, survey and report work. **Do not sweep yet:**
  Test mode (#52) and the verification pause both touch it heavily, and `egui_kittest` may
  change how its UI is structured for testability. Sweeping now would be designing against
  requirements that arrive next week.
  *File:* `app.rs`.

- [ ] **`ui()` is 410 lines** *(was 325 after the 2026-07-29 extraction).* Grew back by 85 in
  three days. Not alarming on its own; recorded so the next sweep sees a trend rather than a
  number.
  *File:* `app.rs`.

- [ ] **`central_panel_ui()` is 756 lines** *(664 when extracted 2026-07-29).* A coherent unit —
  stage tab bar, status banners, sub-tab bars, per-stage dispatch — and it takes only
  `&mut FrameIntent`, so further extraction is unblocked. The natural next cut is per-stage.
  **Do not split before #52** reworks the sub-tab bars; that deferral is deliberate and Doug
  acknowledged it.
  *File:* `app.rs`.

- [ ] **`bridge.rs` is 2,855 lines** *(2,365 on 2026-07-29).* It owns `focus.json`, so it owns
  half of noun parity, and #42 kept touching it. Decomposition was deferred into #42 rather
  than swept; #42's remaining work is small, so this is now a genuine candidate.
  *File:* `bridge.rs`.

- [ ] **`Expansion::force_open` exists only because "Reveal identifiers" is a mode.**
  Source-tooling Phase 6 is expected to make revealing an *action*, at which point forcing
  headers open every frame becomes unnecessary and the struct collapses back to one set. Remove
  it when that lands.
  *File:* `tree.rs`.

- [ ] **`lldb.verboseLogging` is off but retained** in `.vscode/settings.json` with a comment
  explaining when to switch it on. **Keep** — it documents a diagnostic that was hard to find.

- [ ] **HRW has never been run on Linux or macOS, and carries untested code for both.**
  *(Doug, 2026-08-08, deferring a cppvsdbg-only sweep: "in the future, we will work together to
  test and verify whether or not HRW works on linux or macos.")* **This is a verification task,
  not a deletion task**, and the distinction is the entry. Nothing here is known broken; it is
  known **unmeasured** — the same shape as a claim of absence with no failing test behind it.

  **What is unmeasured:** `main.rs`'s ~60 lines of Wayland/X11 probing
  (`prefer_x11_if_wayland_dead`, `wayland_socket_reachable`, `xdpyinfo`) — Linux, `#[cfg]`'d out
  here, never exercised; `OutputCapture`'s `#[cfg(unix)]` arms in `worker.rs`, which at least have
  `#[cfg(windows)]` twins that *are* exercised; and the CodeLLDB launch configuration in both
  `launch.json` files.

  **⚠ THE TRAP — read before touching anything.** `bridge.rs`'s `\\?\` path strip is commented
  *"LLDB doesn't recognize that prefix"*, **but `cppvsdbg` needs the strip too**: VS Code's
  breakpoint URIs will not match an extended-length path. A cppvsdbg-only cleanup that trusts that
  comment **deletes working code and breaks breakpoint matching.** The comment is wrong, not the
  code — and most of the apparent LLDB surface is comments like it, not LLDB-specific code.

  **Correct the premise before acting on it.** *"Only cppvsdbg is up to the task"* is **not
  established**: `launch.json` records the case against CodeLLDB as *"now SUSPECT: it predates the
  discovery of Rumoca's compile cache… It has not been re-tested."* The defensible argument for
  narrowing is the one that retired the LLDB teardown — **we run one configuration, and a second
  untested one rots** — not a capability claim. This repository is public and aimed at maintainers
  who mostly run Linux; asserting LLDB cannot do this would be a claim we cannot support.

  **Boundary, non-negotiable:** `crates/rumoca-*` stays debugger- and platform-neutral.
  `live_trace_breakpoint`'s shape is about linkers and optimizers (`/OPT:ICF`), not about one
  adapter. Narrowing HRW is a local choice; narrowing the instrumentation would poison the PR
  `docs/upstream-strategy.md` is building toward.

  **Done looks like:** run HRW on Linux and macOS, *then* fix or delete with evidence — not
  before. *Files:* `main.rs`, `worker.rs`, `bridge.rs`, `.vscode/launch.json`,
  `hrw/.vscode/launch.json`.

## Test debt

- [x] **CLOSED 2026-08-22 — and it was three tests, not one.** `worker::tests::
  a_scratch_specimen_compiles_end_to_end` skipped on the same condition, and
  `a_scratch_specimen_cannot_shadow_a_curated_one` had the other half of the defect: it removed
  its `BouncingBall.mo` on the **last line**, so a failing assertion left a file behind that
  shadows a curated specimen — the exact *"makes Claude guess"* failure that test exists to
  prevent.

  **`test_support::ScratchSpecimen` is the fix**, on the `ui_tests::Answer` contract: save what
  was there, write, and restore in `Drop` so unwinding cannot poison the directory. All three
  tests now establish their own precondition, and the probe's source is a shared constant because
  `worker`'s `n_states == 1` assertion is a property of *that* model.

  **Verified by breaking it**, not by the tests passing: with the probe moved off disk all five
  still passed (so they create their own), and with the guard's write removed **four of five
  failed** — proving the assertions are reached rather than skipped. That break also exposed a
  hole in the new panic test, which passed because *"unchanged before and after"* is equally true
  of a guard that never wrote anything; it now records existence from **inside** the scope.

- [ ] ~~**`app::tests::a_scratch_specimen_is_listed_and_marked` passes vacuously when no probe
  exists.**~~ It returned early unless `.hrw-bridge/specimens/ScratchProbe.mo` was present, so in a
  clean checkout it checked nothing and said so to no one — the must-fire rule pointed at a test
  rather than at production code.

  **The fix is not simply to drop the early return.** Making it non-vacuous means the test writes
  its own probe and removes it, in a directory Doug and Claude both use during a session. That is
  exactly the shape of the three `.hrw-bridge/answer.md` defects (`CLAUDE.md`, *"a test that wrote
  its own and **deleted Doug's** afterwards"*), so it wants the `ui_tests::Answer` treatment —
  restore what was there, including on a panic — rather than a naive create/delete.

  *Found 2026-08-22, when two unrelated scratch specimens broke the test's other assertion. That
  one is fixed; this one is the half that stayed.*

## UI defects — found by running

- [x] **An infinite-width widget must be the LAST thing in its row — FIXED 2026-11-05, and
  the general case is unchecked.** *(Doug, smoke-testing the rebase: the Settings window
  grew off the right of the screen; then, after that fix, "the 'Settings' banner does not
  extend to the right edge of the settings dialog… the 'X' … is placed leftward of the
  right edge.")*

  **One mechanism produced both reports.** `desired_width(f32::INFINITY)` means *all the
  width available **at that moment***. In a left-to-right row it is therefore the **whole
  row**, and anything appended after it is laid out past the container — the row measures
  wider than the window was told it could be. `egui::Window` then disagrees with itself:
  `title_ui` sizes the bar to the width the inner `Resize` *claimed*
  (`child_ui.available_width()`), while `Resize::end` advances the frame by
  `last_content_size`, the width the body *measured*. The banner stops at the claim and
  the frame goes on to the measurement, which is the bare strip Doug saw. Logged as
  `upstream-issues.md` **E4**.

  **The three bounds that hold it**, all in `app::settings_ui`: `min_width == max_width`
  pins the claim (a width that cannot move cannot ratchet, which subsumes the growth fix);
  the specimen-directory row is `right_to_left` so the ⟳ is allocated *before* the field
  asks for infinity; and the library-status label is `.truncate()`, because a label in a
  horizontal row does not wrap and this one reports paths. Each was proved load-bearing by
  reverting it — 52 pt, 28.5 pt and 533 pt of failure respectively.
  `app::tests::an_opened_windows_title_bar_spans_the_window` pins all three.

  **What is NOT checked: every other row in HRW.** The invariant is *a width-filling widget
  is the last widget in its row*, and nothing asserts it. The sweep is small — four
  `desired_width(f32::INFINITY)` sites in `src/`, and the only other live one
  (`model_list.rs:262`) is already correct, with the 🔍 label ahead of the field rather than
  a button behind it. **So there is nothing to fix and no mechanism to catch the next one**,
  which is the gap worth recording: the natural checker is a layout assertion per window, and
  only Settings has one.

  **Who caught it: Doug, twice.** Both reports are the class `CLAUDE.md` names — *what did
  it look like* rather than *what did it contain* — except that this one turned out to be a
  rect rather than a pixel, so it was testable after all. *Files:* `app.rs`.

- [ ] **The "✨ Claude's answer" row vanishes when no Answer exists, and its absence is
  indistinguishable from breakage.** *(Doug, 2026-08-15: "the 'Claude's Answer' lab seems to
  have disappeared from the labs list in the lab mode.")*

  **Not a regression — diagnosed before logging.** `.hrw-bridge/answer.md` does not exist, and
  `lab.rs:181` gates the row on that file:

  ```rust
  if std::path::Path::new(bridge::LAB_FILE).exists() {
      labs.push(LabSource::Answer);
  }
  ```

  No Answer has been written this week, `.hrw-bridge/` is gitignored and ephemeral by
  design, and the listing logic has not changed — the last commits touching `lab.rs` all
  predate the session. So the row is behaving exactly as built.

  **The defect is that "exactly as built" is wrong here**, and the project already has the rule
  it breaks. `lib.rs`'s `LiveState` convention, restated in `docs/ideas.md` #77: *"controls are
  enabled and disabled, never shown and hidden… A layout that makes the lab vanish with no
  trace of how to bring it back is the same defect in a new dress."* A row that silently ceases
  to exist gives the reader no way to tell **"nothing has been written yet"** from **"the
  feature broke"** — and Doug read it, correctly by the available evidence, as the second.

  **Fix:** render the row **always**, disabled when the file is absent, with hover text saying
  what makes one appear (Claude writes `.hrw-bridge/answer.md`; it is regenerated per question,
  never retrieved — `docs/ideas.md` #42). Same shape as the disabled Debug button.

  **Why it matters more than one row.** The Answer is the whole *"Claude can compose an
  answer inside HRW"* capability (#42). If it looks broken, it stops being reached for, and the
  capability dies of apparent absence rather than of any decision.

  **Who caught it: Doug.** That is the backward sweep trigger — the code lives somewhere nothing
  checks, and no test asserts what the lab list offers when the ad hoc file is missing.
  *Files:* `lab.rs`, `app.rs`.

- [ ] **A selected fixture lab does not always open scrolled to the top.**
  *(Doug, 2026-08-01, running the fixture labs as a smoke test.)*

  **Cause identified, not guessed.** The lab pane is
  `egui::ScrollArea::vertical().id_salt("lab")` (`app.rs`) — **one fixed id for every
  lab** — and egui persists scroll offset per `ScrollArea` id. Switching labs therefore
  reuses the previous lab's offset. `select_lab` already resets the right-hand side on a
  real change (specimen, stage, log, compiling) and deliberately *not* on re-selecting the
  same lab; **the scroll offset was simply never included in that reset.**

  **Fix:** extend the existing "only on an actual change" branch in `select_lab` to raise a
  flag, and have the pane apply `.vertical_scroll_offset(0.0)` for one frame. Salting the id
  per lab is the other option and is worse here — it would make each lab *resume* where it
  was left, which contradicts the reset that branch exists to perform.

  **Why it matters more than it looks.** Opening mid-document means Station 1 is off-screen, so
  a lab whose whole purpose is a sequence starts by hiding its start — the same species as
  the RHS-not-re-initialising bug, which made Station 1 look already done.

  **This is the class `egui_kittest` is for** (`verification-plan.md` item 2): its table
  already lists *"the RHS doesn't re-initialise on a second lab"*, and *"scroll offset is 0
  after selecting a different lab"* is the same assertion shape. **Caught by Doug, which the
  backward trigger says is the signal** — nothing checks the rendered surface today.
  *File:* `app.rs`.

- [ ] **`compile_cost` makes the survey artifact non-reproducible.** *(Measured 2026-08-01
  while verifying the runbook.)* The column is **wall-clock derived**, so it varies with how
  many shards are competing: re-running the survey at 4 shards against the committed 6-shard
  artifact moved **16 of 2,626 models, every one `slow` → `fast`** — one direction, which is
  load rather than randomness.

  **It matters because of who we hand this to.** `upstream-strategy.md` requires anything
  published to be **reproducible**, and a maintainer's natural check is *regenerate and diff*.
  Sixteen spurious rows is exactly the noise that teaches people to ignore diffs — the same
  argument `upstream-issues.md` issue 3 makes about the `message` column.

  **Options, none chosen:** drop the column from the committed artifact and keep it in the
  health log; bucket it far more coarsely so threshold-crossing is rare; or record the shard
  count in the sidecar and state that the column is comparable only within a shard count. The
  runbook now documents the instability and gives a diff recipe that excludes it, which is the
  honest interim.
  *Files:* `examples/survey_msl.rs`, `docs/long-runs.md`.

## Robustness

- [ ] **Test races under parallel execution — two causes, not one.**
  *(Corrected 2026-07-29: the entry previously blamed only `bridge.rs`.)*
  1. `bridge.rs` tests share `.hrw-bridge/focus.json` — requires parameterizing the bridge dir
     to fix.
  2. `worker::tests::output_capture_handles_large_write_without_deadlock` redirects
     **process-global stdout**, so any concurrently-running test that writes to stdout steals
     bytes. Inherently exclusive; would need a serial guard or a non-global capture mechanism.

  Both reproduce **on a clean tree** — verified by stashing. Mitigated by `--test-threads=1`,
  required for the *whole* suite. **Worth fixing before any CI, since the default harness both
  fails *and* hangs** — and a hang reads as a broken build rather than a test-isolation problem.
  *Files:* `bridge.rs`, `worker.rs` — test modules.

  3. ~~**A third cause, and `--test-threads=1` does nothing for it**~~ — **PAID 2026-08-15**,
     hours after being logged, at Doug's direction. The other process was **VS Code**:
     `live_trace_breakpoint_arm_remove_and_ack` wrote `breakpoint-request.json` and read it back
     on the next line, while `extension.ts` watched that exact file and *consumed* it — read,
     act, `unlinkSync`, write ack. The read-back raced a filesystem watcher on a path the test
     did not own.

     **It was never observed failing**, because the read landed microseconds after the write and
     the watcher's latency is far longer. That is the part worth keeping: *a test that has never
     lost a race it should not be running is indistinguishable from a correct test*, and the fuse
     was lit only on a machine with the extension installed — Doug's.

     `arm_live_trace_breakpoint` and `remove_live_trace_breakpoint` now **return the JSON they
     wrote**, and the tests assert on that. Both in-app call sites ignore the value, so the change
     is additive. Same move as `Plot::problems()`: push the checkable data out of the I/O path
     rather than testing through it.

     **And the return value is verified, not trusted.** Trading a read for a return is only sound
     if the two cannot disagree — otherwise the tests pass while the extension receives something
     else, which is *worse* than the race, because it fails silently instead of intermittently.
     `write_breakpoint_request_to` is split out so
     `a_written_breakpoint_request_round_trips_to_the_returned_value` can prove the file gets
     exactly the returned value, against a **temp path nothing watches**. Verified must-fire by
     making the writer alter the request.

     ~~`DisarmAnchor`~~ — the guard this originally shipped with is **deleted**. It was the
     wrong shape of answer, and Doug said so by reporting the symptom again: *"There's still a
     live_trace.rs breakpoint set."*

     **Cleanup could never have worked**, because `extension.ts` declines to remove a breakpoint
     it did not add. An arm onto a location that already has one is reported `alreadyEnabled` and
     never enters `armedBreakpoints` — the only list `handleRemove` *or* `clearArmed` consults. A
     window reload empties that list, so anything HRW armed before it is permanently unremovable
     by HRW **and** by the extension's own Clear command. Doug's decision, 2026-08-15: the
     extension keeps that rule.

     So the invariant is **never arm**, and it is now enforced three ways, deepest first:

     - **By construction:** `App::test_with_sender` starts at `Prewarm::Done`. This is the one
       that mattered — `frame_ui` ticks the pre-warm every frame, so *every `egui_kittest`
       harness* armed a real breakpoint on its first paint, and **no test body mentions the
       pre-warm at all.** Two earlier fixes missed it for exactly that reason. The comment there
       claimed "tests drive `tick_prewarm` explicitly; nothing is armed for them", which was
       false the whole time.
     - **By path:** `arm_live_trace_breakpoint_to` / `remove_live_trace_breakpoint_to` /
       `check_breakpoint_ack_at` / `tick_prewarm_at` take the path, and every test passes a temp
       one. `arming_to_a_temp_path_does_not_touch_the_bridge_directory` asserts the real files
       are unchanged.
     - **By source check:** `no_test_arms_a_breakpoint_on_the_watched_path`, over **all** of
       `src/`. Its first version scanned only `bridge.rs` and passed while `app.rs` was still
       arming — *a check scoped to the file it lives in reads as a check on the invariant.*

     **The measurement that closed it:** clear the bridge, run the full gate, read what appears.
     Before, an ack timestamped inside the run said `"action":"add"`. Now the only artefact is
     `"action":"remove"`, from the shutdown-release test — a request that cannot create anything.

     **The transferable rule: isolation by construction beats isolation by convention.** A source
     check sees only what a test *names*, and this arrived through the frame loop.

- [ ] **A stage's emitted JSON depends on what else the session holds.**
  *(Found 2026-08-01 while building the memoisation guard for #48.)* Compiling `Drivetrain`
  early in the test run and again late produces **different Resolve JSON**, against the same
  shared `Session`. The compiles are identical; what changed is that the session has
  accumulated every other specimen's document in between.

  **Not yet characterised** — the difference could be diagnostics that name other documents,
  or something that would matter more. It was found by elimination (two back-to-back compiles
  agree; two separated ones do not), not by inspecting the diff, so **treat the cause as
  unverified.**

  **Why it is worth chasing.** HRW shows the Resolve tab for *one* model, and if that view
  varies with what was loaded earlier in the session then two users — or the same user in two
  orders — see different things for the same file. It is adjacent to
  [`upstream-issues.md`](upstream-issues.md) issue 1, where a *failed* resolve leaked into the
  next model's result; this is the same surface with a milder symptom.

  **First step is a probe, not a theory:** compile a specimen, dump Resolve's JSON, compile ten
  others, compile it again, and diff. Cheap, and it converts a suspicion into a fact.
  *Files:* `worker.rs`, and possibly `crates/rumoca-compile` session caching.

- [ ] **`build_declaring_classes` resolves only the first path segment.** `src.V` resolves
  exactly; `gear.flange_a.tau` yields `gear`'s type, which *contains* the declaration rather
  than being it. The UI wording says "in" rather than "declared in", so it is honest — but a
  deeper resolution would need to run into library class IRs that are loaded on demand.
  *File:* `app.rs`.

- [x] **The bridge ack says "I read your request", and HRW reads it as "a breakpoint exists."**
  *(Found 2026-08-08 while diagnosing `ideas.md` #74; **not** what caused that bug.
  **FIXED the same day** — `ideas.md` #75.)* The ack now answers one question, *"does an **enabled**
  breakpoint now exist at every requested line?"*, and `BreakpointAck` has four variants where a
  `bool` used to be: `Armed`, `NotArmed(reason)`, `Unreportable`, `Pending`. **`replied()` ends the
  handshake; only `is_armed()` licenses the claim.** The decision logic lives in
  `vscode-extension/src/arm_verdict.ts`, which imports no `vscode` so `node --test` reaches it —
  `extension.ts` cannot be tested at all. **A second bug surfaced while fixing this**: `isDuplicate`
  never checked `bp.enabled`, so one click of *Disable All Breakpoints* reported a dead line as
  covered. It is deleted; `findExisting` returns the breakpoint so the caller can read the flag.
  The original entry follows. The
  extension writes `.hrw-bridge/breakpoint-ack.json` unconditionally at the end of
  `handleRequest` — after an add that armed nothing because every entry was a duplicate, after a
  remove that matched nothing, after any request at all. HRW then does
  `self.live_breakpoint_armed = acked`.
  **This is `#71`'s fiction one layer down**: that fix stopped a *timeout* passing for success,
  and left an ack that carries no information about what happened passing for success. It reaches
  Claude too — the context capture emits `breakpoint_armed`.
  **It is live right now**, because the #74 fix makes the second Debug press hit
  `Already armed: … — skipped`: a breakpoint does exist, so the claim is true by luck rather than
  by evidence. **The shape of the fix** is for the ack to carry counts (`armed`, `skipped`,
  `removed`) and for HRW to believe only a positive one. Needs a must-fire test on both sides;
  the extension half belongs in `debug_state.ts`-style pure code, not in `extension.ts`, which
  imports `vscode` and cannot be tested. *Files:* `vscode-extension/src/extension.ts`,
  `src/app.rs`, `src/bridge.rs`.

- [x] **HRW never clears its live-trace breakpoint on shutdown.** *(Doug, 2026-08-07;
  **FIXED 2026-08-08**.)* `eframe::App::on_exit` now calls
  `App::release_live_breakpoint_at_exit`, guarded by
  `app::tests::quitting_releases_an_armed_live_breakpoint` — which checks **both** branches,
  since writing a removal request on every quit would undo `#71`'s rule that HRW must not assert
  state it cannot see. The body is a separate method because `eframe` owns the call to `on_exit`
  and no test can reach it; that is the extraction `format-and-app-plan.md` allows, one that
  buys a test which could not be written before.
  **Two limits stay, stated rather than fixed:** a panic, a kill or a debugger stop runs no
  destructor, and an orphan from a *late* bridge ack was never tracked, so `on_exit` cannot see
  it. `HRW: Clear Armed Breakpoints` remains the remedy for both. The original entry follows,
  since its measurement is what justified the shape.
  **Measured, not assumed:** `remove_live_trace_breakpoint` has **13 call sites in `app.rs`**,
  and every one is reactive to an in-app event — a session ending, a stage changing, a new
  Debug click. **HRW implements neither `eframe::App::on_exit` nor `Drop for App`**, so closing
  the window while a breakpoint is armed leaves it registered in VS Code.

  **Why it matters beyond tidiness.** The orphan is a breakpoint *the user did not set*, sitting
  in `live_trace.rs`, and it will stop **any** debug session that reaches that code — a plain
  debug launch, or stepping a Rumoca test. A breakpoint nobody remembers arming is a confusing
  stop, and this project's whole thesis is that a confusing signal teaches something false.

  **`#71` made an orphan more likely**, which is why this is logged now: a bridge ack arriving
  after `LIVE_DEBUG_ACK_TIMEOUT` now leaves a real breakpoint HRW deliberately does not track,
  so no in-app event will ever remove it. That trade was accepted on the grounds that HRW must
  not claim state it cannot see — this entry is the other half of paying for it.

  **What is NOT yet known** — do not build before checking, the `#67`/`#70` discipline:
  the next HRW start pre-warms by arming the same anchor and removing it
  (`tick_prewarm`), so a stale anchor **may already be cleaned up on the next run**.
  <!-- unverified --> If so the exposure is only the window between runs, which lowers the
  priority but does not close it — the window is exactly when a learner switches to debugging
  something else.

  **The remedy already exists manually**: the extension contributes
  `HRW: Clear Armed Breakpoints`. **First step is to measure the window, not to write the
  hook** — arm a breakpoint, close HRW, and look at VS Code's Breakpoints pane.

  *Files:* `app.rs` (no exit hook), `bridge.rs` (`remove_live_trace_breakpoint`).
  **Note a hook cannot be complete**: a debugger stop, a panic or a kill runs no destructor, so
  the extension side is the only place a guarantee could live.

## The committed notebook was stale for 25 days, and nothing in the toolchain could notice

**Found 2026-08-15, while regenerating traces for the equation-id sweep. Paid the same day.**

`parse.json` had not been regenerated since the subtree import on **2026-07-21**; the rest of
the pre-Flatten stages since **2026-07-29**. Only **7 of 21** manifests listed the `dae` stage,
so a reader of the committed notebook saw a pipeline HRW no longer had. `hrw/CLAUDE.md` says the
trace is *"generated and therefore correct by construction — any number about a specimen is read
from here"*, which makes this a correctness problem: that sentence is a standing instruction to
trust the notebook, and every count in the nine labs was read from it.

**`architecture.md` had exactly this disease and was cured on 2026-08-12 by generating it *and*
adding a currency test.** The notebook got the first half seventeen days earlier and never got
the second. It was the largest generated artefact in the repository with no drift detection.

### The detour that nearly became the fix, recorded because the reasoning was wrong

Regenerating writes ~6,600 absolute paths (`C:\Users\dougd\...\specimens\RcCircuit.mo`), and the
first response was to propose normalising them to repo-relative before writing — described in
this file as a tension with the accuracy rule, since it would mean *"editing what the compiler
said"*.

**That was wrong on the facts.** `Location::file_name` is not something Rumoca discovers; it is
the argument HRW passes to `parse_to_ast`, and `worker.rs` passes the **full URI deliberately** —
a basename made HRW's AST differ from the compiler's own on 400 of 400 MSL documents, and made
`bridge::slice_source` unable to resolve a file. There was no accuracy tension to weigh, because
nothing was being edited.

**Doug: *"you designed a workable absolute-path-based system a while back. Why are you
considering changing that? It seems that we should keep what works."*** He was right, and
checking settled it: `slice_source` requires `is_absolute() && is_file()` and **falls back to the
specimen path** when either fails, so a trace carrying another machine's path degrades to the
safe branch. The cost was cosmetic — one large diff — and his two clones are at the same path,
so it is not even that.

**The transferable failure: a blocker on the way to the problem became the topic.** The problem
was 25-day staleness; the paths were an obstacle to fixing it, and half a discussion went to
redesigning a working subsystem instead. Second occurrence in one day.

### What closing it turned up: the notebook takes the session as a hidden argument

The content check was first written against `compile_specimen_shared` — 49 s, cheap enough for
the `slow-tests` gate. **It passed alone and failed in the full suite**, nine files across
`GearWithBrake` and `MissingComponentClass`. That is not a flake; it is the open item below
(*"a stage's emitted JSON depends on what else the session holds"*) arriving as a consequence
nobody had drawn from it.

**The consequence is about the notebook, not the test.** A committed trace is one sample of a
function whose hidden argument is the session. `gen_trace` runs **one process per specimen**, so
the committed value is the *virgin-session* value — and any check that compiles in company is
comparing against a different input. So the check builds a fresh `WorkerState` per specimen, is
therefore order-independent, and reloads the MSL 21 times: **157 s**, which is why it has its own
feature rather than joining a gate already reported as friction.

**And the path spelling is part of the output.** The first faithful run reported **109 of 109**
files drifted. Nothing had drifted: the test built the specimen path with `Path::join`
(`hrw\specimens\X.mo`) while `gen_trace` uses `format!` (`hrw/specimens/X.mo`), and that string is
stamped into every `Location` by `parse_to_ast`. One separator, total apparent drift. **A check
that reproduces a generator must reproduce how it spells things, not merely what it means.**

*Closed by:* `manifest_stage_rosters_match_the_pipeline` (fast, in the ordinary gate) and
`the_committed_notebook_matches_what_the_pipeline_produces_now` (`--features notebook-check`).

**Run the content check after touching a `*_to_json` writer, and after rebasing on upstream
Rumoca** — the same two triggers the large fidelity sweep carries, for the same reason.

```text
cargo test -p hrw --lib --features notebook-check -- --test-threads=1 the_committed_notebook
cargo run -p hrw --example gen_trace -- --all      # 3m45s, when it fails
```

## The Context Bar reported less than it emitted, and only Doug noticed

**Who caught it: Doug** — which is the backward sweep trigger. Nothing in the
toolchain could have: the bar had **no test of its own**, so every claim it made
was true and the claims it *failed* to make cost nothing.

Two distinct omissions in one widget, both invisible for the same reason:

- the **stage** was never drawn, from the bar's first commit (`b2732393`);
- the **specimen** was drawn only in the populated branch, so the empty-state
  branch — the common case — rendered `Context — nothing assembled` while
  `focus.json` was carrying both.

**The general shape: a reporter that under-reports.** The must-fire rule says
silence must be a failure, and the bar was never silent — it spoke, just
incompletely. **Everything it said was correct**, which is why review, tests and
daily use all passed over it. A partial report is harder to catch than no report,
and this one sat in the widget whose single job is to make emission legible.

**What now checks it:** `ui_tests::the_background_names_both_the_specimen_and_the_stage`
and `..._names_the_stage_before_a_model_exists`. The two branches share one
`background_ui`, so they can no longer drift apart.

**What remains unswept:** the rest of `context_bar_ui`'s rows have no headless
test either. The point-at and follow rows are *labelled*, so an omission there is
visible in a way the background's was not — which lowers the priority without
removing it.

## Sweep 2026-08-04 (comprehensive) — silent data loss in the JSON readers

**Scope override, at Doug's direction:** *"comprehensive is an override for this sweep because
we've changed a lot of code today and discovered some real problems today."* The standing rule
is next-phase-scoped; this one is not, and the justification is recorded so the rule does not
erode into "sweep everything, always."

**Measured at the start** (compare the 2026-08-01 sweep): `app.rs` **10,598** lines (9,434 at
the UI pause, 9,900 on 2026-08-03), `worker.rs` **8,466**, 583 `#[test]`s, **0** TODO/FIXME
markers, **6** `#[allow]`/`#[expect]` attributes.

### The finding: `filter_map` with `?` is silent data loss wearing defensive-parsing clothes

Every JSON reader that turns a compiler report into a view was built this way. At the call site
it reads as careful parsing. What it actually does is **drop any entry the parser does not
understand, leaving no gap where it was** — the same shape as the Context Bar defect, which is
why `CLAUDE.md` requires a pane to ship with a test.

**Two cases were collapsed into one**, and separating them is the whole fix:

| Case | Truth | Old behaviour | Now |
|---|---|---|---|
| key **absent** | the compiler produced nothing here | empty list | empty list ✅ |
| key present, **not a list** | the report's shape changed | empty list ❌ | reported |
| one **entry unreadable** | a row is missing | shorter list ❌ | reported with a count |

**Measured spread: 31 `filter_map` sites** — `worker.rs` 12, `ic_plan_anim` 6, `bridge.rs` 3,
`reduction_view` 3, `incidence_view` 3, and one each in `alias_anim`, `matching_anim`,
`tarjan_anim`, `tearing_anim`. **Not all are fiction-generating** — some genuinely filter by a
predicate, where returning `None` means *this entry does not qualify* rather than *I could not
read it*. The two are indistinguishable from the call site, which is the deeper problem.

**Fixed: `reduction_view.rs`**, via a `parse_list` helper that separates absent from unreadable
and records what it could not read; the pane renders it **above the summary, in the error
colour**, because a reader who scrolls past would be reading an incomplete list believing it
complete. Three tests, including the negative case.

**This view first, deliberately.** `differentiated_rows` *is* the Pantelides output, index
reduction is the next link in Doug's chain, and the priority-1 example at the top of this file
is a case where reasoning about that list would have been confidently wrong.

**Also fixed: `events_stage` asserted smoothness from a failure to read.** `.unwrap_or(0)` on an
unreadable summary produced zero, and zero produced *"no events — this model is a smooth
(continuous) system"* — **a positive physical claim about Doug's model, manufactured from a
parse failure.** A model full of `when` clauses would have carried that label with nothing on
screen to hint the label came from HRW rather than the compiler. The three cases (countable and
zero, countable and non-zero, not countable) are now distinct.

### `incidence_view.rs` — three sites, the most consequential in HRW

Audited and fixed second, because this matrix is the object the matching and BLT labs teach on.

1. **A dropped column.** A non-numeric entry in a row's `unknowns` was removed silently, so the
   matrix showed an equation **not depending on a variable it depends on**. Every structural
   conclusion a reader draws from the picture would come from a different matrix.
2. **A BLT block that lost members was still drawn**, at its reduced size — a four-equation
   coupled block could appear as a one-equation block, a decomposition the compiler never
   produced. Same family as the fabricated BLT tabs, reached by a different route.
3. **An unresolved matching pair was skipped, and `n_matched` *is* the structural rank** —
   `caption` reports deficiency by comparing it against `min(n_eq, n_var)`. A dropped pair makes
   a fully-matched system read as **singular**, which is the single conclusion the matching
   curriculum turns on.

**None of the three could have been caught downstream**: each produces a well-formed matrix that
is quietly a *different* matrix.

`caption()` now leads with a warning when anything failed to resolve, and `caption_ui()` exists
so a caller **cannot render the caption and forget the problems** — which is what the three
`app.rs` sites did, each with `ui.weak(mat.caption())` and nothing else. Dim grey is also the
wrong weight for a warning. Four tests including the clean-report negative.

### `alias_anim.rs` and `ic_plan_anim.rs` — the same shape, two sharper consequences

- **`alias_anim`: a dropped elimination corrupts a count, not just a frame.**
  `unknowns_before` is computed as `n_unknowns + frames.len()`, so an unreadable entry makes the
  animation narrate removing *n* variables from a system it simultaneously reports as one
  variable smaller than it actually was.
- **`ic_plan_anim`: `dropped_equations` is the sharpest case in HRW.** It lists the equations the
  compiler **threw away** to make initialization solvable, so a lost entry under-reports what was
  discarded — telling the reader the compiler relaxed *less* than it did, which is the exact
  opposite of what the hint exists to disclose.
- Both render problems **above** their own "nothing here" messages, which are positive claims
  (*"No alias eliminations in this model"*, *"Nothing has to be solved at t=0"*) that become
  false the moment an entry is lost.

**`parse_list` moved to [`src/json_read.rs`](../src/json_read.rs)** now that three views use it,
with the module docs carrying the whole argument and three tests of its own. Its docs state
explicitly what it is *not* for: a `filter_map` that genuinely filters one enum variant out of a
frame stream is correct and must **not** be converted.

### Audit complete — and the headline count was inflated by test code

**The `filter_map` audit is finished.** Final classification of the 31 sites:

| | Count | Where |
|---|---|---|
| **Fixed** (silent data loss) | 15 | `ic_plan_anim` 6, `reduction_view` 3, `incidence_view` 3, `alias_anim` 1, `tearing_anim` 1, `worker.rs` 1 |
| **Genuine filters**, correct as they stand | 3 | `matching_anim`, `tarjan_anim` (one enum variant out of a frame stream), `worker::partial_matching_to_json` (`var_idx: Option` — `None` means *this equation is unmatched*, which is the fact being reported) |
| **Test code** | 13 | `worker.rs` 10, `bridge.rs` 3 |

**The correction worth keeping: the per-file counts came from an unfiltered `grep -c`, so they
counted assertions inside `mod tests`.** `bridge.rs` was called "the highest remaining priority"
on that basis — its emitter has **no** `filter_map` at all, and all three sites are test
assertions. `worker.rs` had 2 production sites, not 12. **A measurement that does not exclude
test code is not a measurement of the product**, and it produced a wrong priority call in this
very sweep, which is the sort of thing this file exists to remember.

### Two findings the audit produced that the first pass missed

- **`events_stage` was fixed twice.** The first fix (three commits earlier, same day) replaced
  `.unwrap_or(0)` on the summary *object* but left `filter_map(as_u64)` on its *values* — so a
  summary that was present but held a non-numeric count still summed to zero, and zero still
  produced *"this model is a smooth (continuous) system"*. **The audit caught the fix's own
  blind spot.** Four cases are now distinct: unreadable object, unreadable entries, zero, and
  non-zero.
- **`tearing_anim` refuses rather than reports**, unlike the other four. That constructor
  already returns `None` when the capture and the report disagree on block count, because *"an
  animation that quietly pairs them would be a wrong picture with no symptom"* — and an
  unreadable member name is the same disagreement one level down. Tearing is *about* which
  variables in a block get torn, so a block missing a member does not look smaller, it looks
  like a different choice over a different system. The caller already says the animation is
  unavailable.
- **`parse_list` lives in `reduction_view.rs`** and should move somewhere shared once a second
  view uses it. Not moved yet — one caller is not a pattern.
- **The predicate-vs-parse ambiguity has no mechanical guard.** A `filter_map` that filters and
  one that silently loses data are the same expression. Worth thinking about whether the parsing
  ones can be made to look different at the call site, which is what `parse_list` does for one
  file.

## Accuracy item 3 — sub-view provenance, re-scoped by what it found

**The item as written asked for a `Provenance` label on the sub-views** (`IncidenceMatrix`,
`ReductionView`, `Plot`, the animations). Examining them first showed that would be **ceremony**:
every production sub-view is fed from a report or a capture, the re-deriving constructors are
`#[cfg(test)]` so the compiler forbids the fabricating case, and a field whose value is the same
everywhere catches nothing. Recorded rather than built, because the reasoning is the deliverable.

**The real failure mode for these views is not fabrication — it is MISPAIRING**, and it has
occurred twice: the `CapacitorLoop` Tarjan animation drawing blocks the compiler never built, and
raw-run frames rendered against the reduced matrix (Drivetrain, 97 equations vs 20). Both are
"the right kind of data about the wrong system".

**That defence already exists** and was checked view by view: `matching_anim` and `tarjan_anim`
validate the capture against the matrix, `tearing_anim` refuses on a block-count mismatch, and
`connection_anim`/pre-lowering take frames alone with nothing to mispair against.

### What was actually missing: a count was standing in for an identity

`match_eq.len() == n_eq` catches the mismatch that happened and is **blind to two systems of
equal size** — which index reduction can produce, since demoting a state to algebraic moves a
variable without necessarily changing the equation count. That is a count deciding identity, the
same objection [`identity-and-provenance.md`](identity-and-provenance.md) makes to substrings.

Both constructors now also require every matched column to exist in this matrix — free, strictly
stronger, still a proxy. Tested with frames that pass the length check and address a column
beyond the matrix.

**A true identity would need the capture to carry something naming its DAE**, which is a
capture-side (Rumoca) change and is deliberately not attempted here.
<!-- unbuilt: StructuralFrames::dae_fingerprint -->

## Accuracy item 6 — spyplot, and the audit's own blind spot

### `str_vec` hid the pattern the audit was grepping for

**The `filter_map` audit searched each file for the literal token.** `str_vec` is
`filter_map(as_str)` **and** `unwrap_or_default()` in one shared helper with **9 callers**, under
a name that hints at neither — so every one of those call sites reached the same silent loss and
was invisible to the audit. **An audit by grep finds a pattern only where it is spelled out.**

`str_vec_checked` now exists beside it and returns what it could not read. Converted where the
loss changes a picture (all four `spyplot` calls); the remaining callers are logged below.

### Two defects in `spyplot.rs`

1. **An unreadable `kind` became a scalar block.** `kind == Some("coupled")` sent every
   unreadable kind down the `else` branch, which builds a **1×1 block from a single
   equation/unknown pair** — so a coupled block was drawn as one cell on the diagonal, and
   `coupled_count` (quoted in the caption as *"N coupled (algebraic loops)"*) came out one too
   low. **The spy plot exists to show where the coupling is**; reclassifying a coupled block as
   scalar inverts the one thing the picture is for.
2. **Tear variables were dropped silently** through `str_vec`. They are the *answer* tearing
   produces, so a lost one shows a block torn on fewer variables than it was.

`caption()` leads with the caveat, and `Plot::problems()` makes both checkable — which matters
here more than elsewhere, because **this canvas is one of the three surfaces `egui_kittest`
cannot reach**, so the parsed data is the only thing a test can hold.

### The Context Bar assertions asserted something else

Four tests used `query_by_label_contains("RcCircuit")`. That is satisfied by **any** source
line, list row or notice mentioning the specimen — so they did not assert *"the Context Bar
names the specimen"*, they asserted *"exactly one thing on screen mentions it"*. One broke
earlier today when the source pane legitimately started rendering. The precondition now matches
the background's own shape (`· name · stage`).

### Still open from this item

- **`str_vec`'s other callers are unconverted**: `reduction_view` (demoted states),
  `incidence_view` (unknown names, BLT members), `tearing_anim`. `incidence_view`'s
  `unknown_names` is already protected by a length check against `n_var`, and its BLT members by
  the size-mismatch report added earlier today — the rest are unexamined.
  <!-- unbuilt: str_vec_checked_everywhere -->

## Accuracy items 4 and 5 — stale docs, and the labs

**Item 5, the labs: checked and NOT stale.** Verified rather than assumed, four ways —
no fixture lab cites an HRW source line; the `docs/compiler-phases/` citations that matched
today's changed symbols are all to **Rumoca's** names (`build_blt_from_incidence`, snippets
quoted from Rumoca sources) and no `crates/rumoca-*` file was touched today; the numeric claims
(`2 equations, 3 unknowns, balance = -1`) are pinned by passing tests; and
`fixture_lab_links_all_resolve` runs every build.

**Item 4, stale doc comments: the six known ones were fixed as they were found. A systematic
scan found no further FALSE comments — it found an ambiguity instead**, and the ambiguity is
the more dangerous finding.

### "Replay" now means two things, and one of them is forbidden

The word is live in both senses, in the source, the UI and the labs:

- **Playback of frames recorded during the real compile** — the animation feature. Correct.
  The UI says *"no frame 3 in this replay"*; `frame-seeking.md` says *"the reduction replay
  opens"*.
- **Re-execution presented as the compile** — the fiction, eliminated 2026-08-04.

And a third case that is neither: **a live debug session genuinely re-runs a phase**, because
that is what the user asked for. `PendingLiveDebug::PreLowering`'s comment saying it *"re-runs
from the flat model"* is **correct and must not be 'fixed'**.

**The failure mode this creates is bidirectional**, which is why it is written into
[`../CLAUDE.md`](../CLAUDE.md) rather than left as a rename: a later session reads "the
reduction replay" in a lab and either deletes a working feature, or concludes the fictions
were already handled and stops looking. **Judge by where the frames came from, never by the
word.** `CompileFrames` — which holds the good kind — was reworded to avoid it entirely.

## `unsafe_code` is the one workspace lint HRW could adopt and has not

**Logged 2026-08-05**, during the formatting work that found `hrw` inherits **no** workspace
lints at all. Five were adopted the same day at zero cost (`hrw/Cargo.toml` records which, and
which were declined and why). `unsafe_code` is the remaining adoptable one.

**13 violations, all real and all in one place**: `OutputCapture`'s fd-level pipe handling in
`worker.rs` (~8955 and ~9410), where `libc::pipe` and direct `write` to fds 1/2 are required
rather than incidental — HRW captures output *below* Rust's `BufWriter` because `tracing` and
`println!` from the Rumoca crates bypass it.

**What it needs is not a flag but thirteen reasons.** Adopting means
`#[allow(unsafe_code, reason = "…")]` at each site, and the value is entirely in those strings:
*why* this must be unsafe and what invariant makes it sound. Written well they document the one
module in HRW that can corrupt process state; written as `reason = "needed"` they are noise.

**Not urgent, and not free.** No defect traces to this code. The reason to do it is that
`unsafe` is exactly the category where a **new** occurrence should be argued rather than
noticed, and today nothing would notice.
<!-- unbuilt: hrw unsafe_code lint -->

## ~~A flagged stage shows its error INSTEAD OF its artifact~~ — FIXED 2026-08-05

**Found by Doug 2026-08-05**, running `failure-typecheck.md`: the lab claimed a tree below the
diagnostic and there was none. **Rank 0** — the pane shows less than the stage holds, and says
nothing about it.

**The data is there.** `DimensionMismatch`'s Typecheck stage carries **7.4 KB**: the full
instantiated overlay (`components`, `classes`, `type_roots`, …) **plus** an `error` key. The
worker assembles it on purpose — `worker.rs`'s comment reads *"the instantiated overlay is the
last good state to show **beside** them"*.

**`App::central_panel_ui` discards it.** It computes
`note_is_error() && value["error"].is_some()` and, when true, renders `generic_error_summary` in
an `if`, with the whole tree branch as the `else`. **Beside became instead of**, and the overlay
is built on every compile of every flagged model and dropped at the last step.

**Nothing on screen indicates content is being withheld** — the Context Bar defect's exact shape:
a partial report leaves no gap where the missing part was.

### The fix is not "always show the tree"

`note_is_error()` is true for **both** abnormal outcomes, and the two differ in what the value
holds:

- **`Flagged`** (`Stage::recovered`) — the value is a **real artifact** plus an error. Both should
  render.
- **`Failed`** (`Stage::err_with_details`) — the value is **only** `{"error": …}`. A tree there
  would render the error payload as a tree, which is noise beside the summary.

So the condition is not the outcome but **whether the value carries anything beyond `error`**.
That distinction is cheap to compute and is the whole fix.

### Fixed the same day, after Doug rejected the reason for deferring it

**It was first logged as "not attempted — more invasive than the remaining session budget
allowed."** Doug: *"I do not understand your reasoning. Why not correctly implement the whole fix
right now?"* He was right. **The honest reason was low context and a worry about starting
something unfinishable, described as though it were a property of the code.** The edit is about
fifteen lines.

**Recorded because the failure mode is a habit, not an incident**: dressing a budget concern as a
technical one makes a deferral look principled and stops the reader from questioning it. The
"this is more invasive than it looks" claim should carry a line count.

**What landed:** the error summary now renders in a collapsible `⚠ What went wrong` header
**above** the tree when the value carries content beyond `error`, and fills the pane alone when it
does not. Two tests, `a_flagged_stage_shows_its_artifact_beside_its_error` and
`a_failed_stage_with_only_an_error_shows_no_tree` — the first **verified to fail on the old
behaviour** before being kept. 598 tests pass.

## Accuracy item 1 — the `unwrap_or_*` family (a second, separate silent substitution)

**The `filter_map` audit did not cover this.** `unwrap_or_default()` / `unwrap_or(0)` /
`unwrap_or("")` substitute a *value* where `filter_map` drops an *entry*, and they are far more
common: measured **75** in `worker.rs` production, 10 in `reduction_view`, 9 in
`incidence_view`, 7 in `matching_anim`, 6 in `ic_plan_anim`, 3 in `tearing_anim`, 2 each in
`spyplot`/`equation_sheet`/`tarjan_anim`, 1 in `identifier_index`.

**Most are legitimate** — a genuinely optional field with a truthful default. Four were not:

1. **`expr_to_short` rendered a missing operand as the empty string**, so `a * 2` with an
   unreadable left side displayed as `" * 2"` and `x + y` as `"x + "` — **a different equation,
   shown as the model's equation**, with correct spacing and plausible syntax. Equations are
   what Doug reads; this is as damaging as a wrong incidence matrix. Now `(missing)`, a marker
   deliberately distinct from the existing `(expr)` (which means *a shape HRW cannot print* — a
   limit of the tool, not a hole in the data).
2. **An unreadable function name rendered as `"f"`** — a legal Modelica identifier, so `sin(x)`
   became `f(x)`, indistinguishable from a real call. **The substitution was not merely lossy,
   it was convincing.** Absent args and an empty args list were also collapsed, though `time()`
   really does take none.
3. **A nameless incidence row was named `""`, and `eq_index` is a name→row map** — so two
   unnamed rows collided, the second overwriting the first, and a matching entry with its own
   name missing (also `""`) then resolved to whichever row won. **A matched pair attached to the
   wrong equation**, in a picture that stays well-formed.
4. **`match_progress` returned `(0, n_eq)` when no frames had arrived**, and its own doc says
   `matched < total` means structurally singular. So no data produced **the strongest possible
   claim about the model**. Unreachable for a recorded animation; a **live debug session** is
   what reaches it, rendering while frames still arrive. Now `Option`.

**Left alone, and checked:** `unwrap_or("?")` in the incidence hover tooltip (visible, honest),
and `binary_op_symbol`'s fallthrough to the variant name (an unknown operator renders as itself,
which is odd-looking and therefore truthful).

## The source pane told a selected model to select a model

**Found by the sweep, 2026-08-04**, following the `&Path`-vs-qualified-name generalisation from
the `SecondOrder` bug. The pane read `self.selected` from disk with
`read_to_string(path).unwrap_or_default()` — and for a library model `selected` holds the
**qualified name**, not a path. When the worker had not yet supplied the declaring file's text,
the read failed, **the empty string was cached**, and the fallback arm printed *"Select a
specimen to view its source."* while a model was selected.

**A failure to read, rendered as a different, plausible, false claim** — and this one sends the
reader to fix something that is not broken. Four distinct causes had shared one sentence:
nothing selected, library text not yet arrived, read failed, file genuinely empty.

**The library half was already right.** `source.library_error` exists with the comment *"A read
failure is reported, never rendered as blank"*. The specimen half never got the same treatment,
which is the asymmetry this sweep was looking for.

`load_error` doubles as the retry guard — without it a missing file was re-read **every frame**,
a filesystem call in the paint path.

### Two things this broke, and both were the fixture's fault

- `test_set_session_state` names files that do not exist (`RcCircuit.mo`). Harmless while the
  failure was silent; once the pane reported it, tests broke. **The fixture was the unrealistic
  thing** — a test whose app is in a state the real app cannot reach is testing something that
  does not happen — so it now seeds source text rather than the assertion being loosened.
- **The seeded text deliberately omits the model name.** Several tests assert on the Context Bar
  by searching for the specimen name, and *any* source on screen gives them a second match.
  **That coupling is fragile and unlogged until now**: those tests do not really assert "the
  Context Bar names the specimen", they assert "exactly one thing on screen mentions it".
  <!-- unbuilt: ui_tests::context_bar_query_is_specific -->

### Also: `get_all_by_label_contains` cannot express absence

It **panics** when nothing matches, so `.next().is_none()` is not a way to assert something is
absent — it is a way to fail. Use `query_by_label_contains(..).is_none()`. Cost one debugging
round in this sweep.

## Simulation never worked on a corpus model, and nothing could have found it

**Found by Doug 2026-08-04**, pressing Run on `Modelica.Blocks.Continuous.SecondOrder`:
*"read error: The system cannot find the file specified. (os error 2)"*. Fixed the same day.

**The defect.** For a library model the UI's `selected` holds the **qualified name**, not a
file — `open_library_model` sets `PathBuf::from(qualified)` and a `selected_is_library` flag
beside it. `ToWorker::Simulate` carried only the path, and `simulate` opened with
`read_to_string(path)`. The compile path had gained `CompileLibraryModel` when the corpus list
shipped; **the simulate path never got its counterpart**, so every one of the 2,626 corpus
models was un-simulable from the day the list arrived.

**Why it survived, which is the part worth keeping.** It was not un-tested — it was
**un-testable**. Every simulate test went through `simulate_specimen(&Path, …)`, whose
signature *cannot express a library model*. There was no headless way to reach the branch, so
no amount of diligence in writing tests would have covered it. `simulate_library_model` was
added as half of the fix, and `an_msl_library_model_simulates` is the first test that could
exist.

**The sweep trigger this belongs to is #2 — caught by Doug, not the toolchain** — and its
lesson generalises past this bug: **when one entry point takes `&Path` and its sibling takes a
qualified name, the pair is a fork, and a test suite that can only call one of them is
reporting on half a system.** Worth a pass over the other `&Path`-shaped entry points for the
same asymmetry; not done.

## UI review — what is the interface doing that Claude would do better?

**Requested by Doug 2026-08-05**, on adopting Charter **Decision 8, "the instrument assumes the
reasoner"**. A standing review, not a one-off; **the first has not been run.**

**The question, applied to each pane and control:**

> **Is the answer known in advance?**

**Fixed answer, stable question → the UI should hold it**, because a tooltip costs zero seconds
and no focus while asking Claude costs a context switch and a wait. **Answer depends on the
question actually being asked → leave it to Claude**, because building it means guessing the
question. Decision 8 carries the reasoning.

### The first draft of this item asked the wrong question, and it matters

It read *"would Claude answer this better, given the same data?"* — and on that basis listed
**`field_help` for deletion**. Doug, immediately: *"Field help is still very much a good thing.
Many questions such as 'What is this field for?' have answers which will always be known in
advance and can therefore be answered much more quickly with tool tips and such."*

**The crude test ignores latency and focus**, which is most of why UI exists. Corrected the same
day. **Recorded because a review run against the wrong question would have deleted working
features with a charter decision as justification** — the most expensive kind of mistake this
document can cause.

### What the review hunts, after the correction

Not "could Claude do it" but **does it infer**: ranking by importance, recommending what to look
at next, summarising in place of the artifact, filtering by guessed intent. Those depend on the
unasked question.

**Most of the first candidate list survives the sharper test**, which is itself the finding:

- **`field_help`** — **KEEP, not a candidate.** Fixed answers to a stable question. The
  correction above.
- **`generic_error_summary`** — *probably keep.* It formats compiler output; the content varies
  but the presentation is fixed. It becomes a candidate only if it starts *choosing* which parts
  matter.
- **Stage-diff highlighting** — *probably keep.* A diff is a computed fact, not a judgement.
  Ranking diffs by importance would be the verb; showing them is not.
- **The equation sheet's ordering** — *keep if it is the compiler's order*, which is a fact.
  A relevance ranking would be a verb.
- **The labs list** — **the one live candidate.** Showing `kind` and specimen on a row is a fixed
  fact and belongs there. What was dropped is faceted search and prerequisite chains, which infer
  what Doug is looking for. `docs/ideas.md` #62.

**So the expected outcome is NOT mostly deletions**, which the first draft of this item assumed.
It is a small number of removals plus a clearer account of why the rest earns its place — and the
sweep rules permit *closed as obsolete* when a removal is genuinely warranted.

## Verb coverage — the fictions are fixed, the gap that allowed them is not

**Logged 2026-08-04, at the end of the day spent removing them.** Priority: **rank 0** by the
list above, which is a statement about what a *new* fiction would cost, not a claim that this
item must be paid before anything else.

**The measured state.** Every claim HRW makes about *what the compiler did* — which phase ran,
in what order, nested inside what, whether a view came from the compile or from HRW re-running
the algorithm, whether a pane's contents exist at all — is checked today by roughly **a dozen
assertions in `worker.rs`**: bracket balance, trace containment, the no-replay guard, the
attribution count, and the per-condition absence tests added the same day. That is thin
protection for the category that produced **every fiction found**, and it exists only where a
fiction was actually found. **Nothing generalises it.**

**Why the obvious answer is the wrong one.** Extending F1-F9 does not reach this. Those checks
compare HRW's structures to Rumoca's, and a fabricated block *is* a well-formed structure while
a replay's output *is* identical by construction — see the scope table in
[`fidelity-plan.md`](fidelity-plan.md). A verb check has to assert against **what the compile
recorded**, which is what the capture scopes now make possible and did not exist before
2026-08-04.

### Paid 2026-08-04, first pass — the two mechanisms, not the whole category

**1. Re-derivation is now impossible from the UI, by the compiler rather than by grep.**
`MatchingAnimation::from_incidence`, `TarjanAnimation::from_incidence` and
`TearingAnimation::record` are `#[cfg(test)]`. The previous guard was
`doc_citations::no_animation_re_runs_a_phase_by_default` asserting that `app.rs` does not
*contain the string* `from_incidence` — which works, and which a re-export, an alias, a wrapper
or a move to another module defeats silently. **A substring was deciding a safety property in
the test suite of a project whose own rule is that no substring decides identity.** The test
stays for the half a `cfg` cannot state: that the *captured* constructors are reached.

**2. A log bracket must name something that exists — checked at every emit.**
`StageKind::log_name()` plus an argued three-entry `NON_PHASE_BRACKETS` list, asserted in
`make_log` under `debug_assert!`. A test would have checked one specimen; this checks **every
bracket of every compile in Doug's dev build**. Brackets must also *pair by name*, which the
old count-based balance check cannot see: open `Flatten`, close `Typecheck`, and the count is
still zero while the indentation tells the reader one phase contains another.

**It found a real defect within seconds of being added** — the simulate path opened
`"Compile (for simulation)"` and closed `"Compile (…ms)"`. Two names for one bracket, on the
path the log tests never run because they compile and stop.

**Three stale doc comments corrected the same day**, all describing behaviour removed hours
earlier: two `from_captured*` constructors still promised a re-derivation fallback they no
longer have, and `FromWorker::Compiled` still called the pre-lowering and connection frames
*"replay frames … recorded by re-running"*. **The source is a learning artifact here, so it is
held to the same rule as a pane.**

**And `examples/frame_index.rs` was rewritten**, which is the one with teeth: the tool that
generates the frame numbers the matching lab cites was re-deriving from a committed trace
while the panel rendered captured frames. Its header claimed it *"drives the same constructor
the panel does"*, true until the panel changed. It now compiles the specimen and reads
`matching_frames` off the result. **Verified against the lab: 8 / 16 / 114 frames for
BouncingBall / ProportionalLoop / CapacitorLoop, exactly the numbers in `matching.md`** — so
the defect was latent, agreeing by determinism, which is the reasoning the capture scopes exist
to stop relying on. Third defect in that one file.

**3. Panes carry their provenance, and absence is checked as a class.** `Stage` gained a
`Provenance` field — `Empty` / `Compiler` / `Hrw` — set by the constructors rather than by hand.

**The line is drawn at "is this a function of THIS RUN's compiler output?"**, not at "did HRW do
arithmetic". Selecting fields, reshaping into JSON and summarising compiler-produced counts are
all `Compiler`; what makes content `Hrw` is HRW **executing an algorithm the compiler also
runs**, or synthesising a structure it never emitted. That line is crisp because both removed
fictions land unambiguously on the far side of it — and a fuzzy taxonomy would have been filled
in by habit.

`Stage::computed` is the only way to produce `Hrw`, and it **demands a written reason** the UI
shows. It is unused, marked `#[expect(dead_code)]` so that **the scaffolding removes itself**
the moment a caller appears. The friction sits on fabrication rather than on honesty: the
alternative to a supported path is not that nobody derives content, it is somebody calling
`Stage::ok` with synthesised JSON — which is what happened to the BLT tabs.

`no_stage_shows_content_hrw_invented` checks every stage of four specimens, **two of which
fail** (`UnbalancedShaft`, balance −1; `CapacitorLoop`, singular) because a healthy model
populates every pane and never takes the branch the fabrication took. Verified by mutation.

### Still open

**Provenance does not yet detect a fabrication — it makes one *expressible*.** A pane that
calls `Stage::ok` with synthesised JSON still claims `Compiler`, and nothing compares the pane's
values against the compiler's artifact. **That comparison is the verb-check work** (the fidelity
pass Doug approved on 2026-08-04): with provenance recorded, a check can say *"this stage claims
Compiler, so every value in it must appear in the artifact"* — which is a real assertion, where
before there was no claim to test against.

**Also unpinned: the animation and sub-view surfaces.** `IncidenceMatrix`, `ReductionView`,
`Plot` and the four animations carry no provenance. They are all fed from captures or
`from_report` today, and the `#[cfg(test)]` gate stops the re-deriving path — but that is a
statement about *which constructor exists*, not about what a given view is showing.

**The shape worth exploring for those, not yet chosen:**

- **Provenance on the pane, not just in the code.** Every stage view already knows whether it
  came from a capture, an artifact read, or HRW's own derivation. If that were *data* rather
  than an implicit fact about which branch built it, a single test could assert that no pane
  reports derived content without saying so — one check covering the whole class instead of
  one per fiction.
- **A must-fire for absence**, matching the existing one for silence: a pane whose source
  produced nothing must fail a test if it renders content anyway.
- **The cheap interim, already partly true:** every new pane and every new log claim gets its
  own assertion at the time it is written, per the pane-is-a-reporter rule.

**Do not treat this as swept because the fictions are gone.** The fictions were found by Doug
running two labs, not by any check, and the next one will be found the same way unless the
category acquires coverage.

## UI testing debt — the harness exists and almost nothing uses it

**Logged 2026-08-01, at Doug's request**, immediately after the Context Bar defect
above. That defect is this item's evidence, not a separate story: `egui_kittest`
landed in the verification pause on the same day, and the bar it could have
guarded shipped unguarded for weeks.

**The debt is not "few tests". It is that the untested surface is invisible.**
A missing unit test leaves a function that still looks untested. A missing UI
test leaves a *screen that looks fine* — the reader cannot tell a pane that
renders everything from a pane that renders most things, because **what is
omitted leaves no gap where it was**. The Context Bar said three true things and
skipped a fourth; nobody sees a fourth thing missing.

### Measured 2026-08-01

- **11 tests** in `src/ui_tests.rs`, against **17 `*_ui` functions** in `app.rs`.
- **5 of the 11 are lab-link tests.** They exist because labs were the first
  thing with a machine-checkable contract (`fixture_lab_links_all_resolve`), not
  because labs are the highest-risk surface.
- **Panes with no headless test at all:** `menu_bar_ui`, `equation_sheet_ui`,
  `source_map_ui`, `specimen_source_ui`, the stage tab row, the chat panel, the
  help panel, the log view, the Purpose tab, and the status bar's notices.

### What the harness genuinely cannot see — and it is narrower than assumed

`egui_kittest` queries the **accessibility tree**, so anything drawn straight to
a `Painter` is invisible to it. Checked rather than presumed: HRW has ten painter
call sites, and **most are decoration layered under real widgets** — the jump
highlight in `tree.rs`, the selection fills, the stage-diff rule. Those panes are
widget-based and **are** testable.

Only two surfaces paint their content as text and are therefore genuinely out of
reach: **`incidence_view.rs:457`** (the matrix cell glyphs) and
**`spyplot.rs:289`**. The **animations are testable** — their controls, step
labels and state text are ordinary widgets; only the matrix underneath is not.

**This matters for the division of labour.** The standing plan is that fixture
labs cover what cannot be automated, and that set is much smaller than it looked
— which means labs should *not* be spent re-running widget panes a headless
test can hold. Two things stay Doug's alone regardless of harness reach:
**colour** (fills carry meaning throughout HRW and the tree records no colour)
and **layout** (a widget laid out off-screen is still in the tree — the reason
the harness runs at 1600×1200).

### Priority order for paying it down

Not "write tests for all 17". The ordering follows the failure this item came
from:

1. **Panes that report** — the status bar's notices, the log view, the equation
   sheet. Same shape as the Context Bar: they exist to say something, so a
   partial report is both plausible and unnoticeable. **The must-fire rule
   already covers these in principle and has never been applied to a pane.**
2. **Panes whose emptiness is legitimate**, where "nothing here" and "broken"
   look identical: `specimen_source_ui` (which was silently empty for library
   models until 2026-08-01), the Purpose tab, the source map.
3. **Everything reachable by a click that changes state elsewhere** — the stage
   tab row, the mode switch. Cross-pane effects are where a test beats a run,
   since a human checks the pane they clicked in.

**The animations are testable, and none of them are tested** *(added 2026-08-02, from
`ui-findings.md` C6/H7)*. The earlier reading of this entry assumed the animation panes were
out of reach because they sit near painters. Checked: only `incidence_view.rs`'s cell glyphs
and `spyplot.rs` paint their content as text — every animation's **controls, step labels and
state text are ordinary widgets**. Six panes, no tests, and no line in the pause's four steps.
**Not scheduled**, deliberately: they are not on the refactor's path, so they are debt rather
than blocking work.

**~~A third surface the harness cannot reach: scroll-area configuration~~ — CORRECTED
2026-08-12.** This entry claimed *"nothing observable differs"* between
`ScrollArea::both()` and `ScrollArea::vertical()`. **That is false, and a real defect hid
behind it for eight days.** The correction is kept in full because the mistake is more
instructive than the fact.

**What differs is the SIZE OF THE ENCLOSING PANEL.** A vertical-only scroll area reports
its content's full width as the width it *wants*, and that propagates outward: with the
real `the-concepts.md` loaded, the lab panel opened at **899pt of a 1280pt window
instead of 512pt, and the divider froze solid** — the panel had been sized to the widest
table in the document. Under `both()` the same document opens at 512pt and drags freely.
Both numbers are read from `App`, and
`ui_tests::the_left_panel_content_never_detaches_from_the_divider` fails by name when the
axis is reverted.

**The original measurements were all correct.** They were just all taken *inside* the
scroll area:

- The rendered row's a11y `rect()` is **logical** and reports full width whether or not
  the pane can scroll to it. — still true.
- `content_size.x` already exceeds the viewport under *both* settings. — still true.
- Forcing `TextWrapMode::Wrap` changes nothing measurable under `both()`. — still true.

**So the lesson is not "measure harder", it is WHERE to measure.** Every probe asked
*"does the content look different?"* and none asked *"does the container?"* A scroll
axis is a claim about how a widget negotiates size with its parent, and the parent is
the one place nobody looked. The entry then generalised three specific null results into
*"configuration rather than behaviour"*, and that sentence is what made the surface
un-investigated rather than merely untested.

**The three deleted tests were still rightly deleted** — each passed on unfixed code,
and *"a test that can pass while checking nothing is worse than none"* stands. Deleting
them was correct; concluding the surface was unreachable was not.

**And the prescription this entry gave was wrong too.** It said the way in would be to
*"drive a horizontal scroll and observe the offset move"*, which `egui_kittest` still
cannot do ergonomically. The way in was to measure the parent panel's width — available
the whole time, needing no harness feature, and reached only because a **user-reported
symptom named the container**: *"the divider does not move."*

**Do not chase a coverage number.** The metric that would matter is *panes whose
reports are guarded*, and counting tests instead would reward the lab-link tests
that already dominate the file.

### The rule going forward

**A new pane that reports something ships with a headless test, the way a new
reporter ships with a must-fire test.** Retrofitting the existing 17 is the debt;
not growing it is free. The Context Bar is the worked example — two guards, both
of which fail against the code as it stood that morning.

## Nothing cross-checks a stage's summary against the frames from the same run

**Logged 2026-08-17**, after `index-reduction.md` spent its whole existence teaching the opposite
of what the compiler did. Full account in `DECISIONS.md`, same date.

**The Index Reduction stage reported `differentiated_rows: []` while the frames captured from that
same compile recorded four `IndexReductionStep::Differentiated` events.** Both are true. The
summary scans the *final* DAE for surviving origin markers; the frames record what happened along
the way; and step 10 (`eliminate_trivial`) removes 77 equations, taking the differentiated rows
with them.

**Every checker in this repository compares a document to a trace, and the trace said zero.** So
the lab was consistent with the artefact and wrong about the compiler, and no amount of
document-versus-trace checking could have found it.

### The check that would have

For each stage that publishes both a **summary** and **frames**, assert they tell the same story
about the same run. On `Drivetrain`: `differentiated_rows.len()` versus the count of
`Differentiated` frames. They will legitimately differ — that is the whole finding — so the
assertion is **not** equality but *"if the frames record N > 0 events of a kind, the summary must
not describe zero of that kind without saying why."*

**Cheaper first move: rename or document the field.** `differentiated_rows` means *rows still
carrying a differentiation marker at the end*. A name like `differentiated_rows_retained`, or a
one-line note in the pane, removes the trap for a fraction of the cost. The checker is the durable
fix; the rename is what stops the next reader falling in.

### Why it is worth building rather than noting

**This is trigger 2 — a defect only a human caught** — and the human did not even catch *it*. Doug
reported that the lab *felt thin*. The wrong number was three steps down from that. **A defect
found by aesthetic dissatisfaction is one nothing else was looking for**, which is exactly the
signal this file says should fire a sweep.

## ~~The Index Reduction tab is a re-run, and nothing on screen says so~~

**RESOLVED 2026-11-03 by the v0.10.0 rebase, and by removal rather than by either planned fix.**
HRW's mirror funnel is gone: `index_reduce_for_structural_analysis` no longer exists, no stage
carries a reduced DAE, and the pane is listed in `playback::DISABLED_VIEWS` behind a placeholder
that says it has nothing to show. **Nothing re-executes the pass and nothing claims to observe
it**, so there is no longer a label to owe.

**The reason is not that fix 1 was chosen: v0.10.0 deleted the steps.** `dae_prepare/` became
`dae_transform/` and the functions HRW called by hand are absent under any name, so the mirror
could not have survived. **The ordering risk the entry was really about is therefore gone at the
root** — there is no HRW-maintained order left to drift.

**Fix 2 is not an ask at all any more — upstream already did it.**
`crates/rumoca-phase-structural/src/dae_transform/observation.rs` carries the observation hook
this entry asked for, and **`inspect_prepare_for_solve` is the public entry**: it returns
`(Result<PreparedDae>, ReductionReport)` with the observer type erased, and `ReductionReport` is
fully public — `records` of `Round` / `Candidates` / `Attempt` / `Selected` / `RetriedPristine`
carrying lanes, residues and named unmatched coordinates, plus a `ReductionSnapshot` of the
discarded still-singular DAE when preparation fails. That is *more* than this entry asked for: it
reports the attempts, not only the outcome.

*Corrected 2026-11-03, having been written wrong the same day.* The first version of this
paragraph said the seam was `pub(super)` and needed widening. The **trait and the borrowed
event** are private, which is the design — `inspect_sort` is identical — and reading that as "the
pass cannot be observed" confused the seam with its consumer surface. **A wrong negative is the
error nobody catches, because acting on it means not looking**, and this one would have sent an
unnecessary request upstream and parked the re-model behind it.

**The sharpest part: this file already had it right.** The *Index reduction* re-model row above
names `inspect_prepare_for_solve` as the replacement and describes its report accurately — it was
written during the rebase, from the same source. The wrong claim came from re-reading
`observation.rs`, seeing `pub(super)` on the trait, and preferring that fresh read to a correct
record. **A grep for a visibility marker is not a search for the public entry**, and the record
written by someone who had looked for the entry was the better evidence.

So the reduction re-model is HRW-side work, blocked on nothing, and tracked as the reduction
`unbuilt:` row.

*The original entry follows, unedited — including the doc comment it quotes, whose path named
0.9.20.*

<!-- historical: rumoca-sim/src/solve_lowering/structural_lowering.rs -->

**Logged 2026-08-17**, found while investigating the above.

`worker.rs` clones the compiler's DAE and runs **HRW's own funnel** over it —
`index_reduce_for_structural_analysis`, which calls Rumoca's `dae_prepare` steps **in an order HRW
maintains**. Its doc comment says so plainly: *"HRW mirrors Rumoca's funnel order… re-verify it
against `rumoca-sim/src/solve_lowering/structural_lowering.rs` on a Rumoca pin bump."*

**This is not a fiction, and the distinction matters.** The steps are Rumoca's, the data is
Rumoca's compiled DAE, and the results are real. `CLAUDE.md` permits exactly this: *"re-running a
phase to observe it is sometimes the only way to see inside it, and that is legitimate **when
labelled**."*

**It is not labelled.** `reduction_view.rs` does not say it, and `index-reduction.md` does not say
it. So:

- A reader takes the tab for an observation of what the compiler did. It is a faithful
  re-execution, which is *nearly* the same thing and not the same thing.
- **A reordering upstream leaves the tab looking identical and being wrong.** The doc comment
  names this risk and `docs/updating-rumoca.md` step 3 exists because of it — a rebase check, which
  is a human remembering, not a test.

### Two fixes, and they are not alternatives

1. **Label it now** — one line in the pane and one in the lab. Cheap, honest, and it satisfies the
   rule as written.
2. **Ask upstream for the real thing.** rumoca-sim runs this funnel during compilation and HRW
   cannot see it, which is why the mirror exists at all. An observation hook there would make the
   tab an observation rather than a re-execution, and delete the ordering risk outright. Filed in
   `upstream-issues.md`; it is the kind of additive observability change `CLAUDE.md` says to make
   rather than approximate around.

## `CATALOGUE.md` should carry each stop's `Expected` line

**Logged 2026-08-17 at Doug's request**, immediately after `hrw://lab/<name>/station/<slug>` was
made to actually land at its stop. Doug: *"Your proposal would eliminate a cause for bugs."*

### The bug class it removes

The catalogue lists every stop's **heading and slug** — 169 rows — and nothing about what the
stop *claims*. So citing one accurately means opening the lab, reading the stop, and writing a
summary beside the link. **That summary is written from recollection of something read minutes
earlier, which is a mechanism this repository has already watched fail:**

> On 2026-08-16 a new stop of `connect-expansion.md` was written claiming `C.v` reads
> `der in f_x[19]` — a real, existing equation, and the wrong one: `f_x[19]` is the connection
> equation `src.p.v = R.p.v`, while the capacitor's rate law is `f_x[14]`. **The number was
> written from memory of one seen an hour earlier.**

That one was caught by building `an_equation_id_a_lab_cites_names_the_equation_the_prose_claims`.
**The general case is not covered**: an equation *id* is checkable against a trace, and a prose
claim about what a stop says is not.

**Carrying the `Expected` line into the catalogue removes the recollection step entirely** —
the claim is quoted from the index rather than remembered from a reading. And because the
catalogue is **generated**, the quote cannot drift: `lab_catalogue_is_current` already fails
when the file no longer matches the labs.

### Why the cost of not doing it is now rising

**Before 2026-08-17 a stop link did not work**, so the corpus contained exactly one and citation
was rare. It works now, which makes composing an answer out of checked stops the natural move
rather than an unusual one — see `.hrw-bridge/answer.md`'s four-citation example. **The debt's cost
scales with how often the feature is used, and the feature just became usable.**

### What it does NOT close, and this needs saying

**An Answer's summary is still unchecked, and will remain so.** `.hrw-bridge/answer.md` is
gitignored by construction — it is the answer to the question just asked — so no test runs on it.
The asymmetry to keep in mind:

| | today | with this item |
|---|---|---|
| a **dead** slug | loud — *"has no stop `x` — opened at the top"* | unchanged |
| a **wrong** paraphrase | **silent** | less likely, still silent |

This makes the mistake harder to make. It does not make it detectable. **The verification stays
with Doug**, which is why the example lab tells him to check that each stop says what the
citation claims.

### How to do it

`examples/gen_lab_catalogue.rs` already walks every heading to build the slug list. For each
numbered stop, take the first `**Expected:**` line of its body — `every_station_of_every_lab_owes_an_expected`
guarantees one exists — and emit it under the row.

**Two judgement calls to make with measurement, not guesswork:** the file is **21 KB** today and
some `Expected` lines run to several sentences with tables under them, so decide whether to quote
the first line or the whole block **after seeing the size**; and the catalogue's audience is
Claude, so legibility to a human is not a constraint — but its own generator test compares whole
file contents, which means a long quote makes every diff noisier. **If it doubles the file,
quote the first sentence.**

## Remove the split reporting once the LHS width has proven itself

**Logged 2026-08-03 at Doug's request**, immediately after the width was confirmed working.

`SplitState::observe` writes up to six lines per session to the diagnostics file recording the
available width, the panel width and the resulting fraction. It exists because **five attempts
at the opening width failed and the sixth succeeded the moment somebody looked at the numbers**
(`ui-findings.md` C15).

**It is committed, which under `CLAUDE.md`'s probe rule makes it a decision rather than a
drift.** The case for keeping it: it is capped, it writes only to the diagnostics file, it costs
nothing per frame, and if the width ever misbehaves again it turns another five-attempt loop
into one restart. The case for removing it: it was added to diagnose a bug that is now fixed,
and instrumentation that outlives its question is how a codebase accretes noise nobody dares
delete.

**The trigger is confidence, not a date.** Remove it when the width has survived a stretch of
ordinary use — different window sizes, a maximise, a restart or two — without a surprise. If it
is still there and nothing has gone wrong for weeks, that is the answer.

### The trigger fired, and it fired the other way — 2026-08-16

**It named "a maximise" specifically, and a maximise produced a surprise.** Doug normalised the
window and maximised it; the divider jumped to 75 %. The two recorded lines named the cause
outright — `panel 200px, available 267px`, a window narrow enough that the legal range collapses
to a point — and the fix took **one reading**, against five wrong theories for the 2026-08-03
version of the same class.

**And the instrument was very nearly not there.** `reports_left` capped it at six observations,
a budget sized for diagnosing startup and spent by startup, with the check sitting *above* the
recording despite the comment beneath saying *"always to the diagnostics file"*. It was switched
off before the bug it was needed for. Fixed the same day: recording is unconditional, only the
log-view message is rationed.

**So this item is now KEEP, and the reasoning has inverted.** It has paid for itself twice, on
two different mechanisms, and the second time it was the difference between one reading and
another five-theory loop. Deleting it would be removing the only thing that has ever resolved a
bug in this area quickly.

**Reopen only if** the split survives a long stretch including maximise, restore, restart and a
monitor change with no surprise — which is a higher bar than the original, deliberately, because
the evidence moved.

**If it is removed, remove the whole path**: `reports_left`, `log_split`, and the
`record_action("split", ..)` call. Leaving a disabled reporter behind is worse than either
choice, because the next reader cannot tell whether it is off on purpose.

## `cargo fmt --all -- --check` fails — `hrw/` only, as of 2026-08-05

**Logged 2026-08-03 at Doug's request**, after `cargo fmt` was ruled out mid-change as too
disruptive to run in passing.

> **The title said "`hrw/`'s fault alone" and that was false when written.** Measured
> 2026-08-05: the instrumented Rumoca crates carried **82 unformatted hunks** —
> `rumoca-phase-structural` 49, `-flatten` 16, `-dae` 13, `-compile` 4 — and they were **ours**,
> confirmed by restoring `rumoca-phase-structural/src/lib.rs` from upstream `8cdc7419` and
> re-checking: upstream clean, our copy 5 diffs.
>
> **Fixed the same day**, so the title is true now. It is left in place, corrected rather than
> rewritten, because the interesting part is *why nobody noticed*: `CLAUDE.md`'s rule for
> touching a Rumoca crate named **clippy and not rustfmt**, and clippy was run every time. **A
> rule that names one of two gates reads as complete.** The rule now names both, and says to run
> `fmt` first — formatting rewrapped one traced function from 99 lines to 102, over
> `too_many_lines`, so clippy-then-fmt certifies code in a shape it will not ship in.

### This is a live CI failure, not a tidiness preference

Three facts, each measured rather than assumed:

- **Upstream CI runs it.** `.github/workflows/ci.yml:86` — `cargo fmt --all -- --check`.
- **`hrw` is a workspace member.** `Cargo.toml:62`. So `--all` includes it.
- **The `crates/` are already clean and `hrw/` is not.** Running the exact CI command under the
  pinned toolchain (`rust-toolchain.toml`, `nightly-2026-02-27`) reports **zero** hunks under
  `crates/` and roughly **900** under `hrw/`.

So the fmt job is red on this branch, and **`hrw/` is the entire reason.** That was not known
when the entry was requested — the assumption was that formatting was merely unenforced.

Worst offenders: `worker.rs` (276 hunks), `app.rs` (231), `bridge.rs` (56), `fidelity.rs` (42),
`tree.rs` (32), `ui_tests.rs` (30). Long tail across most other modules.

### What this does and does not block

**It does not block upstreaming.** An upstream PR is a cherry-pick of `crates/rumoca-*` changes
only (`CLAUDE.md`, the separable-commits rule), and those are clean. The instrumentation
commits would pass CI on arrival.

**It does block having a trustworthy green build**, which `docs/upstream-strategy.md` stakes
Doug's credibility on: work that is *reproducible and honestly bounded*. A maintainer glancing
at the fork sees a failing check and cannot tell that it is confined to a directory their PR
will never contain.

### Why it is not fixed in passing

**`cargo fmt` would touch ~900 sites across most of `hrw/`**, which buries whatever change it
rides along with. This was hit for real on 2026-08-03: wrapping the playback bar in a frame
left one closure body under-indented, and the correct-looking fix — run `fmt` — would have made
a four-line change unreviewable. It was re-indented by hand over a fixed line range instead.

### How to do it

**One commit, containing nothing else.** The whole value is that it is mechanically verifiable:
a reviewer confirms `cargo fmt --all` was run and the tests still pass, and reads none of it.

1. Run the full suite first and record the count, so "the same tests pass" is checkable.
2. `cargo fmt --all`, then `cargo clippy -p hrw --all-targets` (**check the exit code**, per
   `CLAUDE.md` — a compile error survives a warning-count grep).
3. Full suite again, `--features slow-tests`. Same count, no failures.
4. Commit with a subject that says it is formatting only, and push nothing else with it.

**Two things to watch.** `doc_citations` asserts on source structure — the field-count ratchet
parses `pub struct App` by line shape, and the two-`#[test]` check scans attributes; both should
survive reformatting, but they are the tests most likely to be sensitive to it, so run them
first. And the `#[rustfmt::skip]` escape hatch exists for any table-shaped literal whose
alignment carries meaning: **prefer skipping a block to accepting a formatting that makes data
harder to read**, since the source is a learning artifact here.

### Then keep it clean

Once green, formatting becomes a pre-commit step rather than a project. Until then, **do not run
`cargo fmt` as part of unrelated work** — hand-indent the block and note it, as
`f3c6fb90` did.

---

## A3 — the duplicate resolve: measured, solved, and reverted at the last step

**Status 2026-08-04: not fixed. The fix is known and proven to work; one error path
defeats it.** Recorded here rather than left in a branch, because the measurement was
the expensive part and it should not need repeating.

### The problem, measured

Every HRW compile resolves the whole library **twice** — confirmed, not inferred: a
traced compile emits two `resolve timing summary` lines, `def_count=38855` and
`class_count=6521` both times.

`Session::tree()` builds under `ResolveBuildMode::Standard`. A strict compile builds
under `StrictCompileRecovery`. **Both are cached, but separately**, so inspecting the
tree and then compiling resolves twice. Nothing is wrong with either — HRW simply had
no way to ask for the tree the compile was about to build.

### The fix, and that it works

Add `Session::strict_compile_resolved()` — additive, public, returns the `Arc` — and
have HRW call it instead of `tree()`. The compile then finds its tree cached.
Measured with the change in place:

| | Resolve | Whole compile |
|---|---|---|
| before | 883 ms | ~2000 ms |
| after the clone fix (`tree()`, landed) | 740 ms | 1815 ms |
| **after this** | **596 ms** | **1102 ms** |

`resolve timing summary` count went **2 → 1**. The compile roughly halves.

### Why it was reverted

**Recovery mode succeeds past errors that `Standard` treats as fatal** — that is what
recovery is for. So `Ok` no longer implies the model resolved cleanly, and HRW's
Resolve tab stopped reporting resolve failures. Two tests caught it:
`a_broken_specimen_does_not_poison_the_next_compile` and
`a_resolve_failure_names_the_reference_and_its_line` (*"a resolve failure must carry a
structured payload"*).

That is the tests working. The old code used `tree()`'s `Err` as its failure signal,
and the replacement has no `Err` to use.

### What finishing it needs

The diagnostics are available — `build_resolved_for_strict_compile_with_diagnostics`
returns them alongside the tree, and the accessor was already amended to pass them
through. The remaining work is **deciding which diagnostics mean "this model failed to
resolve"**: the set includes library-wide diagnostics that must not fail a good model,
and the existing failure arm gets its structured payload from a *model-scoped* call
(`compile_model_diagnostics`) rather than from the error value.

So: on `Ok`, filter to the model's own errors and route to the existing failure arm
when any exist. **Not attempted** — the semantics of model-scoped versus library-wide
diagnostics were not verified, and guessing them would produce a Resolve tab that is
silent on real failures, which is worse than the duplicate resolve.

---

## Replays: eliminated (2026-08-04)

**Closed.** Every recorded view is now built from frames captured during the compile
that produced the IR on screen. `no_animation_re_runs_a_phase_by_default` keeps it
that way: it fails if `app.rs` ever names a re-deriving constructor again.

Two of the three items this section previously listed as outstanding **needed no work
at all** — verified, not assumed:

- `reduction_anim` already took `from_frames(Vec<IndexReductionFrame>)`, fed by
  `index_reduction_stage`'s real run. The `tarjan` mention was a doc comment.
- `ic_plan_anim`'s only constructor is `from_report`; its `build_ic_plan` call is
  inside a test.

**That is the second time in one day that recorded outstanding work was already
done** (the first was CapacitorLoop's stale trace). The cost of checking is one grep;
the cost of not checking is planning an arc around imaginary work.

### The fallbacks are gone: absence is stated, never filled in

Doug, 2026-08-04, on the singular case: *"it would be helpful if the parts of the UI
which depend upon the BLT blocks made clear that no BLT blocks are available because
no attempt was made by the compiler to create those BLT blocks."*

**He was right, and it reversed the argument for keeping the fallbacks.** Measured on
`CapacitorLoop`: the compiler matches 13 of 14 equations, declares the system singular
and returns **before** `build_blt_blocks` — and the Tarjan tab then built its own
matching and BLT and drew a **non-empty SCC decomposition of blocks that were never
created**.

That is a fiction in the same sense as the "DAE pipeline" log entry removed the same
day, and worse: the log was mislabelled, this was fabricated. I had argued the
fallbacks were *"the only source"* for singular models — being the only source was the
problem.

**The rule now, across all four conditions:** when a capture is absent, say *why* —
never re-derive.

| condition | what happens |
|---|---|
| singular system | *"No BLT block decomposition to show — structural analysis stopped before this step"*, plus the compiler's own message |
| empty system | *"No … was recorded for this model"* |
| size mismatch | refused; it is a bug, not a fallback |
| stage not reached | already correct — the pane says nothing because there is nothing |

The absence is the more useful thing to know. It teaches the chain's contract: BLT
decomposition and tearing are **entitled** to a matched system, and a phase that
refuses when it has not got one is doing its job.

`a_singular_system_captures_matching_but_no_blocks` pins the compiler's half —
matching runs, Tarjan and tearing do not — so a future change that continued past a
singular matching would make the panes' explanation stale and fail here.
`no_animation_re_runs_a_phase_by_default` forbids `from_incidence` *and*
`TearingAnimation::record` in `app.rs`.

**What survives:** `from_incidence` (used by `examples/frame_index`), `record` and
`start_live`. None is reachable from a recorded view.

### The pattern, applied six times

`rumoca-phase-flatten` (connections), `rumoca-phase-dae` (pre-lowering),
`rumoca-phase-structural` (matching, Tarjan, tearing), `rumoca-compile` (the typed
overlays and, attempted, the resolved tree). Two shapes, chosen by whether the
untraced entry point already routes through the emit site:

- **Branch at the call site** when there are two implementations (matching, Tarjan).
- **Hook the emit site** when the untraced entry *is* the traced one with `None`
  (connections, pre-lowering, tearing) — no call-site change at all.

Both cost one thread-local read when closed, and neither moves a signature. **The
sweep never opens a scope**, so a 2,626-model run pays nothing.


## Source provenance thins through the pipeline (and the first measurement of it was wrong) — 41 → 15 → 2 → 0

**Found 2026-08-05**, when Doug resumed the DAE lab: *"I'm not able to relate anything in the DAE
tree to the Modelica source from which the tree was derived."* He reported it as a UI problem
constrained by screen real estate. **It is not primarily a UI problem.**

Measured on `SingleInertia`'s committed trace, counting `location` entries per stage:

| Stage | locations |
|---|---|
| Parse | **41** |
| Instantiate | 15 |
| Flatten | **2** |
| DAE | **0** |

The DAE's equations carry `"origin": "source"` — a *category*, distinguishing a source equation
from a `connect` expansion or an index-reduction artifact — but **no line, no file**. So no pane
can show what the data does not carry, and screen real estate was never the binding constraint.

### The half that was fixable in HRW, and is done

**A variable's declaration is recoverable by name.** HRW's identifier index is built from Parse,
where all 41 locations survive, so `w` in the DAE tree resolves to its declaration line without
any compiler change. That is a **fixed fact known in advance**, so Charter Decision 8 puts it on
screen and Decision 9 makes it a tooltip — which costs no real estate at all.

Shipped as `TreeOptions::variable_lines`, rendered by `row_hover`, stated **before** the
interaction hints because where a thing came from precedes what you can do with it.

### ~~The half that needs Rumoca~~ — WRONG, corrected 2026-08-05 the same day

**This section claimed equations had no recoverable origin and needed a Rumoca change. That was
false, and it was false when written.**

**DAE equations carry a `span`** — `{ source, start, end }`, 38 of them in `SingleInertia`'s
trace — and `equation_sheet.rs` has been resolving spans to 1-based source lines since it was
written (`byte_offset_to_line`, plus origin-matching for `connect`-generated equations that have
no direct span).

**The error was in the measurement, not the reasoning.** The count above searched the serialized
JSON for `"location"` and found zero, and the conclusion "the DAE carries no source locations"
was drawn from that. The field is called **`span`**. Counting one spelling and concluding a
capability is absent is the same shape as the `str_vec` blind spot: **an audit by grep finds a
thing only where it is spelled the way you guessed.**

**The variable half of that section stands** — the identifier index really is how a *variable*
resolves, and the tooltip built on it is correct. What is wrong is only the claim about
equations, and the recommendation to change Rumoca that followed from it.

**So the equation feature was HRW-side and cheap — and is BUILT, 2026-08-05.**
`EquationSheet::node_lines` maps a tree-node path to the source line, built from the same
`source_lines` the sheet already resolved; `TreeOptions::path_lines` carries it; the tree looks
up the path it is already holding.

**Keyed by path, because the tree is type-agnostic by charter §4.4** and must not learn that
`f_x[3]` is an equation. Both sides build the key with `describe_path`, so the formats agree by
construction rather than by being kept in step — and `an_equation_node_path_resolves_to_its_source_line`
pins that, because a key that never matches produces **no tooltip and no error**.

**Only the DAE stage supplies the map**, which is also what keeps the cost down: every other
tree pays a null check rather than building a path string per row per frame.

The same three affordances as variables, wording adjusted: a variable is *declared*, an equation
is *written*, so the menu item drops the name it does not have.

**Doug's sequencing:** variables first, test, then equations.

## The `worker.rs` Stage-construction sweep — 2026-08-25, and its two NULLS

**Method: a column read of all 58 `Stage` construction sites**, audited against the two rules
established that day — *absence is stated, never filled*, and *`Outcome::Failed` means "the pipeline
stopped here", so at most one stage per compile may carry it*.

| constructor | sites | | constructor | sites |
|---|---:|---|---|---:|
| `Stage::info` | 23 | | `Stage::err_with_details` | 6 |
| `Stage::recovered` | 9 | | `Stage::err` | 5 |
| `Stage::default` | 7 | | `Stage::ok_with_note` | 3 |
| `Stage::ok` | 6 | | | |

**Three live defects were found and fixed** (the blank DAE tab, the blank Instantiate/Typecheck
tabs, and the dead `Stage::default()` reset). **Two candidates died on measurement, and that is
what this section is for** — both looked compelling, and re-deriving them would cost a session.

**NULL 1 — "HRW's serialize failure is reported as the compiler's" is near-dead code.** Three sites
(`Stage::from_ser`, `flatten_stage`, `solve_lowering`) return `Stage::err` when
`serde_json::to_value` fails on a *successful* compile, which does assert the pipeline stopped at a
stage that completed. **But the plausible trigger cannot fire.** `to_value`'s `serialize_f64` is
`Ok(Value::from(float))` — it does not error on non-finite input. Reaching these arms needs a
`Serialize` impl that itself errors, which the IR types do not have. Left as written.

**NULL 2 — non-finite floats silently becoming `null` is unreachable.** `From<f64> for Value` maps
infinities and NaN to `Value::Null`, so such a value would vanish from a pane with no error at all —
a fidelity defect of the omission kind, and invisible to every checker, since `null` is well-formed
JSON that round-trips. **No model in reach produces one.** The specimen corpus contains no `inf` or
`nan`, and the MSL's own infinity is finite by design:

```text
ModelicaServices 4.1.0/package.mo:184
    final constant Real inf = 1.7976931348623157E+308
      "Maximum representable finite floating-point number";
```

It would take a literal overflow (`1e400`) or constant folding of `1/0`. **The prediction that
started this was wrong in both directions** — the reachable-looking defect was dead, and the fatal
one was unreachable.

**What the sweep did NOT cover, stated so the null is not read too widely:** only `Stage`
construction. The log and bracket machinery, the source-root memo, `OutputCapture` and the simulate
path were not audited. **Simulation trajectories were the live version of NULL 2's question, and the answer
came back positive on 2026-08-25.** They carry computed `f64`s, they travel through `SimData`
rather than `Stage` JSON, and a three-equation model proved a successful run can carry an infinity.
`SimData::non_finite_series` and the pane warning above the plot are the result — so unlike NULL 2,
this one was reachable and is now reported.

### ~~The Index Reduction tab offers five sub-views that draw nothing~~

**Recovered 2026-11-04, the same day it was found.** Doug: *"I want to recover index reduction tabs
which are recoverable. And, I don't want to conflate or otherwise recover badly."*

**What landed, and what it refused to do.** The cause was not the payload: `incidence_pane_ui`
took one report, dug the Before matrix out of `report["before"]`, and parsed After from the
**root** — an unstated convention that was true on 0.9.20 and false the moment the re-modelled
stage published the two as siblings. Both reports are parameters now, named by
`App::matrix_reports`, and **nothing is merged**. Hoisting the after-report to the top level would
have fixed these panes and, in the same move, handed the three frame-driven animations a matrix to
pair the **raw** system's frames with — the bad recovery, and the one the shortest diff would have
produced.

So:

- **`Incidence` and `SpyPlot` read `after`** on that stage, with the Before pane reading `before`.
  The reduced system's matrix is back, with its matching marked, beside the raw one.
- **`MatchingAnim`, `TarjanAnim` and `TearingAnim` are refused on that stage**, stated rather than
  starved. Their frames can only be the raw system's, because `inspect_prepare_for_solve` records
  the reduction's rounds and not the matching passes inside each rebuild.
- **`Incidence`/`SpyPlot` require an admitted reduction.** A refused one publishes no `after`, and
  showing `before` instead would let the pane's subject vary between the reduced and the raw system
  with nothing saying which — the raw matrix is one tab over on Structural. Found by the new check
  below on its first run, as `CartesianPendulum / IndexReduction / Incidence`.
- **`ir_split` keys on both reports existing**, not on the note containing `"singular"` — which
  with the re-modelled notes was true only for the failures, so the split was off for exactly the
  successful cases it exists for.

**The durable part is the check, not the fix:**
`app::tests::every_offered_sub_view_has_something_to_draw` asserts that every sub-view the app
*offers* is one a pane can fill. That is the complement of
`every_lab_sub_view_link_is_available_for_its_specimen`, which only ever asked whether a *link*
names an offered tab — and the missing direction is how all five hid. It is **not** a claim about
pixels; `egui_kittest` cannot reach a painted cell, so a tab whose data parses can still look
wrong, and only Doug can see that.

**One thing the fixtures were quietly wrong about.** `matrix_panes`'s split fixture uses
deliberately different sizes so a swap is visible — and measured on real specimens, `Drivetrain` is
97×97 on both sides, `CapacitorLoop` 14×14, `BenchActuator` 48×48. Reduction moves unknowns between
the integrated and algebraic sets without changing the matrix's dimensions, so **a swap is
invisible by size on every real specimen**; what distinguishes the halves is rank.
`drivetrain_before_report_parseable_as_incidence` is what pins that, and the fixture's doc now says
it is a device rather than a likeness.

*The original account follows.*



**Found 2026-11-04 by Doug asking what the rebase had cost**, after two defects in the same
area had already been found by the same question. Nothing on screen is *wrong*; five of the
eight tabs are simply empty, and three of them cannot be filled.

`structural_view_available_from_stage` approves every sub-view on the Index Reduction stage
(`is_index_reduction` short-circuits the singular guards). Measured against what each pane can
actually read from the re-modelled payload:

| sub-view | state on that tab | why |
|---|---|---|
| `Summary` | **works** | the re-modelled panel |
| `Animate` | **works** | the stepper — and it was unavailable until this was found |
| `Tree` | **works** | the generic IR tree |
| `Incidence` | empty | `IncidenceMatrix::from_report` wants a **top-level** `incidence`; the payload has `before.incidence` and `after.incidence` |
| `SpyPlot` | empty | same, for `blocks` |
| `MatchingAnim` | empty | same, and see below |
| `TarjanAnim` | empty | same, and see below |
| `TearingAnim` | empty | same, and see below |

**The two halves have different answers, and conflating them is how this gets fixed badly.**

**`Incidence` and `SpyPlot` are recoverable and should be.** They are static renderings of a
report with no frames involved, so pointing them at `after` (and the split's Before pane at
`before`) restores a genuinely useful view: the reduced system's matrix beside the raw one. That
is the before/after split the 0.9.20 lab referred to, and `drivetrain_before_report_parseable_as_incidence`
already asserts both halves parse.

**The three animations are not recoverable, and should stop being offered.** They need the
reduced system's *search steps*, and `inspect_prepare_for_solve` records the reduction's rounds
rather than the matching passes inside each rebuild — so the reduced system's report is
observable and its search is not. `app::structural_frames_for_stage` can only hand them the
**raw** system's frames. They draw nothing today purely because they cannot find a top-level
`incidence`, which is **protection by accident of payload shape**: anyone who fixes the Incidence
pane by hoisting the after-report to the top level would silently pair raw frames with reduced
rows, and `TearingAnimation::from_captured`'s guard would not catch it (it compares
`blocks.len()` against `segments.len()` and ordinals that run `0..n` in both systems).

**And `ir_split` keys on the wrong thing.** It turns the Before pane on when the stage note
contains `"singular"` — which, with the re-modelled notes, is true only for reductions that
**failed**, and those publish no `after` to compare against. For the successful reductions, where
before/after is the whole point, the split is off. It should key on whether an `after` exists.

**Why no test caught any of it.** `every_lab_sub_view_link_is_available_for_its_specimen` skipped
`Animate` as frame-dependent and its doc claimed a count would say so — the count appears only
inside another assertion's failure message. It now builds an `App` from the committed stage and
asks `structural_view_available`, so it catches a tab the app refuses. **Nothing yet checks the
complement: a tab the app OFFERS and the pane cannot fill.** That check is what this row really
wants, and it is the one that would have found all five at once.

### 11-04, the two-phase reduction — three panes, three numbers, all correct

**v0.10.4 reduces in two places, and HRW reports them as if they were one.** `prepare_for_solve`
demotes what its lanes can reach; `rumoca-phase-solve`'s state selection then picks a chart on
what remains. So a reader of `Drivetrain` sees:

| pane | says | about |
|---|---|---|
| Index Reduction | `sorted after 6 round(s): 9 state(s) → 5` | the reduced *system* |
| Solve Lowering | `state_scalar_count: 3` | the integrated *chart* |
| Simulation | 3 states | the same chart |

**Every number is right and nothing on screen says they count different things.** `CartesianPendulum`
is the acute case — Index Reduction reported a refusal while the model simulated. `Drivetrain` is
the quiet case: no contradiction loud enough to notice, just two counts a reader will assume are the
same quantity and subtract.

> **⟶ CORRECTION, 11-05. This paragraph said the acute case "was fixed on the day by
> `report_sub_view::reduction_banner`", and Doug falsified it by loading the specimen: the banner
> was green and the TAB WAS STILL RED.** The fix had covered the pane and not the stage outcome the
> pane sits under, which is a second expression of the same fact — exactly the hazard the banner's
> own doc comment warns about, written while the second site went untouched. **A fix verified at one
> of a fact's two homes is not a fix**, and the claim of completion here is what stopped anyone
> looking for a day. Both now derive from the one `state_scalar_count`:
> `worker::reconcile_reduction_with_lowering` sets the outcome, and `stage_tabs::tab_color` paints
> it. Corpus effect, measured: exactly one row of the outcome matrix moved.

**Not fixed, and deliberately not papered over.** The honest fix is for the Index Reduction pane to
name what it is counting and to say when a chart follows — which needs the selection to be
*observable*, and it is not: `state_selection` is a private module, exactly as `prepare_for_solve`
was before `watch_prepare_for_solve`. <!-- unbuilt: rumoca_phase_solve::watch_state_selection -->

**Found by re-testing an upstream entry, not by a checker**, which is the "who caught it?" trigger
pointing at a gap: nothing compares two panes' numbers against each other, and the three defects of
this class found on 11-04 were each caught by a human or a photograph.

| date | defect | caught by | note |
|---|---|---|---|
| 11-04 | the Index Reduction banner called a reduced model **Singular**, in red | Claude, reading the regenerated trace | `inspect_prepare_for_solve` refuses `CartesianPendulum` while the compiler lowers and simulates it on 2 coordinates. True about the entry point, false about the compiler |
| 11-04 | `x` and `y` reported as **states** when both are reconstructed algebraics | chasing the banner into the data | `is_state` was `i < n_states`, a positional rule. v0.10.4 lowers one `State` declaration — a 2-vector chart — and `SimVariableMeta::is_state` was being dropped on the floor |
| 11-04 | `Drivetrain`'s three panes report 5, 3 and 3 states with nothing reconciling them | re-testing an upstream entry | the row above |
| 11-05 | the Index Reduction **tab** stayed red on `CartesianPendulum` after the banner was fixed | **Doug**, loading the specimen | the stage outcome is the banner's second home and went untouched. Nothing compares a label's colour against the pane beneath it, and `egui::RichText` cannot be read back — so the colour was unreachable from a test until `tab_color` was split out |
| 11-05 | `Outcome::Flagged` painted `error_fg_color`, on **six** specimens that simulate | measuring the corpus while chasing the tab | the split exists to tell a flagged stage from a failed one, and `Outcome`'s own doc recorded that the UI was *"unchanged"* — true, harmless-sounding, and the whole defect |
| 11-05 | `the_reconciled_note_still_offers_the_summary_view` **passed with the fix stubbed out** | the must-fire check, run before trusting the test | the note it falls back to contains the asserted word too. A substring assertion cannot tell a replacement from no replacement; it needed a non-vacuity guard |

### 11-05, mixed path separators — fixed where Doug can edit them, 20 instances left

**`concat!(env!("CARGO_MANIFEST_DIR"), "/…")` produces a mixed path on Windows**, because Cargo
spells the manifest dir natively and the appended literal does not. Doug found it smoke-testing the
rebase: *"`C:\Users\dougd\source\repos\rumoca\hrw/specimens`. It seems that the slashes should all
be the same kind."*

**Fixed for the two the Settings dialog hands back to the user** — `DEFAULT_SPECIMEN_DIR` and
`DEFAULT_LIBRARIES` — with a `cfg`-selected `native_sep!()` literal, so the mixed form is
unconstructible rather than cleaned up afterwards. That mattered: `app::tests` compares paths
against `DEFAULT_SPECIMEN_DIR` with `starts_with`, so a display-only fix would have put the two
spellings one assertion apart. `the_default_paths_use_one_kind_of_separator` pins both.

**Twenty instances of the same shape remain in `hrw/src`, twelve of them in `bridge.rs`** —
`BRIDGE_DIR`, `STAGES_DIR`, `DOCS_DIR` and the rest. **Deliberately not swept**, and the reason is
not that they are harmless: they reach `focus.json` (`stages.dir`), the log, and generator output
(`gen_trace` prints `…\hrw/docs/specimen-notebook/…`). They are *internal* — nothing offers them
back to a user to edit — and the committed notebook traces carry canonicalised paths, so changing
what the constants emit could move trace content during a smoke test. **The sweep is cheap and the
mechanism now exists; it wants a quiet moment and a full gate, not a mid-test change.**

*This repository has a name for stopping here: `lib.rs`'s `msl_roots` doc records a sweep that
consolidated seven copies and was itself half-finished, with the eighth copy being the one the
running app used. **Twenty left is that shape.** The difference is that this one is written down
with its count, which is the whole of the mitigation.*

**Found by Doug reading a dialog**, which is the "who caught it?" trigger: nothing checks that an
emitted path is spelled consistently, and **the defect is invisible on Linux and macOS**, where the
literal and the separator already agree — so it could not have failed on a CI box.

### 11-05, a file's age read as a process's liveness — the hook now says which

**Claude asserted "HRW is open" twice while it was closed**, from the hook line *"focus.json written
54s ago"*. Doug corrected it. The inference was never stated as one: a bridge file **outlives the
process that wrote it**, so a write 54 seconds ago is equally consistent with an app that exited 53
seconds ago, and the age ticking up across prompts (54s → 2m → 4m) is one write ageing rather than a
session recording actions.

**`tasklist` was available and had already been used earlier in the same session.** So this is not a
missing instrument; it is a reporter whose output invited a wrong reading and a reader who did not
check. **The repository's own name for it is *a true measurement of the wrong unit*** — `focus.json`'s
mtime is a true fact about a write, reported as a fact about a running app.

**`hrw-context.ps1` now reports the process**, both ways, and says in the negative case that *"every
age below is a file's, not a live session's"* — the inference blocked where it was being made rather
than in prose someone would have to already be reading. Verified in both states before being
trusted: launched HRW and saw `HRW IS RUNNING`, killed it and saw the line flip.

**The same script already carried the previous version of this mistake**, in a comment: its first
version took the *age* from `focus.json` and the *content* from `session.json`, conflating two file
ages with each other. **This one is that defect one step out — a file age conflated with a
process** — which suggests the family is the hazard, not the instance. Anything this hook reports
should say what it is a fact *about*.

| date | defect | caught by | note |
|---|---|---|---|
| 11-05 | "HRW is open" asserted twice while it was closed | **Doug** | nothing in the hook reported the process, and every other line was an age |

### 11-05, `note_is_error` has six remaining callers and one of them feeds Claude

> ### ⟶ FIXED 2026-11-05 IN CHICAGO — and measuring first changed the fix twice
>
> *"I very much want to widen the scope to fix `App::failure_context`"*, then the machine was shut
> down to travel. Resumed and done. **`App::pipeline_context` replaces it**, returning flags and an
> optional stop; `bridge` emits `pipeline_flags` beside `pipeline_failure`.
>
> **What the measurement found that the code read had not.** `Drivetrain` was loaded in a running
> HRW and the Structural tab clicked — the only path that writes `focus.json`, which is itself worth
> knowing, since **neither `hrw://load/...` nor `hrw://stage/.../node/...` through the screenshot
> harness captures anything.** The real file carried `pipeline_failure` naming Structural with the
> note *"the FIRST stage that failed"* **and `downstream_not_reached: []` in the same object**. The
> section contradicted itself and asserted the failure anyway.
>
> **Then the fix changed shape twice, both times because something was checked rather than
> assumed:**
>
> 1. *"Emit the failure only when the outcome is `Failed`"* — **wrong.** `DimensionMismatch`'s
>    Typecheck is `Flagged` and **does** stop the pipeline (7 stages never run), while
>    `Drivetrain`'s Structural is `Flagged` and does not. The outcome label does not decide it;
>    the measured tail does.
> 2. *"The tail is the later stages with no value"* — **also wrong**, and the counter-example was
>    already written down on a sibling test: `the_not_reached_tail_is_contiguous_and_agrees_on_its_cause`
>    records that `OverDeterminedShaft`'s Flatten is empty **because nothing was retained**, not
>    because it never ran. `CartesianPendulum` is the other direction — Initialization produces no
>    IR, says so about the *model*, and Events and Solve lowering after it both run. So the old code
>    told Claude that a stage in a finished compile had never been reached. **Reading the sibling
>    test's doc comment is what caught this**, before it was written rather than after.
>
> **So it was three defects, not one**, and `Stage::says_not_reached` now single-sources the reading
> side of the signal `not_reached_note` writes — a leak that existed in the other direction all
> along, with `fidelity::tests::a_stage_that_says_it_never_ran_shows_no_ir` matching the words by
> hand.
>
> **Verified at the artefact, not only in tests**: the same click re-run against the fix gives no
> `pipeline_failure`, a `pipeline_flags` naming Structural, and the full diagnosis — 4 unmatched
> unknowns with their 4 MSL source locations — intact. Both tests were proved to fail against the
> old behaviour, **and defect 3 was isolated on its own** so claim 3 could not pass vacuously.
>
> **Still open, and next:** the 0.10.4 rebase — steps 6, 6b, 7, 8 — then
> [`updating-rumoca.md`](updating-rumoca.md) **step 10**, the long-running suites and the oracle.

**`Stage::note_is_error()` is `outcome != Ok`**, so every caller reads *"Rumoca had something to
report"* as *"this stage failed"*. `stage_tabs::tab_color` was fixed on 11-05; the rest were scoped
out **in the same conversation that then widened the scope**, which is why the table survives as the
work list rather than as a record of a decision.

**The population is not an edge case.** Six corpus specimens simulate successfully with a flagged
Structural stage — `BenchActuator`, `CapacitorLoop`, `CartesianPendulum`, `Drivetrain`,
`GearWithBrake`, `MotorWithBrake` — measured from the committed manifests.

| caller | what it claims | verified how |
|---|---|---|
| ~~`App::failure_context` → `bridge::PipelineFailure` → **`focus.json`'s `pipeline_failure`**~~ | ~~names the first flagged stage as *the* pipeline failure, plus every later stage as a consequence~~ | **FIXED 2026-11-05** — measured on a real `Drivetrain` capture first, which is what turned one defect into three. See the box above |
| `App::compile_outcome` → `diagnostics::record_action("compiled", …)` | the action trail records `"<model>: FAILED at Structural"` | read from the code |
| `App::last_successful_stage` | a flagged stage is not "where the tabs should land", so a singular model opens further back than it reached | read from the code |
| `app.rs`'s stage-note colour (~5342) | the note under the tab row is drawn in `error_fg_color` — **the same conflation as the tab, in the pane below it** | read from the code |
| `app.rs`'s `has_error_summary` (~5329) | requires an `"error"` payload as well, so it is probably right | — |
| `artifact_pane` (~168) | its module doc already says `note_is_error` covers *both* abnormal outcomes, so it is deliberate | — |

**The first row was the one that mattered most, and it was not the colour.** `focus.json` is the
context Claude reasons from, and *"emitted context must be exact, never approximate"* — a
`pipeline_failure` naming Structural for a model that simulates is a false claim in the one
artefact that cannot be cross-checked by looking at the screen. **Fixed 11-05, and the instruction
to measure first paid for itself twice**: the measurement confirmed the key was really there, and
then two successive candidate fixes were each refuted by a specimen before being written.

**Four rows remain and none is in that class** — `compile_outcome` writes `"<model>: FAILED at
Structural"` into the action trail, `last_successful_stage` lands the tabs further back than the
compile reached, and the stage-note colour repeats the tab's old conflation one pane down. All
three are seen by Doug rather than by Claude, which is why they rank below the row above and not
beside it.

**Why this went unseen for as long as the outcome split has existed:** `Outcome`'s own doc comment
says the UI colouring is *"unchanged"*. That sentence was true, read as harmless, and was the whole
defect — the split was introduced precisely to stop `Flagged` meaning failure, and then nothing
downstream was moved. **A doc comment recording that the consumers were not updated is a to-do
wearing the clothes of a design note.**

### 11-04 night, F4 has been blind since v0.10.0 — and nobody ran the large suite to see it

**Step 3b's first post-rebase run reported 24 violations across 20 MSL models, every one F4, and
every one a false positive.**

`BlockReport::StructuredScalar` arrived in **v0.10.0**. It is a compaction — a regular equation
family whose rows are each their own scalar block, reported once with its point count instead of
expanded into one line per element — and `worker::block_to_json` emits `point_count` and
`equations_per_point` for it, deliberately carrying **no `equation` or `equations` key** and no
`id`, because the family names no single equation.

`fidelity::check_f4` matches on exactly those two keys:

```rust
let names: Vec<&str> = match (&b["equation"], &b["equations"]) {
    (Value::String(one), _) => vec![one.as_str()],
    (_, Value::Array(many)) => ...,
    _ => { v.push(format!("{}: block {bi} names no equations", s.label)); continue }
};
```

**This is the failure mode `fidelity_msl.rs`'s own header names** — *"a check that knows one form
of the truth reports every other form as a defect"* — nine of twelve first-run violations were
this shape. It is the tenth.

**The noise is the smaller half.** F4's claim is that the BLT blocks **partition** the equations.
A `StructuredScalar` block contributes nothing to `seen`, so on any model containing one the
partition is checked over a *subset* and the missing rows are reported as holes
(`blocks cover 2 of 3 equations; missing rows [2]`). **So F4 is simultaneously loud and blind:**
it fails models that are fine and cannot detect a real partition defect in any model with a
compacted family.

### ⟶ FIXED 2026-11-06 — the row range is published, and a specimen now reaches the shape

**Doug chose the stronger half of the pair** — *"fix F4 so that we are able to make full use of the
fidelity test"* — because teaching F4 to skip compacted blocks would have silenced the noise and
left it unable to find a real partition defect in any model containing a family.

`BlockReport::StructuredScalar` gains `first_row` and `row_count` from
`StructuredScalarBlock::row_range()`, which the type already had; `worker::block_to_json` publishes
them, and `check_f4` expands the range into the rows the family accounts for. Additive fields on a
report type — the most upstreamable shape a `crates/` change can take.

**And the process hole is closed, which matters more than the code fix.** F4 went blind for two
releases *because the curated specimens did not reach the shape*, so only the hours-long MSL sweep
exercised it. `specimens/ArrayFamily.mo` — a `for` equation over an 8-element array, the simplest
model that compacts — is now in `fidelity::MODELS`, so the pre-commit suite covers F4's compacted
path in seconds.

**Falsified rather than assumed.** With the two keys renamed so F4 cannot see them, `ArrayFamily`
reproduces both documented halves of the defect exactly:

```text
ArrayFamily / Structural: block 0 names no equations
ArrayFamily / Structural: blocks cover 0 of 8 equations; missing rows [0, 1, 2, 3, 4, 5, 6, 7]
```

Loud and blind at once. With the keys restored both disappear, and the published block reads
`first_row: 0, row_count: 8` against an 8-equation incidence. A new must-fire test
`f4_reads_a_compacted_family_and_still_sees_a_partition_defect` drives the shape through four
cases — whole coverage quiet, a hole still seen, a double-cover still seen, and a family claiming
rows past the incidence — because F4's *existing* must-fire used `{"kind": "scalar"}`, the shape it
was written against, and so proved only that the handled shape was handled.

**The account below is kept as written**, because what the pair refused is the useful part.

---

**NOT FIXED TONIGHT, deliberately.** The repair needs a decision this run is not allowed to take
alone: what rows a compacted family covers for partition purposes. The block publishes
`point_count × equations_per_point` as a *size*, but `check_f4` resolves names through
`row_labels`/`index_of`, and a family has no per-row label to resolve. Expanding it needs the
incidence row range the family occupies — which `StructuredScalarBlock::row_range()` has and the
JSON does not publish. **That is an HRW publishing change plus an F4 change, and it is a pair for
Doug:** publish the family's row range, or teach F4 to skip compacted blocks and say so in the
report rather than counting them as holes.

**The committed artifacts in `docs/reports/` are therefore stale as well as wrong**, and the suite
has not been run since before the 0.10.0 rebase — which is how a check could go blind for two
releases unnoticed. *(This said `docs/reports.md`, the design document, which carries no F4 claim
and no corpus counts. Corrected 2026-11-06, when the artifacts were re-promoted: a pointer that
resolves to the wrong real file sends its reader looking for a defect that is not there.)*

| date | defect | caught by | note |
|---|---|---|---|
| 11-04 | F4 reports every `StructuredScalar` block as "names no equations", and cannot see a partition defect in any model containing one | the large fidelity suite, step 3b, on its first post-rebase run | blind since v0.10.0. The large suite is trigger 1 of the fidelity policy and is the only thing that runs F4 at scale; the pre-commit suite's 16 curated specimens do not reach the shape |

### 11-06, OPEN — a green fidelity row has no denominator, so F4's repair is unproven at scale

**The 0.10.4 corpus sweep came back green on all eight checks over 2,780 models. That does not say
F4 was exercised even once.**

A check that passes and a check that had nothing to look at write the identical row. F4's whole
defect was that it could not see a compacted equation family; the repair is falsified on
`ArrayFamily` (rename the two keys and both documented halves return), so the *mechanism* is
proven. What is unproven is **coverage** — whether any of those 2,780 models contains a family at
all. Nothing in the report, the profile or the sidecar can answer it.

**Why this is not pedantry.** The same confusion is what let F4 go blind for two releases: three
corpus sweeps ran green across it. **Green over a corpus is evidence about the corpus the check
examined, which may be empty.** `docs/reports.md` now carries this as the second way a rebase
threatens the fidelity report, beside the one it already described.

**The fix is the must-fire rule aimed at coverage rather than at silence:** have each check report
*that it engaged*, and have the runner carry the tallies into the sidecar, so "zero violations"
arrives with "F4 examined N families across M models". Then a check that stops seeing its subject
shows up as a **coverage collapse** rather than as continued success.

**Deliberately not done while closing the rebase**, and the reason is scheduling rather than
scope: populating a denominator means re-running the three-hour sweep, which would reopen step 10
to answer a question step 10 did not ask. The measurement it would produce is about HRW's
instruments, not about v0.10.4.

*Who caught it: Claude, reasoning about what the finished run had established — not a checker.
There is no checker for this, which is the entry.*

### 11-06, CLOSED BY MEASUREMENT — widening `qualified_citations_resolve` to the history documents

**Rejected, and the measurement is the useful part.** A rename on 11-06 left dead pointers in
`tech-debt.md` and `unattended-run-log.md` behind a green gate, because
`qualified_citations_resolve` scans six documents and neither is one. The obvious repair is to add
them.

**Adding both produced 11 failures, all in `tech-debt.md`, and every one correct as history** —
`PendingLiveDebug::PreLowering`, `reduction_view::ReductionView::from_search`,
`Stage::ok_with_note`, `Focus::LabPassage` and the rest name things this project deliberately
deleted, in entries whose content *is* that they were deleted.

**So the checker cannot be aimed here**: it cannot distinguish a deliberate reference to a deleted
symbol from an accidental reference to a renamed one, and pointing it at a history document turns
the record into failures — which pressures the next session to falsify history to get green. Same
ruling as `DECISIONS.md`, also absent from the roster.

**What is actually missing is narrower: a citation of a *test* name.** A test name is always meant
to resolve — nobody writes an entry about a deleted test by name without saying so — and both of
11-06's dead pointers were exactly that. A check over `` `[a-z_]+` `` tokens that look like test
functions, across all documents, would have caught both and none of the 11.

*Who caught it: a measurement that contradicted the obvious fix. Recorded so the next session does
not re-propose the widening and then discover the same 11.*

### 11-04 night, the Initialization pane has no tooltips and the docs for them are already on disk

**Step 4 of the rebase procedure warns about exactly this and calls it silent.**
`StageKind::ir_crate()` decides which crate's harvested `///` docs a stage's tooltips come from,
and the step says: *"`IR_CRATES` failing to resolve is loud. `StageKind::ir_crate()` pointing a
stage at a crate that no longer holds its type is silent — the stage simply loses its tooltips."*

**It is not pointing at the wrong crate. It is pointing at nothing:**

```rust
StageKind::Initialization | StageKind::Simulation => return None,
```

**That was correct on 0.9.20 and stopped being correct during the 0.10.0 rebase**, when the
Initialization pane was re-modelled onto `initial_pins`. The stage publishes two keys,
`determinacy` and `pins`, and `rumoca-phase-structural` — already a harvested crate — documents
`pins` as *"MLS 3.6 §8.6 initial equations rewritten onto the state each one determines. The
proof that decides which state…"*. The text is in `src/field_help.json` right now. Nothing can
reach it.

**Simulation is the same shape but needs more than a mapping.** Its pane is fed by
`rumoca_solver::SimVariableMeta`, and `rumoca-solver` is not in `field_help::IR_CRATES` at all,
so there is no harvested doc to point at. That matters more since 11-04, because
`SimVariableMeta::state_coordinate` — *"the source scalar a generated reduced-selection state
scalar equals"* — is now rendered in that pane and is exactly the field a reader will not guess.

**NOT FIXED TONIGHT.** Adding a mapping changes what a pane displays, and the honest fix may not
be a mapping at all: `Structural` and `IndexReduction` already draw from
`rumoca-phase-structural`, so pointing `Initialization` there too gives three stages one
namespace with no disambiguation — which is the collision the per-stage split was built to end
(66 field names are documented in two or more crates, and a flat map once served an AST import
clause's doc as Solve lowering's `names` tooltip). Within a single crate the table has no key
finer than the field name.

### ⟶ AND STEP 4 WAS REVERTED, because regenerating the table restores a lie Doug reported

**`gen_field_help` ran clean and its output was thrown away.** The regenerated table is a
faithful reading of v0.10.4 — and committing it would reintroduce a defect with a name.

**v0.10.4 deleted two `///` doc comments that HRW's per-stage keying depends on**, and the two
guards that exist for exactly this caught both:

| field | crate that lost its doc | what the pane would say instead |
|---|---|---|
| `names` | `rumoca-ir-solve` | `rumoca-ir-ast`'s *"All names imported from the package"* — an **import clause** |
| `causality` | `rumoca-ir-dae` | `rumoca-ir-ast`'s type-alias causality, not the FMI meaning |

**The `names` one is Doug's own 2026-09-03 report**, which
`the_solve_lowering_names_symptom_is_unreachable` is named after: five times in one
afternoon he could not find `Y` in Solve lowering, and part of why was that hovering `names`
produced a doc about imports. The fix needed **both** halves — the per-stage keying *and* the
`rumoca-ir-solve` doc comment giving the stage something of its own to prefer. **Upstream removed
the second half.** Per-stage keying alone cannot help: with only one crate documenting the name,
every stage gets that one.

**So `src/field_help.json` is deliberately left at its v0.10.0 content and step 4 is UNFINISHED.**
A stale-but-true tooltip beats a fresh-but-false one, and the guards are right — weakening them to
let the regeneration through would delete the only thing that noticed.

**The pair for Doug.** Either carry the two doc comments as part of this fork's instrumentation —
they are `///` text on upstream fields, additive and semantics-preserving, and plausibly a welcome
PR in their own right — or give `FieldHelp` a local override table for names upstream has stopped
documenting, and say in the tooltip that the text is HRW's. **The first keeps one source of truth;
the second admits a second one.** Not a 3 a.m. choice.

**The pair for Doug:** map `Initialization` to `rumoca-phase-structural` and accept
within-crate collisions, or key the table by something finer than `(field, crate)` before
mapping anything new to a crate another stage already uses.

| date | defect | caught by | note |
|---|---|---|---|
| 11-04 | regenerating `field_help.json` for v0.10.4 would restore an import-clause doc as Solve lowering's `names` tooltip | `field_help`'s two guards, on the gate | upstream deleted `rumoca-ir-solve`'s `names` doc and `rumoca-ir-dae`'s `causality` doc. The `names` case is Doug's own 2026-09-03 report, and the test is named after it |
| 11-04 | the Initialization pane shows no field tooltips although its `pins` doc is harvested and committed | step 4 of the rebase procedure, which names this failure as silent | introduced by the 0.10.0 re-model of the pane onto `initial_pins`; survived into 0.10.4 because nothing checks that a stage with IR has a crate to draw docs from |

### 11-05 night, two MSL models crash the fidelity runner and every layer reports a clean sweep

**The overnight sweep's report reads 2,613 models, every outcome `ok`, `n_violations = 0`. Two of
the models it claims to have swept were never compiled: the runner died on them.**

The arithmetic is where it shows. The memory profile accounts for all 2,626 models — 2,615 `ok` and
11 `aborted:proc-ceiling` — while the report carries 2,613 rows. The 11 ceiling aborts are
*correctly* absent, because a killed process writes no row and the profile records the verdict. The
remaining **two** are called `ok` by the profile and do not exist in the report:

- `Modelica.Blocks.Continuous.Der` — profile says 759 MB, 3 s, `ok`
- `Modelica.Electrical.Machines.Utilities.DQCurrentController` — 785 MB, 2.9 s, `ok`

Both reproduce on demand. The runner prints `--models: 1 matched`, `loading MSL…`, `loaded in 1.4s`
and then **exits 101 with an empty stderr and a header-only report**. With and without
`--only-checks`, so the crash is in the compile, before any check runs.

#### The chain, and every link was verified rather than assumed

1. **The compile panics, and nothing catches it.** `compile_model_by_name` (`worker.rs:2732`) is not
   wrapped in `guard`, whose only non-test caller is `Worker::spawn`'s message loop
   (`worker.rs:1641`). So the **GUI survives these two models** and would surface the panic as a
   message; only the examples, which call the method directly, die of it.
2. **The panic message is destroyed.** `WorkerState::compile` holds an `OutputCapture` that
   redirects fds 1 and 2 through a pipe (`worker.rs:7220`) so Rumoca's own `println!` diagnostics can
   be forwarded as log entries. The default panic hook prints *before* unwinding reaches that
   guard's `Drop`, so the message goes into the pipe and dies with the process. `RUST_BACKTRACE=full`
   changes nothing — this is not a missing-backtrace problem, the text never reaches a live fd.
3. **No crash file is written either.** `fidelity_msl` never calls `diagnostics::init()`, whose hook
   writes `crash-<utc>.json` **to a file** and would have bypassed the redirect entirely. The one
   mechanism built for exactly this is not installed in the binary that needs it.
4. **`measure-fidelity.ps1` never looks at the exit code.** Its verdict is chosen from free RAM, the
   process ceiling and the timeout — all sampled *while the process runs*. A process that panicked
   after 3 s at 759 MB trips none of them, so it is recorded `ok`. Searching the script for
   `ExitCode` returns nothing.
5. **A missing row then reads as "not yet done".** `--resume` keys off rows in the report, which is
   right for a ceiling abort and wrong here: the model is retried every run and crashes every run,
   forever, at no visible cost.

**Why this one matters more than its size suggests.** A hard compiler crash on a stock MSL model is
the strongest finding this instrument can produce, and five layers in sequence each turned it into
silence — ending in a report whose denominator is wrong and which says nothing about being wrong.
The failure is not that a model crashed; it is that **`ok` here means "did not exceed memory or
time", while every reader takes it to mean "was measured"**.

#### What the crash probably is — hypothesis, not a measurement

The survey compiles both models *successfully*: `Der` as `n_equations=1`, `structural=singular`,
`index_reduced=error:empty system: no equations or unknowns`; `DQCurrentController` as
`structural=singular`, `index_reduced=singular`. The difference between the two instruments is that
fidelity destructures `equation_sheet` and `identifier_index` out of the compile and the survey does
not. **Suspect the sheet or the identifier index on a singular or empty system.** Both models being
singular, and one having an empty system, is the kind of coincidence worth starting from.

#### The recommended order — three separable fixes, widening blast radius

1. ~~**`measure-fidelity.ps1` records `crashed:exit-<n>`.**~~ **DONE 2026-11-06.** The verdict is
   decided after `WaitForExit` and is deliberately absent from the default `-RetryVerdicts`: a panic
   is deterministic, exactly like `aborted:proc-ceiling`.
2. ~~**`fidelity_msl` guards the compile.**~~ **DONE 2026-11-06**, and `diagnostics::init()` turned
   out to be unnecessary — `catch_unwind` recovers the panic **payload**, which the fd redirect never
   touches, so the message needs no crash file. The model now gets a row with outcome `panicked`
   carrying the message, the session is rebuilt rather than reused, and the non-`Compiled` path that
   used to `continue` silently writes a `no-compile` row. Both previously-fatal models now exit 0.
3. ~~**Root-cause the panic.**~~ **DONE 2026-11-06 — and it is one upstream defect, not two.**
   Both models abort at `rumoca-phase-dae/src/construction/expression/temporal.rs:15`, where
   `lower_derivative` calls `.expect("analysis proves derivative role")` on a `Result` that returns
   `Err(UnsupportedFlatSemantics { feature: "derivative target", … })` for a `der()` target that is
   not a continuous state. The condition is diagnosed correctly and then asserted away, in a function
   whose own signature already returns `DaeConstructionError`. **The same file disagrees with
   itself**: of its three functions, `lower_derivative` and `lower_pre` assert while
   `lower_history_operator` propagates with `map_err(|_| missing())?` — and line 102 propagates on
   `coordinate.previous(span)`, *the identical call* `lower_pre` asserts on at line 38. (First
   recorded here as "`lower_pre` at line 102", which was wrong: line 102 is
   `lower_history_operator`'s, and `lower_pre` shares the defect rather than the fix.)
   Written up as `upstream-issues.md` **D1**; the fix is a change to Rumoca *semantics* rather than
   observability, so it is **Doug's call**, not a unilateral instrumentation edit.

### Last night's sweep ran FIVE checks, not eight, and silently dropped the one that matters most

No full-corpus fidelity run on disk has ever recorded a violation: `fid-full.csv`, `fid-0804.csv`
and last night's carry 2,614, 2,614 and 2,613 rows, with `checks_failed` empty and `n_violations`
at `0` in every one. The question that matters is whether those zeros are evidence.

**Partly.** Must-fire coverage does exist. `each_invariant_catches_its_own_violation`
(`fidelity.rs:1673`) drives **F3, F4 and F5** with deliberate violations and asserts each fires, and
`f10_catches_each_way_a_pane_can_mislead` covers F10's three claims. F2 appears in the first test
*only* as a skip assertion — `check_f2` returns empty when there is no DAE — which proves nothing
about firing.

**F2, F6, F7 and F9 have no must-fire test at all.** `check_f6` and `check_f7` are reachable only
from `check_model` and the corpus test; nothing anywhere hands either one a violation.
`a_rumoca_failure_is_represented_faithfully` is a corpus *observation* of F9, not a must-fire. Their
zeros carry no information.

#### But first, what actually ran — and it is not what the sweep was asked for

**`check_model`'s selectable roster is F2, F3, F4, F5, F6, F7 and F10** — those are its seven
`want()` calls, and nothing else is gated by `--only-checks`. Against that roster, the string this
sweep was launched with, `-OnlyChecks 'F1,F2,F3,F5,F6,F7,F8,F9'`:

- **omits F10**, which is gated by `want("F10")` and therefore did not run;
- **names F1, which is RETIRED — not missing, and the difference matters.** Its two re-derivation
  checks in `worker.rs` were deliberately removed when the animations began reading the compile's
  own steps, on the ground that *a test comparing a thing against itself passes for no reason*; its
  surviving half is `check_f2`. `worker/tests.rs` records all of it. The flag accepts the name and
  selects nothing.
- **names F8, which is not a check.** It is inline size accounting at `fidelity_msl.rs:300` that
  produces no violations at all; `every_stage_serializes_without_panicking` is what covers it.
- **names F9, which `--only-checks` cannot gate** — it lives in the runner outside `check_model`, so
  it ran regardless of being listed.

The runner prints the proof per model, and the last model's stderr says it plainly:
`[check cost] F7, F2, F5, F6, F3`. **Five checks**, plus ungated F9. F4 was excluded on purpose;
**F10 was excluded by accident, by the same string.**

**F10 is the worst one to lose.** It is the only check about a *verb* — whether a pane's claim about
the compiler is honest rather than whether a structure matches — and it is the one check with
must-fire coverage proving it can fail three distinct ways. The check most able to find something is
the one the sweep dropped, and nothing reported the loss.

**The missing mechanism is roster discipline.** `--only-checks` accepts any string: an unknown name
selects nothing and is never rejected. This repository already has the pattern that would have
caught it — `StageKind::ALL` and `FloatingWindow::ALL`, matched exhaustively with no `_ =>` arm — and
the fidelity checks have no equivalent, so a stale or mistyped name narrows a 2.5-hour sweep in
silence. **The run is not a measurement of the corpus under the checks; it is a measurement under
five of them.**

One smaller consequence, same cause: the runner's end-of-run note reads *"0 empty stages, so F10's
absence clause was never exercised"* **even when F10 was deselected**, which reads as "F10 ran and
had nothing to act on."

#### The part worth more than the count

**F4's must-fire test passes while F4 is blind in production.** The test builds blocks as
`{ "kind": "scalar", … }`; F4's blindness is on **`StructuredScalar`** blocks. The test exercised the
shape the check already handles and said nothing about the shape v0.10.x actually produces — so a
must-fire test demonstrates that a check fires *on the input its author had in mind*, which is the
same input the check was written against. **It cannot detect blindness to a shape neither of them
considered.** That is now measured rather than argued, and it is the reason "add must-fire coverage"
is a floor and not the fix.

#### Correcting the first version of this entry, written the same morning

It claimed no vacuity test existed, and that last night's zero should be read as *"the checks did not
fire"*. **Both were wrong.** Three checks are proven able to fire. And the August runs were made on
**0.9.x, before F4 went blind**, so their zeros are consistent with a genuinely clean corpus rather
than evidence of vacuity — the two runs that would have shown F4's 24 false positives postdate those
files. The defensible reading is narrower than the one first recorded: **plausibly real for the
checks with must-fire coverage, uninformative for the four without.**

**And a second correction, an hour later and larger.** The first two versions of this entry both
said "the eight enabled checks", and the report to Doug said the same. There were never eight. Two
of the names in the flag do not exist as checks, one cannot be gated by it, and one real check —
F10 — was silently deselected by it. **The count was taken from the string that was typed rather
than from the roster the code offers**, which is the same error as reading a verdict of `ok` as "was
measured": a true statement about the wrong thing. The roster was four greps away the whole time.

#### What is owed

1. ~~**`--only-checks` must reject a name that is not on the roster.**~~ **DONE 2026-11-06.**
   `fidelity::CHECKS` is the roster, `unknown_checks` is the guard, and the runner exits 2 naming
   both the offending names and the roster. `the_check_roster_matches_the_code` scans this file's
   gates and the runner's for `want(…)` and requires their union to equal `CHECKS` — so a check
   added without a roster entry fails, and **a roster entry nothing gates fails too**, which is the
   half that would have caught F1 and F8. F9 is now gated in the runner with the same spelling
   instead of running unconditionally. The runner also prints **`NOT running: …`**, because printing
   the request back can never reveal a check left out of it, and the omission of F10 is the failure
   this flag actually suffered. Verified by falsification: adding `F1` to the roster fails both
   tests, and the sweep's own string now exits 2.
2. **Re-run the sweep with the real roster.** Last night's measured neither F4 nor F10, so the corpus
   is unmeasured under both — and F10 is the check with the best demonstrated ability to fail.
3. ~~**Must-fire for F2, F6, F7 and F9.**~~ **DONE 2026-11-06.** All four now have one, and
   **F9 moved out of the example into `fidelity.rs`** to get it: living in
   `examples/fidelity_msl.rs` put it outside `--only-checks` *and* out of reach of `cargo test`, and
   `check_f10` has the same signature and never left, so the split bought nothing. The roster
   exception it required is now empty. Each test was verified by falsification, not by passing.
4. ~~**Must-fire driven by IR shapes taken from the corpus.**~~ **DONE for F2 and F6, by
   cross-pairing** — compile two specimens and hand one's equation sheet and published incidence to
   the other's DAE. The violation is then built entirely out of IR the compiler really emits, which
   is the property F4's synthetic `{"kind": "scalar"}` test lacked. Measured on `SingleInertia`
   against `TwoLoops`: 2 equations versus 4, and **the equation-count assertion is load-bearing** —
   if the two specimens ever converge the pairing becomes valid and the test would pass having
   proved nothing, so it asserts they differ before anything else.
5. ~~**Decide what F1 and F8 are.**~~ **CLOSED 2026-11-06 — there was nothing to decide, and the
   item's premise was wrong.** It read "either implement F1 and give F8 a violation it can raise, or
   stop naming them as checks", on the belief that F1 was simply unimplemented. **F1 is retired by
   design.** Two of its three checks were removed when the animations began reading the compile's
   own steps — *a test comparing a thing against itself passes for no reason, which is worse than no
   test* — and its surviving half became `check_f2`, which compares against a fresh
   `build_incidence` instead of a second implementation. `worker/tests.rs` had that account written
   out the whole time. F8 is a size measurement, covered by
   `every_stage_serializes_without_panicking`.

   So nothing needs implementing. What needed fixing was **the flag accepting their names**, and the
   roster does that. **`fidelity-plan.md`'s "F1-F9" is left alone deliberately**: it is a dated
   record of what was true on 2026-07-31, and this repository keeps such accounts rather than
   back-editing them — the retirement note itself is the model for that.

   **Fourth correction of the day, same shape as the first three.** "F1 is not implemented" came
   from grepping `\bF1\b` and finding only comments — but a test is named for what it checks, never
   for its plan number, so the grep could not have found it. Reading one neighbouring file would
   have. Corrected in `fidelity::CHECKS`, the runner's rejection message, `measure-fidelity.ps1` and
   above.

| date | defect | caught by | note |
|---|---|---|---|
| 11-05 | two MSL models panic the fidelity runner; the report omits them and the wrapper calls them `ok` | Claude, reconciling the report's row count against the memory profile | 2,615 profiled `ok` against 2,613 reported. Five layers converted a hard crash into silence; the GUI path is guarded and unaffected |
| 11-05 | a panic inside `WorkerState::compile` can never be read | reproducing the crash with `RUST_BACKTRACE=full` and getting an empty stderr | the `OutputCapture` dup2 redirect is still installed when the default hook prints. `diagnostics::init()`'s crash file would bypass it and is not installed in the examples |
| 11-05 | `measure-fidelity.ps1` reports `ok` for a process that exited 101 | reading the verdict logic after the row count failed to reconcile | the verdict is sampled from RAM, ceiling and timeout while running, and the exit code is never consulted at all |
| 11-06 | **the survey has not run since 2026-08-01**; six shards die instantly and the merge silently uses three-month-old part files | asking why a byte-identical cache could give two different results, which forced "was the 20:20 run a run?" | the transcript's whole survey phase is two lines with no shard output. `while (Get-Process survey_msl)` cannot tell instant death from completion, and `--merge` returns before `load_msl()` so it never fails with them |
| 11-06 | **Claude committed a survey artifact stamped `0.10.4` that holds 0.9.x data**, then built a provenance feature on top of it | the row above; reverted the same hour | `5601dfc8`'s message claims a re-run that never happened. The ≥2,000-model guard exists to catch a partial survey and passes a complete stale one |
| 11-06 | "zero stable-column differences across 2,626 models" was reported as the strongest rebase-health signal available; it was the same data compared against itself | the row above | the cleanliness was flagged out loud as suspicious and the one cause that explains it exactly was not checked |
| 11-05 | **a 2.5-hour sweep ran five checks while reporting as though it ran eight**, F10 among the missing | reading `check_model`'s `want()` calls instead of trusting the flag string | `-OnlyChecks` omitted F10, named F1 and F8 which are not checks, and named F9 which it cannot gate. `[check cost] F7, F2, F5, F6, F3` is the per-model proof the runner printed all night |
| 11-05 | `--only-checks` accepts any string; an unknown check name selects nothing and is never rejected | the row above, asking why the omission was silent | the repo's own `StageKind::ALL` / `FloatingWindow::ALL` roster pattern is the fix and was not applied here |
| 11-05 | the end-of-run note says F10's absence clause "was never exercised" even when F10 was deselected | reading the note beside a check list that lacked F10 | reads as "F10 ran and found nothing to act on", which is the opposite of what happened |
| 11-05 | F2, F6, F7 and F9 have no must-fire test, so nothing has ever falsified them | checking whether "zero violations" was informative before reporting it as good news | F3/F4/F5/F10 are covered. `check_f6` and `check_f7` are reachable only from `check_model` and the corpus test |
| 11-05 | **a passing must-fire test did not stop F4 going blind** | reading `each_invariant_catches_its_own_violation` after claiming no such test existed | the test uses `"kind": "scalar"`; F4 is blind on `StructuredScalar`. Must-fire over author-chosen JSON proves only that the handled shape is handled |
| 11-05 | Claude wrote "no vacuity test exists", "the checks did not fire", and "eight checks" into this file and into its report; all three false | re-measuring each claim before building on it | one file read and four greps would have prevented all three. Each was asserted from the CSVs alone because the CSV evidence felt sufficient, which is how a true statement about the wrong unit gets recorded as a finding |

### 11-06, THE SURVEY HAS NOT RUN SINCE 2026-08-01, and a stale merge hid it for three months

**This blocks the re-run, and it is the largest open item on this page.**

#### Reproduced

`survey_msl` panics during startup at `survey_msl.rs:165`:

```text
MSL should resolve: Resolve errors: annotation Evaluate has no effect on function local 'm' …
… unresolved function call: 'Medium.density' …
```

- Reproduced with `--limit 2` **and** with `--slice 0/6 --rebuild-every 200`, the exact
  invocation the 11-05 sweep used.
- Reproduced with `target/release/examples/survey_msl.exe` **built at 20:16 on 11-05**, which is the
  binary the sweep used, so nothing committed on 11-06 is responsible. (This was first written as
  "the same binary that surveyed 2,626 models successfully at 20:20" — it did no such thing; see
  below.)
- The failing call is `session.model_names()`, which the survey `.expect()`s.

#### The asymmetry, which is the useful part

**MSL still loads and still compiles.** `fidelity_msl` loads the same roots and compiles models
right now, in seconds, exit 0. The gate's `msl_resolve` integration test —
`rotational_inertia_specimen_resolves_against_msl` — passed minutes before this was found.

So the libraries parse and a specimen resolves against them. What fails is **enumerating the model
names from the session**, which only the survey does. Both paths reach the same on-disk cache
through `parse_source_root_with_cache`, so the loader is *not* the difference.

#### The first mechanism recorded here was WRONG, and the correction is the finding

This entry originally blamed `upstream-issues.md` **P1** — a workspace document poisoning the
durable roots' resolution in `%LOCALAPPDATA%\Rumoca`, with the sweep's survey-then-fidelity
ordering hiding it. **The cache disproves it.** `source-roots/parsed-files` has mtime **11-05
17:04** and `.rumoca-prune-checked` **17:05**, both *before* the 20:20 sweep; nothing has written
to or pruned it since, which is consistent with no `crates/` change having been made.

So the cache is byte-identical to what the 20:20 run used. Same binary, same cache, same vendored
MSL — which forces the real question: **was the 20:20 run actually a run?**

#### It was not. The survey did not run on 11-05 either

The sweep transcript's entire survey phase:

```text
=== MSL survey: 6 shards ===

[survey] merged -> hrw\docs\reports\msl-survey.csv
[survey] 2626 models listed for the fidelity sweep
```

**No shard produced a single line.** Three things combined:

1. The six shards panicked immediately, on this same resolve failure.
2. The script waits with `while (Get-Process survey_msl) { Start-Sleep 15 }`. Processes that die
   before the first poll are indistinguishable from processes that finished, so it fell straight
   through. **Instant death reads as completion.**
3. `--merge` returns at `survey_msl.rs:125`, *before* `run()` calls `load_msl()` — so the merge
   cannot fail for the reason the shards did. It merged whatever was at `C:\tmp\part-*.csv`.

And what was there: **`part-0..3` dated Aug 1 16:29–16:32 (657/657/656/656 rows) and `part-4,5`
dated Jul 31 14:55–14:56 (437/437 rows)** — three months old, and from *two different shard
generations*.

**The survey therefore last ran successfully on 2026-08-01, before the 0.10.0 rebase.** The rebase
is the obvious suspect for the resolve failure, and the stale-merge path is why three months passed
without anyone learning that.

#### The damage, and it reached a commit

- **`msl-survey.csv` committed in `5601dfc8` was 0.9.x data re-merged**, and the merge binary
  stamped its `.meta.json` `rumoca_version: 0.10.4` — a file asserting provenance it does not have,
  which is strictly worse than the `["msl"]` loss recorded below it. **Reverted.**
- **"Zero stable-column differences across 2,626 models" was not evidence of anything.** It was the
  same data compared against itself. It was reported to Doug as *the strongest rebase-health signal
  available*, and its suspicious cleanliness was even flagged out loud — without checking the one
  cause that explains it exactly.
- **The 0.10.4 capability map does not exist.** Nothing is known about how 0.10.4 compiles MSL at
  corpus scale.

#### Why nothing noticed — the same wrong unit, one level up

The script's guards are real and were all satisfied: binaries newer than the newest `.rs`, ≥8 GB
free, ≥2,000 models listed. **Every one measures a precondition; none measures an outcome.**
`[survey] 2626 models listed` was a true measurement of a stale file. The ≥2,000 guard was written
precisely to catch a partial survey and could not, because a complete *old* survey passes it.

#### What the script needs before it is trusted again

1. **Wait on process handles and check exit codes**, not on `Get-Process` by name.
2. **Delete the part files before launching**, so a merge cannot reach a previous generation.
3. **Verify the part files exist, are newer than the run's start, and number exactly `$shards`**
   before merging.
4. **Compare `models_surveyed` against the number of rows the shards actually wrote**, so the
   corpus count is derived from this run rather than from whatever was on disk.

#### Consequences to carry

1. **The 11-05 fidelity results are over a corpus list of 0.9.x vintage.** The model *names* are
   very likely still valid — the stable columns did not move between the two committed artifacts —
   but every statement about "2,626 models" inherits that provenance and should say so.
2. **The re-run is blocked on the survey**, and the survey is blocked on the resolve failure, which
   is now known to be months old rather than hours.
3. **`msl_libraries` is committed but unproven end to end.** The unit test shows the function
   returns the right names; nothing has shown the field rendering into a `.meta.json`, because that
   needs a survey that runs.

### 11-06, the survey runs again — and 0.10.4 loses 409 MSL models that 0.9.20 compiled

**The first real 0.10.4 capability map exists**: 2,791 models, `partial_survey: false`, every shard
verified complete before the merge. Two repairs were needed, and the second was only reachable
after the first.

#### Repair 1 — the enumeration asked for a guarantee MSL cannot give

`session.model_names()` reaches `ResolveBuildMode::Standard`, which requires a
`ResolutionPlanningTree::Complete` and returns `Err` otherwise. **MSL as a whole never resolves
completely and never will** — `Modelica.Fluid`'s `Medium.*` references are unresolved until a medium
is redeclared, which is what `replaceable` means. The survey was demanding library-wide resolution
in order to obtain a list of names that does not depend on it.

Per-model compiles were never affected, which is why HRW, the gate's `msl_resolve` test and
`fidelity_msl` all kept working: `build_resolution_plan_for_strict_compile` *"ignores parse-error
diagnostics from unrelated documents"* and resolves only the target's closure.

**The pre-rebase `hrw` branch has the identical line**, so Rumoca's behaviour changed in 0.10.x; the
survey did not. **No `crates/` change was needed**: `rumoca_compile::parsing::collect_model_names`
is public, is the function the resolved path itself calls, and its sibling's doc states the property
that matters — the names *"address the same targets as compilation **without resolving the AST**"*.

#### Repair 2 — D1 was killing whole shards, and only the first repair revealed it

With enumeration fixed the survey reached two models it had never got to, and both abort the process
at `temporal.rs:15` (**`upstream-issues.md` D1**). Unguarded that took their slices with them:
**shard 1 wrote 0 rows of 465 and shard 3 wrote 175.** `catch_unwind` around `survey_one` now records
`outcome = panicked` with the message and rebuilds the session, exactly as `fidelity_msl` does.

**D1 therefore blocks the survey as well as the fidelity report** — it is not only a reporting
defect.

#### The corpus grew by 165, and it is accounted for exactly

2,626 → 2,791, with **nothing removed**: `ModelicaReference` (147) and `ObsoleteModelica4` (18),
neither of which the old three-root list included. That closes the open question this page recorded
when `msl_roots` collapsed to `["msl"]` — those libraries are *in* the corpus now, and the count
moved by exactly their contribution. `msl_libraries` is also **verified end to end** at last, naming
all five libraries with their versions.

#### The result: 409 regressions and 59 improvements against 0.9.20

On the 2,626 models both artifacts share:

| transition | count |
|---|---|
| `success` → `failed:ToDae` | **353** |
| `success` → `failed:Flatten` | 32 |
| `success` → `failed:Instantiate` | 10 |
| `success` → `failed:Typecheck` | 8 |
| `success` → `no_result` | 4 |
| `success` → `panicked` | 2 |
| `failed:ToDae` → `success` | 48 |
| `failed:Typecheck` → `success` | 11 |

**409 models that compiled on 0.9.20 no longer do; 59 that failed now succeed.**

#### Why this is not an artifact of repair 1 — checked, because it would be the obvious suspect

Removing the `model_names()` call also removed a *warm-up*: the old run built and cached a complete
standard resolution before compiling anything. So the regressions had to be tested against the
alternative that they are caused by its absence.

**The failure messages settle it.** Not one names an unresolved reference:

| count | message |
|---|---|
| 78 | `unbalanced model` |
| 40 | `unsupported runtime operator \`sample\`` |
| 34 | `unsupported model algorithm in canonical DAE` |
| 26 | `unsupported Flat semantic owner \`function conditional\`` |
| 22 | `unsupported equation form` |
| 18 | `unsupported Flat semantic owner \`clocked equation ownership proof\`` |
| 16 | `canonical DAE construction rejected an invalid operation` |
| 14 | `unresolved clock schedule for \`clock\`` |

These are **feature-coverage and model-balance diagnostics from DAE construction**. The shape is a
re-implemented canonical DAE with narrower support for synchronous/clocked constructs, model
algorithms and function conditionals, plus a balance check 0.9.20 did not enforce — and an
`unbalanced model` rejection may well be a *correctness improvement* rather than a loss.

**What is not yet known** is which of the 409 are deliberate narrowings and which are defects. That
is the next question, and it is now answerable, which it was not this morning.

#### One test was pinned to the corpus size, and eight comments still are

`the_corpus_is_visible_unfiltered_and_opens_on_click` asserted the literal `2626` and so **failed on
a correct change**. It now derives the count from `docs/reports/msl-survey.csv` with the same parser
the header uses, plus a floor so a missing artifact cannot make it pass vacuously. Its sensitivity
to the number needs no separate proof: the pinned version failing against a 2,791-row corpus *is*
the falsification.

`model_list.rs`'s two module docs called it "the 2,626-row MSL corpus" — a present-tense definition
a reader would take as current — and now name no number at all.

**Eight mentions of 2,626 remain in `app.rs`, `doc_citations.rs` and `fidelity.rs`**, and are left
alone on purpose: each is an argument about *scale* at the time it was written ("2,626 models in one
widget", "2,626 rows of noise from a check that is simply wrong"), not a claim about the corpus's
present size. Re-pinning them to 2,791 would make them wrong again at the next consolidation, and
rewriting ten reasoning passages is a larger and more error-prone edit than the staleness costs.
Worth a sweep only if one of them is ever read as a definition.

### 11-06, step 10's `--ignored` run repaired itself — a lab-prose fix landed and nobody knew

**`updating-rumoca.md` step 10 includes `--ignored` for one stated reason** — *"the ignored set a
release may have repaired"* — and on its first run that is exactly what it found.

`ui_tests::lab_prose_after_a_table_wraps_to_the_panel_not_the_table` was ignored with the reason
*"no fix known: prose after a wide table inherits the table's wrap width"*, and its doc said **"Un-
ignore it when a fix lands."** It **passes**. The ignore is retired.

**Vacuity was checked before it was trusted**, because the test's own comment records that *three
earlier drafts passed vacuously* — a synthetic table narrow enough to fit the panel does not
reproduce the bug, which is why it reads the real `the-pipeline.md`. Inverting the assertion printed
the geometry: **590pt wide before the table, 591pt after, both 45pt tall.** Panel width on both
sides; had the prose still inherited the table's wrap width the two would differ. `rect` panics on a
missing label, so both paragraphs are proven to render.

**The cause is NOT established.** Candidates are the 0.10.4 rebase's egui pin and a layout change in
HRW; no bisect was attempted, and the test is the guard either way.

#### The prediction that was wrong, and why

This run was announced as *"one of the three will fail by design."* It did not. The `#[ignore]`
reason string said "no fix known", and that was read as a live fact — **it is a note from the day it
was written.** The doc two lines above it said the truer thing: an *acceptance* test, awaiting a fix.
An acceptance test that starts passing is the good outcome, and the only way to find out is to run
it, which is the whole argument for step 10 carrying `--ignored` at all.

### 11-06, the oracle ran — one specimen System Modeler rejects that Rumoca accepts silently

First oracle sweep over all 28 specimens: **19 simulated, 8 refused, 1 import-failed**, in 110
seconds. `scripts/oracle.wls` and the runbook section in `long-runs.md` make it repeatable.

#### Agreement on five rejections — these confirm Rumoca is right

| specimen | Rumoca | System Modeler |
|---|---|---|
| `UnclosedModel` | Parse **Failed** | import-failed |
| `MissingComponentClass` | Resolve **Failed** | refused |
| `UndefinedRef` | Resolve **Failed** | refused |
| `UnbalancedShaft` | Dae **Failed** — `2 equations, 3 unknowns` | refused |
| `OverDeterminedShaft` | Dae **Failed** — `3 equations, 2 unknowns` | refused |

An independent implementation rejecting the same five is what "oracle first" exists to buy: these
specimens are genuinely bad, and Rumoca's diagnosis is not in question.

#### ONE mismatch, not four — and the correction is the lesson

First reading called four specimens mismatches, on the rule *"System Modeler refuses and Rumoca
does not **fail**"*. That rule is wrong, because **in Rumoca's idiom `Flagged` IS a rejection** —
the problem is reported and the pipeline continues. Re-run on *"did the tool report a problem"*:

| specimen | Rumoca | both reject? |
|---|---|---|
| `DimensionMismatch` | Typecheck **Flagged**, downstream stages not reached | yes — not a mismatch |
| `TwiceDefined` | Structural **Flagged** `singular`; initialization states it cannot plan | yes — not a mismatch |
| `IncompatibleConnect` | Structural **Flagged** `singular` | yes; diagnosis quality is `upstream-issues.md` issue 2 |
| **`OverInitRc`** | **every stage `Ok`, Initialization `Ok`, no note** | **no — the one true mismatch** |

Checked exhaustively rather than by eye: of the 14 specimens Rumoca reports `all Ok` and the 9
System Modeler rejects, the intersection is **`OverInitRc` alone**.

**`TwiceDefined` was the clearest error of the four.** Its own header reads *"Breaks at: STRUCTURAL
ANALYSIS … the specimen that shows why counting is not enough, which is the whole reason matching
exists as a phase."* Rumoca's `singular` is the diagnosis the specimen was authored to provoke, and
it was nearly filed as a defect.

**This is the same trap, taken in both directions in one sitting.** Six `Flagged singular`
specimens that System Modeler *simulates* were correctly excluded for exactly this reason — and
then three `Flagged` specimens that System Modeler *refuses* were wrongly included. `Flagged` means
the same thing in both cases.

#### It is now ADMISSIBLE, and the chain produced its first result

`reports.md`: a mismatch is a finding only once fidelity is green for the model, or it may be *the
instrument lying rather than the compiler erring*.

`OverInitRc`, `TwiceDefined` and `IncompatibleConnect` were added to `fidelity::MODELS` — all three
reach `Dae` with IR, so the structural invariants have something to check — and all three come back
clean, with the specimen report now carrying `OverInitRc,Component,ok,,,0`. All 13 fidelity tests
pass, including the pinned `subjects_checked == MODELS.len()`.

So **survey → eligible, fidelity → trustworthy, oracle → finding** has delivered one
high-confidence upstream bug: `upstream-issues.md` **D2**, with System Modeler's own derivation of
the contradiction quoted verbatim.

`DimensionMismatch` stays in `FAILING_MODELS` alone: it stops at Typecheck with **no IR**, so it
yields no subject and cannot join `MODELS` without breaking that pinned equality — whose comment
says it must be rewritten deliberately rather than nudged. `TwiceDefined` was also missing from
`FAILING_MODELS` entirely and has been added.

#### Six `Flagged singular` specimens that System Modeler simulates are NOT mismatches

`BenchActuator`, `CapacitorLoop`, `CartesianPendulum`, `Drivetrain`, `GearWithBrake` and
`MotorWithBrake` are flagged singular by Rumoca and simulate cleanly in System Modeler. That looked
like a disagreement and is not: `Outcome::Flagged` means Rumoca produced usable IR and *continued* —
index reduction then runs, and `BenchActuator`'s own line reads `sorted after 1 round(s): 4 state(s)
→ 3`. Both tools resolve the singularity; Rumoca merely reports the intermediate fact. **Counting
these as findings would have put six false positives in front of a maintainer.**

### 11-06, triage of the 409 — about a fifth are 0.10.4 being RIGHT, and two thirds are one unported feature set

Buckets, over the 409 models that compiled on 0.9.20 and do not on 0.10.4:

| count | bucket | reading |
|---|---|---|
| **78** | balance check | **not a regression** — see below |
| **127** | synchronous / clocked features | capability gap |
| **89** | algorithm and function-body lowering | capability gap |
| **67** | expression and record forms | capability gap |
| 2 | D1 | **already fixed**; this artifact predates it |
| 46 | long tail | **where defects are likely** |

#### The 78 `unbalanced model` rejections are 0.9.20's defect, not 0.10.4's

**73 of the 78 do not satisfy `n_equations = n_states + n_algebraic` according to 0.9.20's own
published columns.** It reported `success` on systems its own numbers called unbalanced;
0.10.4 added a check and rejects them. Examples, with 0.9.20's figures:

```text
Modelica.Electrical.QuasiStatic.SinglePhase.Sensors.PotentialSensor   eq=6  states=1 alg=7
Modelica.Magnetic.QuasiStatic.FluxTubes.Basic.Ground                  eq=2  states=0 alg=5
```

**Bounded honestly.** `n_equations = n_states + n_algebraic` is the test used here, and 12 of the 78
are `QuasiStatic`, where complex-valued connectors may count differently — so the confound touches
at most 12 and the robust floor is **61 genuinely imbalanced**. The group is dominated by
`Modelica.Mechanics` (65 of 78), not QuasiStatic, so the confound is marginal rather than
structural. An earlier reading of this group cited only QuasiStatic examples and so looked like the
opposite.

**The five that WERE balanced by 0.9.20's counts are the real suspects** — the candidates for
0.10.4's check being wrong:

```text
Modelica.Mechanics.MultiBody.Forces.ForceAndTorque              eq=245 states=0 alg=245
Modelica.Mechanics.MultiBody.Forces.WorldForce                  eq=197 states=0 alg=197
Modelica.Mechanics.MultiBody.Forces.WorldTorque                 eq=197 states=0 alg=197
Modelica.Mechanics.MultiBody.Parts.Rotor1D                      eq=228 states=7 alg=221
Modelica.Thermal.HeatTransfer.Components.ThermalCollectorMatrix eq=2   states=0 alg=2
```

#### 283 of the 409 are three capability gaps, and they read as unported rather than broken

Every message in buckets B, C and D says **`unsupported`** or **`unresolved clock`** — the phase
naming a limit, not failing an invariant. They cluster so cleanly that they look like work not yet
carried across the 0.10.0 re-implementation rather than defects in what was:

- **Synchronous / clocked (127)** — `sample`, `delay`, `Clock`, clock schedules, clocked ownership
  proofs. Modelica 3.3's synchronous language, essentially absent.
- **Algorithms and function bodies (89)** — `model algorithm in canonical DAE`, `function
  conditional`, call statements, element assignment, branch selection, shape proofs.
- **Expression and record forms (67)** — `equation form`, `derivative`/`change`/`pre`/`edge`
  expressions, `native table constructor`, record-array slices and output assembly.

**This is the most useful single fact for study planning.** Clocked and algorithm-bearing MSL models
do not reach a DAE on 0.10.4, so they cannot be studied at DAE level or below until that work lands.

#### The 46-model tail is where to look for defects

Nothing here is a feature name; the phrasing is internal:

| count | message |
|---|---|
| 9 | `canonical DAE construction rejected an invalid operation` |
| 6 | `flat variable is missing structured identity` |
| 9 | `conditional component … requires parameter expression` (4 distinct components) |
| 4 | `unresolved Flat reference …` |
| 4 | `… could not be compiled` (no reason given) |
| 4 | `unevaluable array dimensions for …` |
| 2 | `unordered foreign library state` |

**`rejected an invalid operation` (9) is the most suspicious**: it reads as an internal invariant
refusing something, not as a user-facing limit. `could not be compiled` (4) gives no reason at all,
which is its own small finding.

#### Two things this artifact gets wrong about today

1. **It contains 2 `panicked` rows** — the D1 models. D1 is fixed, so a fresh survey would record
   them as ordinary `failed:ToDae`. The artifact is one fix stale and a re-run is ~4 minutes.
2. Nothing here is adjudicated against an independent implementation. The 5 balance suspects, the 9
   `invalid operation` models and the 4 `could not be compiled` models are the **oracle candidates**:
   System Modeler either simulates them or it does not, and that answer is worth more than any
   reading of the messages.

### 11-06, the fidelity sweep completed and reconciles — 2,780 measured, 0 violations, 7 checks

**The first fidelity sweep whose arithmetic closes.** Over the repaired 2,791-model corpus:

| | |
|---|---|
| report rows | **2,780** |
| `aborted:proc-ceiling` | 11 (the Spice3 / EngineV6 / FullRobot family, as before) |
| 2,780 + 11 | **= 2,791, the whole corpus** |
| checks run | **F2, F3, F5, F6, F7, F9, F10** — F4 excluded, still blind |
| violations | **0** |

**The zero now means something.** This morning a zero carried no information because four of the
checks had never been shown able to fire. All seven in this run have must-fire coverage, F10's
proving it fails three distinct ways — so 0 violations over 2,780 models is a result rather than an
absence. **F10 ran for the first time at corpus scale**, having been silently dropped from the 11-05
sweep by the `-OnlyChecks` string.

No `panicked` rows, which is D1's fix showing: the two models that aborted the process this morning
compiled and were measured.

#### And a third cause of a false `ok`, found by the same reconciliation

The first count was **2,778** rows against 2,780 profiled `ok` — two short, exactly the shape that
exposed D1. The missing pair:

```text
ModelicaReference.Classes.'function'.'function partial application'
ModelicaReference.Classes.'function'.'pure function'
```

**Quoted Modelica identifiers containing spaces.** `measure-fidelity.ps1` passed the name raw in
`-ArgumentList`, so `Start-Process` split it on the space, the runner received a truncated name,
matched no model, wrote no row — and **exited 0**, so the `crashed:exit-<n>` guard added hours
earlier could not see it.

Both are fixed: the name is quoted, and the two models re-run and are now in the report.

**Three different causes have produced the same false `ok`** — a panic, a non-`Compiled` event, and
an unmatched name. Each previous fix was cause-specific. The new `no-row` verdict asks the only
question that generalises: **is the row there?** A clean exit with no row is no longer `ok`.

#### The new guard was vacuous on first write, and falsification caught it

It began as `if ($verdict -eq "ok" -and (Test-Path $Out))`. When the runner declines every model it
returns *before creating the report*, so `Test-Path` was false, the condition short-circuited, and
**the check went silent in precisely the case it was written for** — a missing report is the
strongest possible evidence of a missing row, and the guard treated it as grounds for saying
nothing. Driven with a nonexistent model name, it scored `ok`.

Corrected, and verified both ways: a nonexistent model now records `no-row`, and a real one still
records `ok`. **That is the second guard today whose first version could not fail** (the other
reported `ok` for everything via an empty `ExitCode`), and both were written while fixing this
exact class of defect.

### 11-06, a key spelled `(tree root)` cannot navigate back — and F7's must-fire test now depends on it

Writing F7's must-fire coverage required an input the path grammar gets **wrong**, because F7 is a
round-trip property over `describe_path` / `parse_path` / `navigate` and no input the grammar handles
correctly can falsify it. One exists:

`bridge::key_needs_quoting` is `k.is_empty() || k.contains(['.', '[', ']', '"', '\\'])`. The string
`(tree root)` contains none of those, so a JSON key spelled that way is written **bare** — and
`describe_path` returns exactly `(tree root)` for the *empty* path. The description of
`[Key("(tree root)")]` is therefore indistinguishable from the root's, and navigating the parsed
description lands on the whole tree instead of the child. **F7 catches it**, which is the proof that
F7 can fire.

**Latent, not active.** No Modelica IR key is spelled `(tree root)`; the corpus run reads 11 stage
IRs per model across 2,626 models and has never produced one. The collision is reachable only by a
hand-written document.

**The pair, and it must be done as a pair.** Adding the sentinel to `key_needs_quoting` is a
one-line fix *that breaks F7's must-fire test* — the defect is currently load-bearing, which is not
a state to leave unremarked. So either:

- **make `check_f7` injectable first** — parameterise the round trip over its describer so a test
  can supply a deliberately broken one — then fix the grammar and replace the test's input; or
- **fix the grammar and accept that F7 loses must-fire coverage again**, which puts it back in the
  category this morning's work existed to empty.

The first is the real repair. Doing the second half alone is the trap.

### 11-05, the survey artifact stopped recording which MSL it surveyed

Regenerating `msl-survey.csv` moved `msl_roots` in the sibling `.meta.json` from
`["Modelica 4.1.0", "ModelicaServices 4.1.0", "Complex.mo"]` to **`["msl"]`**. The meta file exists
to make the artifact auditable, and the single question it most needs to answer — *which MSL version
is this a map of?* — it no longer answers.

The cause is not the rebase. `msl_roots()` was deliberately consolidated to one root,
`hrw/vendor/msl`, and `no_second_copy_of_the_msl_root_list` now guards that shape; the survey writes
each root's file name, so one root means one uninformative name. The versions are still **right
there** as subdirectory names under `vendor/msl`, so the fix is to record the libraries found under
the roots rather than the roots themselves.

**And an open question that consolidation raises.** `vendor/msl` also contains
`ModelicaReference 4.1.0` and `ObsoleteModelica4.mo`, neither of which was in the old three-entry
root list — yet `models_surveyed` and `models_available` are **2,626 before and after**, and the
`outcomes` block is identical to the model. Either Rumoca's loader selects libraries by name under a
root and those two are correctly excluded, or two libraries are now in scope and absent from a survey
that calls itself complete. **Not measured.** It needs one look at what the loader does with a root
directory, and whichever answer comes back, `models_available` should be made to state it rather
than leaving the two readings indistinguishable.

| date | defect | caught by | note |
|---|---|---|---|
| 11-05 | the survey's metadata no longer names the MSL version it surveyed | reading the `.meta.json` diff rather than only the CSV's | a consequence of the deliberate one-root consolidation; the versions remain available as subdirectory names |
| 11-06 | **`ArrayFamily` was committed with no System Modeler verdict** — the oracle report held 28 rows for 29 specimens | reading the directory listing against the artifact, while closing the rebase | *oracle-first* was a rule with no mechanism, and the absence is the invisible kind: a specimen with no row contributes nothing, so a missing adjudication and a specimen that was never authored look identical. Now `fidelity::every_specimen_has_an_oracle_verdict`, with the predicate split out so its must-fire needs neither System Modeler nor a file |
| 11-06 | the promoted sidecar told maintainers the run covered **`F1-F9`** | reading the sidecar that `promote_run` had just written | every word wrong by then: F1 retired into `check_f2`, F8 a measurement rather than a check, F10 absent from the range and present in the run. It now derives from `fidelity::CHECKS` and carries a `checks` array. **This is the artifact a maintainer reads without the conversation around it**, which is the whole argument for the `not_checked` field one line above it |
| 11-06 | two documents cited `solve_lowerings_names_is_documented_as_slots_not_imports`, renamed the same day | grep, while looking for something else | `qualified_citations_resolve` scans six documents and neither `tech-debt.md` nor `unattended-run-log.md` is one, so the rename gated green with two dead pointers behind it. **Widening it was measured and rejected** — see the entry below |
