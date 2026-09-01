# HRW Project Charter

**Purpose:** the project's purpose, scope, method, and binding decisions.
**Status:** authority — the most binding document here. Amended deliberately, never drifted
from.
**Read when:** any design question whose answer might contradict a settled decision. Do not
re-litigate one of its decisions in-session; amend the charter or accept it.

**How Rumoca Works — a mastery project in the mathematics and computer science of modeling and simulating deterministic systems**

*Adopted July 2, 2026. Amended to v1.1 on July 18, 2026 (Decision 2 rewritten: specimen systems changed from Chicago/Bloomington-Normal infrastructure to robot subsystems, for time-constraint and Purdue-alignment reasons; consequential edits throughout Sections 2, 4, 5, 6, and 7). **Amended to v1.2 on August 4, 2026: Decision 7 — Accuracy — added at Doug's direction, after fictions were found in the observatory's log and UI that the fidelity programme was structurally unable to detect. It is the first decision to state a rank against the others. Amended to v1.3 on August 5, 2026: Decision 8 — The instrument assumes the reasoner — recording the noun/verb formulation as a guiding principle and adopting its consequence as binding on what UI gets built. Amended to v1.4 the same day: Decision 9 — Minimize learning friction — stating the hierarchy education → accuracy → low friction, and that accuracy outranks friction where they conflict. Amended to v1.5 on August 24, 2026: Decision 7 gains a second justification — accuracy and consistency are preconditions of *Claude's* ability to reason about and maintain the codebase, not only of Doug's learning — and consistency is stated as a test subordinate to what it serves. Amended to v1.6 on September 1, 2026: Decision 10 — The documents are constitutional; conclusions are not rules — after a document review found that the only genuine rule-versus-rule contradiction in the governing documents existed solely because an earlier conversational conclusion had been written down as a rule. Amended to v1.7 the same day: Decision 11 — Four documents, and what decides where a thing goes — adopting Doug's division (his decisions in the charter, Claude's craft in CLAUDE.md, procedure in running-things.md, history in DECISIONS.md) and the who-authored-it test that routes between them. Amended to v1.8 the same day: Decision 12 — three standing prohibitions on the observatory's own code, moved here from `CLAUDE.md` under Decision 11's test, since each was Doug's ruling sitting where ordinary maintenance could erode it. Amended to v1.9 the same day: Decision 13 — constitutional content is timeless and principled — after Decision 2's candidate robot portfolio was found already falsified, seven weeks old and undetectable by any checker, demonstrating that a clause can pass Decision 10 and still rot. Decision 2 was rewritten under it the same day, dropping the candidate kit portfolio, seven product names, the URDF technique note and an amendment rationale this log already carried; the three clauses that bind — physically owned robots, glass-box eligibility, and the multirotor exclusion — are unchanged in force.** Doug, principal. This charter is the HRW analog of the HCW Project Constitution: a standing statement of purpose, scope, method, and binding decisions, amended deliberately rather than drifted from.*

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

**Decision 2 — Specimen systems (amended v1.1).** Specimens are drawn from the subsystems of one or more physically owned robots, built from open-source kits and modeled in Modelica from their published design documents. Kits must be glass-box enough to teach: open hardware design documents with computable mass properties, open controller logic, and actuator transparency — with at least one fully transparent actuator included, so the electrical archetypes have a specimen with no opacity at all. Multirotor aircraft are excluded from this layer; mechanically a single rigid body, their richness lies entirely in the deferred scope.

**Decision 3 — Authoring toolchain.** Modelica models are authored in Wolfram System Modeler. This serves an independent professional goal — skilled use of Wolfram's applications — and is accepted despite System Modeler being a black box, because Decision 4 renders the opacity harmless.

**Decision 4 — Compilation and simulation toolchain.** Models are compiled and simulated with Rumoca, the glass box. Every specimen therefore runs through two independent compilers, constituting a standing differential-testing rig. Disagreement between the toolchains is the most valuable event the setup can produce: it is either a Modelica semantic subtlety (learning), a Rumoca defect (contribution), or undisclosed System Modeler behavior (exactly what the glass box exists to see around). Once Rumoca's pipeline is transparent, System Modeler ceases to be opaque in any way that matters — it becomes a reference oracle, a well-upholstered instance of the same pipeline.

**Decision 5 — No web deployment.** As stated in Scope. Native builds only; Astro shelved; nothing foreclosed.

**Decision 9 — Minimize learning friction (adopted v1.4, August 5, 2026).** *Numbered 9 for reading order; adopted alongside the refinement to Decision 8.*

**The hierarchy, in Doug's words:**

> *"My top priority for this HRW effort is my education. Therefore, a first corollary is that HRW accuracy is required. A second, and new, corollary is a UI design principle: we should strive in the HRW UI to minimize learning friction."*

So the three sit in a fixed relation, and it is worth stating plainly because they are sometimes mistaken for peers:

1. **The purpose** — Doug's education (Section 1, Decision 1). Everything else is derived.
2. **First corollary — accuracy** (Decision 7). An inaccurate instrument does not teach less; it teaches something false.
3. **Second corollary — low friction** (this decision). An accurate instrument that costs attention to operate spends the attention that was meant for learning.

**Friction is anything between Doug and the idea**: a click he should not have needed, a search for a tour named after a phase when he was thinking of a specimen, a question he has to type whose answer was already known, a rebuild he did not know he needed, a pane that makes him wonder whether it is telling the truth.

**Attention is the scarce resource, not time.** The tours rule already says so — *the scarce resource is Doug's attention per expectation* — and this generalises it past tours to the whole interface.

**RANK: ACCURACY OUTRANKS FRICTION.** Decision 7 ranks above everything in this repository, this decision included. Where reducing friction would cost accuracy — a summary that saves a click by asserting something unverified, a default that guesses — **accuracy wins and the friction stays.** The two rarely conflict, and naming the winner in advance keeps a plausible trade from being made quietly.

**This decision explains the same day's refinement to Decision 8.** The test *"would Claude answer this better?"* had `field_help` queued for deletion; Doug objected that *"What is this field for?"* has an answer known in advance and *"can therefore be answered much more quickly with tool tips and such."* **That objection was this principle, before it had a name** — a tooltip costs zero attention, and a typed question costs a context switch. Decision 8 asks *whether the answer is fixed*; **Decision 9 says why that matters.**

**Decision 8 — The instrument assumes the reasoner (adopted v1.3, August 5, 2026).** *Numbered 8 for reading order; adopted after Decision 7.*

**The guiding formulation, which predates this decision and is now recorded where it survives a clone:**

> **The noun is assembled by mouse; the verb is an unbounded utterance.**

Doug points, clicks and selects to assemble the *noun* — a specimen, a stage, a node, a frame, a tour. The *verb* is whatever he then says about it, in natural language, and it has no fixed vocabulary: *"explain this"*, *"why is this singular"*, *"demonstrate how Rumoca responds to a failure in the typecheck phase"*. **No menu can enumerate the verbs**, which is why the noun must be self-sufficient — complete enough that any verb can be applied to it.

**The consequence, stated by Claude on August 5, 2026 and adopted as binding:**

> **The picker does not need to be smart, because Claude is the smart part.** Grouping, filtering, prerequisite chains, faceted search — all of it is UI built to answer questions Claude answers better, from the same data, with the advantage of knowing what was actually asked. **Building it would be building a worse Claude.**

**So the test for any proposed UI feature is: IS THE ANSWER KNOWN IN ADVANCE?**

- **Fixed answer, stable question → build it into the UI.** *"What is this field for?"* has one answer that does not depend on what else is being asked. A tooltip delivers it in **zero seconds without breaking focus**, where asking Claude costs a context switch, a typed question and a wait. **For this class the UI is strictly better, not merely acceptable.**
- **Answer depends on the question actually being asked → leave it to Claude.** *"Why is this system singular?"* has no answer knowable in advance, because the useful answer depends on what Doug is trying to understand. Building UI for it means guessing the question, and guessing is what a reasoner is for.

*(Refined 2026-08-05, the same day, after Doug corrected a first formulation of this test. It read "would Claude answer this better, given the same data?" — which is a different and worse question, because it ignores **latency and focus**. Under it, `field_help` was listed for deletion. Doug: "Many questions such as 'What is this field for?' have answers which will always be known in advance and can therefore be answered much more quickly with tool tips and such." He is right, and the crude test would have removed something good.)*

**This is not an argument for a poor UI**, and the refinement makes that clearer. A noun must be *assemblable* — findable, selectable, unambiguous — and known facts about it should be **on screen, not on request**. What the decision forbids is UI that **infers**: ranking by importance, recommending what to look at next, summarising in place of the artifact, filtering by guessed intent. Those depend on the unasked question, and they are verbs wearing a widget.

**Worked example, the one that produced the decision.** With fourteen fixture tours, the obvious feature was a smarter picker: group by kind, order by pipeline phase, index by specimen, track prerequisites. All of it was dropped. What was built instead was a **catalogue Claude can read** and a **link form that opens a tour at a stop** — so the answer to *"demonstrate a typecheck failure"* is prose plus a composed tour linking into the fixtures, rather than a filter Doug has to operate. `docs/ideas.md` #62, #63, #64.

**Why this project can make this trade when others cannot.** Claude's presence here is continuous rather than occasional (`hrw-works-with-claude-not-without`). A feature that depends on the reasoner being present is a liability in software shipped to absent users and is simply the design here. **HRW is an education project with one user and one collaborator**, and the charter's Section 2 already says the deliverable is understanding rather than artifacts for an audience.

**The standing consequence: a periodic UI review asking what the interface is doing that Claude would do better.** Logged in [`tech-debt.md`](tech-debt.md); the first has not been run.

**Decision 7 — Accuracy, and its rank (adopted v1.2, August 4, 2026).** *Numbered 7 and placed here for reading order; adopted after Decisions 1-6.* **Everything the instrumentation displays must be traceable to something the compiler actually did on the run being observed.** Absence is stated, never filled with a plausible substitute; a view derived by the observatory rather than recorded from the compiler declares itself as derived; and the ordering, nesting and attribution of a log are claims about what happened, held to the same standard as its contents.

**Rank: accuracy outranks every other consideration in the instrumentation** — features, polish, performance, completeness of a pane, and the cost of a change to the Rumoca crates. Where the two conflict, the Rumoca change is the cheap option, because Decision 6's glass box is worth nothing if the glass distorts.

**Rationale, from the failure that produced this decision.** On August 4, 2026, after a corpus-scale fidelity programme had reported 2,614 models green with zero violations, Doug walked the first two curriculum tours and found the observatory's log and UI carrying fictions: a named phase that does not exist, phases re-run and presented as the compilation, and decomposition blocks rendered for a system the compiler had refused to decompose. **The fidelity programme could not have caught any of them** — it verifies that a structure matches what Rumoca produced, and a fabricated structure is well-formed while a replay's output is identical by construction. Doug's statement of the principle:

> *"My top priority continues to be education. HRW is merely a tool to help me learn. In order for me to learn about Rumoca, HRW must accurately represent Rumoca."*

**Consequence, binding: the work stops for accuracy whenever accuracy requires it** — *"we will pause and fix code as often as necessary in order to deliver accuracy."* A day removing fictions is curriculum work, not a detour from it. This is a standing authorisation and does not need to be re-sought.

**Why it belongs in the charter rather than a rules file.** Section 1's central proposition is a *proxy claim*: understanding Rumoca yields the underlying math and CS. **The proxy holds only while the observatory is faithful.** An instrument that misrepresents the compiler does not slow the bet down — it silently substitutes a different subject, and the learner cannot tell which parts were substituted. Accuracy is therefore a precondition of the charter's purpose, not a quality attribute of its tooling.

**Amended v1.5, August 24, 2026 — accuracy serves a second end, and consistency becomes a test.** Doug, after watching how the work actually goes: *"basing decisions upon HRW accuracy and consistency benefits not only my learning experience, but also your ability to reason about this project."* The education argument is strongest where he looks. **This one reaches every part of the codebase, including plumbing he will never read, because the project's continuity runs through Claude's comprehension — no human has yet needed to maintain HRW's code.**

**Consistency is subordinate to what it serves, and the test runs in both directions.** Doug asks how a change would affect his education, and refuses a consistency gain that costs it. Claude asks **whether the change improves or worsens his ability to reason about and maintain this code**, and refuses on the same terms. It means uniformity of *meaning* — one fact in one place, a claim the size of its mechanism — and **never symmetry of shape**: `compile_views` and `stage_views` are invalidated differently *because their sources differ*, and a rule that pushed them together would have made the code wrong. This generalises the refusal already recorded for the complexity lints, where a proxy for comprehension enforced against comprehension is worse than no proxy. Application, hazard and worked examples: [`../DECISIONS.md`](../DECISIONS.md), 2026-08-24.

**Decision 10 — The documents are constitutional; conclusions are not rules (adopted v1.6, September 1, 2026).** *Numbered 10 and placed here because it governs the same thing Decision 7 does — what is allowed to bind.*

**Only what binds in ALL cases belongs in a document a session must read before acting.** A conclusion reached in conversation is a **ruling about one case**. It is recorded in `DECISIONS.md`, which is a record and does not bind, or it is not recorded at all. **It is not promoted to a rule because it was interesting.**

**Doug's reasoning, September 1, 2026, and the danger is specific:**

> *"Whenever we reach an interesting conclusion, you tend to add another rule to your documents… That creates bloat in your documents. Worse, it risks creating contradictions. I'm an inconsistent human being. If you record as a rule every conclusion which we reach, inevitably, you will record a contradiction in your rules. And that means that you will have propagated my inconsistencies to you."*

**That is the mechanism, not a tidiness complaint.** Doug's judgements are made in a moment and are allowed to differ across moments — that is what being a principal means. Recording each one as a durable rule converts a sequence of reasonable rulings into a set of simultaneous constraints, some of which conflict. Claude then reasons from contradictory premises and cannot tell which binds.

**It is already evidenced.** On September 1, 2026 the only genuine rule-versus-rule contradiction in the governing documents was the two-pass tour model (August 15) against code-grounding (August 31). **It existed solely because the August 15 conclusion had been written down as a rule.** Every other contradiction found that week was a mechanism and its description drifting apart — a different failure with a different fix.

**The consequence, binding: when uncertain, ask for a ruling rather than write a rule.** Doug: *"If ever you are uncertain when needing to make a decision, you can simply ask me for a ruling. As you wrote, what we really need is a better feedback loop, not more rules."* A ruling costs one exchange and binds one case; a rule costs a line in every future session and binds every case, including the ones nobody considered.

**And this decision is why the charter is where it sits.** A rule about how rules are made is constitutional by construction, so it is amended deliberately and by Doug — which is also the only self-consistent way to adopt it, since adding it to a rules file would be an instance of the habit it forbids.

**Decision 11 — Four documents, and what decides where a thing goes (adopted v1.7, September 1, 2026).** *The filing rule Decision 10 needs in order to be actionable.*

**Doug's words: *"your decisions in the charter, my craft in CLAUDE.md, procedure in running-things.md, history in DECISIONS.md."***

| document | holds | authored by |
|---|---|---|
| **`CHARTER.md`** | purpose, scope, and standing **decisions** | **Doug** |
| **`CLAUDE.md`** | the working **craft** that binds every session | **Claude**, learned from defects |
| **`docs/running-things.md`** | **procedure** — commands, gates, diagnostic tells | either; it is operational |
| **`DECISIONS.md`** | **history** — what was ruled, when, and why | either; **it does not bind** |

**THE ROUTING TEST IS WHO AUTHORED IT.** *Did Doug decide it, or did Claude learn it?* A thing may
bind in all cases without being a decision of the principal: *"use the Edit tool, never a shell
heredoc"* binds always and is craft, learned from three silent corruptions — putting it in the
charter would make Doug the amender of Claude's working habits. Conversely *"do not optimise HRW to
widen test scope"* binds because **Doug said so**, and belongs here however operational it sounds.

**The failure this prevents is a decision of Doug's living in a rules file**, where Claude may
compress, generalise or quietly supersede it in the course of ordinary maintenance. A charter
decision can only be amended deliberately and by him — which is the whole point of having one.

**And the corollary that makes Decision 10 usable: when something binds but is neither a decision
nor craft, it is a RULING, and rulings go to `DECISIONS.md` or nowhere.** That is the default, not
the exception.

**Decision 12 — Three standing prohibitions on the observatory's own code (adopted v1.8, September 1, 2026).** *Moved to the charter under Decision 11's who-authored-it test: each was Doug's ruling, and each had been living in `CLAUDE.md` where ordinary maintenance could erode it.*

**(a) Do not optimise HRW to widen test scope** *(2026-07-31)*. Measurement showed HRW's **compile path**, not the checks, costs 30 s and 3.5 GB on a 4,193-equation model. Doug: *"we should not redesign worker.rs's compile path. Perhaps ever… If some models cannot be fidelity-tested within our limits, so be it."* The stage JSON trees, equation sheet, identifier index and animation frames **are the product**. Raising a timeout or memory ceiling when measurement justifies it is calibration, not optimisation, and is fine.

**Revisable on evidence, and that condition is part of the decision** *(2026-08-21)*: *"until we have an evidence-based reason to change our policy, let's maintain our prohibition."* **"Evidence-based reason" means bringing the evidence to Doug**, never concluding in-session that a measurement authorises proceeding. Splitting `worker.rs` into modules is **not** a redesign of the compile path and needs no permission; changing how the MSL session is loaded, cached or shared does. So `worker.rs` carries a boundary `app.rs` never had: **extract around the compile path, do not restructure it.**

**(b) Refactor for Claude's comprehension, not for a human's** *(2026-08-05)*. Doug: *"no human being has yet needed to comprehend or maintain any functions [in HRW]… We will refactor HRW functions when doing so improves your ability to comprehend or maintain those functions, or will improve your ability to test those functions and keep them correct."* **The trigger is one of those three and never a line count** — which is why the three complexity lints are declined: they encode a human-comprehension heuristic and would reward splitting a function to satisfy the lint.

**(c) The composition primitives are frozen** — one point-at, one follow, background. Multiple `follow` items and a third "compare" primitive were considered and deliberately not built. **Do not re-propose them from first principles**; a practical scenario demonstrating a need is what reopens this.

**Decision 13 — Constitutional content is timeless and principled (adopted v1.9, September 1, 2026).** *The second axis of Decision 10: that one governs whether a thing may bind, this one governs whether what it names can expire.*

**Doug's words, September 1, 2026:** *"Constitutional information and rules should be timeless and principled. Names of robot products are not timeless or principled."*

**The rule: a decision names a PROPERTY where the property is what binds, and names a THING only where the thing itself is the decision.** Decision 3 names Wolfram System Modeler, Decision 4 names Rumoca and Decision 6 names Rust/egui — correctly, because in each case swapping the named thing would change what was decided. Decision 2 named seven robot kits, which were merely *instances satisfying a criterion*; the criterion was the decision, and the instances were illustration wearing a decision's clothes.

**This is not a restatement of Decision 10, and the difference is what makes it worth adopting.** Decision 10 asks *"is this a rule, or a conclusion about one case?"* Decision 2 is a genuine rule — it binds specimen selection — so it **passes** Decision 10 and rotted regardless. A clause can bind in all cases and still name something with a shelf life. The two tests fail independently and must both be applied.

**It is evidenced, and the evidence is that the charter was already wrong.** Decision 2's candidate portfolio was falsified within seven weeks: Doug's first robot was ordered September 2026 and is not among the seven kits named. **Nothing in the repository could detect it** — no checker reads the charter's prose, no document cites Decision 2, and none of the seven names appears anywhere else. A charter clause became false in silence, which is the wrong-negative failure the absence-tag rule exists for: acting on a false clause means not looking.

**The perishable classes to watch: product and vendor names, version numbers, prices, vendor availability, institutional affiliations, and candidate lists marked "pending."** A list that is explicitly not yet decided is not a decision at all, and does not belong in a document of decisions.

**Where the cut material goes.** Perishable specifics that remain useful are planning, not history: they belong in [`ideas.md`](ideas.md), which is numbered, expected to age, and binds nothing.

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
