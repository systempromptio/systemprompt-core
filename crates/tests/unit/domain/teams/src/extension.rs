//! Tests for the Teams `Extension` registration surface.
//!
//! Exercises the metadata, config-prefix, config-schema, and config-validation
//! hooks the loader calls during bootstrap. Pure-logic — no registry wiring, no
//! network.

use serde_json::json;
use systemprompt_extension::{ConfigError, Extension};
use systemprompt_teams::TeamsExtension;

#[test]
fn metadata_identifies_the_teams_extension() {
    let meta = TeamsExtension.metadata();
    assert_eq!(meta.id, "teams");
    assert_eq!(meta.name, "Microsoft Teams");
    assert!(!meta.version.is_empty());
}

#[test]
fn config_is_scoped_under_the_teams_prefix() {
    assert_eq!(TeamsExtension.config_prefix(), Some("teams"));
}

#[test]
fn config_schema_is_published() {
    let schema = TeamsExtension
        .config_schema()
        .expect("teams publishes a config schema");
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
    TeamsExtension
        .validate_config(&config)
        .expect("a valid app map passes validation");
}

#[test]
fn validate_config_rejects_a_non_object_document() {
    let err = TeamsExtension
        .validate_config(&json!("not an app map"))
        .expect_err("a scalar document cannot deserialize into the app map");
    assert!(
        matches!(err, ConfigError::ParseError { .. }),
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
    let err = TeamsExtension
        .validate_config(&config)
        .expect_err("an empty app_id fails the per-app validation");
    assert!(
        matches!(err, ConfigError::SchemaValidation(_)),
        "expected SchemaValidation, got {err:?}"
    );
}
