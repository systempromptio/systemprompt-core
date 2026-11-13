use std::sync::Arc;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{HeaderMap, Method, Request, StatusCode};
use serde_json::{Value, json};
use systemprompt_api::routes::notifications::{NotificationsState, router};
use systemprompt_api::services::middleware::{
    ContextExtractor, RouterExt, UserOnlyContextMiddleware,
};
use systemprompt_extension::AuthzPolicy;
use systemprompt_identifiers::{
    Actor, AgentName, ContextId, NotificationStreamId, SessionId, TraceId, UserId,
};
use systemprompt_models::RequestContext;
use systemprompt_models::auth::UserType;
use systemprompt_models::execution::ContextExtractionError;
use systemprompt_models::notifications::{AudienceSubject, StreamAttention, StreamDecl};
use systemprompt_notifications::repository::NotificationStreamRepository;
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
    let id = UserId::generate();
    seed_user_row(
        ctx.db_pool(),
        &id,
        &format!("{}@notif-subs.invalid", id.as_str()),
    )
    .await
    .expect("seed user");
    id
}

async fn stream(ctx: &AppContext, owner: &UserId) -> NotificationStreamId {
    let id = NotificationStreamId::try_new(format!("subs-{}", Uuid::new_v4().simple()))
        .expect("stream id");
    let decl = StreamDecl {
        id: id.clone(),
        name: "Subscription route stream".to_owned(),
        description: None,
        audience: vec![AudienceSubject::User(owner.clone())],
        publishers: vec![AudienceSubject::User(owner.clone())],
        attention: StreamAttention::default(),
        retention_days: None,
        coalesce_window_secs: None,
    };
    let repos = ctx.notifications().service.repositories();
    NotificationStreamRepository::upsert(repos.write_pool(), &decl, "test")
        .await
        .expect("declare stream");
    id
}

fn app(ctx: &AppContext, user: &UserId) -> Router {
    let caller = RequestContext::new(
        SessionId::generate(),
        TraceId::generate(),
        ContextId::generate(),
        AgentName::try_new("notifications-subs").expect("agent name"),
        Actor::user(user.clone()),
    )
    .with_user_type(UserType::User);
    router(NotificationsState::from_context(ctx)).with_auth(
        UserOnlyContextMiddleware::new(Caller(caller)),
        AuthzPolicy::user(),
    )
}

async fn call(app: Router, method: Method, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
    let builder = Request::builder().method(method).uri(uri);
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

async fn listed_ids(ctx: &AppContext, who: &UserId) -> Vec<String> {
    let (status, body) = call(app(ctx, who), Method::GET, "/subscriptions", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["subscriptions"]
        .as_array()
        .expect("subscriptions")
        .iter()
        .filter_map(|s| s["id"].as_str().map(str::to_owned))
        .collect()
}

#[tokio::test]
async fn a_subscription_is_listed_and_then_removed() {
    let ctx = context().await;
    let who = user(&ctx).await;
    let stream = stream(&ctx, &who).await;
    assert!(listed_ids(&ctx, &who).await.is_empty());

    let (status, created) = call(
        app(&ctx, &who),
        Method::POST,
        "/subscriptions",
        Some(json!({ "subject": format!("user:{who}"), "target": { "stream": stream.as_str() } })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let id = created["id"].as_str().expect("id").to_owned();
    assert_eq!(listed_ids(&ctx, &who).await, vec![id.clone()]);

    let (status, _) = call(
        app(&ctx, &who),
        Method::DELETE,
        &format!("/subscriptions/{id}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(listed_ids(&ctx, &who).await.is_empty());

    let (status, again) = call(
        app(&ctx, &who),
        Method::DELETE,
        &format!("/subscriptions/{id}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{again}");
}

#[tokio::test]
async fn another_users_subscription_cannot_be_removed() {
    let ctx = context().await;
    let owner = user(&ctx).await;
    let other = user(&ctx).await;
    let stream = stream(&ctx, &owner).await;
    let (_, created) = call(
        app(&ctx, &owner),
        Method::POST,
        "/subscriptions",
        Some(
            json!({ "subject": format!("user:{owner}"), "target": { "stream": stream.as_str() } }),
        ),
    )
    .await;
    let id = created["id"].as_str().expect("id").to_owned();

    let (status, _) = call(
        app(&ctx, &other),
        Method::DELETE,
        &format!("/subscriptions/{id}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(listed_ids(&ctx, &other).await.is_empty());
    assert_eq!(listed_ids(&ctx, &owner).await, vec![id]);
}

#[tokio::test]
async fn a_malformed_subscription_id_is_refused() {
    let ctx = context().await;
    let who = user(&ctx).await;
    let (status, _) = call(
        app(&ctx, &who),
        Method::DELETE,
        "/subscriptions/not-a-uuid",
        None,
    )
    .await;
    assert!(status.is_client_error(), "{status}");
    assert_ne!(status, StatusCode::NOT_FOUND);
}
