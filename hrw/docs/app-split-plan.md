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
| 2026-08-19 | **`tour_prose_ui`** — the inner scroll area of `tour_panel_ui`, + `no_tour_ui` + a constant | **12,908** | `tour_panel.rs` (735, renamed from `tour_transport.rs`) |
| 2026-08-19 | **the tabs of `stage_tab_bar_ui`** — the span below the ▶ button, + `tab_label` + the row's teaching comment | **12,715** | `stage_tabs.rs` (493, of which 190 are tests) |

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

### ⟶ NEXT: `context_bar_ui`, and its seven methods now get the deferral test

255 lines, 13 fields, **7 `App` methods**. Under the old reading that made it the most expensive
thing left; under the sharpened test above it is an open question, because **the cost is how
many of the seven sit mid-body**. Two of them — `background_ui` and `empty_context_hint` — are
rendering helpers rather than presses, and a rendering helper that takes `&self` can simply
move with the pane.

**Run the census in this order:** for each `self.<method>(` in the body, record (a) is it a
press or a render helper, (b) does anything *after* it in the same function read what it wrote.
Only the calls that answer *press* and *yes* are barriers, and only barriers force the cut to
be inside the function.

**`central_panel_ui` (602 lines, 43 fields) and `frame_ui` (299, 32) still look unreachable**,
and still shrink as their callees leave.

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

4. ⟶ **NEXT: `stage_tab_bar_ui`** (280, 12 fields, **2 `App` methods**) — and it is next
   *instead of* `context_bar_ui` (255, 6 fields, **7 `App` methods**), which the coupling table
   ordered first. The measurement and the reasoning are in the finding above. **Apply the region
   rule before the obstacle checklist:** find the span that calls neither `self.open` nor
   `self.start_simulation`, and check whether `sim_data`/`sim_error`/`sim_running` and
   `model`/`model_list` collapse into structs the way `self.tour` did.

5. **`context_bar_ui`** (255, 6, 7) — after it, because seven `App` methods around six fields is
   a pane made of policy, and the region rule may find no clean middle in it at all.

6. **`central_panel_ui`** (602) — last, because it is the stage-routing hub and every other move
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
