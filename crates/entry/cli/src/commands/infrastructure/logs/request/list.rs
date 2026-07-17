//! `infra logs request list` command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use clap::Args;
use std::sync::Arc;
use systemprompt_logging::TraceQueryService;

use super::{RequestListRow, build_request_list};
use crate::commands::infrastructure::logs::duration::parse_since;
use crate::shared::CommandOutput;
use systemprompt_models::text::truncate_with_ellipsis;

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

crate::define_pool_command!(ListArgs => CommandOutput, no_config);

async fn execute_with_pool_inner(
    args: ListArgs,
    pool: &Arc<sqlx::PgPool>,
) -> Result<CommandOutput> {
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

    let requests: Vec<RequestListRow> = rows
        .into_iter()
        .map(|r| {
            let input = r.input_tokens.unwrap_or(0);
            let output = r.output_tokens.unwrap_or(0);
            let cost_dollars = r.cost_microdollars as f64 / 1_000_000.0;

            RequestListRow {
                request_id: truncate_with_ellipsis(r.id.as_str(), 12),
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

    Ok(build_request_list(&requests))
}
