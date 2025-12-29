use chrono::Utc;
use systemprompt_identifiers::{ContextId, MessageId, TaskId};
use systemprompt_models::a2a::{
    Message, Part, Task, TaskState as A2aTaskState, TaskStatus, TextPart,
};

use super::{ChatState, LoadingState};

const PENDING_MESSAGE_TIMEOUT_SECS: u64 = 30;

impl ChatState {
    pub fn on_message_sent(&mut self, content: String) {
        self.set_pending_message(content);
        self.set_loading(LoadingState::Sending);
    }

    pub fn on_task_created(&mut self, task: Task) {
        self.clear_pending_message();
        self.upsert_task(task);
        if self.progress.active_task_id.is_some() {
            self.set_loading(LoadingState::Streaming);
        } else {
            self.set_loading(LoadingState::Connecting);
        }
    }

    pub fn on_progress_started(&mut self, task_id: TaskId) {
        self.progress.active_task_id = Some(task_id);
        self.progress.started_at = Some(Utc::now());
        self.progress.streaming_content.clear();
        self.progress.current_steps.clear();
        self.progress.step_count = 0;
        self.set_loading(LoadingState::Streaming);
    }

    pub fn on_progress_finished(&mut self) {
        self.progress.reset();
    }

    pub fn set_pending_message(&mut self, message: String) {
        self.pending_user_message = Some(message);
        self.pending_message_sent_at = Some(std::time::Instant::now());
    }

    pub fn check_pending_timeout(&mut self) -> bool {
        if let (Some(_), Some(sent_at)) = (&self.pending_user_message, self.pending_message_sent_at)
        {
            if sent_at.elapsed().as_secs() >= PENDING_MESSAGE_TIMEOUT_SECS {
                tracing::warn!(
                    "Pending message timed out after {} seconds",
                    PENDING_MESSAGE_TIMEOUT_SECS
                );
                self.create_failed_task(&format!(
                    "Request timed out after {}s. The server may be busy or unreachable.",
                    PENDING_MESSAGE_TIMEOUT_SECS
                ));
                return true;
            }
        }
        false
    }

    pub fn create_failed_task(&mut self, error_message: &str) {
        let Some(user_message_text) = self.pending_user_message.take() else {
            return;
        };
        self.pending_message_sent_at = None;

        let task_id = TaskId::generate();
        let now = Utc::now();
        let context_id = self.context_id.clone().unwrap_or_else(ContextId::generate);

        let user_message = Message {
            role: "user".to_string(),
            parts: vec![Part::Text(TextPart {
                text: user_message_text,
            })],
            id: MessageId::generate(),
            task_id: Some(task_id.clone()),
            context_id: context_id.clone(),
            kind: "message".to_string(),
            metadata: None,
            extensions: None,
            reference_task_ids: None,
        };

        let error_response = Message {
            role: "agent".to_string(),
            parts: vec![Part::Text(TextPart {
                text: format!("Error: {}", error_message),
            })],
            id: MessageId::generate(),
            task_id: Some(task_id.clone()),
            context_id: context_id.clone(),
            kind: "message".to_string(),
            metadata: None,
            extensions: None,
            reference_task_ids: None,
        };

        let task = Task {
            id: task_id,
            context_id,
            status: TaskStatus {
                state: A2aTaskState::Failed,
                message: Some(error_response.clone()),
                timestamp: Some(now),
            },
            history: Some(vec![user_message, error_response]),
            artifacts: None,
            metadata: None,
            kind: "task".to_string(),
        };

        self.tasks.push(task);
        self.set_loading(LoadingState::Idle);
        self.progress.reset();
    }

    pub fn clear_pending_message(&mut self) {
        self.pending_user_message = None;
        self.pending_message_sent_at = None;
    }

    pub fn load_historical_tasks(&mut self, tasks: Vec<Task>) {
        self.tasks = tasks;
        self.needs_initial_load = false;
    }

    pub fn upsert_task(&mut self, task: Task) {
        let task_id = task.id.as_ref();

        if let Some(existing) = self.tasks.iter_mut().find(|t| t.id.as_ref() == task_id) {
            let should_update = match (&existing.status.timestamp, &task.status.timestamp) {
                (Some(existing_ts), Some(new_ts)) => new_ts >= existing_ts,
                (None, Some(_)) => true,
                _ => false,
            };

            if should_update {
                *existing = task;
            }
        } else {
            self.tasks.push(task);
            self.tasks.sort_by(|a, b| {
                let a_time = a.status.timestamp;
                let b_time = b.status.timestamp;
                a_time.cmp(&b_time)
            });

            if self.pending_user_message.is_some() {
                tracing::debug!("Clearing pending message - task received from context stream");
                self.clear_pending_message();
            }
        }
    }

    pub fn append_response_chunk(&mut self, chunk: &str) {
        self.progress.streaming_content.push_str(chunk);
    }
}
