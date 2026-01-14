use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::Args;
use systemprompt_core_logging::CliService;
use systemprompt_runtime::AppContext;

use super::{RequestListOutput, RequestListRow};
use crate::commands::logs::duration::parse_since;
use crate::shared::{render_result, CommandResult};
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct ListArgs {
    #[arg(
        long,
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
    provider: String,
    model: String,
    input_tokens: Option<i32>,
    output_tokens: Option<i32>,
    cost_cents: i32,
    latency_ms: Option<i32>,
}

pub async fn execute(args: ListArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let pool = ctx.db_pool().pool_arc()?;

    let since_timestamp = parse_since(args.since.as_ref())?;

    let rows = if let Some(since_ts) = since_timestamp {
        sqlx::query_as!(
            AiRequestRow,
            r#"
            SELECT
                id as "id!",
                created_at as "created_at!",
                provider as "provider!",
                model as "model!",
                input_tokens,
                output_tokens,
                cost_cents as "cost_cents!",
                latency_ms
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
                provider as "provider!",
                model as "model!",
                input_tokens,
                output_tokens,
                cost_cents as "cost_cents!",
                latency_ms
            FROM ai_requests
            ORDER BY created_at DESC
            LIMIT $1
            "#,
            args.limit
        )
        .fetch_all(pool.as_ref())
        .await?
    };

    // Apply model and provider filters and transform
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
            let cost_dollars = f64::from(r.cost_cents) / 1_000_000.0;

            RequestListRow {
                request_id: truncate_id(&r.id),
                timestamp: r.created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
                provider: r.provider,
                model: r.model,
                tokens: format!("{}/{}", input, output),
                cost: format!("${:.6}", cost_dollars),
                latency_ms: r.latency_ms.map(i64::from),
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
            ]);
        render_result(&result);
    } else {
        render_text_output(&output);
    }

    Ok(())
}

fn truncate_id(id: &str) -> String {
    if id.len() > 12 {
        format!("{}...", &id[..12])
    } else {
        id.to_string()
    }
}

fn render_text_output(output: &RequestListOutput) {
    CliService::section("Recent AI Requests");

    for req in &output.requests {
        let latency = req
            .latency_ms
            .map_or_else(String::new, |ms| format!(" ({}ms)", ms));

        CliService::info(&format!(
            "{} | {} | {} | {} | {}{}",
            req.request_id, req.timestamp, req.model, req.tokens, req.cost, latency
        ));
    }

    CliService::info(&format!("Total: {} requests", output.total));
}
