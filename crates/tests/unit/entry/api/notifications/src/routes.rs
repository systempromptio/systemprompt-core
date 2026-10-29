use std::sync::Arc;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{HeaderMap, Method, Request, StatusCode};
use systemprompt_api::routes::notifications::{NotificationsState, router};
use systemprompt_api::services::middleware::{
    AuthzPolicy, ContextExtractor, RouterExt, UserOnlyContextMiddleware,
};
use systemprompt_identifiers::{
    Actor, AgentName, ContextId, NotificationStreamId, SessionId, TraceId, UserId,
};
use systemprompt_models::RequestContext;
use systemprompt_models::auth::UserType;
use systemprompt_models::execution::ContextExtractionError;
use systemprompt_models::notifications::{
    ActionPrompt, AudienceSubject, NotificationKind, StreamAttention, StreamDecl, SubscriptionDecl,
    SubscriptionTarget,
};
use systemprompt_notifications::repository::{
    NotificationStreamRepository, NotificationSubscriptionRepository,
};
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, seed_user_row, test_app_context, test_db_pool,
};
use tower::ServiceExt;
use uuid::Uuid;

#[derive(Clone)]
struct Caller(Option<RequestContext>);

#[async_trait::async_trait]
impl ContextExtractor for Caller {
    async fn extract_from_headers(
        &self,
        _headers: &HeaderMap,
    ) -> Result<RequestContext, ContextExtractionError> {
        self.0
            .clone()
            .ok_or(ContextExtractionError::MissingAuthHeader)
    }
}

fn caller(user: &UserId) -> RequestContext {
    RequestContext::new(
        SessionId::generate(),
        TraceId::generate(),
        ContextId::generate(),
        AgentName::try_new("notifications-routes").expect("agent name"),
        Actor::user(user.clone()),
    )
    .with_user_type(UserType::User)
}

async fn context() -> Arc<AppContext> {
    let boot = ensure_test_bootstrap();
    let pool = test_db_pool().await;
    test_app_context(&pool, &boot.database_url)
}

fn app(ctx: &AppContext, user: Option<&UserId>) -> Router {
    router(NotificationsState::from_context(ctx)).with_auth(
        UserOnlyContextMiddleware::new(Caller(user.map(caller))),
        AuthzPolicy::user(),
    )
}

async fn call(
    app: Router,
    method: Method,
    uri: &str,
    body: Option<serde_json::Value>,
) -> (StatusCode, serde_json::Value) {
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
    let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, json)
}

async fn declare_stream(ctx: &AppContext, publisher: &UserId) -> NotificationStreamId {
    declare_stream_for(
        ctx,
        publisher,
        vec![AudienceSubject::User(publisher.clone())],
    )
    .await
}

async fn declare_stream_for(
    ctx: &AppContext,
    publisher: &UserId,
    audience: Vec<AudienceSubject>,
) -> NotificationStreamId {
    let id = NotificationStreamId::try_new(format!("routes-{}", Uuid::new_v4().simple()))
        .expect("stream id");
    let stream = StreamDecl {
        id: id.clone(),
        name: "Route test stream".to_owned(),
        description: None,
        audience,
        publishers: vec![AudienceSubject::User(publisher.clone())],
        attention: StreamAttention::default(),
        retention_days: None,
        coalesce_window_secs: None,
    };
    let repos = ctx.notifications().service.repositories();
    NotificationStreamRepository::upsert(repos.write_pool(), &stream, "test")
        .await
        .expect("declare stream");
    id
}

#[tokio::test]
async fn every_route_requires_a_signed_in_user() {
    let ctx = context().await;
    for (method, uri) in [
        (Method::GET, "/"),
        (Method::GET, "/unread"),
        (Method::GET, "/streams"),
        (Method::GET, "/rules"),
    ] {
        let (status, _) = call(app(&ctx, None), method, uri, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{uri}");
    }
}

#[tokio::test]
async fn an_anonymous_caller_is_refused_by_the_user_policy() {
    let ctx = context().await;
    let anon = caller(&UserId::generate()).with_user_type(UserType::Anon);
    let app = router(NotificationsState::from_context(&ctx)).with_auth(
        UserOnlyContextMiddleware::new(Caller(Some(anon))),
        AuthzPolicy::user(),
    );
    let (status, _) = call(app, Method::GET, "/unread", None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn a_new_user_has_an_empty_feed() {
    let ctx = context().await;
    let user = UserId::generate();
    let (status, body) = call(app(&ctx, Some(&user)), Method::GET, "/", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["notifications"], serde_json::json!([]));
    assert_eq!(body["next_cursor"], serde_json::Value::Null);
    assert_eq!(body["unread"], 0);
    assert_eq!(body["unread_capped"], false);

    let (status, body) = call(app(&ctx, Some(&user)), Method::GET, "/unread", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["unread"], 0);
    assert_eq!(body["unread_capped"], false);
}

#[tokio::test]
async fn a_malformed_cursor_is_a_bad_request() {
    let ctx = context().await;
    let user = UserId::generate();
    let (status, _) = call(
        app(&ctx, Some(&user)),
        Method::GET,
        "/?since=not-a-cursor!",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn escalating_an_unknown_notification_is_not_found() {
    let ctx = context().await;
    let user = UserId::generate();
    let uri = format!("/{}/escalate", Uuid::new_v4());
    let (status, body) = call(
        app(&ctx, Some(&user)),
        Method::POST,
        &uri,
        Some(serde_json::json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert_eq!(body["error_key"], "notification_not_found");
}

#[tokio::test]
async fn escalating_a_malformed_id_is_a_bad_request() {
    let ctx = context().await;
    let user = UserId::generate();
    let (status, body) = call(
        app(&ctx, Some(&user)),
        Method::POST,
        "/not-a-uuid/escalate",
        Some(serde_json::json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
}

#[tokio::test]
async fn a_publisher_the_stream_does_not_declare_is_refused() {
    let ctx = context().await;
    let owner = UserId::generate();
    let stranger = UserId::generate();
    let stream = declare_stream(&ctx, &owner).await;
    let (status, body) = call(
        app(&ctx, Some(&stranger)),
        Method::POST,
        &format!("/streams/{stream}/publish"),
        Some(serde_json::json!({"title": "hello"})),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(body["error_key"], "publish_refused");
}

#[tokio::test]
async fn a_declared_publisher_is_accepted_for_routing() {
    let ctx = context().await;
    let owner = UserId::generate();
    let stream = declare_stream(&ctx, &owner).await;
    let (status, body) = call(
        app(&ctx, Some(&owner)),
        Method::POST,
        &format!("/streams/{stream}/publish"),
        Some(serde_json::json!({"title": "hello", "body": "world", "attention": "agent"})),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{body}");
    assert_eq!(body["enqueued"], true);
}

#[tokio::test]
async fn publishing_to_an_unknown_stream_is_not_found() {
    let ctx = context().await;
    let user = UserId::generate();
    let (status, body) = call(
        app(&ctx, Some(&user)),
        Method::POST,
        &format!(
            "/streams/routes-missing-{}/publish",
            Uuid::new_v4().simple()
        ),
        Some(serde_json::json!({"title": "hello"})),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert_eq!(body["error_key"], "stream_not_found");
}

#[tokio::test]
async fn subscribing_to_an_unknown_stream_is_not_found() {
    let ctx = context().await;
    let user = UserId::generate();
    let (status, body) = call(
        app(&ctx, Some(&user)),
        Method::POST,
        "/subscriptions",
        Some(serde_json::json!({
            "subject": format!("user:{user}"),
            "target": {"stream": format!("routes-missing-{}", Uuid::new_v4().simple())}
        })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert_eq!(body["error_key"], "stream_not_found");
}

async fn seeded_user(ctx: &AppContext) -> UserId {
    let user = UserId::generate();
    seed_user_row(
        ctx.db_pool(),
        &user,
        &format!("{}@notifications-routes.invalid", user.as_str()),
    )
    .await
    .expect("seed user");
    user
}

fn subscription_body(user: &UserId, stream: &NotificationStreamId) -> serde_json::Value {
    serde_json::json!({
        "subject": format!("user:{user}"),
        "target": {"stream": stream.as_str()}
    })
}

#[tokio::test]
async fn subscribing_outside_a_streams_audience_is_forbidden_not_in_audience() {
    let ctx = context().await;
    let owner = seeded_user(&ctx).await;
    let stranger = seeded_user(&ctx).await;
    let stream = declare_stream(&ctx, &owner).await;
    let (status, body) = call(
        app(&ctx, Some(&stranger)),
        Method::POST,
        "/subscriptions",
        Some(subscription_body(&stranger, &stream)),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(body["error_key"], "not_in_audience");

    let (status, body) = call(
        app(&ctx, Some(&owner)),
        Method::POST,
        "/subscriptions",
        Some(subscription_body(&owner, &stream)),
    )
    .await;
    assert!(status.is_success(), "an audience member subscribes: {body}");
}

#[tokio::test]
async fn a_console_write_onto_a_declared_subscription_conflicts_declared_by_config() {
    let ctx = context().await;
    let user = seeded_user(&ctx).await;
    let stream = declare_stream(&ctx, &user).await;
    let declared = SubscriptionDecl {
        subject: AudienceSubject::User(user.clone()),
        target: SubscriptionTarget::Stream(stream.clone()),
        attention_ceiling: None,
        quiet_hours: None,
        digest: None,
    };
    let repos = ctx.notifications().service.repositories();
    NotificationSubscriptionRepository::upsert(
        repos.write_pool(),
        &declared,
        "code:notifications/subscriptions.yaml",
        None,
    )
    .await
    .expect("declare subscription")
    .expect("a declared row");

    let (status, body) = call(
        app(&ctx, Some(&user)),
        Method::POST,
        "/subscriptions",
        Some(subscription_body(&user, &stream)),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error_key"], "declared_by_config");
}

async fn insert_notification(ctx: &AppContext, recipient: &UserId) -> String {
    let action = serde_json::to_value(ActionPrompt {
        prompt: "route test".to_owned(),
        open_url: None,
        document_id: None,
        comment_id: None,
        kind: NotificationKind::new(NotificationKind::SHARED).expect("kind"),
        actor: recipient.clone(),
    })
    .expect("action");
    sqlx::query_scalar::<_, String>(
        "INSERT INTO notifications (recipient_user_id, kind, actor_user_id, actor_is_agent, \
         subjects, attention, attention_reason, action, coalesce_key, first_event_at, \
         last_event_at, outbox_id, delivery_state) \
         VALUES ($1, 'document.shared', $1, FALSE, '[]'::jsonb, 'agent', 'default', $2, \
         gen_random_uuid()::text, now(), now(), 'routes', 'delivered') RETURNING id::text",
    )
    .bind(recipient.as_str())
    .bind(action)
    .fetch_one(ctx.db_pool().write_pool().as_ref())
    .await
    .expect("insert notification")
}

async fn seen_receipts(ctx: &AppContext, id: &str) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM notification_receipts WHERE notification_id = $1::uuid \
         AND state = 'seen'",
    )
    .bind(id)
    .fetch_one(ctx.db_pool().write_pool().as_ref())
    .await
    .expect("receipts")
}

#[tokio::test]
async fn seen_records_the_callers_rows_once_and_the_feed_records_nothing() {
    let ctx = context().await;
    let reader = seeded_user(&ctx).await;
    let other = seeded_user(&ctx).await;
    let id = insert_notification(&ctx, &reader).await;

    let (status, body) = call(app(&ctx, Some(&reader)), Method::GET, "/", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["unread"], 1);
    assert_eq!(
        seen_receipts(&ctx, &id).await,
        0,
        "a feed read writes nothing"
    );

    let (status, body) = call(
        app(&ctx, Some(&other)),
        Method::POST,
        "/seen",
        Some(serde_json::json!({"ids": [id]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, serde_json::json!({"seen": 0}), "not the caller's row");

    for expected in [1, 0] {
        let (status, body) = call(
            app(&ctx, Some(&reader)),
            Method::POST,
            "/seen",
            Some(serde_json::json!({"ids": [id]})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body, serde_json::json!({"seen": expected}));
    }
    assert_eq!(seen_receipts(&ctx, &id).await, 1);
}

#[tokio::test]
async fn seen_requires_a_signed_in_user() {
    let ctx = context().await;
    let (status, _) = call(
        app(&ctx, None),
        Method::POST,
        "/seen",
        Some(serde_json::json!({"ids": []})),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
