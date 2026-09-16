use std::fs;

use serde_json::{Value, json};
use systemprompt_bridge::claude_policy::stripped_settings;

fn strip(existing: Option<&Value>) -> Option<Value> {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("managed-settings.json");
    if let Some(doc) = existing {
        fs::write(&path, doc.to_string()).expect("seed");
    }
    stripped_settings(&path)
        .expect("strip")
        .map(|body| serde_json::from_str(&body).expect("json"))
}

// Why: `allowManagedMcpServersOnly` made Claude Desktop 1.44121+ deny Cowork's
// built-in workspace server (the sandbox bash tool) by permission rule, which
// broke every skill while the bridge showed green. The bridge must strip the
// lock wherever it finds one and never write it again.
#[test]
fn a_lock_left_by_an_older_install_is_stripped_and_other_keys_kept() {
    let seeded = json!({
        "allowManagedMcpServersOnly": true,
        "allowedMcpServers": [{ "serverUrl": "http://127.0.0.1:1/mcp/x" }],
        "allowAllClaudeAiMcps": true,
        "someOtherKey": "kept"
    });
    let doc = strip(Some(&seeded)).expect("changed");
    assert_eq!(doc, json!({ "someOtherKey": "kept" }));
}

#[test]
fn an_unlocked_file_is_left_alone() {
    assert!(strip(Some(&json!({ "someOtherKey": "kept" }))).is_none());
}

#[test]
fn a_missing_file_is_nothing_to_do() {
    assert!(strip(None).is_none());
}

fn strip_raw(bytes: &[u8]) -> Result<Option<String>, std::io::Error> {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("managed-settings.json");
    fs::write(&path, bytes).expect("seed");
    stripped_settings(&path)
}

// Why: a zero-byte policy file left by another installer must read as "nothing
// there" so removal proceeds; it once aborted the whole claude-code sync with
// a bare `EOF while parsing a value`.
#[test]
fn a_blank_file_is_nothing_to_do() {
    assert!(strip_raw(b"").expect("empty").is_none());
    assert!(strip_raw(b" \n\t\n").expect("whitespace").is_none());
}

#[test]
fn a_malformed_file_names_its_path_in_the_error() {
    let err = strip_raw(b"{ not json").expect_err("garbage");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    assert!(err.to_string().contains("managed-settings.json"), "{err}");
}

#[test]
fn a_non_object_document_names_its_path_in_the_error() {
    let err = strip_raw(b"[1, 2]").expect_err("array");
    assert!(err.to_string().contains("managed-settings.json"), "{err}");
    assert!(err.to_string().contains("not a JSON object"), "{err}");
}
