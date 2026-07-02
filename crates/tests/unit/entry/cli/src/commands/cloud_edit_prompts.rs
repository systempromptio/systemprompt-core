//! Tests for the `cloud profile edit` settings prompts, driven through
//! `ScriptedPrompter` against an in-memory profile fixture.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::PathBuf;

use systemprompt_cli::cloud::profile::edit_settings::{
    edit_runtime_settings, edit_security_settings, edit_server_settings,
};
use systemprompt_cli::interactive::ScriptedPrompter;
use systemprompt_models::auth::JwtAudience;
use systemprompt_models::services::SystemAdminConfig;
use systemprompt_models::{
    ContentNegotiationConfig, Environment, ExtensionsConfig, LogLevel, PathsConfig, Profile,
    ProfileDatabaseConfig, ProfileType, RateLimitsConfig, RuntimeConfig, SecurityConfig,
    SecurityHeadersConfig, ServerConfig, SiteConfig,
};

fn scripted(answers: &[&str]) -> ScriptedPrompter {
    ScriptedPrompter::new(answers.iter().map(|s| (*s).to_owned()))
}

fn make_profile() -> Profile {
    Profile {
        name: "test".to_string(),
        display_name: "Test".to_string(),
        target: ProfileType::Local,
        site: SiteConfig {
            name: "Test Site".to_string(),
            github_link: None,
        },
        database: ProfileDatabaseConfig {
            db_type: "postgres".to_string(),
            external_db_access: false,
            pool: None,
        },
        server: ServerConfig {
            host: "127.0.0.1".to_string(),
            port: 8080,
            api_server_url: "http://localhost:8080".to_string(),
            api_internal_url: "http://localhost:8080".to_string(),
            api_external_url: "https://example.com".to_string(),
            use_https: false,
            cors_allowed_origins: vec![],
            content_negotiation: ContentNegotiationConfig::default(),
            security_headers: SecurityHeadersConfig::default(),
            instance_id: None,
            max_concurrent_streams: systemprompt_models::config::DEFAULT_MAX_CONCURRENT_STREAMS,
            trusted_proxies: Vec::new(),
        },
        paths: PathsConfig {
            system: "/tmp/test/system".to_string(),
            services: "/tmp/test/services".to_string(),
            bin: "/tmp/test/bin".to_string(),
            web_path: None,
            storage: None,
            geoip_database: None,
        },
        security: SecurityConfig {
            issuer: "test-issuer".to_string(),
            access_token_expiration: 3600,
            refresh_token_expiration: 86400,
            audiences: vec![JwtAudience::Api],
            allowed_resource_audiences: vec![],
            allow_registration: true,
            signing_key_path: PathBuf::from("/tmp/test-signing-key.pem"),
            trusted_issuers: vec![],
            id_jag_ttl_secs: systemprompt_models::profile::DEFAULT_ID_JAG_TTL_SECS,
        },
        rate_limits: RateLimitsConfig::default(),
        runtime: RuntimeConfig::default(),
        cloud: None,
        secrets: None,
        extensions: ExtensionsConfig::default(),
        providers: systemprompt_models::profile::ProviderRegistry::default(),
        gateway: None,
        governance: None,
        system_admin: SystemAdminConfig {
            username: "admin".to_string(),
        },
    }
}

#[test]
fn edit_server_settings_applies_scripted_answers() {
    let mut profile = make_profile();
    let prompter = scripted(&[
        "0.0.0.0",
        "9090",
        "http://internal:9090",
        "https://public.example.com",
        "yes",
    ]);

    edit_server_settings(&prompter, &mut profile).expect("edit succeeds");

    assert_eq!(profile.server.host, "0.0.0.0");
    assert_eq!(profile.server.port, 9090);
    assert_eq!(profile.server.api_server_url, "http://internal:9090");
    assert_eq!(
        profile.server.api_external_url,
        "https://public.example.com"
    );
    assert!(profile.server.use_https);
}

#[test]
fn edit_server_settings_keeps_defaults_on_empty_answers() {
    let mut profile = make_profile();
    let prompter = scripted(&["", "", "", "", "no"]);

    edit_server_settings(&prompter, &mut profile).expect("edit succeeds");

    assert_eq!(profile.server.host, "127.0.0.1");
    assert_eq!(profile.server.port, 8080);
    assert!(!profile.server.use_https);
}

#[test]
fn edit_server_settings_rejects_non_numeric_port() {
    let mut profile = make_profile();
    let prompter = scripted(&["localhost", "not-a-port"]);

    let err = edit_server_settings(&prompter, &mut profile).unwrap_err();
    assert!(err.to_string().contains("Invalid port"));
}

#[test]
fn edit_security_settings_applies_scripted_answers() {
    let mut profile = make_profile();
    let prompter = scripted(&["new-issuer", "7200", "172800"]);

    edit_security_settings(&prompter, &mut profile).expect("edit succeeds");

    assert_eq!(profile.security.issuer, "new-issuer");
    assert_eq!(profile.security.access_token_expiration, 7200);
    assert_eq!(profile.security.refresh_token_expiration, 172_800);
}

#[test]
fn edit_security_settings_rejects_non_numeric_expiration() {
    let mut profile = make_profile();
    let prompter = scripted(&["issuer", "soon"]);

    let err = edit_security_settings(&prompter, &mut profile).unwrap_err();
    assert!(err.to_string().contains("Invalid access token expiration"));
}

#[test]
fn edit_runtime_settings_applies_selected_options() {
    let mut profile = make_profile();
    let prompter = scripted(&["3", "2"]);

    edit_runtime_settings(&prompter, &mut profile).expect("edit succeeds");

    assert_eq!(profile.runtime.environment, Environment::Production);
    assert_eq!(profile.runtime.log_level, LogLevel::Verbose);
}

#[test]
fn edit_runtime_settings_out_of_range_selection_errors() {
    let mut profile = make_profile();
    let prompter = scripted(&["9"]);

    let err = edit_runtime_settings(&prompter, &mut profile).unwrap_err();
    assert!(err.to_string().contains("out of range"));
}

#[test]
fn edit_runtime_settings_exhausted_prompter_errors() {
    let mut profile = make_profile();
    let prompter = scripted(&["0"]);

    let err = edit_runtime_settings(&prompter, &mut profile).unwrap_err();
    assert!(err.to_string().contains("exhausted"));
}
