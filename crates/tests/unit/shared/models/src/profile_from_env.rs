//! Unit tests for `Profile::from_env` (cloud/subprocess boot path).
//!
//! Env mutation is process-global; nextest runs each test in its own
//! process, so per-test env setup is safe here.

use systemprompt_models::profile::{Profile, ProfileError, ProfileType};

fn set(key: &str, value: &str) {
    unsafe { std::env::set_var(key, value) };
}

fn unset(key: &str) {
    unsafe { std::env::remove_var(key) };
}

fn set_required_env() {
    set("DATABASE_TYPE", "postgres");
    set("SITENAME", "Test Site");
    set("PORT", "8080");
    set("HOST", "0.0.0.0");
    set("API_SERVER_URL", "http://127.0.0.1:8080");
    set("API_INTERNAL_URL", "http://internal:8080");
    set("API_EXTERNAL_URL", "https://example.com");
    set("SYSTEM_PATH", "/srv/system");
    set("SYSTEMPROMPT_SERVICES_PATH", "/srv/services");
    set("BIN_PATH", "/srv/bin");
    set("JWT_ISSUER", "https://issuer.example.com");
    set("JWT_ACCESS_TOKEN_EXPIRATION", "900");
    set("JWT_REFRESH_TOKEN_EXPIRATION", "86400");
    set("JWT_AUDIENCES", "web, api");
    set("SYSTEM_ADMIN_USERNAME", "admin");
    for optional in [
        "GITHUB_LINK",
        "USE_HTTPS",
        "CORS_ALLOWED_ORIGINS",
        "CONTENT_NEGOTIATION_ENABLED",
        "STORAGE_PATH",
        "GEOIP_DATABASE_PATH",
        "SYSTEMPROMPT_WEB_PATH",
        "ALLOW_REGISTRATION",
        "RATE_LIMIT_DISABLED",
        "RATE_LIMIT_TASKS_PER_SECOND",
        "SYSTEMPROMPT_ENV",
        "SYSTEMPROMPT_LOG_LEVEL",
        "SYSTEMPROMPT_OUTPUT_FORMAT",
        "NO_COLOR",
        "CI",
    ] {
        unset(optional);
    }
}

#[test]
fn builds_cloud_profile_from_complete_env() {
    set_required_env();

    let profile = Profile::from_env("fly", "Fly Deployment").expect("profile builds");

    assert_eq!(profile.name, "fly");
    assert_eq!(profile.display_name, "Fly Deployment");
    assert_eq!(profile.target, ProfileType::Cloud);
    assert_eq!(profile.database.db_type, "postgres");
    assert!(!profile.database.external_db_access);
    assert_eq!(profile.server.port, 8080);
    assert_eq!(profile.server.host, "0.0.0.0");
    assert_eq!(profile.server.api_external_url, "https://example.com");
    assert!(!profile.server.use_https);
    assert!(profile.server.cors_allowed_origins.is_empty());
    assert_eq!(profile.paths.system, "/srv/system");
    assert_eq!(profile.paths.services, "/srv/services");
    assert_eq!(profile.paths.bin, "/srv/bin");
    assert!(profile.paths.storage.is_none());
    assert_eq!(profile.security.issuer, "https://issuer.example.com");
    assert_eq!(profile.security.access_token_expiration, 900);
    assert_eq!(profile.security.refresh_token_expiration, 86400);
    assert_eq!(profile.security.audiences.len(), 2);
    assert!(profile.security.allow_registration);
    assert_eq!(profile.system_admin.username, "admin");
    assert!(profile.cloud.is_none());
    assert!(profile.secrets.is_none());
    assert!(profile.gateway.is_none());
}

#[test]
fn missing_database_type_fails_with_named_var() {
    set_required_env();
    unset("DATABASE_TYPE");

    let err = Profile::from_env("p", "P").expect_err("must fail");
    assert!(matches!(
        err,
        ProfileError::MissingEnvVar {
            name: "DATABASE_TYPE"
        }
    ));
}

#[test]
fn missing_system_admin_username_fails_with_named_var() {
    set_required_env();
    unset("SYSTEM_ADMIN_USERNAME");

    let err = Profile::from_env("p", "P").expect_err("must fail");
    assert!(matches!(
        err,
        ProfileError::MissingEnvVar {
            name: "SYSTEM_ADMIN_USERNAME"
        }
    ));
}

#[test]
fn non_numeric_port_is_invalid_env_var() {
    set_required_env();
    set("PORT", "eighty");

    let err = Profile::from_env("p", "P").expect_err("must fail");
    assert!(matches!(
        err,
        ProfileError::InvalidEnvVar { name: "PORT", .. }
    ));
}

#[test]
fn non_numeric_jwt_expiration_is_invalid_env_var() {
    set_required_env();
    set("JWT_ACCESS_TOKEN_EXPIRATION", "soon");

    let err = Profile::from_env("p", "P").expect_err("must fail");
    assert!(matches!(
        err,
        ProfileError::InvalidEnvVar {
            name: "JWT_ACCESS_TOKEN_EXPIRATION",
            ..
        }
    ));
}

#[test]
fn optional_server_flags_are_honoured() {
    set_required_env();
    set("USE_HTTPS", "TRUE");
    set("CORS_ALLOWED_ORIGINS", "https://a.com, https://b.com");
    set("CONTENT_NEGOTIATION_ENABLED", "true");
    set("GITHUB_LINK", "https://github.com/org/repo");
    set("STORAGE_PATH", "/srv/storage");

    let profile = Profile::from_env("p", "P").expect("profile builds");

    assert!(profile.server.use_https);
    assert_eq!(
        profile.server.cors_allowed_origins,
        vec!["https://a.com".to_owned(), "https://b.com".to_owned()]
    );
    assert!(profile.server.content_negotiation.enabled);
    assert_eq!(
        profile.site.github_link.as_deref(),
        Some("https://github.com/org/repo")
    );
    assert_eq!(profile.paths.storage.as_deref(), Some("/srv/storage"));
}

#[test]
fn allow_registration_false_disables_registration() {
    set_required_env();
    set("ALLOW_REGISTRATION", "FALSE");

    let profile = Profile::from_env("p", "P").expect("profile builds");
    assert!(!profile.security.allow_registration);
}

#[test]
fn rate_limit_overrides_parse_and_garbage_falls_back() {
    set_required_env();
    set("RATE_LIMIT_DISABLED", "true");
    set("RATE_LIMIT_TASKS_PER_SECOND", "7");
    set("RATE_LIMIT_AGENTS_PER_SECOND", "not-a-number");

    let profile = Profile::from_env("p", "P").expect("profile builds");
    unset("RATE_LIMIT_AGENTS_PER_SECOND");
    let default_agents = Profile::from_env("q", "Q")
        .expect("profile builds")
        .rate_limits
        .agents_per_second;

    assert!(profile.rate_limits.disabled);
    assert_eq!(profile.rate_limits.tasks_per_second, 7);
    assert_eq!(
        profile.rate_limits.agents_per_second, default_agents,
        "unparseable rate value must fall back to the default"
    );
}

#[test]
fn runtime_config_defaults_and_flags() {
    set_required_env();
    set("NO_COLOR", "1");
    set("CI", "1");

    let profile = Profile::from_env("p", "P").expect("profile builds");

    assert!(profile.runtime.no_color);
    assert!(profile.runtime.non_interactive);
    assert!(profile.runtime.environment.is_development());
}

#[test]
fn invalid_runtime_environment_is_rejected() {
    set_required_env();
    set("SYSTEMPROMPT_ENV", "galaxy");

    let err = Profile::from_env("p", "P").expect_err("must fail");
    assert!(matches!(
        err,
        ProfileError::InvalidEnvVar {
            name: "SYSTEMPROMPT_ENV",
            ..
        }
    ));
}

#[test]
fn explicit_runtime_environment_is_parsed() {
    set_required_env();
    set("SYSTEMPROMPT_ENV", "production");

    let profile = Profile::from_env("p", "P").expect("profile builds");
    assert!(profile.runtime.environment.is_production());
}
