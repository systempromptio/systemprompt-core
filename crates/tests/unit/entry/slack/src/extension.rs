//! Tests for the Slack extension registration surface.

use serde_json::json;
use systemprompt_extension::{Extension, ExtensionConfigError};
use systemprompt_models::errors::ServicesValidationError;
use systemprompt_slack_entry::SlackExtension;

#[test]
fn config_schema_describes_a_map_of_slack_app_configs() {
    let schema = SlackExtension
        .config_schema()
        .expect("slack advertises a config schema");

    let rendered = schema.to_string();
    assert!(
        rendered.contains("SlackAppConfig"),
        "schema should reference SlackAppConfig, got: {rendered}"
    );
    assert!(
        rendered.contains("workspace_id"),
        "schema should expose the per-app workspace_id field, got: {rendered}"
    );
}

#[test]
fn metadata_and_prefix_identify_the_slack_extension() {
    let meta = SlackExtension.metadata();
    assert_eq!(meta.id, "slack");
    assert_eq!(SlackExtension.config_prefix(), Some("slack"));
}

fn app(signing_secret_ref: &str, bot_token_ref: &str) -> serde_json::Value {
    json!({
        "primary": {
            "workspace_id": "T1",
            "signing_secret_ref": signing_secret_ref,
            "bot_token_ref": bot_token_ref,
            "default_agent": "support-agent",
        }
    })
}

fn refused_field(err: &ExtensionConfigError) -> Option<&'static str> {
    let ExtensionConfigError::SchemaValidation(source) = err else {
        return None;
    };
    match source.downcast_ref::<ServicesValidationError>()? {
        ServicesValidationError::BuiltInSecretRef { field, .. } => Some(field),
        _ => None,
    }
}

#[test]
fn validate_config_accepts_custom_secret_refs() {
    SlackExtension
        .validate_config(&app("slack_signing_secret", "slack_bot_token"))
        .expect("custom secret names pass validation");
}

#[test]
fn validate_config_refuses_a_signing_secret_ref_naming_a_built_in_secret() {
    let err = SlackExtension
        .validate_config(&app("oauth_at_rest_pepper", "slack_bot_token"))
        .expect_err("a built-in secret name is refused at config validation");
    assert_eq!(
        refused_field(&err),
        Some("signing_secret_ref"),
        "got {err:?}"
    );
}

#[test]
fn validate_config_refuses_a_bot_token_ref_naming_a_built_in_secret() {
    let err = SlackExtension
        .validate_config(&app("slack_signing_secret", "DATABASE_URL"))
        .expect_err("a built-in secret alias is refused at config validation");
    assert_eq!(refused_field(&err), Some("bot_token_ref"), "got {err:?}");
}
