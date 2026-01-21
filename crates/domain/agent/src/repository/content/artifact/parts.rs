use crate::models::a2a::{DataPart, FilePart, FileWithBytes, Part, TextPart};
use crate::models::ArtifactPartRow;
use sqlx::PgPool;
use systemprompt_traits::RepositoryError;

pub async fn get_artifact_parts(
    pool: &PgPool,
    artifact_id: &str,
    context_id: &str,
) -> Result<Vec<Part>, RepositoryError> {
    let part_rows = sqlx::query_as!(
        ArtifactPartRow,
        r#"SELECT
            id as "id!",
            artifact_id as "artifact_id!",
            context_id as "context_id!",
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
        artifact_id,
        context_id
    )
    .fetch_all(pool)
    .await
    .map_err(|e| RepositoryError::Database(e.to_string()))?;

    let mut parts = Vec::new();

    for row in part_rows {
        let part = match row.part_kind.as_str() {
            "text" => {
                let text = row
                    .text_content
                    .ok_or_else(|| RepositoryError::InvalidData("Missing text_content".into()))?;
                Part::Text(TextPart { text })
            },
            "file" => {
                let bytes = row
                    .file_bytes
                    .ok_or_else(|| RepositoryError::InvalidData("Missing file_bytes".into()))?;
                Part::File(FilePart {
                    file: FileWithBytes {
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
    artifact_id: &str,
    context_id: &str,
    sequence_number: i32,
) -> Result<(), RepositoryError> {
    match part {
        Part::Text(text_part) => {
            sqlx::query!(
                r#"INSERT INTO artifact_parts (artifact_id, context_id, part_kind, sequence_number, text_content)
                VALUES ($1, $2, 'text', $3, $4)"#,
                artifact_id,
                context_id,
                sequence_number,
                text_part.text
            )
            .execute(pool)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;
        },
        Part::File(file_part) => {
            let file_uri: Option<&str> = None;
            sqlx::query!(
                r#"INSERT INTO artifact_parts (artifact_id, context_id, part_kind, sequence_number, file_name, file_mime_type, file_uri, file_bytes)
                VALUES ($1, $2, 'file', $3, $4, $5, $6, $7)"#,
                artifact_id,
                context_id,
                sequence_number,
                file_part.file.name,
                file_part.file.mime_type,
                file_uri,
                file_part.file.bytes
            )
            .execute(pool)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;
        },
        Part::Data(data_part) => {
            let data_json = serde_json::to_value(&data_part.data)
                .map_err(|e| RepositoryError::Serialization(e.to_string()))?;
            sqlx::query!(
                r#"INSERT INTO artifact_parts (artifact_id, context_id, part_kind, sequence_number, data_content)
                VALUES ($1, $2, 'data', $3, $4)"#,
                artifact_id,
                context_id,
                sequence_number,
                data_json
            )
            .execute(pool)
            .await
            .map_err(|e| RepositoryError::Database(e.to_string()))?;
        },
    }

    Ok(())
}
