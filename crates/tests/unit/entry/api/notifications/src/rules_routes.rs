use std::sync::Arc;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{HeaderMap, Method, Request, StatusCode};
use serde_json::{Value, json};
use systemprompt_api::routes::notifications::{NotificationsState, router};
use systemprompt_api::services::middleware::{
    AuthzPolicy, ContextExtractor, RouterExt, UserOnlyContextMiddleware,
};
use systemprompt_identifiers::{Actor, AgentName, ContextId, SessionId, TraceId, UserId};
use systemprompt_models::RequestContext;
use systemprompt_models::auth::UserType;
use systemprompt_models::execution::ContextExtractionError;
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, seed_user_row, test_app_context, test_db_pool,
};
use tower::ServiceExt;
use uuid::Uuid;

#[derive(Clone)]
struct Caller(RequestContext);

#[async_trait::async_trait]
impl ContextExtractor for Caller {
    async fn extract_from_headers(
        &self,
        _headers: &HeaderMap,
    ) -> Result<RequestContext, ContextExtractionError> {
        Ok(self.0.clone())
    }
}

async fn context() -> Arc<AppContext> {
    let boot = ensure_test_bootstrap();
    let pool = test_db_pool().await;
    test_app_context(&pool, &boot.database_url)
}

async fn user(ctx: &AppContext) -> UserId {
    let id = UserId::new(format!("notif-rules-{}", Uuid::new_v4().simple()));
    seed_user_row(
        ctx.db_pool(),
        &id,
        &format!("{}@example.invalid", id.as_str()),
    )
    .await
    .expect("seed user");
    id
}

fn app(ctx: &AppContext, user: &UserId) -> Router {
    let caller = RequestContext::new(
        SessionId::generate(),
        TraceId::generate(),
        ContextId::generate(),
        AgentName::try_new("notifications-rules").expect("agent name"),
        Actor::user(user.clone()),
    )
    .with_user_type(UserType::User);
    router(NotificationsState::from_context(ctx)).with_auth(
        UserOnlyContextMiddleware::new(Caller(caller)),
        AuthzPolicy::user(),
    )
}

async fn call(app: Router, method: Method, body: Option<Value>) -> (StatusCode, Value) {
    let builder = Request::builder().method(method).uri("/rules");
    let request = match body {
        Some(json) => builder
            .header("content-type", "application/json")
            .body(Body::from(json.to_string())),
        None => builder.body(Body::empty()),
    }
    .expect("request");
    let response = app.oneshot(request).await.expect("response");
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1 << 20).await.expect("body");
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn rule_id() -> String {
    Uuid::new_v4().to_string()
}

#[tokio::test]
async fn a_new_user_has_no_rules() {
    let ctx = context().await;
    let who = user(&ctx).await;
    let (status, body) = call(app(&ctx, &who), Method::GET, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["rules"], json!([]));
}

#[tokio::test]
async fn replaced_rules_are_read_back_and_replaced_again() {
    let ctx = context().await;
    let who = user(&ctx).await;
    let first = rule_id();
    let (status, body) = call(
        app(&ctx, &who),
        Method::PUT,
        Some(json!({ "rules": [
            { "id": first, "name": "Mute docs", "priority": 3,
              "match": { "kinds": ["document.comment"] },
              "action": { "mute": true } }
        ]})),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");

    let (_, listed) = call(app(&ctx, &who), Method::GET, None).await;
    let rules = listed["rules"].as_array().expect("rules");
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0]["id"], first.as_str());
    assert_eq!(rules[0]["name"], "Mute docs");
    assert_eq!(rules[0]["priority"], 3);
    assert_eq!(rules[0]["action"]["mute"], true);

    let (status, _) = call(app(&ctx, &who), Method::PUT, Some(json!({ "rules": [] }))).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, listed) = call(app(&ctx, &who), Method::GET, None).await;
    assert_eq!(listed["rules"], json!([]));
}

#[tokio::test]
async fn duplicate_rule_ids_are_refused_and_nothing_is_stored() {
    let ctx = context().await;
    let who = user(&ctx).await;
    let id = rule_id();
    let (status, body) = call(
        app(&ctx, &who),
        Method::PUT,
        Some(json!({ "rules": [
            { "id": id, "name": "a" },
            { "id": id, "name": "b" }
        ]})),
    )
    .await;
    assert!(status.is_client_error(), "{status} {body}");
    let (_, listed) = call(app(&ctx, &who), Method::GET, None).await;
    assert_eq!(listed["rules"], json!([]));
}

#[tokio::test]
async fn a_floor_above_the_cap_is_refused() {
    let ctx = context().await;
    let who = user(&ctx).await;
    let (status, body) = call(
        app(&ctx, &who),
        Method::PUT,
        Some(json!({ "rules": [
            { "id": rule_id(), "name": "inverted",
              "action": { "floor": "human", "cap": "silent" } }
        ]})),
    )
    .await;
    assert!(status.is_client_error(), "{status} {body}");
    let (_, listed) = call(app(&ctx, &who), Method::GET, None).await;
    assert_eq!(listed["rules"], json!([]));
}

#[tokio::test]
async fn a_non_uuid_rule_id_is_rejected() {
    let ctx = context().await;
    let who = user(&ctx).await;
    let (status, _) = call(
        app(&ctx, &who),
        Method::PUT,
        Some(json!({ "rules": [{ "id": "not-a-uuid", "name": "x" }] })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn a_body_without_rules_is_rejected() {
    let ctx = context().await;
    let who = user(&ctx).await;
    let (status, _) = call(app(&ctx, &who), Method::PUT, Some(json!({ "items": [] }))).await;
    assert!(status.is_client_error(), "{status}");
}
