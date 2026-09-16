//! Single-task construction from rows with message/part loading.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::models::a2a::{Artifact, Message, Part, Task, TaskStatus};
use crate::models::{MessagePart, TaskMessage, TaskRow};
use crate::repository::parts::parts_from_rows;
use systemprompt_identifiers::{
    AgentName, ContextId, MessageId, SessionId, TaskId, TraceId, UserId,
};
use systemprompt_models::ExecutionStep;
use systemprompt_traits::RepositoryError;

use super::{TaskConstructor, converters};

pub(super) async fn construct_task_from_task_id(
    constructor: &TaskConstructor,
    task_id: &TaskId,
) -> Result<Option<Task>, RepositoryError> {
    match fetch_task_row(constructor, task_id).await? {
        Some(row) => Ok(Some(construct_task_from_row(constructor, &row).await?)),
        None => Ok(None),
    }
}

async fn fetch_task_row(
    constructor: &TaskConstructor,
    task_id: &TaskId,
) -> Result<Option<TaskRow>, RepositoryError> {
    let pool = constructor.pool();
    let task_id_str = task_id.as_str();

    sqlx::query_as!(
        TaskRow,
        r#"SELECT
            task_id as "task_id!: TaskId",
            context_id as "context_id!: ContextId",
            status as "status!",
            status_timestamp,
            user_id as "user_id?: UserId",
            session_id as "session_id?: SessionId",
            trace_id as "trace_id?: TraceId",
            agent_name as "agent_name?: AgentName",
            started_at,
            completed_at,
            execution_time_ms,
            error_message,
            metadata,
            created_at as "created_at!",
            updated_at as "updated_at!"
        FROM agent_tasks WHERE task_id = $1"#,
        task_id_str
    )
    .fetch_optional(pool.as_ref())
    .await
    .map_err(RepositoryError::database)
}

async fn construct_task_from_row(
    constructor: &TaskConstructor,
    row: &TaskRow,
) -> Result<Task, RepositoryError> {
    let task_id = row.task_id.clone();

    let history = load_task_messages(constructor, &task_id).await?;
    let artifacts = load_task_artifacts(constructor, &task_id).await?;
    let execution_steps = load_execution_steps(constructor, &task_id).await?;

    let mut metadata = converters::construct_metadata(row);
    if let Some(steps) = execution_steps {
        metadata.execution_steps = Some(steps);
    }

    let task_state = converters::parse_task_state(row)?;

    Ok(Task {
        id: task_id,
        context_id: row.context_id.clone(),
        status: TaskStatus {
            state: task_state,
            message: None,
            timestamp: row.status_timestamp,
        },
        history,
        artifacts,
        metadata: Some(metadata),
        created_at: Some(row.created_at),
        last_modified: Some(row.updated_at),
    })
}

async fn load_task_messages(
    constructor: &TaskConstructor,
    task_id: &TaskId,
) -> Result<Option<Vec<Message>>, RepositoryError> {
    let pool = constructor.pool();
    let task_id_str = task_id.as_str();

    let message_rows: Vec<TaskMessage> = sqlx::query_as!(
        TaskMessage,
        r#"SELECT
            id as "id!",
            task_id as "task_id!: TaskId",
            message_id as "message_id!: MessageId",
            client_message_id,
            role as "role!",
            context_id as "context_id!: ContextId",
            user_id as "user_id?: UserId",
            session_id as "session_id?: SessionId",
            trace_id as "trace_id?: TraceId",
            sequence_number as "sequence_number!",
            created_at as "created_at!",
            updated_at as "updated_at!",
            metadata,
            reference_task_ids
        FROM task_messages WHERE task_id = $1 ORDER BY sequence_number ASC"#,
        task_id_str
    )
    .fetch_all(pool.as_ref())
    .await
    .map_err(RepositoryError::database)?;

    if message_rows.is_empty() {
        return Ok(None);
    }

    let mut messages = Vec::new();
    for msg_row in message_rows {
        let parts = load_message_parts(constructor, &msg_row.message_id, task_id).await?;
        messages.push(converters::message_from_row(msg_row, parts));
    }

    Ok(Some(messages))
}

async fn load_message_parts(
    constructor: &TaskConstructor,
    message_id: &MessageId,
    task_id: &TaskId,
) -> Result<Vec<Part>, RepositoryError> {
    let pool = constructor.pool();
    let task_id_str = task_id.as_str();
    let message_id_str = message_id.as_str();

    let part_rows: Vec<MessagePart> = sqlx::query_as!(
        MessagePart,
        r#"SELECT
            id as "id!",
            message_id as "message_id!",
            task_id as "task_id!",
            part_kind as "part_kind!",
            sequence_number as "sequence_number!",
            text_content,
            file_name,
            file_mime_type,
            file_uri,
            file_bytes,
            data_content,
            metadata
        FROM message_parts WHERE message_id = $1 AND task_id = $2 ORDER BY sequence_number ASC"#,
        message_id_str,
        task_id_str
    )
    .fetch_all(pool.as_ref())
    .await
    .map_err(RepositoryError::database)?;

    parts_from_rows(&part_rows)
}

async fn load_task_artifacts(
    constructor: &TaskConstructor,
    task_id: &TaskId,
) -> Result<Option<Vec<Artifact>>, RepositoryError> {
    let artifacts = constructor
        .artifact_repo()
        .get_artifacts_by_task(task_id)
        .await
        .map_err(|e| RepositoryError::InvalidData(e.to_string()))?;

    if artifacts.is_empty() {
        Ok(None)
    } else {
        Ok(Some(artifacts))
    }
}

async fn load_execution_steps(
    constructor: &TaskConstructor,
    task_id: &TaskId,
) -> Result<Option<Vec<ExecutionStep>>, RepositoryError> {
    let steps = constructor
        .execution_step_repo()
        .list_by_task(task_id)
        .await
        .map_err(|e| RepositoryError::Internal(e.to_string()))?;

    if steps.is_empty() {
        Ok(None)
    } else {
        Ok(Some(steps))
    }
}
