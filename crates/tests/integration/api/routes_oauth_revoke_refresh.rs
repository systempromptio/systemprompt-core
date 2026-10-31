use axum::Router;
use axum::body::Body;
use axum::http::{self, Request, StatusCode, header};
use axum::middleware::{self, Next};
use systemprompt_api::routes::oauth::authenticated_router;
use systemprompt_identifiers::{
    Actor, AgentName, ContextId, RefreshTokenId, SessionId, TraceId, UserId,
};
use systemprompt_models::execution::context::RequestContext;
use systemprompt_oauth::OAuthState;
use systemprompt_oauth::repository::{OAuthRepository, RefreshTokenParams};
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{OAuthClientFixture, seed_oauth_client};
use systemprompt_traits::AppContext as _;
use tower::ServiceExt;
use uuid::Uuid;

use super::common::setup_ctx;

struct Fixture {
    ctx: std::sync::Arc<AppContext>,
    owner: UserId,
    client: OAuthClientFixture,
    token: RefreshTokenId,
}

async fn user(ctx: &AppContext) -> anyhow::Result<UserId> {
    let id = UserId::new(Uuid::new_v4().to_string());
    sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $1, $2) ON CONFLICT DO NOTHING")
        .bind(id.as_str())
        .bind(format!("{}@revoke-refresh.invalid", id.as_str()))
        .execute(ctx.db_pool().write_pool().as_ref())
        .await?;
    Ok(id)
}

fn repo(ctx: &AppContext) -> &OAuthRepository {
    &ctx.oauth_repositories().oauth
}

async fn fixture() -> anyhow::Result<Fixture> {
    let (pool, ctx) = setup_ctx().await?;
    let owner = user(&ctx).await?;
    let client = seed_oauth_client(&pool, &owner).await?;
    let token = RefreshTokenId::new(format!("rt-{}", Uuid::new_v4().simple()));
    repo(&ctx)
        .store_refresh_token(RefreshTokenParams {
            token_id: &token,
            client_id: &client.client_id,
            user_id: &owner,
            scope: "user",
            expires_at: chrono::Utc::now().timestamp() + 3600,
            family_id: None,
        })
        .await?;
    Ok(Fixture {
        ctx,
        owner,
        client,
        token,
    })
}

fn app(ctx: &AppContext, caller: &UserId) -> Router {
    let state = OAuthState::new(
        repo(ctx).clone(),
        ctx.analytics_provider().expect("analytics"),
        ctx.session_provider().expect("sessions"),
        ctx.user_provider().expect("user"),
        systemprompt_test_fixtures::test_oauth_signing(),
    );
    let caller = caller.clone();
    let inject = move |mut req: Request<Body>, next: Next| {
        let ctx = RequestContext::new(
            SessionId::generate(),
            TraceId::new("revoke-refresh"),
            ContextId::generate(),
            AgentName::system(),
            Actor::user(caller.clone()),
        );
        async move {
            req.extensions_mut().insert(ctx);
            next.run(req).await
        }
    };
    authenticated_router()
        .layer(middleware::from_fn(inject))
        .with_state(state)
        .layer(axum::Extension(ctx.config_arc().clone()))
}

fn form_encode(value: &str) -> String {
    value
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            },
            _ => format!("%{b:02X}"),
        })
        .collect()
}

async fn revoke(app: Router, pairs: &[(&str, &str)]) -> anyhow::Result<StatusCode> {
    let body = pairs
        .iter()
        .map(|(k, v)| format!("{k}={}", form_encode(v)))
        .collect::<Vec<_>>()
        .join("&");
    let request = Request::builder()
        .method(http::Method::POST)
        .uri("/revoke")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))?;
    Ok(app.oneshot(request).await?.status())
}

async fn held(f: &Fixture) -> anyhow::Result<bool> {
    Ok(repo(&f.ctx)
        .find_refresh_token_holder(&f.token)
        .await?
        .is_some())
}

#[tokio::test]
async fn the_owner_revokes_their_refresh_token() -> anyhow::Result<()> {
    let f = fixture().await?;
    assert!(held(&f).await?);
    let status = revoke(
        app(&f.ctx, &f.owner),
        &[
            ("token", f.token.as_str()),
            ("token_type_hint", "refresh_token"),
            ("client_id", f.client.client_id.as_str()),
            ("client_secret", &f.client.client_secret),
        ],
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert!(!held(&f).await?);
    Ok(())
}

#[tokio::test]
async fn an_unhinted_refresh_token_is_found_and_revoked() -> anyhow::Result<()> {
    let f = fixture().await?;
    let status = revoke(app(&f.ctx, &f.owner), &[("token", f.token.as_str())]).await?;
    assert_eq!(status, StatusCode::OK);
    assert!(!held(&f).await?);
    Ok(())
}

#[tokio::test]
async fn another_user_cannot_revoke_the_token() -> anyhow::Result<()> {
    let f = fixture().await?;
    let stranger = user(&f.ctx).await?;
    let status = revoke(
        app(&f.ctx, &stranger),
        &[
            ("token", f.token.as_str()),
            ("token_type_hint", "refresh_token"),
        ],
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert!(held(&f).await?, "a stranger's revoke must leave the token");
    Ok(())
}

#[tokio::test]
async fn a_different_authenticating_client_is_refused() -> anyhow::Result<()> {
    let f = fixture().await?;
    let (pool, _) = setup_ctx().await?;
    let other = seed_oauth_client(&pool, &f.owner).await?;
    assert_ne!(other.client_id, f.client.client_id);
    let status = revoke(
        app(&f.ctx, &f.owner),
        &[
            ("token", f.token.as_str()),
            ("token_type_hint", "refresh_token"),
            ("client_id", other.client_id.as_str()),
            ("client_secret", &other.client_secret),
        ],
    )
    .await?;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(held(&f).await?);
    Ok(())
}
