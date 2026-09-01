use super::test_msl::*;
use super::*;
// `Mutex` — a mutual exclusion lock. Wrapping `WorkerState` in `Mutex`
// lets multiple test functions share it safely, but only one can access
// it at a time (the others block). This is how the tests run serially
// against a single MSL-loaded session.
//
// `OnceLock` — a thread-safe cell that can be written to exactly once.
// Used here for lazy one-time initialization of the shared worker.
// `OnceLock` is the thread-safe equivalent of `OnceCell` (or Python's
// `functools.lru_cache` with `maxsize=1` conceptually).

/// End-to-end: after resolving `RotationalInertia` against the MSL, the
/// component *types* (`type_def_id`) must resolve to their MSL classes.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn resolves_def_ids_against_msl() {
    let FromWorker::Compiled {
        def_index, stages, ..
    } = compile_specimen_shared("RotationalInertia")
    else {
        panic!("expected Compiled");
    };
    assert!(
        stages.resolve.value.is_some(),
        "resolve failed: {:?}",
        stages.resolve.note
    );
    assert!(!def_index.is_empty(), "no DefIds resolved");

    let names: Vec<&str> = def_index.values().map(|d| d.name.as_str()).collect();
    // The three declared component types resolved to their MSL classes.
    for expected in [
        "Mechanics.Rotational.Components.Inertia",
        "Mechanics.Rotational.Sources.Torque",
        "Blocks.Sources.Constant",
    ] {
        assert!(
            def_index
                .values()
                .any(|d| d.kind == DefKind::Class && d.name.ends_with(expected)),
            "{expected} not resolved as a class; got {names:?}"
        );
    }
}

/// Navigation: after compiling the specimen, opening a class the model
/// points at (the MSL `Inertia`) returns its IR and its own DefId index.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn open_def_extracts_a_navigated_class() {
    let name = "Modelica.Mechanics.Rotational.Components.Inertia";
    let result = {
        let mut w = shared_worker().lock().unwrap_or_else(|e| e.into_inner());
        let path = Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/specimens/RotationalInertia.mo"
        ));
        w.compile(path, &|_: FromWorker| {}); // register the specimen document
        let FromWorker::DefTree { result, .. } = w.open_def(name) else {
            panic!("expected DefTree");
        };
        result
    };
    let (value, def_index) = result.expect("Inertia class extracted");
    // It's a class body with a name matching Inertia.
    assert_eq!(value["name"]["text"], serde_json::json!("Inertia"));
    // Its own references resolved, so navigation can continue from here.
    assert!(
        !def_index.is_empty(),
        "navigated class has no resolved DefIds"
    );
}

/// The drivetrain specimen compiles through the whole pipeline (it
/// crosses electrical → rotational → translational, so this exercises
/// connector expansion / flow-sum generation across domains).
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn drivetrain_compiles_through_flatten() {
    let FromWorker::Compiled { model, stages, .. } = compile_specimen_shared("Drivetrain") else {
        panic!("expected Compiled");
    };
    assert_eq!(model.as_deref(), Some("Drivetrain"));
    assert!(
        stages.flatten.value.is_some(),
        "Drivetrain did not flatten: {:?}",
        stages.flatten.note
    );
}

/// The structural stage builds a matching + BLT report for an index-1 model.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn structural_report_for_rotational_inertia() {
    let FromWorker::Compiled { stages, .. } = compile_specimen_shared("RotationalInertia") else {
        panic!("expected Compiled");
    };
    let v = stages.structural.value.expect("structural report");
    assert!(
        v["matching"].as_array().is_some_and(|a| !a.is_empty()),
        "no matching"
    );
    assert!(
        v["blocks"].as_array().is_some_and(|a| !a.is_empty()),
        "no BLT blocks"
    );
    // A plain index-1 ODE sorts into scalar blocks only — no algebraic loop.
    assert_eq!(
        v["coupled_block_count"],
        serde_json::json!(0),
        "unexpected coupled block"
    );
}

/// The proportional-loop specimen closes an algebraic feedback loop, so
/// structural analysis MUST report a coupled block (a simultaneous algebraic
/// SCC) — the case the BLT spy-plot draws as a box. This is the specimen's
/// whole reason for existing, so guard it.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn proportional_loop_has_a_coupled_block() {
    let FromWorker::Compiled { stages, .. } = compile_specimen_shared("ProportionalLoop") else {
        panic!("expected Compiled");
    };
    let v = stages
        .structural
        .value
        .unwrap_or_else(|| panic!("no structural report: {:?}", stages.structural.note));
    let count = v["coupled_block_count"].as_u64().unwrap_or(0);
    assert!(
        count >= 1,
        "expected a coupled algebraic block, got {count}; blocks = {}",
        v["blocks"]
    );
    // The coupled block should carry a tearing report (iteration variable(s)).
    let coupled = v["blocks"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|b| b["kind"] == serde_json::json!("coupled"))
        .expect("a coupled block");
    assert!(
        coupled["size"].as_u64().unwrap_or(0) >= 2,
        "coupled block must be size >= 2"
    );
}

/// Compile a `specimens/<name>.mo` against the MSL and return its structural
/// report JSON — shared by the block-structure guards below.
fn structural_report_for(name: &str) -> serde_json::Value {
    let FromWorker::Compiled { stages, .. } = compile_specimen_shared(name) else {
        panic!("expected Compiled");
    };
    stages.structural.value.unwrap_or_else(|| {
        panic!(
            "no structural report for {name}: {:?}",
            stages.structural.note
        )
    })
}

fn block_kinds(v: &serde_json::Value) -> Vec<String> {
    v["blocks"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|b| b["kind"].as_str().map(str::to_owned))
        .collect()
}

/// MixedLoop brackets an algebraic loop with scalar solves, so its BLT must
/// contain BOTH scalar and coupled blocks — the mixed spy-plot case.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn mixed_loop_has_scalar_and_coupled_blocks() {
    let v = structural_report_for("MixedLoop");
    assert_eq!(v["coupled_block_count"], serde_json::json!(1));
    let kinds = block_kinds(&v);
    assert!(
        kinds.iter().any(|k| k == "scalar") && kinds.iter().any(|k| k == "coupled"),
        "expected mixed scalar + coupled blocks, got {kinds:?}"
    );
}

/// TwoLoops chains two algebraic loops, so structural analysis must report
/// TWO coupled blocks (two orange boxes).
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn two_loops_has_two_coupled_blocks() {
    let v = structural_report_for("TwoLoops");
    assert_eq!(v["coupled_block_count"], serde_json::json!(2));
}

/// NonlinearLoop is structurally identical to ProportionalLoop (structure is
/// blind to the nonlinearity) — still one coupled block.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn nonlinear_loop_has_a_coupled_block() {
    let v = structural_report_for("NonlinearLoop");
    assert_eq!(v["coupled_block_count"], serde_json::json!(1));
}

/// The `dae_prepare` funnel (mirroring rumoca-sim's internal
/// `prepare_dae_for_structural_analysis` — the shared prep the simulator and
/// `--inspect structure` both run) reduces Drivetrain's **singular, high-index**
/// DAE to a non-singular, structurally analyzable one. This confirms Rumoca can
/// index-reduce (not blocked-on-upstream) and pins the exact public API the
/// observatory stage will call. NOTE: HRW mirrors Rumoca's funnel *order*;
/// re-verify it against `rumoca-sim/src/solve_lowering/structural_lowering.rs`
/// on a pin bump.
#[test]
fn drivetrain_index_reduces_from_singular_to_solvable() {
    let report = {
        let mut w = shared_worker().lock().unwrap_or_else(|e| e.into_inner());
        let path = Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/specimens/Drivetrain.mo"
        ));
        let source = std::fs::read_to_string(path).unwrap();
        let uri = path.to_string_lossy().to_string();
        w.session.update_document(&uri, &source);
        let qualified = w.session.qualify_model_name(&uri, "Drivetrain");
        w.session
            .compile_model_strict_reachable_with_recovery(&qualified)
    };
    let cr = match report.requested_result.as_ref() {
        Some(PhaseResult::Success(cr)) => cr,
        _ => panic!("expected a Success result for Drivetrain"),
    };
    // Before: the raw DAE is structurally singular (high index).
    let before = rumoca_phase_structural::build_structural_report(&cr.dae);
    assert!(
        before.is_err(),
        "expected Drivetrain to start singular, got {before:?}"
    );

    // Apply the index-reduction funnel, then re-analyze.
    let mut reduced = cr.dae.clone();
    index_reduce_for_structural_analysis(&mut reduced);
    let after = rumoca_phase_structural::build_structural_report(&reduced);
    assert!(
        after.is_ok(),
        "index reduction should make Drivetrain structurally analyzable, got {after:?}"
    );
}

/// Blow-up: a capacitor directly across an ideal source can't be
/// consistently initialized — its state voltage is pinned to the source. Unlike
/// Drivetrain, index reduction can NOT rescue it: both Structural and Index
/// reduction stay singular (an observable initialization blow-up).
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn capacitor_loop_is_singular_and_irreducible() {
    let FromWorker::Compiled { stages, .. } = compile_specimen_shared("CapacitorLoop") else {
        panic!("expected Compiled");
    };
    assert!(
        stages.flatten.value.is_some(),
        "CapacitorLoop should still flatten"
    );
    assert!(
        stages.structural.note_is_error(),
        "expected singular Structural"
    );
    assert!(
        stages
            .structural
            .value
            .as_ref()
            .unwrap()
            .get("error")
            .is_some(),
        "singular Structural should carry error details"
    );
    assert!(
        stages.index_reduction.note_is_error(),
        "index reduction should NOT rescue a capacitor-across-source loop"
    );
    assert!(
        stages
            .index_reduction
            .value
            .as_ref()
            .unwrap()
            .get("error")
            .is_some(),
        "irreducible index reduction should carry error details"
    );
}

/// The Initialization stage plans a consistent initial state for the RC
/// circuit — a non-empty IC plan plus the ground-current relaxation hint.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn rc_circuit_has_an_ic_plan() {
    let FromWorker::Compiled { stages, .. } = compile_specimen_shared("RcCircuit") else {
        panic!("expected Compiled");
    };
    let v = stages
        .initialization
        .value
        .unwrap_or_else(|| panic!("no IC plan: {:?}", stages.initialization.note));
    assert!(
        v["block_count"].as_u64().unwrap_or(0) >= 1,
        "expected a non-empty IC plan"
    );
    assert!(
        v["relaxation_hint"].is_object(),
        "expected a relaxation hint (ground-current redundancy)"
    );
    // Well-posed init must NOT be mis-flagged as over-determined (idea #6).
    assert_ne!(
        v["determinacy"]["verdict"],
        serde_json::json!("over-determined")
    );
}

/// Idea #6: over-specified initialization is flagged. `OverInitRc` pins the
/// capacitor voltage twice (`C.v = 0` and `der(C.v) = 0`), so the
/// Initialization stage reports an over-determined init (surplus > 0) with a
/// red note — the pure init blow-up `build_ic_plan` alone doesn't catch.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn over_init_rc_is_flagged_over_determined() {
    let FromWorker::Compiled { stages, .. } = compile_specimen_shared("OverInitRc") else {
        panic!("expected Compiled");
    };
    let init = &stages.initialization;
    let v = init.value.as_ref().expect("IC plan");
    assert_eq!(
        v["determinacy"]["verdict"],
        serde_json::json!("over-determined")
    );
    assert!(
        v["determinacy"]["surplus_over_states"]
            .as_i64()
            .unwrap_or(0)
            >= 1
    );
    // `Flagged`, not `Failed` — and the distinction is the point of the enum.
    // The IC plan above is real; Rumoca simply also reported that it is
    // over-determined. Asserting `note_is_error()` here would pass equally for
    // a stage that produced nothing at all.
    assert_eq!(
        init.outcome,
        Outcome::Flagged,
        "over-determined init is flagged, not failed"
    );
}

/// HRW can RUN a model, not just inspect it. Lower
/// `SingleInertia`'s DAE to a `SolveModel` and simulate it, checking the
/// trajectory is produced AND numerically right: constant torque tau=1 with
/// J=1 gives der(w)=1, so w(t)=t and w(2) is ~2.
#[test]
fn single_inertia_simulates_to_a_correct_trajectory() {
    let report = {
        let mut w = shared_worker().lock().unwrap_or_else(|e| e.into_inner());
        let path = Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/specimens/SingleInertia.mo"
        ));
        let src = std::fs::read_to_string(path).unwrap();
        let uri = path.to_string_lossy().to_string();
        w.session.update_document(&uri, &src);
        let q = w.session.qualify_model_name(&uri, "SingleInertia");
        w.session.compile_model_strict_reachable_with_recovery(&q)
    };
    let cr = match report.requested_result.as_ref() {
        Some(PhaseResult::Success(cr)) => cr,
        _ => panic!("expected Success for SingleInertia"),
    };
    let sm =
        rumoca_phase_solve::lower_dae_to_solve_model(&cr.dae).expect("lower DAE -> SolveModel");
    let opts = rumoca_sim::SimOptions {
        t_end: 2.0,
        ..Default::default()
    };
    let result = rumoca_sim::simulate_solve_model(&sm, &opts).expect("simulate");

    assert!(
        result.times.last().copied().unwrap_or(0.0) >= 1.99,
        "should integrate to t_end"
    );
    let w_idx = result
        .names
        .iter()
        .position(|n| n == "w")
        .expect("w in outputs");
    assert_eq!(
        result.data[w_idx].len(),
        result.times.len(),
        "trajectory length = time points"
    );
    let w_final = *result.data[w_idx].last().unwrap();
    assert!(
        (w_final - 2.0).abs() < 0.05,
        "w(2) should be ~2.0 (constant torque), got {w_final}"
    );
}

/// The stiff bench actuator (a DC motor spinning up an inertial
/// load) simulates — the Auto solver (BDF) copes with the ~1000x separation
/// between the fast winding (L/R ~ 1e-4 s) and the slow rotor (J = 0.05). The
/// current is driven high and the load spins up.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn bench_actuator_simulates_stiff_spinup() {
    let d = {
        let mut w = shared_worker().lock().unwrap_or_else(|e| e.into_inner());
        let path = Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/specimens/BenchActuator.mo"
        ));
        w.simulate(
            CompileTarget::File(path),
            "BenchActuator",
            0.5,
            &|_: FromWorker| {},
        )
    }
    .expect("simulate BenchActuator");
    let get = |name: &str| -> f64 {
        let i = d
            .names
            .iter()
            .position(|n| n == name)
            .unwrap_or_else(|| panic!("{name} in outputs"));
        *d.data[i].last().unwrap()
    };
    assert!(get("L.i") > 5.0, "winding current should be driven high");
    assert!(get("load.w") > 1.0, "the load should spin up");
    // Smooth trajectories: BenchActuator has a bare zero-crossing but no
    // discrete update, so the plot must never break its (coarsely sampled,
    // steep) current spike into false discontinuities.
    assert!(
        !d.has_discontinuities,
        "BenchActuator has no discrete updates — all trajectories continuous"
    );
}

/// The discontinuity-plotting helper. A smooth ramp is one segment;
/// a signal with a reinit-style jump splits into two, breaking at the jump so
/// the plot won't slope a line across it. Calibrated against BouncingBall's `v`
/// (smooth step ~0.06, bounce jump ~8 — a ~40x separation).
#[test]
fn discontinuity_segments_break_at_jumps() {
    // Smooth monotone ramp → a single segment.
    let ramp: Vec<f64> = (0..50).map(|i| f64::from(i) * 0.1).collect();
    assert_eq!(discontinuity_segments(&ramp), vec![0..50]);
    // A falling ramp that flips sign once (like a single bounce) → two segments,
    // split right at the jump.
    let mut v: Vec<f64> = (0..40).map(|i| -f64::from(i) * 0.1).collect(); // 0 → -3.9
    v.extend((0..40).map(|i| 3.0 - f64::from(i) * 0.1)); // jumps -3.9 → +3.0
    let segs = discontinuity_segments(&v);
    assert_eq!(segs.len(), 2, "one jump → two segments, got {segs:?}");
    assert_eq!(segs[0], 0..40, "first segment ends at the pre-jump sample");
    assert_eq!(
        segs[1],
        40..80,
        "second segment starts at the post-jump sample"
    );
}

/// End-to-end: BouncingBall is hybrid, and its velocity trajectory
/// breaks into several segments (one per bounce) while its height stays one
/// continuous curve.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn bouncing_ball_velocity_plots_as_discontinuous() {
    let data = {
        let mut w = shared_worker().lock().unwrap_or_else(|e| e.into_inner());
        let path = Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/specimens/BouncingBall.mo"
        ));
        w.simulate(
            CompileTarget::File(path),
            "BouncingBall",
            3.0,
            &|_: FromWorker| {},
        )
    }
    .expect("simulate BouncingBall");
    assert!(
        data.has_discontinuities,
        "BouncingBall reinits v at each bounce"
    );
    let v = &data.data[data.names.iter().position(|n| n == "v").expect("v")];
    let h = &data.data[data.names.iter().position(|n| n == "h").expect("h")];
    assert!(
        discontinuity_segments(v).len() > 1,
        "velocity flips at each bounce → multiple segments"
    );
    assert_eq!(
        discontinuity_segments(h).len(),
        1,
        "height is continuous across bounces → one segment"
    );
}

/// The worker's `simulate` path (compile → lower → integrate) runs a
/// hybrid model — BouncingBall — and returns trajectories. Exercises event
/// handling in the solver (the ball must stay ~above the floor).
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn worker_simulate_runs_bouncing_ball() {
    let data = {
        let mut w = shared_worker().lock().unwrap_or_else(|e| e.into_inner());
        let path = Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/specimens/BouncingBall.mo"
        ));
        w.simulate(
            CompileTarget::File(path),
            "BouncingBall",
            3.0,
            &|_: FromWorker| {},
        )
    }
    .expect("simulate BouncingBall");
    assert!(!data.times.is_empty(), "should produce a trajectory");
    let h_idx = data
        .names
        .iter()
        .position(|n| n == "h")
        .expect("h in outputs");
    assert!(
        data.data[h_idx].iter().all(|&h| h > -0.5),
        "the ball should stay ~above the floor (events reflect it)"
    );
}

/// The Solve-lowering stage (phase 8) lowers the DAE to a `SolveModel`
/// (the solvable form the simulator consumes) and renders it.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn single_inertia_lowers_to_a_solve_model() {
    let FromWorker::Compiled { stages, .. } = compile_specimen_shared("SingleInertia") else {
        panic!("expected Compiled");
    };
    let v = stages.solve_lowering.value.expect("SolveModel IR");
    assert!(
        v.get("problem").is_some(),
        "SolveModel should carry the solve problem"
    );
    assert!(
        v.get("variable_meta").is_some(),
        "SolveModel should carry variable metadata"
    );
}

/// BouncingBall is a hybrid model — the Events stage reports its
/// condition (`h <= 0`) + discrete update (the `reinit`). A smooth model
/// (SingleInertia) reports none.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn bouncing_ball_has_events_smooth_model_has_none() {
    let total_events = |v: &serde_json::Value| -> u64 {
        v["summary"]
            .as_object()
            .into_iter()
            .flatten()
            .filter_map(|(_, x)| x.as_u64())
            .sum()
    };
    let FromWorker::Compiled { stages, .. } = compile_specimen_shared("BouncingBall") else {
        panic!("expected Compiled");
    };
    let v = stages.events.value.expect("events IR");
    assert!(
        total_events(&v) >= 1,
        "BouncingBall should have hybrid structure"
    );
    assert!(
        v["discrete_updates"]["real_updates_f_z"]
            .as_array()
            .is_some_and(|a| !a.is_empty()),
        "expected the reinit as a discrete real update"
    );

    let FromWorker::Compiled {
        stages: smooth_stages,
        ..
    } = compile_specimen_shared("SingleInertia")
    else {
        panic!("expected Compiled");
    };
    assert_eq!(
        total_events(&smooth_stages.events.value.expect("events IR")),
        0,
        "SingleInertia is smooth"
    );
}

/// The parked hand-built PlanarMechanics library (the four-bar-linkage
/// prerequisite, deferred until Rumoca's Rust-path reduction handles nonlinear
/// holonomic constraints — see DECISIONS.md) still parses as a source root, so
/// it doesn't bit-rot while deferred.
#[test]
fn planar_mechanics_library_parses() {
    let roots = vec![PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/lib/PlanarMechanics.mo"
    ))];
    let mut state = WorkerState::new();
    let loaded = state
        .load_libraries(roots)
        .expect("planar mechanics library should parse");
    assert!(loaded >= 1, "expected the planar mechanics library to load");
}

/// Asking [`parsed_source_root`] twice returns **the same documents** both times.
///
/// This covers the real wiring — the fingerprint, the parser and the memo
/// composed as `load_libraries` calls them — on a small library rather than the
/// MSL, so it runs in the fast suite.
///
/// **What it deliberately does NOT check, measured rather than assumed:** it
/// cannot tell a memo hit from a re-parse. Both perturbations used to verify
/// this file's memo (ignore the fingerprint; never store) leave this test
/// **green**, because either way the documents come back equal. The claim that
/// the memo actually memoises, and honours a changed fingerprint, belongs to
/// [`a_changed_fingerprint_defeats_the_memo`], which counts parses.
#[test]
fn a_memoised_source_root_parse_returns_the_same_documents() {
    let root = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/lib/PlanarMechanics.mo"
    ));
    let first = parsed_source_root(&root).expect("first parse");
    let second = parsed_source_root(&root).expect("memoised parse");

    assert!(
        !first.is_empty(),
        "the library parsed to no documents at all"
    );
    assert_eq!(
        first.len(),
        second.len(),
        "the memo returned a different number of documents",
    );
    let uris = |docs: &[(String, StoredDefinition)]| {
        docs.iter().map(|(uri, _)| uri.clone()).collect::<Vec<_>>()
    };
    assert_eq!(uris(&first), uris(&second), "the memo returned other files");
    // The documents themselves, not just their names: a memo that handed back
    // an empty or truncated AST would pass every assertion above.
    assert_eq!(
        serde_json::to_string(&first).expect("serialize first parse"),
        serde_json::to_string(&second).expect("serialize memoised parse"),
        "the memo returned documents that differ from the parse",
    );
}

/// **The session holds at most one specimen document, however many are compiled.**
///
/// `compile_target` removes the previous specimen before registering the next
/// (`last_specimen_uri`), so the shared test session does not fill up with every
/// specimen the suite has touched. Two doc comments claimed the opposite until
/// 2026-08-21 — one of them explaining *why* a real test had to be restructured
/// — and nothing could notice, because the claim was prose.
///
/// **Uses a bare `WorkerState` with no libraries loaded**, so it costs ~0.06 s:
/// the property is about document bookkeeping and needs no MSL. That is also the
/// control `docs/ideas.md` #48 measured a compile at 0.03 s in.
#[test]
fn the_session_holds_at_most_one_specimen_document() {
    let mut w = WorkerState::new();
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/specimens");
    // Three MSL-free specimens, so a bare session can resolve them.
    let names = ["SingleInertia", "BouncingBall", "TwoLoops"];
    let mut seen_uris = Vec::new();
    for name in names {
        let path = PathBuf::from(format!("{dir}/{name}.mo"));
        let _ = w.compile(&path, &|_: FromWorker| {});
        let held = w.specimen_document_uris();
        assert_eq!(
            held.len(),
            1,
            "after compiling {name} the session held {} specimen documents, not 1: {held:?}",
            held.len(),
        );
        seen_uris.push(held[0].clone());
    }
    // Non-vacuity: each compile must actually have registered *its own* file,
    // or "exactly one" would be satisfied by never replacing the first.
    assert_eq!(
        seen_uris.len(),
        3,
        "expected one registered URI per compile",
    );
    for (name, uri) in names.iter().zip(&seen_uris) {
        assert!(
            uri.contains(name),
            "compiling {name} left {uri} registered instead",
        );
    }
}

/// **A changed fingerprint must defeat the memo — the must-fire half.**
///
/// The memo's entire safety argument is that it serves a stored value *only*
/// when the fingerprint matches. A test that always asks for the same bytes
/// would pass with the comparison deleted, so this asks for a different
/// fingerprint and demands the parser run again.
///
/// **Tests the bookkeeping against a counting fake parser, not the real one**,
/// because a real edit is an artifact-cache miss and a miss costs ~21 s of
/// cache pruning — see [`memoised_by_fingerprint`] for the measurement.
#[test]
fn a_changed_fingerprint_defeats_the_memo() {
    let memo = Mutex::new(HashMap::new());
    let root = Path::new("some/library/root");
    // Counts parses, so "the memo served a stored value" is checked directly
    // rather than inferred from the answer being equal.
    let parses = std::cell::Cell::new(0);

    let first = memoised_by_fingerprint(&memo, root, "fingerprint-A", |_| {
        parses.set(parses.get() + 1);
        Ok("documents-A".to_owned())
    })
    .expect("first parse");
    assert_eq!(first, "documents-A");
    assert_eq!(parses.get(), 1, "the first call must parse");

    // Same fingerprint: the stored value, and the parser must NOT run. The fake
    // returns something else, so a memo that re-parsed would be visible twice —
    // in the count and in the answer.
    let hit = memoised_by_fingerprint(&memo, root, "fingerprint-A", |_| {
        parses.set(parses.get() + 1);
        Ok("documents-B".to_owned())
    })
    .expect("memo hit");
    assert_eq!(
        hit, "documents-A",
        "an unchanged root was re-parsed instead of served from the memo",
    );
    assert_eq!(parses.get(), 1, "a memo hit must not parse");

    // Changed fingerprint: the parser must run and its answer must win.
    let after = memoised_by_fingerprint(&memo, root, "fingerprint-B", |_| {
        parses.set(parses.get() + 1);
        Ok("documents-B".to_owned())
    })
    .expect("re-parse");
    assert_eq!(
        after, "documents-B",
        "a changed root was served the stale value: an edited library would be invisible",
    );
    assert_eq!(parses.get(), 2, "a changed fingerprint must parse again");

    // And the change must be STORED, not merely passed through — otherwise every
    // later call re-parses and the memo silently stops working after one edit.
    let restored = memoised_by_fingerprint(&memo, root, "fingerprint-B", |_| {
        parses.set(parses.get() + 1);
        Ok("documents-C".to_owned())
    })
    .expect("memo hit after re-parse");
    assert_eq!(
        restored, "documents-B",
        "the re-parse was not stored, so every later call re-parses",
    );
    assert_eq!(parses.get(), 2, "the re-parsed value must be memoised too");
}

/// **Clearing the memo cannot bound memory — only disabling it can.**
///
/// This encodes the reasoning error that
/// [`disable_parsed_source_root_memo`] exists to prevent, because the wrong
/// version looked obviously right: the fidelity sweep's rebuild point already
/// discards state to reclaim memory, so "clear the memo there too" reads as the
/// natural fix. It reclaims **nothing** — the reload immediately re-fills the
/// memo — and doing it the other way round is worse, since a memoised load
/// briefly holds two copies.
///
/// So: with the memo live, a load re-populates it. That is the fact that makes
/// clear-then-reload useless, and it is checked here rather than left as prose.
#[test]
fn a_load_repopulates_the_memo_so_clearing_it_first_reclaims_nothing() {
    let memo = Mutex::new(HashMap::new());
    let root = Path::new("some/library/root");

    memoised_by_fingerprint(&memo, root, "fingerprint-A", |_| {
        Ok("documents-A".to_owned())
    })
    .expect("first parse");
    assert_eq!(
        memo.lock().expect("memo").len(),
        1,
        "a parse must populate the memo, or there is nothing to bound",
    );

    // The sweep's tempting move: clear, then load again.
    memo.lock().expect("memo").clear();
    memoised_by_fingerprint(&memo, root, "fingerprint-A", |_| {
        Ok("documents-A".to_owned())
    })
    .expect("reload after clearing");
    assert_eq!(
        memo.lock().expect("memo").len(),
        1,
        "clearing before a load reclaims nothing: the load re-fills the memo, \
         which is why the sweep disables the memo instead of clearing it",
    );
}

/// The fingerprint the memo is keyed on **actually tracks file contents**.
///
/// [`a_changed_fingerprint_defeats_the_memo`] proves the memo honours a changed
/// fingerprint; this proves an edit *produces* one. Neither is sufficient alone
/// — together they are the claim "an edited library is not served stale".
///
/// Costs ~15 ms: computing the key never touches the artifact cache.
#[test]
fn editing_a_file_changes_its_source_root_fingerprint() {
    let dir = std::env::temp_dir().join(format!("hrw-fingerprint-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp source root");
    let file = dir.join("FingerprintProbe.mo");
    let key_of = |body: &str| {
        std::fs::write(&file, body).expect("write temp library");
        source_root_input_cache_key(&file).expect("fingerprint the temp root")
    };

    let before = key_of("model P\n  Real x;\nequation\n  x = 1;\nend P;\n");
    let unchanged = key_of("model P\n  Real x;\nequation\n  x = 1;\nend P;\n");
    let after = key_of("model P\n  Real y;\nequation\n  y = 2;\nend P;\n");
    std::fs::remove_dir_all(&dir).ok();

    assert_eq!(
        before, unchanged,
        "identical bytes produced different fingerprints, so the memo could never hit",
    );
    assert_ne!(
        before, after,
        "an edited file kept its fingerprint, so the memo would serve the old parse",
    );
}

/// For the high-index Drivetrain, the raw `structural` stage is singular
/// **and still produces IR**, and `index_reduction` then makes it solvable —
/// the before/after the two tabs show side by side.
///
/// The comment here used to say "singular (no IR)" while the line below
/// asserted the IR was there. That contradiction is the one
/// [`Outcome::Flagged`] exists to end.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn drivetrain_index_reduction_stage_recovers_singular() {
    let FromWorker::Compiled { stages, .. } = compile_specimen_shared("Drivetrain") else {
        panic!("expected Compiled");
    };
    assert_eq!(
        stages.structural.outcome,
        Outcome::Flagged,
        "raw Structural is singular for Drivetrain — flagged, not failed",
    );
    assert!(
        stages
            .structural
            .value
            .as_ref()
            .unwrap()
            .get("error")
            .is_some(),
        "singular Structural should carry error details"
    );
    let v = stages.index_reduction.value.unwrap_or_else(|| {
        panic!(
            "index reduction should recover Drivetrain: {:?}",
            stages.index_reduction.note
        )
    });
    assert!(
        v["coupled_block_count"].as_u64().is_some(),
        "reduced report missing block count"
    );
    let red = &v["reduction"];
    assert!(
        red["funnel_completed"].as_bool() == Some(true),
        "funnel should complete for Drivetrain"
    );
    let steps = red["steps"].as_array().expect("steps array");
    assert!(!steps.is_empty(), "should have logged funnel steps");
    assert!(red["n_states_before"].as_u64().unwrap() > 0);
}

/// **Every constructor maps to exactly one outcome, and `note_is_error()`
/// still says what the old boolean field said.**
///
/// The second half is what makes this split safe to land: it changed no
/// colour and no control flow, because every former reader of the field now
/// calls the method and sees the identical answer. Only code that asks for
/// [`Stage::outcome`] can tell `Flagged` from `Failed`.
#[test]
fn each_constructor_reaches_one_outcome_and_colour_is_unchanged() {
    let v = || serde_json::json!({ "ir": true });
    let cases = [
        (Stage::ok(v()), Outcome::Ok, false),
        (
            Stage::ok_with_note(v(), "already index-1"),
            Outcome::Ok,
            false,
        ),
        (Stage::info("not reached"), Outcome::Ok, false),
        (Stage::recovered(v(), "singular"), Outcome::Flagged, true),
        (Stage::err("boom"), Outcome::Failed, true),
        (
            Stage::err_with_details(serde_json::json!({"kind": "singular"}), "boom"),
            Outcome::Failed,
            true,
        ),
    ];
    for (stage, want, red) in cases {
        assert_eq!(stage.outcome, want, "note: {:?}", stage.note);
        assert_eq!(
            stage.note_is_error(),
            red,
            "colour must match the pre-split boolean for {want:?}",
        );
    }

    // `recovered` keeps the caller's IR; `err_with_details` replaces it with
    // the error payload. Same JSON *shape*, opposite meaning — the conflation
    // that motivated the enum.
    assert_eq!(
        Stage::recovered(v(), "n").value.unwrap()["ir"],
        serde_json::json!(true)
    );
    assert!(Stage::err_with_details(v(), "n").error_json().is_some());
    assert!(
        Stage::ok(v()).error_json().is_none(),
        "a clean stage carries no error payload"
    );
}

/// **The miscount, pinned.** `Drivetrain` compiles all the way through, yet
/// two of its stages set the old `note_is_error` flag — so a census counting
/// that boolean would report a healthy high-index model as broken.
///
/// This is not hypothetical: it produced a false finding on 2026-07-29
/// (`docs/ideas.md` #51), which is why `docs/fidelity-plan.md` sequences the
/// three-way split ahead of any harness that counts outcomes at scale.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn a_healthy_high_index_compile_has_no_failed_stage() {
    let FromWorker::Compiled { stages, .. } = compile_specimen_shared("Drivetrain") else {
        panic!("expected Compiled");
    };

    let failed: Vec<_> = StageKind::COMPILATION
        .iter()
        .filter(|&&k| stages.get(k).outcome == Outcome::Failed)
        .map(|&k| (k, stages.get(k).note.clone()))
        .collect();
    assert!(
        failed.is_empty(),
        "Drivetrain should reach the end of the pipeline; failed: {failed:?}"
    );

    let flagged: Vec<_> = StageKind::COMPILATION
        .iter()
        .filter(|&&k| stages.get(k).outcome == Outcome::Flagged)
        .collect();
    assert!(
        !flagged.is_empty(),
        "Drivetrain is high-index — at least Structural must be flagged, or this test \
         has stopped guarding anything",
    );

    // And the pipeline really did finish, rather than merely not failing.
    assert!(
        stages.solve_lowering.value.is_some(),
        "solve lowering should have produced a model"
    );
}

/// A singular Structural stage carries structured error data (equation
/// and unknown counts, rank deficiency, unmatched names) plus the
/// incidence matrix and partial matching for UI rendering.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn singular_structural_carries_summary_data() {
    let FromWorker::Compiled { stages, .. } = compile_specimen_shared("Drivetrain") else {
        panic!("expected Compiled");
    };
    let v = stages
        .structural
        .value
        .as_ref()
        .expect("singular Structural should have a value");
    let err = &v["error"];
    assert_eq!(err["kind"].as_str(), Some("singular"));
    assert!(err["n_equations"].as_u64().unwrap() > 0);
    assert!(err["n_unknowns"].as_u64().unwrap() > 0);
    assert!(err["rank_deficiency"].as_u64().unwrap() > 0);
    assert!(!err["unmatched_equations"].as_array().unwrap().is_empty());
    assert!(!err["unmatched_unknowns"].as_array().unwrap().is_empty());
    let inc = &v["incidence"];
    assert!(inc["n_eq"].as_u64().unwrap() > 0);
    let matching = v["matching"]
        .as_array()
        .expect("should have partial matching");
    assert!(!matching.is_empty(), "partial matching should be non-empty");
    let mat = crate::incidence_view::IncidenceMatrix::from_report(v)
        .expect("singular structural report should parse as IncidenceMatrix");
    assert!(mat.n_eq() > 0);
}

/// Drivetrain's index-reduction trace produces animation frames — the
/// constrained-dummy reduction finds multiple demotions, each emitting
/// **A stage's summary may not claim more than the frames from the same run
/// recorded, and the corpus must differentiate somewhere.**
///
/// # The defect this is built from
///
/// `index-reduction.md` taught, for its whole existence, that `Drivetrain`
/// performs **zero** differentiations — *"the textbook mechanism was not
/// needed"* — in the lab named for the algorithm that differentiates. The
/// compiler differentiates at least four times on that model.
///
/// **Both halves of HRW were telling the truth.** `differentiated_rows` is
/// built by scanning the *final* DAE for surviving `index_reduction:d_dt_for_`
/// origin markers, and step 10 (`eliminate_trivial`) removes 77 equations,
/// taking them with it. The frames record what *happened*; the summary reports
/// what *survived*. Nothing said so, and nothing compared them.
///
/// **Every other checker in this repository compares a document to a trace,
/// and the trace said zero.** So the lab was consistent with the artefact and
/// wrong about the compiler, and no amount of document-versus-trace checking
/// could have found it. This is the first check that holds two views of the
/// *same run* against each other.
///
/// # What it asserts, and why not equality
///
/// **`survivors <= events`** — the summary cannot report more differentiated
/// rows than differentiations that occurred. That is a real invariant and the
/// gap between the two is legitimate, being exactly the elimination above.
///
/// **And at least one specimen must differentiate.** That clause is the one
/// that matters most, because it encodes the thing Claude got wrong: reading
/// `differentiated_rows: []` across all 17 specimens produced the confident
/// conclusion that *Rumoca never differentiates*, and a lab was written on it.
/// **A green corpus-wide zero is indistinguishable from a feature that never
/// runs** — the same shape as `fidelity-plan.md`'s F10, whose absence clause
/// had nothing to act on.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn a_reduction_summary_never_claims_more_than_its_frames_recorded() {
    use rumoca_phase_structural::dae_prepare::IndexReductionStep;

    // Specimens that reach index reduction with something to reduce. Named
    // rather than globbed: a glob that silently matched nothing would make
    // every assertion below vacuous, which is this file's recurring failure.
    const SPECIMENS: &[&str] = &[
        "Drivetrain",
        "GearWithBrake",
        "BouncingBall",
        "RcCircuit",
        "BenchActuator",
    ];

    let mut total_events = 0usize;
    let mut checked = 0usize;
    let mut rows: Vec<String> = Vec::new();

    for name in SPECIMENS {
        let FromWorker::Compiled {
            stages,
            index_reduction_frames,
            ..
        } = compile_specimen_shared(name)
        else {
            panic!("expected {name} to compile");
        };

        let events = index_reduction_frames
            .iter()
            .filter(|f| matches!(&f.step, IndexReductionStep::Differentiated { .. }))
            .count();

        // Absent is not zero: a stage that never produced a report is a
        // different fact from one reporting an empty list, and conflating them
        // is how a silent failure reads as a passing check.
        let Some(value) = stages.index_reduction.value.as_ref() else {
            rows.push(format!("{name}: no index-reduction stage"));
            continue;
        };
        let Some(survivors) = value
            .get("reduction")
            .and_then(|r| r.get("differentiated_rows"))
            .and_then(|d| d.as_array())
            .map(Vec::len)
        else {
            rows.push(format!("{name}: no differentiated_rows field"));
            continue;
        };

        checked += 1;
        total_events += events;
        rows.push(format!(
            "{name}: {events} differentiated, {survivors} survived"
        ));

        assert!(
            survivors <= events,
            "{name}: the summary reports {survivors} differentiated row(s) but the \
             frames from the same run recorded only {events} differentiation(s). A \
             summary may report FEWER than happened — later steps eliminate rows — \
             but never more, and more means the two are describing different runs",
        );
    }

    assert!(
        checked >= 3,
        "only {checked} specimen(s) yielded both a summary and frames — the \
         extraction is broken, not the compiler:\n  {}",
        rows.join("\n  "),
    );
    assert!(
        total_events > 0,
        "no specimen in this set differentiates anything, so the comparison above \
         ran against nothing on the side that matters. **This clause exists because \
         the corpus-wide zero in `differentiated_rows` was once read as proof that \
         Rumoca never differentiates, and a lab was written on it.** If this fires, \
         either the specimens changed or index reduction stopped differentiating — \
         and the second is a finding, not a test to relax:\n  {}",
        rows.join("\n  "),
    );
    println!(
        "reduction summaries checked against their frames:\n  {}",
        rows.join("\n  ")
    );
}

/// BeginState, Differentiated, and Demoted frames.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn drivetrain_index_reduction_produces_trace_frames() {
    let FromWorker::Compiled {
        index_reduction_frames,
        ..
    } = compile_specimen_shared("Drivetrain")
    else {
        panic!("expected Compiled");
    };
    assert!(
        !index_reduction_frames.is_empty(),
        "Drivetrain should produce index-reduction animation frames"
    );
    use rumoca_phase_structural::dae_prepare::IndexReductionStep;
    let n_demoted = index_reduction_frames
        .iter()
        .filter(|f| matches!(&f.step, IndexReductionStep::Demoted { .. }))
        .count();
    assert!(
        n_demoted >= 4,
        "expected at least 4 demotions, got {n_demoted}"
    );
    let n_differentiated = index_reduction_frames
        .iter()
        .filter(|f| matches!(&f.step, IndexReductionStep::Differentiated { .. }))
        .count();
    assert!(
        n_differentiated >= 4,
        "expected at least 4 differentiations, got {n_differentiated}"
    );
}

/// The trace opens on `Start`, so the animation has a visible "before".
///
/// Without it the replay begins on the first `BeginState` — which announces
/// an intention and reads as though reduction had already happened — and no
/// frame anywhere shows the unreduced system.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn index_reduction_trace_opens_on_the_starting_system() {
    let FromWorker::Compiled {
        index_reduction_frames,
        ..
    } = compile_specimen_shared("Drivetrain")
    else {
        panic!("expected Compiled");
    };
    use rumoca_phase_structural::dae_prepare::IndexReductionStep;
    let first = index_reduction_frames.first().expect("frames");
    let IndexReductionStep::Start { states, equations } = &first.step else {
        panic!("first frame should be Start, got {:?}", first.step);
    };
    assert!(
        !states.is_empty(),
        "Drivetrain has states entering reduction"
    );
    assert!(
        *equations > 0,
        "Drivetrain has equations entering reduction"
    );
    assert!(
        first.demoted_so_far.is_empty(),
        "nothing is demoted by the traced passes before they begin"
    );
    // Exactly one — the two traced passes must not each contribute a start.
    let n_start = index_reduction_frames
        .iter()
        .filter(|f| matches!(&f.step, IndexReductionStep::Start { .. }))
        .count();
    assert_eq!(n_start, 1, "expected a single opening frame, got {n_start}");
}

/// The index reduction stage embeds a "before" report with the raw
/// (pre-reduction) DAE's incidence matrix and partial matching.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn drivetrain_index_reduction_has_before_report() {
    let FromWorker::Compiled { stages, .. } = compile_specimen_shared("Drivetrain") else {
        panic!("expected Compiled");
    };
    let v = stages
        .index_reduction
        .value
        .expect("index reduction should succeed");
    let before = &v["before"];
    assert!(
        before.is_object(),
        "missing 'before' sub-object in index reduction JSON"
    );
    let inc = &before["incidence"];
    assert!(
        inc["n_eq"].as_u64().unwrap() > 0,
        "before incidence should have equations"
    );
    assert!(
        inc["n_var"].as_u64().unwrap() > 0,
        "before incidence should have unknowns"
    );
    let matching = before["matching"]
        .as_array()
        .expect("before should have matching");
    assert!(!matching.is_empty(), "partial matching should be non-empty");
    let n_eq = inc["n_eq"].as_u64().unwrap() as usize;
    assert!(
        matching.len() < n_eq,
        "partial matching should be incomplete (singular)"
    );
}

/// The "before" report is parseable by `IncidenceMatrix::from_report`,
/// enabling the Before/After split view on the Index Reduction tab.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn drivetrain_before_report_parseable_as_incidence() {
    let FromWorker::Compiled { stages, .. } = compile_specimen_shared("Drivetrain") else {
        panic!("expected Compiled");
    };
    let v = stages
        .index_reduction
        .value
        .expect("index reduction should succeed");
    let before = &v["before"];
    let mat = crate::incidence_view::IncidenceMatrix::from_report(before)
        .expect("before report should parse into an IncidenceMatrix");
    assert!(mat.n_eq() > 0);
    assert!(mat.n_var() > 0);
    let caption = mat.caption();
    assert!(
        caption.contains("rank deficiency"),
        "singular system should show rank deficiency: {caption}"
    );

    // The after incidence must resolve matching (equation names must
    // agree between the structural report's matching array and the
    // incidence rows — both use the labeled form like "f_x[0] (origin)").
    let after_mat = crate::incidence_view::IncidenceMatrix::from_report(&v)
        .expect("after report should parse into an IncidenceMatrix");
    let after_caption = after_mat.caption();
    assert!(
        after_caption.contains("full rank"),
        "reduced system should be full rank: {after_caption}"
    );
}

/// For an already index-1 system, the "before" report still exists (so
/// the split view code doesn't crash), but the note says "index-1".
#[test]
fn single_inertia_index_reduction_note_says_index_1() {
    let FromWorker::Compiled { stages, .. } = compile_specimen_shared("SingleInertia") else {
        panic!("expected Compiled");
    };
    let note = stages.index_reduction.note.as_deref().unwrap_or("");
    assert!(
        !note.contains("singular"),
        "SingleInertia should not be singular: {note}"
    );
    assert!(
        note.contains("index-1"),
        "note should mention index-1: {note}"
    );
    let v = stages
        .index_reduction
        .value
        .expect("index reduction should succeed");
    assert!(
        v.get("before").is_some(),
        "before report should exist even for index-1 systems"
    );
}

/// MotorWithBrake produces trace frames from the missing-derivative path
/// (index_reduce_missing_state_derivatives) — 1 EMF demotion.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn motor_with_brake_index_reduction_produces_trace_frames() {
    let FromWorker::Compiled {
        index_reduction_frames,
        ..
    } = compile_specimen_shared("MotorWithBrake")
    else {
        panic!("expected Compiled");
    };
    assert!(
        !index_reduction_frames.is_empty(),
        "MotorWithBrake should produce index-reduction animation frames"
    );
}

/// A scratch specimen compiles like any other (ideas #42).
///
/// The listing and marking are tested in `app`; this is the half that matters for
/// answering a question — Claude writes a probe mid-conversation and it goes
/// through the same pipeline as the curated corpus, with the same IR available.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn a_scratch_specimen_compiles_end_to_end() {
    // Establishes its own precondition — it used to skip when no probe happened to
    // be on disk, so in a clean checkout it compiled nothing and reported success.
    // The source is shared with the other two scratch tests because the `n_states`
    // assertion below is a property of *that* model.
    let probe_file = crate::test_support::ScratchSpecimen::probe();
    let path = probe_file.path().to_path_buf();
    let mut w = shared_worker().lock().unwrap_or_else(|e| e.into_inner());
    let FromWorker::Compiled { stages, model, .. } = w.compile(&path, &|_: FromWorker| {}) else {
        panic!("expected Compiled");
    };
    assert_eq!(model.as_deref(), Some("ScratchProbe"));
    assert!(
        stages.solve_lowering.value.is_some() && !stages.solve_lowering.note_is_error(),
        "a scratch probe reaches the end of the pipeline like any specimen",
    );
    // And its IR is real: one state, from `tau * der(x) = -x`.
    let n_states = stages
        .initialization
        .value
        .as_ref()
        .and_then(|v| v.get("n_states"))
        .and_then(serde_json::Value::as_u64);
    assert_eq!(n_states, Some(1), "the probe has exactly one state");
}

/// **HRW's re-derived tearing matches Rumoca's own report.**
///
/// `docs/fidelity-plan.md` F1, and the first test of the question Doug raised: does
/// HRW represent what Rumoca *decided*, or something of its own?
///
/// The tearing animation does not read the compiler's result — it **re-runs the
/// algorithm** on each coupled block to produce frames. Until 2026-07-30 nothing
/// compared the two, so they agreed by assumption. A divergence here would mean an
/// animation teaching a decision the compiler never made, which is the worst failure
/// available to a tool whose purpose is explanation.
///
/// The non-vacuity guard is not decoration: a model with no coupled block reports
/// `[]` and derives `[]` — agreement on nothing. Without the guard a corpus of such
/// models would pass while testing nothing at all.
///
/// **Compared per tab, against the DAE that tab animates.** The Structural and Index
/// Reduction tabs describe *different systems* (`App::tearing_dae` re-runs the
/// reduction funnel for the latter), so comparing one tab's re-derivation against the
/// other's report tests nothing and fails on models that are singular before
/// reduction. Singular stages are skipped because `structural_view_available` hides
/// the tearing view there — a re-derivation the UI never shows is not a
/// misrepresentation.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn hrw_rederived_tearing_matches_rumocas_report() {
    /// The tear variables Rumoca's report lists, flattened across blocks in
    /// report order — the same order `tear_variable_names` walks.
    fn reported_tears(report: &serde_json::Value) -> Vec<String> {
        report
            .get("blocks")
            .and_then(serde_json::Value::as_array)
            .map(|blocks| {
                blocks
                    .iter()
                    .filter_map(|b| b.get("tearing")?.get("tear_vars")?.as_array())
                    .flatten()
                    .filter_map(serde_json::Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default()
    }

    let mut tabs_with_tears = 0usize;

    for name in F1_MODELS {
        let FromWorker::Compiled { stages, dae, .. } = compile_specimen_shared(name) else {
            panic!("{name}: expected Compiled");
        };
        let dae = dae.unwrap_or_else(|| panic!("{name}: no DAE"));

        // (tab, the DAE HRW animates there, the report it shows beside it)
        let mut reduced = dae.clone();
        index_reduce_in_place(&mut reduced);
        let cases: [(&str, &rumoca_ir_dae::Dae, &Stage); 2] = [
            ("Structural", &dae, &stages.structural),
            ("IndexReduction", &reduced, &stages.index_reduction),
        ];

        for (tab, tab_dae, stage) in cases {
            // Singular stages hide the tearing view entirely.
            if stage.outcome != Outcome::Ok {
                continue;
            }
            let Some(report) = stage.value.as_ref() else {
                continue;
            };

            let reported = reported_tears(report);
            let derived =
                crate::tearing_anim::TearingAnimation::record(tab_dae).tear_variable_names();

            assert_eq!(
                derived, reported,
                "{name} / {tab}: the tearing animation re-derives a different answer \
                 than the compiler reported — it would be teaching a decision Rumoca \
                 never made",
            );
            if !reported.is_empty() {
                tabs_with_tears += 1;
            }
        }
    }

    assert!(
        tabs_with_tears >= 4,
        "only {tabs_with_tears} tabs actually tore anything; the rest agreed on an \
         empty list, which tests nothing",
    );
}

/// **A library model compiles by name, all the way through.**
///
/// The entry point Test mode needs to open a report row, and the one
/// fidelity testing at MSL scale needs — checking HRW's representation of an
/// MSL model means compiling it *through HRW's own path*.
///
/// Deliberately picks a model nested deep inside a **multi-class** file:
/// `CriticalDamping` sits at lines 1498-1620 of `Blocks/Continuous.mo`. The
/// specimen path takes "the first class in the file" as the model, which
/// would silently compile something else entirely here — so this is the case
/// that proves the by-name path is not the file path in disguise.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn a_library_model_compiles_by_qualified_name() {
    const NAME: &str = "Modelica.Blocks.Continuous.CriticalDamping";
    let mut w = shared_worker().lock().unwrap_or_else(|e| e.into_inner());
    let FromWorker::Compiled {
        model,
        stages,
        dae,
        identifier_index,
        ..
    } = w.compile_model_by_name(NAME, &|_: FromWorker| {})
    else {
        panic!("expected Compiled");
    };

    assert_eq!(
        model.as_deref(),
        Some("CriticalDamping"),
        "the requested model, not the first class in a file of many",
    );
    let dae = dae.expect("a library model should reach a DAE");
    assert!(
        !dae.continuous.equations.is_empty(),
        "CriticalDamping is a real block; an empty DAE means the wrong class was compiled",
    );

    // Every compilation stage produced something — the by-name path is the
    // whole pipeline, not a shortcut to one phase.
    for kind in StageKind::COMPILATION {
        let stage = stages.get(*kind);
        assert!(
            stage.value.is_some() || stage.note.is_some(),
            "{kind:?} produced neither IR nor a note",
        );
    }
    assert_eq!(
        stages.parse.outcome,
        Outcome::Ok,
        "parse: {:?}",
        stages.parse.note
    );
    assert_eq!(
        stages.flatten.outcome,
        Outcome::Ok,
        "flatten: {:?}",
        stages.flatten.note
    );

    // Source-linked features work too, which is the half that needs the
    // declaring document rather than merely the name.
    let index = identifier_index.expect("identifier index");
    assert!(
        !index.variables.is_empty(),
        "no identifiers indexed — the library document's source text did not reach the index",
    );
}

/// A name that is not a class is refused with a message that says so, rather
/// than compiling something adjacent or panicking.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn an_unknown_qualified_name_is_refused() {
    let mut w = shared_worker().lock().unwrap_or_else(|e| e.into_inner());
    let FromWorker::Compiled { stages, dae, .. } =
        w.compile_model_by_name("Modelica.Nope.NotAClass", &|_: FromWorker| {})
    else {
        panic!("expected Compiled");
    };
    assert!(dae.is_none(), "nothing should have been compiled");
    let note = stages.parse.note.unwrap_or_default();
    assert!(
        note.contains("not a class in the loaded libraries"),
        "the refusal should name the problem; got {note:?}",
    );
}

/// **Compiling a library model does not disturb the session.**
///
/// The by-name path deliberately does not register the document — it is
/// already in a durable source root. If it did, the session would hold the
/// file twice and a later removal would evict part of the library. This
/// checks the observable consequence: a specimen compiled afterwards is
/// unaffected.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn a_library_compile_leaves_the_session_usable_for_specimens() {
    let mut w = shared_worker().lock().unwrap_or_else(|e| e.into_inner());
    let before = w.session.document_uris().len();
    let _ = w.compile_model_by_name("Modelica.Blocks.Continuous.CriticalDamping", &|_| {});
    let after = w.session.document_uris().len();
    assert_eq!(
        after, before,
        "a library compile must not add or remove documents"
    );

    // And a specimen still compiles against the same session.
    let path = PathBuf::from(format!(
        "{}/specimens/ProportionalLoop.mo",
        env!("CARGO_MANIFEST_DIR"),
    ));
    let FromWorker::Compiled { dae, .. } = w.compile(&path, &|_: FromWorker| {}) else {
        panic!("expected Compiled");
    };
    assert!(
        dae.is_some(),
        "a specimen must still compile after a library model did"
    );
}

/// The specimens F1 re-derives on. Shared by the three checks so a model
/// added here is covered by all of them at once.
#[cfg(test)]
const F1_MODELS: &[&str] = &[
    "ProportionalLoop",
    "MixedLoop",
    "TwoLoops",
    "NonlinearLoop",
    "Drivetrain",
    "RcCircuit",
    "SingleInertia",
    "CapacitorLoop",
    "BouncingBall",
    "MotorWithBrake",
];

/// **HRW's re-derived matching matches Rumoca's own report.**
///
/// `docs/fidelity-plan.md` F1, second of three. The incidence view renders the
/// matching overlay from the report, but [`MatchingAnimation`] **re-runs Kuhn's
/// algorithm** on the parsed matrix to produce its frames — so the green circles
/// the animation walks through could in principle end somewhere the compiler
/// never went.
///
/// The comparison is exact rather than by size, because a maximum matching is
/// not unique: two matchings of equal cardinality are equally *valid* and still
/// mean the animation is narrating a different transversal than the one the
/// solve order was built from. `match_progress` cannot see that difference,
/// which is why `final_matching` exists.
///
/// What this really exercises is the **JSON round trip** — report → names →
/// indices → re-run. Both sides call the same Rumoca function, so a divergence
/// means the row or column order did not survive it.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn hrw_rederived_matching_matches_rumocas_report() {
    let mut compared = 0usize;

    for name in F1_MODELS {
        let FromWorker::Compiled { stages, .. } = compile_specimen_shared(name) else {
            panic!("{name}: expected Compiled");
        };
        let Some(report) = stages.structural.value.as_ref() else {
            continue;
        };
        let Some(mat) = crate::incidence_view::IncidenceMatrix::from_report(report) else {
            continue;
        };
        if mat.n_eq() == 0 {
            continue;
        }

        let derived =
            crate::matching_anim::MatchingAnimation::from_incidence(&mat).final_matching();
        let reported = mat.reported_matching();

        assert_eq!(
            derived.len(),
            reported.len(),
            "{name}: re-derived matching covers {} equations, the report {}",
            derived.len(),
            reported.len(),
        );
        assert_eq!(
            derived, reported,
            "{name}: the matching animation ends on a different transversal than \
             Rumoca reported — the overlay and the animation would disagree",
        );
        compared += 1;
    }

    assert!(
        compared >= 5,
        "only {compared} models produced an incidence matrix to compare; F1's matching \
         check is testing almost nothing",
    );
}

/// **HRW's re-derived BLT blocks match Rumoca's own report.**
///
/// `docs/fidelity-plan.md` F1, third of three. [`TarjanAnimation`] re-runs
/// matching *and* Tarjan to build its graph, so it is the furthest-removed
/// re-derivation in HRW — two algorithms deep from anything the compiler said.
///
/// Compared as a **partition**, not a sequence: Tarjan emits components in
/// reverse topological order while the report lists them in solve order, so
/// requiring equal ordering would fail on a difference that means nothing.
/// Requiring equal *sets* still catches the thing that matters — an equation
/// placed in the wrong block, which is a different solve order and a different
/// algebraic loop.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn hrw_rederived_blocks_match_rumocas_report() {
    use std::collections::BTreeSet;

    let mut compared = 0usize;
    let mut saw_a_coupled_block = false;

    for name in F1_MODELS {
        let FromWorker::Compiled { stages, .. } = compile_specimen_shared(name) else {
            panic!("{name}: expected Compiled");
        };
        let Some(report) = stages.structural.value.as_ref() else {
            continue;
        };
        let Some(mat) = crate::incidence_view::IncidenceMatrix::from_report(report) else {
            continue;
        };
        let reported = mat.reported_blocks();
        if reported.is_empty() {
            continue;
        }
        let Some(anim) = crate::tarjan_anim::TarjanAnimation::from_incidence(&mat) else {
            continue;
        };

        let as_sets = |bs: Vec<Vec<usize>>| -> BTreeSet<BTreeSet<usize>> {
            bs.into_iter().map(|b| b.into_iter().collect()).collect()
        };
        if reported.iter().any(|b| b.len() > 1) {
            saw_a_coupled_block = true;
        }

        assert_eq!(
            as_sets(anim.final_sccs()),
            as_sets(reported),
            "{name}: Tarjan re-derives a different block partition than Rumoca \
             reported — the animation would show the wrong solve order",
        );
        compared += 1;
    }

    assert!(
        compared >= 5,
        "only {compared} models had blocks to compare"
    );
    assert!(
        saw_a_coupled_block,
        "every model compared had only singleton blocks; the partition check never \
         had a chance to be wrong",
    );
}

/// A resolve failure names the offending reference **and its line**, with the
/// library noise separated out.
///
/// Two problems fixed together 2026-07-29:
///
/// 1. `Diagnostic::labels` — the `Span` marking exactly where the error is — was
///    dropped by every diagnostic emitter in HRW.
/// 2. The resolve payload was `format!("{e:#}")`: ~39 semicolon-separated items of
///    which ~38 were MSL deprecation warnings, the model's real error last. The
///    signal was the final 2% of a 2000-character string.
///
/// The fix uses `compile_model_diagnostics` for structured, model-scoped diagnostics
/// and partitions them by **severity** — so nothing is pattern-matched out of message
/// text and no real error can be filtered away by a wording change.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn a_resolve_failure_names_the_reference_and_its_line() {
    let FromWorker::Compiled { stages, .. } = compile_specimen_shared("UndefinedRef") else {
        panic!("expected Compiled");
    };
    let err = stages
        .resolve
        .value
        .as_ref()
        .and_then(|v| v.get("error"))
        .expect("a resolve failure must carry a structured payload");

    let errors = err["diagnostics"]["errors"]
        .as_array()
        .expect("errors array");
    assert_eq!(
        errors.len(),
        1,
        "one error, not 34 items of library noise: {errors:?}"
    );
    assert_eq!(errors[0]["code"], "ER002");
    assert!(
        errors[0]["message"]
            .as_str()
            .is_some_and(|m| m.contains("missingGain")),
        "{}",
        errors[0]["message"],
    );

    // The label is the point: a line Doug can look at.
    let loc = &errors[0]["labels"][0]["location"];
    assert_eq!(loc["line"], 9, "the reference is on line 9: {loc}");
    assert!(
        loc["line_text"]
            .as_str()
            .is_some_and(|t| t.contains("missingGain")),
        "line_text must be quotable: {loc}",
    );

    // Warnings are kept, deduplicated, and clearly not the cause.
    let warnings = &err["diagnostics"]["warnings"];
    let total = warnings["total"].as_u64().expect("total");
    let distinct = warnings["distinct"].as_array().expect("distinct").len();
    assert!(
        total > distinct as u64,
        "{total} warnings collapse to {distinct} distinct"
    );

    // Never lossy: the original concatenated message survives verbatim.
    assert!(
        err["message"]
            .as_str()
            .is_some_and(|m| m.contains("missingGain"))
    );
}

/// A typecheck failure names its line too, through the same shared helper.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn a_typecheck_failure_names_its_line() {
    let FromWorker::Compiled { stages, .. } = compile_specimen_shared("DimensionMismatch") else {
        panic!("expected Compiled");
    };
    let err = stages
        .typecheck
        .value
        .as_ref()
        .and_then(|v| v.get("error"))
        .expect("a typecheck failure must carry a structured payload");

    let diags = err["diagnostics"].as_array().expect("diagnostics array");
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0]["code"], "ET002");
    assert!(
        diags[0]["message"]
            .as_str()
            .is_some_and(|m| m.contains("dimension mismatch")),
        "{}",
        diags[0]["message"],
    );
    let loc = &diags[0]["labels"][0]["location"];
    assert_eq!(
        loc["line"], 11,
        "the offending equation is on line 11: {loc}"
    );
    assert!(
        loc["line_text"]
            .as_str()
            .is_some_and(|t| t.contains("small = big")),
        "{loc}"
    );
}

/// A library compile reports the **qualified name** as its identity.
///
/// **This is the bug that made every MSL model appear to hang.** The UI's
/// three staleness checks compare a result's `path` against `App::selected`,
/// which for a library model holds the qualified name. The worker's
/// early-error return already reported that; the success path reported the
/// MSL *file* URI instead. So a successful compile never matched, every
/// result was discarded as stale, and the UI showed a log full of work with
/// no stages and a spinner that never stopped.
///
/// **The two returns disagreeing is the defect**, so this asserts they agree.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn a_library_compile_identifies_itself_by_qualified_name() {
    const NAME: &str = "Modelica.Electrical.Analog.Basic.Resistor";
    let mut w = shared_worker().lock().unwrap_or_else(|e| e.into_inner());
    let FromWorker::Compiled { path, .. } = w.compile_model_by_name(NAME, &|_| {}) else {
        panic!("expected Compiled");
    };
    assert_eq!(
        path,
        std::path::PathBuf::from(NAME),
        "a library compile must report the qualified name it was asked for. Reporting              the document URI instead makes every result look stale to the UI, which is              indistinguishable from a compile that never finishes",
    );
}

/// A library compile **carries the source of the file that declares the model**.
///
/// The source view refused MSL models outright until 2026-08-01, on the stated
/// grounds that a library model had "no single source file to show". That was
/// never true — `locate_library_model` reads exactly that file out of the
/// session *in order to compile it*, then dropped it. Doug: *"The modelica
/// source view for an MSL model should be just as functional as for an HRW
/// specimen."*
///
/// Checks the two things the pane cannot work without, and would otherwise fail
/// at silently: **non-empty text**, and a **declaration line inside it**.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn a_library_compile_carries_the_declaring_file_source() {
    let mut w = shared_worker().lock().unwrap_or_else(|e| e.into_inner());
    let out = w.compile_model_by_name("Modelica.Electrical.Analog.Basic.Resistor", &|_| {});
    let FromWorker::Compiled { library_source, .. } = out else {
        panic!("expected Compiled");
    };
    let lib = library_source.expect(
        "a library compile must carry its declaring file: the source view has no other \
         way to get it, and without it the pane renders empty",
    );
    let text = lib
        .text
        .clone()
        .expect("the declaring file must be readable");
    assert!(
        !text.trim().is_empty(),
        "empty source would render as a blank pane \u{2014} indistinguishable from the \
         refusal this replaced",
    );
    assert!(
        lib.uri.ends_with(".mo"),
        "the URI names the declaring document, and it is shown to the reader: {}",
        lib.uri,
    );

    // **The declaration line must land inside the file.** `Resistor` opens
    // roughly 1,500 lines into `Basic.mo`, so a reader dropped at line 1 sees a
    // package header and none of what they asked for. An out-of-range line
    // scrolls nowhere and looks like the scroll is broken.
    let lines = text.lines().count() as u32;
    let decl = lib.decl_line.expect("a located class has a start line");
    assert!(
        decl >= 1 && decl <= lines,
        "declaration line {decl} is outside the {lines}-line file it indexes",
    );
    let decl_text = text.lines().nth(decl as usize - 1).unwrap_or("");
    assert!(
        decl_text.contains("Resistor"),
        "line {decl} should be Resistor\u{2019}s declaration, found: {decl_text:?}",
    );
}

/// An MSL model's identifiers are indexed **on the lines they occupy**.
///
/// Doug, 2026-08-01: *"Identifiers in the modelica source view of an MSL
/// model do not seem to be clickable to cause following."*
///
/// The index and the source pane must agree about where a variable is.
/// `IdentifierIndex::build` counts newlines in the text it is handed to turn
/// a `source_span` byte offset into a line, and it was handed `""` for every
/// library model — so **every variable landed on line 1**. The index was
/// populated, which is why nothing looked broken; it was simply pointing at
/// the wrong lines, and `clickable_spans` found nothing to underline on the
/// lines a reader was actually looking at.
///
/// **Line 1 is the tell**, so that is what this asserts against: a real
/// index over a multi-thousand-line library file cannot have everything on
/// its first line.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn an_msl_model_indexes_identifiers_on_their_own_lines() {
    let mut w = shared_worker().lock().unwrap_or_else(|e| e.into_inner());
    let out = w.compile_model_by_name("Modelica.Electrical.Analog.Basic.Resistor", &|_| {});
    let FromWorker::Compiled {
        identifier_index,
        library_source,
        ..
    } = out
    else {
        panic!("expected Compiled");
    };
    let idx = identifier_index.expect("a successful library compile builds an index");
    assert!(
        !idx.variables.is_empty(),
        "an index with no variables makes nothing clickable at all",
    );

    let text = library_source
        .expect("carries its source")
        .text
        .expect("readable");
    let total_lines = text.lines().count() as u32;

    // **Every line must be inside the file the pane renders.** A span
    // resolved against different text can land past the end, where it
    // silently matches nothing.
    for (name, v) in &idx.variables {
        assert!(
            v.source_line >= 1 && v.source_line <= total_lines,
            "{name} is indexed at line {} of a {total_lines}-line file",
            v.source_line,
        );
    }

    // The defect's signature: everything collapsed onto line 1.
    let on_line_1 = idx
        .variables
        .values()
        .filter(|v| v.source_line == 1)
        .count();
    assert!(
        on_line_1 < idx.variables.len(),
        "all {} variables are indexed on line 1, which means the index was built \
         against text that is not what the pane shows — the exact defect that \
         made MSL identifiers unclickable",
        idx.variables.len(),
    );
}

/// **The compiler's byte offsets and the bytes on screen are the same bytes.**
///
/// Doug asked whether displaying MSL source is a hack, and whether spans
/// agree between the source view and the stage trees. This is that question
/// made checkable.
///
/// The pane's text does **not** come from the compiler: Rumoca discards
/// source-root text (`Document::new(uri, String::new(), ..)`), so HRW re-reads
/// the declaring file from disk. That leaves two paths to what ought to be one
/// string, and **agreement becomes a property to maintain rather than a
/// structural guarantee.** Rumoca's parsed-artifact cache is keyed on a
/// `blake3` hash of every file's bytes, recomputed on each load, so a file
/// edited behind the cache invalidates it -- but that is a chain of reasoning,
/// and this is a measurement.
///
/// **Slicing is the sharp end.** `CriticalDamping` lives ~62,000 bytes into
/// `Continuous.mo`; if the two texts differed by a single byte anywhere before
/// it, the slice would land on unrelated characters. Nothing would crash, and
/// the pane would underline confident nonsense.
///
/// Three files, deliberately: two where the model is the whole file, and one
/// multi-class file deep enough that a drifting offset could not stay hidden.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn compiler_spans_address_the_text_the_pane_shows() {
    let mut checked = 0usize;
    for name in [
        "Modelica.Electrical.Analog.Basic.Resistor",
        "Modelica.Mechanics.Rotational.Components.Inertia",
        "Modelica.Blocks.Continuous.CriticalDamping",
    ] {
        let mut w = shared_worker().lock().unwrap_or_else(|e| e.into_inner());
        let out = w.compile_model_by_name(name, &|_| {});
        let FromWorker::Compiled {
            identifier_index,
            library_source,
            ..
        } = out
        else {
            panic!("{name}: expected Compiled");
        };
        let idx = identifier_index.expect("index");
        let text = library_source.expect("source").text.expect("readable");

        for (var, v) in &idx.variables {
            let leaf = var.rsplit('.').next().unwrap_or(var);
            let (s, e) = v.source_byte_range;

            // **In range, and on a character boundary.** A slice that is
            // merely in range can still be nonsense; `get` returning None on
            // a non-boundary is itself a disagreement signal.
            let slice = text.get(s..e).unwrap_or_else(|| {
                panic!(
                    "{name}: {var} spans {s}..{e}, which is not a valid slice of the \
                     {}-byte file the pane renders",
                    text.len(),
                )
            });
            assert!(
                slice.contains(leaf),
                "{name}: {var} spans {s}..{e}, which reads {slice:?} -- the compiler's \
                 offsets do not address the text on screen, so every underline and \
                 blamed line in this file points somewhere arbitrary",
            );

            // And the line the index reports must hold it too, since that,
            // not the byte range, is what places the underline.
            let line = text.lines().nth(v.source_line as usize - 1).unwrap_or("");
            assert!(
                line.contains(leaf),
                "{name}: {var} is indexed at line {}, which reads {line:?}",
                v.source_line,
            );
            checked += 1;
        }
    }

    // **Non-vacuity.** Every assertion above lives inside a loop that an empty
    // index would skip entirely, leaving the test green while checking nothing.
    assert!(
        checked >= 10,
        "only {checked} variables checked -- too few to have exercised anything",
    );
}

/// The Parse stage of an MSL model **holds the classes its file declares**.
///
/// It used to hold `{"classes":{},"within":null}` for every library model,
/// coloured as a success, because it parsed the empty string Rumoca keeps in
/// place of source-root text. **An empty green tab asserts "this model parsed
/// to nothing"** -- false, and indistinguishable from a model that genuinely
/// declares nothing. The source view made the contradiction visible: a pane
/// full of declarations beside a tab claiming the file held none.
///
/// Asserts the requested class is **among** the classes parsed, not that it is
/// the only one: a library file declares many, and the reader is looking at
/// all of them in the source view.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn an_msl_models_parse_stage_holds_its_declaring_file() {
    for (qualified, leaf) in [
        ("Modelica.Electrical.Analog.Basic.Resistor", "Resistor"),
        // A multi-class file: `Continuous.mo` declares CriticalDamping among
        // dozens, ~62 KB in. If only the first class survived, this fails.
        (
            "Modelica.Blocks.Continuous.CriticalDamping",
            "CriticalDamping",
        ),
    ] {
        let mut w = shared_worker().lock().unwrap_or_else(|e| e.into_inner());
        let FromWorker::Compiled { stages, model, .. } =
            w.compile_model_by_name(qualified, &|_| {})
        else {
            panic!("{qualified}: expected Compiled");
        };
        let value = stages
            .parse
            .value
            .as_ref()
            .unwrap_or_else(|| panic!("{qualified}: the Parse stage produced no value"));
        let classes = value
            .get("classes")
            .and_then(|c| c.as_object())
            .unwrap_or_else(|| panic!("{qualified}: parse value has no classes map"));

        assert!(
            !classes.is_empty(),
            "{qualified}: the Parse stage is empty and reports success, which claims \
             the file declares nothing while the source view shows it declaring plenty",
        );
        // **The AST is a tree, not a flat list.** `Continuous.mo` declares a
        // *package* `Continuous` holding CriticalDamping among dozens, so a
        // top-level lookup finds only the package. Descending is the point:
        // it proves the whole file was parsed, not just its outer shell.
        fn declares(value: &serde_json::Value, leaf: &str) -> bool {
            match value.get("classes").and_then(|c| c.as_object()) {
                Some(map) => map.contains_key(leaf) || map.values().any(|v| declares(v, leaf)),
                None => false,
            }
        }
        assert!(
            declares(value, leaf),
            "{qualified}: parsed {} top-level classes, none of which declares {leaf}                  anywhere beneath it: {:?}",
            classes.len(),
            classes.keys().take(8).collect::<Vec<_>>(),
        );
        assert_eq!(
            model.as_deref(),
            Some(leaf),
            "{qualified}: the model name must survive, since the caller supplied it",
        );
    }
}

/// **HRW's parse of a library file is the compiler's own AST, byte for byte.**
///
/// This is the guard on the whole "second source" question Doug asked: HRW
/// re-reads the declaring file from disk because Rumoca discards source-root
/// text, so there are two paths to what ought to be one artifact. If they can
/// diverge, the Parse tab shows something the compiler never saw.
///
/// **They already agreed on bytes and spans and differed on one field.**
/// `parse_to_ast`'s `file_name` argument is stamped into every `Location`, and
/// passing a basename where the session used the full URI made **400 of 400**
/// MSL documents differ. Passing `&uri` makes it **0 of 400**. Nothing about
/// that is self-evident, which is why it is measured rather than assumed.
///
/// Serialised comparison rather than structural: it is the serialised form
/// that reaches the stage tree, so it is the form whose agreement matters.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn hrw_reparse_of_a_library_file_matches_the_sessions_own_ast() {
    let mut w = shared_worker().lock().unwrap_or_else(|e| e.into_inner());
    // Any compile populates the session with the MSL documents.
    let _ = w.compile_model_by_name("Modelica.Electrical.Analog.Basic.Resistor", &|_| {});

    let uris: Vec<String> = w
        .session
        .document_uris()
        .into_iter()
        .map(str::to_owned)
        .collect();

    let mut compared = 0usize;
    // A sample, not the whole 2,553: this runs in the pre-commit suite, and
    // the property is uniform -- a divergence in how HRW calls `parse_to_ast`
    // would show in the first handful, not only in the tail.
    for uri in uris.iter().take(120) {
        let Some(doc) = w.session.get_document(uri) else {
            continue;
        };
        let Some(session_ast) = doc.parsed() else {
            continue;
        };
        let Ok(text) = std::fs::read_to_string(uri) else {
            continue;
        };
        let Ok(mine) = rumoca_phase_parse::parse_to_ast(&text, uri) else {
            panic!("{uri}: HRW cannot parse a file the session parsed");
        };
        assert_eq!(
            serde_json::to_string(&mine).unwrap_or_default(),
            serde_json::to_string(session_ast).unwrap_or_default(),
            "{uri}: HRW's re-parse differs from the AST the session holds, so the \
             Parse tab would show something the compiler never saw",
        );
        compared += 1;
    }

    // **Non-vacuity.** Every `continue` above is a silent skip, and a session
    // that produced no readable documents would leave this green.
    assert!(
        compared >= 50,
        "only {compared} documents compared -- too few to have exercised the property",
    );
}

/// A **specimen** compile carries no library source, and must not.
///
/// The pane reads a specimen from its own path so live edits show; seeding the
/// cache from the compile would silently freeze the text at whatever was last
/// compiled, and an edited file that keeps rendering its old contents is a far
/// worse failure than the one being fixed.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn a_specimen_compile_carries_no_library_source() {
    let FromWorker::Compiled { library_source, .. } = compile_specimen_shared("RcCircuit") else {
        panic!("expected Compiled");
    };
    assert!(
        library_source.is_none(),
        "a specimen\u{2019}s pane must keep reading from disk, or edits stop showing",
    );
}

/// **A broken specimen must not poison the next compile.**
///
/// Found 2026-07-29 by auditing the front-end failure payloads. Name resolution runs
/// over the *whole session*, not just the requested model, and a specimen that failed
/// to resolve leaves errors in the session's resolved-state cache. So loading a broken
/// model and then a good one made the good one report **the other file's error** --
/// which would have Claude diagnosing the wrong model entirely, the priority-1
/// failure in `docs/tech-debt.md`.
///
/// `remove_document` does *not* clear it, despite
/// `apply_document_removal_at_revision` calling `invalidate_resolved_state`.
/// Rebuilding the session does; that is the mitigation, guarded on the previous
/// compile having actually failed so the reparse is paid only when it buys something.
/// The root cause is inside Rumoca's cache and is logged as an upstream issue.
///
/// Uses a **fresh** `WorkerState` rather than the shared one, so this cannot pass or
/// fail because of what other tests happened to compile first.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn a_broken_specimen_does_not_poison_the_next_compile() {
    let mut w = WorkerState::new();
    w.load_libraries(msl_roots()).expect("load MSL");
    let dir = format!("{}/specimens", env!("CARGO_MANIFEST_DIR"));
    let resolve_note = |w: &mut WorkerState, name: &str| -> (bool, String) {
        let path = PathBuf::from(format!("{dir}/{name}.mo"));
        match w.compile(&path, &|_: FromWorker| {}) {
            FromWorker::Compiled { stages, .. } => {
                let st = stages.get(StageKind::Resolve);
                (st.note_is_error(), st.note.clone().unwrap_or_default())
            }
            _ => panic!("expected Compiled for {name}"),
        }
    };

    let (failed, _) = resolve_note(&mut w, "CapacitorLoop");
    assert!(!failed, "CapacitorLoop resolves cleanly on its own");

    let (failed, note) = resolve_note(&mut w, "UndefinedRef");
    assert!(failed, "UndefinedRef references an undeclared name");
    assert!(note.contains("missingGain"), "and says which one: {note}");

    // The moment of truth: the same good specimen, compiled after the broken one.
    let (failed, note) = resolve_note(&mut w, "CapacitorLoop");
    assert!(
        !failed,
        "a good model must not inherit the previous specimen's failure: {note}",
    );
    assert!(
        !note.contains("missingGain"),
        "`missingGain` appears only in UndefinedRef.mo; leaking it here would have \
         Claude diagnosing the wrong file: {note}",
    );
}

/// A memoised compile equals a fresh one, stage for stage.
///
/// **This is the price of `docs/ideas.md` #48, paid deliberately.** Memoising
/// specimens took the full suite from 375s to about 100s, but it *weakens* the
/// suite: before, the second test to ask for `Drivetrain` re-verified that
/// compiling it produced the same thing. Now it gets a copy of the first answer,
/// so nothing checks reproducibility, and a compiler that had become
/// non-deterministic would sail through a green run.
///
/// So one test keeps doing what the others stopped doing. It compares every
/// compilation stage's **emitted JSON** rather than a summary, because that tree
/// is what HRW renders, what the bridge publishes and what the fidelity checks
/// read -- a difference invisible there is invisible everywhere that matters.
///
/// **Two back-to-back uncached compiles, not memo-versus-fresh.** The first
/// version compared the memo against a fresh compile and failed on Resolve in
/// the full suite while passing alone — because those two compiles happen at
/// *different points in the session's life*, and what a compile sees depends on
/// what the session has already done. That difference is a property of the
/// session, not non-determinism, so the comparison could never have been stable.
///
/// **This used to say the shared session "accumulates every specimen the suite
/// has touched", and that is not the mechanism** *(corrected 2026-08-21)*. The
/// session holds at most one specimen document — `compile_target` removes the
/// previous one — so the carried-over state is the session's *resolved* state,
/// not a pile of documents. **Which part of it produces the difference is not
/// established here**; the observation that it differs is what this test is
/// built on, and that is unchanged.
/// Compiling twice in a row holds the session constant and isolates the property
/// actually at issue. *(The session-dependence itself is logged in
/// `docs/tech-debt.md`; it is adjacent to upstream issue 1.)*
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn compiling_a_specimen_twice_is_reproducible() {
    let memoised = compile_specimen_uncached("Drivetrain");
    let fresh = compile_specimen_uncached("Drivetrain");

    let (
        FromWorker::Compiled {
            stages: a,
            def_index: da,
            ..
        },
        FromWorker::Compiled {
            stages: b,
            def_index: db,
            ..
        },
    ) = (&memoised, &fresh)
    else {
        panic!("expected Compiled from both");
    };

    for kind in StageKind::COMPILATION {
        let (sa, sb) = (a.get(*kind), b.get(*kind));
        assert_eq!(
            sa.outcome,
            sb.outcome,
            "{} outcome differs between a memoised and a fresh compile",
            kind.name(),
        );
        assert_eq!(
            sa.value.is_some(),
            sb.value.is_some(),
            "{} presence differs between a memoised and a fresh compile",
            kind.name(),
        );
        if sa.value != sb.value {
            panic!(
                "{} emits different JSON on a fresh compile — memoisation is hiding \
                 non-determinism, which is exactly what this test exists to catch",
                kind.name(),
            );
        }
    }
    assert_eq!(da.len(), db.len(), "def_index size differs");

    // Non-vacuity: comparing two empty pipelines proves nothing.
    assert!(
        StageKind::COMPILATION
            .iter()
            .filter(|k| a.get(**k).value.is_some())
            .count()
            >= 8,
        "expected a substantially compiled Drivetrain; got mostly empty stages",
    );
}

/// An unbalanced model reports its balance, not just "DAE construction failed".
///
/// #45 step 2. Until 2026-07-29 this failure path returned a bare informational
/// note while `error`, `error_code` and `diagnostics` sat in scope unused — making
/// the **most common Modelica authoring error** (declare a variable, forget its
/// equation) the least informative failure in the pipeline.
///
/// **This test is also the tripwire for the message-format parse.** The structured
/// counts are recovered from Rumoca's display string, because `rumoca-compile`
/// stringifies the typed `ToDaeError::Unbalanced` at its boundary. If that wording
/// changes, this fails loudly instead of the fields silently disappearing.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn an_unbalanced_model_reports_its_balance() {
    let FromWorker::Compiled { stages, .. } = compile_specimen_shared("UnbalancedShaft") else {
        panic!("expected Compiled");
    };

    // **`dae`, not `flatten` — corrected 2026-08-25.** This test was written on
    // 2026-07-29, when `flatten_stage` adopted the `FailedPhase::ToDae` error
    // because DAE construction had no tab of its own. It has had one since
    // 2026-08-03, and *both* stages rendered the payload until the C20 fix removed
    // the adoption. Everything this test is for is unchanged — the balance counts
    // survive, and the message-format parse keeps its tripwire — it was simply
    // reading them off the stage next door.
    let dae = &stages.dae;
    assert!(
        dae.note_is_error(),
        "a failed DAE construction is an error, not an info note"
    );
    let err = dae
        .value
        .as_ref()
        .and_then(|v| v.get("error"))
        .expect("the failure must carry a structured payload");

    assert_eq!(err["kind"], "dae_construction");
    assert_eq!(err["error_code"], "rumoca::todae::ED001");
    // 2 equations for 3 unknowns: `tau` is declared and never determined.
    assert_eq!(
        err["n_equations"], 2,
        "parsed from the message: {}",
        err["message"]
    );
    assert_eq!(
        err["n_unknowns"], 3,
        "parsed from the message: {}",
        err["message"]
    );
    assert_eq!(err["balance"], -1);
    assert!(
        err["reading"]
            .as_str()
            .is_some_and(|r| r.contains("nothing to determine it")),
        "the direction of the imbalance is the actionable half: {}",
        err["reading"],
    );

    // **And the stage before it must no longer claim the failure.** One pipeline
    // stop, one stage reporting it — the invariant
    // `the_corpus_outcome_matrix_is_unchanged` now holds for every specimen.
    assert!(
        !stages.flatten.note_is_error(),
        "flatten did not fail \u{2014} reaching ToDae requires it to have succeeded",
    );
    assert!(
        stages.flatten.value.is_none(),
        "flatten must not carry a second copy of DAE construction's payload",
    );
}

/// **The stage that failed must not be the quietest one.**
///
/// On `UnbalancedShaft` every stage downstream of DAE construction said "not
/// reached (ToDae failed earlier)", and the DAE tab — the phase that actually
/// refused — rendered blank. The attribution was a leftover: `flatten_stage`
/// adopted the `FailedPhase::ToDae` error in 2026-07-29 because Flatten was then
/// the last tab before Structural, so **the succeeding stage reported the
/// failure and the failing stage reported nothing.**
///
/// Found by walking `docs/fixture-labs/dae-construction.md`, whose
/// counterexample stop opens this tab expecting an explanation — the pane-is-a-
/// reporter rule reaching a pane that was already shipping.
///
/// **Checks the property, not the message**: any stage that produced no IR must
/// say something, and the one that failed must say at least as much as the ones
/// that merely never ran.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn the_dae_stage_explains_its_own_absence() {
    let FromWorker::Compiled { stages, .. } = compile_specimen_shared("UnbalancedShaft") else {
        panic!("expected Compiled");
    };

    let dae = &stages.dae;
    assert!(
        dae.note_is_error(),
        "DAE construction refused; its own tab must record that as an error, not silence",
    );
    let err = dae
        .value
        .as_ref()
        .and_then(|v| v.get("error"))
        .expect("the DAE tab must carry the structured payload of its own failure");
    assert_eq!(err["kind"], "dae_construction");
    assert_eq!(err["error_code"], "rumoca::todae::ED001");
    assert_eq!(err["n_equations"], 2);
    assert_eq!(err["n_unknowns"], 3);
    assert_eq!(err["balance"], -1);

    // The property. Every stage with no IR explains itself, and the DAE — the
    // one that failed — is not the silent member of that set.
    for &kind in StageKind::COMPILATION {
        let s = stages.get(kind);
        if s.value.is_some() {
            continue;
        }
        assert!(
            s.note.is_some(),
            "{} produced no IR and gave no reason — an empty pane with no note is \
             indistinguishable from a pane that is still loading",
            kind.name(),
        );
    }

    // Non-vacuity: this specimen must actually fail where the test assumes.
    assert!(
        stages
            .structural
            .note
            .as_deref()
            .is_some_and(|n| n.contains("ToDae")),
        "UnbalancedShaft must still fail in ToDae, or this test is checking nothing",
    );
}

/// The balance parse yields nothing rather than something wrong.
#[test]
fn the_balance_parse_is_absent_rather_than_wrong() {
    assert_eq!(
        parse_unbalanced("unbalanced model: 2 equations, 3 unknowns (balance = -1)"),
        Some((2, 3, -1)),
    );
    // Any deviation returns None, so a reworded message loses the structured
    // fields and never invents them. A wrong number reads as authoritative — the
    // lesson of the `rank_deficiency` bug.
    assert!(parse_unbalanced("internal todae error: something else").is_none());
    assert!(
        parse_unbalanced("unbalanced model: two equations, 3 unknowns (balance = -1)").is_none()
    );
    assert!(parse_unbalanced("unbalanced model: 2 equations, 3 unknowns balance = -1").is_none());
}

/// A structural failure is reported **in terms of Doug's source** (ideas #45).
///
/// This is the whole diagnostic claim: "unknown `gnd.p.i`" tells Doug nothing
/// about the model he wrote, while "line 9, `connect(src.n, gnd.p);`" is a
/// diagnosis. `StructuralError::Singular` has carried `unmatched_unknown_spans`
/// all along; HRW dropped it until 2026-07-29.
///
/// `CapacitorLoop` is the specimen for this because it fails structurally **and
/// stays failed** after index reduction — a capacitor straight across an ideal
/// source is genuinely ill-posed, not merely high-index. `MotorWithBrake` and
/// `Drivetrain` are also singular but get rescued, so neither is a diagnostic case.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn a_structural_failure_names_the_source_line() {
    let FromWorker::Compiled { stages, .. } = compile_specimen_shared("CapacitorLoop") else {
        panic!("expected Compiled");
    };

    for (label, stage) in [
        ("structural", &stages.structural),
        ("index_reduction", &stages.index_reduction),
    ] {
        let err = stage
            .value
            .as_ref()
            .and_then(|v| v.get("error"))
            .unwrap_or_else(|| panic!("{label} should be singular for CapacitorLoop"));

        let locs = err
            .get("unmatched_unknown_locations")
            .and_then(|v| v.as_array())
            .unwrap_or_else(|| panic!("{label} must carry unmatched_unknown_locations"));
        assert_eq!(locs.len(), 1, "{label}: one unmatched unknown");

        let entry = &locs[0];
        assert_eq!(entry["unknown"], "gnd.p.i", "{label}");
        let loc = &entry["location"];
        assert!(
            !loc.is_null(),
            "{label}: the unknown must have a source location"
        );
        assert_eq!(
            loc["line"], 9,
            "{label}: gnd.p.i traces to the ground connect()"
        );
        assert!(
            loc["line_text"]
                .as_str()
                .is_some_and(|t| t.contains("connect(src.n, gnd.p)")),
            "{label}: line_text must be quotable back at Doug: {loc:?}",
        );
    }
}

/// Rank deficiency comes from the **error's own** counts, not from whatever
/// incidence the caller happened to pass.
///
/// Regression test for a wrong number found 2026-07-29. The field used to read
/// `inc.n_eq.max(inc.n_var) - n_matched`, and `index_reduction_stage` passes the
/// *raw* incidence while its error describes the *reduced* system — so
/// `CapacitorLoop` reported a deficiency of **7** (14 raw equations minus 7
/// reduced matches) where the truth is 1.
///
/// A wrong number is worse than a missing one: it reads as authoritative, and
/// Claude would have quoted it straight into an answer.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn rank_deficiency_is_consistent_with_its_own_counts() {
    let FromWorker::Compiled { stages, .. } = compile_specimen_shared("CapacitorLoop") else {
        panic!("expected Compiled");
    };
    for (label, stage) in [
        ("structural", &stages.structural),
        ("index_reduction", &stages.index_reduction),
    ] {
        let err = stage
            .value
            .as_ref()
            .and_then(|v| v.get("error"))
            .expect(label);
        let n_eq = err["n_equations"].as_u64().expect("n_equations");
        let n_var = err["n_unknowns"].as_u64().expect("n_unknowns");
        let n_matched = err["n_matched"].as_u64().expect("n_matched");
        let deficiency = err["rank_deficiency"].as_u64().expect("rank_deficiency");
        assert_eq!(
            deficiency,
            n_eq.max(n_var) - n_matched,
            "{label}: deficiency must follow from the counts beside it",
        );
        assert_eq!(
            deficiency, 1,
            "{label}: CapacitorLoop is one short, before and after"
        );
    }
}

/// A **singular** structural report still produces a matching animation, and
/// that animation ends on the failure (ideas #44).
///
/// This is the claim the #44 fix rests on. Until 2026-07-29 the `Matching ▶`
/// sub-tab was hidden whenever the Structural stage was singular, so the one
/// view that shows *why* a rank deficiency exists was unreachable exactly when
/// it mattered. Nothing had to be built to fix it — the trace already emits
/// `MatchingStep::EquationFailed` and the view already paints the failed row —
/// but nothing tested it either, which is how it stayed hidden.
///
/// Guards two regressions: `from_report` learning to bail on a report that
/// carries an `error`, and the trace stopping short instead of recording the
/// give-up.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn a_singular_report_still_animates_and_ends_on_the_failure() {
    use rumoca_phase_structural::matching::MatchingStep;

    let FromWorker::Compiled { stages, .. } = compile_specimen_shared("MotorWithBrake") else {
        panic!("expected Compiled");
    };
    let report = stages
        .structural
        .value
        .as_ref()
        .expect("a structural report");
    assert!(
        report.get("error").is_some(),
        "MotorWithBrake's raw structural stage is expected to be singular",
    );

    let mat = crate::incidence_view::IncidenceMatrix::from_report(report)
        .expect("a singular report still carries an incidence matrix");
    let anim = crate::matching_anim::MatchingAnimation::from_incidence(&mat);

    let failures = anim.failed_equations();
    assert_eq!(
        failures.len(),
        1,
        "a deficiency of 1 means exactly one equation gives up: {failures:?}",
    );

    let progress = anim
        .match_progress()
        .expect("a recorded animation has frames, so progress is known");
    assert_eq!(progress, (47, 48), "47 of 48 matched");

    // The give-up must be *recorded*, not merely implied by the count.
    assert!(
        anim.steps()
            .iter()
            .any(|s| matches!(s, MatchingStep::EquationFailed(_))),
        "the trace must record the equation it gave up on",
    );
}

/// The connection-expansion replay reaches HRW with real frames (MLS §9).
///
/// End to end through the worker for the same reason the `pre()` test is:
/// the interesting part is **where the frames come from**.
///
/// Until 2026-08-04 they came from a replay — HRW re-ran instantiate +
/// typecheck + flatten with an observer, because the session's compile
/// flattened without one. Get that sequence wrong (skip the typecheck, use
/// different `FlattenOptions`) and the result was silently zero frames, or
/// frames describing a flatten that never happened.
///
/// They now come from **the compile itself**, through
/// `rumoca-phase-flatten`'s capture scope. This test still earns its keep:
/// it is what proves the scope is opened and taken around the right call, and
/// a non-zero count here is the only evidence the animation is fed at all.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn connection_frames_reach_hrw_from_the_real_compile() {
    use rumoca_phase_flatten::connections::trace::ConnectionStep;

    let FromWorker::Compiled {
        connection_frames, ..
    } = compile_specimen_shared("RcCircuit")
    else {
        panic!("expected Compiled");
    };
    assert!(
        !connection_frames.is_empty(),
        "RcCircuit wires four components together with connect()",
    );

    // Bookends, so a truncated trace is not mistaken for a short model.
    assert!(
        matches!(
            connection_frames.first().map(|f| &f.step),
            Some(ConnectionStep::Start { .. }),
        ),
        "{:?}",
        connection_frames.first(),
    );
    let Some(ConnectionStep::Complete {
        sets,
        equations_added,
    }) = connection_frames.last().map(|f| f.step.clone())
    else {
        panic!(
            "last frame must be Complete: {:?}",
            connection_frames.last()
        );
    };
    assert!(sets > 0, "an RC circuit has connection sets");
    assert!(equations_added > 0, "and they produce equations");

    // The asymmetry must be present in a real model, not just in the unit
    // test's hand-built frames: some potential set yields more than one
    // equation, and every flow set yields exactly one.
    let generated: Vec<(&str, usize, usize)> = connection_frames
        .iter()
        .filter_map(|f| match &f.step {
            ConnectionStep::EquationsGenerated {
                kind,
                set_size,
                equations_added,
                ..
            } => Some((*kind, *set_size, *equations_added)),
            _ => None,
        })
        .collect();
    assert!(
        generated
            .iter()
            .any(|(k, n, e)| *k == "potential" && *n > 2 && *e == n - 1),
        "a potential set of n must yield n-1 equalities: {generated:?}",
    );
    assert!(
        generated
            .iter()
            .filter(|(k, ..)| *k == "flow")
            .all(|(_, _, e)| *e == 1),
        "every flow set yields exactly one sum-to-zero equation: {generated:?}",
    );

    // The running total must land on what Complete reported.
    assert_eq!(
        connection_frames.last().unwrap().equations_so_far,
        equations_added,
        "the running count and the Complete frame must agree",
    );

    // **The lane view's grouping agrees with Rumoca's own set count.**
    //
    // `Lanes` groups frames by kind for display, which is presentation — but a
    // grouping that dropped or duplicated a set would be presenting a different
    // decomposition than the compiler built, and would look entirely plausible.
    // Comparing against the `Complete` frame's `sets` is the check that cannot be
    // satisfied by a well-formed mistake: the number comes from Rumoca, not from
    // counting the same frames a second way.
    let lanes = crate::connection_anim::Lanes::upto(
        &connection_frames,
        connection_frames.len().saturating_sub(1),
    );
    let declared_sets = connection_frames
        .iter()
        .find_map(|f| match f.step {
            ConnectionStep::Complete { sets, .. } => Some(sets),
            _ => None,
        })
        .expect("the pass must report Complete");
    assert_eq!(
        lanes.set_count(),
        declared_sets,
        "the lane view shows {} sets; Rumoca reported {declared_sets}",
        lanes.set_count(),
    );
    assert!(
        lanes.lanes.len() >= 2,
        "RcCircuit has both potential and flow sets, so a single lane means the \
         kinds were merged: {:?}",
        lanes.lanes.iter().map(|l| &l.kind).collect::<Vec<_>>(),
    );
}

/// The `pre()` lowering replay reaches HRW with real frames (idea #40).
///
/// End to end through the worker, because the interesting part is *where the
/// frames come from*: the pass runs inside DAE construction, so the compiled
/// DAE is already past it and the worker has to re-run construction over
/// `cr.flat` to see anything. A unit test on the animation type would not
/// have caught getting that wrong — it would just have shown zero frames.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn pre_lowering_frames_reach_hrw_from_the_real_compile() {
    use rumoca_phase_dae::PreLoweringStep;

    let FromWorker::Compiled {
        pre_lowering_frames,
        flat,
        ..
    } = compile_specimen_shared("MotorWithBrake")
    else {
        panic!("expected Compiled");
    };
    assert!(
        flat.is_some(),
        "the flat model must be carried for live replay"
    );
    assert!(
        !pre_lowering_frames.is_empty(),
        "MotorWithBrake uses pre() via its when-equation"
    );

    let named: Vec<(String, String)> = pre_lowering_frames
        .iter()
        .filter_map(|f| match &f.step {
            PreLoweringStep::Named { base, slot } => Some((base.clone(), slot.clone())),
            _ => None,
        })
        .collect();
    assert!(
        named
            .iter()
            .any(|(b, s)| b == "overSpeed" && s == "__pre__.overSpeed"),
        "the slot the Events IR references must be seen being named: {named:?}",
    );

    // The pass runs twice per compile, and the second run creates nothing.
    // That was *mis*-stated as the opposite until the instrumentation showed
    // otherwise, so it is pinned here rather than left to memory.
    let completions: Vec<usize> = pre_lowering_frames
        .iter()
        .filter_map(|f| match &f.step {
            PreLoweringStep::Complete { slots_created } => Some(*slots_created),
            _ => None,
        })
        .collect();
    assert_eq!(
        completions.len(),
        2,
        "the pass runs twice per compile: {completions:?}"
    );
    assert!(completions[0] > 0, "the first pass creates the slots");
    assert_eq!(completions[1], 0, "the second finds nothing left to lower");
}

/// Following an identifier must survive **real** IR, whatever is in it.
///
/// Regression for a crash on the simplest possible action: open
/// MotorWithBrake, click `overSpeed` in the source. Following walks every
/// stage's IR and lexes each code-bearing string, and MotorWithBrake's
/// structural note contains an em dash. The lexer stepped one *byte* over
/// non-ASCII, so a token boundary landed inside that character and slicing
/// it panicked — see `modelica_lex::bare_non_ascii_lexes_on_character_boundaries`.
///
/// The synthetic tests could not have caught it: they lex Modelica, which
/// is ASCII. Only prose written by the compiler reaches the lexer with an
/// em dash in it, and only because following searches IR strings.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn following_an_identifier_walks_every_stage_without_panicking() {
    let FromWorker::Compiled { stages, .. } = compile_specimen_shared("MotorWithBrake") else {
        panic!("expected Compiled");
    };
    let pairs = stages.as_stage_pairs();
    for name in ["overSpeed", "__pre__.overSpeed", "emf.phi", "der(emf.phi)"] {
        let t = crate::bridge::Tracking {
            seq: 1,
            name,
            declared_line: None,
            declaring_class: None,
            stage_values: &pairs,
        };
        crate::bridge::summarize_tracking(&t);
    }

    let t = crate::bridge::Tracking {
        seq: 1,
        name: "overSpeed",
        declared_line: None,
        declaring_class: None,
        stage_values: &pairs,
    };
    let (mentions, stage_count) = crate::bridge::summarize_tracking(&t);
    assert!(mentions > 0, "overSpeed is declared in MotorWithBrake");
    assert!(stage_count > 1, "it should survive past a single stage");
}

// -----------------------------------------------------------------------
// Full-pipeline regression guards: every specimen compiles through ALL
// expected stages. These are the most rebase-sensitive tests — if an
// upstream Rumoca change breaks a phase or renames an API, at least one
// of these will catch it.
// -----------------------------------------------------------------------

/// Every specimen that should compile through solve lowering does so
/// (all stages produce IR). The three known exceptions are tested separately:
/// CapacitorLoop (structurally singular, irreducible) and OverInitRc
/// (over-determined init) still produce partial pipelines.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn all_healthy_specimens_compile_through_solve_lowering() {
    let healthy = [
        "SingleInertia",
        "RotationalInertia",
        "ProportionalLoop",
        "NonlinearLoop",
        "MixedLoop",
        "TwoLoops",
        "Drivetrain",
        "RcCircuit",
        "BouncingBall",
        "BenchActuator",
    ];
    for name in healthy {
        let FromWorker::Compiled { model, stages, .. } = compile_specimen_shared(name) else {
            panic!("{name}: expected Compiled");
        };
        assert!(model.is_some(), "{name}: model name not extracted");
        assert!(
            stages.parse.value.is_some(),
            "{name}: parse failed: {:?}",
            stages.parse.note
        );
        assert!(
            stages.resolve.value.is_some(),
            "{name}: resolve failed: {:?}",
            stages.resolve.note
        );
        assert!(
            stages.instantiate.value.is_some(),
            "{name}: instantiate failed: {:?}",
            stages.instantiate.note
        );
        assert!(
            stages.typecheck.value.is_some(),
            "{name}: typecheck failed: {:?}",
            stages.typecheck.note
        );
        assert!(
            stages.flatten.value.is_some(),
            "{name}: flatten failed: {:?}",
            stages.flatten.note
        );
        assert!(
            stages.index_reduction.value.is_some(),
            "{name}: index reduction failed: {:?}",
            stages.index_reduction.note
        );
        assert!(
            stages.events.value.is_some(),
            "{name}: events failed: {:?}",
            stages.events.note
        );
        assert!(
            stages.solve_lowering.value.is_some(),
            "{name}: solve lowering failed: {:?}",
            stages.solve_lowering.note
        );
    }
}

/// Every specimen that compiles through solve lowering also simulates
/// successfully — the end-to-end path from source to trajectories.
/// RcCircuit is excluded: it compiles but the BDF solver hits a step-size
/// floor (stiff RC with the default tolerances).
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn all_healthy_specimens_simulate() {
    let healthy = [
        "SingleInertia",
        "RotationalInertia",
        "ProportionalLoop",
        "NonlinearLoop",
        "MixedLoop",
        "TwoLoops",
        "Drivetrain",
        "BouncingBall",
        "BenchActuator",
    ];
    for name in healthy {
        let data = {
            let mut w = shared_worker().lock().unwrap_or_else(|e| e.into_inner());
            let path = PathBuf::from(format!(
                "{}/specimens/{name}.mo",
                env!("CARGO_MANIFEST_DIR")
            ));
            w.simulate(CompileTarget::File(&path), name, 1.0, &|_: FromWorker| {})
        };
        let data = data.unwrap_or_else(|e| panic!("{name}: simulate failed: {e}"));
        assert!(!data.times.is_empty(), "{name}: no time points");
        assert!(!data.names.is_empty(), "{name}: no output variables");
        assert_eq!(
            data.data.len(),
            data.names.len(),
            "{name}: data/names length mismatch"
        );
    }
}

/// The headless `compile_specimen` path (used by `gen_trace`) produces the same
/// stages as compiling through the shared worker.
///
/// **IT NEVER TOUCHED THE WORKER UNTIL 2026-08-21.** The name and the doc both
/// claimed a comparison; the body asserted four `is_some()`s on the headless
/// path alone, and paid a full MSL load to do it. It would have passed with the
/// two paths producing entirely different IR, and with `compile_specimen`
/// silently compiling the wrong model.
///
/// **It does NOT catch `docs/ideas.md` #48 lever B, and an earlier draft of this
/// comment claimed it did.** B compiles MSL-free specimens in a bare session,
/// which renumbers DefIds while leaving every stage's roster identical — so the
/// roster comparison below is blind to it *by construction*. **The check that
/// catches B is `--features notebook-check`**, which compares emitted JSON.
///
/// **Compares the stage rosters, not the JSON**, and that limit is deliberate
/// rather than timid: the two compiles happen in *different sessions* — one
/// virgin, one shared — and `CLAUDE.md` records that a stage's emitted JSON
/// depends on what the session already holds (`GearWithBrake`,
/// `MissingComponentClass`). Demanding byte-equality would encode a claim known
/// to be false and fail in company while passing alone, which is the exact trap
/// `compiling_a_specimen_twice_is_reproducible` was restructured to escape.
///
/// So what is checked is what *is* session-independent: the same model name, and
/// every stage that produced IR on one path producing IR on the other.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn compile_specimen_headless_matches_worker() {
    let headless = compile_specimen(
        Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/specimens/SingleInertia.mo"
        )),
        msl_roots(),
    )
    .expect("compile_specimen");
    let FromWorker::Compiled {
        model: headless_model,
        stages: headless_stages,
        ..
    } = headless
    else {
        panic!("expected Compiled from the headless path");
    };

    let FromWorker::Compiled {
        model: worker_model,
        stages: worker_stages,
        ..
    } = compile_specimen_shared("SingleInertia")
    else {
        panic!("expected Compiled from the shared worker");
    };

    assert_eq!(headless_model.as_deref(), Some("SingleInertia"));
    assert_eq!(
        headless_model, worker_model,
        "the two paths named different models",
    );

    // Non-vacuity first: if neither path produced IR, every comparison below is
    // satisfied by two empty compiles.
    assert!(
        headless_stages.parse.value.is_some()
            && headless_stages.resolve.value.is_some()
            && headless_stages.flatten.value.is_some()
            && headless_stages.solve_lowering.value.is_some(),
        "the headless path did not reach solve lowering: resolve note {:?}",
        headless_stages.resolve.note,
    );

    for &kind in StageKind::COMPILATION {
        assert_eq!(
            headless_stages.get(kind).value.is_some(),
            worker_stages.get(kind).value.is_some(),
            "{kind:?} produced IR on one path and not the other -- headless note \
             {:?}, worker note {:?}",
            headless_stages.get(kind).note,
            worker_stages.get(kind).note,
        );
    }
}

/// The headless `simulate_specimen` path (used by gen_trace) runs and
/// produces trajectories.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn simulate_specimen_headless_produces_trajectories() {
    let data = simulate_specimen(
        Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/specimens/SingleInertia.mo"
        )),
        "SingleInertia",
        2.0,
        msl_roots(),
    )
    .expect("simulate_specimen");
    assert!(!data.times.is_empty());
    assert!(
        data.names.iter().any(|n| n == "w"),
        "expected 'w' in output names"
    );
}

/// **An MSL model simulates.** The path that did not exist until 2026-08-04.
///
/// Doug pressed Run on `Modelica.Blocks.Continuous.SecondOrder` and got *"read
/// error: The system cannot find the file specified. (os error 2)"*. For a
/// library model the UI's `selected` holds the **qualified name**, not a file, and
/// `simulate` opened with `read_to_string(path)` — so simulation had never worked
/// for anything in the corpus. The compile path had gained
/// `CompileLibraryModel`; the simulate path never got its counterpart.
///
/// **Why it survived: nothing headless could reach it.** Every simulate test went
/// through `simulate_specimen`, which takes a `&Path` and therefore cannot express
/// a library model. The gap was not un-tested, it was **un-testable** — so
/// `simulate_library_model` exists as much for this test as for the UI, and the
/// two halves landed together.
///
/// `SecondOrder` deliberately: it is the model Doug reported, it is a genuine
/// second-order system with states to plot, and `Blocks/Continuous.mo` declares
/// several classes — which is exactly the case where re-deriving the qualified
/// name from the declaring file would pick the wrong one.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn an_msl_library_model_simulates() {
    let data = simulate_library_model("Modelica.Blocks.Continuous.SecondOrder", 1.0, msl_roots())
        .expect("SecondOrder must simulate through the library path");
    assert!(
        !data.times.is_empty(),
        "the solver returned no time points for SecondOrder",
    );
    // Non-vacuity on the *content*: an empty name list would satisfy the above
    // while telling a reader nothing was actually integrated.
    assert!(
        !data.names.is_empty(),
        "SecondOrder simulated but produced no named trajectories",
    );
}

// -----------------------------------------------------------------------
// Stage-specific content guards: verify that key JSON fields are present
// in each stage's IR. These catch Rumoca IR renames or restructurings.
// -----------------------------------------------------------------------

/// The Flatten stage IR for a simple model has the expected top-level
/// structure: variables, equations, and the flat model fields.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn flatten_ir_has_expected_structure() {
    let FromWorker::Compiled { stages, .. } = compile_specimen_shared("SingleInertia") else {
        panic!("expected Compiled");
    };
    let v = stages.flatten.value.expect("flatten IR");
    assert!(
        v.get("variables").is_some(),
        "flat IR should have 'variables'"
    );
    assert!(
        v.get("equations").is_some(),
        "flat IR should have 'equations'"
    );
}

/// The Events stage IR has the expected summary structure.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn events_ir_has_expected_summary_keys() {
    let FromWorker::Compiled { stages, .. } = compile_specimen_shared("BouncingBall") else {
        panic!("expected Compiled");
    };
    let v = stages.events.value.expect("events IR");
    let summary = v["summary"].as_object().expect("summary object");
    for key in [
        "condition_equations",
        "relations",
        "discrete_real_updates",
        "discrete_valued_updates",
        "zero_crossing_conditions",
        "scheduled_time_events",
    ] {
        assert!(
            summary.contains_key(key),
            "events summary missing key: {key}"
        );
    }
}

/// The Solve-lowering IR has the expected top-level fields.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn solve_lowering_ir_has_expected_fields() {
    let FromWorker::Compiled { stages, .. } = compile_specimen_shared("SingleInertia") else {
        panic!("expected Compiled");
    };
    let v = stages.solve_lowering.value.expect("solve lowering IR");
    assert!(
        v.get("problem").is_some(),
        "SolveModel should have 'problem'"
    );
    assert!(
        v.get("variable_meta").is_some(),
        "SolveModel should have 'variable_meta'"
    );
}

/// The Structural stage IR has matching, blocks, and incidence matrix.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn structural_ir_has_incidence_matrix() {
    let FromWorker::Compiled { stages, .. } = compile_specimen_shared("ProportionalLoop") else {
        panic!("expected Compiled");
    };
    let v = stages.structural.value.expect("structural IR");
    assert!(
        v["matching"].as_array().is_some_and(|a| !a.is_empty()),
        "missing matching"
    );
    assert!(
        v["blocks"].as_array().is_some_and(|a| !a.is_empty()),
        "missing blocks"
    );
    let inc = &v["incidence"];
    assert!(
        inc["unknown_names"].as_array().is_some(),
        "incidence missing unknown_names"
    );
    assert!(inc["rows"].as_array().is_some(), "incidence missing rows");
    assert!(inc["n_eq"].as_u64().is_some(), "incidence missing n_eq");
}

#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn structural_incidence_has_equation_text_labels() {
    let FromWorker::Compiled { stages, .. } = compile_specimen_shared("SingleInertia") else {
        panic!("expected Compiled");
    };
    let v = stages.structural.value.expect("structural IR");
    let rows = v["incidence"]["rows"].as_array().expect("incidence rows");
    for row in rows {
        let text = row.get("equation_text").and_then(|v| v.as_str());
        assert!(text.is_some(), "row missing equation_text: {row}");
        let text = text.unwrap();
        assert!(!text.is_empty(), "equation_text should not be empty");
        assert!(
            !text.starts_with("f_x["),
            "equation_text should be readable, not an index label: {text}"
        );
    }
}

/// The Index-reduction stage IR includes the reduction report.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn index_reduction_ir_has_reduction_report() {
    let FromWorker::Compiled { stages, .. } = compile_specimen_shared("Drivetrain") else {
        panic!("expected Compiled");
    };
    let v = stages.index_reduction.value.expect("index reduction IR");
    let red = &v["reduction"];
    assert!(red.is_object(), "should have a reduction report");
    assert!(red.get("steps").is_some(), "reduction should have steps");
    assert!(
        red.get("n_states_before").is_some(),
        "reduction should have n_states_before"
    );
    assert!(
        red.get("n_states_after").is_some(),
        "reduction should have n_states_after"
    );
    assert!(
        red.get("funnel_completed").is_some(),
        "reduction should have funnel_completed"
    );
}

/// The Initialization stage IR includes the determinacy check.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn initialization_ir_has_determinacy() {
    let FromWorker::Compiled { stages, .. } = compile_specimen_shared("RcCircuit") else {
        panic!("expected Compiled");
    };
    let v = stages.initialization.value.expect("initialization IR");
    let det = &v["determinacy"];
    assert!(det.is_object(), "should have a determinacy section");
    for key in [
        "states",
        "initial_equations",
        "fixed_start_states",
        "explicit_initial_conditions",
        "surplus_over_states",
        "verdict",
    ] {
        assert!(det.get(key).is_some(), "determinacy missing key: {key}");
    }
}

// -----------------------------------------------------------------------
// Utility function guards
// -----------------------------------------------------------------------

/// `is_def_id_key` recognizes the three DefId field names.
#[test]
fn is_def_id_key_recognizes_all_three() {
    assert!(is_def_id_key("def_id"));
    assert!(is_def_id_key("type_def_id"));
    assert!(is_def_id_key("base_def_id"));
    assert!(!is_def_id_key("id"));
    assert!(!is_def_id_key("def_id_extra"));
    assert!(!is_def_id_key(""));
}

/// `discontinuity_segments` handles edge cases.
#[test]
fn discontinuity_segments_edge_cases() {
    assert_eq!(discontinuity_segments(&[]).len(), 1); // degenerate: one empty segment
    assert_eq!(discontinuity_segments(&[1.0]), vec![0..1]);
    assert_eq!(discontinuity_segments(&[1.0, 1.0, 1.0]), vec![0..3]);
}

/// Compilation produces log entries with the expected stage structure.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn compilation_emits_log_entries() {
    let logs = compile_specimen_logs_shared("SingleInertia");
    let stage_starts: Vec<&str> = logs
        .iter()
        .filter(|e| matches!(e.level, LogLevel::StageStart))
        .map(|e| e.message.as_str())
        .collect();
    let stage_ends: Vec<&str> = logs
        .iter()
        .filter(|e| matches!(e.level, LogLevel::StageEnd))
        .map(|e| e.message.split(" (").next().unwrap_or(""))
        .collect();
    assert!(stage_starts.contains(&"Parse"), "missing Parse stage start");
    assert!(
        stage_starts.contains(&"Resolve"),
        "missing Resolve stage start"
    );
    assert!(
        stage_starts.contains(&"Flatten"),
        "missing Flatten stage start"
    );
    assert!(
        stage_starts.contains(&"Solve lowering"),
        "missing Solve lowering stage start"
    );
    assert_eq!(
        stage_starts.len(),
        stage_ends.len(),
        "every stage start should have a matching end"
    );
    assert!(
        logs.iter().any(|e| matches!(e.level, LogLevel::Info)),
        "should have at least one info entry"
    );

    // **DAE construction is logged, and in its true position.**
    //
    // Doug, 2026-08-04: *"our logs do not report the begin or end of that DAE
    // phase. Worse, our logs contain a fiction about a DAE pipeline which
    // includes the phases which follow the DAE phase."* The stage had a tab, a
    // trace file and a lab, and the log skipped straight from Flatten to
    // Structural — while a bracket labelled "DAE pipeline" claimed to span five
    // phases that come *after* DAE construction.
    //
    // **Order is asserted, not just presence.** Logging it where it used to be
    // built would have reported DAE construction finishing after the phases that
    // consume its output — a second fiction in place of the first.
    let pos = |name: &str| {
        stage_starts
            .iter()
            .position(|s| *s == name)
            .unwrap_or_else(|| panic!("no `{name}` stage start in {stage_starts:?}"))
    };
    assert!(
        pos("Flatten") < pos("DAE construction"),
        "DAE construction must be logged after Flatten: {stage_starts:?}",
    );
    assert!(
        pos("DAE construction") < pos("Structural analysis"),
        "and before the phases that consume the DAE: {stage_starts:?}",
    );
    assert!(
        stage_ends.contains(&"DAE construction"),
        "a phase that starts must also be reported as ending: {stage_ends:?}",
    );

    // **The four phases that ran inside the compile are nested inside it.**
    //
    // Doug, 2026-08-04: *"let's figure out how to show the log lines for
    // Instantiate, Typecheck, Flatten and DAE construction nested within the log
    // lines for Rumoca compile."* They ran inside that call, and the entries are
    // HRW reading out what they produced — so containment is the accurate
    // rendering, and a flat list could only say *adjacent to*.
    //
    // Structural onward is HRW's own work on the DAE, so it must stay OUTSIDE:
    // nesting everything would be as wrong as nesting nothing, in the other
    // direction.
    let depth_of = |name: &str| {
        logs.iter()
            .find(|e| matches!(e.level, LogLevel::StageStart) && e.message == name)
            .unwrap_or_else(|| panic!("no {name} bracket"))
            .depth
    };
    for inside in ["Instantiate", "Typecheck", "Flatten", "DAE construction"] {
        assert!(
            depth_of(inside) > 0,
            "{inside} ran inside the Rumoca compile call and must render nested \
             within it; at depth 0 the log says it happened beside the compile",
        );
    }
    assert_eq!(
        depth_of("Structural analysis"),
        0,
        "Structural analysis is HRW's own work on the DAE, not a reading of the \
         compile's output \u{2014} nesting it would claim the compile performed it",
    );

    // **The fiction stays gone.** Named as a substring so a revival under any
    // wording ("DAE pipeline (flatten -> ...)") is caught.
    for e in &logs {
        assert!(
            !e.message.contains("DAE pipeline"),
            "the DAE is a phase, not a pipeline, and the old bracket claimed a \
             span reaching five phases past it: {:?}",
            e.message,
        );
    }
}

/// **A compile never reports another run's traces.**
///
/// Doug, 2026-08-04: with the tracing checkbox *off*, *"detailed rumoca logs are
/// still included in the log view, but for a smaller subset of compiler phases"* —
/// and with it on, logs appeared *"for only a subset of compiler phases."* One
/// cause: `TRACE_BUFFER` is drained after each Rumoca call, but
/// `to_dae_with_options_traced` ran last with **no drain after it**, so every
/// event it emitted was stranded and reported against the *following* compile.
/// The run that produced them was missing them; the run that showed them had not
/// asked for them.
///
/// **Checks the property, not the one call.** A stranded event is planted
/// directly, so any future undrained Rumoca call is caught by the same test
/// rather than needing its own.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn a_compile_never_reports_another_runs_traces() {
    const STALE: &str = "STRANDED BY A PREVIOUS RUN";

    // Exactly what an undrained Rumoca call leaves behind. Same thread as the
    // compile below, so this is the buffer that compile will see.
    TRACE_BUFFER.with(|b| {
        b.borrow_mut()
            .push((tracing::Level::DEBUG, STALE.to_owned()))
    });

    let logs = std::sync::Mutex::new(Vec::new());
    {
        let mut w = shared_worker().lock().unwrap_or_else(|e| e.into_inner());
        let path = PathBuf::from(format!(
            "{}/specimens/SingleInertia.mo",
            env!("CARGO_MANIFEST_DIR")
        ));
        w.compile(&path, &|msg: FromWorker| {
            if let FromWorker::Log(entry) = msg {
                logs.lock().unwrap().push(entry);
            }
        });
    }
    let logs = logs.into_inner().unwrap();

    // Non-vacuity: the compile must actually have logged, or "no stale entry"
    // is true for the uninteresting reason.
    assert!(
        logs.iter().any(|e| matches!(e.level, LogLevel::StageEnd)),
        "the compile produced no stage entries, so this proves nothing",
    );
    assert!(
        !logs.iter().any(|e| e.message.contains(STALE)),
        "a compile reported a trace event stranded before it began \u{2014} which is \
         how tracing appeared to stay on after being switched off",
    );
    // And it must not still be waiting to ambush the next one.
    assert!(
        TRACE_BUFFER.with(|b| b.borrow().is_empty()),
        "the buffer must be empty when a compile ends, or the next run inherits it",
    );
}

// **The connection-replay parity test was deleted on 2026-08-04**, and its
// deletion is the point. It compared HRW's replay flatten options against
// `flatten_options_for_tree` upstream, because the Connections view was
// populated by a *second* flatten that had to be configured like the first.
//
// There is no second flatten now. The frames come from the compile that
// produced the flat model, so the two cannot disagree — the question the test
// asked can no longer be posed. **A guard removed because the hazard is gone
// beats a guard that passes**, and it is why the Rumoca API change was worth
// making rather than testing around.

/// **HRW's own replays are never logged as phases, and nesting is real.**
///
/// Doug, 2026-08-04: *"that suggests that the connection replay is a fiction,
/// invented for logging … I want the log to be accurate."* He was right. HRW
/// re-runs connection expansion and DAE construction to capture frames for two
/// views; both were reported with `StageStart`/`StageEnd`, which told the reader
/// the compile had steps it does not have.
///
/// **Two properties, because either alone permits the bug.** A bracket named for
/// a replay is a fiction even if the depths are right; and correct names with a
/// flat log cannot express *contained in*, which is what left replays looking
/// like siblings of real phases.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn no_hrw_replay_is_logged_as_a_phase() {
    let logs = compile_specimen_logs_shared("SingleInertia");

    // 1. No bracket names a replay. Checked on the *word*, so a future replay
    //    called "re-run" or "replay" anything is caught without being listed.
    for e in logs
        .iter()
        .filter(|e| matches!(e.level, LogLevel::StageStart | LogLevel::StageEnd))
    {
        let m = e.message.to_lowercase();
        assert!(
            !m.contains("replay") && !m.contains("re-ran") && !m.contains("re-run"),
            "a phase bracket names a replay: {:?}. HRW's re-executions are \
             observation, not steps in the chain \u{2014} report them as Info.",
            e.message,
        );
    }

    // 2. **There is no replay to report.** This assertion was the opposite for
    //    a few hours on 2026-08-04: while HRW still re-ran connection expansion
    //    and DAE construction, hiding that cost would have been its own
    //    inaccuracy, so the test required it to be stated.
    //
    //    Then the Rumoca capture scopes landed and both replays were deleted, so
    //    the honest assertion inverted. **A log that mentions a replay now would
    //    mean one came back** — which would silently double the compile and put
    //    the animations back on data from a second execution.
    assert!(
        !logs.iter().any(|e| e.message.contains("HRW re-ran")),
        "a replay is being performed again. Frames come from the compile itself \
         via the capture scopes; re-running a phase to observe it makes the view \
         describe an execution the user never asked for.",
    );

    // 3. Brackets balance, so the depth a reader sees means something.
    //
    // *That* something is nested is checked in the tracing-on test instead:
    // with tracing off a phase may legitimately emit nothing between its own
    // two lines, so "nothing at depth > 0" is not evidence of a broken counter
    // here. Asserting it in both places would fail on correct behaviour.
    let mut depth: i32 = 0;
    for e in &logs {
        match e.level {
            LogLevel::StageStart => depth += 1,
            LogLevel::StageEnd => depth -= 1,
            _ => {}
        }
        assert!(
            depth >= 0,
            "a StageEnd without a StageStart at {:?}",
            e.message
        );
    }
    assert_eq!(depth, 0, "{depth} phase bracket(s) left unclosed");
}

/// **A compile leaves nothing in the buffer — with tracing actually on.**
///
/// The companion to `a_compile_never_reports_another_runs_traces`, and the half
/// that needs tracing *enabled* to mean anything: with it off, Rumoca emits no
/// events and an empty buffer proves nothing.
///
/// This is what catches a Rumoca call added without a `drain_traces` after it.
/// `to_dae_with_options_traced` was exactly that — a full re-run of DAE
/// construction, the last call in the compile, with nothing to drain it — so its
/// output arrived one compile late for as long as the feature existed.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn a_compile_with_tracing_on_leaves_nothing_behind() {
    let logs = std::sync::Mutex::new(Vec::new());
    let left_behind;
    let sink = |msg: FromWorker| {
        if let FromWorker::Log(entry) = msg {
            logs.lock().unwrap().push(entry);
        }
    };
    {
        let mut w = shared_worker().lock().unwrap_or_else(|e| e.into_inner());
        w.handle(ToWorker::SetTracing(true), &sink);
        let path = PathBuf::from(format!(
            "{}/specimens/SingleInertia.mo",
            env!("CARGO_MANIFEST_DIR")
        ));
        w.compile(&path, &sink);
        // **Sampled here, before the toggle.** `SetTracing(false)` clears the
        // buffer, so asserting after it would assert the cleanup rather than the
        // compile — the first version of this test did exactly that and passed
        // with the drain removed.
        left_behind = TRACE_BUFFER.with(|b| b.borrow().len());
        // Restore, or every later test on this shared worker runs traced.
        w.handle(ToWorker::SetTracing(false), &sink);
    }
    let logs = logs.into_inner().unwrap();

    // **Non-vacuity, and it is the whole point here**: tracing must have
    // produced something, or "nothing left behind" is trivially true.
    assert!(
        logs.iter().any(|e| matches!(e.level, LogLevel::Trace)),
        "tracing was on and no trace entries were reported \u{2014} this test cannot \
         detect a missing drain unless Rumoca is actually emitting",
    );
    assert_eq!(
        left_behind, 0,
        "a Rumoca call emitted {left_behind} trace event(s) that no drain \
         collected; they would surface under the NEXT compile instead of this one",
    );

    // **A phase's trace appears inside that phase's bracket.**
    //
    // Doug, 2026-08-04: *"rumoca trace log lines tend not to appear between our
    // major phase start/end log lines."* Events are buffered until drained, so a
    // drain placed after a bracket reports that phase's output under whatever
    // comes next — the log read as a wall of trace with the phase structure
    // beside it rather than around it.
    //
    // **Asserted as "at least one, inside"** rather than "all": a phase's crate
    // can legitimately emit during *another* bracket, because HRW replays some
    // work — the connection replay re-runs flatten and typecheck. Requiring all
    // would fail on correct behaviour; requiring none-outside would too. What was
    // actually broken is that *none* landed inside, and that is what this checks.
    let bracket = |name: &str| -> Option<(usize, usize)> {
        // `starts_with`, because a bracket may carry a qualifier after its
        // name ("Rumoca compile — full pipeline; …").
        let start = logs
            .iter()
            .position(|e| matches!(e.level, LogLevel::StageStart) && e.message.starts_with(name))?;
        let end = logs[start..]
            .iter()
            .position(|e| matches!(e.level, LogLevel::StageEnd) && e.message.starts_with(name))
            .map(|o| start + o)?;
        Some((start, end))
    };

    // **Each phase's trace inside the bracket where that phase actually ran.**
    //
    // Which is not always the bracket named after it, and that is the point.
    // Resolve runs in HRW, so its traces belong under `Resolve`. Typecheck runs
    // *inside the session's compile* — HRW's `Typecheck` entry times turning the
    // captured overlay into a view — so its traces belong under `Rumoca compile`.
    //
    // Asserting typecheck traces under the Typecheck bracket was right until
    // 2026-08-04 and became wrong the moment HRW stopped running typecheck
    // itself. The test failing on that change is the test working: it was
    // pinned to where the work happens, and the work moved.
    //
    // **And it moved back the same day**, by a different mechanism: the compile's
    // traces are now split by target and replayed under the phase each names, so
    // `rumoca_phase_typecheck` is under `Typecheck` again — not because HRW runs
    // the phase, but because the line says which phase emitted it.
    for (phase, target) in [
        ("Typecheck", "rumoca_phase_typecheck"),
        ("Instantiate", "rumoca_phase_instantiate"),
        ("Resolve", "rumoca_phase_resolve"),
    ] {
        let (start, end) =
            bracket(phase).unwrap_or_else(|| panic!("no {phase} bracket in the log"));
        let inside = logs[start..=end]
            .iter()
            .filter(|e| matches!(e.level, LogLevel::Trace) && e.message.contains(target))
            .count();
        assert!(
            inside > 0,
            "no {target} trace fell inside the {phase} bracket (lines {start}..={end}). \
             The phase's own output is being reported somewhere else in the log, \
             which is what a drain placed outside the bracket does.",
        );
        // And it is *rendered* as contained, not merely ordered between the two
        // lines — the indentation is what makes containment visible.
        assert!(
            logs[start..=end]
                .iter()
                .any(|e| matches!(e.level, LogLevel::Trace) && e.depth > 0),
            "{phase}'s trace output is not nested; the log can order entries but \
             not show that they belong to the phase",
        );
    }

    // **Each of the four carries its own traces now** — the thing Doug asked for.
    //
    // Not all four are asserted: `rumoca_phase_dae` may legitimately emit nothing
    // on a two-equation model, and demanding a line it has no reason to write
    // would make this test fail on correct behaviour. The three that *were*
    // measured emitting are pinned; DAE construction is covered by the
    // no-duplicate check above and by the totals below.
    for phase in ["Instantiate", "Typecheck", "Flatten"] {
        let (s, e) = bracket(phase).unwrap_or_else(|| panic!("no {phase} bracket"));
        assert!(
            logs[s..=e]
                .iter()
                .any(|x| matches!(x.level, LogLevel::Trace)),
            "{phase} carries no trace output. Its events are emitted during the \
             Rumoca compile call and routed here by target \u{2014} an empty \
             bracket means the routing dropped them, which is the bug this \
             replaced ('I still do not see rumoca trace log lines for phases such \
             as Instantiate')",
        );
    }

    // **And the notice's number is the truth**, not a constant that drifted.
    //
    // This is also what catches the two ways routing can fail: a dropped vector
    // makes `filed` fall short of the quoted count, a vector replayed under two
    // phases makes it exceed. **Identifying duplicates by message text was tried
    // first and is unsound** — `rumoca_eval_flat` emits *"expression evaluated
    // successfully result=Some(1)"* six times for six real evaluations, and no
    // substring distinguishes them (`docs/identity-and-provenance.md`). Counting
    // against a number the code computed is the sound form.
    let filed: usize = ["Instantiate", "Typecheck", "Flatten", "DAE construction"]
        .iter()
        .map(|p| {
            let (s, e) = bracket(p).unwrap_or_else(|| panic!("no {p} bracket"));
            // **Warn and Error count too.** `take_traces` maps Rumoca's event
            // levels onto three HRW levels, and Flatten's constant-injection
            // warnings arrive as `Warn` — counting only `Trace` here read 36
            // against the notice's 38 and looked like two lines had been lost.
            logs[s..=e]
                .iter()
                .filter(|x| matches!(x.level, LogLevel::Trace | LogLevel::Warn | LogLevel::Error))
                .count()
        })
        .sum();
    let notice = logs
        .iter()
        .find(|e| {
            matches!(e.level, LogLevel::Info)
                && e.message
                    .contains("are filed under the phase each one names")
        })
        .expect(
            "nothing explains why those brackets hold what they hold, or why some \
             lines stayed in the compile bracket. That is exactly the question a \
             reader is left holding \u{2014} it was asked twice",
        );
    assert!(
        notice.message.contains(&format!("{filed} trace line(s)")),
        "the notice quotes a different count than the log actually contains \
         ({filed} filed under the four phases): {}",
        notice.message,
    );
}

/// **No pane shows content HRW invented — checked across every stage of several
/// specimens, including the ones that fail.**
///
/// This is the class-level version of the fix applied one tab at a time on
/// 2026-08-04. The BLT tabs of a structurally singular model rendered blocks HRW
/// had computed itself, and **nothing about the pane distinguished them from the
/// compiler's**: well-formed JSON, every path resolving, the fidelity suite
/// green. What was missing was any record of *where the content came from*.
///
/// Two invariants, and the second is the one with teeth:
///
/// 1. **No production stage is [`Provenance::Hrw`]**. `Stage::computed` is the
///    only way to become one and nothing calls it, so this is a claim of absence
///    pinned to something that can fail — the moment a pane starts deriving its
///    own content, this test says so and the same commit must deal with how the
///    UI declares it.
/// 2. **A stage with no content and a stage with content are told apart by the
///    type, not by inspection.** `Empty` exactly when `value` is `None`.
///
/// **Specimens deliberately include failures.** A healthy model reaches every
/// phase, so its stages are all populated and the interesting branch — a phase
/// that produced nothing — is never taken. `UnbalancedShaft` (balance −1) and
/// `CapacitorLoop` (structurally singular) are the ones that exercise it.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn no_stage_shows_content_hrw_invented() {
    let mut checked = 0usize;
    let mut empties = 0usize;
    for model in [
        "SingleInertia",
        "UnbalancedShaft",
        "CapacitorLoop",
        "Drivetrain",
    ] {
        let FromWorker::Compiled { stages, .. } = compile_specimen_shared(model) else {
            panic!("{model}: expected a compiled result");
        };
        // `get` rather than `as_stage_pairs`, which yields only the values —
        // provenance is a property of the *stage*, and reading it off the value
        // is the very inspection this replaces.
        for kind in StageKind::COMPILATION {
            let (name, stage) = (kind.name(), stages.get(*kind));
            checked += 1;
            assert_ne!(
                stage.provenance,
                Provenance::Hrw,
                "{model}/{name} shows content HRW produced rather than the \
                 compiler's. That may be legitimate \u{2014} but it must be \
                 declared where the user can see it, and this test is the prompt \
                 to do that rather than let the pane pass as the compiler's",
            );
            let empty = stage.provenance == Provenance::Empty;
            assert_eq!(
                empty,
                stage.value.is_none(),
                "{model}/{name}: provenance says {:?} while value.is_none() is {}. \
                 These must agree, or 'this pane is showing nothing' becomes a \
                 judgement call made by reading the pane",
                stage.provenance,
                stage.value.is_none(),
            );
            if empty {
                empties += 1;
            }
        }
    }
    // **Non-vacuity, both halves.** Without content-bearing stages the Hrw check
    // is trivial; without empty ones the consistency check never sees the branch
    // that the fabrication took.
    assert!(checked >= 30, "only {checked} stage(s) examined");
    assert!(
        empties > 0,
        "no stage across four specimens \u{2014} two of which fail \u{2014} came \
         back empty. Either the specimens all now succeed, or absence stopped \
         being represented as absence, which is the defect this guards",
    );
}

/// **An invented bracket name is rejected.** The must-fire half, and it runs fast
/// because it needs no compile — the predicate is pure.
///
/// `"DAE pipeline"` is the real string that was in HRW's log until 2026-08-04: a
/// bracket wrapping five genuine phases under a parent that does not exist,
/// written because it read tidily. Doug found it walking the DAE lab. **A test
/// that only checked real names against real logs would have passed the entire
/// time that string was shipping**, so the negative case is the test.
#[test]
fn the_bracket_check_rejects_an_invented_phase() {
    for invented in [
        "DAE pipeline",
        "DAE pipeline (12.0ms)",
        "Lowering pipeline",
        "Frontend",
        "Analysis",
    ] {
        assert!(
            !bracket_names_a_real_phase(invented),
            "{invented:?} is accepted as a log bracket. It names no StageKind and \
             is on no allow-list, so it is a phase that does not exist \u{2014} \
             which is exactly what \"DAE pipeline\" was",
        );
    }

    // Every real phase is accepted, with and without its closing timing.
    for k in StageKind::COMPILATION {
        assert_eq!(bracket_phase_name(k.log_name()), Some(k.log_name()));
        assert_eq!(
            bracket_phase_name(&format!("{} (0.1ms)", k.log_name())),
            Some(k.log_name()),
            "a closing bracket's timing suffix must not hide its name",
        );
    }
    // And the argued non-phase brackets, including the compile's long qualifier.
    assert_eq!(
        bracket_phase_name("Rumoca compile \u{2014} full pipeline; HRW takes the flat model"),
        Some("Rumoca compile"),
    );
    for b in NON_PHASE_BRACKETS {
        assert_eq!(bracket_phase_name(b), Some(*b));
    }
}

/// **Every bracket in a real compile names something that exists, and they pair.**
///
/// Two claims a log makes that nothing checked before 2026-08-04:
///
/// 1. **The name is real.** Covered as a class now: a bracket name must come from
///    a `StageKind::log_name()` or the argued `NON_PHASE_BRACKETS` list.
/// 2. **The nesting is real.** The pre-existing balance check counted starts
///    against ends, which a *mismatched* pair satisfies perfectly — open
///    `Flatten`, close `Typecheck`, and the count is still zero while the
///    indentation on screen tells the reader that one phase contains another.
///    This walks a stack of names instead.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn every_log_bracket_names_a_real_phase_and_pairs_with_its_own_end() {
    let logs = compile_specimen_logs_shared("SingleInertia");

    let brackets: Vec<_> = logs
        .iter()
        .filter(|e| matches!(e.level, LogLevel::StageStart | LogLevel::StageEnd))
        .collect();
    // Non-vacuity: a compile that logged nothing would pass every loop below.
    assert!(
        brackets.len() >= 20,
        "only {} bracket(s) in the log \u{2014} this cannot detect a bad one",
        brackets.len(),
    );

    let mut stack: Vec<&'static str> = Vec::new();
    for e in &brackets {
        let Some(name) = bracket_phase_name(&e.message) else {
            panic!(
                "log bracket {:?} names no phase. Every bracket is a claim that the \
                 named thing ran and that what is nested inside belongs to it \
                 \u{2014} a name that exists nowhere makes both claims unverifiable",
                e.message,
            );
        };
        match e.level {
            LogLevel::StageStart => stack.push(name),
            LogLevel::StageEnd => match stack.pop() {
                Some(open) => assert_eq!(
                    open, name,
                    "bracket {:?} closes while {open:?} is the innermost open one. \
                     The counts still balance, so the old check passed \u{2014} but \
                     the indentation now shows one phase containing another that it \
                     does not",
                    e.message,
                ),
                None => panic!("{:?} closes a bracket that was never opened", e.message),
            },
            _ => unreachable!("filtered above"),
        }
    }
    assert!(
        stack.is_empty(),
        "left open at the end of the compile: {stack:?}",
    );
}

/// **A healthy compile logs every phase, once each, in pipeline order.**
///
/// # The gap this closes, and why it is the one that matters for `worker.rs`
///
/// The fidelity programme (F1–F9) verifies **nouns**: is this structure what Rumoca
/// produced? `CLAUDE.md` states plainly that the **verbs** are outside it — *which
/// phase ran, in what order, nested inside what, what it declined to do* — and the
/// verbs are exactly what the compile path decides.
///
/// [`every_log_bracket_names_a_real_phase_and_pairs_with_its_own_end`] covers two of
/// them: every bracket **names** something real, and the nesting **pairs**. Neither
/// says anything about **order** or **completeness**. A change that ran Flatten
/// before Typecheck, or silently skipped Events, produces a log whose every bracket
/// is real and whose every pair matches — and passes.
///
/// That is the shape this repository keeps finding: a check whose claim is narrower
/// than the reader assumes. Here the assumption is dangerous, because a reader
/// learning the pipeline **reads the log as the pipeline**.
///
/// # Why the expectation is derived and not written down
///
/// It is `StageKind::COMPILATION` itself, so a phase added to the compiler is
/// required in the log the day it is added, with nobody remembering to update a
/// list. The same reasoning as the stage roster in `architecture.md`.
///
/// # Why a healthy specimen
///
/// A *failing* compile is supposed to stop early — later phases log nothing because
/// they did not run, which is the log telling the truth. `SingleInertia` reaches the
/// end, so for it "every phase" is the honest expectation. A specimen that stops is
/// covered by [`crate::fidelity::tests::a_rumoca_failure_is_represented_faithfully`].
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn a_healthy_compile_logs_every_phase_once_in_pipeline_order() {
    let logs = compile_specimen_logs_shared("SingleInertia");

    // Opening brackets only, and only those naming a phase — the outer
    // "Rumoca compile" wrapper and its siblings are real brackets that are not
    // phases, which is what `NON_PHASE_BRACKETS` exists to say.
    let observed: Vec<&'static str> = logs
        .iter()
        .filter(|e| matches!(e.level, LogLevel::StageStart))
        .filter_map(|e| bracket_phase_name(&e.message))
        .filter(|name| !NON_PHASE_BRACKETS.contains(name))
        .collect();

    let expected: Vec<&'static str> = StageKind::COMPILATION
        .iter()
        .map(|k| k.log_name())
        .collect();

    assert_eq!(
        observed, expected,
        "\nthe log's phase sequence is not the pipeline.\n  logged:   {observed:?}\n  \
         pipeline: {expected:?}\n\nEvery bracket may still name a real phase and \
         every pair may still match — those are checked next door and would not \
         notice this. A reader learns the pipeline by reading this log, so an order \
         it does not have, or a phase it skipped in silence, teaches the pipeline \
         wrong.",
    );
}

/// **Every bracket reports a time, and no bracket costs less than what it contains.**
///
/// The third verb, after *which phase ran* and *in what order*: **how long, and
/// charged to whom.** The log's timings are what the reader uses to say where a
/// compile spends itself, so they are claims like any other.
///
/// # The two invariants, and why only these two
///
/// 1. **Every close carries a parseable time.** A bracket that closes without one
///    drops silently out of the timeline — present in the log, absent from the
///    picture it is read for.
/// 2. **A bracket's direct children sum to no more than the bracket itself.** This
///    is the *attribution* invariant: a child charged time it spent outside its
///    parent, or counted twice, breaks it. `run_stage!` already takes care over
///    this — captured output is replayed **before** the clock starts, because
///    charging it to the extraction *"would be a second small fiction of the kind
///    this bracket exists to end"* — and until now that care rested on a comment.
///
/// **What is deliberately NOT asserted: that a phase takes nonzero time.** Measured
/// 2026-08-24 before writing this — `Initialization` and `Events` both report
/// **0.0ms** on `SingleInertia`, honestly, because they read fields the DAE already
/// carries. An invariant fitted to a guess would have failed on the truth.
///
/// **Nor is any absolute duration.** How long a phase *should* take is performance,
/// which this project does not optimise for; whether the log describes what
/// happened is accuracy, which it ranks first.
///
/// # The shape the measurement corrected
///
/// `"Rumoca compile"` is **not** a parent of every phase — it wraps Instantiate,
/// Typecheck, Flatten and DAE construction, while Parse, Resolve and everything
/// from Structural analysis on are its siblings. A "sum of phases ≤ the whole
/// compile" check would have been simply wrong, and only reading a real log said so.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn every_bracket_is_timed_and_none_costs_less_than_its_contents() {
    /// `"Flatten (0.4ms)"` and `"Rumoca compile (1253.0ms; +8.0ms reading …)"` —
    /// the first number before `ms`, which is the bracket's own span.
    fn millis(msg: &str) -> Option<f64> {
        let open = msg.rfind('(')?;
        let rest = &msg[open + 1..];
        let ms = rest.find("ms")?;
        rest[..ms].trim().parse::<f64>().ok()
    }

    let logs = compile_specimen_logs_shared("SingleInertia");

    // Each open frame accumulates the time of its direct children.
    let mut stack: Vec<(String, f64)> = Vec::new();
    let mut timed = 0usize;
    for e in &logs {
        match e.level {
            LogLevel::StageStart => stack.push((e.message.clone(), 0.0)),
            LogLevel::StageEnd => {
                let Some((name, children)) = stack.pop() else {
                    panic!("{:?} closes a bracket that was never opened", e.message);
                };
                let Some(own) = millis(&e.message) else {
                    panic!(
                        "bracket {:?} closes without a time. It is in the log and \
                         missing from the timeline the log is read for",
                        e.message,
                    );
                };
                timed += 1;
                assert!(
                    children <= own + 0.05,
                    "{name:?} reports {own}ms but the brackets inside it sum to \
                     {children}ms. A phase charged time it did not spend inside its \
                     parent, or counted twice \u{2014} either way the timeline \
                     attributes work to the wrong phase.",
                );
                if let Some((_, parent_children)) = stack.last_mut() {
                    *parent_children += own;
                }
            }
            _ => {}
        }
    }

    assert!(stack.is_empty(), "left open at the end: {stack:?}");
    assert!(
        timed >= 11,
        "only {timed} timed bracket(s) \u{2014} this cannot detect an untimed one",
    );
}

/// **The "no instrumentation" claim is still true.**
///
/// `UNINSTRUMENTED_PHASES` tells the reader that silence from these phases means
/// *unwired*, not *quiet*. That is a claim of **absence**, and this project's
/// standing rule is that a claim of absence rots unnoticed unless something
/// fails when it stops being true — acting on a wrong positive means going to
/// look and finding nothing, while acting on a wrong negative means **not
/// looking**.
///
/// So this counts tracing call sites in each named crate. Instrument one of them
/// upstream and this fails until the entry is removed, which is the only way the
/// notice can stay honest across a rebase.
///
/// **Both directions.** A listed crate must have none, *and* a crate known to be
/// instrumented must not be listed — otherwise an over-broad list would silence
/// real output in the reader's mind, which is the same defect pointed the other
/// way.
#[test]
fn the_uninstrumented_phase_list_matches_the_crates() {
    // `tracing::debug!` and friends, plus the bare form after a `use tracing::…`.
    fn tracing_sites(crate_dir: &Path) -> usize {
        fn walk(dir: &Path, out: &mut usize) {
            let Ok(entries) = std::fs::read_dir(dir) else {
                return;
            };
            for e in entries.flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, out);
                } else if p.extension().and_then(|x| x.to_str()) == Some("rs") {
                    let Ok(src) = std::fs::read_to_string(&p) else {
                        continue;
                    };
                    // Only `tracing`'s macros count: the parser's generated code
                    // uses `parol_runtime::log::trace`, which HRW never sees, and
                    // treating that as instrumentation would make the notice lie
                    // in the more damaging direction.
                    let uses_tracing = src.contains("use tracing") || src.contains("tracing::");
                    for m in [
                        "tracing::debug!",
                        "tracing::info!",
                        "tracing::warn!",
                        "tracing::trace!",
                        "tracing::error!",
                    ] {
                        *out += src.matches(m).count();
                    }
                    // **Crate-local trace macros count too.** Counting only
                    // `tracing::` undercounted `rumoca-phase-structural` by
                    // 27 — it wraps every call in `structural_trace!`, so the
                    // naive count read the *best* instrumented phase as the
                    // worst. A test that undercounts here would let a genuinely
                    // instrumented crate stay on the silent list.
                    *out += src.matches("_trace!(").count();
                    if uses_tracing && !src.contains("parol_runtime::log") {
                        for m in [
                            "\n    debug!(",
                            "\n    info!(",
                            "\n    warn!(",
                            "\n    trace!(",
                        ] {
                            *out += src.matches(m).count();
                        }
                    }
                }
            }
        }
        let mut n = 0;
        walk(&crate_dir.join("src"), &mut n);
        n
    }

    let crates = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("hrw/ has a parent")
        .join("crates");
    assert!(
        crates.is_dir(),
        "the Rumoca crates must be beside hrw/: {crates:?}"
    );

    for (phase, krate, why) in UNINSTRUMENTED_PHASES {
        match krate {
            // Backed by a real crate: it must genuinely emit nothing.
            Some(krate) => {
                let dir = crates.join(krate);
                assert!(
                    dir.is_dir(),
                    "{phase} names {krate}, which is not a crate. An absent \
                     directory greps as zero call sites, so a wrong crate name \
                     makes this whole notice a self-confirming fiction \u{2014} \
                     which is exactly how the first draft claimed two phases were \
                     uninstrumented when they were not phases at all.",
                );
                let n = tracing_sites(&dir);
                assert_eq!(
                    n, 0,
                    "{phase} is listed as silent ({why}), but {krate} now has {n} \
                     tracing call site(s). The log is telling readers to expect \
                     silence from a phase that speaks \u{2014} remove the entry.",
                );
            }
            // **Claimed to render DAE data without calling Rumoca.** Checked by
            // reading the stage function: if it names any `rumoca_phase_*`, a
            // real algorithm runs and the tab can emit traces after all.
            //
            // This check exists because the first version only asked whether a
            // crate named `rumoca-phase-<tab>` existed — which is *no evidence at
            // all*, and duly passed for Initialization, whose
            // `initialization_stage` calls
            // `rumoca_phase_structural::build_ic_plan` on its eleventh line.
            //
            // **Bounded, and honestly so**: it reads the stage function's own
            // body, so a Rumoca call hidden inside a helper would escape it. That
            // is a weaker guarantee than the crate-count branch above and is
            // stated rather than implied.
            None => {
                let worker = std::fs::read_to_string(
                    Path::new(env!("CARGO_MANIFEST_DIR")).join("src/worker.rs"),
                )
                .expect("worker.rs must be readable");
                let needle = format!("fn {}_stage(", phase.to_lowercase());
                let start = worker.find(&needle).unwrap_or_else(|| {
                    panic!("{phase} claims to be HRW-derived but has no {needle}")
                });
                let body_end = worker[start..]
                    .find("\nfn ")
                    .map(|e| start + e)
                    .unwrap_or(worker.len());
                let body = &worker[start..body_end];
                assert!(
                    !body.contains("rumoca_phase_"),
                    "{phase} is described as rendering DAE data ({why}), but \
                     {needle} calls into a rumoca_phase_* crate \u{2014} a real \
                     algorithm runs, so the tab can emit tracing and must not be \
                     listed as permanently silent",
                );
            }
        }
    }

    // The other direction: a crate that *is* instrumented must not be listed.
    for krate in ["rumoca-phase-flatten", "rumoca-phase-dae"] {
        assert!(
            tracing_sites(&crates.join(krate)) > 0,
            "{krate} was expected to be instrumented; if that changed, this test's \
             own premise is stale and the notice needs rechecking",
        );
        assert!(
            !UNINSTRUMENTED_PHASES
                .iter()
                .any(|(_, k, _)| *k == Some(krate)),
            "{krate} emits tracing but is listed as silent",
        );
    }

    // And the notice actually names them, rather than being an empty sentence.
    let notice = uninstrumented_notice();
    for (phase, _, _) in UNINSTRUMENTED_PHASES {
        assert!(
            notice.contains(phase),
            "the notice must name {phase}: {notice}"
        );
    }
}

/// **Every Rumoca phase crate is either shown or deliberately excluded.**
///
/// Doug, 2026-08-04: *"have we correctly accounted for all rumoca phases in the
/// logs and our stage tabs now?"* — a question I had answered wrongly three times
/// by reasoning from silence. This makes it a matter of record instead.
///
/// **The failure mode it guards is a phase HRW never mentions.** A tab that
/// should not exist is visible and gets questioned; a phase with no tab is
/// invisible, and a student mapping the chain has no way to learn it was there.
/// That is the wrong-negative shape again, and the reason `rumoca-phase-codegen`
/// is named below rather than merely absent.
///
/// Add a phase crate upstream and this fails until it is either given a stage or
/// listed here with a reason — which is the only way the answer stays true across
/// a rebase.
#[test]
fn every_rumoca_phase_crate_is_shown_or_explained() {
    /// Which HRW stage tab shows each Rumoca phase.
    ///
    /// Checked against `StageKind::ALL`, **not** against `worker.rs` text: HRW
    /// does not name every phase crate directly — resolution runs through the
    /// session inside `rumoca-compile` — so "does the source mention it" answers
    /// the wrong question and fails on `rumoca-phase-resolve`, which is plainly
    /// shown. **What matters is whether a reader can see the phase**, and a tab
    /// is what they see.
    const PHASE_TO_STAGE: &[(&str, &str)] = &[
        ("rumoca-phase-parse", "Parse"),
        ("rumoca-phase-resolve", "Resolve"),
        ("rumoca-phase-instantiate", "Instantiate"),
        ("rumoca-phase-typecheck", "Typecheck"),
        ("rumoca-phase-flatten", "Flatten"),
        ("rumoca-phase-dae", "DAE"),
        // One crate, three tabs: structural analysis, index reduction and the
        // IC plan (`build_ic_plan`) all live here. Naming one is enough to
        // establish the phase is reachable.
        ("rumoca-phase-structural", "Structural"),
        ("rumoca-phase-solve", "Solve lowering"),
    ];

    /// Phase crates HRW deliberately does not run, and why.
    const NOT_IN_HRWS_PATH: &[(&str, &str)] = &[(
        "rumoca-phase-codegen",
        "HRW simulates through `rumoca-sim` and the solver crates rather than \
         generating code; codegen serves the MLIR/native execution path \
         (`rumoca-exec-mlir`), which HRW does not take. It is not a gap in the \
         chain HRW shows \u{2014} it is a different branch of it.",
    )];

    let crates = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("hrw/ has a parent")
        .join("crates");

    let mut phase_crates: Vec<String> = std::fs::read_dir(&crates)
        .expect("crates/ must be readable")
        .flatten()
        .filter_map(|e| {
            let n = e.file_name().to_string_lossy().into_owned();
            (n.starts_with("rumoca-phase-") && e.path().is_dir()).then_some(n)
        })
        .collect();
    phase_crates.sort();
    assert!(
        phase_crates.len() >= 9,
        "expected the workspace's phase crates, found {phase_crates:?}",
    );

    let stage_names: Vec<&str> = StageKind::ALL.iter().map(|s| s.name()).collect();

    for krate in &phase_crates {
        let shown = PHASE_TO_STAGE.iter().find(|(k, _)| k == krate);
        let excused = NOT_IN_HRWS_PATH.iter().any(|(k, _)| k == krate);
        assert!(
            shown.is_some() || excused,
            "{krate} is a Rumoca phase that HRW neither shows nor explains. Give it \
             a stage tab, or add it to NOT_IN_HRWS_PATH with the reason \u{2014} an \
             unmentioned phase is invisible to someone learning the chain from HRW, \
             which is the one kind of gap nobody notices.",
        );
        if let Some((_, stage)) = shown {
            assert!(
                stage_names.contains(stage),
                "{krate} maps to a stage named {stage:?}, which is not in \
                 StageKind::ALL ({stage_names:?})",
            );
        }
        assert!(
            !(shown.is_some() && excused),
            "{krate} is both mapped to a stage and excused; the exclusion is stale",
        );
    }

    // Neither table may name a crate that no longer exists. **A missing
    // directory is exactly what made three earlier claims here self-confirming**
    // — it greps as zero, reads as "nothing there", and proves nothing.
    for (krate, _) in PHASE_TO_STAGE.iter().chain(NOT_IN_HRWS_PATH.iter()) {
        assert!(
            crates.join(krate).is_dir(),
            "{krate} is named in this test but is not a crate",
        );
    }
}

/// **The resolve-failure predicate, tested away from the compile path.**
///
/// Unit-tested on synthesized diagnostics deliberately: the end-to-end tests take
/// minutes and confound five things, and this is the one decision the A3 change
/// turns on. The numbers below are the measured ones —
/// `resolve_diagnostics_indicate_failure`'s doc comment carries the table.
#[test]
fn a_library_warning_is_not_a_resolve_failure() {
    use rumoca_core::{Diagnostic, Diagnostics};

    // No labels: the predicate reads severity only, and a span would be
    // scaffolding that implies this test cares where the diagnostic points.
    let warn = |msg: &str| Diagnostic {
        severity: rumoca_core::DiagnosticSeverity::Warning,
        code: None,
        message: msg.to_owned(),
        labels: Vec::new(),
        notes: Vec::new(),
    };

    // The good specimen's real shape: many diagnostics, none of them errors.
    let mut clean = Diagnostics::new();
    for i in 0..33 {
        clean.emit(warn(&format!("library note {i}")));
    }
    assert!(
        !resolve_diagnostics_indicate_failure(&clean),
        "33 non-error diagnostics is what a HEALTHY model looks like in this \
         workspace; treating any diagnostic as failure would fail every model",
    );

    // And one error is enough, among the same noise.
    let mut broken = clean.clone();
    broken.emit(Diagnostic {
        severity: rumoca_core::DiagnosticSeverity::Error,
        code: Some("E".into()),
        message: "undefined reference".into(),
        labels: Vec::new(),
        notes: Vec::new(),
    });
    assert!(
        resolve_diagnostics_indicate_failure(&broken),
        "a single error must surface even buried in library noise \u{2014} a Resolve \
         tab silent on a real failure is worse than the duplicate resolve this \
         change removes",
    );

    // Empty is not failure.
    assert!(!resolve_diagnostics_indicate_failure(&Diagnostics::new()));
}

/// **The matching animation's frames come from the compile.**
///
/// Doug, 2026-08-04: *"our ability to play animations is tremendously valuable
/// and I want to preserve that. But I want to capture the data for those
/// animations during the actual compilation rather than use replays."*
///
/// Until then the animation re-ran `maximum_matching` on the incidence matrix
/// when its tab was opened. Deterministic, so it agreed — but it agreed **by luck
/// of the algorithm**, and the search a reader watched described an execution
/// that produced nothing, while the blocks on screen came from one nobody saw.
///
/// **Checked against the report, not against a number.** The captured frames must
/// end at the matching the report published; a re-derivation that happened to
/// differ would show a search converging on the wrong answer, which is the one
/// failure this change exists to make impossible.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn the_matching_animation_is_fed_by_the_compile() {
    use rumoca_phase_structural::matching::MatchingStep;

    let FromWorker::Compiled {
        matching_frames,
        stages,
        ..
    } = compile_specimen_shared("ProportionalLoop")
    else {
        panic!("expected Compiled");
    };

    assert!(
        !matching_frames.is_empty(),
        "no frames captured \u{2014} the animation would fall back to re-deriving, \
         silently, and this change would have done nothing",
    );

    // ProportionalLoop is the corpus's smallest model whose matching has to back
    // up, so a capture of the real run must contain a displacement. A capture of
    // the wrong thing very likely would not.
    assert!(
        matching_frames
            .iter()
            .any(|f| matches!(f.step, MatchingStep::TryDisplace { .. })),
        "ProportionalLoop's search displaces an earlier assignment; frames \
         without one are not this model's search",
    );

    // **The frames must land on the matching the report published.** The last
    // frame's state is the algorithm's answer; the report's `matching` is what
    // every downstream stage used.
    let published = stages
        .structural
        .value
        .as_ref()
        .and_then(|v| v.get("matching"))
        .and_then(|m| m.as_array())
        .expect("the structural report carries its matching");
    let final_frame = matching_frames.last().expect("checked non-empty");
    let matched_in_frames = final_frame.match_eq.iter().filter(|m| m.is_some()).count();
    assert_eq!(
        matched_in_frames,
        published.len(),
        "the captured search ends on a different matching than the report \
         published \u{2014} the animation would replay a run that did not produce \
         what the rest of the pipeline used",
    );
}

/// **The Tarjan animation's frames come from the compile too.**
///
/// Same provenance argument as matching: `build_structural_report` runs the SCC
/// search inside `blt::build_blt_blocks` and returns only the blocks, so the
/// animation re-ran it on tab open — agreeing by determinism, replaying a search
/// that produced nothing.
///
/// **Checked against the report's block structure**, not a frame count.
/// `ProportionalLoop`'s three equations form one coupled block, so the captured
/// search must find a component of size 3 — a capture of a different run, or of a
/// different model, would not.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn the_tarjan_animation_is_fed_by_the_compile() {
    let FromWorker::Compiled {
        tarjan_frames,
        stages,
        ..
    } = compile_specimen_shared("ProportionalLoop")
    else {
        panic!("expected Compiled");
    };

    assert!(
        !tarjan_frames.is_empty(),
        "no SCC frames captured \u{2014} the animation would silently fall back to \
         re-deriving and this change would have done nothing",
    );

    // The report says how many equations are in coupled blocks; the search that
    // produced it must have visited at least that many nodes.
    let coupled = stages
        .structural
        .value
        .as_ref()
        .and_then(|v| v.get("coupled_block_count"))
        .and_then(serde_json::Value::as_u64)
        .expect("the structural report counts its coupled blocks");
    assert_eq!(
        coupled, 1,
        "precondition: ProportionalLoop is the corpus's one-coupled-block model",
    );
    assert!(
        tarjan_frames.len() >= 3,
        "a three-equation SCC cannot be found in {} frames",
        tarjan_frames.len(),
    );
}

/// **Tearing frames come from the compile, segmented per coupled block.**
///
/// The last recorded animation to stop re-deriving. `walk_blocks` rebuilt four
/// things to get here — incidence, matching, Tarjan, then the tearing itself —
/// so the loops a reader watched being torn were torn by a run that produced
/// nothing.
///
/// **`TwoLoops` is the specimen that makes the segmenting testable rather than
/// merely asserted**: two coupled blocks of size 2. A flat capture, or one that
/// mis-assigned segments to blocks, would show one loop's reasoning under the
/// other with nothing on screen to say so.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn tearing_frames_are_captured_per_coupled_block() {
    let FromWorker::Compiled {
        tearing_frames,
        stages,
        ..
    } = compile_specimen_shared("TwoLoops")
    else {
        panic!("expected Compiled");
    };

    let blocks = stages
        .structural
        .value
        .as_ref()
        .and_then(|v| v.get("blocks"))
        .and_then(serde_json::Value::as_array)
        .expect("the structural report lists its blocks");
    let coupled = blocks
        .iter()
        .filter(|b| b.get("kind").and_then(serde_json::Value::as_str) == Some("coupled"))
        .count();

    assert_eq!(coupled, 2, "precondition: TwoLoops has two coupled blocks");
    assert_eq!(
        tearing_frames.len(),
        coupled,
        "one segment per coupled block. A different count means the segments \
         cannot be matched to blocks, and pairing them anyway would animate one \
         loop's reasoning under another",
    );
    assert!(
        tearing_frames.iter().all(|seg| !seg.is_empty()),
        "every coupled block was torn, so no segment should be empty: {:?}",
        tearing_frames.iter().map(Vec::len).collect::<Vec<_>>(),
    );

    // Each segment must open with its own Start — that is what delimits them,
    // so a flat capture spliced into one list would fail here.
    use rumoca_phase_structural::tearing::TearingStep;
    for (i, seg) in tearing_frames.iter().enumerate() {
        assert!(
            matches!(seg[0].step, TearingStep::Start { .. }),
            "segment {i} does not begin at a Start; the segmenting is not \
             tracking loop boundaries",
        );
    }
}

/// **Two systems, two captures, and they do not fit each other.**
///
/// The matching, Tarjan and tearing views render under Structural *and* Index
/// Reduction, over two different DAEs. `Drivetrain` is the specimen where that
/// gap is unmistakable: **97 equations raw, 20 after reduction.**
///
/// Until 2026-08-04 the Index Reduction tab had no capture of its own, which is
/// why the captured constructors needed a re-deriving fallback at all — and, for
/// a few hours, why they were briefly handed the *raw* system's frames to draw
/// over the reduced matrix. Doug's question about those fallbacks is what
/// surfaced it.
///
/// **Asserts they are genuinely different**, not merely both present: two
/// captures of the same system would be the bug wearing a second field.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn the_reduced_system_has_its_own_captured_frames() {
    let FromWorker::Compiled {
        matching_frames,
        reduced_frames,
        ..
    } = compile_specimen_shared("Drivetrain")
    else {
        panic!("expected Compiled");
    };

    let raw_n = matching_frames
        .first()
        .map(|f| f.match_eq.len())
        .expect("the raw system was matched");
    let reduced_n = reduced_frames
        .matching
        .first()
        .map(|f| f.match_eq.len())
        .expect(
            "the reduced system was matched \u{2014} without this the Index \
                 Reduction tab has nothing to animate and falls back to re-deriving",
        );

    assert!(
        raw_n > reduced_n,
        "index reduction should shrink Drivetrain's system; raw {raw_n}, reduced \
         {reduced_n}. If they are equal, both captures describe the same run and \
         the second is not being taken where I think it is.",
    );

    // The size check inside the animation constructors keys on exactly this, so
    // state the invariant the UI depends on: neither frame set fits the other's
    // matrix, which is why picking by stage is not optional.
    assert_ne!(
        raw_n, reduced_n,
        "the two frame sets must not be interchangeable",
    );
}

/// **A singular system produces no BLT frames, and none are invented.**
///
/// Doug, 2026-08-04: *"it would be helpful if the parts of the UI which depend
/// upon the BLT blocks made clear that no BLT blocks are available because no
/// attempt was made by the compiler to create those BLT blocks."*
///
/// Measured before the fix: on `CapacitorLoop` the compiler matches 13 of 14,
/// declares the system singular and returns before `build_blt_blocks` — yet the
/// Tarjan tab built its own matching and BLT and drew a **non-empty** SCC
/// decomposition of blocks that were never created.
///
/// This pins the compiler's half of the contract: **matching runs, Tarjan and
/// tearing do not.** If a future change made `build_structural_report` continue
/// past a singular matching, the tabs' explanation would become wrong and this
/// says so.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn a_singular_system_captures_matching_but_no_blocks() {
    let FromWorker::Compiled {
        matching_frames,
        tarjan_frames,
        tearing_frames,
        stages,
        ..
    } = compile_specimen_shared("CapacitorLoop")
    else {
        panic!("expected Compiled");
    };

    // Precondition: this specimen really is the singular one.
    let err = stages
        .structural
        .value
        .as_ref()
        .and_then(|v| v.get("error"))
        .and_then(|e| e.get("message"))
        .and_then(serde_json::Value::as_str)
        .expect("CapacitorLoop's structural stage reports why it stopped");
    assert!(
        err.contains("singular"),
        "expected a singularity, got {err:?}"
    );

    assert!(
        !matching_frames.is_empty(),
        "matching runs before the singularity is discovered, so its search IS \
         capturable \u{2014} this is what the matching lab's third act shows",
    );
    assert!(
        tarjan_frames.is_empty(),
        "the compiler returned before build_blt_blocks, so there is no SCC search \
         to capture. {} frames means it now continues past a singular matching, \
         and the tabs' explanation is stale",
        tarjan_frames.len(),
    );
    assert!(
        tearing_frames.is_empty(),
        "nor any tearing: there are no blocks to tear",
    );

    // And the report carries no `blocks`, which is what `from_captured` keys on.
    assert!(
        stages
            .structural
            .value
            .as_ref()
            .and_then(|v| v.get("blocks"))
            .is_none(),
        "a singular report must not carry blocks",
    );
}

/// Simulation also emits log entries with timing.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn simulation_emits_log_entries() {
    let logs = std::sync::Mutex::new(Vec::new());
    {
        let mut w = shared_worker().lock().unwrap_or_else(|e| e.into_inner());
        let path = PathBuf::from(format!(
            "{}/specimens/SingleInertia.mo",
            env!("CARGO_MANIFEST_DIR")
        ));
        w.simulate(
            CompileTarget::File(&path),
            "SingleInertia",
            1.0,
            &|msg: FromWorker| {
                if let FromWorker::Log(entry) = msg {
                    logs.lock().unwrap().push(entry);
                }
            },
        )
        .expect("simulate");
    }
    let logs = logs.into_inner().unwrap();
    let stage_starts: Vec<&str> = logs
        .iter()
        .filter(|e| matches!(e.level, LogLevel::StageStart))
        .map(|e| e.message.as_str())
        .collect();
    assert!(
        stage_starts.contains(&"Compile (for simulation)"),
        "missing compile stage"
    );
    assert!(
        stage_starts.contains(&"Solve lowering"),
        "missing solve lowering stage"
    );
    assert!(
        stage_starts.contains(&"Integration"),
        "missing integration stage"
    );
}

// -----------------------------------------------------------------------
// Error-path tests (TD-14): verify that the worker reports errors
// correctly when given bad inputs, rather than panicking.
// -----------------------------------------------------------------------

/// Compiling a nonexistent file reports a parse-stage error (file read
/// failure) instead of panicking.
#[test]
fn compile_nonexistent_file_reports_error() {
    let mut w = WorkerState::new();
    let path = PathBuf::from("/tmp/hrw_test_nonexistent_file_that_does_not_exist.mo");
    let result = w.compile(&path, &|_: FromWorker| {});
    let FromWorker::Compiled { stages, .. } = result else {
        panic!("expected Compiled");
    };
    assert!(
        stages.parse.note_is_error(),
        "parse stage should flag an error for a missing file"
    );
    assert!(
        stages
            .parse
            .note
            .as_deref()
            .unwrap_or("")
            .contains("read error"),
        "parse note should mention a read error, got: {:?}",
        stages.parse.note
    );
}

/// Compiling a file with invalid Modelica syntax reports a parse-stage
/// error (the parser rejects the input).
#[test]
fn compile_invalid_syntax_reports_parse_error() {
    let tmp_dir = PathBuf::from(concat!(
        "/tmp/claude-1000/-home-dougdew-dev-rumoca/",
        "0033dab5-98a0-4f7a-8241-a545c97992aa/scratchpad"
    ));
    std::fs::create_dir_all(&tmp_dir).expect("create scratchpad dir");
    let bad_file = tmp_dir.join("bad_syntax.mo");
    std::fs::write(&bad_file, "not valid modelica {").expect("write temp file");

    let mut w = WorkerState::new();
    let result = w.compile(&bad_file, &|_: FromWorker| {});
    let FromWorker::Compiled { stages, .. } = result else {
        panic!("expected Compiled");
    };
    assert!(
        stages.parse.note_is_error(),
        "parse stage should flag an error for invalid syntax"
    );
    assert!(
        stages.parse.note.is_some(),
        "parse stage should carry an error message"
    );
}

/// Calling `open_def` on a fresh worker (no compilation, no resolved tree)
/// returns a `DefTree` with `result: Err(...)` instead of panicking.
#[test]
fn open_def_without_resolved_tree_reports_error() {
    let mut w = WorkerState::new();
    let result = w.open_def("SomeName");
    let FromWorker::DefTree { result, .. } = result else {
        panic!("expected DefTree");
    };
    assert!(
        result.is_err(),
        "open_def on a fresh worker should return Err"
    );
}

/// `extract_class` with a name that doesn't exist in the tree returns a
/// `Stage` with `note_is_error == true`.
#[test]
fn extract_class_missing_name_reports_error() {
    let empty_tree = rumoca_ir_ast::ClassTree::default();
    let stage = extract_class(&empty_tree, "NonExistent.Model.Name");
    assert!(
        stage.note_is_error(),
        "extract_class should flag an error for a missing name"
    );
    assert!(
        stage.value.is_none(),
        "extract_class should produce no value for a missing name"
    );
    assert!(
        stage.note.as_deref().unwrap_or("").contains("not found"),
        "error note should mention 'not found', got: {:?}",
        stage.note
    );
}

#[test]
fn stage_kind_all_is_exhaustive() {
    assert_eq!(
        StageKind::ALL.len(),
        12,
        "StageKind::ALL should list every variant (currently 12: 11 pipeline stages \
         plus Simulation). Adding one means wiring it into every per-stage system — \
         stage-diff highlight, stage-file publishing and the notebook trace — which is \
         why this count is asserted rather than derived from the enum."
    );
    // Every name is non-empty and unique.
    let names: Vec<&str> = StageKind::ALL.iter().map(|s| s.name()).collect();
    for name in &names {
        assert!(!name.is_empty());
    }
    let unique: std::collections::HashSet<&&str> = names.iter().collect();
    assert_eq!(unique.len(), names.len(), "duplicate stage names in ALL");
}

#[test]
fn simulate_nonexistent_file_reports_error() {
    let mut w = WorkerState::new();
    let path = PathBuf::from("/tmp/hrw_test_sim_nonexistent.mo");
    let result = w.simulate(
        CompileTarget::File(&path),
        "Model",
        1.0,
        &|_: FromWorker| {},
    );
    assert!(
        result.is_err(),
        "simulate of a missing file should return Err"
    );
}

#[test]
fn simulate_invalid_syntax_reports_error() {
    let tmp_dir = PathBuf::from(concat!(
        "/tmp/claude-1000/-home-dougdew-dev-rumoca/",
        "0033dab5-98a0-4f7a-8241-a545c97992aa/scratchpad"
    ));
    std::fs::create_dir_all(&tmp_dir).expect("create scratchpad dir");
    let bad_file = tmp_dir.join("sim_bad_syntax.mo");
    std::fs::write(&bad_file, "not valid modelica {").expect("write temp file");
    let mut w = WorkerState::new();
    let result = w.simulate(
        CompileTarget::File(&bad_file),
        "Model",
        1.0,
        &|_: FromWorker| {},
    );
    assert!(
        result.is_err(),
        "simulate of invalid syntax should return Err"
    );
}

#[test]
fn compile_emits_progress_messages() {
    let specimen = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/specimens/SingleInertia.mo"
    ));
    let mut w = WorkerState::new();
    let progress = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let p = std::sync::Arc::clone(&progress);
    let _final = w.compile(&specimen, &move |msg: FromWorker| {
        if let FromWorker::CompileProgress { .. } = &msg {
            p.lock().unwrap().push(msg);
        }
    });
    let msgs = progress.lock().unwrap();
    assert!(
        !msgs.is_empty(),
        "compile should emit at least one CompileProgress"
    );
}

#[test]
fn compile_produces_equation_sheet_for_healthy_specimen() {
    let specimen = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/specimens/SingleInertia.mo"
    ));
    let mut w = WorkerState::new();
    let result = w.compile(&specimen, &|_: FromWorker| {});
    let FromWorker::Compiled { equation_sheet, .. } = result else {
        panic!("expected Compiled");
    };
    let sheet = equation_sheet.expect("equation_sheet should be Some");
    assert!(sheet.n_equations > 0, "should have at least one equation");
}

#[test]
fn compile_produces_identifier_index_for_healthy_specimen() {
    let specimen = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/specimens/SingleInertia.mo"
    ));
    let mut w = WorkerState::new();
    let result = w.compile(&specimen, &|_: FromWorker| {});
    let FromWorker::Compiled {
        identifier_index, ..
    } = result
    else {
        panic!("expected Compiled");
    };
    let idx = identifier_index.expect("identifier_index should be Some");
    assert!(
        !idx.variables.is_empty(),
        "should have indexed at least one variable"
    );
    let has_state = idx.variables.values().any(|v| v.kind == "state");
    assert!(
        has_state,
        "SingleInertia should have at least one state variable"
    );
}

// -- OutputCapture tests --------------------------------------------------
//
// These tests use raw `libc::write` instead of `print!`/`eprint!` because
// cargo test intercepts Rust's print macros at the stdlib level — above
// the fd layer — via an internal `set_output_capture` mechanism. Data
// written through `print!` goes into cargo's per-test capture buffer and
// never reaches fd 1 (the pipe). Since `OutputCapture` operates at the fd
// level (`dup2`), its tests must also write at the fd level to exercise
// the actual capture path.
//
// In production this isn't an issue: Rumoca's C-level `printf` and Rust
// `tracing` output write directly to fd 1/2, bypassing Rust's BufWriter.

unsafe fn write_to_fd(fd: i32, data: &[u8]) {
    let mut offset = 0;
    while offset < data.len() {
        let remaining = data.len() - offset;
        // libc::write count is size_t on unix, c_uint on Windows.
        #[cfg(unix)]
        let count = remaining;
        #[cfg(windows)]
        let count = remaining as libc::c_uint;
        let n = unsafe { libc::write(fd, data[offset..].as_ptr().cast(), count) };
        if n <= 0 {
            break;
        }
        offset += n as usize;
    }
}

#[test]
fn output_capture_round_trip() {
    let mut cap = OutputCapture::start().expect("start capture");
    unsafe {
        write_to_fd(1, b"hello stdout");
        write_to_fd(2, b"hello stderr");
    }
    std::thread::sleep(std::time::Duration::from_millis(50));
    let (out, err) = cap.drain();
    drop(cap);
    assert!(out.contains("hello stdout"), "stdout missing: {out:?}");
    assert!(err.contains("hello stderr"), "stderr missing: {err:?}");
}

// Regression test for the pipe-buffer deadlock. Three implementations
// existed; this test distinguishes all three:
//
// 1. Post-hoc drain (original): drain() runs after the API call returns.
//    A 128 KB write exceeds the 64 KB pipe buffer, write() blocks waiting
//    for a reader, but drain() can't run until write() returns — deadlock.
//    This test would hang forever.
//
// 2. O_NONBLOCK on the write side (partial fix): write() returns EAGAIN
//    instead of blocking, preventing the deadlock — but the excess bytes
//    are silently dropped, and Rust's println! panics on EAGAIN. This test
//    would pass but assert_eq would fail (out.len() < 128 KB).
//
// 3. Concurrent reader threads (current fix): reader threads continuously
//    drain the pipe into a mutex buffer, so the pipe never fills. write()
//    stays blocking, all bytes are captured, no data loss.
//    This test passes with all 128 KB captured.
#[test]
fn output_capture_handles_large_write_without_deadlock() {
    let mut cap = OutputCapture::start().expect("start capture");
    let big = vec![b'x'; 128 * 1024];
    unsafe {
        write_to_fd(1, &big);
    }
    std::thread::sleep(std::time::Duration::from_millis(100));
    let (out, _) = cap.drain();
    drop(cap);
    assert_eq!(out.len(), 128 * 1024, "should capture all 128 KB");
}

/// `StageBundle::as_stage_pairs` names must stay in sync with
/// `bridge::STAGE_FILE_NAMES` — a rename or reorder in one but not the
/// other silently breaks stage-file publishing.
#[test]
fn stage_pairs_names_match_stage_file_names() {
    use crate::bridge::STAGE_FILE_NAMES;

    let bundle = StageBundle::default();
    let pair_names: Vec<String> = bundle
        .as_stage_pairs()
        .iter()
        .map(|(name, _)| format!("{name}.json"))
        .collect();
    let file_names: Vec<&str> = STAGE_FILE_NAMES.to_vec();

    assert_eq!(
        pair_names, file_names,
        "StageBundle::as_stage_pairs() names diverged from STAGE_FILE_NAMES"
    );
}

/// **Every committed manifest lists exactly the stages the pipeline has.**
///
/// # The rot this exists to catch
///
/// `gen_trace` carried its own `const STAGES: [&str; 11]`, so when `Dae` joined the
/// pipeline the notebook simply stopped mentioning it. **7 of 21 manifests had a
/// `dae` entry and 14 did not**, for seventeen days, and nothing in the toolchain
/// could say so — while `hrw/CLAUDE.md` instructs that *"any number about a specimen
/// is read from here"*. A reader of the committed notebook saw a compiler HRW no
/// longer had.
///
/// `architecture.md` had the same disease and was cured by generating it **and**
/// adding a currency test. The notebook got the first half seventeen days earlier
/// and never got the second; this is the second half.
///
/// # Why this one is fast and the content check is not
///
/// Reading 21 small JSON files costs milliseconds, so this runs between edits and
/// fails the moment a stage is added to the compiler and not to the notebook.
/// `the_committed_notebook_matches_what_the_pipeline_produces_now` checks the far
/// harder property — that the *contents* are current — and needs 21 compiles to do
/// it, so it is slow-gated. **This one catches the structural half of the rot for
/// free**, which is the half that actually happened.
#[test]
fn manifest_stage_rosters_match_the_pipeline() {
    let notebook =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/specimen-notebook");
    let expected: Vec<String> = StageKind::COMPILATION
        .iter()
        .map(|k| k.notebook_key())
        .collect();

    let mut checked = 0usize;
    // Every specimen the notebook covers, as its manifest names it. Collected
    // while walking, and checked against `specimens/` below.
    let mut covered: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

    for entry in std::fs::read_dir(&notebook)
        .expect("the notebook directory exists")
        .flatten()
    {
        let manifest = entry.path().join("trace/manifest.json");
        let Ok(text) = std::fs::read_to_string(&manifest) else {
            continue;
        };
        let value: serde_json::Value =
            serde_json::from_str(&text).expect("a manifest is valid JSON");
        covered.insert(
            value["specimen"]
                .as_str()
                .unwrap_or_else(|| panic!("{} does not name its specimen", manifest.display()))
                .to_owned(),
        );
        let listed: Vec<String> = value["stages"]
            .as_object()
            .unwrap_or_else(|| panic!("{} has no `stages` map", manifest.display()))
            .keys()
            .cloned()
            .collect();
        // Compared as sets: `serde_json`'s map preserves insertion order, but the
        // claim being made is about *coverage*, and ordering is `gen_trace`'s.
        let mut listed_sorted = listed.clone();
        let mut expected_sorted = expected.clone();
        listed_sorted.sort();
        expected_sorted.sort();
        assert_eq!(
            listed_sorted,
            expected_sorted,
            "{} lists a different set of stages than the pipeline has \u{2014} \
             regenerate with `cargo run -p hrw --example gen_trace -- --all`",
            manifest.display(),
        );
        checked += 1;
    }

    // **Every specimen must HAVE a notebook entry**, which the loop above cannot
    // say — it walks the notebook, so a specimen with no trace is not a failure
    // there but an absence, and the loop skips it in silence.
    //
    // That is not hypothetical. `IncompatibleConnect` and `UndefinedRef` carried
    // **only `purpose.md`** and no trace at all until 2026-08-15, and nothing
    // noticed for as long as they had existed. Doug found it by reading the git
    // view and asking why two specimens were untouched by a regeneration.
    //
    // Driven from `specimens/`, and compared on the manifest's own `specimen`
    // field rather than the directory name: the notebook directory is named by
    // **model**, which need not equal the file stem.
    let specimens_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("specimens");
    let mut missing: Vec<String> = Vec::new();
    let mut total = 0usize;
    for entry in std::fs::read_dir(&specimens_dir)
        .expect("the specimens directory exists")
        .flatten()
    {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("mo") {
            continue;
        }
        total += 1;
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .expect("a specimen file name");
        let key = format!("specimens/{name}");
        if !covered.contains(&key) {
            missing.push(key);
        }
    }
    assert!(
        missing.is_empty(),
        "{} of {total} specimens have no notebook trace, so nothing about them can \
         be read from the notebook \u{2014} generate with \
         `cargo run -p hrw --example gen_trace -- --all`: {missing:?}",
        missing.len(),
    );

    assert!(
        checked >= 20 && total >= 20,
        "only {checked} manifests and {total} specimens were seen; there are 21 of \
         each, so this is not exercising what it claims",
    );
}

/// **The committed notebook is what the pipeline produces today**, stage for stage,
/// specimen for specimen.
///
/// # The rot this exists to catch, and why the fast test is not enough
///
/// `parse.json` had not been regenerated since **2026-07-21** and the other
/// pre-Flatten stages since **2026-07-29** — found 2026-08-15, twenty-five days
/// later, by accident. `manifest_stage_rosters_match_the_pipeline` catches a stage
/// appearing or disappearing; it cannot see a stage whose *contents* have drifted,
/// which is what actually happened and what every count in the nine labs rests on.
///
/// # Why it has its own feature gate, which is the interesting part
///
/// The first version used `compile_specimen_shared`, so the MSL loaded once and the
/// whole check cost 49 s. **It passed alone and failed in the full suite** — nine
/// files across `GearWithBrake` and `MissingComponentClass`. That is not a flake: it
/// is `tech-debt.md`'s open item *"a stage's emitted JSON depends on what else the
/// session holds"*, arriving as a consequence nobody had drawn from it.
///
/// **The consequence is about the notebook, not the test.** A committed trace is one
/// sample of a function that takes the session as a hidden argument. `gen_trace` runs
/// **one process per specimen**, so the committed value is the virgin-session value —
/// and any check compiling in company is comparing against a different input.
///
/// So this calls [`compile_specimen`], which builds a fresh `WorkerState` per
/// specimen exactly as `gen_trace` does. That is faithful and order-independent, and
/// it reloads the MSL 21 times, which is why it is not on the `slow-tests` gate: it
/// would add minutes to a run Doug already reports as friction.
///
/// # What it deliberately does not check
///
/// **Simulation.** `simulation.json` and the manifest's `simulation` block need an
/// actual solver run per specimen, which is the genuinely expensive part and is
/// already exercised by `all_healthy_specimens_simulate`. So a drift confined to
/// simulation output would pass here. Stated rather than left implicit, per the
/// standing rule that a bounded check says what it dropped.
#[cfg_attr(
    not(feature = "notebook-check"),
    ignore = "reloads the MSL per specimen; run with --features notebook-check"
)]
#[test]
fn the_committed_notebook_matches_what_the_pipeline_produces_now() {
    let notebook =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/specimen-notebook");
    let mut stale: Vec<String> = Vec::new();
    let mut compared = 0usize;
    let mut notes_compared = 0usize;
    let mut specimens = 0usize;

    for entry in std::fs::read_dir(&notebook)
        .expect("the notebook directory exists")
        .flatten()
    {
        let trace_dir = entry.path().join("trace");
        let Ok(manifest_text) = std::fs::read_to_string(trace_dir.join("manifest.json")) else {
            continue;
        };
        let manifest: serde_json::Value =
            serde_json::from_str(&manifest_text).expect("a manifest is valid JSON");
        // The specimen name comes from the manifest, not the directory: the
        // directory is named by *model*, and only the manifest records which
        // source file produced it.
        let specimen = manifest["specimen"]
            .as_str()
            .expect("a manifest records its specimen")
            .trim_start_matches("specimens/")
            .trim_end_matches(".mo")
            .to_owned();

        // **A fresh `WorkerState`, exactly as `gen_trace` gets.** Not
        // `compile_specimen_shared`: see the doc comment — the shared session
        // carries whatever earlier tests compiled, and two specimens emit
        // different JSON because of it.
        //
        // **Built by `format!`, not `join`, and that is load-bearing.** The path
        // string is handed to `parse_to_ast` and stamped into every `Location`
        // (`worker.rs`, the Parse stage), so its *spelling* is part of the output.
        // `join` produces `hrw\specimens\X.mo` while `gen_trace` produces
        // `hrw/specimens/X.mo`, and that one separator made **109 of 109** files
        // compare unequal on the first run of this test — a difference with no
        // meaning that looked exactly like total drift.
        let path = std::path::PathBuf::from(format!(
            "{}/specimens/{specimen}.mo",
            env!("CARGO_MANIFEST_DIR")
        ));
        let Ok(FromWorker::Compiled { stages, .. }) =
            compile_specimen(&path, test_msl::msl_roots())
        else {
            stale.push(format!("{specimen}: no longer compiles at all"));
            continue;
        };
        specimens += 1;

        for kind in StageKind::COMPILATION {
            let key = kind.notebook_key();
            let path = trace_dir.join(format!("{key}.json"));
            let committed = std::fs::read_to_string(&path)
                .ok()
                .map(|t| serde_json::from_str::<serde_json::Value>(&t).expect("valid JSON"));
            let produced = stages.get(*kind).value.clone();

            match (&committed, &produced) {
                (None, None) => {}
                (Some(_), None) => stale.push(format!(
                    "{specimen}/{key}.json is committed but the pipeline no longer \
                     produces that stage"
                )),
                (None, Some(_)) => stale.push(format!(
                    "{specimen}/{key}.json is missing but the pipeline produces it now"
                )),
                (Some(c), Some(p)) => {
                    compared += 1;
                    if c != p {
                        stale.push(format!(
                            "{specimen}/{key}.json differs from what the pipeline \
                             produces now"
                        ));
                    }
                }
            }

            // **The manifest's own record is compared too — added 2026-08-25.**
            //
            // Until then this loop read `<key>.json` and nothing else, so a stage's
            // **note** could drift in the committed manifest indefinitely. Proven, not
            // supposed: `UnclosedModel`'s resolve note stayed at its pre-C20 wording
            // through a green 109.9 s run of this very test, and surfaced only when an
            // unrelated fix forced a regeneration.
            //
            // **A note is a claim about the model** — *"not reached (ToDae failed
            // earlier)"* says which phase stopped the compile — so an unchecked note is
            // an unchecked claim, which is exactly what the file comparison above
            // exists to prevent for IR.
            let recorded = &manifest["stages"][key.as_str()];
            if recorded.is_null() {
                stale.push(format!(
                    "{specimen}: the manifest has no entry for stage `{key}`"
                ));
                continue;
            }
            notes_compared += 1;
            let recorded_note = recorded["note"].as_str();
            let produced_note = stages.get(*kind).note.as_deref();
            if recorded_note != produced_note {
                stale.push(format!(
                    "{specimen}/{key}: the manifest's note is {recorded_note:?} but the \
                     pipeline now says {produced_note:?}"
                ));
            }
            // `has_ir` is the manifest's own summary of what the file check above
            // measured. They can only disagree if the two halves were written at
            // different times, which is a staleness this test is for.
            if recorded["has_ir"].as_bool() != Some(produced.is_some()) {
                stale.push(format!(
                    "{specimen}/{key}: the manifest says has_ir={} but the pipeline \
                     {} IR",
                    recorded["has_ir"],
                    if produced.is_some() {
                        "produces"
                    } else {
                        "does not produce"
                    },
                ));
            }
        }
    }

    assert!(
        specimens >= 20 && compared >= 100,
        "only {specimens} specimens and {compared} stage files were compared; the \
         notebook has 21 specimens, so this is not exercising what it claims",
    );
    // Non-vacuity for the note comparison, and separate from the one above: a loop
    // that silently stopped reading manifests would leave `compared` healthy and
    // still check no notes at all.
    assert!(
        notes_compared >= 150,
        "only {notes_compared} stage notes were compared; 21 specimens at 11 stages \
         is 231, so this is not reading the manifests",
    );
    assert!(
        stale.is_empty(),
        "{} committed trace files no longer match the pipeline \u{2014} regenerate \
         with `cargo run -p hrw --example gen_trace -- --all`:\n  {}",
        stale.len(),
        stale.join("\n  "),
    );
}

/// `notebook_key` must be the snake_case of the canonical slug, for every stage.
///
/// Pinned separately from the roster test because the transform is where a new
/// stage would break: `SolveLowering` -> `solve_lowering` is the shape that matters,
/// and a single-word stage must not gain a leading underscore.
#[test]
fn notebook_keys_are_the_snake_case_of_the_slug() {
    assert_eq!(StageKind::Parse.notebook_key(), "parse");
    assert_eq!(StageKind::Dae.notebook_key(), "dae");
    assert_eq!(StageKind::IndexReduction.notebook_key(), "index_reduction");
    assert_eq!(StageKind::SolveLowering.notebook_key(), "solve_lowering");
    for kind in StageKind::ALL {
        let key = kind.notebook_key();
        assert!(
            !key.starts_with('_') && !key.is_empty(),
            "{kind:?} produced a malformed notebook key {key:?}",
        );
    }
}

/// Does this request oblige the worker to answer?
///
/// **Exhaustive on purpose.** A new `ToWorker` variant must be classified here
/// before the crate compiles, which is stronger than a roster: a roster can only
/// fail at run time, and only if something exercises the new variant.
fn expects_a_response(msg: &ToWorker) -> bool {
    match msg {
        ToWorker::SetLibraries(_)
        | ToWorker::Compile(_)
        | ToWorker::CompileLibraryModel(_)
        | ToWorker::OpenDef(_)
        | ToWorker::Simulate { .. } => true,
        ToWorker::SetTracing(_) | ToWorker::LiveDebugConnections { .. } => false,
    }
}

/// Is this the *answer* to a request, rather than something streamed during one?
///
/// Exhaustive for the same reason. `Log` and `CompileProgress` arrive on the same
/// channel while a request is still running, so a test that simply took the first
/// message would pass on a compile that never finished.
fn is_terminal(msg: &FromWorker) -> bool {
    match msg {
        FromWorker::Libraries(_)
        | FromWorker::Compiled { .. }
        | FromWorker::DefTree { .. }
        | FromWorker::Simulated { .. } => true,
        FromWorker::Log(_) | FromWorker::CompileProgress { .. } => false,
    }
}

/// **Every request the worker answers produces exactly one response, of the right
/// kind** — the transport contract, which had no test at all until 2026-08-25.
///
/// # Why this layer needed its own test
///
/// Every other test in this file constructs a [`WorkerState`] and calls it
/// directly. [`Worker::spawn`] — the thread, its event loop, the `emit` closure
/// and the `Option<FromWorker>` contract [`WorkerState::handle`] returns — is
/// reached from exactly **one** place in the codebase, `App::new`, and from no
/// test. It is the layer every message crosses and it was the only one with zero
/// coverage.
///
/// **The failure it prevents is silent.** A request-shaped variant that returns
/// `None` leaves the UI waiting forever, and a UI waiting forever is
/// indistinguishable from a slow compile — the same shape `CLAUDE.md` records for
/// the permission allowlist and for a sleeping machine. Nothing else in the suite
/// would notice, because nothing else sends a message.
///
/// # Why the requests are all failures
///
/// Each one names something that does not exist, so it fails fast and the whole
/// test costs no compile. **What is under test is that an answer ARRIVES**, not
/// what it says; the hundred tests above cover what it says.
///
/// # What this does NOT cover
///
/// `LiveDebugConnections` is *classified* but not *driven* — it needs a live
/// `LiveTrace` with a debugger stepping it, which is a harness this test has no
/// business building. Its contract, no response but `done` signalled, is stated
/// on the variant itself. Nor does this check ordering under load: the loop is
/// serial by construction, so there is no interleaving to catch today.
#[test]
fn every_request_the_worker_answers_produces_exactly_one_response() {
    use std::time::Duration;

    let missing = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/specimens/NoSuchSpecimen.mo"
    ));
    let requests: Vec<(&str, ToWorker)> = vec![
        ("SetLibraries", ToWorker::SetLibraries(Vec::new())),
        ("Compile", ToWorker::Compile(missing.clone())),
        (
            "CompileLibraryModel",
            ToWorker::CompileLibraryModel("No.Such.Model".to_owned()),
        ),
        ("OpenDef", ToWorker::OpenDef("No.Such.Def".to_owned())),
        (
            "Simulate",
            ToWorker::Simulate {
                path: missing.clone(),
                model: "NoSuchModel".to_owned(),
                t_end: 0.1,
                is_library: false,
            },
        ),
        ("SetTracing", ToWorker::SetTracing(false)),
    ];

    // Non-vacuity: every answering variant is actually driven below. If a new one
    // is added, `expects_a_response` forces the classification and this forces
    // the exercise.
    assert_eq!(
        requests
            .iter()
            .filter(|(_, m)| expects_a_response(m))
            .count(),
        5,
        "a ToWorker variant that owes a response is not being sent by this test",
    );

    let worker = Worker::spawn(egui::Context::default());
    let next_terminal = || -> Option<FromWorker> {
        loop {
            // 30s is ~100x what any request here needs (all of them name
            // something that does not exist and fail immediately). It is a
            // ceiling on how long a FAILING run costs, not a real deadline:
            // detecting a hang means waiting one out, and the must-fire check
            // for this test paid the full timeout to prove it fires.
            match worker.rx.recv_timeout(Duration::from_secs(30)) {
                Ok(m) if is_terminal(&m) => return Some(m),
                Ok(_) => continue,
                Err(_) => return None,
            }
        }
    };

    for (name, msg) in requests {
        let owes_an_answer = expects_a_response(&msg);
        worker
            .tx
            .send(msg)
            .unwrap_or_else(|_| panic!("{name}: the worker thread died before it was asked"));

        if owes_an_answer {
            let got = next_terminal().unwrap_or_else(|| {
                panic!(
                    "{name} produced no response. The UI waits on this channel, so a \
                     request that is never answered presents as a hang, not as an error"
                )
            });
            let right_kind = matches!(
                (name, &got),
                ("SetLibraries", FromWorker::Libraries(_))
                    | (
                        "Compile" | "CompileLibraryModel",
                        FromWorker::Compiled { .. }
                    )
                    | ("OpenDef", FromWorker::DefTree { .. })
                    | ("Simulate", FromWorker::Simulated { .. })
            );
            assert!(
                right_kind,
                "{name} was answered with the wrong kind of response, so the UI would \
                 file the result under the wrong request",
            );
        } else {
            // Nothing should answer this one. A fence turns that absence into a
            // positive observation: send a request that MUST be answered, and the
            // next terminal has to be the fence's own. A stray answer then shows
            // up as the wrong kind rather than as silence nobody can measure.
            worker
                .tx
                .send(ToWorker::OpenDef("No.Such.Fence".to_owned()))
                .expect("the worker thread is alive");
            let fenced =
                next_terminal().unwrap_or_else(|| panic!("{name}: even the fence went unanswered"));
            assert!(
                matches!(fenced, FromWorker::DefTree { .. }),
                "{name} is classified as fire-and-forget, but the worker answered it",
            );
        }
    }
}

/// **A dead worker thread is detected only on the NEXT send.**
///
/// # What this measured, and what the measurement bought
///
/// Written when there was no `catch_unwind` anywhere in this file: a panic in any
/// Rumoca phase unwound the worker thread, the request that caused it was never
/// answered, and the UI learned only when it next sent something. Doug ruled on
/// that measurement the same day, and the loop in [`Worker::spawn`] now catches
/// the panic, answers the request and rebuilds the session — so the interval this
/// test was written to expose is closed.
///
/// # The prediction attached to it was WRONG, and that is the part worth keeping
///
/// This comment used to end *"if `catch_unwind` is ever added, **this test should
/// fail**"*. It did not fail. The test drives a **synthetic** panicking thread
/// rather than the real loop, so it never touched the code that changed — and a
/// test that cannot observe the thing it claims to guard says nothing when that
/// thing moves. **The tripwire was written into prose instead of into the test**,
/// which is the same shape as a claim of absence whose target never resolves.
///
/// # What it still holds, which is why it is kept rather than deleted
///
/// `send_failed` remains the only way the UI ever learns the thread is gone, and
/// the thread can still die: a panic in the `emit` closure, a panic while
/// rebuilding the state, an abort, a dropped channel. `guard` narrowed the causes;
/// it did not remove them.
#[test]
fn a_dead_worker_thread_is_detected_on_the_next_send() {
    // Non-vacuity first: while the far end is alive, `send` must NOT report a
    // failure. Without this, a `send` that always set the flag would pass.
    let (tx_live, rx_live) = mpsc::channel::<ToWorker>();
    let (_tx_res_live, rx_res_live) = mpsc::channel::<FromWorker>();
    let mut alive = Worker {
        tx: tx_live,
        rx: rx_res_live,
        send_failed: false,
    };
    alive.send(ToWorker::SetTracing(false));
    assert!(
        !alive.send_failed,
        "a live worker must not be reported as dead",
    );
    assert!(rx_live.try_recv().is_ok(), "the message must have arrived");

    // Now the real chain: a thread holding the worker's receiver panics.
    let (tx, rx_req) = mpsc::channel::<ToWorker>();
    let (tx_res, rx) = mpsc::channel::<FromWorker>();

    // Silence the panic report: this panic is the fixture, not a failure, and an
    // unexplained backtrace in the test log is how a green run gets read as a bad
    // one. Safe under `--test-threads=1`, which this suite requires anyway.
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let panicked = thread::spawn(move || {
        // Owned by the thread exactly as the real loop owns them, so unwinding
        // drops both ends the same way.
        let _rx_req = rx_req;
        let _tx_res = tx_res;
        panic!("a Rumoca phase panicked");
    })
    .join();
    std::panic::set_hook(previous);
    assert!(panicked.is_err(), "the fixture thread must actually panic");

    let mut worker = Worker {
        tx,
        rx,
        send_failed: false,
    };
    assert!(
        !worker.send_failed,
        "nothing has been sent yet, so nothing can have failed \u{2014} the flag \
         reports a failed SEND, not a dead thread",
    );

    worker.send(ToWorker::Compile(PathBuf::from("/no/such/specimen.mo")));
    assert!(
        worker.send_failed,
        "a send to a dead worker must set send_failed; it is the only way the UI \
         ever learns the thread is gone",
    );
}

/// One character describing what a stage did, for the corpus matrix.
///
/// **Five states, and the two splits are the point.** [`Outcome`] has three
/// variants, and each of two of them covers facts a reader must not conflate.
///
/// `Ok` covers both *"produced its IR"* and *"never ran, here is a note saying
/// so"* — [`Stage::info`] reaches `Ok` deliberately, since a skipped stage is not
/// a failure. `Failed` covers both *"this stage failed"* (an error payload) and
/// *"the reachable-closure pipeline produced no result at all"* (no payload) —
/// the same distinction `not_reached_note` and `no_result_note` were
/// single-sourced for on 2026-08-25.
///
/// **Collapsing either split would hide the drift this matrix exists to catch.**
/// A change that stops running a phase would look identical to one that runs it
/// successfully; a change that swallowed an error payload would look identical to
/// one that reported it. Both were collapsed in this function's first draft, and
/// generating the matrix is what exposed it: `MissingComponentClass` read
/// `OF..X.XXXXX`, with a stage *failing* after one that was never reached, which
/// is not a thing the pipeline can do.
fn outcome_code(stage: &Stage) -> char {
    match (stage.outcome, stage.value.is_some()) {
        (Outcome::Failed, _) if stage.error_json().is_some() => 'X',
        (Outcome::Failed, _) => '!',
        (Outcome::Flagged, _) => 'F',
        (Outcome::Ok, true) => 'O',
        (Outcome::Ok, false) => '.',
    }
}

/// **What every specimen does at every stage, pinned as one line each.**
///
/// # The question this answers that no other test does
///
/// The tests above assert particular facts about particular specimens —
/// `Drivetrain` reduces, `OverInitRc` is flagged, `CapacitorLoop` is singular.
/// Each is a point sample. **Nothing asserted the shape of the whole corpus**, so
/// a change in `worker.rs` that quietly turned a `Flagged` into an `Ok`, or
/// stopped a phase being reached, would be caught only where a test happened to
/// look. Twenty-four specimens across eleven stages is 264 cells; the point
/// samples cover a few dozen.
///
/// This is the third of three checks added on 2026-08-25 after Doug asked which
/// categories of `worker.rs` failure went untested. The first two cover the
/// transport layer; this one covers **outcome drift across the corpus**.
///
/// # How to read a row, and what a diff means
///
/// `O` produced IR · `F` produced IR **and** Rumoca reported something ·
/// `X` failed **with** an error payload · `!` failed with none ·
/// `.` no IR and not a failure — usually *not reached*, but also Flatten's
/// "completed, no flat model retained" when DAE construction failed.
/// Stage order is [`StageKind::COMPILATION`]: parse, resolve, instantiate,
/// typecheck, flatten, dae, structural, index-reduction, initialization, events,
/// solve-lowering.
///
/// **Every row now carries at most one `X` or `!`, at the stage that actually
/// stopped the pipeline.** That is the invariant the C20 fix established, and a
/// row with two is the defect it removed — see `no_result_note`.
///
/// **A failure here is "go and look", not "the table is stale".** Every cell is a
/// claim about what the compiler did with a real model. If a row changes, either
/// Rumoca's behaviour changed — which is worth knowing and is what the notebook
/// check would also catch — or HRW's reporting of it did, which nothing else
/// would catch. Update the row **in the same commit as the reasoning**, exactly
/// as the doc-block ratchet requires.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compiles the whole specimen corpus; run with --features slow-tests"
)]
fn the_corpus_outcome_matrix_is_unchanged() {
    // Filled from this test's own failure output on 2026-08-25; see the doc
    // comment for what a character means. Stage order:
    //   par res ins typ fla dae str idx ini evt sol
    const MATRIX: &[(&str, &str)] = &[
        // Healthy: every phase produces IR.
        ("BouncingBall", "OOOOOOOOOOO"),
        ("LoopWithInertia", "OOOOOOOOOOO"),
        ("MixedLoop", "OOOOOOOOOOO"),
        ("NonlinearLoop", "OOOOOOOOOOO"),
        ("ProportionalLoop", "OOOOOOOOOOO"),
        ("RcCircuit", "OOOOOOOOOOO"),
        // `connect`s at two hierarchy levels, and clean all the way through —
        // deliberately, since the point it makes is about how sets are BUILT and a
        // failure anywhere would give the reader something else to look at.
        ("ScopedConnect", "OOOOOOOOOOO"),
        ("SingleInertia", "OOOOOOOOOOO"),
        ("TwoLoops", "OOOOOOOOOOO"),
        // Initialization relaxed something and said so.
        ("OverInitRc", "OOOOOOOOFOO"),
        ("RotationalInertia", "OOOOOOOOFOO"),
        // High-index: structural flags a singular system, reduction fixes it,
        // initialization then reports its relaxation. Four models, one shape.
        ("BenchActuator", "OOOOOOFOFOO"),
        ("Drivetrain", "OOOOOOFOFOO"),
        ("GearWithBrake", "OOOOOOFOFOO"),
        ("MotorWithBrake", "OOOOOOFOFOO"),
        // Singular and NOT repaired by reduction — index reduction flags too.
        ("CapacitorLoop", "OOOOOOFFOOO"),
        ("IncompatibleConnect", "OOOOOOFFOOO"),
        ("TwiceDefined", "OOOOOOFFOOO"),
        // The canonical index-3 DAE Rumoca does not reduce (`ideas.md` #83):
        // initialization is flagged as well, which the three above are not.
        ("CartesianPendulum", "OOOOOOFFFOO"),
        // A flagged typecheck stops the pipeline; everything after is `.`.
        ("DimensionMismatch", "OOOF......."),
        // Flatten and DAE fail with a payload, then nothing is reached.
        ("OverDeterminedShaft", "OOOO.X....."),
        ("UnbalancedShaft", "OOOO.X....."),
        // No pipeline result at all. **These three rows interleave `.` and `!`
        // for one underlying condition** — recorded as `ui-findings.md` C20 and
        // deliberately not changed here, because which of the two a tab shows is
        // a pane claim and Doug's to rule on.
        ("MissingComponentClass", "OF........."),
        ("UndefinedRef", "OF........."),
        ("UnclosedModel", "X.........."),
    ];

    let dir = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/specimens"));
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .expect("the specimen directory must be readable")
        .flatten()
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("mo"))
        .filter_map(|e| {
            e.path()
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
        })
        .collect();
    names.sort();

    // Non-vacuity: a walk that found nothing must not pass as "no drift".
    assert!(
        names.len() >= 20,
        "only {} specimens found \u{2014} the walk is broken, not the corpus",
        names.len(),
    );

    let mut actual: Vec<(String, String)> = Vec::new();
    for name in &names {
        let FromWorker::Compiled { stages, .. } = compile_specimen_shared(name) else {
            panic!("{name}: expected Compiled");
        };
        let row: String = StageKind::COMPILATION
            .iter()
            .map(|k| outcome_code(stages.get(*k)))
            .collect();
        actual.push((name.clone(), row));
    }

    let expected: std::collections::BTreeMap<&str, &str> = MATRIX.iter().copied().collect();
    let mut drift: Vec<String> = Vec::new();
    for (name, row) in &actual {
        match expected.get(name.as_str()) {
            Some(want) if want == row => {}
            Some(want) => drift.push(format!("{name:<22} was {want}  now {row}")),
            None => drift.push(format!("{name:<22} (no baseline)  now {row}")),
        }
    }
    for name in expected.keys() {
        if !actual.iter().any(|(n, _)| n == name) {
            drift.push(format!("{name:<22} has a baseline but no specimen"));
        }
    }

    assert!(
        drift.is_empty(),
        "the corpus outcome matrix moved. Stage order is COMPILATION; \
         O=IR, F=IR+report, X=failed, .=not reached.\n  {}",
        drift.join("\n  "),
    );
}

/// **A panic becomes an answer, and the answer blames no phase.**
///
/// The three properties the catch-report-rebuild design rests on, each of which
/// would be silent if it broke:
///
/// 1. [`guard`] recovers the message from both payload shapes `panic!` produces.
/// 2. [`panicked_compile`] states absence on **every** stage — a blank tab would
///    leave the reader with no account at all.
/// 3. **No stage is `Failed`.** HRW does not know which phase panicked, so
///    claiming one stopped the pipeline is the invented control-flow claim C20
///    removed from four sites. Putting it on Parse would be the worst choice
///    available: Parse is the one stage that demonstrably did run.
#[test]
fn a_panic_is_answered_without_blaming_a_phase() {
    assert_eq!(
        guard(|| 7).ok(),
        Some(7),
        "the non-panicking path must pass through"
    );
    assert_eq!(
        guard(|| panic!("literal payload")).unwrap_err(),
        "literal payload",
        "`panic!(\"...\")` boxes a &str",
    );
    let n = 3;
    assert_eq!(
        guard(|| panic!("formatted {n}")).unwrap_err(),
        "formatted 3",
        "`panic!(\"{{}}\")` boxes a String \u{2014} a different downcast",
    );

    let note = "the compiler panicked: index out of bounds";
    let FromWorker::Compiled { stages, model, .. } =
        panicked_compile(PathBuf::from("/x/Model.mo"), note)
    else {
        panic!("a panicked compile still answers with Compiled");
    };
    assert!(model.is_none(), "no model was identified");
    for kind in StageKind::COMPILATION {
        let stage = stages.get(*kind);
        assert_eq!(
            stage.note.as_deref(),
            Some(note),
            "{kind:?} must state its own absence; a blank tab explains nothing",
        );
        assert_ne!(
            stage.outcome,
            Outcome::Failed,
            "{kind:?} claims the pipeline stopped there, and HRW does not know that",
        );
        assert!(stage.value.is_none(), "{kind:?} must show no IR");
    }
}

/// **A fire-and-forget request that panics still releases whatever waits on it.**
///
/// `LiveDebugConnections` owes no response, but it does own a `done` flag that
/// `handle` sets after the call — so a panic skips it, and the live `Playback`
/// waits for a session that already ended. That is the same hang this whole
/// change removes, reappearing in the one arm that answers nothing.
#[test]
fn a_panic_in_a_fire_and_forget_request_still_signals_its_done_flag() {
    use std::sync::atomic::{AtomicBool, Ordering};

    let done = Arc::new(AtomicBool::new(false));
    let reply = PanicReply::Silent(Some(Arc::clone(&done)));
    assert!(
        panic_response(reply, "boom").is_none(),
        "a fire-and-forget request must not grow a response",
    );
    assert!(
        done.load(Ordering::SeqCst),
        "the waiter was never released, so its controls spin forever",
    );

    // And the arm with nothing to release must not invent something to do.
    assert!(panic_response(PanicReply::Silent(None), "boom").is_none());
}

/// Does this note say the stage did not run?
///
/// **Two wordings, both meaning "did not run", and they are not synonyms.**
/// `not reached (…)` names a stage that was skipped because something earlier
/// stopped; the no-result note names a pipeline that produced nothing to skip *from*.
/// A check about *running* has to accept both, and one about *cause* must not conflate
/// them — which is why the two properties below are separate.
fn did_not_run(note: Option<&str>) -> bool {
    note.is_some_and(|n| n.starts_with("not reached (") || n == no_result_note())
}

/// The phase blamed by a `not reached (X failed earlier)` note, if it is that shape.
fn blamed_phase(note: Option<&str>) -> Option<String> {
    let inner = note?.strip_prefix("not reached (")?.strip_suffix(')')?;
    inner.strip_suffix(" failed earlier").map(str::to_owned)
}

/// **The not-reached tail is contiguous, and its notes agree on one cause.**
///
/// # Why these two, and why they are not the matrix
///
/// `the_corpus_outcome_matrix_is_unchanged` pins the outcome *classes*, so it catches
/// a row that CHANGES. It cannot catch a row that was wrong all along, and it says
/// nothing about the notes — which is where the reason a stage gives for its own
/// emptiness actually lives. These check the notes, over the same compiles, at no
/// extra cost.
///
/// **Contiguity:** once a stage says it did not run, every later stage must say so
/// too. A stage that ran *after* one claiming it was never reached is impossible in a
/// linear pipeline, so observing it means one of the two notes is false.
///
/// **Agreement:** every `not reached (X failed earlier)` note in one compile must name
/// the same `X`. Two stages blaming different predecessors cannot both be right, and
/// detecting it needs no map from `FailedPhase` to `StageKind`.
///
/// # The formulation that was tried first and is WRONG
///
/// *"Once a cell is `.` in the matrix, every later cell is `.`"* — false, and
/// legitimately so. `OverDeterminedShaft` reads `OOOO.X.....`: Flatten is `.` *before*
/// DAE's `X`, because Flatten's `.` means **ran, no flat model retained**, not *not
/// reached*. The class is ambiguous where the note is not, which is the whole reason
/// these assert on notes.
///
/// # What building it found
///
/// The DAE stage rendered a **wholly blank tab** for a compile with no pipeline result
/// — `MissingComponentClass` and `UndefinedRef` — while its six siblings all carried
/// the no-result note. It was deliberate, on the reasoning that Flatten already
/// reported it; see `dae_absent_stage`, where that arm is now fixed.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compiles the whole specimen corpus; run with --features slow-tests"
)]
fn the_not_reached_tail_is_contiguous_and_agrees_on_its_cause() {
    let dir = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/specimens"));
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .expect("the specimen directory must be readable")
        .flatten()
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("mo"))
        .filter_map(|e| {
            e.path()
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
        })
        .collect();
    names.sort();
    assert!(names.len() >= 20, "only {} specimens found", names.len());

    let mut violations: Vec<String> = Vec::new();
    let mut tails_seen = 0usize;

    for name in &names {
        let FromWorker::Compiled { stages, .. } = compile_specimen_shared(name) else {
            panic!("{name}: expected Compiled");
        };

        let mut tail_began: Option<StageKind> = None;
        let mut blamed: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

        for kind in StageKind::COMPILATION {
            let note = stages.get(*kind).note.as_deref();
            if let Some(phase) = blamed_phase(note) {
                blamed.insert(phase);
            }
            match (did_not_run(note), tail_began) {
                (true, None) => tail_began = Some(*kind),
                (false, Some(first)) => violations.push(format!(
                    "{name}: {kind:?} reports work after {first:?} said it did not run \u{2014} \
                     note {note:?}"
                )),
                _ => {}
            }
        }

        if tail_began.is_some() {
            tails_seen += 1;
        }
        if blamed.len() > 1 {
            let list: Vec<&str> = blamed.iter().map(String::as_str).collect();
            violations.push(format!(
                "{name}: stages blame different predecessors {list:?}; at most one can be right"
            ));
        }
    }

    // Non-vacuity: a corpus where nothing ever fails proves neither property.
    assert!(
        tails_seen >= 5,
        "only {tails_seen} specimens have a not-reached tail; these checks barely ran",
    );
    assert!(
        violations.is_empty(),
        "{} not-reached violation(s):\n  {}",
        violations.len(),
        violations.join("\n  "),
    );
}

/// **A successful simulation can contain an infinity, and it must be reported.**
///
/// # The measurement behind this test
///
/// Two probes failed to reproduce it before the third succeeded, and the failures
/// say where the solver's guards actually live. A model infinite at `t = 0` is
/// caught by `rumoca-solver`'s projection guard at initialization; a state crossing
/// zero mid-run also died at init, on an unrelated tolerance stall. What got through
/// was a singularity **independent of any state** — `y = 1 / (time - 0.5)` — because
/// the integrator's error control follows states, and an algebraic output is not
/// one. That returned `Ok` with exactly one non-finite sample.
///
/// So the guard cannot be *"the solver rejects non-finite values"*. It does, in the
/// paths it watches, and this is the path it does not.
#[test]
fn a_simulation_that_succeeded_still_reports_its_non_finite_values() {
    let finite = SimData {
        times: vec![0.0, 0.5, 1.0],
        names: vec!["x".into(), "y".into()],
        data: vec![vec![1.0, 0.5, 0.0], vec![-2.0, -4.0, 2.0]],
        n_states: 1,
        has_discontinuities: false,
        solver_steps: Vec::new(),
    };
    assert!(
        finite.non_finite_series().is_empty(),
        "a healthy run must not be accused of anything",
    );

    // The shape the probe actually produced: one output, one bad sample.
    let mut probed = finite.clone();
    probed.data[1][1] = f64::INFINITY;
    assert_eq!(
        probed.non_finite_series(),
        vec![("y".to_owned(), 1)],
        "the offending series must be named, with how many samples are affected",
    );

    // NaN counts too, and a state is no more exempt than an output.
    let mut both = finite.clone();
    both.data[0][2] = f64::NAN;
    both.data[1][0] = f64::NEG_INFINITY;
    both.data[1][2] = f64::NAN;
    assert_eq!(
        both.non_finite_series(),
        vec![("x".to_owned(), 1), ("y".to_owned(), 2)],
        "every affected series is listed, in the order the model declares them",
    );
}

/// **No `OutputCapture` is ever started while another is live.**
///
/// # Why this is a source scan and not a runtime probe
///
/// The obvious test — start one, start a second inside it, observe what happens — would
/// manipulate **fd 1 and fd 2 under the test harness**. `CLAUDE.md` records that this
/// exact ownership is why a hung run stops printing which test it is on, and a test
/// that corrupts the harness's own output is a poor trade for a fact that can be
/// established by reading. So this checks the structure that makes nesting impossible,
/// and leaves the file descriptors alone.
///
/// # The invariant, and why it holds today
///
/// `OutputCapture::start()` has exactly **two** call sites: `compile_target`, which
/// holds one for a whole compile, and `simulate`, which starts one only for the
/// `nan_trace` retry. They cannot overlap because **`simulate` does not call
/// `compile_target`** — it locates the specimen through the same helper and then runs
/// its own pipeline — and because the worker loop handles one message at a time.
///
/// # What breaks it, which is what this fails on
///
/// A third call site, or `simulate` gaining a call to `compile_target`. Either would
/// make nesting reachable. **Nesting is not obviously catastrophic** — `start` saves
/// the *current* fd 1/2 with `dup`, so a strict LIFO drop restores correctly — but an
/// outer capture that drains while an inner one owns the descriptors reads **nothing**,
/// and a non-LIFO drop crosses them. Neither failure announces itself.
///
/// Added 2026-08-25 for a question Claude created the same day and did not answer: the
/// `nan_trace` retry was written on the assumption that the two never overlap, and an
/// assumption made while writing code is not a measured fact.
#[test]
fn no_output_capture_is_started_while_another_is_live() {
    let src = std::fs::read_to_string(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/worker.rs"),
    )
    .expect("worker.rs is readable");

    // Which function encloses each call site: the nearest preceding `fn` at column 4
    // (an inherent method) or column 0 (a free function).
    let sites = capture_sites(&src);

    let owners: Vec<&str> = sites.iter().map(|(_, o)| o.as_str()).collect();
    assert_eq!(
        owners,
        vec!["simulate", "compile_target"],
        "the OutputCapture call sites moved. Two captures live at once cannot be \
         reasoned about: an outer one draining while an inner owns fd 1/2 reads \
         nothing, and a non-LIFO drop crosses the descriptors. Found at {sites:?}",
    );

    // The reachability half: `simulate` must not call `compile_target`, or the two
    // captures could nest even though the call sites are unchanged.
    let start = src.find("    fn simulate(").expect("simulate exists");
    let end = src[start..]
        .find("\n    fn ")
        .map(|o| start + o)
        .unwrap_or(src.len());
    let body = &src[start..end];
    assert!(
        !body.contains("compile_target("),
        "`simulate` now calls `compile_target`, which holds an OutputCapture for the \
         whole compile \u{2014} so simulate's nan_trace retry would start a second one \
         inside it",
    );
}

/// Every `OutputCapture::start()` call site in `text`, as `(line, enclosing fn)`.
///
/// **Comment lines are skipped**, because prose mentioning the call is not a call —
/// and this file's own doc comments name it repeatedly, which is exactly how a source
/// scan acquires a false positive it cannot explain.
fn capture_sites(text: &str) -> Vec<(usize, String)> {
    let lines: Vec<&str> = text.lines().collect();
    let mut sites = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("//") || !line.contains("OutputCapture::start()") {
            continue;
        }
        let owner = lines[..i]
            .iter()
            .rev()
            .find_map(|l| {
                let is_item = l.starts_with("    fn ")
                    || l.starts_with("fn ")
                    || l.starts_with("    pub fn ")
                    || l.starts_with("pub fn ")
                    || l.starts_with("    pub(crate) fn ");
                is_item.then(|| {
                    l.trim_start()
                        .split("fn ")
                        .nth(1)
                        .unwrap_or("")
                        .split(['(', '<'])
                        .next()
                        .unwrap_or("")
                        .to_owned()
                })
            })
            .unwrap_or_default();
        sites.push((i + 1, owner));
    }
    sites
}

/// **The must-fire half for [`no_output_capture_is_started_while_another_is_live`]**,
/// over literals rather than the real file.
///
/// The alternative — perturbing an actual call site — would mean editing code that
/// owns fd 1 and 2 to prove a scanner works. Testing the scanner directly costs
/// nothing and risks nothing, which is the trade an unattended run should always take.
#[test]
fn the_capture_site_scanner_finds_what_it_claims() {
    let two = "    fn simulate(&mut self) {\n\
               \x20       let c = OutputCapture::start();\n\
               \x20   }\n\
               \x20   fn compile_target(&mut self) {\n\
               \x20       let c = OutputCapture::start();\n\
               \x20   }\n";
    assert_eq!(
        capture_sites(two)
            .iter()
            .map(|(_, o)| o.as_str())
            .collect::<Vec<_>>(),
        vec!["simulate", "compile_target"],
        "the two real sites must be attributed to their own functions",
    );

    // A third site is the defect this guards against, and it must be seen.
    let three = format!(
        "{two}    fn somewhere_new(&mut self) {{\n        let c = OutputCapture::start();\n    }}\n"
    );
    assert_eq!(
        capture_sites(&three).len(),
        3,
        "a new call site must be found, or nesting becomes reachable in silence",
    );

    // Prose naming the call is not a call.
    let commented = "    fn simulate(&mut self) {\n\
                     \x20       // OutputCapture::start() is deliberately not used here\n\
                     \x20   }\n";
    assert!(
        capture_sites(commented).is_empty(),
        "a comment mentioning the call must not be counted as one",
    );
}

/// **Every entry's `depth` equals the number of brackets open above it.**
///
/// # Why this, when three bracket tests already exist
///
/// They check that brackets **name a real phase**, **pair with their own end**, run in
/// **pipeline order**, and are **timed**. Depth is checked only as `> 0`, for four
/// phases — so a bug that pinned every entry to depth 1, or let depth drift upward
/// across a compile, would pass all of them while the log rendered nesting that never
/// happened.
///
/// **Nesting is a claim.** `CLAUDE.md`: *"A log line describes what happened, not what
/// reads well… Ordering, nesting and attribution are claims, and a claim that reads
/// nicely is still a claim."* The "DAE pipeline" bracket that named a phase which does
/// not exist was exactly this failure, invented to give five phases a tidy parent.
///
/// # What it would catch that nothing else does
///
/// A `LogEntry` built by hand rather than through `make_log` — which is not
/// hypothetical, since the worker's panic path constructs one directly. Any such site
/// picks its own `depth`, and picking it wrong is invisible to every other check.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn every_log_entry_is_nested_as_deeply_as_the_brackets_above_it() {
    let logs = compile_specimen_logs_shared("SingleInertia");
    assert!(logs.len() > 20, "only {} log entries", logs.len());

    // Depth derived from the message stream alone, independently of the counter the
    // worker threads through the compile.
    let mut open = 0usize;
    for (i, entry) in logs.iter().enumerate() {
        if matches!(entry.level, LogLevel::StageEnd) {
            open = open.checked_sub(1).unwrap_or_else(|| {
                panic!(
                    "entry {i} closes a bracket that was never opened: {:?}",
                    entry.message
                )
            });
        }
        assert_eq!(
            entry.depth as usize, open,
            "entry {i} ({:?}, {:?}) reports depth {} but {} bracket(s) are open above \
             it. The log renders nesting that did not happen.",
            entry.level, entry.message, entry.depth, open,
        );
        if matches!(entry.level, LogLevel::StageStart) {
            open += 1;
        }
    }
    assert_eq!(
        open, 0,
        "the compile ended with {open} bracket(s) still open, so every later entry in \
         this session renders inside a phase that had finished",
    );
}

/// **No hand-built `LogEntry` invents its own elapsed time.**
///
/// # The defect this was written for
///
/// Almost every log line goes through `make_log`, which stamps the real elapsed time
/// from the compile's own clock. The worker's panic path cannot: it runs *outside*
/// `compile_target`, where that clock does not exist. So it built a `LogEntry` by hand
/// and wrote `elapsed_secs: 0.0` — reporting a panic forty seconds into a compile as
/// having happened at t=0.
///
/// **A timestamp is a claim.** `CLAUDE.md`: *"A log line describes what happened, not
/// what reads well."* A fabricated one is small, which is exactly why it survived
/// review on the day it was written and was found only by a later sweep.
///
/// # Why a source scan rather than a behavioural test
///
/// Reaching the panic path means panicking a real worker thread mid-compile, and the
/// value here is preventing the *next* hand-built entry from guessing — which is a
/// property of the source, not of a run.
#[test]
fn no_hand_built_log_entry_hardcodes_its_elapsed_time() {
    let src = std::fs::read_to_string(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/worker.rs"),
    )
    .expect("worker.rs is readable");

    let offenders: Vec<(usize, String)> = src
        .lines()
        .enumerate()
        .filter(|(_, l)| {
            let t = l.trim();
            !t.starts_with("//")
                && t.starts_with("elapsed_secs:")
                // A literal is a guess; anything computed is a measurement.
                && t.trim_end_matches(',')
                    .trim_start_matches("elapsed_secs:")
                    .trim()
                    .parse::<f64>()
                    .is_ok()
        })
        .map(|(i, l)| (i + 1, l.trim().to_owned()))
        .collect();

    assert!(
        offenders.is_empty(),
        "a LogEntry states its elapsed time as a literal, which fabricates when the \
         event happened. Measure it \u{2014} take an `Instant` at the nearest enclosing \
         scope and use `elapsed()`:\n  {}",
        offenders
            .iter()
            .map(|(n, l)| format!("worker.rs:{n}: {l}"))
            .collect::<Vec<_>>()
            .join("\n  "),
    );

    // Non-vacuity: the scan reaches real `elapsed_secs` sites, so an empty result
    // means "none are literals", not "none were looked at".
    let seen = src
        .lines()
        .filter(|l| l.trim().starts_with("elapsed_secs:"))
        .count();
    assert!(
        seen >= 2,
        "only {seen} elapsed_secs site(s) found; the scan is not reading the file",
    );
}

/// **Two compiles of one specimen log the same structure.**
///
/// # Why this exists: it replaces a guarantee that was only ever a coincidence
///
/// Until 2026-08-26, six tests each ran their own compile of `SingleInertia` purely to
/// inspect the log. That gave six independent samples — so a log that varied run to run
/// had six chances to be noticed. **Nothing claimed to check that.** It happened by
/// repetition, and the repetition cost 75.5 s of a 245 s suite.
///
/// Sharing one capture ([`compile_specimen_logs_shared`]) brings those six to ~13 s and
/// removes the coincidence. **This states the property instead**, which is strictly
/// better: an invariant that fails by name beats a guarantee nobody wrote down. It is
/// the same move `compile_specimen_uncached` records for the result cache — memoising
/// removed an accidental determinism check, so one test was written to keep it.
///
/// # What "the same structure" means, and what it deliberately excludes
///
/// The **bracket sequence and its nesting**: for every `StageStart`/`StageEnd`, the
/// canonical phase name and the depth. **Timings are excluded on purpose** — they
/// differ every run, and `every_bracket_is_timed_and_none_costs_less_than_its_contents`
/// is the test that checks them. Trace and stdout lines are excluded too: their
/// presence depends on what the compiler chose to say, which is not a structural claim.
#[test]
#[cfg_attr(
    not(feature = "slow-tests"),
    ignore = "compile-heavy; run with --features slow-tests"
)]
fn two_compiles_of_one_specimen_log_the_same_structure() {
    // Uncached on both sides: a memoised second capture would compare a value with
    // itself and pass while checking nothing.
    let shape = |logs: &[LogEntry]| -> Vec<(String, &'static str, u8)> {
        logs.iter()
            .filter(|e| matches!(e.level, LogLevel::StageStart | LogLevel::StageEnd))
            .map(|e| {
                let level = format!("{:?}", e.level);
                let name = bracket_phase_name(&e.message).unwrap_or("<unnamed>");
                (level, name, e.depth)
            })
            .collect()
    };

    let first = capture_compile_logs("SingleInertia");
    let second = capture_compile_logs("SingleInertia");

    // Non-vacuity: two empty logs are trivially equal.
    let shape_first = shape(&first);
    assert!(
        shape_first.len() >= 20,
        "only {} bracket entries; the capture is not exercising a real compile",
        shape_first.len(),
    );

    assert_eq!(
        shape_first,
        shape(&second),
        "two compiles of the same specimen produced different log structures. The six \
         tests that read a SHARED capture are only sound while this holds \u{2014} each \
         of them now inspects one run rather than its own.",
    );
}
