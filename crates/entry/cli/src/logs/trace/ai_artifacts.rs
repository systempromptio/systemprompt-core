use std::collections::HashSet;
use systemprompt_core_logging::{CliService, TaskArtifact};
use tabled::settings::Style;
use tabled::Table;

use super::ai_display::{print_content_block, print_section, truncate, ArtifactRow};

pub fn print_artifacts(artifacts: &[TaskArtifact]) {
    if artifacts.is_empty() {
        return;
    }

    let mut seen_artifacts: HashSet<String> = HashSet::new();
    let mut artifact_rows: Vec<ArtifactRow> = Vec::new();

    for artifact in artifacts {
        if seen_artifacts.insert(artifact.artifact_id.clone()) {
            artifact_rows.push(ArtifactRow {
                artifact_id: truncate(&artifact.artifact_id, 12),
                artifact_type: artifact.artifact_type.clone(),
                name: artifact
                    .name
                    .as_ref()
                    .map_or("-".to_string(), |s| truncate(s, 30)),
                source: artifact.source.clone().unwrap_or_else(|| "-".to_string()),
                tool_name: artifact.tool_name.clone().unwrap_or_else(|| "-".to_string()),
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
        if current_artifact.as_ref() != Some(&artifact.artifact_id) {
            current_artifact = Some(artifact.artifact_id.clone());
            CliService::info(&format!(
                "── {} ({}) ──",
                artifact
                    .name
                    .as_ref()
                    .unwrap_or(&truncate(&artifact.artifact_id, 12)),
                artifact.artifact_type
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
                    let formatted = serde_json::to_string_pretty(data).unwrap_or_default();
                    print_content_block(&formatted);
                }
            },
            _ => {},
        }
    }
}
