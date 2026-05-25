use anyhow::{Result, anyhow};
use clap::Args;
use systemprompt_agent::models::a2a::jsonrpc::{JSON_RPC_VERSION_2_0, Request, RequestId};
use systemprompt_agent::models::a2a::protocol::TaskQueryParams;
use systemprompt_identifiers::TaskId;
use systemprompt_models::a2a::{Task, methods};

use super::client::{A2aCall, ensure_agent_exists, send_a2a_request};
use crate::CliConfig;
use crate::interactive::resolve_required;
use crate::session::get_or_create_session;
use crate::shared::CommandResult;

#[derive(Debug, Args)]
pub struct TaskArgs {
    #[arg(help = "Agent name that processed the task")]
    pub agent: Option<String>,

    #[arg(long = "task-id", help = "Task ID to retrieve")]
    pub task: Option<String>,

    #[arg(long, help = "Number of history messages to retrieve")]
    pub history_length: Option<u32>,

    #[arg(long, help = "Gateway URL (overrides profile's api_external_url)")]
    pub url: Option<String>,

    #[arg(
        long,
        help = "Bearer token override (defaults to the active CLI session token)"
    )]
    pub token: Option<String>,

    #[arg(long, default_value = "30", help = "Timeout in seconds")]
    pub timeout: u64,
}

pub(super) async fn execute(args: TaskArgs, config: &CliConfig) -> Result<CommandResult<Task>> {
    let session_ctx = get_or_create_session(config).await?;

    let agent = resolve_required(args.agent, "agent", config, || {
        Err(anyhow!("Agent name is required"))
    })?;

    ensure_agent_exists(&agent)?;

    let task_id = resolve_required(args.task, "task-id", config, || {
        Err(anyhow!("Task ID is required. Use --task-id"))
    })?;

    let base_url = args.url.as_deref().unwrap_or_else(|| session_ctx.api_url());
    let agent_url = format!("{}/api/v1/agents/{}", base_url.trim_end_matches('/'), agent);

    let auth_token = args
        .token
        .as_deref()
        .unwrap_or_else(|| session_ctx.session_token().as_str());

    let request = Request {
        jsonrpc: JSON_RPC_VERSION_2_0.to_string(),
        method: methods::GET_TASK.to_string(),
        params: TaskQueryParams {
            id: TaskId::new(task_id.clone()),
            history_length: args.history_length,
        },
        id: RequestId::String(uuid::Uuid::new_v4().to_string()),
    };

    let task: Task = send_a2a_request(A2aCall {
        agent: &agent,
        agent_url: &agent_url,
        auth_token,
        request: &request,
        timeout: args.timeout,
    })
    .await?;

    Ok(CommandResult::card(task).with_title("Task Details"))
}
