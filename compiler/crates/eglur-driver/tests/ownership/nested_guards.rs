use super::{match_arms::checked, ordered_guards::incomplete};

const SELECTED: &str =
    "fn f(outer: Opt[i64], inner: Opt[i64], side: Own[i64], take_outer: Bool, take_inner: Bool) -> i64 = match outer {
    Some(_) if take_outer => match inner {
        Some(_) if { drop_own move side
yield take_inner } => 1,
        Some(_) => 2,
        None => { drop_own move side
yield 3 } },
    _ => { drop_own move side
yield 4 } }
fn main!() -> Unit { let a = 1
_ = f(Some(0), Some(1), own a, true, false)
let b = 2
_ = f(Some(0), None, own b, true, true)
let c = 3
_ = f(None, Some(1), own c, false, false) }";

#[test]
fn nested_false_edges_and_outer_misses_keep_their_own_consumes() {
    checked(SELECTED, &[]);
    let double = SELECTED.replace("Some(_) => 2", "Some(_) => { drop_own move side\nyield 2 }");
    let snapshot = checked(&double, &["OWNERSHIP_DOUBLE_MOVE"]);
    let span = snapshot.diagnostics()[0]
        .primary_span()
        .as_source()
        .unwrap()
        .file_span()
        .range();
    let second_move = double.find("Some(_) => {").unwrap();
    assert!(
        span.start().as_usize() >= second_move
            && span.end().as_usize() <= double.find("yield 2").unwrap()
    );
    let [first] = snapshot.diagnostics()[0].related() else {
        panic!("expected the original consume");
    };
    let first = first.span().file_span().range();
    assert!(first.start().as_usize() <= double.find("move side").unwrap());
    assert!(
        first.end().as_usize() > double.find("move side").unwrap()
            && first.end().as_usize() < second_move
    );
    for missing in ["yield 3", "yield 4"] {
        checked(
            &SELECTED.replace(&format!("drop_own move side\n{missing}"), missing),
            &["OWNERSHIP_PATH_UNBALANCED"],
        );
    }
}

#[test]
fn an_inner_move_points_back_to_the_consuming_outer_guard_or_body() {
    let inner = "match inner { Some(_) if { drop_own move side\nyield take_inner } => 1, _ => 2 }";
    for arms in [
        format!(
            "Some(_) if {{ drop_own move side\nyield take_outer }} => {inner}, Some(_) => 3, None => {{ drop_own move side\nyield 4 }}"
        ),
        format!(
            "Some(_) if take_outer => {{ drop_own move side\nyield {inner} }}, _ => {{ drop_own move side\nyield 4 }}"
        ),
    ] {
        let source = format!("fn f(outer: Opt[i64], inner: Opt[i64], side: Own[i64], take_outer: Bool, take_inner: Bool) -> i64 =
match outer {{ {arms} }}
fn main!() -> Unit {{ let a = 1\n_ = f(Some(0), Some(1), own a, true, false) }}");
        checked(
            &source.replace("drop_own move side\nyield take_inner", "yield take_inner"),
            &[],
        );
        let snapshot = checked(&source, &["OWNERSHIP_DOUBLE_MOVE"]);
        let diagnostic = &snapshot.diagnostics()[0];
        let second = diagnostic
            .primary_span()
            .as_source()
            .unwrap()
            .file_span()
            .range();
        let [first] = diagnostic.related() else {
            panic!("expected the enclosing consume")
        };
        let first = first.span().file_span().range();
        let enclosing = source.find("move side").unwrap();
        let nested = source.find(inner).unwrap() + inner.find("move side").unwrap();
        assert!(first.start().as_usize() <= enclosing && enclosing < first.end().as_usize());
        assert!(second.start().as_usize() <= nested && nested < second.end().as_usize());
    }
}

#[test]
fn residual_wildcard_can_select_one_constructor_and_resume_its_body() {
    let source = "fn f(outer: Opt[i64], inner: Opt[i64], a: i64, b: i64, flag: Bool) -> i64 {
let mut p = &a
return match outer {
    Some(_) if flag => 0, Some(_) => 1,
    _ => { let n = match inner { Some(_) if { p = &b
yield flag } => 2, Some(_) => 3, None => { p = &b
yield 4 } }
let r = own a
let v = peek(p)
drop_own move r
yield v } } }
fn peek(_v: &i64) -> i64 = 0
fn main!() -> Unit { _ = f(None, None, 1, 2, false) }";
    checked(source, &[]);
    checked(
        &source.replace("let r = own a", "let r = own b"),
        &["OWNERSHIP_USE_AFTER_DISCHARGE"],
    );
}

#[test]
fn a_shared_body_and_its_sequential_spelling_borrow_the_same_roots() {
    let source = |nested: bool, copy: bool, consumed: &str| {
        let (before, referent) = if copy {
            ("let saved = p\n", "saved")
        } else {
            ("", "p")
        };
        let inner = format!(
            "match inner {{ Some(_) if {{ q = {referent}
yield flag }} => 1, Some(_) => {{ let r = own {consumed}
let n = peek(q)
drop_own move r
yield n }}, None => 2 }}"
        );
        let body = if nested {
            format!(
                "return match outer {{ Some(_) if {{ p = &b
yield early }} => 0, _ => {{ {before}yield {inner} }} }}"
            )
        } else {
            format!(
                "let step: Unit = match outer {{ Some(_) => {{ p = &b
if early {{ return 0 }}
yield () }}, None => () }}
{before}return {inner}"
            )
        };
        format!(
            "fn peek(_v: &i64) -> i64 = 0
fn f(outer: Opt[i64], inner: Opt[i64], a: i64, b: i64, c: i64, early: Bool, flag: Bool) -> i64 {{
let mut p = &a
let mut q = &a
{body} }}
fn main!() -> Unit {{ _ = f(None, Some(1), 1, 2, 3, false, false)
_ = f(Some(0), Some(1), 1, 2, 3, false, false) }}"
        )
    };

    for copy in [false, true] {
        for nested in [false, true] {
            checked(&source(nested, copy, "c"), &[]);
            for consumed in ["a", "b"] {
                let source = source(nested, copy, consumed);
                let snapshot = checked(&source, &["OWNERSHIP_USE_AFTER_DISCHARGE"]);
                let span = snapshot.diagnostics()[0]
                    .primary_span()
                    .as_source()
                    .unwrap()
                    .file_span()
                    .range();
                assert_eq!(
                    span.start().as_usize(),
                    source.find("peek(q)").unwrap() + "peek(".len()
                );
                assert_eq!(&source[span.start().as_usize()..span.end().as_usize()], "q");
            }
        }
    }
}

#[test]
fn an_unchanged_assignment_stays_unchanged_across_a_nested_match() {
    let source = "fn f(o: Opt[i64], i: Opt[i64], a: i64, b: i64, flag: Bool) -> i64 {
let mut p = &a
let mut q = &a
return match o { Some(_) if { p = &b
q = &b
yield flag } => 0, _ => {
let first = match i { Some(_) if flag => { q = p
yield 1 }, _ => 2 }
yield match i { Some(0) if { q = p
yield flag } => 3, _ => 4 } } } }
fn main!() -> Unit { _ = f(None, Some(0), 1, 2, false) }";
    checked(source, &[]);
}

#[test]
fn a_payload_entered_on_selection_remains_live_until_a_real_transfer() {
    let source =
        "fn f(outer: Opt[Own[i64]], inner: Opt[i64], take_outer: Bool, take_inner: Bool) -> i64 = match move outer {
Some(side) if take_outer => match inner {
    Some(_) if { drop_own move side
yield take_inner } => { return 1 }, Some(_) => 2,
    None => { drop_own move side
return 3 } },
rest => { drop_own move rest
yield 4 } }
fn main!() -> Unit { let a = 1
_ = f(Some(own a), None, true, false) }";
    checked(source, &[]);
    let missing = source.replace("drop_own move side\nreturn 3", "return 3");
    let snapshot = checked(&missing, &["OWNERSHIP_PATH_UNBALANCED"]);
    let span = snapshot.diagnostics()[0]
        .primary_span()
        .as_source()
        .unwrap()
        .file_span()
        .range();
    assert_eq!(
        missing[span.start().as_usize()..span.end().as_usize()].trim(),
        "return 3"
    );
}

#[test]
fn a_collector_between_searches_restores_the_state_at_the_fault() {
    for (ty, block, fault, result) in [
        ("Opt[i64]", "result", "more?", "Some(n)"),
        ("Res[i64, NumericError]", "checked", "small + small", "n"),
    ] {
        let source = format!("fn peek(_v: &i64) -> i64 = 0
fn f(outer: Opt[i64], inner: Opt[i64], more: Opt[i64], small: i8, a: i64, b: i64, take_outer: Bool, take_inner: Bool) -> i64 {{
let mut p = &a
let mut q = &a
return match outer {{ Some(_) if take_outer => {{
    p = &b
    let caught: {ty} = {block} {{ let n = match inner {{
        Some(_) if {{ q = p
yield take_inner }} => 1,
        Some(_) => {{ q = p
let t = {fault}
q = p
yield 2 }},
        None => {{ q = p
yield 3 }} }}
yield {result} }}
    let r = own a
let v = peek(q)
drop_own move r
yield v }}, _ => 0 }} }}
fn main!() -> Unit {{ _ = f(Some(0), Some(1), None, 127, 1, 2, true, false) }}");
        checked(&source, &[]);
        checked(
            &source.replace("q = p\nlet t", "q = &a\nlet t"),
            &["OWNERSHIP_USE_AFTER_DISCHARGE"],
        );
    }
}

#[test]
fn a_caught_failure_in_a_shared_body_keeps_both_possible_referents() {
    let source = "fn peek(_v: &i64) -> i64 = 0
fn f(outer: Res[i64, i64], inner: Opt[i64], more: Opt[i64], a: i64, b: i64, c: i64, spare: i64, flag: Bool) -> i64 {
let mut p = &a
let mut q = &c
return match outer { Ok(_) if { p = &b
yield flag } => 0, _ => {
let caught: Opt[i64] = result { let n = match inner { Some(_) if { q = p
yield flag } => 1, Some(_) => { let v = more?
yield v }, None => 2 }
q = &c
yield Some(n) }
let r = own spare
let n = peek(q)
drop_own move r
yield n } } }
fn main!() -> Unit { _ = f(Ok(0), Some(1), None, 1, 2, 3, 4, false)
_ = f(Err(0), Some(1), None, 1, 2, 3, 4, false) }";
    checked(source, &[]);
    for consumed in ["a", "b"] {
        let source = source.replace("own spare", &format!("own {consumed}"));
        checked(&source, &["OWNERSHIP_USE_AFTER_DISCHARGE"]);
        checked(&source.replace("let v = more?", "let v = 1"), &[]);
    }
}

#[test]
fn an_independent_arm_inside_a_guard_cannot_start_another_ordered_search() {
    let partial = "match mid { Some(0) => { let mut q = &a
yield match inner { Some(_) if { q = &b
yield flag } => true, Some(_) => { let r = own a
let n = peek(q)
drop_own move r
yield false }, None => false } }, _ => false }";
    let make = |outer: &str| {
        format!(
            "fn peek(_v: &i64) -> i64 = 0
fn f(outer: Opt[i64], mid: Opt[i64], inner: Opt[i64], a: i64, b: i64, flag: Bool) -> Bool =
match outer {{ {outer}, _ => false }}
fn main!() -> Unit {{ _ = f(Some(1), Some(0), None, 1, 2, true) }}"
        )
    };
    incomplete(&make(&format!("Some(_) if {partial} => true")));
    checked(&make(&format!("Some(_) if flag => {partial}")), &[]);
}

#[test]
fn nested_matches_count_operations_where_they_are_written() {
    for depth in [2, 8] {
        let mut body = String::from(
            "match inner { Some(_) if { drop_own move side\nyield take_inner } => 1, Some(_) => 2, None => { drop_own move side\nyield 3 } }",
        );
        for _ in 0..depth {
            body = format!(
                "match outer {{ Some(_) if {{ let n = 0\nlet t = own n\ndrop_own move t\nyield take_outer }} => {body}, _ => {{ drop_own move side\nyield 4 }} }}"
            );
        }
        let source = format!(
            "fn f(outer: Opt[i64], inner: Opt[i64], side: Own[i64], take_outer: Bool, take_inner: Bool) -> i64 = {body}\nfn main!() -> Unit {{ let n = 3\n_ = f(Some(0), Some(1), own n, true, false) }}"
        );
        assert_eq!(super::checked_operation_count(&source, "f"), 3 * depth + 2);
    }
}
