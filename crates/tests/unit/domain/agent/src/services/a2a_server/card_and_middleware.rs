// Tower-oneshot harness for the agent-card handler and the OAuth middleware:
// the card route resolves the configured agent against the fixture registry
// (404 for an unregistered name), and the middleware wrapper rejects missing
// or garbage bearer tokens with 401 while admitting a fixture-minted RS256
// JWT and injecting the authenticated RequestContext.

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::get;
use systemprompt_agent::services::a2a_server::auth::middleware::agent_oauth_middleware_wrapper;
use systemprompt_agent::services::a2a_server::auth::{AgentOAuthConfig, AgentOAuthState};
use systemprompt_agent::services::a2a_server::handlers::AgentHandlerState;
use systemprompt_agent::services::a2a_server::handlers::card::handle_agent_card;
use systemprompt_database::DbPool;
use systemprompt_identifiers::UserId;
use systemprompt_models::auth::JwtAudience;
use systemprompt_models::execution::context::RequestContext;
use tokio::sync::{RwLock, Semaphore};
use tower::ServiceExt;

use super::a2a_helpers::{StubAiProvider, agent_config, make_agent_state};
use systemprompt_test_fixtures::test_db_pool;

fn handler_state(pool: &DbPool) -> Arc<AgentHandlerState> {
    let oauth_state = Arc::new(
        AgentOAuthState::new(
            Arc::clone(pool),
            AgentOAuthConfig::default(),
            JwtAudience::standard(),
        )
        .with_authenticator(systemprompt_test_fixtures::fixture_request_authenticator(
            pool,
        )),
    );
    Arc::new(AgentHandlerState {
        config: Arc::new(RwLock::new(agent_config("card_mw_agent"))),
        oauth_state,
        agent_state: make_agent_state(pool),
        ai_service: Arc::new(StubAiProvider::new()),
        stream_semaphore: Arc::new(Semaphore::new(2)),
        active_tasks: systemprompt_agent::services::a2a_server::ActiveTasks::default(),
    })
}

async fn body_string(response: axum::response::Response) -> String {
    let bytes = axum::body::to_bytes(response.into_body(), 1_048_576)
        .await
        .expect("read body");
    String::from_utf8(bytes.to_vec()).expect("utf8 body")
}

#[tokio::test]
async fn agent_card_unregistered_agent_returns_not_found() {
    let pool = test_db_pool().await;
    let _lock = crate::SKILLS_FIXTURE_LOCK.read().await;
    let app = Router::new()
        .route("/card", get(handle_agent_card))
        .with_state(handler_state(&pool));

    let response = app
        .oneshot(Request::get("/card").body(Body::empty()).expect("request"))
        .await
        .expect("card response");

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = body_string(response).await;
    assert!(body.contains("Agent card not found"), "body: {body}");
    assert!(body.contains("card_mw_agent"), "body: {body}");
}

async fn authed(axum::Extension(ctx): axum::Extension<RequestContext>) -> String {
    ctx.user_id().to_string()
}

fn protected_app(pool: &DbPool) -> Router {
    let state = handler_state(pool);
    Router::new()
        .route("/p", get(authed))
        .layer(axum::middleware::from_fn_with_state(
            Arc::clone(&state),
            agent_oauth_middleware_wrapper,
        ))
        .with_state(state)
}

#[tokio::test]
async fn middleware_rejects_missing_authorization() {
    let pool = test_db_pool().await;
    let response = protected_app(&pool)
        .oneshot(Request::get("/p").body(Body::empty()).expect("request"))
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn middleware_rejects_malformed_bearer_token() {
    let pool = test_db_pool().await;
    let response = protected_app(&pool)
        .oneshot(
            Request::get("/p")
                .header("authorization", "Bearer not-a-jwt")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn middleware_admits_valid_jwt_and_injects_context() {
    let pool = test_db_pool().await;
    let user = UserId::new(uuid::Uuid::new_v4().to_string());
    systemprompt_test_fixtures::seed_user_row(&pool, &user, &format!("{user}@mw.invalid"))
        .await
        .expect("seed user");
    let cookie = systemprompt_test_fixtures::session_cookie_with(&pool, &user, 0, |request| {
        request.browser_session = false;
    })
    .await
    .expect("mint token");
    let token = cookie.trim_start_matches("access_token=").to_owned();

    let response = protected_app(&pool)
        .oneshot(
            Request::get("/p")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_string(response).await;
    assert_eq!(body, user.to_string());
}

#[tokio::test]
async fn middleware_refuses_a_logged_out_sessions_unexpired_token() {
    let pool = test_db_pool().await;
    let user = UserId::new(uuid::Uuid::new_v4().to_string());
    systemprompt_test_fixtures::seed_user_row(&pool, &user, &format!("{user}@mw-out.invalid"))
        .await
        .expect("seed user");
    let cookie = systemprompt_test_fixtures::session_cookie_with(&pool, &user, 0, |request| {
        request.browser_session = false;
    })
    .await
    .expect("mint token");
    let token = cookie.trim_start_matches("access_token=").to_owned();
    sqlx::query("UPDATE user_sessions SET revoked_at = now() WHERE user_id = $1")
        .bind(user.as_str())
        .execute(pool.write_pool().as_ref())
        .await
        .expect("end the session");

    let response = protected_app(&pool)
        .oneshot(
            Request::get("/p")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
