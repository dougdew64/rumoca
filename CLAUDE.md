# CLAUDE.md — Rumoca fork (`hrw` branch)

This is **Doug's fork of Rumoca** (`dougdew64/rumoca`, upstream `CogniPilot/rumoca`). On the **`hrw`
branch**, this workspace additionally hosts the **HRW observatory** — an egui app for *studying* this
Rumoca compiler — as the workspace member **`hrw/`**.

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
