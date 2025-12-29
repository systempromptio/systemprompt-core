use chrono::{DateTime, Utc};

use systemprompt_identifiers::{AiToolCallId, ArtifactId, ContextId, ExecutionStepId, TaskId};
use systemprompt_models::a2a::{Part, Task};
use systemprompt_models::execution::StepStatus;

pub use systemprompt_models::a2a::TaskState;

pub type StepStatusDisplay = StepStatus;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LoadingState {
    #[default]
    Idle,
    Sending,
    Connecting,
    Streaming,
    WaitingForTool,
    WaitingForInput,
}

pub fn format_duration(ms: i64) -> String {
    if ms < 1000 {
        format!("{}ms", ms)
    } else if ms < 60000 {
        format!("{:.1}s", ms as f64 / 1000.0)
    } else {
        let mins = ms / 60000;
        let secs = (ms % 60000) / 1000;
        format!("{}m {}s", mins, secs)
    }
}

pub fn truncate_text(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        text.to_string()
    } else {
        format!("{}...", &text[..max_len.saturating_sub(3)])
    }
}

pub fn short_id(id: &str) -> String {
    if id.len() > 8 {
        id[..8].to_string()
    } else {
        id.to_string()
    }
}

#[derive(Debug, Clone)]
pub struct TaskDisplay {
    pub task_id: TaskId,
    pub context_id: ContextId,
    pub state: TaskState,
    pub user_message: Option<String>,
    pub agent_response: Option<String>,
    pub metadata: TaskMetadataDisplay,
    pub artifacts: Vec<ArtifactReference>,
    pub is_current: bool,
}

impl TaskDisplay {
    pub fn from_task(task: &Task, is_current: bool) -> Self {
        let task_id = task.id.clone();
        let context_id = task.context_id.clone();
        let state = task.status.state;

        let user_message = task.history.as_ref().and_then(|h| {
            h.iter().find(|m| m.role == "user").and_then(|m| {
                m.parts.iter().find_map(|p| {
                    if let Part::Text(text_part) = p {
                        Some(text_part.text.clone())
                    } else {
                        None
                    }
                })
            })
        });

        let agent_response = task.history.as_ref().and_then(|h| {
            h.iter().rfind(|m| m.role == "agent").and_then(|m| {
                m.parts.iter().find_map(|p| {
                    if let Part::Text(text_part) = p {
                        Some(text_part.text.clone())
                    } else {
                        None
                    }
                })
            })
        });

        let artifacts = task
            .artifacts
            .as_ref()
            .map(|a| {
                a.iter()
                    .map(|art| ArtifactReference {
                        artifact_id: art.id.clone(),
                        name: art.name.clone(),
                        artifact_type: Some(art.metadata.artifact_type.clone()),
                    })
                    .collect()
            })
            .unwrap_or_default();

        Self {
            task_id,
            context_id,
            state,
            user_message,
            agent_response,
            metadata: TaskMetadataDisplay::from_task(task),
            artifacts,
            is_current,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TaskMetadataDisplay {
    pub task_id: TaskId,
    pub context_id: ContextId,
    pub state: TaskState,
    pub agent_name: Option<String>,
    pub model: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub execution_time_ms: Option<i64>,
    pub step_count: usize,
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
    pub artifact_ids: Vec<ArtifactId>,
}

impl Default for TaskMetadataDisplay {
    fn default() -> Self {
        Self {
            task_id: TaskId::generate(),
            context_id: ContextId::generate(),
            state: TaskState::Pending,
            agent_name: None,
            model: None,
            started_at: None,
            completed_at: None,
            execution_time_ms: None,
            step_count: 0,
            input_tokens: None,
            output_tokens: None,
            artifact_ids: Vec::new(),
        }
    }
}

impl TaskMetadataDisplay {
    pub fn from_task(task: &Task) -> Self {
        let task_id = task.id.clone();
        let context_id = task.context_id.clone();
        let state = task.status.state;

        let metadata = task.metadata.as_ref();

        let started_at = metadata
            .and_then(|m| m.started_at.as_ref())
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc));

        let completed_at = metadata
            .and_then(|m| m.completed_at.as_ref())
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc));

        let execution_steps = metadata
            .and_then(|m| m.execution_steps.as_ref())
            .map_or(0, Vec::len);

        let artifact_ids = task
            .artifacts
            .as_ref()
            .map(|a| a.iter().map(|art| art.id.clone()).collect())
            .unwrap_or_default();

        Self {
            task_id,
            context_id,
            state,
            agent_name: metadata.map(|m| m.agent_name.clone()),
            model: metadata.and_then(|m| m.model.clone()),
            started_at,
            completed_at,
            execution_time_ms: metadata.and_then(|m| m.execution_time_ms),
            step_count: execution_steps,
            input_tokens: metadata.and_then(|m| m.input_tokens),
            output_tokens: metadata.and_then(|m| m.output_tokens),
            artifact_ids,
        }
    }

    pub fn duration_display(&self) -> Option<String> {
        self.execution_time_ms.map(format_duration).or_else(|| {
            self.started_at.zip(self.completed_at).map(|(start, end)| {
                format_duration(end.signed_duration_since(start).num_milliseconds())
            })
        })
    }

    pub fn tokens_display(&self) -> Option<String> {
        match (self.input_tokens, self.output_tokens) {
            (Some(i), Some(o)) => Some(format!("{}→{} tokens", i, o)),
            (Some(i), None) => Some(format!("{} in", i)),
            (None, Some(o)) => Some(format!("{} out", o)),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExecutionStepDisplay {
    pub step_id: ExecutionStepId,
    pub status: StepStatusDisplay,
    pub step_type: Option<String>,
    pub tool_name: Option<String>,
    pub content: Option<String>,
    pub duration_ms: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct InlineToolCall {
    pub id: AiToolCallId,
    pub name: String,
    pub arguments: serde_json::Value,
    pub arguments_preview: String,
    pub status: ToolCallStatus,
    pub result: Option<String>,
    pub result_preview: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ArtifactReference {
    pub artifact_id: ArtifactId,
    pub name: Option<String>,
    pub artifact_type: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputType {
    #[default]
    Text,
    Choice,
    Confirm,
}

#[derive(Debug, Clone)]
pub struct InputRequest {
    pub request_id: String,
    pub prompt: String,
    pub input_type: InputType,
    pub choices: Option<Vec<String>>,
    pub default_value: Option<String>,
    pub selected_choice: usize,
    pub text_input: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolCallStatus {
    Pending,
    Approved,
    Rejected,
    Executing,
    Completed,
    Failed,
}
