use super::{diagnostic_codes, match_arms::checked, session};
use eglur_diagnostics::DiagnosticSnapshot;
use eglur_driver::{SupportProfile, TrustedCatalog};

const IMBALANCE: &str = "OWNERSHIP_PATH_UNBALANCED";

#[test]
fn abrupt_guard_checks_written_uses_but_only_executed_exits() {
    let source = |later: &str| {
        format!(
            "fn test(_arg: i64) -> Bool = true\n\
         fn take(v: Opt[Own[i64]]) -> Bool {{ drop_own move v\nreturn true }}\n\
         fn choose(r: Opt[Own[i64]]) -> i64 = match move r {{\n\
         Some(x) => {{ drop_own move x\nyield 0 }},\n\
         whole if test({{ return 7 }}) => 1, rest if {later} => 2, last => 3 }}\n"
        )
    };
    checked(&source("true"), &[]);
    fails_at(
        &source("take(move rest)"),
        "OWNERSHIP_GUARD_CONSUME_FORBIDDEN",
        "move rest",
    );
    fails_at(
        &source("take(rest)"),
        "OWNERSHIP_GUARD_CONSUME_FORBIDDEN",
        "rest",
    );
    fails_at(
        &source("test({ return 8\nlet bad = take(move rest)\nyield 0 })"),
        "OWNERSHIP_GUARD_CONSUME_FORBIDDEN",
        "move rest",
    );
}

#[test]
fn stopped_first_argument_still_checks_later_guard_arguments() {
    let source = |second: &str| {
        format!(
            "fn test(_n: i64, _flag: Bool) -> Bool = true\n\
             fn take(v: Opt[Own[i64]]) -> Bool {{ drop_own move v\nreturn true }}\n\
             fn choose(r: Opt[Own[i64]]) -> i64 = match move r {{\n\
             Some(x) => {{ drop_own move x\nyield 0 }},\n\
             whole if test({{ return 7 }}, {second}) => 1, rest => 2 }}\n"
        )
    };
    checked(&source("true"), &[]);
    fails_at(
        &source("take(move whole)"),
        "OWNERSHIP_GUARD_CONSUME_FORBIDDEN",
        "move whole",
    );
}

fn fails_at(source: &str, code: &str, site: &str) -> DiagnosticSnapshot {
    let snapshot = checked(source, &[code]);
    let primary = snapshot.diagnostics()[0]
        .primary_span()
        .as_source()
        .unwrap()
        .file_span()
        .range();
    assert_eq!(
        source[primary.start().as_usize()..primary.end().as_usize()].trim(),
        site
    );
    snapshot
}

#[test]
fn pending_payload_and_selected_whole_have_different_lifetimes() {
    let guard = "{ if stop { return 7 }\nyield false }";
    for (pattern, selected, fallback, exit) in [
        (
            "whole @ None",
            "{ drop_own move whole\nyield 1 }",
            "None => 3, Some(x) => { drop_own move x\nyield 2 }",
            None,
        ),
        (
            "whole @ None",
            "{ return 9 }",
            "None => 3, Some(x) => { drop_own move x\nyield 2 }",
            Some("return 9"),
        ),
        (
            "Some(x)",
            "{ drop_own move x\nyield 1 }",
            "Some(x) => { drop_own move x\nyield 2 }, None => 3",
            Some("return 7"),
        ),
    ] {
        let source = format!(
            "fn choose(r: Opt[Own[i64]], stop: Bool) -> i64 = match move r {{\n{pattern} if {guard} => {selected}, {fallback} }}\n"
        );
        if let Some(exit) = exit {
            fails_at(&source, IMBALANCE, exit);
        } else {
            checked(&source, &[]);
        }
    }
}

#[test]
fn ordinary_guard_false_and_payload_discard_are_not_guard_exits() {
    let make = |guard: &str, pattern: &str| {
        format!(
            "fn check(r: Res[Own[i64], Str], flag: Bool) -> i64 = match move r {{\n\
         Ok(x) {guard} => {{ drop_own move x\nyield 1 }},\n{pattern} => 0 }}\n"
        )
    };
    checked(&make("", "_"), &[]);
    fails_at(&make("if flag", "_"), "OWNERSHIP_DISCARD_FORBIDDEN", "_");
    fails_at(
        &make("if flag", "Ok(..)").replace("=> 0 }", "=> 0, Err(e) => 2 }"),
        "OWNERSHIP_DISCARD_FORBIDDEN",
        "Ok(..)",
    );
}

#[test]
fn temporary_borrow_and_consume_in_a_guard_are_distinct() {
    let make = |guard: &str| {
        format!(
            "fn inspect(_v: &Own[i64]) -> Bool = true\n\
         fn take(v: Own[i64]) -> Bool {{ drop_own move v\nreturn true }}\n\
         fn choose(r: Opt[Own[i64]]) -> i64 = match move r {{\n\
         Some(x) if {guard} => {{ drop_own move x\nyield 1 }},\n\
         Some(x) => {{ drop_own move x\nyield 2 }}, None => 0 }}\n"
        )
    };
    checked(&make("inspect(&x)"), &[]);
    fails_at(
        &make("take(move x)"),
        "OWNERSHIP_GUARD_CONSUME_FORBIDDEN",
        "move x",
    );
    fails_at(&make("take(x)"), "OWNERSHIP_GUARD_CONSUME_FORBIDDEN", "x");
}

#[test]
fn a_named_carrier_still_requires_an_explicit_transfer() {
    let source = "fn choose(r: Opt[Own[i64]]) -> i64 = match r { None => 0, Some(x) => { drop_own move x\nyield 1 } }\n";
    fails_at(source, "OWNERSHIP_TRANSFER_REQUIRES_MOVE", "r");
}

#[test]
fn impossible_carrier_variants_neither_own_payloads_nor_need_arms() {
    checked(
        "fn choose(r: Opt[Own[Never]]) -> i64 = match move r { None => 1 }\n",
        &[],
    );
    checked(
        "fn choose(r: Res[Own[i64], Never]) -> i64 = match move r { Ok(x) => { drop_own move x\nyield 1 } }\n",
        &[],
    );
    checked(
        "fn choose(r: Opt[Own[Never]], flag: Bool) -> i64 = match move r { _ if { if flag { return 7 }\nyield false } => 1, _ => 2 }\n",
        &[],
    );

    let catalog = TrustedCatalog::first_slice().unwrap();
    for source in [
        "fn choose(r: Opt[Own[Never]]) -> i64 = match move r { None => 1, Some(x) => { drop_own move x\nyield 2 } }\n",
        "fn choose(r: Res[Own[i64], Never]) -> i64 = match move r { Err(e) => 1, Ok(x) => { drop_own move x\nyield 2 } }\n",
        "fn choose(r: Res[Own[Never], Never]) -> i64 = match move r { _ => 1 }\n",
    ] {
        let mut compiler = session(source);
        compiler
            .resolve(&SupportProfile::semantic_core_v1(), Some(&catalog))
            .unwrap();
        assert_eq!(
            diagnostic_codes(&compiler.into_diagnostics(None)),
            ["MATCH_UNREACHABLE_ARM"],
            "{source}"
        );
    }
}

#[test]
fn a_question_mark_after_an_ok_arm_depends_on_unconditional_coverage() {
    let route = |earlier_guard: &str| {
        format!(
            "fn route(r: Res[Own[i64], Str], gate: Res[Bool, Str], flag: Bool) -> Res[i64, Str] = result {{\n\
         let n: i64 = match move r {{ Ok(x) {earlier_guard} => {{ drop_own move x\nyield 1 }},\n\
         whole if gate? => {{ drop_own move whole\nyield 2 }},\n\
         rest => {{ drop_own move rest\nyield 3 }} }}\nyield Ok(n) }}\n"
        )
    };
    checked(&route(""), &[]);
    fails_at(&route("if flag"), IMBALANCE, "gate?");
}

#[test]
fn checked_fault_only_leaves_a_match_when_its_target_is_outside() {
    let source = |guard: &str| {
        format!(
            "fn choose(r: Opt[Own[i64]], n: i8) -> Res[i64, NumericError] = checked {{\n\
         yield match move r {{ Some(x) if {guard} => {{ drop_own move x\nyield 1 }},\n\
         rest => {{ drop_own move rest\nyield 2 }} }} }}\n"
        )
    };
    checked(
        &source("{ let v: Res[i8, NumericError] = checked { yield n + n }\nyield false }"),
        &[],
    );
    let crossing = source("{ let v: i8 = n + n\nyield false }");
    let snapshot = fails_at(&crossing, IMBALANCE, "n + n");
    let [witness] = snapshot.diagnostics()[0].related() else {
        panic!("missing carrier witness");
    };
    let span = witness.span().file_span().range();
    assert_eq!(
        crossing[span.start().as_usize()..span.end().as_usize()].trim(),
        "move r"
    );
}

#[test]
fn returning_from_an_empty_inner_guard_still_checks_its_outer_carrier() {
    let source = concat!(
        "fn empty() -> Opt[Own[i64]] = None\n",
        "fn choose(r: Res[Own[i64], Str], stop: Bool) -> Bool = match move r {\n",
        "Ok(x) if match empty() { None if { if stop { return true }\nyield true } => false,\n",
        "rest => { drop_own move rest\nyield false } } => { drop_own move x\nyield true },\n",
        "rest => { drop_own move rest\nyield false } }\n",
    );
    let snapshot = fails_at(source, IMBALANCE, "return true");
    let [witness] = snapshot.diagnostics()[0].related() else {
        panic!("missing outer carrier witness");
    };
    let span = witness.span().file_span().range();
    assert_eq!(
        source[span.start().as_usize()..span.end().as_usize()].trim(),
        "move r"
    );
}
