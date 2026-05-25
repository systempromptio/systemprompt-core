use systemprompt_bridge::config::Config;
use systemprompt_bridge::ids::{KeystoreRef, PinnedPubKey};
use systemprompt_identifiers::ValidatedUrl;

#[test]
fn round_trip_full_config_preserves_wire_format() {
    let toml_input = r#"gateway_url = "https://gateway.example.com"

[pat]
file = "/etc/bridge/pat.token"

[session]
enabled = true

[mtls]
cert_keystore_ref = "macos:my-cert-label"

[sync]
pinned_pubkey = "MCowBQYDK2VwAyEABase64Pubkey=="

[claude]
inference_gateway_base_url = "https://inference.example.com"
auth_scheme = "bearer"
models = ["claude-opus-4", "claude-sonnet-4"]
organization_uuid = "abc-123"
"#;
    let cfg: Config = toml::from_str(toml_input).expect("parse toml");
    assert_eq!(
        cfg.gateway_url.as_ref().map(ValidatedUrl::as_str),
        Some("https://gateway.example.com"),
    );
    assert_eq!(
        cfg.mtls
            .as_ref()
            .and_then(|m| m.cert_keystore_ref.as_ref())
            .map(KeystoreRef::as_str),
        Some("macos:my-cert-label"),
    );
    assert_eq!(
        cfg.sync
            .as_ref()
            .and_then(|s| s.pinned_pubkey.as_ref())
            .map(PinnedPubKey::as_str),
        Some("MCowBQYDK2VwAyEABase64Pubkey=="),
    );
    assert_eq!(
        cfg.claude
            .as_ref()
            .and_then(|c| c.inference_gateway_base_url.as_ref())
            .map(ValidatedUrl::as_str),
        Some("https://inference.example.com"),
    );
}

#[test]
fn empty_inference_gateway_base_url_rejected() {
    let toml_input = r#"
[claude]
inference_gateway_base_url = ""
"#;
    let result: Result<Config, _> = toml::from_str(toml_input);
    assert!(result.is_err(), "empty ValidatedUrl must fail validation");
}
