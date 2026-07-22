//! Tests for the session-resolution helper functions.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::{Path, PathBuf};

use systemprompt_cli::EnvOverrides;
use systemprompt_cli::paths::ResolvedPaths;
use systemprompt_cli::session::resolution::helpers::{
    extract_profile_name, resolve_profile_path_from_session, resolve_profile_path_without_session,
    try_session_from_env,
};
use systemprompt_cloud::{CliSession, SessionIdentity, SessionKey, SessionStore};
use systemprompt_identifiers::{
    ContextId, Email, ProfileName, SessionId, SessionToken, UserId,
};
use systemprompt_models::auth::UserType;
use systemprompt_models::services::SystemAdminConfig;
use systemprompt_models::{
    ContentNegotiationConfig, ExtensionsConfig, PathsConfig, Profile, ProfileDatabaseConfig,
    ProfileType, RateLimitsConfig, RuntimeConfig, SecurityConfig, SecurityHeadersConfig,
    ServerConfig, SiteConfig,
};

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
            system: "/tmp/test".to_string(),
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
            audiences: vec![systemprompt_models::auth::JwtAudience::Api],
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

fn remote_env() -> EnvOverrides {
    let mut env = EnvOverrides::default();
    env.is_remote_cli = true;
    env.session.session_id = Some(SessionId::generate());
    env.session.context_id = Some(ContextId::generate());
    env.session.user_id = Some(UserId::new("user-remote-cli"));
    env.session.auth_token = Some("tok-123".to_string());
    env
}

fn session(profile_name: &str) -> CliSession {
    CliSession::builder(
        ProfileName::new(profile_name),
        SessionToken::new("tok"),
        SessionId::generate(),
        ContextId::generate(),
        SessionIdentity::new(
            UserId::new("user-remote-cli"),
            Email::new("a@b.test"),
            UserType::Admin,
        ),
    )
    .build()
}

#[test]
fn try_session_from_env_requires_remote_flag() {
    let profile = make_profile();
    let mut env = remote_env();
    env.is_remote_cli = false;

    assert!(try_session_from_env(&profile, &env).is_none());
}

#[test]
fn try_session_from_env_requires_every_session_field() {
    let profile = make_profile();
    for missing in 0..4 {
        let mut env = remote_env();
        match missing {
            0 => env.session.session_id = None,
            1 => env.session.context_id = None,
            2 => env.session.user_id = None,
            _ => env.session.auth_token = None,
        }
        assert!(try_session_from_env(&profile, &env).is_none());
    }
}

#[test]
fn try_session_from_env_builds_remote_session() {
    let profile = make_profile();
    let env = remote_env();

    let ctx = try_session_from_env(&profile, &env).unwrap();

    assert_eq!(ctx.session.profile_name.as_str(), "remote");
    assert_eq!(ctx.session.session_token.as_str(), "tok-123");
    assert_eq!(
        ctx.session.session_id,
        env.session.session_id.clone().unwrap()
    );
    assert_eq!(ctx.profile.name, profile.name);
}

#[test]
fn extract_profile_name_uses_parent_dir_name() {
    let name =
        extract_profile_name(Path::new("/home/user/.systemprompt/profiles/dev/profile.yaml"))
            .unwrap();
    assert_eq!(name, "dev");
}

#[test]
fn extract_profile_name_rejects_rootless_path() {
    let err = extract_profile_name(Path::new("/")).unwrap_err();
    assert!(err.to_string().contains("no parent directory"));
}

#[test]
fn resolve_profile_path_from_session_rejects_profile_mismatch() {
    let s = session("alpha");
    let err = resolve_profile_path_from_session(&s, Some("beta")).unwrap_err();
    assert!(err.to_string().contains("No session for active profile 'beta'"));
}

#[test]
fn resolve_profile_path_from_session_returns_existing_path_only() {
    let tmp = tempfile::tempdir().unwrap();
    let profile_yaml = tmp.path().join("profile.yaml");
    std::fs::write(&profile_yaml, "name: alpha\n").unwrap();

    let mut s = session("alpha");
    s.update_profile_path(profile_yaml.clone());
    assert_eq!(
        resolve_profile_path_from_session(&s, Some("alpha")).unwrap(),
        Some(profile_yaml)
    );

    let mut stale = session("alpha");
    stale.update_profile_path(tmp.path().join("missing.yaml"));
    assert_eq!(resolve_profile_path_from_session(&stale, None).unwrap(), None);

    let bare = session("alpha");
    assert_eq!(resolve_profile_path_from_session(&bare, None).unwrap(), None);
}

#[test]
fn resolve_profile_path_without_session_errors_when_store_has_nothing() {
    let paths = ResolvedPaths::discover();
    let tmp = tempfile::tempdir().unwrap();
    let store = SessionStore::load_or_create(tmp.path()).unwrap();

    let err =
        resolve_profile_path_without_session(&paths, &store, &SessionKey::Local, None).unwrap_err();
    assert!(err.to_string().contains("No session for active profile 'unknown'"));
}

#[test]
fn resolve_profile_path_without_session_returns_stored_existing_path() {
    let paths = ResolvedPaths::discover();
    let tmp = tempfile::tempdir().unwrap();
    let mut store = SessionStore::load_or_create(tmp.path()).unwrap();

    let profile_yaml = tmp.path().join("profile.yaml");
    std::fs::write(&profile_yaml, "name: alpha\n").unwrap();
    let mut s = session("alpha");
    s.update_profile_path(profile_yaml.clone());
    store.upsert_session(&SessionKey::Local, s);

    let resolved =
        resolve_profile_path_without_session(&paths, &store, &SessionKey::Local, None).unwrap();
    assert_eq!(resolved, profile_yaml);
}
