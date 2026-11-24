use super::{body_profile, branching_profile, diagnostic_codes, session};
use eglur_diagnostics::DiagnosticDataValue;
use eglur_driver::{
    CompilerSession, FileInput, FileSetRequest, SessionOptions, SupportProfile, SupportProfileId,
    SupportSchemaVersion, TrustedCatalog,
};
use eglur_sema::{CompileCapability, FactState, PrimitiveType, TypeKind, TypecheckCapability};

#[test]
fn matching_local_ordinals_in_distinct_modules_keep_their_pattern_types() {
    let request = FileSetRequest::new(
        [
            FileInput::new(
                "first.eglur",
                b"fn f(input: i64) -> i64 = match input { value => value }".to_vec(),
            ),
            FileInput::new(
                "second.eglur",
                b"fn g(input: Bool) -> Bool = match input { value => value }".to_vec(),
            ),
        ],
        "first.eglur",
        Some("."),
    )
    .unwrap();
    let mut session = CompilerSession::build(request, SessionOptions::new(false)).unwrap();
    let catalog = TrustedCatalog::first_slice().unwrap();
    let analysis = session
        .resolve(&SupportProfile::semantic_core_v1(), Some(&catalog))
        .unwrap();
    assert!(!analysis.trace().has_missing());
    let resolution = analysis.resolution().unwrap();
    let bindings = resolution
        .names()
        .locals()
        .iter()
        .filter(|local| local.name().text() == "value")
        .collect::<Vec<_>>();
    assert_eq!(bindings.len(), 2);
    assert_ne!(bindings[0].id().module(), bindings[1].id().module());
    assert_eq!(bindings[0].id().body(), bindings[1].id().body());
    assert_eq!(bindings[0].id().local(), bindings[1].id().local());
    for (binding, primitive) in bindings
        .iter()
        .zip([PrimitiveType::I64, PrimitiveType::Bool])
    {
        let FactState::Known(local) = resolution.body_types().local(binding.id()) else {
            panic!("missing local type")
        };
        assert_eq!(
            resolution.signatures().types().kind(local.ty()).unwrap(),
            &TypeKind::Primitive(primitive)
        );
    }
    assert!(session.into_diagnostics(None).diagnostics().is_empty());
}

#[test]
fn duplicate_aliases_report_the_later_binding_not_an_internal_collision() {
    let catalog = TrustedCatalog::first_slice().unwrap();
    for (ty, pattern, later) in [
        ("i64", "x @ x", "x"),
        ("i64", "x @ (x)", "x)"),
        ("i64", "x @ x @ x", "x @ x"),
        ("(i64, i64)", "x @ (x, _)", "x, _"),
        ("(i64, i64)", "(x @ x, _)", "x, _"),
        ("Opt[i64]", "x @ Some(x)", "x)"),
        ("Opt[i64]", "Some(x @ x)", "x)"),
    ] {
        let source = format!("fn f(v: {ty}) -> i64 = match v {{ {pattern} => 0, _ => 0 }}\n");
        let mut rejected = session(&source);
        rejected
            .resolve(&branching_profile(), Some(&catalog))
            .unwrap();
        let snapshot = rejected.into_diagnostics(None);
        assert_eq!(
            diagnostic_codes(&snapshot),
            ["PATTERN_DUPLICATE_BINDING"],
            "{pattern}"
        );
        let diagnostic = &snapshot.diagnostics()[0];
        assert_eq!(
            diagnostic.data().get("name"),
            Some(&DiagnosticDataValue::from("x"))
        );
        let range = diagnostic
            .primary_span()
            .as_source()
            .unwrap()
            .file_span()
            .range();
        let pattern_start = source.find(pattern).unwrap();
        let expected = pattern_start + pattern.rfind(later).unwrap();
        assert_eq!(range.start().as_usize(), expected, "{pattern}");
        assert_eq!(
            &source[range.start().as_usize()..range.end().as_usize()],
            "x"
        );
    }
}

#[test]
fn consuming_carrier_patterns_reject_the_second_owning_binding() {
    let catalog = TrustedCatalog::first_slice().unwrap();
    for (input, subject, pattern, first, later) in [
        (
            "v: Opt[Own[i64]]",
            "move v",
            "whole @ Some(item)",
            "whole",
            "item",
        ),
        (
            "v: Res[Own[i64], Str]",
            "move v",
            "whole @ Ok(item)",
            "whole",
            "item",
        ),
        (
            "v: Opt[Own[i64]]",
            "move v",
            "Some(first @ (second))",
            "first",
            "second",
        ),
        (
            "v: Opt[Own[i64]]",
            "move v",
            "one @ two @ Some(three)",
            "one",
            "two",
        ),
        (
            "v: Opt[Own[i64]]",
            "move v",
            "one @ two @ None",
            "one",
            "two",
        ),
        (
            "n: i64",
            "Some(own n)",
            "whole @ Some(item)",
            "whole",
            "item",
        ),
    ] {
        let source = format!(
            "fn f({input}) -> i64 = match {subject} {{ {pattern} if false => 0, rest => {{ drop_own move rest\nyield 1 }} }}\n"
        );
        let mut compiler = session(&source);
        compiler
            .resolve(&SupportProfile::semantic_core_v1(), Some(&catalog))
            .unwrap();
        let snapshot = compiler.into_diagnostics(None);
        assert_eq!(
            diagnostic_codes(&snapshot),
            ["PATTERN_OWNERSHIP_BINDING_OVERLAP"],
            "{source}"
        );
        let diagnostic = &snapshot.diagnostics()[0];
        let primary = diagnostic
            .primary_span()
            .as_source()
            .unwrap()
            .file_span()
            .range();
        let related = diagnostic.related()[0].span().file_span().range();
        let start = source.find(pattern).unwrap();
        for (span, binding) in [(primary, later), (related, first)] {
            assert_eq!(
                span.start().as_usize(),
                start + pattern.find(binding).unwrap()
            );
            assert_eq!(
                &source[span.start().as_usize()..span.end().as_usize()],
                binding
            );
        }
    }
}

#[test]
fn owning_pattern_names_do_not_override_binding_or_shape_errors() {
    let catalog = TrustedCatalog::first_slice().unwrap();
    for (pattern, expected) in [
        ("whole @ Some(whole)", "PATTERN_DUPLICATE_BINDING"),
        ("whole @ Some(first, second)", "PATTERN_ARITY_MISMATCH"),
    ] {
        let source =
            format!("fn f(v: Opt[Own[i64]]) -> i64 = match move v {{ {pattern} => 0, None => 1 }}");
        let mut compiler = session(&source);
        compiler
            .resolve(&SupportProfile::semantic_core_v1(), Some(&catalog))
            .unwrap();
        assert_eq!(
            diagnostic_codes(&compiler.into_diagnostics(None)),
            [expected],
            "{source}"
        );
    }
}

#[test]
fn single_owning_bindings_and_ownership_free_aliases_remain_distinct() {
    let catalog = TrustedCatalog::first_slice().unwrap();
    for source in [
        "fn f(v: Opt[Own[i64]]) -> Opt[Own[i64]] = match move v { whole @ Some(_) => move whole, empty @ None => move empty }",
        "fn f(v: Opt[Own[i64]]) -> Opt[Own[i64]] = match move v { Some(x) => Some(move x), None => None }",
        "fn f(v: Res[Own[i64], Str]) -> Res[Own[i64], Str] = match move v { Ok(x) => Ok(move x), whole @ Err(reason) => move whole }",
        "fn f(v: Opt[i64]) -> i64 = match v { whole @ Some(item) => item, None => 0 }",
    ] {
        let mut compiler = session(source);
        let analysis = compiler
            .resolve(&SupportProfile::semantic_core_v1(), Some(&catalog))
            .unwrap();
        let resolution = analysis.resolution().unwrap();
        let function = resolution
            .declarations()
            .modules()
            .flat_map(|m| m.declarations())
            .find(|d| d.name().is_some_and(|name| name.text() == "f"))
            .unwrap();
        assert!(
            matches!(
                resolution.body_types().body(function.id()),
                FactState::Known(_)
            ),
            "{source}"
        );
        assert!(
            diagnostic_codes(&compiler.into_diagnostics(None)).is_empty(),
            "{source}"
        );
    }
}

#[test]
fn alias_canonicalization_preserves_pattern_diagnostic_precedence() {
    let catalog = TrustedCatalog::first_slice().unwrap();
    for (source, expected) in [
        (
            "fn f(v: i64) -> i64 = match v { x @ (x, _) => 0 }",
            "PATTERN_TYPE_MISMATCH",
        ),
        (
            "fn f(v: (i64, i64)) -> i64 = match v { x @ (x, _, _) => 0 }",
            "PATTERN_ARITY_MISMATCH",
        ),
        (
            "fn f(v: Opt[i64]) -> i64 = match v { x @ Some(x, x) => 0 }",
            "PATTERN_ARITY_MISMATCH",
        ),
        (
            "fn f(v: i64) -> i64 = match v { x @ Missing(x) => 0 }",
            "NAME_NOT_FOUND",
        ),
        (
            "fn f() -> i64 { let x @ x = 1\nyield 0 }",
            "LET_PATTERN_DUPLICATE_BINDING",
        ),
        (
            "fn f() -> i64 { let x @ y = 1\nyield 0 }",
            "LET_PATTERN_UNSUPPORTED",
        ),
        ("fn f(x @ x: i64) -> i64 = 0", "PARAM_PATTERN_UNSUPPORTED"),
        (
            "fn f() -> i64 { let x = 0\nlet x @ x = 1\nyield 0 }",
            "LOCAL_BINDING_REDECLARATION",
        ),
    ] {
        let mut rejected = session(source);
        rejected
            .resolve(&branching_profile(), Some(&catalog))
            .unwrap();
        assert_eq!(
            diagnostic_codes(&rejected.into_diagnostics(None)),
            [expected],
            "{source}"
        );
    }
}

#[test]
fn distinct_aliases_remain_valid_bindings() {
    let source = "fn f(v: i64) -> i64 = match v { outer @ inner @ x => outer }\n";
    let catalog = TrustedCatalog::first_slice().unwrap();
    let mut accepted = session(source);
    let analysis = accepted
        .resolve(&branching_profile(), Some(&catalog))
        .unwrap();
    let resolution = analysis.resolution().expect("body typing is selected");
    assert_eq!(resolution.names().locals().len(), 4);
    for binding in resolution.names().locals() {
        let FactState::Known(ty) = resolution.body_types().local(binding.id()) else {
            panic!("alias binding has no known type");
        };
        assert!(matches!(
            resolution.signatures().types().kind(ty.ty()).unwrap(),
            TypeKind::Primitive(PrimitiveType::I64)
        ));
    }
    assert!(diagnostic_codes(&accepted.into_diagnostics(None)).is_empty());
}

#[test]
fn parser_budgeted_match_patterns_typecheck_on_the_default_stack() {
    let catalog = TrustedCatalog::first_slice().unwrap();
    let grouped = format!("{}value{}", "(".repeat(254), ")".repeat(254));
    let aliases = (0..254)
        .map(|index| format!("alias_{index} @ "))
        .collect::<String>();

    for pattern in [grouped, format!("{aliases}value")] {
        let source = format!("fn f(input: i64) -> i64 = match input {{ {pattern} => value }}\n");
        let mut accepted = session(&source);
        let analysis = accepted
            .resolve(&branching_profile(), Some(&catalog))
            .unwrap();
        assert!(analysis.trace().missing().next().is_none(), "{pattern}");
        assert!(
            diagnostic_codes(&accepted.into_diagnostics(None)).is_empty(),
            "{pattern}"
        );
    }
}

#[test]
fn parser_budgeted_option_patterns_typecheck_with_payload_bindings() {
    let depth = 254;
    let ty = format!("{}i64{}", "Opt[".repeat(depth), "]".repeat(depth));
    let pattern = format!("{}value{}", "Some(".repeat(depth), ")".repeat(depth));
    let source =
        format!("fn f(input: {ty}) -> i64 = match input {{ {pattern} => value, _ => 0 }}\n");
    let catalog = TrustedCatalog::first_slice().unwrap();
    let mut accepted = session(&source);
    let analysis = accepted
        .resolve(&branching_profile(), Some(&catalog))
        .unwrap();
    assert!(analysis.trace().missing().next().is_none());
    let resolution = analysis.resolution().unwrap();
    let binding = resolution
        .names()
        .locals()
        .iter()
        .find(|binding| binding.name().spelling() == "value")
        .unwrap();
    assert!(matches!(
        resolution.body_types().local(binding.id()),
        FactState::Known(_)
    ));
    assert!(diagnostic_codes(&accepted.into_diagnostics(None)).is_empty());
}

#[test]
fn parser_budgeted_pattern_failure_keeps_one_diagnostic() {
    let catalog = TrustedCatalog::first_slice().unwrap();
    let pattern = format!("{}(value, _){}", "(".repeat(253), ")".repeat(253));
    let source = format!("fn f(input: i64) -> i64 = match input {{ {pattern} => 0 }}\n");
    let mut rejected = session(&source);
    rejected
        .resolve(&branching_profile(), Some(&catalog))
        .unwrap();
    assert_eq!(
        diagnostic_codes(&rejected.into_diagnostics(None)),
        ["PATTERN_TYPE_MISMATCH"]
    );
}

#[test]
fn parser_budgeted_let_groups_bind_on_the_default_stack() {
    let catalog = TrustedCatalog::first_slice().unwrap();
    let pattern = format!("{}value{}", "(".repeat(254), ")".repeat(254));
    let source = format!("fn f(input: i64) -> i64 {{ let {pattern} = input\n yield value }}\n");
    let mut accepted = session(&source);
    let analysis = accepted
        .resolve(&branching_profile(), Some(&catalog))
        .unwrap();
    assert!(analysis.trace().missing().next().is_none());
    let resolution = analysis.resolution().unwrap();
    let binding = resolution
        .names()
        .locals()
        .iter()
        .find(|binding| binding.name().spelling() == "value")
        .unwrap();
    assert!(matches!(
        resolution.body_types().local(binding.id()),
        FactState::Known(_)
    ));
    assert!(diagnostic_codes(&accepted.into_diagnostics(None)).is_empty());
}

#[test]
fn parser_budgeted_let_groups_preserve_duplicate_and_initializer_errors() {
    let catalog = TrustedCatalog::first_slice().unwrap();
    let groups = "(".repeat(245);
    let closes = ")".repeat(245);
    let cases = [
        (
            format!(
                "fn f(input: (i64, i64)) -> i64 {{ let {groups}(value, value){closes} = input\n yield 0 }}\n"
            ),
            "LET_PATTERN_DUPLICATE_BINDING",
        ),
        (
            format!("fn f() -> i64 {{ let {groups}value{closes} = missing\n yield 0 }}\n"),
            "NAME_NOT_FOUND",
        ),
    ];
    for (source, expected) in cases {
        let mut rejected = session(&source);
        let analysis = rejected
            .resolve(&branching_profile(), Some(&catalog))
            .unwrap();
        let resolution = analysis.resolution().unwrap();
        let bindings = resolution
            .names()
            .locals()
            .iter()
            .filter(|binding| binding.name().spelling() == "value")
            .collect::<Vec<_>>();
        assert!(!bindings.is_empty(), "{expected}");
        for binding in bindings {
            assert!(
                matches!(
                    resolution.body_types().local(binding.id()),
                    FactState::Poisoned(_)
                ),
                "{expected}"
            );
        }
        assert_eq!(
            diagnostic_codes(&rejected.into_diagnostics(None)),
            [expected],
            "{expected}"
        );
    }
}

#[test]
fn deep_let_groups_preserve_tuple_siblings_mutability_and_inner_blame() {
    let catalog = TrustedCatalog::first_slice().unwrap();
    let groups = "(".repeat(245);
    let closes = ")".repeat(245);
    let source = format!(
        "fn f(input: (i64, i64)) -> i64 {{ let mut ({groups}left{closes}, right) = input\n left = right\n yield left }}\n"
    );
    let mut accepted = session(&source);
    let analysis = accepted
        .resolve(&branching_profile(), Some(&catalog))
        .unwrap();
    assert!(analysis.trace().missing().next().is_none());
    assert!(diagnostic_codes(&accepted.into_diagnostics(None)).is_empty());

    let source = format!(
        "fn f(input: (i64, Unit)) -> i64 {{ let ({groups}(){closes}, ()) = input\n yield 0 }}\n"
    );
    let mut rejected = session(&source);
    rejected
        .resolve(&branching_profile(), Some(&catalog))
        .unwrap();
    let snapshot = rejected.into_diagnostics(None);
    assert_eq!(diagnostic_codes(&snapshot), ["LET_PATTERN_TYPE_MISMATCH"]);
    let range = snapshot.diagnostics()[0]
        .primary_span()
        .as_source()
        .unwrap()
        .file_span()
        .range();
    assert_eq!(range.start().as_usize(), source.find("()").unwrap());
    assert_eq!(
        &source[range.start().as_usize()..range.end().as_usize()],
        "()"
    );
}

#[test]
fn bare_enum_variant_spellings_are_not_whole_value_bindings() {
    let catalog = TrustedCatalog::first_slice().unwrap();
    for (declarations, ty, pattern, name) in [
        ("enum Color { Red\n Blue }\n", "Color", "Red", "Red"),
        ("enum Color { Red\n Blue }\n", "Color", "(Red)", "Red"),
        (
            "enum Color { Red\n Blue }\ntype Shade = Color\n",
            "Shade",
            "Red",
            "Red",
        ),
        (
            "enum Color { Red\n Blue }\n",
            "(Color, i64)",
            "(Red, _)",
            "Red",
        ),
        (
            "enum Color { Red\n Blue }\n",
            "Opt[Color]",
            "Some(Red)",
            "Red",
        ),
        (
            "enum Boxed[T] { Value(T)\n Empty }\n",
            "Boxed[i64]",
            "Value",
            "Value",
        ),
        (
            "import std::io::TextError\n",
            "TextError",
            "InvalidUtf8",
            "InvalidUtf8",
        ),
        ("import std::io::TextError\n", "TextError", "Io", "Io"),
        (
            "import std::io::TextError\n",
            "Opt[TextError]",
            "Some(InvalidUtf8)",
            "InvalidUtf8",
        ),
    ] {
        let source = format!(
            "{declarations}fn f(value: {ty}) -> i64 = match value {{ {pattern} => 0, _ => 1 }}\n"
        );
        let mut rejected = session(&source);
        rejected
            .resolve(&branching_profile(), Some(&catalog))
            .unwrap();
        let snapshot = rejected.into_diagnostics(None);
        assert_eq!(diagnostic_codes(&snapshot), ["NAME_AMBIGUOUS"], "{source}");
        let diagnostic = &snapshot.diagnostics()[0];
        assert_eq!(
            diagnostic.data().get("name"),
            Some(&DiagnosticDataValue::from(name))
        );
        let range = diagnostic
            .primary_span()
            .as_source()
            .unwrap()
            .file_span()
            .range();
        assert_eq!(
            &source[range.start().as_usize()..range.end().as_usize()],
            name
        );
        assert!(range.start().as_usize() > source.find("match value").unwrap());
    }
}

#[test]
fn enum_variant_names_only_conflict_in_bare_match_binding_roles() {
    let catalog = TrustedCatalog::first_slice().unwrap();
    for source in [
        "enum Color { Red\n Blue }\nfn f(value: Color) -> i64 = match value { other => 0 }\n",
        "enum Color { Red\n Blue }\nfn f(value: Color) -> i64 = match value { Red @ _ => 0 }\n",
        "enum Color { Red\n Blue }\nfn f(Red: Color) -> Color = Red\n",
        "enum Color { Red\n Blue }\nfn f(value: Color) -> Color { let Red = value\nyield Red }\n",
        "enum Color { Red\n Blue }\nfn f(value: i64) -> i64 = match value { Red => Red }\n",
        "enum Color { Red\n Blue }\nenum Other { Green }\nfn f(value: Other) -> i64 = match value { Red => 0 }\n",
        "import std::io::TextError\nfn f(value: TextError) -> i64 = match value { InvalidUtf8 @ _ => 0 }\n",
    ] {
        let mut accepted = session(source);
        accepted
            .resolve(&branching_profile(), Some(&catalog))
            .unwrap();
        assert!(
            diagnostic_codes(&accepted.into_diagnostics(None)).is_empty(),
            "{source}"
        );
    }
}

#[test]
fn poisoned_enum_members_do_not_create_derivative_pattern_diagnostics() {
    let catalog = TrustedCatalog::first_slice().unwrap();
    for pattern in ["Red", "other"] {
        let source = format!(
            "enum Color {{ Red(Missing) }}\nfn f(value: Color) -> i64 = match value {{ {pattern} => 0, _ => 1 }}\n"
        );
        let mut rejected = session(&source);
        rejected
            .resolve(&branching_profile(), Some(&catalog))
            .unwrap();
        assert_eq!(
            diagnostic_codes(&rejected.into_diagnostics(None)),
            ["NAME_NOT_FOUND"]
        );
    }
}

#[test]
fn enum_metadata_does_not_enable_unsupported_pattern_or_constructor_paths() {
    let catalog = TrustedCatalog::first_slice().unwrap();
    let without_members = SupportProfile::compile_only(
        SupportSchemaVersion::V1,
        SupportProfileId::new("without-members").unwrap(),
        branching_profile()
            .compile_capabilities()
            .filter(|capability| {
                *capability != CompileCapability::Typecheck(TypecheckCapability::ResolveMembers)
            }),
    )
    .unwrap();
    for source in [
        "import std::io::TextError\nfn f(value: TextError) -> i64 = match value { TextError::InvalidUtf8 => 0, _ => 1 }\n",
        "import std::io::TextError\nfn f() -> TextError = TextError::InvalidUtf8\n",
    ] {
        let mut unsupported = session(source);
        let analysis = unsupported
            .resolve(&without_members, Some(&catalog))
            .unwrap();
        assert!(analysis.trace().missing().any(|entry| entry.capability()
            == CompileCapability::Typecheck(TypecheckCapability::ResolveMembers)));
        assert!(diagnostic_codes(&unsupported.into_diagnostics(None)).is_empty());
    }

    let mut unsupported =
        session("enum Color { Red }\nfn f(value: Color) -> i64 = match value { Red => 0 }\n");
    let analysis = unsupported
        .resolve(&body_profile(), Some(&catalog))
        .unwrap();
    assert!(analysis.trace().missing().any(|entry| entry.capability()
        == CompileCapability::Typecheck(TypecheckCapability::CheckBranching)));
    assert!(diagnostic_codes(&unsupported.into_diagnostics(None)).is_empty());
}

#[test]
fn failed_tuple_patterns_preserve_all_roots_later_bindings_and_later_bodies() {
    let catalog = TrustedCatalog::first_slice().unwrap();
    for count in [8_usize, 32, 128] {
        for mode in ["unit", "mixed", "grouped", "nested", "mut"] {
            let pattern = (0..count)
                .map(|i| match mode {
                    "mixed" | "mut" if i % 2 == 0 => format!("v{i}"),
                    "grouped" => "((()))".into(),
                    "nested" => "((), ())".into(),
                    _ => "()".into(),
                })
                .collect::<Vec<_>>()
                .join(", ");
            let values = (0..count)
                .map(|_| if mode == "nested" { "(0, 1)" } else { "0" })
                .collect::<Vec<_>>()
                .join(", ");
            let source = format!(
                "fn f() -> Unit {{ let {}({pattern}) = ({values})\nyield () }}\nfn after() -> Unit = ()\n",
                if mode == "mut" { "mut " } else { "" }
            );
            let mut rejected = session(&source);
            let analysis = rejected
                .resolve(&SupportProfile::semantic_core_v1(), Some(&catalog))
                .unwrap();
            let resolution = analysis.resolution().unwrap();
            let after = resolution
                .declarations()
                .modules()
                .flat_map(|m| m.declarations())
                .find(|d| d.name().is_some_and(|name| name.text() == "after"))
                .unwrap()
                .id();
            assert!(matches!(
                resolution.body_types().body(after),
                FactState::Known(_)
            ));
            let locals = resolution
                .names()
                .locals()
                .iter()
                .filter(|b| b.name().text().starts_with('v'))
                .collect::<Vec<_>>();
            assert_eq!(
                locals.len(),
                if matches!(mode, "mixed" | "mut") {
                    count / 2
                } else {
                    0
                }
            );
            for binding in locals {
                let FactState::Known(local) = resolution.body_types().local(binding.id()) else {
                    panic!("later local missing")
                };
                assert!(matches!(
                    resolution.signatures().types().kind(local.ty()).unwrap(),
                    TypeKind::Primitive(PrimitiveType::I64)
                ));
                assert_eq!(
                    local.mutability(),
                    if mode == "mut" {
                        eglur_hir::Mutability::Mutable
                    } else {
                        eglur_hir::Mutability::Immutable
                    }
                );
            }
            let snapshot = rejected.into_diagnostics(None);
            assert_eq!(
                diagnostic_codes(&snapshot),
                vec![
                    "LET_PATTERN_TYPE_MISMATCH";
                    match mode {
                        "mixed" | "mut" => count / 2,
                        "nested" => 2 * count,
                        _ => count,
                    }
                ]
            );
            let spans = snapshot
                .diagnostics()
                .iter()
                .map(|d| {
                    d.primary_span()
                        .as_source()
                        .unwrap()
                        .file_span()
                        .range()
                        .start()
                        .as_usize()
                })
                .collect::<std::collections::BTreeSet<_>>();
            assert_eq!(spans.len(), snapshot.diagnostics().len());
        }
    }
}

#[test]
fn callable_setup_binds_every_known_parameter_and_respects_signature_rejection() {
    let catalog = TrustedCatalog::first_slice().unwrap();
    for count in [8_usize, 32, 128] {
        let parameters = (0..count)
            .map(|i| format!("v{i}: i64"))
            .collect::<Vec<_>>()
            .join(", ");
        let source = format!(
            "fn f({parameters}) -> i64 = v{}\nfn after() -> Unit = ()\n",
            count - 1
        );
        let mut accepted = session(&source);
        let analysis = accepted
            .resolve(&SupportProfile::semantic_core_v1(), Some(&catalog))
            .unwrap();
        let resolution = analysis.resolution().unwrap();
        assert_eq!(resolution.names().locals().len(), count);
        assert!(
            resolution
                .names()
                .locals()
                .iter()
                .all(|b| matches!(resolution.body_types().local(b.id()), FactState::Known(_)))
        );
        assert!(analysis.fingerprints().is_some());
        assert!(diagnostic_codes(&accepted.into_diagnostics(None)).is_empty());

        let parameters = (0..count)
            .map(|i| format!("(a{i}, b{i}): (i64, i64)"))
            .collect::<Vec<_>>()
            .join(", ");
        let source = format!("fn f({parameters}) -> Unit = [1]\nfn after() -> Unit = ()\n");
        let mut rejected = session(&source);
        let analysis = rejected.resolve(&body_profile(), Some(&catalog)).unwrap();
        let resolution = analysis.resolution().unwrap();
        let after = resolution
            .declarations()
            .modules()
            .flat_map(|m| m.declarations())
            .find(|d| d.name().is_some_and(|name| name.text() == "after"))
            .unwrap()
            .id();
        assert!(matches!(
            resolution.body_types().body(after),
            FactState::Known(_)
        ));
        assert_eq!(
            diagnostic_codes(&rejected.into_diagnostics(None)),
            vec!["PARAM_PATTERN_UNSUPPORTED"; count]
        );
    }
}
