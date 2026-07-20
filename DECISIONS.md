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
  `docs/notebook/<Specimen>.md` (template + README committed) records specimen↔Rumoca-feature findings
  — HRW's own record, kept distinct from the rumoca clone's `docs/understanding` (Doug's canonical
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

- **2026-07-19 — `docs/understanding` moved into HRW; Rumoca switched to a pinned git dependency.**
  Doug's phase-explanation docs (11 phases + drill-downs) were the only fork-only content in his
  local Rumoca clone (`dougs-docs` branch = official `upstream/main` + 2 docs-only commits; `crates/`
  byte-identical to upstream). Verified, then: (1) moved `docs/understanding/` into this repo next to
  `docs/notebook/` (source links de-linked to crate-relative inline-code refs, since HRW has no
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
  Modeler vs Rumoca (ritual #1); (2) **the per-specimen lab notebook** (`docs/notebook/`, still just
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
  A "Read: Phase N" button opens the matching `docs/understanding` chapter in VS Code's Markdown
  **preview** (scoped `workbench.editorAssociations` in `.vscode/settings.json`). This resurrects the
  originally-superseded lookup layer, correctly scoped to the generic tier; the bridge + chat `explain`
  remain the specific tier.
- **2026-07-19 — Field-help regeneration is a committed one-command tool, not an ad-hoc script.**
  `cargo run --example gen_field_help` (`examples/gen_field_help.rs`) locates `rumoca-ir-ast`'s source
  via `cargo metadata` (robust to the cargo-cache hash/rev — no hard-coded path) and rewrites
  `src/field_help.json`. Verified byte-identical to the original ad-hoc extraction. The broader
  "what to do after a Rumoca pin bump" process lives in `docs/updating-rumoca.md`: compiler + tests
  drive code fixes, one command refreshes field help, `docs/understanding` is Doug-only.
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
  The tab is labeled "Typecheck (instanced)" with a tooltip; `docs/understanding` phase numbers are
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
