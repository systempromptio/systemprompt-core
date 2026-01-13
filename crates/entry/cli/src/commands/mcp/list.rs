use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use clap::Args;
use std::fs;
use std::path::PathBuf;

use super::types::{McpListOutput, McpServerSummary};
use crate::shared::project::ProjectRoot;
use crate::shared::CommandResult;
use crate::CliConfig;
use systemprompt_loader::ConfigLoader;

#[derive(Debug, Clone, Copy, Args)]
pub struct ListArgs {
    #[arg(long, help = "Show only enabled servers")]
    pub enabled: bool,
}

pub async fn execute(args: ListArgs, _config: &CliConfig) -> Result<CommandResult<McpListOutput>> {
    let services_config = ConfigLoader::load().context("Failed to load services configuration")?;
    let project_root = ProjectRoot::discover().ok();

    let mut servers: Vec<McpServerSummary> = services_config
        .mcp_servers
        .iter()
        .filter(|(_, server)| {
            if args.enabled {
                server.enabled
            } else {
                true
            }
        })
        .map(|(name, server)| {
            let binary_name = if server.binary.is_empty() {
                name.clone()
            } else {
                server.binary.clone()
            };
            let (debug_binary, debug_created_at) =
                get_binary_info(&project_root, &binary_name, false);
            let (release_binary, release_created_at) =
                get_binary_info(&project_root, &binary_name, true);

            McpServerSummary {
                name: name.clone(),
                port: server.port,
                enabled: server.enabled,
                status: determine_status(server.enabled, &debug_binary, &release_binary),
                debug_binary,
                debug_created_at,
                release_binary,
                release_created_at,
            }
        })
        .collect();

    servers.sort_by(|a, b| a.name.cmp(&b.name));

    let output = McpListOutput { servers };

    Ok(CommandResult::table(output)
        .with_title("MCP Servers")
        .with_columns(vec![
            "name".to_string(),
            "port".to_string(),
            "enabled".to_string(),
            "status".to_string(),
            "debug_binary".to_string(),
            "release_binary".to_string(),
        ]))
}

fn get_binary_info(
    project_root: &Option<ProjectRoot>,
    binary_name: &str,
    release: bool,
) -> (Option<String>, Option<String>) {
    let Some(root) = project_root else {
        return (None, None);
    };

    let profile = if release { "release" } else { "debug" };
    let binary_path: PathBuf = root.as_path().join("target").join(profile).join(binary_name);

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

fn determine_status(enabled: bool, debug: &Option<String>, release: &Option<String>) -> String {
    if !enabled {
        return "disabled".to_string();
    }

    match (debug.is_some(), release.is_some()) {
        (true, true) => "ready".to_string(),
        (true, false) => "debug-only".to_string(),
        (false, true) => "release-only".to_string(),
        (false, false) => "not-built".to_string(),
    }
}
