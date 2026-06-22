#![allow(clippy::all)]

use std::path::PathBuf;

use systemprompt_config::{ConfigError, SecurityConfigService, SecurityUpdate};
use systemprompt_models::profile::{SecurityConfig, TrustedIssuer, default_resource_audiences};

fn security() -> SecurityConfig {
    SecurityConfig {
        issuer: "https://old.example.com".to_owned(),
        access_token_expiration: 3600,
        refresh_token_expiration: 86400,
        audiences: Vec::new(),
        allowed_resource_audiences: default_resource_audiences(),
        allow_registration: true,
        signing_key_path: PathBuf::from("signing_key.pem"),
        trusted_issuers: Vec::new(),
        id_jag_ttl_secs: systemprompt_models::profile::DEFAULT_ID_JAG_TTL_SECS,
    }
}

fn issuer(url: &str, jwks: &str) -> TrustedIssuer {
    TrustedIssuer {
        issuer: url.to_owned(),
        jwks_uri: jwks.to_owned(),
        audience: "my-audience".to_owned(),
        typ_allowlist: Vec::new(),
        allowed_client_ids: Vec::new(),
        can_issue_id_jag: false,
    }
}

#[test]
fn empty_update_returns_no_changes() {
    let mut config = security();

    let changes =
        SecurityConfigService::apply_update(&mut config, &SecurityUpdate::default()).unwrap();

    assert!(changes.is_empty());
    assert_eq!(config.issuer, "https://old.example.com");
}

#[test]
fn issuer_update_records_old_and_new_values() {
    let mut config = security();
    let update = SecurityUpdate {
        jwt_issuer: Some("https://new.example.com".to_owned()),
        ..SecurityUpdate::default()
    };

    let changes = SecurityConfigService::apply_update(&mut config, &update).unwrap();

    assert_eq!(config.issuer, "https://new.example.com");
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].field, "jwt_issuer");
    assert_eq!(changes[0].old_value, "https://old.example.com");
    assert_eq!(changes[0].new_value, "https://new.example.com");
    assert_eq!(
        changes[0].message,
        "Updated JWT issuer to https://new.example.com"
    );
}

#[test]
fn expiry_updates_apply_both_fields() {
    let mut config = security();
    let update = SecurityUpdate {
        access_token_expiration: Some(600),
        refresh_token_expiration: Some(1200),
        ..SecurityUpdate::default()
    };

    let changes = SecurityConfigService::apply_update(&mut config, &update).unwrap();

    assert_eq!(config.access_token_expiration, 600);
    assert_eq!(config.refresh_token_expiration, 1200);
    assert_eq!(changes.len(), 2);
    assert_eq!(changes[0].field, "access_token_expiration");
    assert_eq!(changes[1].field, "refresh_token_expiration");
}

#[test]
fn non_positive_access_expiry_errors() {
    let mut config = security();
    let update = SecurityUpdate {
        access_token_expiration: Some(0),
        ..SecurityUpdate::default()
    };

    let err = SecurityConfigService::apply_update(&mut config, &update).unwrap_err();

    assert!(matches!(err, ConfigError::NonPositiveAccessTokenExpiry));
    assert_eq!(err.to_string(), "Access token expiry must be positive");
    assert_eq!(config.access_token_expiration, 3600);
}

#[test]
fn non_positive_refresh_expiry_errors() {
    let mut config = security();
    let update = SecurityUpdate {
        refresh_token_expiration: Some(-5),
        ..SecurityUpdate::default()
    };

    let err = SecurityConfigService::apply_update(&mut config, &update).unwrap_err();

    assert!(matches!(err, ConfigError::NonPositiveRefreshTokenExpiry));
    assert_eq!(err.to_string(), "Refresh token expiry must be positive");
    assert_eq!(config.refresh_token_expiration, 86400);
}

#[test]
fn resource_audiences_merge_keeps_defaults_and_dedups() {
    let mut config = security();
    let update = SecurityUpdate {
        resource_audiences: vec!["hook".to_owned(), "custom".to_owned(), "custom".to_owned()],
        ..SecurityUpdate::default()
    };

    let changes = SecurityConfigService::apply_update(&mut config, &update).unwrap();

    let mut expected = default_resource_audiences();
    expected.push("custom".to_owned());
    assert_eq!(config.allowed_resource_audiences, expected);
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].field, "allowed_resource_audiences");
    assert_eq!(changes[0].new_value, expected.join(","));
    assert_eq!(changes[0].message, "Updated allowed resource audiences");
}

#[test]
fn upsert_trusted_issuer_adds_entry() {
    let mut config = security();

    let change = SecurityConfigService::upsert_trusted_issuer(
        &mut config,
        issuer("https://idp.example.com", "https://idp.example.com/jwks"),
    );

    assert_eq!(config.trusted_issuers.len(), 1);
    assert_eq!(change.field, "trusted_issuers");
    assert_eq!(change.old_value, "");
    assert_eq!(change.new_value, "https://idp.example.com");
    assert_eq!(
        change.message,
        "Added trusted issuer https://idp.example.com"
    );
}

#[test]
fn upsert_trusted_issuer_replaces_same_issuer() {
    let mut config = security();
    SecurityConfigService::upsert_trusted_issuer(
        &mut config,
        issuer("https://idp.example.com", "https://idp.example.com/old"),
    );

    SecurityConfigService::upsert_trusted_issuer(
        &mut config,
        issuer("https://idp.example.com", "https://idp.example.com/new"),
    );

    assert_eq!(config.trusted_issuers.len(), 1);
    assert_eq!(
        config.trusted_issuers[0].jwks_uri,
        "https://idp.example.com/new"
    );
}

#[test]
fn remove_trusted_issuer_deletes_entry() {
    let mut config = security();
    SecurityConfigService::upsert_trusted_issuer(
        &mut config,
        issuer("https://idp.example.com", "https://idp.example.com/jwks"),
    );

    let change =
        SecurityConfigService::remove_trusted_issuer(&mut config, "https://idp.example.com")
            .unwrap();

    assert!(config.trusted_issuers.is_empty());
    assert_eq!(change.old_value, "https://idp.example.com");
    assert_eq!(change.new_value, "");
    assert_eq!(
        change.message,
        "Removed trusted issuer https://idp.example.com"
    );
}

#[test]
fn remove_trusted_issuer_unknown_errors() {
    let mut config = security();

    let err = SecurityConfigService::remove_trusted_issuer(&mut config, "https://ghost.example")
        .unwrap_err();

    assert!(matches!(err, ConfigError::TrustedIssuerNotFound { .. }));
    assert_eq!(
        err.to_string(),
        "No trusted issuer found with issuer https://ghost.example"
    );
}
