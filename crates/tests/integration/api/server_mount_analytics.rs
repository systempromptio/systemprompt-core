//! The analytics route groups on the full `setup_api_server` router, declared
//! by `analytics-http` (`systemprompt-analytics-entry`):
//!
//! - `/api/v1/analytics` (events and the live stream) refuses a caller without
//!   credentials and a user token at the admin gate, and records an admin's
//!   event;
//! - `/track/engagement` is public and rate-limited per client address under
//!   the `content` budget;
//! - the profile's disabled list decides mounting:
//!
//! | disables | analytics | engagement | analytics jobs and recording |
//! |---|---|---|---|
//! | `analytics-http` | 404 | 404 | unaffected |
//! | `content` | boot refused | | |
//! | `analytics` | boot refused (required extension) | | |

use std::net::SocketAddr;

use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use http::Method;
use systemprompt_analytics::repository::AnalyticsRepositories;
use systemprompt_analytics_entry::extension::{ANALYTICS_HTTP_EXTENSION, AnalyticsHttpExtension};
use systemprompt_extension::{Extension, ExtensionContext, ExtensionRegistry};
use systemprompt_identifiers::{ExtensionId, JwtToken};
use systemprompt_manifest::profile::RateLimitsConfig;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_app_context_with_extensions, fixture_config,
    seed_admin_credential, seed_bridge_credential, test_db_pool,
};
use systemprompt_traits::{DynSessionBehaviourScorer, registered_jobs};
use tower::ServiceExt;

const EVENTS: &str = "/api/v1/analytics/events";
const STREAM: &str = "/api/v1/analytics/stream";
const ENGAGEMENT: &str = "/track/engagement";
const ENGAGEMENT_BATCH: &str = "/track/engagement/batch";

const GROUPS: [(Method, &str); 4] = [
    (Method::POST, EVENTS),
    (Method::GET, STREAM),
    (Method::POST, ENGAGEMENT),
    (Method::POST, ENGAGEMENT_BATCH),
];

fn page_view() -> String {
    serde_json::json!({
        "event_type": "page_view",
        "page_url": "https://example.com/analytics-mount",
    })
    .to_string()
}

fn engagement_event() -> String {
    serde_json::json!({
        "page_url": "https://example.com/analytics-mount",
        "event_type": "scroll",
        "time_on_page_ms": 1200,
    })
    .to_string()
}

async fn context(
    rate_limits: RateLimitsConfig,
    disabled: &[ExtensionId],
) -> anyhow::Result<std::sync::Arc<systemprompt_runtime::AppContext>> {
    let bootstrap = ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let mut config = fixture_config(&bootstrap.database_url);
    config.rate_limits = rate_limits;
    config.trusted_proxies = Vec::new();
    fixture_app_context_with_extensions(&pool, config, ExtensionRegistry::discover()?, disabled)
}

async fn full_router(
    rate_limits: RateLimitsConfig,
    disabled: &[ExtensionId],
) -> anyhow::Result<Router> {
    let ctx = context(rate_limits, disabled).await?;
    systemprompt_api::services::server::setup_api_server(
        &ctx,
        None,
        &std::sync::Arc::new(systemprompt_api::services::server::ReadinessState::new()),
    )
    .map_err(|e| anyhow::anyhow!("setup_api_server failed: {e}"))
}

fn request(
    method: Method,
    uri: &str,
    body: String,
    bearer: Option<&JwtToken>,
    peer: &str,
) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::HOST, "127.0.0.1")
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(token) = bearer {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {}", token.as_str()));
    }
    let mut req = builder.body(Body::from(body)).expect("request build");
    req.extensions_mut().insert(ConnectInfo(
        peer.parse::<SocketAddr>().expect("peer address must parse"),
    ));
    req
}

fn anonymous(method: Method, uri: &str) -> Request<Body> {
    request(method, uri, "{}".to_owned(), None, "127.0.0.1:40000")
}

#[test]
fn the_routes_extension_carries_the_declared_id() {
    assert_eq!(AnalyticsHttpExtension.id(), ANALYTICS_HTTP_EXTENSION);
    assert_eq!(AnalyticsHttpExtension.dependencies(), vec!["analytics", "content"]);
}

#[tokio::test]
async fn the_analytics_group_refuses_an_anonymous_caller() -> anyhow::Result<()> {
    let app = full_router(RateLimitsConfig::disabled(), &[]).await?;
    for (method, uri) in [(Method::POST, EVENTS), (Method::GET, STREAM)] {
        let status = app
            .clone()
            .oneshot(anonymous(method.clone(), uri))
            .await?
            .status();
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{method} {uri}");
    }
    Ok(())
}

#[tokio::test]
async fn the_analytics_group_refuses_a_user_token_at_the_admin_gate() -> anyhow::Result<()> {
    let app = full_router(RateLimitsConfig::disabled(), &[]).await?;
    let user = seed_bridge_credential(&test_db_pool().await, "analytics-user").await?;
    for (method, uri) in [(Method::POST, EVENTS), (Method::GET, STREAM)] {
        let status = app
            .clone()
            .oneshot(request(
                method.clone(),
                uri,
                page_view(),
                Some(&user.jwt),
                "127.0.0.1:40001",
            ))
            .await?
            .status();
        assert_eq!(status, StatusCode::FORBIDDEN, "{method} {uri}");
    }
    Ok(())
}

#[tokio::test]
async fn the_analytics_group_records_an_admins_event() -> anyhow::Result<()> {
    let app = full_router(RateLimitsConfig::disabled(), &[]).await?;
    let admin = seed_admin_credential(&test_db_pool().await, "analytics-admin").await?;
    let status = app
        .oneshot(request(
            Method::POST,
            EVENTS,
            page_view(),
            Some(&admin.jwt),
            "127.0.0.1:40002",
        ))
        .await?
        .status();
    assert_eq!(status, StatusCode::CREATED);
    Ok(())
}

#[tokio::test]
async fn engagement_is_admitted_without_credentials() -> anyhow::Result<()> {
    let app = full_router(RateLimitsConfig::disabled(), &[]).await?;
    let status = app
        .oneshot(request(
            Method::POST,
            ENGAGEMENT,
            engagement_event(),
            None,
            "127.0.0.1:40003",
        ))
        .await?
        .status();
    assert!(
        ![
            StatusCode::UNAUTHORIZED,
            StatusCode::FORBIDDEN,
            StatusCode::NOT_FOUND,
            StatusCode::METHOD_NOT_ALLOWED,
        ]
        .contains(&status),
        "the public engagement group must reach its handler, got {status}"
    );
    Ok(())
}

#[tokio::test]
async fn engagement_is_rate_limited_per_client_address() -> anyhow::Result<()> {
    let app = full_router(
        RateLimitsConfig {
            content_per_second: 1,
            burst_multiplier: 1,
            disabled: false,
            ..RateLimitsConfig::testing()
        },
        &[],
    )
    .await?;
    let mut statuses = Vec::new();
    for _ in 0..4 {
        let resp = app
            .clone()
            .oneshot(request(
                Method::POST,
                ENGAGEMENT,
                engagement_event(),
                None,
                "203.0.113.91:41000",
            ))
            .await?;
        statuses.push(resp.status());
    }
    assert!(
        statuses.contains(&StatusCode::TOO_MANY_REQUESTS),
        "a burst above the content budget must be throttled, got {statuses:?}"
    );
    Ok(())
}

#[tokio::test]
async fn disabling_the_routes_extension_unmounts_both_groups_and_keeps_the_analytics_jobs()
-> anyhow::Result<()> {
    let disabled = [ExtensionId::new(ANALYTICS_HTTP_EXTENSION)];
    let ctx = context(RateLimitsConfig::disabled(), &disabled).await?;
    let app = systemprompt_api::services::server::setup_api_server(
        &ctx,
        None,
        &std::sync::Arc::new(systemprompt_api::services::server::ReadinessState::new()),
    )
    .map_err(|e| anyhow::anyhow!("setup_api_server failed: {e}"))?;
    for (method, uri) in GROUPS {
        let status = app
            .clone()
            .oneshot(anonymous(method.clone(), uri))
            .await?
            .status();
        assert_eq!(status, StatusCode::NOT_FOUND, "{method} {uri}");
    }

    let enabled: Vec<String> = ctx
        .extension_registry()
        .enabled_extensions(&disabled)?
        .iter()
        .map(|ext| ext.id().to_owned())
        .collect();
    assert!(enabled.iter().any(|id| id == "analytics"), "{enabled:?}");
    let analytics_jobs: Vec<&str> = registered_jobs()
        .filter(|registration| registration.owner == "analytics")
        .map(|registration| registration.job.name())
        .collect();
    for job in ["malicious_ip_blacklist", "behavioral_analysis"] {
        assert!(analytics_jobs.contains(&job), "{job} in {analytics_jobs:?}");
    }

    let state = ctx.extension_state();
    assert!(state.find::<std::sync::Arc<AnalyticsRepositories>>().is_some());
    assert!(
        state.find::<DynSessionBehaviourScorer>().is_some(),
        "request recording keeps its behaviour scorer"
    );
    Ok(())
}

fn content_and_its_dependants_except_the_routes() -> anyhow::Result<Vec<ExtensionId>> {
    let registry = ExtensionRegistry::discover()?;
    let mut disabled = vec!["content".to_owned()];
    loop {
        let added: Vec<String> = registry
            .extensions()
            .iter()
            .filter(|ext| ext.id() != ANALYTICS_HTTP_EXTENSION)
            .filter(|ext| !disabled.iter().any(|id| id == ext.id()))
            .filter(|ext| {
                ext.dependencies()
                    .iter()
                    .any(|dep| disabled.iter().any(|id| id == dep))
            })
            .map(|ext| ext.id().to_owned())
            .collect();
        if added.is_empty() {
            break;
        }
        disabled.extend(added);
    }
    Ok(disabled.into_iter().map(ExtensionId::new).collect())
}

#[tokio::test]
async fn disabling_content_while_the_routes_stay_enabled_refuses_boot() -> anyhow::Result<()> {
    let disabled = content_and_its_dependants_except_the_routes()?;
    let refused = full_router(RateLimitsConfig::disabled(), &disabled).await;
    let error = refused.expect_err("routes depending on content refuse boot");
    let rendered = format!("{error:#}");
    assert!(
        rendered.contains("'content' is disabled")
            && rendered.contains(&format!("'{ANALYTICS_HTTP_EXTENSION}' depends on it")),
        "{rendered}"
    );
    Ok(())
}

#[tokio::test]
async fn disabling_analytics_refuses_boot_as_a_required_extension() -> anyhow::Result<()> {
    let refused = full_router(
        RateLimitsConfig::disabled(),
        &[
            ExtensionId::new("analytics"),
            ExtensionId::new(ANALYTICS_HTTP_EXTENSION),
        ],
    )
    .await;
    let error = refused.expect_err("a profile that disables analytics refuses boot");
    let rendered = format!("{error:#}");
    assert!(
        rendered.contains("Extension 'analytics' is required and cannot be disabled"),
        "{rendered}"
    );
    Ok(())
}
