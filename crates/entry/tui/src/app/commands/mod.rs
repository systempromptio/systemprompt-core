mod conversation;
mod sync;

use anyhow::Result;
use systemprompt_identifiers::{ContextId, TaskId};

use crate::messages::{Command, Message};
use crate::services::{cloud_api, CliExecutor};

use super::TuiApp;

impl TuiApp {
    pub(crate) async fn execute_command(&mut self, command: Command) -> Result<()> {
        if self.handle_cloud_restricted_command(&command) {
            return Ok(());
        }

        match command {
            Command::Batch(commands) => self.execute_batch(commands).await?,
            Command::SendAiMessage(content) => self.handle_send_ai_message(content).await,
            Command::CancelAiStream => self.state.chat.cancel_task(),
            Command::CreateNewContext => self.create_new_context().await,
            Command::ExecuteTool(_) => self.send_output("\nTool execution handled by cloud API\n"),
            Command::RefreshServices => {
                let _ = self.message_tx.send(Message::ServiceRefresh);
            },
            Command::RefreshUsers => {
                let _ = self.message_tx.send(Message::UsersRefresh);
            },
            Command::RefreshConversations => self.spawn_refresh_conversations(),
            Command::SelectConversation(id) => self.select_conversation(id).await,
            Command::RenameConversation { context_id, name } => {
                self.spawn_rename_conversation(context_id, name);
            },
            Command::DeleteConversation(id) => {
                self.spawn_delete_conversation(ContextId::new(&id)).await;
            },
            Command::CreateConversation(name) => self.spawn_create_conversation(name),
            Command::AgentList | Command::AgentStatus => self.spawn_agent_list(),
            Command::AgentsDiscover => self.spawn_agents_discover(),
            Command::AgentA2aSelect(name) => {
                let _ = self.message_tx.send(Message::AgentSelect(name));
            },
            Command::ShowConfig => self.spawn_show_config(),
            Command::Sync(sub) => self.spawn_sync(sub),
            Command::Quit => self.state.should_quit = true,
            Command::DeleteTask(task_id) => self.spawn_delete_task(TaskId::new(&task_id)),
            Command::ExecuteCli(cmd_string) => self.spawn_cli_command(cmd_string),
            Command::RequestAiCommandParams {
                command_path,
                description,
            } => self.handle_ai_command_request(&command_path, &description),
            _ => {},
        }
        Ok(())
    }

    fn handle_cloud_restricted_command(&self, command: &Command) -> bool {
        let is_restricted = matches!(
            command,
            Command::StartService(_)
                | Command::StopService(_)
                | Command::RestartService(_)
                | Command::ExecuteDbQuery(_)
                | Command::UpdateUserRole { .. }
                | Command::AgentEnable(_)
                | Command::AgentDisable(_)
                | Command::AgentRestart(_)
                | Command::AgentHealth(_)
                | Command::AgentCleanup
                | Command::McpList
                | Command::McpStart(_)
                | Command::McpStop(_)
                | Command::McpStatus
                | Command::McpRestart(_)
                | Command::DbTables
                | Command::DbInfo
                | Command::DbDescribe(_)
                | Command::RunCleanup
                | Command::ShowSkills
                | Command::WebBuild
                | Command::WebServe
                | Command::DeleteArtifact(_)
                | Command::RefreshArtifacts
        );

        if is_restricted {
            self.not_available_in_cloud_mode();
        }
        is_restricted
    }

    async fn execute_batch(&mut self, commands: Vec<Command>) -> Result<()> {
        for nested_command in commands {
            Box::pin(self.execute_command(nested_command)).await?;
        }
        Ok(())
    }

    async fn handle_send_ai_message(&self, content: String) {
        let Some(ref agent_name) = self.current_agent_name else {
            tracing::warn!("No agent selected, cannot send message");
            let _ = self.message_tx.send(Message::TaskProgressError(
                "No agent selected. Use /agents list to see available agents.".to_string(),
            ));
            return;
        };

        let sender = self.message_sender.clone();
        let agent = agent_name.clone();
        let context_id = self.current_context_id.read().await.clone();
        let message_tx = self.message_tx.clone();

        tokio::spawn(async move {
            if let Err(e) = sender.send(&agent, &context_id, &content).await {
                tracing::error!("Failed to send message: {}", e);
                let _ = message_tx.send(Message::TaskProgressError(format!(
                    "Failed to send message: {e}"
                )));
            }
        });
    }

    fn send_output(&self, msg: &str) {
        let _ = self
            .message_tx
            .send(Message::CommandOutput(msg.to_string()));
    }

    fn not_available_in_cloud_mode(&self) {
        let _ = self.message_tx.send(Message::CommandOutput(
            "\nThis command is not available in cloud mode.\n".to_string(),
        ));
    }

    async fn create_new_context(&mut self) {
        match cloud_api::create_context(&self.api_external_url, &self.session_token).await {
            Ok(context_id) => self.apply_new_context(context_id).await,
            Err(ref e) => self.report_context_creation_error(e),
        }
    }

    async fn apply_new_context(&mut self, context_id: ContextId) {
        *self.current_context_id.write().await = context_id.clone();
        self.state.chat.set_context(context_id.clone());
        tracing::info!("Created new context: {}", context_id);
    }

    fn report_context_creation_error(&self, e: &anyhow::Error) {
        tracing::error!("Failed to create new context: {}", e);
        let _ = self.message_tx.send(Message::CommandOutput(format!(
            "\nFailed to create new context: {e}\n"
        )));
    }

    fn spawn_agent_list(&self) {
        let api_url = self.api_external_url.clone();
        let token = self.session_token.clone();
        let sender = self.message_tx.clone();
        tokio::spawn(async move {
            match cloud_api::fetch_agents(&api_url, &token).await {
                Ok(agents) => {
                    let mut output = String::from("=== Agents ===\n");
                    for agent in &agents {
                        output.push_str(&format!("  {} - {}\n", agent.name, agent.description));
                    }
                    if agents.is_empty() {
                        output.push_str("  No agents available\n");
                    }
                    let _ = sender.send(Message::CommandOutput(format!("\n{}", output)));
                },
                Err(e) => {
                    let error = format!("Error listing agents: {}", e);
                    let _ = sender.send(Message::CommandError(error.clone()));
                    let _ = sender.send(Message::CommandOutput(format!("\n{}\n", error)));
                },
            }
        });
    }

    fn spawn_agents_discover(&self) {
        let api_url = self.api_external_url.clone();
        let token = self.session_token.clone();
        let sender = self.message_tx.clone();
        tokio::spawn(async move {
            match cloud_api::fetch_agents(&api_url, &token).await {
                Ok(agents) => {
                    tracing::info!("Discovered {} agents from cloud API", agents.len());
                    let _ = sender.send(Message::AgentsUpdate(agents));
                },
                Err(e) => {
                    tracing::error!("Failed to discover agents: {}", e);
                    let _ = sender.send(Message::AgentsError(format!(
                        "Failed to discover agents: {}",
                        e
                    )));
                },
            }
        });
    }

    fn spawn_show_config(&self) {
        let sender = self.message_tx.clone();
        let mode_info = self.state.mode_info.clone();
        tokio::spawn(async move {
            let mut output = String::from("=== Configuration (Cloud Mode) ===\n");
            output.push_str(&format!("Mode: {}\n", mode_info.display_name()));
            output.push_str(&format!("Cloud API: {}\n", mode_info.cloud_api_url()));
            output.push_str(&format!("Local API: {}\n", mode_info.api_external_url()));
            if let Some(user) = mode_info.user_display() {
                output.push_str(&format!("User: {}\n", user));
            }
            let _ = sender.send(Message::CommandOutput(format!("\n{}", output)));
        });
    }

    fn spawn_delete_task(&self, task_id: TaskId) {
        let api_url = self.api_external_url.clone();
        let token = self.session_token.clone();
        let sender = self.message_tx.clone();
        tokio::spawn(async move {
            match cloud_api::delete_task(&api_url, &token, task_id.as_str()).await {
                Ok(()) => {
                    tracing::info!("Deleted task: {}", task_id.as_ref());
                    let _ = sender.send(Message::TaskDeleted(task_id.to_string()));
                },
                Err(e) => {
                    tracing::error!("Failed to delete task: {}", e);
                    let _ = sender.send(Message::CommandError(format!(
                        "Failed to delete task: {}",
                        e
                    )));
                },
            }
        });
    }

    fn spawn_cli_command(&self, cmd_string: String) {
        let executor = CliExecutor::new(self.message_tx.clone());
        executor.spawn_execution(cmd_string);
    }

    fn handle_ai_command_request(&mut self, command_path: &[String], description: &str) {
        use crate::state::ActiveTab;

        let prompt = format!(
            "I want to run the CLI command: systemprompt {}\nDescription: {}\n\nPlease help me \
             determine the appropriate parameters for this command.",
            command_path.join(" "),
            description
        );

        self.state.chat.input_buffer = prompt;
        self.state.chat.cursor_position = self.state.chat.input_buffer.len();
        let _ = self.message_tx.send(Message::SwitchTab(ActiveTab::Chat));
    }
}
