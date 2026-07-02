// Tests for the pure batch-assembly helpers that turn joined DB rows into
// A2A `ExecutionStep`s, `Message`s, and `Artifact`s during batch task
// construction: row parsing, invalid-row skipping, metadata merging, and the
// empty/None edge cases.

use std::collections::HashMap;

use systemprompt_agent::models::a2a::{MessageRole, Part};
use systemprompt_agent::models::{
    ArtifactPartRow, ArtifactRow, ExecutionStepBatchRow, MessagePart, TaskMessage,
};
use systemprompt_agent::repository::task::constructor::batch_builders::{
    build_artifacts, build_execution_steps, build_messages,
};
use systemprompt_identifiers::{
    ArtifactId, ContextId, ExecutionStepId, MessageId, SessionId, TaskId, TraceId, UserId,
};
use systemprompt_models::{StepContent, StepStatus};

fn step_row(task_id: &TaskId, status: &str, content: serde_json::Value) -> ExecutionStepBatchRow {
    ExecutionStepBatchRow {
        step_id: ExecutionStepId::generate(),
        task_id: task_id.clone(),
        status: status.to_owned(),
        content,
        started_at: chrono::Utc::now(),
        completed_at: None,
        duration_ms: Some(12),
        error_message: None,
    }
}

fn message_row(task_id: &TaskId, ctx: &ContextId, role: &str, seq: i32) -> TaskMessage {
    TaskMessage {
        id: seq,
        task_id: task_id.clone(),
        message_id: MessageId::generate(),
        client_message_id: None,
        role: role.to_owned(),
        context_id: ctx.clone(),
        user_id: Some(UserId::new("u-batch")),
        session_id: Some(SessionId::generate()),
        trace_id: Some(TraceId::generate()),
        sequence_number: seq,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        metadata: None,
        reference_task_ids: None,
    }
}

fn artifact_row(task_id: &TaskId, ctx: &ContextId) -> ArtifactRow {
    ArtifactRow {
        artifact_id: ArtifactId::generate(),
        task_id: task_id.clone(),
        context_id: ctx.clone(),
        name: Some("report".to_owned()),
        description: Some("a report".to_owned()),
        artifact_type: "document".to_owned(),
        source: Some("tool".to_owned()),
        tool_name: Some("writer".to_owned()),
        mcp_execution_id: None,
        fingerprint: Some("fp".to_owned()),
        skill_id: None,
        skill_name: None,
        metadata: Some(serde_json::json!({
            "rendering_hints": {"kind": "markdown"},
            "is_internal": false,
            "execution_index": 3
        })),
        created_at: chrono::Utc::now(),
    }
}

fn artifact_part_row(
    artifact_id: &ArtifactId,
    ctx: &ContextId,
    kind: &str,
    seq: i32,
) -> ArtifactPartRow {
    ArtifactPartRow {
        id: seq,
        artifact_id: artifact_id.clone(),
        context_id: ctx.clone(),
        part_kind: kind.to_owned(),
        sequence_number: seq,
        text_content: (kind == "text").then(|| "hello".to_owned()),
        file_name: (kind == "file").then(|| "f.txt".to_owned()),
        file_mime_type: (kind == "file").then(|| "text/plain".to_owned()),
        file_uri: None,
        file_bytes: None,
        data_content: (kind == "data").then(|| serde_json::json!({"k": "v"})),
        metadata: None,
    }
}

#[test]
fn build_execution_steps_none_and_empty_inputs_yield_none() {
    assert!(build_execution_steps(None).is_none());
    let empty: Vec<&ExecutionStepBatchRow> = Vec::new();
    assert!(build_execution_steps(Some(&empty)).is_none());
}

#[test]
fn build_execution_steps_parses_valid_rows() {
    let task_id = TaskId::generate();
    let row = step_row(
        &task_id,
        "completed",
        serde_json::json!({"type": "completion"}),
    );
    let rows = vec![&row];

    let steps = build_execution_steps(Some(&rows)).expect("steps");
    assert_eq!(steps.len(), 1);
    assert_eq!(steps[0].status, StepStatus::Completed);
    assert_eq!(steps[0].content, StepContent::Completion);
    assert_eq!(steps[0].duration_ms, Some(12));
}

#[test]
fn build_execution_steps_skips_invalid_rows() {
    let task_id = TaskId::generate();
    let bad_status = step_row(
        &task_id,
        "nonsense",
        serde_json::json!({"type": "completion"}),
    );
    let bad_content = step_row(
        &task_id,
        "pending",
        serde_json::json!({"type": "unknown_kind"}),
    );
    let good = step_row(
        &task_id,
        "in_progress",
        serde_json::json!({"type": "understanding"}),
    );
    let rows = vec![&bad_status, &bad_content, &good];

    let steps = build_execution_steps(Some(&rows)).expect("steps");
    assert_eq!(steps.len(), 1);
    assert_eq!(steps[0].status, StepStatus::InProgress);
}

#[test]
fn build_execution_steps_all_invalid_yields_none() {
    let task_id = TaskId::generate();
    let bad = step_row(
        &task_id,
        "nonsense",
        serde_json::json!({"type": "completion"}),
    );
    let rows = vec![&bad];
    assert!(build_execution_steps(Some(&rows)).is_none());
}

#[test]
fn build_messages_none_and_empty_inputs_yield_none() {
    let parts: HashMap<MessageId, Vec<&MessagePart>> = HashMap::new();
    assert!(build_messages(None, &parts).is_none());
    let empty: Vec<&TaskMessage> = Vec::new();
    assert!(build_messages(Some(&empty), &parts).is_none());
}

#[test]
fn build_messages_maps_roles_and_reference_task_ids() {
    let task_id = TaskId::generate();
    let ctx = ContextId::generate();
    let mut user_row = message_row(&task_id, &ctx, "ROLE_USER", 1);
    user_row.reference_task_ids = Some(vec!["t-ref".to_owned()]);
    let agent_row = message_row(&task_id, &ctx, "agent", 2);
    let rows = vec![&user_row, &agent_row];
    let parts: HashMap<MessageId, Vec<&MessagePart>> = HashMap::new();

    let messages = build_messages(Some(&rows), &parts).expect("messages");
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].role, MessageRole::User);
    assert_eq!(messages[1].role, MessageRole::Agent);
    let refs = messages[0].reference_task_ids.as_ref().expect("refs");
    assert_eq!(refs[0], TaskId::new("t-ref"));
    assert!(messages[1].metadata.is_none());
}

#[test]
fn build_messages_merges_client_message_id_into_metadata() {
    let task_id = TaskId::generate();
    let ctx = ContextId::generate();
    let mut row = message_row(&task_id, &ctx, "user", 1);
    row.client_message_id = Some("cmid-1".to_owned());
    row.metadata = Some(serde_json::json!({"existing": true}));
    let rows = vec![&row];
    let parts: HashMap<MessageId, Vec<&MessagePart>> = HashMap::new();

    let messages = build_messages(Some(&rows), &parts).expect("messages");
    let metadata = messages[0].metadata.as_ref().expect("metadata");
    assert_eq!(metadata["clientMessageId"], "cmid-1");
    assert_eq!(metadata["existing"], true);
}

#[test]
fn build_artifacts_none_and_empty_inputs_yield_none() {
    let parts: HashMap<ArtifactId, Vec<&ArtifactPartRow>> = HashMap::new();
    assert!(build_artifacts(None, &parts).is_none());
    let empty: Vec<&ArtifactRow> = Vec::new();
    assert!(build_artifacts(Some(&empty), &parts).is_none());
}

#[test]
fn build_artifacts_assembles_metadata_and_parts() {
    let task_id = TaskId::generate();
    let ctx = ContextId::generate();
    let row = artifact_row(&task_id, &ctx);
    let text_part = artifact_part_row(&row.artifact_id, &ctx, "text", 1);
    let file_part = artifact_part_row(&row.artifact_id, &ctx, "file", 2);
    let data_part = artifact_part_row(&row.artifact_id, &ctx, "data", 3);
    let unknown_part = artifact_part_row(&row.artifact_id, &ctx, "mystery", 4);
    let mut parts: HashMap<ArtifactId, Vec<&ArtifactPartRow>> = HashMap::new();
    parts.insert(
        row.artifact_id.clone(),
        vec![&text_part, &file_part, &data_part, &unknown_part],
    );
    let rows = vec![&row];

    let artifacts = build_artifacts(Some(&rows), &parts).expect("artifacts");
    assert_eq!(artifacts.len(), 1);
    let artifact = &artifacts[0];
    assert_eq!(artifact.title.as_deref(), Some("report"));
    assert_eq!(artifact.metadata.artifact_type, "document");
    assert_eq!(artifact.metadata.execution_index, Some(3));
    assert_eq!(artifact.metadata.is_internal, Some(false));
    assert!(artifact.metadata.rendering_hints.is_some());
    assert_eq!(artifact.parts.len(), 3);
    assert!(matches!(artifact.parts[0], Part::Text(_)));
    assert!(matches!(artifact.parts[1], Part::File(_)));
    assert!(matches!(artifact.parts[2], Part::Data(_)));
    assert_eq!(artifact.extensions.len(), 1);
}

#[test]
fn build_artifacts_without_parts_or_metadata_uses_defaults() {
    let task_id = TaskId::generate();
    let ctx = ContextId::generate();
    let mut row = artifact_row(&task_id, &ctx);
    row.metadata = None;
    let parts: HashMap<ArtifactId, Vec<&ArtifactPartRow>> = HashMap::new();
    let rows = vec![&row];

    let artifacts = build_artifacts(Some(&rows), &parts).expect("artifacts");
    let artifact = &artifacts[0];
    assert!(artifact.parts.is_empty());
    assert!(artifact.metadata.rendering_hints.is_none());
    assert_eq!(artifact.extensions.len(), 1);
}
