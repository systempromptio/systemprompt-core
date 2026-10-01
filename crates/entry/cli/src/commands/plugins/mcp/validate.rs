//! `plugins mcp validate` command with per-service connection checks.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result, anyhow};
use clap::Args;
use std::sync::Arc;
use std::time::Duration;

use super::types::{McpBatchValidateOutput, McpValidateOutput, McpValidateSummary};
pub use super::validate_output::{FailureDetail, failure_output, success_output};
use crate::context::CommandContext;
use crate::interactive::{Prompter, resolve_required};
use crate::shared::CommandOutput;
use systemprompt_identifiers::ServiceName;
use systemprompt_loader::ConfigLoader;
use systemprompt_mcp::services::client::validate_connection_with_auth;
use systemprompt_mcp::services::database::DatabaseService;
use systemprompt_models::Deployment;

#[derive(Debug, Args)]
pub struct ValidateArgs {
    #[arg(help = "MCP server name", value_parser = crate::shared::parse_service_name)]
    pub server: Option<ServiceName>,

    #[arg(
        long = "service",
        conflicts_with = "server",
        help = "Alias for the positional MCP server name",
        value_parser = crate::shared::parse_service_name
    )]
    pub service: Option<ServiceName>,

    #[arg(long, help = "Validate all configured servers")]
    pub all: bool,

    #[arg(long, default_value = "10", help = "Connection timeout in seconds")]
    pub timeout: u64,
}

pub(super) async fn execute(
    args: ValidateArgs,
    ctx: &CommandContext,
) -> Result<(CommandOutput, bool)> {
    let prompter = ctx.prompter();
    let config = &ctx.cli;
    let services_config = ConfigLoader::load().context("Failed to load services configuration")?;

    let app = ctx
        .app_context()
        .await
        .context("Failed to initialize application context")?;

    let database = DatabaseService::new(
        (**app.service_repository()).clone(),
        Arc::clone(app.app_paths_arc()),
        app.mcp_registry().clone(),
    );

    let server_arg = args.server.or(args.service);

    let servers_to_validate: Vec<ServiceName> =
        if args.all || (server_arg.is_none() && !config.is_interactive()) {
            services_config
                .mcp_servers
                .keys()
                .map(ServiceName::new)
                .collect()
        } else {
            let service = resolve_required(server_arg, "server", config, || {
                prompt_server_selection(prompter, &services_config)
            })?;

            if !services_config.mcp_servers.contains_key(service.as_str()) {
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
    let all_valid = valid_count == results.len();
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
        "MCP Batch Validation Results".to_owned()
    } else {
        format!(
            "MCP Validation: {}",
            servers_to_validate
                .first()
                .map_or("unknown", ServiceName::as_str)
        )
    };

    Ok((CommandOutput::card_value(title, &output), all_valid))
}

async fn validate_single_service(
    service_name: &ServiceName,
    services_config: &systemprompt_models::ServicesConfig,
    database: &DatabaseService,
    timeout_secs: u64,
) -> McpValidateOutput {
    let Some(server) = services_config.mcp_servers.get(service_name.as_str()) else {
        return failure_output(
            service_name,
            FailureDetail {
                health_status: "not_found",
                validation_type: "config_error",
                latency_ms: 0,
                issue: format!("Server '{}' not found in configuration", service_name),
                message: format!("MCP server '{}' not found", service_name),
            },
        );
    };

    let service_info = match database.get_service_by_name(service_name.as_str()).await {
        Ok(info) => info,
        Err(e) => {
            return failure_output(
                service_name,
                FailureDetail {
                    health_status: "unknown",
                    validation_type: "database_error",
                    latency_ms: 0,
                    issue: format!("Failed to check service status: {}", e),
                    message: format!("Database error for '{}'", service_name),
                },
            );
        },
    };

    let is_running = service_info
        .as_ref()
        .is_some_and(|info| info.status == "running");

    if !is_running {
        return failure_output(
            service_name,
            FailureDetail {
                health_status: "stopped",
                validation_type: "not_running",
                latency_ms: 0,
                issue: "Service is not currently running".to_owned(),
                message: format!("MCP server '{}' is not running", service_name),
            },
        );
    }

    let Some(port) = server.port else {
        return failure_output(
            service_name,
            FailureDetail {
                health_status: "unknown",
                validation_type: "config_error",
                latency_ms: 0,
                issue: "Server declares no local port; external servers are validated at their endpoint".to_owned(),
                message: format!("MCP server '{}' has no local port", service_name),
            },
        );
    };

    run_connection_validation(service_name, server, port, timeout_secs).await
}

pub async fn run_connection_validation(
    service_name: &ServiceName,
    server: &Deployment,
    port: u16,
    timeout_secs: u64,
) -> McpValidateOutput {
    let validation_future = validate_connection_with_auth(
        service_name.as_str(),
        "127.0.0.1",
        port,
        server.oauth.required,
    );

    let validation_result =
        match tokio::time::timeout(Duration::from_secs(timeout_secs), validation_future).await {
            Ok(Ok(result)) => result,
            Ok(Err(e)) => {
                return failure_output(
                    service_name,
                    FailureDetail {
                        health_status: "unhealthy",
                        validation_type: "connection_error",
                        latency_ms: 0,
                        issue: format!("Connection error: {}", e),
                        message: format!("Failed to connect to '{}'", service_name),
                    },
                );
            },
            Err(e) => {
                tracing::debug!(server = %service_name, error = %e, "MCP validation timed out");
                return failure_output(
                    service_name,
                    FailureDetail {
                        health_status: "unhealthy",
                        validation_type: "timeout",
                        latency_ms: timeout_secs as u32 * 1000,
                        issue: format!("Connection timed out after {} seconds", timeout_secs),
                        message: format!("Timeout connecting to '{}'", service_name),
                    },
                );
            },
        };

    success_output(service_name, validation_result)
}

pub fn prompt_server_selection(
    prompter: &dyn Prompter,
    config: &systemprompt_models::ServicesConfig,
) -> Result<ServiceName> {
    let mut servers: Vec<String> = config.mcp_servers.keys().cloned().collect();
    servers.sort();

    if servers.is_empty() {
        return Err(anyhow!("No MCP servers configured"));
    }

    let selection = prompter.select("Select MCP server to validate", &servers)?;
    Ok(ServiceName::new(servers[selection].clone()))
}
