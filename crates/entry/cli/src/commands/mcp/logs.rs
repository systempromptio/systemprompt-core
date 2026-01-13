use anyhow::{anyhow, Context, Result};
use clap::Args;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Select;
use std::path::PathBuf;

use super::types::McpLogsOutput;
use crate::shared::{resolve_input, CommandResult};
use crate::CliConfig;

const DEFAULT_LOGS_DIR: &str = "/var/www/html/tyingshoelaces/logs";

#[derive(Debug, Args)]
pub struct LogsArgs {
    #[arg(help = "MCP server name (optional - lists all if not specified)")]
    pub service: Option<String>,

    #[arg(long, short = 'n', default_value = "50", help = "Number of lines to show")]
    pub lines: usize,

    #[arg(long, short, help = "Follow log output continuously")]
    pub follow: bool,

    #[arg(long, help = "Custom logs directory path")]
    pub logs_dir: Option<String>,
}

pub async fn execute(args: LogsArgs, config: &CliConfig) -> Result<CommandResult<McpLogsOutput>> {
    let logs_dir = args.logs_dir.as_deref().unwrap_or(DEFAULT_LOGS_DIR);
    let logs_path = PathBuf::from(logs_dir);

    if !logs_path.exists() {
        return Err(anyhow!("Logs directory does not exist: {}", logs_dir));
    }

    if args.follow {
        let service = resolve_input(args.service, "service", config, || {
            prompt_log_selection(&logs_path)
        })?;

        let log_file = find_log_file(&logs_path, &service)?;

        use std::process::Command;
        let status = Command::new("tail")
            .arg("-f")
            .arg(&log_file)
            .status()
            .context("Failed to execute tail -f")?;

        if !status.success() {
            return Err(anyhow!("tail -f exited with non-zero status"));
        }

        return Ok(CommandResult::text(McpLogsOutput {
            service: Some(service),
            logs: vec![],
            log_files: vec![],
        })
        .with_title("MCP Logs"));
    }

    if args.service.is_none() && !config.is_interactive() {
        let log_files = list_mcp_log_files(&logs_path)?;
        return Ok(CommandResult::list(McpLogsOutput {
            service: None,
            logs: vec![],
            log_files,
        })
        .with_title("Available MCP Log Files"));
    }

    let service = resolve_input(args.service, "service", config, || {
        prompt_log_selection(&logs_path)
    })?;

    let log_file = find_log_file(&logs_path, &service)?;
    let logs = read_log_lines(&log_file, args.lines)?;

    let output = McpLogsOutput {
        service: Some(service.clone()),
        logs,
        log_files: vec![],
    };

    Ok(CommandResult::text(output).with_title(&format!("Logs: {}", service)))
}

fn list_mcp_log_files(logs_dir: &PathBuf) -> Result<Vec<String>> {
    let mut files = std::fs::read_dir(logs_dir)
        .context("Failed to read logs directory")?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let path = entry.path();
            path.file_name()
                .and_then(|n| n.to_str())
                .filter(|name| name.starts_with("mcp-") && name.ends_with(".log"))
                .map(String::from)
        })
        .collect::<Vec<_>>();

    files.sort();
    Ok(files)
}

fn find_log_file(logs_dir: &PathBuf, service: &str) -> Result<PathBuf> {
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

fn read_log_lines(log_file: &PathBuf, lines: usize) -> Result<Vec<String>> {
    use std::io::{BufRead, BufReader};

    let file = std::fs::File::open(log_file)
        .with_context(|| format!("Failed to open log file: {}", log_file.display()))?;

    let reader = BufReader::new(file);
    let all_lines: Vec<String> = reader
        .lines()
        .collect::<Result<Vec<_>, _>>()
        .context("Failed to read log lines")?;

    let start = all_lines.len().saturating_sub(lines);
    Ok(all_lines[start..].to_vec())
}

fn prompt_log_selection(logs_dir: &PathBuf) -> Result<String> {
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
                .to_string()
        })
        .collect();

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select MCP server logs to view")
        .items(&services)
        .default(0)
        .interact()
        .context("Failed to get log selection")?;

    Ok(services[selection].clone())
}
