# HRW Debugger Bridge

**Purpose:** what this VS Code extension does, how to build and install it, and what to check
when the Debug button does nothing.
**Status:** 👤 reference, written for a human.
**Read when:** setting up HRW's live-trace debugging, or when a breakpoint that should have
been armed was not.

## What it does

**It lets HRW arm a breakpoint on a debug session that is already running**, without a
restart.

That is the whole feature, and it exists because of a specific friction: to study how Rumoca
computes something, you want a breakpoint at the line that computes it — but finding that line
means knowing the compiler's internals, which is the thing you are trying to learn. So HRW
offers *"show this being set"* on an IR field, and something has to turn that into a real
breakpoint. VS Code's debug API can do it; a Rust application cannot reach that API. Hence a
small extension.

**The protocol is three files in `../.hrw-bridge/`** (gitignored, and already where HRW and
Claude exchange context):

```
HRW writes          breakpoint-request.json
this extension      reads it, calls vscode.debug.addBreakpoints() or removeBreakpoints(),
                    deletes the request
this extension      writes breakpoint-ack.json
HRW reads           the ack, and knows the breakpoint is live
```

A file watcher rather than a socket: no port to choose, no handshake, and it survives either
side restarting.

## Build and install — required, or the Debug button silently does nothing

**`out/` is gitignored**, so a fresh clone has no compiled extension. This is the single most
common reason live trace appears broken.

**Node.js is a prerequisite** and is not needed anywhere else in this project, so a machine set
up for HRW may well not have it: `winget install OpenJS.NodeJS.LTS`. A new shell is needed
afterwards for `npm` to be on `PATH`.

```powershell
cd hrw\vscode-extension
npm install
npm run build
npm test          # 11 tests — the surface contract and the request/ack schemas
```

Then install it by **linking this folder into VS Code's extensions directory**, in a shell
started after Node was installed:

```powershell
New-Item -ItemType Junction -Path "$env:USERPROFILE\.vscode\extensions\dougdew64.hrw-debugger-bridge-0.1.0" -Target "$PWD"
```

Then **reload VS Code**. Confirm with `code --list-extensions`, which should list
`dougdew64.hrw-debugger-bridge`.

> **Why a junction and not `code --install-extension`.** That command takes a **marketplace ID
> or a `.vsix` file** — handed a folder, VS Code 1.126.0 answers *"Extension
> 'hrw\vscode-extension' not found"* and exits 1. This README and `docs/setup-windows.md` both
> carried the folder form until 2026-08-07, when a fresh machine ran it for the first time.
>
> A junction is also **better than installing a copy**: the link points at this folder, so
> `npm run build` updates the installed extension in place and only a window reload is needed.
> A `.vsix` would have to be rebuilt and reinstalled after every edit. Junctions need no
> administrator rights, unlike symlinks without Developer Mode.

**To check it is working:** open the **"HRW Bridge"** output channel. A successful arm logs

```
Armed: live_trace.rs:<line>
```

and the status bar carries an item you can click to run **HRW: Clear Armed Breakpoints**.

## Publishing the debug session for Claude

**Claude cannot see a debug session.** Measured 2026-08-07 (`../docs/ideas.md` #70): a stop gives
it no location, no stack and no values, and nothing in its tool surface exposes them. Stopping
reveals the *file* and selecting a line reaches Claude, but the state of the running program does
not.

So this extension publishes it. On every stop it writes **`.hrw-bridge/debug-state.json`** —
which Claude reads — using three Debug Adapter Protocol round-trips: `stackTrace`, then `scopes`
for the innermost frame, then `variables` for its most local scope. On `continued` and at session
end it writes a *running* payload instead.

Each write is logged to the "HRW Bridge" channel, so the feature is visibly working:

```
#7 stopped at matching.rs:189 in augment_traced — 3 frame(s), 12 var(s)
```

**Why the stack matters most.** For `augment_traced`, recursion depth *is* the augmenting path:
three nested frames is a two-edge alternating path, and each frame's `eq` is a node on it. The
structure `matching.md` spends three acts animating is exactly what the stack pane holds.

Four properties are load-bearing, because a wrong answer here is Claude describing a program state
that never existed:

| Property | Why |
|---|---|
| `variables: null` ≠ `variables: []` | `null` + `variablesError` means **not fetched**; `[]` means fetched and empty. Collapsing them reports "no locals" for a frame that was never read |
| `continued` publishes too | Otherwise the last stop stays on disk looking current, and Claude describes a position the program has left |
| `frameCount` is the true total | The list is capped at 40 with `framesTruncated` set — a shortened stack otherwise reads as a complete one |
| `seq` + `writtenAtMs` on every write | **A reader must check staleness before trusting anything else.** A leftover payload from the previous step is worse than none |

Nothing deletes the file at shutdown, deliberately: the staleness check has to work anyway for the
case where VS Code exits without warning. Writes go through a temp file and `rename`, so a read
can never tear.

## When it does not work

| Symptom | Cause |
|---|---|
| Debug button does nothing, "HRW Bridge" channel silent | Not built or not installed — the steps above |
| Channel says *"No .hrw-bridge directory found"* | The workspace folder is wrong. **Open the repository root, not `hrw/`** |
| Breakpoint arms but never hits | Wrong launch config — use **`Debug HRW Observatory (cppvsdbg)`**, and launch it from the dropdown rather than rust-analyzer's Debug CodeLens |
| Locals show `<optimized out>` | The crate under study needs `opt-level = 0` in the workspace `Cargo.toml` |

## Developing it

```powershell
npm run watch    # recompile on change
npm test         # builds, then runs tests/ under node --test
```

**`src/debug_state.ts` holds the logic and imports no `vscode`**, which is what makes it
testable: the `vscode` module exists only inside the extension host, so anything importing it
cannot be exercised by `node --test`. `src/extension.ts` is the wiring shell around it — the same
split HRW makes on the Rust side with `Plot::problems()`, for the same reason.

**Put new logic in `debug_state.ts`, not in `extension.ts`.** `debug_state.test.mjs` tests the
real module; `extension_surface.test.mjs` checks the manifest's promises and, in a few cases,
asserts on literals it built itself — which is what the untestable layer forces.

## Further reading

- 👤 [`../docs/setup-windows.md`](../docs/setup-windows.md) — the full machine setup; this
  extension is steps 7 and 8
- 👤 [`../docs/architecture.md`](../docs/architecture.md) §8 — how live trace works end to
  end, and why each launch-config setting is not optional
- [`../docs/debug-set-sites.md`](../docs/debug-set-sites.md) — the table mapping an IR field
  to the Rumoca line that assigns it *(Claude's working reference, not written for a human)*
