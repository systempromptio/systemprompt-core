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
use systemprompt_agent::services::a2a_server::auth::middleware::{
    agent_oauth_middleware_wrapper, get_user_context,
};
use systemprompt_agent::services::a2a_server::auth::{AgentOAuthConfig, AgentOAuthState};
use systemprompt_agent::services::a2a_server::handlers::AgentHandlerState;
use systemprompt_agent::services::a2a_server::handlers::card::handle_agent_card;
use systemprompt_agent::services::shared::AgentSessionUser;
use systemprompt_database::DbPool;
use systemprompt_identifiers::UserId;
use systemprompt_models::auth::JwtAudience;
use systemprompt_models::execution::context::RequestContext;
use tokio::sync::{RwLock, Semaphore};
use tower::ServiceExt;

use super::a2a_helpers::{StubAiProvider, agent_config, make_agent_state};
use crate::repository::try_pool;

const ISSUER: &str = "card-middleware-tests";

fn handler_state(pool: &DbPool) -> Arc<AgentHandlerState> {
    let oauth_state = Arc::new(AgentOAuthState::new(
        Arc::clone(pool),
        AgentOAuthConfig::default(),
        ISSUER.to_owned(),
        JwtAudience::standard(),
    ));
    Arc::new(AgentHandlerState {
        db_pool: Arc::clone(pool),
        config: Arc::new(RwLock::new(agent_config("card_mw_agent"))),
        oauth_state,
        agent_state: make_agent_state(pool),
        ai_service: Arc::new(StubAiProvider::new()),
        stream_semaphore: Arc::new(Semaphore::new(2)),
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
    let Some(pool) = try_pool().await else {
        return;
    };
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
    let Some(pool) = try_pool().await else {
        return;
    };
    let response = protected_app(&pool)
        .oneshot(Request::get("/p").body(Body::empty()).expect("request"))
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn middleware_rejects_malformed_bearer_token() {
    let Some(pool) = try_pool().await else {
        return;
    };
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
    let Some(pool) = try_pool().await else {
        return;
    };
    let user = UserId::new(uuid::Uuid::new_v4().to_string());
    let token = systemprompt_test_fixtures::mint_admin_jwt(&user, "mw@test.invalid", ISSUER);

    let response = protected_app(&pool)
        .oneshot(
            Request::get("/p")
                .header("authorization", format!("Bearer {}", token.as_str()))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_string(response).await;
    assert_eq!(body, user.to_string());
}

#[test]
fn get_user_context_reads_request_extension() {
    let mut request = Request::get("/x").body(Body::empty()).expect("request");
    assert!(get_user_context(&request).is_none());

    request.extensions_mut().insert(AgentSessionUser {
        id: UserId::new("user_ext"),
        username: "ext".to_owned(),
        user_type: "user".to_owned(),
        permissions: vec!["read".to_owned()],
    });
    let found = get_user_context(&request).expect("extension present");
    assert_eq!(found.id.as_str(), "user_ext");
}
