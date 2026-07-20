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

## The teaching split this reveals

`def_id`/`scope_id` are assigned in **registration** (identities and scopes are minted). `type_def_id`
is assigned later in **contents** (a name reference is *resolved* to an existing identity). Watching
the two breakpoints in one session shows the two halves of Phase 2: *assign* then *resolve*.
