use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::Args;
use std::path::PathBuf;
use std::sync::Arc;
use systemprompt_core_logging::CliService;
use systemprompt_runtime::{AppContext, DatabaseContext};

use super::{ModelUsageRow, ModelsOutput};
use crate::commands::analytics::shared::{
    export_to_csv, format_cost, format_duration_ms, format_number, format_percent, format_tokens,
    parse_time_range,
};
use crate::shared::{render_result, CommandResult, RenderingHints};
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

pub async fn execute(args: ModelsArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let pool = ctx.db_pool().pool_arc()?;
    execute_internal(args, &pool, config).await
}

pub async fn execute_with_pool(
    args: ModelsArgs,
    db_ctx: &DatabaseContext,
    config: &CliConfig,
) -> Result<()> {
    let pool = db_ctx.db_pool().pool_arc()?;
    execute_internal(args, &pool, config).await
}

async fn execute_internal(
    args: ModelsArgs,
    pool: &Arc<sqlx::PgPool>,
    config: &CliConfig,
) -> Result<()> {
    let (start, end) = parse_time_range(args.since.as_ref(), args.until.as_ref())?;
    let output = fetch_models(pool, start, end, args.limit).await?;

    if let Some(ref path) = args.export {
        export_to_csv(&output.models, path)?;
        CliService::success(&format!("Exported to {}", path.display()));
        return Ok(());
    }

    if output.models.is_empty() {
        CliService::warning("No models found in the specified time range");
        return Ok(());
    }

    if config.is_json_output() {
        let hints = RenderingHints {
            columns: Some(vec![
                "provider".to_string(),
                "model".to_string(),
                "request_count".to_string(),
                "total_tokens".to_string(),
                "total_cost_cents".to_string(),
            ]),
            ..Default::default()
        };
        let result = CommandResult::table(output)
            .with_title("Model Usage")
            .with_hints(hints);
        render_result(&result);
    } else {
        render_models(&output);
    }

    Ok(())
}

async fn fetch_models(
    pool: &Arc<sqlx::PgPool>,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    limit: i64,
) -> Result<ModelsOutput> {
    let rows: Vec<(String, String, i64, Option<i64>, Option<i64>, Option<f64>)> = sqlx::query_as(
        r"
        SELECT
            provider,
            model,
            COUNT(*) as request_count,
            SUM(tokens_used) as total_tokens,
            SUM(cost_cents) as total_cost,
            AVG(latency_ms)::float8 as avg_latency
        FROM ai_requests
        WHERE created_at >= $1 AND created_at < $2
        GROUP BY provider, model
        ORDER BY COUNT(*) DESC
        LIMIT $3
        ",
    )
    .bind(start)
    .bind(end)
    .bind(limit)
    .fetch_all(pool.as_ref())
    .await?;

    let total_requests: i64 = rows.iter().map(|(_, _, c, _, _, _)| c).sum();

    let models: Vec<ModelUsageRow> = rows
        .into_iter()
        .map(
            |(provider, model, request_count, total_tokens, total_cost, avg_latency)| {
                let percentage = if total_requests > 0 {
                    (request_count as f64 / total_requests as f64) * 100.0
                } else {
                    0.0
                };

                ModelUsageRow {
                    provider,
                    model,
                    request_count,
                    total_tokens: total_tokens.unwrap_or(0),
                    total_cost_cents: total_cost.unwrap_or(0),
                    avg_latency_ms: avg_latency.map_or(0, |v| v as i64),
                    percentage,
                }
            },
        )
        .collect();

    Ok(ModelsOutput {
        period: format!(
            "{} to {}",
            start.format("%Y-%m-%d %H:%M"),
            end.format("%Y-%m-%d %H:%M")
        ),
        models,
        total_requests,
    })
}

fn render_models(output: &ModelsOutput) {
    CliService::section(&format!("Model Usage ({})", output.period));

    for model in &output.models {
        CliService::subsection(&format!("{}/{}", model.provider, model.model));
        CliService::key_value(
            "Requests",
            &format!(
                "{} ({})",
                format_number(model.request_count),
                format_percent(model.percentage)
            ),
        );
        CliService::key_value("Tokens", &format_tokens(model.total_tokens));
        CliService::key_value("Cost", &format_cost(model.total_cost_cents));
        CliService::key_value("Avg Latency", &format_duration_ms(model.avg_latency_ms));
    }

    CliService::info(&format!(
        "Total: {} requests",
        format_number(output.total_requests)
    ));
}
