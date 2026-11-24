use eglur_diagnostics::DiagnosticSnapshot;
use eglur_driver::{
    CompilerSession, FileSetRequest, Phase, SessionOptions, SupportProfile, SupportProfileId,
    SupportSchemaVersion, TrustedCatalog,
};
use eglur_sema::{
    CompileCapability, OwnershipBodyStatus, PhaseProduct, ResolveCapability, TypecheckCapability,
    TypedOwnershipOperationKind,
};

#[path = "ownership/traversal.rs"]
mod traversal;

#[path = "ownership/unsupported.rs"]
mod unsupported;

#[path = "ownership/projection.rs"]
mod projection;

#[path = "ownership/exit_regions.rs"]
mod exit_regions;

#[path = "ownership/carrier_guards.rs"]
mod carrier_guards;

#[path = "ownership/coverage.rs"]
mod coverage;

#[path = "ownership/match_arms.rs"]
mod match_arms;

fn body_capabilities() -> [CompileCapability; 14] {
    [
        CompileCapability::Resolve(ResolveCapability::CollectDeclarations),
        CompileCapability::Resolve(ResolveCapability::ResolveFileSetImports),
        CompileCapability::Resolve(ResolveCapability::ResolveLexicalValues),
        CompileCapability::Resolve(ResolveCapability::ResolveModuleValues),
        CompileCapability::Resolve(ResolveCapability::ResolveQualifiedValues),
        CompileCapability::Resolve(ResolveCapability::ResolvePatternScopes),
        CompileCapability::Resolve(ResolveCapability::ResolveTrustedExternalNames),
        CompileCapability::Typecheck(TypecheckCapability::ResolveDeclarationSignatures),
        CompileCapability::Typecheck(TypecheckCapability::ResolveTraitEnvironment),
        CompileCapability::Typecheck(TypecheckCapability::CheckBodies),
        CompileCapability::Typecheck(TypecheckCapability::ResolveMembers),
        CompileCapability::Typecheck(TypecheckCapability::AnalyzeInhabitance),
        CompileCapability::Typecheck(TypecheckCapability::CheckFailureFlow),
        CompileCapability::Typecheck(TypecheckCapability::CheckSelectedCheckedIntegerArithmetic),
    ]
}

fn body_profile() -> SupportProfile {
    SupportProfile::compile_only(
        SupportSchemaVersion::V1,
        SupportProfileId::new("ownership-typing-v1").unwrap(),
        body_capabilities(),
    )
    .unwrap()
}

fn ownership_profile() -> SupportProfile {
    SupportProfile::compile_only(
        SupportSchemaVersion::V1,
        SupportProfileId::new("ownership-flow-v1").unwrap(),
        body_capabilities().into_iter().chain([
            CompileCapability::Typecheck(TypecheckCapability::CheckBranching),
            CompileCapability::Typecheck(TypecheckCapability::CheckOwnershipFlow),
        ]),
    )
    .unwrap()
}

fn session(source: &str) -> CompilerSession {
    let request = FileSetRequest::single("main.eglur", source.as_bytes().to_vec()).unwrap();
    CompilerSession::build(request, SessionOptions::default()).unwrap()
}

fn diagnostic_codes(snapshot: &DiagnosticSnapshot) -> Vec<&str> {
    snapshot
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic.code().as_str())
        .collect()
}

#[test]
fn body_typing_records_stable_ownership_operations_without_owning_flow_state() {
    let mut session = session(concat!(
        "fn transfer(x: Own[i64]) -> i64 {\n",
        "  let y = move x\n",
        "  drop_own move y\n",
        "  return 0\n",
        "}\n",
        "fn create(n: i64) -> i64 {\n",
        "  let root = own n\n",
        "  drop_own move root\n",
        "  return 0\n",
        "}\n",
    ));
    let catalog = TrustedCatalog::first_slice().unwrap();

    let analysis = session.resolve(&body_profile(), Some(&catalog)).unwrap();
    let resolution = analysis.resolution().expect("body typing is selected");
    let operations = resolution
        .body_types()
        .ownership_operations()
        .map(|(_, operation)| operation.kind())
        .collect::<Vec<_>>();

    assert_eq!(
        operations,
        [
            TypedOwnershipOperationKind::Move,
            TypedOwnershipOperationKind::DropOwn,
            TypedOwnershipOperationKind::Own,
            TypedOwnershipOperationKind::DropOwn,
        ]
    );
    let snapshot = session.into_diagnostics(None);
    assert!(
        diagnostic_codes(&snapshot).is_empty(),
        "unexpected diagnostics: {:?}",
        diagnostic_codes(&snapshot)
    );
}

#[test]
fn ownership_operator_static_failures_keep_their_specific_codes() {
    let cases = [
        (
            "fn bad(x: i64) -> i64 {\n  let y = move x\n  return 0\n}\n",
            "MOVE_REQUIRES_OWN",
        ),
        (
            "fn bad(x: Own[i64]) -> i64 {\n  let y = own x\n  return 0\n}\n",
            "OWN_REQUIRES_NON_OWN",
        ),
        (
            "fn bad(x: Own[i64]) -> i64 {\n  drop_own x\n  return 0\n}\n",
            "DROP_OWN_REQUIRES_MOVE",
        ),
        (
            "const N: i64 = 1\nfn bad() -> i64 {\n  let y = own N\n  return 0\n}\n",
            "OWN_REQUIRES_LOCAL_BINDING",
        ),
    ];
    let catalog = TrustedCatalog::first_slice().unwrap();

    for (source, expected) in cases {
        let mut session = session(source);
        session.resolve(&body_profile(), Some(&catalog)).unwrap();
        assert_eq!(
            diagnostic_codes(&session.into_diagnostics(None)),
            [expected],
            "source:\n{source}"
        );
    }
}

#[test]
fn unresolved_ownership_operand_does_not_add_a_cascade_diagnostic() {
    let mut session = session("fn bad() -> i64 {\n  let y = own missing\n  return 0\n}\n");
    let catalog = TrustedCatalog::first_slice().unwrap();

    session.resolve(&body_profile(), Some(&catalog)).unwrap();

    assert_eq!(
        diagnostic_codes(&session.into_diagnostics(None)),
        ["NAME_NOT_FOUND"]
    );
}

#[test]
fn balanced_ownership_discharge_across_return_paths_is_checked() {
    let mut session = session(concat!(
        "fn good_paths(x: Own[i64], keep: Bool) -> i64 {\n",
        "  if keep {\n",
        "    drop_own move x\n",
        "    return 1\n",
        "  } else {\n",
        "    drop_own move x\n",
        "    return 0\n",
        "  }\n",
        "}\n",
    ));
    let catalog = TrustedCatalog::first_slice().unwrap();

    let analysis = session
        .resolve(&ownership_profile(), Some(&catalog))
        .unwrap();
    let resolution = analysis
        .resolution()
        .expect("ownership analysis is selected");
    let PhaseProduct::Performed(facts) = resolution.ownership_facts() else {
        panic!("ownership facts were not performed");
    };
    let (_, body) = facts
        .bodies()
        .find_map(|(owner, fact)| match fact {
            eglur_sema::FactState::Known(body) => Some((owner, body)),
            _ => None,
        })
        .expect("checked callable ownership fact is present");

    assert_eq!(body.status(), OwnershipBodyStatus::Checked);
    assert_eq!(body.roots().len(), 1);
    assert_eq!(body.operation_count(), 2);
    let snapshot = session.into_diagnostics(None);
    assert!(
        diagnostic_codes(&snapshot).is_empty(),
        "unexpected diagnostics: {:?}",
        diagnostic_codes(&snapshot)
    );
}

#[test]
fn early_return_state_does_not_pollute_the_continuing_branch() {
    let mut session = session(concat!(
        "fn good_paths(x: Own[i64], stop: Bool) -> i64 {\n",
        "  if stop {\n",
        "    drop_own move x\n",
        "    return 1\n",
        "  } else {\n",
        "  }\n",
        "  drop_own move x\n",
        "  return 0\n",
        "}\n",
    ));
    let catalog = TrustedCatalog::first_slice().unwrap();

    let analysis = session
        .resolve(&ownership_profile(), Some(&catalog))
        .unwrap();

    assert!(matches!(
        analysis
            .resolution()
            .expect("ownership analysis is selected")
            .ownership_facts(),
        PhaseProduct::Performed(_)
    ));
    assert!(diagnostic_codes(&session.into_diagnostics(None)).is_empty());
}

#[test]
fn repeated_move_reports_one_error_at_the_later_consume() {
    let source = concat!(
        "fn bad_double_move(x: Own[i64]) -> i64 {\n",
        "  let y = move x\n",
        "  drop_own move y\n",
        "  let z = move x\n",
        "  drop_own move z\n",
        "  return 0\n",
        "}\n",
    );
    let mut session = session(source);
    let catalog = TrustedCatalog::first_slice().unwrap();

    session
        .resolve(&ownership_profile(), Some(&catalog))
        .unwrap();
    let snapshot = session.into_diagnostics(None);
    let diagnostics = snapshot.diagnostics();

    assert_eq!(diagnostic_codes(&snapshot), ["OWNERSHIP_DOUBLE_MOVE"]);
    let primary = diagnostics[0]
        .primary_span()
        .as_source()
        .expect("ownership error has a source primary");
    let repeated = source.rfind("move x").unwrap();
    let range = primary.file_span().range();
    assert!(range.start().get() as usize <= repeated);
    assert!(range.end().get() as usize >= repeated + "move x".len());
    assert_eq!(diagnostics[0].related().len(), 1);
}

#[test]
fn branch_merge_rejects_use_of_a_maybe_consumed_root() {
    let source = concat!(
        "fn peek(_x: &Own[i64]) -> i64 { return 1 }\n",
        "fn bad_maybe_consumed(x: Own[i64], b: Bool) -> i64 {\n",
        "  if b {\n",
        "    drop_own move x\n",
        "  } else {\n",
        "  }\n",
        "  let n = peek(&x)\n",
        "  if b {\n",
        "    return n\n",
        "  } else {\n",
        "    drop_own move x\n",
        "    return n\n",
        "  }\n",
        "}\n",
    );
    let mut session = session(source);
    let catalog = TrustedCatalog::first_slice().unwrap();

    session
        .resolve(&ownership_profile(), Some(&catalog))
        .unwrap();
    let snapshot = session.into_diagnostics(None);

    assert_eq!(
        diagnostic_codes(&snapshot),
        ["OWNERSHIP_USE_AFTER_DISCHARGE"]
    );
    let primary = snapshot.diagnostics()[0]
        .primary_span()
        .as_source()
        .expect("ownership error has a source primary");
    let borrow = source.rfind("&x").unwrap() + 1;
    let range = primary.file_span().range();
    assert!(
        range.start().get() as usize <= borrow,
        "primary range {range:?} does not start before borrow at {borrow}"
    );
    assert!(
        range.end().get() as usize > borrow,
        "primary range {range:?} does not contain borrow at {borrow}"
    );
}

#[test]
fn yielded_reference_provenance_survives_block_and_if_merges() {
    let source = concat!(
        "fn peek(_x: &Own[i64]) -> i64 { return 1 }\n",
        "fn bad_reference_merge(x: Own[i64], y: Own[i64], choose_x: Bool) -> i64 {\n",
        "  let reference = if choose_x {\n",
        "    yield &x\n",
        "  } else {\n",
        "    yield &y\n",
        "  }\n",
        "  drop_own move x\n",
        "  let value = peek(reference)\n",
        "  drop_own move y\n",
        "  return value\n",
        "}\n",
    );
    let mut session = session(source);
    let catalog = TrustedCatalog::first_slice().unwrap();

    session
        .resolve(&ownership_profile(), Some(&catalog))
        .unwrap();
    let snapshot = session.into_diagnostics(None);

    assert_eq!(
        diagnostic_codes(&snapshot),
        ["OWNERSHIP_USE_AFTER_DISCHARGE"]
    );
    assert!(snapshot.diagnostics()[0].message().contains("`x`"));
    let primary = snapshot.diagnostics()[0]
        .primary_span()
        .as_source()
        .expect("ownership error has a source primary")
        .file_span()
        .range();
    let use_site = source.find("peek(reference)").unwrap() + "peek(".len();
    assert!(primary.start().get() as usize <= use_site);
    assert!(primary.end().get() as usize > use_site);
}

#[test]
fn own_consumes_its_ordinary_source_for_later_uses() {
    let mut session = session(concat!(
        "fn bad(value: i64) -> i64 {\n",
        "  let root = own value\n",
        "  let copied = value\n",
        "  drop_own move root\n",
        "  return copied\n",
        "}\n",
    ));
    let catalog = TrustedCatalog::first_slice().unwrap();

    session
        .resolve(&ownership_profile(), Some(&catalog))
        .unwrap();
    let snapshot = session.into_diagnostics(None);

    assert_eq!(
        diagnostic_codes(&snapshot),
        ["OWNERSHIP_USE_AFTER_DISCHARGE"]
    );
    assert!(snapshot.diagnostics()[0].message().contains("`value`"));
}

#[test]
fn no_normal_continuation_path_requires_no_root_discharge() {
    let mut session = session(concat!(
        "import std::test::abort!\n",
        "fn consume_or_abort!(root: Own[i64], consume: Bool) -> Unit {\n",
        "  if consume {\n",
        "    drop_own move root\n",
        "  } else {\n",
        "    abort!(\"RUNTIME_ABORT\", \"not executed\")\n",
        "  }\n",
        "}\n",
    ));
    let catalog = TrustedCatalog::first_slice().unwrap();

    let analysis = session
        .resolve(&ownership_profile(), Some(&catalog))
        .unwrap();

    assert!(matches!(
        analysis
            .resolution()
            .expect("ownership analysis is selected")
            .ownership_facts(),
        PhaseProduct::Performed(_)
    ));
    assert!(diagnostic_codes(&session.into_diagnostics(None)).is_empty());
}

#[test]
fn absent_control_paths_do_not_create_ownership_flow_failures() {
    let cases = [
        concat!(
            "fn good(x: Own[i64]) -> i64 {\n",
            "  drop_own move x\n",
            "  return 0\n",
            "  let unreachable = move x\n",
            "  drop_own move unreachable\n",
            "}\n",
        ),
        concat!(
            "import std::test::abort!\n",
            "fn good!(x: Own[i64]) -> i64 {\n",
            "  return abort!(\"RUNTIME_ABORT\", \"not executed\")\n",
            "}\n",
        ),
        concat!(
            "import std::test::abort!\n",
            "fn good!(x: Own[i64]) -> Unit {\n",
            "  abort!(\"RUNTIME_ABORT\", \"not executed\")\n",
            "  let first = move x\n",
            "  let second = move x\n",
            "}\n",
        ),
    ];
    let catalog = TrustedCatalog::first_slice().unwrap();

    for source in cases {
        let mut session = session(source);
        session
            .resolve(&ownership_profile(), Some(&catalog))
            .unwrap();
        let snapshot = session.into_diagnostics(None);
        assert!(
            diagnostic_codes(&snapshot).is_empty(),
            "source:\n{source}\ndiagnostics: {:?}",
            diagnostic_codes(&snapshot)
        );
    }
}

#[test]
fn question_failure_edge_checks_live_ownership_obligations() {
    let source = concat!(
        "fn source() -> Res[i64, Str] = Err(\"failed\")\n",
        "fn bad(root: Own[i64]) -> Res[i64, Str] = result {\n",
        "  let value = source()?\n",
        "  drop_own move root\n",
        "  yield Ok(value)\n",
        "}\n",
    );
    let mut session = session(source);
    let catalog = TrustedCatalog::first_slice().unwrap();

    session
        .resolve(&ownership_profile(), Some(&catalog))
        .unwrap();
    let snapshot = session.into_diagnostics(None);

    assert_eq!(diagnostic_codes(&snapshot), ["OWNERSHIP_PATH_UNBALANCED"]);
    let primary = snapshot.diagnostics()[0]
        .primary_span()
        .as_source()
        .expect("ownership error has a source primary")
        .file_span()
        .range();
    let question = source.find("source()?").unwrap() + "source()".len();
    assert!(primary.start().get() as usize <= question);
    assert!(primary.end().get() as usize > question);
}

#[test]
fn recoverable_region_fault_edges_check_inner_roots() {
    let cases = [
        concat!(
            "fn bad(source: Res[i64, Str]) -> Res[i64, Str] = result {\n",
            "  let owned_value = 7\n",
            "  let root = own owned_value\n",
            "  let value = source?\n",
            "  drop_own move root\n",
            "  yield Ok(value)\n",
            "}\n",
        ),
        concat!(
            "fn bad(value: i8) -> Res[i8, NumericError] = checked {\n",
            "  let owned_value = 7\n",
            "  let root = own owned_value\n",
            "  let doubled = value + value\n",
            "  drop_own move root\n",
            "  yield doubled\n",
            "}\n",
        ),
    ];
    let catalog = TrustedCatalog::first_slice().unwrap();

    for source in cases {
        let mut session = session(source);
        session
            .resolve(&ownership_profile(), Some(&catalog))
            .unwrap();
        assert_eq!(
            diagnostic_codes(&session.into_diagnostics(None)),
            ["OWNERSHIP_PATH_UNBALANCED"],
            "source:\n{source}"
        );
    }
}

#[test]
fn expression_body_completion_checks_callable_obligations_and_is_observed() {
    let source = "fn bad(x: Own[i64]) -> i64 = 0\n";
    let request = FileSetRequest::single("main.eglur", source.as_bytes().to_vec()).unwrap();
    let mut session = CompilerSession::build(request, SessionOptions::new(true)).unwrap();
    let catalog = TrustedCatalog::first_slice().unwrap();

    session
        .resolve(&ownership_profile(), Some(&catalog))
        .unwrap();

    assert!(
        session
            .observations()
            .iter()
            .any(|observation| observation.phase() == Phase::OwnershipAnalysis)
    );
    assert_eq!(
        diagnostic_codes(&session.into_diagnostics(None)),
        ["OWNERSHIP_PATH_UNBALANCED"]
    );
}

#[test]
fn expression_body_can_transfer_its_root_result() {
    let mut session = session("fn pass(x: Own[i64]) -> Own[i64] = move x\n");
    let catalog = TrustedCatalog::first_slice().unwrap();

    session
        .resolve(&ownership_profile(), Some(&catalog))
        .unwrap();

    assert!(diagnostic_codes(&session.into_diagnostics(None)).is_empty());
}

#[test]
fn reference_argument_remains_live_through_the_invocation() {
    let mut session = session(concat!(
        "fn consume_with_view(_view: &Own[i64], owned: Own[i64]) -> Unit {\n",
        "  drop_own move owned\n",
        "}\n",
        "fn bad(x: Own[i64]) -> Unit {\n",
        "  consume_with_view(&x, move x)\n",
        "}\n",
    ));
    let catalog = TrustedCatalog::first_slice().unwrap();

    session
        .resolve(&ownership_profile(), Some(&catalog))
        .unwrap();

    assert_eq!(
        diagnostic_codes(&session.into_diagnostics(None)),
        ["OWNERSHIP_USE_AFTER_DISCHARGE"]
    );
}

#[test]
fn tuple_projection_preserves_reference_provenance() {
    let mut session = session(concat!(
        "fn peek(_view: &i64) -> i64 = 0\n",
        "fn bad(value: i64) -> i64 {\n",
        "  let pair = (&value, 0)\n",
        "  let root = own value\n",
        "  let n = peek(pair.0)\n",
        "  drop_own move root\n",
        "  return n\n",
        "}\n",
    ));
    let catalog = TrustedCatalog::first_slice().unwrap();

    session
        .resolve(&ownership_profile(), Some(&catalog))
        .unwrap();

    assert_eq!(
        diagnostic_codes(&session.into_diagnostics(None)),
        ["OWNERSHIP_USE_AFTER_DISCHARGE"]
    );
}

#[test]
fn tuple_projection_uses_only_the_selected_reference_provenance() {
    let mut session = session(concat!(
        "fn peek(_view: &i64) -> i64 = 0\n",
        "fn good(value: i64, other: i64) -> i64 {\n",
        "  let pair = (&value, other)\n",
        "  let root = own value\n",
        "  let n = peek(&pair.1)\n",
        "  drop_own move root\n",
        "  return n\n",
        "}\n",
    ));
    let catalog = TrustedCatalog::first_slice().unwrap();

    session
        .resolve(&ownership_profile(), Some(&catalog))
        .unwrap();

    let snapshot = session.into_diagnostics(None);
    assert!(
        diagnostic_codes(&snapshot).is_empty(),
        "unexpected diagnostics: {:?}",
        diagnostic_codes(&snapshot)
    );
}

#[test]
fn repeated_own_reports_use_after_discharge() {
    let mut session = session(concat!(
        "fn bad(value: i64) -> Unit {\n",
        "  let first = own value\n",
        "  let second = own value\n",
        "  drop_own move first\n",
        "  drop_own move second\n",
        "}\n",
    ));
    let catalog = TrustedCatalog::first_slice().unwrap();

    session
        .resolve(&ownership_profile(), Some(&catalog))
        .unwrap();

    assert_eq!(
        diagnostic_codes(&session.into_diagnostics(None)),
        ["OWNERSHIP_USE_AFTER_DISCHARGE"]
    );
}

#[test]
fn normal_completion_blames_the_closing_delimiter() {
    let source = "fn bad(x: Own[i64]) -> Unit {}\n";
    let mut session = session(source);
    let catalog = TrustedCatalog::first_slice().unwrap();

    session
        .resolve(&ownership_profile(), Some(&catalog))
        .unwrap();
    let snapshot = session.into_diagnostics(None);

    assert_eq!(diagnostic_codes(&snapshot), ["OWNERSHIP_PATH_UNBALANCED"]);
    let range = snapshot.diagnostics()[0]
        .primary_span()
        .as_source()
        .expect("ownership error has a source primary")
        .file_span()
        .range();
    let closing = source.rfind('}').unwrap();
    assert_eq!(range.start().as_usize(), closing);
    assert_eq!(range.end().as_usize(), closing + 1);
}

#[test]
fn named_root_transfers_without_move_report_only_the_transfer_error() {
    let cases = [
        concat!(
            "fn take(x: Own[i64]) -> i64 {\n",
            "  drop_own move x\n",
            "  return 1\n",
            "}\n",
            "fn bad(x: Own[i64]) -> i64 { return take(x) }\n",
        ),
        concat!(
            "fn bad(x: Own[i64]) -> i64 {\n",
            "  let y = x\n",
            "  drop_own move y\n",
            "  return 0\n",
            "}\n",
        ),
        "fn bad(x: Own[i64]) -> Own[i64] { return x }\n",
        "fn bad(x: Own[i64]) -> Opt[Own[i64]] { return Some(x) }\n",
        concat!(
            "fn bad(r: Res[Own[i64], Str]) -> Res[i64, Str] = result {\n",
            "  let x = r?\n",
            "  drop_own move x\n",
            "  yield Ok(1)\n",
            "}\n",
        ),
    ];
    let catalog = TrustedCatalog::first_slice().unwrap();

    for source in cases {
        let mut session = session(source);
        session
            .resolve(&ownership_profile(), Some(&catalog))
            .unwrap();
        assert_eq!(
            diagnostic_codes(&session.into_diagnostics(None)),
            ["OWNERSHIP_TRANSFER_REQUIRES_MOVE"],
            "source:\n{source}"
        );
    }
}

#[test]
fn ownership_diagnostic_selection_emits_only_the_earliest_failure_per_callable() {
    let source = concat!(
        "fn bad(x: Own[i64], y: Own[i64]) -> i64 {\n",
        "  let first_x = move x\n",
        "  drop_own move first_x\n",
        "  let second_x = move x\n",
        "  drop_own move second_x\n",
        "  let first_y = move y\n",
        "  drop_own move first_y\n",
        "  let second_y = move y\n",
        "  drop_own move second_y\n",
        "  return 0\n",
        "}\n",
    );
    let mut session = session(source);
    let catalog = TrustedCatalog::first_slice().unwrap();

    session
        .resolve(&ownership_profile(), Some(&catalog))
        .unwrap();
    let snapshot = session.into_diagnostics(None);

    assert_eq!(diagnostic_codes(&snapshot), ["OWNERSHIP_DOUBLE_MOVE"]);
    let primary = snapshot.diagnostics()[0]
        .primary_span()
        .as_source()
        .expect("ownership error has a source primary")
        .file_span()
        .range();
    let expected = source.find("let second_x = move x").unwrap();
    assert!(primary.start().get() as usize >= expected);
    assert!(primary.end().get() as usize <= source.find("let first_y").unwrap());
}

#[test]
fn ownership_phase_is_unperformed_when_the_capability_is_not_selected() {
    let mut session = session(concat!(
        "fn balanced(x: Own[i64]) -> i64 {\n",
        "  drop_own move x\n",
        "  return 0\n",
        "}\n",
    ));
    let catalog = TrustedCatalog::first_slice().unwrap();

    let analysis = session.resolve(&body_profile(), Some(&catalog)).unwrap();
    let resolution = analysis.resolution().expect("body typing is selected");

    assert!(matches!(
        resolution.ownership_facts(),
        PhaseProduct::Unperformed
    ));
    assert!(diagnostic_codes(&session.into_diagnostics(None)).is_empty());
}

#[test]
fn checked_match_keeps_an_independent_ownership_error() {
    let mut session = session(concat!(
        "fn deferred(x: Own[i64], input: Opt[i64]) -> i64 = match input {\n",
        "  None => {\n",
        "    drop_own move x\n",
        "    yield 1\n",
        "  },\n",
        "  Some(_) => {\n",
        "    drop_own move x\n",
        "    yield 0\n",
        "  },\n",
        "}\n",
        "fn bad(x: Own[i64]) -> i64 { return 0 }\n",
    ));
    let catalog = TrustedCatalog::first_slice().unwrap();

    let analysis = session
        .resolve(&ownership_profile(), Some(&catalog))
        .unwrap();
    let resolution = analysis.resolution().expect("body typing is selected");

    let PhaseProduct::Performed(facts) = resolution.ownership_facts() else {
        panic!("checked ownership facts must remain observable");
    };
    assert!(facts.is_complete());
    assert!(!analysis.trace().missing().any(|entry| {
        entry.capability()
            == CompileCapability::Typecheck(TypecheckCapability::CheckCompleteOwnershipFlow)
    }));
    assert_eq!(
        diagnostic_codes(&session.into_diagnostics(None)),
        ["OWNERSHIP_PATH_UNBALANCED"]
    );
}

#[test]
fn match_can_return_a_fresh_ownership_value_from_either_arm() {
    let mut session = session(concat!(
        "fn make(n: i64) -> Own[i64] { return own n }\n",
        "fn choose(input: Opt[i64]) -> Own[i64] = match input {\n",
        "  None => make(1),\n",
        "  Some(value) => make(value),\n",
        "}\n",
    ));
    let catalog = TrustedCatalog::first_slice().unwrap();

    let analysis = session
        .resolve(&ownership_profile(), Some(&catalog))
        .unwrap();
    let resolution = analysis.resolution().expect("ownership typing is selected");
    let PhaseProduct::Performed(facts) = resolution.ownership_facts() else {
        panic!("ownership facts must remain observable");
    };

    assert!(facts.is_complete());
    assert!(!analysis.trace().missing().any(|entry| {
        entry.capability()
            == CompileCapability::Typecheck(TypecheckCapability::CheckCompleteOwnershipFlow)
    }));
    assert!(diagnostic_codes(&session.into_diagnostics(None)).is_empty());
}

#[test]
fn omitted_resource_metadata_stays_an_explicit_catalog_capability_gap() {
    let mut session = session(concat!(
        "import std::io::ByteReader\n",
        "fn bad(reader: Own[ByteReader]) -> Unit {\n",
        "  drop_own move reader\n",
        "}\n",
    ));
    let catalog = TrustedCatalog::first_slice().unwrap();

    let analysis = session
        .resolve(&ownership_profile(), Some(&catalog))
        .unwrap();

    let PhaseProduct::Performed(facts) = analysis
        .resolution()
        .expect("partial resolution remains observable")
        .ownership_facts()
    else {
        panic!("partial ownership facts must remain observable");
    };
    assert!(!facts.is_complete());
    assert!(analysis.trace().missing().any(|entry| {
        entry.capability()
            == CompileCapability::Resolve(ResolveCapability::ResolveCompleteStandardLibraryNames)
    }));
    let snapshot = session.into_diagnostics(None);
    let codes = diagnostic_codes(&snapshot);
    assert!(!codes.contains(&"RESOURCE_DROP_FORBIDDEN"));
    assert!(!codes.contains(&"IMPORT_NOT_FOUND"));
}

#[test]
fn conditional_ownership_carriers_and_question_flow_are_checked() {
    let cases = [
        "fn wrap(x: Own[i64]) -> Opt[Own[i64]] { return Some(move x) }\n",
        "fn wrap() -> Opt[Own[i64]] = None\n",
        "fn wrap(x: Own[i64]) -> Res[Own[i64], Str] { return Ok(move x) }\n",
        "fn wrap() -> Res[Own[i64], Str] = Err(\"failed\")\n",
        concat!(
            "fn consume(r: Res[Own[i64], Str]) -> Res[i64, Str] = result {\n",
            "  let x = (move r)?\n",
            "  drop_own move x\n",
            "  yield Ok(1)\n",
            "}\n",
        ),
    ];
    let catalog = TrustedCatalog::first_slice().unwrap();

    for source in cases {
        let mut session = session(source);
        let analysis = session
            .resolve(&ownership_profile(), Some(&catalog))
            .unwrap();
        let resolution = analysis.resolution().expect("ownership typing is selected");
        assert!(
            matches!(resolution.ownership_facts(), PhaseProduct::Performed(_)),
            "source:\n{source}"
        );
        assert!(
            diagnostic_codes(&session.into_diagnostics(None)).is_empty(),
            "source:\n{source}"
        );
    }
}

#[test]
fn conditional_carrier_exception_does_not_admit_general_ownership_generics() {
    let mut session = session(concat!(
        "fn identity[T](value: T) -> T = value\n",
        "fn bad(x: Own[i64]) -> Own[i64] = identity(move x)\n",
    ));
    let catalog = TrustedCatalog::first_slice().unwrap();

    session.resolve(&body_profile(), Some(&catalog)).unwrap();

    assert_eq!(
        diagnostic_codes(&session.into_diagnostics(None)),
        ["OWNERSHIP_TYPE_POSITION_FORBIDDEN"]
    );
}

#[test]
fn rebinding_a_reference_does_not_read_its_previous_referent() {
    let mut session = session(concat!(
        "fn rebind(x: Own[i64], y: Own[i64]) -> i64 {\n",
        "  let mut reference = &x\n",
        "  drop_own move x\n",
        "  reference = &y\n",
        "  drop_own move y\n",
        "  return 0\n",
        "}\n",
    ));
    let catalog = TrustedCatalog::first_slice().unwrap();

    let analysis = session
        .resolve(&ownership_profile(), Some(&catalog))
        .unwrap();

    assert!(matches!(
        analysis
            .resolution()
            .expect("ownership analysis is selected")
            .ownership_facts(),
        PhaseProduct::Performed(_)
    ));
    let snapshot = session.into_diagnostics(None);
    assert!(
        diagnostic_codes(&snapshot).is_empty(),
        "unexpected diagnostics: {:?}",
        diagnostic_codes(&snapshot)
    );
}

#[test]
fn references_through_places_retain_ownership_provenance() {
    let cases = [
        concat!(
            "fn peek(_x: &Own[i64]) -> i64 { return 1 }\n",
            "fn bad(x: Own[i64]) -> i64 {\n",
            "  let first = &x\n",
            "  let second = &*first\n",
            "  drop_own move x\n",
            "  return peek(second)\n",
            "}\n",
        ),
        concat!(
            "struct Pair { field value: i64 }\n",
            "fn peek(_x: &i64) -> i64 { return 1 }\n",
            "fn bad(pair: Pair) -> i64 {\n",
            "  let reference = &pair.value\n",
            "  let root = own pair\n",
            "  let value = peek(reference)\n",
            "  drop_own move root\n",
            "  return value\n",
            "}\n",
        ),
    ];
    let catalog = TrustedCatalog::first_slice().unwrap();

    for source in cases {
        let mut session = session(source);
        session
            .resolve(&ownership_profile(), Some(&catalog))
            .unwrap();
        assert_eq!(
            diagnostic_codes(&session.into_diagnostics(None)),
            ["OWNERSHIP_USE_AFTER_DISCHARGE"],
            "source:\n{source}"
        );
    }
}
