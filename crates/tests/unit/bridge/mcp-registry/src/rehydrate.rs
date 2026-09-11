use std::fs;
use std::path::PathBuf;

use std::sync::Arc;

use systemprompt_bridge::mcp_registry::{
    McpRegistrySlot, empty_slot, rehydrate_from_disk, same_origin, snapshot,
};
use systemprompt_identifiers::ValidatedUrl;

const GATEWAY: &str = "https://gw.example.com";

fn gateway() -> ValidatedUrl {
    ValidatedUrl::new(GATEWAY)
}

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
        br#"{"gateway":"https://gw.example.com","servers":[
            {"name":"My Server","url":"https://gw.example.com/mcp/one","headers":{"X-Tenant":"acme"}},
            {"name":"Second","url":"https://gw.example.com/mcp/two"}
        ]}"#,
    )
    .unwrap();

    let slot: Arc<McpRegistrySlot> = empty_slot();
    temp_env::with_var("XDG_STATE_HOME", Some(state.path()), || {
        rehydrate_from_disk(&slot, &gateway()).expect("rehydrate reads the on-disk fragment");
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
        rehydrate_from_disk(&slot, &gateway()).expect("an absent fragment is not an error");
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
        let err = rehydrate_from_disk(&slot, &gateway())
            .expect_err("a malformed fragment is reported, not silently swallowed");
        assert!(err.to_string().contains("mcp-servers.json"), "{err}");
        assert_eq!(sorted_keys(&slot), before);
    });
}

// Why: a switched account used to forward its fresh token to the previous
// gateway's MCP upstreams, which answered 401 and signed the bridge out.
#[test]
fn a_fragment_written_for_another_gateway_is_not_rehydrated() {
    let state = tempfile::tempdir().unwrap();
    let meta = metadata_dir(state.path());
    fs::create_dir_all(&meta).unwrap();
    fs::write(
        meta.join("mcp-servers.json"),
        br#"{"gateway":"http://localhost:8080","servers":[
            {"name":"Old Server","url":"http://127.0.0.1:8080/api/v1/mcp/old/mcp"}
        ]}"#,
    )
    .unwrap();

    let slot: Arc<McpRegistrySlot> = empty_slot();
    temp_env::with_var("XDG_STATE_HOME", Some(state.path()), || {
        rehydrate_from_disk(&slot, &gateway()).expect("a foreign fragment is skipped, not an error");
        assert!(
            sorted_keys(&slot).is_empty(),
            "servers delivered by another gateway never enter this gateway's registry"
        );
    });
}

#[test]
fn a_pre_stamp_array_fragment_is_ignored_until_the_next_sync() {
    let state = tempfile::tempdir().unwrap();
    let meta = metadata_dir(state.path());
    fs::create_dir_all(&meta).unwrap();
    fs::write(
        meta.join("mcp-servers.json"),
        br#"[{"name":"Legacy","url":"https://gw.example.com/mcp/legacy"}]"#,
    )
    .unwrap();

    let slot: Arc<McpRegistrySlot> = empty_slot();
    temp_env::with_var("XDG_STATE_HOME", Some(state.path()), || {
        rehydrate_from_disk(&slot, &gateway()).expect("an unstamped fragment is not an error");
        assert!(
            sorted_keys(&slot).is_empty(),
            "an unstamped fragment cannot prove which gateway it came from"
        );
    });
}

#[test]
fn same_origin_ignores_trailing_slash_and_host_case() {
    assert!(same_origin(
        &ValidatedUrl::new("https://GW.example.com/"),
        &ValidatedUrl::new("https://gw.example.com")
    ));
    assert!(!same_origin(
        &ValidatedUrl::new("http://localhost:8080"),
        &ValidatedUrl::new("https://gw.example.com")
    ));
    assert!(!same_origin(
        &ValidatedUrl::new("https://gw.example.com:8443"),
        &ValidatedUrl::new("https://gw.example.com")
    ));
}
