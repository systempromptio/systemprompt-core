use sp_protocol::wire::discovery::Discovery;

#[test]
fn decodes_the_document_and_ignores_unknown_keys() {
    let doc: Discovery = serde_json::from_str(
        r#"{"contract":"1","device_link_path":"/bridge-auth/device-link","identity_path":"/api/public/bridge/whoami","branding":{"display_name":"Astound Digital","theme":"x"},"enabled_hosts":["claude-code","harness"],"future":{}}"#,
    )
    .unwrap();
    assert!(doc.is_compatible());
    assert_eq!(
        doc.branding.and_then(|b| b.display_name).as_deref(),
        Some("Astound Digital")
    );
    assert_eq!(doc.enabled_hosts, ["claude-code", "harness"]);
}

#[test]
fn another_contract_is_incompatible() {
    let doc: Discovery =
        serde_json::from_str(r#"{"contract":"2","device_link_path":"/x"}"#).unwrap();
    assert!(!doc.is_compatible());
    assert_eq!(doc.enabled_hosts, Vec::<String>::new());
}
