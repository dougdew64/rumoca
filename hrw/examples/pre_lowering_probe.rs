//! Print the `pre()` lowering trace for a specimen — the recorded frames idea
//! #40's animation will replay. `cargo run -p hrw --example pre_lowering_probe`
use std::cell::RefCell;
use std::path::PathBuf;

use rumoca_compile::compile::SourceRootKind;
use rumoca_compile::source_roots::{parse_source_root_with_cache, source_root_source_set_key};
use rumoca_compile::{Session, SessionConfig};

fn main() {
    let model = std::env::args().nth(1).unwrap_or_else(|| "MotorWithBrake".to_owned());
    let msl = PathBuf::from(format!("{}/vendor/msl", env!("CARGO_MANIFEST_DIR")));
    let mut session = Session::new(SessionConfig::default());
    let parsed = parse_source_root_with_cache(&msl).expect("parse MSL");
    let key = source_root_source_set_key(&msl.to_string_lossy());
    session.replace_parsed_source_set(&key, SourceRootKind::DurableExternal, parsed.documents, None);

    let path = PathBuf::from(format!("{}/specimens/{model}.mo", env!("CARGO_MANIFEST_DIR")));
    let source = std::fs::read_to_string(&path).expect("read specimen");
    let uri = path.to_string_lossy().to_string();
    session.update_document(&uri, &source);
    let qualified = session.qualify_model_name(&uri, &model);
    let report = session.compile_model_strict_reachable_uncached_with_recovery(&qualified);
    let result = report.requested_result.expect("compiled");
    let rumoca_compile::compile::PhaseResult::Success(cr) = &result else {
        panic!("compile did not reach DAE");
    };

    let frames = RefCell::new(Vec::new());
    rumoca_phase_dae::to_dae_with_options_traced(
        &cr.flat,
        Default::default(),
        Some(&|f| frames.borrow_mut().push(format!("{:?}", f.step))),
    )
    .expect("to_dae");

    let frames = frames.into_inner();
    println!("{model}: {} frames", frames.len());
    for (i, f) in frames.iter().enumerate() {
        println!("{i:3}  {f}");
    }
}
