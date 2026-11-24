use super::{branching_profile, diagnostic_codes, session};
use eglur_driver::{SupportProfile, SupportProfileId, TrustedCatalog};
use eglur_sema::{CompileCapability, PhaseProduct, ResolveCapability, TypecheckCapability};

#[test]
fn residual_heads_contribute_variant_coverage_not_irrefutable_bindings() {
    let catalog = TrustedCatalog::first_slice().unwrap();
    for (arms, expected) in [
        ("None => 0, Some(value) => value", None),
        ("Some(value) => value, None => 0", None),
        ("None => 0", Some("MATCH_NON_EXHAUSTIVE")),
        (
            "None => 0, None => 1, Some(value) => value",
            Some("MATCH_UNREACHABLE_ARM"),
        ),
        ("None => 0, _ => 1", None),
        (
            "None if false => 0, Some(value) => value",
            Some("MATCH_NON_EXHAUSTIVE"),
        ),
    ] {
        let source = format!("fn f(input: Opt[i64]) -> i64 = match input {{ {arms} }}\n");
        let mut session = session(&source);
        let analysis = session
            .resolve(&branching_profile(), Some(&catalog))
            .unwrap();
        let names = analysis.resolution().unwrap().names();
        assert!(
            names
                .locals()
                .iter()
                .all(|local| local.name().text() != "None"),
            "{arms}"
        );
        assert_eq!(
            diagnostic_codes(&session.into_diagnostics(None)),
            expected.into_iter().collect::<Vec<_>>(),
            "{arms}"
        );
    }
}

#[test]
fn residual_heads_keep_nominal_and_payload_diagnostics() {
    let catalog = TrustedCatalog::first_slice().unwrap();
    for (ty, pattern, expected) in [
        ("Opt[i64]", "Some", "PATTERN_PAYLOAD_FORM_MISMATCH"),
        ("Res[i64, Str]", "Ok", "PATTERN_PAYLOAD_FORM_MISMATCH"),
        ("Res[i64, Str]", "Err", "PATTERN_PAYLOAD_FORM_MISMATCH"),
        ("Opt[i64]", "None()", "PATTERN_PAYLOAD_FORM_MISMATCH"),
        ("Opt[i64]", "None {}", "PATTERN_PAYLOAD_FORM_MISMATCH"),
        (
            "Opt[i64]",
            "Some { value }",
            "PATTERN_PAYLOAD_FORM_MISMATCH",
        ),
        (
            "Res[i64, Str]",
            "Ok { value }",
            "PATTERN_PAYLOAD_FORM_MISMATCH",
        ),
        (
            "Res[i64, Str]",
            "Err { value }",
            "PATTERN_PAYLOAD_FORM_MISMATCH",
        ),
        ("Opt[i64]", "Some()", "PATTERN_ARITY_MISMATCH"),
        ("i64", "None", "PATTERN_TYPE_MISMATCH"),
        ("Res[i64, Str]", "Some(value)", "PATTERN_TYPE_MISMATCH"),
        ("i64", "Some { value }", "PATTERN_TYPE_MISMATCH"),
    ] {
        let source = format!("fn f(input: {ty}) -> i64 = match input {{ {pattern} => 0 }}\n");
        let mut session = session(&source);
        session
            .resolve(&branching_profile(), Some(&catalog))
            .unwrap();
        assert_eq!(
            diagnostic_codes(&session.into_diagnostics(None)),
            [expected],
            "{source}"
        );
    }
}

#[test]
fn residual_spelling_in_binding_roles_does_not_shadow_match_heads() {
    let catalog = TrustedCatalog::first_slice().unwrap();
    for source in [
        "fn f(None: i64, input: Opt[i64]) -> i64 = match input { None => None, Some(v) => v }\n",
        "fn f(Some: i64, input: Opt[i64]) -> i64 = match input { None => Some, Some(v) => v }\n",
        "fn f(Ok: i64, Err: i64, input: Res[i64, i64]) -> i64 = match input { Ok(v) => Ok, Err(e) => Err }\n",
        "fn f(input: Opt[i64]) -> i64 { let None = 3\nlet Some = 4\nyield match input { None => None, Some(v) => Some } }\n",
        "fn f(input: Opt[i64]) -> Opt[i64] = match input { None @ None => None, Some @ Some(_) => Some }\n",
        "fn f(None: i64) -> i64 = None\nfn g() -> i64 { let Ok = 4\nyield Ok }\n",
        "fn f(input: Opt[Opt[i64]]) -> i64 = match input { Some(None) => 1, Some(Some(v)) => v, None => 0, _ => 2 }\n",
        "fn f(input: (Opt[i64], i64)) -> i64 = match input { ((None), n) => n, (Some(v), _) => v, _ => 0 }\n",
    ] {
        let mut session = session(source);
        session
            .resolve(&branching_profile(), Some(&catalog))
            .unwrap();
        assert!(
            diagnostic_codes(&session.into_diagnostics(None)).is_empty(),
            "{source}"
        );
    }
}

#[test]
fn unavailable_residual_head_resolution_cannot_seal_a_checked_program() {
    let catalog = TrustedCatalog::first_slice().unwrap();
    let full = SupportProfile::semantic_core_v1();
    let removed = CompileCapability::Resolve(ResolveCapability::ResolveTrustedExternalNames);
    let reduced = SupportProfile::compile_only(
        full.schema(),
        SupportProfileId::new("residual-pattern-reduced-v1").unwrap(),
        full.compile_capabilities()
            .filter(|capability| *capability != removed),
    )
    .unwrap();
    let mut session =
        session("fn f(input: Opt[i64]) -> i64 = match input { None => 0, Some(v) => v }\n");
    let analysis = session.resolve(&reduced, Some(&catalog)).unwrap();
    assert!(
        analysis
            .trace()
            .missing()
            .any(|entry| entry.capability() == removed)
    );
    let sealed = session.seal(None).unwrap();
    assert!(sealed.checked_program().is_none());
    assert!(diagnostic_codes(sealed.diagnostics()).is_empty());
}

#[test]
fn a_resolved_residual_head_is_not_an_ownership_binding() {
    let catalog = TrustedCatalog::first_slice().unwrap();
    let mut session = session(concat!(
        "fn discharge(input: Opt[i64], root: Own[i64]) -> i64 = match input {\n",
        "  None => { drop_own move root\nyield 0 },\n",
        "  Some(value) => { drop_own move root\nyield value }\n",
        "}\n",
    ));
    let analysis = session
        .resolve(&SupportProfile::semantic_core_v1(), Some(&catalog))
        .unwrap();
    let resolution = analysis.resolution().unwrap();
    let PhaseProduct::Performed(facts) = resolution.ownership_facts() else {
        panic!("ownership analysis must run");
    };
    assert!(facts.is_complete());
    assert!(
        resolution
            .names()
            .locals()
            .iter()
            .all(|local| local.name().text() != "None")
    );
    assert!(analysis.trace().missing().next().is_none());
    let sealed = session.seal(None).unwrap();
    assert!(sealed.checked_program().is_some());
    assert!(diagnostic_codes(sealed.diagnostics()).is_empty());
}

#[test]
fn residual_heads_do_not_turn_an_unsupported_closure_into_a_checked_program() {
    let catalog = TrustedCatalog::first_slice().unwrap();
    let mut session = session(concat!(
        "fn f(None: i64, input: Opt[i64]) -> i64 {\n",
        "  let pick: Fn() -> i64 = {|| match input { None => None, Some(v) => v }}\n",
        "  yield pick()\n",
        "}\n",
    ));
    let analysis = session
        .resolve(&SupportProfile::semantic_core_v1(), Some(&catalog))
        .unwrap();
    assert!(analysis.trace().missing().any(|entry| {
        entry.capability() == CompileCapability::Typecheck(TypecheckCapability::CheckClosures)
    }));
    assert_eq!(
        analysis
            .resolution()
            .unwrap()
            .names()
            .locals()
            .iter()
            .filter(|local| local.name().text() == "None")
            .count(),
        1
    );
    let sealed = session.seal(None).unwrap();
    assert!(sealed.checked_program().is_none());
    assert!(diagnostic_codes(sealed.diagnostics()).is_empty());
}
