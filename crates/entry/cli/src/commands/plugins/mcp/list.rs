//! `plugins mcp list` command with binary/build status.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use clap::Args;
use std::fs;
use std::path::PathBuf;

use super::types::{McpListOutput, McpServerSummary};
use crate::CliConfig;
use crate::shared::CommandOutput;
use crate::shared::project::ProjectRoot;
use systemprompt_loader::ConfigLoader;
use systemprompt_models::mcp::{Deployment, McpServerType};

#[derive(Debug, Clone, Copy, Args)]
pub struct ListArgs {
    #[arg(long, help = "Show only enabled servers")]
    pub enabled: bool,

    #[arg(long, help = "Show only disabled servers")]
    pub disabled: bool,
}

pub(super) fn execute(args: ListArgs, _config: &CliConfig) -> Result<CommandOutput> {
    let services_config = ConfigLoader::load().context("Failed to load services configuration")?;
    let project_root = ProjectRoot::discover().ok();

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
        .map(|(name, server)| summarize_server(name, server, project_root.as_ref()))
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

fn summarize_server(
    name: &str,
    server: &Deployment,
    project_root: Option<&ProjectRoot>,
) -> McpServerSummary {
    if server.server_type == McpServerType::External {
        return McpServerSummary {
            name: name.to_owned(),
            display_name: name.to_owned(),
            server_type: McpServerType::External.as_str().to_owned(),
            port: 0,
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

    let binary_name = if server.binary.is_empty() {
        name.to_owned()
    } else {
        server.binary.clone()
    };
    let (binary_debug, debug_created_at) = get_binary_info(project_root, &binary_name, false);
    let (binary_release, release_created_at) = get_binary_info(project_root, &binary_name, true);

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

fn get_binary_info(
    project_root: Option<&ProjectRoot>,
    binary_name: &str,
    release: bool,
) -> (Option<String>, Option<String>) {
    let Some(root) = project_root else {
        return (None, None);
    };

    let profile = if release { "release" } else { "debug" };
    let binary_path: PathBuf = root
        .as_path()
        .join("target")
        .join(profile)
        .join(binary_name);

    if !binary_path.exists() {
        return (None, None);
    }

    let path_str = Some(binary_path.display().to_string());

    let created_at = fs::metadata(&binary_path)
        .ok()
        .and_then(|m| m.modified().ok())
        .map(|t| {
            let datetime: DateTime<Utc> = t.into();
            datetime.format("%Y-%m-%d %H:%M:%S").to_string()
        });

    (path_str, created_at)
}

fn determine_status(enabled: bool, debug: Option<&str>, release: Option<&str>) -> String {
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
