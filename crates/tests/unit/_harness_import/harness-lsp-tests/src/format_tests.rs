use std::path::PathBuf;

use harness_lsp::{Diagnostic, DiagnosticFormat, Position, Range, Severity, format_diagnostics};

fn diag(path: &str, line: u32, severity: Severity, message: &str) -> Diagnostic {
    Diagnostic {
        path: PathBuf::from(path),
        range: Range {
            start: Position { line, character: 4 },
            end: Position { line, character: 8 },
        },
        severity,
        message: message.to_owned(),
        source: None,
        code: None,
    }
}

fn options() -> DiagnosticFormat {
    DiagnosticFormat {
        base: Some(PathBuf::from("/w")),
        ..DiagnosticFormat::default()
    }
}

#[test]
fn empty_when_nothing_qualifies() {
    assert_eq!(format_diagnostics(&[], &options()), "");
    assert_eq!(
        format_diagnostics(&[diag("/w/a.rs", 0, Severity::Hint, "h")], &options()),
        ""
    );
}

#[test]
fn errors_first_with_code_and_source() {
    let mut error = diag(
        "/w/src/a.rs",
        9,
        Severity::Error,
        "mismatched types\n  expected u8",
    );
    error.code = Some("E0308".into());
    error.source = Some("rustc".into());
    let items = vec![
        diag("/w/src/a.rs", 1, Severity::Warning, "unused import"),
        error,
        diag("/w/b.rs", 0, Severity::Warning, "dead code"),
        diag("/w/b.rs", 0, Severity::Information, "note"),
    ];
    assert_eq!(
        format_diagnostics(&items, &options()),
        "src/a.rs\n  error 10:5 mismatched types expected u8 [E0308] (rustc)\n  warning 2:5 unused import\nb.rs\n  warning 1:5 dead code"
    );
}

#[test]
fn caps_lines_files_and_message_length() {
    let mut items: Vec<Diagnostic> = (0..5)
        .map(|i| diag("/w/a.rs", i, Severity::Error, "e"))
        .collect();
    items.push(diag("/w/b.rs", 0, Severity::Error, "e"));
    items.push(diag("/w/c.rs", 0, Severity::Warning, &"x".repeat(50)));
    let options = DiagnosticFormat {
        max_files: 2,
        max_per_file: 2,
        max_message_chars: 10,
        ..options()
    };
    let text = format_diagnostics(&items, &options);
    assert_eq!(
        text,
        "a.rs\n  error 1:5 e\n  error 2:5 e\n  ... 3 more in this file\nb.rs\n  error 1:5 e\n... 1 more file with diagnostics (0 errors)"
    );
}

#[test]
fn truncates_long_messages_with_an_ellipsis() {
    let options = DiagnosticFormat {
        max_message_chars: 10,
        ..options()
    };
    let items = [diag("/w/c.rs", 0, Severity::Warning, &"x".repeat(50))];
    assert_eq!(
        format_diagnostics(&items, &options),
        "c.rs\n  warning 1:5 xxxxxxx..."
    );
}

#[test]
fn shows_paths_relative_to_the_base_with_forward_slashes() {
    let items = [
        diag("/w/src/a.rs", 0, Severity::Error, "e"),
        diag("/x/a.rs", 0, Severity::Warning, "w"),
    ];
    assert_eq!(
        format_diagnostics(&items, &options()),
        "src/a.rs\n  error 1:5 e\n/x/a.rs\n  warning 1:5 w"
    );
}
