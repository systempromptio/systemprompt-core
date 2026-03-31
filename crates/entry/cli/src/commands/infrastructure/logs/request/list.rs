use anyhow::Result;
use clap::Args;
use std::sync::Arc;
use systemprompt_logging::{CliService, TraceQueryService};
use systemprompt_runtime::{AppContext, DatabaseContext};

use super::{RequestListOutput, RequestListRow};
use crate::CliConfig;
use crate::commands::infrastructure::logs::duration::parse_since;
use crate::commands::infrastructure::logs::shared::truncate_id;
use crate::shared::CommandResult;

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

pub async fn execute(
    args: ListArgs,
    config: &CliConfig,
) -> Result<CommandResult<RequestListOutput>> {
    let ctx = AppContext::new().await?;
    let pool = ctx.db_pool().pool_arc()?;
    execute_with_pool_inner(args, &pool, config).await
}

pub async fn execute_with_pool(
    args: ListArgs,
    db_ctx: &DatabaseContext,
    config: &CliConfig,
) -> Result<CommandResult<RequestListOutput>> {
    let pool = db_ctx.db_pool().pool_arc()?;
    execute_with_pool_inner(args, &pool, config).await
}

async fn execute_with_pool_inner(
    args: ListArgs,
    pool: &Arc<sqlx::PgPool>,
    config: &CliConfig,
) -> Result<CommandResult<RequestListOutput>> {
    let since_timestamp = parse_since(args.since.as_ref())?;
    let model_pattern = args.model.as_ref().map(|m| format!("%{m}%"));
    let provider_pattern = args.provider.as_ref().map(|p| format!("%{p}%"));

    let service = TraceQueryService::new(Arc::clone(pool));
    let rows = service
        .list_ai_requests(
            since_timestamp,
            model_pattern.as_deref(),
            provider_pattern.as_deref(),
            args.limit,
        )
        .await?;

    let single_trace_id = if args.limit == 1 && rows.len() == 1 {
        rows[0].trace_id.as_ref().map(|id| id.to_string())
    } else {
        None
    };

    let requests: Vec<RequestListRow> = rows
        .into_iter()
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

    if result.data.requests.is_empty() {
        if !config.is_json_output() {
            CliService::warning("No AI requests found");
        }
        return Ok(result.with_skip_render());
    }

    if config.is_json_output() {
        return Ok(result);
    }

    render_text_output(&result.data, single_trace_id.as_deref());
    Ok(result.with_skip_render())
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

    if let Some(trace_id) = trace_hint {
        CliService::info(&format!(
            "For full trace: systemprompt infra logs trace show {} --all",
            trace_id
        ));
    }
}
