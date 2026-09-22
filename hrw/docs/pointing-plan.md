# Pointing plan — one verb, reached by right-click, from every region

**Purpose:** the ordered plan for replacing the 🎯 capture button with a right-click *"Point at"*
on selected text, carried by a shared per-region helper that records **where the text came from**.
**Status:** live plan, opened 2026-09-22. **Read when:** resuming this work, or touching the
capture path, `PointKind::LabPassage`, `Focus::LabPassage`, or any pane that renders selectable
text.

---

## Why, in Doug's words

> *"HRW's notion of 'Pointing at' something is powerful for deixis… We have implemented different
> mechanisms for me to point at stuff. Our current capture button scheme seems problematic. As you
> noted, the button is associated with the lab pane, and is only available when a lab is open.
> Second, 'capture' is a low-level plumbing detail which is not as helpful for me as the more
> general high-level concept of 'Pointing at'. Now that we have generalized text selection across
> the entire HRW window, would it be possible… to right-click on selected text to make a context
> menu appear and then select 'Point at'? If so, then we could eliminate the capture button
> entirely, along with the notion of 'capture'."*

And on the choice between a per-panel menu and a shared helper:

> *"Ultimately, I want to go the shared helper route because it is explicit and seems most likely
> to be accurate about the source of the text which has been selected."*

**That reason is the plan's centre of gravity.** A region wrapper *knows what region it is*, so a
selection's origin becomes a structural fact declared at the call site, rather than inferred from
whatever `view.stage_view` happened to be showing. It is the fix for a defect already recorded:
`view` reports what was on screen, not where the selection came from.

**The vocabulary argument is the stronger half.** CHARTER Decision 8's noun/verb formulation says
the noun is assembled by mouse and the verb is an unbounded utterance. *"Point at"* **is** the
assembling verb; *"capture"* is a second word for the same act, named after its implementation.
Four of the five existing ways to make a point already say *"Point at"* — the button is the
odd one out, so this mostly retires an exception rather than inventing a scheme.

---

## What was verified before the plan was written

Read from `egui` 0.35's source, not assumed. **These are the facts the design rests on; re-check
them at any egui bump.**

1. **A selection dies only on a press while no selectable label is hovered.**
   `text_selection/label_text_selection.rs`:
   ```rust
   let clicked_something_else = ui.input(|i| i.pointer.any_pressed()) && !self.any_hovered;
   ```
   `any_hovered` is `|=`-accumulated from every selectable label's `response.hovered()`.
   **A right-click *on* the selection is a press while a label is hovered, so the selection
   survives** — which makes this gesture strictly safer than the button, whose press lands on the
   transport bar and destroys the thing it acts on. That hazard is the sole reason the button acts
   at mouse-DOWN, with a three-frame wait and a *"a cursor position is not a selection"* timeout.

2. **`Popup::context_menu` opens on `secondary_clicked()` — mouse-UP**, and positions
   `at_pointer_fixed()`. Nothing is pressed between the right-click and the menu appearing, so the
   selection is still live when the menu opens.

3. **Clicking a menu item is a press on the menu, not on a label**, so `any_hovered` is false and
   the selection is cleared at that press — *before* the item's `clicked()` fires at release.
   **Therefore the text must be grabbed when the menu OPENS, never when the item is chosen.**
   This is the one non-obvious constraint in the design.

4. **egui will not hand us the selected text.** `LabelSelectionState` exposes exactly
   `has_selection()` and `clear_selection()`. The clipboard round-trip (`egui::Event::Copy`
   intercepted by `copy_sink`) stays. **The button, the word "capture" and the plumbing's
   visibility go; the plumbing itself does not.** Doug approved on that understanding.

---

## The steps

Each step says what it *proves*, because several cannot be settled by reading.

### Step 0 — prove the assumption ✅ *(2026-09-22)*

A test that selects text in a label, right-clicks it, and asserts the selection is
still live and that a `Copy` pushed on the open frame yields that text.

**Proves the three egui facts above hold in this app**, not merely in egui's source. Blocking:
everything rests on it, and the button exists today *because* a similar assumption failed once.
If it fails, the design changes rather than the schedule slipping.

**Done:** `a_right_click_on_a_selection_keeps_it_alive_and_copy_yields_the_text` in
`src/app/tests.rs`, driving a raw `egui::Context` with `run_ui` for exact pointer control. All
three facts hold. **It carries a negative control** — a press with no label hovered *must* clear
the selection — because without it a harness that never clears anything would pass every
assertion for the wrong reason.

### Step 1 — `PointOrigin` ✅ *(2026-09-22)*

An enum naming every region text can be selected from — lab prose, a named stage sub-view, the
equation sheet, the model list, the log.

**Keep checking what is checkable.** A declaration can be wrong the way an inference cannot be
verified, so lab prose keeps the containment check added 2026-09-22 (`in_source`), and the origin
and the check must agree. Everywhere else the wrapper *is* the proof: the region enclosing the
Connections pane cannot be anything else.

**Done**, in `src/pointing.rs`, with steps 1 and 2 landing together because a variant nobody
constructs is dead code. **Designing it found a defect the containment check cannot reach:**
`LabSource::label` returns `✨ Answer` for Claude's answer document, so a selection made in
it emitted `file: hrw/docs/fixture-labs/✨ Answer.md` — a path that never existed. The
check passes there, because the text genuinely *is* in the document that was open; it is the
*file* claim that is false, and only a declared origin separates the two. Hence `Answer` is a
variant of its own.

### Step 2 — the shared helper ✅ *(2026-09-22)*

`selection::region(ui, origin, |ui| …)`: wraps a region, interacts its rect, attaches the menu,
pushes `Copy` at open, and returns an optional point request for `App` to perform — the
render-and-report shape this codebase already uses four times.

The menu keeps **"Point at selection" always present**, greyed with a reason when there is no
selection. The button is visible exactly when a selection exists, which teaches the gesture; a
context menu teaches nothing until tried, and this is the cheap replacement for that affordance.

**Done.** `pointing::region` renders, interacts the rect it occupied with a fresh id, and returns
`MenuOpened` on the secondary click — the last moment the selection is guaranteed alive —
and `PointAt` when the item is chosen. `App` performs both: clear the copy slot and push `Copy` on
the first, use the fetched text on the second. **Clearing before the push is what stops a stale
Ctrl+C being mistaken for this gesture's text.** A test pins that an ordinary left-click reports
*nothing*, since otherwise every click in a pane would overwrite the reader's clipboard.

**Step 7's reachability guard was written early, and carries step 4's checklist.**
`every_point_origin_is_reachable` names the not-yet-adopted variants and **cuts both ways**: an
adopted variant may not stay on the list. So the plan's remaining work is executable rather than
remembered, and step 4 finishes when that list is empty.

### Step 3 — adopt TWO regions, then stop ✅ *(2026-09-22 — closed; check 3 moved to step 4)*

Lab prose and the Connections pane. Two is enough to prove the thing reading cannot settle:
**how the wrapper composes with the tree row menu, which already carries its own "Point at"** when
an inner row and an outer region both claim one right-click.

**The 🎯 button stays through this step.** Removing it before the new path works in Doug's hands
would take the capability away mid-flight. **Doug runs this one**; Claude tests the logical
surface and he tests the rendered one.

**Regions adopted: the lab panel and the Connections pane.** The lab panel was taken first on
Doug's instruction — *"I will definitely want to use the new point-at mechanism to point at text
in your answer documents. In fact, that is probably going to be the most likely place that I will
use that mechanism."* The answer and a fixture lab draw in the same panel, so one wrapper serves
both, and `lab_panel_origin` keeps them apart because only one of them is in a file.

**Step 5 was folded in here rather than deferred.** The moment the answer document became
pointable, the old emit path would have named `hrw/docs/fixture-labs/✨ Answer.md` for every
selection in it. Shipping a known-false `file:` claim to get a step boundary was not worth it, so
`Focus::LabPassage` now carries the origin and the file is the origin's to name.

**THE MECHANISM CHANGED AT THIS STEP, and the plan's premise was wrong.** *"Right-click on the
selection and read it"* is not possible in egui 0.35: `TextCursorState::pointer_interaction`
fires on `response.hovered() && any_pressed()` — **any** button — and collapses the range
to a caret. In `label_text_selection` that is line 562; `got_copy_event` is line 575, so the
collapse always wins and no in-frame trick recovers it. Doug's third run measured it exactly:
`point-copy-landed | 1 chars`. Filed as **E2** in `upstream-issues.md`.

**So HRW captures the text when the selection is MADE** — a `Copy` at the end of the drag
that produced it — and holds it until the reader points. Doug chose this over suppressing the
press event, which would have broken the tree's own row menus at step 4. It costs a clipboard
write per completed drag-selection, accepted knowingly. A caret click does not copy; a *primary*
press invalidates what is held, so it cannot go stale; a *secondary* press does not, because that
is the pointing gesture.

**`can_point` replaced `has_selection` at the menu**, because `has_selection()` is `true` for the
caret the right-click leaves — an item enabled from it would offer to point at nothing.

**Step 0 gave a false green, and that is the lesson worth keeping.** It was written to de-risk
exactly this. It asserted `has_selection()` survived a right-click — which a caret satisfies
— and then asserted the copied text, which came back whole because the synthetic response
does not report itself hovered the way a real one does. **The negative control proved the harness
could CLEAR a selection; nothing proved it could COLLAPSE one.** A harness that diverges from the
app on the one behaviour under test is worse than no harness, because it is believed.

**THE GESTURE WORKS. THE STEP IS NOT DONE** — marked complete on 2026-09-22 and corrected the
same day, by Doug: *"We've completed only the first of those tests."* Four of the five checks
this step was stopped for are outstanding, and **the one it exists for is among them**:

| # | check | what it would catch | state |
|---|---|---|---|
| 1 | right-click in a **lab**, not an Answer | the `LabProse` branch entirely — the containment check and `source_file()` producing a real path | ✅ |
| 2 | right-click with **nothing selected** | that the item greys for the *right* reason; it was seen greyed only when `last_selection` was being wrongly invalidated | ✅ |
| 3 | ~~**tree row menu vs region menu**, in Flatten → Connections~~ | **THE CHECK WAS MISSPECIFIED.** The tree and the Connections replay are *sub-views of one stage* and are never drawn at the same time, so no right-click can reach both and there is nothing to contend. **The conflict cannot arise until the tree's own pane is wrapped**, which is step 4 — so this moves there, and step 4 must adopt the tree FIRST rather than last | ➡ step 4 |
| 4 | ordinary clicking in the lab panel | links, picker, transport bar. **Automated** for links (`clicking_a_lab_link_dispatches_it` and five siblings went red when the wrapper used `Sense::click()`), and confirmed live by the trail. Picker and transport confirmed by Doug directly — **they record no action, so the trail cannot see them**, and his observation is the only instrument there | ✅ |
| 5 | right-click in an **Answer** | the whole chain, end to end | ✅ |

**Check 2 greyed for the right reason, and the trail proves which.** Two `point-menu-opened` lines with **no `point-copy-landed` between them**: a caret click invalidated the held text, and a click is not a drag, so there was nothing to point at. The same appearance had been produced earlier by a defect, which is why this needed confirming rather than observing.

**It also exposed a flaw in the instrument.** `point-menu-click-ignored` would have fired on a click into a legitimately greyed item — **correct behaviour reported as a fault**. It now requires `can_point` as well, so it records exactly one shape: an *enabled* item, hit, that does not report a click. That is what named the last defect; everything else is noise, and a trail that cries wolf is this arc's most valuable instrument losing its value.

**The `LabProse` branch is proven in the running app too, and it settled the risk the
normaliser was written for.** Doug pointed at *"merge sets of variables"* in `connect-expansion`.
The source at line 26 reads `**merge sets of variables**` — **bolded** — and egui hands back
the rendered text with the markup gone. The containment check matched anyway, so the emitted
context says `file: hrw/docs/fixture-labs/connect-expansion.md` and *"which HRW verified does
contain it"*. A raw `contains` would have called that passage foreign, which was the failure mode
that would have made the warning fire on nearly every real capture and so mean nothing.

Note the two captures differ in `kind` exactly as intended: `lab_passage` with a `file` for lab
prose, `selection` with no `file` for the answer and for a pane.

**WORKING, on Doug's eighth run.** `point-copy-landed | 31 chars` → `point-menu-opened` →
`point-menu-click | enabled=true clicked=true inside_item=true` → `point-at-selection`, and
`focus.json` carrying `kind: selection`, `from: Claude's answer`, **no `file`**. The defect this
began as — a false file claim for text in the answer document — is gone at the root rather
than patched.

**Seven failures, each a different link**, in order: the copy pushed after the labels drew; the
right-click collapsing the selection; the drag predicate reading a `press_origin` egui had
already cleared; the menu click invalidating its own text; and the guard for that reading pass
state instead of memory. **Every one was named by the action trail**, never by reasoning from the
symptom — which is why each fix ends with an instrument rather than only a repair.

**Three tests gave false greens, and they share one shape.** Step 0's harness did not reproduce
hover, so it could not reproduce the collapse it existed to catch. A scratch probe clicked a
popup rect captured on the popup's first frame, before the popup moves. The popup-open test
called its predicate *after* `Popup::show` while the code calls it *before*. **Each verified the
right ingredient at the wrong moment.** For UI behaviour, a synthetic harness checks logic; only
the running program checks interaction — so instrument first and let the trail name the link.

**A second defect, and only Doug's run could find it.** He selected text in an answer,
right-clicked, got the correct menu with the correct origin, chose *"Point at selection"* — and
nothing happened. **egui collects a label selection inside `label_text_selection`, as each label
paints**, so a `Copy` pushed after the prose has drawn is seen by nothing and is discarded when
the frame's events are replaced. `perform_pointing` runs after the panel it belongs to, so the
push was always too late. **The 🎯 button never hit this by accident of layout** — it sits in the
transport bar, drawn above the prose. The push is now deferred to the top of the next frame in
`frame_ui`, and `a_copy_pushed_after_the_label_draws_is_lost` pins the egui fact in both
directions. The action trail is what localised it, which is why the menu-open is recorded.

**A defect this step produced and caught the same hour.** The wrapper first used
`Sense::click()`, which registers the region *after* its children and therefore on top of them:
it swallowed every primary click in the lab panel, and clicking an `hrw://` link did nothing.
**Six UI tests went red at once**, which is the only reason it was found before Doug saw it. A
region must be pointable without becoming a lid over what it wraps, so it senses hover only and
detects the secondary click by hand — and `resp.hovered()` does not fire on a hover-sense
response, so the hit test is `ui.rect_contains_pointer`.

### Step 4 — adopt the remaining regions ✅ *(2026-09-22)*

One commit per group, each runnable.

**ANSWERED, 2026-09-22, and it is a fourth outcome none of the three below predicted: the row menu wins and the region menu is UNREACHABLE.** Doug right-clicked tree rows, then drag-selected text in the pane and right-clicked that. The trail carries **no `point-menu-opened` at all**, which `region` records unconditionally — so the region menu never opened once. `point-copy-landed | 7 chars` shows the selection itself was captured normally. The last capture is `kind: node`, so the gesture he read as *"point at my selection"* pointed at the **row under the cursor** instead.

**The failure is silent and looks like success**, because the Context Bar updates either way and only the emitted `kind` distinguishes them — the same shape as the defect this whole arc began with.

**So a region whose items already have menus cannot be served by wrapping it.** The fix is to give the existing menu the item, not to wrap the pane: `tree.rs`'s `row_menu` gains *"Point at selection"* when text is held, beside its *"Point at"*. One menu, no contention, and the two are visibly different choices rather than one that silently means the other. **That splits step 4 in two:** plain-text regions get the wrapper; regions with item menus get the item.

**DONE, and the rule it produced is the lasting part: a region whose items already carry menus takes the ITEM, not the wrapper.** Only one of the four regions turned out to take a wrapper at all.

| region | how | why |
|---|---|---|
| lab panel (prose and the Answer) | wrapper | plain text, no item menus |
| stage pane — Connections replay | wrapper | plain text |
| stage pane — the **tree** | **menu item** | every right-click lands on a row; the wrapper never opened once |
| the **model list** | **menu item** | rows carry their own menu too, found by reading before wrapping |
| the **log** | wrapper | no row menus |

Both menu-item regions also **renamed their node/model entry** — *"Point at this node"*, *"Point at this specimen"* — so the nouns differ from *"Point at selection"*, which appears only when text is held. `every_point_origin_is_reachable`'s pending list is now **empty**, which is what closes this step.

**The model list was caught by the rule rather than by a run.** After the tree, its menu was checked *before* wrapping it — the first time in this arc that a defect was avoided instead of measured.

**The tree is wrapped as of 2026-09-22** — `artifact_pane_ui`, the pane most stages show — and the question is now live and awaiting a run.

**Adopt the TREE first, because it carries step 3's unanswered question.** Every other region is
plain text; the tree's rows already have their own right-click menu, with their own *"Point at"*.
Wrapping that pane is the first moment an inner row menu and an outer region menu can contend for
one right-click, and **nothing before it can tell us what egui does** — step 3 asked Doug to
compare them in Flatten → Connections, which was impossible, because the tree and the replay
are sub-views of one stage and never on screen together.

Possible outcomes, and each changes the design: the row menu wins and the region menu never opens
over a row (ideal — rows keep their richer menu); both open (unusable); the region wins and the
row menu is lost (would require the region to skip rows, or the row menu to gain the selection
item). **Find out before wrapping anything else.**

### Step 5 — origin into the emitted context ✅ *(2026-09-22, folded into step 3)*

Replace the `(a selection from a pane; see `view` for what was shown)` sentinel with the declared
origin, and stop treating `view.stage_view` as evidence of where a selection came from. **Closes
the open defect** recorded in `tech-debt.md` under the 09-22 third pass.

### Step 6 — the atomic rename ⬜

Button deleted; *"capture"* out of the UI strings, the hover text, `focus.json`'s instructions,
the action-trail names, `PointKind::LabPassage` → `Selection`, and the docs — **in one commit**,
per Decision 15's *reimagine now, rename atomically later*.

### Step 7 — guards and records ⬜

- Every `PointOrigin` variant is constructed somewhere, so adding a pane and forgetting the
  wrapper fails loudly.
- No user-facing string says *"capture"*.
- A `DECISIONS.md` entry.
- **The new-stage wiring checklist gains a row.** This creates a new per-stage system: a future
  stage rendering a pane without a region wrapper would be silently unpointable.

---

## Risks Doug accepted before step 0

- **Pushing `Copy` at menu-open writes the system clipboard**, as the button does today, but now
  on a gesture that does not normally clobber it.
- **Menu precedence** cannot be settled by reading, which is why step 3 stops.
- **Discoverability drops**, mitigated by the always-present greyed item.
