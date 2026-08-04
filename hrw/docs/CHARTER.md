# HRW Project Charter

**Purpose:** the project's purpose, scope, method, and binding decisions.
**Status:** authority — the most binding document here. Amended deliberately, never drifted
from.
**Read when:** any design question whose answer might contradict a settled decision. Do not
re-litigate one of its decisions in-session; amend the charter or accept it.

**How Rumoca Works — a mastery project in the mathematics and computer science of modeling and simulating deterministic systems**

*Adopted July 2, 2026. Amended to v1.1 on July 18, 2026 (Decision 2 rewritten: specimen systems changed from Chicago/Bloomington-Normal infrastructure to robot subsystems, for time-constraint and Purdue-alignment reasons; consequential edits throughout Sections 2, 4, 5, 6, and 7). **Amended to v1.2 on August 4, 2026: Decision 7 — Accuracy — added at Doug's direction, after fictions were found in the observatory's log and UI that the fidelity programme was structurally unable to detect. It is the first decision to state a rank against the others.** Doug, principal. This charter is the HRW analog of the HCW Project Constitution: a standing statement of purpose, scope, method, and binding decisions, amended deliberately rather than drifted from.*

---

## 1. Purpose and central proposition

The purpose of HRW is mastery of the mathematics and computer science required to model and simulate deterministic physical systems. This is the top short- and medium-term learning priority, undertaken in preparation for and alongside the Robotics MS at Purdue beginning Fall 2026.

The central proposition — the bet the whole project rests on — is a proxy claim:

> **When I completely understand the mathematical and computer-science hows and whys of the Rumoca compiler pipeline, as exercised by a selected set of specimen systems, I will thereby understand the necessary math and CS.**

Rumoca is fit for this role because a Modelica compiler's pipeline is a physical enumeration of the subject itself. Parsing and intermediate representations exercise compiler CS; flattening exercises the semantics of component-based physical modeling; matching and BLT decomposition exercise graph algorithms (bipartite matching, Tarjan SCC) and sparse linear-algebraic structure; Pantelides index reduction exercises DAE theory; initialization and BDF integration exercise numerical analysis, stiffness, and automatic differentiation. Understanding is demonstrated per specimen, not claimed in the abstract: the unit of proof is a specific model traced through a specific phase with the intermediate representations observed and explained.

Rumoca's own architecture cooperates with this purpose. It is a multi-crate Rust workspace with strict phase boundaries (parse → resolve → typecheck → instantiate → flatten → DAE), IR crates that are pure data, and architectural invariants documented in numbered SPEC files. The crate DAG is the syllabus; the spec directory is the pattern language. This is a designed system in the Rickover sense — consistent patterns from which details can be inferred — and the design rules are written down.

## 2. Scope

**In scope (near/medium term).** Deterministic continuous-time and hybrid (event-driven) systems; the full Rumoca pipeline from Modelica source to simulation results; the linear algebra, graph theory, numerical analysis, and compiler CS that the pipeline embodies; differential validation against Wolfram System Modeler; instrumentation tooling built for observation of the above.

**Deferred, not abandoned.** Stochastic methods (stochastic calculus, estimation, filtering, stochastic control), the geometry of rotation (SO(3)/SE(3) — the parked first spine of the robotics curriculum), and optimization. These arrive via Purdue coursework on their own schedule. The deterministic layer built here is their substrate, not their competitor: an SDE is this project's ODE core plus a diffusion term; Kalman-Bucy is a linear system plus Gaussian uncertainty propagation. Mastering the deterministic layer first is prerequisite-building, not deferral.

**Out of scope (near/medium term).** Web deployment of any kind. No pages are created or published; Astro and the islands/WASM embedding architecture are shelved intact for HCW's eventual public face. All tooling builds native. The deliverable of this project is understanding — specimen models, trace logs, the archetype catalog, and possibly upstream contributions — not artifacts for an audience. The production process is the education.

## 3. Binding decisions

**Decision 1 — Priority.** The top short/medium-term learning priority is the math and CS of modeling and simulating deterministic systems, with complete understanding of Rumoca on the specimen set as the operational definition of success.

**Decision 2 — Specimen systems (amended v1.1).** Specimens are drawn from the subsystems of one or more physically owned robots, built from open-source home robot kits and modeled in Modelica from their published design documents. Kit eligibility criteria, in priority order: (a) open hardware design documents — CAD with computable mass properties, and ideally a maintained URDF, whose link inertial blocks and joint definitions constitute a pre-extracted Modelica parameter set; (b) open controller logic; (c) actuator transparency — sealed hobby servos are black-box muscles inside an open skeleton, so serial-bus servos with documented registers and motor specifications are preferred, and at least one fully glass-box actuator (an open-source FOC brushless controller on a bench rig) is included so the electrical archetypes have a specimen with no opacity at all. The candidate portfolio, final selection pending: an SO-101–class open arm (the LeRobot ecosystem specimen; serial open chain), a parallelogram-linkage arm in the MeArm/EEZYbotARM class (the closed-chain specimen), a SimpleFOC-class bench actuator, and later a differential-drive base (TurtleBot 3–class) or open quadruped (Mini Pupper–class) as coursework indicates. Multirotor drones are deliberately excluded from this layer — mechanically a single rigid body, their richness (SO(3) attitude, stochastic estimation, identified rather than derived aerodynamics) lies entirely in the deferred scope; a Crazyflie-class platform is queued for that layer. Rationale for the amendment: the specimens now serve the Purdue robotics program directly, doing double duty for the compiler curriculum and the coursework, within available time.

**Decision 3 — Authoring toolchain.** Modelica models are authored in Wolfram System Modeler. This serves an independent professional goal — skilled use of Wolfram's applications — and is accepted despite System Modeler being a black box, because Decision 4 renders the opacity harmless.

**Decision 4 — Compilation and simulation toolchain.** Models are compiled and simulated with Rumoca, the glass box. Every specimen therefore runs through two independent compilers, constituting a standing differential-testing rig. Disagreement between the toolchains is the most valuable event the setup can produce: it is either a Modelica semantic subtlety (learning), a Rumoca defect (contribution), or undisclosed System Modeler behavior (exactly what the glass box exists to see around). Once Rumoca's pipeline is transparent, System Modeler ceases to be opaque in any way that matters — it becomes a reference oracle, a well-upholstered instance of the same pipeline.

**Decision 5 — No web deployment.** As stated in Scope. Native builds only; Astro shelved; nothing foreclosed.

**Decision 7 — Accuracy, and its rank (adopted v1.2, August 4, 2026).** *Numbered 7 and placed here for reading order; adopted after Decisions 1-6.* **Everything the instrumentation displays must be traceable to something the compiler actually did on the run being observed.** Absence is stated, never filled with a plausible substitute; a view derived by the observatory rather than recorded from the compiler declares itself as derived; and the ordering, nesting and attribution of a log are claims about what happened, held to the same standard as its contents.

**Rank: accuracy outranks every other consideration in the instrumentation** — features, polish, performance, completeness of a pane, and the cost of a change to the Rumoca crates. Where the two conflict, the Rumoca change is the cheap option, because Decision 6's glass box is worth nothing if the glass distorts.

**Rationale, from the failure that produced this decision.** On August 4, 2026, after a corpus-scale fidelity programme had reported 2,614 models green with zero violations, Doug walked the first two curriculum tours and found the observatory's log and UI carrying fictions: a named phase that does not exist, phases re-run and presented as the compilation, and decomposition blocks rendered for a system the compiler had refused to decompose. **The fidelity programme could not have caught any of them** — it verifies that a structure matches what Rumoca produced, and a fabricated structure is well-formed while a replay's output is identical by construction. Doug's statement of the principle:

> *"My top priority continues to be education. HRW is merely a tool to help me learn. In order for me to learn about Rumoca, HRW must accurately represent Rumoca."*

**Consequence, binding: the work stops for accuracy whenever accuracy requires it** — *"we will pause and fix code as often as necessary in order to deliver accuracy."* A day removing fictions is curriculum work, not a detour from it. This is a standing authorisation and does not need to be re-sought.

**Why it belongs in the charter rather than a rules file.** Section 1's central proposition is a *proxy claim*: understanding Rumoca yields the underlying math and CS. **The proxy holds only while the observatory is faithful.** An instrument that misrepresents the compiler does not slow the bet down — it silently substitutes a different subject, and the learner cannot tell which parts were substituted. Accuracy is therefore a precondition of the charter's purpose, not a quality attribute of its tooling.

**Decision 6 — Instrumentation.** A Rust/egui observatory application, developed in Visual Studio Code with the Claude Code extension, with the VS Code debugger treated as a first-class learning instrument. The app loads a System Modeler-authored model, compiles it via Rumoca linked as a library (git/path dependency on the Rumoca workspace, since v0.8+ distributes binaries via GitHub releases rather than crates.io), and runs simulations — so that a breakpoint can be set inside a compiler phase while it processes a specimen. Your model, their phase, your breakpoint: the curriculum in one gesture.

## 4. Method

### 4.1 Archetype-first specimen selection

Specimens are not selected by first understanding a real system deeply and then modeling it. They are selected the other way around. Each compiler phase is triggered by a specific equation-structure pathology, and the catalog of pathologies is small, closed, and system-agnostic: hierarchy and connection (flattening); algebraic loops — meshes with instantaneous constitutive relations (BLT simultaneous blocks); constraints coupling state variables — rigidly coupled storage elements (index > 1, Pantelides); implicit or over/under-specified conditions at t = 0 (initialization); conditional structure — thresholds, `when` clauses (events); coupled fast and slow dynamics (stiffness).

The method is to learn this archetype catalog first, then dress each archetype in specimen clothing — and, at the workbench, to examine the robot structure-first, asking of each subsystem not "what does the datasheet say" but "what kind of dynamical object is this." The archetype catalog is the field guide and the disassembled kit is the field: an open serial chain as clean ODE hierarchy; a parallelogram linkage as the loop-closure constraint (index 3); the servo's inner control loop closed around instantaneous relations as an algebraic loop; gearbox stick-slip, joint limits, and ground contact as `when`-clause structure; motor winding dynamics against link dynamics as the microseconds-to-tenths-of-seconds stiffness span. A robot is a denser archetype generator per cubic centimeter than any infrastructure system — every pathology in the catalog is present within arm's reach.

HRW specimens begin as structural toys — five to twenty lines of Modelica with placeholder parameters, written directly from the archetype, whose only job is to force a phase to do observable work. They are placeholders that get promoted: as each kit is built and examined, the toy's fake parameters are replaced with values from the kit's URDF and CAD mass properties, and the toy graduates into a validated model of the physical robot on the bench. Mechanical components are hand-built in a small planar (2D) mechanics library written from scratch in the portable subset — revolute joint, rigid link, ideal motor, friction, contact — rather than drawn from MSL MultiBody, which is the most demanding package a young compiler can ingest and would turn arcs into blocked-on-upstream investigations. Planar mechanics exhibits every archetype the 3D version does without requiring rotation-group machinery, keeping SO(3) properly parked per Decision 1 and providing the natural bridge when that spine un-parks.

### 4.2 The curriculum: one arc per phase

The curriculum has seven arcs, each anchored to a specimen designed to make that phase do nontrivial work, each producing a written chapter in HCW format discipline (every fact earns its place as evidence) plus a runnable specimen and a trace log of the IR before and after the phase.

1. **Parse → Resolve → Typecheck.** Specimen: a single rotating inertia driven by an ideal torque source — one joint of the arm, maximally simplified. Attention on AST shape, name resolution, span/diagnostic invariants.
2. **Instantiate → Flatten.** Specimen: a motor–gearbox–link drivetrain chain crossing electrical, rotational, and translational domains — the multi-domain connector semantics Modelica exists for. Where object orientation dies and equations are born: connector expansion, flow-sum generation, modifiers. Diff the output against a hand-flattened prediction.
3. **Matching and BLT.** Specimen: an ideal proportional feedback loop closed around instantaneous relations (the servo's inner loop, idealized), yielding a genuine simultaneous block. Maximum matching, Tarjan SCC. Preferred visualizations: fixed-layout bipartite incidence view; BLT as a spy plot.
4. **Index reduction (Pantelides).** Specimen: a four-bar / parallelogram linkage — the loop-closure constraint that makes multibody mechanics the index-3 domain Pantelides was invented for. Note the boundary fact that motivates the specimen: an open chain in joint coordinates is a plain ODE; the pathology enters only with closed chains or absolute coordinates.
5. **Initialization and IC planning.** Specimens: the resurrected 2025 RC/RL blow-up case, plus a linkage starting from an implicitly defined static equilibrium. The arc where the original bug lived, and where an early compiler most needs contributions.
6. **Events and hybrid structure.** Specimen: stick-slip friction in a joint, plus joint-limit stops — discrete events where the structure of the equations changes between sticking and sliding; step-mode plotting so discontinuities render as discontinuities.
7. **The simulation core.** Specimen: the bench actuator — motor winding dynamics (fast) coupled to mechanical load dynamics (slow), the canonical stiff pairing. BDF integration, exact AD Jacobians, mass matrices, solver fallback. Closes the loop back to arc 5's bug and forward to interactive re-simulation.

Codegen and templates (the MiniJinja rendering path) is studied when web deployment un-shelves; until then it is read for architecture, not mastered.

### 4.3 Differential-testing protocol

Definition of done for every specimen: compiles and runs equivalently in both System Modeler and Rumoca. Two disciplines follow. First, write to the portable Modelica subset — no Wolfram-flavored conveniences; Rumoca is tested against MSL 4.1.0 and is younger than System Modeler, and the constraint is a feature, forcing the modeling toward the standard's core semantics. Second, fix the comparison protocol before the first comparison: identical solver tolerances, identical initial conditions, explicit `experiment` annotations, and an agreed agreement metric (relative error on state trajectories; event-time differences). Sharper cross-checks are available through the Wolfram `SystemModel*` family — linearizing the same specimen in both toolchains and comparing A-matrices beats eyeballing trajectories, and exercises the Wolfram Language goal simultaneously.

### 4.4 The observatory

Shape follows the pipeline: a file picker over the System Modeler export directory; a pipeline panel with one stage per phase, each exposing its IR to a generic serde-value tree inspector (the single highest-value widget — one inspector, pointed at every stage); an egui_graphs pane (petgraph-backed; hierarchical layout) for connection and dependency structure; custom-painter views for the bipartite matching and BLT spy plot; and an egui_plot pane fed by simulation buffers (immediate mode matches the step-and-render loop; ring-buffer or decimate for dense stiff output; linked axes for stacked state plots).

Engineering conventions: Rumoca linked as a library (primary) with a load-IR-from-JSON import path retained (secondary, for robustness across version churn); compilation and simulation on a worker thread with results over a channel, keeping the UI live and the debugger usable; breakpoints in actions, never in the per-frame paint path; `[profile.dev.package]` overrides to keep debug info on the crates being studied while raising opt-level on numerical kernels if debug-build simulation drags; rust-analyzer + CodeLLDB as the debug stack.

The observatory is built incrementally in curriculum order — the arc-1 version is only the file picker and the AST tree view — so the app's git history is itself a record of the curriculum.

## 5. Relationship to adjacent projects

**To Purdue.** As of v1.1 this is the primary adjacency: the specimen robots serve the MS coursework directly, and the deterministic modeling competency is the substrate for the curriculum's later layers (estimation, control, planning). The archetype catalog remains the join table — compiler phases on one side, now robot subsystems rather than city geography on the other. Rerun (rerun.io), the egui-based robotics visualization tool co-owning egui_plot, is noted as the likely convergence point of this tooling lineage and the robotics toolchain. The Crazyflie-class drone, PX4/ArduPilot ecosystem, and SO(3)/stochastic material are the queued specimens of the deferred layer.

**To HCW.** Demoted by v1.1 from instrument supplier to deferred beneficiary — shelved by the same charter mechanism that shelved Astro, intact and unforeclosed. The archetype catalog remains domain-blind: everything learned on robot specimens transfers to infrastructure specimens whenever HCW resumes, and the structure-first observation mode developed at the workbench is the same mode that will eventually walk the two cities. Astro, WASM embedding, and the published simulation site remain HCW's long-term architecture, untouched.

**To Rumoca upstream.** Contributions are a welcome byproduct, not an obligation. The differential rig and the initialization arc are the most likely sources. Contribution work follows the project's own gates (SPEC-governed invariants, MSL parity tests, the `rum` developer CLI).

## 6. Parked items

Three items are parked pending the principal's decision: the archetype catalog skeleton (entries: structural pathology, triggered phase, minimal toy sketch, "spotted on the robot" slot, promotion candidate) — deliberately held for further thought before drafting; final selection and acquisition of the robot kits from the Decision 2 candidate portfolio; and whether HRW enters the daily briefing as a fourth thread or remains its own conversation track.

## 7. First action and amendment

The natural first build, when execution begins: the observatory's arc-1 skeleton — Rust workspace, eframe shell, file picker, AST tree inspector — pointed at the trivial single-inertia specimen authored in System Modeler and compiled through Rumoca as a linked library, run once under the debugger with a breakpoint in the parser to prove the whole instrument end to end. Note that this first action does not wait on kit acquisition: the arc-1 through arc-3 toys are writable from the archetypes alone, so the observatory and the kit orders can proceed in parallel.

This charter is amended the way it was written: by explicit decision, recorded here, not by drift. The proxy claim in Section 1 is the load-bearing wall; if it ever stops being true — if Rumoca stalls, or the pipeline proves too narrow a lens — the charter is revisited from Section 1 down, not patched from Section 6 up.
