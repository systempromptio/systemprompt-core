//! Real SSE bodies exercise core connection limits, durable resync and guard
//! teardown.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::{Extension, Router};
use futures_util::StreamExt;
use systemprompt_identifiers::{Actor, AgentName, ContextId, SessionId, TraceId, UserId};
use systemprompt_models::RequestContext;
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_app_context, fixture_db_pool};
use tower::ServiceExt;

async fn router() -> Router {
    let bootstrap = ensure_test_bootstrap();
    let db = fixture_db_pool(&bootstrap.database_url)
        .await
        .expect("database");
    let ctx = fixture_app_context(&db, &bootstrap.database_url).expect("context");
    let mut actor = RequestContext::new(
        SessionId::generate(),
        TraceId::generate(),
        ContextId::generate(),
        AgentName::new("feedback-test"),
    );
    actor.auth.actor = Actor::user(UserId::new(format!("stream-{}", TraceId::generate())));
    systemprompt_api::routes::evaluation::campaigns::router()
        .with_state(ctx.as_ref().clone())
        .layer(Extension(actor))
}
async fn call(router: &Router, after: Option<&str>) -> axum::response::Response {
    let mut builder = Request::builder().uri("/analytics/live");
    if let Some(after) = after {
        builder = builder.header("last-event-id", after);
    }
    router
        .clone()
        .oneshot(builder.body(Body::empty()).unwrap())
        .await
        .unwrap()
}
#[tokio::test]
async fn durable_generation_mismatch_requests_resync_and_malformed_cursor_is_rejected() {
    let router = router().await;
    assert_eq!(
        call(&router, Some("bad.cursor")).await.status(),
        StatusCode::BAD_REQUEST
    );
    let response = call(&router, Some("999999999.999999999.999999999")).await;
    assert_eq!(response.status(), StatusCode::OK);
    let mut body = response.into_body().into_data_stream();
    let frame = tokio::time::timeout(std::time::Duration::from_secs(10), body.next())
        .await
        .expect("first durable snapshot")
        .unwrap()
        .unwrap();
    let text = std::str::from_utf8(&frame).unwrap();
    assert!(text.contains("event: resync"));
    assert!(text.contains("id: "));
    drop(body);
}
#[tokio::test]
async fn core_per_user_connection_cap_is_released_when_response_bodies_drop() {
    let router = router().await;
    let mut responses = Vec::new();
    for _ in 0..10 {
        let response = call(&router, None).await;
        assert_eq!(response.status(), StatusCode::OK);
        responses.push(response);
    }
    assert_eq!(
        call(&router, None).await.status(),
        StatusCode::TOO_MANY_REQUESTS
    );
    drop(responses);
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let response = call(&router, None).await;
            if response.status() == StatusCode::OK {
                drop(response);
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("guards unregister closed streams");
}
