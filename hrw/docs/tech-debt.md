# Tech Debt — HRW Observatory

Weekly quality improvements identified by code review. Items are grouped by
theme, ordered by severity within each group. Check off items as they are
completed; clear completed items at the end of each cycle.

Previous cycles: 48 items fixed across two passes (2026-07-22). See git history
for details.

---

## Bugs / correctness

- [x] **`open()` does not clear live debug state on specimen change.**
  `App::open()` resets all cached stage data but does NOT clear
  `pending_live_debug` or `live_breakpoint_armed`. If the user switches
  specimens while a live debug session is active or arming, the armed
  breakpoint is never cleaned up, `live_breakpoint_armed` stays stale, and
  `pending_live_debug` keeps polling. Fix: in `open()`, if
  `live_breakpoint_armed` is true call `bridge::remove_live_trace_breakpoint()`,
  then set both fields to their defaults. Same gap exists in `drain_worker`'s
  `Compiled` handler (which clears `cached_matching_anim`/`cached_tarjan_anim`
  but not the debug state).
  *Files:* `app.rs` — `open()` (~line 456) and `Compiled` handler (~line 539).

- [x] **Dangling doc comment on `check_breakpoint_ack`.**
  `bridge.rs` lines 292–296 have two doc comments concatenated: a stale
  fragment ("Write a breakpoint request for `live_trace_breakpoint`...") left
  from a code edit is prepended to the real doc comment for
  `check_breakpoint_ack`. Remove the stale fragment.
  *File:* `bridge.rs` (~line 292).

- [x] **Extension: version-check early return leaves request file on disk.**
  In `extension.ts` `handleRequest`, if `request.version !== 1` the function
  returns without calling `fs.unlinkSync(requestPath)`. The request file stays
  on disk as dead state. The `return` should still delete the file before
  exiting (move `unlinkSync` into a `finally` block or add it before the
  `return`).
  *File:* `vscode-extension/src/extension.ts` (~line 86).

- [x] **Extension: `condition: null` passed to `SourceBreakpoint`.**
  `arm_live_trace_breakpoint` writes `"condition": null` in the JSON.
  The extension passes this `null` as the `condition` parameter to
  `new vscode.SourceBreakpoint(location, true, entry.condition)`. The VS Code
  API types `condition` as `string | undefined`, not `string | null`. Works in
  practice (JS coerces) but is a type mismatch. Fix: either omit the
  `condition` field in the JSON when there is no condition, or coerce in the
  extension (`entry.condition ?? undefined`).
  *Files:* `bridge.rs` (~line 330) and `extension.ts` (~line 159).

## Code quality / duplication

- [x] **Duplicated line-finding logic in `arm_` and `remove_live_trace_breakpoint`.**
  Both functions canonicalize `LIVE_TRACE_FILE`, read its source, and find the
  line containing `pub fn live_trace_breakpoint(` with identical code. Extract
  to a `fn find_live_trace_breakpoint_line() -> io::Result<(PathBuf, usize)>`
  helper.
  *File:* `bridge.rs` (~lines 317–324 and 340–347).

## Test gaps

- [x] **No tests for `bridge::arm_live_trace_breakpoint`.**
  The function finds the line number of `live_trace_breakpoint` in the source
  file and generates a breakpoint-request JSON. No test verifies the
  line-finding logic or the JSON structure. Add a test that calls the function
  (to a temp dir or by inspecting the returned JSON) and asserts the line
  points at `pub fn live_trace_breakpoint(`.
  *File:* `bridge.rs`.

- [x] **No tests for `bridge::remove_live_trace_breakpoint`.**
  Same line-finding logic, same JSON generation, same gap. A test should verify
  it produces an `action: "remove"` request with the correct file and line.
  *File:* `bridge.rs`.

- [x] **No tests for `bridge::check_breakpoint_ack`.**
  The function checks file existence, deletes the file, and returns a bool. No
  test verifies this. Add a test that creates the ack file, calls the function,
  asserts it returns `true` and the file is deleted, then calls again and
  asserts `false`.
  *File:* `bridge.rs`.

- [x] **Extension tests: no coverage for `action: "remove"` protocol.**
  `extension_surface.test.mjs` validates add requests and specimen
  accumulation, but has no test for the remove action schema (the `action`
  field, the matching semantics, the log output).
  *File:* `vscode-extension/tests/extension_surface.test.mjs`.

- [x] **Extension tests: no coverage for ack file protocol.**
  No test verifies that the extension writes `breakpoint-ack.json` after
  processing a request. Add a schema-level test asserting the expected ack
  structure `{ "acked": true }`.
  *File:* `vscode-extension/tests/extension_surface.test.mjs`.

## Documentation gaps

- [x] **DECISIONS.md still references old "arm it" shortcut name.**
  Line 142 says `"arm it"` — should say `"debug"` after the rename
  (commit 9d930e99).
  *File:* `DECISIONS.md` (~line 142).

- [x] **Memory index (MEMORY.md) has stale shortcut name.**
  The index line for `hrw-chat-shortcuts.md` still says
  `` `arm it` (arm the debugger breakpoint) `` though the memory file itself
  was updated to `debug`. Update the index line.
  *File:* `~/.claude/projects/-home-dougdew-dev-rumoca/memory/MEMORY.md`.
