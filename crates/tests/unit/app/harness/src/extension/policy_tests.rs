//! Enablement read from the manifest, failing closed.

use systemprompt_device::protocol::manifest::Manifest;
use systemprompt_harness::extension::{ExtensionId, ExtensionPolicy};
use systemprompt_models::bridge::host::HostKind;

use crate::extension::support::id;

fn manifest(extra: &serde_json::Value) -> Manifest {
    let mut body = serde_json::json!({
        "user_id": "1ecce4e3-bc87-441d-b374-ea29bab88da8",
        "manifest_version": "2026-11-01T09:12:31Z-000001a19661c117",
        "issued_at": "2026-11-01T09:12:31Z",
        "not_before": "2026-11-01T09:11:31Z",
        "revocations": [],
        "managed_mcp_servers": [],
        "plugins": [],
        "enabled_hosts": [HostKind::Harness.as_str()]
    });
    if let (Some(body), Some(extra)) = (body.as_object_mut(), extra.as_object()) {
        body.extend(extra.clone());
    }
    Manifest::parse(&body.to_string()).unwrap()
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
        HostKind::Harness,
    )
    .unwrap();
    assert_eq!(
        enabled.iter().map(ExtensionId::as_str).collect::<Vec<_>>(),
        ["brain"]
    );
}
