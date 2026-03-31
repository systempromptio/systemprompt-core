use std::collections::HashSet;
use systemprompt_logging::{CliService, TaskArtifact};
use tabled::Table;
use tabled::settings::Style;

use super::ai_display::{ArtifactRow, print_content_block, print_section, truncate};

pub fn print_artifacts(artifacts: &[TaskArtifact]) {
    if artifacts.is_empty() {
        return;
    }

    let mut seen_artifacts: HashSet<String> = HashSet::new();
    let mut artifact_rows: Vec<ArtifactRow> = Vec::new();

    for artifact in artifacts {
        if seen_artifacts.insert(artifact.artifact_id.to_string()) {
            artifact_rows.push(ArtifactRow {
                artifact_id: truncate(artifact.artifact_id.as_str(), 12),
                artifact_type: artifact.artifact_type.clone(),
                name: artifact
                    .name
                    .as_ref()
                    .map_or_else(|| "-".to_string(), |s| truncate(s, 30)),
                source: artifact.source.clone().unwrap_or_else(|| "-".to_string()),
                tool_name: artifact
                    .tool_name
                    .clone()
                    .unwrap_or_else(|| "-".to_string()),
            });
        }
    }

    print_section("ARTIFACTS");
    let table = Table::new(&artifact_rows)
        .with(Style::rounded())
        .to_string();
    CliService::info(&table);

    let mut current_artifact: Option<String> = None;
    for artifact in artifacts {
        let id_str = artifact.artifact_id.to_string();
        if current_artifact.as_ref() != Some(&id_str) {
            let truncated_id = truncate(artifact.artifact_id.as_str(), 12);
            current_artifact = Some(id_str);
            let display_name = artifact.name.as_deref().unwrap_or(&truncated_id);
            CliService::info(&format!(
                "── {} ({}) ──",
                display_name, artifact.artifact_type
            ));
        }

        match artifact.part_kind.as_deref() {
            Some("text") => {
                if let Some(ref content) = artifact.text_content {
                    print_content_block(content);
                }
            },
            Some("data") => {
                if let Some(ref data) = artifact.data_content {
                    let formatted = serde_json::to_string_pretty(data).unwrap_or_else(|e| {
                        tracing::warn!(error = %e, "Failed to format artifact data as JSON");
                        String::new()
                    });
                    print_content_block(&formatted);
                }
            },
            _ => {},
        }
    }
}
