//! The global window's `DynRateLimitBuckets` seam:
//! `RateLimitState::from_kernel` charges the users member's windows only when
//! `users` provided them; a kernel without them, or with windows another member
//! provided, charges the per-replica `MemoryRateLimitBuckets`, which refuse a
//! spent window like the shared ones do.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::extract::{ConnectInfo, Request};
use axum::http::StatusCode;
use axum::routing::get;
use chrono::{DateTime, Duration, Utc};
use systemprompt_api::services::middleware::{MemoryRateLimitBuckets, RateLimitState, RouterExt};
use systemprompt_identifiers::{AgentName, ContextId, ExtensionId, SessionId, TraceId, UserId};
use systemprompt_manifest::Config;
use systemprompt_manifest::profile::RateLimitsConfig;
use systemprompt_models::RequestContext;
use systemprompt_models::auth::UserType;
use systemprompt_provider_contracts::Dependencies;
use systemprompt_test_fixtures::fixture_config;
use systemprompt_traits::{DynRateLimitBuckets, RateLimitBuckets, RepositoryError};
use tower::ServiceExt;

const SCOPE: &str = "rate-limit-buckets";

#[derive(Debug)]
struct Spent;

#[async_trait::async_trait]
impl RateLimitBuckets for Spent {
    async fn hit(
        &self,
        _user_id: &UserId,
        _scope: &str,
        _window_start: DateTime<Utc>,
    ) -> Result<i64, RepositoryError> {
        Ok(i64::MAX)
    }
}

const PER_SECOND: u64 = 5;
const WINDOW_SECS: i64 = 10;

fn config() -> Config {
    let mut config = fixture_config("postgres://localhost/rate-limit-buckets");
    config.rate_limits = RateLimitsConfig {
        disabled: false,
        burst_multiplier: 1,
        ..RateLimitsConfig::default()
    };
    config.trusted_proxies = Vec::new();
    config
}

fn authed_request(user: &str) -> Request<Body> {
    let mut req = Request::builder()
        .uri("/")
        .body(Body::empty())
        .expect("test request must build");
    req.extensions_mut()
        .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 4242))));
    req.extensions_mut().insert(
        RequestContext::new(
            SessionId::generate(),
            TraceId::generate(),
            ContextId::generate(),
            AgentName::try_new("rate-limit-buckets").expect("valid AgentName"),
            systemprompt_identifiers::Actor::user(UserId::new(user)),
        )
        .with_user_type(UserType::User),
    );
    req
}

async fn first_status(limits: &RateLimitState, user: &str) -> StatusCode {
    Router::new()
        .route("/", get(|| async { "ok" }))
        .with_rate_limit(limits, PER_SECOND, SCOPE)
        .expect("limiter must build")
        .oneshot(authed_request(user))
        .await
        .expect("request must complete")
        .status()
}

fn provided_by(owner: &str) -> Dependencies {
    let buckets: DynRateLimitBuckets = Arc::new(Spent);
    let mut state = Dependencies::new();
    state
        .provide(&ExtensionId::new(owner), Arc::new(buckets))
        .expect("the windows are provided once");
    state
}

fn window_start(now: DateTime<Utc>) -> DateTime<Utc> {
    let secs = now.timestamp();
    DateTime::from_timestamp(secs - secs.rem_euclid(WINDOW_SECS), 0).expect("timestamp")
}

#[tokio::test]
async fn memory_windows_count_per_user_scope_and_window() {
    let buckets =
        MemoryRateLimitBuckets::with_capacity(std::num::NonZeroU64::new(262_144).unwrap());
    let user = UserId::new("memory-windows");
    let start = DateTime::from_timestamp(1_000, 0).expect("timestamp");

    assert_eq!(buckets.hit(&user, "a", start).await.expect("hit"), 1);
    assert_eq!(buckets.hit(&user, "a", start).await.expect("hit"), 2);
    assert_eq!(buckets.hit(&user, "b", start).await.expect("hit"), 1);
    assert_eq!(
        buckets
            .hit(&UserId::new("other"), "a", start)
            .await
            .expect("hit"),
        1
    );
    let later = start + Duration::seconds(WINDOW_SECS);
    assert_eq!(buckets.hit(&user, "a", later).await.expect("hit"), 1);
    assert_eq!(
        buckets.hit(&user, "a", start).await.expect("hit"),
        1,
        "a window older than the newest is forgotten, never resumed"
    );
}

#[tokio::test]
async fn a_spent_memory_window_refuses_the_caller() {
    let buckets = Arc::new(MemoryRateLimitBuckets::with_capacity(
        std::num::NonZeroU64::new(262_144).unwrap(),
    ));
    let limits = RateLimitState::new(&config(), buckets.clone());
    let budget = i64::try_from(PER_SECOND).expect("small") * WINDOW_SECS;

    let status = loop {
        let user = format!("spent-{}", uuid::Uuid::new_v4().simple());
        let start = window_start(Utc::now());
        for _ in 0..budget {
            buckets
                .hit(&UserId::new(&user), SCOPE, start)
                .await
                .expect("preload hit");
        }
        let status = first_status(&limits, &user).await;
        if window_start(Utc::now()) == start {
            break status;
        }
    };

    assert_eq!(
        status,
        StatusCode::TOO_MANY_REQUESTS,
        "the per-replica window refuses past its budget, never admits unmetered"
    );
}

#[tokio::test]
async fn a_kernel_without_the_users_windows_still_charges_a_window() {
    let limits = RateLimitState::from_kernel(&config(), &Dependencies::new());

    assert_eq!(
        first_status(&limits, "absent-users").await,
        StatusCode::OK,
        "a caller inside the per-replica budget is served"
    );
}

#[tokio::test]
async fn windows_another_member_provided_are_never_charged() {
    let limits = RateLimitState::from_kernel(&config(), &provided_by("look-alike"));

    assert_eq!(
        first_status(&limits, "look-alike").await,
        StatusCode::OK,
        "windows a non-users member provided decide nothing; the per-replica window does"
    );
}

#[tokio::test]
async fn the_users_windows_are_the_ones_charged() {
    let limits = RateLimitState::from_kernel(&config(), &provided_by("users"));

    assert_eq!(
        first_status(&limits, "users-provided").await,
        StatusCode::TOO_MANY_REQUESTS,
        "the users member's spent window refuses the caller"
    );
}
