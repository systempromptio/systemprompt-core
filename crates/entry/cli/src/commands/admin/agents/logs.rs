use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use clap::Args;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Select;

use super::types::AgentLogsOutput;
use crate::shared::{resolve_input, CommandResult};
use crate::CliConfig;
use systemprompt_logging::LoggingRepository;
use systemprompt_loader::ConfigLoader;
use systemprompt_runtime::AppContext;

const DEFAULT_LOGS_DIR: &str = "/var/www/html/tyingshoelaces/logs";

#[derive(Debug, Args)]
pub struct LogsArgs {
    #[arg(help = "Agent name (optional - shows all agent logs if not specified)")]
    pub agent: Option<String>,

    #[arg(
        long,
        short = 'n',
        default_value = "50",
        help = "Number of lines to show"
    )]
    pub lines: usize,

    #[arg(long, short, help = "Follow log output continuously (disk only)")]
    pub follow: bool,

    #[arg(long, help = "Force reading from disk files instead of database")]
    pub disk: bool,

    #[arg(long, help = "Custom logs directory path")]
    pub logs_dir: Option<String>,
}

pub async fn execute(args: LogsArgs, config: &CliConfig) -> Result<CommandResult<AgentLogsOutput>> {
    let logs_dir = args.logs_dir.as_deref().unwrap_or(DEFAULT_LOGS_DIR);
    let logs_path = PathBuf::from(logs_dir);

    if args.follow {
        return execute_follow_mode(&args, config, &logs_path);
    }

    if args.disk {
        return execute_disk_mode(&args, config, &logs_path);
    }

    match execute_db_mode(&args, config).await {
        Ok(result) => Ok(result),
        Err(e) => {
            tracing::debug!(error = %e, "DB log query failed, falling back to disk");
            execute_disk_mode(&args, config, &logs_path)
        },
    }
}

async fn execute_db_mode(
    args: &LogsArgs,
    _config: &CliConfig,
) -> Result<CommandResult<AgentLogsOutput>> {
    let ctx = Arc::new(
        AppContext::new()
            .await
            .context("Failed to initialize app context")?,
    );
    let repo = LoggingRepository::new(Arc::clone(ctx.db_pool()));

    let patterns = match &args.agent {
        Some(agent) => build_agent_patterns(agent),
        None => build_all_agent_patterns()?,
    };

    let entries = repo
        .get_logs_by_module_patterns(&patterns, args.lines as i64)
        .await
        .context("Failed to query logs from database")?;

    if entries.is_empty() {
        return Err(anyhow!("No logs found in database for agents"));
    }

    let logs: Vec<String> = entries
        .iter()
        .map(|e| {
            format!(
                "{} {} [{}] {}",
                e.timestamp.format("%Y-%m-%d %H:%M:%S"),
                e.level,
                e.module,
                e.message
            )
        })
        .collect();

    let agent_label = args.agent.clone().unwrap_or_else(|| "all".to_string());

    Ok(CommandResult::text(AgentLogsOutput {
        agent: Some(agent_label.clone()),
        source: "database".to_string(),
        logs,
        log_files: vec![],
    })
    .with_title(format!("Agent Logs (DB): {}", agent_label)))
}

fn build_agent_patterns(agent: &str) -> Vec<String> {
    vec![
        format!("%{}%", agent),
        "%agent%".to_string(),
        "%a2a%".to_string(),
    ]
}

fn build_all_agent_patterns() -> Result<Vec<String>> {
    let services_config = ConfigLoader::load().context("Failed to load services config")?;

    let mut patterns: Vec<String> = services_config
        .agents
        .keys()
        .flat_map(|name| vec![format!("%{}%", name)])
        .collect();

    patterns.push("%agent%".to_string());
    patterns.push("%a2a%".to_string());
    patterns.push("%orchestration%".to_string());

    Ok(patterns)
}

fn execute_disk_mode(
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

    let agent = resolve_input(args.agent.clone(), "agent", config, || {
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

fn execute_follow_mode(
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

    let agent = resolve_input(args.agent.clone(), "agent", config, || {
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
