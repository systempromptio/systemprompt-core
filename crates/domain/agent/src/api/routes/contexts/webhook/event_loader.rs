use serde_json::json;
use systemprompt_identifiers::{ContextId, TaskId, UserId};
use systemprompt_models::ExecutionStep;
use systemprompt_runtime::AppContext;

use super::types::{AgUiWebhookData, WebhookRequest};
use super::validation::validate_json_serializable;
use crate::repository::content::ArtifactRepository;
use crate::repository::context::ContextRepository;
use crate::repository::execution::ExecutionStepRepository;
use crate::repository::task::TaskRepository;

pub async fn load_event_data(
    app_context: &AppContext,
    request: &WebhookRequest,
) -> Result<AgUiWebhookData, anyhow::Error> {
    let db = app_context.db_pool();

    match request.event_type.as_str() {
        "task_completed" => load_task_completed(db, request).await,
        "artifact_created" => load_artifact_created(db, request).await,
        "message_received" => load_message_received(db, request).await,
        "context_updated" => load_context_updated(db, request).await,
        "execution_step" => load_execution_step(request).await,
        "task_created" => load_task_created(request).await,
        _ => Err(anyhow::anyhow!(
            "Unknown event type: {}",
            request.event_type
        )),
    }
}

async fn load_task_completed(
    db: &systemprompt_core_database::DbPool,
    request: &WebhookRequest,
) -> Result<AgUiWebhookData, anyhow::Error> {
    let task_repo = TaskRepository::new(db.clone());
    let artifact_repo = ArtifactRepository::new(db.clone());
    let step_repo = ExecutionStepRepository::new(db.clone())?;

    use crate::models::a2a::TaskState;
    let task_id = TaskId::new(&request.entity_id);
    let timestamp = chrono::Utc::now();
    task_repo
        .update_task_state(&task_id, TaskState::Completed, &timestamp)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to complete task: {}", e))?;

    let mut task = task_repo
        .get_task(&task_id)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to load task: {}", e))?
        .ok_or_else(|| anyhow::anyhow!("Task not found: {}", request.entity_id))?;

    let artifacts = artifact_repo
        .get_artifacts_by_task(&task_id)
        .await
        .unwrap_or_default();

    let messages = task_repo
        .get_messages_by_task(&task_id)
        .await
        .unwrap_or_default();

    let execution_steps = step_repo.list_by_task(&task_id).await.unwrap_or_default();

    task.history = if messages.is_empty() {
        None
    } else {
        Some(messages)
    };

    if !execution_steps.is_empty() {
        if let Some(ref mut metadata) = task.metadata {
            metadata.execution_steps = Some(execution_steps.clone());
        }
    }

    let payload = json!({
        "task": task,
        "artifacts": if artifacts.is_empty() { None } else { Some(&artifacts) },
        "executionSteps": if execution_steps.is_empty() { None } else { Some(&execution_steps) },
    });

    validate_json_serializable(&payload)
        .map_err(|e| anyhow::anyhow!("JSON validation failed: {}", e))?;

    Ok(AgUiWebhookData {
        event_name: "task_completed".to_string(),
        payload,
    })
}

async fn load_artifact_created(
    db: &systemprompt_core_database::DbPool,
    request: &WebhookRequest,
) -> Result<AgUiWebhookData, anyhow::Error> {
    let artifact_repo = ArtifactRepository::new(db.clone());

    let artifact_id = systemprompt_identifiers::ArtifactId::new(&request.entity_id);
    let artifact = artifact_repo
        .get_artifact_by_id(&artifact_id)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to load artifact: {}", e))?
        .ok_or_else(|| anyhow::anyhow!("Artifact not found: {}", request.entity_id))?;

    Ok(AgUiWebhookData {
        event_name: "artifact".to_string(),
        payload: json!({
            "artifact": artifact,
            "taskId": artifact.metadata.task_id,
            "contextId": request.context_id,
        }),
    })
}

async fn load_message_received(
    db: &systemprompt_core_database::DbPool,
    request: &WebhookRequest,
) -> Result<AgUiWebhookData, anyhow::Error> {
    let pool = db
        .pool_arc()
        .map_err(|e| anyhow::anyhow!("Database error: {}", e))?;

    let message = sqlx::query!(
        r#"SELECT m.id, m.message_id, STRING_AGG(mp.id::text, ',') as part_ids
        FROM task_messages m
        LEFT JOIN message_parts mp ON m.message_id = mp.message_id
        WHERE m.message_id = $1
        GROUP BY m.id, m.message_id"#,
        request.entity_id
    )
    .fetch_optional(pool.as_ref())
    .await
    .map_err(|e| anyhow::anyhow!("Failed to load message: {}", e))?;

    if message.is_some() {
        Ok(AgUiWebhookData {
            event_name: "message_received".to_string(),
            payload: json!({
                "messageId": request.entity_id,
            }),
        })
    } else {
        Err(anyhow::anyhow!("Message not found: {}", request.entity_id))
    }
}

async fn load_context_updated(
    db: &systemprompt_core_database::DbPool,
    request: &WebhookRequest,
) -> Result<AgUiWebhookData, anyhow::Error> {
    let context_repo = ContextRepository::new(db.clone());
    let context_id = ContextId::from(request.context_id.clone());
    let user_id = UserId::new(request.user_id.clone());

    let context = context_repo
        .get_context(&context_id, &user_id)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to load context: {}", e))?;

    Ok(AgUiWebhookData {
        event_name: "context_updated".to_string(),
        payload: json!({
            "contextId": request.context_id,
            "context": context,
        }),
    })
}

async fn load_execution_step(request: &WebhookRequest) -> Result<AgUiWebhookData, anyhow::Error> {
    let step_data = request
        .step_data
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("step_data required for execution_step events"))?;

    let step: ExecutionStep = serde_json::from_value(step_data.clone())
        .map_err(|e| anyhow::anyhow!("Invalid step_data format: {}", e))?;

    let event_name = match step.status {
        systemprompt_models::StepStatus::Completed => "step_finished",
        _ => "step_started",
    };

    Ok(AgUiWebhookData {
        event_name: event_name.to_string(),
        payload: json!({
            "stepName": step.step_type().to_string(),
            "taskId": step.task_id,
            "step": step,
        }),
    })
}

async fn load_task_created(request: &WebhookRequest) -> Result<AgUiWebhookData, anyhow::Error> {
    let task_data = request.task_data.as_ref().ok_or_else(|| {
        anyhow::anyhow!(
            "task_created event MUST include task_data. Task ID: {}",
            request.entity_id
        )
    })?;

    #[derive(serde::Deserialize)]
    struct TaskCreatedData {
        task: crate::models::a2a::Task,
    }

    let payload: TaskCreatedData = serde_json::from_value(task_data.clone()).map_err(|e| {
        anyhow::anyhow!(
            "Failed to deserialize task_created data for task {}: {}",
            request.entity_id,
            e
        )
    })?;

    if payload.task.history.is_none() || payload.task.history.as_ref().map_or(true, Vec::is_empty) {
        return Err(anyhow::anyhow!(
            "task_created payload has empty history - user message is missing! Task ID: {}",
            request.entity_id
        ));
    }

    Ok(AgUiWebhookData {
        event_name: "run_started".to_string(),
        payload: json!({
            "task": payload.task,
            "threadId": request.context_id,
            "runId": request.entity_id,
        }),
    })
}
