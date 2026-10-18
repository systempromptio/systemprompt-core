use chrono::Utc;
use systemprompt_agent_runs::a2a::final_status;
use systemprompt_agent_runs::{HookDecision, RunEvent, RunEventPayload, TaskUpdate, task_events};
use systemprompt_identifiers::{AgentRunId, ContextId, TaskId};
use systemprompt_models::a2a::{StreamResponse, TaskState};

fn updates(payload: RunEventPayload) -> Vec<TaskUpdate> {
    let event = RunEvent {
        seq: 9,
        kind: payload.kind(),
        tool_name: None,
        tool_use_id: None,
        payload,
        created_at: Utc::now(),
    };
    task_events(
        &AgentRunId::generate(),
        &TaskId::generate(),
        &ContextId::generate(),
        &event,
    )
}

#[test]
fn final_status_targets_the_task_and_converts_to_a_status_frame() {
    let task = TaskId::generate();
    let ctx = ContextId::generate();
    let update = final_status(&task, &ctx, TaskState::Canceled);
    match update.into_stream_response() {
        StreamResponse::StatusUpdate(event) => {
            assert_eq!(event.task_id, task);
            assert_eq!(event.context_id, ctx);
            assert_eq!(event.status.state, TaskState::Canceled);
            assert!(event.status.message.is_none());
            assert!(event.status.timestamp.is_some());
        },
        other => panic!("expected a status frame, got {other:?}"),
    }
}

#[test]
fn an_artifact_update_converts_to_an_artifact_frame() {
    let text = updates(RunEventPayload::AssistantText { text: "out".into() });
    let frame = text.into_iter().next().expect("one update").into_stream_response();
    assert!(matches!(frame, StreamResponse::ArtifactUpdate(_)), "{frame:?}");
}

#[test]
fn a_failed_result_without_an_error_says_run_failed() {
    let failed = updates(RunEventPayload::Result {
        subtype: "error_during_execution".into(),
        is_error: true,
        num_turns: 0,
        duration_ms: 0,
        input_tokens: 0,
        output_tokens: 0,
        cost_microdollars: 0,
        structured_output: Some(serde_json::json!({"partial": true})),
        error: None,
    });
    assert_eq!(failed.len(), 2);
    assert!(matches!(failed[0], TaskUpdate::Artifact(_)));
    let TaskUpdate::Status(status) = &failed[1] else {
        panic!("expected the terminal status last");
    };
    assert_eq!(status.status.state, TaskState::Failed);
    assert_eq!(
        status.status.message.as_ref().expect("message").parts[0].as_text(),
        Some("run failed")
    );
}

#[test]
fn tool_and_hook_events_stay_in_the_run_log() {
    assert!(updates(RunEventPayload::ToolUse {
        tool_use_id: "t1".into(),
        name: "Bash".into(),
        input: serde_json::json!({}),
    })
    .is_empty());
    assert!(updates(RunEventPayload::ToolResult {
        tool_use_id: "t1".into(),
        content: "ok".into(),
        is_error: false,
    })
    .is_empty());
    assert!(updates(RunEventPayload::Hook {
        name: "PreToolUse".into(),
        decision: HookDecision::Deny,
        policy: None,
    })
    .is_empty());
}
