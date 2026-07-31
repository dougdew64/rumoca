# Upstream issues — Rumoca bugs found through HRW

**Ready to file with [CogniPilot/rumoca](https://github.com/CogniPilot/rumoca).** Doug files
these when the time is right; Claude adds entries as they are found and never files them
itself.

Each entry is written to be **filable with a copy-paste plus a sentence** — reproduction,
expected vs actual, evidence, and where the suspect code lives — so nothing has to be
re-investigated months later. That is the whole point of the file: an investigation
regenerates only at the cost of doing it again.

**Baseline for everything below:** Rumoca `0.9.20`, `hrw` branch cut from upstream
`8cdc7419`. Verify against current upstream before filing — a bug fixed in the meantime
should be struck out here, not reported.

**Why this file exists:** bugs found through HRW are opportunities to build the maintainer
relationship, not just to work around (`project-engage-rumoca-community`). Both entries
below were found by *auditing failure paths* (`docs/ideas.md` #45), and the second was
adjudicated by an independent Modelica implementation rather than argued from the spec.

---

## Which to file first

**Issue 2 (connector validation).** Doug's plan (2026-07-30) is to open one bug PR with a
screen-capture video of a self-playing HRW tour attached — no campaigning, just something
likely to prompt a reviewer to ask what it is.

Issue 2 suits that far better than issue 1:

- **The reproduction is one 20-line model**, where issue 1 needs three compiles in a
  particular order within one session.
- **It is independently adjudicated.** System Modeler rejects the same source, so there is
  nothing to argue about.
- **The narrative is visual and short**: flatten *succeeds*, structural then fails as a
  *singularity*, System Modeler says "Incompatible types". A misleading diagnosis is
  exactly the thing a phase-by-phase view makes obvious.
- **HRW's usefulness is the point of the story rather than an aside**, so nobody has to
  claim it.

`docs/fixture-tours/the-oracle.md` already walks this narrative and would want tightening
for a recording — a demo tour is a third kind after ad hoc and fixture: few stops, no
scrolling, deterministic start, nothing needing a second read.

---

## 1. `Session::remove_document` leaves a stale resolve failure in the resolved-state cache

**Severity:** high for any multi-document consumer. A model that resolves cleanly can be
reported as failing, **with a different file's error**.

**Found:** 2026-07-29, auditing HRW's front-end failure payloads.

### Reproduction

One `Session`, MSL loaded as a durable source root, then three compiles:

```rust
let mut session = Session::new(SessionConfig::default());
// ... load MSL via replace_parsed_source_set(.., SourceRootKind::DurableExternal, ..) ...

// 1. A model that resolves cleanly.
compile("CapacitorLoop.mo");     // resolve: OK

// 2. A model with an undefined reference.
compile("UndefinedRef.mo");      // resolve: "unresolved component reference: 'missingGain'"

// 3. The SAME clean model again.
compile("CapacitorLoop.mo");     // resolve: FAILS, reporting 'missingGain'
```

Each `compile` does `session.remove_document(uri); session.update_document(uri, src);` for
the file being compiled, then `session.resolved()`.

`UndefinedRef.mo` is the only file containing the identifier `missingGain`.

### Expected

Step 3 resolves cleanly, as step 1 did.

### Actual

Step 3 fails, reporting `unresolved component reference: 'missingGain'` — **byte-identical
to step 2's error**, including the ~33 accompanying MSL warnings. The identical text
suggests a cached result is being returned rather than a fresh resolution.

Removing the previous document does **not** help. Rebuilding the `Session` from scratch
does.

### Why this is surprising

`remove_document` → `apply_document_removal_at_revision`
(`crates/rumoca-compile/src/session/session_impl_inputs.rs:139`) **does** call
`invalidate_resolved_state(CacheInvalidationCause::DocumentRemoval)`. So invalidation is
attempted and something survives it.

Suspects, unverified: the `query_state.resolved.builds` cache read in
`build_resolved_with_diagnostics_inner`
(`crates/rumoca-compile/src/session/session_impl.rs:373`) — the `Standard`-mode branch
returns a cached tree and `record_standard_resolved_cache_hit()`; and
`restore_detached_source_root_document`, called during removal, which may put back a
document the caller meant to drop.

### Impact on consumers

Any tool that compiles several models in one session — an IDE, a language server, a batch
compiler, HRW — will attribute one file's resolve error to another. There is no way for the
consumer to tell, because the returned error looks exactly like a genuine failure of the
model it asked about.

### Workaround in HRW

Rebuild the session when the previous compile failed to resolve. Guarded so the library
reparse is only paid after an actual failure. See `WorkerState::last_resolve_failed` and
the regression test `a_broken_specimen_does_not_poison_the_next_compile`.

---

## 2. `validate_type_compatibility` does not fire for connectors with differing member sets

**Severity:** medium. An invalid model is accepted, and the resulting failure is reported
at the wrong phase with a misleading diagnosis.

**Found:** 2026-07-29, authoring a specimen to exercise the flatten failure path.
**Adjudicated by System Modeler**, not argued from the spec.

### Reproduction

```modelica
model IncompatibleConnect
  connector PinA
    Real v;
    flow Real i;
  end PinA;

  connector PinB
    Real v;
  end PinB;

  PinA a;
  PinB b;
equation
  connect(a, b);
end IncompatibleConnect;
```

Compile with `FlattenOptions { strict_connection_validation: true, .. }` — the setting
`rumoca_compile`'s own `flatten_options_for_tree()` uses.

### Expected

Rejected at flatten as a connector type-compatibility error. **MLS §9.3** requires
connected connectors to be type-compatible, and `PinA` and `PinB` have different member
sets.

### Actual

Flatten **succeeds**. The model then fails at structural analysis as *structurally
singular* — a misleading diagnosis for what is a type error at the `connect()`. A user is
sent to look at their equations when the problem is one line of wiring.

### Independent confirmation

Wolfram System Modeler 15.0 **rejects** the same source:

```
SystemModelSimulate::bld:  Failed to build model "IncompatibleConnect".
SystemModelSimulate::bldl: "Error": "Incompatible types. 'a ...  'b' has type 'PinB'."
```

So the model is genuinely invalid and Rumoca is the outlier.

### Note: the check exists and did not fire

`validate_type_compatibility` is at
`crates/rumoca-phase-flatten/src/connections/mod.rs:671`, and `validate_connections`
reaches it when `strict_connection_validation` is on. So this is **not a missing
validation** — it is one that did not trigger for this input.

Suspects, unverified: `get_validation_var_info` returning `None` for one side (the
validation is skipped rather than failed when info is missing), or `canonical_type_id`
mapping both connector types to the same root via `type_roots`.

`validate_expanded_connector_connection` (same file, ~line 735) may also be the intended
home for a member-set comparison that is not happening.

---

## Adding to this file

One entry per bug, and only for bugs **reproduced**, not suspected. Include the
reproduction, expected vs actual, and the suspect code location — but mark suspicions as
unverified, because a confident wrong diagnosis in a bug report wastes a maintainer's time
and costs credibility that this project is trying to build.

Where an independent implementation can adjudicate, **use it before filing** (see
`docs/ideas.md` #43 for the System Modeler recipe). "System Modeler rejects this and you
accept it" is a far stronger report than "I think the spec says…".
