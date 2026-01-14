use chrono::Utc;

use crate::messages::{Command, ScrollDirection};
use crate::state::{
    ArtifactDisplay, ExecutionStepDisplay, FocusedPanel, InlineToolCall, StepStatusDisplay,
    ToolCallStatus,
};
use systemprompt_identifiers::{AiToolCallId, ExecutionStepId, TaskId};
use systemprompt_models::agui::{
    MessagesSnapshotPayload, RunErrorPayload, RunFinishedPayload, RunStartedPayload,
    StateDeltaPayload, StateSnapshotPayload, StepFinishedPayload, TextMessageContentPayload,
    TextMessageEndPayload, TextMessageStartPayload, ToolCallEndPayload,
};
use systemprompt_models::{AgUiEvent, CustomPayload};

use super::super::TuiApp;

impl TuiApp {
    pub(crate) fn handle_chat_input_changed(&mut self, input: String) -> Vec<Command> {
        self.state.chat.input_buffer = input;
        vec![Command::None]
    }

    pub(crate) fn handle_chat_send_message(&mut self) -> Vec<Command> {
        let content = self.state.chat.input_buffer.clone();
        if content.trim().is_empty() {
            vec![Command::None]
        } else {
            self.state.chat.on_message_sent(content.trim().to_string());
            self.state.chat.clear_input();
            vec![Command::SendAiMessage(content)]
        }
    }

    pub(crate) fn handle_chat_cancel_stream(&mut self) -> Vec<Command> {
        self.state.chat.cancel_task();
        vec![Command::CancelAiStream]
    }

    pub(crate) fn handle_chat_clear_conversation(&mut self) -> Vec<Command> {
        self.state.chat.clear();
        tracing::info!("Cleared conversation, creating new context");
        vec![Command::CreateNewContext]
    }

    pub(crate) fn handle_chat_scroll(&mut self, direction: ScrollDirection) -> Vec<Command> {
        let max_lines = self.state.chat.tasks.len() * 10;
        match direction {
            ScrollDirection::Up => self.state.chat.scroll_up(1),
            ScrollDirection::Down => self.state.chat.scroll_down(1, max_lines),
            ScrollDirection::PageUp => self.state.chat.scroll_up(10),
            ScrollDirection::PageDown => self.state.chat.scroll_down(10, max_lines),
            ScrollDirection::Top => self.state.chat.scroll_offset = 0,
            ScrollDirection::Bottom => self.state.chat.scroll_to_bottom(max_lines),
        }
        vec![Command::None]
    }

    pub(crate) fn handle_ai_tool_call_received(
        &mut self,
        tool_call: crate::tools::PendingToolCall,
    ) -> Vec<Command> {
        let requires_approval = self
            .tool_registry
            .get(&tool_call.tool_name)
            .is_none_or(|t| t.requires_approval());

        if requires_approval {
            self.state.tools.add_pending(tool_call);
            self.state.focus = FocusedPanel::ApprovalDialog;
            vec![Command::None]
        } else {
            vec![Command::ExecuteTool(tool_call.id)]
        }
    }

    pub(crate) fn handle_chat_task_close_detail(&mut self) -> Vec<Command> {
        self.state.chat.close_task_detail();
        vec![Command::None]
    }

    pub(crate) fn handle_chat_task_delete(&self) -> Vec<Command> {
        self.state.chat.selected_task_id().map_or_else(
            || vec![Command::None],
            |task_id| vec![Command::DeleteTask(task_id)],
        )
    }

    pub(crate) fn handle_agui_event(&mut self, event: AgUiEvent) -> Vec<Command> {
        match event {
            AgUiEvent::RunStarted { payload, .. } => self.handle_agui_run_started(&payload),
            AgUiEvent::RunFinished { payload, .. } => self.handle_agui_run_finished(&payload),
            AgUiEvent::RunError { payload, .. } => self.handle_agui_run_error(&payload),
            AgUiEvent::StepStarted { payload, .. } => {
                self.handle_agui_step_started(payload.step_name)
            },
            AgUiEvent::StepFinished { payload, .. } => Self::handle_agui_step_finished(&payload),
            AgUiEvent::TextMessageStart { payload, .. } => {
                Self::handle_agui_text_message_start(&payload)
            },
            AgUiEvent::TextMessageContent { payload, .. } => {
                Self::handle_agui_text_message_content(&payload)
            },
            AgUiEvent::TextMessageEnd { payload, .. } => {
                Self::handle_agui_text_message_end(&payload)
            },
            AgUiEvent::ToolCallStart { payload, .. } => {
                self.handle_agui_tool_call_start(payload.tool_call_id, payload.tool_call_name)
            },
            AgUiEvent::ToolCallArgs { payload, .. } => {
                self.handle_agui_tool_call_args(&payload.tool_call_id, &payload.delta)
            },
            AgUiEvent::ToolCallEnd { payload, .. } => Self::handle_agui_tool_call_end(&payload),
            AgUiEvent::ToolCallResult { payload, .. } => {
                self.handle_agui_tool_call_result(&payload.tool_call_id, &payload.content)
            },
            AgUiEvent::StateSnapshot { payload, .. } => Self::handle_agui_state_snapshot(&payload),
            AgUiEvent::StateDelta { payload, .. } => Self::handle_agui_state_delta(&payload),
            AgUiEvent::MessagesSnapshot { payload, .. } => {
                Self::handle_agui_messages_snapshot(&payload)
            },
            AgUiEvent::Custom { payload, .. } => self.handle_agui_custom_payload(&payload),
        }
    }

    fn handle_agui_run_started(&mut self, payload: &RunStartedPayload) -> Vec<Command> {
        self.state
            .chat
            .on_progress_started(TaskId::new(payload.thread_id.to_string()));
        vec![Command::None]
    }

    fn handle_agui_run_finished(&mut self, _payload: &RunFinishedPayload) -> Vec<Command> {
        self.state.chat.on_progress_finished();
        vec![Command::None]
    }

    fn handle_agui_run_error(&mut self, payload: &RunErrorPayload) -> Vec<Command> {
        self.state.chat.on_progress_finished();
        tracing::error!("Run error: {}", payload.message);
        vec![Command::None]
    }

    fn handle_agui_step_started(&mut self, step_name: String) -> Vec<Command> {
        self.state.chat.add_execution_step(ExecutionStepDisplay {
            step_id: ExecutionStepId::generate(),
            status: StepStatusDisplay::InProgress,
            step_type: Some("step".to_string()),
            tool_name: None,
            content: Some(step_name),
            duration_ms: None,
        });
        vec![Command::None]
    }

    fn handle_agui_step_finished(_payload: &StepFinishedPayload) -> Vec<Command> {
        vec![Command::None]
    }

    fn handle_agui_text_message_start(_payload: &TextMessageStartPayload) -> Vec<Command> {
        vec![Command::None]
    }

    fn handle_agui_text_message_content(_payload: &TextMessageContentPayload) -> Vec<Command> {
        vec![Command::None]
    }

    fn handle_agui_text_message_end(_payload: &TextMessageEndPayload) -> Vec<Command> {
        vec![Command::None]
    }

    fn handle_agui_tool_call_start(
        &mut self,
        tool_call_id: AiToolCallId,
        tool_call_name: String,
    ) -> Vec<Command> {
        self.state.chat.add_inline_tool_call(InlineToolCall {
            id: tool_call_id,
            name: tool_call_name,
            arguments: serde_json::Value::String(String::new()),
            arguments_preview: String::new(),
            status: ToolCallStatus::Executing,
            result: None,
            result_preview: None,
        });
        vec![Command::None]
    }

    fn handle_agui_tool_call_args(
        &mut self,
        tool_call_id: &AiToolCallId,
        delta: &str,
    ) -> Vec<Command> {
        if let Some(tool) = self
            .state
            .chat
            .current_inline_tools
            .iter_mut()
            .find(|t| &t.id == tool_call_id)
        {
            tool.arguments_preview.push_str(delta);
        }
        vec![Command::None]
    }

    fn handle_agui_tool_call_result(
        &mut self,
        tool_call_id: &AiToolCallId,
        content: &serde_json::Value,
    ) -> Vec<Command> {
        let result_str = content
            .as_str()
            .map_or_else(|| content.to_string(), String::from);
        self.state
            .chat
            .update_inline_tool_result(tool_call_id.as_ref(), &result_str, false);
        vec![Command::None]
    }

    fn handle_agui_tool_call_end(_payload: &ToolCallEndPayload) -> Vec<Command> {
        vec![Command::None]
    }

    fn handle_agui_state_snapshot(_payload: &StateSnapshotPayload) -> Vec<Command> {
        vec![Command::None]
    }

    fn handle_agui_state_delta(_payload: &StateDeltaPayload) -> Vec<Command> {
        vec![Command::None]
    }

    fn handle_agui_messages_snapshot(_payload: &MessagesSnapshotPayload) -> Vec<Command> {
        vec![Command::None]
    }

    fn handle_agui_custom_payload(&mut self, payload: &CustomPayload) -> Vec<Command> {
        match payload {
            CustomPayload::Artifact(p) => self.handle_artifact_payload(p),
            CustomPayload::ExecutionStep(p) => Self::log_execution_step(p),
            CustomPayload::SkillLoaded(p) => Self::log_skill_loaded(p),
            CustomPayload::Generic(p) => self.handle_generic_custom_event(&p.name, &p.value),
        }
    }

    fn log_execution_step(
        payload: &systemprompt_models::agui::ExecutionStepCustomPayload,
    ) -> Vec<Command> {
        tracing::debug!(step_type = ?payload.step.step_type(), "Execution step");
        vec![Command::None]
    }

    fn log_skill_loaded(
        payload: &systemprompt_models::agui::SkillLoadedCustomPayload,
    ) -> Vec<Command> {
        tracing::debug!(skill_name = %payload.skill_name, "Skill loaded");
        vec![Command::None]
    }

    fn handle_artifact_payload(
        &mut self,
        payload: &systemprompt_models::agui::ArtifactCustomPayload,
    ) -> Vec<Command> {
        let artifact = &payload.artifact;
        self.state.artifacts.add_artifact(ArtifactDisplay {
            artifact_id: artifact.id.clone(),
            name: artifact.name.clone(),
            artifact_type: Some(artifact.metadata.artifact_type.clone()),
            task_id: payload.task_id.clone(),
            context_id: payload.context_id.clone(),
            created_at: Utc::now(),
        });
        vec![Command::None]
    }

    fn handle_generic_custom_event(
        &mut self,
        name: &str,
        value: &serde_json::Value,
    ) -> Vec<Command> {
        match name {
            "run_started" => self.handle_generic_run_started(value),
            "task_completed" => self.handle_generic_task_completed(value),
            _ => {
                tracing::debug!(name = %name, value = ?value, "Unhandled generic event");
                vec![Command::None]
            },
        }
    }

    fn handle_generic_run_started(&mut self, value: &serde_json::Value) -> Vec<Command> {
        if let Some(run_id) = value.get("runId").and_then(|v| v.as_str()) {
            self.state.chat.on_progress_started(TaskId::new(run_id));
        }
        vec![Command::None]
    }

    fn handle_generic_task_completed(&mut self, value: &serde_json::Value) -> Vec<Command> {
        self.state.chat.on_progress_finished();
        if let Some(task_value) = value.get("task") {
            if let Ok(task) =
                serde_json::from_value::<systemprompt_models::a2a::Task>(task_value.clone())
            {
                self.state.chat.upsert_task(task);
            }
        }
        vec![Command::None]
    }
}
