use super::match_arms::checked;

pub(super) fn incomplete(source: &str) {
    let mut session = super::session(source);
    let catalog = eglur_driver::TrustedCatalog::first_slice().unwrap();
    let analysis = session
        .resolve(
            &eglur_driver::SupportProfile::semantic_core_v1(),
            Some(&catalog),
        )
        .unwrap();
    let missing: Vec<_> = analysis
        .trace()
        .missing()
        .map(|item| item.capability().stable_key())
        .collect();
    let snapshot = session.into_diagnostics(None);
    assert!(snapshot.diagnostics().is_empty(), "{source}\n{snapshot:?}");
    assert_eq!(missing, ["typecheck.complete_ownership_flow"], "{source}");
}

#[test]
fn equal_joined_aliases_do_not_prove_equal_boolean_exits() {
    incomplete(
        "fn peek(_v: &i64) -> i64 = 0\n\
        fn f(input: Opt[i64], a: i64, b: i64, flag: Bool) -> i64 {\n\
        let mut p = if flag { yield &a } else { yield &b }\nreturn match input {\n\
        _ if { yield if flag { p = &a\nyield true } else { p = &b\nyield false } } => { let r = own b\nlet n = peek(p)\ndrop_own move r\nyield n },\n\
        _ => { let r = own a\nlet n = peek(p)\ndrop_own move r\nyield n } }\n}\n\
        fn main!() -> Unit { _ = f(Some(0), 1, 2, true)\n_ = f(None, 1, 2, false) }\n",
    );
}

fn alias_guard(guard: &str, nested: bool, initial: &str) -> String {
    let inner = format!(
        "match input {{ _ if {{ {guard} }} => {{ let r = own b\nlet n = peek(p)\ndrop_own move r\nyield n }}, _ => {{ let r = own a\nlet n = peek(p)\ndrop_own move r\nyield n }} }}"
    );
    let value = if nested {
        format!("match input {{ _ if flag => {inner}, _ => 0 }}")
    } else {
        inner
    };
    format!(
        "fn peek(_v: &i64) -> i64 = 0\nfn f(input: Opt[i64], a: i64, b: i64, flag: Bool) -> i64 {{\nlet mut p = {initial}\nreturn {value} }}\nfn main!() -> Unit {{ _ = f(Some(0), 1, 2, true)\n_ = f(None, 1, 2, false) }}\n"
    )
}

#[test]
fn conditional_aliases_survive_copies_and_nested_guard_search() {
    for nested in [false, true] {
        for guard in [
            "let mut q = p\nlet answer = if flag { q = &a\nyield true } else { q = &b\nyield false }\np = q\nyield answer",
            "p = if flag { yield &a } else { yield &b }\nyield flag",
            "yield if flag { p = &a\nyield true } else { p = &b\nyield false }",
        ] {
            incomplete(&alias_guard(
                guard,
                nested,
                "if flag { yield &a } else { yield &b }",
            ));
        }
    }
}

#[test]
fn overwriting_a_conditional_alias_restores_definite_referents() {
    for nested in [false, true] {
        let source = alias_guard("if flag { p = &b }\np = &a\nyield flag", nested, "&a")
            .replace("let r = own a", "let r = own b");
        checked(&source, &[]);
    }
}

#[test]
fn only_the_conditional_tuple_field_needs_outcome_specific_sources() {
    for nested in [false, true] {
        let guard =
            "let pair = if flag { yield (&a, &a) } else { yield (&a, &b) }\np = pair.0\nyield flag";
        checked(
            &alias_guard(guard, nested, "&a").replace("let r = own a", "let r = own b"),
            &[],
        );
        incomplete(&alias_guard(
            &guard.replace("pair.0", "pair.1"),
            nested,
            "if flag { yield &a } else { yield &b }",
        ));
    }
}

#[test]
fn a_nested_match_in_a_partial_guard_cannot_erase_a_choice_of_referent() {
    incomplete(
        "fn peek(_v: &i64) -> i64 = 0\n\
        fn f(input: Opt[i64], a: i64, b: i64, flag: Bool) -> i64 {\n\
        let mut p = if flag { yield &a } else { yield &b }\nreturn match input {\n\
        Some(0) if { yield match input { Some(_) if flag => { p = &a\nyield true }, _ => { p = &b\nyield false } } } => { let r = own b\nlet n = peek(p)\ndrop_own move r\nyield n },\n\
        Some(0) => { let r = own a\nlet n = peek(p)\ndrop_own move r\nyield n }, _ => 0 } }\n\
        fn main!() -> Unit { _ = f(Some(0), 1, 2, true)\n_ = f(Some(0), 1, 2, false) }\n",
    );
}

#[test]
fn a_shared_body_inside_a_live_guard_preserves_its_choice_of_referent() {
    incomplete(
        "fn peek(_v: &i64) -> i64 = 0
fn f(outer: Opt[i64], inner: Opt[i64], a: i64, b: i64, take: Bool, flag: Bool) -> i64 {
let mut q = if flag { yield &a } else { yield &b }
return match outer { Some(0) if { let mut p = &a
let step = match inner { Some(_) if { p = &b
yield take } => { return 7 }, _ => { q = p
yield 1 } }
yield flag } => { let r = own b
let n = peek(q)
drop_own move r
yield n }, _ => 0 } }
fn main!() -> Unit { _ = f(Some(0), Some(1), 1, 2, false, true)
_ = f(Some(0), None, 1, 2, false, true) }",
    );
}

#[test]
fn a_false_nested_guard_preserves_a_copied_enclosing_choice() {
    let source = alias_guard(
        "let q = if flag { yield &a } else { yield &b }\nlet n = match input { _ if { p = q\nyield flag } => { return 3 }, _ => 0 }\nyield flag",
        false,
        "if flag { yield &a } else { yield &b }",
    );
    incomplete(&source);
    incomplete(&source.replacen("match input { _ if", "match input { Some(0) if", 1));
}

#[test]
fn a_partial_payload_head_does_not_stand_for_its_whole_constructor() {
    incomplete(
        "fn f(input: Opt[i64], x: Own[i64], flag: Bool) -> i64 = match input {\n\
        Some(0) if { drop_own move x\nyield flag } => 0,\nSome(0) => 1,\n\
        Some(_) => { drop_own move x\nyield 2 }, None => { drop_own move x\nyield 3 } }\n\
        fn main!() -> Unit { let n = 0\n_ = f(Some(0), own n, false) }\n",
    );
}

#[test]
fn caught_exit_observes_the_search_state_at_the_question_mark() {
    let make = |absent: &str| {
        format!(
            "fn peek(_v: &i64) -> i64 = 0\n\
         fn f(input: Opt[i64], more: Opt[i64], a: i64, b: i64, flag: Bool) -> i64 {{\n\
         let mut p = &a\nlet caught: Opt[i64] = result {{\n\
         let n = match input {{ Some(_) if {{ p = &b\nyield flag }} => 1,\n\
         Some(_) => {{ let x = more?\nyield x }}, None => {absent} }}\nyield Some(n) }}\n\
         let r = own a\nlet v = peek(p)\ndrop_own move r\nreturn v }}\n\
         fn main!() -> Unit {{ _ = f(Some(1), None, 1, 2, false)\n_ = f(None, None, 1, 2, true) }}\n"
        )
    };
    checked(&make("{ p = &b\nyield 2 }"), &[]);
    checked(&make("2"), &["OWNERSHIP_USE_AFTER_DISCHARGE"]);
}

#[test]
fn nested_selected_bodies_preserve_the_parked_false_edge() {
    let source = "fn f(input: Opt[i64], other: Opt[i64], side: Own[i64], flag: Bool) -> i64 = match input {\n\
        Some(_) if { drop_own move side\nyield flag } => match other { Some(v) if flag => v, _ => 0 },\n\
        None => match other { Some(v) if flag => { drop_own move side\nyield v },\n\
        _ => { drop_own move side\nyield 0 } }, Some(_) => 2 }\n\
        fn main!() -> Unit { let a = 1\nlet b = 2\n_ = f(Some(0), Some(1), own a, false)\n_ = f(None, None, own b, true) }\n";
    checked(source, &[]);
    checked(
        &source.replace("Some(_) => 2", "Some(_) => { drop_own move side\nyield 2 }"),
        &["OWNERSHIP_DOUBLE_MOVE"],
    );
}

#[test]
fn shared_guards_and_bodies_count_each_written_operation_once() {
    for width in [4, 32] {
        let mut parameters = String::new();
        let mut arms = String::new();
        for i in 0..width {
            parameters.push_str(&format!(", a{i}: i64"));
            let pattern = if i % 2 == 0 { "Some(_)" } else { "_" };
            arms.push_str(&format!("{pattern} if {{ let t = own a{i}\ndrop_own move t\nyield flag }} => {{ let n = 1\nlet t = own n\ndrop_own move t\nyield 0 }},\n"));
        }
        let args = ", 1".repeat(width);
        let source = format!(
            "fn f(input: Opt[i64], flag: Bool{parameters}) -> i64 = match input {{ {arms}_ => 1 }}\nfn main!() -> Unit {{ _ = f(None, false{args}) }}\n"
        );
        assert_eq!(super::checked_operation_count(&source, "f"), 4 * width);
    }
}

#[test]
fn a_shared_guard_copies_each_constructors_tuple_referents() {
    let make = |slot: usize, body: &str| {
        format!(
            "fn peek(_v: &i64) -> i64 = 0\n\
         fn f(input: Opt[i64], inner: Opt[i64], a: i64, b: i64, flag: Bool) -> i64 {{\n\
         let mut p = (&a, &b)\nlet mut q = (&b, &a)\nreturn match input {{\n\
         Some(_) if {{ p = (&b, &a)\nq = (&a, &b)\nyield flag }} => 0,\n\
         _ if {{ p = q\nyield flag }} => {body},\n\
         None => {{ let r = own a\nlet n = peek(p.{slot})\ndrop_own move r\nyield n }},\n\
         Some(_) => {{ let r = own b\nlet n = peek(p.{slot})\ndrop_own move r\nyield n }} }}\n}}\n\
         fn main!() -> Unit {{ _ = f(Some(0), None, 1, 2, false)\n_ = f(None, Some(0), 1, 2, false) }}\n"
        )
    };
    for body in [
        "1",
        "match inner { Some(_) if { p = (&b, &b)\nyield flag } => 1, Some(_) => 2, None => 3 }",
    ] {
        checked(&make(0, body), &[]);
        checked(&make(1, body), &["OWNERSHIP_USE_AFTER_DISCHARGE"]);
    }
}

#[test]
fn shared_body_checks_the_join_not_two_separate_borrows() {
    let make = |final_arms: &str| {
        format!(
            "fn peek(_v: &i64) -> i64 = 0\n\
         fn f(input: Opt[i64], a: i64, b: i64, flag: Bool) -> i64 {{\n\
         let mut p = &a\nreturn match input {{\n\
         Some(_) if {{ p = &b\nlet r = own a\ndrop_own move r\nyield flag }} => 1,\n\
         None if {{ let r = own b\ndrop_own move r\nyield flag }} => 2,\n\
         {final_arms} }}\n}}\nfn main!() -> Unit {{ _ = f(Some(0), 1, 2, false)\n_ = f(None, 1, 2, false) }}\n"
        )
    };
    checked(&make("Some(_) => peek(p), None => peek(p)"), &[]);
    checked(&make("_ => peek(p)"), &["OWNERSHIP_USE_AFTER_DISCHARGE"]);
}

#[test]
fn a_false_guard_preserves_the_move_but_a_pattern_miss_skips_it() {
    let make = |fallthrough: &str, miss: &str| {
        format!(
            "fn take(root: Own[i64], choice: Bool) -> Bool {{ drop_own move root\nreturn choice }}\n\
         fn f(input: Opt[i64], root: Own[i64], flag: Bool) -> i64 = match input {{\n\
         Some(_) if take(move root, flag) => 0,\n\
             None => {miss},\nSome(_) => {fallthrough} }}\n\
             fn main!() -> Unit {{ let a = 1\nlet b = 2\nlet c = 3\n\
             _ = f(Some(0), own a, true)\n_ = f(Some(0), own b, false)\n_ = f(None, own c, false) }}\n"
        )
    };
    let release = "{ drop_own move root\nyield 1 }";
    checked(&make("2", release), &[]);
    let double = make(release, release);
    let errors = checked(&double, &["OWNERSHIP_DOUBLE_MOVE"]);
    let primary = &errors.diagnostics()[0];
    let span = primary
        .primary_span()
        .as_source()
        .unwrap()
        .file_span()
        .range();
    let primary_site = double.rfind("drop_own move root").unwrap();
    assert!(span.start().as_usize() <= primary_site && primary_site < span.end().as_usize());
    assert_eq!(
        double[span.start().as_usize()..span.end().as_usize()].trim(),
        "drop_own move root"
    );
    let witness = primary.related()[0].span().file_span().range();
    assert_eq!(
        double[witness.start().as_usize()..witness.end().as_usize()].trim(),
        "move root"
    );
    checked(&make("2", "1"), &["OWNERSHIP_PATH_UNBALANCED"]);
}
