use chrono::{DateTime, Utc};
use systemprompt_identifiers::TaskId;

use super::types::ExecutionStepDisplay;
use super::LoadingState;

#[derive(Debug)]
pub struct ProgressState {
    pub loading: LoadingState,
    pub active_task_id: Option<TaskId>,
    pub current_steps: Vec<ExecutionStepDisplay>,
    pub streaming_content: String,
    pub started_at: Option<DateTime<Utc>>,
    pub step_count: usize,
}

impl Default for ProgressState {
    fn default() -> Self {
        Self {
            loading: LoadingState::Idle,
            active_task_id: None,
            current_steps: Vec::new(),
            streaming_content: String::new(),
            started_at: None,
            step_count: 0,
        }
    }
}

impl ProgressState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        self.loading = LoadingState::Idle;
        self.active_task_id = None;
        self.current_steps.clear();
        self.streaming_content.clear();
        self.started_at = None;
        self.step_count = 0;
    }

    pub fn is_active(&self) -> bool {
        self.loading != LoadingState::Idle
    }
}
