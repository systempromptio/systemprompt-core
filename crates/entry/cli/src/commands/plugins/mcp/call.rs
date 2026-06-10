use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use clap::Args;
use dialoguer::Select;
use dialoguer::theme::ColorfulTheme;
use systemprompt_loader::ConfigLoader;
use systemprompt_mcp::services::McpOrchestrator;
use systemprompt_models::ai::tools::CallToolResult;
use systemprompt_runtime::AppContext;

use super::call_client::{
    ToolCallParams, convert_content, execute_tool_call, list_available_tools,
};
use super::types::{McpCallOutput, McpToolContent};
use crate::CliConfig;
use crate::interactive::resolve_required;
use crate::session::{CliSessionContext, get_or_create_session};
use crate::shared::{CommandOutput, render_result};

#[derive(Debug, Args)]
#[command(
    after_help = "Examples:\n  systemprompt plugins mcp call systemprompt systemprompt \\\n      \
                  --args '{\"command\":\"core skills list\"}'\n\n  systemprompt plugins mcp call \
                  <server> <tool> -a '{\"key\":\"value\"}'"
)]
pub struct CallArgs {
    #[arg(help = "MCP server name (required in non-interactive mode)")]
    pub server: Option<String>,

    #[arg(help = "Tool name to execute (required in non-interactive mode)")]
    pub tool: Option<String>,

    #[arg(short = 'a', long, help = "Tool arguments as JSON string")]
    pub args: Option<String>,

    #[arg(long, default_value = "30", help = "Timeout in seconds")]
    pub timeout: u64,
}

pub(super) async fn execute(args: CallArgs, config: &CliConfig) -> Result<CommandOutput> {
    let services_config = ConfigLoader::load().context("Failed to load services configuration")?;
    let session_ctx = get_or_create_session(config).await?;

    let server_arg = args.server.clone();
    let tool_arg = args.tool.clone();
    let timeout_secs = args.timeout;

    let server_name = resolve_required(server_arg, "server", config, || {
        prompt_server_selection(&services_config)
    })?;

    let _server_config = services_config
        .mcp_servers
        .get(&server_name)
        .ok_or_else(|| anyhow!("MCP server '{}' not found in configuration", server_name))?;

    let port = resolve_running_port(&server_name).await?;

    let tool_name = resolve_required(tool_arg, "tool", config, || {
        prompt_tool_selection(&server_name, port, &session_ctx, timeout_secs)
    })?;

    let tool_args: Option<serde_json::Value> = args
        .args
        .as_ref()
        .map(|s| serde_json::from_str(s))
        .transpose()
        .context("Invalid JSON in --args")?;

    let start_time = std::time::Instant::now();

    let result = execute_tool_call(ToolCallParams {
        server_name: &server_name,
        port,
        tool_name: &tool_name,
        arguments: tool_args,
        session_ctx: &session_ctx,
        timeout_secs,
    })
    .await;

    let execution_time_ms = start_time.elapsed().as_millis() as u64;

    let (output, failure) = match result {
        Ok(tool_result) => {
            success_outcome(&tool_result, &server_name, &tool_name, execution_time_ms)
        },
        Err(e) => failure_outcome(e.to_string(), &server_name, &tool_name, execution_time_ms),
    };

    let card = CommandOutput::card_value(format!("Tool Execution: {}", tool_name), &output);

    if let Some(msg) = failure {
        render_result(&card);
        return Err(anyhow!(msg));
    }

    Ok(card)
}

async fn resolve_running_port(server_name: &str) -> Result<u16> {
    let ctx = AppContext::new()
        .await
        .context("Failed to initialize application context")?;

    let manager = McpOrchestrator::new(
        Arc::clone(ctx.db_pool()),
        Arc::clone(ctx.app_paths_arc()),
        ctx.mcp_registry().clone(),
    )
    .context("Failed to initialize MCP manager")?;
    let running_servers = manager
        .get_running_servers()
        .await
        .context("Failed to get running servers")?;

    running_servers
        .iter()
        .find(|s| s.name == server_name)
        .map(|s| s.port)
        .ok_or_else(|| anyhow!("MCP server '{}' is not running", server_name))
}

fn success_outcome(
    tool_result: &CallToolResult,
    server_name: &str,
    tool_name: &str,
    execution_time_ms: u64,
) -> (McpCallOutput, Option<String>) {
    let content: Vec<McpToolContent> = tool_result
        .content
        .iter()
        .map(|c| convert_content(&c.raw))
        .collect();
    let is_error = tool_result.is_error.unwrap_or(false);
    let failure = is_error.then(|| {
        let detail = content
            .iter()
            .filter_map(|c| c.text.as_deref())
            .collect::<Vec<_>>()
            .join("\n");
        if detail.is_empty() {
            format!(
                "MCP tool '{}' on '{}' reported is_error=true with no message",
                tool_name, server_name
            )
        } else {
            format!(
                "MCP tool '{}' on '{}' reported is_error=true: {}",
                tool_name, server_name, detail
            )
        }
    });

    (
        McpCallOutput {
            server: server_name.to_owned(),
            tool: tool_name.to_owned(),
            success: !is_error,
            content,
            execution_time_ms,
            error: failure.clone(),
        },
        failure,
    )
}

fn failure_outcome(
    message: String,
    server_name: &str,
    tool_name: &str,
    execution_time_ms: u64,
) -> (McpCallOutput, Option<String>) {
    (
        McpCallOutput {
            server: server_name.to_owned(),
            tool: tool_name.to_owned(),
            success: false,
            content: vec![],
            execution_time_ms,
            error: Some(message.clone()),
        },
        Some(message),
    )
}

fn prompt_server_selection(config: &systemprompt_models::ServicesConfig) -> Result<String> {
    let mut servers: Vec<&String> = config.mcp_servers.keys().collect();
    servers.sort();

    if servers.is_empty() {
        return Err(anyhow!("No MCP servers configured"));
    }

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select MCP server")
        .items(&servers)
        .default(0)
        .interact()
        .context("Failed to get server selection")?;

    Ok(servers[selection].clone())
}

fn prompt_tool_selection(
    server_name: &str,
    port: u16,
    session_ctx: &CliSessionContext,
    timeout_secs: u64,
) -> Result<String> {
    let rt = tokio::runtime::Handle::current();
    let tools = rt.block_on(async {
        list_available_tools(server_name, port, session_ctx, timeout_secs).await
    })?;

    if tools.is_empty() {
        return Err(anyhow!("No tools available on server '{}'", server_name));
    }

    let tool_names: Vec<&str> = tools.iter().map(String::as_str).collect();

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select tool to execute")
        .items(&tool_names)
        .default(0)
        .interact()
        .context("Failed to get tool selection")?;

    Ok(tools[selection].clone())
}
