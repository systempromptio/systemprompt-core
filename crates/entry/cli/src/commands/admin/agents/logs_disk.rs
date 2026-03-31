use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, anyhow};
use dialoguer::Select;
use dialoguer::theme::ColorfulTheme;

use super::logs::LogsArgs;
use super::types::AgentLogsOutput;
use crate::CliConfig;
use crate::interactive::resolve_required;
use crate::shared::CommandResult;

pub fn execute_disk_mode(
    args: &LogsArgs,
    config: &CliConfig,
    logs_path: &Path,
) -> Result<CommandResult<AgentLogsOutput>> {
    if !logs_path.exists() {
        return Err(anyhow!(
            "Logs directory does not exist: {}",
            logs_path.display()
        ));
    }

    if args.agent.is_none() && !config.is_interactive() {
        let log_files = list_agent_log_files(logs_path)?;
        return Ok(CommandResult::list(AgentLogsOutput {
            agent: None,
            source: "disk".to_string(),
            logs: vec![],
            log_files,
        })
        .with_title("Available Agent Log Files"));
    }

    let agent = resolve_required(args.agent.clone(), "agent", config, || {
        prompt_log_selection(logs_path)
    })?;

    let log_file = find_log_file(logs_path, &agent)?;
    let logs = read_log_lines(&log_file, args.lines)?;

    Ok(CommandResult::text(AgentLogsOutput {
        agent: Some(agent.clone()),
        source: "disk".to_string(),
        logs,
        log_files: vec![],
    })
    .with_title(format!("Agent Logs (Disk): {}", agent)))
}

pub fn execute_follow_mode(
    args: &LogsArgs,
    config: &CliConfig,
    logs_path: &Path,
) -> Result<CommandResult<AgentLogsOutput>> {
    if !logs_path.exists() {
        return Err(anyhow!(
            "Logs directory does not exist: {}",
            logs_path.display()
        ));
    }

    let agent = resolve_required(args.agent.clone(), "agent", config, || {
        prompt_log_selection(logs_path)
    })?;

    let log_file = find_log_file(logs_path, &agent)?;

    let status = Command::new("tail")
        .arg("-f")
        .arg(&log_file)
        .status()
        .context("Failed to execute tail -f")?;

    if !status.success() {
        return Err(anyhow!("tail -f exited with non-zero status"));
    }

    Ok(CommandResult::text(AgentLogsOutput {
        agent: Some(agent),
        source: "disk".to_string(),
        logs: vec![],
        log_files: vec![],
    })
    .with_title("Agent Logs"))
}

pub fn list_agent_log_files(logs_dir: &Path) -> Result<Vec<String>> {
    let mut files = std::fs::read_dir(logs_dir)
        .context("Failed to read logs directory")?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            path.file_name()
                .and_then(|n| n.to_str())
                .filter(|name| {
                    name.starts_with("agent-")
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

fn find_log_file(logs_dir: &Path, agent: &str) -> Result<PathBuf> {
    let exact_path = logs_dir.join(format!("{}.log", agent));
    if exact_path.exists() {
        return Ok(exact_path);
    }

    let prefixed_path = logs_dir.join(format!("agent-{}.log", agent));
    if prefixed_path.exists() {
        return Ok(prefixed_path);
    }

    let log_files = list_agent_log_files(logs_dir)?;
    log_files
        .iter()
        .find(|file| file.contains(agent))
        .map(|file| logs_dir.join(file))
        .ok_or_else(|| {
            anyhow!(
                "Log file not found for agent '{}'. Available: {:?}",
                agent,
                log_files
            )
        })
}

fn read_log_lines(log_file: &Path, lines: usize) -> Result<Vec<String>> {
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

fn prompt_log_selection(logs_dir: &Path) -> Result<String> {
    let log_files = list_agent_log_files(logs_dir)?;

    if log_files.is_empty() {
        return Err(anyhow!(
            "No agent log files found in {}",
            logs_dir.display()
        ));
    }

    let agents: Vec<String> = log_files
        .iter()
        .map(|f| {
            f.strip_prefix("agent-")
                .unwrap_or(f)
                .strip_suffix(".log")
                .unwrap_or(f)
                .to_string()
        })
        .collect();

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select agent logs to view")
        .items(&agents)
        .default(0)
        .interact()
        .context("Failed to get agent selection")?;

    Ok(agents[selection].clone())
}
