use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::Args;
use std::sync::Arc;
use systemprompt_logging::CliService;
use systemprompt_runtime::{AppContext, DatabaseContext};

use super::{RequestListOutput, RequestListRow};
use crate::commands::infrastructure::logs::duration::parse_since;
use crate::commands::infrastructure::logs::shared::truncate_id;
use crate::shared::{render_result, CommandResult};
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct ListArgs {
    #[arg(
        long,
        short = 'n',
        default_value = "20",
        help = "Maximum number of requests to return"
    )]
    pub limit: i64,

    #[arg(
        long,
        help = "Only show requests since this duration (e.g., '1h', '24h', '7d')"
    )]
    pub since: Option<String>,

    #[arg(long, help = "Filter by model name (partial match)")]
    pub model: Option<String>,

    #[arg(long, help = "Filter by provider (e.g., 'openai', 'anthropic')")]
    pub provider: Option<String>,
}

struct AiRequestRow {
    id: String,
    created_at: DateTime<Utc>,
    trace_id: Option<String>,
    provider: String,
    model: String,
    input_tokens: Option<i32>,
    output_tokens: Option<i32>,
    cost_microdollars: i64,
    latency_ms: Option<i32>,
    status: String,
}

pub async fn execute(args: ListArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let pool = ctx.db_pool().pool_arc()?;
    execute_with_pool_inner(args, &pool, config).await
}

pub async fn execute_with_pool(
    args: ListArgs,
    db_ctx: &DatabaseContext,
    config: &CliConfig,
) -> Result<()> {
    let pool = db_ctx.db_pool().pool_arc()?;
    execute_with_pool_inner(args, &pool, config).await
}

async fn execute_with_pool_inner(
    args: ListArgs,
    pool: &Arc<sqlx::PgPool>,
    config: &CliConfig,
) -> Result<()> {
    let since_timestamp = parse_since(args.since.as_ref())?;

    let rows = if let Some(since_ts) = since_timestamp {
        sqlx::query_as!(
            AiRequestRow,
            r#"
            SELECT
                id as "id!",
                created_at as "created_at!",
                trace_id,
                provider as "provider!",
                model as "model!",
                input_tokens,
                output_tokens,
                cost_microdollars as "cost_microdollars!",
                latency_ms,
                status as "status!"
            FROM ai_requests
            WHERE created_at >= $1
            ORDER BY created_at DESC
            LIMIT $2
            "#,
            since_ts,
            args.limit
        )
        .fetch_all(pool.as_ref())
        .await?
    } else {
        sqlx::query_as!(
            AiRequestRow,
            r#"
            SELECT
                id as "id!",
                created_at as "created_at!",
                trace_id,
                provider as "provider!",
                model as "model!",
                input_tokens,
                output_tokens,
                cost_microdollars as "cost_microdollars!",
                latency_ms,
                status as "status!"
            FROM ai_requests
            ORDER BY created_at DESC
            LIMIT $1
            "#,
            args.limit
        )
        .fetch_all(pool.as_ref())
        .await?
    };

    // Store trace_id for single-result hint
    let single_trace_id = if args.limit == 1 && rows.len() == 1 {
        rows[0].trace_id.clone()
    } else {
        None
    };

    let requests: Vec<RequestListRow> = rows
        .into_iter()
        .filter(|r| {
            if let Some(ref model) = args.model {
                if !r.model.to_lowercase().contains(&model.to_lowercase()) {
                    return false;
                }
            }
            if let Some(ref provider) = args.provider {
                if !r.provider.to_lowercase().contains(&provider.to_lowercase()) {
                    return false;
                }
            }
            true
        })
        .map(|r| {
            let input = r.input_tokens.unwrap_or(0);
            let output = r.output_tokens.unwrap_or(0);
            let cost_dollars = r.cost_microdollars as f64 / 1_000_000.0;

            RequestListRow {
                request_id: truncate_id(&r.id, 12),
                timestamp: r.created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
                provider: r.provider,
                model: r.model,
                tokens: format!("{}/{}", input, output),
                cost: format!("${:.6}", cost_dollars),
                latency_ms: r.latency_ms.map(i64::from),
                status: r.status,
            }
        })
        .collect();

    let output = RequestListOutput {
        total: requests.len() as u64,
        requests,
    };

    if output.requests.is_empty() {
        CliService::warning("No AI requests found");
        return Ok(());
    }

    if config.is_json_output() {
        let result = CommandResult::table(output)
            .with_title("AI Requests")
            .with_columns(vec![
                "request_id".to_string(),
                "timestamp".to_string(),
                "provider".to_string(),
                "model".to_string(),
                "tokens".to_string(),
                "cost".to_string(),
                "latency_ms".to_string(),
                "status".to_string(),
            ]);
        render_result(&result);
    } else {
        render_text_output(&output, single_trace_id.as_deref());
    }

    Ok(())
}

fn render_text_output(output: &RequestListOutput, trace_hint: Option<&str>) {
    CliService::section("Recent AI Requests");

    for req in &output.requests {
        let latency = req
            .latency_ms
            .map_or_else(String::new, |ms| format!(" ({}ms)", ms));

        let status_indicator = if req.status == "failed" {
            " [FAILED]"
        } else {
            ""
        };

        CliService::info(&format!(
            "{} | {} | {} | {} | {}{}{}",
            req.request_id,
            req.timestamp,
            req.model,
            req.tokens,
            req.cost,
            latency,
            status_indicator
        ));
    }

    CliService::info(&format!("Total: {} requests", output.total));

    // Show trace hint for single-result queries
    if let Some(trace_id) = trace_hint {
        CliService::info(&format!(
            "For full trace: systemprompt infra logs trace show {} --all",
            trace_id
        ));
    }
}
