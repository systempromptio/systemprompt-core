//! A2A task, status, and state wire types.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::artifact::Artifact;
use super::message::Message;
use super::task_metadata::TaskMetadata;
use serde::{Deserialize, Serialize};

use crate::errors::ParseEnumError;
use systemprompt_identifiers::{ContextId, TaskId};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: TaskId,
    pub context_id: ContextId,
    pub status: TaskStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history: Option<Vec<Message>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifacts: Option<Vec<Artifact>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<TaskMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_modified: Option<chrono::DateTime<chrono::Utc>>,
}

impl Default for Task {
    fn default() -> Self {
        let now = chrono::Utc::now();
        Self {
            id: TaskId::generate(),
            context_id: ContextId::generate(),
            status: TaskStatus::default(),
            history: None,
            artifacts: None,
            metadata: None,
            created_at: Some(now),
            last_modified: Some(now),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct TaskStatus {
    pub state: TaskState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<Message>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<chrono::DateTime<chrono::Utc>>,
}

impl Default for TaskStatus {
    fn default() -> Self {
        Self {
            state: TaskState::Submitted,
            message: None,
            timestamp: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    #[serde(rename = "TASK_STATE_SUBMITTED")]
    Submitted,
    #[serde(rename = "TASK_STATE_WORKING")]
    Working,
    #[serde(rename = "TASK_STATE_COMPLETED")]
    Completed,
    #[serde(rename = "TASK_STATE_FAILED")]
    Failed,
    #[serde(rename = "TASK_STATE_CANCELED")]
    Canceled,
    #[serde(rename = "TASK_STATE_REJECTED")]
    Rejected,
    #[serde(rename = "TASK_STATE_INPUT_REQUIRED")]
    InputRequired,
    #[serde(rename = "TASK_STATE_AUTH_REQUIRED")]
    AuthRequired,
}

impl TaskState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Submitted => "TASK_STATE_SUBMITTED",
            Self::Working => "TASK_STATE_WORKING",
            Self::Completed => "TASK_STATE_COMPLETED",
            Self::Failed => "TASK_STATE_FAILED",
            Self::Canceled => "TASK_STATE_CANCELED",
            Self::Rejected => "TASK_STATE_REJECTED",
            Self::InputRequired => "TASK_STATE_INPUT_REQUIRED",
            Self::AuthRequired => "TASK_STATE_AUTH_REQUIRED",
        }
    }

    pub const fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Failed | Self::Canceled | Self::Rejected
        )
    }

    pub const fn can_transition_to(&self, target: &Self) -> bool {
        if self.is_terminal() {
            return false;
        }
        match self {
            Self::Submitted => matches!(
                target,
                Self::Working
                    | Self::Completed
                    | Self::Failed
                    | Self::Canceled
                    | Self::Rejected
                    | Self::AuthRequired
            ),
            Self::Working => matches!(
                target,
                Self::Completed | Self::Failed | Self::Canceled | Self::InputRequired
            ),
            Self::InputRequired => matches!(
                target,
                Self::Working | Self::Completed | Self::Failed | Self::Canceled
            ),
            Self::AuthRequired => matches!(target, Self::Working | Self::Failed | Self::Canceled),
            Self::Completed | Self::Failed | Self::Canceled | Self::Rejected => false,
        }
    }
}

impl std::str::FromStr for TaskState {
    type Err = ParseEnumError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "TASK_STATE_SUBMITTED" => Ok(Self::Submitted),
            "TASK_STATE_WORKING" => Ok(Self::Working),
            "TASK_STATE_COMPLETED" => Ok(Self::Completed),
            "TASK_STATE_FAILED" => Ok(Self::Failed),
            "TASK_STATE_CANCELED" => Ok(Self::Canceled),
            "TASK_STATE_REJECTED" => Ok(Self::Rejected),
            "TASK_STATE_INPUT_REQUIRED" => Ok(Self::InputRequired),
            "TASK_STATE_AUTH_REQUIRED" => Ok(Self::AuthRequired),
            _ => Err(ParseEnumError::new("task state", s)),
        }
    }
}
