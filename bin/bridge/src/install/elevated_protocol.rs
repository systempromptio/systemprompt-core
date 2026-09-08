//! Versioned completion evidence for a single elevated installation job.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const PROTOCOL_VERSION: u32 = 2;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompletedStep {
    pub operation: String,
    pub target: String,
    pub policies: Vec<crate::config::store::verified::PolicyReceipt>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum ElevatedState {
    Started,
    Completed {
        steps: Vec<CompletedStep>,
    },
    Failed {
        steps: Vec<CompletedStep>,
        error: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ElevatedResult {
    pub version: u32,
    pub job_id: Uuid,
    pub outcome: ElevatedState,
}

#[derive(Debug, thiserror::Error)]
pub enum ProtocolError {
    #[error("elevated result protocol {actual} is unsupported; expected {PROTOCOL_VERSION}")]
    Version { actual: u32 },
    #[error("elevated result belongs to another installation job")]
    JobMismatch,
    #[error("elevated installation started but did not finish")]
    Incomplete,
    #[error("elevated installation exited {0}; completion cannot be accepted")]
    Exit(u32),
    #[error("elevated installation reported completion without all requested steps")]
    MissingSteps,
    #[error("elevated installation failed after verified steps {steps:?}: {error}")]
    Failed {
        steps: Vec<CompletedStep>,
        error: String,
    },
}

impl ElevatedResult {
    pub fn verify(
        self,
        job_id: Uuid,
        exit_code: u32,
        expected: &[CompletedStep],
    ) -> Result<Vec<CompletedStep>, ProtocolError> {
        if self.version != PROTOCOL_VERSION {
            return Err(ProtocolError::Version {
                actual: self.version,
            });
        }
        if self.job_id != job_id {
            return Err(ProtocolError::JobMismatch);
        }
        match self.outcome {
            ElevatedState::Started => Err(ProtocolError::Incomplete),
            ElevatedState::Failed { steps, error } => Err(ProtocolError::Failed { steps, error }),
            ElevatedState::Completed { steps } => {
                if exit_code != 0 {
                    return Err(ProtocolError::Exit(exit_code));
                }
                if steps.len() != expected.len()
                    || steps.iter().zip(expected).any(|(actual, expected)| {
                        actual.operation != expected.operation
                            || actual.target != expected.target
                            || (matches!(actual.operation.as_str(), "policy" | "bridge_policy")
                                && actual.policies.is_empty())
                    })
                {
                    return Err(ProtocolError::MissingSteps);
                }
                Ok(steps)
            },
        }
    }
}
