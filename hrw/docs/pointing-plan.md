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

### Step 3 — adopt TWO regions, then stop ✅ *(2026-09-22, awaiting Doug’s run)*

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

**A defect this step produced and caught the same hour.** The wrapper first used
`Sense::click()`, which registers the region *after* its children and therefore on top of them:
it swallowed every primary click in the lab panel, and clicking an `hrw://` link did nothing.
**Six UI tests went red at once**, which is the only reason it was found before Doug saw it. A
region must be pointable without becoming a lid over what it wraps, so it senses hover only and
detects the secondary click by hand — and `resp.hovered()` does not fire on a hover-sense
response, so the hit test is `ui.rect_contains_pointer`.

### Step 4 — adopt the remaining regions ⬜

One commit per group, each runnable.

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
