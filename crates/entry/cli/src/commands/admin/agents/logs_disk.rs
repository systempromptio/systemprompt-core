use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, anyhow};

use super::logs::LogsArgs;
use super::types::AgentLogsOutput;
use crate::CliConfig;
use crate::interactive::{Prompter, resolve_required};
use crate::shared::CommandOutput;
use crate::shared::disk_logs::{display_names, find_log_file, list_log_files, read_log_lines};
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
        let log_files = list_log_files(logs_path, "agent-")?;
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

    let log_file = find_log_file(logs_path, "agent-", &agent)?;
    let logs = read_log_lines(&log_file, args.lines, |_| true)?;

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

    let log_file = find_log_file(logs_path, "agent-", &agent)?;

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

fn prompt_log_selection(prompter: &dyn Prompter, logs_dir: &Path) -> Result<String> {
    let log_files = list_log_files(logs_dir, "agent-")?;
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

    let agents = display_names(log_files, "agent-");

    let selection = prompter
        .select("Select agent logs to view", &agents)
        .context("Failed to get agent selection")?;

    Ok(agents[selection].clone())
}
