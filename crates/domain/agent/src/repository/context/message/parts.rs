//! Message part-row persistence.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_identifiers::{MessageId, TaskId};
use systemprompt_traits::RepositoryError;

use crate::models::a2a::Part;
use crate::repository::parts::parts_from_rows;

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

    parts_from_rows(&part_rows)
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
            sqlx::query!(
                r#"INSERT INTO message_parts (message_id, task_id, part_kind, sequence_number, file_name, file_mime_type, file_uri, file_bytes)
                VALUES ($1, $2, 'file', $3, $4, $5, $6, $7)"#,
                message_id.as_str(),
                task_id.as_str(),
                sequence_number,
                file_part.file.name,
                file_part.file.mime_type,
                file_part.file.url.as_deref(),
                file_part.file.bytes.as_deref()
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
