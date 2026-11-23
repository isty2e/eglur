use super::*;

#[test]
fn a_constructor_miss_bypasses_even_a_guard_that_would_return_from_the_function() {
    for (args, number, selected) in [
        ("Some(1), true, false", "1", Some("selected")),
        ("Some(1), false, false", "2", Some("fallthrough")),
        ("None, false, true", "3", Some("miss")),
        ("Some(1), false, true", "99", None),
    ] {
        let source = format!(
            "import std::test::emit!\n\
             fn mark!(label: Str, n: i64) -> i64 {{ emit!(label)\nyield n }}\n\
             fn choose!(input: Opt[i64], flag: Bool, stop: Bool) -> i64 = match input {{\n\
               Some(_) if {{ if stop {{ return 99 }}\nyield flag }} => mark!(\"selected\", 1),\n\
               None => mark!(\"miss\", 3), Some(_) => mark!(\"fallthrough\", 2) }}\n\
             fn main!() -> Res[Str, Str] = Ok(to_str(choose!({args})))\n"
        );
        let (program, catalog, profile) = compile_single(&source);
        let result = completed(&program, &catalog, &profile);
        assert_eq!(ok_str(result.value()), number);
        let events = selected.map(|label| eglur_driver::RuntimeEvent::Emitted(label.into()));
        assert_eq!(result.events(), events.as_slice());
    }
}

#[test]
fn guarded_match_evaluates_the_scrutinee_once_and_only_the_selected_arm() {
    for (guard, expected, event) in [("true", "7", "one"), ("false", "9", "fallback")] {
        let source = format!(
            "import std::test::emit!\nfn mark!(label: Str, value: i64) -> i64 {{ emit!(label)\nyield value }}\nfn main!() -> Res[Str, Str] = Ok(to_str(match mark!(\"scrutinee\", 1) {{ 0 => mark!(\"zero\", 0), 1 if {guard} => mark!(\"one\", 7), _ => mark!(\"fallback\", 9) }}))\n"
        );
        let (program, catalog, profile) = compile_single(&source);
        let result = completed(&program, &catalog, &profile);
        assert_eq!(ok_str(result.value()), expected);
        assert_eq!(
            result.events(),
            [
                eglur_driver::RuntimeEvent::Emitted("scrutinee".into()),
                eglur_driver::RuntimeEvent::Emitted(event.into())
            ]
        );
    }
}
