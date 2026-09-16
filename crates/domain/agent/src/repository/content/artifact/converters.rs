//! Row-to-model converters for artifacts and their metadata.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::models::ArtifactRow;
use crate::models::a2a::{Artifact, ArtifactMetadata, Part};
use crate::repository::parts::part_from_row;
use crate::repository::task::constructor::batch_queries;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use systemprompt_identifiers::ArtifactId;
use systemprompt_traits::RepositoryError;

pub(super) async fn rows_to_artifacts_batch(
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
            let part = part_from_row(&part_row)?;
            map.entry(part_row.artifact_id).or_default().push(part);
        }
        map
    };

    let mut artifacts = Vec::new();
    for row in rows {
        let parts = parts_by_artifact
            .get(&row.artifact_id)
            .map_or_else(Vec::new, Clone::clone);
        artifacts.push(artifact_from_row(row, parts));
    }

    Ok(artifacts)
}

pub(crate) fn artifact_from_row(row: ArtifactRow, parts: Vec<Part>) -> Artifact {
    let context_id = row.context_id.clone();
    let StoredArtifactMetadata {
        rendering_hints,
        mcp_schema,
        is_internal,
        execution_index,
        artifact_extensions,
    } = row
        .metadata
        .as_ref()
        .map(stored_metadata)
        .unwrap_or_default();

    Artifact {
        id: row.artifact_id,
        title: row.name,
        description: row.description,
        parts,
        extensions: artifact_extensions,
        metadata: ArtifactMetadata {
            artifact_type: row.artifact_type,
            context_id,
            created_at: row.created_at.to_rfc3339(),
            task_id: row.task_id,
            rendering_hints,
            source: row.source,
            mcp_execution_id: row.mcp_execution_id.map(|id| id.as_str().to_owned()),
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

#[derive(Default)]
struct StoredArtifactMetadata {
    rendering_hints: Option<serde_json::Value>,
    mcp_schema: Option<serde_json::Value>,
    is_internal: Option<bool>,
    execution_index: Option<usize>,
    artifact_extensions: Vec<serde_json::Value>,
}

fn stored_metadata(metadata: &serde_json::Value) -> StoredArtifactMetadata {
    let non_null = |key: &str| {
        metadata
            .get(key)
            .and_then(|v| if v.is_null() { None } else { Some(v.clone()) })
    };

    StoredArtifactMetadata {
        rendering_hints: non_null("rendering_hints"),
        mcp_schema: non_null("mcp_schema"),
        is_internal: metadata
            .get("is_internal")
            .and_then(serde_json::Value::as_bool),
        execution_index: metadata
            .get("execution_index")
            .and_then(serde_json::Value::as_u64)
            .and_then(|v| usize::try_from(v).ok()),
        artifact_extensions: metadata
            .get("artifact_extensions")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default(),
    }
}
