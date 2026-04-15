use anyhow::Result;
use clap::{Args, ValueEnum};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use systemprompt_logging::TraceQueryService;

use super::duration::parse_since;
use super::{LogEntryRow, LogExportOutput};
use crate::shared::CommandResult;

#[derive(Debug, Clone, Copy, Default, ValueEnum)]
pub enum ExportFormat {
    #[default]
    Json,
    Csv,
    Jsonl,
}

#[derive(Debug, Args)]
pub struct ExportArgs {
    #[arg(
        long,
        short = 'f',
        value_enum,
        default_value = "json",
        help = "Export format"
    )]
    pub format: ExportFormat,

    #[arg(long, short = 'o', help = "Output file path (stdout if not specified)")]
    pub output: Option<PathBuf>,

    #[arg(long, help = "Filter by log level")]
    pub level: Option<String>,

    #[arg(long, help = "Filter by module name")]
    pub module: Option<String>,

    #[arg(
        long,
        help = "Only export logs since this duration (e.g., '1h', '24h', '7d')"
    )]
    pub since: Option<String>,

    #[arg(
        long,
        default_value = "10000",
        help = "Maximum number of logs to export"
    )]
    pub limit: i64,
}

crate::define_pool_command!(ExportArgs => CommandResult<LogExportOutput>, no_config);

async fn execute_with_pool_inner(
    args: ExportArgs,
    pool: &Arc<sqlx::PgPool>,
) -> Result<CommandResult<LogExportOutput>> {
    let since_timestamp = parse_since(args.since.as_ref())?;
    let level_filter = args.level.as_deref().map(str::to_uppercase);

    let service = TraceQueryService::new(Arc::clone(pool));
    let entries = service
        .list_logs_filtered(since_timestamp, level_filter.as_deref(), args.limit)
        .await?;

    let logs: Vec<LogEntryRow> = entries
        .into_iter()
        .filter(|e| {
            args.module
                .as_ref()
                .is_none_or(|module| e.module.contains(module))
        })
        .map(|e| LogEntryRow {
            id: e.id.clone(),
            trace_id: e.trace_id.clone(),
            timestamp: e.timestamp.format("%Y-%m-%d %H:%M:%S%.3f").to_string(),
            level: e.level.to_string().to_uppercase(),
            module: e.module,
            message: e.message,
            metadata: e.metadata,
        })
        .collect();

    let exported_count = logs.len() as u64;
    let format_str = match args.format {
        ExportFormat::Json => "json",
        ExportFormat::Csv => "csv",
        ExportFormat::Jsonl => "jsonl",
    };

    let content = match args.format {
        ExportFormat::Json => serde_json::to_string_pretty(&logs)?,
        ExportFormat::Jsonl => logs
            .iter()
            .map(serde_json::to_string)
            .collect::<Result<Vec<_>, _>>()?
            .join("\n"),
        ExportFormat::Csv => format_csv(&logs),
    };

    if let Some(ref path) = args.output {
        let mut file = std::fs::File::create(path)?;
        file.write_all(content.as_bytes())?;

        let output = LogExportOutput {
            exported_count,
            format: format_str.to_string(),
            file_path: Some(path.display().to_string()),
        };

        Ok(CommandResult::card(output).with_title("Logs Exported"))
    } else {
        std::io::stdout().write_all(content.as_bytes())?;
        std::io::stdout().write_all(b"\n")?;

        let output = LogExportOutput {
            exported_count,
            format: format_str.to_string(),
            file_path: None,
        };

        Ok(CommandResult::text(output)
            .with_title("Logs Exported")
            .with_skip_render())
    }
}

fn format_csv(logs: &[LogEntryRow]) -> String {
    let mut output = String::from("id,trace_id,timestamp,level,module,message\n");

    for log in logs {
        let escaped_message = log.message.replace('"', "\"\"");
        output.push_str(&format!(
            "{},{},{},{},{},\"{}\"\n",
            log.id, log.trace_id, log.timestamp, log.level, log.module, escaped_message
        ));
    }

    output
}
