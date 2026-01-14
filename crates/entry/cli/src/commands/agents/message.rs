use anyhow::{anyhow, Context, Result};
use clap::Args;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use super::types::{MessageOutput, TaskInfo};
use crate::shared::{resolve_input, CommandResult};
use crate::CliConfig;

const DEFAULT_GATEWAY_URL: &str = "http://localhost:8080";

#[derive(Debug, Args)]
pub struct MessageArgs {
    #[arg(help = "Agent name to send message to (required in non-interactive mode)")]
    pub agent: Option<String>,

    #[arg(short = 'm', long, help = "Message text to send")]
    pub message: Option<String>,

    #[arg(long, help = "Context ID for conversation continuity")]
    pub context_id: Option<String>,

    #[arg(long, help = "Task ID to continue an existing task")]
    pub task_id: Option<String>,

    #[arg(long, help = "Gateway URL (default: http://localhost:8080)")]
    pub url: Option<String>,

    #[arg(long, help = "Use streaming mode")]
    pub stream: bool,

    #[arg(long, help = "Wait for task completion (blocking mode)")]
    pub blocking: bool,

    #[arg(long, default_value = "30", help = "Timeout in seconds for blocking mode")]
    pub timeout: u64,
}

#[derive(Debug, Serialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    method: String,
    params: MessageSendParams,
    id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MessageSendParams {
    message: A2aMessage,
    #[serde(skip_serializing_if = "Option::is_none")]
    configuration: Option<MessageConfiguration>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct A2aMessage {
    role: String,
    parts: Vec<MessagePart>,
    message_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    task_id: Option<String>,
    context_id: String,
    kind: String,
}

#[derive(Debug, Serialize)]
struct MessagePart {
    kind: String,
    text: String,
}

#[derive(Debug, Serialize)]
struct MessageConfiguration {
    blocking: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    #[serde(default)]
    result: Option<TaskResponse>,
    #[serde(default)]
    error: Option<JsonRpcError>,
    id: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct JsonRpcError {
    code: i32,
    message: String,
    #[serde(default)]
    data: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskResponse {
    id: String,
    context_id: String,
    status: TaskStatusResponse,
    #[serde(default)]
    artifacts: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Deserialize)]
struct TaskStatusResponse {
    state: String,
    #[serde(default)]
    message: Option<serde_json::Value>,
    #[serde(default)]
    timestamp: Option<String>,
}

pub async fn execute(args: MessageArgs, config: &CliConfig) -> Result<CommandResult<MessageOutput>> {
    let agent = resolve_input(args.agent, "agent", config, || {
        Err(anyhow!("Agent name is required"))
    })?;

    let message_text = resolve_input(args.message, "message", config, || {
        Err(anyhow!("Message text is required. Use -m or --message"))
    })?;

    let base_url = args.url.as_deref().unwrap_or(DEFAULT_GATEWAY_URL);
    let agent_url = format!(
        "{}/api/v1/agents/{}",
        base_url.trim_end_matches('/'),
        agent
    );

    let context_id = args
        .context_id
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let message_id = Uuid::new_v4().to_string();
    let request_id = Uuid::new_v4().to_string();

    let method = if args.stream {
        "message/stream"
    } else {
        "message/send"
    };

    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: method.to_string(),
        params: MessageSendParams {
            message: A2aMessage {
                role: "user".to_string(),
                parts: vec![MessagePart {
                    kind: "text".to_string(),
                    text: message_text.clone(),
                }],
                message_id,
                task_id: args.task_id.clone(),
                context_id: context_id.clone(),
                kind: "message".to_string(),
            },
            configuration: if args.blocking {
                Some(MessageConfiguration {
                    blocking: Some(true),
                })
            } else {
                None
            },
        },
        id: request_id,
    };

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(args.timeout))
        .build()
        .context("Failed to create HTTP client")?;

    let response = client
        .post(&agent_url)
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await
        .with_context(|| format!("Failed to send message to agent at {}", agent_url))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!(
            "Agent request failed with status {}: {}",
            status,
            body
        );
    }

    let json_response: JsonRpcResponse = response
        .json()
        .await
        .context("Failed to parse agent response")?;

    if let Some(error) = json_response.error {
        anyhow::bail!(
            "Agent returned error ({}): {}",
            error.code,
            error.message
        );
    }

    let task = json_response
        .result
        .ok_or_else(|| anyhow!("No result in agent response"))?;

    let artifacts_count = task.artifacts.as_ref().map(|a| a.len()).unwrap_or(0);

    let output = MessageOutput {
        agent: agent.clone(),
        task: TaskInfo {
            task_id: task.id,
            context_id: task.context_id,
            state: task.status.state,
            timestamp: task.status.timestamp,
        },
        message_sent: message_text,
        artifacts_count,
    };

    Ok(CommandResult::card(output).with_title(format!("Message sent to {}", agent)))
}
