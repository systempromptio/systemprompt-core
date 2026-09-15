//! `stamp_hooks_file` attributes emitted HTTP hooks to the host running them.

use std::path::Path;

use systemprompt_bridge::sync::apply::stamp_hooks_file;

const HOOKS: &str = r#"{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "http",
            "url": "http://127.0.0.1:1/api/public/hooks/govern?plugin_id=p",
            "headers": {
              "Authorization": "Bearer hook-token",
              "x-systemprompt-device-credential": "must-remove"
            },
            "allowedEnvVars": [],
            "timeout": 10
          }
        ]
      }
    ],
    "Stop": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "http",
            "url": "http://127.0.0.1:1/api/public/hooks/track?plugin_id=p",
            "headers": { "Authorization": "Bearer hook-token" },
            "allowedEnvVars": [],
            "timeout": 10,
            "async": true,
            "event": "Stop"
          },
          {
            "type": "command",
            "command": "echo hi",
            "timeout": 10,
            "event": "Stop"
          }
        ]
      }
    ]
  }
}"#;

fn write_fixture(dir: &Path) -> std::path::PathBuf {
    let path = dir.join("hooks").join("hooks.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, HOOKS).unwrap();
    path
}

fn http_entries(doc: &serde_json::Value) -> Vec<&serde_json::Value> {
    doc["hooks"]
        .as_object()
        .unwrap()
        .values()
        .flat_map(|groups| groups.as_array().unwrap())
        .flat_map(|group| group["hooks"].as_array().unwrap())
        .filter(|hook| hook["type"] == "http")
        .collect()
}

#[test]
fn every_http_entry_is_stamped_and_the_device_credential_is_dropped() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_fixture(dir.path());
    stamp_hooks_file(&path, "claude-code").unwrap();
    let doc: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let entries = http_entries(&doc);
    assert_eq!(entries.len(), 2, "{doc}");
    for entry in entries {
        assert_eq!(
            entry["headers"]["x-systemprompt-host"], "claude-code",
            "{doc}"
        );
        assert_eq!(entry["headers"]["Authorization"], "Bearer hook-token");
        assert!(
            entry["headers"]
                .get("x-systemprompt-device-credential")
                .is_none(),
            "{doc}"
        );
    }
    let raw = std::fs::read_to_string(&path).unwrap();
    assert!(!raw.contains("must-remove"));
}

#[test]
fn command_entries_are_left_alone() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_fixture(dir.path());
    stamp_hooks_file(&path, "claude-desktop").unwrap();
    let doc: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let command = &doc["hooks"]["Stop"][0]["hooks"][1];
    assert_eq!(command["type"], "command", "{doc}");
    assert_eq!(command["command"], "echo hi");
    assert!(command.get("headers").is_none(), "{doc}");
}

#[test]
fn restamping_for_another_host_replaces_the_stamp() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_fixture(dir.path());
    stamp_hooks_file(&path, "claude-code").unwrap();
    stamp_hooks_file(&path, "claude-desktop").unwrap();
    let doc: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    for entry in http_entries(&doc) {
        assert_eq!(entry["headers"]["x-systemprompt-host"], "claude-desktop");
    }
}

#[test]
fn a_missing_file_is_a_no_op() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("hooks").join("hooks.json");
    stamp_hooks_file(&path, "claude-code").unwrap();
    assert!(!path.exists());
}
