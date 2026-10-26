//! Agent registry: a snapshot of configured agents loaded from services
//! config, with lookups and [`AgentCard`] assembly (security schemes, skills,
//! runtime extensions).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod security;
pub mod skills;

use std::sync::Arc;
use systemprompt_identifiers::{AgentName, McpServerId};
use systemprompt_loader::ServicesRootBootstrap;
use systemprompt_manifest::services::ServicesView;
use systemprompt_manifest::{AgentConfig, ServicesConfig};

use crate::error::{AgentError, AgentResult};

use crate::models::a2a::{
    AgentCapabilities, AgentCard, AgentExtension, AgentInterface, AgentProvider, ProtocolBinding,
};
use security::{oauth_to_security_config, override_oauth_urls, requirements_from_config};
use skills::load_skill_from_disk;
use std::path::Path;

#[derive(Clone, Debug)]
pub struct AgentRegistry {
    config: Arc<ServicesConfig>,
}

impl AgentRegistry {
    #[must_use]
    pub fn new(services: &ServicesView) -> Self {
        Self {
            config: services.config(),
        }
    }

    #[must_use]
    pub fn from_config(config: ServicesConfig) -> Self {
        Self {
            config: Arc::new(config),
        }
    }

    pub fn get_agent(&self, name: &str) -> AgentResult<AgentConfig> {
        self.config
            .agents
            .get(name)
            .cloned()
            .ok_or_else(|| AgentError::NotFound(name.to_owned()))
    }

    pub fn list_agents(&self) -> Vec<AgentConfig> {
        self.config.agents.values().cloned().collect()
    }

    pub fn list_enabled_agents(&self) -> AgentResult<Vec<AgentConfig>> {
        let is_cloud = is_cloud_deployment()?;
        Ok(self
            .config
            .agents
            .values()
            .filter(|a| a.enabled && !(a.dev_only && is_cloud))
            .cloned()
            .collect())
    }

    pub fn get_default_agent(&self) -> AgentResult<AgentConfig> {
        let is_cloud = is_cloud_deployment()?;
        self.config
            .agents
            .values()
            .find(|a| a.default && a.enabled && !(a.dev_only && is_cloud))
            .cloned()
            .ok_or_else(|| AgentError::NotFound("default agent not configured".to_owned()))
    }

    pub fn to_agent_card(
        &self,
        name: &str,
        api_external_url: &str,
        mcp_extensions: Vec<AgentExtension>,
        runtime_status: Option<&(String, Option<u16>, Option<u32>)>,
    ) -> AgentResult<AgentCard> {
        let agent = self.get_agent(name)?;
        let url = agent.construct_url(api_external_url);

        let extensions = build_extensions(&agent, runtime_status, mcp_extensions);

        let (security_schemes, security_requirements) = if agent.card.security_schemes.is_some()
            || agent.card.security_requirements.is_some()
        {
            let mut schemes = agent.card.security_schemes.clone();
            if let Some(ref mut s) = schemes {
                override_oauth_urls(s, api_external_url);
            }
            (
                schemes,
                agent
                    .card
                    .security_requirements
                    .as_deref()
                    .map(requirements_from_config),
            )
        } else {
            oauth_to_security_config(&agent.oauth, api_external_url)
        };

        let all_skills = load_agent_skills(&agent)?;

        let protocol_binding = match agent.card.preferred_transport.as_str() {
            "GRPC" => ProtocolBinding::Grpc,
            "HTTP+JSON" => ProtocolBinding::HttpJson,
            _ => ProtocolBinding::JsonRpc,
        };

        Ok(AgentCard {
            name: agent.name.clone(),
            description: agent.card.description.clone(),
            supported_interfaces: vec![AgentInterface {
                url,
                protocol_binding,
                protocol_version: systemprompt_models::a2a::PROTOCOL_VERSION.to_owned(),
                tenant: None,
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
                extensions: Some(extensions),
                extended_agent_card: Some(agent.card.capabilities.extended_agent_card),
            },
            default_input_modes: agent.card.default_input_modes.clone(),
            default_output_modes: agent.card.default_output_modes,
            skills: all_skills,
            security_schemes,
            security_requirements,
            signatures: None,
        })
    }

    pub fn get_mcp_servers(&self, agent_name: &AgentName) -> AgentResult<Vec<McpServerId>> {
        let agent = self.get_agent(agent_name.as_str())?;
        Ok(agent
            .metadata
            .mcp_servers
            .include
            .into_iter()
            .map(McpServerId::new)
            .collect())
    }

    pub fn find_next_available_port(&self) -> AgentResult<u16> {
        const BASE_PORT: u16 = 9000;
        const MAX_PORT: u16 = 9999;

        let agents = self.list_agents();
        let used_ports: Vec<u16> = agents.iter().map(|a| a.port).collect();

        for port in BASE_PORT..=MAX_PORT {
            if !used_ports.contains(&port) {
                return Ok(port);
            }
        }

        Err(AgentError::NoAvailablePort {
            min: BASE_PORT,
            max: MAX_PORT,
        })
    }
}

fn is_cloud_deployment() -> AgentResult<bool> {
    systemprompt_manifest::Config::get()
        .map(|config| config.is_cloud)
        .map_err(|e| AgentError::invalid_config("cannot resolve dev_only agents", e))
}

fn build_extensions(
    agent: &AgentConfig,
    runtime_status: Option<&(String, Option<u16>, Option<u32>)>,
    mcp_extensions: Vec<AgentExtension>,
) -> Vec<AgentExtension> {
    let mut extensions = vec![AgentExtension::agent_identity(&AgentName::new(&agent.name))];

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
    let skills_path = ServicesRootBootstrap::require()?.path.join("skills");
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
                AgentError::invalid_config(
                    format!(
                        "agent {} advertises skill {skill_id} which failed to load",
                        agent.name
                    ),
                    e,
                )
            })
        })
        .collect()
}
