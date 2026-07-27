//! `infra logs request list` command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use clap::Args;
use std::sync::Arc;
use systemprompt_logging::{AiRequestFilter, TraceQueryService};

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

    #[arg(long, help = "Filter by user id (exact match)")]
    pub user: Option<String>,
}

crate::define_pool_command!(ListArgs => CommandOutput, no_config);

async fn execute_with_pool_inner(
    args: ListArgs,
    pool: &Arc<sqlx::PgPool>,
) -> Result<CommandOutput> {
    let mut filter = AiRequestFilter::new(args.limit);
    if let Some(since) = parse_since(args.since.as_ref())? {
        filter = filter.with_since(since);
    }
    if let Some(model) = args.model.as_ref() {
        filter = filter.with_model(format!("%{model}%"));
    }
    if let Some(provider) = args.provider.as_ref() {
        filter = filter.with_provider(format!("%{provider}%"));
    }
    if let Some(user) = args.user {
        filter = filter.with_user(user);
    }

    let service = TraceQueryService::new(Arc::clone(pool));
    let rows = service.list_ai_requests(&filter).await?;

    let requests: Vec<RequestListRow> = rows
        .into_iter()
        .map(|r| {
            let input = r.input_tokens.unwrap_or(0);
            let output = r.output_tokens.unwrap_or(0);
            let cost_dollars = r.cost_microdollars as f64 / 1_000_000.0;

            RequestListRow {
                request_id: truncate_with_ellipsis(r.id.as_str(), 12),
                timestamp: r.created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
                user_id: r.user_id,
                actor: format!("{}:{}", r.actor_kind, r.actor_id),
                provider: r.provider.unwrap_or_else(|| "-".to_owned()),
                model: r.model.unwrap_or_else(|| "-".to_owned()),
                tokens: format!("{input}/{output}"),
                cost: format!("${cost_dollars:.6}"),
                latency_ms: r.latency_ms.map(i64::from),
                status: r.status,
            }
        })
        .collect();

    Ok(build_request_list(&requests))
}
