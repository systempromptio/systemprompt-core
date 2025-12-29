use crate::messages::{Command, ContextLifecycleEvent, ContextStreamTaskEvent, ScrollDirection};
use systemprompt_identifiers::TaskId;

use super::super::TuiApp;

impl TuiApp {
    pub(crate) fn configure_selected_agent(&mut self) -> Vec<Command> {
        if let Some(agent) = self.state.agents.get_selected_agent() {
            self.current_agent_name = Some(agent.name.clone());
            tracing::info!("Selected agent: {}", agent.name);
        }
        vec![Command::None]
    }

    pub(crate) fn handle_service_status_update(
        &mut self,
        services: Vec<crate::state::ServiceStatus>,
    ) -> Vec<Command> {
        self.state.services.update_services(services);
        vec![Command::None]
    }

    pub(crate) fn handle_service_refresh() -> Vec<Command> {
        vec![Command::RefreshServices]
    }

    pub(crate) fn handle_service_select(&mut self, index: usize) -> Vec<Command> {
        self.state.services.select_service_by_index(index);
        vec![Command::None]
    }

    pub(crate) fn handle_service_action(action: crate::messages::ServiceAction) -> Vec<Command> {
        match action {
            crate::messages::ServiceAction::Start(name) => vec![Command::StartService(name)],
            crate::messages::ServiceAction::Stop(name) => vec![Command::StopService(name)],
            crate::messages::ServiceAction::Restart(name) => vec![Command::RestartService(name)],
        }
    }

    pub(crate) fn handle_users_update(
        &mut self,
        users: Vec<crate::state::UserDisplay>,
    ) -> Vec<Command> {
        self.state.users.update_users(users);
        vec![Command::None]
    }

    pub(crate) fn handle_users_select(&mut self, index: usize) -> Vec<Command> {
        self.state.users.selected_index = index;
        vec![Command::None]
    }

    pub(crate) fn handle_conversations_update(
        &mut self,
        conversations: Vec<crate::state::ConversationDisplay>,
    ) -> Vec<Command> {
        self.state.conversations.update_conversations(conversations);
        vec![Command::None]
    }

    pub(crate) fn handle_analytics_update(
        &mut self,
        data: crate::state::AnalyticsData,
    ) -> Vec<Command> {
        self.state.analytics.update(data);
        vec![Command::None]
    }

    pub(crate) fn handle_analytics_scroll(&mut self, direction: ScrollDirection) -> Vec<Command> {
        match direction {
            ScrollDirection::Up => self.state.analytics.scroll_up(),
            ScrollDirection::Down => self.state.analytics.scroll_down(),
            _ => {},
        }
        vec![Command::None]
    }

    pub(crate) fn handle_analytics_next_view(&mut self) -> Vec<Command> {
        self.state.analytics.next_view();
        vec![Command::None]
    }

    pub(crate) fn handle_analytics_prev_view(&mut self) -> Vec<Command> {
        self.state.analytics.prev_view();
        vec![Command::None]
    }

    pub(crate) fn handle_log_entry(&mut self, entry: crate::messages::LogEntry) -> Vec<Command> {
        self.state.logs.add_entry(entry);
        vec![Command::None]
    }

    pub(crate) fn handle_logs_batch(
        &mut self,
        entries: Vec<crate::messages::LogEntry>,
    ) -> Vec<Command> {
        self.state.logs.add_entries(entries);
        vec![Command::None]
    }

    pub(crate) fn handle_logs_toggle_follow(&mut self) -> Vec<Command> {
        self.state.logs.toggle_follow();
        vec![Command::None]
    }

    pub(crate) fn handle_logs_set_filter(
        &mut self,
        level: Option<crate::messages::LogLevel>,
    ) -> Vec<Command> {
        self.state.logs.set_level_filter(level);
        vec![Command::None]
    }

    pub(crate) fn handle_logs_clear(&mut self) -> Vec<Command> {
        self.state.logs.clear();
        vec![Command::None]
    }

    pub(crate) fn handle_command_output(&mut self, output: String) -> Vec<Command> {
        self.state.commands.set_output(output);
        vec![Command::None]
    }

    pub(crate) fn handle_command_error(&mut self, error: &str) -> Vec<Command> {
        self.state.commands.set_error(error);
        vec![Command::None]
    }

    pub(crate) fn handle_command_executing(&mut self) -> Vec<Command> {
        self.state.commands.is_executing = true;
        self.state.commands.output = Some("Executing...".to_string());
        vec![Command::None]
    }

    pub(crate) fn handle_agents_refresh(&mut self) -> Vec<Command> {
        self.state.agents.set_loading(true);
        self.state.agents.set_error(None);
        vec![Command::AgentsDiscover]
    }

    pub(crate) fn handle_agents_loading(&mut self, loading: bool) -> Vec<Command> {
        self.state.agents.set_loading(loading);
        vec![Command::None]
    }

    pub(crate) fn handle_agents_update(
        &mut self,
        cards: Vec<systemprompt_models::AgentCard>,
    ) -> Vec<Command> {
        self.state.agents.set_loading(false);
        self.state.agents.set_error(None);
        self.state.agents.set_agents_with_cards(cards);
        self.configure_selected_agent()
    }

    pub(crate) fn handle_agents_error(&mut self, error: String) -> Vec<Command> {
        self.state.agents.set_loading(false);
        self.state.agents.set_error(Some(error));
        vec![Command::None]
    }

    pub(crate) fn handle_agent_select(&mut self, name: &str) -> Vec<Command> {
        if self.state.agents.select_agent(name) {
            self.state.chat.clear();
            self.configure_selected_agent()
        } else {
            vec![Command::None]
        }
    }

    pub(crate) fn handle_agent_select_next(&mut self) -> Vec<Command> {
        self.state.agents.select_next();
        self.state.chat.clear();
        self.configure_selected_agent()
    }

    pub(crate) fn handle_agent_select_previous(&mut self) -> Vec<Command> {
        self.state.agents.select_previous();
        self.state.chat.clear();
        self.configure_selected_agent()
    }

    pub(crate) fn handle_sse_status_update(
        &mut self,
        status: crate::state::SseStatus,
    ) -> Vec<Command> {
        self.state.sse_status = status;
        vec![Command::None]
    }

    pub(crate) fn handle_context_stream_task(
        &mut self,
        event: ContextStreamTaskEvent,
    ) -> Vec<Command> {
        match event {
            ContextStreamTaskEvent::Created(task) => {
                self.state.chat.on_task_created(task);
            },
            ContextStreamTaskEvent::StatusChanged(task)
            | ContextStreamTaskEvent::Completed(task) => {
                self.state.chat.upsert_task(task);
            },
        }
        vec![Command::None]
    }

    pub(crate) fn handle_context_lifecycle(event: &ContextLifecycleEvent) -> Vec<Command> {
        tracing::debug!(event = ?event, "Context lifecycle");
        vec![Command::None]
    }

    pub(crate) fn handle_context_snapshot(&mut self) -> Vec<Command> {
        self.state.chat.needs_initial_load = false;
        vec![Command::None]
    }

    pub(crate) fn handle_task_progress_started(&mut self, task_id: TaskId) -> Vec<Command> {
        self.state.chat.on_progress_started(task_id);
        vec![Command::None]
    }

    pub(crate) fn handle_task_progress_finished(&mut self) -> Vec<Command> {
        self.state.chat.on_progress_finished();
        vec![Command::None]
    }

    pub(crate) fn handle_task_progress_error(&mut self, error: &str) -> Vec<Command> {
        tracing::error!("Task progress error: {}", error);
        self.state.chat.create_failed_task(error);
        vec![Command::None]
    }

    pub(crate) fn handle_task_deleted(&mut self, task_id: &str) -> Vec<Command> {
        self.state.chat.remove_task(task_id);
        vec![Command::None]
    }
}
