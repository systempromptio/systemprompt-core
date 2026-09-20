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
        AgentName::try_new("feedback-test").expect("valid fixture agent name"),
    );
    actor.auth.actor = Actor::user(UserId::new(format!("stream-{}", TraceId::generate())));
    systemprompt_api::routes::managed::router()
        .with_state(systemprompt_api::routes::managed::state::ManagedState::new(
            ctx.as_ref().clone(),
        ))
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

#[tokio::test]
async fn reconnect_header_overrides_query_and_query_alone_emits_durable_resync() {
    let router = router().await;
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/analytics/live?after=malformed")
                .header("last-event-id", "999999999.999999999.999999999")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        response.headers()["content-type"]
            .to_str()
            .unwrap()
            .starts_with("text/event-stream")
    );
    drop(response);
    let invalid_header = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/analytics/live?after=0.0.0")
                .header("last-event-id", "malformed")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid_header.status(), StatusCode::BAD_REQUEST);
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/analytics/live?after=999999999.999999999.999999999")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let mut body = response.into_body().into_data_stream();
    let frame = tokio::time::timeout(std::time::Duration::from_secs(10), body.next())
        .await
        .expect("query reconnect frame")
        .unwrap()
        .unwrap();
    let frame = std::str::from_utf8(&frame).unwrap();
    assert!(frame.contains("event: resync"));
    let token = frame
        .lines()
        .find_map(|line| line.strip_prefix("id: "))
        .unwrap();
    let data: serde_json::Value = serde_json::from_str(
        frame
            .lines()
            .find_map(|line| line.strip_prefix("data: "))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(data["resync"], true);
    let components: Vec<i64> = token.split('.').map(|part| part.parse().unwrap()).collect();
    assert_eq!(components.len(), 3);
    assert_eq!(data["generation"]["snapshots"], components[0]);
    assert_eq!(data["generation"]["inventory"], components[1]);
    assert_eq!(data["generation"]["installations"], components[2]);
}

#[tokio::test]
async fn invalid_resume_values_are_rejected_before_consuming_connection_capacity() {
    let router = router().await;
    for value in [
        "-1.0.0",
        "+1.2.3",
        "-0.2.3",
        "9223372036854775808.0.0",
        "1.2.3.4",
        "",
        "1.2",
    ] {
        assert_eq!(
            call(&router, Some(value)).await.status(),
            StatusCode::BAD_REQUEST
        );
    }
    let oversized = format!("{}.0.0", "9".repeat(97));
    assert_eq!(
        call(&router, Some(&oversized)).await.status(),
        StatusCode::BAD_REQUEST
    );
    let unknown_query = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/analytics/live?unknown=value")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unknown_query.status(), StatusCode::BAD_REQUEST);
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
}


#[tokio::test]
async fn non_utf8_resume_header_is_rejected_without_consuming_connection_capacity() {
    let router = router().await;
    let invalid = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/analytics/live?after=0.0.0")
                .header(
                    "last-event-id",
                    axum::http::HeaderValue::from_bytes(&[0xff]).expect("opaque header bytes"),
                )
                .body(Body::empty())
                .expect("invalid resume request"),
        )
        .await
        .expect("invalid resume response");
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);

    let mut responses = Vec::new();
    for _ in 0..10 {
        let response = call(&router, None).await;
        assert_eq!(response.status(), StatusCode::OK);
        responses.push(response);
    }
    assert_eq!(
        call(&router, None).await.status(),
        StatusCode::TOO_MANY_REQUESTS,
        "the rejected non-UTF-8 cursor never occupied one of the ten stream slots"
    );
    drop(responses);
}

#[tokio::test]
async fn generation_read_failure_emits_nothing_then_recovers_with_a_current_resync() {
    ensure_test_bootstrap();
    let database =
        systemprompt_test_fixtures::DisposableDb::installed("snapshot_stream_generation_recovery")
            .await
            .expect("private snapshot-stream database");
    let db = database.pool().await.expect("private snapshot-stream pool");
    let ctx = fixture_app_context(&db, database.url()).expect("private application context");
    let owner = ctx.system_admin().id().clone();
    let expected_snapshots = ctx
        .feedback_snapshots_repository()
        .health(&owner)
        .await
        .expect("snapshot generation")
        .generation;
    let expected_installations = ctx
        .managed_repository()
        .installation_coverage_status(&owner)
        .await
        .expect("installation generation")
        .generation;
    let pool = db.pool_arc().expect("private SQL pool");
    sqlx::query("ALTER TABLE managed_inventory_state RENAME TO managed_inventory_state_faulted")
        .execute(pool.as_ref())
        .await
        .expect("inject private generation-read fault");

    let mut actor = RequestContext::new(
        SessionId::generate(),
        TraceId::generate(),
        ContextId::generate(),
        AgentName::try_new("feedback-recovery").expect("fixture agent name"),
    );
    actor.auth.actor = Actor::user(UserId::new(format!(
        "stream-recovery-{}",
        TraceId::generate()
    )));
    let app = systemprompt_api::routes::managed::router()
        .with_state(systemprompt_api::routes::managed::state::ManagedState::new(
            ctx.as_ref().clone(),
        ))
        .layer(Extension(actor));
    let request = || {
        Request::builder()
            .uri("/analytics/live")
            .body(Body::empty())
            .expect("snapshot stream request")
    };
    let response = app
        .clone()
        .oneshot(request())
        .await
        .expect("faulted snapshot stream response");
    assert_eq!(response.status(), StatusCode::OK);
    let mut body = response.into_body().into_data_stream();
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(250), body.next())
            .await
            .is_err(),
        "a failed durable generation read must not fabricate a snapshot event"
    );

    sqlx::query("ALTER TABLE managed_inventory_state_faulted RENAME TO managed_inventory_state")
        .execute(pool.as_ref())
        .await
        .expect("repair private generation table");
    sqlx::query("SELECT pg_notify('feedback_snapshots', 'generation-repaired')")
        .execute(pool.as_ref())
        .await
        .expect("wake repaired snapshot stream");
    let frame = tokio::time::timeout(std::time::Duration::from_secs(10), body.next())
        .await
        .expect("repaired stream emits within polling bound")
        .expect("repaired stream remains open")
        .expect("infallible SSE frame");
    let frame = std::str::from_utf8(&frame).expect("UTF-8 SSE frame");
    assert!(frame.contains("event: resync"), "{frame}");
    let payload: serde_json::Value = serde_json::from_str(
        frame
            .lines()
            .find_map(|line| line.strip_prefix("data: "))
            .expect("SSE data line"),
    )
    .expect("generation payload");
    assert_eq!(payload["resync"], true);
    assert_eq!(payload["generation"]["snapshots"], expected_snapshots);
    assert_eq!(payload["generation"]["inventory"], 0);
    assert_eq!(
        payload["generation"]["installations"],
        expected_installations
    );

    let mut peers = Vec::new();
    for _ in 0..9 {
        let peer = app
            .clone()
            .oneshot(request())
            .await
            .expect("peer snapshot response");
        assert_eq!(peer.status(), StatusCode::OK);
        peers.push(peer);
    }
    assert_eq!(
        app.clone().oneshot(request()).await.unwrap().status(),
        StatusCode::TOO_MANY_REQUESTS
    );
    drop(body);
    let replacement = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let response = app.clone().oneshot(request()).await.unwrap();
            if response.status() == StatusCode::OK {
                break response;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("dropping recovered body releases its connection slot");
    drop(replacement);
    drop(peers);
    drop(app);
    drop(ctx);
    drop(pool);
    drop(db);
    database.drop_now().await;
}
