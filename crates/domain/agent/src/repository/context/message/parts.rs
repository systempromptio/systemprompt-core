//! Message part-row persistence with jsonb-cast binding.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_identifiers::{MessageId, TaskId};
use systemprompt_traits::RepositoryError;

use crate::models::a2a::Part;

pub async fn get_message_parts(
    pool: &Arc<PgPool>,
    message_id: &MessageId,
) -> Result<Vec<Part>, RepositoryError> {
    let part_rows: Vec<crate::models::MessagePart> = sqlx::query_as!(
        crate::models::MessagePart,
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
        FROM message_parts WHERE message_id = $1 ORDER BY sequence_number ASC"#,
        message_id.as_str()
    )
    .fetch_all(pool.as_ref())
    .await
    .map_err(RepositoryError::database)?;

    let mut parts = Vec::new();

    for row in part_rows {
        let part = match row.part_kind.as_str() {
            "text" => {
                let text = row
                    .text_content
                    .ok_or_else(|| RepositoryError::InvalidData("Missing text_content".into()))?;
                Part::Text(crate::models::a2a::TextPart { text })
            },
            "file" => Part::File(crate::models::a2a::FilePart {
                file: crate::models::a2a::FileContent {
                    name: row.file_name,
                    mime_type: row.file_mime_type,
                    bytes: row.file_bytes,
                    url: row.file_uri,
                },
            }),
            "data" => {
                let data_value = row
                    .data_content
                    .ok_or_else(|| RepositoryError::InvalidData("Missing data_content".into()))?;
                let serde_json::Value::Object(data) = data_value else {
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

#[expect(
    missing_debug_implementations,
    reason = "params struct holds non-Debug references"
)]
pub struct PersistPartSqlxParams<'a> {
    pub tx: &'a mut sqlx::Transaction<'static, sqlx::Postgres>,
    pub part: &'a Part,
    pub message_id: &'a MessageId,
    pub task_id: &'a TaskId,
    pub sequence_number: i32,
}

pub(super) async fn persist_part_sqlx(
    params: PersistPartSqlxParams<'_>,
) -> Result<(), RepositoryError> {
    let PersistPartSqlxParams {
        tx,
        part,
        message_id,
        task_id,
        sequence_number,
    } = params;
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
            .map_err(RepositoryError::database)?;
        },
        Part::File(file_part) => {
            let file_id: Option<uuid::Uuid> = None;
            let file_uri: Option<String> = None;

            sqlx::query!(
                r#"INSERT INTO message_parts (message_id, task_id, part_kind, sequence_number, file_name, file_mime_type, file_uri, file_bytes, file_id)
                VALUES ($1, $2, 'file', $3, $4, $5, $6, $7, $8)"#,
                message_id.as_str(),
                task_id.as_str(),
                sequence_number,
                file_part.file.name,
                file_part.file.mime_type,
                file_uri,
                file_part.file.bytes.as_deref(),
                file_id
            )
            .execute(&mut **tx)
            .await
            .map_err(RepositoryError::database)?;
        },
        Part::Data(data_part) => {
            let data_json =
                serde_json::to_value(&data_part.data).map_err(RepositoryError::Serialization)?;
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
            .map_err(RepositoryError::database)?;
        },
    }

    Ok(())
}

pub(super) async fn persist_part_with_tx(
    tx: &mut dyn systemprompt_database::DatabaseTransaction,
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
                    &file_part.file.bytes.as_deref(),
                ],
            )
            .await?;
        },
        Part::Data(data_part) => {
            let data_json = serde_json::to_string(&data_part.data)?;
            // `data_content` is a jsonb column; cast the bound text payload
            // so Postgres accepts it without an OID mismatch.
            let query: &str = "INSERT INTO message_parts (message_id, task_id, part_kind, \
                               sequence_number, data_content) VALUES ($1, $2, 'data', $3, \
                               $4::jsonb)";
            tx.execute(
                &query,
                &[&message_id_str, &task_id_str, &sequence_number, &data_json],
            )
            .await?;
        },
    }

    Ok(())
}
