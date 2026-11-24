//! Disk-log rendering for `plugins mcp logs`.
//!
//! Both disk modes select a log file the same way ([`select_log_file`]) from
//! a logs directory the caller has already confined to the profile's logs
//! root.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, anyhow};

use super::logs::LogsArgs;
use super::types::McpLogsOutput;
use systemprompt_extension::cli::{
    ChildSupervisor, CliConfig, CommandOutput, Prompter, resolve_required,
};
use systemprompt_models::artifacts::ListItem;
use systemprompt_models::path_confine::logs::{
    LogFile, display_names, find_log_file, list_log_files, read_log_lines,
};

const PREFIX: &str = "mcp-";

pub(super) fn execute_disk_mode(
    args: &LogsArgs,
    prompter: &dyn Prompter,
    config: &CliConfig,
    logs_path: &Path,
) -> Result<CommandOutput> {
    ensure_exists(logs_path)?;

    if args.server.is_none() && !config.is_interactive() {
        let log_files = list_log_files(logs_path, PREFIX)?;
        let items: Vec<ListItem> = log_files
            .iter()
            .map(|file| ListItem::new(file.clone(), String::new(), file.clone()))
            .collect();
        return Ok(CommandOutput::list(items).with_title("Available MCP Log Files"));
    }

    let (service, log_file) = select_log_file(args, prompter, config, logs_path)?;
    let logs = read_log_lines(log_file, args.lines, |line| {
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
    prompter: &dyn Prompter,
    config: &CliConfig,
    logs_path: &Path,
    children: &dyn ChildSupervisor,
) -> Result<CommandOutput> {
    ensure_exists(logs_path)?;
    let (service, log_file) = select_log_file(args, prompter, config, logs_path)?;

    let mut cmd = Command::new("tail");
    cmd.arg("-f").arg(log_file.path());
    let status = children.status(cmd).context("Failed to execute tail -f")?;

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

fn ensure_exists(logs_path: &Path) -> Result<()> {
    if logs_path.exists() {
        return Ok(());
    }
    Err(anyhow!(
        "Logs directory does not exist: {}",
        logs_path.display()
    ))
}

fn select_log_file(
    args: &LogsArgs,
    prompter: &dyn Prompter,
    config: &CliConfig,
    logs_path: &Path,
) -> Result<(String, LogFile)> {
    let service = resolve_required(args.server.clone(), "server", config, || {
        prompt_log_selection(prompter, logs_path)
    })?;
    let log_file = find_log_file(logs_path, PREFIX, "MCP server name", &service)?;
    Ok((service, log_file))
}

fn prompt_log_selection(prompter: &dyn Prompter, logs_dir: &Path) -> Result<String> {
    let log_files = list_log_files(logs_dir, PREFIX)?;

    if log_files.is_empty() {
        return Err(anyhow!("No MCP log files found in {}", logs_dir.display()));
    }

    let services = display_names(&log_files, PREFIX);

    let selection = prompter.select("Select MCP server logs to view", &services)?;
    Ok(services[selection].clone())
}
