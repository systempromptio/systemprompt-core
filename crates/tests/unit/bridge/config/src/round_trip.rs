use systemprompt_bridge::config::Config;
use systemprompt_identifiers::ValidatedUrl;

#[test]
fn round_trip_full_config_preserves_wire_format() {
    let toml_input = r#"gateway_url = "https://gateway.example.com"

[pat]
file = "/etc/bridge/pat.token"

[session]
enabled = true

[sync.trust]
gateway = "https://gateway.example.com"
key = "WGZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmY="
source = "operator"

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
    let trust = cfg
        .sync
        .as_ref()
        .and_then(|s| s.trust.as_ref())
        .expect("trust record");
    assert_eq!(trust.gateway.as_str(), "https://gateway.example.com");
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

#[test]
fn deserializes_deployment_organization_uuid() {
    let cfg: Config =
        toml::from_str(r#"deployment_organization_uuid = "f8e4d915-f8ad-5304-ab0d-c1bf895df963""#)
            .expect("parse toml");
    assert_eq!(
        cfg.deployment_organization_uuid
            .as_ref()
            .map(systemprompt_bridge::ids::DeploymentOrganizationUuid::as_str),
        Some("f8e4d915-f8ad-5304-ab0d-c1bf895df963")
    );
}

#[test]
fn deployment_organization_uuid_defaults_to_none() {
    let cfg: Config = toml::from_str("").expect("parse empty toml");
    assert!(cfg.deployment_organization_uuid.is_none());
}
