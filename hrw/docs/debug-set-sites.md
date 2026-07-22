# Debug set-sites — IR field → where Rumoca assigns it

Reference for the 🐞 **"Show this being set (debugger)"** feature. When a focus with
`request: "debug-where-set"` is captured, Claude maps the field (the last segment of the node's
`key_path`) to the Rumoca source line that assigns it, and arms a breakpoint there in the
`.vscode/launch.json` "Debug HRW — break where Claude armed" config.

**Keyed by function + the assignment statement**, not just line numbers — line numbers drift, so
Claude re-locates the statement in the current clone at arm-time (the numbers below are hints).

Paths are relative to the Rumoca crate root. Rumoca is now a git dependency pinned in `Cargo.toml`,
so the source lives in Cargo's cache — locate a file with
`find ~/.cargo/git/checkouts -path '*rumoca*/<file>'`. The 🐞 launch config matches breakpoints by
basename, so it works regardless of the absolute path.

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
- **Fresh process required:** resolution runs lazily on first specimen select, so launch the debug
  config *before* clicking the specimen.

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

Each traced algorithm (`maximum_matching_with_trace`, `tarjan_scc_with_trace`) accepts an optional
`LiveTrace<Frame>` — a shared `Arc<Mutex<Vec<Frame>>>` buffer. When present, every frame push also
writes to the shared buffer. The animation view polls this buffer each UI frame and auto-advances
the cursor to the latest frame.

When the debugger pauses the algorithm thread at the `LiveTrace::push` call, the UI shows the
current state. When the user resumes/steps, the next frame is pushed and the UI updates.

### How to use it

1. **Launch HRW under the debugger** (F5) — **do not set any breakpoints yet**. Setting the
   breakpoint before loading a specimen can freeze the UI (the debugger pauses all threads when
   any thread hits a breakpoint).
2. **Load a specimen** in HRW (select it from the specimen list).
3. **Navigate to the Structural or Index Reduction tab**, then select the **Matching** or **BLT**
   animation sub-tab.
4. **Now set a breakpoint** on `live_trace_breakpoint` (see table below).
5. **Click the "Debug" button** — this spawns a dedicated algorithm thread. After each frame is
   pushed, a 20ms delay lets the HRW UI render, then the breakpoint fires. Each time you
   Continue (F5) in the debugger, the next frame appears in the HRW animation.
6. **Inspect locals**: at the breakpoint, `frame_index` tells you which step you're on. Step up
   one frame to reach the algorithm code (`augment_traced` or `strongconnect`) where the full
   local state (match_eq, match_var, visited, eq, var, etc.) is in scope.
7. **Re-run**: after the session finishes, the Debug button reappears — click it to start a new
   live session.

### Breakpoint sites for live stepping

| Algorithm | File | Function | Line to break on |
|---|---|---|---|
| **All** (recommended) | `crates/rumoca-phase-structural/src/live_trace.rs` | `live_trace_breakpoint` | the `black_box` line |
| **Matching** (per-frame) | `crates/rumoca-phase-structural/src/matching.rs` | `emit_matching_frame` | `frames.push(frame)` (after the `lt.push` call) |
| **Tarjan** (SCC discovery) | `crates/rumoca-phase-structural/src/tarjan.rs` | `TracedTarjanState::record` | `self.frames.push(frame)` (after the `lt.push` call) |

The recommended site is `live_trace_breakpoint` — it is `#[inline(never)]` and non-generic, so the
debugger resolves it to a single unambiguous address (unlike `Vec::push` calls that can share
monomorphized code at higher opt-levels). It fires for both matching and Tarjan. The `frame_index`
parameter tells you which step you're on.

### Thread model

```
UI thread                     Algorithm thread (matching-debug / tarjan-debug)
──────────                    ────────────────────────────────────────────────
clicks "Debug"  ──►           spawns thread running maximum_matching_with_trace
                               with LiveTrace<MatchingFrame>
polls LiveTrace  ◄──          pushes frames to shared buffer
  each UI frame               ◄── debugger pauses here ──►
shows current frame
```

The UI thread never blocks — it polls the `LiveTrace` via `Mutex::lock` (uncontended when the
algorithm thread is paused at a breakpoint). The algorithm thread is the only writer; the UI thread
is the only reader.
