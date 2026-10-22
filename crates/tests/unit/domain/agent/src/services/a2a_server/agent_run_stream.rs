// The agent-run streaming path: a message to an agent whose metadata names an
// `agent_run` template is queued on the launcher, the submitted task is the
// first frame, and the run's stored frames follow until a terminal status,
// the run finishing, or the launcher failing.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use axum::http::StatusCode;
use serde_json::json;
use systemprompt_agent::models::a2a::jsonrpc::NumberOrString;
use systemprompt_agent::models::a2a::protocol::{
    SendMessageParams, StreamResponse, TaskStatusUpdateEvent,
};
use systemprompt_agent::models::a2a::{
    A2aRequestParams, Message, MessageRole, Part, TaskState, TaskStatus,
};
use systemprompt_agent::services::a2a_server::handlers::request::routing::handle_streaming_path;
use systemprompt_identifiers::{AgentRunId, ContextId, MessageId, TaskId};
use systemprompt_traits::{AgentRunLaunch, AgentRunLaunchError, AgentRunLauncher, AgentRunUpdates};

use super::a2a_helpers::{make_handler_state_with_launcher, request_context};
use crate::repository::{repos, seed_context_and_task, seed_user_and_session};
use systemprompt_test_fixtures::test_db_pool;

#[derive(Default)]
struct ScriptedLauncher {
    fail_enqueue: bool,
    fail_lookup: bool,
    no_run: bool,
    fail_updates: bool,
    updates: Mutex<VecDeque<AgentRunUpdates>>,
    launches: Mutex<Vec<AgentRunLaunch>>,
    polled_after: Mutex<Vec<i64>>,
}

#[async_trait]
impl AgentRunLauncher for ScriptedLauncher {
    async fn enqueue(&self, request: AgentRunLaunch) -> Result<AgentRunId, AgentRunLaunchError> {
        self.launches.lock().expect("lock").push(request);
        if self.fail_enqueue {
            return Err(AgentRunLaunchError::unavailable(std::io::Error::other(
                "queue offline",
            )));
        }
        Ok(AgentRunId::generate())
    }

    async fn cancel(&self, _run_id: &AgentRunId) -> Result<(), AgentRunLaunchError> {
        Ok(())
    }

    async fn run_for_task(
        &self,
        _task_id: &TaskId,
    ) -> Result<Option<AgentRunId>, AgentRunLaunchError> {
        if self.fail_lookup {
            return Err(AgentRunLaunchError::unavailable(std::io::Error::other(
                "lookup offline",
            )));
        }
        Ok((!self.no_run).then(AgentRunId::generate))
    }

    async fn updates_after(
        &self,
        run_id: &AgentRunId,
        after_seq: i64,
    ) -> Result<AgentRunUpdates, AgentRunLaunchError> {
        self.polled_after.lock().expect("lock").push(after_seq);
        if self.fail_updates {
            return Err(AgentRunLaunchError::NotFound(run_id.clone()));
        }
        Ok(self
            .updates
            .lock()
            .expect("lock")
            .pop_front()
            .unwrap_or(AgentRunUpdates {
                last_seq: after_seq,
                finished: true,
                ..AgentRunUpdates::default()
            }))
    }
}

fn status_frame(ctx: &ContextId, state: TaskState) -> serde_json::Value {
    serde_json::to_value(StreamResponse::StatusUpdate(TaskStatusUpdateEvent::new(
        TaskId::new("t-run"),
        ctx.clone(),
        TaskStatus {
            state,
            ..TaskStatus::default()
        },
    )))
    .expect("frame json")
}

fn send(ctx: &ContextId, text: &str) -> A2aRequestParams {
    A2aRequestParams::SendStreamingMessage(SendMessageParams {
        tenant: None,
        message: Message {
            role: MessageRole::User,
            parts: vec![Part::text(text.to_owned())],
            message_id: MessageId::generate(),
            context_id: ctx.clone(),
            task_id: None,
            reference_task_ids: None,
            metadata: None,
            extensions: None,
        },
        configuration: None,
        metadata: None,
    })
}

async fn frames(response: axum::response::Response) -> Vec<serde_json::Value> {
    let bytes = tokio::time::timeout(
        std::time::Duration::from_secs(20),
        axum::body::to_bytes(response.into_body(), usize::MAX),
    )
    .await
    .expect("stream ends")
    .expect("SSE body");
    std::str::from_utf8(&bytes)
        .expect("utf8")
        .lines()
        .filter_map(|line| line.strip_prefix("data:"))
        .map(|data| serde_json::from_str(data.trim()).expect("frame is JSON"))
        .collect()
}

fn state_of(frame: &serde_json::Value) -> Option<&str> {
    frame["result"]["statusUpdate"]["status"]["state"].as_str()
}

async fn run(
    launcher: Arc<ScriptedLauncher>,
    agent_run: Option<serde_json::Value>,
    id: i64,
) -> (
    Vec<serde_json::Value>,
    systemprompt_agent::repository::A2ARepositories,
) {
    let pool = test_db_pool().await;
    let repos_handle = repos(&pool);
    let (user, session) = seed_user_and_session(&pool).await;
    let (ctx, _task) = seed_context_and_task(&repos_handle, &user, &session).await;
    let state = make_handler_state_with_launcher(&pool, launcher, agent_run);
    let response = handle_streaming_path(
        send(&ctx, "summarise the quarter"),
        state,
        NumberOrString::Number(id),
        request_context(&ctx, &session, &user, "test_agent"),
        std::time::Instant::now(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    (frames(response).await, repos_handle)
}

fn ctx_any() -> ContextId {
    ContextId::generate()
}

#[tokio::test]
async fn a_queued_run_streams_the_task_then_its_frames_then_the_final_status() {
    let ctx = ctx_any();
    let launcher = Arc::new(ScriptedLauncher::default());
    launcher.updates.lock().expect("lock").extend([
        AgentRunUpdates {
            frames: vec![
                json!({"not": "a stream response"}),
                status_frame(&ctx, TaskState::Working),
            ],
            last_seq: 4,
            finished: false,
            final_status: None,
        },
        AgentRunUpdates {
            frames: Vec::new(),
            last_seq: 4,
            finished: true,
            final_status: Some(status_frame(&ctx, TaskState::Completed)),
        },
    ]);

    let (frames, _repos) = run(
        Arc::clone(&launcher),
        Some(json!({"agent": "researcher"})),
        21,
    )
    .await;

    assert_eq!(frames.len(), 3, "task, working, completed: {frames:?}");
    assert!(frames[0]["result"]["task"].is_object(), "{}", frames[0]);
    assert_eq!(frames[0]["id"], 21);
    assert_eq!(state_of(&frames[1]), Some("TASK_STATE_WORKING"));
    assert_eq!(state_of(&frames[2]), Some("TASK_STATE_COMPLETED"));
    assert_eq!(*launcher.polled_after.lock().expect("lock"), vec![0, 4]);

    let launches = launcher.launches.lock().expect("lock");
    assert_eq!(launches.len(), 1);
    assert_eq!(launches[0].origin_kind, "a2a");
    assert_eq!(launches[0].request["agent"], "researcher");
    assert_eq!(launches[0].request["prompt"], "summarise the quarter");
    assert_eq!(launches[0].origin_id, launches[0].task_id.to_string());
}

#[tokio::test]
async fn a_terminal_frame_ends_the_stream_without_polling_again() {
    let ctx = ctx_any();
    let launcher = Arc::new(ScriptedLauncher::default());
    launcher
        .updates
        .lock()
        .expect("lock")
        .push_back(AgentRunUpdates {
            frames: vec![
                status_frame(&ctx, TaskState::Failed),
                status_frame(&ctx, TaskState::Working),
            ],
            last_seq: 2,
            finished: false,
            final_status: None,
        });

    let (frames, _repos) = run(Arc::clone(&launcher), Some(json!({})), 22).await;

    assert_eq!(frames.len(), 2, "{frames:?}");
    assert_eq!(state_of(&frames[1]), Some("TASK_STATE_FAILED"));
    assert_eq!(launcher.polled_after.lock().expect("lock").len(), 1);
}

#[tokio::test]
async fn unavailable_updates_leave_only_the_submitted_task() {
    let launcher = Arc::new(ScriptedLauncher {
        fail_updates: true,
        ..ScriptedLauncher::default()
    });
    let (frames, _repos) = run(Arc::clone(&launcher), Some(json!({})), 23).await;
    assert_eq!(frames.len(), 1, "{frames:?}");
    assert!(frames[0]["result"]["task"].is_object());
}

#[tokio::test]
async fn a_failed_or_empty_run_lookup_streams_the_task_alone() {
    for launcher in [
        ScriptedLauncher {
            fail_lookup: true,
            ..ScriptedLauncher::default()
        },
        ScriptedLauncher {
            no_run: true,
            ..ScriptedLauncher::default()
        },
    ] {
        let launcher = Arc::new(launcher);
        let (frames, _repos) = run(Arc::clone(&launcher), Some(json!({})), 24).await;
        assert_eq!(frames.len(), 1, "{frames:?}");
        assert!(frames[0]["result"]["task"].is_object());
        assert!(launcher.polled_after.lock().expect("lock").is_empty());
    }
}

#[tokio::test]
async fn a_queue_failure_streams_an_error_and_fails_the_task() {
    let launcher = Arc::new(ScriptedLauncher {
        fail_enqueue: true,
        ..ScriptedLauncher::default()
    });
    let (frames, repos_handle) = run(Arc::clone(&launcher), Some(json!({})), 25).await;

    assert_eq!(frames.len(), 1, "{frames:?}");
    assert_eq!(frames[0]["id"], 25);
    assert!(frames[0]["error"].is_object(), "{}", frames[0]);
    let task_id = launcher.launches.lock().expect("lock")[0].task_id.clone();
    let task = repos_handle
        .tasks
        .find_task(&task_id)
        .await
        .expect("lookup")
        .expect("task persisted before the queue was tried");
    assert_eq!(task.status.state, TaskState::Failed);
}

#[tokio::test]
async fn a_non_object_agent_run_template_is_invalid_params() {
    let launcher = Arc::new(ScriptedLauncher::default());
    let (frames, _repos) = run(Arc::clone(&launcher), Some(json!("researcher")), 26).await;

    assert_eq!(frames.len(), 1, "{frames:?}");
    assert_eq!(frames[0]["error"]["code"], -32602, "{}", frames[0]);
    assert!(launcher.launches.lock().expect("lock").is_empty());
}
