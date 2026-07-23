//! Unit tests for profile_authoring builders and display-name generation

use systemprompt_cloud::profile_authoring::{
    CloudProfileBuilder, LocalProfileBuilder, generate_display_name,
};
use systemprompt_identifiers::TenantId;
use systemprompt_models::profile::{
    AuthzMode, SecretsSource, SecretsValidationMode, TrustedIssuer,
};
use systemprompt_models::{CloudValidationMode, Environment, LogLevel, OutputFormat, ProfileType};

#[test]
fn test_generate_display_name_known_aliases() {
    assert_eq!(generate_display_name("dev"), "Development");
    assert_eq!(generate_display_name("development"), "Development");
    assert_eq!(generate_display_name("prod"), "Production");
    assert_eq!(generate_display_name("PRODUCTION"), "Production");
    assert_eq!(generate_display_name("staging"), "Staging");
    assert_eq!(generate_display_name("stage"), "Staging");
    assert_eq!(generate_display_name("test"), "Test");
    assert_eq!(generate_display_name("testing"), "Test");
    assert_eq!(generate_display_name("local"), "Local Development");
    assert_eq!(generate_display_name("cloud"), "Cloud");
}

#[test]
fn test_generate_display_name_capitalizes_unknown_names() {
    assert_eq!(generate_display_name("my-profile"), "My-profile");
    assert_eq!(generate_display_name("a"), "A");
    assert_eq!(generate_display_name(""), "");
}

#[test]
fn test_local_profile_defaults() {
    let profile = LocalProfileBuilder::new("local", "./secrets.json", "/srv/services").build();

    assert_eq!(profile.name, "local");
    assert_eq!(profile.display_name, "Local Development");
    assert!(matches!(profile.target, ProfileType::Local));
    assert_eq!(profile.site.name, "systemprompt.io");
    assert_eq!(profile.database.db_type, "postgres");
    assert!(!profile.database.external_db_access);
    assert_eq!(profile.server.host, "127.0.0.1");
    assert_eq!(profile.server.port, 8080);
    assert_eq!(profile.server.api_server_url, "http://localhost:8080");
    assert_eq!(profile.server.api_internal_url, "http://localhost:8080");
    assert_eq!(profile.server.api_external_url, "http://localhost:8080");
    assert!(!profile.server.use_https);
    assert_eq!(
        profile.server.cors_allowed_origins,
        vec![
            "http://localhost:8080".to_owned(),
            "http://localhost:5173".to_owned()
        ]
    );
    assert_eq!(profile.paths.services, "/srv/services");
    assert_eq!(profile.security.issuer, "systemprompt-local");
    assert!(profile.security.trusted_issuers.is_empty());
    assert!(profile.security.allow_registration);
    assert!(profile.rate_limits.disabled);
    assert_eq!(
        profile.server.trusted_proxies,
        systemprompt_cloud::trusted_proxies::default_local_trusted_proxies()
    );
    assert!(matches!(
        profile.runtime.environment,
        Environment::Development
    ));
    assert!(matches!(profile.runtime.log_level, LogLevel::Verbose));
    assert!(matches!(profile.runtime.output_format, OutputFormat::Text));
    assert!(!profile.runtime.no_color);
    assert!(!profile.runtime.non_interactive);
    assert_eq!(profile.system_admin.username, "admin");
}

#[test]
fn test_local_profile_cloud_and_secrets_sections() {
    let profile = LocalProfileBuilder::new("local", "./secrets.json", "/srv/services")
        .with_tenant_id(TenantId::new("local_tenant"))
        .build();

    let cloud = profile.cloud.as_ref().unwrap();
    assert_eq!(
        cloud.tenant_id.as_ref().map(|t| t.as_str().to_owned()),
        Some("local_tenant".to_owned())
    );
    assert!(matches!(cloud.validation, CloudValidationMode::Warn));

    let secrets = profile.secrets.as_ref().unwrap();
    assert_eq!(secrets.secrets_path, "./secrets.json");
    assert!(matches!(secrets.validation, SecretsValidationMode::Warn));
    assert!(matches!(secrets.source, SecretsSource::File));
}

#[test]
fn test_local_profile_governance_webhook() {
    let profile = LocalProfileBuilder::new("local", "./secrets.json", "/srv/services").build();

    let governance = profile.governance.as_ref().unwrap();
    let authz = governance.authz.as_ref().unwrap();
    assert!(matches!(authz.hook.mode, AuthzMode::Webhook));
    assert_eq!(
        authz.hook.url.as_deref(),
        Some("http://localhost:8080/api/public/govern/authz")
    );
    assert_eq!(authz.hook.timeout_ms, 500);
    assert!(authz.hook.acknowledgement.is_none());
}

#[test]
fn test_cloud_profile_defaults() {
    let profile = CloudProfileBuilder::new("prod").build();

    assert_eq!(profile.name, "prod");
    assert_eq!(profile.display_name, "Production");
    assert!(matches!(profile.target, ProfileType::Cloud));
    assert_eq!(profile.database.db_type, "postgres");
    assert!(!profile.database.external_db_access);
    assert_eq!(profile.server.host, "0.0.0.0");
    assert_eq!(profile.server.port, 8080);
    assert_eq!(
        profile.server.api_server_url,
        "https://cloud.systemprompt.io"
    );
    assert_eq!(profile.server.api_internal_url, "http://localhost:8080");
    assert_eq!(
        profile.server.api_external_url,
        "https://cloud.systemprompt.io"
    );
    assert!(profile.server.use_https);
    assert_eq!(
        profile.server.cors_allowed_origins,
        vec!["https://cloud.systemprompt.io".to_owned()]
    );
    assert_eq!(profile.paths.system, "/app");
    assert_eq!(profile.paths.services, "/app/services");
    assert_eq!(profile.paths.bin, "/app/bin");
    assert_eq!(profile.paths.storage.as_deref(), Some("/app/storage"));
    assert_eq!(profile.paths.web_path.as_deref(), Some("/app/web"));
    assert_eq!(profile.security.issuer, "systemprompt");
    assert!(!profile.rate_limits.disabled);
    assert_eq!(
        profile.server.trusted_proxies,
        systemprompt_cloud::trusted_proxies::default_cloud_trusted_proxies()
    );
    assert!(systemprompt_cloud::trusted_proxies::covers_fly_peer(
        &profile.server.trusted_proxies
    ));
    assert!(matches!(
        profile.runtime.environment,
        Environment::Production
    ));
    assert!(matches!(profile.runtime.log_level, LogLevel::Normal));
    assert!(matches!(profile.runtime.output_format, OutputFormat::Json));
    assert!(profile.runtime.no_color);
    assert!(profile.runtime.non_interactive);
}

#[test]
fn test_cloud_profile_custom_fields() {
    let issuer = TrustedIssuer {
        issuer: "https://api.systemprompt.io".to_owned(),
        jwks_uri: "https://api.systemprompt.io/.well-known/jwks.json".to_owned(),
        audience: "tenant_1".to_owned(),
        typ_allowlist: Vec::new(),
        allowed_client_ids: Vec::new(),
        can_issue_id_jag: false,
    };
    let profile = CloudProfileBuilder::new("prod")
        .with_tenant_id(TenantId::new("tenant_1"))
        .with_external_url("https://example.com")
        .with_external_db_access(true)
        .with_secrets_path("./secrets.json")
        .with_trusted_issuer(issuer)
        .build();

    assert!(profile.database.external_db_access);
    assert_eq!(profile.server.api_server_url, "https://example.com");
    assert_eq!(profile.server.api_external_url, "https://example.com");
    assert_eq!(
        profile.server.cors_allowed_origins,
        vec!["https://example.com".to_owned()]
    );

    let cloud = profile.cloud.as_ref().unwrap();
    assert_eq!(
        cloud.tenant_id.as_ref().map(|t| t.as_str().to_owned()),
        Some("tenant_1".to_owned())
    );
    assert!(matches!(cloud.validation, CloudValidationMode::Strict));

    let secrets = profile.secrets.as_ref().unwrap();
    assert_eq!(secrets.secrets_path, "./secrets.json");
    assert!(matches!(secrets.validation, SecretsValidationMode::Strict));
    assert!(matches!(secrets.source, SecretsSource::Env));

    assert_eq!(profile.security.trusted_issuers.len(), 1);
    assert_eq!(
        profile.security.trusted_issuers[0].issuer,
        "https://api.systemprompt.io"
    );
}

#[test]
fn test_cloud_profile_secrets_path_defaults_to_empty() {
    let profile = CloudProfileBuilder::new("prod").build();
    let secrets = profile.secrets.as_ref().unwrap();
    assert_eq!(secrets.secrets_path, "");
}

#[test]
fn test_cloud_profile_governance_targets_internal_url() {
    let profile = CloudProfileBuilder::new("prod")
        .with_external_url("https://example.com")
        .build();

    let governance = profile.governance.as_ref().unwrap();
    let authz = governance.authz.as_ref().unwrap();
    assert!(matches!(authz.hook.mode, AuthzMode::Webhook));
    assert_eq!(
        authz.hook.url.as_deref(),
        Some("http://localhost:8080/api/public/govern/authz")
    );
}
