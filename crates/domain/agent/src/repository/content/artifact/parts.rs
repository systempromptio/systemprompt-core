//! Artifact part-row persistence.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::models::ArtifactPartRow;
use crate::models::a2a::Part;
use crate::repository::parts::{parts_from_rows, row_values};
use sqlx::PgPool;
use systemprompt_identifiers::{ArtifactId, ContextId};
use systemprompt_traits::RepositoryError;

pub async fn get_artifact_parts(
    pool: &PgPool,
    artifact_id: &ArtifactId,
    context_id: &ContextId,
) -> Result<Vec<Part>, RepositoryError> {
    let part_rows = sqlx::query_as!(
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
        FROM artifact_parts
        WHERE artifact_id = $1 AND context_id = $2
        ORDER BY sequence_number ASC"#,
        artifact_id as _,
        context_id as _
    )
    .fetch_all(pool)
    .await?;

    parts_from_rows(&part_rows)
}

pub async fn persist_artifact_part(
    pool: &PgPool,
    part: &Part,
    artifact_id: &ArtifactId,
    context_id: &ContextId,
    sequence_number: i32,
) -> Result<(), RepositoryError> {
    let values = row_values(part);
    sqlx::query!(
        r#"INSERT INTO artifact_parts (artifact_id, context_id, part_kind, sequence_number, text_content, file_name, file_mime_type, file_uri, file_bytes, data_content, metadata)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, COALESCE($11, '{}'::jsonb))"#,
        artifact_id as _,
        context_id as _,
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
    .execute(pool)
    .await?;

    Ok(())
}
