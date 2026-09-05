// The webhook-delivery arms of `complete_task`. The existing task_completion
// suite points at the shared fixture's API URL, where nothing listens, so only
// the transport-failure arm is ever taken. An isolated bootstrap advertising a
// mock as `api_server_url` is what makes the delivered-response arms reachable.

use systemprompt_agent::repository::task::TaskRepository;
use systemprompt_agent::services::mcp::task_helper::complete_task;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::repository::{repos, seed_context_and_task, seed_user_and_session, try_pool_or_skip};

const BROADCAST_PATH: &str = "/api/v1/webhook/broadcast";

// Why: the broadcast is how a completed task reaches subscribers, and the
// payload is the only thing they receive. A task id or user id that fails to
// reach the wire strands the completion silently, because the caller swallows
// every delivery outcome.
#[tokio::test]
async fn a_completed_task_broadcasts_its_identifiers_and_bearer_to_the_configured_api() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(BROADCAST_PATH))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock)
        .await;
    systemprompt_test_fixtures::init_isolated_bootstrap(&mock.uri(), "{}\n");

    let Some(pool) = try_pool_or_skip().await else {
        return;
    };
    let repos = repos(&pool);
    let (user, session) = seed_user_and_session(&pool).await;
    let (ctx, task_id) = seed_context_and_task(&repos, &user, &session).await;

    let task_repo = TaskRepository::new(&pool, crate::session_usage(&pool)).expect("task repo");
    complete_task(&task_repo, &task_id, "webhook-bearer")
        .await
        .expect("completion must succeed");

    let requests = mock
        .received_requests()
        .await
        .expect("the mock must record requests");
    let broadcast = requests
        .iter()
        .find(|r| r.url.path() == BROADCAST_PATH)
        .expect("the completion must reach the configured api server");

    let body: serde_json::Value =
        serde_json::from_slice(&broadcast.body).expect("the payload must be json");
    assert_eq!(body["event_type"], "task_completed");
    assert_eq!(body["entity_id"], task_id.as_str());
    assert_eq!(
        body["context_id"],
        ctx.as_str(),
        "the broadcast must carry the task's own context"
    );
    assert_eq!(
        body["user_id"],
        user.as_str(),
        "the broadcast must carry the task's owner"
    );
    assert_eq!(
        broadcast
            .headers
            .get("authorization")
            .and_then(|h| h.to_str().ok()),
        Some("Bearer webhook-bearer"),
        "the broadcast must be authenticated with the caller's token"
    );
}

// Why: a webhook endpoint that answers 500 is a delivered response, not a
// transport failure, and it must be treated the same way — logged and
// swallowed. A task that failed to complete because its subscribers were down
// would be unrecoverable.
#[tokio::test]
async fn a_rejected_broadcast_is_swallowed_so_completion_still_succeeds() {
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(BROADCAST_PATH))
        .respond_with(ResponseTemplate::new(500))
        .mount(&mock)
        .await;
    systemprompt_test_fixtures::init_isolated_bootstrap(&mock.uri(), "{}\n");

    let Some(pool) = try_pool_or_skip().await else {
        return;
    };
    let repos = repos(&pool);
    let (user, session) = seed_user_and_session(&pool).await;
    let (_ctx, task_id) = seed_context_and_task(&repos, &user, &session).await;

    let task_repo = TaskRepository::new(&pool, crate::session_usage(&pool)).expect("task repo");
    complete_task(&task_repo, &task_id, "webhook-bearer")
        .await
        .expect("a rejected broadcast must not fail completion");

    let requests = mock
        .received_requests()
        .await
        .expect("the mock must record requests");
    assert!(
        requests.iter().any(|r| r.url.path() == BROADCAST_PATH),
        "the rejected arm is only reached if the broadcast was actually delivered"
    );
}
