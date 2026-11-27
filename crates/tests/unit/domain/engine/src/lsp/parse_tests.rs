use std::path::PathBuf;

use serde_json::{Value, json};
use systemprompt_engine::lsp::Severity;
use systemprompt_engine::lsp::parse::{
    SaveMode, configuration_section, document_symbols, hover, locations, publish_diagnostics,
    server_capabilities, workspace_symbols,
};

fn file(name: &str) -> (String, PathBuf) {
    if cfg!(windows) {
        (
            format!("file:///C:/w/{name}"),
            PathBuf::from(format!(r"C:\w\{name}")),
        )
    } else {
        (
            format!("file:///w/{name}"),
            PathBuf::from(format!("/w/{name}")),
        )
    }
}

#[test]
fn reads_published_diagnostics() {
    let (uri, path) = file("a.rs");
    let params = json!({
        "uri": uri,
        "version": 3,
        "diagnostics": [
            {"range": {"start": {"line": 1, "character": 2}, "end": {"line": 1, "character": 5}},
             "severity": 2, "message": "unused", "source": "rustc", "code": "W1"},
            {"range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 1}},
             "message": "no severity", "code": 7},
            {"message": "no range"}
        ]
    });
    let published = publish_diagnostics(&params).unwrap();
    assert_eq!(published.path, path);
    assert_eq!(published.version, Some(3));
    assert_eq!(published.diagnostics.len(), 2);
    assert_eq!(published.diagnostics[0].severity, Severity::Warning);
    assert_eq!(published.diagnostics[1].severity, Severity::Error);
    assert_eq!(published.diagnostics[1].code.as_deref(), Some("7"));
}

#[test]
fn reads_locations_and_links() {
    let (uri, path) = file("b.ts");
    let span = json!({"start": {"line": 4, "character": 1}, "end": {"line": 4, "character": 9}});
    let single = json!({"uri": uri, "range": span});
    let link = json!({"targetUri": uri, "targetRange": span, "targetSelectionRange": span});
    assert_eq!(locations(&single).len(), 1);
    assert_eq!(locations(&json!([single, link]))[1].path, path);
    assert_eq!(
        locations(&Value::Null),
        Vec::<systemprompt_engine::lsp::Location>::new()
    );
}

#[test]
fn flattens_hover_contents() {
    let markup = json!({"contents": {"kind": "markdown", "value": "**x**"}});
    assert_eq!(hover(&markup).unwrap().contents, "**x**");
    let marked = json!({"contents": [{"language": "rust", "value": "fn x()"}, "docs", ""]});
    assert_eq!(
        hover(&marked).unwrap().contents,
        "```rust\nfn x()\n```\n\ndocs"
    );
    assert!(hover(&json!({"contents": ""})).is_none());
}

#[test]
fn flattens_symbol_trees() {
    let (_, path) = file("c.rs");
    let span = json!({"start": {"line": 0, "character": 0}, "end": {"line": 9, "character": 0}});
    let tree = json!([{
        "name": "Outer", "kind": 23, "range": span, "selectionRange": span,
        "children": [{"name": "inner", "kind": 6, "range": span, "selectionRange": span, "detail": "fn()"}]
    }]);
    let symbols = document_symbols(&tree, &path);
    assert_eq!(symbols.len(), 2);
    assert_eq!(symbols[1].container.as_deref(), Some("Outer"));
    assert_eq!(symbols[1].depth, 1);
    assert_eq!(symbols[1].kind.name(), "method");
}

#[test]
fn looks_up_configuration_sections() {
    let settings = json!({"python": {"analysis": {"typeCheckingMode": "basic"}}});
    assert_eq!(
        configuration_section(Some(&settings), Some("python.analysis")),
        json!({"typeCheckingMode": "basic"})
    );
    assert_eq!(
        configuration_section(Some(&settings), Some("python.missing")),
        Value::Null
    );
    assert_eq!(configuration_section(Some(&settings), None), settings);
    assert_eq!(configuration_section(None, Some("x")), Value::Null);
}

#[test]
fn reads_capabilities() {
    let result = json!({"capabilities": {
        "textDocumentSync": {"openClose": true, "change": 1, "save": {"includeText": false}},
        "hoverProvider": true, "definitionProvider": {"workDoneProgress": true}, "referencesProvider": false
    }});
    let caps = server_capabilities(&result);
    assert_eq!(caps.save, Some(SaveMode::Plain));
    assert!(caps.hover && caps.definition);
    assert!(!caps.references && !caps.document_symbol);
    assert_eq!(
        server_capabilities(&json!({"capabilities": {"textDocumentSync": 2}})).save,
        None
    );
}

#[test]
fn reads_workspace_symbols() {
    let (uri, path) = file("d.rs");
    let span = json!({"start": {"line": 2, "character": 0}, "end": {"line": 2, "character": 5}});
    let items = json!([
        {"name": "run", "kind": 12, "containerName": "app", "location": {"uri": uri, "range": span}},
        {"name": "no kind", "location": {"uri": uri, "range": span}}
    ]);
    let symbols = workspace_symbols(&items);
    assert_eq!(symbols.len(), 1);
    assert_eq!(symbols[0].path, path);
    assert_eq!(symbols[0].container.as_deref(), Some("app"));
    assert_eq!(symbols[0].range.start.line, 2);
    assert_eq!(
        workspace_symbols(&Value::Null),
        Vec::<systemprompt_engine::lsp::Symbol>::new()
    );
}
