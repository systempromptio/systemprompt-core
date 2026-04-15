use crate::models::ArtifactPartRow;
use crate::models::a2a::{DataPart, FileContent, FilePart, Part, TextPart};
use sqlx::PgPool;
use systemprompt_identifiers::{ArtifactId, ContextId};
use systemprompt_traits::RepositoryError;

pub async fn get_artifact_parts(
    pool: &PgPool,
    artifact_id: &ArtifactId,
    context_id: &ContextId,
) -> Result<Vec<Part>, RepositoryError> {
    let artifact_id_str = artifact_id.as_str();
    let context_id_str = context_id.as_str();
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
        artifact_id_str,
        context_id_str
    )
    .fetch_all(pool)
    .await
    .map_err(RepositoryError::database)?;

    let mut parts = Vec::new();

    for row in part_rows {
        let part = match row.part_kind.as_str() {
            "text" => {
                let text = row
                    .text_content
                    .ok_or_else(|| RepositoryError::InvalidData("Missing text_content".into()))?;
                Part::Text(TextPart { text })
            },
            "file" => Part::File(FilePart {
                file: FileContent {
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
                Part::Data(DataPart { data })
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

pub async fn persist_artifact_part(
    pool: &PgPool,
    part: &Part,
    artifact_id: &ArtifactId,
    context_id: &ContextId,
    sequence_number: i32,
) -> Result<(), RepositoryError> {
    let artifact_id_str = artifact_id.as_str();
    let context_id_str = context_id.as_str();
    match part {
        Part::Text(text_part) => {
            sqlx::query!(
                r#"INSERT INTO artifact_parts (artifact_id, context_id, part_kind, sequence_number, text_content)
                VALUES ($1, $2, 'text', $3, $4)"#,
                artifact_id_str,
                context_id_str,
                sequence_number,
                text_part.text
            )
            .execute(pool)
            .await
            .map_err(RepositoryError::database)?;
        },
        Part::File(file_part) => {
            let file_uri: Option<&str> = None;
            sqlx::query!(
                r#"INSERT INTO artifact_parts (artifact_id, context_id, part_kind, sequence_number, file_name, file_mime_type, file_uri, file_bytes)
                VALUES ($1, $2, 'file', $3, $4, $5, $6, $7)"#,
                artifact_id_str,
                context_id_str,
                sequence_number,
                file_part.file.name,
                file_part.file.mime_type,
                file_uri,
                file_part.file.bytes.as_deref()
            )
            .execute(pool)
            .await
            .map_err(RepositoryError::database)?;
        },
        Part::Data(data_part) => {
            let data_json =
                serde_json::to_value(&data_part.data).map_err(RepositoryError::Serialization)?;
            sqlx::query!(
                r#"INSERT INTO artifact_parts (artifact_id, context_id, part_kind, sequence_number, data_content)
                VALUES ($1, $2, 'data', $3, $4)"#,
                artifact_id_str,
                context_id_str,
                sequence_number,
                data_json
            )
            .execute(pool)
            .await
            .map_err(RepositoryError::database)?;
        },
    }

    Ok(())
}
