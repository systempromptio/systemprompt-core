use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use systemprompt_identifiers::{ContextId, MessageId, SessionId, TaskId, TraceId, UserId};
use systemprompt_traits::RepositoryError;

use crate::models::a2a::{Message, MessageRole, Part};
use crate::repository::task::constructor::batch_queries;


pub async fn get_messages_by_task(
    pool: &Arc<PgPool>,
    task_id: &TaskId,
) -> Result<Vec<Message>, RepositoryError> {
    let message_rows: Vec<crate::models::TaskMessage> = sqlx::query_as!(
        crate::models::TaskMessage,
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
        FROM task_messages WHERE task_id = $1 ORDER BY sequence_number ASC"#,
        task_id.as_str()
    )
    .fetch_all(pool.as_ref())
    .await
    .map_err(RepositoryError::database)?;

    let task_ids: Vec<String> = message_rows
        .iter()
        .map(|r| r.task_id.to_string())
        .collect();
    let all_parts = batch_queries::fetch_message_parts(pool, &task_ids).await?;
    let parts_by_message = group_parts_by_message(all_parts);

    let mut messages = Vec::new();

    for row in message_rows {
        let parts = parts_by_message
            .get(&row.message_id)
            .cloned()
            .unwrap_or_default();

        let reference_task_ids = row
            .reference_task_ids
            .map(|ids| ids.into_iter().map(TaskId::new).collect());

        let role = match row.role.as_str() {
            "user" | "ROLE_USER" => MessageRole::User,
            _ => MessageRole::Agent,
        };

        messages.push(Message {
            role,
            message_id: row.message_id,
            task_id: Some(row.task_id),
            context_id: row.context_id.unwrap_or_else(ContextId::empty),
            parts,
            metadata: row.metadata,
            extensions: None,
            reference_task_ids,
        });
    }

    Ok(messages)
}

pub async fn get_messages_by_context(
    pool: &Arc<PgPool>,
    context_id: &ContextId,
) -> Result<Vec<Message>, RepositoryError> {
    let message_rows: Vec<crate::models::TaskMessage> = sqlx::query_as!(
        crate::models::TaskMessage,
        r#"SELECT
            m.id as "id!",
            m.task_id as "task_id!: TaskId",
            m.message_id as "message_id!: MessageId",
            m.client_message_id,
            m.role as "role!",
            m.context_id as "context_id?: ContextId",
            m.user_id as "user_id?: UserId",
            m.session_id as "session_id?: SessionId",
            m.trace_id as "trace_id?: TraceId",
            m.sequence_number as "sequence_number!",
            m.created_at as "created_at!",
            m.updated_at as "updated_at!",
            m.metadata,
            m.reference_task_ids
        FROM task_messages m
        JOIN agent_tasks t ON m.task_id = t.task_id
        WHERE t.context_id = $1
        ORDER BY m.created_at ASC"#,
        context_id.as_str()
    )
    .fetch_all(pool.as_ref())
    .await
    .map_err(RepositoryError::database)?;

    let task_ids: Vec<String> = message_rows
        .iter()
        .map(|r| r.task_id.to_string())
        .collect();
    let all_parts = batch_queries::fetch_message_parts(pool, &task_ids).await?;
    let parts_by_message = group_parts_by_message(all_parts);

    let mut messages = Vec::new();

    for row in message_rows {
        let parts = parts_by_message
            .get(&row.message_id)
            .cloned()
            .unwrap_or_default();

        let role = match row.role.as_str() {
            "user" | "ROLE_USER" => MessageRole::User,
            _ => MessageRole::Agent,
        };

        messages.push(Message {
            role,
            message_id: row.message_id,
            task_id: Some(row.task_id),
            context_id: row.context_id.unwrap_or_else(|| context_id.clone()),
            parts,
            metadata: row.metadata,
            extensions: None,
            reference_task_ids: None,
        });
    }

    Ok(messages)
}

pub async fn get_next_sequence_number(
    pool: &Arc<PgPool>,
    task_id: &TaskId,
) -> Result<i32, RepositoryError> {
    let row = sqlx::query!(
        r#"SELECT MAX(sequence_number) as "max_seq" FROM task_messages WHERE task_id = $1"#,
        task_id.as_str()
    )
    .fetch_optional(pool.as_ref())
    .await
    .map_err(RepositoryError::database)?;

    Ok(row.and_then(|r| r.max_seq).map_or(0, |s| s + 1))
}

pub async fn get_next_sequence_number_sqlx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    task_id: &TaskId,
) -> Result<i32, RepositoryError> {
    let row = sqlx::query!(
        r#"SELECT MAX(sequence_number) as "max_seq" FROM task_messages WHERE task_id = $1"#,
        task_id.as_str()
    )
    .fetch_optional(&mut **tx)
    .await
    .map_err(RepositoryError::database)?;

    Ok(row.and_then(|r| r.max_seq).map_or(0, |s| s + 1))
}

pub async fn get_next_sequence_number_in_tx(
    tx: &mut dyn systemprompt_database::DatabaseTransaction,
    task_id: &TaskId,
) -> Result<i32, RepositoryError> {
    let query: &str =
        "SELECT MAX(sequence_number) as max_seq FROM task_messages WHERE task_id = $1";
    let task_id_str = task_id.as_str();
    let row = tx.fetch_optional(&query, &[&task_id_str]).await?;

    let max_seq = row.as_ref().and_then(|r| {
        r.get("max_seq")
            .and_then(serde_json::Value::as_i64)
            .map(|v| v as i32)
    });

    Ok(max_seq.map_or(0, |s| s + 1))
}

fn group_parts_by_message(all_parts: Vec<crate::models::MessagePart>) -> HashMap<MessageId, Vec<Part>> {
    use crate::models::a2a::{DataPart, FileContent, FilePart, TextPart};

    let mut map: HashMap<MessageId, Vec<Part>> = HashMap::new();
    for row in all_parts {
        let part = match row.part_kind.as_str() {
            "text" => row.text_content.map(|text| Part::Text(TextPart { text })),
            "file" => Some(Part::File(FilePart {
                file: FileContent {
                    name: row.file_name,
                    mime_type: row.file_mime_type,
                    bytes: row.file_bytes,
                    url: row.file_uri,
                },
            })),
            "data" => row.data_content.and_then(|v| match v {
                serde_json::Value::Object(data) => Some(Part::Data(DataPart { data })),
                _ => None,
            }),
            _ => None,
        };
        if let Some(part) = part {
            map.entry(row.message_id).or_default().push(part);
        }
    }
    map
}
