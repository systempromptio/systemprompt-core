use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::Args;
use std::path::PathBuf;
use systemprompt_core_logging::CliService;
use systemprompt_runtime::AppContext;

use super::{BotRow, BotsOutput};
use crate::commands::analytics::shared::{
    export_single_to_csv, format_number, format_percent, parse_time_range,
};
use crate::shared::{render_result, CommandResult};
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct BotsArgs {
    #[arg(long, default_value = "24h", help = "Time range")]
    pub since: Option<String>,

    #[arg(long, help = "End time")]
    pub until: Option<String>,

    #[arg(long, help = "Export to CSV")]
    pub export: Option<PathBuf>,
}

pub async fn execute(args: BotsArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let pool = ctx.db_pool().pool_arc()?;

    let (start, end) = parse_time_range(&args.since, &args.until)?;
    let output = fetch_bots(&pool, start, end).await?;

    if let Some(ref path) = args.export {
        export_single_to_csv(&output, path)?;
        CliService::success(&format!("Exported to {}", path.display()));
        return Ok(());
    }

    if config.is_json_output() {
        let result = CommandResult::card(output).with_title("Bot Traffic Analysis");
        render_result(&result);
    } else {
        render_bots(&output);
    }

    Ok(())
}

async fn fetch_bots(
    pool: &std::sync::Arc<sqlx::PgPool>,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<BotsOutput> {
    let totals: (i64, i64) = sqlx::query_as(
        r#"
        SELECT
            COUNT(*) FILTER (WHERE is_bot = false OR is_bot IS NULL) as human,
            COUNT(*) FILTER (WHERE is_bot = true) as bot
        FROM user_sessions
        WHERE started_at >= $1 AND started_at < $2
        "#,
    )
    .bind(start)
    .bind(end)
    .fetch_one(pool.as_ref())
    .await?;

    let total = totals.0 + totals.1;
    let bot_percentage = if total > 0 {
        (totals.1 as f64 / total as f64) * 100.0
    } else {
        0.0
    };

    let bot_types: Vec<(Option<String>, i64)> = sqlx::query_as(
        r#"
        SELECT
            COALESCE(
                CASE
                    WHEN user_agent ILIKE '%googlebot%' THEN 'Googlebot'
                    WHEN user_agent ILIKE '%bingbot%' THEN 'Bingbot'
                    WHEN user_agent ILIKE '%chatgpt%' THEN 'ChatGPT'
                    WHEN user_agent ILIKE '%claude%' THEN 'Claude'
                    WHEN user_agent ILIKE '%perplexity%' THEN 'Perplexity'
                    ELSE 'Other'
                END,
                'Unknown'
            ) as bot_type,
            COUNT(*) as count
        FROM user_sessions
        WHERE started_at >= $1 AND started_at < $2
          AND is_bot = true
        GROUP BY 1
        ORDER BY COUNT(*) DESC
        "#,
    )
    .bind(start)
    .bind(end)
    .fetch_all(pool.as_ref())
    .await
    .unwrap_or_default();

    let bot_breakdown: Vec<BotRow> = bot_types
        .into_iter()
        .map(|(bot_type, count)| {
            let percentage = if totals.1 > 0 {
                (count as f64 / totals.1 as f64) * 100.0
            } else {
                0.0
            };
            BotRow {
                bot_type: bot_type.unwrap_or_else(|| "Unknown".to_string()),
                request_count: count,
                percentage,
            }
        })
        .collect();

    Ok(BotsOutput {
        period: format!("{} to {}", start.format("%Y-%m-%d"), end.format("%Y-%m-%d")),
        human_sessions: totals.0,
        bot_sessions: totals.1,
        bot_percentage,
        bot_breakdown,
    })
}

fn render_bots(output: &BotsOutput) {
    CliService::section(&format!("Bot Traffic Analysis ({})", output.period));

    CliService::key_value("Human Sessions", &format_number(output.human_sessions));
    CliService::key_value("Bot Sessions", &format_number(output.bot_sessions));
    CliService::key_value("Bot Percentage", &format_percent(output.bot_percentage));

    if !output.bot_breakdown.is_empty() {
        CliService::subsection("Bot Breakdown");
        for bot in &output.bot_breakdown {
            CliService::key_value(
                &bot.bot_type,
                &format!(
                    "{} ({})",
                    format_number(bot.request_count),
                    format_percent(bot.percentage)
                ),
            );
        }
    }
}
