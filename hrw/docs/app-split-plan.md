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

### THE STOPPING RULE: one extraction per session, then `/clear` — 2026-08-19

**One unit of work, gated, committed, and then a deliberate fresh session.** Not "continue until
the context runs out."

**Because context maintenance is insurance, not recovery.** It *costs* context — reading, editing,
committing — to make running out **survivable**. It frees nothing. Doug had been requesting it
expecting relief, and each request spent more of the budget it was meant to protect. Saying that
plainly is the point of this section.

**What running to exhaustion actually cost on 2026-08-19:** the last third of a very long session
went on limping — reverted extractions, four backtick corruptions repaired, handoff boxes that had
drifted two iterations behind. **Two or three fresh sessions would have moved more code.**

**The rule makes each iteration's cost bounded**, and it makes maintenance cheap for the reason
that matters: there is little accumulated state to write down when you stop after one thing.

**Claude's side of it, since three of these are self-inflicted:**

- **Commit messages of 20–40 lines are context spent by choice.** The reasoning belongs in the
  code and the docs, where it is greppable; the message needs what changed and why it was safe.
- **Do not narrate each step.** Doug asked for the loop to run without updates and got essays
  anyway. Each one was real budget.
- **Read the body before scripting an extraction.** The two reverts and the `specimen_source_ui`
  cascade all came from rewriting first and discovering the shape afterwards.

**And this confounds the handoff-frequency signal a third time** (`CLAUDE.md`): model change, file
growth, *and* verbosity discipline all move it. Isolating `app.rs`'s contribution needs the other
two held still.

**THE LOOP'S COST PROFILE CHANGES AFTER ITERATION FIVE, AND THAT NEEDS A DECISION.** The first
five were cheap *because the items could not fail interestingly*: leaf types with no `App`
coupling, moved by marker, verified by a build. **That supply is now exhausted.**

What remains is eight rendering functions reaching across `App` — a 602-line `central_panel_ui`
touching fifteen-odd fields. **"A bit of refactoring per iteration" may not survive contact with
one of those**, because the unit of work is no longer "move an item" but "establish what state
this function actually touches", and that cannot be half-done and left green.

**Decide the rhythm before starting one, rather than discovering it mid-extraction.** Two honest
options: spend a whole session on *one* function, or spend one first on **measuring the coupling**
of all eight and let the numbers pick the order. The second is cheaper and is what the plan's own
"estimate each step by locating its items first" rule already asks for.

### Progress

| date | move | `app.rs` | new module |
|---|---|---|---|
| 2026-08-19 | *(baseline)* | 14,437 | — |
| 2026-08-19 | sub-view enums, impls, name helpers (12 items) | 14,298 | `stage_view.rs` (165) |
| 2026-08-19 | `Viewport`, its `Default`, `sub_view_name_for` | 14,199 | `stage_view.rs` (266) |
| 2026-08-19 | `StageViewCaches` + impl | 14,127 | `stage_caches.rs` (99) |
| 2026-08-19 | `UiMode`, `SpecimenDetail`, `NavEntry` | 14,076 | `ui_state.rs` (73) |
| 2026-08-19 | **`source_map_ui`** + its constant — *first rendering fn* | **13,838** | `source_map.rs` (281) |
| 2026-08-19 | **`specimen_source_ui`** + `SourceViewState` — *the reverted one, redone by hand* | **13,507** | `specimen_source.rs` (397) |
| 2026-08-19 | **`autoplay_controls_ui`** + `autoplay_stop_heading` + two constants | **13,152** | `tour_transport.rs` (458) |

**The first rendering function left, and the signature is the result.** Four parameters instead of
`&mut self`: `ui`, three shared refs, and `&mut Viewport` because the view genuinely moves the
camera. **That is what makes it an extraction rather than a rename** — the compiler now enforces
what the source map may touch, and a reader learns it from the signature.

**`SOURCE_MAP_SPLIT_FRACTION` moved with it.** A constant used by exactly one function is state
that function owns; leaving it behind would reduce what `app.rs` *holds* without reducing what it
*declares*.

**−238 lines from one function** — more than the first three state moves put together (−310 across
three), which is the coupling measurement paying for itself.

**Trap 2 fired again on that last move** — a `sed` insert landed between `#[derive]` and its enum,
an hour after the trap was written down. **Reading a rule is not the same as it being available at
the moment it applies.** The durable fix is not another note: it is to stop inserting imports by
line and put them in the module's header when the file is first written.

**Scale check, recorded so it is not rediscovered:** −139 lines against a ~12,800-line gap. **Leaf
types will not get there.** The weight is in the rendering blocks, and §3's field-group order says
nothing about where those sit — see the second finding below.

### `specimen_source_ui` extracted on the second attempt — 2026-08-19, and the method is the finding

**−331 lines, the largest single move so far**, and it succeeded for one reason: **the body was
read before anything was written.** The first attempt rewrote first and discovered the shape from
the errors; this one enumerated the seven fields and the three multiline `self` accesses up front,
so every obstacle the revert had hit was already known when the edit started.

**The four obstacles, and what each actually cost once seen in advance:**

| obstacle | fix | cost |
|---|---|---|
| `self.set_tracked_identifier(name)` — a method | return `Option<String>`, `App` follows | 6 lines |
| three multiline `self\n .field` accesses | one `perl -0777` pass instead of line-wise `sed` | one regex |
| `source` is mutated | `&mut SourceViewState` | a keyword |
| a local `let source` **shadowing** the parameter | rename the local to `source_text` | one line |

**Three of the four were one-line fixes.** What made the first attempt cost hours was not their
difficulty — it was meeting them one at a time, each invisible until the previous was repaired.
**The generalisable rule is therefore not "hand-edit the hard ones"**: it is *enumerate the
obstacles before editing*, after which a script does most of the work anyway. This extraction was
still 90% scripted.

**Two type errors the rewrite introduced that no enumeration would have caught**, both from
`&Option<T>` parameters replacing owned fields, and both caught by the first build:

- `self.tracked_identifier != self.source.scrolled_for` needs `*tracked_identifier` once the left
  side is a reference.
- the early `return;` in the library-error arm becomes `return None;`.

**`SourceViewState` moved with it**, the same rule that moved `SOURCE_MAP_SPLIT_FRACTION`: state
used by exactly one pane is state that pane owns. Its nine fields became `pub(crate)`, which is
the unavoidable price and has the `Viewport` precedent — a struct cannot cross a module boundary
and stay private to `App`.

**Eight parameters, and that is deliberately not a rename.** `too_many_arguments` is `allow` in
`hrw/Cargo.toml` because multi-arg widget fns are egui idiom, so the lint did not decide this; the
plan's own test did. Seven named pieces of state is a signature a reader learns the pane's reach
from — `&mut App` would not be.

**Split into two modules rather than one.** The plan groups `specimen_source_ui` and
`source_map_ui` as one *concern*; they ship as `specimen_source.rs` (397) and `source_map.rs`
(281) because both sit inside the target band already and merging them would have meant renaming
`source_map.rs` in the same commit. **Concern is the ordering unit, not necessarily the file.**

### `autoplay_controls_ui` → `tour_transport.rs` — 2026-08-19, and it corrects the coupling metric

**−355 lines, the largest single move so far, built on the first attempt with no revert.** The
method from the previous iteration was followed exactly — enumerate every obstacle before
writing anything — and it held: the extraction was ~90 % scripted, the first `cargo build`
succeeded, and the only test that failed was the generated `architecture.md`.

**THE COUPLING NUMBER WAS COUNTING THE WRONG THING.** The table below rates this function at
**6 fields**, which sits it between `source_map_ui` (4) and `specimen_source_ui` (7) — and it
was far cheaper than either. The reason is visible only in the raw accesses:

| accesses | of what |
|---|---|
| **18** | `self.tour` — one already-grouped struct |
| 1 | `self.compiling` — one `bool` |
| 3 | `self.tour_back`, `self.start_autoplay`, `self.restore_mode_after_autoplay` — methods |
| 1 | `self.autoplay_stop_heading` — a method that **never used `self`** |

**So the signature is four parameters, of which one carries eighteen accesses.** The real
predictor is not *how many fields* but **how many distinct state groups**, because a field
already inside a struct costs the same as one field: `&mut TourState`.

**Which means the 2026-08-02 UI pause paid for this extraction in advance.** That pause created
`TourState` and dropped `App` from 105 fields to 57. Grouping state *is* the preparation for
extracting the views that read it, and this is the first move that demonstrates it — an argument
for finishing the grouping of any pane that resists.

**Re-count the remaining functions by state groups before trusting their order.** The table
below is still the best evidence available, but a function whose fifteen fields turn out to be
three structs is a cheap extraction wearing an expensive number.

**The callback pattern generalises to an ENUM, and that is now the second instance.**
`specimen_source_ui` returned `Option<String>` for one follow; this one has three presses that
`App` must perform, so it returns `Option<TransportRequest>` — `Switch`, `Back`, `Play`,
`Stopped` — and `App` matches on it. **Render and report, own no policy.** Two instances make it
a pattern rather than a special case; expect the third.

**One behaviour was preserved deliberately, and one was allowed to change — both stated in the
module docs rather than hidden:**

- **Stop still stops the clock inside the module.** `Autoplay::stop` is pure `TourState`, so
  deferring it would have let the readout below render one more frame of a run that had ended.
  Only the *mode restore* leaves, which is why the variant is `Stopped` — a report that it
  happened, not a request to do it.
- **Play is deferred by exactly one frame.** `start_autoplay` parses the tour, builds a schedule
  and dispatches a beat; it cannot run mid-paint. On the click frame the length picker is still
  enabled and the progress bar is not yet drawn. **Say this in the module doc**, because a
  future session comparing the two variants will otherwise read the asymmetry as an oversight.

**The chrome helper leaked, and it will keep leaking.** `section_style` and `SectionStyle` had
to become `pub(crate)` — the first cross-module use, following `model_list.rs`'s precedent of
importing `read_purpose` and `section_header` from `crate::app`. **Every left-panel pane that
leaves will need it.** When the third one does, move the pair into its own module rather than
widening more of `app.rs`; there is no reason to do it before then.

**Two mechanical notes worth more than they look:**

- **A quoted heredoc (`<<'RUSTEOF'`) writes backticked prose safely**, which is the concrete
  answer to trap 4 below. The corruptions came from `node -e` and *unquoted* shell strings, where
  the shell expands `` ` `` before the file is written. `grep -c '`'` on the result confirms it
  in one command.
- **Do not dedent a moved body — let `cargo fmt` reindent it.** The body drops from 8-space
  method indentation to 4-space free-function indentation, and a blanket `sed 's/^    //'` would
  also strip four spaces from inside any multi-line string literal. `rustfmt` reindents code and
  leaves literals alone.

### §3's seam order is WRONG for the remaining work — measured 2026-08-19

**Three iterations took `app.rs` from 14,437 to 14,127 — and the next planned step buys nothing.**
Field group 10, "compilation log", is **three fields and no struct**: `log_entries`,
`viewing_log`, `tracing_enabled`. Grouping them into a type is a state-tidying change, and the
plan's own rule forbids an extraction whose only justification is line count.

**The mass is in the rendering functions, and the field-group map never mentions them.** Measured:

| lines | function |
|---|---|
| **602** | `central_panel_ui` |
| 331 | `autoplay_controls_ui` |
| 299 | `frame_ui` |
| 280 | `stage_tab_bar_ui` |
| 274 | `specimen_source_ui` |
| 255 | `context_bar_ui` |
| 246 | `tour_panel_ui` |
| 244 | `source_map_ui` |

**Those eight are 2,531 lines — eight times what three iterations of state-struct moves
achieved.** The first three moves were correct as *mechanism* rehearsal on items that could not
fail interestingly; they were never going to reach the target.

### The coupling measurement — 2026-08-19, and it reorders the work again

**Distinct `self.<field>` accesses per function**, which is the real cost of extracting one:

| fields | lines | function |
|---|---|---|
| **43** | 602 | `central_panel_ui` |
| 36 | — | `drain_worker` |
| **32** | 299 | `frame_ui` |
| 25 | — | `diagnostic_snapshot` |
| 23 | — | `dispatch_hrw_link` |
| 14 | 280 | `stage_tab_bar_ui` |
| 13 | 255 | `context_bar_ui` |
| **7** | 274 | `specimen_source_ui` |
| **7** | 246 | `tour_panel_ui` |
| **6** | 331 | `autoplay_controls_ui` |
| **4** | 244 | `source_map_ui` |

**Size and coupling are nearly uncorrelated, and coupling is what decides the cost.**
`autoplay_controls_ui` is 331 lines and touches **6** fields; `frame_ui` is 299 lines and touches
**32**. Ordering by size would have started on one of the worst.

**Revised order, cheapest first:**

1. **`source_map_ui`** (245 lines, 4 fields) — the genuine easiest, and a real test of whether a
   rendering function can leave at all.

   **The four fields are already identified, so the next session starts at the edit:**

   ```text
   self.cached_equation_sheet   self.identifier_index
   self.tracked_identifier      self.viewport
   ```

   **Shape to aim for:** a free `pub(crate) fn source_map_ui(ui, …four refs…)` in
   `source_map.rs`, with `App::source_map_ui` reduced to a one-line delegate. **Check first
   whether `viewport` is mutated** — if it is, that parameter is `&mut` and the rest stay shared,
   which is still four parameters and still qualifies. **If it needs `&mut self`, stop**: the
   "what NOT to do" list rejects an extraction that moves a method behind a new name.
2. **`specimen_source_ui`** (274, 7) — ✅ **DONE 2026-08-19, on the second attempt**, after the
   revert below. `specimen_source.rs` (397), −331 lines. The finding is above; the history is
   kept because it is the evidence for *enumerate before editing*.

   **ATTEMPTED AND REVERTED 2026-08-19 first.** Four cascading problems, each invisible until the
   previous was fixed:

   - **`self.set_tracked_identifier(name)`** — a method, not a field, called once at the very end.
     Solved cleanly by returning `Option<String>` and letting `App` perform the follow, which is
     the pattern `model_list` already uses. **That part was right and is worth keeping.**
   - **Multiline `self\n    .field` accesses** that a single-line rewrite cannot see.
   - **`source` is mutated** (`source.scrolled_for = …`), so it needs `&mut`, discovered only
     after the field accesses were fixed.
   - **A local `let source = …` shadows the parameter**, so the rewrite silently retargeted field
     accesses at a `Option<&str>`. This is where it stopped: regex cannot distinguish a parameter
     from a shadowing local.

   **The lesson is about method, not difficulty.** `source_map_ui` extracted cleanly by script
   because its four fields were all simple single-line reads. **This one needs the body read and
   edited by hand** — and that is the real reason it costs hours: not the coupling number, but
   whether the accesses are mechanically rewritable. **Add that to the estimate for every
   remaining function.**
3. **`autoplay_controls_ui` + `tour_panel_ui`** (577, 6 and 7) — the tour panel, whose state
   already lives in `tour.rs`. ✅ **The first half shipped 2026-08-19** and proved the guess in
   that last clause: state that already lives in a struct costs one parameter, not one per field.

**`central_panel_ui` (43) and `frame_ui` (32) are last and may never qualify.** At 43 fields an
extraction is a signature with forty-three arguments or a `&mut App` parameter — which the plan's
"what NOT to do" list rejects as reducing nothing. **They shrink as their callees leave**, and
that is the only mechanism likely to help them.

**So the remaining plan is by RENDERING CONCERN, ordered by coupling:**

1. ✅ **`specimen_source_ui` + `source_map_ui`** (518) — one concern, the specimen's own text.
   **Both shipped 2026-08-19**, as two modules rather than one.
2. ✅ **`autoplay_controls_ui`** (331, 6) — **DONE 2026-08-19**, `tour_transport.rs` (458),
   **−355 lines**. The prediction in this line was right: *"the state to pass is a `&mut
   TourState` plus little else, which would be the cheapest signature yet."* It is four
   parameters, and the finding above explains why the field count did not show that in advance.

3. ⟶ **NEXT: `tour_panel_ui`** (246, 7) — the other half of the tour panel, and the direct
   caller of the function that just left. **Enumerate first**, per the checklist: which of the
   seven fields it touches are `self.tour` (the transport bar's eighteen were, and that decided
   its cost), which accesses are multiline, whether any parameter is mutated, whether any local
   shadows a parameter name.

   **Two things are already known and should not be rediscovered.** It calls
   `self.autoplay_controls_ui`, which is now a thin delegate — so the extracted function will
   either call `tour_transport::autoplay_controls_ui` directly and return the request upward, or
   the two merge into one module. **The second is likelier the right shape**: the transport bar
   and the prose beneath it are one pane, and `tour_transport.rs` at 458 lines has room. And it
   returns `Option<HrwLink>`, so the callback pattern is *already* how it reports — that part
   needs no design.

   **It also touches `self.split` and `self.commonmark_cache`**, which the transport bar did
   not. Check whether the `SplitState` configuration can stay in `App` with only the inner
   closure moving; if it cannot, this is a bigger job than the transport bar was.
4. **`central_panel_ui`** (602) — last, because it is the stage-routing hub and every other move
   shrinks what it has to route.

**And each needs a decision the state moves did not.** These take `&mut self` and reach across
`App`. Moving one behind a new name reduces nothing — the plan's "what NOT to do" list says so
directly. **Each extraction must first establish what state it actually touches**, which means
reading it, which means the estimate for these is *hours*, not the minutes the state moves took.

### A FOURTH trap, and it is Claude's own rule being broken repeatedly

**Do not write prose containing backticks through `node -e` or any shell string.** It happened
**four times on 2026-08-19** — in `worker.rs`, `tour.rs`, `reduction_view.rs` and this very
section, where three trap descriptions had every backticked term silently deleted while the shell
printed `command not found` for each one.

**`CLAUDE.md` forbids exactly this**, names three prior corruptions, and says to use the Edit
tool. The rule was read, recorded, quoted in a commit message the same day — and broken again
within the hour, because a one-liner *feels* cheaper than an Edit call.

**The tell is in the output, not the file:** lines like `` /usr/bin/bash: `app.rs`: No such file
or directory `` mean content was eaten. The file still compiles or renders, so nothing else
notices. **Read the shell's stderr after any generated write.**

### Three mechanical traps the loop hit — 2026-08-19

**None cost more than a build**, because the loop builds after each cut. Recorded so the next
iteration spends no time rediscovering them.

1. **`/tmp` is not the same directory for `node` and for bash.** A cut body written by node
   landed at `C:\tmp\` while bash looked in its own `/tmp` — and the `cat` that would have
   reassembled it failed *after* the items were already removed from `app.rs`. **Write scratch
   files inside the repo's temp dir, or pass absolute Windows paths to node.**
2. **A `sed` insert before a struct lands between its `#[derive]` and the item.** Exactly the
   attribute-orphaning trap `CLAUDE.md` records for tests. **Insert imports after the module doc
   comment, never above the first item.**
3. **Regenerate `architecture.md` BEFORE the slow gate.** It carries module line counts, so
   every move stales it and  fails ~300 seconds in.
4. **`cargo test -p hrw --lib` with default parallelism HANGS.** Observed twice on 2026-08-19,
   reproducibly: the test binary sits at ~1 GB with **frozen CPU time** after roughly 250 of the
   623 tests, and never returns. `-- --test-threads=1` runs the same 623 in **21 seconds**.
   **Not attributed to any change** — no baseline run was taken, and the extraction that day was
   a pure refactor of a rendering function. It is recorded because it cost this session two
   ten-minute waits, and because `CLAUDE.md`'s gate already passes `--test-threads=1`, so the
   documented workflow never meets it. **Always pass `--test-threads=1`**, including for the
   fast between-edits run that `Cargo.toml` documents without it.

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
