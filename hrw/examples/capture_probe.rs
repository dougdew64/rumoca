//! Print an enriched capture for a real specimen, to check what it carries.
//!
//! `cargo run -p hrw --example capture_probe -- MotorWithBrake __pre__.overSpeed`
//!
//! Exists to answer the only question that matters about the capture's design:
//! does it carry what a reader would otherwise have to go find by hand? Reading
//! the emitted file is the measurement; the unit tests check shape, not value.
use std::path::PathBuf;

fn main() {
    let mut args = std::env::args().skip(1);
    let model = args.next().unwrap_or_else(|| "MotorWithBrake".to_owned());
    let follow = args
        .next()
        .unwrap_or_else(|| "__pre__.overSpeed".to_owned());

    let path = PathBuf::from(format!(
        "{}/specimens/{model}.mo",
        env!("CARGO_MANIFEST_DIR")
    ));
    let libs = vec![PathBuf::from(format!(
        "{}/vendor/msl",
        env!("CARGO_MANIFEST_DIR")
    ))];
    let hrw::worker::FromWorker::Compiled { stages, .. } =
        hrw::worker::compile_specimen(&path, libs).expect("compile")
    else {
        panic!("expected Compiled");
    };

    let pairs = stages.as_stage_pairs();
    let tracking = hrw::bridge::Tracking {
        seq: 1,
        name: &follow,
        declared_line: None,
        declaring_class: None,
        stage_values: &pairs,
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&hrw::bridge::build_tracking(&tracking)).unwrap()
    );
}
