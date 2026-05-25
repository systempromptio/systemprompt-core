use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use clap::Args;

use super::tools_client::{list_tools_authenticated, list_tools_unauthenticated};
use super::tools_schema::print_schema_view;
use super::types::{McpToolEntry, McpToolsOutput, McpToolsSummary};
use crate::CliConfig;
use crate::session::get_or_create_session;
use crate::shared::CommandResult;
use systemprompt_loader::ConfigLoader;
use systemprompt_mcp::services::McpOrchestrator;
use systemprompt_runtime::AppContext;

#[derive(Debug, Args)]
pub struct ToolsArgs {
    #[arg(long, short, help = "Filter to a specific MCP server")]
    pub server: Option<String>,

    #[arg(long, help = "Show full input/output schemas in JSON output")]
    pub detailed: bool,

    #[arg(long, help = "Display parameter schemas in a readable format")]
    pub schema: bool,

    #[arg(long, default_value = "30", help = "Timeout in seconds")]
    pub timeout: u64,
}

pub(super) async fn execute(
    args: ToolsArgs,
    config: &CliConfig,
) -> Result<CommandResult<McpToolsOutput>> {
    let services_config = ConfigLoader::load().context("Failed to load services configuration")?;

    let session_ctx = get_or_create_session(config).await?;
    let session_token = session_ctx.session_token();

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

    let servers_to_query: Vec<_> = args.server.as_ref().map_or_else(
        || running_servers.iter().collect(),
        |filter| {
            running_servers
                .iter()
                .filter(|s| &s.name == filter)
                .collect()
        },
    );

    if servers_to_query.is_empty() {
        let message = args.server.as_ref().map_or_else(
            || "No MCP servers are currently running".to_owned(),
            |name| format!("MCP server '{}' is not running", name),
        );
        return Err(anyhow!(message));
    }

    let mut all_tools = Vec::new();
    let mut servers_queried = 0;

    for server in &servers_to_query {
        let server_config = services_config.mcp_servers.get(&server.name);
        let requires_auth = server_config.is_some_and(|c| c.oauth.required);

        let tools_result = if requires_auth {
            list_tools_authenticated(&server.name, server.port, session_token, args.timeout).await
        } else {
            list_tools_unauthenticated(&server.name, server.port, args.timeout).await
        };

        match tools_result {
            Ok(tools) => {
                for tool in tools {
                    all_tools.push(McpToolEntry {
                        name: tool.name,
                        server: server.name.clone(),
                        description: tool.description,
                        parameters_count: tool.parameters_count,
                        input_schema: (args.detailed || args.schema)
                            .then_some(tool.input_schema)
                            .flatten(),
                        output_schema: (args.detailed || args.schema)
                            .then_some(tool.output_schema)
                            .flatten(),
                    });
                }
                servers_queried += 1;
            },
            Err(e) => {
                tracing::warn!(
                    server = %server.name,
                    error = %e,
                    "Failed to list tools from server"
                );
            },
        }
    }

    all_tools.sort_by(|a, b| (&a.server, &a.name).cmp(&(&b.server, &b.name)));

    if args.schema && !config.is_json_output() {
        print_schema_view(&all_tools);
    }

    let output = McpToolsOutput {
        tools: all_tools.clone(),
        summary: McpToolsSummary {
            total_tools: all_tools.len(),
            servers_queried,
        },
    };

    let columns = vec![
        "name".to_owned(),
        "server".to_owned(),
        "description".to_owned(),
        "parameters_count".to_owned(),
    ];

    Ok(CommandResult::table(output)
        .with_title("MCP Tools")
        .with_columns(columns))
}
