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
