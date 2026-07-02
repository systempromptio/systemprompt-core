use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, anyhow};

use super::logs::{LogLevel, LogsArgs};
use super::types::McpLogsOutput;
use crate::CliConfig;
use crate::interactive::{Prompter, resolve_required};
use crate::shared::CommandOutput;
use systemprompt_models::artifacts::ListItem;

pub(super) fn execute_disk_mode(
    args: &LogsArgs,
    prompter: &dyn Prompter,
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
        let log_files = list_mcp_log_files(logs_path)?;
        let items: Vec<ListItem> = log_files
            .iter()
            .map(|file| ListItem::new(file.clone(), String::new(), file.clone()))
            .collect();
        return Ok(CommandOutput::list(items).with_title("Available MCP Log Files"));
    }

    let service = resolve_required(args.server.clone(), "server", config, || {
        prompt_log_selection(prompter, logs_path)
    })?;

    let log_file = find_log_file(logs_path, &service)?;
    let logs = read_log_lines(&log_file, args.lines, args.level)?;

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
) -> Result<CommandOutput> {
    if !logs_path.exists() {
        return Err(anyhow!(
            "Logs directory does not exist: {}",
            logs_path.display()
        ));
    }

    let service = resolve_required(args.server.clone(), "server", config, || {
        prompt_log_selection(prompter, logs_path)
    })?;

    let log_file = find_log_file(logs_path, &service)?;

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

fn list_mcp_log_files(logs_dir: &Path) -> Result<Vec<String>> {
    let mut files = std::fs::read_dir(logs_dir)
        .context("Failed to read logs directory")?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            path.file_name()
                .and_then(|n| n.to_str())
                .filter(|name| {
                    name.starts_with("mcp-")
                        && path
                            .extension()
                            .is_some_and(|ext| ext.eq_ignore_ascii_case("log"))
                })
                .map(String::from)
        })
        .collect::<Vec<_>>();

    files.sort();
    Ok(files)
}

fn find_log_file(logs_dir: &Path, service: &str) -> Result<PathBuf> {
    let exact_path = logs_dir.join(format!("{}.log", service));
    if exact_path.exists() {
        return Ok(exact_path);
    }

    let prefixed_path = logs_dir.join(format!("mcp-{}.log", service));
    if prefixed_path.exists() {
        return Ok(prefixed_path);
    }

    let log_files = list_mcp_log_files(logs_dir)?;
    log_files
        .iter()
        .find(|file| file.contains(service))
        .map(|file| logs_dir.join(file))
        .ok_or_else(|| {
            anyhow!(
                "Log file not found for service '{}'. Available: {:?}",
                service,
                log_files
            )
        })
}

fn read_log_lines(log_file: &Path, lines: usize, level: Option<LogLevel>) -> Result<Vec<String>> {
    use std::io::{BufRead, BufReader};

    let file = std::fs::File::open(log_file)
        .with_context(|| format!("Failed to open log file: {}", log_file.display()))?;

    let reader = BufReader::new(file);
    let all_lines: Vec<String> = reader
        .lines()
        .collect::<Result<Vec<_>, _>>()
        .context("Failed to read log lines")?;

    let filtered_lines: Vec<String> = if let Some(log_level) = level {
        all_lines
            .into_iter()
            .filter(|line| log_level.matches(line))
            .collect()
    } else {
        all_lines
    };

    let start = filtered_lines.len().saturating_sub(lines);
    Ok(filtered_lines[start..].to_vec())
}

fn prompt_log_selection(prompter: &dyn Prompter, logs_dir: &Path) -> Result<String> {
    let log_files = list_mcp_log_files(logs_dir)?;

    if log_files.is_empty() {
        return Err(anyhow!("No MCP log files found in {}", logs_dir.display()));
    }

    let services: Vec<String> = log_files
        .iter()
        .map(|f| {
            f.strip_prefix("mcp-")
                .unwrap_or(f)
                .strip_suffix(".log")
                .unwrap_or(f)
                .to_owned()
        })
        .collect();

    let selection = prompter.select("Select MCP server logs to view", &services)?;
    Ok(services[selection].clone())
}
