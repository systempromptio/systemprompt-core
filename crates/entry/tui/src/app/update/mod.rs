mod chat;
mod domain;
mod navigation;
mod tools;

use crate::events::handle_key_event;
use crate::messages::{Command, Message, MessageDomain};

use super::TuiApp;

impl TuiApp {
    pub(crate) fn update(&mut self, message: Message) -> Vec<Command> {
        match message.domain() {
            MessageDomain::Input => self.handle_input(&message),
            MessageDomain::Navigation => self.handle_navigation(message),
            MessageDomain::Chat => self.handle_chat(message),
            MessageDomain::Services => self.handle_services(message),
            MessageDomain::Users => self.handle_users(message),
            MessageDomain::Conversations => self.handle_conversations(message),
            MessageDomain::Analytics => self.handle_analytics(message),
            MessageDomain::Logs => self.handle_logs(message),
            MessageDomain::Commands => self.handle_commands(message),
            MessageDomain::Tools => self.handle_tools(message),
            MessageDomain::Agents => self.handle_agents(message),
            MessageDomain::Context => self.handle_context(message),
            MessageDomain::Artifacts => self.handle_artifacts(message),
            MessageDomain::System => self.handle_system(&message),
        }
    }

    fn handle_input(&mut self, message: &Message) -> Vec<Command> {
        match message {
            Message::Key(key) => {
                let (msg, command) = handle_key_event(*key, &mut self.state, &self.config);
                let mut commands = vec![command];
                if let Some(m) = msg {
                    commands.extend(self.update(m));
                }
                commands
            },
            Message::Mouse(mouse) => self.handle_mouse(*mouse),
            Message::Tick => {
                if self.state.chat.check_pending_timeout() {
                    tracing::error!("Message send timed out - stream may have failed silently");
                }
                vec![Command::None]
            },
            _ => vec![Command::None],
        }
    }

    fn handle_navigation(&mut self, message: Message) -> Vec<Command> {
        match message {
            Message::FocusPanel(panel) => self.handle_focus_panel(panel),
            Message::SwitchTab(tab) => self.handle_switch_tab(tab),
            Message::ToggleLogs => self.handle_toggle_logs(),
            Message::ToggleSidebar => self.handle_toggle_sidebar(),
            Message::SlashCommand(command) => self.handle_slash_command(command),
            _ => vec![Command::None],
        }
    }

    fn handle_chat(&mut self, message: Message) -> Vec<Command> {
        match message {
            Message::ChatInputChanged(input) => self.handle_chat_input_changed(input),
            Message::ChatSend => self.handle_chat_send_message(),
            Message::ChatCancelStream => self.handle_chat_cancel_stream(),
            Message::ChatClearConversation => self.handle_chat_clear_conversation(),
            Message::ChatScroll(direction) => self.handle_chat_scroll(direction),
            Message::ChatTaskCloseDetail => self.handle_chat_task_close_detail(),
            Message::ChatTaskDelete => self.handle_chat_task_delete(),
            Message::AiToolCallReceived(tool_call) => self.handle_ai_tool_call_received(*tool_call),
            Message::AgUiEvent(event) => self.handle_agui_event(event),
            _ => vec![Command::None],
        }
    }

    fn handle_services(&mut self, message: Message) -> Vec<Command> {
        match message {
            Message::ServiceStatusUpdate(services) => self.handle_service_status_update(services),
            Message::ServiceRefresh => Self::handle_service_refresh(),
            Message::ServiceSelect(index) => self.handle_service_select(index),
            Message::ServiceAction(action) => Self::handle_service_action(action),
            _ => vec![Command::None],
        }
    }

    fn handle_users(&mut self, message: Message) -> Vec<Command> {
        match message {
            Message::UsersRefresh => vec![Command::RefreshUsers],
            Message::UsersUpdate(users) => self.handle_users_update(users),
            Message::UsersSelect(index) => self.handle_users_select(index),
            _ => vec![Command::None],
        }
    }

    fn handle_conversations(&mut self, message: Message) -> Vec<Command> {
        match message {
            Message::ConversationsRefresh => vec![Command::RefreshConversations],
            Message::ConversationsUpdate(conversations) => {
                self.handle_conversations_update(conversations)
            },
            _ => vec![Command::None],
        }
    }

    fn handle_analytics(&mut self, message: Message) -> Vec<Command> {
        match message {
            Message::AnalyticsUpdate(data) => self.handle_analytics_update(data),
            Message::AnalyticsScroll(direction) => self.handle_analytics_scroll(direction),
            Message::AnalyticsNextView => self.handle_analytics_next_view(),
            Message::AnalyticsPrevView => self.handle_analytics_prev_view(),
            _ => vec![Command::None],
        }
    }

    fn handle_logs(&mut self, message: Message) -> Vec<Command> {
        match message {
            Message::LogEntry(entry) => self.handle_log_entry(entry),
            Message::LogsBatch(entries) => self.handle_logs_batch(entries),
            Message::LogsToggleFollow => self.handle_logs_toggle_follow(),
            Message::LogsSetFilter(level) => self.handle_logs_set_filter(level),
            Message::LogsClear => self.handle_logs_clear(),
            _ => vec![Command::None],
        }
    }

    fn handle_commands(&mut self, message: Message) -> Vec<Command> {
        match message {
            Message::CommandOutput(output) => self.handle_command_output(output),
            Message::CommandError(error) => self.handle_command_error(&error),
            Message::CommandExecuting => self.handle_command_executing(),
            _ => vec![Command::None],
        }
    }

    fn handle_tools(&mut self, message: Message) -> Vec<Command> {
        match message {
            Message::ToolApprove(id) => self.handle_tool_approve(id),
            Message::ToolReject(id) => self.handle_tool_reject(id),
            Message::ToolExecutionComplete(id, result) => {
                self.handle_tool_execution_complete(id, result)
            },
            _ => vec![Command::None],
        }
    }

    fn handle_agents(&mut self, message: Message) -> Vec<Command> {
        match message {
            Message::AgentsRefresh => self.handle_agents_refresh(),
            Message::AgentsLoading(loading) => self.handle_agents_loading(loading),
            Message::AgentsUpdate(cards) => self.handle_agents_update(cards),
            Message::AgentsError(error) => self.handle_agents_error(error),
            Message::AgentSelect(name) => self.handle_agent_select(&name),
            Message::AgentSelectNext => self.handle_agent_select_next(),
            Message::AgentSelectPrevious => self.handle_agent_select_previous(),
            _ => vec![Command::None],
        }
    }

    fn handle_context(&mut self, message: Message) -> Vec<Command> {
        match message {
            Message::SseStatusUpdate(status) => self.handle_sse_status_update(status),
            Message::ContextStreamTask(event) => self.handle_context_stream_task(*event),
            Message::ContextLifecycle(ref event) => Self::handle_context_lifecycle(event),
            Message::ContextSnapshot(_) => self.handle_context_snapshot(),
            Message::TaskProgressStarted { task_id } => self.handle_task_progress_started(task_id),
            Message::TaskProgressFinished => self.handle_task_progress_finished(),
            Message::TaskProgressError(error) => self.handle_task_progress_error(&error),
            Message::TaskDeleted(task_id) => self.handle_task_deleted(&task_id),
            _ => vec![Command::None],
        }
    }

    fn handle_artifacts(&mut self, message: Message) -> Vec<Command> {
        match message {
            Message::ArtifactsSelect(index) => self.handle_artifacts_select(index),
            Message::ArtifactsScroll(direction) => self.handle_artifacts_scroll(direction),
            Message::ArtifactsSelectNext => self.handle_artifacts_select_next(),
            Message::ArtifactsSelectPrevious => self.handle_artifacts_select_previous(),
            Message::ArtifactsLoaded(artifacts) => self.handle_artifacts_loaded(artifacts),
            Message::ArtifactDeleted(artifact_id) => self.handle_artifact_deleted(&artifact_id),
            Message::ArtifactsRefresh => vec![Command::RefreshArtifacts],
            _ => vec![Command::None],
        }
    }

    fn handle_system(&mut self, message: &Message) -> Vec<Command> {
        match message {
            Message::Quit => {
                self.state.should_quit = true;
                vec![Command::Quit]
            },
            _ => vec![Command::None],
        }
    }

    fn handle_mouse(&mut self, mouse: crossterm::event::MouseEvent) -> Vec<Command> {
        use crate::state::ActiveTab;
        use crossterm::event::{MouseButton, MouseEventKind};

        if mouse.kind == MouseEventKind::Down(MouseButton::Left) && mouse.row == 1 {
            let tab_positions = [
                (0, 6, ActiveTab::Chat),
                (9, 17, ActiveTab::Agents),
                (20, 26, ActiveTab::Users),
                (29, 39, ActiveTab::Analytics),
                (42, 52, ActiveTab::Services),
                (55, 63, ActiveTab::Config),
                (66, 77, ActiveTab::Shortcuts),
            ];

            for (start, end, tab) in tab_positions {
                if mouse.column >= start && mouse.column < end {
                    self.state.active_tab = tab;
                    break;
                }
            }
        }
        vec![Command::None]
    }
}
