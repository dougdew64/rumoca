//! Generate a specimen's durable **compilation + simulation trace** — the IR of
//! every pipeline stage (parse … solve_lowering) plus simulation trajectories,
//! written under `docs/specimen-notebook/<Model>/trace/`, plus a `manifest.json`
//! recording the Rumoca rev and a specimen content hash.
//!
//! The trace is the **ground truth** for every number anyone states about a
//! specimen. Regenerate after editing the specimen or bumping the Rumoca pin
//! (see `docs/updating-rumoca.md`) and review the diff to see what changed.
//!
//! It used to be the reference a hand-written `narrative.md` was checked
//! against; those narratives were retired 2026-07-29 (`docs/ideas.md` #42)
//! because Claude regenerates that explanation on demand. Nothing now needs
//! re-verifying after a regeneration — which is the point of generating it.
//!
//! ```text
//! cargo run --example gen_trace -- ProportionalLoop   # one specimen
//! cargo run --example gen_trace -- --all              # all specimens
//! ```
//!
//! Uses `hrw::worker::compile_specimen` and `simulate_specimen` — the exact paths
//! the app's worker runs — so the trace is byte-identical to what the running
//! observatory produces.

use std::path::PathBuf;

use hrw::worker::{FromWorker, Stage, StageKind, compile_specimen, simulate_specimen};

// **The stage roster is `StageKind::COMPILATION`, not a list kept here.**
//
// This file used to carry `const STAGES: [&str; 11]`, a second roster nothing held
// to the first. `Dae` was added to the pipeline and this list was not updated, so
// every notebook regenerated afterwards described an eleven-stage compiler — 7 of 21
// manifests had a `dae` entry and the other 14 did not, for seventeen days, with
// nothing able to notice. `StageKind::notebook_key()` derives the key from the
// canonical slug so the two cannot drift again.

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
    let arg = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: cargo run --example gen_trace -- <SpecimenName>");
        eprintln!("       cargo run --example gen_trace -- --all");
        std::process::exit(2);
    });

    if arg == "--all" {
        let root = env!("CARGO_MANIFEST_DIR");
        let specimens_dir = PathBuf::from(format!("{root}/specimens"));
        let mut names: Vec<String> = std::fs::read_dir(&specimens_dir)
            .expect("read specimens dir")
            .filter_map(|e| {
                let e = e.ok()?;
                let name = e.file_name().to_string_lossy().to_string();
                name.strip_suffix(".mo").map(|n| n.to_owned())
            })
            .collect();
        names.sort();
        let total = names.len();
        let mut failed = Vec::new();
        for (i, name) in names.iter().enumerate() {
            eprintln!("[{}/{}] {name}", i + 1, total);
            if let Err(e) = generate_trace(name) {
                eprintln!("  FAILED: {e}");
                failed.push(name.clone());
            }
        }
        if failed.is_empty() {
            eprintln!("\nAll {total} specimens traced successfully.");
        } else {
            eprintln!(
                "\n{} of {total} failed: {}",
                failed.len(),
                failed.join(", ")
            );
            std::process::exit(1);
        }
        return;
    }

    let name = &arg;
    generate_trace(name).unwrap_or_else(|e| {
        eprintln!("FAILED: {e}");
        std::process::exit(1);
    });
}

fn generate_trace(name: &str) -> Result<(), String> {
    let root = env!("CARGO_MANIFEST_DIR");
    let specimen = PathBuf::from(format!("{root}/specimens/{name}.mo"));
    let source = std::fs::read_to_string(&specimen)
        .map_err(|e| format!("read {}: {e}", specimen.display()))?;

    let FromWorker::Compiled { model, stages, .. } =
        compile_specimen(&specimen, msl_roots()).map_err(|e| format!("compile: {e}"))?
    else {
        return Err("expected a Compiled result".into());
    };
    let model = model.unwrap_or_else(|| name.to_owned());
    let by_name: [(&str, &Stage); 11] = [
        ("parse", &stages.parse),
        ("resolve", &stages.resolve),
        ("instantiate", &stages.instantiate),
        ("typecheck", &stages.typecheck),
        ("flatten", &stages.flatten),
        ("dae", &stages.dae),
        ("structural", &stages.structural),
        ("index_reduction", &stages.index_reduction),
        ("initialization", &stages.initialization),
        ("events", &stages.events),
        ("solve_lowering", &stages.solve_lowering),
    ];

    let trace_dir = PathBuf::from(format!("{root}/docs/specimen-notebook/{model}/trace"));
    std::fs::create_dir_all(&trace_dir).map_err(|e| format!("create trace dir: {e}"))?;

    // --- Compilation trace ---
    let mut manifest_stages = serde_json::Map::new();
    for kind in StageKind::COMPILATION {
        let stage_name = kind.notebook_key();
        // A stage in the roster with no entry here is a wiring gap, not a missing
        // file: `by_name` is this generator's own map from key to captured stage.
        // Panicking names the stage rather than skipping it silently.
        let stage = by_name
            .iter()
            .find(|(n, _)| *n == stage_name)
            .map(|(_, s)| *s)
            .unwrap_or_else(|| {
                panic!(
                    "stage {stage_name:?} is in StageKind::COMPILATION but gen_trace \
                     has no source for it \u{2014} add it to `by_name`"
                )
            });
        if let Some(value) = &stage.value {
            let path = trace_dir.join(format!("{stage_name}.json"));
            std::fs::write(
                &path,
                format!("{}\n", serde_json::to_string_pretty(value).unwrap()),
            )
            .map_err(|e| format!("write {stage_name}: {e}"))?;
        }
        manifest_stages.insert(
            stage_name,
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
                .map_err(|e| format!("write simulation: {e}"))?;
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
    .map_err(|e| format!("write manifest: {e}"))?;

    eprintln!("  wrote trace for {model} → {}", trace_dir.display());
    Ok(())
}

/// Build a JSON summary of simulation results: variable names, final values,
/// time span, and a sampled trajectory (first/last 5 time points for each
/// variable) — enough to answer questions about the run without storing the
/// full trajectory.
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
