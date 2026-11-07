//! End-to-end verification of the dependency-loading path, mirroring the
//! worker: load the staged reference MSL 4.1.0 as source roots, then resolve a
//! specimen that references MSL. The proof the `Modelica.*` references resolved is
//! that **no diagnostic reports anything unresolved** — an unloaded library would
//! yield "unresolved type reference" errors.
//!
//! *Until 2026-11-03 the proof was a clean `Ok` from `resolved()`, which stopped
//! being a proxy for it on Rumoca v0.10.0; the test body says why.*

use std::path::Path;

use rumoca_compile::compile::SourceRootKind;
use rumoca_compile::source_roots::{parse_source_root_with_cache, source_root_source_set_key};
use rumoca_compile::{Session, SessionConfig};

/// The MSL roots as strings, from the one home in `hrw::msl_roots`.
///
/// This held its own three-root copy, which 0.10.0 rejects — see
/// `hrw::msl_roots` for the measurement and why there is now one home.
fn msl_roots() -> Vec<String> {
    hrw::msl_roots()
        .into_iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect()
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
    let path = hrw::manifest_path!("specimens", "RotationalInertia.mo");
    let source = std::fs::read_to_string(path).expect("read specimen");

    let mut session = session_with_msl();
    let uri = path.to_string();
    session.update_document(&uri, &source);

    // **Recovery mode, and the proof is now asserted rather than inferred.**
    //
    // This called `session.resolved()` — `ResolveBuildMode::Standard` — and took a
    // clean `Ok` as proof that the `Modelica.*` references bound, on the reasoning
    // in this file's header: an unloaded library would yield "unresolved type
    // reference" instead. **v0.10.0 breaks the proxy, not the proof.** Standard mode
    // treats as fatal the ~75 resolve errors the 0.10.0 MSL emits about its own
    // `Modelica.Mechanics` friction classes (MLS §5.3.1), so `Ok` is unreachable for
    // any model that loads the library, however well its own references resolve.
    //
    // So this now builds the tree the compile itself uses, and asserts the
    // discriminating property directly: **no diagnostic says anything is
    // unresolved.** That is strictly closer to what the header always claimed the
    // test proved.
    let (resolved, diagnostics) = session
        .strict_compile_resolved()
        .unwrap_or_else(|e| panic!("the library could not be resolved at all:\n{e:#}"));

    // **Scoped to the packages this specimen uses, since v0.10.4 — the library-wide
    // form was measuring the wrong unit.**
    //
    // It asserted *zero* "unresolved" diagnostics anywhere in the resolved library.
    // v0.10.4 surfaces three the specimen cannot possibly cause —
    // `Medium.density_pTX`, `Medium.p_default`, `Medium.T_default` — which are
    // `Modelica.Media`/`Fluid` members of an un-redeclared `replaceable package
    // Medium`, with nothing to bind to until a medium is supplied.
    // `RotationalInertia.mo` contains the string `Medium` zero times.
    //
    // So the old assertion failed on a library the specimen resolved against
    // perfectly. The discriminating property this test's header always claimed —
    // *"an unresolved reference means the MSL did not load"* — is about the
    // specimen's own references, and those live in exactly two packages.
    //
    // **Not scoped by file**, because `Label` carries only a `Span` and a span needs
    // the right source map to name a file; `docs/tech-debt.md` records four sites that
    // resolved one against whichever file the caller happened to hold.
    const PACKAGES_THIS_SPECIMEN_USES: [&str; 2] = ["Modelica.Mechanics", "Modelica.Blocks"];
    let unresolved: Vec<&str> = diagnostics
        .iter()
        .filter(|d| d.message.contains("unresolved"))
        .map(|d| d.message.as_str())
        .filter(|m| PACKAGES_THIS_SPECIMEN_USES.iter().any(|p| m.contains(p)))
        .collect();
    assert!(
        unresolved.is_empty(),
        "an unresolved reference in a package this specimen uses means the MSL did not \
         load, which is what this test exists to catch:\n  {}",
        unresolved.join("\n  "),
    );

    let qualified = session.qualify_model_name(&uri, "RotationalInertia");
    let class = resolved
        .tree()
        .get_class_by_qualified_name(&qualified)
        .expect("RotationalInertia present in resolved tree");

    // Extracted user-class stays small (not the whole 430MB MSL aggregate).
    let json = serde_json::to_string(&serde_json::to_value(class).unwrap()).unwrap();
    eprintln!(
        "resolved RotationalInertia — user-class JSON bytes = {}",
        json.len()
    );
    assert!(
        json.len() < 200_000,
        "extracted class unexpectedly large: {}",
        json.len()
    );
}

/// Negative control: with NO libraries loaded, the same MSL references must
/// fail to resolve — proving the source roots are what makes resolution work.
#[test]
fn same_specimen_fails_without_libraries() {
    let path = hrw::manifest_path!("specimens", "RotationalInertia.mo");
    let source = std::fs::read_to_string(path).expect("read specimen");

    let mut session = Session::new(SessionConfig::default());
    let uri = path.to_string();
    session.update_document(&uri, &source);

    assert!(
        session.resolved().is_err(),
        "expected resolve to fail without MSL loaded"
    );
}
