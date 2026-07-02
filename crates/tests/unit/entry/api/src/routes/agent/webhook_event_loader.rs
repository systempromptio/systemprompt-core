//! Unit tests for the pure webhook event loaders: `execution_step` and
//! `task_created` payload shaping.

use serde_json::json;
use systemprompt_api::routes::agent::contexts::webhook::WebhookRequest;
use systemprompt_api::routes::agent::contexts::webhook::test_api::{
    LoadEventError, load_execution_step, load_task_created,
};
use systemprompt_identifiers::{ContextId, MessageId, TaskId, UserId};
use systemprompt_models::a2a::{Message, MessageRole, Part, Task, TextPart};
use systemprompt_models::execution::{ExecutionStep, StepContent, StepStatus};

fn request(event_type: &str) -> WebhookRequest {
    WebhookRequest {
        event_type: event_type.to_owned(),
        entity_id: "entity-1".to_owned(),
        context_id: ContextId::generate(),
        user_id: UserId::new("user-1"),
        step_data: None,
        task_data: None,
    }
}

fn sample_message(context_id: &ContextId) -> Message {
    Message {
        role: MessageRole::User,
        parts: vec![Part::Text(TextPart {
            text: "hello".to_owned(),
        })],
        message_id: MessageId::generate(),
        task_id: None,
        context_id: context_id.clone(),
        metadata: None,
        extensions: None,
        reference_task_ids: None,
    }
}

#[test]
fn execution_step_without_step_data_is_missing_field() {
    let req = request("execution_step");
    let err = load_execution_step(&req).expect_err("missing step_data");
    assert!(matches!(err, LoadEventError::MissingField("step_data")));
}

#[test]
fn execution_step_with_malformed_step_data_is_deserialize_error() {
    let mut req = request("execution_step");
    req.step_data = Some(json!({"not": "a step"}));
    let err = load_execution_step(&req).expect_err("malformed step_data");
    assert!(matches!(
        err,
        LoadEventError::Deserialize {
            field: "step_data",
            ..
        }
    ));
}

#[test]
fn completed_step_maps_to_step_finished() {
    let mut step = ExecutionStep::new(TaskId::new("task-1"), StepContent::understanding());
    step.status = StepStatus::Completed;
    let mut req = request("execution_step");
    req.step_data = Some(serde_json::to_value(&step).expect("serialize step"));

    let data = load_execution_step(&req).expect("load step");
    assert_eq!(data.event_name, "step_finished");
    assert_eq!(data.payload["taskId"], json!("task-1"));
    assert!(data.payload["stepName"].is_string());
}

#[test]
fn in_progress_step_maps_to_step_started() {
    let mut step = ExecutionStep::new(TaskId::new("task-2"), StepContent::understanding());
    step.status = StepStatus::InProgress;
    let mut req = request("execution_step");
    req.step_data = Some(serde_json::to_value(&step).expect("serialize step"));

    let data = load_execution_step(&req).expect("load step");
    assert_eq!(data.event_name, "step_started");
    assert_eq!(data.payload["taskId"], json!("task-2"));
}

#[test]
fn task_created_without_task_data_is_missing_field() {
    let req = request("task_created");
    let err = load_task_created(&req).expect_err("missing task_data");
    assert!(matches!(err, LoadEventError::MissingField("task_data")));
}

#[test]
fn task_created_with_malformed_task_data_is_deserialize_error() {
    let mut req = request("task_created");
    req.task_data = Some(json!({"task": 42}));
    let err = load_task_created(&req).expect_err("malformed task_data");
    assert!(matches!(
        err,
        LoadEventError::Deserialize {
            field: "task_data",
            ..
        }
    ));
}

#[test]
fn task_created_with_empty_history_is_invalid_payload() {
    let task = Task::default();
    let mut req = request("task_created");
    req.task_data = Some(json!({"task": serde_json::to_value(&task).expect("serialize task")}));
    let err = load_task_created(&req).expect_err("empty history");
    assert!(matches!(err, LoadEventError::InvalidPayload(msg) if msg.contains("entity-1")));
}

#[test]
fn task_created_with_history_maps_to_run_started() {
    let mut task = Task::default();
    task.history = Some(vec![sample_message(&task.context_id)]);
    let mut req = request("task_created");
    req.task_data = Some(json!({"task": serde_json::to_value(&task).expect("serialize task")}));

    let data = load_task_created(&req).expect("load task_created");
    assert_eq!(data.event_name, "run_started");
    assert_eq!(data.payload["runId"], json!("entity-1"));
    assert_eq!(data.payload["threadId"], json!(req.context_id.as_str()));
    assert_eq!(data.payload["task"]["id"], json!(task.id.as_str()));
}
