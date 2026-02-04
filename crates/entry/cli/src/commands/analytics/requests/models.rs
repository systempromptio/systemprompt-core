use anyhow::Result;
use clap::Args;
use std::path::PathBuf;
use systemprompt_analytics::RequestAnalyticsRepository;
use systemprompt_logging::CliService;
use systemprompt_runtime::{AppContext, DatabaseContext};

use super::{ModelUsageRow, ModelsOutput};
use crate::commands::analytics::shared::{export_to_csv, parse_time_range, resolve_export_path};
use crate::shared::{CommandResult, RenderingHints};
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct ModelsArgs {
    #[arg(
        long,
        default_value = "24h",
        help = "Time range (e.g., '1h', '24h', '7d')"
    )]
    pub since: Option<String>,

    #[arg(long, help = "End time for range")]
    pub until: Option<String>,

    #[arg(
        long,
        short = 'n',
        default_value = "20",
        help = "Maximum number of models"
    )]
    pub limit: i64,

    #[arg(long, help = "Export results to CSV file")]
    pub export: Option<PathBuf>,
}

pub async fn execute(args: ModelsArgs, _config: &CliConfig) -> Result<CommandResult<ModelsOutput>> {
    let ctx = AppContext::new().await?;
    let repo = RequestAnalyticsRepository::new(ctx.db_pool())?;
    execute_internal(args, &repo).await
}

pub async fn execute_with_pool(
    args: ModelsArgs,
    db_ctx: &DatabaseContext,
    _config: &CliConfig,
) -> Result<CommandResult<ModelsOutput>> {
    let repo = RequestAnalyticsRepository::new(db_ctx.db_pool())?;
    execute_internal(args, &repo).await
}

async fn execute_internal(
    args: ModelsArgs,
    repo: &RequestAnalyticsRepository,
) -> Result<CommandResult<ModelsOutput>> {
    let (start, end) = parse_time_range(args.since.as_ref(), args.until.as_ref())?;

    let rows = repo.list_models(start, end, args.limit).await?;

    let total_requests: i64 = rows.iter().map(|r| r.request_count).sum();

    let models: Vec<ModelUsageRow> = rows
        .into_iter()
        .map(|row| {
            let percentage = if total_requests > 0 {
                (row.request_count as f64 / total_requests as f64) * 100.0
            } else {
                0.0
            };

            ModelUsageRow {
                provider: row.provider,
                model: row.model,
                request_count: row.request_count,
                total_tokens: row.total_tokens.unwrap_or(0),
                total_cost_microdollars: row.total_cost.unwrap_or(0),
                avg_latency_ms: row.avg_latency.map_or(0, |v| v as i64),
                percentage,
            }
        })
        .collect();

    let output = ModelsOutput {
        period: format!(
            "{} to {}",
            start.format("%Y-%m-%d %H:%M"),
            end.format("%Y-%m-%d %H:%M")
        ),
        models,
        total_requests,
    };

    if let Some(ref path) = args.export {
        let resolved_path = resolve_export_path(path)?;
        export_to_csv(&output.models, &resolved_path)?;
        CliService::success(&format!("Exported to {}", resolved_path.display()));
        return Ok(CommandResult::table(output).with_skip_render());
    }

    if output.models.is_empty() {
        CliService::warning("No models found in the specified time range");
        return Ok(CommandResult::table(output).with_skip_render());
    }

    let hints = RenderingHints {
        columns: Some(vec![
            "provider".to_string(),
            "model".to_string(),
            "request_count".to_string(),
            "total_tokens".to_string(),
            "total_cost_microdollars".to_string(),
        ]),
        ..Default::default()
    };

    Ok(CommandResult::table(output)
        .with_title("Model Usage")
        .with_hints(hints))
}
