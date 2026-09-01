use std::fs;
use std::path::PathBuf;

use std::sync::Arc;

use systemprompt_bridge::mcp_registry::{
    McpRegistrySlot, empty_slot, rehydrate_from_disk, snapshot,
};

fn metadata_dir(state_home: &std::path::Path) -> PathBuf {
    state_home.join("systemprompt-bridge").join("metadata")
}

fn sorted_keys(slot: &McpRegistrySlot) -> Vec<String> {
    let mut keys: Vec<String> = snapshot(slot).keys().cloned().collect();
    keys.sort();
    keys
}

#[test]
fn rehydrates_published_servers_from_fragment() {
    let state = tempfile::tempdir().unwrap();
    let meta = metadata_dir(state.path());
    fs::create_dir_all(&meta).unwrap();
    fs::write(
        meta.join("mcp-servers.json"),
        br#"[
            {"name":"My Server","url":"https://gw.example.com/mcp/one","headers":{"X-Tenant":"acme"}},
            {"name":"Second","url":"https://gw.example.com/mcp/two"}
        ]"#,
    )
    .unwrap();

    let slot: Arc<McpRegistrySlot> = empty_slot();
    temp_env::with_var("XDG_STATE_HOME", Some(state.path()), || {
        rehydrate_from_disk(&slot);
        let registry = snapshot(&slot);
        assert_eq!(registry.len(), 2);
        let first = registry.get("my-server").expect("normalized key present");
        assert_eq!(first.url.as_str(), "https://gw.example.com/mcp/one");
        assert_eq!(
            first.headers.get("X-Tenant").map(String::as_str),
            Some("acme")
        );
        let second = registry.get("second").expect("second server present");
        assert!(second.headers.is_empty());
    });
}

#[test]
fn missing_fragment_leaves_registry_untouched() {
    let state = tempfile::tempdir().unwrap();
    let slot: Arc<McpRegistrySlot> = empty_slot();
    temp_env::with_var("XDG_STATE_HOME", Some(state.path()), || {
        let before = sorted_keys(&slot);
        rehydrate_from_disk(&slot);
        assert_eq!(sorted_keys(&slot), before);
    });
}

#[test]
fn malformed_fragment_leaves_registry_untouched() {
    let state = tempfile::tempdir().unwrap();
    let meta = metadata_dir(state.path());
    fs::create_dir_all(&meta).unwrap();
    fs::write(meta.join("mcp-servers.json"), b"not json").unwrap();

    let slot: Arc<McpRegistrySlot> = empty_slot();
    temp_env::with_var("XDG_STATE_HOME", Some(state.path()), || {
        let before = sorted_keys(&slot);
        rehydrate_from_disk(&slot);
        assert_eq!(sorted_keys(&slot), before);
    });
}
