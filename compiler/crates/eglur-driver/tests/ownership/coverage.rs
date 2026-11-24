use super::session;
use eglur_driver::{CompileAttempt, ProgramMode, SupportProfile, ToolLimit, TrustedCatalog};

const GUARD: &str = "OWNERSHIP_GUARD_CONSUME_FORBIDDEN";
const GAP: &str = "MATCH_NON_EXHAUSTIVE";
const DEAD: &str = "MATCH_UNREACHABLE_ARM";

fn attempt(body: &str, limit: Option<ToolLimit>) -> (String, CompileAttempt) {
    let source = format!(
        "fn take(v: Own[i64]) -> Bool {{ drop_own move v\nreturn true }}\n{body}\nfn main!() -> Unit = ()\n"
    );
    let mut session = session(&source);
    let catalog = TrustedCatalog::first_slice().unwrap();
    session
        .resolve_for(
            ProgramMode::Executable,
            &SupportProfile::semantic_core_v1(),
            Some(&catalog),
        )
        .unwrap();
    (
        source,
        CompileAttempt::finalize(session.seal(None).unwrap(), limit).unwrap(),
    )
}

fn diagnostics(body: &str) -> Vec<(String, String)> {
    let (source, attempt) = attempt(body, None);
    let CompileAttempt::Rejected(rejected) = attempt else {
        panic!("expected rejection: {:?}", attempt.disposition());
    };
    rejected
        .errors()
        .diagnostics()
        .iter()
        .map(|d| {
            let span = d.primary_span().as_source().unwrap().file_span().range();
            (
                d.code().as_str().to_owned(),
                source[span.start().as_usize()..span.end().as_usize()]
                    .trim()
                    .to_owned(),
            )
        })
        .collect()
}

#[test]
fn written_ownership_precedes_only_its_match_coverage() {
    for earlier in ["", "Some(x) => { drop_own move x\nyield 0 },"] {
        for (guard, expected, site) in [
            (
                "true",
                if earlier.is_empty() { GAP } else { DEAD },
                "Some(x)",
            ),
            ("take(x)", GUARD, "x"),
            ("take(move x)", GUARD, "move x"),
        ] {
            let source = format!(
                "fn f(r: Opt[Own[i64]]) -> i64 = match move r {{ {earlier}\n\
                 Some(x) if {guard} => {{ drop_own move x\nyield 1 }} }}"
            );
            assert_eq!(
                diagnostics(&source),
                [(expected.into(), site.into())],
                "{source}"
            );
        }
    }
}

#[test]
fn poisoned_match_value_does_not_hide_independent_sibling_roots() {
    let source = "fn f(r: Opt[Own[i64]], x: Opt[i64]) -> Unit {\n\
        let ignored = match move r { Some(v) if take(move v) => { drop_own move v\nyield 0 } }\n\
        let other = match x { None => 0 }\n}\n\
        fn sibling(n: Bool) -> i64 = n\n";
    let found = diagnostics(source);
    assert_eq!(
        found.iter().map(|d| d.0.as_str()).collect::<Vec<_>>(),
        [GUARD, GAP, "RETURN_TYPE_MISMATCH"]
    );
    assert_eq!(found[0].1, "move v");
    assert_eq!(found[1].1, "None");
}

#[test]
fn earlier_ownership_failure_remains_the_function_primary() {
    let source = "fn f(side: Own[i64], r: Opt[Own[i64]]) -> i64 {\n\
        drop_own move side\ndrop_own move side\n\
        return match move r { Some(x) if take(move x) => { drop_own move x\nyield 1 } }\n}";
    assert_eq!(
        diagnostics(source),
        [("OWNERSHIP_DOUBLE_MOVE".into(), "drop_own move side".into())]
    );
}

#[test]
fn invalid_match_results_do_not_supply_types_to_their_consumers() {
    let inner = "match move r { Some(x) if take(move x) => { drop_own move x\nyield 1 } }";
    for expression in [
        format!("expect_bool({inner})"),
        format!("match ({inner}) {{ _ => true }}"),
        format!("match flag {{ _ => {inner} }}"),
        format!("if flag {{ yield {inner} }} else {{ yield false }}"),
    ] {
        let source = format!(
            "fn expect_bool(v: Bool) -> Bool = v\nfn f(r: Opt[Own[i64]], flag: Bool) -> Bool = {expression}\n"
        );
        assert_eq!(
            diagnostics(&source),
            [(GUARD.into(), "move x".into())],
            "{source}"
        );
    }
}

#[test]
fn distinct_coverage_conflicts_share_one_ownership_primary() {
    let source = "fn f(r: Opt[Own[i64]], s: Opt[Own[i64]]) -> Unit {\n\
        let a = match move r { Some(x) if take(move x) => { drop_own move x\nyield 0 } }\n\
        let b = match move s { Some(y) if take(move y) => { drop_own move y\nyield 1 } }\n}";
    assert_eq!(diagnostics(source), [(GUARD.into(), "move x".into())]);
}

#[test]
fn unselected_call_does_not_require_a_move_from_another_argument() {
    let source = "fn check(v: Own[i64], n: i64) -> Bool = take(move v)\n\
        fn f(side: Own[i64], r: Opt[Own[i64]]) -> Bool = check(side,\n\
        match move r { Some(x) if take(move x) => { drop_own move x\nyield 1 } })";
    assert_eq!(diagnostics(source), [(GUARD.into(), "move x".into())]);
}

#[test]
fn independent_type_errors_do_not_become_ownership_assertions() {
    let make = |binding: &str, guard: &str| {
        format!(
            "fn f(r: Opt[Own[i64]]) -> Unit {{\n{binding}\n\
         let n = match move r {{ Some(x) if {guard} => {{ drop_own move x\nyield 0 }} }}\n}}"
        )
    };
    for binding in ["let wrong: Bool = 1", "let wrong = (true, false).9"] {
        let base = diagnostics(&make(binding, "true"));
        let mixed = diagnostics(&make(binding, "take(move x)"));
        assert_eq!(base.len(), 2, "{base:?}");
        assert_eq!(base[1].0, GAP);
        assert_eq!(mixed, [base[0].clone(), (GUARD.into(), "move x".into())]);
    }
}

#[test]
fn ownership_selection_preserves_an_independent_effect_error() {
    let source = "import std::io::print!\nimport std::io::TextError\n\
        fn f(r: Opt[Own[i64]]) -> i64 = match move r { Some(x) if take(move x) => { drop_own move x\nyield 0 } }\n\
        fn write() -> Res[Unit, TextError] = print!(\"bad\")\n";
    let found = diagnostics(source);
    assert_eq!(
        found.iter().map(|d| d.0.as_str()).collect::<Vec<_>>(),
        [GUARD, "EFFECT_VIOLATION"]
    );
}

#[test]
fn unavailable_body_and_tool_limit_keep_their_dispositions() {
    let valid_inputs = "fn f(r: Opt[Own[i64]]) -> i64 = match move r { Some(x) if take(move x) => { drop_own move x\nyield 0 } }";
    let (_, limited) = attempt(
        valid_inputs,
        Some(ToolLimit::new("driver.test_budget").unwrap()),
    );
    let CompileAttempt::ToolLimitExceeded(limited) = limited else {
        panic!("tool limit lost");
    };
    assert_eq!(super::diagnostic_codes(limited.diagnostics()), [GUARD]);

    let unavailable = "fn f(r: Opt[Own[i64]]) -> i64 {\nlet other = [true]\n\
        return match move r { Some(x) if take(move x) => { drop_own move x\nyield 0 } }\n}";
    let (_, result) = attempt(unavailable, None);
    let CompileAttempt::Unsupported(result) = result else {
        panic!("missing input was treated as checked");
    };
    assert_eq!(super::diagnostic_codes(result.diagnostics()), [GAP]);
}
