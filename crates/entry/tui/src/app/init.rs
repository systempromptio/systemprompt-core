use std::path::PathBuf;

use tracing::{error, info};

use crate::config::TuiConfig;
use crate::services::cloud_api;
use crate::state::{AgentDisplayMetadata, AppState, SystemInstructionsSource, TuiModeInfo};
use systemprompt_identifiers::{ContextId, SessionToken};
use systemprompt_models::{AgentCard, AgentExtension, Profile};

use super::TuiApp;

impl TuiApp {
    pub(super) fn log_startup_info(
        cloud_api_url: &str,
        profile: &Profile,
        user_email: Option<&str>,
    ) {
        info!(
            mode = "CLOUD",
            profile = %profile.display_name,
            cloud_api = %cloud_api_url,
            local_api = %profile.server.api_external_url,
            user = user_email.unwrap_or("-"),
            "TUI starting"
        );
    }

    pub(super) fn init_state(
        config: &TuiConfig,
        cloud_api_url: String,
        user_email: Option<String>,
        tenant_id: Option<String>,
        profile: Profile,
    ) -> AppState {
        let mode_info = TuiModeInfo::Cloud {
            cloud_api_url,
            user_email,
            tenant_id,
            profile,
        };
        let mut state = AppState::new(config, mode_info);
        state.agents.set_loading(true);
        state
    }

    pub(super) fn update_init_progress(state: &mut AppState, step: &str, completed: usize) {
        state.init_status.current_step = step.to_string();
        state.init_status.steps_completed = completed;
    }

    pub(super) async fn load_agents(
        state: &mut AppState,
        api_external_url: &str,
        session_token: &SessionToken,
    ) -> Option<String> {
        let result = cloud_api::fetch_agents(api_external_url, session_token).await;
        state.agents.set_loading(false);

        match result {
            Ok(agents) => Self::apply_loaded_agents(state, agents),
            Err(e) => {
                error!(error = %e, "Failed to fetch agents");
                state
                    .agents
                    .set_error(Some(format!("Failed to load agents: {e}")));
                None
            },
        }
    }

    fn apply_loaded_agents(state: &mut AppState, agents: Vec<AgentCard>) -> Option<String> {
        info!(count = agents.len(), "Loaded agents from local API");
        state.agents.set_agents_with_cards(agents);
        state.agents.get_selected_agent().map(|agent| {
            info!(agent = %agent.name, "Auto-selected agent");
            agent.name.clone()
        })
    }

    pub(super) async fn load_chat_history(
        state: &mut AppState,
        api_external_url: &str,
        session_token: &SessionToken,
        context_id: &ContextId,
    ) {
        match cloud_api::fetch_tasks_by_context(
            api_external_url,
            session_token,
            context_id.as_str(),
        )
        .await
        {
            Ok(tasks) => {
                for task in tasks {
                    state.chat.upsert_task(task);
                }
                state.chat.needs_initial_load = false;
            },
            Err(e) => {
                error!("Failed to load chat history: {}", e);
            },
        }
    }

    pub(super) async fn load_conversations(
        state: &mut AppState,
        api_external_url: &str,
        session_token: &SessionToken,
    ) {
        match cloud_api::list_contexts(api_external_url, session_token).await {
            Ok(contexts) => Self::apply_loaded_conversations(state, contexts),
            Err(e) => error!(error = %e, "Failed to load conversations"),
        }
    }

    fn apply_loaded_conversations(
        state: &mut AppState,
        contexts: Vec<systemprompt_models::UserContextWithStats>,
    ) {
        use crate::state::ConversationDisplay;

        info!(count = contexts.len(), "Loaded conversations");
        let conversations: Vec<ConversationDisplay> = contexts
            .into_iter()
            .map(|c| ConversationDisplay {
                context_id: c.context_id,
                name: c.name,
                task_count: c.task_count,
                message_count: c.message_count,
                last_message_at: c.last_message_at,
                updated_at: Some(c.updated_at),
            })
            .collect();
        state.conversations.update_conversations(conversations);
    }

    pub(super) async fn load_artifacts(
        state: &mut AppState,
        api_external_url: &str,
        session_token: &SessionToken,
    ) {
        let result =
            cloud_api::list_all_artifacts(api_external_url, session_token, Some(100)).await;
        Self::apply_artifacts_result(state, result);
    }

    fn apply_artifacts_result(
        state: &mut AppState,
        result: anyhow::Result<Vec<systemprompt_models::A2aArtifact>>,
    ) {
        let artifacts = match result {
            Ok(a) => a,
            Err(e) => return error!(error = %e, "Failed to load artifacts"),
        };
        info!(count = artifacts.len(), "Loaded artifacts");
        state.artifacts.load_artifacts(artifacts);
    }

    pub(super) fn populate_agent_display_metadata(state: &mut AppState) {
        let profile = state.mode_info.profile();
        let skills_path = profile.paths.skills();
        let config_path = profile.paths.config();

        let agent_names: Vec<String> = state.agents.agent_cards.keys().cloned().collect();

        for agent_name in agent_names {
            let metadata = state
                .agents
                .agent_cards
                .get(&agent_name)
                .map(|card| {
                    Self::build_agent_metadata(card, Some(&skills_path), Some(&config_path))
                })
                .unwrap_or_default();

            state
                .agents
                .set_agent_display_metadata(&agent_name, metadata);
        }
    }

    fn build_agent_metadata(
        card: &AgentCard,
        skills_path: Option<&str>,
        config_path: Option<&str>,
    ) -> AgentDisplayMetadata {
        let mut metadata = AgentDisplayMetadata::default();

        Self::populate_skill_paths(&mut metadata, card, skills_path);
        Self::populate_extension_metadata(&mut metadata, card, config_path);

        metadata
    }

    fn populate_skill_paths(
        metadata: &mut AgentDisplayMetadata,
        card: &AgentCard,
        skills_path: Option<&str>,
    ) {
        let Some(skills_base) = skills_path else {
            return;
        };

        for skill in &card.skills {
            let skill_file = PathBuf::from(skills_base).join(&skill.id).join("index.md");
            metadata.skill_paths.insert(skill.id.clone(), skill_file);
        }
    }

    fn populate_extension_metadata(
        metadata: &mut AgentDisplayMetadata,
        card: &AgentCard,
        config_path: Option<&str>,
    ) {
        let Some(config) = config_path else {
            return;
        };
        let Some(extensions) = &card.capabilities.extensions else {
            return;
        };

        Self::extract_mcp_servers(metadata, extensions, config);
        Self::extract_system_instructions(metadata, extensions);
    }

    fn extract_mcp_servers(
        metadata: &mut AgentDisplayMetadata,
        extensions: &[AgentExtension],
        config: &str,
    ) {
        let Some(mcp_ext) = extensions
            .iter()
            .find(|e| e.uri == "systemprompt:mcp-tools")
        else {
            return;
        };
        let Some(params) = &mcp_ext.params else {
            return;
        };
        let Some(servers) = params.get("servers").and_then(|s| s.as_array()) else {
            return;
        };

        for server in servers {
            if let Some(name) = server.get("name").and_then(|n| n.as_str()) {
                metadata
                    .mcp_server_paths
                    .insert(name.to_string(), config.to_string());
            }
        }
    }

    fn extract_system_instructions(
        metadata: &mut AgentDisplayMetadata,
        extensions: &[AgentExtension],
    ) {
        let Some(instr_ext) = extensions
            .iter()
            .find(|e| e.uri == "systemprompt:system-instructions")
        else {
            return;
        };
        let Some(params) = &instr_ext.params else {
            return;
        };

        metadata.system_instructions_source = params
            .get("source")
            .and_then(|s| s.as_str())
            .map_or(SystemInstructionsSource::Inline, |source| {
                SystemInstructionsSource::FilePath(PathBuf::from(source))
            });
    }
}
