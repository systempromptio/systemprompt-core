//! HTTP coverage for the bridge credential exchange `identity-http` builds and
//! the bridge manifest and event stream `gateway-http` builds, driven through
//! the gateway router so each request crosses the member's own verification.

use anyhow::Result;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use axum::response::Response;
use systemprompt_api::routes::gateway::gateway_router;
use systemprompt_documents::actor::bearer_actor;
use systemprompt_files::documents::agents::DocumentActor;
use systemprompt_models::ApiPaths;
use systemprompt_models::auth::{DynRequestAuthenticator, TokenInteraction};
use systemprompt_runtime::AppContext;
use systemprompt_users::{ApiKeyService, IssueApiKeyParams};
use tower::ServiceExt;

use super::common::setup_ctx;

async fn send(
    ctx: &AppContext,
    method: Method,
    path: &str,
    headers: &[(&str, String)],
    body: Option<serde_json::Value>,
) -> Result<Response> {
    let app = gateway_router(ctx, ctx.admitted(), ctx.kernel())?
        .layer(axum::Extension(crate::common::edge_view(&ctx)));
    let mut builder = Request::builder().method(method).uri(path);
    for (name, value) in headers {
        builder = builder.header(*name, value);
    }
    let request = match body {
        Some(json) => builder
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&json)?))?,
        None => builder.body(Body::empty())?,
    };
    Ok(app.oneshot(request).await?)
}

async fn call(
    ctx: &AppContext,
    method: Method,
    path: &str,
    headers: &[(&str, String)],
    body: Option<serde_json::Value>,
) -> Result<(StatusCode, serde_json::Value)> {
    let response = send(ctx, method, path, headers, body).await?;
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await?;
    let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    Ok((status, json))
}

fn bearer(token: &str) -> Vec<(&'static str, String)> {
    vec![("authorization", format!("Bearer {token}"))]
}

fn path(route: &'static str) -> &'static str {
    ApiPaths::gateway_relative(route)
}

#[tokio::test]
async fn capabilities_lists_supported_modes() -> Result<()> {
    let (_db, ctx) = setup_ctx().await?;
    let (status, body) = call(
        &ctx,
        Method::GET,
        path(ApiPaths::BRIDGE_AUTH_CAPABILITIES),
        &[],
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    let modes = body["modes"].as_array().cloned().unwrap_or_default();
    assert!(modes.iter().any(|m| m == "pat"));
    assert!(modes.iter().any(|m| m == "session"));
    Ok(())
}

#[tokio::test]
async fn pat_without_bearer_is_unauthorized() -> Result<()> {
    let (_db, ctx) = setup_ctx().await?;
    let (status, _) = call(
        &ctx,
        Method::POST,
        path(ApiPaths::BRIDGE_AUTH_PAT),
        &[],
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    Ok(())
}

#[tokio::test]
async fn pat_with_invalid_token_is_unauthorized() -> Result<()> {
    let (_db, ctx) = setup_ctx().await?;
    let (status, _) = call(
        &ctx,
        Method::POST,
        path(ApiPaths::BRIDGE_AUTH_PAT),
        &bearer("sk_not_a_real_key"),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    Ok(())
}

#[tokio::test]
async fn pat_with_valid_key_issues_bridge_access() -> Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let secret = issue_key(&ctx, &pool, systemprompt_users::ApiKeyKind::User, None).await?;
    let (status, body) = call(
        &ctx,
        Method::POST,
        path(ApiPaths::BRIDGE_AUTH_PAT),
        &bearer(&secret),
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body["token"].as_str().is_some_and(|t| !t.is_empty()));
    Ok(())
}

#[tokio::test]
async fn session_with_blank_code_is_bad_request() -> Result<()> {
    let (_db, ctx) = setup_ctx().await?;
    let (status, _) = call(
        &ctx,
        Method::POST,
        path(ApiPaths::BRIDGE_AUTH_SESSION),
        &[],
        Some(serde_json::json!({ "code": "   " })),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    Ok(())
}

#[tokio::test]
async fn session_with_unknown_code_is_unauthorized() -> Result<()> {
    let (_db, ctx) = setup_ctx().await?;
    let (status, _) = call(
        &ctx,
        Method::POST,
        path(ApiPaths::BRIDGE_AUTH_SESSION),
        &[],
        Some(serde_json::json!({ "code": format!("code-{}", uuid::Uuid::new_v4()) })),
    )
    .await?;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    Ok(())
}

#[tokio::test]
async fn session_pat_with_blank_code_is_bad_request() -> Result<()> {
    let (_db, ctx) = setup_ctx().await?;
    let (status, _) = call(
        &ctx,
        Method::POST,
        path(ApiPaths::BRIDGE_AUTH_SESSION_PAT),
        &[],
        Some(serde_json::json!({ "code": "" })),
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    Ok(())
}

#[tokio::test]
async fn session_pat_with_unknown_code_is_unauthorized() -> Result<()> {
    let (_db, ctx) = setup_ctx().await?;
    let (status, _) = call(
        &ctx,
        Method::POST,
        path(ApiPaths::BRIDGE_AUTH_SESSION_PAT),
        &[],
        Some(serde_json::json!({
            "code": format!("code-{}", uuid::Uuid::new_v4()),
            "device_name": "my device",
        })),
    )
    .await?;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    Ok(())
}

#[tokio::test]
async fn provision_oauth_client_without_bearer_is_unauthorized() -> Result<()> {
    let (_db, ctx) = setup_ctx().await?;
    let (status, _) = call(
        &ctx,
        Method::POST,
        path(ApiPaths::BRIDGE_AUTH_OAUTH_CLIENT),
        &[],
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    Ok(())
}

fn authenticator(ctx: &AppContext) -> DynRequestAuthenticator {
    ctx.kernel()
        .state()
        .get::<DynRequestAuthenticator>()
        .cloned()
        .expect("request authenticator")
}

async fn seed_exchange_code(
    ctx: &AppContext,
    pool: &systemprompt_database::DbPool,
) -> Result<(String, systemprompt_identifiers::UserId)> {
    let uniq = uuid::Uuid::new_v4();
    let user = systemprompt_identifiers::UserId::new(uniq.to_string());
    systemprompt_test_fixtures::seed_user_row(pool, &user, &format!("ex-{uniq}@example.invalid"))
        .await?;
    let code = format!("code-{uniq}");
    let repo = systemprompt_test_fixtures::oauth_fixture_repository(ctx.db_pool());
    repo.create_bridge_exchange_code(systemprompt_oauth::repository::CreateExchangeCodeParams {
        code_hash: &systemprompt_oauth::services::hash_exchange_code(&code),
        user_id: &user,
        expires_at: chrono::Utc::now() + chrono::Duration::minutes(5),
    })
    .await?;
    Ok((code, user))
}

async fn session_pat(
    ctx: &AppContext,
    code: String,
    device: &str,
) -> Result<(StatusCode, serde_json::Value)> {
    call(
        ctx,
        Method::POST,
        path(ApiPaths::BRIDGE_AUTH_SESSION_PAT),
        &[],
        Some(serde_json::json!({ "code": code, "device_name": device })),
    )
    .await
}

#[tokio::test]
async fn session_pat_with_valid_code_mints_pat() -> Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let (code, _user) = seed_exchange_code(&ctx, &pool).await?;
    let (status, body) = session_pat(&ctx, code, "cov device").await?;
    assert_eq!(status, StatusCode::OK, "{body}");
    Ok(())
}

#[tokio::test]
async fn session_with_valid_code_issues_bridge_access() -> Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let (code, _user) = seed_exchange_code(&ctx, &pool).await?;
    let (status, body) = call(
        &ctx,
        Method::POST,
        path(ApiPaths::BRIDGE_AUTH_SESSION),
        &[],
        Some(serde_json::json!({ "code": code })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{body}");
    Ok(())
}

async fn issue_key(
    ctx: &AppContext,
    pool: &systemprompt_database::DbPool,
    kind: systemprompt_users::ApiKeyKind,
    purpose: Option<&str>,
) -> Result<String> {
    let uniq = uuid::Uuid::new_v4();
    let user = systemprompt_identifiers::UserId::new(uniq.to_string());
    systemprompt_test_fixtures::seed_user_row(pool, &user, &format!("key-{uniq}@example.invalid"))
        .await?;
    let service = ApiKeyService::new(
        systemprompt_test_fixtures::user_repository(&ctx),
        systemprompt_users::ApiKeyLifetime::unbounded(),
    );
    let issued = service
        .issue(IssueApiKeyParams {
            user_id: &user,
            name: "attended pin",
            expires_at: None,
            limits: &systemprompt_users::ApiKeyLimits::default(),
            scopes: &[],
            purpose,
            kind,
        })
        .await?;
    Ok(issued.secret)
}

async fn exchanged_actor(
    ctx: &AppContext,
    secret: &str,
) -> Result<(Option<TokenInteraction>, DocumentActor)> {
    let (status, access) = call(
        ctx,
        Method::POST,
        path(ApiPaths::BRIDGE_AUTH_PAT),
        &bearer(secret),
        None,
    )
    .await?;
    assert_eq!(
        status,
        StatusCode::OK,
        "a valid key must exchange: {access}"
    );
    let token = access["token"].as_str().expect("token").to_owned();
    let (claims, _user) = authenticator(ctx)
        .decode_for_gateway(&systemprompt_identifiers::JwtToken::new(&token))
        .await
        .expect("the exchanged bridge token must decode");
    let actor = bearer_actor(
        claims.purpose.as_deref(),
        claims.client_id.as_ref(),
        claims.interaction,
    );
    Ok((claims.interaction, actor))
}

fn is_attended(actor: &DocumentActor) -> bool {
    matches!(actor, DocumentActor::Agent(agent) if agent.attended)
}

#[tokio::test]
async fn device_key_from_session_pat_exchanges_for_an_attended_token() -> Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let (code, _user) = seed_exchange_code(&ctx, &pool).await?;
    let (status, device) = session_pat(&ctx, code, "attended host").await?;
    assert_eq!(status, StatusCode::OK, "{device}");
    let pat = device["pat"].as_str().expect("pat").to_owned();

    let (interaction, actor) = exchanged_actor(&ctx, &pat).await?;
    assert_eq!(interaction, Some(TokenInteraction::Interactive));
    assert!(
        is_attended(&actor),
        "a device key's token is attended: {actor:?}"
    );
    Ok(())
}

#[tokio::test]
async fn user_pat_exchanges_for_an_unattended_token() -> Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let secret = issue_key(&ctx, &pool, systemprompt_users::ApiKeyKind::User, None).await?;

    let (interaction, actor) = exchanged_actor(&ctx, &secret).await?;
    assert_eq!(interaction, None);
    assert!(
        matches!(actor, DocumentActor::Agent(_)) && !is_attended(&actor),
        "a user PAT's token is an unattended agent: {actor:?}"
    );
    Ok(())
}

#[tokio::test]
async fn device_key_with_a_purpose_exchanges_for_an_unattended_token() -> Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let secret = issue_key(
        &ctx,
        &pool,
        systemprompt_users::ApiKeyKind::Device,
        Some("workflow:attended-pin"),
    )
    .await?;

    let (interaction, actor) = exchanged_actor(&ctx, &secret).await?;
    assert_eq!(interaction, None);
    assert!(
        matches!(actor, DocumentActor::Agent(_)) && !is_attended(&actor),
        "a purpose-bound device key's token is an unattended agent: {actor:?}"
    );
    Ok(())
}

#[tokio::test]
async fn manifest_without_credential_is_unauthorized() -> Result<()> {
    let (_db, ctx) = setup_ctx().await?;
    let (status, _) = call(
        &ctx,
        Method::GET,
        path(ApiPaths::BRIDGE_MANIFEST),
        &[],
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    Ok(())
}

// `GET /v1/bridge/stream` exists because the browser-shaped stream routes
// reject `x-api-key`, which is how every other `/v1/bridge/*` endpoint
// authenticates. That makes its own auth gate the thing worth asserting: an
// additive route that skipped it would be an unauthenticated feed of every
// AG-UI event on the instance.
mod bridge_stream_auth {
    use std::time::Duration;

    use super::{bearer, call, path, send, setup_ctx};
    use anyhow::Result;
    use axum::http::{Method, StatusCode};
    use systemprompt_events::Broadcaster;
    use systemprompt_models::ApiPaths;
    use systemprompt_test_fixtures::seed_bridge_credential;

    #[tokio::test]
    async fn a_request_with_no_credential_is_unauthorized() -> Result<()> {
        let (_db, ctx) = setup_ctx().await?;
        let (status, _) = call(&ctx, Method::GET, path(ApiPaths::BRIDGE_STREAM), &[], None).await?;
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "an unauthenticated caller must not reach the event feed"
        );
        Ok(())
    }

    #[tokio::test]
    async fn a_bearer_token_that_does_not_decode_is_unauthorized() -> Result<()> {
        let (_db, ctx) = setup_ctx().await?;
        let (status, _) = call(
            &ctx,
            Method::GET,
            path(ApiPaths::BRIDGE_STREAM),
            &bearer("not-a-real-token"),
            None,
        )
        .await?;
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "an undecodable token must be refused, not treated as anonymous access"
        );
        Ok(())
    }

    // Why: the bridge authenticates with `x-api-key`, so that header must be
    // read here — but a bogus key must still be refused.
    #[tokio::test]
    async fn an_api_key_that_does_not_decode_is_unauthorized() -> Result<()> {
        let (_db, ctx) = setup_ctx().await?;
        let (status, _) = call(
            &ctx,
            Method::GET,
            path(ApiPaths::BRIDGE_STREAM),
            &[("x-api-key", "sk-not-a-real-key".to_owned())],
            None,
        )
        .await?;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        Ok(())
    }

    #[tokio::test]
    async fn an_authenticated_bridge_stream_registers_and_releases_the_users_live_connection()
    -> Result<()> {
        let (pool, ctx) = setup_ctx().await?;
        let credential = seed_bridge_credential(&pool, "bridge-stream@example.invalid").await?;
        let response = send(
            &ctx,
            Method::GET,
            path(ApiPaths::BRIDGE_STREAM),
            &bearer(credential.jwt.as_str()),
            None,
        )
        .await?;
        assert_eq!(response.status(), StatusCode::OK);
        assert!(
            response
                .headers()
                .get("content-type")
                .and_then(|value| value.to_str().ok())
                .is_some_and(|value| value.starts_with("text/event-stream"))
        );
        let count = || async {
            ctx.event_broadcasters()
                .agui
                .connection_count(&credential.user_id)
                .await
        };
        tokio::time::timeout(Duration::from_secs(1), async {
            while count().await != 1 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("authenticated stream registers under its user");

        drop(response);
        tokio::time::timeout(Duration::from_secs(1), async {
            while count().await != 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("dropping the stream releases its broadcaster connection");
        Ok(())
    }
}

#[tokio::test]
async fn a_manifest_whose_stores_are_offline_fails_closed() -> Result<()> {
    let (pool, _auth_ctx) = setup_ctx().await?;
    let cred = systemprompt_test_fixtures::seed_admin_credential(
        &pool,
        &format!("manifest-fallback-{}@example.invalid", uuid::Uuid::new_v4()),
    )
    .await?;
    let offline_ctx = systemprompt_test_fixtures::test_app_context(
        &systemprompt_test_fixtures::ensure_test_bootstrap().database_url,
    )
    .await;
    offline_ctx.db_pool().pool().close().await;
    offline_ctx.db_pool().write_pool().close().await;
    let (status, body) = call(
        &offline_ctx,
        Method::GET,
        path(ApiPaths::BRIDGE_MANIFEST),
        &bearer(cred.jwt.as_str()),
        None,
    )
    .await?;
    assert!(
        status.is_client_error() || status.is_server_error(),
        "a manifest whose stores cannot be read is never served, got {status}: {body}"
    );
    Ok(())
}
