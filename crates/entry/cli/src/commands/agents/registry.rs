use anyhow::{Context, Result};
use clap::Args;
use reqwest::Client;
use serde::Deserialize;

use super::types::{RegistryAgentInfo, RegistryOutput};
use crate::shared::CommandResult;
use crate::CliConfig;

const DEFAULT_GATEWAY_URL: &str = "http://localhost:8080";

#[derive(Debug, Args)]
pub struct RegistryArgs {
    #[arg(long, help = "Gateway URL (default: http://localhost:8080)")]
    pub url: Option<String>,

    #[arg(long, help = "Show only running agents")]
    pub running: bool,

    #[arg(long, help = "Include full agent card details")]
    pub verbose: bool,
}

#[derive(Debug, Deserialize)]
struct RegistryResponse {
    data: Vec<AgentCardResponse>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentCardResponse {
    name: String,
    description: String,
    url: String,
    version: String,
    #[serde(default)]
    capabilities: CapabilitiesResponse,
    #[serde(default)]
    skills: Vec<SkillResponse>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CapabilitiesResponse {
    streaming: Option<bool>,
    #[serde(default)]
    extensions: Option<Vec<ExtensionResponse>>,
}

#[derive(Debug, Deserialize)]
struct ExtensionResponse {
    uri: String,
    #[serde(default)]
    params: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct SkillResponse {
    name: String,
}

pub async fn execute(
    args: RegistryArgs,
    _config: &CliConfig,
) -> Result<CommandResult<RegistryOutput>> {
    let base_url = args.url.as_deref().unwrap_or(DEFAULT_GATEWAY_URL);
    let registry_url = format!("{}/api/v1/agents/registry", base_url.trim_end_matches('/'));

    let client = Client::new();
    let response = client
        .get(&registry_url)
        .send()
        .await
        .with_context(|| format!("Failed to connect to gateway at {}", registry_url))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("Registry request failed with status {}: {}", status, body);
    }

    let registry: RegistryResponse = response
        .json()
        .await
        .context("Failed to parse registry response")?;

    let agents: Vec<RegistryAgentInfo> = registry
        .data
        .into_iter()
        .filter(|agent| {
            if args.running {
                is_agent_running(agent)
            } else {
                true
            }
        })
        .map(|agent| {
            let status = extract_status(&agent);
            let skills: Vec<String> = agent.skills.iter().map(|s| s.name.clone()).collect();

            RegistryAgentInfo {
                name: agent.name,
                description: if args.verbose {
                    agent.description
                } else {
                    truncate(&agent.description, 50)
                },
                url: agent.url,
                version: agent.version,
                status,
                streaming: agent.capabilities.streaming.unwrap_or(false),
                skills_count: skills.len(),
                skills: if args.verbose { skills } else { vec![] },
            }
        })
        .collect();

    let output = RegistryOutput {
        gateway_url: base_url.to_string(),
        agents_count: agents.len(),
        agents,
    };

    Ok(CommandResult::table(output)
        .with_title("Agent Registry")
        .with_columns(vec![
            "name".to_string(),
            "url".to_string(),
            "status".to_string(),
            "version".to_string(),
            "streaming".to_string(),
            "skills_count".to_string(),
        ]))
}

fn is_agent_running(agent: &AgentCardResponse) -> bool {
    agent.capabilities.extensions.as_ref().is_some_and(|exts| {
        exts.iter().any(|ext| {
            ext.uri == "systemprompt:service-status"
                && ext
                    .params
                    .as_ref()
                    .and_then(|p| p.get("status"))
                    .and_then(|s| s.as_str())
                    .is_some_and(|s| s == "running")
        })
    })
}

fn extract_status(agent: &AgentCardResponse) -> String {
    agent
        .capabilities
        .extensions
        .as_ref()
        .and_then(|exts| {
            exts.iter().find_map(|ext| {
                if ext.uri == "systemprompt:service-status" {
                    ext.params
                        .as_ref()
                        .and_then(|p| p.get("status"))
                        .and_then(|s| s.as_str())
                        .map(String::from)
                } else {
                    None
                }
            })
        })
        .unwrap_or_else(|| "unknown".to_string())
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}
