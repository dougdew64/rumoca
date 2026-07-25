# UI Modes Design — HRW Observatory

Design for the three-mode UI structure and the implementation plan to get there.
Agreed 2026-07-25 between Doug and Claude.

---

## The three modes

HRW has three usage modes. All three share the same binary and the same app
state — the mode determines which panels are visible and how the left half of
the window is used.

### Tour mode

- **Window:** fullscreen, no VS Code alongside.
- **Layout:** 50/50 split. LHS = tour guide (rendered markdown from the
  end-to-end tour document). RHS = stage tabs pane.
- **Purpose:** guided learning — the tour text drives navigation into the stage
  views. Clicking a tour link (e.g. "load BouncingBall", "switch to Structural
  tab") directly loads the specimen and switches the stage tab.

### Specimen mode

- **Window:** fullscreen, no VS Code alongside.
- **Layout:** 50/50 split. LHS divided into top third (specimen list) and
  bottom two-thirds (specimen narrative from `docs/specimen-notebook/`). RHS =
  stage tabs pane.
- **Purpose:** self-directed exploration — browse specimens, read each one's
  narrative (the "why this specimen matters" story), and inspect the compiler
  output.

### Debug mode

- **Window:** right half of the screen, VS Code on the left half.
- **Layout:** LHS hidden, stage tabs pane fills the full HRW window width.
- **Purpose:** debugger-assisted investigation — source code, breakpoints, and
  the call stack in VS Code; stage views in HRW. The Claude bridge and live-debug
  protocol operate in this mode.

### Summary table

| Aspect          | Tour          | Specimen              | Debug             |
|-----------------|---------------|-----------------------|-------------------|
| Window          | Fullscreen    | Fullscreen            | Right half        |
| LHS content     | Tour guide    | Specimen list + narrative | Hidden         |
| RHS content     | Stage tabs    | Stage tabs            | Stage tabs        |
| VS Code needed  | No            | No                    | Yes               |

---

## Design decisions

### Mode switching is a panel toggle

The three modes are a single app with a left-panel selector (tour / specimen /
hidden), not three separate launch configurations. The View menu and a keyboard
shortcut cycle between modes. The RHS stage tabs pane is always present — it's
the constant across all modes.

### Navigation actions are shared primitives

Both tour mode and specimen mode need to load a specimen, switch a stage tab,
and (eventually) scroll to a specific tree node. These are the same operations,
triggered differently: by clicking a tour link (tour mode) or by clicking a
specimen in the list (specimen mode). The navigation layer is built once and
shared.

Navigation primitives:
- `load_specimen(path)` — load and compile a specimen.
- `switch_stage(StageKind)` — change the active stage tab.
- `navigate_to(path_segments)` — expand the tree to a specific node (future).

### Markdown rendering via `egui_commonmark`

Tour content and specimen narratives are both markdown. `egui_commonmark`
renders them inside egui with heading/paragraph/list/code-block support. Custom
link handling intercepts clicks on navigation links (e.g.
`hrw://load/BouncingBall`, `hrw://stage/Structural`) and dispatches to the
shared navigation primitives.

### Content loading

Tour content: `include_str!` at build time from
`docs/compiler-phases/end_to_end_tour.md`. The tour is part of the binary —
no file-path dependencies at runtime.

Specimen narratives: loaded at runtime from
`docs/specimen-notebook/<Model>/narrative.md` (relative to the manifest
directory). Loaded on demand when a specimen is selected, cached in memory.

### Specimen-switcher in debug mode

Debug mode hides the left panel (no specimen list visible). A compact
specimen-switcher dropdown in the stage tabs header bar allows switching
specimens without leaving debug mode.

---

## Implementation plan

**All six steps completed 2026-07-25.** Steps 1–5 delivered the full structural
and interactive UI. Step 6 (content walkthrough) is deferred to when Doug begins
the learning effort — tour and narrative content will evolve through Q&A
conversations alongside Cellier reading, not front-loaded.

### Step 1: Add the mode enum and left-panel routing ✅

- Add `UiMode` enum: `Tour`, `Specimen`, `Debug`.
- Add `ui_mode: UiMode` field to `App` (default: `Specimen`).
- Replace `show_left_panel: bool` with mode-based logic: `Tour` and `Specimen`
  show the left panel; `Debug` hides it.
- View menu: replace "Specimens panel" checkbox with mode selector (Tour /
  Specimen / Debug).
- No new dependencies. No visual change yet in Tour mode (left panel is empty).

### Step 2: Specimen mode — narrative pane ✅

- Split the left panel into top third (existing specimen list) and bottom
  two-thirds (narrative display).
- Load `narrative.md` for the selected specimen on demand. Cache in a
  `HashMap<PathBuf, String>`.
- Add `egui_commonmark` dependency (record in `DECISIONS.md`).
- Render the narrative as styled markdown in a scrollable region. The specimen
  list stays pinned at the top.
- Tour mode left panel still empty (placeholder text).

### Step 3: Tour mode — render the tour document ✅

- Embed the end-to-end tour via `include_str!`.
- Render it in the left panel when `ui_mode == Tour`.
- Scrollable, styled markdown. No clickable links yet — just readable text.
- At this point all three modes are structurally complete: tour text on the
  left, specimen list + narrative on the left, or nothing on the left.

### Step 4: Debug mode — specimen-switcher dropdown ✅

- Add a compact specimen dropdown (combo box) to the stage tabs header bar.
- Visible in all modes but essential in debug mode (no specimen list visible).
- Selecting a specimen from the dropdown loads and compiles it, same as
  clicking in the specimen list.

### Step 5: Navigation links — make tour and narrative clickable ✅

- Define a simple link scheme: `hrw://load/<Specimen>`,
  `hrw://stage/<StageName>`, `hrw://load/<Specimen>/<StageName>` (combined).
- Hook `egui_commonmark`'s link callback to intercept `hrw://` links and
  dispatch to the shared navigation primitives.
- Add navigation links to the end-to-end tour document where it currently says
  things like "click the Parse tab" or "load BouncingBall".
- Same link scheme works in specimen narratives.

### Step 6: Tour polish and completion (deferred)

- Walk through the end-to-end tour in tour mode. Fix any rendering issues,
  missing links, or navigation gaps discovered during the walkthrough.
- **Deferred:** content walkthrough will happen alongside Doug's Cellier reading.
  Tour and narrative content will evolve through Q&A conversations — they are
  living documents, not deliverables to front-load.

---

## What this design does NOT include (future work)

- Tour progress tracking (checkmarks, "you are here" indicator).
- Multiple tour documents (currently just the end-to-end tour).
- Animated algorithm stepping embedded in tour steps (idea #9).
- Tree-node-level navigation links (hrw://node/path — step 5 covers specimen
  and stage level only).

These are natural extensions but not needed for phase 4 completion.
