use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use systemprompt_loader::ConfigLoader;
use systemprompt_models::profile_bootstrap::ProfileBootstrap;
use systemprompt_models::{AgentConfig, AgentOAuthConfig, ServicesConfig};
use tokio::sync::RwLock;

use crate::models::a2a::{
    AgentCapabilities, AgentCard, AgentExtension, AgentProvider, AgentSkill, OAuth2Flow,
    OAuth2Flows, SecurityScheme, TransportProtocol,
};

fn convert_json_security_to_struct(
    security_schemes: Option<&serde_json::Value>,
    security: Option<&Vec<serde_json::Value>>,
) -> (
    Option<HashMap<String, SecurityScheme>>,
    Option<Vec<HashMap<String, Vec<String>>>>,
) {
    let schemes = security_schemes.and_then(|schemes_json| {
        serde_json::from_value::<HashMap<String, SecurityScheme>>(schemes_json.clone()).ok()
    });

    let security_reqs = security.and_then(|sec_vec| {
        let reqs: Result<Vec<HashMap<String, Vec<String>>>, _> = sec_vec
            .iter()
            .map(|v| serde_json::from_value::<HashMap<String, Vec<String>>>(v.clone()))
            .collect();
        reqs.ok()
    });

    (schemes, security_reqs)
}

fn oauth_to_security_config(
    oauth: &AgentOAuthConfig,
    api_external_url: &str,
) -> (
    Option<HashMap<String, SecurityScheme>>,
    Option<Vec<HashMap<String, Vec<String>>>>,
) {
    if !oauth.required {
        return (None, None);
    }

    let flows = OAuth2Flows {
        authorization_code: Some(OAuth2Flow {
            authorization_url: Some(format!("{}/api/v1/core/oauth/authorize", api_external_url)),
            token_url: Some(format!("{}/api/v1/core/oauth/token", api_external_url)),
            refresh_url: Some(format!("{}/api/v1/core/oauth/token", api_external_url)),
            scopes: oauth
                .scopes
                .iter()
                .map(|s| (s.to_string(), format!("{} access", s)))
                .collect(),
        }),
        implicit: None,
        password: None,
        client_credentials: None,
    };

    let scheme = SecurityScheme::OAuth2 {
        flows: Box::new(flows),
        description: Some(format!(
            "OAuth 2.0 authentication for {} audience",
            oauth.audience
        )),
    };

    let mut schemes = HashMap::new();
    schemes.insert("oauth2".to_string(), scheme);

    let mut requirement = HashMap::new();
    requirement.insert(
        "oauth2".to_string(),
        oauth.scopes.iter().map(|s| s.to_string()).collect(),
    );
    let requirements = vec![requirement];

    (Some(schemes), Some(requirements))
}

fn override_oauth_urls(schemes: &mut HashMap<String, SecurityScheme>, api_external_url: &str) {
    if let Some(SecurityScheme::OAuth2 { flows, .. }) = schemes.get_mut("oauth2") {
        if let Some(auth_code) = flows.authorization_code.as_mut() {
            auth_code.authorization_url = auth_code.authorization_url.as_ref().map(|url| {
                if url.starts_with('/') {
                    format!("{api_external_url}{url}")
                } else {
                    url.clone()
                }
            });

            auth_code.token_url = auth_code.token_url.as_ref().map(|url| {
                if url.starts_with('/') {
                    format!("{api_external_url}{url}")
                } else {
                    url.clone()
                }
            });

            auth_code.refresh_url = auth_code.refresh_url.as_ref().map(|url| {
                if url.starts_with('/') {
                    format!("{api_external_url}{url}")
                } else {
                    url.clone()
                }
            });
        }
    }
}

fn load_skill_from_disk(skills_path: &Path, skill_id: &str) -> Result<AgentSkill> {
    let skill_dir = skills_path.join(skill_id);
    let index_path = skill_dir.join("index.md");

    if !index_path.exists() {
        anyhow::bail!(
            "Skill directory or index.md not found: {}",
            index_path.display()
        );
    }

    let content = fs::read_to_string(&index_path)?;
    let frontmatter = parse_skill_frontmatter(&content)?;

    Ok(AgentSkill {
        id: skill_id.to_string(),
        name: frontmatter.title.unwrap_or_else(|| skill_id.to_string()),
        description: frontmatter.description.unwrap_or_else(String::new),
        tags: frontmatter.keywords.unwrap_or_else(Vec::new),
        examples: frontmatter.examples,
        input_modes: frontmatter.input_modes,
        output_modes: frontmatter.output_modes,
        security: None,
    })
}

#[derive(Debug, Default)]
struct SkillFrontmatter {
    title: Option<String>,
    description: Option<String>,
    keywords: Option<Vec<String>>,
    examples: Option<Vec<String>>,
    input_modes: Option<Vec<String>>,
    output_modes: Option<Vec<String>>,
}

fn parse_skill_frontmatter(content: &str) -> Result<SkillFrontmatter> {
    if !content.starts_with("---") {
        return Ok(SkillFrontmatter::default());
    }

    let content_after_start = &content[3..];
    let yaml_content = match content_after_start.find("\n---") {
        Some(pos) => &content_after_start[..pos],
        None => return Ok(SkillFrontmatter::default()),
    };

    let yaml: serde_yaml::Value = serde_yaml::from_str(yaml_content)
        .map_err(|e| anyhow!("Failed to parse skill frontmatter: {}", e))?;

    let title = yaml.get("title").and_then(|v| v.as_str()).map(String::from);
    let description = yaml
        .get("description")
        .and_then(|v| v.as_str())
        .map(String::from);

    let keywords = yaml.get("keywords").and_then(|v| {
        v.as_str()
            .map(|s| {
                s.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .or_else(|| {
                v.as_sequence().map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
            })
    });

    let examples = yaml.get("examples").and_then(|v| {
        v.as_sequence().map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
    });

    let input_modes = yaml.get("input_modes").and_then(|v| {
        v.as_sequence().map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
    });

    let output_modes = yaml.get("output_modes").and_then(|v| {
        v.as_sequence().map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
    });

    Ok(SkillFrontmatter {
        title,
        description,
        keywords,
        examples,
        input_modes,
        output_modes,
    })
}

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
