//! End-to-end verification of the dependency-loading path, mirroring the
//! worker: load the staged reference MSL 4.1.0 as source roots, then resolve a
//! specimen that references MSL. A clean `Ok` from `resolved()` on an all-MSL
//! model IS the proof the `Modelica.*` references resolved — an unloaded
//! library would instead yield "unresolved type reference" errors.

use std::path::Path;

use rumoca_compile::compile::SourceRootKind;
use rumoca_compile::source_roots::{parse_source_root_with_cache, source_root_source_set_key};
use rumoca_compile::{Session, SessionConfig};

fn msl_roots() -> Vec<String> {
    let base = concat!(env!("CARGO_MANIFEST_DIR"), "/vendor/msl");
    vec![
        format!("{base}/Modelica 4.1.0"),
        format!("{base}/ModelicaServices 4.1.0"),
        format!("{base}/Complex.mo"),
    ]
}

fn session_with_msl() -> Session {
    let mut session = Session::new(SessionConfig::default());
    for root in msl_roots() {
        let parsed = parse_source_root_with_cache(Path::new(&root))
            .unwrap_or_else(|e| panic!("parse source root {root}: {e}"));
        let key = source_root_source_set_key(&root);
        session.replace_parsed_source_set(
            &key,
            SourceRootKind::DurableExternal,
            parsed.documents,
            None,
        );
    }
    session
}

/// The arc-1 specimen in MSL form (a System-Modeler-export shape) must resolve.
#[test]
fn rotational_inertia_specimen_resolves_against_msl() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/specimens/RotationalInertia.mo");
    let source = std::fs::read_to_string(path).expect("read specimen");

    let mut session = session_with_msl();
    let uri = path.to_string();
    session.update_document(&uri, &source);

    let resolved = session
        .resolved()
        .unwrap_or_else(|e| panic!("resolve failed (MSL references unresolved?):\n{e:#}"));

    let qualified = session.qualify_model_name(&uri, "RotationalInertia");
    let class = resolved
        .0
        .get_class_by_qualified_name(&qualified)
        .expect("RotationalInertia present in resolved tree");

    // Extracted user-class stays small (not the whole 430MB MSL aggregate).
    let json = serde_json::to_string(&serde_json::to_value(class).unwrap()).unwrap();
    eprintln!("resolved RotationalInertia — user-class JSON bytes = {}", json.len());
    assert!(json.len() < 200_000, "extracted class unexpectedly large: {}", json.len());
}

/// Negative control: with NO libraries loaded, the same MSL references must
/// fail to resolve — proving the source roots are what makes resolution work.
#[test]
fn same_specimen_fails_without_libraries() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/specimens/RotationalInertia.mo");
    let source = std::fs::read_to_string(path).expect("read specimen");

    let mut session = Session::new(SessionConfig::default());
    let uri = path.to_string();
    session.update_document(&uri, &source);

    assert!(
        session.resolved().is_err(),
        "expected resolve to fail without MSL loaded"
    );
}
