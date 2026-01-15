use anyhow::{anyhow, Context, Result};
use clap::Args;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::types::{HistoryMessage, TaskArtifact, TaskGetOutput};
use crate::shared::{resolve_input, CommandResult};
use crate::CliConfig;

const DEFAULT_GATEWAY_URL: &str = "http://localhost:8080";
const JSON_RPC_VERSION: &str = "2.0";

#[derive(Debug, Args)]
pub struct TaskArgs {
    #[arg(help = "Agent name that processed the task")]
    pub agent: Option<String>,

    #[arg(long, help = "Task ID to retrieve")]
    pub task_id: Option<String>,

    #[arg(long, help = "Number of history messages to retrieve")]
    pub history_length: Option<u32>,

    #[arg(long, help = "Gateway URL (default: http://localhost:8080)")]
    pub url: Option<String>,

    #[arg(
        long,
        env = "SYSTEMPROMPT_TOKEN",
        help = "Bearer token for authentication"
    )]
    pub token: Option<String>,

    #[arg(long, default_value = "30", help = "Timeout in seconds")]
    pub timeout: u64,
}

#[derive(Debug, Serialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    method: String,
    params: TaskGetParams,
    id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TaskGetParams {
    id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    history_length: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    #[serde(default)]
    result: Option<TaskResponse>,
    #[serde(default)]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcError {
    code: i32,
    message: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskResponse {
    id: String,
    context_id: String,
    status: TaskStatusResponse,
    #[serde(default)]
    history: Option<Vec<HistoryEntry>>,
    #[serde(default)]
    artifacts: Option<Vec<ArtifactEntry>>,
}

#[derive(Debug, Deserialize)]
struct TaskStatusResponse {
    state: String,
    #[serde(default)]
    timestamp: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HistoryEntry {
    role: String,
    parts: Vec<MessagePart>,
}

#[derive(Debug, Deserialize)]
struct MessagePart {
    kind: String,
    #[serde(default)]
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ArtifactEntry {
    #[serde(default)]
    name: Option<String>,
    parts: Vec<ArtifactPart>,
}

#[derive(Debug, Deserialize)]
struct ArtifactPart {
    kind: String,
    #[serde(default)]
    text: Option<String>,
}

pub async fn execute(args: TaskArgs, config: &CliConfig) -> Result<CommandResult<TaskGetOutput>> {
    let agent = resolve_input(args.agent, "agent", config, || {
        Err(anyhow!("Agent name is required"))
    })?;

    let task_id = resolve_input(args.task_id, "task-id", config, || {
        Err(anyhow!("Task ID is required. Use --task-id"))
    })?;

    let base_url = args.url.as_deref().unwrap_or(DEFAULT_GATEWAY_URL);
    let agent_url = format!("{}/api/v1/agents/{}", base_url.trim_end_matches('/'), agent);

    let request_id = uuid::Uuid::new_v4().to_string();

    // Per A2A spec Section 7.3: tasks/get only requires task ID
    // Context is resolved from task storage by the server
    let request = JsonRpcRequest {
        jsonrpc: JSON_RPC_VERSION.to_string(),
        method: "tasks/get".to_string(),
        params: TaskGetParams {
            id: task_id.clone(),
            history_length: args.history_length,
        },
        id: request_id,
    };

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(args.timeout))
        .build()
        .context("Failed to create HTTP client")?;

    let mut request_builder = client
        .post(&agent_url)
        .header("Content-Type", "application/json");

    if let Some(token) = &args.token {
        request_builder = request_builder.header("Authorization", format!("Bearer {}", token));
    }

    let response = request_builder
        .json(&request)
        .send()
        .await
        .with_context(|| format!("Failed to get task from agent at {}", agent_url))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("Agent request failed with status {}: {}", status, body);
    }

    let json_response: JsonRpcResponse = response
        .json()
        .await
        .context("Failed to parse agent response")?;

    if json_response.jsonrpc != JSON_RPC_VERSION {
        anyhow::bail!(
            "Invalid JSON-RPC version: expected {}, got {}",
            JSON_RPC_VERSION,
            json_response.jsonrpc
        );
    }

    if let Some(error) = json_response.error {
        anyhow::bail!("Agent returned error ({}): {}", error.code, error.message);
    }

    let task = json_response
        .result
        .ok_or_else(|| anyhow!("No result in agent response"))?;

    // Convert history entries to output format
    let history: Vec<HistoryMessage> = task
        .history
        .as_ref()
        .map(|entries| {
            entries
                .iter()
                .flat_map(|entry| {
                    let role = match entry.role.as_str() {
                        "user" => "User".to_string(),
                        "agent" => "Agent".to_string(),
                        other => other.to_string(),
                    };
                    entry
                        .parts
                        .iter()
                        .filter(|p| p.kind == "text")
                        .filter_map(|p| p.text.clone())
                        .map(move |text| HistoryMessage {
                            role: role.clone(),
                            text,
                        })
                })
                .collect()
        })
        .unwrap_or_default();

    // Convert artifacts to output format
    let artifacts: Vec<TaskArtifact> = task
        .artifacts
        .as_ref()
        .map(|arts| {
            arts.iter()
                .map(|artifact| {
                    let content = artifact
                        .parts
                        .iter()
                        .filter(|p| p.kind == "text")
                        .filter_map(|p| p.text.clone())
                        .collect::<Vec<_>>()
                        .join("\n");
                    TaskArtifact {
                        name: artifact.name.clone(),
                        content,
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    let output = TaskGetOutput {
        task_id: task.id,
        context_id: task.context_id,
        state: task.status.state,
        timestamp: task.status.timestamp,
        history,
        artifacts,
    };

    Ok(CommandResult::card(output).with_title("Task Details"))
}
