use super::artifact::Artifact;
use super::message::Message;
use super::task_metadata::TaskMetadata;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{ContextId, TaskId};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: TaskId,
    pub context_id: ContextId,
    pub status: TaskStatus,
    pub history: Option<Vec<Message>>,
    pub artifacts: Option<Vec<Artifact>>,
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
    pub message: Option<Message>,
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
    #[serde(rename = "TASK_STATE_PENDING")]
    Pending,
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
    #[serde(rename = "TASK_STATE_UNKNOWN")]
    Unknown,
}

impl TaskState {
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
            Self::Pending => matches!(target, Self::Submitted),
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
            _ => false,
        }
    }
}

impl std::str::FromStr for TaskState {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "TASK_STATE_PENDING" | "pending" => Ok(Self::Pending),
            "TASK_STATE_SUBMITTED" | "submitted" => Ok(Self::Submitted),
            "TASK_STATE_WORKING" | "working" => Ok(Self::Working),
            "TASK_STATE_COMPLETED" | "completed" => Ok(Self::Completed),
            "TASK_STATE_FAILED" | "failed" => Ok(Self::Failed),
            "TASK_STATE_CANCELED" | "canceled" => Ok(Self::Canceled),
            "TASK_STATE_REJECTED" | "rejected" => Ok(Self::Rejected),
            "TASK_STATE_INPUT_REQUIRED" | "input-required" => Ok(Self::InputRequired),
            "TASK_STATE_AUTH_REQUIRED" | "auth-required" => Ok(Self::AuthRequired),
            "TASK_STATE_UNKNOWN" | "unknown" => Ok(Self::Unknown),
            _ => Err(format!("Invalid task state: {s}")),
        }
    }
}
