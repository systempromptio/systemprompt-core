use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use clap::{Args, ValueEnum};
use dialoguer::theme::ColorfulTheme;
use dialoguer::Select;

use super::types::McpLogsOutput;
use crate::shared::{resolve_input, CommandResult};
use crate::CliConfig;
use systemprompt_logging::LoggingRepository;
use systemprompt_loader::ConfigLoader;
use systemprompt_models::AppPaths;
use systemprompt_runtime::AppContext;

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    fn matches(&self, level_str: &str) -> bool {
        let level_upper = level_str.to_uppercase();
        match self {
            Self::Debug => true, // Show all levels
            Self::Info => !level_upper.contains("DEBUG"),
            Self::Warn => level_upper.contains("WARN") || level_upper.contains("ERROR"),
            Self::Error => level_upper.contains("ERROR"),
        }
    }
}

#[derive(Debug, Args)]
pub struct LogsArgs {
    #[arg(help = "MCP server name (optional - shows all MCP logs if not specified)")]
    pub service: Option<String>,

    #[arg(
        long,
        short = 'n',
        visible_alias = "tail",
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

    #[arg(
        long,
        value_enum,
        help = "Filter by log level: debug, info, warn, error"
    )]
    pub level: Option<LogLevel>,
}

fn get_default_logs_dir() -> PathBuf {
    AppPaths::get().map_or_else(|_| PathBuf::from("/var/log"), |paths| paths.system().logs())
}

pub async fn execute(args: LogsArgs, config: &CliConfig) -> Result<CommandResult<McpLogsOutput>> {
    let logs_path = args
        .logs_dir
        .as_ref()
        .map_or_else(get_default_logs_dir, PathBuf::from);

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
) -> Result<CommandResult<McpLogsOutput>> {
    let ctx = Arc::new(
        AppContext::new()
            .await
            .context("Failed to initialize app context")?,
    );
    let repo = LoggingRepository::new(Arc::clone(ctx.db_pool()));

    let patterns = match &args.service {
        Some(service) => build_service_patterns(service),
        None => build_all_mcp_patterns()?,
    };

    let fetch_limit = if args.level.is_some() {
        (args.lines * 5) as i64
    } else {
        args.lines as i64
    };

    let entries = repo
        .get_logs_by_module_patterns(&patterns, fetch_limit)
        .await
        .context("Failed to query logs from database")?;

    if entries.is_empty() {
        return Err(anyhow!("No logs found in database for MCP services"));
    }

    let logs: Vec<String> = entries
        .iter()
        .filter(|e| {
            args.level
                .is_none_or(|level| level.matches(&e.level.to_string()))
        })
        .take(args.lines)
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

    let service_label = args.service.clone().unwrap_or_else(|| "all".to_string());
    let level_label = args.level.map_or_else(String::new, |l| {
        format!(" [{}+]", format!("{:?}", l).to_uppercase())
    });

    Ok(CommandResult::text(McpLogsOutput {
        service: Some(service_label.clone()),
        source: "database".to_string(),
        logs,
        log_files: vec![],
    })
    .with_title(format!("MCP Logs (DB): {}{}", service_label, level_label)))
}

fn build_service_patterns(service: &str) -> Vec<String> {
    vec![format!("%{}%", service), format!("%rmcp%")]
}

fn build_all_mcp_patterns() -> Result<Vec<String>> {
    let services_config = ConfigLoader::load().context("Failed to load services config")?;

    let mut patterns: Vec<String> = services_config
        .mcp_servers
        .keys()
        .flat_map(|name| vec![format!("%{}%", name)])
        .collect();

    patterns.push("%rmcp%".to_string());
    patterns.push("%mcp%".to_string());

    Ok(patterns)
}

fn execute_disk_mode(
    args: &LogsArgs,
    config: &CliConfig,
    logs_path: &Path,
) -> Result<CommandResult<McpLogsOutput>> {
    if !logs_path.exists() {
        return Err(anyhow!(
            "Logs directory does not exist: {}",
            logs_path.display()
        ));
    }

    if args.service.is_none() && !config.is_interactive() {
        let log_files = list_mcp_log_files(logs_path)?;
        return Ok(CommandResult::list(McpLogsOutput {
            service: None,
            source: "disk".to_string(),
            logs: vec![],
            log_files,
        })
        .with_title("Available MCP Log Files"));
    }

    let service = resolve_input(args.service.clone(), "service", config, || {
        prompt_log_selection(logs_path)
    })?;

    let log_file = find_log_file(logs_path, &service)?;
    let logs = read_log_lines(&log_file, args.lines, args.level)?;

    let level_label = args.level.map_or_else(String::new, |l| {
        format!(" [{}+]", format!("{:?}", l).to_uppercase())
    });

    Ok(CommandResult::text(McpLogsOutput {
        service: Some(service.clone()),
        source: "disk".to_string(),
        logs,
        log_files: vec![],
    })
    .with_title(format!("MCP Logs (Disk): {}{}", service, level_label)))
}

fn execute_follow_mode(
    args: &LogsArgs,
    config: &CliConfig,
    logs_path: &Path,
) -> Result<CommandResult<McpLogsOutput>> {
    if !logs_path.exists() {
        return Err(anyhow!(
            "Logs directory does not exist: {}",
            logs_path.display()
        ));
    }

    let service = resolve_input(args.service.clone(), "service", config, || {
        prompt_log_selection(logs_path)
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

    Ok(CommandResult::text(McpLogsOutput {
        service: Some(service),
        source: "disk".to_string(),
        logs: vec![],
        log_files: vec![],
    })
    .with_title("MCP Logs"))
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

fn prompt_log_selection(logs_dir: &Path) -> Result<String> {
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
