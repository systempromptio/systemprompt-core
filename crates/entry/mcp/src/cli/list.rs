//! `plugins mcp list` command with binary/build status.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use clap::Args;
use systemprompt_extension::cli::{CommandHost, CommandOutput};
use systemprompt_models::mcp::{Deployment, McpServerType};

use super::types::{McpListOutput, McpServerSummary};
use super::{build_target_dir, services_config};

#[derive(Debug, Clone, Copy, Args)]
pub struct ListArgs {
    #[arg(long, help = "Show only enabled servers")]
    pub enabled: bool,

    #[arg(long, help = "Show only disabled servers")]
    pub disabled: bool,
}

pub(super) async fn execute(args: ListArgs, host: &dyn CommandHost) -> Result<CommandOutput> {
    let services_config = services_config(host)
        .await
        .context("Failed to load services configuration")?;
    let target_dir = build_target_dir(host).await?;

    let mut servers: Vec<McpServerSummary> = services_config
        .mcp_servers
        .iter()
        .filter(|(_, server)| {
            if args.enabled && args.disabled {
                true
            } else if args.enabled {
                server.enabled
            } else if args.disabled {
                !server.enabled
            } else {
                true
            }
        })
        .map(|(name, server)| summarize_server(name, server, target_dir.as_deref()))
        .collect();

    servers.sort_by(|a, b| a.name.cmp(&b.name));

    let output = McpListOutput { servers };

    Ok(CommandOutput::table_of(
        vec![
            "name",
            "server_type",
            "port",
            "enabled",
            "status",
            "endpoint",
            "binary_debug",
            "binary_release",
        ],
        &output.servers,
    )
    .with_title("MCP Servers"))
}

pub fn summarize_server(
    name: &str,
    server: &Deployment,
    target_dir: Option<&Path>,
) -> McpServerSummary {
    if server.server_type == McpServerType::External {
        return McpServerSummary {
            name: name.to_owned(),
            display_name: name.to_owned(),
            server_type: McpServerType::External.as_str().to_owned(),
            port: None,
            enabled: server.enabled,
            status: Some(if server.enabled {
                "remote".to_owned()
            } else {
                "disabled".to_owned()
            }),
            endpoint: server.endpoint.clone(),
            binary_debug: None,
            binary_release: None,
            debug_created_at: None,
            release_created_at: None,
            created_at: None,
        };
    }

    let binary_name = server
        .binary
        .as_deref()
        .filter(|binary| !binary.is_empty())
        .unwrap_or(name);
    let (binary_debug, debug_created_at) = get_binary_info(target_dir, binary_name, false);
    let (binary_release, release_created_at) = get_binary_info(target_dir, binary_name, true);

    McpServerSummary {
        name: name.to_owned(),
        display_name: name.to_owned(),
        server_type: McpServerType::Internal.as_str().to_owned(),
        port: server.port,
        enabled: server.enabled,
        status: Some(determine_status(
            server.enabled,
            binary_debug.as_deref(),
            binary_release.as_deref(),
        )),
        endpoint: None,
        binary_debug,
        binary_release,
        debug_created_at,
        release_created_at,
        created_at: None,
    }
}

pub fn binary_path(target_dir: Option<&Path>, binary_name: &str, profile: &str) -> Option<PathBuf> {
    target_dir
        .map(|dir| dir.join(profile).join(binary_name))
        .filter(|path| path.exists())
}

pub fn get_binary_info(
    target_dir: Option<&Path>,
    binary_name: &str,
    release: bool,
) -> (Option<String>, Option<String>) {
    let profile = if release { "release" } else { "debug" };
    let Some(binary_path) = binary_path(target_dir, binary_name, profile) else {
        return (None, None);
    };

    let created_at = fs::metadata(&binary_path)
        .ok()
        .and_then(|m| m.modified().ok())
        .map(|t| {
            let datetime: DateTime<Utc> = t.into();
            datetime.format("%Y-%m-%d %H:%M:%S").to_string()
        });

    (Some(binary_path.display().to_string()), created_at)
}

pub fn determine_status(enabled: bool, debug: Option<&str>, release: Option<&str>) -> String {
    if !enabled {
        return "disabled".to_owned();
    }

    match (debug.is_some(), release.is_some()) {
        (true, true) => "ready".to_owned(),
        (true, false) => "debug-only".to_owned(),
        (false, true) => "release-only".to_owned(),
        (false, false) => "not-built".to_owned(),
    }
}
