//! Tests for the `cloud profile edit` settings prompts, driven through
//! `ScriptedPrompter` against an in-memory profile fixture.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::PathBuf;

use systemprompt_cli::cloud::profile::edit_settings::{
    edit_runtime_settings, edit_security_settings, edit_server_settings,
};
use systemprompt_cli::interactive::ScriptedPrompter;
use systemprompt_manifest::services::SystemAdminConfig;
use systemprompt_manifest::{
    ContentNegotiationConfig, Environment, ExtensionsConfig, LogLevel, PathsConfig, Profile,
    ProfileDatabaseConfig, ProfileType, RateLimitsConfig, RuntimeConfig, SecurityConfig,
    SecurityHeadersConfig, ServerConfig, SiteConfig,
};
use systemprompt_models::auth::JwtAudience;

fn scripted(answers: &[&str]) -> ScriptedPrompter {
    ScriptedPrompter::new(answers.iter().map(|s| (*s).to_owned()))
}

fn make_profile() -> Profile {
    Profile {
        storage: Default::default(),
        observability: Default::default(),
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
            metrics_port: None,
            max_concurrent_streams: systemprompt_manifest::config::DEFAULT_MAX_CONCURRENT_STREAMS,
            role: Default::default(),
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
            issuer: "https://issuer.test".to_string(),
            access_token_expiration: 3600,
            refresh_token_expiration: 86400,
            audiences: vec![JwtAudience::Api],
            allowed_resource_audiences: vec![],
            allow_registration: true,
            allow_dynamic_client_registration: true,
            login_page_url: None,
            signing_key_path: PathBuf::from("/tmp/test-signing-key.pem"),
            trusted_issuers: vec![],
            id_jag_ttl_secs: systemprompt_manifest::profile::DEFAULT_ID_JAG_TTL_SECS,
        },
        rate_limits: RateLimitsConfig::default(),
        runtime: RuntimeConfig::default(),
        cloud: None,
        secrets: None,
        extensions: ExtensionsConfig::default(),
        governance: None,
        judge: Default::default(),
        retention: Default::default(),
        services: Default::default(),
        system_admin: SystemAdminConfig {
            username: "admin".to_string(),
            email: None,
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

const HOST_PLACEHOLDER: &str = "${SP_EDIT_DOC_UNSET_HOST:-127.0.0.1}";

fn read_raw(path: &std::path::Path) -> serde_yaml::Value {
    serde_yaml::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

// Why: the edit used to save the interpolated struct, so every `${VAR}` in
// the profile was replaced by the operator's shell value (or its default).
#[test]
fn saving_an_edit_keeps_untouched_placeholders_and_writes_the_changed_leaf() {
    use systemprompt_cli::cloud::profile::edit_document::ProfileDocument;

    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("profile.yaml");
    let mut raw = serde_yaml::to_value(make_profile()).unwrap();
    raw["server"]["host"] = serde_yaml::Value::String(HOST_PLACEHOLDER.to_owned());
    let text = serde_yaml::to_string(&raw).unwrap();
    std::fs::write(&path, &text).unwrap();

    let before = Profile::from_yaml(&text, &path).unwrap();
    assert_eq!(before.server.host, "127.0.0.1");
    let mut after = before.clone();
    after.server.port = 9090;

    let mut document = ProfileDocument::open(&path).unwrap();
    document.apply_changes(&before, &after).unwrap();
    document.save().unwrap();

    let saved = read_raw(&path);
    assert_eq!(
        saved["server"]["host"],
        serde_yaml::Value::String(HOST_PLACEHOLDER.to_owned()),
        "an untouched placeholder must survive the save"
    );
    assert_eq!(saved["server"]["port"], serde_yaml::Value::from(9090));
}
