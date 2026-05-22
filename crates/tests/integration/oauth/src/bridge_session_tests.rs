//! Regression coverage for the gateway bootstrap deadlock: a freshly minted
//! bridge JWT must have a backing `user_sessions` row so the hardened gateway
//! validator (which requires an active session) admits it on first use, and
//! that row must carry the analytics captured from the exchange request.

use std::path::PathBuf;
use std::sync::Once;

use crate::{create_test_user, setup_test_db};
use http::HeaderMap;
use systemprompt_analytics::AnalyticsService;
use systemprompt_identifiers::SessionId;
use systemprompt_models::Config;
use systemprompt_models::auth::JwtAudience;
use systemprompt_models::config::RateLimitConfig;
use systemprompt_oauth::services::issue_bridge_access;
use systemprompt_security::keys::{RsaSigningKey, authority};
use systemprompt_traits::AnalyticsProvider;

static AUTHORITY: Once = Once::new();

fn ensure_runtime() {
    AUTHORITY.call_once(|| {
        let key = RsaSigningKey::generate_bits(2048).expect("generate signing key");
        authority::install_for_test(key);
    });
    // `Config::install` is a one-shot global; ignore the error when another
    // test in this binary already installed it.
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

fn exchange_request_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert("x-forwarded-for", "203.0.113.7".parse().unwrap());
    headers.insert("user-agent", "test-agent/1.0".parse().unwrap());
    headers
}

fn exchange_headers_with_session(session_id: &SessionId) -> HeaderMap {
    let mut headers = exchange_request_headers();
    headers.insert(
        systemprompt_identifiers::headers::SESSION_ID,
        session_id.as_str().parse().unwrap(),
    );
    headers
}

#[tokio::test]
async fn fresh_bridge_jwt_has_active_session() {
    ensure_runtime();
    let db = setup_test_db().await;
    let user_id = create_test_user(&db).await;
    let analytics = AnalyticsService::new(&db, None, None).expect("analytics service");

    let result = issue_bridge_access(&db, &analytics, &exchange_request_headers(), &user_id)
        .await
        .expect("mint bridge access");

    let session_id = SessionId::new(
        result
            .headers
            .get(systemprompt_identifiers::headers::SESSION_ID)
            .expect("minted token carries a session id"),
    );

    let session = analytics
        .find_active_session_by_id(&session_id)
        .await
        .expect("session lookup ok")
        .expect("freshly minted bridge JWT must have an active session row");

    assert_eq!(
        session.user_id.as_ref().map(|u| u.as_str()),
        Some(user_id.as_str()),
        "session must be bound to the minting user"
    );
}

#[tokio::test]
async fn bridge_session_captures_request_analytics() {
    ensure_runtime();
    let db = setup_test_db().await;
    let user_id = create_test_user(&db).await;
    let analytics = AnalyticsService::new(&db, None, None).expect("analytics service");

    let result = issue_bridge_access(&db, &analytics, &exchange_request_headers(), &user_id)
        .await
        .expect("mint bridge access");

    let session_id = result
        .headers
        .get(systemprompt_identifiers::headers::SESSION_ID)
        .expect("minted token carries a session id");

    let pool = db.pool_arc().expect("read pool");
    let row = sqlx::query!(
        "SELECT ip_address, user_agent FROM user_sessions WHERE session_id = $1",
        session_id
    )
    .fetch_one(pool.as_ref())
    .await
    .expect("session row exists");

    assert_eq!(
        row.ip_address.as_deref(),
        Some("203.0.113.7"),
        "session row must record the exchanging device IP"
    );
    assert_eq!(
        row.user_agent.as_deref(),
        Some("test-agent/1.0"),
        "session row must record the exchanging device user-agent"
    );
}

#[tokio::test]
async fn bridge_jwt_binds_supplied_session_id() {
    ensure_runtime();
    let db = setup_test_db().await;
    let user_id = create_test_user(&db).await;
    let analytics = AnalyticsService::new(&db, None, None).expect("analytics service");

    let supplied = SessionId::generate();
    let result = issue_bridge_access(
        &db,
        &analytics,
        &exchange_headers_with_session(&supplied),
        &user_id,
    )
    .await
    .expect("mint bridge access");

    assert_eq!(
        result
            .headers
            .get(systemprompt_identifiers::headers::SESSION_ID)
            .map(String::as_str),
        Some(supplied.as_str()),
        "minted JWT must carry the session id the bridge supplied via x-session-id"
    );

    let session = analytics
        .find_active_session_by_id(&supplied)
        .await
        .expect("session lookup ok")
        .expect("supplied session id must back an active row");
    assert_eq!(
        session.user_id.as_ref().map(|u| u.as_str()),
        Some(user_id.as_str())
    );
}

#[tokio::test]
async fn repeated_mint_with_same_session_id_is_idempotent() {
    ensure_runtime();
    let db = setup_test_db().await;
    let user_id = create_test_user(&db).await;
    let analytics = AnalyticsService::new(&db, None, None).expect("analytics service");

    let supplied = SessionId::generate();
    let headers = exchange_headers_with_session(&supplied);

    // The bridge re-mints hourly with its stable session id; both mints must
    // succeed (idempotent upsert), not fail on the existing primary key.
    issue_bridge_access(&db, &analytics, &headers, &user_id)
        .await
        .expect("first mint");
    issue_bridge_access(&db, &analytics, &headers, &user_id)
        .await
        .expect("re-mint with the same session id must not fail");

    let pool = db.pool_arc().expect("read pool");
    let count = sqlx::query_scalar!(
        "SELECT COUNT(*) FROM user_sessions WHERE session_id = $1",
        supplied.as_str()
    )
    .fetch_one(pool.as_ref())
    .await
    .expect("count query");
    assert_eq!(count, Some(1), "re-mint must reuse the single session row");
}
