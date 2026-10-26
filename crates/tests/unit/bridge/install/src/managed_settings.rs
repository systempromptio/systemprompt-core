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

#[test]
fn a_blank_file_is_recognised_as_blank_and_a_document_is_not() {
    use systemprompt_bridge::claude_policy::is_blank_file;

    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("managed-settings.json");
    assert!(!is_blank_file(&path), "a missing file is absent, not blank");
    fs::write(&path, b"").expect("seed");
    assert!(is_blank_file(&path));
    fs::write(&path, b" \n").expect("seed");
    assert!(is_blank_file(&path));
    fs::write(&path, b"{}").expect("seed");
    assert!(!is_blank_file(&path));
}

// Why: the 0.51.0 field log read `io error in remove managed MCP policy: EOF
// while parsing a value at line 1 column 0` and named no file; every read
// failure must carry the path so the operator can find the offender.
#[cfg(unix)]
#[test]
fn an_unreadable_file_names_its_path_in_the_error() {
    use std::os::unix::fs::PermissionsExt;

    // skip-ok: root reads a mode-0o000 file, so there is no permission error to
    // observe
    if nix_is_root() {
        return;
    }
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("managed-settings.json");
    fs::write(&path, b"{}").expect("seed");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).expect("chmod");

    let err = stripped_settings(&path).expect_err("unreadable");
    assert_eq!(err.kind(), std::io::ErrorKind::PermissionDenied);
    assert!(err.to_string().contains("managed-settings.json"), "{err}");

    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).expect("restore");
}

#[cfg(unix)]
fn nix_is_root() -> bool {
    std::process::Command::new("id")
        .arg("-u")
        .output()
        .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).trim() == "0")
}

#[test]
fn an_allowlist_another_manager_deployed_is_left_in_place() {
    let seeded = json!({
        "allowManagedMcpServersOnly": true,
        "allowedMcpServers": [{ "serverUrl": "https://mcp.corp.example/tools" }],
    });
    assert!(strip(Some(&seeded)).is_none());
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("managed-settings.json");
    fs::write(&path, seeded.to_string()).expect("seed");
    assert!(systemprompt_bridge::claude_policy::holds_foreign_mcp_keys(
        &path
    ));
}

#[test]
fn an_allowlist_mixing_bridge_and_corporate_servers_is_not_the_bridges() {
    let seeded = json!({
        "allowManagedMcpServersOnly": true,
        "allowedMcpServers": [
            { "serverUrl": "http://127.0.0.1:1/mcp/x" },
            { "serverUrl": "https://mcp.corp.example/tools" }
        ],
    });
    assert!(strip(Some(&seeded)).is_none());
}

#[test]
fn a_lock_with_no_allowlist_is_not_attributed_to_the_bridge() {
    assert!(strip(Some(&json!({ "allowManagedMcpServersOnly": true }))).is_none());
}

fn mcp_author(body: &[u8]) -> Option<systemprompt_bridge::claude_policy::PolicyAuthor> {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("managed-mcp.json");
    fs::write(&path, body).expect("seed");
    systemprompt_bridge::claude_policy::managed_mcp_author(&path).expect("read")
}

#[test]
fn a_managed_mcp_file_naming_only_bridge_loopback_servers_is_the_bridges() {
    use systemprompt_bridge::claude_policy::PolicyAuthor;

    let body = json!({ "mcpServers": {
        "atlassian": { "type": "http", "url": "http://127.0.0.1:48123/mcp/atlassian" },
        "systemprompt": { "type": "http", "url": "http://localhost:48123/mcp/systemprompt" }
    }});
    assert_eq!(
        mcp_author(body.to_string().as_bytes()),
        Some(PolicyAuthor::Bridge)
    );
}

#[test]
fn any_other_managed_mcp_file_belongs_to_whoever_deployed_it() {
    use systemprompt_bridge::claude_policy::PolicyAuthor;

    let corporate = json!({ "mcpServers": {
        "corp": { "type": "http", "url": "https://mcp.corp.example/tools" }
    }});
    let mixed = json!({ "mcpServers": {
        "atlassian": { "type": "http", "url": "http://127.0.0.1:48123/mcp/atlassian" },
        "local": { "type": "stdio", "command": "corp-mcp" }
    }});
    let loopback_not_mcp = json!({ "mcpServers": {
        "dev": { "type": "http", "url": "http://127.0.0.1:9000/tools" }
    }});
    for body in [
        corporate,
        mixed,
        loopback_not_mcp,
        json!({ "mcpServers": {} }),
    ] {
        assert_eq!(
            mcp_author(body.to_string().as_bytes()),
            Some(PolicyAuthor::Foreign),
            "{body}"
        );
    }
    assert_eq!(mcp_author(b""), Some(PolicyAuthor::Foreign));
    assert_eq!(mcp_author(b"{ not json"), Some(PolicyAuthor::Foreign));
}

#[test]
fn a_missing_managed_mcp_file_has_no_author() {
    let dir = tempfile::tempdir().expect("tempdir");
    assert!(
        systemprompt_bridge::claude_policy::managed_mcp_author(
            &dir.path().join("managed-mcp.json")
        )
        .expect("read")
        .is_none()
    );
}
