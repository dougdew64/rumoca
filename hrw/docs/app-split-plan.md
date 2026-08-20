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

  > **READ BOTH CLAUSES — the second one is not a footnote, and misreading it cost an argument
  > on 2026-08-20.** Claude cited the first sentence against moving `app.rs`'s test blocks out,
  > calling it *"the purest possible line-count move"*. **It is not**: naming *"5,613 lines a
  > session no longer has to hold to edit this file"* **is** trigger 2, spelled out, which is
  > exactly what the second clause admits. Doug: *"our rule which prevents extractions which are
  > justified only by line count does not apply. You have experienced problems when working with
  > `app.rs` … if we move the tests to a new file, we would be doing so to eliminate problems
  > which you have been experiencing."*
  >
  > **The rule bars line count as a JUSTIFICATION, not as a MECHANISM.** It exists to stop
  > splitting for tidiness, and its test is *"what does this stop a session from holding?"* — a
  > question a big mechanical move can answer as well as a clever seam can.
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

### THE SIZE NUMBER IS NOT THE RETURN, AND DOUG HAS RULED ON THAT — 2026-08-20

> *"This `app.rs` refactoring effort has been beneficial, regardless of the reduction in size of
> `app.rs`. You've identified and fixed bugs and you've identified and fixed testing gaps. Very
> good stuff."*

**This governs how a session reports its own iteration.** `app.rs` has gone 14,437 → 12,250 in
twenty-odd iterations, and a session scored on that number alone would read several of the best
ones as failures — the live-debug gate **added** 41 lines, the cache-lifetime split added 173,
and the wrong-model annotation fix was a **net zero**. All four are among the most valuable things
the loop has produced.

**What the effort has actually returned, so it is countable rather than asserted:**

| defects found *by extracting*, each shipped fixed | how it surfaced |
|---|---|
| the **stranded alias view** — a pane claiming "no alias eliminations" about a model with several | the extraction's first test |
| the **stranded `Animate` arm** — the index-reduction replay drawn under the Events tab | reading a dispatch chain as a column |
| the **navigated tree annotated from the wrong model** — a library class citing the specimen's source lines | moving one of two 100-line-apart copies |
| a **doc comment adopted by the wrong function**, three days old, describing a different signature | moving the item below it |
| a **`_ =>` wildcard inside a cluster a regression test already guarded** | deduplicating the six copies |
| a **must-fire guard that could not fire**, cited by four documents | establishing what the gate actually asserts |
| a **replay restarting because you passed through a report stage** — a rule nobody had designed | asking what one cache's lifetime was |
| two **expired comments** that had been silently false, one for five days | the seams that expired them |

**And the testing gaps, which are the half a line count cannot show at all.** Panes that could
previously be reached only by building an `App`, giving it a worker and driving a specimen to a
*failing* stage now have tests that run in hundredths of a second: `error_summary` (5),
`matrix_panes` (6), `equation_sheet_view` (6), `nav_view` (9), `stage_tabs`, `context_bar`,
`report_sub_view`, and the four ack verdicts split out of one. **Two claims about what could not
be tested were narrowed by measurement** — panes around the two unreachable painters, and
`egui_kittest`'s ability to open a context menu.

**So the honest scoring rule: an iteration reports what it found and what it made checkable
first, and the line count last.** The size target remains the experiment's *proxy* (§1), not its
purpose.

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
| 2026-08-19 | **`tour_prose_ui`** — the inner scroll area of `tour_panel_ui`, + `no_tour_ui` + a constant | **12,908** | `tour_panel.rs` (735, renamed from `tour_transport.rs`) |
| 2026-08-19 | **the tabs of `stage_tab_bar_ui`** — the span below the ▶ button, + `tab_label` + the row's teaching comment | **12,715** | `stage_tabs.rs` (493, of which 190 are tests) |
| 2026-08-19 | **the assembled state of `context_bar_ui`** + `background_ui` — *the seven-method one* | **12,519** | `context_bar.rs` (520, of which 205 are tests) |
| 2026-08-19 | **`generic_error_summary` + `structural_singular_summary`** — *the `self`-free pair* | **12,292** | `error_summary.rs` (440, of which 140 are tests) |
| 2026-08-19 | **`ContextBarState` + `PointedAt` + `PointKind` + `next_seq`** — *the state follows its pane* | **12,194** | `context_bar.rs` (649) |
| 2026-08-19 | **`equation_sheet_ui`** — *two accumulators collapsed into one report* | **12,008** | `equation_sheet_view.rs` (446, of which 208 are tests) |
| 2026-08-19 | **`report_sub_view_row_ui`** — *the pane whose only `App` method was a QUESTION* | **11,857** | `report_sub_view.rs` (541, of which 320 are tests) |
| 2026-08-19 | *(fix, not a move)* the stranded-alias defect the extraction exposed | 11,985 | `report_sub_view.rs` (650) |
| 2026-08-19 | **the live-debug prologue, six copies → one `live_debug_gate`** — *the first move that made the file BIGGER* | **12,026** | *(none — it cannot leave `app.rs`)* |
| 2026-08-20 | *(seam, not a move)* the ack path forwarded to `live_debug_poll` + `live_debug_gate_at` — *bought the order test; 8 lines of it are production* | 12,132 | *(none)* |
| 2026-08-20 | **the four compile replays → `CompileViewCaches`** — *a lifetime decision, and it found a second defect* | 12,305 | `compile_caches.rs` (101), `stage_caches.rs` 99 → 120 |
| 2026-08-20 | *(test split, not a move)* the four ack verdicts become four named tests — *zero production lines; it expired a second comment that had been a no-op for five days* | 12,356 | *(none)* |
| 2026-08-20 | **the spy-plot and incidence arms of `central_panel_ui`'s dispatch → `matrix_panes.rs`** — *the first cut INTO a router; the chain is now thirteen one-line arms* | **12,273** | `matrix_panes.rs` (451, of which 246 are tests) |
| 2026-08-20 | **the navigation branch of `central_panel_ui` → `nav_view.rs`** — *the router's OUTERMOST list, which no census row had ever counted* | **12,250** | `nav_view.rs` (388, of which 220 are tests) |
| 2026-08-20 | *(accuracy, not a move)* the navigated tree stops being annotated from the specimen — *`nav_view_ui` loses its `TreeOptions` parameter; net **zero** lines on `app.rs`* | 12,250 | `nav_view.rs` (483, of which 285 are tests) |

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

### `tour_prose_ui` → `tour_panel.rs` — 2026-08-19, and the seam is inside the function

**−244 lines, first attempt, no revert.** But the finding is not the number — it is that
**the function was never the unit.**

**`tour_panel_ui` rated 7 fields and looked expensive, and extracting it whole would have
been.** Its `self` census splits cleanly in two:

| what it touches | where |
|---|---|
| `poll_tour_file`, `autoplay_controls_ui`, `log_split`, `select_tour` — **four `App` methods** | the outer 37 lines |
| `self.split` — configure, `inner_width`, `observe` | the outer 37 lines |
| `self.tour` (21 accesses) and `self.commonmark_cache` (2) — **and nothing else** | the inner 209 |

**So the whole-function move needed a compound return** carrying three unrelated reports — a
[`TransportRequest`], a split-log line, and the switched-to tour — and it would have **deferred
Back, Play and Stop by a frame**, because `App::autoplay_controls_ui` performs them *inside* the
panel closure today. A behaviour change bought nothing.

**The inner scroll area needed no return value at all.** Two state groups, four parameters,
`-> ()`. The prose renders a document and records where it ended up; there is no decision in it
to report.

**THE RULE THIS ADDS, and it goes before the obstacle checklist rather than into it:**

> **Ask which contiguous region of the body calls no `App` method.** One grep answers it
> (`grep -o 'self\.[a-z_]*('`), and here it separated 209 cheap lines from 37 expensive ones.

**The previous two iterations each refined the *cost metric* — fields, then state groups. This
one says the metric is being applied to the wrong object.** A rendering function is not
homogeneous: `App` policy clusters at its edges, where the panel is opened and the presses are
answered, and the middle is usually pure rendering over one or two structs. **Cut the middle
out, and what is left is not a delegate — it is the shell that genuinely is the panel.**
`App::tour_panel_ui` is now 37 lines and every one of them is policy.

**The prediction of a third callback instance did not come true, and that is the point.** Two
extractions invented a report because they contained a decision the pane could not make; this
one was chosen *because* it contained none.

### The module was renamed, and four references is the whole cost

`tour_transport.rs` → **`tour_panel.rs`**: the bar and the prose beneath it are one pane, which
[`app-split-plan.md`](app-split-plan.md) predicted before either half moved. Four references
(`lib.rs`, a `use`, a call, a doc link) plus `git mv`. **Done in the same commit deliberately** —
a module whose name describes a third of its contents teaches a reader something false, and
`docs/architecture.md` regenerates the size table either way.

### A third instance of the error class the first build catches

`&mut self.commonmark_cache` rewrites to `&mut cache`, and `cache` is **already** `&mut T` —
`E0596`, twice, fixed by `&mut *cache`. Joins the `*deref` on a `!=` and the `return;` →
`return None;` from `specimen_source_ui`. **All three come from a reference parameter replacing
an owned field, and all three are found by the first `cargo build`** — which is the argument for
building after each item rather than for a longer checklist.

### The dedent warning has an exception, and it is checkable

The plan says never blanket-`sed` a moved body's indentation because it strips four spaces from
inside multi-line string literals. `no_tour_ui` has four such literals and was dedented anyway,
**safely** — every one of them uses a `\`-continuation, and Rust strips the newline *and all
leading whitespace* after `\`. **So the test is not "does it contain a multi-line literal" but
"is the continuation escaped".** `cargo fmt` still reindents everything else, so the dedent
bought only a readable intermediate file.

### The next candidate is NOT the one the coupling table names — measured 2026-08-19

Re-censused by the two metrics that have actually predicted cost — **state groups** and
**`App` methods called**:

| function | fields | `App` methods | the tell |
|---|---|---|---|
| `stage_tab_bar_ui` (280) | 12 | **2** — `open`, `start_simulation` | both are *presses*, so both are the callback pattern already proven twice |
| `context_bar_ui` (255) | 6 | **7** — `emit_context`, `navigate_to`, `jump_to_next_match`, `refresh_jump_matches`, `next_seq`, `background_ui`, `empty_context_hint` | fewer fields, far more policy |

**The coupling table ordered these 13 then 14 and put `context_bar_ui` first. That is backwards.**
Six fields wrapped around seven `App` methods is a pane made of policy; twelve fields answered by
two presses is a pane made of rendering — and several of its twelve (`sim_data`, `sim_error`,
`sim_running`; `model`, `model_list`) are clusters that a `&mut` struct would collapse the way
`self.tour` did.

**So: `stage_tab_bar_ui` next**, and apply the region rule first — find the span that calls
neither `self.open` nor `self.start_simulation` before deciding whether the whole function moves.

### The tabs left `stage_tab_bar_ui` — 2026-08-19, and the `App`-method COUNT is not the test

**−193 lines, first attempt, no revert.** `stage_tabs.rs` is 493 lines, 190 of them tests.

**The prediction above was half right and half wrong, and the wrong half is the finding.** It
was right that the region rule should be applied first. It was wrong that two `App` methods
made this a candidate for moving *whole* — **it has the fewest `App` methods of any function
left, and it still could not move whole.**

**The real test is not how many `App` methods a function calls. It is whether DEFERRING the
press changes what this frame draws.** Both of these fail it, and neither failure is visible in
a count:

- **`App::open`** sets `compiling`, clears the stage bundle and switches to the log view — and
  the tabs *below the switcher* read all three in the same frame. Reporting the press would
  draw one frame of the previous specimen's tabs, highlighted and enabled, over a specimen that
  is no longer loaded.
- **`App::start_simulation`** sets `sim_running`, and the spinner three lines later reads it.

**So a press is cheap to defer only when nothing downstream of it in the same function reads
what it wrote.** `specimen_source_ui`'s `set_tracked_identifier` and `tour_prose_ui`'s
`select_tour` were both the last thing their functions did, which is why the pattern looked
free. **Add the position of the call to the census, not just its existence** — a method at the
end of a body is a callback, a method in the middle is a barrier.

**The region rule then did the rest**: the 163 lines after the ▶ button call no `App` method,
and that is the tab row proper. What stays is the chrome that genuinely needs the application —
the Debug-mode specimen switcher, the Log button, the ▶ button, and the two status spinners —
at about 100 lines, down from 280.

**The third callback enum arrived**, as predicted after `autoplay_controls_ui` and not delivered
by `tour_prose_ui`. `Option<TabClick>`, `Stage` | `Simulation`. **Its shape is new**: the two
variants do not ask `App` for different *work* so much as for a different *amount* of it — both
leave the log view, only `Stage` asks for a capture. The row still owns the selection itself
(`&mut StageKind`), because selecting is what a tab row is.

**A mutation was moved out of the module and it is behaviour-identical, which is worth the
sentence.** The row used to clear `viewing_log` in two click handlers; it now takes the flag by
value and lets `App` clear it. That is only safe because **both writes happened after the only
read** — `stage_selected` is computed before any tab is drawn. Check that ordering before
demoting any `&mut` to a value; it is not a general licence.

### THE NEW MECHANICAL TRAP: a clipped widget is queryable but not clickable

**Cost one debug cycle, and the failure impersonated the thing under test.** The first version
of `the_simulation_tab_is_reported_separately_from_a_stage` built its harness without the
`ui.horizontal_wrapped` that both real call sites wrap the row in. The tabs stacked vertically,
Simulation fell below the viewport, and:

- `query_by_label_contains("Simulation")` **found it** — a clipped widget is still in the
  accessibility tree, the same property `CLAUDE.md` records for both scroll-area bugs.
- `.click()` **did nothing**, silently.
- The assertion that failed was `stage == Simulation`, which reads exactly like *"the row did
  not report the press"* — a defect in the code under test.

**So a widget harness must reproduce the caller's layout, not merely call the function.** Two
other traps met the same day, both trivial once seen: `StageKind` has **no `Default`**, so a
test-state struct cannot derive one (it is a position in a pipeline; there is no neutral
position), and the moved body's `self.stages` must be rewritten **before** `self.stage`, since
the second pattern is a prefix of the first.

### What the extraction bought that no earlier test could have

**The row was already covered** — `ui_tests` drives it through a real `App` and asserts that a
tab click selects the stage, leaves the log view and reaches the Context Bar. **Every one of
those assertions runs downstream of the row**, so they see only the consequences `App` chose to
apply, and the distinction that matters most is invisible from there: a Simulation click and a
stage click look identical on screen and differ only in a capture.

`the_simulation_tab_is_reported_separately_from_a_stage` asserts it in two lines against a
`StageBundle` and three bools — no worker, no channels, no compile. Its companion
`drawing_the_row_without_clicking_reports_nothing` is the non-vacuity guard the must-fire rule
asks for: without it, a row that reported a click every frame passes the other two.

**Both `App`-side halves of the wiring were revert-checked**, and each is caught by an existing
test: swapping the variant in `App`'s handler fails
`clicking_a_stage_tab_reaches_the_context_bar`, and the log-clearing is
`clicking_a_stage_tab_leaves_the_log_view`.

### `context_bar_ui`'s assembled state left — 2026-08-19, and six of the seven methods were free

**−196 lines, first attempt, no revert.** `context_bar.rs` is 520 lines, 205 of them tests.

**The seven-`App`-method count that put this last in the coupling table was almost entirely
noise, and the census that shows why is about *position*.** Applying the sharpened test from
`stage_tab_bar_ui` — *does anything after this call read what it wrote?* — sorted the seven in
one pass:

| the call | verdict |
|---|---|
| `refresh_jump_matches` | **the one barrier.** It rebuilds the match list the Following row reports two lines later. Stayed in `App`, called before the extracted function. |
| `jump_to_next_match`, `next_seq` ×2, `emit_context` ×2, `navigate_to` | **free.** All five sit in a trailing block *below* `ui.separator()` — below the last `ui` call in the whole function. |
| `background_ui` | a render helper; **moved with the pane**. |
| `empty_context_hint` | a render helper; **stayed**. See below. |

**THE TEST SHARPENS ONCE MORE: a press below the last `ui` call costs exactly zero frames to
defer.** `stage_tab_bar_ui` distinguished *end of body* (callback) from *mid-body* (barrier);
this one shows the end-of-body case is not merely cheap but **provably identical** — the same
statements run in the same order, one function boundary later, with nothing drawn in between.
Seven methods reduced to one obstacle because six of them were in that trailing block. **Look
for the trailing block first**: a rendering function that accumulates presses into locals and
acts on them below the last widget is already shaped for this cut.

**AND THE PARAMETER LIST DECIDES WHICH `&self` HELPER MOVES.** The previous iteration's note
said a `&self` render helper "can simply move with the pane". That is too permissive:

- **`background_ui` moved** — its three inputs (`model`, `selected.is_some()`, `stage`) were
  already needed by the pane, so it cost **zero** new parameters.
- **`empty_context_hint` stayed** — it reads `ui_mode`, `specimen_detail` and `viewing_log`,
  three pieces of state the bar otherwise never touches, purely to phrase one sentence. Moving
  it would have added three arguments that teach a reader nothing about what the bar *reports*,
  and would have broken three existing `App`-side tests that call it directly.

**So the rule is: a helper moves if its inputs are already in the signature, and stays if it
would widen it.** That decision is also what kept the empty-state branch in `App` — it is four
lines of rendering around that hint — so `App::context_bar_ui` is now the empty state, the one
barrier, and a `match` over four presses.

**The fourth callback instance, and the first to COLLAPSE accumulators rather than add one.**
The function carried five independent locals — `clear_point`, `clear_thread`, `jump_forward`,
`jump_back`, `go_to_class` — and acted on all of them below the rows. One
`Option<ContextBarPress>` replaces all five. **That is sound and not merely convenient**: every
one is set by a distinct `small_button` or `link`, and egui delivers a pointer press to a
single widget, so two could never be true in the same frame. The old shape could *express* it;
nothing could *produce* it. Say that in the module doc, because the collapse looks lossy.

**THE GREP CENSUS UNDERCOUNTS — it cannot see a multiline `self` access.** `self.identifier_index`
and `self.stages` both showed as **zero** in `grep -o 'self\.[a-z_]*'` because both are written
as `self\n    .field` by `rustfmt`. Two real state groups, invisible to the metric the coupling
table is built from. **The multiline access was already on the obstacle checklist as an editing
hazard; it is also a measurement hazard**, and every number in that table may be low by one or
two. `perl -0777 -ne` with `self\s*\.\s*(\w+)` is the honest count.

### The two mechanical costs, both in the tests rather than the extraction

**`get_all_by_label_contains` PANICS when nothing matches — it cannot express absence.** The
must-fire test for the pre-slot branch asserts that "declared at line" is *not* rendered, and
`.next().is_none()` never runs: the query panics first, with a full accessibility-tree dump that
reads like the widget was missing for some other reason. **Use `query_by_label_contains` for any
negative assertion**; it returns `Option`. Cost one test cycle.

**`..Default::default()` requires EVERY field visible, not just the ones being set.** Avoiding
`clippy::field_reassign_with_default` in a cross-module test widened all ten `ContextBarState`
fields to `pub(crate)`, where the pane itself reads only five. **That is a cost of the test, not
of the extraction**, and it is the argument for the follow-up below rather than a reason to
regret it.

### `generic_error_summary` left, and the `self`-free class is now EXHAUSTED — 2026-08-19

**−227 lines, first attempt, no revert, and the first extraction that required no design at
all.** `error_summary.rs` is 440 lines, 140 of them tests. `generic_error_summary` and its
Structural entry point `structural_singular_summary` both sat in `impl App` and never mentioned
`self` across 228 lines: no signature to establish, no callback to invent, no press to defer.
Three call sites changed from `Self::` to `crate::error_summary::`, and that was the whole edit.

**THE SWEEP IS THE FINDING, AND ITS RESULT IS "DO NOT RUN IT AGAIN."** One `awk` pass over
`impl App` for bodies containing no `self` returned five candidates, and this iteration
consumed the only ones worth moving:

| lines | fn | verdict |
|---|---|---|
| 220 | `generic_error_summary` | **moved** |
| 8 | `structural_singular_summary` | **moved** — its only caller, and one concern with it |
| 30 | `build_declaring_classes` | stays: it is `StageBundle` → `DefInfo` plumbing, not a pane |
| 20 | `structural_view_available_from_stage` | stays: sub-view policy, 20 lines |
| 3 | `note_says_singular` | stays |

**So the class held one item, not a supply.** The previous iteration's advice — *"sweep for that
whole class first"* — was worth taking exactly once, and the honest record is that **228 of the
281 `self`-free lines were a single function.** A future session should not re-run this pass
expecting a second harvest; the remaining three are under 30 lines each and belong where a
reader looks for them.

**The rule the five iterations before it were missing:** every one measured *coupling* and
sorted by it, which silently assumes coupling is non-zero. **Check for zero first** — it costs
one `awk` pass, and it found the cheapest 227 lines in the file after five iterations of
hunting for seams in methods that have real coupling.

### What the extraction bought, which is the part the line count does not show

**The summary was previously reachable only through a whole compile.** As a private associated
function of `App`, exercising it meant building an `App`, giving it a worker, and driving a
specimen to a *failing* stage — so no test had ever asserted what the pane renders, only that a
failure reached it. Against a free function taking `(ui, &Value, StageKind)`, the error object
is an argument, and five tests run in **0.02 s** with no worker and no compile.

**And one of them documents a correctness property that nothing held before.** The singularity
grid is **all-or-nothing**: the four counts are read as a tuple, so a missing `rank_deficiency`
withholds the equation, unknown and matched counts with it. That is `CLAUDE.md`'s
*nothing-may-be-invented* rule in a place nobody had written it down — three of four counts on
screen invite the reader to infer the fourth, and the inferred number would be HRW's rather than
the compiler's. `the_singularity_grid_needs_all_four_counts` now fails if that is ever
"improved" into four independent `if let`s.

**Each test carries its own non-vacuity guard in the same body** — the absence assertions are
paired with a presence assertion using the same query, so a query that finds nothing at all
fails rather than passing quietly. `query_by_label_contains` throughout, per the trap recorded
above: `get_all_by_label_contains` panics on no match and cannot express absence.

### `ContextBarState` followed its pane — 2026-08-19, and the estimate was low for a reason worth keeping

**−98 lines, first attempt, no revert, and the first extraction that produced ZERO build
errors.** `context_bar.rs` is 520 → 649.

**The plan said "~35 lines" and it was 98, because a type does not travel alone.** `PointedAt`
is the type of `ContextBarState::pointed_at` and `PointKind` is the type of `PointedAt::kind`,
so the cluster is three types, not one. **Estimate a type move by its field types, not by the
struct's own line count** — the same rule the plan already applies to functions (*"how many
separate places is this in?"*), pointed at data.

**What decided the cluster boundary is the DIRECTION of the module dependency.** After the pane
moved last iteration, `context_bar.rs` imported its own state back out of `app.rs`: `app` →
`context_bar` for the rendering, `context_bar` → `app` for the two types that rendering draws.
Rust permits the cycle and says nothing about it, so nothing would ever have failed — but a
reader asking *"where does the Context Bar live?"* got two answers. **That is the whole purchase
of this step**, and it is trigger 2, not trigger 3.

**AND IT BOUGHT NO TEST, WHICH IS RECORDED RATHER THAN DRESSED UP.** The plan's rule admits two
justifications — a test it buys, or what a session no longer has to hold — and every iteration
since `source_map_ui` has been able to claim the first. This one cannot, and the check that
proved it is worth repeating before writing any "this could not be tested before" sentence:

| the property | already asserted by |
|---|---|
| the shared counter makes `seq` and `track_seq` comparable for recency | `app.rs`, three assertions around `track_seq > after_point` |
| the jump cursor wraps, and resets across a stage switch | `jumping_cycles_within_the_current_stage_and_resets_across_stages` |

**Both run on `App::test_default()` — no worker, no compile.** `error_summary`'s five new tests
were bought because that function was reachable *only* through a failing compile; nothing here
was. **Grep for the property before claiming the extraction buys it**, because the claim is
about the old code and is therefore checkable in advance.

**ONLY `next_seq` MOVED WITH THE STATE, AND THE FILTER IS THE SAME PARAMETER-LIST RULE.** Four
`App` methods operate on this state; three stayed:

| method | verdict |
|---|---|
| `next_seq` (2 lines) | **moved** — touches `context_seq` and nothing else. Zero new parameters. |
| `refresh_jump_matches` | stays — reads `tracked_identifier`, `stage`, and `current_stage().value` |
| `jump_to_next_match` | stays — same two, plus it clears `viewing_log` |
| `emit_context` and the capture paths | stay — they *build* a `PointedAt` from `App`-wide state |

**So the helper rule that sorted `background_ui` from `empty_context_hint` sorts methods onto a
moved struct too:** it moves if its inputs are already there, and stays if it would widen the
signature. Three of the four would have needed three arguments to carry state this module never
otherwise touches — and moving them would have traded a working `App`-level test for a wider
seam, which is the trade the plan's "what NOT to do" list rejects.

**The mechanical note: this is the first move with no `cargo build` errors at all**, because a
type move has none of the four obstacles on the checklist — no `self` accesses to rewrite, no
multiline `self\n .field`, no mutated parameter, no shadowing local. The only edits outside the
cut were one `use` line in each direction and six `self.next_seq()` → `self.context.next_seq()`
rewrites, all in plain statement position. **The obstacle checklist is about function bodies;
for a type move the whole risk is `#[derive]` orphaning** (trap 2), which is why both types were
cut with their attributes by Edit rather than by line range.

### `equation_sheet_ui` — the trailing-block cut, generalised — 2026-08-19

**−186 lines, first attempt, no revert.** `app.rs` 12,194 → 12,008; `equation_sheet_view.rs` is
446 lines, **208 of them tests**.

**The region rule and the deferral test agreed for once, and the shape is now named.** Everything
between the `ScrollArea` and the closing brace — 190 lines — calls no `App` method. The one method
it does call, `set_tracked_identifier`, sits *below the last `ui` call*, in the trailing block that
`context_bar_ui` taught us to look for first. So the cut is provably identical: the same statements
in the same order, one function boundary later, with nothing drawn in between.

**Two accumulators became one report, and this is the second instance of that collapse.**
`clicked_row: Option<Option<usize>>` and `clicked_variable: Option<String>` became
`Option<SheetClick>` — `Equation(Option<usize>)` | `Variable(String)`. Sound for the same reason
`ContextBarPress` was: each is set by a distinct widget, and egui delivers a press to one widget
per frame. **The `Option<Option<usize>>` is worth noticing on its own** — the outer layer meant
"was there a click" and the inner meant "is the row now highlighted", two questions in one type.
The enum separates them, so `Equation(None)` reads as *un-highlight* rather than as *no press*.

**`has_incidence` stayed in `App`, and that is the parameter-list rule deciding a computation
rather than a helper.** It reads `self.stage_views.incidence` and `self.stages` — two groups this
pane never otherwise touches — to produce one `bool`. Passing the `bool` costs one parameter;
moving the computation would have cost two state groups. Final signature: five parameters
(`ui`, `Option<&EquationSheet>`, `bool`, `Option<&str>`, `Option<usize>`).

**THE TEST THIS BOUGHT WAS WRITTEN DOWN IN ADVANCE, BY A COMMENT THAT HAD GIVEN UP.** `ui_tests.rs`
carried a 20-line note ending: *"The sheet's real behaviour — that it renders the equations it
holds — needs a populated `EquationSheet`, so it belongs with the tests that compile a specimen
behind `slow-tests`."* That sentence **is** the extraction's justification, recorded weeks before
anyone was looking for one, and it stopped being true the moment the sheet became an argument
rather than a field. Six tests now run in **0.04 s** against a hand-built `EquationSheet`.

**So the sweep to run is not another `awk` pass over `impl App` — it is a grep over the test files
for deferrals.** `error_summary` was found by asking *"what has zero coupling?"*; this one was
already labelled *"cannot be tested from here"* by a past session that hit the wall and wrote a
note instead of a test. **A comment explaining why something is untestable is a coupling
measurement someone else already took.** The note has been corrected in place rather than deleted,
because the correction is the finding.

**What the six tests hold**, none of it previously asserted:

| the property | why it can break |
|---|---|
| the family heading totals *every* group in the family (3, not 2) | a heading that reported its own group's length under-counts what `connect` contributed — the exact misreading Doug flagged on 2026-08-13 |
| the heading is drawn **once** above the contiguous run | one per group re-implies the separateness the heading exists to deny |
| no incidence matrix ⇒ no "click an equation" hint | offering a highlight that has nowhere to land |
| the row click **toggles**: clicking the highlighted row reports `Equation(None)` | the pane's whole click contract, previously observable only from inside `App` |
| a variable name reports itself | reverse tracking (#37) |

**AND THE CLIPPED-WIDGET TRAP FIRED AGAIN, IN A NEW PLACE AND WITH A SECOND CAUSE.** Both click
tests failed at first, and the two causes had to be fixed separately:

1. **The harness must be sized like a pane** — `1200×900`. Probed deliberately by shrinking it to
   `200×120`: the four *query* tests still passed and both *click* tests failed. That is
   `stage_tabs`'s trap exactly — a clipped widget stays in the accessibility tree and refuses
   clicks — reproduced here by a `ScrollArea` instead of by a missing `horizontal_wrapped`. **The
   cause is not the layout wrapper; it is any container that clips.**
2. **A returned press must be accumulated, not assigned.** `*out = pane_ui(...)` every frame throws
   the press away on the frame after it lands, and `run_steps(2)` guarantees there is one. Use
   `build_ui_state` with `if click.is_some() { state.reported = click }`, as `stage_tabs` does.

**Both failures read as "the pane did not report the press."** Any pane extracted with a callback
return needs both fixes before its click test means anything.

### `report_sub_view_row_ui` left, and its `App` method was a QUESTION, not a press — 2026-08-19

**−151 lines, zero build errors on the first attempt**, and the second such run after
`ContextBarState`. The obstacle checklist came back clean: four state groups, no multiline
`self\n    .field` access, no mutated parameter, no local shadowing a parameter.

**The new shape, and it is the inverse of the callback pattern.** Four extractions invented a
report because the pane contained a *decision* it could not make. This one contained a **question**
it could not answer: `structural_view_available` is consulted four times, mid-render, to decide
whether a tab exists at all. A press can be deferred to the caller; **a question cannot — the
answer is needed before the widget is drawn.** So the caller answers all four *before* the call
and passes a `TabAvailability` struct:

```rust
let available = report_sub_view::TabAvailability {
    summary:  self.structural_view_available(StructuralView::Summary),
    animate:  self.structural_view_available(StructuralView::Animate),
    aliases:  self.structural_view_available(StructuralView::AliasAnim),
    spy_plot: self.structural_view_available(StructuralView::SpyPlot),
};
```

**Moving the predicate instead was considered and rejected, on the parameter-list rule plus one
more.** Its inputs are *nearly* all in the pane's signature — only `frames.index_reduction` is not
— so the rule alone would have allowed it. What decided it is that `structural_view_available` and
`structural_view_available_from_stage` are **cited by name from `DECISIONS.md`,
`fidelity-plan.md` and `worker.rs`** as the one predicate the tab bar and the link guard share.
Moving them costs four documents and buys the row nothing it does not get from four booleans.
**So the rule gains a clause: a helper also stays when documents outside the code name where it
lives.**

**And the field count over-counted, for a new reason: count the NARROWEST BORROW.** The census
rated this pane **5 fields**, one of which was `viewport` — but every access is
`self.viewport.structural`, so the parameter is `&mut StructuralView`, not `&mut Viewport`. A
state group that is only touched through *one* of its own fields costs one parameter of that
field's type, and the signature then says exactly what the pane may move. This is the mirror image
of the `autoplay_controls_ui` finding (a whole struct costs one parameter): **the unit is the
narrowest borrow that compiles, which may be smaller than a group or larger than a field.**

**What it bought: ten tests in 0.03 s, on a pane that had never had one.** Reaching it before
meant building an `App`, giving it a worker, compiling a specimen and driving it to Structural or
Index Reduction with the right singularity — so nothing had ever asserted which banner appears,
which tabs a singular system hides, or where a stage change lands. Two are must-fire in the strict
sense and both were **probed by reverting**: deleting the singular banner fails
`a_singular_structural_stage_shows_the_singular_banner`, and ignoring `reset_for`'s return value
fails `redrawing_the_row_does_not_re_apply_the_default_sub_view`.

**`default_sub_view_for` is the piece worth having separately, and it turned up a live defect.**
The stage-change default was three branches buried in a `&mut self` render body; as a pure
`fn(is_index_reduction, is_singular, current) -> StructuralView` it is four assertions, and it
exposed an asymmetry: **only Summary and Animate were redirected to the spy plot**, while
Incidence and Tree carried over. That was first written up here as a finding — *"the asymmetry
nobody had written down"* — and Doug asked whether it was a finding or a bug. **Checking it made
it a bug**, fixed the same day; see the box below.

#### AND THE ASYMMETRY WAS A DEFECT: `AliasAnim` was missing from the redirect list

**Three views are Index-Reduction-only — Summary, Animate and AliasAnim — and only two were
redirected.** The Aliases tab was added after that condition was written, and nothing compared the
two lists.

**The symptom, verified with a harness probe before anything was changed:** on a non-singular model
with aliases (`RcCircuit`, `TwoLoops`, `ProportionalLoop`, `MixedLoop` all qualify — checked
against their committed traces), choose **Aliases ▶** on Index Reduction, then click the
**Structural** tab. `viewport.structural` stays `AliasAnim`, Structural offers no such tab, so
**the row draws with nothing highlighted** — and the panel dispatch, which checked only
`report_ready`, rendered the alias view against the *Structural* report. That report carries no
eliminations, so the pane said *"(no alias eliminations in this report)"* about a model with
several.

**That is the absence-filled class, not a cosmetic one.** A reader standing on Structural would
conclude `RcCircuit` has no aliases. Nothing in the corpus checks a pane's claim against a
*different stage's* report, which is why no checker here could have caught it.

**Two fixes, and they close different things:**

1. **`default_sub_view_for` redirects all three.** The regression guard asserts them **as a set**
   (`no_index_reduction_only_view_survives_a_move_to_structural`), so a fourth such view added
   without the redirect fails there rather than on screen — the exact mistake, encoded. A second
   test walks the reader's route through the widget, because the pure function is only half the
   path: the row must actually *call* it. Both fail against the old code.
2. **`App::clamp_structural_sub_view` checks the RESULT of every door.** Three places write
   `viewport.structural` — the tab row, the stage-change default, the `hrw://` link guard — and
   each had its own guard while **nothing checked the outcome**. Adding a door without its guard
   is precisely what happened. The clamp falls back to Tree (the one view every report stage
   offers) **and notifies**, because after fix 1 there is no known path to it: a silent correction
   would hide the regression it exists to catch. Three tests, including the non-vacuity one that
   a clamp-everything implementation would fail.

**The seam is what found it.** As three branches inside a render body reachable only through a
worker and a compiled specimen, the missing case had nowhere to show up. This is the strongest
instance so far of the plan's own claim that extraction buys testability — the test it bought
found a defect in the code it was extracted from, on the same day.

#### THE DEFECT THIS FOUND: a doc comment had been adopted by the wrong function for three days

**`report_sub_view_row_ui` had no doc comment.** Its nineteen lines of documentation sat above
`apply_pending_view_and_seek`, which was inserted between the doc block and the `fn` line by
`545b4aaa` on **2026-08-16**. Rust merges contiguous `///` lines into one doc, so that method
carried a doc whose first three paragraphs describe a *different* function — including
*"`&mut self` is right here for the same reason as the tab row"*, about a method that takes no
`ui`. The row itself was undocumented and nothing said so.

**This is the third target of one mechanical trap, and `CLAUDE.md` names only two.** The rule
reads *"INSERT A TEST AFTER A FUNCTION'S CLOSING BRACE, never before its `fn` line"*, and trap 2
here names `#[derive]`. **The trap is not about tests or derives — it is about anything that binds
downward:** an attribute, a `#[test]`, and a doc comment all attach to the item that follows, so
an item inserted above any of them steals it. `doc_citations::no_function_has_two_test_attributes`
catches the `#[test]` case and **nothing catches this one**, because a merged doc block is
well-formed Rust that rustdoc renders happily.

**There is an exact detector, and it is worth knowing even though it is not built here.** The
orphaning necessarily leaves the *original* item with **zero** doc comment — so "every method in
`impl App` is documented" would have failed by name on 2026-08-16. **Measured: 19 methods in
`app.rs` are undocumented**, most of them one-line `test_*` accessors, so the checker costs about
an hour of one-line docs and then holds permanently. Filed rather than built, under the
one-extraction-per-session rule. <!-- unbuilt: doc_citations::every_app_method_is_documented -->

### THE LIVE-DEBUG DEDUPLICATION IS DONE, AND IT MADE `app.rs` BIGGER — 2026-08-19

**+41 lines, and that is the finding rather than an embarrassment.** The plan estimated
*"~30 lines saved per pane, so ~150–200 lines"*. The measured result:

| | added | removed | net |
|---|---|---|---|
| production code | 51 | 113 | **−62** |
| test code | 32 | 0 | +32 |
| comments | 85 | 16 | **+69** |
| **file** | **168** | **129** | **+41** |

**Six copies of an eighteen-line prologue (113 lines) became one thirty-line method, and the file
grew.** The duplication cost nothing to explain — a reader who understood one copy understood all
six, and nobody documents a copy. An abstraction must be explained *once, thoroughly*, at the
place it is introduced: why the cache arrives as a `fn` pointer, why the step order is
load-bearing, what deliberately stayed with the callers.

**So a deduplication cannot be scored on `app.rs`'s line count, because the duplicate and its
replacement live in the same file.** Every earlier iteration moved code *out*, where the count
measured something real. **`live_debug_gate` cannot move out**: it calls `is_arming`,
`has_live_debug_data` and `live_debug_poll`, all of which read four to six `App` fields apiece, so
the parameter-list rule keeps them — and a caller of three `App` methods is an `App` method. **The
count and the goal came apart here for the first time**, and the plan's target ("no module over
~1,500 lines") does not name what this iteration bought. What it bought is that the protocol has
one implementation: a seventh view gets the three answers in the right order or does not compile.

**Read the trigger-2 rule carefully before the next one of these.** It forbids an extraction whose
*only* justification is line count. It does not promise that a justified change reduces the count,
and this one raises it.

### THE SIX WERE NOT IDENTICAL, AND THE PLAN WAS RIGHT TO SAY SO FIRST

The plan required *"verify the six really are identical before assuming it — a difference would be
either a bug or a reason."* Four differences, each judged, because a difference described neutrally
has not thereby been judged:

- **The prologue is identical in all six.** Verified line by line. That part deduplicated.
- **`pre_lowering_anim` is cached OUTSIDE `StageViewCaches`** — `self.cached_pre_lowering_anim`,
  where the other five live in `self.stage_views`. **A behavioural asymmetry, and probably a
  defect, but not certainly** — recorded rather than fixed. `StageViewCaches` is dropped on every
  stage change; this field is dropped only on a new compile. Yet `reduction_anim` and
  `connection_anim` are built from `self.frames`, which is compile-scoped exactly like
  pre-lowering's, so **three views with one lifetime are given two.** The visible consequence:
  leave the Index Reduction stage and return, and the reduction animation restarts at frame 0
  while `pre()` lowering resumes where it was — and a *live* session on those two would be dropped
  mid-run. Which behaviour is intended is a real question, so it needs deciding, not patching.
- **`connection_anim_ui` never releases the armed breakpoint when the live start fails.** The other
  five run `if live.is_none() { remove_live_trace_breakpoint(); … }`. **Principled, not a defect:**
  `ConnectionAnimation::start_live` returns the animation rather than an `Option`, because the
  worker owns the run and there is no local failure to detect. **The real gap it exposes is
  elsewhere** — nothing releases that breakpoint if the *worker* never reaches connection
  expansion, and the session-end safety net that used to cover this was deliberately removed
  (`docs/ideas.md` #74).
- **`request_fit()` after a live start, only in Matching and Tarjan.** **Principled** — those two
  are the only views with a camera.

**And the epilogue could not be deduplicated for a borrow-checker reason worth recording.** The
four-line breakpoint release sits inside `if let Some(Some(mat)) = &self.stage_views.incidence`,
which holds an immutable borrow of `self` across the whole block, so a `&mut self` helper cannot be
called there — where a *field* assignment can, being disjoint. **A repeated block inside a borrow
of `self` is not extractable into a method without restructuring its caller**, and that restructure
differs per view, which is the duplication back in another shape. A free function taking
`&mut self.live_breakpoint_armed` would compile, but it enforces nothing that the five already do.

### THE `_ =>` ARM WAS THE SAME SILENT-OMISSION SHAPE A TEST ALREADY GUARDS — fixed

**`has_live_debug_data` ended in `_ => matches!(&self.stage_views.incidence, …)`.** A seventh
variant would have compiled cleanly and been told to look for an incidence matrix it may have no
use for. **That is exactly the shape `every_live_debug_variant_is_recognised_while_arming` exists
for** — its doc describes the `pre()`-lowering Debug button doing nothing because a hand-written
list of matching pairs never grew a fourth entry — **and it was still present one function over,
in the same cluster the test guards.**

**A test that iterates `ALL` proves the machinery handles today's variants; it cannot make tomorrow's
loud if the code has a wildcard.** Naming `Matching | Tarjan` makes the next view a compile error.
**Grep the cluster a regression test covers for `_ =>` before trusting the test.**

### THE DOC-COMMENT TRAP HAS A THIRD AND FOURTH CAUSE — and the proposed detector catches neither

The trap recorded on 2026-08-19 was *"an item inserted above a doc comment steals it"*, with the
proposed detector *"the orphaned item ends up with ZERO doc comment."* **Two more instances were
found in this cluster, and both victims ended up with too MANY doc lines, not zero:**

- **Split.** `has_live_debug_data` carried four lines describing `live_debug_lifecycle` — *"Returns
  `SpawnLive` when the ack handshake completes"*, about a function that returns `bool`. Nothing was
  inserted: `live_debug_lifecycle` was **split into four methods**, and its doc stayed above
  whichever piece landed first. That function no longer exists.
- **Rewrite.** `connection_anim_ui` carried two doc paragraphs, and **the first was false**:
  *"Recorded only — see `connection_anim`'s module note on why there is no Debug button yet."*
  There has been a Debug button for some time; the replacement paragraph was written *above* the
  old one instead of replacing it, and Rust merged them. **A reader of the rendered doc is told
  there is no Debug button and then told how it works.**

**So the detector is not "zero doc comments" — it is a doc block that contradicts its item's
signature or itself.** The zero-doc check would have found neither of these, and both are worse
than an undocumented function: an undocumented function teaches nothing, and these teach something
false. Cheap partial detectors that would have fired: a doc block containing two `///`-paragraphs
that each read like an opening summary, and a doc naming a return type the signature does not have.

### FOUR DOCS CITE `live_debug_lifecycle`, WHICH DOES NOT EXIST — filed, not fixed

`matching_anim.rs:226`, `matching_anim.rs:863`, `playback.rs:114` and `tarjan_anim.rs:731` all name
it, and all four describe **a breakpoint-release safety net that `live_debug_poll`'s own doc says was
deliberately removed** (*"there is no session-end safety net… With the release gone…"*, `ideas.md`
#74). `playback.rs` explains that `Playback::recorded` starts `live_done` at `true` *"so
`live_debug_lifecycle` can release a breakpoint left armed by a session that never started"* —
nothing reads it for that any more.

**Not fixed here, and the reason is the accuracy rule rather than budget:** correcting them means
first establishing what, if anything, now releases a breakpoint left by a session that never
started. That is a question about behaviour, not a rename, and answering it wrongly would replace
one false statement with another. **A dangling symbol reference is a cheap fix; a dangling
*mechanism* description is not.**

### ✅ DONE 2026-08-20 — and the count was five, the question was already answered, and the guard was vacuous

**Three corrections to the box above, in ascending order of value.** Full account in
`DECISIONS.md`, 2026-08-20.

- **FIVE sites, not four.** `playback.rs`'s test doc — *"which is what lets the lifecycle release a
  breakpoint armed for a session that never began"* — describes the same deleted mechanism without
  naming the function, so the symbol grep that found the other four walked straight past it.
  **A stale-mechanism sweep cannot be a symbol grep**, because prose describing a thing outlives
  prose naming it.
- **The question needed verifying, not establishing.** It was answered in the repository twice
  already: `docs/ideas.md` #74 names the three releases it deliberately left ungated, and the
  comment on `App::live_breakpoint_armed` lists the same three. Confirmed against the call sites —
  five spawn-failure releases in the `*_anim_ui` panes, two specimen-change releases, and
  `release_live_breakpoint_at_exit`, plus the manual `HRW: Clear Armed Breakpoints`. **None reads a
  `LiveState`**, which is the substance: the deleted net asked the animation *"is anything
  running?"* and every survivor asks the app *"was I told a breakpoint exists?"*. **Grep for the
  answer before booking a session to find it** — the plan had priced this as research.
- **The must-fire guard those docs cited could not fire.** Flipping `Playback::recorded`'s
  `live_done` to `false` and running the fast suite fails **exactly one** test, and it is not
  either of the two named for the job: `matching_anim` and `tarjan_anim`'s
  `recorded_animation_reports_no_live_session` both stay green, because `live_state` returns `Idle`
  from `is_live()` being false and never consults the flag. **A test named for a field it cannot
  see** — written at the view layer for a defect two types down, with an abstraction between them
  that short-circuits. Both are kept with corrected docs (they hold a real property one layer up),
  and `playback::tests::a_recorded_animation_reports_no_running_session` is named as the guard that
  actually fires.

### THE TEST THE GATE WANTED IS BLOCKED BY A SEAM THAT ALREADY EXISTS ONE LAYER DOWN

**The property worth asserting is the ORDER** — `is_arming` must be read *before* `live_debug_poll`,
because the poll clears `pending_live_debug` on the frame the ack lands, so a reordering would drop
the "Arming…" badge on exactly that frame. Reaching that frame in a test needs the poll's
`SpawnLive` branch, which calls `bridge::check_breakpoint_ack()` — the live path, which **deletes**
the ack file in the real `.hrw-bridge` directory.

**`bridge::check_breakpoint_ack_at(path)` exists precisely so tests need not touch it**, and
`live_debug_poll` does not take that seam. So the test that shipped asserts the *composition*
(no variant offers Debug without its data, across `ALL`, touching no bridge file) rather than the
order. **Giving `live_debug_poll` the same path parameter its callee already has is the next
cheap thing in this cluster**, and it would buy the order test.

### ✅ DONE 2026-08-20 — the seam was forwarded, and the order test exists

**`live_debug_poll` takes `ack_path: &Path`, and `live_debug_gate` gained an `_at` sibling**
holding the body while the default-path wrapper keeps the six paint-path callers unchanged — the
same two-function shape `bridge::check_breakpoint_ack` / `check_breakpoint_ack_at` already uses,
one layer up. **+113 lines, of which 8 are production logic**; the rest is the test and the
reasoning.

**`the_arming_badge_survives_the_frame_its_ack_lands` asserts `arming` and `spawn_live` true
TOGETHER**, which is the whole property: the poll consumes `pending_live_debug` on the frame the
ack lands, so reading `is_arming` after it reports `false` on exactly the frame the badge is still
wanted — and the live animation does not exist yet, because the caller builds it from
`spawn_live`. One frame of a view mid-handshake claiming nothing is happening.

**Must-fire verified by swapping the two lines**, and it fails on the badge assertion by name.
**A second perturbation was run by accident and is worth recording**: leaving *both* polls in
place (the swap applied without removing the original) failed on `spawn_live` instead — the first
poll consumed the ack and the second returned `None`. So the test also catches double-polling,
which is a plausible edit for someone "clarifying" this function.

**THE SEAM WAS ONE LAYER DOWN AND ONE LAYER UP AT THE SAME TIME, AND ONLY ONE HALF WAS NOTICED.**
The section above spotted that `check_breakpoint_ack_at` existed and `live_debug_poll` threw it
away. What it did not spot is that **forwarding the parameter to the poll is not enough** — the
property under test belongs to the *gate*, which composes three methods, so the seam has to reach
the gate too. That is the second `_at` function, and it was invisible until the test was written.
**When a missing seam is diagnosed, ask which function the ASSERTION is about, not which function
touches the resource** — they were one apart here.

**AND IT LEFT A CHEAPER FOLLOW-UP THAN THE ONE IT CAME FROM.**
`a_timed_out_arm_claims_nothing_and_says_so` covers **four** distinct paths in one `#[test]`, and
its own doc explains why: *"Both paths share the single `.hrw-bridge/breakpoint-ack.json` … as
separate tests they would race for that file."* **That sentence stopped being true with this
change** — they can each take their own path now and split into four named tests, so a failure
names the path instead of a line number. It is the same shape as the `equation_sheet_ui` finding:
**a comment explaining why a test cannot be split is a measurement someone already took, and it
expires silently when the seam it describes is added.** Not done here, under the one-item rule;
the four call sites were left pointing at the real constant so this change asserts nothing new
about them.

### ✅ DONE 2026-08-20 — the four verdicts are four tests, and a second comment expired with them

**+51 lines, zero production lines.** `an_armed_verdict_starts_the_run_and_stays_quiet`,
`a_disabled_breakpoint_spawns_and_names_the_cause`,
`a_stale_bridge_reply_claims_nothing_and_names_its_fix` and
`a_timed_out_arm_claims_nothing_and_says_so` — one per verdict of
[`bridge::check_breakpoint_ack_at`], each against its own `std::env::temp_dir()` ack file, in the
shape `the_arming_badge_survives_the_frame_its_ack_lands` already used.

**The split is what made the must-fire evidence legible, and that is the whole purchase.** Three
perturbations, and each one lands on a *named* subset instead of a line number:

| perturbation | fails |
|---|---|
| `live_breakpoint_armed = ack.is_armed()` → `= true` (#71's fiction) | disabled, stale, timeout — **not** armed |
| `BreakpointAck::Armed => {}` → `=> self.notify("armed")` | armed only |
| the `Unreportable` notice stops saying `npm run build` | stale only |

The old single test could only report *"app.rs:8057"*. **A must-fire perturbation's value is in
which tests it does NOT break**, and one function covering four paths throws that away — the first
perturbation above is the interesting one precisely because `Armed` stays green under it.

**A SECOND EXPIRED LINE CAME OUT WITH THE FIRST, AND IT HAD BEEN A NO-OP LONGER.** The old body
carried `app.prewarm = Prewarm::Done;` under *"Keep the pre-warm out of the way; it competes for
the same ack file."* The line arrived with the test on **2026-08-07** (`1e2fcb23`) and stopped
meaning anything on **2026-08-15**, when `3037fca1` pinned `prewarm: Prewarm::Done` in
`App::test_with_sender` to stop harnesses arming a real breakpoint in Doug's editor — so for five
days it had been assigning a value to itself, under a comment describing a competition the
constructor had already ended. **Two expiries, two causes**: the ack-file sentence expired when a
seam was added *here*, the pre-warm line when a default changed *elsewhere*. The second kind is
worse, because nothing in this file changed on the day it stopped being true. `#[test]` bodies get
no dead-store lint, so both survived a `cargo clippy --all-targets` every session since.

**And a doc that had been correct became a citation to fix**: `ideas.md` #71 item 4 states the
one-test rationale as fact. Corrected in place rather than deleted, per the standing rule, since
what expired it is the finding.

### ⟶ NEXT — and the remaining `_ui` census says the job has changed shape

**Measured 2026-08-19, after `equation_sheet_ui`.** Every rendering method still on `App`, with its
line count, its distinct `self.<field>` accesses, and the `App` methods it calls:

| lines | fields | function | `App` methods called |
|---|---|---|---|
| ~~154~~ | ~~5~~ | ~~`report_sub_view_row_ui`~~ | **done 2026-08-19** → `report_sub_view.rs` |
| 93 | 14 | `tarjan_anim_ui` | the live-debug four, `structural_frames_for_stage`, `structural_unavailable`, `notify` |
| 86 | 12 | `matching_anim_ui` | the live-debug four, `structural_frames_for_stage`, `structural_unavailable` |
| 80 | 7 | `menu_bar_ui` | `notify` |
| 71 | 11 | `tearing_anim_ui` | the live-debug four, + `tearing_dae` |
| 61 | 10 | `connection_anim_ui` | the live-debug four |
| 52 | 8 | `reduction_anim_ui` | the live-debug four |
| 51 | 8 | `pre_lowering_anim_ui` | the live-debug four |
| 18 | 3 | `alias_anim_ui` | — |
| 18 | 2 | `ic_plan_anim_ui` | — |
| **~620** | **43** | `central_panel_ui` | the router |
| **~483** | **32** | `frame_ui` | the router |

**"The live-debug four" is `is_arming`, `has_live_debug_data`, `live_debug_poll` and
`start_live_debug`, and SIX PANES CALL ALL FOUR.** That is the finding, and it means the eight
`*_anim_ui` bodies are **not panes waiting to be extracted**. Their rendering already left — each
ends in a single `anim.ui(ui, arming, debug_enabled)` call into an existing `*_anim.rs` module.
What is left in `app.rs` is the **live-debug handshake**, written out six times with the
`PendingLiveDebug` variant, the `stage_views` field and the animation constructor swapped:

```text
is_arming → derive LiveState → has_live_debug_data → live_debug_poll → SpawnLive?
  → construct live anim (or clear the breakpoint) → fall back to captured frames
  → draw → if the button was clicked, start_live_debug
```

**So the next move is deduplication, not extraction, and it must be justified as such.** The
plan's rule forbids an extraction whose only justification is line count; collapsing six copies of
one handshake is justified by something else — **the six are supposed to be the same protocol, and
nothing enforces that they are.** A `live_debug` module owning the handshake (parameterised by
variant, `stage_views` accessor and constructor) would make a divergence a compile error instead of
a shrug. Estimate ~30 lines saved per pane, so ~150–200 lines, plus the two that call no `App`
method at all. **Verify the six really are identical before assuming it** — a difference would be
either a bug or a reason one of them is genuinely different, and both are worth knowing.

> **DONE 2026-08-19 — `App::live_debug_gate`, and every prediction in this paragraph was wrong
> except the justification.** The six were **not** identical (four differences, judged in the box
> above). The estimate of 150–200 lines saved was **+41 lines added**, because the handshake
> **cannot leave `app.rs`** — a `live_debug` module was the plan's guess, and the parameter-list
> rule forbids it: the three methods the gate calls read four to six `App` fields apiece. **The
> one thing that held is the justification**, which is why it is worth doing anyway: the protocol
> now has one implementation, and a seventh view gets its three answers in the right order or does
> not compile. `alias_anim_ui` and `ic_plan_anim_ui` — the two with no live debug at all — were
> left alone; they are 18 lines each and have nothing to share.

**`report_sub_view_row_ui` was that cheapest single pane and is DONE** — see the box above. It was
*"the last one shaped like the seven already done"*, and it was: zero build errors, four state
groups, ten tests. **With it gone, nothing shaped like it remains**, which is what makes the
live-debug deduplication the next move rather than a next extraction.

**`menu_bar_ui` (80, 7, one `notify`) is small enough that moving it buys little**, and `notify` is
the `App`-wide toast channel rather than a policy decision — likely a callback return, but at 80
lines the trigger-2 justification is thin. Do it only if it falls out of something else.

**`frame_ui` and `central_panel_ui` are still last, and still shrinking as their callees leave.**
Both *grew* in line count while `app.rs` shrank: they are the routers, and routing survives every
extraction.

### ⟶ NEXT, after the live-debug gate — the census is spent and the routers are what is left

**Every `_ui` method in the census above is now either extracted or judged not worth extracting**,
which means the supply of *panes* is gone. What remains in `app.rs` at 12,026 lines is the two
routers (~1,100 lines between them) and everything that is not a pane at all.

**Three candidates, and the recommendation is the third:**

1. **`central_panel_ui` (~620 lines, 43 fields) or `frame_ui` (~483, 32).** The mass, and the only
   things left that could reach the target. **But a router's coupling is not incidental** — it is
   43 fields *because* its job is to decide which pane runs, and every pane's state is a candidate.
   Neither will move whole. The cut is inside, the way `tour_prose_ui` and the `stage_tab_bar_ui`
   tabs were cut, and finding it is a whole session's work with no guarantee it ends green. **Do
   not start one on a session that has already spent context.**
2. **`menu_bar_ui` (80 lines, 7 fields, one `notify`).** Cheap, and the trigger-2 justification is
   thin at 80 lines. Do it only if it falls out of something else.
3. **The three follow-ups this cluster left, none of which is an extraction.** Recommended next,
   because each is small, each is bounded, and two of them are *accuracy* items — which outrank
   the line count outright:
   - ~~**Give `live_debug_poll` the `path` parameter `check_breakpoint_ack_at` already has.**~~
     ✅ **DONE 2026-08-20** — and it needed a second `_at` on the *gate*, because the property is
     the gate's. Box above. It left a cheaper successor: **split
     `a_timed_out_arm_claims_nothing_and_says_so` into its four paths**, now that they need not
     share one ack file.
   - ~~**Decide `pre_lowering_anim`'s cache lifetime**, since three views built from
     `self.frames` currently get two different ones. A question first, an edit second.~~
     ✅ **DONE 2026-08-20** — it was **four** views, not three, and the question was Doug's to
     answer. Box below.
   - ~~**The four `live_debug_lifecycle` citations**, which describe a removed mechanism. Needs the
     behaviour established before the prose is rewritten.~~ ✅ **DONE 2026-08-20** — **five** sites,
     not four; the question was already answered twice in the repository rather than needing
     research; and the must-fire guard two of them cited turned out to be **vacuous**. Box above.

**THE CLUSTER IS NOW EMPTY, so the next session starts a router** — `central_panel_ui` or
`frame_ui`, per option 1 above, on a session that has spent nothing.

> ### ✅ STARTED 2026-08-20 — and the router's seam was an ASYMMETRY AMONG ITS ARMS, not a region
>
> **`central_panel_ui` 640 → 552, `app.rs` 12,356 → 12,273.** Box below. The recommendation
> above said *"the cut is inside, the way `tour_prose_ui` was cut"* — a **contiguous region**
> that calls no `App` method. That is not what was found, and the difference is the finding.
>
> ### ✅ CONTINUED 2026-08-20 — `central_panel_ui` 552 → 484, and the branch taken was on nobody's list
>
> **The navigation branch → `nav_view.rs`.** `app.rs` 12,273 → **12,250**, which is the smallest
> reduction of any extraction so far and is not what the move rests on — see the box below for
> the eight tests and the open question it surfaced.
   - ~~**Split `a_timed_out_arm_claims_nothing_and_says_so` into its four paths** — the successor
     the ack-path seam left, and the cheapest thing on this list.~~ ✅ **DONE 2026-08-20** — four
     named tests, and the perturbation table is the purchase. Box above.

**The rhythm decision the plan asked for is now forced.** The loop's cheap supply is exhausted in
both directions: no leaf types, no panes. The next *extraction* is a router, and a router is the
"spend a whole session on one function" option — so it wants a fresh session, and the items
above are what fits in a shared one.

### THE ROUTER'S OUTERMOST LIST HAS TWO MEMBERS, AND EVERY CENSUS COUNTED ONLY ONE — 2026-08-20, `nav_view.rs`

**`central_panel_ui` 552 → 484; `app.rs` 12,273 → 12,250; the new module is 388 lines, 220 of
them tests.** The unit of work was the **`else` of `if self.nav.is_empty()`** — the router's
outermost branch, 73 lines, calling **zero `App` methods**.

**IT WAS NOT ON ANY LIST, AND THAT IS THE FINDING.** The `_ui` census, the coupling table and both
"⟶ NEXT" boxes enumerate what is inside the `nav.is_empty()` *if*: the sub-view row block, the
thirteen dispatch arms, the default artifact pane. **The `else` is a sibling of all of them and was
never a row anywhere.** The previous box's rule — *a router is a list, and a list's defect is the
member that does not look like the others* — was applied one level too deep: **the outermost `if`
is itself a two-member list**, and its second member is a whole pane about a different IR.

**So the rule gains a step: find the OUTERMOST list first, then descend.** The dispatch chain is a
list of thirteen; the block that chooses between the stage view and the navigation view is a list
of two, and it is the one a reader meets first.

**THE LINE COUNT IS THE WEAKEST RESULT HERE — −23 — AND IT IS SUPPOSED TO BE.** The branch is 73
lines, and 42 came back as `App::specimen_tree_options` and its doc. Scored on `app.rs`'s size
this barely registers; the plan's rule forbids resting on that number anyway, and the two
justifications are:

- **Eight tests in 0.08 s on a pane that had none and could not have had any.** Reaching it before
  meant building an `App`, giving it a worker, and pushing a `NavEntry` onto the go-to-definition
  stack — which is the precondition that failed *four times* while testing `CompileViewCaches`
  two boxes above. The crumb's composition, both buttons, the spinner's naming, the error line and
  the jump suppression are now assertions rather than reading.
- **The jump suppression became a property of the pane instead of a literal.** `jump_to: None` and
  `highlight: None` were two lines in a `TreeOptions` literal in `app.rs`, correct because whoever
  wrote them was paying attention. They are now applied inside `nav_view_ui` over whatever the
  caller passes, so a caller that hands it a live stage jump target gets the same answer.

**AND THE EXTRACTION EXPOSED A DUPLICATE THAT HAD BEEN INVISIBLE BECAUSE ITS TWO HALVES WERE 100
LINES APART.** The stage tree and the navigated tree each built a seven-field `TreeOptions`
literal, and **the five model-knowledge fields were identical down to a verbatim seven-line
comment.** Neither copy is near the other, so no column-read and no region scan could see it; it
took moving one of them. They are now `App::specimen_tree_options`, and each tree adds only what
it addresses.

**THE SHARED HELPER ASSERTS THAT THE FIVE FIELDS BELONG IN BOTH TREES, AND THEY DO NOT.** Per the
rule this plan already carries (*when an extraction exposes an asymmetry, the next sentence must
say whether it is principled or a defect*): **a defect. Ruled on by Doug, 2026-08-20 — the
navigated tree is annotated from the class or not at all, and today it can only be "not at all".**

The argument that blanks `jump_to` for the navigated tree — *a library class is a different IR, so
an address computed against a stage means nothing here* — **applies unchanged to three of the five
fields that are not blanked:**

| field | what it is | on a navigated class |
|---|---|---|
| `path_lines` | *stage* node path → source line | a path string that collides resolves to the **specimen's** DAE line |
| `variable_lines` | variable name → declaring line **in the specimen** | `R` in `Resistor` gets the specimen's `R` |
| `declaring_classes` | variable name → declaring class, **of the specimen** | same collision, feeding "Go to definition" |
| `known_variables` | the specimen's variables | decides what is *trackable*; a name that exists in both is offered |
| `tracked` | the identifier being followed | arguably right — following it across an IR boundary is the point |

**Nobody has reported this**, and it may be unreachable in practice: `path_lines` is `None`
outside Dae/Flatten, and a library class's paths are shaped differently from a DAE's. **That is
exactly the profile of the alias defect and the stranded `Animate` arm** — a wrong answer nothing
on screen admits to, in a pane visited rarely.

**The failure mode is presence substituted, not absence filled.** The gutter would say *"declared
at line 41"* over a row of the Resistor, naming a line of the specimen. Same class as the arm that
drew the index-reduction replay under the Events tab.

#### ✅ DONE 2026-08-20 — and the five were the WHOLE STRUCT, so the PARAMETER went instead

**`nav_view_ui` no longer takes a `TreeOptions` at all.** The box below records why that is the
same ruling implemented one step further, what it cost, and the two tests it bought.

**The original shape of the work, kept because the correction is the finding:**

> **`nav_view_ui` blanks all five, joining `jump_to` and `highlight` in the suppression it
> already applies.**

**THE AUTHORITY IS `docs/identity-and-provenance.md`, and the interesting part is that these five
do NOT break its written rule.** That document forbids *substring* search deciding identity and
prescribes exact equality modulo one `der(…)`; all five use exact equality and are compliant as
written. **What they step outside is the rule's unstated precondition — that both sides are the
same model.** Until "Go to definition" existed, they always were. **Exact equality across two
namespaces is a collision wearing identity's clothes**, and nothing flags it because the rule it
violates was never written down. *(Add that precondition to that document as part of this work; it
is the durable half.)*

**THE CONFIRMING DETAIL IS WHAT THE FIX DOES NOT REMOVE.** `def_index` is per-`NavEntry` — the
class's own DefId table, resolved structurally by the worker — and it is **not** one of the five,
so "Go to definition" keeps working *through DefIds* while the name-matched shortcuts go. The
structural route that document prescribes survives untouched. That is the rule working rather than
a coincidence, and it is the strongest single argument for this shape of fix.

**`tracked` is the one judgement call in the five, and it goes with the others.** The tracked
identifier is a *flat* name (`resistor.R`), so no key inside the `Resistor` class equals it and the
highlight rarely fires at all; when it does fire it is a bare-name collision. Blanked for
consistency, and recorded here as a call rather than a consequence.

**What it costs, stated so the trade is visible:** the navigated tree loses its underlines, its
follow offers and its "declared at line N" links entirely. **Absence stated rather than filled**,
which is the trade this repository makes everywhere else.

**And the rule is "annotate from the class, or not at all" — blanking is the correct answer NOW,
not the destination.** Nothing indexes an MSL class's own variables, declaring positions or source
lines, so the class-derived versions do not exist to substitute. If the navigated tree should ever
be annotated, they must be **built from the class**; do not re-derive them from the specimen.

**Shape of the work**, so the next session starts at the edit:

- Extend the `TreeOptions` literal inside `nav_view_ui` to blank `tracked`, `known_variables`,
  `declaring_classes`, `variable_lines` and `path_lines` beside the two already there. The
  suppression is already a property of the pane, so this is the same construct widened.
- **The test is named `a_navigated_class_is_not_annotated_from_the_specimen`** — spelled exactly
  that, because the `unbuilt:` tag above resolves against it and a misspelled target is silently
  permanent. `Nav::one`'s fixture already nests a leaf; give the harness a `known_variables`
  containing the leaf's value and assert the row is **not** offered as trackable.
  `query_by_label_contains`, never `get_all_by_label_contains`, for the negative.
- Must-fire it the way `a_jump_target_is_not_honoured_against_a_navigated_class` was: restore one
  field and confirm exactly that test fails.
- **Then delete the `unbuilt:` tag on this section**, or `claims_of_absence_are_still_true` fails —
  which is the mechanism doing its job.

### THE FIVE FIELDS WERE THE WHOLE STRUCT, SO THE PARAMETER WENT — done 2026-08-20

**`app.rs` 12,250 → 12,250; `nav_view.rs` 388 → 483, of which 285 are tests.** The file is
**exactly unchanged in length**: the call site lost the `self.specimen_tree_options()` argument
and the method's doc gained one line. **Not scored on `app.rs`'s line count** — this is an
accuracy fix, and the plan's rule already says an accuracy or testability item is paid for *in*
`app.rs` and cannot be measured there. **A net of zero is the clearest instance of that rule the
loop has produced**, and it is worth having on the record: a session scored on the size number
would read this as a wasted iteration.

**THE PLANNED EDIT WAS FIVE `None`s, AND COUNTING THE STRUCT CHANGED IT.** `TreeOptions` has
**seven** fields; `jump_to` and `highlight` were already blanked here. Five plus two is all of
them — so the queued edit was, without anyone noticing, *"ignore this parameter entirely"*.
**A parameter that is wholly ignored is a lie in the signature**, so `nav_view_ui` stopped taking
one and hands `tree_ui` a `TreeOptions::default()`.

**And the argument that settles it is not tidiness — it is what happens when `TreeOptions` gains
an eighth field.** The blanking literal ended in `..opts`, so a new field would flow straight
through to the navigated tree and re-open exactly this defect, silently. **Shape B fails open on
a future field; the missing parameter fails closed.** Same class as the `_ =>` arm two boxes up:
a construct that silently accepts tomorrow's variant.

**THE COST IS REAL AND IS A DELETED TEST.**
`a_jump_target_is_not_honoured_against_a_navigated_class` — must-fire-verified two boxes above —
**is gone, because it can no longer be written**: it worked by passing a stage's jump target in,
and there is no longer a parameter to pass it through. **A property that moved from a test to the
type system takes its test with it**, and the honest way to record that is to say so rather than
to leave a test that can only be made to fail by re-adding a parameter.

**Two tests replace it, and they are on the four annotations that DO still have a route in.**

- **`a_navigated_class_is_not_annotated_from_the_specimen`** right-clicks a leaf and reads the
  row menu. **The context menu is the only surface these five reach** — no annotation changes a
  label, so `known_variables`, `variable_lines` and `declaring_classes` are invisible to a query
  until a right-click opens *"🔎 Follow R"*, *"📄 Show R in the Modelica source"* and *"↪ Go to …"*.
- **`a_navigated_node_is_not_given_the_specimens_source_line`** covers `path_lines`, which takes a
  **different arm of `node_ui`**: it is keyed by node *path*, so it appears on a collapsible
  header, not a leaf.

**`egui_kittest` CAN DRIVE A CONTEXT MENU — `click_secondary()`, and nothing here had used it.**
Nine `.context_menu(` call sites across the codebase and not one test had ever opened one. It
works with no ceremony: right-click, `run_steps(2)`, and the items are ordinary queryable labels.
**That is a whole class of assertion this project had been treating as unreachable**, and it is
the same correction shape as the `matrix_panes` narrowing — *a surface nobody had tried*, filed
under what cannot be tested.

**THE PERTURBATION FOUND THAT THE THREE MENU ITEMS ARE NOT INDEPENDENT.** All three are gated on
`trackable_name`, which returns `None` without `known_variables` — so restoring `variable_lines`
or `declaring_classes` alone changes **nothing on screen**, and those two assertions can only
fire in company. `known_variables` is the master switch. Recorded in the test's doc, because a
reader would otherwise assume three independent guards.

**Must-fire, verified:** a literal setting `known_variables` fails the first test **on its Follow
assertion** and leaves the other seven green; `path_lines` fails the second **on its source-line
assertion**. Both preconditions (*the right-click opened a menu at all*) held in every run — which
is what stops these negatives passing because nothing was drawn.

**`tracked` has no test and cannot have one here**: it is a painted fill behind the row, leaving
no accessibility node. It rests on the signature, which is now the only thing it needs.

**THE FIXTURE HAD TO CHANGE, AND THE OLD ONE WOULD HAVE PASSED VACUOUSLY.** `Nav::one`'s IR was
`{"outer": {"inner_leaf": 42}}` — and the plan's own recipe said *"give the harness a
`known_variables` containing the leaf's value"*. **`42` is a number, and `trackable_name` requires
a string**, so that test could never have failed however the options were set. The IR now carries
`name: "R"` at the top level: a string, under a non-prose key, at a depth the tree actually opens.
**Three separate preconditions, none of them visible from the plan's description** — which is the
*"read the body before scripting"* rule applying to test fixtures as much as to extractions.

**AND THE CALLER'S DOC WAS A QUESTION THAT IS NOW ANSWERED.** `App::specimen_tree_options`
carried a section headed *"One method because the question has one answer, not because the answer
is known"*, explaining that the five were an open question for the navigated tree and that one
method made ruling on it one edit. **That is exactly what happened, so the paragraph had to go**
— it now records that the method had two callers for a day and that the second one was the defect.
**A doc that describes a pending decision expires the moment the decision lands**, and nothing
links the two.

## ⟶ THE NEXT TWO STEPS — decided 2026-08-20, do them in this order after a `/clear`

**Doug's direction: `Continue the app.rs split` should land on these two, in order, and then go
back to the routers.** They are one unit of work each under the stopping rule, so **step 1, then
`/clear`, then step 2** unless step 1 proves as small as it looks.

**Why these two and why now.** `app.rs` is 12,250 lines of which **5,613 are test code** — every
line from 6,638 to the end. Moving them out halves what a session must hold to edit this file,
which is trigger 2 stated exactly, and it changes no behaviour. **It is also the best-shaped
probe the experiment has:** the outcome metric is noisy (`CLAUDE.md` names two confounds — the
Opus 4.6 → Opus 5 change and Claude's own verbosity), and **a single −5,613 step is readable
through noise that would swallow twenty −150 extractions.** A discontinuity can be seen; a drift
cannot. The model has been stable since, which is the condition the confound note requires.

**`worker.rs` remains the control and is not touched** (§4). That is what makes this an
experiment rather than a campaign.

### Step 1 — make `arch_doc::module_sizes()` recurse, and key rows by RELATIVE PATH

**Do this FIRST, in its own commit, because step 2 makes a generated document silently wrong
without it.** `module_sizes()` reads `src/` with `read_dir`, filters on `extension == "rs"`, and
has **no `is_dir` branch** — so `app/tests.rs` (new, under `src/`) would simply not exist to it, while
`architecture.md` goes on printing *"Every file under `src/`, including the test-only ones"*.
**5,613 lines absent from a generated table that claims completeness.**

**Its own doc comment already asks for this fix**, which is the tell that it is right:

> *"Scanned, not listed. A hard-coded list would let a new module be silently absent from the
> table, and absence leaves no gap where the missing thing was."*

**It has the failure mode it was written to prevent.** It just needs a subdirectory to exist
before it can bite, and nothing in `src/` has ever had one.

**`MIN_MODULES` cannot catch it, and that is worth seeing.** The floor is a *minimum count of
rows*, and `app.rs` still exists after the move — the count does not drop, it fails to rise.
**A non-vacuity guard that cannot fire on the change being made**, the same shape as the
`recorded_animation_reports_no_live_session` finding.

**THE ROW KEY IS THE REAL DESIGN DECISION, AND IT IS NOT "ADD RECURSION".** `ModuleSize.file` is
built from `path.file_name()` — a **bare** file name. Recursing without changing that keys the new
module as `` `tests.rs` ``: ambiguous on sight, and it **collides outright** the moment a second
module gains a `tests.rs` submodule, which is the obvious next thing to happen if step 2 works.
**Key by the path relative to `src/`** (`` `app/tests.rs` ``) so the column is unique by
construction and reads as a location.

- The table is sorted largest-first then alphabetically *"so the ordering is total and the file is
  byte-stable"* — check that the tiebreak now sorts on the new key.
- **The test to add is the one that would have caught this**: a module in a subdirectory appears
  in the table, keyed by its relative path. Must-fire by reverting the recursion.
- **Two freebies while you are in this function**, both stale comments rather than defects:
  `MIN_MODULES`'s comment says *"30 against 38 today"* and there are **55** `.rs` files in `src/`;
  and `module_sizes_are_scanned_and_ordered` asserts `app.lines > 5_000` with the message
  *"`app.rs` is a five-figure file"* — **which step 2 makes false** (6,637 is four figures). Fix
  the message in step 2's commit, or the perturbation it describes stops matching the code.

### Step 2 — move `app.rs`'s five `cfg(test)` blocks to `app/tests.rs` (new, under `src/`)

**The split is a clean tail, not a carve.** Verified 2026-08-20: every item at column 0 after line
6,638 is `#[cfg(test)]` or the item one attributes. **Five blocks**, at 6638, 7069, 12050, 12123
and 12172:

| block | what it is |
|---|---|
| `#[cfg(test)] impl App` | the `pub(crate)` test-only accessors `ui_tests.rs` reaches |
| `mod tests` | the bulk, ~4,980 lines |
| `mod tests_incidence_row_link` | |
| `mod tests_tour_in_diagnostics` | |
| `mod tests_tour_back` | |

**The mechanism needs no `#[path]` and no `mod.rs`.** Rust 2018 lets a file-module own a
subdirectory: `src/app.rs` keeps `#[cfg(test)] mod tests;` and the body goes to
`app/tests.rs` (new, under `src/`). Three facts that make this safe, each checked rather than assumed:

- **`super` still means `app`**, so `use super::*` is unchanged.
- **A child module sees its parent's private items**, which is the whole reason these tests can
  touch `App`'s private fields today. That does not weaken; it is the same relationship.
- **An inherent `impl App` is legal in any module of the same crate**, so the accessor block moves
  with the rest and its `pub(crate)` methods stay visible to `ui_tests.rs` (a *sibling*, which is
  why they are `pub(crate)` and not private in the first place).

**What is checked and what is not, so neither is assumed:**

- **`doc_citations::rust_sources()` DOES recurse** (skipping `target`, `node_modules`, `vendor`,
  `.git`), so `no_function_has_two_test_attributes` and every other source checker keeps its
  coverage. **Stated because the two corpora disagree** — `arch_doc`'s does not recurse, and
  assuming they behave alike is how one of them silently covers less.
- **`arch_doc::app_field_groups()` is unaffected**: it reads `src/app.rs` and splits on
  `"\npub struct App {"`. The struct stays.
- **Consolidate the five blocks into one `mod tests` or keep five?** Keep five, moved verbatim.
  A move plus a merge is two changes, and only one of them can be verified by the suite going
  green unchanged.

#### A PLAN CANNOT CURRENTLY NAME A FILE IT INTENDS TO CREATE — found writing this section

**`doc_citations::every_documented_source_path_exists` rejected this very page.** Spelling the new
module as a `src/`-prefixed path made it a *citation*, and a citation must resolve — so the two
sentences describing step 2 failed the fast suite. **The checker is right**: it guards against
rotted references, and it cannot tell a forward reference from a stale one.

**The repository already has the vocabulary for "this does not exist yet" and the two mechanisms
do not compose.** An `<!-- unbuilt: hrw/src/… -->` tag would be the honest form —
`still_absent` handles a path target, and it would fail the moment the file appeared, forcing the
tag's removal. **But the tag's own text contains the path**, so `citations()` extracts it and the
path checker fires on the tag itself. Tagging makes it worse, not better.

**Worked around here by naming the file without a `src/` prefix**, with its location in the prose
beside it. That is accurate — the file genuinely does not exist — but it is a workaround, and the
next plan that names a file before creating it will hit the same wall.

**The fix, when someone is next in `doc_citations.rs`:** exempt a cited path that carries an
`unbuilt:` tag from `every_documented_source_path_exists`, and let
`claims_of_absence_are_still_true` own it instead. **Forward references become expressible and
self-expiring** rather than unspellable. Not done here — it is a third piece of work in a session
whose unit was already spent, and it belongs to the checker, not to this plan.

**Record it in the progress table as the experiment's STEP CHANGE, not as an extraction** —
`app.rs` 12,250 → ~6,637 with nothing refactored and no behaviour touched. **A session reading
that row later must not mistake it for 5,613 lines of seam work.**

**Then continue with the routers**, which is where the remaining named work is: the sub-view row
block (~125), the default artifact pane (~152, the largest single block left), and `frame_ui`'s
Specimen left panel (~113).

---

### A ROUTER'S SEAM IS AN ASYMMETRY AMONG ITS ARMS — 2026-08-20, `matrix_panes.rs`

**−83 net on `app.rs`; `central_panel_ui` 640 → 552; the new module is 451 lines, 246 of them
tests.** The unit of work was **two arms of a thirteen-arm dispatch chain**, not a region.

**THE SEAM WAS FOUND BY READING THE CHAIN AS A COLUMN — the same check that caught the stranded
`Animate` arm, run for a different purpose.** Eleven arms were a single delegation
(`self.tarjan_anim_ui(ui, ir_split)`); **two carried their whole pane inline**, 35 and 86 lines.
That is a seam a region scan cannot see, because the odd arms are *not contiguous* — the spy plot
and incidence arms sit at the top of the chain with eleven one-liners below them, and any
region-shaped cut would have taken the delegations with them.

**So the router rule is: a router is a LIST, and a list's defect is a member that does not look
like the others.** `tour_prose_ui`'s rule (*"which contiguous region calls no `App` method"*) is
right for a **body**; a dispatch chain has no interesting regions, only members. Both arms called
**zero** `App` methods, which the region rule would have reported as one 121-line region only if
the intervening eleven arms had not existed.

**AND THE UNIFORMITY IS THE REAL PURCHASE, not the 83 lines.** The column-read check that found
the `report_ready` omission now works **on sight**: thirteen arms, thirteen single lines, and a
missing guard is a difference in a short line rather than something to be found by grepping past
two long bodies. The line count is the weakest justification available here and is not the one
this move rests on.

**`before: Option<MatrixPane>` REPLACED A BOOLEAN AND A CACHE, and that is a contract rather than
a tidy-up.** The old code carried `ir_split` and touched `stage_views.before_incidence`
separately, so *"the split is on"* and *"the Before pane has somewhere to draw"* were two facts
that could disagree. As an `Option`, the Before pane exists exactly when the split does.

**The spy plot deliberately did NOT get that shape**, and refusing to make the two signatures
match is the accurate choice: it has no Before pane at all, so its `bool` asks a different
question — *do I owe the reader an explanation for an empty left half?* — which is exactly what
the "Spy-plot unavailable" notice answers. **Two panes that look symmetrical are not, and a
signature that hid that would have been the tidier lie.**

| perturbation | fails | stays green |
|---|---|---|
| swap the two `get_or_insert_with` report sources | `the_before_pane_reads_the_before_half_of_the_report`, `a_missing_before_half_is_reported_rather_than_substituted` | the other four — **including `the_split_labels_both_halves`** |
| `get_or_insert_with` → `insert` (rebuild every frame) | `a_built_matrix_is_not_rebuilt_on_the_next_frame` | the other five |

**THE SECOND COLUMN IS WHY THE DIMENSION ASSERTION EXISTS.** `the_split_labels_both_halves`
survives the swap, because both headings still render over both matrices — **the swap is
invisible to every check about what is on screen.** A Before/After exchange puts the *reduced*
system under "Before (raw DAE)": well-formed, correctly labelled, and the exact inverse of what
Index Reduction teaches. Only `n_eq × n_var` catches it, and only because the fixture gives the
two halves different sizes on purpose.

**"NOT QUERYABLE" WAS A CLAIM ABOUT THE PIXELS AND HAD BEEN READ AS A CLAIM ABOUT THE PANE.**
`ui_tests.rs` lists the incidence matrix and spy plot in its *"not queryable"* column, and
`CLAUDE.md` names them as the two surfaces `egui_kittest` cannot reach. **True of the `Painter`
output; false of everything around it** — captions, split headings and four absence notices are
ordinary labels, and the caches are fields a test reads after the frame. **Six tests in 0.02 s on
a surface recorded as untestable**, and nothing had to change in either painter to get them.
Same shape as the 2026-08-12 scroll-area correction: *a null result taken at one level was
generalised into a property of the whole thing.*

**A FIFTH CAUSE OF THE DOC-COMMENT TRAP, found while reading the region: DELETION.**
`FrameIntent::canvas_capture` carried *"Copied out of `self` because the stage-tree block holds an
immutable"* — **a sentence that does not finish** — as its opening summary. `71d0dcbf`
(2026-08-04) deleted the `expand_trackable` field and only the *second* line of its two-line doc;
Rust merged the survivor downward. **Sixteen days, and the four earlier causes were insertion,
split, rewrite and adoption — all of which leave grammatical prose.** This one does not, which
makes it the first instance with a cheap mechanical tell: *a doc block whose first sentence has no
terminator.* Fixed in place, with the account kept in the comment.

### THE CACHE LIFETIME WAS THE WRONG QUESTION ASKED ABOUT THE RIGHT FIELD — 2026-08-20

**`CompileViewCaches` (`compile_caches.rs`, 101 lines) now owns four replays**, and `app.rs` grew
by 173: +51 for one behavioural test, +58 for a second defect's fix and guard, the rest doc. Scored
on line count this is another loss, and per the rule established by the live-debug deduplication
that is the wrong scoreboard — an accuracy item is paid for *in* `app.rs`.

**The plan asked "what lifetime should `pre_lowering_anim` have?" and the answer was that it
already had the right one and three of its siblings did not.** `reduction_anim`,
`connection_anim` and `ic_plan_anim` sat in `StageViewCaches`, whose own doc promises *"views
derived from a stage's report, all valid for exactly one stage"* — **false of all three**, and
checkable in one pass: the eight that stay read `stages.get(self.stage)` or branch on
`self.stage`, and the three that left never mention it. Each of the four is shown on exactly one
stage, so its input cannot vary with the stage.

**`ic_plan_anim` was the one nobody had counted.** The plan said *three views built from
`self.frames`*, which is the right instinct pointed at the wrong attribute: `ic_plan_anim` is
built from a **report**, just not the *current* stage's — it reads `stages.initialization`
unconditionally. **The membership test is "does the input depend on `self.stage`?", not "where
does the input come from?"** — and only the first is checkable by reading the build site.

**AND THE BEHAVIOUR WAS NEVER A DESIGN, WHICH IS WHAT MADE THE DECISION EASY.**
`StageViewCaches::reset_for` is called from **one place**, `report_sub_view_row_ui`, which draws
only on Structural and Index Reduction. So `built_for` never held any other stage, and the rule in
force was not *"a replay restarts when you come back to it"* but ***"a replay restarts if you
happened to pass through a report stage in between."*** Events → Flatten → Events dropped nothing;
Flatten → Structural → Flatten dropped the connection replay. **Nobody designs that**, and saying
so is what turned a question about intent into a question about which accident to keep. Doug chose
per-compile for all four.

**Present the evidence that a behaviour is unintended, not just the two options.** The question
put to Doug carried the single call site and the round-trip asymmetry, and that is why it took one
exchange.

**THE FIRST TEST ASSERTED THE REFACTOR RATHER THAN THE BEHAVIOUR, and it looked fine.** It set the
four cache fields, called `reset_for`, and asserted they survived — which **cannot fail**, because
after the split they are in a different struct. Its doc comment even claimed it would catch
someone adding `compile_views.invalidate_all()` beside the `reset_for` call; it would not, since
it never runs that code. **The tell is that a test's setup and its assertion touch the same struct
the change just separated.** The shipped test paints `frame_ui`, walks the IC plan to block 2,
switches to Structural and back, and asserts `(2, 3)` — must-fire verified by restoring the old
clearing, which fails with `(0, 3)`.

**Four wrong guesses before that test ran, and the fourth probe found the real bug.** The
precondition *"the IC plan replay is on screen"* failed, and the causes were: no `selected`
specimen (early return), a pushed `NavEntry` (`nav` is the **go-to-definition stack**, so
non-empty means the pane shows a drilled-into class — the guard reads backwards until you know
that), and finally a missing `report_ready` on one dispatch arm. **A failing precondition is a
finding, not an obstacle** — the pull was to keep adjusting the fixture until it passed, and doing
that would have hidden the box below.

### A STRANDED SUB-VIEW WAS DRAWING THE WRONG PHASE'S ANIMATION — 2026-08-20, fixed

**One arm of eight in `central_panel_ui`'s dispatch chain was missing `report_ready`**, and it was
the `StructuralView::Animate` arm. `viewport.structural` deliberately survives a stage change (it
is a camera) and `clamp_structural_sub_view` returns early on every non-report stage (also
deliberate), so that guard was the *only* thing between a left-behind `Animate` and another
stage's pane — and the arm sits **above** the Events, Initialization and Flatten arms, so it won.

**Choose Animate ▶ on Index Reduction, click Events: the index-reduction replay is drawn under the
Events tab**, with the Events sub-view row above it offering Tree / pre() lowering. Same for
Initialization ▸ IC Plan and Flatten ▸ Equations.

**Third instance of the stranded-sub-view class, and a different failure mode.** The alias defect
was **absence filled** — a pane saying a model had no alias eliminations when it had several.
This is **presence substituted**: a correct animation of the wrong phase, under another phase's
tab, with nothing on screen admitting it. Both earlier fixes were about *which sub-view is
selected*; this is about **whether the dispatch honours a selection the current stage does not
offer**, which no amount of clamping on report stages could reach.

**The generalisable check is cheap and was never run: read a dispatch chain's arms as a column and
look for the odd one.** Seven arms carried `report_ready &&`; one did not. That is visible in a
single `grep` of the `} else if` lines, and it is the same shape as the `_ =>` wildcard found
inside the live-debug cluster — **a guarded cluster is only as good as its least-guarded member,
and nothing here compares members to each other.**

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

3. ✅ **`tour_panel_ui`'s inner scroll area** (209 of 246 lines) — **DONE 2026-08-19**,
   `tour_panel.rs` (735, renamed from `tour_transport.rs`), **−244 lines**. **The question this
   line asked was the right one and the answer was yes:** *"check whether the `SplitState`
   configuration can stay in `App` with only the inner closure moving."* It can, and that is
   the whole extraction — the panel, the split and the four `App` methods stayed; only the
   prose left. `App::tour_panel_ui` is 37 lines of pure policy now. See the finding above for
   the rule it generalises to.

4. ✅ **`stage_tab_bar_ui`** (280, 12 fields, **2 `App` methods**) — **DONE 2026-08-19**,
   `stage_tabs.rs` (493), **−193 lines**. It was taken *instead of* `context_bar_ui`, which the
   coupling table ordered first, and the ordering was right for the wrong reason: two `App`
   methods did not make it cheap to move whole. It moved by the region rule, and the deferral
   test came out of it.

5. ✅ **`context_bar_ui`** (255, 6 fields, **7 `App` methods**) — **DONE 2026-08-19**,
   `context_bar.rs` (520), **−196 lines**. **The prediction in this line was wrong in both
   halves.** Seven `App` methods did not make it "a pane made of policy": six of the seven were
   free, five of them because they sat below the last `ui` call. And the region rule found a
   very clean middle — everything between the empty-state early return and the trailing press
   block. See the finding above.

6. **`central_panel_ui`** (619) — last, because it is the stage-routing hub and every other move
   shrinks what it has to route. **It has not shrunk yet**; it grew by 17 lines across five
   extractions, because routing is what survives them.

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
