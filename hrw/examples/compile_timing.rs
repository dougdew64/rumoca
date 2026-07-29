//! What the uncached compile actually costs, per specimen reselect.
//!
//! Mirrors `WorkerState`: one long-lived `Session` with the MSL loaded once,
//! then the same model compiled repeatedly — which is exactly what reselecting
//! a specimen in HRW does. Measures the cached call against the uncached one,
//! so the cost of choosing debuggability over speed is a number rather than an
//! assumption.
use std::path::PathBuf;
use std::time::Instant;

use rumoca_compile::compile::SourceRootKind;
use rumoca_compile::source_roots::{parse_source_root_with_cache, source_root_source_set_key};
use rumoca_compile::{Session, SessionConfig};

fn main() {
    let msl = PathBuf::from(format!("{}/vendor/msl", env!("CARGO_MANIFEST_DIR")));
    let mut session = Session::new(SessionConfig::default());
    let t = Instant::now();
    let parsed = parse_source_root_with_cache(&msl).expect("parse MSL");
    let key = source_root_source_set_key(&msl.to_string_lossy());
    session.replace_parsed_source_set(&key, SourceRootKind::DurableExternal, parsed.documents, None);
    println!("MSL load: {:?}\n", t.elapsed());

    for model in ["MotorWithBrake", "BouncingBall"] {
        let path = PathBuf::from(format!("{}/specimens/{model}.mo", env!("CARGO_MANIFEST_DIR")));
        let source = std::fs::read_to_string(&path).expect("read specimen");
        let uri = path.to_string_lossy().to_string();
        session.remove_document(&uri);
        session.update_document(&uri, &source);
        let qualified = session.qualify_model_name(&uri, model);

        let t = Instant::now();
        let _ = session.compile_model_strict_reachable_uncached_with_recovery(&qualified);
        let cold = t.elapsed();

        // Now the cache is warm for this model: what HRW used to serve.
        let t = Instant::now();
        let _ = session.compile_model_strict_reachable_with_recovery(&qualified);
        let cached = t.elapsed();

        // And what it serves now, on every reselect.
        let t = Instant::now();
        let _ = session.compile_model_strict_reachable_uncached_with_recovery(&qualified);
        let uncached = t.elapsed();

        println!("{model}");
        println!("  first compile      {cold:?}");
        println!("  cached (was)       {cached:?}");
        println!("  uncached (now)     {uncached:?}");
    }
}
