use std::path::Path;
use systemprompt_bridge::gui::server_marketplace::hooks::{hook_items, list_hooks};

const SYSTEM_HOOKS_JSON: &str = r#"{
    "hooks": {
        "PreToolUse": [{"matcher": "*", "hooks": [
            {"type": "http", "url": "http://127.0.0.1:1/api/public/hooks/govern", "headers": {}, "allowedEnvVars": [], "timeout": 10}]}],
        "PostToolUse": [{"matcher": "*", "hooks": [{"type": "http", "url": "http://127.0.0.1:1/api/public/hooks/track", "headers": {}, "allowedEnvVars": [], "timeout": 10, "async": true, "event": "PostToolUse"}]}],
        "PostToolUseFailure": [{"matcher": "*", "hooks": [{"type": "http", "url": "http://127.0.0.1:1/api/public/hooks/track", "headers": {}, "allowedEnvVars": [], "timeout": 10, "async": true, "event": "PostToolUseFailure"}]}],
        "SessionStart": [{"matcher": "*", "hooks": [{"type": "http", "url": "http://127.0.0.1:1/api/public/hooks/track", "headers": {}, "allowedEnvVars": [], "timeout": 10, "async": true, "event": "SessionStart"}]}],
        "SessionEnd": [{"matcher": "*", "hooks": [{"type": "http", "url": "http://127.0.0.1:1/api/public/hooks/track", "headers": {}, "allowedEnvVars": [], "timeout": 10, "async": true, "event": "SessionEnd"}]}],
        "UserPromptSubmit": [{"matcher": "*", "hooks": [{"type": "http", "url": "http://127.0.0.1:1/api/public/hooks/track", "headers": {}, "allowedEnvVars": [], "timeout": 10, "async": true, "event": "UserPromptSubmit"}]}],
        "Stop": [{"matcher": "*", "hooks": [{"type": "http", "url": "http://127.0.0.1:1/api/public/hooks/track", "headers": {}, "allowedEnvVars": [], "timeout": 10, "async": true, "event": "Stop"}]}],
        "SubagentStart": [{"matcher": "*", "hooks": [{"type": "http", "url": "http://127.0.0.1:1/api/public/hooks/track", "headers": {}, "allowedEnvVars": [], "timeout": 10, "async": true, "event": "SubagentStart"}]}],
        "SubagentStop": [{"matcher": "*", "hooks": [{"type": "http", "url": "http://127.0.0.1:1/api/public/hooks/track", "headers": {}, "allowedEnvVars": [], "timeout": 10, "async": true, "event": "SubagentStop"}]}],
        "Notification": [{"matcher": "*", "hooks": [{"type": "http", "url": "http://127.0.0.1:1/api/public/hooks/track", "headers": {}, "allowedEnvVars": [], "timeout": 10, "async": true, "event": "Notification"}]}],
        "TaskCompleted": [{"matcher": "*", "hooks": [{"type": "http", "url": "http://127.0.0.1:1/api/public/hooks/track", "headers": {}, "allowedEnvVars": [], "timeout": 10, "async": true, "event": "TaskCompleted"}]}],
        "TeammateIdle": [{"matcher": "*", "hooks": [{"type": "http", "url": "http://127.0.0.1:1/api/public/hooks/track", "headers": {}, "allowedEnvVars": [], "timeout": 10, "async": true, "event": "TeammateIdle"}]}],
        "PermissionRequest": [{"matcher": "*", "hooks": [{"type": "http", "url": "http://127.0.0.1:1/api/public/hooks/track", "headers": {}, "allowedEnvVars": [], "timeout": 10, "async": true, "event": "PermissionRequest"}]}]
    }
}"#;

#[test]
fn missing_file_yields_empty() {
    assert!(list_hooks(Path::new("/var/empty/does-not-exist/hooks")).is_empty());
}

#[test]
fn unparseable_json_yields_empty() {
    assert!(hook_items(b"not json", Path::new("hooks.json")).is_empty());
}

#[test]
fn system_hooks_collapse_to_one_summary_row() {
    let items = hook_items(SYSTEM_HOOKS_JSON.as_bytes(), Path::new("hooks.json"));
    assert_eq!(items.len(), 1, "system govern/track collapse into one row");
    let row = serde_json::to_value(&items[0]).unwrap();
    assert_eq!(row["id"], "systemprompt-governance");
    let summary = row["summary"].as_str().unwrap_or_default();
    assert!(
        summary.contains("Governing PreToolUse"),
        "summary: {summary}"
    );
    assert!(summary.contains("tracking 12 events"), "summary: {summary}");
}

#[test]
fn user_command_hooks_get_their_own_rows() {
    let json = r#"{
        "hooks": {
            "PreToolUse": [{"matcher": "*", "hooks": [{"type": "http", "url": "http://127.0.0.1:1/api/public/hooks/govern", "headers": {}, "allowedEnvVars": [], "timeout": 10}]}],
            "PostToolUse": [{"matcher": "Bash", "hooks": [{"type": "command", "command": "echo hi", "timeout": 10, "event": "PostToolUse"}]}]
        }
    }"#;
    let items = hook_items(json.as_bytes(), Path::new("hooks.json"));
    assert_eq!(items.len(), 2, "system summary + one user row");
    assert_eq!(
        serde_json::to_value(&items[0]).unwrap()["id"],
        "systemprompt-governance"
    );
    let user = serde_json::to_value(&items[1]).unwrap();
    assert_eq!(user["name"], "PostToolUse (Bash)");
    assert_eq!(user["summary"], "echo hi");
}
