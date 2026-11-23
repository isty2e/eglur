use super::{diagnostic_codes, session};
use eglur_driver::{
    CompileAttempt, CompileAttemptError, CompileDisposition, CompilerSession, FileInput,
    FileSetRequest, ProgramMode, SessionOptions, SupportProfile, ToolLimit, TrustedCatalog,
};
use eglur_sema::{DeclarationKind, FactState, OwnershipBodyStatus, PhaseProduct, SemanticLocation};

const OWNERSHIP_MATCH: &str = concat!(
    "fn discharge(input: Opt[Own[i64]]) -> i64 = match move input {\n",
    "  None => 0,\n",
    "  Some(value) => { drop_own move value\nyield 1 }\n",
    "}\n",
);

fn core_attempt(source: &str, limit: Option<ToolLimit>) -> CompileAttempt {
    let mut session = session(source);
    let catalog = TrustedCatalog::first_slice().unwrap();
    session
        .resolve_for(
            ProgramMode::Executable,
            &SupportProfile::semantic_core_v1(),
            Some(&catalog),
        )
        .unwrap();
    CompileAttempt::finalize(session.seal(None).unwrap(), limit).unwrap()
}

#[test]
fn incomplete_ownership_bodies_keep_individual_locations_and_checked_siblings() {
    let main = format!(
        "{OWNERSHIP_MATCH}\
         fn balanced(root: Own[i64], flag: Bool) -> Unit {{\n\
           if flag {{ drop_own move root }} else {{ drop_own move root }}\n\
         }}\n\
         fn ordinary(input: Opt[i64]) -> i64 = match input {{ None => 0, Some(value) => value }}\n\
         fn main!() -> Unit = ()\n"
    );
    let request = FileSetRequest::new(
        [
            FileInput::new("main.eglur", main.as_bytes().to_vec()),
            FileInput::new(
                "dep.eglur",
                concat!(
                    "fn make(n: i64) -> Own[i64] { return own n }\n",
                    "fn choose(input: Opt[i64]) -> Own[i64] = match input {\n",
                    "  None => make(1), Some(value) => make(value)\n",
                    "}\n",
                    "fn release(input: Res[Own[i64], Str]) -> Unit = match move input {\n",
                    "  Ok(value) => { drop_own move value }, Err(_) => ()\n",
                    "}\n",
                )
                .as_bytes()
                .to_vec(),
            ),
        ],
        "main.eglur",
        Some("."),
    )
    .unwrap();
    let mut session = CompilerSession::build(request, SessionOptions::default()).unwrap();
    let catalog = TrustedCatalog::first_slice().unwrap();
    let analysis = session
        .resolve(&SupportProfile::semantic_core_v1(), Some(&catalog))
        .unwrap();
    let resolution = analysis.resolution().unwrap();
    let PhaseProduct::Performed(facts) = resolution.ownership_facts() else {
        panic!("partial ownership facts must be retained");
    };
    assert!(!facts.is_complete());

    let mut expected_locations = Vec::new();
    for module in resolution.declarations().modules() {
        for declaration in module
            .declarations()
            .filter(|declaration| declaration.kind() == DeclarationKind::Function)
        {
            let name = declaration.name().unwrap().spelling();
            let status = if matches!(name.as_str(), "discharge" | "release") {
                expected_locations.push(SemanticLocation::origin(
                    module.file(),
                    declaration.origin(),
                ));
                OwnershipBodyStatus::Unperformed
            } else {
                OwnershipBodyStatus::Checked
            };
            assert!(
                matches!(facts.body(declaration.id()), FactState::Known(body) if body.status() == status),
                "{name}: {:?}",
                facts.body(declaration.id())
            );
        }
    }
    expected_locations.sort();
    assert_eq!(expected_locations.len(), 2);

    let missing = analysis.trace().missing().collect::<Vec<_>>();
    assert_eq!(missing.len(), 1);
    assert_eq!(
        missing[0].capability().stable_key(),
        "typecheck.complete_ownership_flow"
    );
    assert_eq!(missing[0].locations(), expected_locations);

    let sealed = session.seal(None).unwrap();
    assert!(sealed.checked_program().is_none());
    assert!(diagnostic_codes(sealed.diagnostics()).is_empty());
}

#[test]
fn ownership_gap_projects_as_unsupported_without_a_source_diagnostic() {
    let source = format!("{OWNERSHIP_MATCH}fn main!() -> Unit = ()\n");
    let attempt = core_attempt(&source, None);
    assert_eq!(attempt.disposition(), CompileDisposition::Unsupported);
    assert!(matches!(
        attempt.compiler_result(),
        Err(CompileAttemptError::Inconclusive(
            CompileDisposition::Unsupported
        ))
    ));
    let payload: serde_json::Value =
        serde_json::from_str(&attempt.development_attempt().unwrap().to_json()).unwrap();
    assert_eq!(payload["disposition"], "unsupported");
    assert!(payload["diagnostics"].as_array().unwrap().is_empty());
    let missing = payload["capability_trace"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| entry["disposition"] == "missing")
        .collect::<Vec<_>>();
    assert_eq!(missing.len(), 1);
    assert_eq!(
        missing[0]["capability"],
        "typecheck.complete_ownership_flow"
    );
    assert_eq!(missing[0]["locations"].as_array().unwrap().len(), 1);
    assert!(
        attempt
            .render_human()
            .contains("typecheck.complete_ownership_flow")
    );
}

#[test]
fn unsupported_ownership_preserves_independent_errors_and_tool_limit_precedence() {
    let source = format!(
        "{OWNERSHIP_MATCH}\
         fn bad(root: Own[i64]) -> i64 {{ return 0 }}\n\
         fn main!() -> Unit = ()\n"
    );
    for (limit, expected) in [
        (None, CompileDisposition::Unsupported),
        (
            ToolLimit::new("driver.test_budget"),
            CompileDisposition::ToolLimitExceeded,
        ),
    ] {
        let attempt = core_attempt(&source, limit);
        assert_eq!(attempt.disposition(), expected);
        assert!(attempt.compiler_result().is_err());
        let payload: serde_json::Value =
            serde_json::from_str(&attempt.development_attempt().unwrap().to_json()).unwrap();
        let diagnostics = payload["diagnostics"].as_array().unwrap();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0]["code"], "OWNERSHIP_PATH_UNBALANCED");
        assert!(payload["capability_trace"].as_array().unwrap().iter().any(
            |entry| entry["capability"] == "typecheck.complete_ownership_flow"
                && entry["disposition"] == "missing"
        ));
    }
}

#[test]
fn upstream_control_and_catalog_gaps_do_not_invent_an_ownership_cause() {
    for (source, expected) in [
        (
            "fn deferred(root: Own[i64], flag: Bool) -> Unit = match flag { true => { drop_own move root }, false => { drop_own move root } }\n",
            "typecheck.pattern_coverage",
        ),
        (
            "fn deferred(root: Own[i64], flag: Bool) -> Unit { while flag { drop_own move root\nbreak } }\n",
            "typecheck.loops",
        ),
        (
            "fn deferred(root: Own[i64]) -> Unit { for item in [1] { drop_own move root\ncontinue } }\n",
            "typecheck.loops",
        ),
        (
            "fn deferred(root: Own[i64]) -> Unit { let f: Fn() -> Unit = {|| drop_own move root }\n _ = f }\n",
            "typecheck.closures",
        ),
        (
            "import std::io::ByteReader\nfn deferred(root: Own[ByteReader]) -> Unit { drop_own move root }\n",
            "resolve.complete_stdlib_names",
        ),
    ] {
        let source = format!("{source}fn main!() -> Unit = ()\n");
        let attempt = core_attempt(&source, None);
        let CompileAttempt::Unsupported(unsupported) = attempt else {
            panic!("source must remain unsupported: {source}");
        };
        let missing = unsupported
            .trace()
            .trace()
            .missing()
            .map(|entry| entry.capability().stable_key())
            .collect::<Vec<_>>();
        assert!(missing.contains(&expected), "{source}: {missing:?}");
        assert!(
            !missing.contains(&"typecheck.complete_ownership_flow"),
            "{source}: {missing:?}"
        );
    }
}

#[test]
fn supported_branch_and_result_ownership_remain_conclusive() {
    let source = concat!(
        "fn ordinary(input: Opt[i64]) -> i64 = match input { None => 0, Some(value) => value }\n",
        "fn balanced(root: Own[i64], flag: Bool) -> Unit {\n",
        "  if flag { drop_own move root } else { drop_own move root }\n",
        "}\n",
        "fn wrap(root: Own[i64]) -> Opt[Own[i64]] { return Some(move root) }\n",
        "fn take(source: Res[Own[i64], Str]) -> Res[i64, Str] = result {\n",
        "  let value = (move source)?\n",
        "  drop_own move value\n",
        "  yield Ok(1)\n",
        "}\n",
        "fn checked_drop(root: Own[i64], value: i8) -> Res[i8, NumericError] = checked {\n",
        "  drop_own move root\n",
        "  yield value + value\n",
        "}\n",
        "fn main!() -> Unit = ()\n",
    );
    let attempt = core_attempt(source, None);
    assert_eq!(
        attempt.disposition(),
        CompileDisposition::Succeeded,
        "{}",
        attempt.render_human()
    );
    assert!(attempt.compiler_result().unwrap().succeeded());
}
