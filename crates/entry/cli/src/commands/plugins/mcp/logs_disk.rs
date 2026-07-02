use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, anyhow};
use dialoguer::Select;
use dialoguer::theme::ColorfulTheme;

use super::logs::LogsArgs;
use super::types::McpLogsOutput;
use crate::CliConfig;
use crate::interactive::resolve_required;
use crate::shared::CommandOutput;
use crate::shared::disk_logs::{display_names, find_log_file, list_log_files, read_log_lines};
use systemprompt_models::artifacts::ListItem;

pub(super) fn execute_disk_mode(
    args: &LogsArgs,
    config: &CliConfig,
    logs_path: &Path,
) -> Result<CommandOutput> {
    if !logs_path.exists() {
        return Err(anyhow!(
            "Logs directory does not exist: {}",
            logs_path.display()
        ));
    }

    if args.server.is_none() && !config.is_interactive() {
        let log_files = list_log_files(logs_path, "mcp-")?;
        let items: Vec<ListItem> = log_files
            .iter()
            .map(|file| ListItem::new(file.clone(), String::new(), file.clone()))
            .collect();
        return Ok(CommandOutput::list(items).with_title("Available MCP Log Files"));
    }

    let service = resolve_required(args.server.clone(), "server", config, || {
        prompt_log_selection(logs_path)
    })?;

    let log_file = find_log_file(logs_path, "mcp-", &service)?;
    let logs = read_log_lines(&log_file, args.lines, |line| {
        args.level.is_none_or(|level| level.matches(line))
    })?;

    let level_label = args.level.map_or_else(String::new, |l| {
        format!(" [{}+]", format!("{:?}", l).to_uppercase())
    });

    let output = McpLogsOutput {
        service: Some(service.clone()),
        source: "disk".to_owned(),
        logs,
        log_files: vec![],
    };

    Ok(CommandOutput::card_value(
        format!("MCP Logs (Disk): {}{}", service, level_label),
        &output,
    ))
}

pub(super) fn execute_follow_mode(
    args: &LogsArgs,
    config: &CliConfig,
    logs_path: &Path,
) -> Result<CommandOutput> {
    if !logs_path.exists() {
        return Err(anyhow!(
            "Logs directory does not exist: {}",
            logs_path.display()
        ));
    }

    let service = resolve_required(args.server.clone(), "server", config, || {
        prompt_log_selection(logs_path)
    })?;

    let log_file = find_log_file(logs_path, "mcp-", &service)?;

    let status = Command::new("tail")
        .arg("-f")
        .arg(&log_file)
        .status()
        .context("Failed to execute tail -f")?;

    if !status.success() {
        return Err(anyhow!("tail -f exited with non-zero status"));
    }

    let output = McpLogsOutput {
        service: Some(service),
        source: "disk".to_owned(),
        logs: vec![],
        log_files: vec![],
    };

    Ok(CommandOutput::card_value("MCP Logs", &output))
}

fn prompt_log_selection(logs_dir: &Path) -> Result<String> {
    let log_files = list_log_files(logs_dir, "mcp-")?;

    if log_files.is_empty() {
        return Err(anyhow!("No MCP log files found in {}", logs_dir.display()));
    }

    let services = display_names(&log_files, "mcp-");

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select MCP server logs to view")
        .items(&services)
        .default(0)
        .interact()
        .context("Failed to get log selection")?;

    Ok(services[selection].clone())
}
