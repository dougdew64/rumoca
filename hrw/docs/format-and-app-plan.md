# Plan — `cargo fmt`, and what to do about `app.rs`

**Purpose:** the ordered plan for the two structural items left after the accuracy sweep, with
the measurements they were sized from.
**Status:** proposal, pending Doug's agreement. Not started.
**Read when:** before beginning either piece, and afterwards to check the claims held.

Written 2026-08-05, after the six accuracy items closed and the corpus run came back green.

---

## What was measured, before proposing anything

| | Measurement | Note |
|---|---|---|
| `app.rs` | **10,688 lines** | 9,434 at the UI pause (2026-08-02), 9,900 on 08-03 — **+13% in three days** |
| `hrw/` formatting | **1,206 diff hunks** | `worker.rs` 309, `app.rs` 229, `bridge.rs` 56, `fidelity.rs` 48, `ui_tests.rs` 36, then a long tail |
| **Rumoca crates** | **82 diff hunks** | structural 49, flatten 16, dae 13, compile 4, typecheck 0 |
| CI | `cargo fmt --all -- --check` | a **gating job**, not advisory |
| `rustfmt.toml` | **absent** | default style; nothing to negotiate |

**The 82 Rumoca hunks are ours, and that is a new finding.** `crates/rumoca-phase-structural/src/lib.rs`
at upstream `8cdc7419` is **fmt-clean**; the working copy has 5 diffs. Checked by restoring the
upstream file and re-running the check. **The instrumentation introduced them**, and
`tech-debt.md`'s standing claim that the fmt failure is *"`hrw/`'s fault alone"* is therefore
**false** and is corrected as part of step 1.

**The rule that let this happen names clippy and not rustfmt.** `CLAUDE.md` says: *after touching
a `crates/rumoca-*` file, run `cargo clippy -p <that-crate> --all-targets`*, because those crates
are clippy-clean and `[workspace.lints]` denies. That was followed every time. **It says nothing
about `cargo fmt`**, and upstream CI runs both.

---

## Step 1 — Rumoca-side formatting (82 hunks)

**First, and separable from everything else**, because it is the only piece that blocks something
external: `docs/upstream-strategy.md` stakes Doug's credibility on work that is reproducible and
honestly bounded, and a PR that fails the `fmt` job on arrival spends that credit before a
maintainer reads a line of it.

```powershell
cargo fmt -p rumoca-phase-structural -p rumoca-phase-dae -p rumoca-phase-flatten -p rumoca-compile
cargo clippy -p rumoca-phase-structural --all-targets
cargo test -p hrw --lib -- --test-threads=1
```

- **One commit, Rumoca crates only**, per the standing rule that instrumentation commits stay
  separable for a clean cherry-pick.
- **Amend the rule in `CLAUDE.md`** so the next instrumentation change runs `cargo fmt --check`
  alongside clippy. The rule failing silently for a week is the actual defect; the 82 hunks are
  the symptom.
- **Correct `tech-debt.md`'s "`hrw/`'s fault alone".**

### ~~Risk: none worth naming.~~ — DONE 2026-08-05, and that claim was wrong

**Formatting broke the build.** Rewrapping pushed
`reduce_constrained_dummy_derivatives_with_trace` from 99 lines to **102**, over
`[workspace.lints]`'s `too_many_lines` threshold of 100. *"82 hunks of whitespace in code whose
tests pass either way"* was written without asking whether any lint counts lines — and one does.

**Fixed by extracting `emit_reduction_step`**, which fills in the `demoted_so_far` and `round`
fields that all five emission sites repeated identically: mechanical, same frames in the same
order, and worth removing regardless. The function is **ours** (absent from upstream
`8cdc7419`), so the limit is ours to respect.

**The general lesson, which step 2 inherits:** `fmt` and `clippy` interact, so **run `fmt`
first and `clippy` on the formatted code.** The reverse certifies the code in a shape it will
not ship in. `CLAUDE.md`'s rule now says so.

**Outcome:** four crates at 0 clippy warnings and 0 fmt diffs; 596 HRW tests pass, including
the index-reduction animation tests that consume the frames this touched.

---

## Step 2 — `hrw/` formatting (1,206 hunks)

**Second, and before the `app.rs` work, not after.** Formatting first means the refactor's diff
is readable as a refactor; refactoring first means two large mechanical diffs interleave and
neither can be reviewed.

```powershell
cargo fmt -p hrw
cargo clippy -p hrw --all-targets
cargo test -p hrw --lib --features slow-tests -- --test-threads=1
```

**One commit, formatting only, nothing else in it.** The value of that is not tidiness — it is
that the commit can be **reverted wholesale** if anything downstream turns out to depend on
current layout, and can be **skipped in `git blame`** afterwards.

### Three things to check before running it, each cheap and each capable of failing

1. **Does any document cite an `hrw/` source line number?** The fixture tours do not (verified
   2026-08-04, and `docs/compiler-phases/` cites *Rumoca* symbols). **`docs/` more broadly is
   unchecked.** `grep -rn "\.rs:[0-9]" docs/` before, and fix what it finds — a reformat
   invalidates every such citation silently.
2. **Comments are not reflowed.** `wrap_comments` is off by default, so the long explanatory
   comments this codebase is built from stay byte-identical. **Confirm it rather than assume:**
   the diff should contain no comment-only hunks.
3. **`doc_citations` must pass unchanged.** It parses source for `#[test]` placement and
   attribute adjacency, which is exactly the sort of thing formatting moves.

### The one real hazard

**A formatting pass is indistinguishable from a semantic change in review**, which is why it
gets its own commit and why the test suite is the gate rather than reading the diff. If the
suite passes before and after, and no non-formatting file is touched, the change is what it
says it is.

---

## Step 3 — `app.rs`

**Last, and the only one of the three that needs judgement rather than a command.**

### What the UI pause settled, and what it explicitly did not

`docs/ui-pause-plan.md` (2026-08-02) cut `App` from 105 fields to 57, `frame_ui` from 727 lines
to 419, and `central_panel_ui` from 771 to 430. **It also recorded, deliberately, that the claim
"`app.rs`'s size causes editing defects" remains unproven either way** — and that the honest test
is whether `ui-findings.md`'s R-series stops recurring.

**Three days of heavy editing since then is evidence, and it points the other way than expected.**
`app.rs` grew 13% and the defects found in it during the accuracy sweep — the source pane's
`unwrap_or_default`, the Context Bar's over-broad test queries — were **not** size-related. They
were the same silent-substitution and identity-by-substring patterns found in 700-line files.

**So the refactor's justification is blast radius and testability, as recorded, and not size.**
That distinction decides the shape below.

### The seam: extract by pane, and only where a test follows

The largest functions, measured:

| Lines | Function |
|---|---|
| 419 | `central_panel_ui` |
| 391 | `Default::default` |
| 308 | `frame_ui` |
| 267 | `specimen_source_ui` |
| 256 | `context_bar_ui` |
| 253 | `stage_tab_bar_ui` |
| 247 | `source_map_ui` |

**`specimen_source_ui` and `context_bar_ui` are the two to move first**, and for a reason that is
not their size: **both produced accuracy defects this week**, and both are panes — so under the
pane-is-a-reporter rule each extraction ships with a headless test it does not have today.

**The rule for the whole step, and it is the point of doing it this way:** *no extraction lands
without a test that could not have been written before it.* An extraction that only moves lines
buys nothing measurable and costs a large diff. If a candidate has no such test, **leave it
where it is** and say so.

### What NOT to do

- **Do not extract `Default::default`** (391 lines). It is long because `App` has 57 fields, it
  is mechanical, and it has no behaviour to test.
- **Do not chase a line-count target.** `MAX_APP_FIELDS` already ratchets the thing that
  matters, and it is a *field* count precisely because that is what correlates with coupling.
- **Do not sell this on the corruption episodes.** Recorded 2026-08-02 after Doug pushed back on
  exactly that over-claim: a large file *pressures* toward generators, but the corruption habit
  operates on small files too.

### Sequencing against the fidelity policy

**The `app.rs` work can trip fidelity trigger 3** — "when HRW changes how it emits or reads
stage JSON" — if an extraction touches the `*_to_json` functions, `bridge.rs`'s path grammar,
`IncidenceMatrix::from_report`, or an animation's construction. **`cargo fmt` cannot**, being
formatting only.

So: **no re-run after step 1 or 2. After step 3, say plainly which of those it touched** — and
if it touched none, do not run. And the standing gate applies either way:
[`ideas.md` #46](ideas.md) is built **before** the next large run, because the 2026-08-04 sweep
measured that the corpus cannot exercise F10's absence clause.

---

## Order, and why it is not negotiable in one place

1. **Rumoca fmt** — unblocks upstreaming, tiny, isolated.
2. **`hrw/` fmt** — must precede step 3 or the refactor becomes unreviewable.
3. **`app.rs`** — needs judgement, and each move waits for a test that justifies it.

Steps 1 and 2 are commands with checks around them. **Step 3 is open-ended and should stay
open-ended** — it stops when extractions stop buying tests, not when a number is reached.
