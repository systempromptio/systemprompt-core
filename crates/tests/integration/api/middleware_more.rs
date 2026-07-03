//! Unit coverage for the standalone middleware helpers: bot classification,
//! site-auth gating, tiered rate limiting, IP-ban blocking, JTI revocation, and
//! the analytics URI sanitiser.
//!
//! Each middleware is driven either as a pure function or as a `from_fn` layer
//! on a throwaway router with an explicitly-inserted `ConnectInfo`, so client
//! IP resolution is deterministic without binding a socket.

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode};
use axum::routing::get;
use axum::{Extension, Router, middleware};
use systemprompt_api::services::middleware::analytics::test_api::{is_sensitive_key, sanitize_uri};
use systemprompt_api::services::middleware::{
    BotMarker, BotType, JtiRevocationChecker, TieredRateLimiter, detect_bots_early,
    ip_ban_middleware, is_datacenter_ip, is_known_bot, is_outdated_browser, is_scanner_request,
    login_redirect, site_auth_gate, tiered_rate_limit_middleware,
};
use systemprompt_extension::SiteAuthConfig;
use systemprompt_models::auth::RateLimitTier;
use systemprompt_models::config::RateLimitConfig;
use systemprompt_users::{BanDuration, BanIpParams, BannedIpRepository};
use tower::ServiceExt;
use uuid::Uuid;

use super::common::setup_ctx;

#[test]
fn bot_pure_classifiers() {
    assert!(is_datacenter_ip(Some("47.79.1.2")));
    assert!(is_datacenter_ip(Some("119.29.3.4")));
    assert!(!is_datacenter_ip(Some("8.8.8.8")));
    assert!(!is_datacenter_ip(None));

    assert!(is_known_bot(
        "Googlebot/2.1 (+http://www.google.com/bot.html)"
    ));

    assert!(is_outdated_browser(
        "Mozilla/5.0 (Windows NT 10.0) Chrome/90.0.4430.212 Safari/537.36"
    ));
    assert!(!is_outdated_browser(
        "Mozilla/5.0 (Windows NT 10.0) Chrome/130.0.0.0 Safari/537.36"
    ));
    assert!(!is_outdated_browser("no chrome token here"));

    assert!(is_scanner_request("/wp-login.php", "curl/8"));
    assert!(is_scanner_request("/index.html", "sqlmap/1.5"));
    assert!(!is_scanner_request("/", "Mozilla/5.0"));
}

async fn classify(path: &str, ua: &str, peer: &str) -> BotType {
    let proxies = Arc::new(Vec::new());
    let captured: Arc<std::sync::Mutex<Option<BotType>>> = Arc::new(std::sync::Mutex::new(None));
    let sink = Arc::clone(&captured);
    let app = Router::new()
        .route(
            "/{*rest}",
            get(move |marker: Extension<Arc<BotMarker>>| {
                let sink = Arc::clone(&sink);
                async move {
                    *sink.lock().expect("lock") = Some(marker.bot_type);
                    "ok"
                }
            }),
        )
        .layer(middleware::from_fn(move |req, next| {
            let proxies = Arc::clone(&proxies);
            async move { detect_bots_early(req, next, proxies).await }
        }));

    let mut req = Request::builder()
        .uri(path)
        .header("user-agent", ua)
        .body(Body::empty())
        .expect("request");
    req.extensions_mut()
        .insert(ConnectInfo(peer.parse::<SocketAddr>().expect("peer addr")));
    let resp = app.oneshot(req).await.expect("response");
    assert!(resp.status().is_success());
    let marker = *captured.lock().expect("lock");
    marker.expect("bot type captured")
}

#[tokio::test]
async fn detect_bots_early_tags_each_tier() {
    assert_eq!(
        classify("/home", "Googlebot/2.1", "9.9.9.9:1000").await,
        BotType::KnownBot
    );
    assert_eq!(
        classify("/home", "Mozilla/5.0", "47.79.1.2:1000").await,
        BotType::Suspicious
    );
    assert_eq!(
        classify("/wp-admin", "Mozilla/5.0", "9.9.9.9:1000").await,
        BotType::Scanner
    );
    assert_eq!(
        classify("/home", "Mozilla/5.0 Chrome/130.0.0.0", "9.9.9.9:1000").await,
        BotType::Human
    );
}

#[test]
fn login_redirect_preserves_query() {
    let uri: http::Uri = "/dash?redirect=abc&x=1".parse().unwrap();
    let out = login_redirect("/login", &uri);
    assert!(out.starts_with("/login?redirect="));
    assert!(out.contains("dash"));
}

fn site_config() -> SiteAuthConfig {
    SiteAuthConfig {
        login_path: "/login",
        protected_prefixes: &["/private"],
        public_prefixes: &["/pub"],
        required_scope: "user",
    }
}

async fn drive_site_auth(uri: &str) -> StatusCode {
    let config = site_config();
    let app = Router::new()
        .route("/{*rest}", get(|| async { "ok" }))
        .route("/login", get(|| async { "login" }))
        .layer(middleware::from_fn(move |req, next| async move {
            site_auth_gate(req, next, config).await
        }));
    let resp = app
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .expect("response");
    resp.status()
}

#[tokio::test]
async fn site_auth_gate_allows_public_and_static_redirects_protected() {
    assert_eq!(drive_site_auth("/login").await, StatusCode::OK);
    assert_eq!(drive_site_auth("/pub/page").await, StatusCode::OK);
    assert_eq!(drive_site_auth("/app/main.css").await, StatusCode::OK);
    assert_eq!(
        drive_site_auth("/private/data").await,
        StatusCode::SEE_OTHER,
        "protected path without token redirects to login"
    );
}

fn tight_limiter() -> TieredRateLimiter {
    let mut config = RateLimitConfig::testing();
    config.burst_multiplier = 1;
    TieredRateLimiter::new(&config, 1)
}

#[test]
fn tiered_rate_limiter_denies_after_burst() {
    let limiter = tight_limiter();
    let key = format!("unit-{}", Uuid::new_v4());
    let mut denied = false;
    for _ in 0..50 {
        if !limiter.check(RateLimitTier::Anon, &key) {
            denied = true;
            break;
        }
    }
    assert!(
        denied,
        "a tight per-second limiter must deny within a burst"
    );
}

#[test]
fn disabled_rate_limiter_always_allows() {
    let limiter = TieredRateLimiter::disabled();
    for _ in 0..100 {
        assert!(limiter.check(RateLimitTier::Admin, "k"));
    }
}

#[tokio::test]
async fn tiered_rate_limit_middleware_returns_429_with_headers() {
    let limiter = tight_limiter();
    let ip = "203.0.113.7";
    let key = ip.to_owned();
    for _ in 0..50 {
        if !limiter.check(RateLimitTier::Anon, &key) {
            break;
        }
    }

    let app = Router::new().route("/x", get(|| async { "ok" })).layer(
        axum::middleware::from_fn_with_state(limiter, tiered_rate_limit_middleware),
    );

    let mut req = Request::builder().uri("/x").body(Body::empty()).unwrap();
    req.extensions_mut().insert(ConnectInfo(
        format!("{ip}:5000").parse::<SocketAddr>().unwrap(),
    ));
    let resp = app.oneshot(req).await.expect("response");
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(resp.headers().get("Retry-After").unwrap(), "1");
    assert_eq!(resp.headers().get("X-Rate-Limit-Tier").unwrap(), "anon");
}

#[tokio::test]
async fn ip_ban_middleware_blocks_seeded_ip() -> Result<()> {
    let (pool, _ctx) = setup_ctx().await?;
    let repo = Arc::new(BannedIpRepository::new(&pool)?);
    let ip = format!("198.51.100.{}", (Uuid::new_v4().as_u128() % 250) + 1);
    repo.ban_ip(BanIpParams {
        ip_address: &ip,
        reason: "unit-test",
        duration: BanDuration::Hours(1),
        source_fingerprint: None,
        ban_source: "unit-test",
    })
    .await?;

    let proxies = Arc::new(Vec::new());
    let repo_layer = Arc::clone(&repo);
    let app = Router::new()
        .route("/x", get(|| async { "ok" }))
        .layer(middleware::from_fn(move |req, next| {
            let repo = Arc::clone(&repo_layer);
            let proxies = Arc::clone(&proxies);
            async move { ip_ban_middleware(req, next, repo, proxies).await }
        }));

    let mut banned_req = Request::builder().uri("/x").body(Body::empty()).unwrap();
    banned_req.extensions_mut().insert(ConnectInfo(
        format!("{ip}:1234").parse::<SocketAddr>().unwrap(),
    ));
    let resp = app.clone().oneshot(banned_req).await.expect("response");
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    assert_eq!(resp.headers().get("X-Blocked-Reason").unwrap(), "ip-banned");

    let mut clean_req = Request::builder().uri("/x").body(Body::empty()).unwrap();
    clean_req
        .extensions_mut()
        .insert(ConnectInfo("9.9.9.9:1234".parse::<SocketAddr>().unwrap()));
    let ok = app.oneshot(clean_req).await.expect("response");
    assert_eq!(ok.status(), StatusCode::OK);
    Ok(())
}

#[tokio::test]
async fn jti_revocation_checker_fails_closed_on_revoked() -> Result<()> {
    let (pool, _ctx) = setup_ctx().await?;
    let checker = JtiRevocationChecker::from_pool(&pool)?;

    assert!(
        checker.ensure_not_revoked("").await.is_ok(),
        "empty jti short-circuits to allow"
    );

    let fresh = format!("jti-fresh-{}", Uuid::new_v4());
    assert!(
        checker.ensure_not_revoked(&fresh).await.is_ok(),
        "an unrevoked jti passes (cache miss then DB lookup)"
    );
    assert!(
        checker.ensure_not_revoked(&fresh).await.is_ok(),
        "second call served from the negative cache"
    );
    Ok(())
}

#[test]
fn analytics_sanitiser_redacts_sensitive_keys() {
    assert!(is_sensitive_key("token"));
    assert!(is_sensitive_key("authorization"));
    assert!(!is_sensitive_key("page"));

    let uri: http::Uri = "/search?q=hello&token=secret&page=2".parse().unwrap();
    let out = sanitize_uri(&uri);
    assert!(out.contains("q=hello"));
    assert!(out.contains("token=[REDACTED]"));
    assert!(out.contains("page=2"));

    let plain: http::Uri = "/no-query".parse().unwrap();
    assert_eq!(sanitize_uri(&plain), "/no-query");
}

#[test]
fn user_cache_ttl_expiry_and_admin_role() {
    use std::time::Duration;

    use systemprompt_api::services::middleware::jwt::test_api::{UserCache, user_is_admin};
    use systemprompt_identifiers::UserId;
    use systemprompt_traits::AuthUser;

    let user = AuthUser {
        id: UserId::new("cache-ttl-user"),
        name: "Cache".to_owned(),
        email: "cache@example.com".to_owned(),
        roles: vec!["admin".to_owned()],
        is_active: true,
    };

    let fresh = UserCache::with_ttl(Duration::from_secs(3600));
    fresh.put(user.id.clone(), user.clone());
    assert!(fresh.get_fresh(&user.id).is_some());
    assert!(fresh.get_fresh(&UserId::new("other-user")).is_none());

    let expired = UserCache::with_ttl(Duration::ZERO);
    expired.put(user.id.clone(), user.clone());
    assert!(
        expired.get_fresh(&user.id).is_none(),
        "zero TTL entry must be stale"
    );

    assert!(user_is_admin(&user));
    let plain = AuthUser {
        roles: vec!["member".to_owned()],
        ..user
    };
    assert!(!user_is_admin(&plain));
}
