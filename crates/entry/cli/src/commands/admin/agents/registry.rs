//! Agent-registry HTTP client for admin commands.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use clap::Args;
use reqwest::Client;
use serde::Deserialize;
use systemprompt_config::ProfileBootstrap;

use super::types::{RegistryAgentInfo, RegistryOutput};
use crate::CliConfig;
use crate::shared::{CommandOutput, truncate_with_ellipsis};

const FALLBACK_GATEWAY_URL: &str = "http://localhost:8080";

#[derive(Debug, Args)]
pub struct RegistryArgs {
    #[arg(
        long,
        help = "Gateway URL (default: active profile's api_external_url)"
    )]
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
pub struct AgentCardResponse {
    name: String,
    description: String,
    #[serde(default)]
    supported_interfaces: Vec<AgentInterfaceResponse>,
    version: String,
    #[serde(default)]
    capabilities: CapabilitiesResponse,
    #[serde(default)]
    skills: Vec<SkillResponse>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentInterfaceResponse {
    url: String,
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

pub(super) async fn execute(args: RegistryArgs, _config: &CliConfig) -> Result<CommandOutput> {
    let profile_url = ProfileBootstrap::get()
        .ok()
        .map(|p| p.server.api_external_url.clone());
    let base_url = args
        .url
        .clone()
        .or(profile_url)
        .unwrap_or_else(|| FALLBACK_GATEWAY_URL.to_owned());
    let registry_url = format!("{}/api/v1/agents/registry", base_url.trim_end_matches('/'));

    let registry = fetch_registry(&registry_url).await?;

    let agents: Vec<RegistryAgentInfo> = registry
        .data
        .into_iter()
        .filter(|agent| !args.running || is_agent_running(agent))
        .map(|agent| to_agent_info(agent, args.verbose))
        .collect();

    let output = RegistryOutput {
        gateway_url: base_url.clone(),
        agents_count: agents.len(),
        agents,
    };

    Ok(CommandOutput::table_of(
        vec![
            "name",
            "url",
            "status",
            "version",
            "streaming",
            "skills_count",
        ],
        &output.agents,
    )
    .with_title("Agent Registry"))
}

async fn fetch_registry(registry_url: &str) -> Result<RegistryResponse> {
    let client = Client::new();
    let response = client
        .get(registry_url)
        .send()
        .await
        .with_context(|| format!("Failed to connect to gateway at {}", registry_url))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_else(|e| {
            tracing::warn!(error = %e, "Failed to read error response body");
            String::new()
        });
        anyhow::bail!("Registry request failed with status {}: {}", status, body);
    }

    response
        .json()
        .await
        .context("Failed to parse registry response")
}

pub fn to_agent_info(agent: AgentCardResponse, verbose: bool) -> RegistryAgentInfo {
    let status = extract_status(&agent);
    let skills: Vec<String> = agent.skills.iter().map(|s| s.name.clone()).collect();

    let url = agent
        .supported_interfaces
        .first()
        .map_or_else(String::new, |i| i.url.clone());

    RegistryAgentInfo {
        name: agent.name,
        description: if verbose {
            agent.description
        } else {
            truncate_with_ellipsis(&agent.description, 50)
        },
        url,
        version: agent.version,
        status,
        streaming: agent.capabilities.streaming.unwrap_or(false),
        skills_count: skills.len(),
        skills: if verbose { skills } else { vec![] },
    }
}

pub fn is_agent_running(agent: &AgentCardResponse) -> bool {
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

pub fn extract_status(agent: &AgentCardResponse) -> String {
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
        .unwrap_or_else(|| "unknown".to_owned())
}
