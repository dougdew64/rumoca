# DECISIONS — HRW Observatory

Running log of nontrivial implementation choices, newest last. One line + rationale each.
See `docs/CHARTER.md` for binding decisions; this file records the smaller calls made in-session.

## Arc 1 — Parse → Resolve → Typecheck

- **2026-07-18 — Dependencies added.** `eframe 0.35` (charter-blessed shell), `serde_json`
  (vehicle for the generic serde-value tree — the AST derives `Serialize` unconditionally),
  `anyhow` (Rumoca's parse API returns `anyhow::Result`). Path deps on `rumoca-phase-parse` and
  `rumoca-ir-ast` (relative `../rumoca/crates/...`). No feature flags needed — serde is always on
  in `rumoca-ir-ast`. *Pending user ratification per charter "ask before adding a dependency."*
- **2026-07-18 — Build on stable toolchain.** Rumoca pins `nightly-2026-02-27`, but its own comment
  says that pin exists only for WASM threading; the parse/ir-ast crates and their native transitive
  deps compile on the installed `stable 1.96.0`. HRW is native-only (Decision 5), so no toolchain
  file is added. Revisit if a later arc's crate needs a nightly feature natively.
- **2026-07-18 — Worker serializes IR to `serde_json::Value`, not the AST type.** The worker thread
  runs the phase and converts its output to a generic value *before* sending it over the channel, so
  the UI thread never links against Rumoca IR types and the tree inspector stays phase-agnostic
  (charter §4.4). Channel messages: `ToWorker::Parse{path}` / `FromWorker::Parsed{path, result}`.
- **2026-07-18 — File picker = directory scan, not a native dialog.** Arc 1 lists `*.mo` in a
  specimen directory (default `<manifest>/specimens`, editable in-app) rather than pulling in `rfd`.
  Matches charter §4.4 ("file picker over the export directory") with one fewer dependency; revisit
  if arbitrary-location browsing is wanted.
- **2026-07-18 — `SingleInertia.mo` smoke-test specimen.** Authored by Claude directly from the
  arc-1 archetype (single rotating inertia + ideal torque source, §4.2.1) in the portable subset, to
  give the instrument a real input and back an end-to-end unit test. It is a *placeholder*: not yet
  round-tripped through System Modeler, so the §4.3 differential test is still open.
- **2026-07-18 — WSLg Wayland → X11 fallback in `main`.** WSLg advertises `WAYLAND_DISPLAY=wayland-0`
  but the socket lives under `/mnt/wslg/runtime-dir` while `XDG_RUNTIME_DIR=/run/user/1000`, so the
  path winit resolves is absent and it dies with `NoCompositor`. winit 0.30 no longer honors
  `WINIT_UNIX_BACKEND` and forbids a second event loop (retry-after-failure gives `RecreationAttempt`),
  so `main` probes the exact socket winit would use (`UnixStream::connect`) *before* `run_native`; if
  unreachable and `DISPLAY` is set, it removes the Wayland env so winit selects X11. Keeps Wayland
  where it genuinely works. The harmless `libEGL … DRI3` warnings are Mesa's software-GL fallback.
- **2026-07-18 — `.vscode/` debug config pins CodeLLDB.** `launch.json` provides CodeLLDB (`type:
  lldb`) launch configs for the app and the unit tests; `settings.json` sets
  `rust-analyzer.debug.engine = vadimcn.vscode-lldb` so the "Debug" CodeLens uses CodeLLDB instead of
  falling back to the cpptools/GDB engine (only cpptools was installed). Charter Decision 6 mandates
  CodeLLDB, and its Rust-aware formatters are what make IR values legible while single-stepping a
  phase. Requires installing the CodeLLDB extension (`vadimcn.vscode-lldb`).

## Arc 1 — dependency loading (MSL) for resolve

- **2026-07-18 — Library MSL = official reference MSL 4.1.0, staged in `vendor/msl/` (gitignored).**
  Downloaded the `ModelicaStandardLibrary_v4.1.0` release — the version Rumoca is tested against.
  *Not* System Modeler's bundled copy: SM ships a Wolfram-specific `ModelicaServices` that references
  `WSMServices`, which Rumoca can't resolve. The standard `Modelica` package is identical in both, so
  this doesn't compromise the differential test. Source roots used: `Modelica 4.1.0`,
  `ModelicaServices 4.1.0`, `Complex.mo`.
- **2026-07-18 — Load dependencies via `rumoca-compile`'s `Session`, not the umbrella `Compiler`.**
  The `Compiler`'s source-driven planner only loads roots the model *names*, and does not follow
  `uses()`; loading `Modelica` alone left `ModelicaServices`/`Complex` unloaded → MSL internally
  unresolvable. The `Session` lets us load ALL configured roots unconditionally (as
  `SourceRootKind::DurableExternal`). Dropped the `rumoca` umbrella dep (pulled clap/minijinja/sim).
- **2026-07-18 — Persistent `Session` on the worker; extract only the user model's IR.**
  `session.resolved()` resolves the whole aggregate (user model + MSL): ~1.9s cold, ~0.3s incremental,
  but ~430MB if serialized whole. So we keep the Session alive (load MSL once), and per stage serialize
  only the user model's class via `ClassTree::get_class_by_qualified_name` (~4–13KB). Model name comes
  from parse; `session.qualify_model_name(uri, name)` handles a `within` clause.
- **2026-07-18 — Typecheck stage DEFERRED to Arc 2.** Whole-tree pre-instantiation
  `rumoca_phase_typecheck::typecheck(ResolvedTree)` fails on the full MSL (1454 MSL-internal errors,
  returns no tree). The clean, model-scoped typecheck is `typecheck_instanced`, which runs *after*
  instantiation via the reachable-closure pipeline (`compile_model_strict_reachable_with_recovery`) —
  Arc 2 machinery. Per CLAUDE.md ("stop and ask" on later-arc machinery), shipped Parse + Resolve;
  the Typecheck tab is disabled with an explanatory tooltip. **Doug's decision (2026-07-18): defer the
  typecheck stage to Arc 2**, where `typecheck_instanced` (post-instantiation) gives a proper
  model-scoped typed tree — keeping the curriculum build order. (Removed the `rumoca-phase-typecheck`
  direct dep until the stage lands.)

## Arc 1 — Claude bridge (question-driven help)

- **2026-07-19 — Help = question-answering, not a static lookup table.** The planned help feature
  (memory `arc1-dogfood-then-help`) was a field/concept lookup layer keyed by field-path. Superseded:
  the most valuable question in an IR observatory is *provenance/causation* ("where did this come
  from, which phase made it, why does it look like this"), which a static table answers poorly and a
  reasoning agent with the specimen + IR + Rumoca source + phase docs in context answers well. So help
  is reframed as **asking Claude about a concrete result** from inside the app. (Doug's reframing;
  Claude moved the mechanism off build-time hard-coding — see next.)
- **2026-07-19 — Runtime bridge, not build-time pre-instrumentation.** Doug's first sketch hard-coded
  the specimen path at build time so Claude could pre-instrument, dropping the file chooser. Rejected:
  hard-coding creates no channel from the running app to Claude, and pre-instrumentation is *guessing*
  what will be asked, whereas a runtime handoff carries the **exact** node clicked. Chosen: the app is
  a **thin emitter** — on "Ask Claude about this" it writes one JSON *focus file*
  (`<manifest>/.hrw-bridge/focus.json`, gitignored, repo-relative so it's stable across Claude Code
  sessions); Doug then asks in the Claude Code chat and Claude reads the file. No rebuild, any
  specimen, keeps the charter §4.4 chooser. No new dependencies (serde_json only). The fuller "answer
  rendered inside the app" mode (needs `egui_commonmark`) and an autonomous `Monitor`-watched mode are
  deferred, ratify-first.
- **2026-07-19 — Span-ascent for provenance.** Rumoca IR carries source provenance pervasively
  (`rumoca_core::Location` = byte offsets + `file_name`; `Span` = byte offsets + opaque `SourceId`),
  but a clicked leaf usually has none of its own. `bridge.rs` walks the serde tree *up* from the
  clicked node to the tightest enclosing `location`/`span`, then slices that byte range from source
  (expanded to whole lines). Fully generic — no Rumoca types — so the one-generic-tree rule (§4.4)
  holds. `Location` is preferred (self-contained file_name); `span` falls back to the specimen file.
- **2026-07-19 — Learnings accumulate in a per-specimen HRW lab notebook, Doug-authored.**
  `docs/specimen-notebook/<Specimen>.md` (template + README committed) records specimen↔Rumoca-feature findings
  — HRW's own record, kept distinct from the rumoca clone's `docs/compiler-phases` (Doug's canonical
  phase docs, *never* silently written by Claude). **Doug authors**; Claude drafts on request and
  challenges. Rationale: the goal is Doug learning Rumoca, and writing the synthesis is the learning —
  auto-populating would defeat the purpose. The bridge produces the conversations; keepers get
  promoted into the notebook by hand.
- **2026-07-19 — Bridge improvement 1: resolve the DefIds.** The opaque integers Resolve produces
  (`def_id`, `type_def_id`, `base_def_id`) are now resolved to their definitions. The worker owns the
  resolved tree, so it does the lookup (deterministic → app side, per the thin-emitter/thick-reasoner
  line): after extracting the model, it scans the IR for DefId-valued keys, resolves each via
  `ClassTree.def_map` + `get_class_by_qualified_name`, and ships a `DefId → DefInfo {name, kind,
  class_type, file_name, line}` map (`worker::DefInfo`) alongside the stage results. Two surfaces
  consume it: the tree annotates DefId leaves inline (`type_def_id: 27579 → model Modelica.…Inertia`),
  and the bridge focus carries the whole map as `def_resolutions` so Claude follows real pointers. No
  new dependency — `def_map` iteration reads `DefId`'s public `.0` field, so `rumoca-core` stays out
  of the direct deps. Component *own* def_ids may or may not resolve (depends on `def_map` membership);
  the high-value `type_def_id` class links are verified end-to-end (`resolves_def_ids_against_msl`).
- **2026-07-19 — Bridge improvement 2: cross-stage Parse↔Resolve diff.** A node focus now carries the
  *same* node before (Parse) and after (Resolve) name resolution, plus a scalar-delta `changes` list,
  so "what did Resolve do here?" is answered from data. Node correspondence is by **class-relative
  path**: `class_subtree` auto-detects each stage's user class (descend `classes.<model>` when the
  root wraps it — the parsed `StoredDefinition` — else the root already is the class — the resolve
  extract), so the same node lines up whether captured from either tab, without changing either tab.
  The app passes both `parse_value`/`resolve_value` in `Ask`; `bridge::build_cross_stage` + `diff`
  compute it. **Known limitation (dogfooding finding):** the raw diff mixes ~2 semantic changes
  (`def_id`, `type_def_id` null→id) with ~14 bookkeeping ones (parse records `file_name` as a basename
  and one `SourceId`; the session re-registers the document under its full URI, so `location.file_name`
  and `span.source` churn everywhere). Per thin-emitter/thick-reasoner, Claude categorizes these in its
  answers rather than the app filtering — a code-level filter/categorization is a candidate refinement.
- **2026-07-19 — Bridge improvement 3: DefId navigation ("go to definition").** A `def_id`/`type_def_id`
  that resolves to a class is now a doorway, not a dead-end label. Right-click → "↪ Go to <name>" sends
  `ToWorker::OpenDef(qualified_name)`; the worker extracts that class from the resolved tree
  (`get_class_by_qualified_name`) and returns its IR + its own `def_index` (so navigation recurses).
  The UI keeps a browser-style nav stack (← Back / ⌂ Specimen / breadcrumb) and renders the navigated
  class in the **same** generic tree (charter §4.4) — no new view widget. Capturing inside a navigated
  class writes a node focus scoped to it (model = class name, no cross-stage since library classes have
  no Parse stage here); no bridge changes were needed. Serves the "reach more context conveniently"
  goal (convenient context-identification × context-sensitive explanation).
- **2026-07-19 — 🐞 "Show this being set (debugger)" — one-click debug capture.** The debugger walk
  (watch a field get assigned in a Rumoca phase) was a headline feature but clumsy to invoke. The app
  can't drive VS Code's debugger, so per thin-emitter/thick-reasoner it does the *capture* and Claude
  does the *arming*: a context-menu item tags the focus with `request: "debug-where-set"` (the focus
  now carries a `request` field). Claude reads that, maps the field → the exact Rumoca assignment site
  (`docs/debug-set-sites.md`, keyed by function so line drift doesn't break it — re-located in the
  clone at arm-time), and rewrites the `preRunCommands` of the `.vscode/launch.json` "Debug HRW — break
  where Claude armed" config. User flow: right-click → 🐞 → "debug" → launch that config → select the
  specimen. The one irreducible step (launching the debug config) is VS Code's; everything up to it is
  automated. Tier-2 upgrade (a `Monitor` watcher that arms the instant 🐞 is clicked, no cue) is
  offered but not built.

## Arc 1 — repo/dependency reorganization

- **2026-07-19 — `docs/compiler-phases` moved into HRW; Rumoca switched to a pinned git dependency.**
  Doug's phase-explanation docs (11 phases + drill-downs) were the only fork-only content in his
  local Rumoca clone (`dougs-docs` branch = official `upstream/main` + 2 docs-only commits; `crates/`
  byte-identical to upstream). Verified, then: (1) moved `docs/compiler-phases/` into this repo next to
  `docs/specimen-notebook/` (source links de-linked to crate-relative inline-code refs, since HRW has no
  `crates/`); (2) switched `Cargo.toml` from path deps on `../rumoca` to **git deps on official
  `github.com/CogniPilot/rumoca` pinned to `rev = 8cdc7419`** — the exact commit HRW was built
  against. Rationale: a path dep tracks whatever the clone is checked out to (unpinned); a `rev` +
  committed `Cargo.lock` is immutable and reproducible — the real fix for "always build against the
  correct Rumoca version." Build + all tests pass against the fetched official crates. Consequence:
  Rumoca source now lives in `~/.cargo/git/checkouts/` (read-only, hash-named); the 🐞 debugger still
  works (basename breakpoints + default dev debug info); no editable clone unless kept aside for a
  future arc that instruments Rumoca. HRW no longer depends on Doug's fork or local clone.

## Arc 2 — advanced (with Arc-1 ritual items deferred)

- **2026-07-19 — Advanced Current Arc to Arc 2 (Instantiate → Flatten) with two Arc-1 close-out items
  deferred.** The Arc-1 help-system + reorg work is committed/pushed (commit `23f8ccc`). Two close-out
  ritual items are intentionally left open so arc progress isn't blocked while Doug thinks through the
  workflow: (1) **the differential test** — round-tripping `RotationalInertia.mo` through System
  Modeler vs Rumoca (ritual #1); (2) **the per-specimen lab notebook** (`docs/specimen-notebook/`, still just
  template + README). Doug's decision (2026-07-19): advance to Arc 2 now and **revisit round-tripping
  + notebooking after he has given the matter consideration** — their absence is deliberate, not an
  oversight. CLAUDE.md's Current Arc section updated to Arc 2 and carries this deferral note.

## Arc 2 prep — generic field-help panel + Rumoca-update process

- **2026-07-19 — Two-tier explanations: a generic (build-time) tier alongside the specific (bridge)
  tier.** Doug's dogfooding insight: waiting for Claude to explain *what a field is* (generic) is
  wasteful when only the *why did this one happen* (specific) needs runtime reasoning. So the RHS
  "About this field" panel shows, instantly on left-click, the field's own Rumoca `///` doc — 194
  fields extracted from the pinned `rumoca-ir-ast` into `src/field_help.json`, embedded via
  `include_str!` (`src/field_help.rs`). Keyed by field name (v1; type-path disambiguation deferred).
  A "Read: Phase N" button opens the matching `docs/compiler-phases` chapter in VS Code's Markdown
  **preview** (scoped `workbench.editorAssociations` in `.vscode/settings.json`). This resurrects the
  originally-superseded lookup layer, correctly scoped to the generic tier; the bridge + chat `explain`
  remain the specific tier.
- **2026-07-19 — Field-help regeneration is a committed one-command tool, not an ad-hoc script.**
  `cargo run --example gen_field_help` (`examples/gen_field_help.rs`) locates `rumoca-ir-ast`'s source
  via `cargo metadata` (robust to the cargo-cache hash/rev — no hard-coded path) and rewrites
  `src/field_help.json`. Verified byte-identical to the original ad-hoc extraction. The broader
  "what to do after a Rumoca pin bump" process lives in `docs/updating-rumoca.md`: compiler + tests
  drive code fixes, one command refreshes field help, `docs/compiler-phases` is Doug-only.
- **2026-07-19 — Rumoca version in Help/About is derived, not hand-maintained.** A `build.rs` reads
  `rumoca-compile`'s version + git commit from `Cargo.lock` (`cargo:rerun-if-changed=Cargo.lock`) and
  emits them as `HRW_RUMOCA_VERSION`/`HRW_RUMOCA_REV`; the About dialog shows them via `env!(...)`. So
  it always matches what was compiled in and can never drift — a pin bump refreshes it automatically
  on the next build (no manual step; the checklist just says "verify About shows the new rev").

## Arc 2 — increment 2 (instantiate + instanced-typecheck stages)

- **2026-07-19 — Instantiate/Typecheck tabs show real IR via direct phase calls; two new deps.** The
  high-level reachable pipeline exposes only `flat`/`dae`, not the intermediate `InstancedTree` or
  typed tree. So the worker calls the phase crates directly: `rumoca_phase_instantiate::instantiate_model(&resolved, model)`
  → the `InstanceOverlay` (Instantiate tab), then `rumoca_phase_typecheck::typecheck_instanced(&tree,
  &mut overlay, model)` enriches the *same* overlay in place (Typecheck tab). Added `rumoca-phase-instantiate`
  and `rumoca-phase-typecheck` as direct git deps (same pin) — **accepted dependencies** (charter
  "ask before adding"): required because the pipeline doesn't surface the intermediates and
  rumoca-compile doesn't re-export the phase functions.
- **2026-07-19 — HRW's pipeline order is Resolve → Instantiate → instanced-Typecheck → Flatten,
  diverging from Rumoca's nominal phase numbering (typecheck=3 before instantiate=4).** HRW cannot use
  the nominal phase-3 whole-tree typecheck (fails on the full MSL — the Arc-1 deferral). It uses
  `typecheck_instanced`, which types the instantiated overlay and therefore runs *after* instantiate.
  The tab is labeled "Typecheck (instanced)" with a tooltip; `docs/compiler-phases` phase numbers are
  left intact (they describe Rumoca's nominal phases, and are Doug's authoritative reference).
- **2026-07-19 — Per-stage "changed vs previous" green highlight + stage-file diff publishing.** Each
  compile writes every stage's full IR to `.hrw-bridge/stages/<name>.json` (`bridge::write_stages`),
  which the focus references so Claude diffs any pair on request. In-app, each stage's tree paints a
  leaf value green when it differs from the previous stage's value at the same path (`CHANGED_COLOR`,
  driven by `App::previous_stage_value` + `prev` threaded through `tree_ui`). **Standing expectation:
  every pipeline stage added in later arcs must be wired into BOTH mechanisms** (see the file-by-file
  checklist in Claude's memory `hrw-stage-diff-highlight-extend`).

## Arc 3 — Matching & BLT

- **2026-07-20 — Advanced to Arc 3 (charter §4.2.3): structural analysis (matching / BLT / tearing).**
  A **Structural** stage tab renders `build_structural_report(&CompilationResult.dae)` — max matching
  (equation↔unknown), BLT blocks (scalar vs coupled, with tearing). Added `rumoca-phase-structural`
  as a direct git dep (same pin) — **accepted** (Doug: "add any dependencies you need"); required
  because the rich `StructuralReport` isn't re-exported by rumoca-compile. The report types aren't
  `Serialize`, so `worker::{structural_to_json, block_to_json, tearing_to_json}` build the JSON. The
  stage is wired into the diff-highlight (previous = None; the report has no path-aligned prior) and
  stage-file publishing per the standing rule.
- **2026-07-20 — Custom `egui::Painter` canvas for the Arc-3 views, NOT `egui_graphs`.** Decision
  driven by Doug's criterion ("choose the egui route that most empowers you to implement the view
  enhancements I'll request during dogfooding, don't be limited by egui"). A graph library is an
  abstraction that caps what enhancements are expressible; a raw `Painter` canvas draws every pixel
  (matrices, spy-plots, highlights, block outlines, hover, click-to-capture) with no ceiling and plugs
  straight into the bridge (hit-test → capture). Incidence view + BLT spy-plot both built on one
  reusable canvas scaffold (pan/zoom/pointer→cell).
- **2026-07-20 — Finding: the Drivetrain is structurally singular (high-index).** `build_structural_report`
  reports 93/97 matched (unmatched are connector flows/potentials at the ideal gears). The *ideal*
  (rigid) gears impose position constraints → high index → not matchable without index reduction
  (Pantelides, Arc 4). Not a specimen defect; the arc's coupled-block study comes from the
  feedback-loop specimen (increment 4). RotationalInertia (a plain ODE) analyzes cleanly (12 scalar
  blocks, 0 coupled).
- **2026-07-20 — Increment 2: the first custom view is a BLT block spy-plot, NOT a raw incidence
  matrix.** Scouting the structural API found the raw incidence (`eq_unknowns` — each equation's set
  of referenced unknowns, the off-diagonal sparsity) is built by `incidence::build_incidence`, which
  is `pub(crate)`; `matching` and `equation_label` are private too. `build_solver_sparsity_triplets`
  is public but is RHS-only Jacobian sparsity in solver-column order with no names — it won't line up
  with the matching. So the only public, named, matching-consistent data is `StructuralReport.matching`
  + `.blocks`. Doug chose (over "reimplement incidence in HRW" and "make build_incidence pub upstream")
  to **build the BLT block spy-plot from the report now** — the arc's real payoff, honest and robust.
  `src/spyplot.rs` draws the diagonal blocks (scalar cells + coupled boxes with tearing) in BLT order;
  inter-block (lower-triangular) couplings are *not* drawn because they need the unexposed incidence,
  and reproducing it in HRW risks a subtly-wrong matrix the charter's phase-boundary rule forbids.
  Built on `src/canvas.rs`, a reusable pan/zoom canvas scaffold (world↔screen transform, drag-pan,
  scroll-zoom-about-pointer, fit-to-content) for every future custom-`Painter` view. Spy-plot blocks
  are clickable → capture `blocks[i]` into the bridge, so the visual emitter feeds the question
  channel. A `Spy-plot | Tree` toggle keeps the generic report tree available.
- **2026-07-20 — Increment 3: the coupled-block specimen `ProportionalLoop.mo`.** An *idealized*
  proportional servo inner loop: `error = reference - measurement`, `command = Kp*error`,
  `measurement = plantGain*command`. Every relation is algebraic — the servo's integrating dynamics
  are deliberately removed — so the feedback closes on itself with no state to break it. Self-contained
  (portable subset, no MSL; follows the `SingleInertia` style). Rumoca flattens it to 3 equations / 3
  unknowns forming **one coupled BLT block of size 3** (`coupled_block_count == 1`); tearing picks
  `command` as the single iteration variable (residual `f_x[0]`), then solves `error` and `measurement`
  causally. This is the first specimen whose spy-plot shows an **orange coupled box** (the whole 3x3)
  with a tearing report on hover — the object of study for the matching/BLT arc. Guarded by
  `worker::tests::proportional_loop_has_a_coupled_block` (and RotationalInertia's test now also asserts
  `coupled_block_count == 0`, so the scalar-vs-coupled distinction can't silently regress). The
  System Modeler differential round-trip stays deferred per the arc's standing note.
- **2026-07-20 — Per-specimen lab notebook activated: trace + narrative.** Doug un-deferred the
  `docs/specimen-notebook/` notebook, arriving (during design discussion) at a concrete shape: **specimen +
  durable compilation trace + Claude-written narrative**. Each `docs/specimen-notebook/<Model>/` holds `trace/`
  (the six stage IR files — parse…structural — + `manifest.json` stamping the Rumoca rev + an FNV-1a
  specimen hash) and `narrative.md` (the grounded story of that specimen's compilation, foregrounding
  the designed phenomenon, citing specific trace locations, linking to `docs/compiler-phases` + external
  math references). **Why trace-anchored:** the trace is ground truth, so every "interesting" claim in
  the narrative is checkable and a trace diff flags staleness — the guard against confident-but-wrong
  AI prose. **Why not just docs/compiler-phases:** those are Doug's *generic* phase theory; the notebook
  is Claude's *specimen-specific* synthesis (links back to them). `ProportionalLoop` is the pilot.
  - **Trace generator (`examples/gen_trace.rs`):** required a **bin→lib split** — added `src/lib.rs`
    exposing the modules + `worker::compile_specimen` (headless, reuses the exact worker path so traces
    are byte-identical to the app's). `main.rs` now `use hrw::app`. Accepted: cleaner architecture,
    unblocks headless tooling; no behavior change.
  - **Trace vs bridge stages:** the bridge already writes all stages to `.hrw-bridge/stages/` but those
    are transient + gitignored; the trace is the *durable, committed* snapshot. The narrative
    foregrounds the arc's boundary (flatten→structural) but the trace carries **all six** stages —
    restricting it would be arbitrary since the app computes them all anyway (Doug's point).
  - **UI:** a **"Read: specimen narrative"** button in the right panel (beside the generic-chapter
    button), shown only when `docs/specimen-notebook/<model>/narrative.md` exists — closes the dual-emitter loop
    (visual channel → durable narrative). `editorAssociations` now opens `docs/specimen-notebook/**/*.md` in
    Markdown preview too. Regeneration on pin bump is `docs/updating-rumoca.md` step 5.
- **2026-07-20 — Three spy-plot-diversity specimens: `MixedLoop`, `TwoLoops`, `NonlinearLoop`.** Added
  to fill gaps in what the BLT spy-plot had shown (previously only all-scalar or one all-consuming
  coupled box). Each self-contained (portable subset), with a trace + narrative and a worker test
  guarding its block structure: **MixedLoop** → `[scalar, coupled(3), scalar]` (a loop bracketed by
  scalar solves — makes BLT *ordering* visible; note `output` is a reserved Modelica keyword, so the
  sink variable is `result`); **TwoLoops** → two `coupled(2)` blocks in series (two orange boxes,
  sequenced by data dependency); **NonlinearLoop** → structurally *identical* to `ProportionalLoop`
  (one `coupled(3)`, tear `command`) because incidence is blind to nonlinearity — the difference is
  numerical (the torn residual is nonlinear → Newton), which makes it the bridge to the
  simulation/convergence-narrative idea (`docs/ideas.md` #1). Tests:
  `worker::tests::{mixed_loop_has_scalar_and_coupled_blocks, two_loops_has_two_coupled_blocks,
  nonlinear_loop_has_a_coupled_block}`.
- **2026-07-20 — Specimen purpose hints in the file + UI (docs/ideas.md #2).** Convention: a
  `// purpose: <one-line>` comment in each specimen states the compiler phenomenon it exercises,
  kept distinct from the Modelica description string (which stays a faithful model description).
  The app scans it at rescan (`app::read_purpose`, no compile — so hints show even for a specimen
  that fails to compile) and renders it as weak, truncated subtext (+ hover for the full line) under
  each filename in the LHS list, turning the list into an index of what each specimen teaches. All
  seven specimens carry one; convention recorded in CLAUDE.md's specimen rules. Adding the comment
  shifted source spans, so all traces were regenerated (only `structural.json` was unaffected — the
  analysis is span-free); the four narratives that quoted an FNV hash now reference `manifest.json`
  instead (robust to any future source tweak).
- **2026-07-20 — Advanced to Arc 4 (charter §4.2.4): index reduction (Pantelides / dummy derivatives).**
  Arc 3 (Matching & BLT) closed: observatory renders the structural phase (Structural stage + BLT
  spy-plot), traces captured (the 7-specimen notebook). Gates 1 (differential test) + 3 (debugger
  single-step) accepted as deferred/unconfirmed — Doug is separately weighing whether they stay gates
  (docs/ideas.md #4). **De-risk scout before advancing:** confirmed Rumoca is NOT blocked-on-upstream —
  `rumoca-phase-structural::dae_prepare` is a public module with dummy-derivative index reduction
  (`expand_compound_derivatives`, `promote_der_algebraics_to_states`, `eliminate_derivative_aliases`,
  `symbolic_time_derivative_for_expr`, constrained dummy-state reduction, direct state demotion). Arc-4
  increment plan (CLAUDE.md): scout dae_prepare + capture before/after on Drivetrain (already high-index,
  no new library needed) → wire the Index-reduction stage/view → build the portable-subset planar
  mechanics library (revolute joint, rigid link, fixed; no MSL MultiBody) → author the four-bar linkage
  specimen (index-3 → reduced).
- **2026-07-20 — Arc 4 step 1: Rumoca DOES index-reduce; funnel identified + wired.** Scout finding:
  `cr.dae` is the RAW (pre-reduction) DAE — that's why `build_structural_report` reports Drivetrain
  singular (97/97, 93 matched; the 4 unmatched are constraint forces `emf.p.v`, `shaft.flange_a.tau`,
  `load.flange_a.f`, `wall.flange.f`). `eliminate::eliminate_trivial` only does trivial-alias removal
  (97→39 eqs) and leaves the singularity. The **full** index reduction is rumoca-sim's internal
  `prepare_dae_for_structural_analysis` (`solve_lowering/structural_lowering.rs`, `pub(super)`), a 9-step
  funnel over the **public** `rumoca_phase_structural::dae_prepare::*` fns (demote_exact_alias →
  demote_direct_assigned → reduce_constrained_dummy → index_reduce_missing_state_derivatives →
  demote_no_assignable → eliminate_derivative_aliases → demote_no_retained → expand_compound_derivatives
  → substitute_standalone_state_derivatives). HRW **replicates that funnel** in
  `worker::index_reduce_for_structural_analysis(&mut Dae)` (rather than depend on rumoca-sim, whose entry
  is `pub(super)`) — legitimate since it's Rumoca's own public fns in Rumoca's own documented order, but
  it **couples to that order**: guarded by `worker::tests::drivetrain_index_reduces_from_singular_to_solvable`
  (before = singular Err, after funnel = Ok), which fails loudly if a pin bump reorders/renames the funnel.
  Verified: Drivetrain singular (97/97, 93 matched) → after funnel Ok (97 eq, 1 coupled block). Added
  `rumoca-ir-dae` as a direct dep (accepted) to name `rumoca_ir_dae::Dae` for the funnel signature.
- **2026-07-20 — Arc 4 step 2: Index-reduction observation wired into the observatory.** New
  `StageKind::IndexReduction` tab (after Structural), fed by `worker::index_reduction_stage` — runs the
  funnel on a clone of `cr.dae`, then `build_structural_report` on the reduced DAE. **Structural stays
  on the RAW DAE (the singular "before"); Index reduction shows the reduced "after"** — the two tabs are
  the Arc-4 contrast side by side. For Drivetrain: Structural = singular (no IR), Index reduction = OK
  (97 eq, 87 blocks incl. 1 coupled); for an index-1 model the two are identical (a `// note` says so).
  Followed the full new-stage checklist: worker field + `FromWorker::Compiled`; app `StageKind`/field/
  init/reset/`drain_worker`/`current_stage`/`stage_name`/tab/`last_successful_stage` (Index reduction is
  now the furthest clean stage) / `previous_stage_value` (diffs vs raw Structural) / `write_stages`
  (`index_reduction.json`); the **BLT spy-plot generalized** to both report stages (was Structural-only);
  `field_help::chapter_for_stage` → `phase6_dae_construction/index_reduction.md`; `gen_trace` STAGES +
  by_name (traces regenerated — Drivetrain now carries `index_reduction.json`); Drivetrain narrative +
  notebook README updated (the Arc-4 forward reference is now fulfilled). Guarded by
  `worker::tests::drivetrain_index_reduction_stage_recovers_singular`.
- **2026-07-20 — Arc 4 step 3: FINDING — Rumoca's Rust-path index reduction handles LINEAR high-index but
  not NONLINEAR holonomic constraints; four-bar linkage DEFERRED, Arc 4 reframed around Drivetrain.**
  Drafted the hand-built planar mechanics library (`lib/PlanarMechanics.mo`: Frame connector, Fixed,
  Revolute, FixedTranslation, Body — portable subset, no MSL MultiBody) and a pendulum to validate it.
  It flattens fine, but its structural report is singular AND does not index-reduce. Isolated the cause:
  the *barest* textbook Cartesian pendulum (5 eqs, point mass, `x²+y²=L²`, no library) is ALSO not reduced
  — so it is **not** a library-formulation bug but the **nonlinear holonomic constraint**. Rumoca's funnel
  (dummy-state demotion + `eliminate_trivial`) reduces *linear* constraints (Drivetrain's ideal gears →
  singular→solvable ✅) but not `x²+y²=L²` (leaves the tension `F` + constraint unmatched). The charter's
  four-bar linkage (§4.2.4) has nonlinear loop-closure, so it hits the same wall. **Doug's decision:
  reframe Arc 4 around `Drivetrain`** (linear high-index, already demonstrated in steps 1–2); document the
  nonlinear limitation as a finding (possible upstream contribution); **park** the planar library
  (kept + parse-guarded by `worker::tests::planar_mechanics_library_parses`) and defer the four-bar.
  Kept the funnel completion (`eliminate_trivial` + `apply_elimination_substitutions_to_dae`) from this
  investigation — it makes the reduction faithful to rumoca-sim's real sequence (Drivetrain still reduces).
  Honest caveat: proven the *public* reduction API doesn't handle it; the full private sim path
  (`remove_duplicate_continuous_equations` is `pub(super)`) or the CasADi target may differ — unconfirmed.
- **2026-07-20 — Arc 4 closed; advanced to Arc 5 (charter §4.2.5): initialization & IC planning.** Arc 4
  done (reframed): index reduction is observable on Drivetrain (Structural singular → Index reduction
  solvable); the nonlinear-constraint four-bar + planar library are parked/deferred (docs/ideas.md #5).
  Close-out gates 1 (differential test) + 3 (debugger single-step) accepted as deferred/unconfirmed (still
  under review, docs/ideas.md #4). **Arc-5 de-risk scout:** confirmed `rumoca-phase-structural::ic_plan` is
  public — `build_ic_plan(dae, n_x) -> Vec<IcBlock>` (ScalarDirect / ScalarNewton / TornBlock / CoupledLM)
  + `build_ic_relaxation_hint` for singular initial subsystems; the flat model carries `initial_equations`.
  Not blocked-on-upstream. Specimen: the RC/RL blow-up case (the 2025 bug; likely upstream-contribution
  source). Increment plan in CLAUDE.md: scout build_ic_plan on RC/RL → wire an Initialization stage/view →
  author the RC/RL specimen.
- **2026-07-20 — Arc 5 steps 1–2: Initialization / IC-planning stage wired.** Scout confirmed
  `rumoca-phase-structural::build_ic_plan(dae, n_states)` + `build_ic_relaxation_hint` are public and
  produce a rich plan for a simple RC circuit (21 blocks: 20 ScalarDirect + 1 ScalarNewton, plus a
  relaxation hint dropping the redundant ground-KCL equation and pinning `gnd.p.i`). New
  `StageKind::Initialization` tab (after Index reduction), fed by `worker::initialization_stage` →
  `ic_plan_to_json` (IcBlock carries `rumoca_core::Expression`, so build JSON like structural; solutions
  serialized via serde into the tree). Empty plan (n_eq ≤ n_states, a pure ODE) → an info note. Followed
  the full new-stage checklist (worker field/compile; app StageKind/field/init/reset/store/current_stage/
  stage_name/tab/last_successful_stage/previous_stage_value(None)/write_stages; field_help →
  phase7/ic_plan.md; gen_trace STAGES+by_name, traces regenerated). Specimen `RcCircuit.mo` (MSL
  electrical) + narrative added. The charter's RC/RL **blow-up (failure)** case is a deliberate later
  iteration — this establishes the IC-plan mechanism first. Guarded by
  `worker::tests::rc_circuit_has_an_ic_plan`.
- **2026-07-20 — Arc 5 (blow-up): `CapacitorLoop` — the "where it fails and why" specimen.** A capacitor
  directly across an ideal voltage source: `C.v` is a state yet algebraically pinned to `src.V`, so no
  consistent t=0 state exists (degenerate/ill-posed). The observatory flags it: **Structural = singular**
  (13/14, `gnd.p.i` unmatched), **Index reduction = STILL singular** (7/8) — the decisive contrast with
  Drivetrain (singular→reduced): index reduction rescues genuine high index, not ill-posedness. Guarded by
  `worker::tests::capacitor_loop_is_singular_and_irreducible`. **Finding + honest gap:** the failure
  surfaces at Structural/Index-reduction, and `build_ic_plan` still emits a (untrustworthy) plan. A *pure*
  user-initialization over-determination (conflicting `initial equation`s — the tested-then-removed
  `OverInitRc`) is NOT surfaced today because `build_ic_plan` ignores the user's initial equations →
  future enhancement `docs/ideas.md #6` (make the Initialization stage report init-system determinacy).
- **2026-07-20 — Idea #6 implemented: Initialization stage flags over-determined user init.** The
  Initialization stage now computes a `determinacy` block from the DAE — explicit initial conditions
  (`initialization.equations` + states with `fixed == Some(true)`) vs states — and flags **over-determined**
  init (surplus > 0) with a red note. Rumoca's `rumoca-phase-dae::balance` is the *continuous* balance
  (states+alg vs f_x), not init-specific, so it wouldn't catch this; computed the init count directly.
  **Under-determination intentionally NOT flagged**: a state with no explicit condition initializes from
  its `start` attribute (calibrated empirically — RcCircuit surplus −1 and SingleInertia −2 are well-posed;
  only OverInitRc's +1 is a real over-specification). Specimen `OverInitRc.mo` (RcCircuit + `C.v = 0` and
  `der(C.v) = 0`) restored + narrative; the two Arc-5 blow-ups now cover both kinds: CapacitorLoop
  (structural) and OverInitRc (init-determinacy). Guarded by
  `worker::tests::over_init_rc_is_flagged_over_determined` (+ RcCircuit asserts it is NOT mis-flagged).
- **2026-07-20 — Advanced to Arc 6 (charter §4.2.6): events & hybrid structure; specimen reframed to
  `BouncingBall`.** Arc 5 closed (initialization observable: RcCircuit IC plan + relaxation; CapacitorLoop
  structural blow-up + OverInitRc init-determinacy blow-up). **Arc-6 de-risk scout:** Rumoca's hybrid
  structure is all in **public** `rumoca-ir-dae` fields on `cr.dae` — `dae.discrete.{real_updates(f_z),
  valued_updates(f_m)}`, `dae.conditions.{equations(f_c), relations}`, `dae.events.{synthetic_root_conditions,
  scheduled_time_events}`. Not blocked-on-upstream; no new dep. **Specimen reframe (as Doug directed, à la
  Arc 4):** the charter's stick-slip-friction / joint-limit specimen needs the parked planar mechanics
  library, and Doug's suggested MSL `IdealDiode` rectifier **fails Rumoca's typecheck** (too demanding, like
  MSL MultiBody). So Arc 6 uses **`BouncingBall`** — the archetypal self-contained hybrid
  (`when h <= 0 then reinit(v, -e*pre(v))`), which compiles cleanly and produces 1 condition/relation
  (`h <= 0`) + 1 discrete update (the reinit). Increment plan (CLAUDE.md): wire an **Events** stage
  rendering the hybrid partitions → author + narrate BouncingBall. "Step-mode plotting" (running + plotting
  discontinuities) is a stretch bridging to Arc 7 (simulation core), not required for the compile-level
  event structure.
- **2026-07-20 — Arc 6 step 2: Events (hybrid structure) stage wired.** New `StageKind::Events` tab (after
  Initialization), fed by `worker::events_stage` → `events_to_json`, reading the DAE's public hybrid
  partitions directly (`dae.conditions.{equations,relations}`, `dae.discrete.{real_updates,valued_updates}`,
  `dae.events.{synthetic_root_conditions,scheduled_time_events}`) — expressions serialize, no new dep. A
  smooth model → an info note "no events". Full new-stage checklist (worker field/compile; app StageKind/
  field/init/reset/store/current_stage/stage_name/tab/last_successful_stage/previous_stage_value(None)/
  write_stages; field_help → phase6_dae_construction/dae_construction.md; gen_trace STAGES+by_name, traces
  regenerated → all specimens gain events.json). Specimen `BouncingBall.mo` + narrative: the archetypal
  hybrid (`when h<=0 then reinit(v,-e*pre(v))`) → 1 condition/relation (`h<=0`) + 1 discrete update (the
  reinit); first specimen with a non-empty Events tab. Guarded by
  `worker::tests::bouncing_ball_has_events_smooth_model_has_none`. "Step-mode plotting" (run + plot) is
  deferred to Arc 7 (simulation core).
- **2026-07-20 — Arc 6 closed; advanced to Arc 7 (charter §4.2.7): the simulation core.** Arc 6 done
  (compile-level hybrid structure observable via the Events tab: BouncingBall's `h <= 0` condition + reinit;
  smooth models "no events"). **Arc 7 crosses from static IR inspection to live execution** — the biggest
  inflection — covering the two remaining phases: **solve lowering** (phase 8, the gap Doug flagged) +
  **simulation** (phase 9). **Arc-7 de-risk scout:** Rumoca's sim is library-callable, not blocked-on-upstream
  — `rumoca-phase-solve::lower_dae_to_solve_model(&dae) -> SolveModel`, then
  `rumoca-sim::simulate_solve_model(&SolveModel, &SimOptions) -> SimResult` (Auto / RK45 / BDF-diffsol) or the
  stepwise `SimulationSession::{new, step(dt), time(), state()}` (ideal for the step-and-render loop). New
  deps to ratify (simulation + egui_plot are charter-pre-blessed §4.4/Decision 6; the specific crates are
  not yet): `rumoca-sim` (+ a solver feature: `solver-rk45` non-stiff / `solver-diffsol` BDF-stiff), likely
  `rumoca-phase-solve` + `rumoca-ir-solve`, and `egui_plot`. Increment plan (CLAUDE.md): confirm deps + scout
  run SingleInertia → trajectories → Solve-lowering observation → worker-thread sim runner + egui_plot pane
  (step-mode) → stiff bench-actuator specimen + BouncingBall discontinuity plot.
- **2026-07-20 — Arc 7 increment 1: HRW can RUN a simulation (deps added, proven).** Added `rumoca-sim`
  (default features `native-solvers` = both `solver-rk45` + `solver-diffsol`/BDF — Doug chose Auto mode) and
  `rumoca-phase-solve` (both pinned to `8cdc7419`). Path: `rumoca_phase_solve::lower_dae_to_solve_model(&dae)
  -> SolveModel` (solve lowering, phase 8) → `rumoca_sim::simulate_solve_model(&sm, &SimOptions)` →
  `SimResult { times: Vec<f64>, names: Vec<String>, data: Vec<Vec<f64>> (data[var][t]), n_states, ... }`.
  `SimOptions::default()` = t 0..1, rtol/atol 1e-6, solver Auto (diffsol BDF). Verified numerically:
  SingleInertia runs to t=2 with `w(t)=t` (`w(2) ≈ 2.0`) — guarded by
  `worker::tests::single_inertia_simulates_to_a_correct_trajectory`. **Cost accepted:** diffsol pulls
  linear-algebra crates, so build/binary are heavier now (first build +~45s). Next: a Solve-lowering
  observation (the SolveModel — closes phase 8) → worker-thread sim runner + egui_plot pane (step-mode).
- **2026-07-20 — Arc 7 #2: Solve-lowering stage (closes the phase-8 gap).** New `StageKind::SolveLowering`
  tab (after Events), fed by `worker::solve_lowering_stage` → `rumoca_phase_solve::lower_dae_to_solve_model(&cr.dae)`
  → `serde_json::to_value(&SolveModel)` (SolveModel derives Serialize, so the generic tree renders it — no
  new type dep). The SolveModel is the solvable form the simulator consumes (keys: problem, artifacts,
  initial_y, parameters, variable_meta, …). This instruments **phase 8 (solve lowering)**, the gap Doug
  flagged when auditing docs/compiler-phases. Full new-stage checklist (worker field/compile; app StageKind/
  field/init/reset/store/current_stage/stage_name/tab/last_successful_stage/previous_stage_value(None)/
  write_stages; field_help → phase8_solve_lowering/solve_lowering.md; gen_trace STAGES+by_name, all traces
  regenerated → +solve_lowering.json). Guarded by `worker::tests::single_inertia_lowers_to_a_solve_model`.
- **2026-07-20 — Arc 7 #3: worker-thread simulation runner + egui_plot pane.** A new **Simulation** tab
  (last, `▶`) with a Run button + stop-time slider; Run dispatches `ToWorker::Simulate { path, model, t_end }`
  to the worker, which compiles → `lower_dae_to_solve_model` → `simulate_solve_model` and returns
  `FromWorker::Simulated { SimData }` (plain `{ times, names, data[var][t], n_states }` — no Rumoca types
  cross into the UI). The pane plots each variable's trajectory with `egui_plot`. Runs on the worker thread
  (UI never blocks); Simulation is on-demand (not a compile stage), so it has an empty placeholder `Stage`
  for `current_stage` and is rendered specially (not a tree). **Dependency note:** used `egui_plot = "0.36"`
  — egui_plot's version numbers are **offset** from egui's (egui_plot 0.35 → egui 0.34; **0.36 → egui 0.35**),
  so "0.35" pulled a second egui version; 0.36 unifies on egui 0.35. Verified end to end:
  `worker_simulate_runs_bouncing_ball` (the hybrid model's events are handled by the solver — ball stays
  above the floor) + `single_inertia_simulates_to_a_correct_trajectory` (w(2) ≈ 2). Next: #4 (stiff
  bench-actuator specimen) + step-mode plotting for discontinuities.
- **2026-07-20 — Arc 7 #4: the stiff bench-actuator specimen (`BenchActuator`).** A DC motor
  (`ConstantVoltage → R → L → RotationalEMF`) spinning up an `Inertia` — the canonical stiff pairing
  (fast winding `L/R ≈ 1e-4 s` vs slow rotor `J = 0.05`, ~1000×). Full-pipeline round trip: raw Structural
  is **singular** (grounded-circuit reference redundancy, 47/48), but **index reduction resolves it** (unlike
  CapacitorLoop — it's a linear redundancy), solve lowering succeeds, and it **simulates** (Auto → BDF/diffsol
  copes with the stiffness where RK45 would crawl): `L.i` spikes toward `V/R = 12 A` then eases as back-EMF
  grows (~10.9 A @ 0.5 s), `load.w` ramps slowly (~11.4 rad/s). First specimen you *run*; guarded by
  `worker::tests::bench_actuator_simulates_stiff_spinup`. Note: the charter's Damper-to-Fixed friction made
  the **initializer** fail to converge (`initial variable projection plan did not converge`), so the specimen
  is the friction-free motor→inertia form. Step-mode plotting for discontinuities logged as `docs/ideas.md` #8.

- **Arc 7 UI: context-sensitive right-hand panel.** The RHS panel used to be unconditionally titled
  "About this field" with tree-item help + doc links — wrong on the Simulation tab (no tree, no field).
  Split into `App::right_panel` which dispatches on view: `right_panel_simulation` (a "Simulation" panel:
  model name, plot-control hints, a plan-ahead placeholder — *"capture a curve or time window to ask Claude
  about the run"* — the seat for the future plot-question view) when `nav.is_empty() && stage==Simulation`,
  else `right_panel_field_help` (the old generic field help). Both share `right_panel_read_links` (the
  "Read: <phase chapter>" + "Read: specimen narrative" buttons). Added a `Simulation → phase9_simulation/
  simulation.md` mapping to `field_help::chapter_for_stage`.

- **Arc 7 #4: step-mode / discontinuity plotting.** The simulator resamples onto a uniform time grid
  (no event times in `SimResult`), so a reinit at an event lands *between* two samples and a plain line
  interpolates a misleading diagonal across the jump. Fix: `worker::discontinuity_segments(&[f64])`
  splits a series into contiguous index ranges, breaking where `|Δ| > max(range·0.08, 6·median|Δ|)`
  (for BouncingBall's `v` the smooth step ~0.06 and the bounce jump ~8 differ ~40×, a clean gap); the
  plot draws one polyline per segment so egui never slopes a line through the discontinuity. GATED on
  `SimData.has_discontinuities` = "the DAE has a discrete update (`reinit` f_z or `when` assignment f_m)":
  without the gate a smooth-but-stiff transient (BenchActuator's current spike, 0→~11 in one grid step)
  would false-trigger. Chose the DISCRETE-UPDATE gate over "has any event" because BenchActuator carries
  a bare zero-crossing (`zero_crossing_conditions: 1`) yet no update — so its trajectories are continuous.
  Honest rendering = a break (gap) at the jump, not a fabricated vertical/step riser at an endpoint (we
  don't know the exact event time). Closes `docs/ideas.md` #8.

- **Arc 7 closed (2026-07-21) — the seven-arc curriculum is complete.** The observatory now instruments
  the whole Rumoca pipeline Parse → … → Solve lowering → Simulation (static IR through live execution).
  Close-out ritual: (2) panes render the IR ✓ (Solve lowering + Simulation tabs); (4) traces captured ✓
  (gen_trace includes solve_lowering); (1) differential test + (3) debugger single-step remain
  satisfiable-by-acceptance per Doug's standing reconsideration of those gates; (5) CLAUDE.md advanced.
  The charter has no Arc 8 — subsequent work is drawn from docs/ideas.md, the deferred items, or a new
  charter decision, chosen with Doug rather than assumed.

- **HRW moved INTO the Rumoca fork as a workspace member (2026-07-21).** Pass one (the seven-arc
  curriculum) hit the ceiling of Rumoca's *public API*, which exposes phase *results* (the IR) but not
  the algorithms' internal *process* (Pantelides iterating, matching's augmenting paths, BDF order/step
  control, Newton iterations) — exactly what deep learning requires. So pass two must **instrument
  Rumoca internals**, which means depending on a branch of Doug's fork. Given that, moved HRW itself
  into the fork (`github.com/dougdew64/rumoca`, branch `hrw`) as a top-level workspace member `hrw/`,
  swapping the 9 `rumoca-* = { git, rev = 8cdc7419 }` deps for `path = "../crates/rumoca-*"`. Rationale:
  pass-two features are intrinsically two-sided (a hook inside a phase + HRW's render of it), so a
  monorepo gives **atomic co-evolution** (one commit changes phase + consumer), path-dep iteration
  (`cargo run -p hrw`, no rev-bump), seamless debugger stepping, and one toolchain/profile
  (`hrw` inherits Rumoca's `optimized+debuginfo` dev profile — the charter's debugger tuning, now free).
  Charter-consistent: Decision 6 already blesses "path dependency on the workspace." Doug values
  learning over HRW's independent identity, and aims to **upstream the instrumentation (and possibly
  HRW) and become a Rumoca maintainer**. Mechanics: `hrw` branch cut from the pin `8cdc7419` (v0.9.20,
  the exact code HRW knew); `git subtree add --prefix=hrw` preserved HRW's full history under `hrw/`;
  MSL provisioned at `hrw/vendor/` (gitignored copy); `build.rs` now reads the commit from the
  workspace git HEAD (path deps carry no git rev). Disciplines: hooks are **additive, observation-only**
  (semantics-preserving → faithful to real Rumoca, clean rebases) and shaped to be **upstreamable**;
  keep HRW in `hrw/` and hooks separable so an upstream PR is a clean cherry-pick of Rumoca-only changes.
  Verified: `cargo build -p hrw` + `cargo test -p hrw` green (29 tests) in the workspace. "Updating
  Rumoca" is now **rebasing the `hrw` branch on upstream**, not a pin bump (see `docs/updating-rumoca.md`).

- **Pass two: re-implement Arcs 1–7 with internal Rumoca access, then the log view (2026-07-21).**
  Now that HRW is in-workspace, revisit each of the seven arcs to deliver *richer* stage views than the
  public-API-only pass one allowed, then build a log view as the payoff. Key mechanic: across a crate
  boundary a phase's `pub(crate)` internals aren't reachable, so "using non-public APIs" means
  **additively widening visibility / adding observation hooks in `../crates/rumoca-*`** — it *is* the
  instrumentation, held to the additive/observation-only/upstreamable discipline. Sequence Doug set:
  **(1) re-implement Arcs 1–7** (per arc: scout the crate → expose additively → render → re-wire into
  the per-stage systems + notebook), **(2) the log view** — compilation + simulation log with
  **timestamps** and far more phase/solver detail than was possible (per-phase timing was impossible
  when phases 5–9 come from one opaque `compile_model_strict_reachable_with_recovery` call). The log
  view is the deliberate *proof the migration was worthwhile*. Pass one is the baseline to surpass, not
  discard. Highest-confidence early unlocks: the **incidence-matrix view** (Arc 3 — deferred in pass one
  *because incidence was `pub(crate)`*) and the **real Pantelides/dummy-derivative process** (Arc 4 —
  pass one only replicated it via public `dae_prepare` and couldn't do nonlinear constraints). Remaining
  per-arc instrumentation opportunities captured in `docs/ideas.md` (#19–#22).

## Guided tours and backlog prioritization

- **2026-07-22 — Guided tours drive backlog prioritization.** The `docs/compiler-phases/` documents
  (written pre-HRW as standalone theory) are being re-envisioned as **guided tour scripts** —
  walkthroughs that leverage HRW + specimens to teach each phase interactively. The prioritization
  principle for the ideas backlog becomes: *build the feature that the next guided tour needs.* The
  workflow: design the tour → identify HRW gaps → build those features (from the backlog) → write
  the tour. Features must be general-purpose HRW enhancements, not tour-specific widgets. The
  backlog's prioritization table (top of `docs/ideas.md`) maps each item to the tour(s) that need
  it. Claude acts as a curriculum-aware product manager: designing tours, identifying prerequisite
  features, and sequencing work by learning value.

- **2026-07-22 — Algorithm trace instrumentation in Rumoca crates (matching + Tarjan).** Added
  `MatchingStep`, `MatchingFrame`, `maximum_matching_with_trace()` to `rumoca-phase-structural::matching`
  and `TarjanStep`, `TarjanFrame`, `tarjan_scc_with_trace()` to `rumoca-phase-structural::tarjan`. Both
  modules widened from `mod` to `pub mod`. The traces record every algorithmic decision (explore edge,
  find free variable, displace match, assign pair, visit node, back edge, SCC found) with a snapshot of
  the state at that moment. HRW replays these traces frame-by-frame in `matching_anim.rs` and
  `tarjan_anim.rs` (UI tabs "Matching ▶" and "BLT ▶" in the structural view). Design: the trace is
  computed **on-demand in HRW** from the parsed incidence data (no JSON bloat in the structural report
  or specimen traces), and the algorithm logic stays in Rumoca (no duplication). This is the first
  animated stepping (ideas backlog #9), proving the concept for future algorithms (Pantelides, tearing,
  Newton). The instrumentation follows the additive/observation-only/upstreamable discipline.

- **2026-07-24 — Simulate specimens in Wolfram System Modeler until Rumoca's simulator matures.**
  Rumoca's simulator (diffsol BDF + RK45) is not yet reliable enough for production use as a
  reference. Until it improves, develop a discipline of simulating each specimen in System Modeler
  as the ground-truth reference — use those results for learning, trajectory comparison, and
  validating Rumoca's output when it works. HRW's simulation tab and solver diagnostics (#29)
  remain in place for studying Rumoca's solver behavior, but System Modeler is the authoritative
  simulation tool for now.

- **Separate HRW VS Code extension for debugger bridge** (`hrw/vscode-extension/`). The `debug`
  chat shortcut now uses `vscode.debug.addBreakpoints()` via a file-watching bridge instead of
  rewriting `launch.json` `preRunCommands` — breakpoints are set on the running debug session
  without restart. A separate extension (not modifying the upstream Rumoca extension in
  `packages/vscode/`) avoids rebase conflicts, keeps HRW-specific features cleanly separated, and
  preserves the upstream contribution path. The extension watches `.hrw-bridge/breakpoint-request.json`
  for requests written by Claude during `debug`. Breakpoints **accumulate** per specimen and are
  cleared automatically when the specimen changes. The specimen list's context menu offers
  **Recompile** to re-run compilation and hit armed breakpoints (the worker calls
  `session.remove_document()` before `update_document()` to bypass the session's
  content-comparison cache).

- **2026-07-25 — Generic field help delivered as tooltips, not the RHS panel.** Tree node
  field help (doc-comment strings from `field_help.json`) now appears as hover tooltips on
  tree items — instant, zero-click, at the point of attention. The RHS panel previously
  displayed this same text on click, consuming an entire panel for a sentence of reference.
  Tooltips are the better delivery mechanism for this kind of "what is this?" help. With
  field help removed, the RHS panel had no remaining purpose and was removed entirely (below).
  Comprehensive tooltip coverage across other widgets is tracked as idea #33.

- **2026-07-25 — Right panel removed entirely; two-column layout.** With field help
  delivered as tooltips, the right panel had no remaining content — the "Read: phase
  chapter" and "Read: specimen narrative" buttons were its only other occupants, and
  their content is more naturally accessed in VS Code alongside HRW. Removed: the
  entire `right_panel` / `right_panel_field_help` / `right_panel_simulation` /
  `right_panel_read_links` rendering path, `show_right_panel` toggle, View menu
  checkbox, and `narrative_exists` field. The panel layout simplified from three
  columns (left + center + right) to two (left + center), giving the center panel
  more space. Net -341 lines across 4 files. The two-tier help model is now: fast
  tier = tooltips (field_help.json), specific tier = Claude bridge capture + "explain"
  chat shortcut.
- **2026-07-25 — Three UI modes: Tour / Specimen / Debug.** Replaced `show_left_panel: bool` with
  `UiMode` enum. Tour mode: 50/50 split, tour guide on left. Specimen mode: 50/50 split, specimen
  list (top third) + narrative (bottom two-thirds) on left. Debug mode: left panel hidden (VS Code
  alongside). Mode selector in View menu. Behaviour documented in `docs/architecture.md` § Panel layout and UI modes (design doc retired 2026-07-28).
- **2026-07-25 — Dependency added: `egui_commonmark 0.24`.** Renders markdown (tour content,
  specimen narratives) inside egui. No-default-features (image loading disabled — we embed no
  images). Used by both tour mode and specimen mode left panels.
- **2026-07-26 — Tour specimen changed: GearWithBrake → MotorWithBrake.** GearWithBrake's
  initialization fails (Rumoca limitation) and simulation produces all-constant trajectories — a
  spurious equilibrium from the failed IC solve. MotorWithBrake uses BenchActuator's proven EMF
  structure (which simulates dynamically despite the same IC limitation) plus a `when`/`elsewhen`
  speed-limit event. It exercises every compiler phase: index reduction (1 EMF demotion), events
  (2 conditions, 1 discrete update), and stiff dynamics (38/51 variables dynamic). GearWithBrake
  remains a valid specimen for studying index reduction with IdealGear; it is just no longer the
  tour specimen.
- **2026-07-26 — Index Reduction tab redesigned: Before/After split view.** The raw (singular) DAE's
  incidence and partial matching are now computed alongside the reduced system and embedded in a
  `"before"` sub-object in the stage JSON. The Index Reduction tab shows: (1) a status banner
  (Singular vs Index-1), (2) a Summary sub-tab (leftmost, full-width — the reduction funnel), (3) a
  Before/After split for the Incidence view, (4) Spy-plot, Matching, and BLT for the After system
  only (BLT/full-matching requires non-singularity), (5) Animate and Tree as full-width views.
  `matching::maximum_matching` widened from `pub(crate)` to `pub` in rumoca-phase-structural (additive,
  upstreamable). Summary-first tab pattern is a pilot for potential adoption on other phase tabs.
- **2026-07-27 — Live trace debugging repaired on Windows; five coordinated changes.** The Debug
  button's breakpoint never fired after the WSL2→Windows move. Root cause was not the bridge but
  the anchor itself: `LAST_FRAME_INDEX` was written by `live_trace_breakpoint` and read nowhere,
  so at the workspace's `[profile.dev] opt-level = 1` the store was dead-store-eliminated, the
  function became a bare `ret`, and the MSVC linker's `/OPT:ICF` folded it onto every other empty
  function in the binary (notably eframe's `App::raw_input_hook`). Breakpoints on the anchor
  therefore fired from eframe's render loop — reported, correctly, as "Paused on breakpoint" in an
  unrelated crate. `#[inline(never)]` does not prevent this: it protects the function, not the body.
  Fixes: (1) the anchor now has a body that survives optimization — a real reader
  (`last_frame_index`) plus `black_box`, guarded by `breakpoint_anchor_store_is_observable`;
  (2) `[profile.dev.package.rumoca-phase-structural] opt-level = 0`, which *lowers* opt-level for
  debuggability (opposite in purpose to the four speed overrides above it) and restores readable
  locals; (3) `WGPU_BACKEND=gl` in the launch configs — a D3D12 device does not survive the long
  pauses live trace depends on, and the loss surfaced as an `egui-wgpu` staging-buffer panic on the
  main thread with exit code 101; (4) a breakpoint pre-warm (`App::tick_prewarm`) that arms and
  removes the anchor once at startup, so the first Debug click does not pay for the cold line-table
  load — previously the first click of every session missed and the second worked; (5) all-threads
  stepping aliases (`ns`/`si`/`so`) committed to `.vscode/launch.json`, having previously lived in
  an untracked `~/.lldbinit`. Full write-up in `docs/architecture.md` § Live trace debugging on
  Windows; setup in `README.md`.
- **2026-07-27 — `ms-vscode.cpptools` / `cppvsdbg` kept as a secondary debug adapter.** Added while
  CodeLLDB was (wrongly) suspected of misreading PDB debug info. It reproduced the fault
  identically, which is what proved the problem lay in the binary rather than in either reader.
  Retained because it reads PDB natively and reports moved breakpoints honestly, where CodeLLDB
  silently kept a stale entry — a useful cross-check. CodeLLDB remains primary: it has the
  Rust-aware formatters and the thread run-mode control that all-threads stepping requires, so
  charter Decision 6 and the `CLAUDE.md` debug stack are unchanged.
- **2026-07-27 — Root `.vscode/` configs force-added.** `.gitignore:18` excludes `.vscode/`, so the
  launch/tasks/settings files that live trace depends on were untracked and would not have survived
  a clone. Tracked via `git add -f`, the same mechanism by which `hrw/.vscode/` is already tracked,
  rather than editing upstream's `.gitignore` — which keeps the upstream rebase workflow clean. Any
  new file under a `.vscode/` directory needs `git add -f`.
- **2026-07-27 — `hrw/README.md` added.** Setup guide for reproducing the environment on a fresh
  Windows machine: toolchain, the gitignored MSL vendor staging step, the gitignored VS Code
  extension build step, required launch settings, and a failure-signature table. Supersedes the
  setup checklist in `README.md` (the migration doc was retired 2026-07-28; its diagnosis lives in `docs/architecture.md`).
- **2026-07-27 — Bridge targets the anchor's body statement, not its signature.** Debuggers skip a
  function prologue, so a breakpoint requested on the `pub fn` line resolves one line lower. The
  bridge and the debugger therefore disagreed about the location, and a bridge-armed breakpoint sat
  alongside a hand-set one as a second entry in VS Code's Breakpoints list. `find_live_trace_line`
  now walks signature → opening brace → first non-blank, non-comment line, structurally rather than
  by a fixed offset, so it survives edits to the anchor (guarded by
  `find_live_trace_line_targets_first_body_statement`). Complementary change in the extension:
  `isDuplicate` now checks all of `vscode.debug.breakpoints` instead of only the ones it armed, so a
  hand-set breakpoint suppresses the bridge's — and because `handleRemove` only removes what the
  extension added, the user's breakpoint survives the end of the session.
- **2026-07-27 — Playback controls return when a live session finishes (`playback_applies`).**
  Play/Pause and the speed slider were gated on `!is_live`, and `live_rx` is never cleared once
  `start_live` sets it — so `is_live()` stays true for the animation's lifetime and the controls
  vanished permanently after any use of the Debug button. Since the three-tier design (snapshot →
  replay → live trace) exists so the tiers reinforce each other, losing replay as the price of live
  trace was backwards. Playback is now suppressed only *while* a session runs
  (`playback_applies(is_live, live_finished)`), which had to be applied both to the control row in
  `lib.rs` and to the `playing && !is_live()` advance guards in all three animations — changing only
  the former would have rendered a Play button that did nothing. The "Live (done)" badge stays, since
  it is still true and worth knowing.
- **2026-07-27 — Animation controls survive the empty-frames state; `live_done` normalized to
  `true` for recorded animations.** Two loose ends from the playback fix. (1) All three animations
  early-returned when `frames` was empty, *before* rendering the control row — so while a live
  session sat at the startup gate waiting for the first Continue, the entire row vanished, Reset
  included, leaving no way to back out. The controls now render first and the empty case returns
  after them; `frame_label` shows "No frames yet" rather than "Frame 1/0". Everything downstream was
  already guarded by `if let Some(frame) = self.current_frame()`, so no other code had to change.
  (2) `MatchingAnimation`/`TarjanAnimation` initialized recorded animations with `live_done: false`
  while `ReductionAnimation` used `true`. `true` is correct — for a recorded animation the honest
  answer to "is a live session still running?" is no — and `live_debug_lifecycle` relies on it as a
  breakpoint-cleanup safety net, releasing an armed breakpoint when no live session is coming. With
  `false` that net was inert for two of the three views, so a stray armed breakpoint could leak.
- **2026-07-27 — UI rule: controls are enabled/disabled, never shown/hidden (`LiveState`).** Doug:
  "Buttons should not be disappearing. Instead, buttons should be disabled (or enabled as
  appropriate)." A control that vanishes gives no clue the action exists or why it is unavailable,
  and the row reflows under the pointer as it goes. The Debug button and the whole playback row now
  render unconditionally, with `add_enabled` and `on_disabled_hover_text` explaining the state.
  Driven by a new `LiveState` enum (`Idle` / `Arming` / `Running` / `Finished`) replacing the
  `is_live` + `live_finished` boolean pair, which could not express two of the four states: `Arming`
  (the breakpoint handshake takes several frames during which the view still holds the *recorded*
  animation, so `is_live()` is false and controls stayed enabled right after the click) and the
  distinction that makes `Finished` re-enable everything (`live_rx` is never cleared, so `is_live()`
  stays true for the animation's lifetime). `arming` cannot be derived inside an animation — it comes
  from `App::is_arming`, which checks whether `pending_live_debug` names that view's algorithm.
  Starting a session now also clears any timed playback, so it cannot resume when the session ends.
- **2026-07-27 — Debug button moved onto the animation control row.** Layout is now
  `[Play/Pause] [Reset] [Back] [Step] | Frame n/m | Speed | [Debug]  <badge>` — playback, then the
  frame/speed group, then a divider, then the live debug controls, with the status badge moved from
  the head of the row to the tail. The Debug button is rendered by `animation_controls` rather than
  by the caller so the row stays together, but it cannot arm a session itself (that needs the
  bridge, the model name, and `pending_live_debug`), so it returns its click and `app.rs` acts on
  it. `live_debug_lifecycle` accordingly split into `live_debug_poll` (handshake only, renders
  nothing) and `start_live_debug` (invoked on the click), plus `has_live_debug_data` for the
  enablement test. The `ui()` of all three animations and `animation_controls` are `#[must_use] ->
  bool` so a dropped click is a compile error. Known consequence: when a view has no data at all,
  there is no control row, so the Debug button is absent rather than present-and-disabled — accepted
  because the entire animation UI is absent in that state, not just one control.
- **2026-07-27 — Stage tab renamed "Typecheck (instanced)" → "Typecheck".** The parenthetical made
  the tab the widest in the bar for a distinction almost no reading of the tab needs. The nuance it
  carried is unchanged and still one hover away: the tooltip continues to explain that this is the
  model-scoped instanced typecheck, running *after* Instantiate rather than in Rumoca's nominal
  phase-3 slot. Amends the label choice in the 2026-07-19 pipeline-order entry above; the ordering
  decision itself is unchanged.
- **2026-07-27 — Compiled-model name removed from the stage tab row.** The row was short on
  horizontal space and the same identity is already visible in the specimen list and the tree
  breadcrumb, so the label and its leading separator are gone. **`self.model` itself is unchanged
  and must stay** — it is compilation state that the label happened to display, not display state:
  it supplies `model` in the Claude bridge's `focus.json`, the specimen key for
  `arm_live_trace_breakpoint`, the gate for capture actions, the specimen-notebook narrative lookup,
  and the tree root label/breadcrumb. Removing it would silently degrade the bridge. A comment at
  the former label site records this.
- **2026-07-27 — Index-reduction animation opens on a `Start` frame.** The replay began on the first
  `BeginState` ("Round 0: searching for a constraint to differentiate for state emf.phi"), which
  announces an *intention* and so read as though reduction had already happened on opening the pane
  — and no frame anywhere showed the unreduced system, which is precisely what an animation about
  what reduction *changed* has to establish. `IndexReductionStep::Start { states, equations }` plus
  `emit_index_reduction_start` (crate commit 09634b15) record it; HRW emits it at both trace sites
  (recorded, in `worker.rs`; live, in `reduction_anim.rs` so the first Continue after the startup
  gate lands on it) and renders it as "Starting point: N states, M equations — nothing reduced yet"
  with a states table laid out like the "Demoted states" table so before/after read as a pair.
  Emission is an explicit call rather than something the traced passes do for themselves: the
  pipeline runs two traced passes back to back and only their combined output is one animation, so
  self-emission would put a second start mid-replay. The snapshot is the DAE *as tracing begins*,
  not the raw DAE — `demote_exact_alias_component_states` and `demote_direct_assigned_states` run
  untraced beforehand — so it is labelled "starting point" rather than "original system".
- **2026-07-27 — Tracked-identifier matching excludes prose fields; Modelica text shares one visual
  language.** Two fixes in the same family as the lexer work. (1) `tree.rs` matched the tracked
  identifier against every `Value::String` in the IR, including `description` — so `Real h "height
  of h"` highlighted the description and expanded the path to it when tracking `h`, claiming a use
  where the variable is only talked about. Fixed by field, not by content: `PROSE_FIELDS`
  (`description`, `comment`, `file_name`) are excluded. Lexing would be the wrong tool here — those
  strings are not Modelica, so tokenizing them would be a category error; what matters is which
  field the string came from. The list is deliberately short because listing a field wrongly hides
  real matches, the worse failure. (2) The equation sheet rendered equations as plain monospace and
  tested `eq.text.contains(tracked)` — a raw substring match that shaded whole equations containing
  `height` when tracking `h`. Equations are `expr_format` output and therefore Modelica-shaped, so
  they now route through `source_view::modelica_job`, sharing the source view's syntax colouring,
  with the tracked identifier highlighted per token rather than by tinting the row. Canvas-painted
  axis labels (incidence rows, Tarjan nodes) still render unstyled — per-token colour there means
  measuring and placing runs by hand, which is a larger change.
- **2026-07-27 — Colour rule: foreground carries syntax, background carries relationship.** Applying
  syntax highlighting to the Flatten source map exposed a channel conflict rather than an oversight:
  its equation column used *foreground* colour (`cat.color()`) to mark equations linked to the
  selected source line, so syntax colouring would have put "this is a keyword" and "this is
  selected" on the same channel, with the selection cue losing. Settled it as a rule instead of a
  workaround — relationship cues (selected, line-linked, tracked) move to background; foreground
  means syntax everywhere. The source-lines column already worked this way, so the linked cue now
  reads identically on both sides of the view, and the category is still carried by the group header
  above each block. `modelica_job` became the `ModelicaText` builder in the same change: it was
  about to reach six positional parameters including two adjacent colours, exactly the transposable
  signature logged as tech debt for `animation_controls` earlier the same day. Line-number gutters
  go through `append_plain` so they are never coloured as if they were code.
- **2026-07-27 — Highlight tints were premultiplied wrongly; and a relationship cue must actually
  vary.** Two corrections found by testing the source map. (1) `TRACKED_FILL`,
  `TRACKED_FILL_MEDIUM`, and `SOURCE_MAP_LINK` used `Color32::from_rgba_premultiplied` with
  full-strength RGB and a low alpha. Premultiplied form requires every channel to be **≤ alpha**, so
  these rendered as near-opaque additive washes rather than the faint tints their alphas implied.
  Harmless while the text underneath was uncoloured; the moment syntax colouring landed the wash
  buried it and no syntax colour was distinguishable. Fixed with a `const fn tint()` doing the
  premultiplication (`from_rgba_unmultiplied` is not `const` in this egui version). This makes every
  tracked highlight across the app genuinely translucent — the alphas now mean what they say.
  (2) The source map's flat-equations column carried a "linked to the selected line" cue — first as
  `cat.color()` foreground, then, in the Option A change, as a background. It never conveyed
  anything: that list is already *filtered* to the selected line's equations, so the cue was true of
  every visible row and false of none. Removed. The filter and the "N equations from line X" header
  are the signal. The source-lines column keeps its highlight because it is *unfiltered*, so there
  its cue genuinely picks out a subset. Sharpens the colour rule: background carries relationship,
  **and a relationship cue is only worth a channel when it varies across what is on screen**.
- **2026-07-27 — Trackability by ground truth; "Reveal identifiers"; declaring-class lookup.** Three
  changes to make "where did this come from?" work from any stage. (1) `trackable_name` first
  decided *syntactically* — any string shaped like a dotted identifier — which in a real Flatten IR
  marked `causality: "None"`, `op: "Add"`, and `quantity: "Angle"` as trackable and offered to track
  them. Roughly half the marks were meaningless, and when everything is marked nothing is: that
  over-marking *was* the discoverability problem Doug reported. Trackability now asks the model —
  the set of variable names from `EquationSheet.variables`, which includes library-origin variables
  that `IdentifierIndex` omits. No compiled model means nothing is offered; a wrong offer is worse
  than none. (2) A "Reveal identifiers" checkbox expands every path leading to a variable. This
  needed `CollapsingHeader::open(Some(true))`, not `default_open`, which applies only the first time
  a header is shown and is ignored once egui has stored its state — so the checkbox initially did
  nothing. The two reasons to expand are kept distinct in `Expansion`: the reveal toggle *forces*
  (an explicit mode, untick to regain control), tracking only *suggests* (it persists, so you must
  still be able to collapse). (3) `build_declaring_classes` resolves a flat name's first path
  segment against the model's components to the class that declares it, so `src.V` reports
  "in `Modelica.Electrical.Analog.Sources.ConstantVoltage`" with a Go-to-definition button reusing
  the existing nav stack, instead of the previous dead end. Only the first segment resolves —
  deeper names give the containing component's type — so the wording is "in", never "declared in".
- **2026-07-27 — Glyphs are limited to ones this codebase already renders.** The tracking bar's
  clear button used `\u{2715}` (✕ MULTIPLICATION X), which none of egui's bundled fonts covers, so
  it drew as a tofu box. HRW already makes every bundled font a fallback for both families, but
  that only helps when *some* bundled font has the glyph. Changed to `\u{00d7}` (×), which is
  Latin-1 and present in every text font including the monospace one. The "Track" menu item added
  the same day used `\u{25ce}`, chosen without checking — also replaced, with `\u{1f3af}` (🎯),
  already rendered by the Tarjan view, and deliberately not a magnifier since 🔎 Capture sits
  directly above it in the same menu. **Rule: pick glyphs already used elsewhere in `hrw/src`;**
  anything else is a silent tofu risk that only surfaces in manual testing.
- **2026-07-27 — Capture and tracking are one concept: context assembly; the "Tracking" bar becomes
  a Context Bar.** Doug, on testing Phase 4: the difference between captured and tracked has no
  better answer than *point versus thread* — capture is one node with its provenance, tracking is
  one identifier across every stage. Both exist only to assemble context for questions to Claude.
  The UI treated them asymmetrically for no reason: capture is persistent state shown transiently
  (a status line), tracking is persistent state shown permanently but **never emitted**. So the one
  thing actually in Claude's context was invisible, and the visible one was not context at all.
  The Context Bar replaces the Tracking bar and is governed by one rule — **it renders what will be
  emitted, nothing more and nothing less** — which makes it honest by construction and means it
  cannot exist until tracking emits. Compound capture and Context Bar are therefore one piece of
  work, and that work is now Phase 5. Design in `docs/context-assembly.md` (renamed from
  `tracking-as-capture.md`).
- **2026-07-27 — The tracking bar carries context and the controls that change it; navigation moves
  out.** Testing Phase 4 raised the question of whether the bar's "Go to definition" button belongs
  in the Context Bar that will replace it. It does not: the bar's job is to state what Claude will
  receive, and navigation changes nothing about that. The `[x]` clear buttons *do* belong, because
  they mutate what gets emitted. But deleting the button outright would have made the action
  unreachable when tracking was started from the equation sheet or source view, where there is no
  tree row to right-click. Resolution: **the fact stays, the chrome goes** — the declaring class is
  rendered as a link rather than as text plus a button, so the bar reads as context and one of its
  facts happens to be navigable. Separately, `nav_target` now resolves a variable name to its
  declaring class, not just DefId fields, so "↪ Go to …" appears in the tree row menu for
  identifiers too — navigation lives with the other actions, and the same menu item means the same
  thing whether the class was found through a DefId or through the variable.
- **2026-07-28 — Phase 4 complete; canvas views deferred to a new Phase 7.** Reverse identifier
  tracking is done for the widget-based views: one `set_tracked_identifier` entry point, scroll-to-
  line on change, following from the equation sheet and from the IR tree's row menu (the tree being
  on every stage tab, which makes the gesture ambient), trackability decided by the model rather
  than by string shape, the declaring-class lookup, and the rule that tracking never answers with
  silence. The canvas-painted views — incidence and spy-plot row/column labels, Tarjan nodes,
  reduction rows — are **deliberately postponed**. Doug's reasoning: wiring them now risks "a code
  mess ... that we would have to clean up later", because Phase 5 turns every tracking entry point
  into an *emission* point, and Phase 6 may change what a label even is. Phase 7 pairs that work
  with idea #38 (syntax highlighting for canvas labels) since both require labels to stop being a
  single `painter.text` call and become laid-out, measured, hit-testable runs — one code region, so
  one phase. **Phase 4's more valuable outcome was diagnostic**: it exposed that capture and
  tracking are one concept wearing two vocabularies, that the UI showed emitted context transiently
  and un-emitted context permanently, and that "where did this come from?" is not always a source
  line. Phases 5 and 6 exist because of what Phase 4 turned up.
- **2026-07-28 — Heuristic name-matching removed; the tracking plan's principle is now honoured.**
  `docs/cross-stage-tracking-plan.md` opens with *"No heuristic name-matching. All mappings use
  Rumoca's typed provenance"* — and `matches_tracked`, used by every stage view to decide
  highlighting, was a whole-word **substring search**. Digging in showed why: the structural report
  Rumoca emits carries *names only* (`"unknown": "src.n.i"`), no `def_id`, so the views genuinely
  had only strings. But the heuristic was never "using names" — flat names are canonical and name
  exactly one variable. It was **fuzzy search over them**, and `matches_tracked` conflated two
  different questions. Identity ("is this unknown the tracked variable?") is now `same_variable`:
  exact equality modulo one `der(...)`, which is all the fuzziness ever bought. Membership ("does
  this equation mention it?") is answered **structurally** where the data exists —
  `tarjan_anim::equation_mentions` reads the incidence matrix's `rows[eq]`, which the structural
  phase already computed, instead of substring-searching pretty-printed equation text. Where a
  caller holds only text, `source_view::mentions_identifier` asks the **lexer**: `height` is one
  token and not a mention of `h`, and identifiers inside string literals and comments are excluded.
  `matches_tracked` and its word-boundary scanner are deleted — the heuristic is gone rather than
  contained. Also consolidated three copies of `der(...)`-stripping into
  `identifier_index::strip_der`.
- **2026-07-25 (recorded 2026-07-28) — Identifier tracking is an HRW interaction, not a VS Code
  extension one.** Carried forward from the retired `cross-stage-tracking-plan.md`, where it was a
  design decision with no DECISIONS entry. A VS Code-extension path for selecting and highlighting
  identifiers was evaluated and rejected: context selection belongs where the IR is rendered, and
  the extension's job is narrower — arming breakpoints on request, which it still does. Keeping the
  interaction in HRW is also what lets the source view, the stage views, and the bridge share one
  notion of what is being followed.
- **2026-07-28 — Four completed-plan documents retired.** `end-to-end-tour-plan.md` (initiative
  complete), `ui-modes-design.md` (three-mode UI shipped; the *behaviour* is documented in
  `architecture.md` § Panel layout and UI modes, and the decisions already have entries here),
  `windows-migration.md` (migration complete; the durable diagnosis lives in `architecture.md`
  § Live trace debugging on Windows and the setup in `README.md`), and
  `cross-stage-tracking-plan.md` (superseded by `source-tooling-plan.md`). Durable content was
  folded out first, not discarded: the **no-heuristic-name-matching principle** and the **inventory
  of provenance Rumoca preserves** moved into `source-tooling-plan.md`, and the HRW-not-extension
  decision is recorded above. **Rationale for deleting rather than archiving:** git holds every
  deleted file, so nothing is lost — but nobody greps deleted files, so stale plans in `docs/` cost
  attention on every read and risk being mistaken for current intent. `DECISIONS.md` already *is*
  the project's history; a second archive would compete with it and be read by no one.
- **2026-07-28 — A mention is the whole dotted path, not a matching leaf; and instrumented Rumoca
  crates must stay clippy-clean.** Two corrections found by testing the Context Bar.
  (1) The first real `explain` on `__pre__.overSpeed` emitted **four mentions in Parse and Resolve**
  — stages where that variable does not exist. `source_view::identifier_is` accepted a bare
  `overSpeed` because the tracked name ends with `.overSpeed`, and `mentions_identifier` used it
  per-token. Membership now reconstructs the dotted path around each identifier token
  (`dotted_path_ending_at`) and compares the **whole path** with `same_variable`, so `a.phi` is not
  a mention of `b.phi`. `identifier_is` survives, documented as deliberately leaf-tolerant and
  restricted to *highlighting a token inside text already identified as relevant* — never to decide
  whether text mentions a variable. **The governing rule (Doug):** correctness in the Context Bar
  and emitted context outranks reach — the reasoner can relate `overSpeed` to `__pre__.overSpeed`
  from the names, but it cannot recover from a false count. One shared implementation of the path
  walk now lives in `source_view`; `identifier_index::clickable_spans` calls it.
  (2) Checking this with clippy showed the **instrumentation had introduced six clippy errors** into
  `rumoca-phase-structural`, which is clippy-clean at the fork point (verified against `8cdc7419` in
  a scratch worktree). Rumoca denies these via `[workspace.lints]`, so an upstream PR would have
  failed CI — a direct violation of the *upstreamable* half of the instrumentation discipline. All
  six are fixed in the algorithm crates, not silenced: `IndexReductionStep::Differentiated` boxes
  its two `Expression`s (they made every frame in a trace pay 360 bytes), the traced
  missing-derivative pass takes `demoted_so_far: &[String]` (it reads the accumulator; only its
  demoting sibling extends it), the per-state candidate filter is extracted as
  `differentiable_candidate_equations`, Tarjan's SCC pop is extracted as `pop_scc`, and the traced
  augmenting path uses `continue` guards. **Standing rule: run `cargo clippy -p <rumoca-crate>`
  after touching an instrumented Rumoca crate** — `cargo test` passes right through these.
- **2026-07-28 — The lexer scans characters, not bytes, in its catch-all arm.** Clicking `overSpeed`
  in MotorWithBrake's source crashed HRW instantly. `modelica_lex::tokenize` matches ASCII in every
  arm, so any non-ASCII character fell to the catch-all, which advanced **one byte** — splitting a
  3-byte em dash into three tokens with boundaries inside the character. `source[tok.start..tok.end]`
  then panicked. The trigger was not the specimen: following an identifier lexes every code-bearing
  string in every stage's IR, and Rumoca's structural note reads *"…before it can be solved — see the
  Index Reduction tab."* **The lexer is the right place to fix it, not the slicing callers** — a
  defensive slice at each call site would hide the next such bug, while the guarantee "token
  boundaries fall between characters" is checkable once and relied on everywhere. It is now stated in
  the module docs alongside the tiling guarantee and asserted inside `assert_tiles`, so every
  existing corpus test enforces it too. **Why the tests missed it:** they lex Modelica, and Modelica
  is ASCII — `non_ascii_inside_comments_and_strings_is_safe` put the accents inside comments and
  strings, where scanning is delimiter-driven and never reaches the catch-all. Prose written by the
  *compiler* is the text that carries an em dash, and it only reaches the lexer because Phase 5's
  following searches IR strings. New tests cover bare non-ASCII directly and cover the whole
  click path on real IR (`following_an_identifier_walks_every_stage_without_panicking`).
- **2026-07-28 — HRW writes a crash and diagnostic log, designed for Claude.** Doug, after the
  em-dash crash: *"you have done an amazing job of troubleshooting problems. But, that might not
  always be possible… we have skipped a step in creating HRW."* The gap is not "we lack a log" but
  **when HRW dies, the evidence dies with it** — it is a windowed app, so a panic's stderr usually
  goes nowhere, and the crash was diagnosed only because that particular path happened to be
  re-creatable headlessly. A crash in the paint path or one depending on GPU state would have left
  nothing. Two design choices are worth recording because both invert the obvious one:
  (1) **the backtrace is the less useful half.** Location says where the process died; it rarely
  says why the app was there, and *that* is what costs hours to reconstruct. So the centrepiece is
  `App::diagnostic_snapshot` — specimen, model, stage tab, detail view, nav stack, the assembled
  noun, live-trace arming, which animation at which frame, which stage IRs exist. The field list is
  the 2026-07-28 debugging session's findings turned into code, not a guess.
  (2) **an action ring buffer, because the cause is usually the action before last.** State alone is
  a still photograph; the buffer is a reproduction script. Recorded at four choke points, not at
  every UI site.
  Two files: `crash-<utc>.json` from the panic hook, and `session.json` rewritten per user action so
  that deaths running *no* hook (stack overflow, driver `SIGSEGV`, hard kill) still leave something.
  `Help ▸ Write diagnostic snapshot` covers problems that never kill the app — Doug asked for
  "crashes *and other problems*". Written for Claude, so nothing is summarised
  away — same rule as [the emitter principle above]. `build.rs` gained `HRW_GIT_DIRTY`: without it
  the stamped rev is actively misleading mid-session, naming a commit whose code is not what ran.
  Date formatting is hand-rolled (Hinnant's `civil_from_days`) rather than taking a date
  dependency for two format strings. `examples/crash_probe.rs` verifies the panic path, which
  **cannot** be a unit test — the harness installs its own hook and catches the unwind.
- **2026-07-28 — The capture carries the IR *around* an address, what was on screen, and where the
  phase code lives.** Doug: *"Design the context captures in a way which will best enable you to
  answer my 'Explain' questions. You are the consumer of context, not me."* Five changes, each
  measured against what the first two real `explain` answers cost to produce rather than guessed at:
  (1) **Mentions carry a neighbourhood.** An address alone makes the reader open the stage file *and*
  already know what to look for. Following `__pre__.overSpeed` gave the path
  `discrete_updates.valued_updates_f_m[0].rhs.If.else_branch.VarRef.name.name`, and the decisive fact
  — `generated: true` on the object above the leaf, meaning the variable is *manufactured by the
  Events phase* — took four reads of `events.json` to find. `enclosing_context` now spends a byte
  budget **greedily upward**, returning the largest ancestor that fits, so a small leaf brings its
  whole equation and a leaf in a 1014-entry map brings just its parent. No per-case rule needed.
  (2) **A sibling *window centred on the hit*, not the first N.** The other decisive finding —
  Solve lowering makes a `__pre__` companion for everything the event logic samples — came only from
  keys adjacent to the hit. The first 40 keys of that map would have said nothing. Position is an
  exact fact, so this stays inside the emit-facts-not-interpretation rule. Verified against real IR:
  the window now contains `__pre__.c`, `__pre__.c[1..2]`, `__pre__.load.w`, `__pre__.maxSpeed`.
  *(Where those slots come from was misattributed until 2026-07-29; see the idea #40 entry.)*
  (3) **`view`** — a point made in a tree and one made paused at animation frame 12 previously
  produced *identical* files, though in the second case the frame is most of the question.
  (4) **`phase_source`** — stage → crate + entry function, so the algorithm can be read rather than
  inferred from its output. That is what the in-workspace move was for. A test asserts every named
  crate directory exists, because a confidently wrong pointer is worse than none.
  (5) **The caps re-justified on need, not file size.** `MAX_NODE_BYTES` 16 KiB → 256 KiB: the old
  limit was perverse, degrading the *largest and most interesting* nodes to a list of key names.
  `MAX_MENTION_PATHS` 12 → 40, plus a second tier `MAX_MENTION_CONTEXTS` = 6 — `paths` answers
  *where*, `contexts` answer *what it looks like*. `MAX_CHANGES` stays 400, and the asymmetry is
  deliberate: the others bound how much surrounding IR to carry, where more is better; that one
  bounds a list of differences, and a 400-entry diff has stopped being informative anyway.
  `find_mentions` now collects `Vec<Seg>` rather than rendered strings — a formatted `a.b[0].c`
  cannot be navigated back, and re-parsing one is guesswork the moment a key contains a dot, which
  in `bindings.__pre__.overSpeed` it does. `examples/capture_probe.rs` prints the capture for a real
  specimen, because whether it carries what a reader would hunt for is answered by *reading the
  emitted value*, not by a shape assertion.
- **2026-07-28 — "Point at" and "Follow" everywhere; the status bar loses its bridge role.** The
  menus said "Capture" and "Track" while the Context Bar said "Pointing at" and "Following" — two
  vocabularies for one concept, and the bar's were the better ones (see `context-assembly.md`).
  Renamed in **UI labels, hover text, help, and docs**; the wire format and internal identifiers
  (`focus.json`, `Focus`, `AskRequest`, `TreeActions::capture`/`track`) deliberately stay, because
  renaming a protocol Claude already reads buys nothing and breaks continuity with recorded
  sessions. The `instructions` string inside `focus.json` *is* updated, so a reader of that file
  does not meet a third vocabulary. Glyphs: only codepoints HRW already renders (U+2715 was a tofu
  box here once), so 🎯 goes to "Point at" and 🔎 to "Follow" — and the magnifier fits *better* on
  Follow, since following is a search across every stage in which absence counts as much as
  presence. **The status bar's confirmation is gone**: `status_line` returns `Option<String>` and a
  successful point returns `None`. It was not merely redundant with the bar — it stated the point
  once and then went stale, so two surfaces claiming to describe what Claude has could disagree,
  which is the failure this whole design keeps meeting. Two exceptions survive because the bar
  cannot express them: an emission **failure** (silence would leave the bar describing context that
  was never written) and the **debugger** request (it asks the user to do something next, and an
  instruction is not a confirmation). `bridge_status` is renamed `notice`, its remaining traffic
  being genuinely transient — "specimen not found", "diagnostic written to …", a stage-file write
  failure.
- **2026-07-28 — The surface determines the verb; the hover says which, and admits the send.** Doug,
  testing: left-clicking a source identifier *follows*, left-clicking a stage tab *points at* —
  "is that merely a leftover of our previous design, or is that intentional?" Both. The rule is
  coherent — **where every clickable thing is a name, following is the only thing a name affords;
  where they are nodes, most are not names at all** — and underneath it sits a hard constraint: a
  source token has no IR address, so "point at" is not even expressible in the source view. But it
  was never *decided*: the source-view click came from Phase 3/4's reverse tracking, before the two
  primitives were named. Now written into `context-assembly.md`.
  Doug then made the sharper point: *"It's as though we are conflating 'point at' with 'select'… a
  mere point-at doesn't really suggest a side effect."* True — every left-click writes `focus.json`.
  **"Select" was rejected anyway**, for two reasons. It misdescribes in the other direction
  (selection is expected to be free, local and private; this is none of those), and taken seriously
  it is a *behaviour* change rather than a rename: it implies a later explicit **Send**, which costs
  a click per question and reintroduces the bar/file disagreement this design has already fixed
  twice. Worth recording *why* the publish is eager: in classic noun-verb, selection can be free
  because the app sees the verb — here the verb is typed in another process, so HRW never learns a
  question was asked and must publish speculatively on every change to the noun. **Eager publication
  is forced by the split that makes the paradigm work.**
  The fix is therefore neither renaming nor re-architecting: the **gesture announces itself**.
  `follow_hover` / `POINT_AT_HOVER` live in `lib.rs`, shared so the wording cannot drift across the
  five surfaces, and they name the verb *and* admit the send. The Context Bar answered "which did I
  just do?" afterwards; the hover answers it beforehand, which is where the ambiguity lived. Tree
  hovers **append** to the field's Rumoca doc rather than replacing it — field help is the fast,
  no-AI tier, and burying a real answer under directions for asking one is a bad trade. The variable
  grid's hover also still read "track" after the rename; routing it through the shared helper fixes
  that and prevents the next drift.
- **2026-07-28 — The point is clearable, and clearing it emits `kind: "none"` rather than the current
  stage.** Doug, testing: *"I am currently pointing at something. How do I clear that so that I am
  pointing at nothing? I'd like to ask for an explanation of only what I'm following."* He could
  not. The Context Bar's Following row had a `×`; the Pointing at row had none, so a point could only
  ever be **replaced**, never removed — `pointed_at = None` happened in exactly one place, `open()`,
  meaning the only escape was reloading the specimen, which recompiles and discards everything
  including what he was following. A plain asymmetry, invisible from the code and caught in a minute
  of use.
  The emitted half is the load-bearing one. The no-point path already existed (following with
  nothing pointed at is normal) but emitted `Focus::Stage` — which claims *"pointing at the Typecheck
  stage as a whole"*, a claim the user makes by clicking a tab and **not** one they made by clearing.
  Attributing it would be the confident lie the whole design exists to prevent, so `Focus::Nothing`
  was added, emitting `kind: "none"`. Deliberately **not** `Ask { focus: Option<Focus> }`: absence
  must be *stated, not implied* — an omitted field reads as "unknown", `kind: "none"` reads as
  "deliberately empty, the thread is the whole subject". Same reasoning as `mentions: 0`. The
  `instructions` string says so explicitly, including *"do not fall back to describing the current
  stage"*, since a reader who guesses would guess wrong in the one case the user was most explicit
  about. `emit_focus` guards against `Nothing` by returning rather than panicking — a UI path must
  not abort the app to report a programming error.
- **2026-07-28 — The empty context states itself, and `request` is a property of the point.** Doug:
  *"What happens if neither a follow nor a point-at are set, and I request 'debug'?"* Nothing bad —
  no breakpoint can be armed on nothing, since `arm_live_trace_breakpoint` belongs to the animation
  Debug button and `debug-where-set` can only come from a tree row, which necessarily creates a
  point. But the question exposed two places still practising **absence by implication**, the thing
  this design eliminates everywhere else:
  (1) **The Context Bar vanished when nothing was assembled**, making "I have nothing" indistinguishable
  from "the bar is not rendering" — and that is precisely the state a user is in just before asking a
  question that quietly has nothing behind it. It now says so, once a specimen is loaded (before
  that there is genuinely nothing to say, and the status bar carries the opening hint).
  (2) **`request` defaulted to `"explain"` with no point**, claiming an intent never expressed. It is
  a property of the *point* — "explain this node" versus "show me where this node gets set" — so it
  is now `null` whenever `kind` is `"none"`. Null rather than omitted, for the same reason
  `kind: "none"` beats a missing `kind`. The `instructions` string says so, and adds the rule that
  matters most for this state: **if both are absent, say nothing has been assembled rather than
  answering from `stage` or from whatever was captured before.**
- **2026-07-28 — Jump to the followed identifier, rather than search for it.** Doug: *"That
  ['Reveal identifiers'] checkbox has never really delivered the benefit which I had hoped for. Even
  after I check the box, I struggle to find the identifier which is being tracked."* He proposed a
  tree search feature (idea #11). **Search was the wrong shape for this problem**, and the reason is
  worth keeping: search asks the user to *type what they are looking for*, but HRW already knows —
  they said which identifier they are following. The missing piece was never the query, it was the
  **jump**.
  Why Reveal failed, precisely: it is a *mode* that expands every path leading to **any** trackable
  name, so to surface one identifier it surfaces N. It makes the haystack bigger. It was built to
  answer "what can I follow here?", and that question got a better answer later anyway — the
  `known_variables` ground-truth fix that stopped underlining `op: "Add"`. Left in place for now
  rather than removed on a prediction; Phase 6's "reveal as action not mode" is where it gets
  settled, and `Expansion::force_open` is already logged in `tech-debt.md` as existing only for it.
  **The design constraint that mattered most: one match list, not two.** `bridge::mention_paths`
  wraps the same walk that produces `tracking.paths` in the emitted context, with the cap
  parameterised (the emitted list is sampled for a reader; the jump list is uncapped so cycling can
  reach every occurrence). A separate matcher written for the UI would have been a second definition
  of *mention* — the app highlighting one set of nodes while telling Claude about another, which is
  the drift this phase was spent removing. A test asserts the two lists are identical node for node.
  Mechanics worth knowing: `jump_to` is set for **one frame**. Forcing a header open with
  `open(Some(true))` also *stores* that state, so after the jump the ancestors stay open on their own
  and remain collapsible; held longer it would re-scroll every frame and take the headers out of the
  user's hands — the Reveal complaint again. The jump also clears `viewing_log`, since the matches
  live in a stage IR and a jump with the log showing would look broken. `0 matches` renders as "not
  in <stage>", which is information of the same kind as `mentions: 0`.
  This is half of idea #11 (find-and-jump); the query half remains, and now reuses this plumbing.
- **2026-07-28 — A synthesized name reports its origin instead of claiming a declaration.** Following
  `__pre__.overSpeed` emitted `declared_at_line: 41` — where `overSpeed` is declared. The number was
  real (a generated variable inherits its base's span) but the **field name asserted a declaration
  that does not exist**, and a reader trusting it would look at line 41 and find a *different*
  variable. Same species as the phantom `request` and `kind: "stage"` for a cleared point; found in
  an `explain` and fixed on both surfaces, because the Context Bar told the identical lie
  ("— declared at line 41") and bar and file must agree *and* both be honest.
  Now: `tracking.generated` carries `{kind: "pre-slot", base: "overSpeed", note: …}`, the line is
  retained under `span_inherited_from_base_at_line`, and the bar reads `— generated: pre(overSpeed)`
  with the explanation on hover. `declared_at_line` / `declared_in_class` are emitted only for names
  that really are declared.
  **Recognition goes through `rumoca_core::pre_slot_base`, never a string match.**
  `crates/rumoca-core/src/ir_primitives/generated_names.rs` is the owning definition of the
  convention and states the contract outright: *"Consumers must never string-match `\"__pre__\"`
  directly — construct slot names with `pre_slot_name` and recover structure with `pre_slot_base` /
  `is_pre_slot`."* Spelling it out here would re-derive a convention that crate owns and would break
  silently the day it changes. Worth noting how this was found: `phase_source` pointed at
  `rumoca-ir-dae`, reading from there led to `generated_names.rs`, and the contract came with it —
  which is exactly what that field was added for.
- **2026-07-28 — Phase breakpoints fire only on the first compile of a model: Rumoca's compile
  cache.** Four rounds of misdiagnosis, worth recording so the fifth does not happen.
  Breakpoints in `rumoca-phase-dae::pre_lowering` verified but never fired, while a breakpoint in
  `rumoca-phase-structural` fired every time — **identical build flags, both path-dep crates, both
  `opt-level = 0`, both `debuginfo = 2`** (measured from `cargo build -v`, not assumed). The cause is
  `CompiledSourceRoot::compile_cache`, an `IndexMap<String, PhaseResult>` keyed by model name:
  `compile_model_strict_reachable_with_recovery` returns the cached result, so the phases do not run
  on any reselect. Structural kept firing only because HRW calls `build_structural_report` *itself*,
  outside the cached call. Confirmed by loading a model the process had never compiled — all three
  breakpoints fired immediately.
  **What made it expensive:** each wrong hypothesis was individually reasonable and each cost a
  relaunch. `opt-level = 1` dropping line tables was real (it is why a breakpoint in the `hrw`
  package will not bind at all — that package still has no override) but was not this. The recorded
  windows-migration finding that CodeLLDB mis-binds breakpoints in path-dep `crates/rumoca-*` was
  reached for as an authority and **is at best incomplete** — a path-dep breakpoint fired fine here.
  It is left in `launch.json` because the symptom it describes (pausing at an unrelated address in
  `epi.rs`) is different from this one, but it should not be treated as settled.
  **Method note, the actual lesson:** the answer came from a three-way experiment — one breakpoint
  in HRW's own package, one in a phase crate known to work, one in the target — run in a single
  specimen load. Serial single-hypothesis fixes cost four rounds; the experiment that distinguished
  them cost one. Prefer the discriminating test over the next plausible fix. It also required
  distrusting a *negative* result: an earlier probe "proved" the function was never called, when in
  fact the probe had failed to compile and its errors had been discarded.
- **2026-07-28 — HRW compiles uncached, and builds itself unoptimized. Debuggability over speed.**
  Doug, deciding both: *"This project is for learning, not for production performance. Debuggability
  is of the highest priority."*
  (1) **`compile_model_strict_reachable_uncached_with_recovery`** at both production call sites.
  Rumoca's compile cache returned an identical `PhaseResult` for any model already compiled in the
  process — so the IR was right but **the phases did not run**, which for an observatory is the
  wrong kind of correct: "watch the compiler work" has to mean the compiler worked. It also made
  phase breakpoints fire exactly once per model per session.
  **Measured before switching** (one session, MSL loaded — the case a reselect hits): MotorWithBrake
  302 ms cached vs 297 ms uncached; BouncingBall 271 vs 261. **Within noise.** The cache skipped DAE
  construction, the part worth watching, while resolve and MSL reachability — which dominate —
  ran either way. The old behaviour paid for a compile and withheld the observable part of it, so
  this costs nothing and there was never a real trade-off. `examples/compile_timing.rs` reproduces
  the table; keep it, so the next person tempted to re-enable the cache has the number.
  (2) **`[profile.dev.package.hrw] opt-level = 0`.** `hrw` was the one package still inheriting
  `[profile.dev] opt-level = 1`, so breakpoints in HRW's own worker, bridge and UI would not bind at
  all — the observatory could debug the compiler but not itself, in a project whose charter calls the
  debugger a first-class learning instrument. This one does have a real cost: HRW's per-frame code is
  now unoptimized, so watch for sluggishness on large trees and revert if it bites.
- **2026-07-28 — Recompiling the same specimen keeps the assembled context.** Doug, the first time
  the phase breakpoints actually fired: *"After I requested 'debug' and you set breakpoints, it seems
  that you also cleared the captured context as my context bar is now empty."* Not the breakpoint
  request — `open()` cleared `pointed_at` and `tracked_identifier` unconditionally, and hitting a
  phase breakpoint *requires* a recompile. **The workflow was self-defeating: assembling context to
  ask for breakpoints, then destroying that context in order to reach them.** Neither half was wrong
  on its own, which is why it survived until the two were used together.
  Clearing is right when the specimen **changes** — a key-path addresses one model's IR and means
  nothing in another's. For a reselect the IR is normally identical, so the point still resolves.
  "Normally" is not "always" (the file may have been edited between loads), so a retained point is
  **validated** against the new IR rather than assumed to survive: `bridge::node_exists` re-walks the
  key-path, and a dangling point is dropped **out loud** via `notice`. Keeping it would leave the bar
  naming a node that no longer exists over an emitted `subtree: null`.
  Three deliberate asymmetries. The **follow is not validated** — it is a name, not an address, and a
  name matching nothing is already reported honestly as `mentions: 0` in every stage; dropping it
  would discard a deliberate choice to answer a question the emitted context answers better. **Stage
  and specimen points cannot dangle**, so they are exempt by construction. And the **jump match list
  is always invalidated**, even on a reselect, because `refresh_jump_matches` caches on
  `(stage, name)` — both unchanged across a recompile while the IR underneath them is not.
  A surviving point or follow triggers re-emission, since the focus file otherwise describes the
  previous compile's IR: same node, different values, and a stale subtree is worse than an absent one.
- **2026-07-28 — cppvsdbg is the preferred debug adapter; all-threads stepping was never
  LLDB-specific.** Doug verified both halves end to end under cppvsdbg: breakpoints in path-dep
  `crates/rumoca-phase-dae` bound and fired, and live-trace stepping of index reduction advanced the
  animation with plain **F10**. He uses cppvsdbg going forward.
  **The correction that matters:** all-threads stepping was treated for weeks as a CodeLLDB
  capability that cppvsdbg lacked — it is the reverse. **LLDB defaults to stepping one thread and
  must be told otherwise** (hence the `ns`/`si`/`so` aliases); the Visual Studio debugger already
  runs all threads on a step. The aliases are a workaround for a default, not access to a feature.
  That misreading is why cppvsdbg sat unused as a "cross-check" while the live-trace requirement was
  believed to rule it out.
  `launch.json` now lists cppvsdbg **first** (VS Code's default pick), `README.md` promotes
  `ms-vscode.cpptools` to required, and `architecture.md` §4 carries the adapter comparison.
  **Left explicitly unsettled:** the recorded claim that CodeLLDB mis-binds breakpoints in path-dep
  crates. It predates the compile-cache discovery, which produces a near-identical symptom and
  accounted for every case investigated that day, and it has not been re-tested. Marked *suspect*
  rather than deleted — it may still be real, and the honest state is "unverified", not "wrong".
  Anyone returning to CodeLLDB should retest it on a model the process has not yet compiled.
- **2026-07-28 — Phase 5 closed; the context-assembly primitives are frozen pending evidence.** Doug,
  after testing the full loop end to end — follow from the specimen source, jump to the node in a
  tree, point at it, `explain`, then breakpoints that fire: *"Until I can demonstrate with practical
  scenarios why we need to change the context assembly primitives, we will keep them as they are."*
  **The frozen set:** one point-at (node / stage / specimen) + one follow (an identifier across all
  stages) + the always-captured background. Two candidates were designed and deliberately **not**
  built: **multiple simultaneous `follow` items** (Doug's own idea, deferred by him pending
  experience) and a **third "compare these two" primitive** (Claude's suggestion, declined because
  comparison already works well as *background* via `cross_stage`, so a manual version would be
  labour rather than capability).
  **The reasoning is about evidence, not conservatism.** This is new interaction design with no prior
  art — the noun is assembled rather than selected, and published eagerly because the verb is typed
  in another process — so intuition about what is missing is untrustworthy. An argument that
  something would be *more expressive* does not count; a scenario Doug actually hit does. Phase 5's
  own history supports the rule: every genuine defect it produced (the phantom `request`, `kind:
  "stage"` for a cleared point, the un-clearable point, the wrong empty-state hint, context destroyed
  by recompile) was found by **using** the thing, not by reasoning about it.
  Enriching what the existing primitives *emit* is explicitly unaffected and still encouraged — a
  different axis, governed by "Claude is the context consumer".
- **2026-07-29 — `Playback<T>` and `Animated`: the animation debt paid, with frame content folded
  in.** The three animation views each declared the *same seven fields* (`frames`, `cursor`,
  `playing`, `interval`, `elapsed`, `live_rx`, `live_done`) and carried five **byte-identical**
  methods over them, plus ~30 lines of identical timing prologue in each `ui()`.
  `ReductionAnimation` was those seven fields and nothing else.
  **A generic struct, not a trait.** A trait would have shared the *behaviour* and left the state
  declared three times, so it could still drift apart. `Playback<T>` shares both; each view now owns
  one and keeps only what is genuinely its own (matrix geometry for matching and Tarjan, nothing at
  all for reduction). `Animated` is the small trait *on top*, for the one thing that cannot be
  generic — what the current frame means. Named `Animated` rather than `AnimationView` so it cannot
  be confused with `bridge::AnimationView`, the emitted shape it feeds; both appear in `app.rs`.
  Rolled up with the two items that were the same refactor from other angles: `animation_controls`
  went from **8 positional parameters to 4** (its four transposable `&mut`s — two adjacent bools —
  became `PlaybackControls`), and `app.rs` lost two further copies of "which animation is on
  screen?" to `on_screen_animation()`.
  **Why now, having been deliberately deferred:** the deferral assumed Phase 7 would rework these
  views first. Idea #40 adds a **fourth** view before Phase 7, so copying the pattern again would
  have left Phase 7 four near-duplicates and made `live_state` identical in four files.
  **Folded in: `Animated::current_frame_context`.** The capture's `view.animation` carried position
  only, so a question asked while paused on frame 12 said *where* Doug was but not *what he was
  looking at* — frames live in memory and appear in no stage IR. Each view already computes a
  human-readable step description in order to draw it; `reduction_anim`'s was split out of
  `render_step` as `step_summary` and is now handed to the capture unchanged, so screen and emitted
  context cannot give different accounts of one frame. This matters for the route Doug named into
  the algorithms: watch, get confused, ask — *before* knowing enough to phrase a question about the
  algorithm itself. The crash log gets it too, since a crash mid-animation is among the harder ones
  to reproduce.
  Measured: `matching_anim` −68 lines, `tarjan_anim` −55, `playback.rs` +324 of which roughly half
  is tests and rationale. **Line count is up; duplication is down from three copies to one** — which
  was the point.
- **2026-07-29 — Idea #40 delivered: `pre()` lowering is replayable, and phases take an observer
  rather than a `LiveTrace`.** The `__pre__.x` slots appear in the Events IR, in Solve lowering's
  parameter vector, and **in no source file** — they are synthesized because a `when` equation needs
  a value to hold when no branch fires and a DAE cannot say "unchanged". Reading the phase's output
  shows the slot already existing; only a replay shows it being made.
  **The observer is a callback, not `LiveTrace`.** This idea was written partly to test whether
  `LiveTrace` generalises beyond the structural phases. The answer turned out to be better than
  yes: **the phases do not need it.** `LiveTrace` lives in `rumoca-phase-structural`, and a
  dependency from DAE construction onto structural analysis would run backwards through the
  pipeline. `rumoca-phase-dae` takes `Option<&dyn Fn(&PreLoweringFrame)>`; HRW owns the `LiveTrace`
  and hands over a closure that pushes into it. The phase crate never learns `LiveTrace` exists.
  That is the more upstreamable shape and the existing three could migrate to it (logged).
  **Tracing had to wrap DAE construction, not the pass.** `pre()` lowering runs *inside*
  `to_dae_with_options`, so by the time HRW holds a DAE the slots exist and the `pre()` calls are
  gone — replaying the pass on that DAE traces nothing. Hence `to_dae_with_options_traced`, and
  hence the worker carrying `cr.flat`: the flat model is the last artifact from before the pass ran.
  **`Playback<T>` generalised.** `PreLoweringAnimation` declares no cursor, no timing, no channel,
  and compiled first try — the payoff from sequencing the animation debt ahead of this work rather
  than after it, which was the whole argument for reordering.
  **Hosted on the Events stage** as a sub-tab rather than a new `StageKind`: the slots exist because
  of `when` equations and Events is the stage that shows them, and a new stage would have to be
  wired into every per-stage system to say something that belongs beside what is already there.
- **2026-07-29 — Live-debug variants compare by derived equality, not a hand-written pair list.**
  Doug: *"The Play button works, but when I click the Debug button nothing happens."* Not the new
  view's plumbing — `live_debug_poll` and `is_arming` both decided "is the pending session this
  view's?" by enumerating matching pairs *by hand*:
  `(Matching, Matching) | (Tarjan, Tarjan) | (Reduction, Reduction)`. Adding a fourth variant
  compiled cleanly and **silently never matched**, so the click armed nothing, showed no "Arming…"
  badge, and produced no error. `matches!` over tuple patterns has no exhaustiveness check to fail.
  Fixed by deriving `PartialEq` and comparing with `==`, which **cannot go stale**. That removes the
  class rather than the instance: the next view added will work without touching the arming code.
  A test iterates `PendingLiveDebug::ALL` (test-only, since nothing in the app enumerates variants —
  which is precisely why the omission was silent) and asserts each variant is recognised while
  arming *and* is not mistaken for another view's session.
  Worth noting as the second silent-omission bug of this shape: the same day, `on_screen_animation`
  replaced three copies of "which animation is showing?" for the same reason. Both were the
  `hrw-stage-diff-highlight-extend` rule — every new view must be wired into *all* per-stage systems
  — failing in the direction where the compiler cannot help.
- **2026-07-29 — All four traced phases take an observer callback; `LiveTrace` is one implementation
  of it, not the interface.** Completes the migration idea #40 proposed. `matching`, `tarjan`,
  index reduction and `pre()` lowering now take `Option<rumoca_core::FrameObserver<'_, F>>` — a
  plain `&dyn Fn(&F)` defined once in `rumoca-core`.
  **The change was forced, not chosen.** Instrumenting `rumoca-phase-dae` the old way would have
  meant taking `Option<&LiveTrace<F>>`, with `LiveTrace` living in `rumoca-phase-structural` — a
  dependency from DAE construction onto structural analysis, pointing the wrong way down the
  pipeline. A callback needs no dependency at all, and lets a consumer buffer, stream, count or
  debugger-step frames without any phase being changed to allow it. It is also the more upstreamable
  contract: CogniPilot can implement whatever observer they like rather than adopting HRW's.
  Two incidental wins. The observer takes `&F` rather than `F`, so **an untraced run allocates
  nothing and a watching one clones only if it decides to keep the frame** — the old code cloned
  every frame on the live path. And the crate tests now demonstrate the intended usage instead of
  exercising a type only HRW uses.
  `LiveTrace` stays in `rumoca-phase-structural`, re-documented as *one implementation* of an
  observer. HRW wires it up as `Some(&|f| lt.push(f.clone()))` at three call sites. Moving it out is
  a separate change with its own risks: it carries `live_trace_breakpoint`, the debugger anchor, and
  both the `opt-level = 0` override and `bridge::find_live_trace_line` are keyed to that file.
- **2026-07-29 — A running live session shows no frame total.** Doug, stepping all four animations:
  *"Instead of showing 1/11, 2/11 … 11/11 the frame report shows 1/1, 2/2 … 11/11."* Correct, and the
  reason is sharper than an off-by-one. In a live session frames arrive **one at a time** from the
  algorithm thread and the cursor follows the newest, so the count so far always equals `cursor + 1`.
  The denominator was not wrong arithmetic — it was a **claim about the total**, and `3/3` says *you
  are at the end* on every single frame, when the algorithm may have fifty steps left.
  The total is genuinely unknown until the session finishes, so the honest rendering is to omit it:
  `Frame 3 · live`. Once `Finished` the frames are an ordinary recorded trace, the total is real, and
  `n/total` returns. Recorded playback was never affected.
  **Not caused by the `Playback<T>` refactor** — the old `start_live` also began with an empty frame
  vector, so this had been true since live tracing was built. It surfaced now because Doug stepped
  all four animations in one sitting after the observer migration, which is the kind of scrutiny a
  single view never got. Same species as the phantom `request` and `kind: "stage"`: **a field that
  prints the one number available rather than the one that is true.**
- **2026-07-29 — Text summaries beside the visual animations too.** Doug, after using the reduction
  and `pre()` replays: *"Originally, I had incorrectly assumed that the only animations which would
  be helpful would be the ones which included visualizations of mathematical objects such as
  matrices. But, your text-only animations… are tremendously helpful. If nothing else, the text only
  playbacks provide useful summaries of what I will find if I decide to step through the algorithm
  code."*
  So matching and Tarjan gained a **running-state panel** under their step line, in the shape the two
  text-only views already had: a one-line statement of what the algorithm is *for*, then where it
  has got to. Matching shows `Matched 3 of 8 — still unmatched: …`; Tarjan shows blocks closed,
  stack depth, and the largest block so far. All counts come from the frame's own snapshot, so they
  track the algorithm rather than reporting the final answer early.
  Two design notes. **Naming what is *not* yet matched is the useful half** — a system ending with an
  unmatched equation is structurally singular, which is exactly what the Index Reduction stage
  exists to fix, so the unmatched list is the bridge between the two stages. And **stack depth is
  the number worth watching in Tarjan**: a component closes only when the stack unwinds to a node
  whose lowlink never fell below its own index, so a deep stack means "still inside something that
  might be one big block".
  The data was already being emitted — `matched_so_far`, `sccs_found_so_far` and `stack_depth` went
  into `current_frame_context` for the capture on 2026-07-29 and were never rendered. The capture
  knew more than the screen did.

- **2026-07-29 — Three more animations: tearing, alias elimination, IC planning.** Doug: *"Implement
  animations for tearing, alias elimination, initial condition planning and connection expansion"*
  (connection expansion follows separately — it needs `rumoca-phase-flatten` instrumented). Three
  design calls fell out of building them.

  **(1) Not every phase hides a search, and the views say which they are.** Tearing is a genuine
  greedy algorithm: it counts appearances, picks a winner, and the cascade of causal assignments
  that follows is the payoff. Alias elimination and IC planning are not — the elimination pass walks
  a list and substitutes, and the IC plan is already computed by the time HRW holds the report.
  So the tearing view is a **replay** with a Debug button and a live trace; the other two are
  **reveals** of recorded lists, with no Debug button, and each module's doc comment says so
  explicitly. Giving all three the same chrome would have been easier and would have taught
  something false.

  **(2) The tearing view rebuilds from the DAE rather than reading the report.** Every other
  animated view is built from the stage JSON. Tearing cannot be: it works in each coupled block's
  own `0..n` index space, and `StructuralReport` has already translated back to names. So
  `walk_blocks` redoes incidence → matching → BLT → `block_local_incidence` and re-runs the
  algorithm with an observer. Recorded and live playback call the *same* walk, so they cannot
  diverge. This is also why the Rumoca side needed `pub mod blt` and `pub fn block_local_incidence`
  (commit `e3124880`) — the *result* was already public; the way back into the algorithm's index
  space was not.

  **(3) The Structural and Index Reduction tabs tear different DAEs.** They describe different
  systems, and a high-index model's raw DAE has no full matching — hence no blocks, hence nothing
  to tear. `App::tearing_dae` re-runs the reduction funnel for the Index Reduction tab rather than
  caching a second DAE the two tabs would have to keep in sync; it is pure and cheap.

  Also added `src/test_support.rs` (`dae_for(model)`), because a view that *reconstructs* compiler
  state has a failure mode hand-built frames cannot catch — the reconstruction can be wired to the
  wrong index space and every unit test still passes. One end-to-end test per such view closes that
  gap; it returns `Option` so a missing specimen skips rather than fails.

- **2026-07-29 — Connection expansion animated; recorded only, deliberately.** The fourth of Doug's
  four requested animations, and the first to need a *new crate* instrumented
  (`rumoca-phase-flatten`, commit `edaa2bb8`). The thing worth seeing is MLS §9.2's asymmetry — a
  potential set of *n* connected variables becomes *n − 1* equality equations, a flow set of the
  same *n* becomes exactly one sum-to-zero equation (Kirchhoff) — plus the fact that connection sets
  are transitive, since they are built by union-find. Neither survives into the flat model.

  **No Debug button, and the reason is plumbing not principle.** The phase *is* instrumented for a
  live trace, unlike alias elimination and IC planning where there is genuinely nothing to trace.
  But re-running flatten needs the resolved `ClassTree` — which contains the whole MSL — and the
  instance overlay, on the UI thread. The right fix is a **worker-side live-debug path**: spawn the
  traced re-run where the tree already lives and stream frames back over the existing channel. That
  would also let the three views that currently clone a DAE into the app stop doing so. Logged in
  `docs/ideas.md` #9 rather than half-built here. The distinction matters for honesty: this view's
  missing Debug button means "not yet", the other two mean "never".

  **The re-run must match the real flatten.** `worker::record_connection_frames` re-runs instantiate
  + typecheck + flatten with an observer, and its `FlattenOptions` must equal `rumoca_compile`'s own
  `flatten_options_for_tree()` — `strict_connection_validation: true` above all, since that is what
  makes an incompatible-connector model fail rather than expand. Recorded frames from different
  options would describe a flatten that never happened. The end-to-end worker test exists for
  exactly this: a unit test on the animation type would show zero frames and call it a pass.

## Arcs 4-7 — closure record (moved out of `CLAUDE.md` 2026-08-01)

Pass one built the whole pipeline observatory under a self-imposed **public-API-only**
constraint, which is **now lifted** — HRW lives in the Rumoca workspace and may reach internal
phase state. Pass one remains the **baseline to surpass, not discard**: its stage views,
specimens, notebook and tests are the reference pass two enriches. This record moved here
because a closed arc is a decision, not current work, and it was crowding the file that gets
read at the start of every session.

- **Arc 4 closed** — index reduction on `Drivetrain`. The nonlinear four-bar + planar library
  (`lib/PlanarMechanics.mo`) parked/deferred; see `docs/ideas.md` #5.
- **Arc 5 closed** — initialization observable: `RcCircuit` IC plan + relaxation; `CapacitorLoop`
  structural and `OverInitRc` init-determinacy blow-ups.
- **Arc 6 closed (2026-07-20)** — compile-level hybrid structure is observable. The Events tab
  shows `BouncingBall`'s condition (`h <= 0`) plus discrete reinit; smooth models show
  "no events".
- **Arc 7 closed (2026-07-21) — the simulation core** (charter §4.2.7), the biggest inflection
  (static IR → live execution). **Solve lowering** (phase 8 — DAE → `SolveModel`, via
  `rumoca-phase-solve::lower_dae_to_solve_model`) as a stage tab, and **Simulation** (phase 9 —
  a worker-thread runner calling `rumoca-sim::simulate_solve_model`; Auto solver = BDF-via-diffsol
  for stiff, RK45 otherwise, plotted in an `egui_plot` pane). The UI never blocks and never
  shells out to the CLI. Ran start-simple: `SingleInertia` → `BouncingBall` → the stiff
  `BenchActuator`. **Step-mode plotting** landed (`worker::discontinuity_segments` breaks the
  line at reinit jumps, gated on `SimData.has_discontinuities`; `series_color` pins per-variable
  colour), closing the Arc-6-deferred "discontinuities render as discontinuities" and
  `docs/ideas.md` #8. Closed the "solve lowering not instrumented" gap (Doug, 2026-07-20).

**The log view** — delivered — a pane streaming compilation and simulation log messages with
timestamps and far more phase/solver detail than the public API could give. Per-phase timing was
impossible when phases 5-9 arrived from one opaque
`compile_model_strict_reachable_with_recovery` call. **Doug's proof the in-workspace migration
was worthwhile.**

**Delivered 2026-07-29 — four more phase animations** (tearing, alias elimination,
initial-condition planning, connection expansion), bringing the total to eight. Building them
established a distinction to preserve: **not every phase hides a search.** Tearing and connection
expansion are real processes with reasons that exist only mid-run, so they are *replays*; alias
elimination and IC planning are lists computed before HRW sees them, so they are *reveals* with
no Debug button, and their module docs say why. Connection expansion is instrumented for a live
trace but has no Debug button *yet* — re-running flatten needs the whole MSL on the UI thread;
the fix is a worker-side live-debug path (`docs/ideas.md` #9). New Rumoca instrumentation:
`rumoca-phase-structural` (`pub mod blt`, `block_local_incidence`) and `rumoca-phase-flatten`
(`connections::trace`, `flatten_ref_with_options_traced`) — the first non-structural, non-DAE
crate instrumented.

**Close-out gates under review.** Doug is separately weighing whether the differential test
(System Modeler round-trip) and the debugger single-step should remain arc close-out gates at
all — Arcs 3 and 4 closed with both accepted (deferred / unconfirmed). Until he decides, treat
them as **satisfiable-by-acceptance, not hard blockers** (`docs/ideas.md` #4).

**Superseded work order (Doug, 2026-07-28)** — its items 3-5 (attempt the tour, refactor
`bridge.rs`, source-tooling Phases 6-7) were written before the tour was attempted and found
wanting; the answer-platform plan superseded them, and it is now itself retired to
`docs/history/`. Items 1-2 (animation debt, idea #40) were delivered. The reasoning is preserved
in `docs/history/answer-platform-plan.md`.

## Documentation audience — two readerships, one boundary (2026-08-01)

Doug, setting the convention:

> README.md files and the files to which they link should be written with the assumption
> that those files are for me to read and might also be read by rumoca maintainers. There
> should be README.md and linked files wherever it is necessary for me and rumoca
> maintainers to learn without having to ask you for help.

This supersedes the blanket form recorded earlier the same day (*"the documents are
primarily for Claude's consumption"*), which was true of most documents and wrong about
READMEs — and being wrong about the boundary meant every README got written in a register
aimed at the wrong reader.

### The two readerships, and what each fails at

| | Audience | The failure |
|---|---|---|
| **README.md and its further reading** | Doug, and Rumoca maintainers | **the reader has to ask Claude** |
| everything else | Claude | Claude acts on something stale |

**The human criterion is testable, and that is the point.** "Readable" is a judgement;
*"a maintainer got from a fresh clone to a running HRW without asking"* is a fact, in the
same way a fixture tour's expectation is violable or it is not. It also has a cost behind
it: `docs/upstream-strategy.md` orders deliverables by **their** cost to accept, and HRW is
already the item asking for the most. Needing Claude in the loop to evaluate it is more of
exactly that cost.

### The boundary: an index link is not an endorsement

"Files to which they link" is transitive, and `docs/README.md` is an **index** that
deliberately links to nearly everything. Read literally, the convention would make
`ideas.md`, `tech-debt.md` and the compiler-phases database human-facing, and the
Claude-facing category would vanish.

So the rule is about what a README **promises**, not what it mentions:

> **A README must let its reader finish the job without following any link into a
> Claude-facing document.**

Links are therefore of two kinds, and an index says which:

- **Further reading** — held to the human standard. `setup-windows.md`, `CHARTER.md`,
  `reports.md`, `compiler-phases/the-chain-of-problems.md`.
- **Working notes, listed so Doug can audit what exists** — not held to it. `ideas.md`,
  `tech-debt.md`, `question-ledger.md`, `DECISIONS.md`, the per-phase drill-downs.

### Facts by reference, never by transcription

**Human-facing prose is precisely what this project has watched rot.**
`end_to_end_tour.md` was human-facing prose, and it died asserting a 7x7 incidence matrix
on a tab that shows 48 equations — uncaught for weeks because nothing checks prose. A
convention that produces more of it, unprotected, re-creates the failure we spent
2026-08-01 removing.

So:

> **A README states facts it does not own by REFERENCE, not by transcription.** Counts,
> outcomes and measurements point at the generated artifact — a report CSV and its
> provenance sidecar, a specimen `trace/`, a fixture tour — rather than repeating numbers
> in prose.

`docs/reports/README.md` is the pattern: "2,614 of 2,626" sits beside a link to the CSV and
its `meta.json`. Written as prose alone it would be wrong by the next sweep.

### Where a README is required

Wherever a human would otherwise have to ask. Written 2026-08-01 to close the gaps this
convention exposed: `specimens/`, `docs/fixture-tours/`, `vscode-extension/`,
`docs/compiler-phases/`. Already present: `hrw/README.md`, `docs/README.md`,
`docs/reports/README.md`, `scripts/README.md`, `docs/specimen-notebook/README.md`.

**The repo root `README.md` is upstream Rumoca's and stays untouched** — editing it would
conflict on every rebase, and keeping the fork cleanly cherry-pickable outranks a pointer.

### The maintenance division — who owns what, and which signal only Doug can give

Amendment, 2026-08-01, same session:

> Most of the documents exist for your consumption. So, you should maintain the content of
> those documents so as best to enable you to help me learn. But we must ensure that the
> content of the README.md files is appropriate for human readers such as me and rumoca
> maintainers. [...] I will provide feedback as a learner for that file.

| Scope | Written and maintained by | Judged by |
|---|---|---|
| Everything except READMEs | Claude, for **Claude's** effectiveness at helping Doug learn | Claude — fewer wrong answers, less re-derivation |
| README.md files | Claude | **Doug** — did he act on it without asking? |
| `hrw/README.md`'s value case | **Both, deliberately together** | a maintainer, and a learner |

**Claude maintains its own documents without asking.** Reorganising, condensing, deleting a
rotted file, correcting a stale claim — these need no approval, because the standard is
whether they make Claude a better teacher and Claude can evaluate that directly.

#### "For now I am the only learner" is a constraint, not a placeholder

It rules out writing the learner half for a generic Modelica-curious developer, which is the
default and is vague enough to reach nobody. The concrete reader is **someone learning the
mathematics of robotics through a compiler that implements it** — decades of C/C++/Java, new
to Rust, wanting Pantelides to stop being a word. Specific enough to write against, and
specific *because* it is Doug.

#### The signal Claude cannot generate

**Whether a README lands is not self-checkable.** Claude can verify a README's *facts* — the
link checker, the control-character test, the by-reference rule all do that mechanically — but
not whether it teaches or persuades. The precedent is not hypothetical: `end_to_end_tour.md`
was Claude's solo attempt at explanation and it was worthless, while the animations nobody
asked for were the best thing the project produced.

So the standing shape is: **speculative features, freely. Speculative persuasion, not at
all.** Until the joint rewrite, Claude's job on `hrw/README.md` is to keep it accurate and
current and to **stop short of investing in the value case**, because that is precisely the
artifact Claude gets wrong alone.

#### Open, and not to be settled unilaterally

**Which audience the page opens with.** `upstream-strategy.md` argues the maintainer framing
should lead, since HRW is the deliverable asking for the most maintenance burden — and the
capture plan in `hrw/README.md` currently records that argument. **It is recorded there as an
argument, not adopted as a decision.** The two audiences want different first images, and that
choice belongs to the joint rewrite.

## The repository is the system of record; memory is a cache (2026-08-01)

Doug, describing what he lost migrating this project from Linux to Windows:

> I learned during that migration that much of what you understood about the project would be
> lost. In other words, your understanding was apparently captured in files that are not part
> of the project. In the near future, I'm going to attempt to clone this project on a
> different windows machine and hope that when I do, you'll immediately understand my top
> priority and other such stuff without me having to re-explain everything.

**Measured the same day: 39 memory files, 1,600 lines, none of them in the repository.** They
live at `~/.claude/projects/<key>/memory`, and **the key is derived from the project's
filesystem path** — so this is not only a machine boundary. **Clone to a different path on the
same machine and the memory is equally gone.**

### The rule

> **The repository is the system of record. Memory is an accelerator, never the sole carrier.
> Memory may duplicate the repo freely; where a memory holds something the repo does not,
> that is a portability bug rather than good hygiene.**

**This deliberately overrides the default memory discipline**, which says *don't save what the
repo already records* — a rule optimising for non-duplication. Followed literally it
**guarantees** the migration failure, because every memory is then by construction something
the repo does not say.

### What the reconciliation found

Of 38 memories, **9 carried claims the repository did not**, and they were the ones a fresh
session most needs and least infers: who Doug is, how he learns, what Claude is expected to do
unprompted, and the standing quality bar. Ported 2026-08-01 into
[`docs/working-with-doug.md`](docs/working-with-doug.md) (new),
[`docs/context-assembly.md`](docs/context-assembly.md), [`docs/vision.md`](docs/vision.md) and
this file.

**A keyword sweep is not sufficient to find them.** Two of the nine passed a naive grep and
were false negatives: "the cost of being wrong" matched a *memory-safety* warning in the
fidelity runner, and "Claude is the consumer" matched a comment about *crash files*. Same
words, unrelated claims. **The check has to be "is this claim carried", not "does this phrase
appear."**

### The standing check

**When writing a memory, name where it belongs in the repo** — the same way a bug now arrives
with a test. If the answer is "nowhere", that is the finding.

## The cost of being wrong collapses — but only where it is detected (2026-07-29)

Recorded here 2026-08-01; it had lived only in memory, and it is the insight several other
decisions rest on. Claude's formulation, which Doug answered with "EXACTLY":

> Not that the reasoner is smart, but that **the cost of being wrong collapses**, so you can
> afford to think out loud and reverse yourself the same afternoon.

It explains why speculative **features** are permitted, why the tech-debt trigger could move
from calendar to phase boundary, and why this project reversed its own philosophy twice in one
day at no cost.

**But the collapse is maintained, not free.** What kept the cost low on 2026-07-29 was
concrete: a test suite that caught four mistakes; **probing instead of reasoning** (the
contamination bug hid behind a fix placed in the wrong function and was found only by printing
guard state); the System Modeler oracle; verify-before-asserting; and Doug correcting Claude
directly. **Those are the mechanism, not fastidiousness.**

**Doug's sharpening, without which this inverts into permission to be casual:**

> The most effective way for us to continue moving fast in this project is to slow down and
> take the time necessary to detect bugs so that the costs of those bugs can be limited to
> reverts and not investigations.

**Wrong-and-caught-now is cheap; wrong-and-caught-later is expensive; the two are
indistinguishable at the moment of being wrong.** So detection is not a tax on speed, it *is*
the speed. And the curve is not linear — a revert is minutes and loses one change, while an
investigation is hours and **may not converge**.

**The one place wrong stays expensive: stored claims.** A mistake in conversation costs one
exchange. A mistake written into the repo as fact costs months and is indistinguishable from
knowledge later — the `rank_deficiency: 7` bug, the stale handoff, the tour's 7×7 matrix.
**Speed on actions, care on records.**

**Operationally:** the full suite before every commit, not a filtered subset; stash-check to
decide pre-existing versus introduced *before* diagnosing; track the clippy count across a
change. And **Claude cannot run the GUI** — for UI work it must say which parts are
test-verified and which are only reasoned, rather than letting a report imply more than
happened.

## No Test mode — one corpus list with a filter instead (2026-08-01)

Doug, questioning his own earlier decision:

> More than once, we have both likened test mode to specimen mode. The more that I have
> thought about that, the more that I have questioned whether it makes sense to have a new
> test mode. After all we could simply add the MSL examples to the specimen list.

**Dropped before it was built.** `docs/ideas.md` #52 is now the merged design; #53's deferred
"should the two modes merge?" is answered.

### Why "mode" was the wrong unit

Every existing mode changes the left panel **and the interaction loop** — Tour reads stops and
clicks links, Specimen picks from a list and inspects, Debug arms and steps. **Test mode's loop
is *pick from a list, inspect the pipeline*, which is Specimen mode's loop exactly.** What
differed was the list's source and its columns: a data question wearing a layout question's
clothes. Two modes with the same layout, the same right-hand side and the same gestures read as
important in a plan and as noise in the app.

### The report composition argues FOR merging, which is what settled it

`docs/reports.md` calls the three reports load-bearing — survey → *eligible*, fidelity →
*trustworthy*, oracle → *findings* — and that looked like justification for a dedicated
surface. It is the opposite.

**The interesting question is never "show me the fidelity report."** It is the **join**:
*fidelity-green **and** oracle-mismatched*, because `reports.md` makes fidelity-green a
precondition of a mismatch being admissible. **Three views make that join something you do in
your head; one list with filter predicates makes it a query** — which is exactly the "query
axes over the corpus" #53 calls the enabler for everything else.

So the reports do not want three views. **They want to be columns and filters over one row
set.**

### What replaces it

One list widget over **three visible sources** — curated `specimens/`, scratch
`.hrw-bridge/specimens/`, and the 2,626 MSL rows — with **the filter as a prerequisite, not an
enhancement**: 18 files need none, 2,644 do. That is probably the real reason this felt like it
needed its own mode.

**Merge the widget, not the corpora.** Curated specimens have properties MSL rows do not — a
`purpose.md`, a generated trace, System Modeler round-trip intent — and the sources stay
visibly distinct, the same care #53 already required of scratch specimens.

**Filing state does not go in the list.** Unfiled / filed / fixed-upstream is the status of a
finding you intend to send upstream, not a property of a model, and it has a home in
`docs/upstream-issues.md` under the standing rule that Claude never files. The list may *show*
it as a column; the list is not where it is *managed*. **A corpus browser that grows a workflow
becomes a bug tracker.**

### What would prove this wrong

**A question that genuinely cannot be expressed as a filter over the joined rows.** That would
mean something Test-mode-shaped was right after all, and it should reopen #52 rather than be
worked around. Recorded so the signal is recognisable rather than rationalised away.

### The sequencing survives unchanged

#53 said to build the filter *before* deciding whether the modes merge. That still holds — not
because the answer is unclear now, but because **the filter is required either way.** It is the
one piece no decision changes, which makes it the safe thing to build while the rest is cheap
to reverse.

## Parse means "what parsing this file produced" — and will not be scoped

*2026-08-01, Doug.* When the Parse stage was fixed to parse a library model's
declaring file, the payload for a multi-class file turned out to be large:
`Blocks/Continuous.mo` serialises to **~4-5 MB**, most of it classes the reader
did not ask for. Claude raised scoping the tab to the requested class as an
option, in the UI and to cut the fidelity sweep's cost.

**Rejected, and the rule is general.** Doug: *"If 'parse' meant 'what parsing
this file produced' before our code changes, it should continue to mean the exact
same thing after our code changes. So, no scoping to reduce parse payloads or
anything like. I will pay the costs in the UI and during the run of the fidelity
test. As always, we are striving for correctness and accuracy."*

**A stage's meaning is not a performance knob.** Scoping would have made the tab
cheaper by making it answer a *different question* than its name promises, and a
reader comparing Parse to Resolve would have been comparing two things neither of
which was what they thought. That the cheaper version would still have looked
plausible is exactly what makes it the wrong trade.

**Do not re-propose scoping, truncating, or lazily materialising the Parse stage
as a cost measure.** If cost becomes a genuine blocker, the honest moves are to
make the *tree viewer* handle large values better, or to bound the *sweep*, not to
change what the stage contains. Related: `docs/ideas.md` on tree rendering.

## Claude is the primary consumer of HRW's code, and decides its tests and shape

*2026-08-02, Doug.* Setting the purpose of the UI pause, and of testing generally:

> *"My primary testing goal for this project is to provide the verification which you
> declared earlier that you need to be effective. If you are effective, then we maintain
> feature development velocity. If you are not effective, then this project will grind to
> a halt. So, you will decide what tests you need and when to implement those tests as you
> refactor. … You are the primary consumer of this code. You must do what you need to
> this code to make it work for you."*

**This is a delegation of judgement, not of purpose.** It extends
`feedback-claude-is-the-context-consumer` from `focus.json` to the source itself: Doug reads
HRW's code rarely and by audit, so code shaped for a human reader optimises for a reader who
is not there. Claude picks the tests, the seams and the timing, unasked.

### The limit, stated so it is not quietly overrun

**Claude's effectiveness is instrumental. Doug's understanding is the goal.** Where the two
diverge, understanding wins — the standing rule *"DO NOT optimise HRW to widen test
scope"* is exactly that case already decided, and this mandate does not touch it. A change
that makes the code easier for Claude while making HRW a worse instrument is a bad change,
however much velocity it buys.

### The standard Claude is held to

**Refactor where there is evidence of friction, never where code merely looks large.** The
same rule as *"start from the tour holes"*: items that arrive with evidence are the only
ones whose priority is not a judgement call. Evidence means a defect the structure caused, a
change that could not be made confidently, or a question that could not be answered without
running something.

**Every refactoring commit names the friction it removes.** That is what keeps the delegation
auditable: Doug cannot review 9,562 lines, but he can read a claim and ask whether it is true.

### The evidence on the table as of 2026-08-01

- **The file's size caused defects directly.** `app.rs` at 9,562 lines pushed Claude to edit
  by generated scripts with string anchors instead of targeted edits. That mechanism caused
  attribute theft twice in one day (silently disabling a regression guard), leaked Rust
  escapes into comment text, and produced one blind global replace Doug was right to reject.
  **This is not an aesthetic argument.** It is the editing tax made visible as bugs.
- **105 fields on `App` make blast radius unpredictable.** Threading one new field through
  `FromWorker::Compiled` meant hunting every construction site; changing `source` meant
  tracing by hand whether it fed the compile. That tracing is the tax that slows everything.
- **Reasoning was wrong repeatedly where measurement was right.** *"Fidelity is unaffected"*,
  *"a library model has no file to show"*, *"layout cannot be tested"* — three confident
  claims, all false, all caught by running something. **So the highest-value investment is
  making things cheap to check, not making them pleasant to read.**

---

## 2026-08-03 — The DAE gets a tab, and the node link loses its mandatory sub-view

**The pipeline built a DAE and never showed it.** `StageKind` ran
`Flatten → Structural`, so the artifact the whole chain is organised around — the MLS
Appendix B partition into `x`/`y`/`u`/`p`/`z`/`m` and `f_x`/`f_z`/`f_m`/`f_c` — was the one
thing a tour about DAE construction could not point at. `docs/fixture-tours/dae-construction.md`
had to teach it from its *neighbours*, inferring the count from Flatten's balance check on one
side and Structural's `n_unknowns` on the other.

**This was not the "phase internals" change it looked like.** Doug flagged it as a departure
from depending only on phase-boundary IR, and it is worth recording why it is not:
`rumoca-ir-dae` is a **boundary IR**, exactly like `rumoca-ir-flat`; `Dae` already implements
`Serialize`; and HRW already **held** the value in `cached_dae`. No `crates/rumoca-*` file was
touched. **The whole change was a missing tab** — which is its own small lesson about how a
gap can look structural when it is clerical.

The stage note reports what the tour needs without opening anything:
`2 state(s), 0 algebraic(s), 2 continuous equation(s)`.

**Three guard tests caught the wiring the checklist names**, rather than the wiring being
remembered: `stage_file_names_covers_all_pipeline_stages`, `stage_kind_all_is_exhaustive`
(11 → 12) and `stage_pairs_names_match_stage_file_names` each failed until their system knew
about `Dae`. That is the CLAUDE.md rule *"new pipeline stages must be wired into ALL per-stage
systems"* being enforced instead of merely written down.

### The hole the new tab exposed

`HrwLink::PointAtNode` carried a **mandatory** `SubView`, while `SwitchStage` had carried an
`Option<SubView>` since it was written. The consequence went unnoticed for as long as it did
because nothing had wanted it: **five stages — Parse, Resolve, Instantiate, Typecheck and now
DAE — render one generic tree and have no `SubView` variants at all**, so no
`stage/<Stage>/<SubView>/node/<path>` could name a node in any of them. **The richest noun in
the link vocabulary was unavailable on precisely the stages with the least else to point at.**

`stage/<Stage>/node/<path>` now parses, and `describe()` renders it back. **The four-segment
form is still refused** for a stage with no such sub-view: a link naming a view that does not
exist is malformed, not silently downgraded to "somewhere in the stage" — the quiet-wrong-place
failure a link checker cannot see.

**The checker found this, before the tour was walked.** Every `hrw://stage/Dae/Tree/node/…`
in the rewritten tour failed to parse, and `fixture_tour_links_all_resolve` said so on the
next test run. That is the case `docs/fixture-tours/` exists to buy, and it paid here for the
first time on a link form rather than a typo.

`a_node_link_reaches_every_stage_including_the_tree_only_ones` checks the **property over
`StageKind::ALL`**, not the five known names, so a tree-only stage added later fails the test
rather than quietly inheriting the hole.

---

## 2026-08-03 — Tour prose becomes load-bearing, and curriculum tours split from capability tours

**Doug's instruction:** assume thorough explanations beat terse ones until he says otherwise;
assume **the prose of a tour is load-bearing**, not captioning; and link out to System Modeler
or Wolfram Desktop where those answer better than prose.

**The apparent conflict with the fixture-tour rules is not real, and getting that right is the
whole decision.** Those rules cap **claims**, not **words**. *"Every `**Expected:**` line must
be violable"* exists because a hedged expectation teaches Doug to read expectations loosely —
it says nothing about how much explanation surrounds one. The first `dae-construction.md` was
terse in its *prose* because its *claims* had to be tight; that was a conflation, not a
constraint the rules imposed.

**The rule that did collide is *"one capability per tour, and keep it narrow"*,** whose stated
reason is that the scarce resource is *attention per expectation*. That reason is about
**capability** tours, where Doug's surplus attention produces off-stop findings about HRW. A
curriculum tour spends attention on the **concept**. So `docs/fixture-tours/README.md` now
separates the two kinds, and states what survives in both: **claims stay austere and
trace-sourced however long the prose gets; length is bought with explanation, never with
hedging.**

### The echo-chamber risk, and why it did not block this

CLAUDE.md records that the project's clearest failure was `end_to_end_tour.md`, a solo attempt
at explanation, and that **stored regenerable prose builds an echo chamber a later session
mistakes for fact**. That rule was waiting on evidence of real use, and Doug supplying the
instruction *is* that evidence — the precondition is met, not overridden.

The residual risk is narrow and real: long explanations can launder a plausible guess into the
repository. The existing mitigation fits without inventing anything — **load-bearing claims
carry provenance; untagged prose is a lead, not a fact**. A paragraph that cannot be tagged is
a signal that the explanation has run past what was verified.

### The division of labour between the four instruments

Settled here rather than re-derived per tour:

| | Answers | Uniquely because |
|---|---|---|
| **HRW** | what *Rumoca* did | the only view of this compiler's internals |
| **System Modeler** | what a mature independent implementation does | independence (the oracle), plus actual trajectories |
| **Wolfram Desktop** | what the *mathematics* says | symbolic; the algorithm by hand; the general case |
| **Tour prose** | **why the step exists at all** | purpose is not visible in any of the three |

**The last row is the one not to outsource.** No tool can show why a phase is in the pipeline,
because that is about the problem being solved rather than the artifact produced.

Two stops in `dae-construction.md` were rewritten as excursions on this basis. *"`phi` is a
state"* could only be restated in prose; System Modeler plotting `phi(t)` against a flat `J`
makes it **ostensive**. And *"2 equations, 3 unknowns is not simulable"* invites the reading
*"no solution"*, which is **backwards** — `Solve` returns a one-parameter family, and Wolfram's
own `Solve::svars` warning is an independent implementation reaching Rumoca's `ED001` in
different vocabulary. `docs/fixture-tours/notebooks/dae-balance.nb` carries it, including the
identity **balance = −(nullity)**, which connects the compiler's integer to rank-nullity.

### The defect the rewrite found

**The DAE stage rendered a blank tab for its own failure.** On `UnbalancedShaft`, every stage
*downstream* said "not reached (ToDae failed earlier)" while the phase that actually refused
said nothing — because `flatten_stage` had adopted the `FailedPhase::ToDae` error in 2026-07-29,
correctly at the time, since Flatten was then the last tab before Structural. **The succeeding
stage reported the failure and the failing stage reported nothing.**

`dae_absent_stage` fixes it **additively** — Flatten keeps its copy. The DAE tab now carries
`rumoca::todae::ED001`, the counts, the balance, the `reading` line, and the MLS §4.9 guidance.
`the_dae_stage_explains_its_own_absence` checks the **property**: every stage with no IR must
say something, and the one that failed is not the silent member of that set.

This is the pane-is-a-reporter rule reaching a pane that had already shipped, and it is the
first time a *curriculum* tour found a defect — the mechanism `project-tours-multiply-testing`
predicted, arriving from the direction nobody had aimed it.
