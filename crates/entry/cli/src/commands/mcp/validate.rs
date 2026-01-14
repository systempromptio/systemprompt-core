//! Validate MCP connection

use anyhow::{anyhow, Context, Result};
use clap::Args;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Select;
use std::sync::Arc;
use std::time::Duration;

use super::types::{
    McpBatchValidateOutput, McpServerInfo, McpValidateOutput, McpValidateSummary,
};
use crate::shared::{resolve_input, CommandResult};
use crate::CliConfig;
use systemprompt_core_mcp::services::client::validate_connection_with_auth;
use systemprompt_core_mcp::services::database::DatabaseManager;
use systemprompt_core_mcp::services::McpManager;
use systemprompt_loader::ConfigLoader;
use systemprompt_runtime::AppContext;

#[derive(Debug, Args)]
pub struct ValidateArgs {
    /// MCP server name to validate (required in non-interactive mode unless --all is used)
    #[arg(help = "MCP server name")]
    pub service: Option<String>,

    /// Validate all configured MCP servers
    #[arg(long, help = "Validate all configured servers")]
    pub all: bool,

    /// Connection timeout in seconds
    #[arg(long, default_value = "10", help = "Connection timeout in seconds")]
    pub timeout: u64,
}

pub async fn execute(
    args: ValidateArgs,
    config: &CliConfig,
) -> Result<CommandResult<McpBatchValidateOutput>> {
    let services_config = ConfigLoader::load().context("Failed to load services configuration")?;

    let ctx = Arc::new(
        AppContext::new()
            .await
            .context("Failed to initialize application context")?,
    );

    let _manager = McpManager::new(Arc::clone(&ctx)).context("Failed to initialize MCP manager")?;
    let database = DatabaseManager::new(Arc::clone(ctx.db_pool()));

    let servers_to_validate: Vec<String> = if args.all {
        services_config.mcp_servers.keys().cloned().collect()
    } else {
        let service = resolve_input(args.service, "service", config, || {
            prompt_server_selection(&services_config)
        })?;

        if !services_config.mcp_servers.contains_key(&service) {
            return Err(anyhow!("MCP server '{}' not found", service));
        }

        vec![service]
    };

    let mut results = Vec::new();

    for service_name in &servers_to_validate {
        let result =
            validate_single_service(service_name, &services_config, &database, args.timeout).await;
        results.push(result);
    }

    let valid_count = results.iter().filter(|r| r.valid).count();
    let healthy_count = results
        .iter()
        .filter(|r| r.health_status == "healthy")
        .count();

    let output = McpBatchValidateOutput {
        summary: McpValidateSummary {
            total: results.len(),
            valid: valid_count,
            invalid: results.len() - valid_count,
            healthy: healthy_count,
            unhealthy: results.len() - healthy_count,
        },
        results,
    };

    let title = if args.all {
        "MCP Batch Validation Results".to_string()
    } else {
        format!(
            "MCP Validation: {}",
            servers_to_validate.first().unwrap_or(&"unknown".to_string())
        )
    };

    Ok(CommandResult::card(output).with_title(title))
}

async fn validate_single_service(
    service_name: &str,
    services_config: &systemprompt_models::ServicesConfig,
    database: &DatabaseManager,
    timeout_secs: u64,
) -> McpValidateOutput {
    let server = match services_config.mcp_servers.get(service_name) {
        Some(s) => s,
        None => {
            return McpValidateOutput {
                server: service_name.to_string(),
                valid: false,
                health_status: "not_found".to_string(),
                validation_type: "config_error".to_string(),
                tools_count: 0,
                latency_ms: 0,
                server_info: None,
                issues: vec![format!("Server '{}' not found in configuration", service_name)],
                message: format!("MCP server '{}' not found", service_name),
            };
        },
    };

    // Check if service is running
    let service_info = match database.get_service_by_name(service_name).await {
        Ok(info) => info,
        Err(e) => {
            return McpValidateOutput {
                server: service_name.to_string(),
                valid: false,
                health_status: "unknown".to_string(),
                validation_type: "database_error".to_string(),
                tools_count: 0,
                latency_ms: 0,
                server_info: None,
                issues: vec![format!("Failed to check service status: {}", e)],
                message: format!("Database error for '{}'", service_name),
            };
        },
    };

    let is_running = service_info
        .as_ref()
        .is_some_and(|info| info.status == "running");

    if !is_running {
        return McpValidateOutput {
            server: service_name.to_string(),
            valid: false,
            health_status: "stopped".to_string(),
            validation_type: "not_running".to_string(),
            tools_count: 0,
            latency_ms: 0,
            server_info: None,
            issues: vec!["Service is not currently running".to_string()],
            message: format!("MCP server '{}' is not running", service_name),
        };
    }

    // Perform actual validation with timeout
    let validation_future = validate_connection_with_auth(
        service_name,
        "127.0.0.1",
        server.port,
        server.oauth.required,
    );

    let validation_result = match tokio::time::timeout(
        Duration::from_secs(timeout_secs),
        validation_future,
    )
    .await
    {
        Ok(Ok(result)) => result,
        Ok(Err(e)) => {
            return McpValidateOutput {
                server: service_name.to_string(),
                valid: false,
                health_status: "unhealthy".to_string(),
                validation_type: "connection_error".to_string(),
                tools_count: 0,
                latency_ms: 0,
                server_info: None,
                issues: vec![format!("Connection error: {}", e)],
                message: format!("Failed to connect to '{}'", service_name),
            };
        },
        Err(_) => {
            return McpValidateOutput {
                server: service_name.to_string(),
                valid: false,
                health_status: "unhealthy".to_string(),
                validation_type: "timeout".to_string(),
                tools_count: 0,
                latency_ms: timeout_secs as u32 * 1000,
                server_info: None,
                issues: vec![format!(
                    "Connection timed out after {} seconds",
                    timeout_secs
                )],
                message: format!("Timeout connecting to '{}'", service_name),
            };
        },
    };

    let health_status = validation_result.health_status().to_string();
    let message = validation_result.status_description();

    let server_info = validation_result.server_info.map(|info| McpServerInfo {
        name: info.server_name,
        version: info.version,
        protocol_version: info.protocol_version,
    });

    let issues = if let Some(ref error) = validation_result.error_message {
        if error.is_empty() {
            vec![]
        } else {
            vec![error.clone()]
        }
    } else {
        vec![]
    };

    McpValidateOutput {
        server: service_name.to_string(),
        valid: validation_result.success,
        health_status,
        validation_type: validation_result.validation_type,
        tools_count: validation_result.tools_count,
        latency_ms: validation_result.connection_time_ms,
        server_info,
        issues,
        message,
    }
}

fn prompt_server_selection(config: &systemprompt_models::ServicesConfig) -> Result<String> {
    let mut servers: Vec<&String> = config.mcp_servers.keys().collect();
    servers.sort();

    if servers.is_empty() {
        return Err(anyhow!("No MCP servers configured"));
    }

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select MCP server to validate")
        .items(&servers)
        .default(0)
        .interact()
        .context("Failed to get server selection")?;

    Ok(servers[selection].clone())
}
