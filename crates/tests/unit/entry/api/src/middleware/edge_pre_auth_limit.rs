use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::Router;
use axum::body::Body;
use axum::extract::{ConnectInfo, Request};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::routing::get;
use systemprompt_api::services::middleware::{RateLimitState, with_pre_auth_ip_limit};
use systemprompt_manifest::profile::RateLimitsConfig;
use systemprompt_test_fixtures::{closed_db_pool, fixture_config};
use systemprompt_users::UserRateLimitBucketRepository;
use tower::ServiceExt;

async fn limits(rate_limits: RateLimitsConfig) -> RateLimitState {
    let mut config = fixture_config("postgres://localhost/pre-auth");
    config.rate_limits = rate_limits;
    config.trusted_proxies = Vec::new();
    let db = closed_db_pool().await;
    RateLimitState::new(&config, Arc::new(UserRateLimitBucketRepository::new(&db)))
}

fn tight() -> RateLimitsConfig {
    RateLimitsConfig {
        disabled: false,
        pre_auth_ip_per_second: 1,
        burst_multiplier: 2,
        ..RateLimitsConfig::default()
    }
}

fn counting_auth_router(calls: &Arc<AtomicUsize>) -> Router {
    let calls = Arc::clone(calls);
    Router::new()
        .route("/", get(|| async { "ok" }))
        .layer(axum::middleware::from_fn(
            move |req: Request, next: Next| {
                let calls = Arc::clone(&calls);
                async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    next.run(req).await
                }
            },
        ))
}

fn forged_bearer_from(ip: [u8; 4]) -> Request<Body> {
    let mut req = Request::builder()
        .uri("/")
        .header("authorization", "Bearer forged.token.value")
        .body(Body::empty())
        .expect("request builds");
    req.extensions_mut()
        .insert(ConnectInfo(SocketAddr::from((ip, 4242))));
    req
}

#[tokio::test]
async fn a_credential_flood_from_one_address_is_refused_before_authentication_runs() {
    let calls = Arc::new(AtomicUsize::new(0));
    let app = with_pre_auth_ip_limit(counting_auth_router(&calls), &limits(tight()).await)
        .expect("limiter builds");

    let mut statuses = Vec::new();
    for _ in 0..6 {
        let resp = app
            .clone()
            .oneshot(forged_bearer_from([198, 51, 100, 1]))
            .await
            .expect("infallible");
        statuses.push(resp.status());
    }

    assert_eq!(calls.load(Ordering::SeqCst), 2, "{statuses:?}");
    assert_eq!(
        statuses
            .iter()
            .filter(|s| **s == StatusCode::TOO_MANY_REQUESTS)
            .count(),
        4,
        "{statuses:?}"
    );
}

#[tokio::test]
async fn another_address_keeps_its_own_pre_auth_budget() {
    let calls = Arc::new(AtomicUsize::new(0));
    let app = with_pre_auth_ip_limit(counting_auth_router(&calls), &limits(tight()).await)
        .expect("limiter builds");

    for _ in 0..3 {
        let _ = app
            .clone()
            .oneshot(forged_bearer_from([198, 51, 100, 1]))
            .await
            .expect("infallible");
    }
    let other = app
        .oneshot(forged_bearer_from([203, 0, 113, 9]))
        .await
        .expect("infallible");

    assert_eq!(other.status(), StatusCode::OK);
}

#[tokio::test]
async fn a_disabled_rate_limit_config_mounts_no_pre_auth_limiter() {
    let calls = Arc::new(AtomicUsize::new(0));
    let config = RateLimitsConfig {
        disabled: true,
        ..tight()
    };
    let app = with_pre_auth_ip_limit(counting_auth_router(&calls), &limits(config).await)
        .expect("limiter builds");

    for _ in 0..6 {
        let resp = app
            .clone()
            .oneshot(forged_bearer_from([198, 51, 100, 1]))
            .await
            .expect("infallible");
        assert_eq!(resp.status(), StatusCode::OK);
    }
    assert_eq!(calls.load(Ordering::SeqCst), 6);
}
