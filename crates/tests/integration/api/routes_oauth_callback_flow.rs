//! `/oauth/callback` full browser-flow round trip — `handle_callback`.
//!
//! The callback resolves the server's own browser client by the configured
//! redirect URI, redeems the returned authorization code for tokens, opens an
//! authenticated session, and 302-redirects to the origin recovered from a
//! consumed `state` binding. To reach that success path the test seeds a
//! confidential client whose redirect URI matches `api_external_url`, a user,
//! an authorization code, and a state binding. Each test process installs a
//! Config with a unique `api_external_url` host so the redirect-URI lookup can
//! never collide with a browser client seeded by another test.

use std::sync::{Arc, Once, OnceLock};

use axum::Router;
use axum::body::Body;
use axum::http::{Request, Response, StatusCode, header};
use axum::middleware::{self, Next};
use systemprompt_api::routes::oauth::public_router;
use systemprompt_identifiers::{
    AgentName, AuthorizationCode, ClientId, ContextId, SessionId, TraceId, UserId,
};
use systemprompt_models::Config;
use systemprompt_models::execution::context::RequestContext;
use systemprompt_oauth::OAuthState;
use systemprompt_oauth::repository::{
    AuthCodeParams, ClientRepository, CreateClientParams, OAuthRepository, StateBindingParams,
};
use systemprompt_oauth::services::hash_client_secret;
use systemprompt_test_fixtures::{
    TEST_CLIENT_SECRET, ensure_test_bootstrap, fixture_config, fixture_db_pool,
    install_test_signing_key,
};
use systemprompt_traits::AppContext as _;
use tower::ServiceExt;
use uuid::Uuid;

use super::common::setup_ctx;

static CONFIG_INSTALL: Once = Once::new();
static HOST: OnceLock<String> = OnceLock::new();

fn unique_host() -> &'static str {
    HOST.get_or_init(|| format!("cb-{}.test", Uuid::new_v4().simple()))
}

fn callback_redirect_uri() -> String {
    format!("http://{}/api/v1/core/oauth/callback", unique_host())
}

fn ensure_config() {
    CONFIG_INSTALL.call_once(|| {
        let mut config = fixture_config("postgres://x");
        config.api_external_url = format!("http://{}", unique_host());
        let _ = Config::install(config);
    });
}

async fn inject_context(mut req: Request<Body>, next: Next) -> Response<Body> {
    req.extensions_mut().insert(RequestContext::new(
        SessionId::generate(),
        TraceId::new("callback-flow"),
        ContextId::generate(),
        AgentName::system(),
    ));
    next.run(req).await
}

async fn callback_app() -> anyhow::Result<Router> {
    ensure_config();
    install_test_signing_key();
    let (_pool, ctx) = setup_ctx().await?;
    let state = OAuthState::new(
        Arc::clone(ctx.db_pool()),
        ctx.analytics_provider().expect("analytics"),
        ctx.user_provider().expect("user"),
    );
    Ok(public_router()
        .layer(middleware::from_fn(inject_context))
        .with_state(state))
}

struct BrowserFlow {
    client_id: ClientId,
    code: AuthorizationCode,
}

async fn seed_browser_flow() -> anyhow::Result<BrowserFlow> {
    ensure_config();
    let b = ensure_test_bootstrap();
    let pool = fixture_db_pool(&b.database_url).await?;
    let user = UserId::new(Uuid::new_v4().to_string());
    let p = pool.pool_arc().expect("read pool");
    sqlx::query(
        "INSERT INTO users (id, name, email, roles) VALUES ($1, $1, $2, $3) ON CONFLICT DO NOTHING",
    )
    .bind(user.as_str())
    .bind(format!("{}@callback.invalid", user.as_str()))
    .bind(vec!["user".to_owned()])
    .execute(p.as_ref())
    .await?;

    let redirect_uri = callback_redirect_uri();
    let client_id = ClientId::new(format!("browser-{}", Uuid::new_v4().simple()));
    let secret_hash =
        hash_client_secret(TEST_CLIENT_SECRET).map_err(|e| anyhow::anyhow!("hash: {e}"))?;
    let client_repo = ClientRepository::new(&pool).map_err(|e| anyhow::anyhow!("repo: {e}"))?;
    client_repo
        .create(CreateClientParams {
            client_id: client_id.clone(),
            owner_user_id: user.clone(),
            client_secret_hash: secret_hash,
            client_name: "browser".to_owned(),
            redirect_uris: vec![redirect_uri.clone()],
            grant_types: Some(vec![
                "authorization_code".to_owned(),
                "refresh_token".to_owned(),
            ]),
            response_types: Some(vec!["code".to_owned()]),
            scopes: vec!["user".to_owned()],
            token_endpoint_auth_method: Some("client_secret_basic".to_owned()),
            application_type: "web".to_owned(),
            client_uri: None,
            logo_uri: None,
            contacts: None,
        })
        .await
        .map_err(|e| anyhow::anyhow!("create browser client: {e}"))?;

    let repo = OAuthRepository::new(&pool).map_err(|e| anyhow::anyhow!("oauth repo: {e}"))?;
    let code = AuthorizationCode::new(format!("cbcode-{}", Uuid::new_v4().simple()));
    repo.store_authorization_code(AuthCodeParams {
        code: &code,
        client_id: &client_id,
        user_id: &user,
        redirect_uri: &redirect_uri,
        scope: "user",
        code_challenge: None,
        code_challenge_method: None,
        resource: None,
    })
    .await
    .map_err(|e| anyhow::anyhow!("store auth code: {e}"))?;

    Ok(BrowserFlow { client_id, code })
}

async fn seed_state_binding(state_token: &str, client_id: &ClientId) -> anyhow::Result<()> {
    let b = ensure_test_bootstrap();
    let pool = fixture_db_pool(&b.database_url).await?;
    let repo = OAuthRepository::new(&pool).map_err(|e| anyhow::anyhow!("oauth repo: {e}"))?;
    let redirect_uri = callback_redirect_uri();
    repo.store_state_binding(
        StateBindingParams::builder(state_token)
            .with_return_to("/dashboard")
            .with_client_id(client_id)
            .with_redirect_uri(&redirect_uri)
            .build(),
    )
    .await
    .map_err(|e| anyhow::anyhow!("store state binding: {e}"))?;
    Ok(())
}

fn get(uri: &str) -> Request<Body> {
    Request::builder()
        .method(http::Method::GET)
        .uri(uri)
        .body(Body::empty())
        .expect("build")
}

#[tokio::test]
async fn callback_full_flow_redirects_with_cookie() -> anyhow::Result<()> {
    let flow = seed_browser_flow().await?;
    let state_token = format!("state-{}", Uuid::new_v4().simple());
    seed_state_binding(&state_token, &flow.client_id).await?;
    let app = callback_app().await?;
    let resp = app
        .oneshot(get(&format!(
            "/callback?code={}&state={}",
            flow.code.as_str(),
            state_token
        )))
        .await?;
    assert_eq!(
        resp.status(),
        StatusCode::SEE_OTHER,
        "callback must 302 on success, got {}",
        resp.status()
    );
    assert_eq!(
        resp.headers()
            .get(header::LOCATION)
            .and_then(|v| v.to_str().ok()),
        Some("/dashboard"),
        "must redirect to the state binding's return_to"
    );
    assert!(
        resp.headers().get(header::SET_COOKIE).is_some(),
        "callback must set the access-token cookie"
    );
    Ok(())
}

#[tokio::test]
async fn callback_missing_state_returns_400() -> anyhow::Result<()> {
    let flow = seed_browser_flow().await?;
    let app = callback_app().await?;
    let resp = app
        .oneshot(get(&format!("/callback?code={}", flow.code.as_str())))
        .await?;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn callback_unknown_state_returns_400() -> anyhow::Result<()> {
    let flow = seed_browser_flow().await?;
    let app = callback_app().await?;
    let resp = app
        .oneshot(get(&format!(
            "/callback?code={}&state=never-stored-{}",
            flow.code.as_str(),
            Uuid::new_v4().simple()
        )))
        .await?;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn callback_unknown_code_with_browser_client_returns_401() -> anyhow::Result<()> {
    let _flow = seed_browser_flow().await?;
    let app = callback_app().await?;
    let resp = app
        .oneshot(get(&format!(
            "/callback?code=not-a-real-code-{}&state=x",
            Uuid::new_v4().simple()
        )))
        .await?;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED, "{}", resp.status());
    Ok(())
}
