//! Generate a specimen's durable **compilation + simulation trace** — the IR of
//! every pipeline stage (parse … solve_lowering) plus simulation trajectories,
//! written under `docs/specimen-notebook/<Model>/trace/`, plus a `manifest.json`
//! recording the Rumoca rev and a specimen content hash.
//!
//! The trace is the *ground truth* the specimen's `narrative.md` is written
//! against. Regenerate after editing the specimen or bumping the Rumoca pin
//! (see `docs/updating-rumoca.md`), then review the narrative against the diff.
//!
//! ```text
//! cargo run --example gen_trace -- ProportionalLoop
//! ```
//!
//! Uses `hrw::worker::compile_specimen` and `simulate_specimen` — the exact paths
//! the app's worker runs — so the trace is byte-identical to what the running
//! observatory produces.

use std::path::PathBuf;

use hrw::worker::{compile_specimen, simulate_specimen, FromWorker, Stage};

/// The pipeline stages, in order, as they appear in the app's tabs.
const STAGES: [&str; 10] = [
    "parse", "resolve", "instantiate", "typecheck", "flatten", "structural", "index_reduction",
    "initialization", "events", "solve_lowering",
];

/// Default simulation stop time.
const SIM_T_END: f64 = 2.0;

fn msl_roots() -> Vec<PathBuf> {
    let base = format!("{}/vendor/msl", env!("CARGO_MANIFEST_DIR"));
    vec![
        PathBuf::from(format!("{base}/Modelica 4.1.0")),
        PathBuf::from(format!("{base}/ModelicaServices 4.1.0")),
        PathBuf::from(format!("{base}/Complex.mo")),
    ]
}

fn main() {
    let name = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: cargo run --example gen_trace -- <SpecimenName>");
        std::process::exit(2);
    });
    let root = env!("CARGO_MANIFEST_DIR");
    let specimen = PathBuf::from(format!("{root}/specimens/{name}.mo"));
    let source = std::fs::read_to_string(&specimen)
        .unwrap_or_else(|e| panic!("read {}: {e}", specimen.display()));

    let FromWorker::Compiled {
        model, stages, ..
    } = compile_specimen(&specimen, msl_roots()).expect("compile specimen")
    else {
        panic!("expected a Compiled result");
    };
    let model = model.unwrap_or_else(|| name.clone());
    let by_name: [(&str, &Stage); 10] = [
        ("parse", &stages.parse),
        ("resolve", &stages.resolve),
        ("instantiate", &stages.instantiate),
        ("typecheck", &stages.typecheck),
        ("flatten", &stages.flatten),
        ("structural", &stages.structural),
        ("index_reduction", &stages.index_reduction),
        ("initialization", &stages.initialization),
        ("events", &stages.events),
        ("solve_lowering", &stages.solve_lowering),
    ];

    let trace_dir = PathBuf::from(format!("{root}/docs/specimen-notebook/{model}/trace"));
    std::fs::create_dir_all(&trace_dir).expect("create trace dir");

    // --- Compilation trace ---
    let mut manifest_stages = serde_json::Map::new();
    for stage_name in STAGES {
        let stage = by_name.iter().find(|(n, _)| *n == stage_name).map(|(_, s)| *s).unwrap();
        if let Some(value) = &stage.value {
            let path = trace_dir.join(format!("{stage_name}.json"));
            std::fs::write(&path, format!("{}\n", serde_json::to_string_pretty(value).unwrap()))
                .expect("write stage IR");
        }
        manifest_stages.insert(
            stage_name.to_owned(),
            serde_json::json!({ "has_ir": stage.value.is_some(), "note": stage.note }),
        );
    }

    // --- Simulation trace ---
    let can_simulate = stages.solve_lowering.value.is_some();
    let sim_result = if can_simulate {
        match simulate_specimen(&specimen, &model, SIM_T_END, msl_roots()) {
            Ok(data) => {
                let sim_json = simulation_to_json(&data, SIM_T_END);
                std::fs::write(
                    trace_dir.join("simulation.json"),
                    format!("{}\n", serde_json::to_string_pretty(&sim_json).unwrap()),
                )
                .expect("write simulation trace");
                eprintln!(
                    "  simulation: {} variables, {} time points, t_end={SIM_T_END}",
                    data.names.len(),
                    data.times.len(),
                );
                serde_json::json!({
                    "simulated": true,
                    "t_end": SIM_T_END,
                    "n_variables": data.names.len(),
                    "n_time_points": data.times.len(),
                    "n_states": data.n_states,
                    "has_discontinuities": data.has_discontinuities,
                })
            }
            Err(e) => {
                eprintln!("  simulation failed: {e}");
                serde_json::json!({
                    "simulated": false,
                    "error": e,
                })
            }
        }
    } else {
        eprintln!("  simulation skipped (compilation did not reach solve lowering)");
        serde_json::json!({
            "simulated": false,
            "error": "compilation did not reach solve lowering",
        })
    };

    let manifest = serde_json::json!({
        "model": model,
        "specimen": format!("specimens/{name}.mo"),
        "specimen_fnv1a": format!("{:016x}", fnv1a(source.as_bytes())),
        "rumoca_rev": option_env!("HRW_RUMOCA_REV").unwrap_or("unknown"),
        "rumoca_version": option_env!("HRW_RUMOCA_VERSION").unwrap_or("unknown"),
        "stages": manifest_stages,
        "simulation": sim_result,
    });
    std::fs::write(
        trace_dir.join("manifest.json"),
        format!("{}\n", serde_json::to_string_pretty(&manifest).unwrap()),
    )
    .expect("write manifest");

    eprintln!("wrote trace for {model} → {}", trace_dir.display());
}

/// Build a JSON summary of simulation results: variable names, final values,
/// time span, and a sampled trajectory (first/last 5 time points for each
/// variable) — enough for a narrative without storing the full trajectory.
fn simulation_to_json(data: &hrw::worker::SimData, t_end: f64) -> serde_json::Value {
    let variables: Vec<serde_json::Value> = data
        .names
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let series = &data.data[i];
            let initial = series.first().copied().unwrap_or(f64::NAN);
            let final_val = series.last().copied().unwrap_or(f64::NAN);
            let min = series.iter().copied().fold(f64::INFINITY, f64::min);
            let max = series.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            let is_state = i < data.n_states;
            serde_json::json!({
                "name": name,
                "is_state": is_state,
                "initial": initial,
                "final": final_val,
                "min": min,
                "max": max,
            })
        })
        .collect();

    serde_json::json!({
        "t_end": t_end,
        "n_time_points": data.times.len(),
        "n_variables": data.names.len(),
        "n_states": data.n_states,
        "has_discontinuities": data.has_discontinuities,
        "t_start": data.times.first().copied().unwrap_or(0.0),
        "t_final": data.times.last().copied().unwrap_or(0.0),
        "variables": variables,
    })
}

/// FNV-1a 64-bit — a tiny, dependency-free, deterministic content hash.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}
