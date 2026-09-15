//! Agent registry: a snapshot of configured agents loaded from services
//! config, with lookups and [`AgentCard`] assembly (security schemes, skills,
//! runtime extensions).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod security;
pub mod skills;

use std::sync::Arc;
use systemprompt_config::ProfileBootstrap;
use systemprompt_loader::{ConfigLoader, ServicesRootBootstrap};
use systemprompt_models::{AgentConfig, ServicesConfig};

use crate::error::{AgentError, AgentResult};

use crate::models::a2a::{
    AgentCapabilities, AgentCard, AgentExtension, AgentInterface, AgentProvider, TransportProtocol,
};
use security::{oauth_to_security_config, override_oauth_urls};
use skills::load_skill_from_disk;
use std::path::Path;

#[derive(Clone, Debug)]
pub struct AgentRegistry {
    config: Arc<ServicesConfig>,
}

impl AgentRegistry {
    pub fn new() -> AgentResult<Self> {
        let config = ConfigLoader::load()?;
        Ok(Self::from_config(config))
    }

    #[must_use]
    pub fn from_config(config: ServicesConfig) -> Self {
        Self {
            config: Arc::new(config),
        }
    }

    #[expect(
        clippy::unused_async,
        clippy::unused_async_trait_impl,
        reason = "async signature required by AgentRegistryProvider trait impl in \
                  registry_provider.rs"
    )]
    pub async fn get_agent(&self, name: &str) -> AgentResult<AgentConfig> {
        self.config
            .agents
            .get(name)
            .cloned()
            .ok_or_else(|| AgentError::NotFound(name.to_owned()))
    }

    #[expect(
        clippy::unused_async,
        clippy::unused_async_trait_impl,
        reason = "async signature required by AgentRegistryProvider trait impl in \
                  registry_provider.rs"
    )]
    pub async fn list_agents(&self) -> AgentResult<Vec<AgentConfig>> {
        Ok(self.config.agents.values().cloned().collect())
    }

    #[expect(
        clippy::unused_async,
        clippy::unused_async_trait_impl,
        reason = "async signature required by AgentRegistryProvider trait impl in \
                  registry_provider.rs"
    )]
    pub async fn list_enabled_agents(&self) -> AgentResult<Vec<AgentConfig>> {
        let is_cloud = systemprompt_models::Config::get().is_ok_and(|c| c.is_cloud);
        Ok(self
            .config
            .agents
            .values()
            .filter(|a| a.enabled && !(a.dev_only && is_cloud))
            .cloned()
            .collect())
    }

    #[expect(
        clippy::unused_async,
        clippy::unused_async_trait_impl,
        reason = "async signature required by AgentRegistryProvider trait impl in \
                  registry_provider.rs"
    )]
    pub async fn get_default_agent(&self) -> AgentResult<AgentConfig> {
        let is_cloud = systemprompt_models::Config::get().is_ok_and(|c| c.is_cloud);
        self.config
            .agents
            .values()
            .find(|a| a.default && a.enabled && !(a.dev_only && is_cloud))
            .cloned()
            .ok_or_else(|| AgentError::NotFound("default agent not configured".to_owned()))
    }

    pub async fn to_agent_card(
        &self,
        name: &str,
        api_external_url: &str,
        mcp_extensions: Vec<AgentExtension>,
        runtime_status: Option<(String, Option<u16>, Option<u32>)>,
    ) -> AgentResult<AgentCard> {
        let agent = self.get_agent(name).await?;
        let url = agent.construct_url(api_external_url);

        let extensions = build_extensions(&agent, runtime_status.as_ref(), mcp_extensions);

        let (security_schemes, security) =
            if agent.card.security_schemes.is_some() || agent.card.security.is_some() {
                let mut schemes = agent.card.security_schemes.clone();
                if let Some(ref mut s) = schemes {
                    override_oauth_urls(s, api_external_url);
                }
                (schemes, agent.card.security.clone())
            } else {
                oauth_to_security_config(&agent.oauth, api_external_url)
            };

        let all_skills = load_agent_skills(&agent)?;

        let protocol_binding = match agent.card.preferred_transport.as_str() {
            "GRPC" => TransportProtocol::Grpc,
            "HTTP+JSON" => TransportProtocol::HttpJson,
            _ => TransportProtocol::JsonRpc,
        };

        Ok(AgentCard {
            name: agent.name.clone(),
            description: agent.card.description.clone(),
            supported_interfaces: vec![AgentInterface {
                url,
                protocol_binding,
                protocol_version: agent.card.protocol_version.clone(),
            }],
            version: agent.card.version.clone(),
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

    pub async fn get_mcp_servers(&self, agent_name: &str) -> AgentResult<Vec<String>> {
        let agent = self.get_agent(agent_name).await?;
        Ok(agent.metadata.mcp_servers.include)
    }

    pub async fn find_next_available_port(&self) -> AgentResult<u16> {
        const BASE_PORT: u16 = 9000;
        const MAX_PORT: u16 = 9999;

        let agents = self.list_agents().await?;
        let used_ports: Vec<u16> = agents.iter().map(|a| a.port).collect();

        for port in BASE_PORT..=MAX_PORT {
            if !used_ports.contains(&port) {
                return Ok(port);
            }
        }

        Err(AgentError::Validation(format!(
            "No available ports in range {BASE_PORT}-{MAX_PORT}"
        )))
    }
}

fn build_extensions(
    agent: &AgentConfig,
    runtime_status: Option<&(String, Option<u16>, Option<u32>)>,
    mcp_extensions: Vec<AgentExtension>,
) -> Vec<AgentExtension> {
    let mut extensions = vec![AgentExtension::agent_identity(&agent.name)];

    if let Some(prompt) = &agent.metadata.system_prompt {
        extensions.push(AgentExtension::system_instructions(prompt));
    }

    if let Some((status, port, pid)) = runtime_status {
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

fn load_agent_skills(agent: &AgentConfig) -> AgentResult<Vec<crate::models::a2a::AgentSkill>> {
    let profile = ProfileBootstrap::get().map_err(|e| AgentError::Config(e.to_string()))?;
    let skills_path = ServicesRootBootstrap::active_path_or(&profile.paths.services, "skills");
    load_agent_skills_from_dir(agent, &skills_path)
}

// Why: the card advertises every skill in `metadata.skills.include`; a skill
// that cannot be loaded is an agent configuration error, not a skill to omit.
pub fn load_agent_skills_from_dir(
    agent: &AgentConfig,
    skills_dir: &Path,
) -> AgentResult<Vec<crate::models::a2a::AgentSkill>> {
    agent
        .metadata
        .skills
        .include
        .iter()
        .map(|skill_id| {
            let skill_id_typed = systemprompt_identifiers::SkillId::new(skill_id);
            load_skill_from_disk(skills_dir, &skill_id_typed).map_err(|e| {
                AgentError::Config(format!(
                    "agent {} advertises skill {skill_id} which failed to load: {e}",
                    agent.name
                ))
            })
        })
        .collect()
}
