//! Tests for the Teams `Extension` registration surface.
//!
//! Exercises the metadata and the `teams` config section's prefix, schema and
//! validation the composition runs at admission. Pure logic — no kernel, no
//! network.

use serde_json::json;
use systemprompt_extension::{Extension, ExtensionConfig, ExtensionConfigError};
use systemprompt_models::errors::ServicesValidationError;
use systemprompt_teams_entry::TeamsExtension;
use systemprompt_teams_entry::extension::TeamsAppsConfig;

fn validate(section: &serde_json::Value) -> Result<(), ExtensionConfigError> {
    serde_json::from_value::<TeamsAppsConfig>(section.clone())
        .map_err(|source| ExtensionConfigError::ParseError {
            source: Box::new(source),
        })?
        .validate()
}

fn schema() -> serde_json::Value {
    serde_json::to_value(schemars::schema_for!(TeamsAppsConfig)).expect("schema renders")
}

#[test]
fn metadata_identifies_the_teams_extension() {
    let meta = TeamsExtension.metadata();
    assert_eq!(meta.id, "teams");
    assert_eq!(meta.name, "Microsoft Teams");
    assert!(!meta.version.is_empty());
}

#[test]
fn config_is_scoped_under_the_teams_prefix() {
    assert_eq!(<TeamsAppsConfig as ExtensionConfig>::PREFIX, "teams");
}

#[test]
fn config_schema_is_published() {
    let schema = schema();
    assert!(schema.is_object());
}

#[test]
fn validate_config_accepts_a_well_formed_app_map() {
    let config = json!({
        "primary": {
            "tenant_id": "tenant-1",
            "app_id": "app-1",
            "app_password_ref": "teams_app_password",
            "default_agent": "support-agent",
        }
    });
    validate(&config).expect("a valid app map passes validation");
}

#[test]
fn validate_config_rejects_a_non_object_document() {
    let err = validate(&json!("not an app map"))
        .expect_err("a scalar document cannot deserialize into the app map");
    assert!(
        matches!(err, ExtensionConfigError::ParseError { .. }),
        "expected ParseError, got {err:?}"
    );
}

#[test]
fn validate_config_rejects_a_structurally_invalid_app() {
    let config = json!({
        "broken": {
            "tenant_id": "tenant-1",
            "app_id": "",
            "app_password_ref": "teams_app_password",
            "default_agent": "support-agent",
        }
    });
    let err = validate(&config).expect_err("an empty app_id cannot deserialize into a TeamsAppId");
    assert!(
        matches!(err, ExtensionConfigError::ParseError { .. }),
        "expected ParseError, got {err:?}"
    );
}

fn built_in_secret_ref(err: &ExtensionConfigError) -> Option<(&'static str, String)> {
    let ExtensionConfigError::SchemaValidation(source) = err else {
        return None;
    };
    match source.downcast_ref::<ServicesValidationError>()? {
        ServicesValidationError::BuiltInSecretRef { field, secret, .. } => {
            Some((*field, secret.clone()))
        },
        _ => None,
    }
}

#[test]
fn validate_config_refuses_an_app_password_ref_naming_a_built_in_secret() {
    let config = json!({
        "primary": {
            "tenant_id": "tenant-1",
            "app_id": "app-1",
            "app_password_ref": "signing_key_pem",
            "default_agent": "support-agent",
        }
    });
    let err =
        validate(&config).expect_err("a built-in secret name is refused at config validation");
    assert_eq!(
        built_in_secret_ref(&err),
        Some(("app_password_ref", "signing_key_pem".to_owned())),
        "got {err:?}"
    );
}
