//! The manifest's `host_extensions`: which extensions an instance enables per
//! host.

use systemprompt_device::protocol::manifest::Manifest;
use systemprompt_models::bridge::host::HostKind;

fn minimal(extra: &serde_json::Value) -> Manifest {
    let mut body = serde_json::json!({
        "user_id": "1ecce4e3-bc87-441d-b374-ea29bab88da8",
        "manifest_version": "2026-11-01T09:12:31Z-000001a19661c117",
        "issued_at": "2026-11-01T09:12:31Z",
        "not_before": "2026-11-01T09:11:31Z",
        "revocations": [],
        "managed_mcp_servers": [],
        "plugins": []
    });
    if let (Some(body), Some(extra)) = (body.as_object_mut(), extra.as_object()) {
        body.extend(extra.clone());
    }
    Manifest::parse(&body.to_string()).unwrap()
}

fn extensions(manifest: &Manifest, host: HostKind) -> Option<Vec<String>> {
    manifest
        .extensions_for(host)
        .map(|ids| ids.iter().map(ToString::to_string).collect())
}

#[test]
fn host_extensions_are_read_per_host() {
    let manifest = minimal(&serde_json::json!({
        "host_extensions": { "harness": ["drive", "brain"], "claude-code": ["slack"] }
    }));
    assert_eq!(
        extensions(&manifest, HostKind::Harness),
        Some(vec!["drive".to_owned(), "brain".to_owned()])
    );
    assert_eq!(
        extensions(&manifest, HostKind::ClaudeCode),
        Some(vec!["slack".to_owned()])
    );
    assert_eq!(extensions(&manifest, HostKind::Hermes), None);
}

#[test]
fn an_unknown_host_key_is_skipped() {
    let mut body = serde_json::to_value(minimal(&serde_json::json!({})).signed()).unwrap();
    body["host_extensions"] = serde_json::json!({ "other-host": ["slack"] });
    let manifest = Manifest::parse(&body.to_string()).unwrap();
    assert!(manifest.extensions_for(HostKind::Harness).is_none());
}

#[test]
fn absent_host_extensions_are_empty() {
    let manifest = minimal(&serde_json::json!({}));
    assert!(manifest.host_extensions.is_empty());
    assert_eq!(extensions(&manifest, HostKind::Harness), None);
}

#[test]
fn host_extensions_round_trip() {
    let manifest = minimal(&serde_json::json!({ "host_extensions": { "harness": [] } }));
    let encoded = serde_json::to_value(manifest.signed()).unwrap();
    assert_eq!(
        encoded["host_extensions"],
        serde_json::json!({ "harness": [] })
    );
    assert_eq!(extensions(&manifest, HostKind::Harness), Some(Vec::new()));
}
