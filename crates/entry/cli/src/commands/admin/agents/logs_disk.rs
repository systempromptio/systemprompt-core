use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, anyhow};

use super::logs::LogsArgs;
use super::types::AgentLogsOutput;
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

    if args.agent.is_none() && !config.is_interactive() {
        let log_files = list_agent_log_files(logs_path)?;
        let output = AgentLogsOutput {
            agent: None,
            source: "disk".to_owned(),
            logs: vec![],
            log_files,
        };
        let items: Vec<ListItem> = output
            .log_files
            .iter()
            .map(|file| ListItem::new(file.clone(), String::new(), file.clone()))
            .collect();
        return Ok(CommandOutput::list(items).with_title("Available Agent Log Files"));
    }

    let agent = resolve_required(args.agent.clone(), "agent", config, || {
        prompt_log_selection(prompter, logs_path)
    })?;

    let log_file = find_log_file(logs_path, &agent)?;
    let logs = read_log_lines(&log_file, args.lines)?;

    Ok(CommandOutput::card_value(
        format!("Agent Logs (Disk): {}", agent),
        &AgentLogsOutput {
            agent: Some(agent),
            source: "disk".to_owned(),
            logs,
            log_files: vec![],
        },
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

    let agent = resolve_required(args.agent.clone(), "agent", config, || {
        prompt_log_selection(prompter, logs_path)
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

    Ok(CommandOutput::card_value(
        "Agent Logs",
        &AgentLogsOutput {
            agent: Some(agent),
            source: "disk".to_owned(),
            logs: vec![],
            log_files: vec![],
        },
    ))
}

fn list_agent_log_files(logs_dir: &Path) -> Result<Vec<String>> {
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

fn prompt_log_selection(prompter: &dyn Prompter, logs_dir: &Path) -> Result<String> {
    let log_files = list_agent_log_files(logs_dir)?;
    select_agent_from_log_files(prompter, &log_files, logs_dir)
}

pub fn select_agent_from_log_files(
    prompter: &dyn Prompter,
    log_files: &[String],
    logs_dir: &Path,
) -> Result<String> {
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
                .to_owned()
        })
        .collect();

    let selection = prompter
        .select("Select agent logs to view", &agents)
        .context("Failed to get agent selection")?;

    Ok(agents[selection].clone())
}
