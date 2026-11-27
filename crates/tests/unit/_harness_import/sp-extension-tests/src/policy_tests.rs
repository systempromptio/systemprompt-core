//! Enablement read from the manifest, failing closed.

use sp_extension::{ExtensionId, ExtensionPolicy};
use sp_protocol::host::HOST_ID;
use sp_protocol::wire::manifest::Manifest;

use crate::support::id;

fn manifest(extra: &serde_json::Value) -> Manifest {
    let mut body = serde_json::json!({
        "user_id": "1ecce4e3-bc87-441d-b374-ea29bab88da8",
        "managed_mcp_servers": [],
        "plugins": [],
        "enabled_hosts": [HOST_ID]
    });
    if let (Some(body), Some(extra)) = (body.as_object_mut(), extra.as_object()) {
        body.extend(extra.clone());
    }
    serde_json::from_value(body).unwrap()
}

#[test]
fn the_hosts_entry_enables_its_extensions() {
    let enabled = ExtensionPolicy::enabled(&manifest(&serde_json::json!({
        "host_extensions": { "harness": ["drive", "brain"], "elsewhere": ["slack"] }
    })))
    .unwrap();
    assert!(enabled.contains(&id("drive")));
    assert!(enabled.contains(&id("brain")));
    assert!(!enabled.contains(&id("slack")));
    assert_eq!(enabled.len(), 2);
}

#[test]
fn no_entry_for_the_host_enables_nothing() {
    let absent = manifest(&serde_json::json!({}));
    assert_eq!(ExtensionPolicy::enabled(&absent), None);
    let other = manifest(&serde_json::json!({ "host_extensions": { "elsewhere": ["drive"] } }));
    assert_eq!(ExtensionPolicy::enabled(&other), None);
}

#[test]
fn a_host_the_manifest_does_not_enable_gets_nothing() {
    let disabled = manifest(&serde_json::json!({
        "enabled_hosts": ["elsewhere"],
        "host_extensions": { "harness": ["drive"] }
    }));
    assert_eq!(ExtensionPolicy::enabled(&disabled), None);
}

#[test]
fn a_malformed_id_is_not_enabled_and_the_rest_are() {
    let enabled = ExtensionPolicy::enabled_for(
        &manifest(&serde_json::json!({ "host_extensions": { "harness": ["Drive!", "brain"] } })),
        HOST_ID,
    )
    .unwrap();
    assert_eq!(
        enabled.iter().map(ExtensionId::as_str).collect::<Vec<_>>(),
        ["brain"]
    );
}
