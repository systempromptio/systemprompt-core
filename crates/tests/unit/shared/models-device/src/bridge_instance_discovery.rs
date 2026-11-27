use systemprompt_models::bridge::host::HostKind;
use systemprompt_models::bridge::instance::{BridgeDiscovery, DISCOVERY_CONTRACT};

#[test]
fn decodes_the_document_and_ignores_unknown_keys() {
    let doc: BridgeDiscovery = serde_json::from_str(
        r#"{"contract":"1","device_link_path":"/bridge-auth/device-link","identity_path":"/api/public/bridge/whoami","branding":{"display_name":"Astound Digital","theme":"x"},"enabled_hosts":["claude-code","harness"],"future":{}}"#,
    )
    .unwrap();
    assert_eq!(doc.contract, DISCOVERY_CONTRACT);
    assert_eq!(
        doc.branding.display_name.as_deref(),
        Some("Astound Digital")
    );
    assert_eq!(doc.enabled_hosts, [HostKind::ClaudeCode, HostKind::Harness]);
}

#[test]
fn another_contract_is_incompatible() {
    let doc: BridgeDiscovery =
        serde_json::from_str(r#"{"contract":"2","device_link_path":"/x","identity_path":"/y"}"#)
            .unwrap();
    assert_ne!(doc.contract, DISCOVERY_CONTRACT);
    assert_eq!(doc.enabled_hosts, Vec::<HostKind>::new());
    let anonymous =
        serde_json::from_str::<BridgeDiscovery>(r#"{"contract":"2","device_link_path":"/x"}"#);
    assert!(anonymous.is_err(), "no identity_path: {anonymous:?}");
}
