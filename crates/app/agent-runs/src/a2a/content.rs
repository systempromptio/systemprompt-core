//! A run's A2A content: the artifacts of a [`RunOutcome`] and the prompt
//! flattened from an inbound [`Message`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{AgentRunId, ArtifactId, ContextId, TaskId};
use systemprompt_models::a2a::{Artifact, ArtifactMetadata, DataPart, Message, Part, TextPart};

use crate::outcome::RunOutcome;
use crate::request::RunRequest;

pub const AGENT_RUN_ARTIFACT_TYPE: &str = "agent_run_output";
pub const AGENT_RUN_ARTIFACT_SOURCE: &str = "agent_run";

#[must_use]
pub fn outcome_artifact_id(run_id: &AgentRunId) -> ArtifactId {
    ArtifactId::new(format!("{run_id}-output"))
}

pub(crate) fn run_artifact(
    id: ArtifactId,
    parts: Vec<Part>,
    task_id: &TaskId,
    context_id: &ContextId,
) -> Artifact {
    let metadata = ArtifactMetadata::new(
        AGENT_RUN_ARTIFACT_TYPE.to_owned(),
        context_id.clone(),
        task_id.clone(),
    )
    .with_source(AGENT_RUN_ARTIFACT_SOURCE.to_owned());
    Artifact {
        id,
        title: None,
        description: None,
        parts,
        extensions: Vec::new(),
        metadata,
    }
}

// JSON: A2A `DataPart.data` is an object; a non-object answer is wrapped
// under `value` so it still travels as data.
pub(crate) fn data_part(value: &serde_json::Value) -> Part {
    let data = match value {
        serde_json::Value::Object(map) => map.clone(),
        other => serde_json::Map::from_iter([("value".to_owned(), other.clone())]),
    };
    Part::Data(DataPart { data })
}

impl RunOutcome {
    #[must_use]
    pub fn artifacts(
        &self,
        run_id: &AgentRunId,
        task_id: &TaskId,
        context_id: &ContextId,
    ) -> Vec<Artifact> {
        let mut parts = Vec::new();
        if let Some(value) = &self.structured_output {
            parts.push(data_part(value));
        }
        if let Some(text) = &self.final_text {
            parts.push(Part::Text(TextPart { text: text.clone() }));
        }
        if parts.is_empty() {
            return Vec::new();
        }
        vec![run_artifact(
            outcome_artifact_id(run_id),
            parts,
            task_id,
            context_id,
        )]
    }
}

impl RunRequest {
    #[must_use]
    pub fn prompt_from_message(message: &Message) -> String {
        message
            .parts
            .iter()
            .map(flatten_part)
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

fn flatten_part(part: &Part) -> String {
    match part {
        Part::Text(text) => text.text.clone(),
        Part::Data(data) => {
            let value = serde_json::Value::Object(data.data.clone());
            let pretty = serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string());
            format!("```json\n{pretty}\n```")
        },
        Part::File(file) => {
            let named = file
                .file
                .url
                .as_deref()
                .or(file.file.name.as_deref())
                .unwrap_or("unnamed");
            format!("[file: {named}]")
        },
    }
}
