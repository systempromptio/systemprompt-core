use anyhow::{anyhow, Context, Result};
use clap::Args;
use reqwest::Client;
use systemprompt_agent::models::a2a::jsonrpc::{
    JsonRpcResponse, Request, RequestId, JSON_RPC_VERSION_2_0,
};
use systemprompt_agent::models::a2a::protocol::TaskQueryParams;
use systemprompt_models::a2a::Task;

use crate::interactive::resolve_required;
use crate::session::get_or_create_session;
use crate::shared::CommandResult;
use crate::CliConfig;

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

pub async fn execute(args: TaskArgs, config: &CliConfig) -> Result<CommandResult<Task>> {
    let session_ctx = get_or_create_session(config).await?;

    let agent = resolve_required(args.agent, "agent", config, || {
        Err(anyhow!("Agent name is required"))
    })?;

    let task_id = resolve_required(args.task_id, "task-id", config, || {
        Err(anyhow!("Task ID is required. Use --task-id"))
    })?;

    let base_url = args.url.as_deref().unwrap_or_else(|| session_ctx.api_url());
    let agent_url = format!("{}/api/v1/agents/{}", base_url.trim_end_matches('/'), agent);

    let auth_token = args
        .token
        .as_deref()
        .unwrap_or_else(|| session_ctx.session_token().as_str());

    let request_id = RequestId::String(uuid::Uuid::new_v4().to_string());

    let request = Request {
        jsonrpc: JSON_RPC_VERSION_2_0.to_string(),
        method: "tasks/get".to_string(),
        params: TaskQueryParams {
            id: task_id.clone(),
            history_length: args.history_length,
        },
        id: request_id,
    };

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(args.timeout))
        .build()
        .context("Failed to create HTTP client")?;

    let request_builder = client
        .post(&agent_url)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", auth_token));

    let response = request_builder
        .json(&request)
        .send()
        .await
        .with_context(|| format!("Failed to get task from agent at {}", agent_url))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_else(|_| String::new());
        anyhow::bail!("Agent request failed with status {}: {}", status, body);
    }

    let json_response: JsonRpcResponse<Task> = response
        .json()
        .await
        .context("Failed to parse agent response")?;

    if json_response.jsonrpc != JSON_RPC_VERSION_2_0 {
        anyhow::bail!(
            "Invalid JSON-RPC version: expected {}, got {}",
            JSON_RPC_VERSION_2_0,
            json_response.jsonrpc
        );
    }

    if let Some(error) = json_response.error {
        let details = error
            .data
            .map_or_else(String::new, |d| format!("\n\nDetails: {}", d));
        anyhow::bail!(
            "Agent returned error ({}): {}{}",
            error.code,
            error.message,
            details
        );
    }

    let task = json_response
        .result
        .ok_or_else(|| anyhow!("No result in agent response"))?;

    Ok(CommandResult::card(task).with_title("Task Details"))
}
