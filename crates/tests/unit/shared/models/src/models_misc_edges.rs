//! Unit tests for small model edges: protocol bindings, security schemes,
//! path errors, repository-error HTTP mapping, and cloud claims.

use std::str::FromStr;
use systemprompt_models::a2a::{ApiKeyLocation, ProtocolBinding, SecurityScheme};
use systemprompt_models::auth::CloudAuthClaims;
use systemprompt_models::{ApiError, PathNotConfiguredError};
use systemprompt_traits::{ConstraintKind, RepositoryError};

#[test]
fn protocol_binding_round_trips_all_variants() {
    for (binding, tag) in [
        (ProtocolBinding::JsonRpc, "JSONRPC"),
        (ProtocolBinding::Grpc, "GRPC"),
        (ProtocolBinding::HttpJson, "HTTP+JSON"),
    ] {
        assert_eq!(binding.as_str(), tag);
        assert_eq!(binding.to_string(), tag);
        assert_eq!(String::from(binding), tag);
        assert_eq!(ProtocolBinding::from_str(tag).unwrap(), binding);
        assert_eq!(serde_json::to_value(binding).unwrap(), tag);
    }
    assert!(ProtocolBinding::from_str("SOAP").is_err());
}

#[test]
fn api_key_location_parse_display_round_trip() {
    for (loc, s) in [
        (ApiKeyLocation::Query, "query"),
        (ApiKeyLocation::Header, "header"),
        (ApiKeyLocation::Cookie, "cookie"),
    ] {
        assert_eq!(loc.to_string(), s);
        assert_eq!(ApiKeyLocation::from_str(s).unwrap(), loc);
    }
    assert!(ApiKeyLocation::from_str("body").is_err());
}

#[test]
fn security_scheme_api_key_serializes_with_in_field() {
    let scheme = SecurityScheme::ApiKey {
        name: "X-Api-Key".to_owned(),
        location: ApiKeyLocation::Header,
        description: None,
    };
    let json = serde_json::to_value(&scheme).unwrap();
    assert_eq!(json["type"], "apiKey");
    assert_eq!(json["in"], "header");
    assert_eq!(json["name"], "X-Api-Key");
}

#[test]
fn path_not_configured_error_names_field_and_profile() {
    let err = PathNotConfiguredError::new("storage").with_profile_path("/etc/profile.yaml");
    let msg = err.to_string();
    assert!(msg.contains("paths.storage"));
    assert!(msg.contains("/etc/profile.yaml"));

    let bare = PathNotConfiguredError::new("bin").to_string();
    assert!(bare.contains("paths.bin"));
    assert!(!bare.contains("Profile: "));
}

#[test]
fn repository_error_variants_map_to_http_statuses() {
    let cases: Vec<(RepositoryError, u16)> = vec![
        (RepositoryError::not_found("row", "r1"), 404),
        (
            RepositoryError::conflict("task", "t1", "stale version"),
            409,
        ),
        (RepositoryError::invalid_argument("state", "bad"), 400),
        (RepositoryError::invalid_data("agent_name", "corrupt"), 500),
        (RepositoryError::Internal("boom".into()), 500),
        (
            RepositoryError::database(std::io::Error::other("down")),
            500,
        ),
    ];
    for (err, status) in cases {
        let api: ApiError = err.into();
        assert_eq!(api.code.status_code(), status);
    }
}

#[test]
fn a_constraint_violation_answers_conflict_without_the_constraint_name() {
    let err = RepositoryError::Constraint {
        kind: ConstraintKind::Unique,
        constraint: "users_email_key".to_owned(),
        source: Box::new(std::io::Error::other("duplicate key value")),
    };
    let api: ApiError = err.into();
    assert_eq!(api.code.status_code(), 409);
    assert_eq!(api.error_key.as_deref(), Some("unique_violation"));
    let body = serde_json::to_string(&api).unwrap();
    assert!(!body.contains("users_email_key"), "{body}");
    assert!(!body.contains("duplicate key value"), "{body}");
    assert!(api.source().is_some());
}

#[test]
fn a_server_error_body_never_carries_internal_text() {
    let api: ApiError =
        RepositoryError::database(std::io::Error::other("relation \"secret_table\" missing"))
            .into();
    let body = serde_json::to_string(&api).unwrap();
    assert!(!body.contains("secret_table"), "{body}");
    assert!(api.source().is_some(), "the cause is kept for logging");

    let raw = ApiError::internal_error("context").with_details("SELECT * FROM users");
    let body = serde_json::to_value(&raw).unwrap();
    assert_eq!(body["message"], "Internal server error");
    assert!(body.get("details").is_none(), "{body}");
}

#[test]
fn cloud_claims_expiry_is_relative_to_now() {
    let now = chrono::Utc::now().timestamp();
    let live = CloudAuthClaims {
        sub: "user-1".to_owned(),
        exp: now + 3600,
        email: Some("e@example.com".to_owned()),
    };
    assert!(!live.is_expired());
    assert_eq!(live.subject(), "user-1");
    assert_eq!(live.expires_at(), now + 3600);

    let stale = CloudAuthClaims {
        sub: "user-2".to_owned(),
        exp: now - 10,
        email: None,
    };
    assert!(stale.is_expired());
}
