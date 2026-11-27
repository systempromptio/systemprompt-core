use systemprompt_models::bridge::catalog::{ModelCatalog, base_model_id};
use systemprompt_models::bridge::gateway::WhoamiResponse;
use systemprompt_models::bridge::profile::BridgeProfileResponse;

#[test]
fn decodes_the_profile_fixture() {
    let profile: BridgeProfileResponse = serde_json::from_value(serde_json::json!({
        "inference_gateway_base_url": "http://localhost:8080/v1",
        "auth_scheme": "bearer",
        "models": ["claude-haiku-5-5", "claude-opus-5-5", "claude-sonnet-5-5"],
        "default_model": "claude-sonnet-5-5[1m]",
        "organization_uuid": null,
        "providers": [
            {
                "name": "anthropic",
                "credential": "anthropic",
                "clients": ["claude"],
                "configured": true,
                "models": ["claude-haiku-5-5", "claude-opus-5-5", "claude-sonnet-5-5"],
                "display_name": "Anthropic"
            },
            {
                "name": "go",
                "credential": "go",
                "clients": ["openai"],
                "configured": false,
                "models": ["go-kimi-k3"],
                "config_issue": "personal key not added",
                "display_name": "Go",
                "personal_key": true
            }
        ],
        "model_limits": {
            "claude-sonnet-5-5": { "context_window": 1_000_000, "max_output_tokens": 128_000 }
        }
    }))
    .unwrap();
    assert_eq!(profile.models.len(), 3);
    assert_eq!(profile.providers.len(), 2);
    assert!(profile.providers[1].personal_key);
    assert_eq!(
        profile.default_model.as_deref().map(base_model_id),
        Some("claude-sonnet-5-5")
    );
    assert_eq!(
        profile
            .model_limits
            .values()
            .next()
            .map(|l| l.context_window),
        Some(1_000_000)
    );
}

#[test]
fn decodes_the_catalog_and_whoami_fixtures() {
    let catalog: ModelCatalog = serde_json::from_str(
        r#"{"data":[{"type":"model","id":"claude-haiku-5-5","display_name":"Claude Haiku 5.5","created_at":"1970-01-01T00:00:00Z","max_input_tokens":1000000,"max_tokens":128000}],"has_more":false}"#,
    )
    .unwrap();
    assert_eq!(
        catalog.get("claude-haiku-5-5").and_then(|m| m.max_tokens),
        Some(128_000)
    );
    let whoami: WhoamiResponse = serde_json::from_str(
        r#"{"user_id":"1ecce4e3-bc87-441d-b374-ea29bab88da8","email":"dev@astound.test","display_name":"Dev User","roles":["user"]}"#,
    )
    .unwrap();
    assert_eq!(
        whoami
            .user_id
            .as_ref()
            .map(systemprompt_identifiers::UserId::as_str),
        Some("1ecce4e3-bc87-441d-b374-ea29bab88da8")
    );
    assert_eq!(whoami.roles, ["user"]);
}

#[test]
fn model_suffixes_are_stripped() {
    assert_eq!(base_model_id("claude-sonnet-5-5[1m]"), "claude-sonnet-5-5");
    assert_eq!(base_model_id("plain"), "plain");
    assert_eq!(base_model_id("odd[x"), "odd[x");
}
