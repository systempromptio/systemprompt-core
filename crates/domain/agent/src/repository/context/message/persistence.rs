use systemprompt_identifiers::{ContextId, SessionId, TaskId, TraceId, UserId};
use systemprompt_traits::RepositoryError;

use crate::models::a2a::Message;

use super::parts::{persist_part_sqlx, persist_part_with_tx};

pub async fn persist_message_sqlx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    message: &Message,
    task_id: &TaskId,
    context_id: &ContextId,
    sequence_number: i32,
    user_id: Option<&UserId>,
    session_id: &SessionId,
    trace_id: &TraceId,
) -> Result<(), RepositoryError> {
    let metadata_json = serde_json::to_value(&message.metadata)
        .map_err(|e| RepositoryError::Serialization(e.to_string()))?;

    sqlx::query!(
        "DELETE FROM message_parts WHERE message_id = $1",
        message.id.as_str()
    )
    .execute(&mut **tx)
    .await
    .map_err(|e| RepositoryError::Database(e.to_string()))?;

    sqlx::query!(
        "DELETE FROM task_messages WHERE message_id = $1",
        message.id.as_str()
    )
    .execute(&mut **tx)
    .await
    .map_err(|e| RepositoryError::Database(e.to_string()))?;

    let client_message_id = message
        .metadata
        .as_ref()
        .and_then(|m| m.get("clientMessageId"))
        .and_then(|v| v.as_str());

    let reference_task_ids: Option<Vec<String>> = message
        .reference_task_ids
        .as_ref()
        .map(|ids| ids.iter().map(|id| id.to_string()).collect());

    sqlx::query!(
        r#"INSERT INTO task_messages (task_id, message_id, client_message_id, role, context_id,
        user_id, session_id, trace_id, sequence_number, metadata, reference_task_ids)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)"#,
        task_id.as_str(),
        message.id.as_str(),
        client_message_id,
        message.role,
        context_id.as_str(),
        user_id.map(UserId::as_str),
        session_id.as_str(),
        trace_id.as_str(),
        sequence_number,
        metadata_json,
        reference_task_ids.as_deref()
    )
    .execute(&mut **tx)
    .await
    .map_err(|e| RepositoryError::Database(e.to_string()))?;

    for (idx, part) in message.parts.iter().enumerate() {
        persist_part_sqlx(tx, part, &message.id, task_id, idx as i32).await?;
    }

    Ok(())
}

pub async fn persist_message_with_tx(
    tx: &mut dyn systemprompt_core_database::DatabaseTransaction,
    message: &Message,
    task_id: &TaskId,
    context_id: &ContextId,
    sequence_number: i32,
    user_id: Option<&UserId>,
    session_id: &SessionId,
    trace_id: &TraceId,
) -> Result<(), RepositoryError> {
    let metadata_json = serde_json::to_string(&message.metadata)?;

    let delete_parts_query: &str = "DELETE FROM message_parts WHERE message_id = $1";
    tx.execute(&delete_parts_query, &[&message.id.as_str()])
        .await?;

    let delete_messages_query: &str = "DELETE FROM task_messages WHERE message_id = $1";
    tx.execute(&delete_messages_query, &[&message.id.as_str()])
        .await?;

    let client_message_id = message
        .metadata
        .as_ref()
        .and_then(|m| m.get("clientMessageId"))
        .and_then(|v| v.as_str());

    let reference_task_ids = message
        .reference_task_ids
        .as_ref()
        .map(|ids| ids.iter().map(|id| id.to_string()).collect::<Vec<String>>());

    let task_id_str = task_id.as_str();
    let context_id_str = context_id.as_str();
    let user_id_str = user_id.map(UserId::as_str);
    let session_id_str = session_id.as_str();
    let trace_id_str = trace_id.as_str();

    let insert_query: &str = "INSERT INTO task_messages (task_id, message_id, client_message_id, \
                              role, context_id, user_id, session_id, trace_id, sequence_number, \
                              metadata, reference_task_ids) VALUES ($1, $2, $3, $4, $5, $6, $7, \
                              $8, $9, $10, $11)";
    tx.execute(
        &insert_query,
        &[
            &task_id_str,
            &message.id.as_str(),
            &client_message_id,
            &message.role,
            &context_id_str,
            &user_id_str,
            &session_id_str,
            &trace_id_str,
            &sequence_number,
            &metadata_json,
            &reference_task_ids,
        ],
    )
    .await?;

    for (idx, part) in message.parts.iter().enumerate() {
        persist_part_with_tx(tx, part, &message.id, task_id, idx as i32).await?;
    }

    Ok(())
}
