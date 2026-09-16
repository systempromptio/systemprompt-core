//! Mount-level contracts on the full `setup_api_server` router: the gateway
//! group is rate-limited like every other route group, and an unissued bearer
//! presented to the nested MCP proxy is unauthorized on every server.

use std::net::SocketAddr;

use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use http::Method;
use systemprompt_api::services::server::setup_api_server;
use systemprompt_models::config::RateLimitConfig;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_app_context_with_config, fixture_config, fixture_db_pool,
};
use tower::ServiceExt;

async fn full_router(rate_limits: RateLimitConfig) -> anyhow::Result<Router> {
    let bootstrap = ensure_test_bootstrap();
    let pool = fixture_db_pool(&bootstrap.database_url).await?;
    let mut config = fixture_config(&bootstrap.database_url);
    config.rate_limits = rate_limits;
    config.cors_allowed_origins = vec!["http://127.0.0.1".to_owned()];
    config.trusted_proxies = Vec::new();
    let ctx = fixture_app_context_with_config(&pool, config)?;
    setup_api_server(&ctx, None).map_err(|e| anyhow::anyhow!("setup_api_server failed: {e}"))
}

fn post(uri: &str, peer: &str, headers: &[(&str, &str)]) -> Request<Body> {
    let mut req = Request::builder()
        .method(Method::POST)
        .uri(uri)
        .header(header::HOST, "127.0.0.1")
        .header(header::CONTENT_TYPE, "application/json");
    for (name, value) in headers {
        req = req.header(*name, *value);
    }
    let mut req = req
        .body(Body::from(
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#,
        ))
        .expect("request build");
    req.extensions_mut().insert(ConnectInfo(
        peer.parse::<SocketAddr>().expect("peer address must parse"),
    ));
    req
}

// Why: `/otel` and the bridge credential exchanges answer to anyone who can
// reach the gateway; without a limiter on the mount they are the cheapest
// storage-fill and credential-guessing surfaces the server exposes.
#[tokio::test]
async fn gateway_mount_is_rate_limited_per_client_ip() -> anyhow::Result<()> {
    let app = full_router(RateLimitConfig {
        gateway_per_second: 1,
        burst_multiplier: 1,
        disabled: false,
        ..RateLimitConfig::testing()
    })
    .await?;

    let peer = "203.0.113.77:41000";
    let mut statuses = Vec::new();
    for _ in 0..4 {
        let resp = app.clone().oneshot(post("/v1/otel", peer, &[])).await?;
        statuses.push(resp.status());
    }

    assert!(
        statuses.contains(&StatusCode::TOO_MANY_REQUESTS),
        "a burst above the configured gateway limit must be throttled, got {statuses:?}"
    );
    assert!(
        statuses
            .iter()
            .all(|s| *s == StatusCode::TOO_MANY_REQUESTS || *s == StatusCode::UNAUTHORIZED),
        "no request may be admitted without a credential, got {statuses:?}"
    );
    Ok(())
}

// Why: the MCP router is nested at `/api/v1/mcp`, so inside its middleware
// `request.uri()` reads `/<server>/mcp`. A credential nobody issued must be
// refused as unauthenticated on every server, never classified by scope.
#[tokio::test]
async fn an_unissued_bearer_on_the_nested_mcp_proxy_is_unauthorized_on_every_server()
-> anyhow::Result<()> {
    let app = full_router(RateLimitConfig::disabled()).await?;

    for (server, peer, session) in [
        ("evaluation_fixture", "203.0.113.78:41000", "sess-fixture"),
        ("other-server", "203.0.113.79:41000", "sess-other"),
    ] {
        let resp = app
            .clone()
            .oneshot(post(
                &format!("/api/v1/mcp/{server}/mcp"),
                peer,
                &[
                    ("authorization", "Bearer not-an-issued-credential"),
                    ("x-session-id", session),
                ],
            ))
            .await?;
        assert_eq!(
            resp.status(),
            StatusCode::UNAUTHORIZED,
            "an unissued bearer on {server} is a 401, never a scope verdict"
        );
    }
    Ok(())
}
