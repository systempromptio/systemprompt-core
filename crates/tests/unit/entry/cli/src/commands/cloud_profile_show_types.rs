//! Tests for the `cloud profile show` view-type projections and redaction.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::collections::HashMap;

use systemprompt_cli::cloud::profile::show_types::{
    FullConfig, SettingsOutput, build_env_config, redact_database_url,
};
use systemprompt_models::AiConfig;
use systemprompt_test_fixtures::fixture_config;

#[test]
fn redact_database_url_masks_credentials() {
    assert_eq!(
        redact_database_url("postgres://user:pass@localhost:5432/db"),
        "postgres://[REDACTED]@localhost:5432/db"
    );
}

#[test]
fn redact_database_url_without_credentials_is_unchanged() {
    assert_eq!(
        redact_database_url("postgres://localhost/db"),
        "postgres://localhost/db"
    );
}

#[test]
fn redact_database_url_without_scheme_is_unchanged() {
    assert_eq!(
        redact_database_url("user:pass@host/db"),
        "user:pass@host/db"
    );
}

#[test]
fn build_env_config_redacts_secrets_and_handles_missing_paths() {
    let mut config = fixture_config("postgres://admin:secret@localhost:5432/cov");
    config.github_token = Some("ghp_realtoken".to_owned());

    let env = build_env_config(&config, None);

    assert_eq!(env.core.github_token.as_deref(), Some("[REDACTED]"));
    assert_eq!(env.jwt.secret, "[REDACTED]");
    assert_eq!(
        env.database.database_url,
        "postgres://[REDACTED]@localhost:5432/cov"
    );
    assert!(env.systemprompt.services_path.is_none());
    assert!(env.paths.system_path.is_empty());
    assert_eq!(env.core.port, config.port);
}

#[test]
fn build_env_config_without_github_token() {
    let config = fixture_config("postgres://admin:secret@localhost:5432/cov");
    let env = build_env_config(&config, None);
    assert!(env.core.github_token.is_none());
}

#[test]
fn full_config_empty_serializes_to_empty_object() {
    let json = serde_json::to_value(FullConfig::empty()).unwrap();
    assert_eq!(json, serde_json::json!({}));
}

#[test]
fn full_config_builders_populate_sections() {
    let full = FullConfig::empty()
        .with_settings(SettingsOutput {
            agent_port_range: (9000, 9100),
            mcp_port_range: (5000, 5100),
            auto_start_enabled: false,
            validation_strict: true,
            schema_validation_mode: "strict".to_owned(),
        })
        .with_agents(HashMap::new())
        .with_mcp_servers(HashMap::new())
        .with_ai(AiConfig::default())
        .with_web(None);

    let json = serde_json::to_value(&full).unwrap();
    assert!(json.get("settings").is_some());
    assert!(json.get("agents").is_some());
    assert!(json.get("mcp_servers").is_some());
    assert!(json.get("ai").is_some());
    assert!(json.get("web").is_none());
    assert!(json.get("environment").is_none());
}
