mod security;
mod skills;

use anyhow::{anyhow, Result};
use std::sync::Arc;
use systemprompt_loader::ConfigLoader;
use systemprompt_models::profile_bootstrap::ProfileBootstrap;
use systemprompt_models::{AgentConfig, ServicesConfig};
use tokio::sync::RwLock;

use crate::models::a2a::{
    AgentCapabilities, AgentCard, AgentExtension, AgentProvider, TransportProtocol,
};
use security::{convert_json_security_to_struct, oauth_to_security_config, override_oauth_urls};
use skills::load_skill_from_disk;
use std::path::Path;

#[derive(Clone, Debug)]
pub struct AgentRegistry {
    config: Arc<RwLock<ServicesConfig>>,
}

impl AgentRegistry {
    pub async fn new() -> Result<Self> {
        let config = ConfigLoader::load()?;
        Ok(Self {
            config: Arc::new(RwLock::new(config)),
        })
    }

    pub async fn get_agent(&self, name: &str) -> Result<AgentConfig> {
        let config = self.config.read().await;
        config
            .agents
            .get(name)
            .cloned()
            .ok_or_else(|| anyhow!("Agent not found: {}", name))
    }

    pub async fn list_agents(&self) -> Result<Vec<AgentConfig>> {
        let config = self.config.read().await;
        Ok(config.agents.values().cloned().collect())
    }

    pub async fn list_enabled_agents(&self) -> Result<Vec<AgentConfig>> {
        let config = self.config.read().await;
        let is_cloud = systemprompt_models::Config::get()
            .map(|c| c.is_cloud)
            .unwrap_or(false);
        Ok(config
            .agents
            .values()
            .filter(|a| a.enabled && !(a.dev_only && is_cloud))
            .cloned()
            .collect())
    }

    pub async fn get_default_agent(&self) -> Result<AgentConfig> {
        let config = self.config.read().await;
        let is_cloud = systemprompt_models::Config::get()
            .map(|c| c.is_cloud)
            .unwrap_or(false);
        config
            .agents
            .values()
            .find(|a| a.default && a.enabled && !(a.dev_only && is_cloud))
            .cloned()
            .ok_or_else(|| anyhow!("No default agent configured"))
    }

    pub async fn to_agent_card(
        &self,
        name: &str,
        api_external_url: &str,
        mcp_extensions: Vec<AgentExtension>,
        runtime_status: Option<(String, Option<u16>, Option<u32>)>,
    ) -> Result<AgentCard> {
        let agent = self.get_agent(name).await?;
        let url = agent.construct_url(api_external_url);

        let extensions = build_extensions(&agent, runtime_status, mcp_extensions);

        let (security_schemes, security) =
            if agent.card.security_schemes.is_some() || agent.card.security.is_some() {
                let (mut schemes, sec) = convert_json_security_to_struct(
                    agent.card.security_schemes.as_ref(),
                    agent.card.security.as_ref(),
                );
                if let Some(ref mut s) = schemes {
                    override_oauth_urls(s, api_external_url);
                }
                (schemes, sec)
            } else {
                oauth_to_security_config(&agent.oauth, api_external_url)
            };

        let all_skills = load_agent_skills(&agent);

        Ok(AgentCard {
            protocol_version: agent.card.protocol_version.clone(),
            name: agent.name.clone(),
            description: agent.card.description.clone(),
            url,
            version: agent.card.version.clone(),
            preferred_transport: Some(match agent.card.preferred_transport.as_str() {
                "JSONRPC" => TransportProtocol::JsonRpc,
                "GRPC" => TransportProtocol::Grpc,
                "HTTP+JSON" => TransportProtocol::HttpJson,
                _ => TransportProtocol::JsonRpc,
            }),
            additional_interfaces: None,
            icon_url: agent.card.icon_url.clone(),
            documentation_url: agent.card.documentation_url.clone(),
            provider: agent.card.provider.as_ref().map(|p| AgentProvider {
                organization: p.organization.clone(),
                url: p.url.clone(),
            }),
            capabilities: AgentCapabilities {
                streaming: Some(agent.card.capabilities.streaming),
                push_notifications: Some(agent.card.capabilities.push_notifications),
                state_transition_history: Some(agent.card.capabilities.state_transition_history),
                extensions: Some(extensions),
            },
            default_input_modes: agent.card.default_input_modes.clone(),
            default_output_modes: agent.card.default_output_modes.clone(),
            supports_authenticated_extended_card: Some(
                agent.card.supports_authenticated_extended_card,
            ),
            skills: all_skills,
            security_schemes,
            security,
            signatures: None,
        })
    }

    pub async fn reload(&self) -> Result<()> {
        let new_config = ConfigLoader::load()?;
        let mut config = self.config.write().await;
        *config = new_config;
        drop(config);
        Ok(())
    }

    pub async fn get_mcp_servers(&self, agent_name: &str) -> Result<Vec<String>> {
        let agent = self.get_agent(agent_name).await?;
        Ok(agent.metadata.mcp_servers)
    }

    pub async fn find_next_available_port(&self) -> Result<u16> {
        const BASE_PORT: u16 = 9000;
        const MAX_PORT: u16 = 9999;

        let agents = self.list_agents().await?;
        let used_ports: Vec<u16> = agents.iter().map(|a| a.port).collect();

        for port in BASE_PORT..=MAX_PORT {
            if !used_ports.contains(&port) {
                return Ok(port);
            }
        }

        Err(anyhow!(
            "No available ports in range {}-{}",
            BASE_PORT,
            MAX_PORT
        ))
    }
}

fn build_extensions(
    agent: &AgentConfig,
    runtime_status: Option<(String, Option<u16>, Option<u32>)>,
    mcp_extensions: Vec<AgentExtension>,
) -> Vec<AgentExtension> {
    let mut extensions = vec![AgentExtension::agent_identity(&agent.name)];

    if let Some(prompt) = &agent.metadata.system_prompt {
        extensions.push(AgentExtension::system_instructions(prompt));
    }

    if let Some((status, port, pid)) = &runtime_status {
        extensions.push(AgentExtension::service_status(
            status,
            *port,
            *pid,
            agent.default,
        ));
    }

    extensions.extend(mcp_extensions);
    extensions
}

fn load_agent_skills(agent: &AgentConfig) -> Vec<crate::models::a2a::AgentSkill> {
    let mut all_skills = Vec::new();

    let skills_path = ProfileBootstrap::get()
        .map(|p| p.paths.skills())
        .unwrap_or_else(|_| String::new());

    if !skills_path.is_empty() {
        let skills_dir = Path::new(&skills_path);
        for skill_id in &agent.metadata.skills {
            match load_skill_from_disk(skills_dir, skill_id) {
                Ok(skill) => all_skills.push(skill),
                Err(e) => {
                    tracing::warn!(
                        skill_id = %skill_id,
                        error = %e,
                        "Failed to load skill for agent card, skipping"
                    );
                },
            }
        }
    }

    all_skills
}
