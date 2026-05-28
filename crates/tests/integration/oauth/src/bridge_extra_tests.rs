//! Coverage for the bridge service paths not exercised by
//! `bridge_session_tests`: exchange-code issue & consume, and
//! bridge OAuth client provisioning.

use std::path::PathBuf;
use std::sync::Once;

use crate::{create_test_user, setup_test_db};
use systemprompt_models::Config;
use systemprompt_models::auth::JwtAudience;
use systemprompt_models::config::RateLimitConfig;
use systemprompt_oauth::services::{
    exchange_bridge_session_code, hash_exchange_code, issue_bridge_exchange_code,
    provision_bridge_oauth_client,
};
use systemprompt_security::keys::{RsaSigningKey, authority};

static AUTHORITY: Once = Once::new();

fn ensure_runtime() {
    AUTHORITY.call_once(|| {
        let key = RsaSigningKey::generate_bits(2048).expect("generate signing key");
        authority::install_for_test(key);
    });
    let _ = Config::install(test_config());
}

fn test_config() -> Config {
    let database_url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL environment variable required");
    Config {
        instance_id: "test-instance".to_string(),
        max_concurrent_streams: 256,
        sitename: "test".to_string(),
        database_type: "postgres".to_string(),
        database_url,
        database_write_url: None,
        github_link: String::new(),
        github_token: None,
        system_path: String::new(),
        services_path: String::new(),
        bin_path: String::new(),
        skills_path: String::new(),
        settings_path: String::new(),
        content_config_path: String::new(),
        geoip_database_path: None,
        web_path: String::new(),
        web_config_path: String::new(),
        web_metadata_path: String::new(),
        host: "127.0.0.1".to_string(),
        port: 8080,
        api_server_url: "http://127.0.0.1:8080".to_string(),
        api_internal_url: "http://127.0.0.1:8080".to_string(),
        api_external_url: "http://127.0.0.1:8080".to_string(),
        jwt_issuer: "https://test.invalid".to_string(),
        jwt_access_token_expiration: 3600,
        jwt_refresh_token_expiration: 604_800,
        jwt_audiences: vec![JwtAudience::Bridge],
        allowed_resource_audiences: Vec::new(),
        trusted_issuers: Vec::new(),
        signing_key_path: PathBuf::new(),
        use_https: false,
        rate_limits: RateLimitConfig::default(),
        cors_allowed_origins: Vec::new(),
        trusted_proxies: Vec::new(),
        is_cloud: false,
        content_negotiation: Default::default(),
        security_headers: Default::default(),
        allow_registration: false,
        system_admin_username: "admin".to_string(),
    }
}

#[tokio::test]
async fn exchange_code_issued_and_consumed_once() {
    ensure_runtime();
    let db = setup_test_db().await;
    let user_id = create_test_user(&db).await;

    let issued = issue_bridge_exchange_code(&db, &user_id)
        .await
        .expect("issue exchange code");
    assert!(!issued.code.is_empty());

    let analytics =
        systemprompt_analytics::AnalyticsService::new(&db, None, None).expect("analytics service");
    let headers = http::HeaderMap::new();
    let result = exchange_bridge_session_code(&db, &analytics, &headers, &issued.code)
        .await
        .expect("consume code");
    assert!(
        result.is_some(),
        "first consume must yield a BridgeAuthResult"
    );

    let replay = exchange_bridge_session_code(&db, &analytics, &headers, &issued.code)
        .await
        .expect("replay returns None, not Err");
    assert!(replay.is_none(), "exchange code must be single-use");
}

#[tokio::test]
async fn exchange_unknown_code_returns_none() {
    ensure_runtime();
    let db = setup_test_db().await;
    let analytics =
        systemprompt_analytics::AnalyticsService::new(&db, None, None).expect("analytics service");
    let headers = http::HeaderMap::new();
    let result = exchange_bridge_session_code(&db, &analytics, &headers, "deadbeef")
        .await
        .expect("not an error");
    assert!(result.is_none());
}

#[tokio::test]
async fn provision_oauth_client_is_idempotent() {
    ensure_runtime();
    let db = setup_test_db().await;
    let user_id = create_test_user(&db).await;

    let first = provision_bridge_oauth_client(&db, &user_id, "http://example/token".into())
        .await
        .expect("provision first");
    let second = provision_bridge_oauth_client(&db, &user_id, "http://example/token".into())
        .await
        .expect("provision second");

    assert_eq!(first.client_id, second.client_id);
    assert_ne!(
        first.client_secret, second.client_secret,
        "secret must rotate on re-provision"
    );
    assert!(first.scopes.iter().all(|s| s.starts_with("hook:")));
}

#[test]
fn hash_exchange_code_is_stable_hex() {
    let h = hash_exchange_code("abcdef0123");
    assert_eq!(h.len(), 64, "sha256 hex digest is 64 chars");
    assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    assert_eq!(h, hash_exchange_code("abcdef0123"), "stable");
    assert_ne!(h, hash_exchange_code("abcdef0124"), "differs on input");
}
