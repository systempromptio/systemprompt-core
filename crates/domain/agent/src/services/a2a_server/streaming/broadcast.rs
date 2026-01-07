use serde_json::json;
use systemprompt_identifiers::{ContextId, TaskId};
use systemprompt_models::{Config, TaskMetadata};

use crate::models::a2a::{Message, Task, TaskState, TaskStatus};

pub async fn broadcast_task_created(
    task_id: &TaskId,
    context_id: &ContextId,
    user_id: &str,
    user_message: &Message,
    agent_name: &str,
    token: &str,
) {
    let event_task = build_event_task(task_id, context_id, user_message, agent_name);

    let api_url = match Config::get() {
        Ok(c) => c.api_internal_url.clone(),
        Err(e) => {
            tracing::warn!(error = %e, "Cannot broadcast task_created: config unavailable");
            return;
        },
    };
    let webhook_url = format!("{}/api/v1/webhook/broadcast", api_url);

    let payload = json!({
        "event_type": "task_created",
        "entity_id": task_id.as_str(),
        "context_id": context_id.as_str(),
        "user_id": user_id,
        "task_data": json!({ "task": event_task })
    });

    let client = reqwest::Client::new();
    match client
        .post(&webhook_url)
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
    {
        Ok(response) => {
            if response.status().is_success() {
                tracing::info!(task_id = %task_id, "Broadcast task_created via webhook");
            } else {
                tracing::warn!(
                    task_id = %task_id,
                    status = %response.status(),
                    "Webhook broadcast failed"
                );
            }
        },
        Err(e) => {
            tracing::warn!(task_id = %task_id, error = %e, "Webhook broadcast error");
        },
    }
}

pub async fn broadcast_task_completed(task: &Task, user_id: &str, token: &str) {
    let api_url = match Config::get() {
        Ok(c) => c.api_internal_url.clone(),
        Err(e) => {
            tracing::warn!(error = %e, "Cannot broadcast task_completed: config unavailable");
            return;
        },
    };
    let webhook_url = format!("{}/api/v1/webhook/broadcast", api_url);

    let task_data = match serde_json::to_value(task) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(error = %e, task_id = %task.id, "Failed to serialize task for broadcast");
            serde_json::json!(null)
        },
    };

    let payload = json!({
        "event_type": "task_completed",
        "entity_id": task.id.as_str(),
        "context_id": task.context_id.as_str(),
        "user_id": user_id,
        "task_data": task_data
    });

    let client = reqwest::Client::new();
    match client
        .post(&webhook_url)
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
    {
        Ok(response) => {
            if response.status().is_success() {
                tracing::info!(task_id = %task.id, "Broadcast task_completed");
            } else {
                tracing::warn!(
                    task_id = %task.id,
                    status = %response.status(),
                    "Webhook failed"
                );
            }
        },
        Err(e) => {
            tracing::warn!(task_id = %task.id, error = %e, "Webhook error");
        },
    }
}

fn build_event_task(
    task_id: &TaskId,
    context_id: &ContextId,
    user_message: &Message,
    agent_name: &str,
) -> Task {
    Task {
        id: task_id.clone(),
        context_id: context_id.clone(),
        status: TaskStatus {
            state: TaskState::Submitted,
            message: None,
            timestamp: Some(chrono::Utc::now()),
        },
        history: Some(vec![user_message.clone()]),
        artifacts: None,
        metadata: Some(TaskMetadata::new_agent_message(agent_name.to_string())),
        kind: "task".to_string(),
    }
}

pub async fn broadcast_artifact_created(
    artifact: &crate::models::a2a::Artifact,
    task_id: &TaskId,
    context_id: &ContextId,
    user_id: &str,
    token: &str,
) -> Result<(), anyhow::Error> {
    let api_url = Config::get()
        .map_err(|e| anyhow::anyhow!("Config unavailable for artifact broadcast: {}", e))?
        .api_internal_url
        .clone();
    let webhook_url = format!("{}/api/v1/webhook/broadcast", api_url);

    let payload = json!({
        "event_type": "artifact_created",
        "entity_id": artifact.id.clone(),
        "context_id": context_id.as_str(),
        "user_id": user_id,
    });

    let client = reqwest::Client::new();
    let response = client
        .post(&webhook_url)
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("Webhook request failed: {}", e))?;

    if response.status().is_success() {
        tracing::info!(
            artifact_id = %artifact.id,
            task_id = %task_id,
            "Broadcast artifact_created via webhook"
        );

        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "Webhook broadcast failed: status={}, artifact_id={}",
            response.status(),
            artifact.id
        ))
    }
}
