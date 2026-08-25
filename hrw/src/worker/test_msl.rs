use super::{FromWorker, WorkerState};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

/// Returns the three MSL (Modelica Standard Library) root paths needed
/// to compile any specimen that uses standard components.
pub(crate) fn msl_roots() -> Vec<PathBuf> {
    let base = concat!(env!("CARGO_MANIFEST_DIR"), "/vendor/msl");
    vec![
        PathBuf::from(format!("{base}/Modelica 4.1.0")),
        PathBuf::from(format!("{base}/ModelicaServices 4.1.0")),
        PathBuf::from(format!("{base}/Complex.mo")),
    ]
}

/// One MSL-loaded worker, built once and shared across the worker tests behind
/// a mutex. Each test needs the full MSL (~430MB resolved); loading it per-test
/// OOMs / thrashes when cargo runs them in parallel. So tests lock this shared,
/// already-loaded worker and run serially against it — MSL is parsed once, and
/// peak memory stays at a single session.
///
/// # Why `&'static Mutex<WorkerState>`?
///
/// - `&'static` — the returned reference lives for the entire program lifetime
///   (it's backed by a `static` variable, not the stack).
/// - `OnceLock::new()` — creates an uninitialized lock. `get_or_init()` lazily
///   initializes it on first access, thread-safely. Subsequent calls return the
///   same value without re-running the init closure.
/// - `Mutex<WorkerState>` — wraps the worker in a mutex so only one test at a
///   time can access it. `lock().unwrap()` blocks until the mutex is available.
///
/// # Why tests run serially
///
/// `cargo test` runs test functions in parallel by default. Without the mutex,
/// multiple tests would try to use the `Session` concurrently (which isn't
/// thread-safe). The mutex serializes access.
///
/// **The session holds at most ONE specimen document at a time**, not one per
/// specimen the suite has touched: `compile_target` removes the previous
/// specimen before registering the next (grep `last_specimen_uri`). This said
/// "accumulates each specimen's document (distinct URIs)" until 2026-08-21,
/// which stopped being true when that removal was added — checked now by
/// `the_session_holds_at_most_one_specimen_document` rather than asserted.
///
/// **What a compile still inherits from the session's history is its resolved
/// state**, which is why compiling the same specimen at two points in one
/// session can differ — see `compiling_a_specimen_twice_is_reproducible`.
/// `pub(super)`, deliberately not `pub(crate)`: it hands out `WorkerState`,
/// which is private to `worker`, so a wider visibility would leak a private
/// type. `worker::tests` needs it directly for the tests that drive the
/// worker rather than just read a compile; everything outside `worker` goes
/// through [`compile_specimen_shared`], which returns only the result. The
/// narrower door is the one worth opening.
pub(super) fn shared_worker() -> &'static Mutex<WorkerState> {
    static WORKER: OnceLock<Mutex<WorkerState>> = OnceLock::new();
    WORKER.get_or_init(|| {
        let mut state = WorkerState::new();
        state
            .load_libraries(msl_roots())
            .expect("load MSL once for tests");
        Mutex::new(state)
    })
}

/// Compile `specimens/<name>.mo` against the shared MSL worker.
///
/// `unwrap_or_else(|e| e.into_inner())` — if a previous test panicked while
/// holding the mutex, the mutex is "poisoned" (marked as potentially in an
/// inconsistent state). `into_inner()` recovers from the poison by taking
/// the inner value anyway — we accept the risk because our WorkerState is
/// still usable after a panic (it's just a Session, not half-modified data).
pub(crate) fn compile_specimen_shared(name: &str) -> FromWorker {
    if let Some(hit) = specimen_cache()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(name)
    {
        return hit.clone();
    }
    // **Compile outside the cache lock.** Holding it across a compile would
    // mean holding two locks in a fixed order for tens of seconds; harmless
    // under `--test-threads=1` but a deadlock waiting for the day that
    // changes. A duplicate compile on a race is wasted work, never wrong.
    let fresh = compile_specimen_uncached(name);
    specimen_cache()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(name.to_owned(), fresh.clone());
    fresh
}

/// The library-model sibling of [`compile_specimen_shared`].
///
/// **The same asymmetry that hid the simulate bug**, one layer down: the test
/// helpers could reach a specimen by path and had no way to reach a corpus model
/// by name, so every test that wanted one had to build a `WorkerState` by hand —
/// and most simply did not. Added 2026-08-05 when a test of F10's absence clause
/// needed a **partial class**, a shape no specimen has and 350 corpus entries do.
///
/// Shares the specimen cache, keyed by qualified name; the two namespaces cannot
/// collide because a specimen name never contains a dot.
pub(crate) fn compile_library_shared(qualified: &str) -> FromWorker {
    if let Some(hit) = specimen_cache()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(qualified)
    {
        return hit.clone();
    }
    let fresh = {
        let mut w = shared_worker().lock().unwrap_or_else(|e| e.into_inner());
        w.compile_model_by_name(qualified, &|_: FromWorker| {})
    };
    specimen_cache()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(qualified.to_owned(), fresh.clone());
    fresh
}

/// One compile per specimen per test process. See [`compile_specimen_shared`].
fn specimen_cache() -> &'static Mutex<std::collections::HashMap<String, FromWorker>> {
    static CACHE: OnceLock<Mutex<std::collections::HashMap<String, FromWorker>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

/// Compile a specimen **without** consulting the memo — a genuinely fresh
/// run through every phase.
///
/// # When a test must use this
///
/// **Any test whose subject is the act of compiling**, rather than the
/// result. Two kinds exist today:
///
/// - **Cross-compile contamination.**
///   `a_broken_specimen_does_not_poison_the_next_compile` exists because a
///   failed resolve once leaked into the *next* model's result. A memoised
///   answer would never touch the session and the test would pass
///   vacuously — proving nothing while looking green, which is worse than
///   deleting it.
/// - **Reproducibility.** `compiling_a_specimen_twice_is_reproducible` is
///   the mitigation for what memoisation costs: the second test to ask for
///   `Drivetrain` no longer re-verifies that compiling it is deterministic,
///   so one test keeps doing exactly that.
pub(crate) fn compile_specimen_uncached(name: &str) -> FromWorker {
    let path = PathBuf::from(format!(
        "{}/specimens/{name}.mo",
        env!("CARGO_MANIFEST_DIR")
    ));
    let mut w = shared_worker().lock().unwrap_or_else(|e| e.into_inner());
    w.compile(&path, &|_: FromWorker| {})
}
