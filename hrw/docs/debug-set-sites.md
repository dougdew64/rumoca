# Debug set-sites — IR field → where Rumoca assigns it

Reference for the 🐞 **"Show this being set (debugger)"** feature. When a focus with
`request: "debug-where-set"` is captured, Claude maps the field (the last segment of the node's
`key_path`) to the Rumoca source line that assigns it, and writes a breakpoint request to
`.hrw-bridge/breakpoint-request.json`. The **HRW Debugger Bridge** VS Code extension
(`hrw/vscode-extension/`) watches this file and calls `vscode.debug.addBreakpoints()` to arm the
breakpoint on the **active debug session** — no restart required.

**Keyed by function + the assignment statement**, not just line numbers — line numbers drift, so
Claude re-locates the statement in the current source at arm-time (the numbers below are hints).

Paths are relative to the Rumoca crate root. With HRW in-workspace, the source lives at
`crates/<crate-name>/src/<file>` relative to the repo root. Claude writes absolute paths in the
breakpoint request so the VS Code extension can set breakpoints without path resolution.

### Breakpoint request protocol

Claude writes `.hrw-bridge/breakpoint-request.json`:

```json
{
  "version": 1,
  "specimen": "ProportionalLoop",
  "breakpoints": [
    {
      "path": "/home/dougdew/dev/rumoca/crates/rumoca-phase-resolve/src/registration.rs",
      "line": 22,
      "condition": "def_id.0 == 85"
    }
  ]
}
```

The extension reads it, calls `vscode.debug.addBreakpoints()`, shows a status bar indicator,
and deletes the file. Breakpoints **accumulate** across requests for the same specimen. When the
`specimen` field changes, all previously armed breakpoints are cleared before adding the new ones.
The status bar shows the total count; clicking it clears all armed breakpoints manually.

To **remove** a breakpoint, set `"action": "remove"` — the extension matches by file path and line,
removes the breakpoint from VS Code, and updates the status bar. HRW uses this automatically when
a live debug session finishes (all algorithm frames pushed) to prevent a SIGSTOP signal from the
debugger hitting a breakpoint on an exiting thread.

**Ack handshake**: after processing any request (add or remove), the extension writes
`.hrw-bridge/breakpoint-ack.json`. For live debug sessions, HRW polls for this file before spawning
the algorithm thread — this guarantees the breakpoint is registered with LLDB before the first
frame is pushed. The ack file is a simple `{"acked": true}` JSON; HRW deletes it after reading.
`arm_live_trace_breakpoint` clears any stale ack before writing the request so only a fresh ack
from the current request triggers the spawn.

| IR field | Phase / sub-phase | File · function | Assignment (find this) | Line hint |
|---|---|---|---|---|
| `def_id` (class) | resolve · registration | `rumoca-phase-resolve/src/registration.rs` · `register_stored_definition` | `class.def_id = Some(def_id);` | ~22 |
| `def_id` (component) | resolve · registration | `…/registration.rs` · `register_class` | `comp.def_id = Some(def_id);` | ~69 |
| `def_id` (nested class) | resolve · registration | `…/registration.rs` · `register_class` | `nested.def_id = Some(def_id);` | ~80 |
| `scope_id` | resolve · registration | `…/registration.rs` · `register_class` | `class.scope_id = Some(class_scope);` | ~56 |
| `type_def_id` | resolve · contents (reference resolution) | `rumoca-phase-resolve/src/contents.rs` | `comp.type_def_id = Some(type_def_id);` | ~130, ~137 |

## Notes on triggering / conditions

- **First-hit convenience:** `RotationalInertia` is the first source class registered (DefId 85, right
  after ~84 builtins), so the registration breakpoints stop on it almost immediately — no condition
  needed; check the `name` variable and Continue if it isn't the one you want.
- **`type_def_id` in a loop:** `contents.rs` resolves every component's type, including MSL ones, so
  that breakpoint hits many times. Continue until the surrounding component/class is the one captured
  (inspect the local names), or Claude adds a name/id condition when arming.
- **Live arming:** breakpoints are set on the running debug session via the HRW Debugger Bridge
  extension. No restart needed — arm a breakpoint, then select a new specimen to trigger it.

## Conditional arming (integer-identity) — break only for the captured item

Big systems process the same phase code hundreds of times (every component's `type_def_id`, every
coupled block's tearing). An *unconditional* breakpoint there makes you Continue past dozens of
irrelevant hits to reach the one you captured. So when arming, Claude generates a **conditional**
breakpoint keyed on the captured item's identity — the debugger doesn't pause until *your* item is
being processed.

**The one rule: a condition can only reference what's in lexical scope at the armed line.** So the
site is chosen for its discriminator, and the discriminator must be something the LLDB expression
evaluator handles cleanly — i.e. an **integer / enum-as-int**, not a Rust `String` (name matching
needs CodeLLDB's `/py` evaluator; deliberately out of scope here). Pick the discriminator from the
focus and generate `--condition '<int expr>'`:

| Captured item | Best in-scope discriminator | Site · condition |
|---|---|---|
| A **coupled block** (tearing) | block **size** `n` (a plain `usize` local) | `tearing.rs` · `tear_algebraic_loop` · `--condition 'n == <size>'` |
| A coupled block, when several share that size | its **global equation index** (`EquationRef(pub usize)`) | one frame up in `lib.rs` · `tear_loop`, on the `tear_algebraic_loop(…)` call, condition on the block's equation index, then *step into* the tearing |
| A component's `type_def_id` | the enclosing component/class name or its `def_id` | `contents.rs` — condition on the id integer when available |

**Worked example — the `ProportionalLoop` coupled block (size 3).** Its focus subtree gives
`size: 3` and equations `f_x[2] / f_x[1] / f_x[0]` (the `f_x[N]` label *is* the global equation
index, via `EquationRef`'s `Display`). Armed condition: `--condition 'n == 3'` at
`tearing.rs` (`tear_algebraic_loop`). For this specimen that's already unique (its only loop);
in a system with other size-3 loops, escalate the site to `tear_loop` and condition on an equation
index (e.g. the residual `f_x[0]` → index `0`).

**Reliability note.** Conditions on a *plain scalar local* (`n`, an `usize`) are the dependable
form and what Claude arms by default. Conditions that must reach into a Rust slice/newtype
(`equations[0].0`) depend on the CodeLLDB expression evaluator's field/index access — arm them when
size alone isn't unique, but **confirm the breakpoint actually stops on the intended item** in the
IDE (LLDB-command breakpoints don't paint a gutter dot; verify by inspecting the locals on first
stop), and Claude will lock in whatever expression form your CodeLLDB accepts.

## The teaching split this reveals

`def_id`/`scope_id` are assigned in **registration** (identities and scopes are minted). `type_def_id`
is assigned later in **contents** (a name reference is *resolved* to an existing identity). Watching
the two breakpoints in one session shows the two halves of Phase 2: *assign* then *resolve*.

---

## Live debug stepping — algorithm animation synced to the VS Code debugger

The third animation mode: the user steps through algorithm code in the VS Code debugger and the HRW
animation view updates in lockstep, showing each algorithmic decision as it happens.

### Architecture

Each traced algorithm (`maximum_matching_with_trace`, `tarjan_scc_with_trace`,
`reduce_constrained_dummy_derivatives_with_trace`,
`index_reduce_missing_state_derivatives_with_trace`) accepts an optional
`&LiveTrace<Frame>` — the producer half of an `mpsc` channel. When present, every frame
push sends through the channel. The animation struct holds the `Receiver<Frame>` and
drains it via `try_iter()` each UI frame, auto-advancing the cursor to the latest frame.

The producer and consumer share no application-level lock — the `Sender` and `Receiver`
use the channel's internal synchronization only.

When the debugger pauses the algorithm thread at `live_trace_breakpoint`, the UI shows the
current state. When the user continues (F5), the next frame is pushed and the UI updates.

### How to use it

1. **Launch HRW under the debugger** (F5).
2. **Load a specimen** in HRW (select it from the specimen list).
3. **Navigate to the Structural or Index Reduction tab**, then select the **Matching**,
   **BLT**, or **Reduction** animation sub-tab.
4. **Click the "Debug" button** — this automatically arms a breakpoint on
   `live_trace_breakpoint` via the HRW Debugger Bridge extension, then spawns a dedicated
   algorithm thread. The thread first calls `wait_for_debugger()` which hits the breakpoint
   *before* any algorithm work begins — your first Continue (F5) starts the algorithm from
   step zero, so no steps are missed. After each frame is pushed, a 20ms delay lets the HRW
   UI render, then the breakpoint fires again. Each subsequent Continue (F5) advances one step.
5. **Inspect locals**: at the breakpoint, `frame_index` tells you which step you're on
   (`usize::MAX` on the startup gate, then 0, 1, 2, …). Step up one frame to reach the
   algorithm code (`augment_traced`, `strongconnect`, or
   `reduce_constrained_dummy_derivatives_with_trace`) where the full local state is in scope.
6. **Re-run**: after the session finishes, the Debug button reappears — click it to start a new
   live session.

### Breakpoint sites for live stepping

| Algorithm | File | Function | Line to break on |
|---|---|---|---|
| **All** (recommended) | `crates/rumoca-phase-structural/src/live_trace.rs` | `live_trace_breakpoint` | the `LAST_FRAME_INDEX.store` line |
| **Matching** (per-frame) | `crates/rumoca-phase-structural/src/matching.rs` | `emit_matching_frame` | `frames.push(frame)` (after the `lt.push` call) |
| **Tarjan** (SCC discovery) | `crates/rumoca-phase-structural/src/tarjan.rs` | `TracedTarjanState::record` | `self.frames.push(frame)` (after the `lt.push` call) |
| **Reduction** (per-step) | `crates/rumoca-phase-structural/src/dae_prepare.rs` | reduction trace call sites | after each `lt.push` call |

The recommended site is `live_trace_breakpoint` — it is `#[inline(never)]` and non-generic, so the
debugger resolves it to a single unambiguous address (unlike `Vec::push` calls that can share
monomorphized code at higher opt-levels). It fires for all three algorithms (matching, Tarjan, and
reduction). The `frame_index` parameter tells you which step you're on — `usize::MAX` on the
startup gate (before the algorithm starts), then 0, 1, 2, … for real frames.

### Thread model

```
UI thread                     Algorithm thread (matching-debug / tarjan-debug / reduction-debug)
──────────                    ─────────────────────────────────────────────────────────────────
clicks "Debug"  ──►           spawns thread with LiveTrace producer (owns Sender)
                               calls wait_for_debugger() → breakpoint fires
drains Receiver  ◄── channel ── pushes frames via Sender
  each UI frame               ◄── debugger pauses at live_trace_breakpoint ──►
shows current frame
```

The UI thread never blocks — it drains the `mpsc::Receiver` via `try_iter()` (non-blocking,
no shared lock with the producer). The algorithm thread sends frames through the channel;
the two sides share no explicit mutex, so debugger step-over cannot deadlock on HRW code.

**Known limitation — step-over deadlocks on WSL2**: LLDB's step-over mechanism
(`thread step-over`, F10) deadlocks on multi-threaded Rust programs under WSL2. The symptom
is a spinning "Local variables" indicator that never resolves. This affects both single-thread
mode (default) and all-threads mode (`-m all-threads`). Continue (F5) between breakpoints
works correctly — only the step-over mechanism is affected. The root cause is WSL2's ptrace
implementation, not HRW or Rumoca code. The fix is to run HRW on native Windows, where the
debugger uses Windows debug APIs instead of ptrace.
