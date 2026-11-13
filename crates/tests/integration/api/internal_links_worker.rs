//! A worker node serves `POST /api/v1/internal/links` for the managed MCP
//! servers it supervises (they call their own node), with the hop check and
//! the per-subject throttle of an api node, and still mounts none of the
//! documents surface; the links it signs point at the public API.

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use systemprompt_api::services::server::{ReadinessState, setup_api_server};
use systemprompt_database::DbPool;
use systemprompt_files::uploads::models::NewUpload;
use systemprompt_identifiers::{JwtToken, McpServerId, SessionId, TraceId, UserId, headers};
use systemprompt_manifest::profile::{NodeRole, RateLimitsConfig};
use systemprompt_models::auth::{AuthenticatedUser, McpHopIdentity, Permission, UserType};
use systemprompt_models::documents::content_hash;
use systemprompt_models::modules::ApiPaths;
use systemprompt_runtime::AppContext;
use systemprompt_runtime::documents::links::LinkTarget;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_app_context_with_services, fixture_config,
    seed_bridge_credential, services_hosting, test_db_pool,
};
use tower::ServiceExt;

const SERVER: &str = "links-worker-srv";

async fn worker() -> Result<(DbPool, Arc<AppContext>, Router)> {
    let bootstrap = ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let mut config = fixture_config(&bootstrap.database_url);
    config.role = NodeRole::Worker;
    config.rate_limits = RateLimitsConfig::testing();
    config.rate_limits.content_per_second = 1;
    config.rate_limits.burst_multiplier = 1;
    config.trusted_proxies = Vec::new();
    let ctx = fixture_app_context_with_services(
        &pool,
        config,
        services_hosting(SERVER, &["documents"]),
        true,
    )?;
    let router = setup_api_server(&ctx, None, &Arc::new(ReadinessState::new()))
        .map_err(|e| anyhow::anyhow!("setup_api_server failed: {e}"))?;
    Ok((pool, ctx, router))
}

async fn owner(pool: &DbPool) -> Result<AuthenticatedUser> {
    let fixture = seed_bridge_credential(pool, "links-worker").await?;
    let id = fixture.user_id.as_str().to_owned();
    Ok(AuthenticatedUser::new(
        UserId::new(id.clone()),
        id.clone(),
        format!("{id}@example.test"),
        vec![Permission::User],
    ))
}

fn hop(ctx: &AppContext, user: &AuthenticatedUser, server: &str) -> JwtToken {
    let identity = McpHopIdentity {
        user_id: user.id.clone(),
        user: Some(user.clone()),
        session_id: SessionId::generate(),
        user_type: UserType::User,
        client_id: None,
        act_chain: Vec::new(),
        purpose: None,
        interaction: None,
        mcp_session: None,
    };
    JwtToken::new(
        ctx.token_authority()
            .mint_mcp_hop(&McpServerId::new(server), &identity)
            .expect("hop"),
    )
}

fn request(method: &str, uri: &str, hop: Option<&JwtToken>, body: Body) -> Result<Request<Body>> {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::HOST, "127.0.0.1")
        .header(header::USER_AGENT, "links-worker-test/1.0")
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(hop) = hop {
        builder = builder.header(headers::MCP_HOP, hop.as_str());
    }
    let mut request = builder.body(body)?;
    request
        .extensions_mut()
        .insert(ConnectInfo("203.0.113.31:40000".parse::<SocketAddr>()?));
    Ok(request)
}

fn link_body(server: &str, target: &LinkTarget) -> Result<Body> {
    Ok(Body::from(serde_json::to_vec(
        &serde_json::json!({"server": server, "target": target}),
    )?))
}

#[tokio::test]
async fn a_worker_signs_links_for_its_servers_behind_the_hop_check() -> Result<()> {
    let (pool, ctx, app) = worker().await?;
    let user = owner(&pool).await?;
    let bytes = vec![2u8; 32];
    let session = ctx
        .documents()
        .uploads()
        .create(
            &user.id,
            NewUpload {
                name: "w.bin".to_owned(),
                mime: "application/octet-stream".to_owned(),
                size: 32,
                sha256: content_hash(&bytes),
            },
            &TraceId::generate(),
        )
        .await?;
    let target = LinkTarget::Upload {
        upload_id: session.id,
    };
    let path = ApiPaths::INTERNAL_LINKS;

    let get = app
        .clone()
        .oneshot(request("GET", path, None, Body::empty())?)
        .await?;
    assert_eq!(get.status(), StatusCode::METHOD_NOT_ALLOWED);

    let bare = app
        .clone()
        .oneshot(request("POST", path, None, link_body(SERVER, &target)?)?)
        .await?;
    assert_eq!(bare.status(), StatusCode::UNAUTHORIZED);

    let foreign = app
        .clone()
        .oneshot(request(
            "POST",
            path,
            Some(&hop(&ctx, &user, "not-hosted-srv")),
            link_body("not-hosted-srv", &target)?,
        )?)
        .await?;
    assert_eq!(foreign.status(), StatusCode::FORBIDDEN);

    let signed = app
        .clone()
        .oneshot(request(
            "POST",
            path,
            Some(&hop(&ctx, &user, SERVER)),
            link_body(SERVER, &target)?,
        )?)
        .await?;
    assert_eq!(signed.status(), StatusCode::OK);
    let body = axum::body::to_bytes(signed.into_body(), usize::MAX).await?;
    let links: serde_json::Value = serde_json::from_slice(&body)?;
    let external = ctx.config().api_external_url.trim_end_matches('/');
    let parts_url = links.to_string();
    assert!(
        parts_url.contains(&format!("{external}{}", ApiPaths::UPLOADS)),
        "the links point at the public API, not the worker: {parts_url}"
    );
    Ok(())
}

#[tokio::test]
async fn a_worker_mounts_no_documents_surface() -> Result<()> {
    let (_pool, _ctx, app) = worker().await?;
    for path in [
        ApiPaths::DOCUMENTS,
        ApiPaths::UPLOADS,
        ApiPaths::DOCUMENT_FOLDERS,
    ] {
        let response = app
            .clone()
            .oneshot(request("GET", path, None, Body::empty())?)
            .await?;
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{path}");
    }
    Ok(())
}
