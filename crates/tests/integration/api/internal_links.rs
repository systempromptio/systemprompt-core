//! `POST /api/v1/internal/links` over HTTP, driven by the MCP server's own
//! link client: the owner's hop token gets the links; another user's, a token
//! for another server, a token from a foreign key and a missing token are
//! refused, and a server hosting no documents or issues toolset is refused
//! every link. On the real router the route answers under its rate limit and
//! is absent when the document store is disabled.

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use systemprompt_api::routes::documents::document_service;
use systemprompt_api::routes::internal::links_router;
use systemprompt_api::services::server::{ReadinessState, setup_api_server};
use systemprompt_database::DbPool;
use systemprompt_files::uploads::models::NewUpload;
use systemprompt_identifiers::{JwtToken, McpServerId, SessionId, TraceId, UserId, headers};
use systemprompt_manifest::profile::RateLimitsConfig;
use systemprompt_models::auth::{AuthenticatedUser, McpHopIdentity, Permission, UserType};
use systemprompt_models::documents::content_hash;
use systemprompt_models::modules::ApiPaths;
use systemprompt_runtime::AppContext;
use systemprompt_runtime::documents::links::{
    ApiLinkIssuer, IssuedLink, LinkIssueError, LinkIssuer, LinkMinter, LinkRefusal, LinkTarget,
};
use systemprompt_security::jwt::{encode_mcp_hop, mcp_hop_claims};
use systemprompt_security::keys::{RsaSigningKey, SigningKeys};
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_app_context_with_services, fixture_config,
    seed_bridge_credential, services_hosting, test_db_pool,
};
use tower::ServiceExt;

use super::common::setup_ctx;

const SERVER: &str = "links-srv";

fn identity(user: &AuthenticatedUser) -> McpHopIdentity {
    McpHopIdentity {
        user_id: user.id.clone(),
        user: Some(user.clone()),
        session_id: SessionId::generate(),
        user_type: UserType::User,
        client_id: None,
        act_chain: Vec::new(),
        purpose: None,
        interaction: None,
        mcp_session: None,
    }
}

fn hop(ctx: &AppContext, user: &AuthenticatedUser, server: &str) -> JwtToken {
    JwtToken::new(
        ctx.token_authority()
            .mint_mcp_hop(&McpServerId::new(server), &identity(user))
            .expect("hop"),
    )
}

async fn person(pool: &systemprompt_database::DbPool, prefix: &str) -> Result<AuthenticatedUser> {
    let fixture = seed_bridge_credential(pool, prefix).await?;
    let id = fixture.user_id.as_str().to_owned();
    Ok(AuthenticatedUser::new(
        UserId::new(id.clone()),
        id.clone(),
        format!("{id}@example.test"),
        vec![Permission::User],
    ))
}

async fn serve(ctx: &Arc<AppContext>) -> Result<String> {
    let minter = LinkMinter {
        tokens: ctx.token_authority(),
        documents: document_service(ctx),
        uploads: Arc::clone(ctx.documents().uploads()),
        services: services_hosting(SERVER, &["documents"]),
        api_external_url: "https://links.example.test".to_owned(),
    };
    let app = Router::new().nest(ApiPaths::INTERNAL_LINKS, links_router(minter));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    tokio::spawn(async move { axum::serve(listener, app).await });
    Ok(format!("http://{addr}"))
}

async fn upload(ctx: &AppContext, owner: &UserId) -> Result<LinkTarget> {
    let bytes = vec![1u8; 32];
    let session = ctx
        .documents()
        .uploads()
        .create(
            owner,
            NewUpload {
                name: "a.bin".to_owned(),
                mime: "application/octet-stream".to_owned(),
                size: 32,
                sha256: content_hash(&bytes),
            },
            &TraceId::generate(),
        )
        .await?;
    Ok(LinkTarget::Upload {
        upload_id: session.id,
    })
}

fn refused_with(outcome: Result<IssuedLink, LinkIssueError>, status: u16) {
    let error = outcome.expect_err("the API refuses the link");
    let shown = error.to_string();
    let LinkIssueError::Refused {
        status: got,
        refusal,
    } = &error
    else {
        panic!("expected a {status} refusal, got {error:?}")
    };
    assert_eq!(*got, status);
    assert_eq!(*refusal, LinkRefusal::from_status(status));
    assert_eq!(
        shown,
        format!("the API refused the link request: {}", refusal.code()),
        "the API's body never reaches the tool result"
    );
}

#[tokio::test]
async fn the_internal_links_route_signs_only_for_the_owner_of_a_verified_hop() -> Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let base = serve(&ctx).await?;
    let issuer = ApiLinkIssuer::new(&base, McpServerId::new(SERVER))?;
    let owner = person(&pool, "links-owner").await?;
    let other = person(&pool, "links-other").await?;
    let target = upload(&ctx, &owner.id).await?;

    let IssuedLink::Upload(links) = issuer
        .issue(Some(&hop(&ctx, &owner, SERVER)), target.clone())
        .await?
    else {
        panic!("an upload link")
    };
    assert!(links.parts_url.starts_with("https://links.example.test/"));

    refused_with(
        issuer
            .issue(Some(&hop(&ctx, &other, SERVER)), target.clone())
            .await,
        403,
    );
    refused_with(
        issuer
            .issue(Some(&hop(&ctx, &owner, "another-server")), target.clone())
            .await,
        401,
    );
    let foreign = SigningKeys::new(RsaSigningKey::generate()?, &[4; 32])?;
    let forged = encode_mcp_hop(
        &foreign,
        &mcp_hop_claims(
            &ctx.config().jwt_issuer,
            &McpServerId::new(SERVER),
            &identity(&owner),
        ),
    )?;
    refused_with(
        issuer
            .issue(Some(&JwtToken::new(forged)), target.clone())
            .await,
        401,
    );
    assert!(matches!(
        issuer.issue(None, target.clone()).await,
        Err(LinkIssueError::MissingHop)
    ));

    let plain = ApiLinkIssuer::new(&base, McpServerId::new("plain-srv"))?;
    refused_with(
        plain
            .issue(Some(&hop(&ctx, &owner, "plain-srv")), target.clone())
            .await,
        403,
    );

    let bare = reqwest::Client::new()
        .post(issuer.endpoint())
        .json(&serde_json::json!({"server": SERVER, "target": target}))
        .send()
        .await?;
    assert_eq!(bare.status(), reqwest::StatusCode::UNAUTHORIZED);
    Ok(())
}

async fn mounted(documents_enabled: bool) -> Result<(DbPool, Arc<AppContext>, Router)> {
    let bootstrap = ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let mut config = fixture_config(&bootstrap.database_url);
    config.rate_limits = RateLimitsConfig::testing();
    config.rate_limits.content_per_second = 1;
    config.rate_limits.burst_multiplier = 1;
    config.trusted_proxies = Vec::new();
    let ctx = fixture_app_context_with_services(
        &pool,
        config,
        services_hosting(SERVER, &["documents"]),
        documents_enabled,
    )?;
    let router = setup_api_server(&ctx, None, &Arc::new(ReadinessState::new()))
        .map_err(|e| anyhow::anyhow!("setup_api_server failed: {e}"))?;
    Ok((pool, ctx, router))
}

fn link_post(hop: Option<&JwtToken>, target: &LinkTarget) -> Result<Request<Body>> {
    let mut builder = Request::builder()
        .method("POST")
        .uri(ApiPaths::INTERNAL_LINKS)
        .header(header::HOST, "127.0.0.1")
        .header(header::USER_AGENT, "links-test/1.0")
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(hop) = hop {
        builder = builder.header(headers::MCP_HOP, hop.as_str());
    }
    let mut request = builder.body(Body::from(serde_json::to_vec(
        &serde_json::json!({"server": SERVER, "target": target}),
    )?))?;
    request
        .extensions_mut()
        .insert(ConnectInfo("203.0.113.21:40000".parse::<SocketAddr>()?));
    Ok(request)
}

#[tokio::test]
async fn the_mounted_links_route_signs_a_verified_hop_under_its_rate_limit() -> Result<()> {
    let (pool, ctx, app) = mounted(true).await?;
    let owner = person(&pool, "links-mount").await?;
    let target = upload(&ctx, &owner.id).await?;
    let hop = hop(&ctx, &owner, SERVER);

    let signed = app.clone().oneshot(link_post(Some(&hop), &target)?).await?;
    assert_eq!(signed.status(), StatusCode::OK);

    let mut limited = false;
    for _ in 0..8 {
        let status = app
            .clone()
            .oneshot(link_post(Some(&hop), &target)?)
            .await?
            .status();
        if status == StatusCode::TOO_MANY_REQUESTS {
            limited = true;
            break;
        }
    }
    assert!(
        limited,
        "the links route answers under the documents rate limit"
    );
    Ok(())
}

#[tokio::test]
async fn the_links_route_is_absent_when_the_document_store_is_disabled() -> Result<()> {
    let (pool, ctx, app) = mounted(false).await?;
    let owner = person(&pool, "links-nodocs").await?;
    let target = LinkTarget::Upload {
        upload_id: systemprompt_identifiers::DocumentUploadId::generate(),
    };
    let response = app
        .oneshot(link_post(Some(&hop(&ctx, &owner, SERVER)), &target)?)
        .await?;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    Ok(())
}
