use anyhow::Result;
use clap::Args;
use std::path::PathBuf;
use systemprompt_core_analytics::TrafficAnalyticsRepository;
use systemprompt_core_logging::CliService;
use systemprompt_runtime::{AppContext, DatabaseContext};

use super::{DeviceRow, DevicesOutput};
use crate::commands::analytics::shared::{
    export_to_csv, format_number, format_percent, parse_time_range,
};
use crate::shared::{render_result, CommandResult, RenderingHints};
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
}

pub async fn execute(args: DevicesArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let repo = TrafficAnalyticsRepository::new(ctx.db_pool())?;
    execute_internal(args, &repo, config).await
}

pub async fn execute_with_pool(
    args: DevicesArgs,
    db_ctx: &DatabaseContext,
    config: &CliConfig,
) -> Result<()> {
    let repo = TrafficAnalyticsRepository::new(db_ctx.db_pool())?;
    execute_internal(args, &repo, config).await
}

async fn execute_internal(
    args: DevicesArgs,
    repo: &TrafficAnalyticsRepository,
    config: &CliConfig,
) -> Result<()> {
    let (start, end) = parse_time_range(args.since.as_ref(), args.until.as_ref())?;

    let rows = repo.get_device_breakdown(start, end, args.limit).await?;

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
        export_to_csv(&output.devices, path)?;
        CliService::success(&format!("Exported to {}", path.display()));
        return Ok(());
    }

    if config.is_json_output() {
        let hints = RenderingHints {
            columns: Some(vec![
                "device_type".to_string(),
                "browser".to_string(),
                "session_count".to_string(),
            ]),
            ..Default::default()
        };
        let result = CommandResult::table(output)
            .with_title("Device Breakdown")
            .with_hints(hints);
        render_result(&result);
    } else {
        render_devices(&output);
    }

    Ok(())
}

fn render_devices(output: &DevicesOutput) {
    CliService::section(&format!("Device Breakdown ({})", output.period));
    CliService::key_value("Total Sessions", &format_number(output.total_sessions));

    for device in &output.devices {
        CliService::key_value(
            &format!("{} / {}", device.device_type, device.browser),
            &format!(
                "{} ({})",
                format_number(device.session_count),
                format_percent(device.percentage)
            ),
        );
    }
}
