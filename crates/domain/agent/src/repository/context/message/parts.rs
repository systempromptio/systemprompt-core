use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_core_files::{FileUploadRequest, FileUploadService};
use systemprompt_identifiers::{ContextId, MessageId, SessionId, TaskId, TraceId, UserId};
use systemprompt_traits::RepositoryError;

use crate::models::a2a::Part;

#[derive(Debug, Clone)]
pub struct FileUploadContext<'a> {
    pub upload_service: &'a FileUploadService,
    pub context_id: &'a ContextId,
    pub user_id: Option<&'a UserId>,
    pub session_id: Option<&'a SessionId>,
    pub trace_id: Option<&'a TraceId>,
}

pub async fn get_message_parts(
    pool: &Arc<PgPool>,
    message_id: &MessageId,
) -> Result<Vec<Part>, RepositoryError> {
    let part_rows: Vec<crate::models::MessagePart> = sqlx::query_as!(
        crate::models::MessagePart,
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
        FROM message_parts WHERE message_id = $1 ORDER BY sequence_number ASC"#,
        message_id.as_str()
    )
    .fetch_all(pool.as_ref())
    .await
    .map_err(|e| RepositoryError::Database(e.to_string()))?;

    let mut parts = Vec::new();

    for row in part_rows {
        let part = match row.part_kind.as_str() {
            "text" => {
                let text = row
                    .text_content
                    .ok_or_else(|| RepositoryError::InvalidData("Missing text_content".into()))?;
                Part::Text(crate::models::a2a::TextPart { text })
            },
            "file" => {
                let bytes = row
                    .file_bytes
                    .ok_or_else(|| RepositoryError::InvalidData("Missing file_bytes".into()))?;
                Part::File(crate::models::a2a::FilePart {
                    file: crate::models::a2a::FileWithBytes {
                        name: row.file_name,
                        mime_type: row.file_mime_type,
                        bytes,
                    },
                })
            },
            "data" => {
                let data_value = row
                    .data_content
                    .ok_or_else(|| RepositoryError::InvalidData("Missing data_content".into()))?;
                let data = if let serde_json::Value::Object(map) = data_value {
                    map
                } else {
                    return Err(RepositoryError::InvalidData(
                        "Data content must be a JSON object".into(),
                    ));
                };
                Part::Data(crate::models::a2a::DataPart { data })
            },
            _ => {
                return Err(RepositoryError::InvalidData(format!(
                    "Unknown part kind: {}",
                    row.part_kind
                )));
            },
        };

        parts.push(part);
    }

    Ok(parts)
}

pub async fn persist_part_sqlx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    part: &Part,
    message_id: &MessageId,
    task_id: &TaskId,
    sequence_number: i32,
    upload_ctx: Option<&FileUploadContext<'_>>,
) -> Result<(), RepositoryError> {
    match part {
        Part::Text(text_part) => {
            sqlx::query!(
                r#"INSERT INTO message_parts (message_id, task_id, part_kind, sequence_number, text_content)
                VALUES ($1, $2, 'text', $3, $4)"#,
                message_id.as_str(),
                task_id.as_str(),
                sequence_number,
                text_part.text
            )
            .execute(&mut **tx)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;
        },
        Part::File(file_part) => {
            let upload_result = try_upload_file(file_part, upload_ctx).await;

            let (file_id, file_uri) = match upload_result {
                Some((id, uri)) => (Some(id), Some(uri)),
                None => (None, None),
            };

            sqlx::query!(
                r#"INSERT INTO message_parts (message_id, task_id, part_kind, sequence_number, file_name, file_mime_type, file_uri, file_bytes, file_id)
                VALUES ($1, $2, 'file', $3, $4, $5, $6, $7, $8)"#,
                message_id.as_str(),
                task_id.as_str(),
                sequence_number,
                file_part.file.name,
                file_part.file.mime_type,
                file_uri,
                file_part.file.bytes,
                file_id
            )
            .execute(&mut **tx)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;
        },
        Part::Data(data_part) => {
            let data_json = serde_json::to_value(&data_part.data)
                .map_err(|e| RepositoryError::Serialization(e.to_string()))?;
            sqlx::query!(
                r#"INSERT INTO message_parts (message_id, task_id, part_kind, sequence_number, data_content)
                VALUES ($1, $2, 'data', $3, $4)"#,
                message_id.as_str(),
                task_id.as_str(),
                sequence_number,
                data_json
            )
            .execute(&mut **tx)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;
        },
    }

    Ok(())
}

async fn try_upload_file(
    file_part: &crate::models::a2a::FilePart,
    upload_ctx: Option<&FileUploadContext<'_>>,
) -> Option<(uuid::Uuid, String)> {
    let ctx = upload_ctx?;

    if !ctx.upload_service.is_enabled() {
        return None;
    }

    let mime_type = file_part
        .file
        .mime_type
        .as_deref()
        .unwrap_or("application/octet-stream");

    let mut builder =
        FileUploadRequest::builder(mime_type, &file_part.file.bytes, ctx.context_id.clone());

    if let Some(name) = &file_part.file.name {
        builder = builder.with_name(name);
    }

    if let Some(user_id) = ctx.user_id {
        builder = builder.with_user_id(user_id.clone());
    }

    if let Some(session_id) = ctx.session_id {
        builder = builder.with_session_id(session_id.clone());
    }

    if let Some(trace_id) = ctx.trace_id {
        builder = builder.with_trace_id(trace_id.clone());
    }

    let request = builder.build();

    match ctx.upload_service.upload_file(request).await {
        Ok(uploaded) => {
            let file_uuid = uuid::Uuid::parse_str(uploaded.file_id.as_str()).ok()?;
            Some((file_uuid, uploaded.public_url))
        },
        Err(e) => {
            tracing::warn!(error = %e, "File upload failed, continuing with base64 only");
            None
        },
    }
}

pub async fn persist_part_with_tx(
    tx: &mut dyn systemprompt_core_database::DatabaseTransaction,
    part: &Part,
    message_id: &MessageId,
    task_id: &TaskId,
    sequence_number: i32,
) -> Result<(), RepositoryError> {
    let message_id_str = message_id.as_str();
    let task_id_str = task_id.as_str();
    match part {
        Part::Text(text_part) => {
            let query: &str = "INSERT INTO message_parts (message_id, task_id, part_kind, \
                               sequence_number, text_content) VALUES ($1, $2, 'text', $3, $4)";
            tx.execute(
                &query,
                &[
                    &message_id_str,
                    &task_id_str,
                    &sequence_number,
                    &text_part.text,
                ],
            )
            .await?;
        },
        Part::File(file_part) => {
            let uri_opt: Option<&str> = None;
            let query: &str = "INSERT INTO message_parts (message_id, task_id, part_kind, \
                               sequence_number, file_name, file_mime_type, file_uri, file_bytes) \
                               VALUES ($1, $2, 'file', $3, $4, $5, $6, $7)";
            tx.execute(
                &query,
                &[
                    &message_id_str,
                    &task_id_str,
                    &sequence_number,
                    &file_part.file.name,
                    &file_part.file.mime_type,
                    &uri_opt,
                    &file_part.file.bytes,
                ],
            )
            .await?;
        },
        Part::Data(data_part) => {
            let data_json = serde_json::to_string(&data_part.data)?;
            let query: &str = "INSERT INTO message_parts (message_id, task_id, part_kind, \
                               sequence_number, data_content) VALUES ($1, $2, 'data', $3, $4)";
            tx.execute(
                &query,
                &[&message_id_str, &task_id_str, &sequence_number, &data_json],
            )
            .await?;
        },
    }

    Ok(())
}
