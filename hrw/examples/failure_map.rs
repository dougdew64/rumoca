//! **Where does each deliberately-broken specimen actually stop?**
//!
//! ```text
//! cargo run -p hrw --example failure_map
//! ```
//!
//! Written for `docs/ideas.md` #46, which adds a failure specimen per compiler
//! phase. The specimen's comment states which phase it is *meant* to break at;
//! this reports which phase it *does* break at, so the two can be compared
//! before a tour is written about either.
//!
//! **The order matters.** Authoring a specimen and then writing a tour asserting
//! it fails at phase X, without checking, is how a tour comes to teach something
//! untrue — and the tour would pass its own link check the whole time. This is
//! the same discipline as `oracle first for specimens`: find out what actually
//! happens before concluding anything.

use hrw::worker::{FromWorker, StageKind};

fn main() {
    let mut names: Vec<String> =
        std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/specimens"))
            .expect("specimens/ must be readable")
            .filter_map(|e| {
                let p = e.ok()?.path();
                (p.extension()? == "mo").then(|| p.file_stem()?.to_str().map(str::to_owned))?
            })
            .collect();
    names.sort();

    let libs = vec![std::path::PathBuf::from(format!(
        "{}/vendor/msl",
        env!("CARGO_MANIFEST_DIR")
    ))];

    println!("{:<26} {:<22} note", "specimen", "where it stops");
    println!("{}", "-".repeat(96));
    for name in names {
        let path = std::path::PathBuf::from(format!(
            "{}/specimens/{name}.mo",
            env!("CARGO_MANIFEST_DIR")
        ));
        let compiled = hrw::worker::compile_specimen(&path, libs.clone());
        let Ok(FromWorker::Compiled { stages, .. }) = compiled else {
            println!("{name:<26} <compile call failed>");
            continue;
        };

        // **`Failed` and `Flagged` are different questions, and conflating them
        // made the first version of this map useless.**
        //
        // `Flagged` means *noted and carried on*: the value is real and downstream
        // stages consume it. Every model needing index reduction is flagged
        // `singular` at Structural and then fixed — `Drivetrain`, `MotorWithBrake`
        // and `BenchActuator` are healthy specimens that all report it. Reading
        // the first abnormal stage as "where it stopped" therefore listed four
        // working models as broken.
        //
        // `Failed` means *stopped*: no value, nothing downstream.
        let failed: Vec<(&str, String)> = StageKind::COMPILATION
            .iter()
            .filter(|k| stages.get(**k).outcome == hrw::worker::Outcome::Failed)
            .map(|k| {
                (
                    k.name(),
                    stages
                        .get(*k)
                        .note
                        .clone()
                        .unwrap_or_default()
                        .replace('\n', " "),
                )
            })
            .collect();
        let flagged: Vec<&str> = StageKind::COMPILATION
            .iter()
            .filter(|k| stages.get(**k).outcome == hrw::worker::Outcome::Flagged)
            .map(|k| k.name())
            .collect();

        let where_ = match failed.first() {
            Some((stage, _)) => (*stage).to_string(),
            None if flagged.is_empty() => "(compiles cleanly)".to_string(),
            None => format!("(flagged: {})", flagged.join(", ")),
        };
        let note = failed.first().map(|(_, n)| n.as_str()).unwrap_or("");
        // Every stage that FAILED, not just the first: `UnbalancedShaft` fails at
        // both Flatten and DAE construction, deliberately — Flatten kept its copy
        // of the `ToDae` error when the DAE tab was added, so a learner opening
        // either finds an explanation rather than a blank pane.
        let also = if failed.len() > 1 {
            format!("  [+{} more failed]", failed.len() - 1)
        } else {
            String::new()
        };
        println!(
            "{name:<26} {where_:<22} {}{also}",
            &note[..note.len().min(40)]
        );
    }
}
