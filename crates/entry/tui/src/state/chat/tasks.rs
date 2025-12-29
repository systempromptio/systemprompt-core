use systemprompt_identifiers::TaskId;
use systemprompt_models::a2a::{Task, TaskState as A2aTaskState};

use super::types::{
    ExecutionStepDisplay, InlineToolCall, InputRequest, InputType, TaskState, ToolCallStatus,
};
use super::{truncate_text, ChatState, LoadingState};

impl ChatState {
    pub fn find_task_mut(&mut self, task_id: &str) -> Option<&mut Task> {
        self.tasks.iter_mut().find(|t| t.id.as_ref() == task_id)
    }

    pub fn start_task_progress(&mut self, task_id: TaskId) {
        self.on_progress_started(task_id);
        self.current_inline_tools.clear();
    }

    pub fn finish_task_progress(&mut self) {
        self.clear_processing_state();
    }

    pub(super) fn clear_processing_state(&mut self) {
        self.progress.reset();
        self.current_inline_tools.clear();
        self.selected_tool_index = None;
        self.tool_panel_scroll = 0;
    }

    pub fn update_task_state(&mut self, state: TaskState) {
        match state {
            TaskState::Working => self.set_loading(LoadingState::Streaming),
            TaskState::InputRequired => self.set_loading(LoadingState::WaitingForInput),
            TaskState::Completed | TaskState::Canceled | TaskState::Failed => {
                self.set_loading(LoadingState::Idle);
            },
            _ => {},
        }
    }

    pub fn add_execution_step(&mut self, step: ExecutionStepDisplay) {
        self.progress.step_count += 1;
        self.progress.current_steps.push(step);
    }

    pub fn add_inline_tool_call(&mut self, tool: InlineToolCall) {
        self.current_inline_tools.push(tool);
    }

    pub fn update_inline_tool_result(&mut self, call_id: &str, result: &str, is_error: bool) {
        if let Some(tool) = self
            .current_inline_tools
            .iter_mut()
            .find(|t| t.id.as_ref() == call_id)
        {
            tool.status = if is_error {
                ToolCallStatus::Failed
            } else {
                ToolCallStatus::Completed
            };
            tool.result = Some(result.to_owned());
            tool.result_preview = Some(truncate_text(result, 100));
        }
    }

    pub fn cancel_task(&mut self) {
        if let Some(task_id) = self.progress.active_task_id.clone() {
            if let Some(task) = self
                .tasks
                .iter_mut()
                .find(|t| t.id.as_ref() == task_id.as_ref())
            {
                task.status.state = A2aTaskState::Canceled;
            }
        }
        self.clear_processing_state();
        self.clear_pending_message();
    }

    pub fn select_next_task(&mut self) {
        if self.tasks.is_empty() {
            self.selected_task_index = None;
            return;
        }
        self.selected_task_index = Some(match self.selected_task_index {
            None => self.tasks.len().saturating_sub(1),
            Some(i) => (i + 1).min(self.tasks.len() - 1),
        });
    }

    pub fn select_prev_task(&mut self) {
        if self.tasks.is_empty() {
            self.selected_task_index = None;
            return;
        }
        self.selected_task_index = Some(match self.selected_task_index {
            None => self.tasks.len().saturating_sub(1),
            Some(i) => i.saturating_sub(1),
        });
    }

    pub fn selected_task(&self) -> Option<&Task> {
        self.selected_task_index.and_then(|i| self.tasks.get(i))
    }

    pub fn selected_task_id(&self) -> Option<String> {
        self.selected_task().map(|t| t.id.as_ref().to_string())
    }

    pub const fn has_task_selected(&self) -> bool {
        self.selected_task_index.is_some()
    }

    pub fn close_task_detail(&mut self) {
        self.selected_task_index = None;
        self.task_detail_scroll = 0;
    }

    pub fn scroll_task_detail_up(&mut self, amount: usize) {
        self.task_detail_scroll = self.task_detail_scroll.saturating_sub(amount);
    }

    pub fn scroll_task_detail_down(&mut self, amount: usize) {
        self.task_detail_scroll = self.task_detail_scroll.saturating_add(amount);
    }

    pub fn remove_task(&mut self, task_id: &str) {
        self.tasks.retain(|t| t.id.as_ref() != task_id);
        if let Some(idx) = self.selected_task_index {
            if idx >= self.tasks.len() {
                self.selected_task_index = if self.tasks.is_empty() {
                    None
                } else {
                    Some(self.tasks.len() - 1)
                };
            }
        }
    }

    pub fn toggle_execution_timeline(&mut self) {
        self.show_execution_timeline = !self.show_execution_timeline;
    }

    pub fn select_next_tool(&mut self) {
        if self.current_inline_tools.is_empty() {
            self.selected_tool_index = None;
            return;
        }
        self.selected_tool_index = Some(match self.selected_tool_index {
            None => 0,
            Some(i) => (i + 1).min(self.current_inline_tools.len() - 1),
        });
    }

    pub fn select_prev_tool(&mut self) {
        if self.current_inline_tools.is_empty() {
            self.selected_tool_index = None;
            return;
        }
        self.selected_tool_index =
            Some(self.selected_tool_index.map_or(0, |i| i.saturating_sub(1)));
    }

    pub fn selected_tool(&self) -> Option<&InlineToolCall> {
        self.selected_tool_index
            .and_then(|i| self.current_inline_tools.get(i))
    }

    pub fn close_tool_panel(&mut self) {
        self.selected_tool_index = None;
        self.tool_panel_scroll = 0;
    }

    pub fn scroll_tool_panel_up(&mut self, amount: usize) {
        self.tool_panel_scroll = self.tool_panel_scroll.saturating_sub(amount);
    }

    pub fn scroll_tool_panel_down(&mut self, amount: usize) {
        self.tool_panel_scroll = self.tool_panel_scroll.saturating_add(amount);
    }

    pub fn set_input_request(&mut self, request: InputRequest) {
        self.pending_input_request = Some(request);
    }

    pub const fn has_pending_input(&self) -> bool {
        self.pending_input_request.is_some()
    }

    pub const fn pending_input(&self) -> Option<&InputRequest> {
        self.pending_input_request.as_ref()
    }

    pub fn input_next_choice(&mut self) {
        if let Some(request) = &mut self.pending_input_request {
            if let Some(choices) = &request.choices {
                if !choices.is_empty() {
                    request.selected_choice = (request.selected_choice + 1) % choices.len();
                }
            }
        }
    }

    pub fn input_prev_choice(&mut self) {
        if let Some(request) = &mut self.pending_input_request {
            if let Some(choices) = &request.choices {
                if !choices.is_empty() {
                    request.selected_choice = if request.selected_choice == 0 {
                        choices.len() - 1
                    } else {
                        request.selected_choice - 1
                    };
                }
            }
        }
    }

    pub fn input_push_char(&mut self, c: char) {
        if let Some(request) = &mut self.pending_input_request {
            request.text_input.push(c);
        }
    }

    pub fn input_pop_char(&mut self) {
        if let Some(request) = &mut self.pending_input_request {
            request.text_input.pop();
        }
    }

    pub fn get_input_value(&self) -> Option<String> {
        self.pending_input_request
            .as_ref()
            .map(|request| match request.input_type {
                InputType::Text => request.text_input.clone(),
                InputType::Choice => request
                    .choices
                    .as_ref()
                    .and_then(|c| c.get(request.selected_choice))
                    .cloned()
                    .unwrap_or_default(),
                InputType::Confirm => {
                    if request.selected_choice == 0 {
                        "yes".to_string()
                    } else {
                        "no".to_string()
                    }
                },
            })
    }

    pub fn clear_input_request(&mut self) -> Option<InputRequest> {
        self.pending_input_request.take()
    }
}
