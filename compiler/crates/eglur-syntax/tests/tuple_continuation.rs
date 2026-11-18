use eglur_source::{FileId, SourceFile};
use eglur_syntax::{FrontendOutcome, SyntaxDiagnosticKind, parse};

#[test]
fn tuple_entries_require_their_own_group_for_leading_binary_continuation() {
    for operator in ["+", "-", "*", "/", "%", "==", "!=", "<", "&&", "||"] {
        for ending in ["\n", "\r\n", "\r", "/*\n*/"] {
            for expression in [
                format!("(left{ending}{operator} right, tail)"),
                format!("(tail, left{ending}{operator} right)"),
                format!("(left{ending}{operator} right,)"),
            ] {
                let text = format!("fn demo() -> Unit = {expression}\n");
                let source = SourceFile::new(FileId::new(0), text.as_bytes());
                let FrontendOutcome::Parsed(parsed) = parse(&source) else {
                    panic!("UTF-8 source must reach parsing");
                };
                assert_eq!(parsed.syntax().text(), text);
                assert_eq!(
                    parsed.diagnostics()[0].kind(),
                    SyntaxDiagnosticKind::ParseUnexpectedToken,
                    "{text}"
                );
            }
        }
    }
}

#[test]
fn tuple_missing_separators_report_unexpected_tokens() {
    for expression in [
        "(left\n -right, tail)",
        "(left\n -right,)",
        "(head, left\n -right)",
        "(left right, tail)",
        "(head, left right)",
    ] {
        let text = format!("fn demo() -> Unit = {expression}\n");
        let source = SourceFile::new(FileId::new(0), text.as_bytes());
        let FrontendOutcome::Parsed(parsed) = parse(&source) else {
            panic!("UTF-8 source must reach parsing");
        };
        assert_eq!(parsed.syntax().text(), text);
        let diagnostics: Vec<_> = parsed
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.kind())
            .collect();
        assert_eq!(
            diagnostics,
            [SyntaxDiagnosticKind::ParseUnexpectedToken],
            "{text}"
        );
    }
}

#[test]
fn tuples_without_a_closing_parenthesis_keep_the_delimiter_diagnostic() {
    let text = "fn demo() -> Unit = (left, tail\n";
    let source = SourceFile::new(FileId::new(0), text.as_bytes());
    let FrontendOutcome::Parsed(parsed) = parse(&source) else {
        panic!("UTF-8 source must reach parsing");
    };
    assert_eq!(parsed.syntax().text(), text);
    assert_eq!(
        parsed.diagnostics()[0].kind(),
        SyntaxDiagnosticKind::ParseUnclosedDelimiter
    );
}

#[test]
fn groups_nested_commas_and_trailing_operators_preserve_continuation() {
    for expression in [
        "(left\n - right)",
        "((left\n - right), tail)",
        "(tail, (left\n - right))",
        "(left -\n right, tail)",
        "(left\n .value, tail)",
        "(call(left, right)\n - tail)",
        "([left, right]\n - tail)",
        "(Record { first: left, second: right }\n - tail)",
        "(\"comma, inside a string\"\n - tail)",
        "(left /* comma, in a comment */\n - right)",
    ] {
        let text = format!("fn demo() -> Unit = {expression}\n");
        let source = SourceFile::new(FileId::new(0), text.as_bytes());
        let FrontendOutcome::Parsed(parsed) = parse(&source) else {
            panic!("UTF-8 source must reach parsing");
        };
        assert_eq!(parsed.syntax().text(), text);
        assert!(parsed.is_success(), "{text}: {:?}", parsed.diagnostics());
    }
}
