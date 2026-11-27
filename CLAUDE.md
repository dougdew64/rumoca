# CLAUDE.md — Rumoca fork (`hrw` branch)

This is **Doug's fork of Rumoca** (`dougdew64/rumoca`, upstream `CogniPilot/rumoca`). On the **`hrw`
branch**, this workspace additionally hosts the **HRW observatory** — an egui app for *studying* this
Rumoca compiler — as the workspace member **`hrw/`**.

**The live branch is `hrw-rebase-0.10.4`.** "The `hrw` branch" below names the fork's line of work;
the git branch literally called `hrw` is frozen before the 0.10.4 rebase. Switching machines:
`hrw/docs/setup-windows.md`, *Switching machines*.

## Where the instructions live

- **HRW work → read [`hrw/CLAUDE.md`](hrw/CLAUDE.md)** (authoritative for the observatory) and
  `hrw/DECISIONS.md`. HRW is a normal workspace member: build/run/test with **`-p hrw`**
  (`cargo run -p hrw`, `cargo test -p hrw`) from here, or `cd hrw/`.
- **Rumoca compiler crates** (`crates/rumoca-*`) are upstream code — respect their SPEC files,
  `[workspace.lints]`, and phase boundaries.

## Why HRW is in this repo

HRW hit the ceiling of Rumoca's *public API*, which exposes each phase's *result* (the IR) but not
the algorithms' internal *process* (Pantelides iterating, matching's augmenting paths, BDF order/step
control…) — exactly what HRW's learning mission needs to make visible. So HRW moved in-workspace
(path deps on `../crates/rumoca-*`) to enable **instrumenting Rumoca internals**, with a monorepo's
atomic phase+render commits. See `hrw/DECISIONS.md` (the in-workspace move).

## Accuracy outranks everything, and it is why the instrumentation exists

**Doug's top priority is his education; HRW is only the tool.** *(2026-08-04)* Learning Rumoca
from HRW requires that HRW represent Rumoca accurately, so **an inaccuracy is not a quality
issue here — it teaches Doug something false, and he cannot tell which parts are false.**
Accuracy therefore outranks features, polish, performance, and **the cost of changing a
`crates/rumoca-*` file.** His standing authorisation: *"we will pause and fix code as often as
necessary in order to deliver accuracy."*

This has a direct consequence for the discipline below. **When HRW cannot observe something
through the existing surface, the answer is to instrument Rumoca — never to have HRW
approximate, re-run, or invent it.** That inversion is not hypothetical: HRW spent weeks
re-running phases and presenting the re-runs as the compilation, because an HRW-side workaround
had no checklist while a Rumoca change did. Every one of those replays was replaced by capture
scopes in two days, once the trade was priced out loud. **The rules below are a quality bar for
a change that should happen, not a reason to avoid it.**

## Every tab observes the run that simulates *(Doug, 2026-11-26)*

**Every HRW tab names the Rumoca call it observes, and a test proves that call is on the path
that produces the simulation.** A tab's data must come from the compile, or from the one
preparation the simulation integrates (`hrw/src/worker/preparation.rs`) — never from HRW calling
a phase itself on some other input. `hrw/src/bridge.rs`'s `phase_source` is the roster of
those calls.

- **A test against HRW's own traces does not count.** The traces are written by the same
  stages, so a lab or tab agreeing with them proves nothing. The proof must reach the real run
  (a capture scope, the shared artifact) or an independent oracle (System Modeler, a closed form).
- **A real Rumoca function is not proof of fidelity.** `inspect_prepare_for_solve` is genuine
  Rumoca on genuine data, and for weeks it was the wrong input: the solve path simplifies the DAE
  first. Read the path that simulates; do not trust a function's name or a code comment.
- **A tab without that test is unproven**, and must be treated as possibly false until it has
  one.

**Why:** found 2026-11-26. From Structural onward the tabs showed HRW's own calls on the DAE
before solve lowering's simplifications. The symptoms — `Drivetrain`'s 5 / 3 / 3, the pendulum
"refused" yet simulating — were explained away and patched, while every check compared HRW to
itself. The labs built on those tabs taught a run Rumoca never makes. See `hrw/DECISIONS.md`
(2026-11-26).

## Instrumentation discipline

Instrumentation of the Rumoca crates is **intended**, but must be:
- **Additive & observation-only** — semantics-preserving, so HRW stays faithful to real Rumoca and
  rebases on upstream stay clean.
- **Upstreamable** — shaped as a general observability/tracing API (a candidate PR to CogniPilot),
  and kept **separable from `hrw/`** so an upstream PR is a clean cherry-pick of Rumoca-only changes.
  After touching a `crates/rumoca-*` file, run **`cargo clippy -p <that-crate> --all-targets`**: the
  Rumoca crates are clippy-clean and `[workspace.lints]` denies, so a lint the instrumentation
  introduces would fail upstream CI. `cargo test` passes straight through these.

Doug aims to upstream this work and become a Rumoca maintainer. "Updating Rumoca" = **rebasing the
`hrw` branch on upstream** (`hrw/docs/updating-rumoca.md`), not a pin bump.
