//! The route groups declared through the one mount, on the real router of an
//! all-role node: the documents, uploads and folders groups refuse a caller
//! with no session at the user gate behind their token gates, the admin
//! brain group refuses a user token at the admin gate, and the
//! handler-verified gateway group, mounted with no context middleware,
//! still refuses a caller that presents no credential in its own verifier.

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use systemprompt_api::services::server::{ReadinessState, setup_api_server};
use systemprompt_database::DbPool;
use systemprompt_identifiers::JwtToken;
use systemprompt_manifest::profile::{NodeRole, RateLimitsConfig};
use systemprompt_models::modules::ApiPaths;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_app_context_with_config, fixture_config,
    seed_bridge_credential, test_db_pool,
};
use tower::ServiceExt;

async fn all_role() -> Result<(DbPool, Router)> {
    let bootstrap = ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let mut config = fixture_config(&bootstrap.database_url);
    config.role = NodeRole::All;
    config.rate_limits = RateLimitsConfig::disabled();
    config.trusted_proxies = Vec::new();
    let ctx = fixture_app_context_with_config(&pool, config)?;
    let router = setup_api_server(&ctx, None, &Arc::new(ReadinessState::new()))
        .map_err(|e| anyhow::anyhow!("setup_api_server failed: {e}"))?;
    Ok((pool, router))
}

async fn status(
    app: &Router,
    method: &str,
    uri: &str,
    bearer: Option<&JwtToken>,
) -> Result<StatusCode> {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::HOST, "127.0.0.1")
        .header(header::USER_AGENT, "declared-groups-test/1.0")
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(token) = bearer {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {}", token.as_str()));
    }
    let mut req = builder.body(Body::from("{}"))?;
    req.extensions_mut()
        .insert(ConnectInfo("203.0.113.9:40000".parse::<SocketAddr>()?));
    Ok(app.clone().oneshot(req).await?.status())
}

#[tokio::test]
async fn the_document_groups_refuse_a_caller_without_a_session_at_the_user_gate() -> Result<()> {
    let (_pool, app) = all_role().await?;
    for (method, uri) in [
        ("GET", ApiPaths::DOCUMENTS.to_owned()),
        ("GET", format!("{}/", ApiPaths::DOCUMENT_FOLDERS)),
        ("POST", format!("{}/", ApiPaths::UPLOADS)),
    ] {
        assert_eq!(
            status(&app, method, &uri, None).await?,
            StatusCode::UNAUTHORIZED,
            "{method} {uri}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn the_admin_brain_group_refuses_a_user_token_at_the_admin_gate() -> Result<()> {
    let (pool, app) = all_role().await?;
    let user = seed_bridge_credential(&pool, "declared-brain").await?;
    let uri = format!("{}/simulate", ApiPaths::ADMIN_BRAIN);
    assert_eq!(
        status(&app, "POST", &uri, None).await?,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        status(&app, "POST", &uri, Some(&user.jwt)).await?,
        StatusCode::FORBIDDEN
    );
    Ok(())
}

#[tokio::test]
async fn the_handler_verified_gateway_refuses_a_caller_without_a_credential() -> Result<()> {
    let (_pool, app) = all_role().await?;
    let uri = format!("{}/otel", ApiPaths::GATEWAY_BASE);
    assert_eq!(
        status(&app, "POST", &uri, None).await?,
        StatusCode::UNAUTHORIZED
    );
    Ok(())
}
