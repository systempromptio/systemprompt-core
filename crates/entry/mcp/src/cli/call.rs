//! `plugins mcp call` command invoking one tool on a running server.
//!
//! The server must be configured and recorded as running; the command never
//! starts a server. The call goes through the API's MCP proxy with the
//! caller's own session ([`super::probe`]), so the execution is attributed to
//! the caller the proxy verified.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result, anyhow};
use clap::Args;
use systemprompt_extension::cli::{
    CommandError, CommandHost, CommandOutput, Prompter, resolve_required,
};
use systemprompt_identifiers::{McpServerId, McpToolName};
use systemprompt_mcp::McpServerConfig;
use systemprompt_models::ai::tools::CallToolResult;

use super::call_client::{ToolCallParams, convert_content, execute_tool_call};
use super::probe::{McpTarget, list_tools};
use super::types::{McpCallOutput, McpToolContent};
use super::{observer, prompt_server_selection, proxy_caller, services_config};

#[derive(Debug, Args)]
#[command(
    after_help = "Examples:\n  systemprompt plugins mcp call systemprompt systemprompt \\\n      \
                  --args '{\"command\":\"core skills list\"}'\n\n  systemprompt plugins mcp call \
                  <server> <tool> -a '{\"key\":\"value\"}'"
)]
pub struct CallArgs {
    #[arg(
        help = "MCP server name (required in non-interactive mode)",
        value_parser = systemprompt_extension::cli::parsers::parse_mcp_server_id
    )]
    pub server: Option<McpServerId>,

    #[arg(
        help = "Tool name to execute (required in non-interactive mode)",
        value_parser = systemprompt_extension::cli::parsers::parse_mcp_tool_name
    )]
    pub tool: Option<McpToolName>,

    #[arg(short = 'a', long, help = "Tool arguments as JSON string")]
    pub args: Option<String>,

    #[arg(long, default_value = "30", help = "Timeout in seconds")]
    pub timeout: u64,
}

pub(super) async fn execute(args: CallArgs, host: &dyn CommandHost) -> Result<CommandOutput> {
    let config = host.output();
    let prompter = host.prompter();
    let services_config = services_config(host)
        .await
        .context("Failed to load services configuration")?;

    let timeout_secs = args.timeout;

    let server_name = resolve_required(args.server.clone(), "server", config, || {
        prompt_server_selection(
            prompter,
            &services_config,
            "Select MCP server",
            McpServerId::new,
        )
    })?;

    if !services_config
        .mcp_servers
        .contains_key(server_name.as_str())
    {
        return Err(anyhow!(
            "MCP server '{server_name}' not found in configuration"
        ));
    }

    let server = resolve_running_server(&server_name, host).await?;
    let (api_url, session) = proxy_caller(host).await?;
    let target = McpTarget::through_proxy(&api_url, server.server_id(), &session);

    let tool_name = resolve_required(args.tool.clone(), "tool", config, || {
        prompt_tool_selection(prompter, &target, timeout_secs)
    })?;

    let tool_args: Option<serde_json::Value> = args
        .args
        .as_ref()
        .map(|s| serde_json::from_str(s))
        .transpose()
        .context("Invalid JSON in --args")?;

    let start_time = std::time::Instant::now();

    let result = execute_tool_call(ToolCallParams {
        target: &target,
        tool_name: &tool_name,
        arguments: tool_args,
        timeout_secs,
    })
    .await;

    let execution_time_ms = u64::try_from(start_time.elapsed().as_millis()).unwrap_or(u64::MAX);

    let (output, failure) = match result {
        Ok(tool_result) => {
            success_outcome(&tool_result, &server_name, &tool_name, execution_time_ms)?
        },
        Err(e) => failure_outcome(e.to_string(), &server_name, &tool_name, execution_time_ms),
    };

    let card = CommandOutput::card_value(format!("Tool Execution: {tool_name}"), &output);

    match failure {
        Some(msg) => Err(anyhow::Error::new(
            CommandError::new(anyhow!(msg)).with_output(card),
        )),
        None => Ok(card),
    }
}

async fn resolve_running_server(
    server_name: &McpServerId,
    host: &dyn CommandHost,
) -> Result<McpServerConfig> {
    let running_servers = observer(host)
        .await?
        .running_servers()
        .await
        .context("Failed to get running servers")?;

    running_servers
        .into_iter()
        .find(|s| s.name == server_name.as_str())
        .ok_or_else(|| {
            anyhow!(
                "MCP server '{server_name}' is not running; start it with `infra services start`"
            )
        })
}

fn success_outcome(
    tool_result: &CallToolResult,
    server_name: &McpServerId,
    tool_name: &McpToolName,
    execution_time_ms: u64,
) -> Result<(McpCallOutput, Option<String>)> {
    let content: Vec<McpToolContent> = tool_result
        .content
        .iter()
        .map(convert_content)
        .collect::<Result<_, _>>()
        .context("MCP tool returned an embedded resource that cannot be rendered as JSON")?;
    let is_error = tool_result.is_error.unwrap_or(false);
    let failure = is_error.then(|| {
        let detail = content
            .iter()
            .filter_map(|c| c.text.as_deref())
            .collect::<Vec<_>>()
            .join("\n");
        if detail.is_empty() {
            format!(
                "MCP tool '{tool_name}' on '{server_name}' reported is_error=true with no message"
            )
        } else {
            format!("MCP tool '{tool_name}' on '{server_name}' reported is_error=true: {detail}")
        }
    });

    Ok((
        McpCallOutput {
            server: server_name.clone(),
            tool: tool_name.clone(),
            success: !is_error,
            content,
            execution_time_ms,
            error: failure.clone(),
        },
        failure,
    ))
}

fn failure_outcome(
    message: String,
    server_name: &McpServerId,
    tool_name: &McpToolName,
    execution_time_ms: u64,
) -> (McpCallOutput, Option<String>) {
    (
        McpCallOutput {
            server: server_name.clone(),
            tool: tool_name.clone(),
            success: false,
            content: vec![],
            execution_time_ms,
            error: Some(message.clone()),
        },
        Some(message),
    )
}

fn prompt_tool_selection(
    prompter: &dyn Prompter,
    target: &McpTarget<'_>,
    timeout_secs: u64,
) -> Result<McpToolName> {
    let rt = tokio::runtime::Handle::current();
    let tools = rt.block_on(list_tools(target, timeout_secs))?;

    if tools.is_empty() {
        return Err(anyhow!(
            "No tools available on server '{}'",
            target.server_name()
        ));
    }

    let names: Vec<String> = tools.into_iter().map(|tool| tool.name).collect();
    let selection = prompter.select("Select tool to execute", &names)?;
    McpToolName::try_new(names[selection].clone())
        .context("MCP server advertised an invalid tool name")
}
