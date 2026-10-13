//! A run's A2A content: the artifacts of a [`RunOutcome`] and the prompt
//! flattened from an inbound [`Message`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{AgentRunId, ArtifactId, ContextId, TaskId};
use systemprompt_models::a2a::{Artifact, ArtifactMetadata, Message, Part, PartContent};

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

// JSON: run output is published as an object; a non-object answer is wrapped
// under `value` so artifact renderers always read an object payload.
pub(crate) fn data_part(value: &serde_json::Value) -> Part {
    let data = match value {
        serde_json::Value::Object(_) => value.clone(),
        other => serde_json::json!({ "value": other }),
    };
    Part::data(data)
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
            parts.push(Part::text(text.clone()));
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
    match &part.content {
        PartContent::Text(text) => text.clone(),
        PartContent::Data(value) => {
            let pretty = serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string());
            format!("```json\n{pretty}\n```")
        },
        PartContent::Url(url) => format!("[file: {url}]"),
        PartContent::Raw(_) => {
            format!("[file: {}]", part.filename.as_deref().unwrap_or("unnamed"))
        },
    }
}
