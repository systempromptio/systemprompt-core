use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use clap::Args;
use dialoguer::Select;
use dialoguer::theme::ColorfulTheme;
use systemprompt_loader::ConfigLoader;
use systemprompt_mcp::services::McpOrchestrator;
use systemprompt_runtime::AppContext;

use super::call_client::{
    ToolCallParams, convert_content, execute_tool_call, list_available_tools,
};
use super::types::{McpCallOutput, McpToolContent};
use crate::CliConfig;
use crate::interactive::resolve_required;
use crate::session::{CliSessionContext, get_or_create_session};
use crate::shared::{CommandResult, render_result};

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

pub(crate) async fn execute(args: CallArgs, config: &CliConfig) -> Result<CommandResult<McpCallOutput>> {
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

    let running_server = running_servers
        .iter()
        .find(|s| s.name == server_name)
        .ok_or_else(|| anyhow!("MCP server '{}' is not running", server_name))?;

    let tool_name = resolve_required(tool_arg, "tool", config, || {
        prompt_tool_selection(
            &server_name,
            running_server.port,
            &session_ctx,
            timeout_secs,
        )
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
        port: running_server.port,
        tool_name: &tool_name,
        arguments: tool_args,
        session_ctx: &session_ctx,
        timeout_secs,
    })
    .await;

    let execution_time_ms = start_time.elapsed().as_millis() as u64;

    let (output, failure) = match result {
        Ok(tool_result) => {
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
                    server: server_name.clone(),
                    tool: tool_name.clone(),
                    success: !is_error,
                    content,
                    execution_time_ms,
                    error: failure.clone(),
                },
                failure,
            )
        },
        Err(e) => {
            let msg = e.to_string();
            (
                McpCallOutput {
                    server: server_name.clone(),
                    tool: tool_name.clone(),
                    success: false,
                    content: vec![],
                    execution_time_ms,
                    error: Some(msg.clone()),
                },
                Some(msg),
            )
        },
    };

    let card = CommandResult::card(output).with_title(format!("Tool Execution: {}", tool_name));

    if let Some(msg) = failure {
        render_result(&card);
        return Err(anyhow!(msg));
    }

    Ok(card)
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
