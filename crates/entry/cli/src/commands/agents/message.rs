use anyhow::{anyhow, Context, Result};
use clap::Args;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{ContextId, MessageId, TaskId};
use systemprompt_models::a2a::{Message, Part, Task, TextPart};

use super::types::MessageOutput;
use crate::session::get_or_create_session;
use crate::shared::{resolve_input, CommandResult};
use crate::CliConfig;

const JSON_RPC_VERSION: &str = "2.0";

#[derive(Debug, Args)]
pub struct MessageArgs {
    #[arg(help = "Agent name to send message to (required in non-interactive mode)")]
    pub agent: Option<String>,

    #[arg(short = 'm', long, help = "Message text to send")]
    pub message: Option<String>,

    #[arg(
        long,
        help = "Context ID for conversation continuity (overrides session)"
    )]
    pub context_id: Option<String>,

    #[arg(long, help = "Task ID to continue an existing task")]
    pub task_id: Option<String>,

    #[arg(long, help = "Gateway URL (default: http://localhost:8080)")]
    pub url: Option<String>,

    #[arg(long, help = "Use streaming mode")]
    pub stream: bool,

    #[arg(long, help = "Wait for task completion (blocking mode)")]
    pub blocking: bool,

    #[arg(
        long,
        default_value = "30",
        help = "Timeout in seconds for blocking mode"
    )]
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
    message: Message,
    #[serde(skip_serializing_if = "Option::is_none")]
    configuration: Option<MessageConfiguration>,
}

#[derive(Debug, Serialize)]
struct MessageConfiguration {
    blocking: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    #[serde(default)]
    result: Option<Task>,
    #[serde(default)]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcError {
    code: i32,
    message: String,
}

fn extract_text_from_parts(parts: &[Part]) -> String {
    parts
        .iter()
        .filter_map(|part| match part {
            Part::Text(text_part) => Some(text_part.text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub async fn execute(
    args: MessageArgs,
    config: &CliConfig,
) -> Result<CommandResult<MessageOutput>> {
    let session_ctx = get_or_create_session(config).await?;

    let agent = resolve_input(args.agent, "agent", config, || {
        Err(anyhow!("Agent name is required"))
    })?;

    let message_text = resolve_input(args.message, "message", config, || {
        Err(anyhow!("Message text is required. Use -m or --message"))
    })?;

    let base_url = args
        .url
        .as_deref()
        .unwrap_or(&session_ctx.profile.server.api_external_url);
    let agent_url = format!("{}/api/v1/agents/{}", base_url.trim_end_matches('/'), agent);

    let context_id: ContextId = args
        .context_id
        .map(ContextId::new)
        .unwrap_or_else(|| session_ctx.context_id().clone());
    let auth_token = session_ctx.session_token().as_str();

    let task_id: Option<TaskId> = args.task_id.map(TaskId::new);

    let message_id = MessageId::generate();
    let request_id = MessageId::generate().to_string();

    let method = if args.stream {
        "message/stream"
    } else {
        "message/send"
    };

    let request = JsonRpcRequest {
        jsonrpc: JSON_RPC_VERSION.to_string(),
        method: method.to_string(),
        params: MessageSendParams {
            message: Message {
                role: "user".to_string(),
                parts: vec![Part::Text(TextPart {
                    text: message_text.clone(),
                })],
                id: message_id,
                task_id,
                context_id: context_id.clone(),
                kind: "message".to_string(),
                metadata: None,
                extensions: None,
                reference_task_ids: None,
            },
            configuration: args.blocking.then_some(MessageConfiguration {
                blocking: Some(true),
            }),
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
        .header("Authorization", format!("Bearer {}", auth_token))
        .json(&request)
        .send()
        .await
        .with_context(|| format!("Failed to send message to agent at {}", agent_url))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_else(|_| String::new());
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

    let response = task
        .status
        .message
        .as_ref()
        .map(|msg| extract_text_from_parts(&msg.parts));

    let output = MessageOutput {
        agent: agent.clone(),
        task,
        message_sent: message_text,
        response,
    };

    Ok(CommandResult::card(output).with_title(format!("Message sent to {}", agent)))
}
