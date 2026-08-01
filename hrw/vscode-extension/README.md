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

```powershell
cd hrw\vscode-extension
npm install
npm run build
cd ..\..
code --install-extension hrw\vscode-extension
```

Then **reload VS Code**.

**To check it is working:** open the **"HRW Bridge"** output channel. A successful arm logs

```
Armed: live_trace.rs:<line>
```

and the status bar carries an item you can click to run **HRW: Clear Armed Breakpoints**.

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

`src/extension.ts` is the whole implementation. The tests check the surface — that the
commands and activation events the manifest promises actually exist — rather than driving VS
Code, which would need a full extension-host harness for very little.

## Further reading

- 👤 [`../docs/setup-windows.md`](../docs/setup-windows.md) — the full machine setup; this
  extension is steps 7 and 8
- 👤 [`../docs/architecture.md`](../docs/architecture.md) §8 — how live trace works end to
  end, and why each launch-config setting is not optional
- [`../docs/debug-set-sites.md`](../docs/debug-set-sites.md) — the table mapping an IR field
  to the Rumoca line that assigns it *(Claude's working reference, not written for a human)*
