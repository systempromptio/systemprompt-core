use systemprompt_agent::models::a2a::{Message, MessageRole, Part, Task, TaskState, TextPart};
use systemprompt_agent::services::a2a_server::processing::task_builder::{
    TaskBuilder, build_canceled_task, build_completed_task, build_mock_task,
};
use systemprompt_identifiers::{ContextId, MessageId, TaskId};
use systemprompt_models::a2a::TaskMetadata;

fn make_user_message(context_id: &ContextId, task_id: &TaskId) -> Message {
    Message {
        role: MessageRole::User,
        parts: vec![Part::Text(TextPart {
            text: "Hello agent".to_string(),
        })],
        message_id: MessageId::generate(),
        task_id: Some(task_id.clone()),
        context_id: context_id.clone(),
        metadata: None,
        extensions: None,
        reference_task_ids: None,
    }
}

#[test]
fn task_builder_new_defaults_to_completed() {
    let ctx = ContextId::generate();
    let task = TaskBuilder::new(ctx).build();
    assert_eq!(task.status.state, TaskState::Completed);
}

#[test]
fn task_builder_new_generates_ids() {
    let ctx = ContextId::generate();
    let task = TaskBuilder::new(ctx.clone()).build();
    assert_eq!(task.context_id, ctx);
    assert!(!task.id.to_string().is_empty());
}

#[test]
fn task_builder_with_task_id() {
    let ctx = ContextId::generate();
    let tid = TaskId::generate();
    let task = TaskBuilder::new(ctx).with_task_id(tid.clone()).build();
    assert_eq!(task.id, tid);
}

#[test]
fn task_builder_with_state_working() {
    let ctx = ContextId::generate();
    let task = TaskBuilder::new(ctx)
        .with_state(TaskState::Working)
        .build();
    assert_eq!(task.status.state, TaskState::Working);
}

#[test]
fn task_builder_with_state_canceled() {
    let ctx = ContextId::generate();
    let task = TaskBuilder::new(ctx)
        .with_state(TaskState::Canceled)
        .build();
    assert_eq!(task.status.state, TaskState::Canceled);
}

#[test]
fn task_builder_with_state_failed() {
    let ctx = ContextId::generate();
    let task = TaskBuilder::new(ctx)
        .with_state(TaskState::Failed)
        .build();
    assert_eq!(task.status.state, TaskState::Failed);
}

#[test]
fn task_builder_with_response_text() {
    let ctx = ContextId::generate();
    let task = TaskBuilder::new(ctx)
        .with_response_text("Generated answer".to_string())
        .build();
    let status_msg = task.status.message.unwrap();
    let text = extract_text_from_message(&status_msg);
    assert_eq!(text, "Generated answer");
}

#[test]
fn task_builder_with_message_id() {
    let ctx = ContextId::generate();
    let mid = MessageId::generate();
    let task = TaskBuilder::new(ctx)
        .with_message_id(mid.clone())
        .build();
    let status_msg = task.status.message.unwrap();
    assert_eq!(status_msg.message_id, mid);
}

#[test]
fn task_builder_without_user_message_has_no_history() {
    let ctx = ContextId::generate();
    let task = TaskBuilder::new(ctx).build();
    assert!(task.history.is_none());
}

#[test]
fn task_builder_with_user_message_creates_history() {
    let ctx = ContextId::generate();
    let tid = TaskId::generate();
    let user_msg = make_user_message(&ctx, &tid);
    let task = TaskBuilder::new(ctx)
        .with_task_id(tid)
        .with_user_message(user_msg)
        .build();
    let history = task.history.unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].role, MessageRole::User);
    assert_eq!(history[1].role, MessageRole::Agent);
}

#[test]
fn task_builder_with_user_message_preserves_response_text_in_history() {
    let ctx = ContextId::generate();
    let tid = TaskId::generate();
    let user_msg = make_user_message(&ctx, &tid);
    let task = TaskBuilder::new(ctx)
        .with_task_id(tid)
        .with_response_text("My response".to_string())
        .with_user_message(user_msg)
        .build();
    let history = task.history.unwrap();
    let agent_msg = &history[1];
    let text = extract_text_from_message(agent_msg);
    assert_eq!(text, "My response");
}

#[test]
fn task_builder_without_artifacts_sets_none() {
    let ctx = ContextId::generate();
    let task = TaskBuilder::new(ctx).build();
    assert!(task.artifacts.is_none());
}

#[test]
fn task_builder_with_empty_artifacts_sets_none() {
    let ctx = ContextId::generate();
    let task = TaskBuilder::new(ctx).with_artifacts(vec![]).build();
    assert!(task.artifacts.is_none());
}

#[test]
fn task_builder_with_metadata() {
    let ctx = ContextId::generate();
    let metadata = TaskMetadata::new_agent_message("test-agent".to_string());
    let task = TaskBuilder::new(ctx).with_metadata(metadata).build();
    assert!(task.metadata.is_some());
    assert_eq!(task.metadata.unwrap().agent_name, "test-agent");
}

#[test]
fn task_builder_without_metadata_sets_none() {
    let ctx = ContextId::generate();
    let task = TaskBuilder::new(ctx).build();
    assert!(task.metadata.is_none());
}

#[test]
fn task_builder_status_message_role_is_agent() {
    let ctx = ContextId::generate();
    let task = TaskBuilder::new(ctx).build();
    let msg = task.status.message.unwrap();
    assert_eq!(msg.role, MessageRole::Agent);
}

#[test]
fn task_builder_status_has_timestamp() {
    let ctx = ContextId::generate();
    let task = TaskBuilder::new(ctx).build();
    assert!(task.status.timestamp.is_some());
}

#[test]
fn task_builder_full_chain() {
    let ctx = ContextId::generate();
    let tid = TaskId::generate();
    let mid = MessageId::generate();
    let user_msg = make_user_message(&ctx, &tid);
    let metadata = TaskMetadata::new_agent_message("chained-agent".to_string());

    let task = TaskBuilder::new(ctx.clone())
        .with_task_id(tid.clone())
        .with_state(TaskState::Completed)
        .with_response_text("Full chain response".to_string())
        .with_message_id(mid.clone())
        .with_user_message(user_msg)
        .with_metadata(metadata)
        .build();

    assert_eq!(task.id, tid);
    assert_eq!(task.context_id, ctx);
    assert_eq!(task.status.state, TaskState::Completed);
    assert!(task.history.is_some());
    assert!(task.metadata.is_some());
}

#[test]
fn task_builder_debug_impl() {
    let ctx = ContextId::generate();
    let builder = TaskBuilder::new(ctx);
    let debug_str = format!("{:?}", builder);
    assert!(debug_str.contains("TaskBuilder"));
}

#[test]
fn build_completed_task_sets_completed_state() {
    let tid = TaskId::generate();
    let ctx = ContextId::generate();
    let user_msg = make_user_message(&ctx, &tid);
    let task = build_completed_task(
        tid.clone(),
        ctx.clone(),
        "done".to_string(),
        user_msg,
        vec![],
    );
    assert_eq!(task.status.state, TaskState::Completed);
    assert_eq!(task.id, tid);
    assert_eq!(task.context_id, ctx);
}

#[test]
fn build_completed_task_has_history_with_user_and_agent() {
    let tid = TaskId::generate();
    let ctx = ContextId::generate();
    let user_msg = make_user_message(&ctx, &tid);
    let task = build_completed_task(tid, ctx, "response".to_string(), user_msg, vec![]);
    let history = task.history.unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].role, MessageRole::User);
    assert_eq!(history[1].role, MessageRole::Agent);
}

#[test]
fn build_canceled_task_sets_canceled_state() {
    let tid = TaskId::generate();
    let ctx = ContextId::generate();
    let task = build_canceled_task(tid.clone(), ctx.clone());
    assert_eq!(task.status.state, TaskState::Canceled);
    assert_eq!(task.id, tid);
    assert_eq!(task.context_id, ctx);
}

#[test]
fn build_canceled_task_has_cancel_message() {
    let tid = TaskId::generate();
    let ctx = ContextId::generate();
    let task = build_canceled_task(tid, ctx);
    let msg = task.status.message.unwrap();
    let text = extract_text_from_message(&msg);
    assert_eq!(text, "Task was canceled.");
}

#[test]
fn build_canceled_task_has_no_history() {
    let tid = TaskId::generate();
    let ctx = ContextId::generate();
    let task = build_canceled_task(tid, ctx);
    assert!(task.history.is_none());
}

#[test]
fn build_mock_task_sets_completed_state() {
    let tid = TaskId::generate();
    let task = build_mock_task(tid.clone());
    assert_eq!(task.status.state, TaskState::Completed);
    assert_eq!(task.id, tid);
}

#[test]
fn build_mock_task_has_success_message() {
    let tid = TaskId::generate();
    let task = build_mock_task(tid);
    let msg = task.status.message.unwrap();
    let text = extract_text_from_message(&msg);
    assert_eq!(text, "Task completed successfully.");
}

#[test]
fn build_mock_task_generates_context_id() {
    let tid = TaskId::generate();
    let task = build_mock_task(tid);
    assert!(!task.context_id.to_string().is_empty());
}

fn extract_text_from_message(msg: &Message) -> String {
    msg.parts
        .iter()
        .filter_map(|p| match p {
            Part::Text(tp) => Some(tp.text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("")
}
