mod messages;
pub mod progress;
mod tasks;
mod types;

pub use progress::ProgressState;
pub use types::{
    format_duration, short_id, truncate_text, ArtifactReference, ExecutionStepDisplay,
    InlineToolCall, InputRequest, InputType, LoadingState, StepStatusDisplay, TaskDisplay,
    TaskMetadataDisplay, TaskState, ToolCallStatus,
};

use std::time::Instant;

use chrono::{DateTime, Utc};
use systemprompt_identifiers::ContextId;
use systemprompt_models::a2a::Task;

#[derive(Debug)]
pub struct ChatState {
    pub context_id: Option<ContextId>,
    pub tasks: Vec<Task>,
    pub needs_initial_load: bool,
    pub input_buffer: String,
    pub cursor_position: usize,
    pub scroll_offset: usize,
    pub progress: ProgressState,
    pub current_inline_tools: Vec<InlineToolCall>,
    pub show_execution_timeline: bool,
    pub selected_tool_index: Option<usize>,
    pub tool_panel_scroll: usize,
    pub pending_input_request: Option<InputRequest>,
    pub pending_user_message: Option<String>,
    pub(crate) pending_message_sent_at: Option<Instant>,
    pub selected_task_index: Option<usize>,
    pub task_detail_scroll: usize,
}

impl ChatState {
    pub fn new() -> Self {
        Self {
            context_id: None,
            tasks: Vec::new(),
            needs_initial_load: true,
            input_buffer: String::new(),
            cursor_position: 0,
            scroll_offset: 0,
            progress: ProgressState::default(),
            current_inline_tools: Vec::new(),
            show_execution_timeline: true,
            selected_tool_index: None,
            tool_panel_scroll: 0,
            pending_input_request: None,
            pending_user_message: None,
            pending_message_sent_at: None,
            selected_task_index: None,
            task_detail_scroll: 0,
        }
    }

    pub fn set_loading(&mut self, state: LoadingState) {
        self.progress.loading = state;
    }

    pub const fn loading_state(&self) -> LoadingState {
        self.progress.loading
    }

    pub fn active_task_id(&self) -> Option<&str> {
        self.progress.active_task_id.as_ref().map(AsRef::as_ref)
    }

    pub const fn current_task_state(&self) -> TaskState {
        match self.progress.loading {
            LoadingState::Idle | LoadingState::Sending | LoadingState::Connecting => {
                TaskState::Pending
            },
            LoadingState::Streaming | LoadingState::WaitingForTool => TaskState::Working,
            LoadingState::WaitingForInput => TaskState::InputRequired,
        }
    }

    pub fn streaming_response(&self) -> &str {
        &self.progress.streaming_content
    }

    pub const fn streaming_started_at(&self) -> Option<DateTime<Utc>> {
        self.progress.started_at
    }

    pub const fn current_step_count(&self) -> usize {
        self.progress.step_count
    }

    pub fn current_execution_steps(&self) -> &[ExecutionStepDisplay] {
        &self.progress.current_steps
    }

    pub fn set_context(&mut self, context_id: ContextId) {
        self.context_id = Some(context_id);
        self.scroll_offset = 0;
        self.needs_initial_load = true;
        self.tasks.clear();
    }

    pub const fn is_processing(&self) -> bool {
        self.pending_user_message.is_some() || self.progress.active_task_id.is_some()
    }

    pub fn clear_input(&mut self) {
        self.input_buffer.clear();
        self.cursor_position = 0;
    }

    pub fn scroll_up(&mut self, amount: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(amount);
    }

    pub fn scroll_down(&mut self, amount: usize, max_lines: usize) {
        self.scroll_offset = (self.scroll_offset + amount).min(max_lines.saturating_sub(1));
    }

    pub fn scroll_to_bottom(&mut self, max_lines: usize) {
        self.scroll_offset = max_lines.saturating_sub(1);
    }

    pub fn clear(&mut self) {
        self.input_buffer.clear();
        self.cursor_position = 0;
        self.scroll_offset = 0;
        self.clear_processing_state();
        self.pending_input_request = None;
        self.clear_pending_message();
        self.tasks.clear();
        self.selected_task_index = None;
        self.task_detail_scroll = 0;
    }
}

impl Default for ChatState {
    fn default() -> Self {
        Self::new()
    }
}
