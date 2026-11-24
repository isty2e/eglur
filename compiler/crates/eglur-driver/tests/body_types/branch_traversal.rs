use super::{diagnostic_codes, session};
use eglur_driver::{
    CompilerSession, SupportProfile, SupportProfileId, SupportSchemaVersion, TrustedCatalog,
};
use eglur_sema::{CompileCapability, FactState, TypecheckCapability};

fn check_branches(mut session: CompilerSession) {
    let catalog = TrustedCatalog::first_slice().unwrap();
    let analysis = session
        .resolve(&SupportProfile::semantic_core_v1(), Some(&catalog))
        .unwrap();
    let missing = analysis.trace().missing().collect::<Vec<_>>();
    assert!(missing.is_empty(), "{missing:?}");
    assert!(analysis.fingerprints().is_some());

    let diagnostics = session.into_diagnostics(None);
    assert!(
        diagnostic_codes(&diagnostics).is_empty(),
        "{:?}",
        diagnostics.diagnostics()
    );
}

#[test]
fn guards_without_a_normal_result_remove_only_matched_inputs() {
    check_branches(session(concat!(
        "fn stop() -> Never = stop()\n",
        "fn test(_arg: i64) -> Bool = true\n",
        "fn n() -> i64 = 2\n",
        "fn all(x: Opt[i64]) -> Never = match x { _ if test(stop()) => n(), _ => n() }\n",
        "fn both(x: Opt[i64]) -> Never = match x {\n",
        "  Some(v) if test(stop()) => n(), None if test(stop()) => n(), _ => n() }\n",
        "fn some(x: Opt[i64]) -> i64 = match x {\n",
        "  Some(v) if test({ return 7 }) => n(), Some(v) => true, None => n() }\n",
        "fn fallback(x: Res[i64, Str]) -> i64 = match x {\n",
        "  Err(\"retry\") if test(stop()) => true, _ => n() }\n",
    )));
}

#[test]
fn stopping_in_a_guard_does_not_contribute_unconditional_coverage() {
    let catalog = TrustedCatalog::first_slice().unwrap();
    let mut session = session(concat!(
        "fn test(n: i64) -> Bool = true\n",
        "fn choose(x: Opt[i64]) -> i64 = match x { _ if test({ return 7 }) => 1 }\n",
    ));
    session
        .resolve(&SupportProfile::semantic_core_v1(), Some(&catalog))
        .unwrap();
    assert_eq!(
        diagnostic_codes(&session.into_diagnostics(None)),
        ["MATCH_NON_EXHAUSTIVE"]
    );
}

#[test]
fn invalid_written_guards_do_not_request_missing_carrier_coverage() {
    let inhabitance = CompileCapability::Typecheck(TypecheckCapability::AnalyzeInhabitance);
    let profile = SupportProfile::compile_only(
        SupportSchemaVersion::V1,
        SupportProfileId::new("without-inhabitance").unwrap(),
        SupportProfile::semantic_core_v1()
            .compile_capabilities()
            .filter(|&c| c != inhabitance),
    )
    .unwrap();
    let catalog = TrustedCatalog::first_slice().unwrap();
    for (guard, expected) in [("false", None), ("1", Some("MATCH_GUARD_TYPE_MISMATCH"))] {
        let source = format!(
            "fn f(x: Opt[Own[i64]]) -> i64 = match move x {{ None if {guard} => 0,\n\
             Some(v) => {{ drop_own move v\nyield 1 }}, None => 2 }}\n"
        );
        let mut session = session(&source);
        let analysis = session.resolve(&profile, Some(&catalog)).unwrap();
        assert_eq!(
            analysis
                .trace()
                .missing()
                .any(|d| d.capability() == inhabitance),
            expected.is_none(),
            "{guard}"
        );
        assert_eq!(
            diagnostic_codes(&session.into_diagnostics(None)),
            expected.into_iter().collect::<Vec<_>>()
        );
    }
}

#[test]
fn a_stopped_literal_guard_does_not_establish_a_later_result_type() {
    let catalog = TrustedCatalog::first_slice().unwrap();
    for literal in ["retry", "denied"] {
        let source = format!(
            "fn test(n: i64) -> Bool = true\nfn n() -> i64 = 1\n\
            fn choose(x: Res[i64, Str]) -> i64 = match x {{\n\
            Ok(v) => v, Err(\"retry\") if test({{ return 7 }}) => n(),\n\
            Err(\"{literal}\") => true, Err(_) => n() }}\n"
        );
        let mut session = session(&source);
        let analysis = session
            .resolve(&SupportProfile::semantic_core_v1(), Some(&catalog))
            .unwrap();
        assert!(analysis.trace().missing().any(|d| d.capability()
            == CompileCapability::Typecheck(TypecheckCapability::AnalyzePatternCoverage)));
        assert_eq!(
            diagnostic_codes(&session.into_diagnostics(None)),
            Vec::<&str>::new()
        );
    }
}

#[test]
fn nested_if_branches_seal_on_the_default_stack() {
    let depth = 240;
    let source = format!(
        "fn f(flag: Bool) -> i64 = {}1{}\n",
        "if flag { yield ".repeat(depth),
        " } else { yield 0 }".repeat(depth),
    );
    check_branches(session(&source));
}

#[test]
fn nested_if_conditions_seal_on_the_default_stack() {
    let depth = 100;
    let source = format!(
        "fn id[T](x: T) -> T = x\nfn f(flag: Bool) -> Bool = {}flag{}\n",
        "if id(".repeat(depth),
        ") { yield true } else { yield false }".repeat(depth),
    );
    check_branches(session(&source));
}

#[test]
fn nested_match_arms_seal_on_the_default_stack() {
    let depth = 100;
    let source = format!(
        "fn f(x: i64) -> i64 = {}x{}\n",
        "match x { value => { yield ".repeat(depth),
        " } }".repeat(depth),
    );
    check_branches(session(&source));
}

#[test]
fn nested_match_scrutinees_seal_on_the_default_stack() {
    let depth = 100;
    let source = format!(
        "fn id[T](x: T) -> T = x\nfn f(x: i64) -> i64 = {}x{}\n",
        "match id(".repeat(depth),
        ") { value => value }".repeat(depth),
    );
    check_branches(session(&source));
}

#[test]
fn nested_match_guards_seal_on_the_default_stack() {
    let depth = 60;
    let source = format!(
        "fn id[T](x: T) -> T = x\nfn f(flag: Bool) -> Bool = {}flag{}\n",
        "match flag { value if id(".repeat(depth),
        ") => value, _ => false }".repeat(depth),
    );
    check_branches(session(&source));
}

#[test]
fn nested_empty_match_scrutinees_seal_on_the_default_stack() {
    let depth = 100;
    let source = format!(
        "fn id[T](x: T) -> T = x\nfn f(x: Never) -> Never = {}x{}\n",
        "match id(".repeat(depth),
        ") {}".repeat(depth),
    );
    check_branches(session(&source));
}

#[test]
fn branches_calls_constructors_and_projections_share_the_work_stack() {
    let depth = 40;
    let source = format!(
        "struct Wrap[T] {{ field value: T }}\nfn id[T](value: T) -> T = value\nfn f(flag: Bool, x: i64) -> i64 = {}x{}\n",
        "id((if flag { yield match x { value => Wrap { value: ".repeat(depth),
        " }.value } } else { yield x },).0)".repeat(depth),
    );
    check_branches(session(&source));
}

#[test]
fn deep_branch_failure_preserves_one_diagnostic() {
    let depth = 60;
    let source = format!(
        "fn f(flag: Bool, x: i64) -> i64 = {}x.0{}\n",
        "if flag { yield match x { _ => ".repeat(depth),
        " } } else { yield x }".repeat(depth),
    );
    let catalog = TrustedCatalog::first_slice().unwrap();
    let mut session = session(&source);
    session
        .resolve(&SupportProfile::semantic_core_v1(), Some(&catalog))
        .unwrap();
    assert_eq!(
        diagnostic_codes(&session.into_diagnostics(None)),
        ["TUPLE_PROJECTION_NON_TUPLE"]
    );
}

#[test]
fn deep_unsupported_guard_stays_unperformed() {
    let depth = 60;
    let source = format!(
        "fn id[T](x: T) -> T = x\nfn f(flag: Bool) -> Bool = {}[true]{}\n",
        "match flag { _ if id(".repeat(depth),
        ") => true, _ => false }".repeat(depth),
    );
    let catalog = TrustedCatalog::first_slice().unwrap();
    let mut session = session(&source);
    let analysis = session
        .resolve(&SupportProfile::semantic_core_v1(), Some(&catalog))
        .unwrap();
    assert!(analysis.trace().missing().any(|entry| {
        entry.capability() == CompileCapability::Typecheck(TypecheckCapability::CheckContainers)
    }));
    let resolution = analysis.resolution().unwrap();
    let function = resolution
        .declarations()
        .modules()
        .flat_map(|module| module.declarations())
        .find(|declaration| {
            declaration
                .name()
                .is_some_and(|name| name.spelling() == "f")
        })
        .unwrap();
    assert!(matches!(
        resolution.body_types().body(function.id()),
        FactState::Unperformed
    ));
    assert!(diagnostic_codes(&session.into_diagnostics(None)).is_empty());
}

#[test]
fn poisoned_branches_are_retained_after_unsupported_conditions_and_guards() {
    let catalog = TrustedCatalog::first_slice().unwrap();
    for source in [
        "fn f(x: i64) -> i64 = if [true] { yield x.0 } else { yield x.1 }\n",
        "fn f(x: i64) -> i64 = match x { _ if [true] => x.0, _ => x.1 }\n",
    ] {
        let mut session = session(source);
        let analysis = session
            .resolve(&SupportProfile::semantic_core_v1(), Some(&catalog))
            .unwrap();
        assert!(analysis.trace().missing().any(|entry| {
            entry.capability() == CompileCapability::Typecheck(TypecheckCapability::CheckContainers)
        }));
        let resolution = analysis.resolution().unwrap();
        let function = resolution
            .declarations()
            .modules()
            .flat_map(|module| module.declarations())
            .find(|declaration| {
                declaration
                    .name()
                    .is_some_and(|name| name.spelling() == "f")
            })
            .unwrap();
        assert!(matches!(
            resolution.body_types().body(function.id()),
            FactState::Poisoned(_)
        ));
        assert_eq!(
            diagnostic_codes(&session.into_diagnostics(None)),
            ["TUPLE_PROJECTION_NON_TUPLE", "TUPLE_PROJECTION_NON_TUPLE"]
        );
    }
}

#[test]
fn nested_control_barriers_preserve_local_yields() {
    use eglur_sema::{ControlRegionKey, ControlTransferKind};

    for absent in [false, true] {
        for depth in [1, 8, 32] {
            let condition = if absent { "as_bool(stop())" } else { "flag" };
            let mut value = "()".to_owned();
            for _ in 0..depth {
                value = format!("if {condition} {{ yield {value} }} else {{ yield () }}");
            }
            let source = format!(
                "fn stop() -> Never = stop()\n\
                 fn as_bool(value: Bool) -> Bool = value\n\
                 fn inspect(flag: Bool) -> Unit = {value}\n"
            );
            let mut session = session(&source);
            let catalog = TrustedCatalog::first_slice().unwrap();
            let analysis = session
                .resolve(&SupportProfile::semantic_core_v1(), Some(&catalog))
                .unwrap();
            assert!(analysis.trace().missing().next().is_none());
            assert!(analysis.fingerprints().is_some());
            let yields = analysis
                .resolution()
                .unwrap()
                .body_types()
                .transfers()
                .iter()
                .filter(|transfer| transfer.kind() == ControlTransferKind::Yield)
                .collect::<Vec<_>>();
            assert_eq!(yields.len(), if absent { depth + 1 } else { 2 * depth });
            for transfer in yields {
                assert!(matches!(
                    transfer.target(),
                    ControlRegionKey::IfBranch { .. }
                ));
                assert_eq!(transfer.exited_regions().len(), 1);
            }
            let diagnostics = session.into_diagnostics(None);
            assert!(
                diagnostic_codes(&diagnostics).is_empty(),
                "{source}\n{diagnostics:?}"
            );
        }
    }
}

#[test]
fn nested_control_barriers_suppress_only_escaping_questions() {
    use eglur_sema::{ControlRegionKey, ControlTransferKind};

    for absent in [false, true] {
        let condition = if absent { "as_bool(stop())" } else { "flag" };
        let source = format!(
            "fn stop() -> Never = stop()\n\
             fn as_bool(value: Bool) -> Bool = value\n\
             fn inspect(x: Opt[i32], flag: Bool) -> Opt[i32] =\n\
             if {condition} {{\n\
               let value: Opt[i32] = result {{\n\
                 yield if {condition} {{ let item = x?\nyield x }} else {{ yield x }}\n\
               }}\n\
               yield value\n\
             }} else {{ yield x }}\n"
        );
        let mut session = session(&source);
        let catalog = TrustedCatalog::first_slice().unwrap();
        let analysis = session
            .resolve(&SupportProfile::semantic_core_v1(), Some(&catalog))
            .unwrap();
        assert!(
            analysis.trace().missing().next().is_none(),
            "{source}\n{:?}",
            analysis.trace().missing().collect::<Vec<_>>()
        );
        assert!(analysis.fingerprints().is_some());
        let questions = analysis
            .resolution()
            .unwrap()
            .body_types()
            .transfers()
            .iter()
            .filter(|transfer| transfer.kind() == ControlTransferKind::QuestionShortCircuit)
            .collect::<Vec<_>>();
        assert_eq!(questions.len(), usize::from(!absent));
        for transfer in questions {
            assert!(matches!(transfer.target(), ControlRegionKey::Result(_)));
        }
        let diagnostics = session.into_diagnostics(None);
        assert!(
            diagnostic_codes(&diagnostics).is_empty(),
            "{source}\n{diagnostics:?}"
        );
    }
}
