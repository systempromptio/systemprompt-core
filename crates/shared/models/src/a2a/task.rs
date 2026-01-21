use super::artifact::Artifact;
use super::message::Message;
use super::task_metadata::TaskMetadata;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{ContextId, TaskId};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Task {
    pub id: TaskId,
    #[serde(rename = "contextId")]
    pub context_id: ContextId,
    pub status: TaskStatus,
    pub history: Option<Vec<Message>>,
    pub artifacts: Option<Vec<Artifact>>,
    pub metadata: Option<TaskMetadata>,
    #[serde(rename = "kind")]
    pub kind: String,
}

impl Default for Task {
    fn default() -> Self {
        Self {
            id: TaskId::generate(),
            context_id: ContextId::generate(),
            status: TaskStatus::default(),
            history: None,
            artifacts: None,
            metadata: None,
            kind: "task".to_string(),
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
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "submitted")]
    Submitted,
    #[serde(rename = "working")]
    Working,
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "canceled")]
    Canceled,
    #[serde(rename = "rejected")]
    Rejected,
    #[serde(rename = "input-required")]
    InputRequired,
    #[serde(rename = "auth-required")]
    AuthRequired,
    #[serde(rename = "unknown")]
    Unknown,
}

impl std::str::FromStr for TaskState {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(Self::Pending),
            "submitted" => Ok(Self::Submitted),
            "working" => Ok(Self::Working),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "canceled" => Ok(Self::Canceled),
            "rejected" => Ok(Self::Rejected),
            "input-required" => Ok(Self::InputRequired),
            "auth-required" => Ok(Self::AuthRequired),
            "unknown" => Ok(Self::Unknown),
            _ => Err(format!("Invalid task state: {s}")),
        }
    }
}
