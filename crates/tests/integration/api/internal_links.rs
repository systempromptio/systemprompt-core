//! `POST /api/v1/internal/links` over HTTP, driven by the MCP server's own
//! link client: the owner's hop token gets the links; another user's, a token
//! for another server, a token from a foreign key and a missing token are
//! refused.

use std::sync::Arc;

use anyhow::Result;
use axum::Router;
use systemprompt_api::routes::documents::document_service;
use systemprompt_api::routes::internal::links_router;
use systemprompt_files::uploads::models::NewUpload;
use systemprompt_identifiers::{JwtToken, McpServerId, SessionId, TraceId, UserId};
use systemprompt_models::auth::{AuthenticatedUser, McpHopIdentity, Permission, UserType};
use systemprompt_models::documents::content_hash;
use systemprompt_models::modules::ApiPaths;
use systemprompt_runtime::AppContext;
use systemprompt_runtime::documents::links::{
    ApiLinkIssuer, IssuedLink, LinkIssueError, LinkIssuer, LinkMinter, LinkTarget,
};
use systemprompt_security::jwt::{encode_mcp_hop, mcp_hop_claims};
use systemprompt_security::keys::{RsaSigningKey, SigningKeys};
use systemprompt_test_fixtures::seed_bridge_credential;

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
    match outcome {
        Err(LinkIssueError::Refused { status: got, .. }) => assert_eq!(got, status),
        other => panic!("expected a {status} refusal, got {other:?}"),
    }
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

    let bare = reqwest::Client::new()
        .post(issuer.endpoint())
        .json(&serde_json::json!({"server": SERVER, "target": target}))
        .send()
        .await?;
    assert_eq!(bare.status(), reqwest::StatusCode::UNAUTHORIZED);
    Ok(())
}
