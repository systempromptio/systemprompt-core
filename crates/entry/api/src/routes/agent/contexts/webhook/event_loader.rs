use serde_json::json;
use systemprompt_agent::models::a2a::TaskState;
use systemprompt_identifiers::{MessageId, TaskId, UserId};
use systemprompt_models::ExecutionStep;
use systemprompt_runtime::AppContext;

use super::error::LoadEventError;
use super::types::{AgUiWebhookData, WebhookRequest};
use super::validation::validate_json_serializable;
use systemprompt_agent::repository::content::ArtifactRepository;
use systemprompt_agent::repository::context::ContextRepository;
use systemprompt_agent::repository::execution::ExecutionStepRepository;
use systemprompt_agent::repository::task::TaskRepository;

pub(super) async fn load_event_data(
    app_context: &AppContext,
    request: &WebhookRequest,
) -> Result<AgUiWebhookData, LoadEventError> {
    let db = app_context.db_pool();

    match request.event_type.as_str() {
        "task_completed" => load_task_completed(db, request).await,
        "artifact_created" => load_artifact_created(db, request).await,
        "message_received" => load_message_received(db, request).await,
        "context_updated" => load_context_updated(db, request).await,
        "execution_step" => load_execution_step(request),
        "task_created" => load_task_created(request),
        other => Err(LoadEventError::UnknownEventType(other.to_owned())),
    }
}

async fn load_task_completed(
    db: &systemprompt_database::DbPool,
    request: &WebhookRequest,
) -> Result<AgUiWebhookData, LoadEventError> {
    let task_repo = TaskRepository::new(db)?;
    let artifact_repo = ArtifactRepository::new(db)?;
    let step_repo = ExecutionStepRepository::new(db)?;

    let task_id = TaskId::new(&request.entity_id);
    let timestamp = chrono::Utc::now();
    task_repo
        .update_task_state(&task_id, TaskState::Completed, &timestamp)
        .await?;

    let mut task = task_repo
        .get_task(&task_id)
        .await?
        .ok_or_else(|| LoadEventError::NotFound {
            entity: "Task",
            id: request.entity_id.clone(),
        })?;

    let artifacts = artifact_repo
        .get_artifacts_by_task(&task_id)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!(error = %e, task_id = %task_id, "Failed to load artifacts for webhook event");
            vec![]
        });

    let messages = task_repo
        .get_messages_by_task(&task_id)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!(error = %e, task_id = %task_id, "Failed to load messages for webhook event");
            vec![]
        });

    let execution_steps = step_repo.list_by_task(&task_id).await.unwrap_or_else(|e| {
        tracing::warn!(error = %e, task_id = %task_id, "Failed to load execution steps for webhook event");
        vec![]
    });

    task.history = if messages.is_empty() {
        None
    } else {
        Some(messages)
    };

    if !execution_steps.is_empty()
        && let Some(ref mut metadata) = task.metadata
    {
        metadata.execution_steps = Some(execution_steps.clone());
    }

    let payload = json!({
        "task": task,
        "artifacts": if artifacts.is_empty() { None } else { Some(&artifacts) },
        "executionSteps": if execution_steps.is_empty() { None } else { Some(&execution_steps) },
    });

    validate_json_serializable(&payload).map_err(LoadEventError::InvalidPayload)?;

    Ok(AgUiWebhookData {
        event_name: "task_completed".to_owned(),
        payload,
    })
}

async fn load_artifact_created(
    db: &systemprompt_database::DbPool,
    request: &WebhookRequest,
) -> Result<AgUiWebhookData, LoadEventError> {
    let artifact_repo = ArtifactRepository::new(db)?;

    let artifact_id = systemprompt_identifiers::ArtifactId::new(&request.entity_id);
    let artifact = artifact_repo
        .get_artifact_by_id(&artifact_id)
        .await?
        .ok_or_else(|| LoadEventError::NotFound {
            entity: "Artifact",
            id: request.entity_id.clone(),
        })?;

    Ok(AgUiWebhookData {
        event_name: "artifact".to_owned(),
        payload: json!({
            "artifact": artifact,
            "taskId": artifact.metadata.task_id,
            "contextId": request.context_id,
        }),
    })
}

async fn load_message_received(
    db: &systemprompt_database::DbPool,
    request: &WebhookRequest,
) -> Result<AgUiWebhookData, LoadEventError> {
    let task_repo = TaskRepository::new(db)?;
    let exists = task_repo
        .message_exists(&MessageId::new(request.entity_id.clone()))
        .await?;

    if exists {
        Ok(AgUiWebhookData {
            event_name: "message_received".to_owned(),
            payload: json!({
                "messageId": request.entity_id,
            }),
        })
    } else {
        Err(LoadEventError::NotFound {
            entity: "Message",
            id: request.entity_id.clone(),
        })
    }
}

async fn load_context_updated(
    db: &systemprompt_database::DbPool,
    request: &WebhookRequest,
) -> Result<AgUiWebhookData, LoadEventError> {
    let context_repo = ContextRepository::new(db)?;
    let context_id = request.context_id.clone();
    let user_id = UserId::new(request.user_id.clone());

    let context = context_repo.get_context(&context_id, &user_id).await?;

    Ok(AgUiWebhookData {
        event_name: "context_updated".to_owned(),
        payload: json!({
            "contextId": request.context_id,
            "context": context,
        }),
    })
}

#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "items are re-exported via `test_api` only when the feature is on"
    )
)]
pub fn load_execution_step(request: &WebhookRequest) -> Result<AgUiWebhookData, LoadEventError> {
    let step_data = request
        .step_data
        .as_ref()
        .ok_or(LoadEventError::MissingField("step_data"))?;

    let step: ExecutionStep =
        serde_json::from_value(step_data.clone()).map_err(|e| LoadEventError::Deserialize {
            field: "step_data",
            source: e,
        })?;

    let event_name = match step.status {
        systemprompt_models::StepStatus::Completed => "step_finished",
        _ => "step_started",
    };

    Ok(AgUiWebhookData {
        event_name: event_name.to_owned(),
        payload: json!({
            "stepName": step.step_type().to_string(),
            "taskId": step.task_id,
            "step": step,
        }),
    })
}

#[derive(serde::Deserialize)]
struct TaskCreatedData {
    task: systemprompt_agent::models::a2a::Task,
}

#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "items are re-exported via `test_api` only when the feature is on"
    )
)]
pub fn load_task_created(request: &WebhookRequest) -> Result<AgUiWebhookData, LoadEventError> {
    let task_data = request
        .task_data
        .as_ref()
        .ok_or(LoadEventError::MissingField("task_data"))?;

    let payload: TaskCreatedData =
        serde_json::from_value(task_data.clone()).map_err(|e| LoadEventError::Deserialize {
            field: "task_data",
            source: e,
        })?;

    if payload.task.history.as_ref().is_none_or(Vec::is_empty) {
        return Err(LoadEventError::InvalidPayload(format!(
            "task_created payload has empty history for task {}",
            request.entity_id
        )));
    }

    Ok(AgUiWebhookData {
        event_name: "run_started".to_owned(),
        payload: json!({
            "task": payload.task,
            "threadId": request.context_id,
            "runId": request.entity_id,
        }),
    })
}
