//! The manifest's `host_extensions`: which extensions an instance enables per
//! host.

use sp_protocol::host::HOST_ID;
use sp_protocol::wire::manifest::Manifest;

fn minimal(extra: &serde_json::Value) -> Manifest {
    let mut body = serde_json::json!({
        "user_id": "1ecce4e3-bc87-441d-b374-ea29bab88da8",
        "managed_mcp_servers": [],
        "plugins": []
    });
    if let (Some(body), Some(extra)) = (body.as_object_mut(), extra.as_object()) {
        body.extend(extra.clone());
    }
    serde_json::from_value(body).unwrap()
}

#[test]
fn host_extensions_are_read_per_host() {
    let manifest = minimal(&serde_json::json!({
        "host_extensions": { "harness": ["drive", "brain"], "other-host": ["slack"] }
    }));
    assert_eq!(
        manifest.extensions_for(HOST_ID),
        Some(&["drive".to_owned(), "brain".to_owned()][..])
    );
    assert_eq!(
        manifest.extensions_for("other-host"),
        Some(&["slack".to_owned()][..])
    );
    assert_eq!(manifest.extensions_for("unknown"), None);
}

#[test]
fn absent_host_extensions_are_empty() {
    let manifest = minimal(&serde_json::json!({}));
    assert!(manifest.host_extensions.is_empty());
    assert_eq!(manifest.extensions_for(HOST_ID), None);
}

#[test]
fn host_extensions_round_trip() {
    let manifest = minimal(&serde_json::json!({ "host_extensions": { "harness": [] } }));
    let encoded = serde_json::to_value(&manifest).unwrap();
    assert_eq!(
        encoded["host_extensions"],
        serde_json::json!({ "harness": [] })
    );
    assert_eq!(manifest.extensions_for(HOST_ID), Some(&[][..]));
}
