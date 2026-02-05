use anyhow::Result;
use clap::Args;
use std::path::PathBuf;
use systemprompt_analytics::TrafficAnalyticsRepository;
use systemprompt_logging::CliService;
use systemprompt_runtime::{AppContext, DatabaseContext};

use super::{DeviceRow, DevicesOutput};
use crate::commands::analytics::shared::{export_to_csv, parse_time_range, resolve_export_path};
use crate::shared::{CommandResult, RenderingHints};
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct DevicesArgs {
    #[arg(long, default_value = "24h", help = "Time range")]
    pub since: Option<String>,

    #[arg(long, help = "End time")]
    pub until: Option<String>,

    #[arg(long, short = 'n', default_value = "20", help = "Maximum devices")]
    pub limit: i64,

    #[arg(long, help = "Export to CSV")]
    pub export: Option<PathBuf>,

    #[arg(long, help = "Include all sessions (ghost sessions, suspected bots that evaded detection)")]
    pub include_all: bool,
}

pub async fn execute(
    args: DevicesArgs,
    _config: &CliConfig,
) -> Result<CommandResult<DevicesOutput>> {
    let ctx = AppContext::new().await?;
    let repo = TrafficAnalyticsRepository::new(ctx.db_pool())?;
    execute_internal(args, &repo).await
}

pub async fn execute_with_pool(
    args: DevicesArgs,
    db_ctx: &DatabaseContext,
    _config: &CliConfig,
) -> Result<CommandResult<DevicesOutput>> {
    let repo = TrafficAnalyticsRepository::new(db_ctx.db_pool())?;
    execute_internal(args, &repo).await
}

async fn execute_internal(
    args: DevicesArgs,
    repo: &TrafficAnalyticsRepository,
) -> Result<CommandResult<DevicesOutput>> {
    let (start, end) = parse_time_range(args.since.as_ref(), args.until.as_ref())?;
    let engaged_only = !args.include_all;

    let rows = repo.get_device_breakdown(start, end, args.limit, engaged_only).await?;

    let total: i64 = rows.iter().map(|r| r.count).sum();

    let devices: Vec<DeviceRow> = rows
        .into_iter()
        .map(|row| {
            let percentage = if total > 0 {
                (row.count as f64 / total as f64) * 100.0
            } else {
                0.0
            };
            DeviceRow {
                device_type: row.device.unwrap_or_else(|| "unknown".to_string()),
                browser: row.browser.unwrap_or_else(|| "unknown".to_string()),
                session_count: row.count,
                percentage,
            }
        })
        .collect();

    let output = DevicesOutput {
        period: format!("{} to {}", start.format("%Y-%m-%d"), end.format("%Y-%m-%d")),
        devices,
        total_sessions: total,
    };

    if let Some(ref path) = args.export {
        let resolved_path = resolve_export_path(path)?;
        export_to_csv(&output.devices, &resolved_path)?;
        CliService::success(&format!("Exported to {}", resolved_path.display()));
        return Ok(CommandResult::table(output).with_skip_render());
    }

    let hints = RenderingHints {
        columns: Some(vec![
            "device_type".to_string(),
            "browser".to_string(),
            "session_count".to_string(),
        ]),
        ..Default::default()
    };

    Ok(CommandResult::table(output)
        .with_title("Device Breakdown")
        .with_hints(hints))
}
