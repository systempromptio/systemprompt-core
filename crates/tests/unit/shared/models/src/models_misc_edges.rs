//! Unit tests for small model edges: protocol bindings, security schemes,
//! path errors, service-error HTTP mapping, cloud claims, verbosity, and
//! process filtering.

use std::str::FromStr;
use systemprompt_models::a2a::{ApiKeyLocation, ProtocolBinding, SecurityScheme};
use systemprompt_models::auth::CloudAuthClaims;
use systemprompt_models::config::{Environment, VerbosityLevel};
use systemprompt_models::repository::process_utils::filter_running_services;
use systemprompt_models::{ApiError, PathNotConfiguredError, ServiceError};

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
fn service_error_maps_to_http_statuses() {
    let cases: Vec<(ServiceError, u16)> = vec![
        (ServiceError::Validation("v".into()), 400),
        (ServiceError::BusinessLogic("b".into()), 400),
        (ServiceError::NotFound("n".into()), 404),
        (ServiceError::Conflict("c".into()), 409),
        (ServiceError::Unauthorized("u".into()), 401),
        (ServiceError::Forbidden("f".into()), 403),
        (ServiceError::External("x".into()), 500),
    ];
    for (err, status) in cases {
        let api: ApiError = err.into();
        assert_eq!(api.code.status_code(), status);
    }
}

#[test]
fn repository_error_variants_map_through_service_error() {
    use systemprompt_traits::RepositoryError;

    let not_found: ApiError = ServiceError::from(RepositoryError::NotFound("row".into())).into();
    assert_eq!(not_found.code.status_code(), 404);

    let invalid: ApiError = ServiceError::from(RepositoryError::InvalidData("bad".into())).into();
    assert_eq!(invalid.code.status_code(), 400);

    let internal: ApiError = ServiceError::from(RepositoryError::Internal("boom".into())).into();
    assert_eq!(internal.code.status_code(), 500);
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

#[test]
fn filter_running_services_drops_dead_and_untracked_pids() {
    let services = vec![
        ("a", Some(10)),
        ("b", None),
        ("c", Some(-5)),
        ("d", Some(20)),
    ];

    let running = filter_running_services(services, |s| s.1, |pid| pid == 20);

    assert_eq!(running.len(), 1);
    assert_eq!(running[0].0, "d");
}

#[test]
fn verbosity_maps_environment_and_predicates() {
    assert_eq!(
        VerbosityLevel::from_environment(Environment::Production),
        VerbosityLevel::Quiet
    );
    assert_eq!(
        VerbosityLevel::from_environment(Environment::Test),
        VerbosityLevel::Normal
    );

    assert!(VerbosityLevel::Quiet.is_quiet());
    assert!(!VerbosityLevel::Quiet.should_log_to_db());
    assert!(VerbosityLevel::Debug.is_verbose());
    assert!(VerbosityLevel::Verbose.should_show_verbose());
    assert!(!VerbosityLevel::Normal.is_verbose());
    assert!(VerbosityLevel::Normal.should_log_to_db());
}

#[test]
fn verbosity_from_env_var_priority_order() {
    unsafe {
        std::env::remove_var("SYSTEMPROMPT_QUIET");
        std::env::remove_var("SYSTEMPROMPT_VERBOSE");
        std::env::remove_var("SYSTEMPROMPT_DEBUG");
        std::env::remove_var("SYSTEMPROMPT_LOG_LEVEL");
    }
    assert_eq!(VerbosityLevel::from_env_var(), None);

    unsafe { std::env::set_var("SYSTEMPROMPT_LOG_LEVEL", "debug") };
    assert_eq!(VerbosityLevel::from_env_var(), Some(VerbosityLevel::Debug));

    unsafe { std::env::set_var("SYSTEMPROMPT_LOG_LEVEL", "banana") };
    assert_eq!(VerbosityLevel::from_env_var(), None);

    unsafe { std::env::set_var("SYSTEMPROMPT_QUIET", "1") };
    assert_eq!(VerbosityLevel::from_env_var(), Some(VerbosityLevel::Quiet));

    unsafe {
        std::env::remove_var("SYSTEMPROMPT_QUIET");
        std::env::set_var("SYSTEMPROMPT_VERBOSE", "1");
    }
    assert_eq!(
        VerbosityLevel::from_env_var(),
        Some(VerbosityLevel::Verbose)
    );
}

#[test]
fn admin_log_level_display_is_uppercase() {
    use systemprompt_models::admin::LogLevel;

    assert_eq!(LogLevel::Trace.to_string(), "TRACE");
    assert_eq!(LogLevel::Error.to_string(), "ERROR");
    assert_eq!(serde_json::to_value(LogLevel::Warn).unwrap(), "warn");
    assert_eq!(LogLevel::default(), LogLevel::Info);
}
