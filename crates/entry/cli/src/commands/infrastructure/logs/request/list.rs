//! `infra logs request list` command.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use clap::Args;
use std::sync::Arc;
use systemprompt_runtime::{AiRequestFilter, RequestCursor, TraceQueryService};

use super::{RequestListRow, build_request_list};
use crate::commands::infrastructure::logs::duration::{parse_since, parse_until};
use crate::shared::CommandOutput;

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

    #[arg(
        long,
        help = "Only show requests before this time (same formats as --since; exclusive)"
    )]
    pub until: Option<String>,

    #[arg(
        long,
        value_name = "CURSOR",
        help = "Page past a previous result: the `cursor` of its last row (rows strictly older \
                than it are returned)"
    )]
    pub before: Option<String>,

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
    if let Some(until) = parse_until(args.until.as_ref())? {
        filter = filter.with_until(until);
    }
    if let Some(raw) = args.before.as_deref() {
        let cursor = RequestCursor::parse(raw).ok_or_else(|| {
            anyhow::anyhow!(
                "Invalid --before cursor: {raw}. Pass the `cursor` value of the last row from a \
                 previous `infra logs request list` page"
            )
        })?;
        filter = filter.with_before(cursor);
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
            let cached = r.cache_read_tokens.unwrap_or(0) + r.cache_creation_tokens.unwrap_or(0);
            let reasoning = r.reasoning_tokens.unwrap_or(0);
            let out = if reasoning > 0 {
                format!("{output}({reasoning}r)")
            } else {
                output.to_string()
            };
            let tokens = if cached > 0 {
                format!("{input}+{cached}c/{out}")
            } else {
                format!("{input}/{out}")
            };
            let cost_dollars = r.cost_microdollars as f64 / 1_000_000.0;

            let cursor = RequestCursor {
                created_at: r.created_at,
                id: r.id.clone(),
            }
            .to_string();

            RequestListRow {
                request_id: r.id.as_str().to_owned(),
                timestamp: r.created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
                cursor,
                user_id: r.user_id,
                actor: format!("{}:{}", r.actor_kind, r.actor_id),
                provider: r.provider.unwrap_or_else(|| "-".to_owned()),
                model: r.model.unwrap_or_else(|| "-".to_owned()),
                tokens,
                cost: format!("${cost_dollars:.6}"),
                latency_ms: r.latency_ms.map(i64::from),
                status: r.status,
            }
        })
        .collect();

    Ok(build_request_list(&requests))
}
