# Tech-debt plan — splitting `app.rs`, then the sweep's findings

**Purpose:** the ordered plan for reducing `app.rs`, and the tech-debt work queued behind it.
**Status:** live plan, opened 2026-08-19. **Read when:** starting any of this work, or tempted to
add to `app.rs`.

**Doug's charge, and the second half is a hard requirement rather than a goal:**

> *"You should determine the size of the pieces based upon a goal of reducing the frequency of
> context maintenance and upon an absolute need of never, ever ruling out potential work simply
> because `app.rs` is too large for you to even consider."*

**The second clause is the binding one.** A size that merely reduces handoffs is an optimisation;
a size that keeps work *considerable* is a capability floor. On 2026-08-19 the floor was breached
in the observable way: during a comprehensive sweep, `app.rs` was filed under *"not looked at"*
because reading it did not fit — **so the file's size removed the file from consideration**, and
the work that would fix that is the work that got skipped. `format-and-app-plan.md` records the
circularity.

---

## 1. The target size, derived rather than asserted

**Measured distribution of `hrw/src/` (59,301 lines total):**

| file | lines | |
|---|---|---|
| `app.rs` | **14,437** | the subject |
| `worker.rs` | 10,594 | next largest, deferred — see §4 |
| `bridge.rs` | 3,772 | |
| `doc_citations.rs` | 3,399 | mostly tests |
| `ui_tests.rs` | 2,550 | mostly tests |
| `fidelity.rs` | 1,765 | |
| `equation_sheet.rs` | 1,540 | |
| `tree.rs` … `lib.rs` | 1,051–1,302 | eight modules |

**The 1,000–1,500 band is the empirical evidence.** Nine modules sit there, and **none has ever
produced the failure modes `app.rs` produced this week** — no line-number arithmetic to locate an
edit, no shell-generated source, no editing against stale assumptions. They are read whole,
routinely.

**So the target is: no module over ~1,500 lines, and most under ~1,000.** Not a style rule — a
statement that a module should be readable *in full* while leaving room to do the work. Claude
must not estimate his own context size (`CLAUDE.md`), so this is anchored to observed behaviour
on real files instead.

**`app.rs` at 14,437 is roughly ten such modules.** That is the shape of the job.

---

## 2. What NOT to do — inherited, and it still binds

From [`format-and-app-plan.md`](format-and-app-plan.md), unchanged:

- **No extraction whose only justification is line count.** Every step below names either a test
  it buys (trigger 3) or the specific thing it stops a session from having to hold (trigger 2).
- **No extraction that just moves a `&mut self` method behind a new name.** If the caller must
  still hold everything, nothing was reduced.
- **Do not extract the paint path from its state** and leave a signature with nine arguments.
  `tree.rs` already carries two `#[allow(clippy::too_many_arguments)]` from that pressure.

**And a new one, from this week:** `architecture.md` is **generated** and carries the module
sizes and `App`'s field groups. **Start from that map.** Reading 14,437 lines to rediscover a
structure already written down is what made this un-proposable in the first place.

---

## 3. The seams, in order

**The generated field-group map is the seam list.** `App` has **15 groups**, six already retired
by earlier extractions — `TourState`, `ModelListState` and the rest came out this way, so the
pattern is proven in this codebase rather than proposed for it.

**Order is by independence, not by size.** Each step must leave the tree green and pushable on
its own; a half-finished split is worse than none.

| # | extract | why it is first / what it buys |
|---|---|---|
| 1 | **Group 9 — "how the reader is looking at the current stage"** (`Viewport`) | Already a struct; the rendering that reads it is the largest single block. Buys: stage-view tests that need no `App`. |
| 2 | **Group 12 — cached structural views** | Pure derived data with an existing invalidation rule. Buys: cache-invalidation tests, currently unreachable. |
| 3 | **Group 10 — compilation log** | Self-contained, append-and-render. |
| 4 | **Group 11 — on-demand simulation** | Owns its own request/response cycle. |
| 5 | **Groups 14 + 15 + 16 — pending stage, deferred debug spawn, breakpoint pre-warm** | Three one-shot handshakes with the same shape; together they are one module about *deferred intent*. |

**Stop when the target is met, not when the list is finished.** If `app.rs` is under ~1,500 after
three steps, steps 4 and 5 need re-justifying on their own merits.

**Each step is one commit** carrying: what moved, the test it buys or the holding it removes, and
the new `app.rs` line count. The count goes in the commit message so the trend is greppable
without a tool.

### The working mode is a LOOP — Doug, 2026-08-19

> *"This refactor is going to require many steps. Let's just go ahead and assume that you're
> going to use a loop of performing context maintenance, doing a bit of refactoring, recording
> your findings and updating the plan. Repeat."*

**Four beats, in this order, and maintenance comes first rather than last.** That inverts the
usual instinct and is deliberate: a session that refactors until its context runs out leaves the
next one with a moved boundary and no account of why. Maintenance first means the *previous*
step's findings are safe before the current one can go wrong.

**A "bit" of refactoring is one item or one small cluster, built and tested before the next.**
Not a step of §3 — those turned out to be too large to be atomic.

### Progress

| date | move | `app.rs` | new module |
|---|---|---|---|
| 2026-08-19 | *(baseline)* | 14,437 | — |
| 2026-08-19 | sub-view enums, impls, name helpers (12 items) | 14,298 | `stage_view.rs` (165) |
| 2026-08-19 | `Viewport`, its `Default`, `sub_view_name_for` | **14,200** | `stage_view.rs` (266) |

**Scale check, recorded so it is not rediscovered:** −139 lines against a ~12,800-line gap. **Leaf
types will not get there.** The weight is in the rendering blocks, and §3's field-group order says
nothing about where those sit — see the second finding below.

### Two findings from the first attempt at step 1 — 2026-08-19, reverted

**`app.rs`'s types are INTERLEAVED, not clustered. Never move a span.** The first attempt cut
from `StructuralView`'s doc comment to the end of `impl Default for Viewport` as one slice, on
the assumption that the viewport cluster was contiguous. It is not: `NavEntry`, `UiMode`,
`SpecimenDetail`, `StageViewCaches` and `CompileFrames` all live *between* those items. The cut
moved 530 lines and produced **179 errors**.

**So an extraction must move items individually, each located by its own marker**, and must
verify the build **after each item** rather than after the cluster. A mistake then costs one item
instead of the whole step. This is the line-number-arithmetic lesson one level up: **a span
between two known-good points is not itself known-good.**

**And the seam list maps FIELDS, not CODE.** `architecture.md`'s field groups describe where
`App`'s *state* is grouped; the code implementing a group is scattered across the file. So §3's
order is sound about *what* to extract and says nothing about *how hard each is* — which is what
actually decides whether a step fits in one session. **Estimate each step by locating its items
first**, and treat "how many separate places is this in?" as the real size, not the field count.

---

## 4. Explicitly deferred

**`worker.rs` at 10,594 lines.** Doug: *"If we find that your context maintenance problems have
improved after refactoring `app.rs`, then we will consider refactoring other large files also."*
**It is second, not simultaneous** — and holding it back is what makes `app.rs` an experiment
rather than a campaign. If handoff frequency does not improve, splitting more files is not the
answer and we should know that before spending the effort.

**Note the confound before reading the result** (`CLAUDE.md`): a model change in the same period
would also move handoff frequency, so record which model was in use.

---

## 5. Then, the sweep's findings

From [`sweep-2026-08-19.md`](sweep-2026-08-19.md), in value order:

1. **Absence tags naming prose can never fire.** One was already false for two days. **Verified
   finding, cheap fix**: make the checker reject a target that is not symbol-shaped, so it fails
   when written rather than being silently permanent.
2. **60+ `let _ = …` sites, unclassified.** Some idiomatic, some possibly swallowing errors that
   matter — the "silence must be a failure" class. Needs reading each site.
3. **Five `#[allow]` suppressions outside `dead_code`.** Two are justified in comments; the rest
   unaudited.

**And the sweep's own gap:** it had no measurement and no adversarial pass. The transport-bar
defect was found by walking into it, not by looking for it — so the next comprehensive sweep
should be driven by something other than Claude's own list.
