use serde_json::{Value, json};
use systemprompt_models::bridge::desktop_policy::{
    DesktopPolicy, DesktopPolicyError, settings_catalog,
};

fn policy(settings: Value) -> DesktopPolicy {
    DesktopPolicy {
        schema_version: 1,
        settings: serde_json::from_value(settings).expect("settings map"),
    }
}

fn err(settings: Value) -> DesktopPolicyError {
    policy(settings).validate().expect_err("policy rejected")
}

#[test]
fn the_embedded_catalog_parses() {
    let catalog = settings_catalog().expect("catalog");
    assert!(
        catalog
            .settings
            .iter()
            .any(|s| s.key == "organizationInstructions")
    );
}

#[test]
fn a_well_formed_operator_policy_validates() {
    policy(json!({
        "chatTabEnabled": true,
        "mcpToolTimeoutSec": 120,
        "inferenceModelPricingMultiplier": 0.5,
        "defaultModelEffort": "high",
        "sshHostAllowlist": ["a.example", "b.example"],
        "otlpContentCapture": [],
        "builtinToolPolicy": {"bash": "ask", "read": "allow"},
        "organizationInstructions": "be kind",
        "claudeAiImport": {"enabled": true, "bannerBehavior": "detect"},
        "allowedPluginMcpServers": [{"serverUrl": "https://*.example.com"}],
        "inferenceModelPricing": [{
            "name": "m", "inputPerMtok": 1.0, "outputPerMtok": 2.0,
            "cacheReadPerMtok": 0.1, "cacheWritePerMtok": 0.2
        }]
    }))
    .validate()
    .expect("valid");
}

#[test]
fn schema_version_must_be_one_unless_empty_and_zero() {
    let mut p = policy(json!({"chatTabEnabled": true}));
    p.schema_version = 2;
    assert!(matches!(
        p.validate(),
        Err(DesktopPolicyError::SchemaVersion)
    ));
    p.schema_version = 0;
    assert!(matches!(
        p.validate(),
        Err(DesktopPolicyError::SchemaVersion)
    ));
    DesktopPolicy::default()
        .validate()
        .expect("empty v0 policy is the unset default");
}

#[test]
fn unknown_and_reserved_keys_are_refused() {
    assert!(
        matches!(err(json!({"noSuchSetting": 1})), DesktopPolicyError::Unsupported(k) if k == "noSuchSetting")
    );
    for key in [
        "inferenceGatewayBaseUrl",
        "inferenceBedrockRegion",
        "otlpHeaders",
        "otlpEndpoint",
    ] {
        let value = if key == "otlpHeaders" {
            json!({})
        } else {
            json!("x")
        };
        let mut settings = serde_json::Map::new();
        settings.insert(key.to_owned(), value);
        assert!(
            matches!(err(Value::Object(settings)), DesktopPolicyError::Reserved(ref k) if k == key),
            "{key}"
        );
    }
}

#[test]
fn values_are_type_range_and_enum_checked() {
    assert!(matches!(
        err(json!({"chatTabEnabled": "yes"})),
        DesktopPolicyError::InvalidType { .. }
    ));
    assert!(matches!(
        err(json!({"mcpToolTimeoutSec": -1})),
        DesktopPolicyError::InvalidType { .. }
    ));
    assert!(matches!(
        err(json!({"sshHostAllowlist": ["a", 1]})),
        DesktopPolicyError::InvalidType { .. }
    ));
    assert!(matches!(
        err(json!({"allowedPluginMcpServers": ["x"]})),
        DesktopPolicyError::InvalidType { .. }
    ));
    assert!(matches!(
        err(json!({"mcpToolTimeoutSec": 10})),
        DesktopPolicyError::Range(_)
    ));
    assert!(matches!(
        err(json!({"mcpToolTimeoutSec": 99999})),
        DesktopPolicyError::Range(_)
    ));
    assert!(matches!(
        err(json!({"defaultModelEffort": "extreme"})),
        DesktopPolicyError::EnumValue(_)
    ));
    assert!(matches!(
        err(json!({"otlpContentCapture": ["everything"]})),
        DesktopPolicyError::EnumValue(_)
    ));
}

#[test]
fn nested_fields_are_checked_against_the_catalog() {
    assert!(matches!(
        err(json!({"claudeAiImport": {"surprise": true}})),
        DesktopPolicyError::Field { ref field, .. } if field == "surprise"
    ));
    assert!(matches!(
        err(json!({"claudeAiImport": {"bannerBehavior": "always"}})),
        DesktopPolicyError::Field { .. }
    ));
    assert!(matches!(
        err(json!({"inferenceModelPricing": [{
            "name": "m", "inputPerMtok": -1.0, "outputPerMtok": 2.0,
            "cacheReadPerMtok": 0.1, "cacheWritePerMtok": 0.2
        }]})),
        DesktopPolicyError::Field { .. }
    ));
    assert!(matches!(
        err(json!({"inferenceModelPricing": [{"name": "m", "inputPerMtok": 1.0}]})),
        DesktopPolicyError::PricingFields
    ));
    assert!(matches!(
        err(json!({"deniedPluginMcpServers": [{}]})),
        DesktopPolicyError::ServerUrl(k) if k == "deniedPluginMcpServers"
    ));
}

#[test]
fn safety_rules_refuse_deny_decisions_long_instructions_and_content_capture() {
    assert!(matches!(
        err(json!({"builtinToolPolicy": {"bash": "deny"}})),
        DesktopPolicyError::ToolDecision
    ));
    assert!(matches!(
        err(json!({"organizationInstructions": "x".repeat(3001)})),
        DesktopPolicyError::InstructionsLength
    ));
    policy(json!({"organizationInstructions": "é".repeat(3000)}))
        .validate()
        .expect("the limit counts characters, not bytes");
    assert!(matches!(
        err(json!({"otlpContentCapture": ["userPrompts"]})),
        DesktopPolicyError::ContentCapture
    ));
}

#[test]
fn deserialising_validates_and_defaults_the_schema_version() {
    let parsed: DesktopPolicy =
        serde_json::from_value(json!({"settings": {"chatTabEnabled": false}})).expect("parse");
    assert_eq!(parsed.schema_version, 1);
    assert!(!parsed.is_empty());
    let refused =
        serde_json::from_value::<DesktopPolicy>(json!({"settings": {"otlpEndpoint": "x"}}))
            .expect_err("reserved key fails to parse");
    assert!(refused.to_string().contains("otlpEndpoint"), "{refused}");
    assert!(serde_json::from_value::<DesktopPolicy>(json!({"extra": 1})).is_err());
}
