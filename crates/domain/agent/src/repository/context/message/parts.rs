//! Message part-row persistence.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{MessageId, TaskId};
use systemprompt_traits::RepositoryError;

use crate::models::a2a::Part;
use crate::repository::parts::row_values;

#[derive(Debug)]
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
    let values = row_values(part);
    sqlx::query!(
        r#"INSERT INTO message_parts (message_id, task_id, part_kind, sequence_number, text_content, file_name, file_mime_type, file_uri, file_bytes, data_content, metadata)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, COALESCE($11, '{}'::jsonb))"#,
        message_id.as_str(),
        task_id.as_str(),
        values.part_kind,
        sequence_number,
        values.text_content,
        values.file_name,
        values.file_mime_type,
        values.file_uri,
        values.file_bytes,
        values.data_content,
        values.metadata
    )
    .execute(&mut **tx)
    .await?;

    Ok(())
}
