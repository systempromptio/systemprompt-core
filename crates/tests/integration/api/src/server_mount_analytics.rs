//! The analytics route groups on the full `setup_api_server` router, declared
//! by `analytics-http` (`systemprompt-analytics-entry`):
//!
//! - `/api/v1/analytics` (events and the live stream) refuses a caller without
//!   credentials and a user token at the admin gate, and records an admin's
//!   event;
//! - the live stream answers an admin with an event stream, so the analytics
//!   broadcaster the API hands to the `Analytics` row reaches its handler
//!   (without it the handler answers 503);
//! - `/track/engagement` is public and rate-limited per client address under
//!   the `content` budget, and nested at its prefix: it answers exactly the
//!   paths a router nested the way the server mounted it at the base answers
//!   (trailing slashes included), and nothing outside the prefix;
//! - the profile's disabled list decides mounting:
//!
//! | disables | analytics | engagement | analytics jobs and recording |
//! |---|---|---|---|
//! | `analytics-http` | 404 | 404 | unaffected |
//! | `content` | boot refused | | |
//! | `analytics` (with or without `analytics-http`) | boot refused (required extension) | | |

use std::net::SocketAddr;

use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use http::Method;
use systemprompt::CompositionError;
use systemprompt_analytics::repository::AnalyticsRepositories;
use systemprompt_analytics_entry::extension::{ANALYTICS_HTTP_EXTENSION, AnalyticsHttpExtension};
use systemprompt_extension::contribute::ContributionSink;
use systemprompt_extension::{Contributions, Extension};
use systemprompt_identifiers::{ExtensionId, JwtToken};
use systemprompt_manifest::profile::RateLimitsConfig;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_app_context_with_extensions, fixture_config,
    seed_admin_credential, seed_bridge_credential, test_db_pool,
};
use systemprompt_traits::DynSessionBehaviourScorer;
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
    let mut config = fixture_config(&bootstrap.database_url);
    config.rate_limits = rate_limits;
    config.trusted_proxies = Vec::new();
    fixture_app_context_with_extensions(config, super::common::reference_admitted(disabled)?).await
}

async fn full_router(
    rate_limits: RateLimitsConfig,
    disabled: &[ExtensionId],
) -> anyhow::Result<Router> {
    let ctx = context(rate_limits, disabled).await?;
    systemprompt_api::services::server::setup_api_server(
        &ctx,
        systemprompt_api::services::server::routes::Composed {
            admitted: ctx.admitted(),
            kernel: ctx.kernel(),
        },
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
fn the_routes_extension_carries_the_declared_id_and_requires_nothing() {
    assert_eq!(
        AnalyticsHttpExtension.metadata().id,
        ANALYTICS_HTTP_EXTENSION
    );
    let mut sink = ContributionSink::default();
    let mut contributions =
        Contributions::__new(ExtensionId::new(ANALYTICS_HTTP_EXTENSION), &mut sink);
    AnalyticsHttpExtension
        .contribute(&mut contributions)
        .expect("the analytics-http member contributes");
    assert!(sink.requires().is_empty(), "{:?}", sink.requires());
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
    let resp = app
        .oneshot(request(
            Method::POST,
            EVENTS,
            page_view(),
            Some(&admin.jwt),
            "127.0.0.1:40002",
        ))
        .await?;
    let status = resp.status();
    let body = axum::body::to_bytes(resp.into_body(), 64 * 1024).await?;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "{}",
        String::from_utf8_lossy(&body)
    );
    Ok(())
}

#[tokio::test]
async fn the_analytics_stream_receives_the_kernel_broadcaster() -> anyhow::Result<()> {
    let app = full_router(RateLimitsConfig::disabled(), &[]).await?;
    let admin = seed_admin_credential(&test_db_pool().await, "analytics-stream-admin").await?;
    let mut req = request(
        Method::GET,
        STREAM,
        String::new(),
        Some(&admin.jwt),
        "127.0.0.1:40004",
    );
    req.headers_mut()
        .insert(header::ACCEPT, "text/event-stream".parse()?);
    let resp = app.oneshot(req).await?;
    assert_eq!(resp.status(), StatusCode::OK);
    let content_type = resp
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    assert!(
        content_type.starts_with("text/event-stream"),
        "the stream must reach its handler with the broadcaster, got {content_type}"
    );
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
        systemprompt_api::services::server::routes::Composed {
            admitted: ctx.admitted(),
            kernel: ctx.kernel(),
        },
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

    let enabled: Vec<&str> = ctx
        .admitted()
        .built()
        .members()
        .map(|member| member.owner.as_str())
        .collect();
    assert!(enabled.contains(&"analytics"), "{enabled:?}");
    assert!(!enabled.contains(&ANALYTICS_HTTP_EXTENSION), "{enabled:?}");
    let analytics_jobs: Vec<&str> = ctx
        .admitted()
        .built()
        .members()
        .filter(|member| member.owner.as_str() == "analytics")
        .flat_map(|member| member.sink.jobs())
        .map(|job| job.job().name())
        .collect();
    for job in ["malicious_ip_blacklist", "behavioral_analysis"] {
        assert!(analytics_jobs.contains(&job), "{job} in {analytics_jobs:?}");
    }

    let state = ctx.kernel().state();
    assert!(
        state
            .find::<std::sync::Arc<AnalyticsRepositories>>()
            .is_some()
    );
    assert!(
        state.find::<DynSessionBehaviourScorer>().is_some(),
        "request recording keeps its behaviour scorer"
    );
    Ok(())
}

fn content_and_its_dependants_except_the_routes() -> Vec<ExtensionId> {
    let mut disabled = vec![ExtensionId::new("content")];
    loop {
        let mut profile = (*systemprompt_test_fixtures::test_boot_profile()).clone();
        profile.extensions.disabled = disabled
            .iter()
            .cloned()
            .map(systemprompt_manifest::DisabledExtension::literal)
            .collect();
        match systemprompt::reference().build(&profile) {
            Err(
                CompositionError::DisabledDependency { owner, .. }
                | CompositionError::WithheldDependency { owner, .. },
            ) if owner.as_str() != ANALYTICS_HTTP_EXTENSION => {
                disabled.push(owner);
            },
            _ => return disabled,
        }
    }
}

#[tokio::test]
async fn disabling_content_while_the_routes_stay_enabled_refuses_boot() -> anyhow::Result<()> {
    let disabled = content_and_its_dependants_except_the_routes();
    let refused = full_router(RateLimitsConfig::disabled(), &disabled).await;
    let error = refused.expect_err("routes depending on content refuse boot");
    let rendered = format!("{error:#}");
    assert!(
        rendered.contains(&format!("{ANALYTICS_HTTP_EXTENSION} requires"))
            && rendered.contains("its provider content is disabled"),
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
        rendered.contains("required extension analytics cannot be disabled or removed"),
        "{rendered}"
    );
    Ok(())
}

#[tokio::test]
async fn disabling_only_analytics_refuses_boot_as_a_required_extension() -> anyhow::Result<()> {
    let refused = full_router(
        RateLimitsConfig::disabled(),
        &[ExtensionId::new("analytics")],
    )
    .await;
    let error = refused.expect_err("analytics-http enabled without analytics refuses boot");
    let rendered = format!("{error:#}");
    assert!(
        rendered.contains("required extension analytics cannot be disabled or removed"),
        "{rendered}"
    );
    Ok(())
}

fn reached_handler(status: StatusCode) -> bool {
    ![StatusCode::NOT_FOUND, StatusCode::METHOD_NOT_ALLOWED].contains(&status)
}

async fn reference_nest_status(uri: &str) -> anyhow::Result<StatusCode> {
    let reference = Router::new().nest(
        ENGAGEMENT,
        Router::new()
            .route("/", axum::routing::post(|| async { StatusCode::OK }))
            .route("/batch", axum::routing::post(|| async { StatusCode::OK })),
    );
    Ok(reference
        .oneshot(anonymous(Method::POST, uri))
        .await?
        .status())
}

#[tokio::test]
async fn engagement_answers_exactly_where_the_base_nesting_answered() -> anyhow::Result<()> {
    let app = full_router(RateLimitsConfig::disabled(), &[]).await?;
    for (index, uri) in [
        ENGAGEMENT,
        ENGAGEMENT_BATCH,
        "/track/engagement/",
        "/track/engagement/batch/",
        "/track/engagement/elsewhere",
        "/track/elsewhere",
    ]
    .into_iter()
    .enumerate()
    {
        let reference = reference_nest_status(uri).await?;
        let served = app
            .clone()
            .oneshot(request(
                Method::POST,
                uri,
                engagement_event(),
                None,
                &format!("127.0.0.1:{}", 40100 + index),
            ))
            .await?
            .status();
        assert_eq!(
            reached_handler(served),
            reference == StatusCode::OK,
            "{uri}: mounted {served}, base nesting {reference}"
        );
    }
    Ok(())
}
