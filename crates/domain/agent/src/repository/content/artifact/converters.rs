use crate::models::ArtifactRow;
use crate::models::a2a::{
    Artifact, ArtifactMetadata, DataPart, FileContent, FilePart, Part, TextPart,
};
use crate::repository::task::constructor::batch_queries;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use systemprompt_identifiers::{ArtifactId, ContextId};
use systemprompt_traits::RepositoryError;

pub async fn rows_to_artifacts_batch(
    pool: &Arc<PgPool>,
    rows: Vec<ArtifactRow>,
) -> Result<Vec<Artifact>, RepositoryError> {
    if rows.is_empty() {
        return Ok(Vec::new());
    }

    let artifact_ids: Vec<String> = rows.iter().map(|r| r.artifact_id.to_string()).collect();
    let all_parts = batch_queries::fetch_artifact_parts(pool, &artifact_ids).await?;

    let parts_by_artifact: HashMap<ArtifactId, Vec<Part>> = {
        let mut map: HashMap<ArtifactId, Vec<Part>> = HashMap::new();
        for part_row in all_parts {
            let part = convert_artifact_part_row(part_row.part_kind.as_str(), &part_row)?;
            map.entry(part_row.artifact_id).or_default().push(part);
        }
        map
    };

    let mut artifacts = Vec::new();
    for row in rows {
        let parts = parts_by_artifact
            .get(&row.artifact_id)
            .cloned()
            .unwrap_or_default();
        artifacts.push(row_to_artifact_with_parts(row, parts));
    }

    Ok(artifacts)
}

fn convert_artifact_part_row(
    part_kind: &str,
    row: &crate::models::ArtifactPartRow,
) -> Result<Part, RepositoryError> {
    match part_kind {
        "text" => {
            let text = row
                .text_content
                .clone()
                .ok_or_else(|| RepositoryError::InvalidData("Missing text_content".into()))?;
            Ok(Part::Text(TextPart { text }))
        },
        "file" => Ok(Part::File(FilePart {
            file: FileContent {
                name: row.file_name.clone(),
                mime_type: row.file_mime_type.clone(),
                bytes: row.file_bytes.clone(),
                url: row.file_uri.clone(),
            },
        })),
        "data" => {
            let data_value = row
                .data_content
                .clone()
                .ok_or_else(|| RepositoryError::InvalidData("Missing data_content".into()))?;
            let serde_json::Value::Object(data) = data_value else {
                return Err(RepositoryError::InvalidData(
                    "Data content must be a JSON object".into(),
                ));
            };
            Ok(Part::Data(DataPart { data }))
        },
        _ => Err(RepositoryError::InvalidData(format!(
            "Unknown part kind: {part_kind}"
        ))),
    }
}

pub fn row_to_artifact_with_parts(row: ArtifactRow, parts: Vec<Part>) -> Artifact {
    let context_id = row.context_id.clone().unwrap_or_else(|| ContextId::new(""));
    let (rendering_hints, mcp_schema, is_internal, execution_index) =
        extract_metadata_fields(row.metadata.as_ref());

    Artifact {
        id: row.artifact_id,
        title: row.name,
        description: row.description,
        parts,
        extensions: vec![],
        metadata: ArtifactMetadata {
            artifact_type: row.artifact_type,
            context_id,
            created_at: row.created_at.to_rfc3339(),
            task_id: row.task_id,
            rendering_hints,
            source: row.source,
            mcp_execution_id: row.mcp_execution_id.map(|id| id.as_str().to_string()),
            mcp_schema,
            is_internal,
            fingerprint: row.fingerprint,
            tool_name: row.tool_name,
            execution_index,
            skill_id: row.skill_id,
            skill_name: row.skill_name,
        },
    }
}

fn extract_metadata_fields(
    metadata: Option<&serde_json::Value>,
) -> (
    Option<serde_json::Value>,
    Option<serde_json::Value>,
    Option<bool>,
    Option<usize>,
) {
    let Some(metadata) = metadata else {
        return (None, None, None, None);
    };

    let rendering_hints = metadata
        .get("rendering_hints")
        .and_then(|v| if v.is_null() { None } else { Some(v.clone()) });

    let mcp_schema = metadata
        .get("mcp_schema")
        .and_then(|v| if v.is_null() { None } else { Some(v.clone()) });

    let is_internal = metadata
        .get("is_internal")
        .and_then(serde_json::Value::as_bool);

    let execution_index = metadata
        .get("execution_index")
        .and_then(serde_json::Value::as_u64)
        .map(|v| v as usize);

    (rendering_hints, mcp_schema, is_internal, execution_index)
}
