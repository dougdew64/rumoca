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
  where Claude armed" config. User flow: right-click → 🐞 → "arm it" → launch that config → select the
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
