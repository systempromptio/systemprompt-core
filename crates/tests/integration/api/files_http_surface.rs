//! The routes the `files-http` member's surface builds, on the real router
//! the API mount layers. Unauthenticated, documents, folders, uploads and
//! issues answer 401 at their gates; the public-link open reaches its own
//! handler, whose refusal page only it produces. With `files-http` disabled
//! none of them is mounted and each answers 404 without that page.

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use systemprompt_api::services::server::{ReadinessState, setup_api_server};
use systemprompt_extension::reserved::CoreId;
use systemprompt_identifiers::ExtensionId;
use systemprompt_manifest::profile::RateLimitsConfig;
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_config};
use tower::ServiceExt;

const UNAVAILABLE: &str = "This link is unavailable";

const GATED: [&str; 5] = [
    "/api/v1/documents",
    "/api/v1/document-folders",
    "/api/v1/uploads/00000000-0000-4000-8000-000000000000",
    "/api/v1/issues",
    "/api/v1/issues/admin",
];

async fn router(disabled: &[ExtensionId]) -> Result<Router> {
    let bootstrap = ensure_test_bootstrap();
    let mut config = fixture_config(&bootstrap.database_url);
    config.rate_limits = RateLimitsConfig::disabled();
    config.trusted_proxies = Vec::new();
    let ctx = systemprompt_test_fixtures::fixture_app_context_with_extensions(
        config,
        super::common::reference_admitted(disabled)?,
    )
    .await?;
    setup_api_server(
        &ctx,
        systemprompt_api::services::server::routes::Composed {
            admitted: ctx.admitted(),
            kernel: ctx.kernel(),
        },
        None,
        &Arc::new(ReadinessState::new()),
    )
    .map_err(|e| anyhow::anyhow!("setup_api_server failed: {e}"))
}

async fn get(app: &Router, uri: &str) -> Result<(StatusCode, String)> {
    let mut req = Request::builder()
        .method("GET")
        .uri(uri)
        .header(header::HOST, "127.0.0.1")
        .header(header::USER_AGENT, "files-http-surface-test/1.0")
        .body(Body::empty())?;
    req.extensions_mut()
        .insert(ConnectInfo("203.0.113.9:40000".parse::<SocketAddr>()?));
    let resp = app.clone().oneshot(req).await?;
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024).await?;
    Ok((status, String::from_utf8_lossy(&bytes).into_owned()))
}

#[tokio::test]
async fn the_files_routes_refuse_an_anonymous_caller_at_their_gates() -> Result<()> {
    let app = router(&[]).await?;
    for uri in GATED {
        let (status, body) = get(&app, uri).await?;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{uri}: {body}");
    }
    let (status, body) = get(&app, "/s/not-a-link").await?;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(body.contains(UNAVAILABLE), "{body}");
    Ok(())
}

#[tokio::test]
async fn disabling_files_http_unmounts_every_files_route() -> Result<()> {
    let app = router(&[ExtensionId::new(CoreId::FilesHttp.id())]).await?;
    for uri in GATED {
        let (status, body) = get(&app, uri).await?;
        assert_eq!(status, StatusCode::NOT_FOUND, "{uri}: {body}");
    }
    let (status, body) = get(&app, "/s/not-a-link").await?;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert!(!body.contains(UNAVAILABLE), "{body}");
    Ok(())
}
