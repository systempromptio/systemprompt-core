//! Admin CLI gateway: a client that disconnects takes its command with it.
//!
//! The SSE stream owns the child and its output forwarders, so dropping the
//! response body — what axum does when the client goes away — must kill the
//! command rather than leave it running unobserved.

use std::os::unix::fs::PermissionsExt;
use std::time::Duration;

use axum::Extension;
use axum::body::Body;
use axum::http::Request;
use futures_util::StreamExt;
use systemprompt_api::routes::admin::cli::{CliBinaryPath, router_with_binary};
use systemprompt_identifiers::{Actor, AgentName, ContextId, SessionId, TraceId, UserId};
use systemprompt_loader::subprocess;
use systemprompt_models::RequestContext;
use systemprompt_test_fixtures::{closed_db_pool, ensure_test_bootstrap, test_app_context};
use tower::ServiceExt;

const EXIT_POLL: Duration = Duration::from_millis(25);
const EXIT_BOUND: Duration = Duration::from_secs(5);

fn req_ctx() -> RequestContext {
    RequestContext::new(
        SessionId::generate(),
        TraceId::generate(),
        ContextId::generate(),
        AgentName::try_new("admin").expect("valid AgentName"),
        Actor::user(UserId::new("00000000-0000-4000-8000-000000000001")),
    )
}

fn started_pid(frames: &str) -> u32 {
    let start = frames
        .find("\"pid\":")
        .expect("the started frame carries the child pid")
        + "\"pid\":".len();
    let digits: String = frames[start..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().expect("the pid is a number")
}

// Why: the child is killed by a signal and the test does not hold its handle,
// so its exit is observed by probing the pid within a bound.
async fn exited_within(pid: u32, bound: Duration) -> bool {
    let deadline = tokio::time::Instant::now() + bound;
    while subprocess::is_running(pid).await {
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(EXIT_POLL).await;
    }
    true
}

#[tokio::test]
async fn dropping_the_stream_kills_the_running_command() {
    let dir = tempfile::tempdir().expect("tempdir");
    let script = dir.path().join("fixture.sh");
    std::fs::write(&script, "#!/bin/sh\necho ready\nexec sleep 30\n").expect("write script");
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
        .expect("make script executable");

    let boot = ensure_test_bootstrap();
    let pool = closed_db_pool().await;
    let ctx = test_app_context(&pool, &boot.database_url);
    let app = router_with_binary(CliBinaryPath::new(script.to_string_lossy()))
        .with_state((*ctx).clone())
        .layer(Extension(req_ctx()));

    let request = Request::post("/")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({ "args": ["status"], "timeout_secs": 60 }).to_string(),
        ))
        .expect("request builds");
    let response = app.oneshot(request).await.expect("the route answers");
    assert_eq!(response.status().as_u16(), 200);

    let mut frames = response.into_body().into_data_stream();
    let mut seen = String::new();
    while !seen.contains("ready") {
        let chunk = frames
            .next()
            .await
            .expect("the stream stays open while the command runs")
            .expect("frame reads");
        seen.push_str(&String::from_utf8_lossy(&chunk));
    }
    let pid = started_pid(&seen);
    assert!(
        subprocess::is_running(pid).await,
        "the command is still running while the client is connected"
    );

    drop(frames);

    assert!(
        exited_within(pid, EXIT_BOUND).await,
        "dropping the stream must kill the command (pid {pid})"
    );
}
