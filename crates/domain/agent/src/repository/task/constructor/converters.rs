//! Row-to-model converters for task construction.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::models::TaskRow;
use crate::models::a2a::{Message, MessageRole, Part, TaskState};
use crate::models::database_rows::TaskMessage;
use systemprompt_models::a2a::TaskMetadata;
use systemprompt_traits::RepositoryError;

pub(super) fn parse_task_state(row: &TaskRow) -> Result<TaskState, RepositoryError> {
    row.status.parse().map_err(|e: String| {
        RepositoryError::InvalidData(format!("unrecognised stored task state: {e}"))
    })
}

pub(super) fn message_from_row(row: TaskMessage, parts: Vec<Part>) -> Message {
    let reference_task_ids = row.reference_task_ids.map(|ids| {
        ids.into_iter()
            .map(systemprompt_identifiers::TaskId::new)
            .collect()
    });

    let mut final_metadata = row.metadata.unwrap_or_else(|| serde_json::json!({}));
    if let Some(client_id) = &row.client_message_id
        && let Some(obj) = final_metadata.as_object_mut()
    {
        obj.insert(
            "clientMessageId".to_owned(),
            serde_json::Value::String(client_id.clone()),
        );
    }

    let role = match row.role.as_str() {
        "user" | "ROLE_USER" => MessageRole::User,
        _ => MessageRole::Agent,
    };

    Message {
        role,
        parts,
        message_id: row.message_id,
        task_id: Some(row.task_id),
        context_id: row.context_id,
        metadata: if final_metadata == serde_json::json!({}) {
            None
        } else {
            Some(final_metadata)
        },
        extensions: None,
        reference_task_ids,
    }
}

pub(super) fn construct_metadata(row: &TaskRow) -> TaskMetadata {
    let metadata_json = row
        .metadata
        .as_ref()
        .map_or_else(|| "{}".to_owned(), ToString::to_string);

    let agent_name = row
        .agent_name
        .as_ref()
        .map_or_else(String::new, ToString::to_string);

    let mut metadata = serde_json::from_str::<TaskMetadata>(&metadata_json)
        .unwrap_or_else(|_| TaskMetadata::new_agent_message(agent_name.clone()));

    metadata.agent_name = agent_name;
    metadata.created_at = row.created_at.to_rfc3339();
    metadata.updated_at = Some(row.updated_at.to_rfc3339());
    metadata.started_at = row.started_at.map(|dt| dt.to_rfc3339());
    metadata.completed_at = row.completed_at.map(|dt| dt.to_rfc3339());
    metadata.execution_time_ms = row.execution_time_ms.map(i64::from);

    metadata
}
