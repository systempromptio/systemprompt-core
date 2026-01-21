use crate::models::{
    ArtifactPartRow, ArtifactRow, ExecutionStepBatchRow, MessagePart, TaskMessage, TaskRow,
};
use std::sync::Arc;
use systemprompt_identifiers::{
    AgentName, ArtifactId, ContextId, ExecutionStepId, McpExecutionId, MessageId, SessionId,
    SkillId, TaskId, TraceId, UserId,
};
use systemprompt_traits::RepositoryError;

pub async fn fetch_task_rows(
    pool: &Arc<sqlx::PgPool>,
    task_ids: &[String],
) -> Result<Vec<TaskRow>, RepositoryError> {
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
        FROM agent_tasks WHERE task_id = ANY($1)"#,
        task_ids
    )
    .fetch_all(pool.as_ref())
    .await
    .map_err(|e| RepositoryError::database(e))
}

pub async fn fetch_messages(
    pool: &Arc<sqlx::PgPool>,
    task_ids: &[String],
) -> Result<Vec<TaskMessage>, RepositoryError> {
    sqlx::query_as!(
        TaskMessage,
        r#"SELECT
            id as "id!",
            task_id as "task_id!: TaskId",
            message_id as "message_id!: MessageId",
            client_message_id,
            role as "role!",
            context_id as "context_id?: ContextId",
            user_id as "user_id?: UserId",
            session_id as "session_id?: SessionId",
            trace_id as "trace_id?: TraceId",
            sequence_number as "sequence_number!",
            created_at as "created_at!",
            updated_at as "updated_at!",
            metadata,
            reference_task_ids
        FROM task_messages WHERE task_id = ANY($1) ORDER BY task_id, sequence_number ASC"#,
        task_ids
    )
    .fetch_all(pool.as_ref())
    .await
    .map_err(|e| RepositoryError::database(e))
}

pub async fn fetch_message_parts(
    pool: &Arc<sqlx::PgPool>,
    task_ids: &[String],
) -> Result<Vec<MessagePart>, RepositoryError> {
    sqlx::query_as!(
        MessagePart,
        r#"SELECT
            id as "id!",
            message_id as "message_id!: MessageId",
            task_id as "task_id!: TaskId",
            part_kind as "part_kind!",
            sequence_number as "sequence_number!",
            text_content,
            file_name,
            file_mime_type,
            file_uri,
            file_bytes,
            data_content,
            metadata
        FROM message_parts WHERE task_id = ANY($1) ORDER BY message_id, sequence_number ASC"#,
        task_ids
    )
    .fetch_all(pool.as_ref())
    .await
    .map_err(|e| RepositoryError::database(e))
}

pub async fn fetch_artifacts(
    pool: &Arc<sqlx::PgPool>,
    task_ids: &[String],
) -> Result<Vec<ArtifactRow>, RepositoryError> {
    sqlx::query_as!(
        ArtifactRow,
        r#"SELECT
            artifact_id as "artifact_id!: ArtifactId",
            task_id as "task_id!: TaskId",
            context_id as "context_id?: ContextId",
            name,
            description,
            artifact_type as "artifact_type!",
            source,
            tool_name,
            mcp_execution_id as "mcp_execution_id?: McpExecutionId",
            fingerprint,
            skill_id as "skill_id?: SkillId",
            skill_name,
            metadata,
            created_at as "created_at!"
        FROM task_artifacts WHERE task_id = ANY($1) ORDER BY task_id, created_at ASC"#,
        task_ids
    )
    .fetch_all(pool.as_ref())
    .await
    .map_err(|e| RepositoryError::database(e))
}

pub async fn fetch_artifact_parts(
    pool: &Arc<sqlx::PgPool>,
    artifact_ids: &[String],
) -> Result<Vec<ArtifactPartRow>, RepositoryError> {
    if artifact_ids.is_empty() {
        return Ok(Vec::new());
    }

    sqlx::query_as!(
        ArtifactPartRow,
        r#"SELECT
            id as "id!",
            artifact_id as "artifact_id!: ArtifactId",
            context_id as "context_id!: ContextId",
            part_kind as "part_kind!",
            sequence_number as "sequence_number!",
            text_content,
            file_name,
            file_mime_type,
            file_uri,
            file_bytes,
            data_content,
            metadata
        FROM artifact_parts WHERE artifact_id = ANY($1) ORDER BY artifact_id, sequence_number ASC"#,
        artifact_ids
    )
    .fetch_all(pool.as_ref())
    .await
    .map_err(|e| RepositoryError::database(e))
}

pub async fn fetch_execution_steps(
    pool: &Arc<sqlx::PgPool>,
    task_ids: &[String],
) -> Result<Vec<ExecutionStepBatchRow>, RepositoryError> {
    if task_ids.is_empty() {
        return Ok(Vec::new());
    }

    sqlx::query_as!(
        ExecutionStepBatchRow,
        r#"SELECT
            step_id as "step_id!: ExecutionStepId",
            task_id as "task_id!: TaskId",
            status as "status!",
            content as "content!",
            started_at as "started_at!",
            completed_at,
            duration_ms,
            error_message
        FROM task_execution_steps WHERE task_id = ANY($1) ORDER BY task_id, started_at ASC"#,
        task_ids
    )
    .fetch_all(pool.as_ref())
    .await
    .map_err(|e| RepositoryError::database(e))
}
