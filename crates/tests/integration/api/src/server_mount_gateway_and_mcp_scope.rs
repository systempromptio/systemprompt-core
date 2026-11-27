//! Mount-level contracts on the full `setup_api_server` router: the gateway
//! group is rate-limited like every other route group, the bridge release
//! routes draw a rate of their own, and an unissued bearer
//! presented to the nested MCP proxy is unauthorized on every server.

use std::net::SocketAddr;

use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use http::Method;
use systemprompt_api::services::server::setup_api_server;
use systemprompt_manifest::profile::RateLimitsConfig;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_app_context_with_config, fixture_config,
};
use tower::ServiceExt;

async fn full_router(rate_limits: RateLimitsConfig) -> anyhow::Result<Router> {
    let bootstrap = ensure_test_bootstrap();
    let mut config = fixture_config(&bootstrap.database_url);
    config.rate_limits = rate_limits;
    config.cors_allowed_origins = vec!["http://127.0.0.1".to_owned()];
    config.trusted_proxies = Vec::new();
    let ctx = fixture_app_context_with_config(config).await?;
    setup_api_server(
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
    let app = full_router(RateLimitsConfig {
        gateway_per_second: 1,
        burst_multiplier: 1,
        disabled: false,
        ..RateLimitsConfig::testing()
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

// Why: one platform of the release feed is served to a caller with no
// credential and each uncached call is an upstream fetch on the operator's
// token, so the feed and the download draw a small fixed rate of their own.
// The gateway budget here is far above the burst, so a refusal can only come
// from that rate.
#[tokio::test]
async fn the_bridge_release_routes_draw_their_own_small_budget() -> anyhow::Result<()> {
    let app = full_router(RateLimitsConfig {
        burst_multiplier: 1,
        disabled: false,
        ..RateLimitsConfig::testing()
    })
    .await?;

    for (uri, peer) in [
        ("/v1/bridge/latest?platform=android", "203.0.113.80:41000"),
        ("/v1/bridge/download/android", "203.0.113.81:41000"),
    ] {
        let mut statuses = Vec::new();
        for _ in 0..12 {
            let mut request = Request::builder()
                .uri(uri)
                .header(header::HOST, "127.0.0.1")
                .body(Body::empty())?;
            request
                .extensions_mut()
                .insert(ConnectInfo(peer.parse::<SocketAddr>()?));
            statuses.push(app.clone().oneshot(request).await?.status());
        }
        assert!(
            statuses.contains(&StatusCode::TOO_MANY_REQUESTS),
            "a burst on {uri} above the release rate must be throttled, got {statuses:?}"
        );
        assert_ne!(
            statuses[0],
            StatusCode::TOO_MANY_REQUESTS,
            "the first request on {uri} is inside the rate"
        );
    }
    Ok(())
}

// Why: the MCP router is nested at `/api/v1/mcp`, so inside its middleware
// `request.uri()` reads `/<server>/mcp`. A credential nobody issued must be
// refused as unauthenticated on every server, never classified by scope.
#[tokio::test]
async fn an_unissued_bearer_on_the_nested_mcp_proxy_is_unauthorized_on_every_server()
-> anyhow::Result<()> {
    let app = full_router(RateLimitsConfig::disabled()).await?;

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
