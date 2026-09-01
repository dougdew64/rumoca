# Running things — the procedures

**Purpose:** the commands, gates and diagnostic tells a session needs to actually run this
project. **Procedure, not rules** — charter Decision 11 puts it here so that `CLAUDE.md` can hold
only what binds.
**Status:** operational. Follow it step by step rather than from memory.
**Read when:** about to gate, commit, run a long sweep, or diagnose a run that is behaving oddly.

*(Split out of `CLAUDE.md` on 2026-09-01. It was 331 of that file's 994 lines and almost none of it
was a rule — it was how to do things. The rules it enforces stayed behind: the gate is green before
every commit, announce a cost before paying it, and the handoff is the last step before a push.)*

**FIRST THING ON A MACHINE YOU HAVE NOT RUN ON BEFORE — RUN THE MACHINE CHECK.** Claude runs it
unprompted: Doug switches machines twice a week, feels every cost, and cannot see any of these
coming, while Claude can.

```text
cargo run -p hrw --example check_machine
```

It verifies what does **not** travel with a `git pull`: the **permission allowlist** (gitignored by
upstream — and *fatal to an unattended run*, since a prompt with nobody awake is indistinguishable
from a hang), whether HRW is holding `hrw.exe`, the parsed-artifact cache, and the bridge
extension. Blocking problems exit non-zero and name their fix.

**ITERATING AND GATING ARE DIFFERENT ACTS, AND CONFLATING THEM COST DOUG TWO HOURS** *(measured
2026-08-15: 172 of that day's 274 compute-minutes went to `--features slow-tests` across 61
invocations, for six commits)*. **The gate is not the problem; running it thirty-odd times is.**

```text
# ITERATE — while editing, and for every must-fire revert-and-check. ~10s.
cargo test -p hrw --lib --features slow-tests -- --test-threads=1 <name-filter>

# GATE — ONCE, immediately before the commit. ~225s.
cargo test -p hrw --lib --test msl_resolve --features slow-tests -- --test-threads=1
cargo clippy -p hrw --all-targets                  # covers the BIN; check the exit code

# The fast suite, when nothing slow-gated is in play. ~15s.
cargo test -p hrw --lib -- --test-threads=1
```

**EVERY TARGET THE GATE NAMES MUST BE SPELLED OUT, because `--lib` is a filter and a filter is
silent about what it removed.** `--lib` alone skipped `tests/msl_resolve.rs` from every pre-commit
gate for at least eleven days; nothing was broken and nothing would have said so.
`doc_citations::every_test_target_runs_in_the_documented_gate` now fails if a file appears in
`tests/` that the command does not name.

**Deliberately still outside the gate**, and these are choices rather than oversights:

| what | why |
|---|---|
| `--features notebook-check` | reloads the MSL 21 times (157 s) and is order-dependent; see the third-gate note above |
| `examples/fidelity_msl`, `examples/survey_msl` | hours-long corpus sweeps with their own runbook and watchdog |
| doc-tests | `cargo test -p hrw --doc` runs **0** tests; there is nothing there to lose |

**The filtered ITERATE line was missing from this file until 2026-08-15, and its absence is the
whole story.** The gate rule answers *"which gate before I commit?"* — correctly — but it was the
**only** decision procedure written down, so it got applied after every edit too, with no
sanctioned cheap option to reach for instead. **A rule that is right for its own question becomes
wrong when it is the only rule present.**

**ANNOUNCE THE COST BEFORE PAYING IT.** Before any command expected to exceed ~60 s, say what
it is and roughly what it costs, so Doug can redirect *before* the wait rather than discover it.
**Elapsed time had no mechanism until 2026-08-26**, when five timing claims died in one day —
each a lone sample quoted as fact, or one total subtracted from another taken under different
conditions. **`cargo run -p hrw --example measure -- <cargo args>`** repeats a command and prints
a figure carrying its provenance; `--versus` interleaves two commands so drift cannot pass for a
difference. **Do not quote a timing it did not produce, and never subtract two that it did.**

**AND THE LAST STEP BEFORE PUSHING IS THE HANDOFF — Claude does this unprompted** *(added
2026-08-19, after Doug had to ask three times)*.

**Ask one question before every push: does a fresh session need to know something it would not
learn from the diff?** A finding, a decision, a correction, or work left owed. If yes, update the
handoff box in *this* commit. If no — most commits — do nothing and move on.

**It is standing authorisation, not a request to be granted**, and it is a **step** in a sequence
that already runs — not a task done when asked, which is the framing that had Doug prompting for
it three times. A session that runs out of context having shipped code and no handoff spent its
budget on the half a `git log` can partly reconstruct and skipped the half nothing can.

**This is the same asymmetry the permission allowlist and the cost-announcement rules record:**
Claude can see the need and does not feel the cost; Doug feels it and cannot see the need.
**Whenever that shape appears, the mechanism belongs on Claude's side.**

**The pre-commit order is FMT, then GENERATE, then GATE — and it is an order, not a set.**
`docs/architecture.md` carries module **line counts** derived from the source, so:

```text
cargo run -p hrw --example gate        # fmt -> generate -> lint -> test, in that order
```

**RUN THE TOOL, NOT THE SEQUENCE — added 2026-08-23, after this order was got wrong for the
TENTH time.** It cost the whole gate four times before that session and **six times during it**,
always the same way: `clippy` and the gate run straight after `cargo fmt`, so the rewrapped source
no longer matches the line counts `architecture.md` carries, and `architecture_regions_are_current`
fails at the end of a 230-second run. **Ten instances is not a memory problem**, and this
repository's own answer to a rule that keeps being got wrong is to give it a mechanism.

**What the runner does is described once, under *Match the gate to the change* below** — not here.
The one thing worth repeating is *why* the decision lives in `gate_policy` rather than in the
runner: choosing FULL needlessly costs four minutes and is obvious, while **choosing FAST wrongly
is silent**, so the rule sits in the library where a test can reach it.

`gen_field_help` is still outside it, deliberately: `build.rs` runs it.

**The gate fails while HRW is running** — `Access is denied` on `hrw.exe`. Doug builds from the
tree and keeps the app open, so this is normal, and **not always transient**: once a preceding
`clippy --all-targets` has invalidated the binary's fingerprint, it retries forever. **Split it and
both halves run with the app open** — selecting one target at a time does not pull in the bin:

```text
cargo test -p hrw --lib --features slow-tests -- --test-threads=1
cargo test -p hrw --test msl_resolve --features slow-tests -- --test-threads=1
```

**With HRW closed the combined line works** (measured 2026-08-22: 823 + 2, 128 s), which is why an
unattended run requires it closed — see [`docs/unattended-runs.md`](docs/unattended-runs.md), **read
that before doing any work while Doug is asleep.**

**`.hrw-bridge/tour.md` IS LIVE STATE, AND TESTS THAT PAINT MUST HOLD IT** *(2026-08-16, three
defects in one hour)*. It is Claude's answer to Doug's last question, and `tour::poll`
**auto-selects it** when nothing else is chosen — which resets the stage side. So its mere
presence changes what a painted frame does, and the suite had never run while one existed.

All three failure modes appeared the first time one did: a test that *asserted* the file was
absent (failing whenever the feature had been used), a test that wrote its own and **deleted**
Doug's afterwards, and a test that painted against whatever was on disk. Use
`ui_tests::AdHocTour::absent()` or `::with(text)`; both restore what was there, including on a
panic.

**A THIRD GATE EXISTS AND IS NOT IN EITHER OF THOSE — the notebook content check** *(added
2026-08-15)*. The committed specimen traces had been stale for **25 days** and nothing could
notice; `manifest_stage_rosters_match_the_pipeline` now catches a stage appearing or
disappearing for free, but only this catches *contents* drifting:

```text
cargo test -p hrw --lib --features notebook-check -- --test-threads=1 the_committed_notebook
cargo run -p hrw --example gen_trace -- --all      # 3m45s, the fix when it fails
```

**Run it after touching a `*_to_json` writer and after rebasing on upstream** — the same two
triggers the large fidelity sweep carries. It costs **109 s** (was 157 s until the parsed-source-root
memo landed 2026-08-21) and has its own feature because it must give each specimen a **fresh**
`WorkerState`: against the shared worker it is *order-dependent*, passing alone and failing in
company.

**IT IS ALSO THE BENCHMARK FOR ANY MSL-LOADING CHANGE, and that is worth knowing before reaching
for the gate.** It performs **21 MSL loads** by construction, so a per-load saving shows up 21
times against very little else — the memo's 48 s was predicted from counters at 49 s and measured
here at 48 s. **The gate cannot do this** — not from variance (**0.7 %**, measured 2026-08-25) but
because 92 % of it is specimen compiles, which drown a per-load saving. `DECISIONS.md`, 2026-08-25.

**AND IT IS A FIDELITY CHECK, not only a staleness check.** It compares every specimen's committed
per-stage IR against a fresh compile, so it is the instrument for *"did this change what Rumoca
produces?"* — the question a green gate cannot answer, since the gate was green before the change
too. Run it for any change to the compile or library-loading path, not just to a `*_to_json` writer.

**That order-dependence is a fact about the notebook, not about the test, and it belongs
here rather than in a test comment.** A committed trace is one sample of a function whose
hidden argument is the session — `gen_trace` runs one process per specimen, so what is
committed is the *virgin-session* value. Two specimens (`GearWithBrake`,
`MissingComponentClass`) demonstrably emit different JSON depending on what the session already
holds. So **"the trace is correct by construction" is true only of a virgin session**, and any
future code that reproduces a trace must reproduce that condition — including how the specimen
path is *spelled*, since `parse_to_ast` stamps it into every `Location` and a `\` for a `/` made
109 of 109 files look like total drift.

**The drive letter's CASE is part of that spelling**, and `CARGO_MANIFEST_DIR` is not stable in it
— `gen_trace::uppercase_drive_letter` canonicalises it, and its call site carries why.

**MATCH THE GATE TO THE CHANGE — and the RUNNER decides, not this file** *(the prose that used
to restate the rule was retired 2026-08-31)*.

```text
cargo run -p hrw --example gate
```

**It reads the working tree and picks FAST, TOUR or FULL.** The rule itself is
`gate_policy::needs_full_gate` and `touches_a_verified_tour_region`, each with a test; the runner
adds the generators, `fmt`/`clippy` for any touched Rumoca crate, and refuses to start while HRW
holds `hrw.exe`. `--fast` / `--full` override.

**Why no table here any more, and this is the general rule not a local tidy-up.** A gate verdict
was stated in **seven** governing documents. Three of 2026-08-31's contradictions were that prose
gone stale — two documents and the runner's own header still saying two verdicts, and three places
still charging FULL for a tour-table edit hours after the TOUR gate made it 11 s. **Not one was a
disagreement about what the gate should do; every one was a copy that had not been updated.**

**So this file applies its own rule to itself: A CHECKER RETIRES THE PROSE IT REPLACES.** The gate
has a mechanism *and* a test, so the mechanism is the statement and prose anywhere else is a copy
waiting to rot. **Run the runner and read its verdict.** What belongs here is only what the runner
cannot say: *this decides what to run BEFORE A COMMIT, and is not the answer to "what do I run
after this edit"* — that is the filtered iteration line above, and conflating the two turned a
latency fix into a latency cost on the day it landed.

**WHERE THE GATE'S TIME GOES, measured 2026-08-21** (`docs/ideas.md` #48): **72 compiles at ~3.4 s
and 10 MSL loads at ~4.4 s are 92 % of the run.** Each compile re-resolves the whole MSL, so a
two-equation specimen costs 3.5 s and the same file with no MSL loaded costs 0.03 s.

**FIVE levers are already ruled out by measurement — do not re-propose them**, and #48 records
which and by how much: cutting `t_end`, parallelism, memoising simulations, memoising specimen
compiles (already built — `compile_specimen_shared`), and feature-set thrashing.

**The pattern in all five, and it is the reason to keep the list: a sum of slow-looking names is
not a measurement.** Three were proposed from arithmetic over test names and died on contact with
a clock. **Measure the thing, then decide** — `examples/measure` exists for exactly this.

**`--test-threads=1` is required, and since 2026-08-20 it is the DEFAULT** — two pre-existing
tests race on process-global stdout and on `focus.json`, and the suite **deadlocks** under the
default harness on a clean tree. `.cargo/config.toml` at the **workspace root** now sets
`RUST_TEST_THREADS = "1"`, so a bare `cargo test` is correct too. The commands below keep the
explicit flag: it is free, it survives someone running from a directory whose config differs, and
it documents the requirement where the reader is.

**Recognising it costs fifteen seconds, and output will not tell you.** `OutputCapture` owns
fd 1, so a hung run **stops printing which test it is on** — the last line is whichever test
flushed, not the culprit, and it will accuse an innocent one. Check the process instead:
**frozen CPU time with every thread in `Wait`** is hung; accumulating CPU is merely slow. It was
misread as slow for ninety minutes on 2026-08-20.

**AND A THIRD CASE THAT TWO-WAY TEST DOES NOT COVER — THE MACHINE SLEPT.** A gate step timed at
**27,668 s** on 2026-08-23 was diagnosed as build contention and was not; Doug supplied the cause,
and the **10,780 s** run above is almost certainly the same thing. **Sleep corrupts the clock, never
the verdict** — a suite that passes across a suspend still passed, so what is lost is every timing,
silently. `check_machine` now rules on it and `machine_policy` carries the reasoning; **do not ask
Doug to remember this**, which is the mistake that produced it twice.

**`cargo test` does not build the binary, and that gap is not theoretical.** On 2026-07-31 a
`#[cfg(test)]` was placed above the first of three lifted helpers — **the attribute applies to
one item** — so two compiled into `--bin hrw` referencing test-only imports. Every test passed;
**Doug's debugger launch failed.** `cargo clippy --all-targets` covers the bin, **but check its
exit code, not its output**: the same breakage survived a clippy run piped to
`grep -c "^warning: "`, which counts warnings and silently ignores a compile error.

**Use the fast suite between edits and the full one before every commit.** Measured 2026-07-29:
49 of 402 tests took 180 of the suite's 183 seconds, nearly all compiling a specimen against the
MSL. They are gated by `slow-tests` and reported as ignored *with a reason*. **Parallelism is
not the fix** — they serialize on a global `Mutex<WorkerState>` regardless, saving about two
seconds; `docs/ideas.md` **#48** (memoize compiled specimens) is.

**Long runs → [`docs/long-runs.md`](docs/long-runs.md)**, the runbook for the MSL survey and the
fidelity sweep: copy-paste commands, what to watch, how to resume, what each abort verdict
means. *Why* each precaution exists is `docs/architecture.md` §11.

**NEVER run the fidelity sweep unbounded, and never in a bare loop** — an unbounded 53-model run
made Doug's machine unusable and forced a hard power-cycle (2026-07-31). **Use the watchdog, and
follow `long-runs.md` step by step rather than from memory**: it carries the commands, the
rust-analyzer stop, why `--release` is not optional, the 3 GB free-RAM floor and the
no-backtick-continuations rule, each with its account.

**Two principles that outlive any particular runbook step:**

- **Only process exit bounds memory.** A session rebuild releases what the session holds, not what
  the allocator fragmented — hence one model per process, so the worst case is one model.
- **A guard that cannot fire is indistinguishable from no guard.** The watchdog samples **free
  RAM** during the run rather than process size, after a proposed 30 GB ceiling on a 31.7 GB
  machine would never have tripped.

**THE LAST LARGE SWEEP IS DONE** — 2026-08-04/05, 2,614 green, zero violations. The run and its
numbers are in [`docs/fidelity-plan.md`](docs/fidelity-plan.md) and
[`docs/reports.md`](docs/reports.md); what a session needs from it is the **two standing limits**
on what a green sweep means:

**Representation is verified at corpus scale; equivalence at sample scale.** The sweep establishes
that HRW's path grammar round-trips over real Modelica ASTs — **not** that HRW's AST equals
Rumoca's, which nothing in it compares. That equivalence is
`worker::tests::hrw_reparse_of_a_library_file_matches_the_sessions_own_ast`, over 120 documents.

**AND IT SAYS NOTHING ABOUT WHAT THE COMPILER DID** *(added 2026-08-04, after the fictions)*.
Every F-check asks about a **noun**: is this structure what Rumoca produced? The claims HRW
makes about **verbs** — which phase ran, in what order, nested inside what, how long it took,
what it declined to do, and whether a view came from the compile or from HRW re-running the
algorithm — **are outside the programme entirely.** That is not a flaw in the checks; it is
their scope. The flaw was reading a corpus-scale green as an answer to "is HRW faithful?"
**Whenever this file, a commit message or a report cites the sweep as evidence of fidelity,
the noun/verb split must be stated with it.**

**When the fidelity checks run** (policy 2026-07-31; reasoning in
[`docs/fidelity-plan.md`](docs/fidelity-plan.md)). Small scale — the 16 curated specimens —
**stays in the pre-commit run** (~90 s), answering *"did HRW drift from itself?"*, which is not
a rare event: both bugs found on 2026-07-31 were HRW-internal drift, weeks old. Large scale gets
its own feature gate. Run the large suite: **(1)** after rebasing on upstream (a step in
`docs/updating-rumoca.md`); **(2)** before submitting a PR to CogniPilot/rumoca; **(3)** when
HRW changes how it **emits or reads stage JSON** — the `*_to_json` functions in `worker.rs`, the
path grammar in `bridge.rs`, `IncidenceMatrix::from_report`, or any animation's re-derivation.
**Trigger 3 is code-shaped, not judgement-shaped, deliberately** — "when a change gives reason
to doubt fidelity" is exactly the judgement that already failed twice.

**BEFORE THE NEXT LARGE RUN, BUILD `docs/ideas.md` #46** *(Doug, 2026-08-05)* — a failure
specimen per compiler phase. **The 2026-08-04 run measured why**: 0 of 2,614 rows carried a
failure message and no MSL model produces an empty stage, so **F10's absence clause had nothing
to act on** and its zero covers only the two near-tautological clauses. Absence is a property of
*failing* compiles and the corpus has none. Another 8.5-hour sweep of the same corpus would
re-confirm the same narrow zero; #46 is what turns it into coverage. See
[`docs/fidelity-plan.md`](docs/fidelity-plan.md), "F10's first corpus run".

### A `crates/` CHANGE COSTS DOUG ONE FULL MSL RE-PARSE ON HIS NEXT LAUNCH — 2026-08-22

**This is about the running app, not the test gate, and it is the answer to "HRW felt broken
today".** Doug rebuilt HRW after a two-week gap on the other machine, and **his first in-app
compile took minutes while every later one was quick.** Diagnosed, then confirmed by his restart:
first compile fast again.

**Three costs, three lifetimes — and the MSL is never *compiled*.** It is parsed and resolved;
only the specimen goes through the phase pipeline. Conflating those three is what makes the
behaviour look mysterious:

| work | lifetime |
|---|---|
| **parse** MSL text → ASTs | **on disk, indefinitely** — `%LOCALAPPDATA%\Rumoca\source-roots\parsed-files` |
| **load** ASTs into the `Session` | until the HRW process exits |
| **resolve** names → DefIds (38,855) | until the next compile — `compile_target` invalidates it every call |

**So HRW reloads the MSL every launch and re-resolves it every compile; what it does not do is
re-parse it** — that result is cached on disk (218,558 files on Doug's machine, since 2026-07-29).
The cost lands on the *first compile* rather than at launch because `App::new` sends
`SetLibraries` and **the worker is one thread processing messages in order**, so an early compile
queues behind the library load.

**THE INVALIDATION RULE, which is the part worth remembering.** The artifact cache key is
`blake3(schema ‖ compiler ‖ file_name ‖ source_hash)`, and `compiler_source_fingerprint()`
(`rumoca-compile/src/source_root_cache.rs`) hashes **the entire `crates/` tree** plus the
workspace `Cargo.toml`, `Cargo.lock` and `rust-toolchain.toml`. **Any change anywhere under
`crates/` invalidates every cached parsed file at once.**

- **A `crates/rumoca-*` edit therefore has a cost this file did not previously price**: beside
  clippy, fmt and upstreamability, it buys Doug **one full MSL re-parse on his next launch.**
  Still cheap against what instrumentation buys — but say so when proposing one, per
  *ANNOUNCE THE COST BEFORE PAYING IT*, since he pays this one and cannot see it coming.
- **`hrw/` edits are FREE.** `hrw/` is outside `crates/`, so ordinary HRW work never invalidates
  it. **Adding a dependency does**, by moving `Cargo.lock`.
- **The tell, when Doug reports a slow launch:** check the mtime of
  `%LOCALAPPDATA%\Rumoca\source-roots\parsed-files`. Recent means it was being *written*, which
  only happens on a miss. `semantic-summaries` is a separate layer with its own lifetime.

**Do not run `du` or `ls -lt` on that directory** — 218k entries, and both timed out at two
minutes while diagnosing this. `ls -ld` on the directory answers the question in milliseconds.

---

## Reading a debugger stop

**WHEN DOUG IS AT A BREAKPOINT, READ `hrw/.hrw-bridge/debug-state.json`** *(built 2026-08-08,
`docs/ideas.md` #72)*. **Claude cannot see a debug session** — #70 measured it: a stop yields no
location, no stack and no values, and no tool exposes them. Stopping does surface the *file* via
an `ide_opened_file` event, and a **selected** line arrives, but the running program's state does
not. So the bridge extension publishes it: stack frames, the innermost location, and the locals of
its most local scope.

**Three rules for reading it, and the first is not optional:**

- **CHECK `writtenAtMs` AND `seq` BEFORE BELIEVING ANY OF IT.** A payload from the *previous* step
  is indistinguishable from a current one by content alone, and describing the wrong state
  confidently is the exact failure this repository spends most of its rules on. Nothing deletes
  the file at shutdown, deliberately — so a stale file is the expected case, not an anomaly.
- **`variables: null` means NOT FETCHED**, with `variablesError` saying why. `[]` means fetched
  and empty. Never report the first as "no locals".
- **`frameCount` is the truth; `frames` may be capped** (`framesTruncated`). Depth matters:
  for `augment_traced` the stack **is** the augmenting path — N nested frames is an N-edge
  alternating path with each frame's `eq` a node on it.

**And do not substitute `breakpoint-request.json` for it.** That file holds the line HRW *asked*
to arm, which at the live-trace anchor coincides with where Doug is stopped — so answering from it
looks like working debugger vision until the day he stops somewhere else. #70 records this trap in
full; **right often enough to be trusted is the failure mode.**

**Two adapter facts that were expensive to learn, and that no layer can detect for you:**

- **`cppvsdbg` will not re-bind a breakpoint at a location whose breakpoint left the adapter's
  active set during a session** — by removal *or* by being disabled. Only a **new debug session**
  recovers it. So a second Debug press can silently fail to stop. (`docs/ideas.md` #74)
- **VS Code exposes no `verified` field to extensions**, so `breakpointPresent` can only ever mean
  *"an enabled breakpoint exists"* — never *"execution will stop there"*. Do not report the first
  as the second. (`docs/ideas.md` #75)
