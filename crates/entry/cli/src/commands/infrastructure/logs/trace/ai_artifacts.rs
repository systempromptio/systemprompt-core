//! Artifact rendering for AI traces.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_logging::{CliService, TaskArtifact};

use super::ai_display::{print_content_block, print_section, truncate};
use crate::presentation::tables::task_artifacts_table;

pub(super) fn print_artifacts(artifacts: &[TaskArtifact]) {
    if artifacts.is_empty() {
        return;
    }

    print_section("ARTIFACTS");
    CliService::info(&task_artifacts_table(artifacts));

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
