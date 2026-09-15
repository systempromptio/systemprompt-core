//! Part/artifact assembly for batch task construction.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::models::a2a::{Artifact, Message, Part};
use crate::models::{
    ArtifactPartRow, ArtifactRow, ExecutionStepBatchRow, MessagePart, TaskMessage,
};
use crate::repository::content::artifact::artifact_from_row;
use crate::repository::parts::part_from_row;
use std::collections::HashMap;
use systemprompt_identifiers::{ArtifactId, MessageId};
use systemprompt_models::{ExecutionStep, StepContent, StepId, StepStatus};
use systemprompt_traits::RepositoryError;

use super::converters;

pub fn build_execution_steps(
    steps: Option<&Vec<&ExecutionStepBatchRow>>,
) -> Result<Option<Vec<ExecutionStep>>, RepositoryError> {
    let Some(steps) = steps else {
        return Ok(None);
    };
    if steps.is_empty() {
        return Ok(None);
    }

    let mut result = Vec::with_capacity(steps.len());
    for row in steps {
        let status = row.status.parse::<StepStatus>().map_err(|e| {
            RepositoryError::InvalidData(format!(
                "execution step {} has an invalid status: {e}",
                row.step_id
            ))
        })?;
        let content: StepContent = serde_json::from_value(row.content.clone()).map_err(|e| {
            RepositoryError::InvalidData(format!(
                "execution step {} has invalid content: {e}",
                row.step_id
            ))
        })?;

        result.push(ExecutionStep {
            step_id: StepId(row.step_id.to_string()),
            task_id: row.task_id.clone(),
            status,
            started_at: row.started_at,
            completed_at: row.completed_at,
            duration_ms: row.duration_ms,
            error_message: row.error_message.clone(),
            content,
        });
    }

    Ok(Some(result))
}

#[expect(
    clippy::implicit_hasher,
    reason = "internal batch-assembly helper; callers always pass the default hasher"
)]
pub fn build_messages(
    messages: Option<&Vec<&TaskMessage>>,
    parts_by_message: &HashMap<MessageId, Vec<&MessagePart>>,
) -> Result<Option<Vec<Message>>, RepositoryError> {
    let Some(messages) = messages else {
        return Ok(None);
    };
    if messages.is_empty() {
        return Ok(None);
    }

    let mut result = Vec::with_capacity(messages.len());
    for msg_row in messages {
        let parts = build_parts(parts_by_message.get(&msg_row.message_id))?;
        result.push(converters::message_from_row((*msg_row).clone(), parts));
    }

    Ok(Some(result))
}

#[expect(
    clippy::implicit_hasher,
    reason = "internal batch-assembly helper; callers always pass the default hasher"
)]
pub fn build_artifacts(
    artifacts: Option<&Vec<&ArtifactRow>>,
    artifact_parts_by_id: &HashMap<ArtifactId, Vec<&ArtifactPartRow>>,
) -> Result<Option<Vec<Artifact>>, RepositoryError> {
    let Some(artifacts) = artifacts else {
        return Ok(None);
    };
    if artifacts.is_empty() {
        return Ok(None);
    }

    let mut result = Vec::with_capacity(artifacts.len());
    for row in artifacts {
        let parts = build_parts(artifact_parts_by_id.get(&row.artifact_id))?;
        result.push(artifact_from_row((*row).clone(), parts));
    }

    Ok(Some(result))
}

fn build_parts<R: crate::repository::parts::PartColumns>(
    parts: Option<&Vec<&R>>,
) -> Result<Vec<Part>, RepositoryError> {
    let Some(parts) = parts else {
        return Ok(Vec::new());
    };
    parts.iter().map(|row| part_from_row(*row)).collect()
}
