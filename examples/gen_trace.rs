//! Generate a specimen's durable **compilation trace log** — the IR of every
//! pipeline stage (parse … structural) written under `docs/notebook/<Model>/trace/`,
//! plus a `manifest.json` recording the Rumoca rev and a specimen content hash.
//!
//! The trace is the *ground truth* the specimen's `narrative.md` is written
//! against. Regenerate after editing the specimen or bumping the Rumoca pin
//! (see `docs/updating-rumoca.md`), then review the narrative against the diff.
//!
//! ```text
//! cargo run --example gen_trace -- ProportionalLoop
//! ```
//!
//! Uses `hrw::worker::compile_specimen` — the exact path the app's worker runs —
//! so the trace is byte-identical to what the running observatory produces.

use std::path::PathBuf;

use hrw::worker::{compile_specimen, FromWorker, Stage};

/// The six pipeline stages, in order, as they appear in the app's tabs.
const STAGES: [&str; 6] =
    ["parse", "resolve", "instantiate", "typecheck", "flatten", "structural"];

fn main() {
    let name = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: cargo run --example gen_trace -- <SpecimenName>");
        std::process::exit(2);
    });
    let root = env!("CARGO_MANIFEST_DIR");
    let specimen = PathBuf::from(format!("{root}/specimens/{name}.mo"));
    let source = std::fs::read_to_string(&specimen)
        .unwrap_or_else(|e| panic!("read {}: {e}", specimen.display()));

    // Same MSL source roots the app loads at startup / the worker tests use.
    let base = format!("{root}/vendor/msl");
    let libraries = vec![
        PathBuf::from(format!("{base}/Modelica 4.1.0")),
        PathBuf::from(format!("{base}/ModelicaServices 4.1.0")),
        PathBuf::from(format!("{base}/Complex.mo")),
    ];

    let FromWorker::Compiled { model, parse, resolve, instantiate, typecheck, flatten, structural, .. } =
        compile_specimen(&specimen, libraries).expect("compile specimen")
    else {
        panic!("expected a Compiled result");
    };
    let model = model.unwrap_or_else(|| name.clone());
    let by_name: [(&str, &Stage); 6] = [
        ("parse", &parse),
        ("resolve", &resolve),
        ("instantiate", &instantiate),
        ("typecheck", &typecheck),
        ("flatten", &flatten),
        ("structural", &structural),
    ];

    let trace_dir = PathBuf::from(format!("{root}/docs/notebook/{model}/trace"));
    std::fs::create_dir_all(&trace_dir).expect("create trace dir");

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

    let manifest = serde_json::json!({
        "model": model,
        "specimen": format!("specimens/{name}.mo"),
        // Non-cryptographic content hash — a staleness anchor: if it changes, the
        // trace (and any narrative written against it) needs regenerating.
        "specimen_fnv1a": format!("{:016x}", fnv1a(source.as_bytes())),
        "rumoca_rev": option_env!("HRW_RUMOCA_REV").unwrap_or("unknown"),
        "rumoca_version": option_env!("HRW_RUMOCA_VERSION").unwrap_or("unknown"),
        "stages": manifest_stages,
    });
    std::fs::write(
        trace_dir.join("manifest.json"),
        format!("{}\n", serde_json::to_string_pretty(&manifest).unwrap()),
    )
    .expect("write manifest");

    eprintln!("wrote trace for {model} → {}", trace_dir.display());
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
