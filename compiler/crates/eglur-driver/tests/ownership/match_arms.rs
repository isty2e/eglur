use super::{diagnostic_codes, session};
use eglur_diagnostics::DiagnosticSnapshot;
use eglur_driver::{SupportProfile, TrustedCatalog};
use eglur_sema::{DeclarationKind, FactState, OwnershipBodyStatus, PhaseProduct, SemanticLocation};

fn checked(source: &str, expected: &[&str]) -> DiagnosticSnapshot {
    let mut session = session(source);
    let catalog = TrustedCatalog::first_slice().unwrap();
    let analysis = session
        .resolve(&SupportProfile::semantic_core_v1(), Some(&catalog))
        .unwrap();
    assert!(analysis.trace().missing().next().is_none(), "{source}");
    let resolution = analysis.resolution().unwrap();
    let PhaseProduct::Performed(facts) = resolution.ownership_facts() else {
        panic!("ownership checking was not performed: {source}");
    };
    assert!(facts.is_complete(), "{source}");
    for declaration in resolution
        .declarations()
        .modules()
        .flat_map(eglur_sema::ModuleDeclarations::declarations)
        .filter(|declaration| declaration.kind() == DeclarationKind::Function)
    {
        assert!(
            matches!(facts.body(declaration.id()), FactState::Known(body) if body.status() == OwnershipBodyStatus::Checked),
            "{source}: {:?}",
            facts.body(declaration.id())
        );
    }
    let snapshot = session.into_diagnostics(None);
    assert_eq!(
        diagnostic_codes(&snapshot),
        expected,
        "{source}\n{snapshot:?}"
    );
    snapshot
}

#[test]
fn arms_discharge_independently_and_missing_discharge_is_reported() {
    let good = concat!(
        "fn inspect(input: Opt[i64], root: Own[i64]) -> i64 = match input {\n",
        "  None => { drop_own move root\nyield 0 },\n",
        "  Some(value) => { drop_own move root\nyield value }\n",
        "}\n",
    );
    checked(good, &[]);

    let bad = concat!(
        "fn inspect(input: Opt[i64], root: Own[i64]) -> i64 {\n",
        "  let value = match input {\n",
        "    None => { drop_own move root\nyield 0 },\n",
        "    Some(value) => value\n",
        "  }\n",
        "  return value\n",
        "}\n",
    );
    let snapshot = checked(bad, &["OWNERSHIP_PATH_UNBALANCED"]);
    let span = snapshot.diagnostics()[0]
        .primary_span()
        .as_source()
        .unwrap()
        .file_span()
        .range();
    let exit = bad.find("return value").unwrap();
    assert!(span.start().as_usize() <= exit && span.end().as_usize() >= exit + "return".len());
}

#[test]
fn returning_the_same_root_from_alternative_arms_transfers_it_once() {
    checked(
        concat!(
            "fn choose(input: Opt[i64], root: Own[i64]) -> Own[i64] = match input {\n",
            "  None => move root,\n",
            "  Some(_) => move root\n",
            "}\n",
        ),
        &[],
    );
}

#[test]
fn consume_after_match_blames_later_move_and_keeps_arm_provenance() {
    let source = concat!(
        "fn inspect(input: Opt[i64], root: Own[i64]) -> Unit {\n",
        "  match input {\n",
        "    None => { drop_own move root },\n",
        "    Some(_) => { drop_own move root }\n",
        "  }\n",
        "  drop_own move root\n",
        "}\n",
    );
    let snapshot = checked(source, &["OWNERSHIP_DOUBLE_MOVE"]);
    let diagnostic = &snapshot.diagnostics()[0];
    let primary = diagnostic
        .primary_span()
        .as_source()
        .unwrap()
        .file_span()
        .range();
    let later = source.rfind("move root").unwrap();
    assert!(
        primary.start().as_usize() <= later
            && primary.end().as_usize() >= later + "move root".len()
    );
    assert_eq!(diagnostic.related().len(), 1);
    let first = diagnostic.related()[0].span().file_span().range();
    let arm_start = source.find("None =>").unwrap();
    assert!(first.start().as_usize() >= arm_start && first.end().as_usize() <= later);
}

#[test]
fn nested_yields_and_returns_do_not_leak_states_into_other_arms() {
    for source in [
        concat!(
            "fn inspect(input: Opt[i64], root: Own[i64], flag: Bool) -> i64 = match input {\n",
            "  None => { if flag { drop_own move root\nreturn 1 } else { yield () }\n",
            "            drop_own move root\nyield 0 },\n",
            "  Some(value) => { drop_own move root\nyield value }\n",
            "}\n",
        ),
        concat!(
            "fn inspect(input: Opt[i64], root: Own[i64]) -> i64 {\n",
            "  match input {\n",
            "    None => { drop_own move root\nreturn 0 },\n",
            "    Some(value) => { drop_own move root\nreturn value }\n",
            "  }\n",
            "}\n",
        ),
        concat!(
            "fn inspect(input: Opt[i64], root: Own[i64], flag: Bool) -> i64 =\n",
            "  match (if flag { drop_own move root\nreturn 0 } else { yield input }) {\n",
            "    None => { drop_own move root\nyield 1 },\n",
            "    Some(value) => { drop_own move root\nyield value }\n",
            "  }\n",
        ),
    ] {
        checked(source, &[]);
    }
}

#[test]
fn many_arms_start_from_the_same_incoming_ownership_state() {
    let arms = (0..16)
        .map(|i| format!("{i} => {{ drop_own move root\nyield {i} }},\n"))
        .collect::<String>();
    let source = format!(
        "fn inspect(input: i64, root: Own[i64]) -> i64 = match input {{\n{arms}_ => {{ drop_own move root\nyield -1 }}\n}}\n"
    );
    checked(&source, &[]);
}

#[test]
fn arm_block_shadowing_keeps_outer_root_identity() {
    checked(
        concat!(
            "fn inspect(input: Opt[i64], root: Own[i64]) -> i64 = match input {\n",
            "  None => { drop_own move root\nyield 0 },\n",
            "  Some(value) => { { let root = own value\n",
            "                      drop_own move root\nyield () }\n",
            "                   drop_own move root\nyield 0 }\n",
            "}\n",
        ),
        &[],
    );
}

#[test]
fn pattern_bindings_keep_only_their_projected_referents() {
    for (alias, expected) in [
        ("first", &["OWNERSHIP_USE_AFTER_DISCHARGE"][..]),
        ("whole.0", &["OWNERSHIP_USE_AFTER_DISCHARGE"][..]),
        ("pair.1", &[][..]),
        ("whole.1.0", &[][..]),
    ] {
        let source = format!(
            "fn peek(_v: &i64) -> i64 = 0\n\
             fn check(a: i64, b: i64, c: i64) -> i64 = match (&a, (&b, &c)) {{\n\
             whole @ (first, pair) => {{ let root = own a\n\
             let n = peek({alias})\ndrop_own move root\nyield n }}\n}}\n"
        );
        checked(&source, expected);
    }
}

#[test]
fn only_continuing_arms_contribute_to_a_returned_reference() {
    for (some, expected) in [
        ("&a", &["OWNERSHIP_USE_AFTER_DISCHARGE"][..]),
        ("{ return 1 }", &[][..]),
    ] {
        let source = format!(
            "fn peek(_v: &i64) -> i64 = 0\n\
             fn check(input: Opt[i64], a: i64, b: i64) -> i64 {{\n\
             let view = match input {{ None => &b, Some(_) => {some} }}\n\
             let root = own a\nlet n = peek(view)\ndrop_own move root\nreturn n\n}}\n"
        );
        checked(&source, expected);
    }
}

#[test]
fn destructuring_a_match_result_keeps_the_two_reference_paths_separate() {
    for (view, expected) in [
        ("first", &["OWNERSHIP_USE_AFTER_DISCHARGE"][..]),
        ("second", &[][..]),
    ] {
        let source = format!(
            "fn peek(_v: &i64) -> i64 = 0\n\
             fn check(input: Opt[i64], a: i64, b: i64) -> i64 {{\n\
             let (first, second) = match input {{ None => (&a, &b), Some(_) => (&a, &b) }}\n\
             let root = own a\nlet n = peek({view})\ndrop_own move root\nreturn n\n}}\n"
        );
        checked(&source, expected);
    }
}

#[test]
fn question_in_an_arm_contributes_to_the_enclosing_result_exit() {
    for (before, after, expected) in [
        ("drop_own move root\n", "", &[][..]),
        (
            "",
            "drop_own move root\n",
            &["OWNERSHIP_PATH_UNBALANCED"][..],
        ),
    ] {
        let source = format!(
            "fn check(input: Opt[i64], candidate: Opt[i64], root: Own[i64]) -> Opt[i64] = result {{\n\
             let n = match input {{ None => {{ drop_own move root\nyield 0 }},\n\
             Some(_) => {{ {before}let v = candidate?\n{after}yield v }} }}\n\
             yield Some(n)\n}}\n"
        );
        checked(&source, expected);
    }
}

#[test]
fn a_reference_overwritten_on_every_continuing_path_loses_its_old_referent() {
    for (on_some, expected) in [
        ("view = &b", &[][..]),
        ("return 3", &[][..]),
        ("()", &["OWNERSHIP_USE_AFTER_DISCHARGE"][..]),
    ] {
        let source = format!(
            "fn peek(_v: &i64) -> i64 = 0\n\
             fn check(input: Opt[i64], a: i64, b: i64) -> i64 {{\n\
             let mut view = &a\nmatch input {{ None => {{ view = &b }}, Some(_) => {{ {on_some} }} }}\n\
             let root = own a\nlet n = peek(view)\ndrop_own move root\nreturn n\n}}\n"
        );
        checked(&source, expected);
    }
}

#[test]
fn scrutinee_transfers_once_before_any_arm_forks() {
    checked(
        concat!(
            "fn take(x: Own[i64]) -> Opt[i64] { drop_own move x\nreturn None }\n",
            "fn f(x: Own[i64]) -> i64 = match take(move x) { None => 1, Some(v) => v }\n",
        ),
        &[],
    );
}

#[test]
fn guard_fallthrough_preserves_outer_roots_and_discards_arm_locals() {
    for guard in [
        "inspect(&root)",
        "{ let temp = own v\ndrop_own move temp\nyield flag }",
    ] {
        for (fallback, expected) in [
            ("{ drop_own move root\nyield 2 }", &[][..]),
            ("2", &["OWNERSHIP_PATH_UNBALANCED"][..]),
        ] {
            let source = format!(
                "fn inspect(_x: &Own[i64]) -> Bool = false\n\
                 fn check(input: Opt[i64], flag: Bool, root: Own[i64]) -> i64 = match input {{\n\
                 Some(v) if {guard} => {{ drop_own move root\nyield 1 }},\n\
                 Some(_) => {fallback}, None => {{ drop_own move root\nyield 3 }} }}\n\
                 fn main!() -> Unit {{ let a: i64 = 1\nlet b: i64 = 2\nlet c: i64 = 3\n\
                 _ = check(Some(1), true, own a)\n_ = check(Some(1), false, own b)\n\
                 _ = check(None, false, own c)\n}}\n"
            );
            checked(&source, expected);
        }
    }
}

#[test]
fn an_outer_move_in_a_guard_is_not_assumed_to_roll_back_on_false() {
    let source = concat!(
        "fn consume(x: Own[i64]) -> Bool { drop_own move x\nreturn false }\n",
        "fn check(input: Opt[i64], root: Own[i64]) -> i64 = match input {\n",
        "  Some(_) if consume(move root) => 1,\n",
        "  Some(_) => 2,\n",
        "  None => { drop_own move root\nyield 3 }\n}\n",
        "fn main!() -> Unit { let n: i64 = 1\n_ = check(Some(1), own n) }\n",
    );
    let mut session = session(source);
    let catalog = TrustedCatalog::first_slice().unwrap();
    let analysis = session
        .resolve(&SupportProfile::semantic_core_v1(), Some(&catalog))
        .unwrap();
    let missing = analysis
        .trace()
        .missing()
        .map(|entry| entry.capability().stable_key())
        .collect::<Vec<_>>();
    assert_eq!(missing, ["typecheck.complete_ownership_flow"]);
    let sealed = session.seal(None).unwrap();
    assert!(sealed.checked_program().is_none());
    assert!(diagnostic_codes(sealed.diagnostics()).is_empty());
}

#[test]
fn the_selected_arm_sees_its_own_guards_consumption() {
    checked(
        concat!(
            "fn check(input: Opt[i64], flag: Bool) -> i64 = match input {\n",
            "  Some(v) if { let t = own v\ndrop_own move t\nyield flag } => v,\n",
            "  Some(other) => other, None => 0 }\n",
            "fn main!() -> Unit { _ = check(Some(1), true) }\n",
        ),
        &["OWNERSHIP_USE_AFTER_DISCHARGE"],
    );
}

#[test]
fn ordinary_match_is_checked_beside_guarded_owned_match_gap() {
    let source = concat!(
        "fn ordinary(input: Opt[i64], root: Own[i64]) -> i64 = match input {\n",
        "  None => { drop_own move root\nyield 0 },\n",
        "  Some(value) => { drop_own move root\nyield value }\n",
        "}\n",
        "fn guarded(input: Opt[Own[i64]]) -> i64 = match move input {\n",
        "  Some(value) if false => { drop_own move value\nyield 0 },\n",
        "  Some(value) => { drop_own move value\nyield 1 },\n",
        "  None => 2\n",
        "}\n",
    );
    let mut session = session(source);
    let catalog = TrustedCatalog::first_slice().unwrap();
    let analysis = session
        .resolve(&SupportProfile::semantic_core_v1(), Some(&catalog))
        .unwrap();
    let resolution = analysis.resolution().unwrap();
    let PhaseProduct::Performed(facts) = resolution.ownership_facts() else {
        panic!("partial ownership facts must be retained");
    };
    assert!(!facts.is_complete());
    let mut guarded_location = None;
    for module in resolution.declarations().modules() {
        for declaration in module
            .declarations()
            .filter(|d| d.kind() == DeclarationKind::Function)
        {
            let name = declaration.name().unwrap().spelling();
            let status = if name == "guarded" {
                guarded_location = Some(SemanticLocation::origin(
                    module.file(),
                    declaration.origin(),
                ));
                OwnershipBodyStatus::Unperformed
            } else {
                assert_eq!(name, "ordinary");
                OwnershipBodyStatus::Checked
            };
            assert!(
                matches!(facts.body(declaration.id()), FactState::Known(body) if body.status() == status),
                "{name}"
            );
        }
    }
    let missing = analysis.trace().missing().collect::<Vec<_>>();
    assert_eq!(missing.len(), 1);
    assert_eq!(
        missing[0].capability().stable_key(),
        "typecheck.complete_ownership_flow"
    );
    assert_eq!(missing[0].locations(), [guarded_location.unwrap()]);
    assert!(diagnostic_codes(&session.into_diagnostics(None)).is_empty());
}
