use anyhow::Result;
use clap::Args;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_runtime::AppContext;

use crate::commands::logs::duration::parse_since;
use crate::shared::{render_result, CommandResult, RenderingHints};
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct StatsArgs {
    #[arg(
        long,
        help = "Only include requests since this duration (e.g., '1h', '24h', '7d')"
    )]
    pub since: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RequestStatsOutput {
    pub total_requests: i64,
    pub total_tokens: TokenStats,
    pub total_cost_dollars: f64,
    pub average_latency_ms: i64,
    pub by_provider: Vec<ProviderStats>,
    pub by_model: Vec<ModelStats>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub struct TokenStats {
    pub input: i64,
    pub output: i64,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProviderStats {
    pub provider: String,
    pub request_count: i64,
    pub total_tokens: i64,
    pub total_cost_dollars: f64,
    pub avg_latency_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ModelStats {
    pub model: String,
    pub provider: String,
    pub request_count: i64,
    pub total_tokens: i64,
    pub total_cost_dollars: f64,
    pub avg_latency_ms: i64,
}

struct TotalRow {
    request_count: Option<i64>,
    total_input_tokens: Option<i64>,
    total_output_tokens: Option<i64>,
    total_cost_cents: Option<i64>,
    avg_latency_ms: Option<i64>,
}

struct ProviderRow {
    provider: String,
    request_count: Option<i64>,
    total_tokens: Option<i64>,
    total_cost_cents: Option<i64>,
    avg_latency_ms: Option<i64>,
}

struct ModelRow {
    model: String,
    provider: String,
    request_count: Option<i64>,
    total_tokens: Option<i64>,
    total_cost_cents: Option<i64>,
    avg_latency_ms: Option<i64>,
}

pub async fn execute(args: StatsArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let pool = ctx.db_pool().pool_arc()?;

    let since_timestamp = parse_since(args.since.as_ref())?;

    // Get totals
    let totals = if let Some(since_ts) = since_timestamp {
        sqlx::query_as!(
            TotalRow,
            r#"
            SELECT
                COUNT(*) as "request_count",
                COALESCE(SUM(input_tokens), 0) as "total_input_tokens",
                COALESCE(SUM(output_tokens), 0) as "total_output_tokens",
                COALESCE(SUM(cost_cents), 0) as "total_cost_cents",
                COALESCE(AVG(latency_ms), 0)::bigint as "avg_latency_ms"
            FROM ai_requests
            WHERE created_at >= $1
            "#,
            since_ts
        )
        .fetch_one(pool.as_ref())
        .await?
    } else {
        sqlx::query_as!(
            TotalRow,
            r#"
            SELECT
                COUNT(*) as "request_count",
                COALESCE(SUM(input_tokens), 0) as "total_input_tokens",
                COALESCE(SUM(output_tokens), 0) as "total_output_tokens",
                COALESCE(SUM(cost_cents), 0) as "total_cost_cents",
                COALESCE(AVG(latency_ms), 0)::bigint as "avg_latency_ms"
            FROM ai_requests
            "#
        )
        .fetch_one(pool.as_ref())
        .await?
    };

    // Get by provider
    let by_provider = if let Some(since_ts) = since_timestamp {
        sqlx::query_as!(
            ProviderRow,
            r#"
            SELECT
                provider as "provider!",
                COUNT(*) as "request_count",
                COALESCE(SUM(input_tokens), 0) + COALESCE(SUM(output_tokens), 0) as "total_tokens",
                COALESCE(SUM(cost_cents), 0) as "total_cost_cents",
                COALESCE(AVG(latency_ms), 0)::bigint as "avg_latency_ms"
            FROM ai_requests
            WHERE created_at >= $1
            GROUP BY provider
            ORDER BY request_count DESC
            "#,
            since_ts
        )
        .fetch_all(pool.as_ref())
        .await?
    } else {
        sqlx::query_as!(
            ProviderRow,
            r#"
            SELECT
                provider as "provider!",
                COUNT(*) as "request_count",
                COALESCE(SUM(input_tokens), 0) + COALESCE(SUM(output_tokens), 0) as "total_tokens",
                COALESCE(SUM(cost_cents), 0) as "total_cost_cents",
                COALESCE(AVG(latency_ms), 0)::bigint as "avg_latency_ms"
            FROM ai_requests
            GROUP BY provider
            ORDER BY request_count DESC
            "#
        )
        .fetch_all(pool.as_ref())
        .await?
    };

    // Get by model
    let by_model = if let Some(since_ts) = since_timestamp {
        sqlx::query_as!(
            ModelRow,
            r#"
            SELECT
                model as "model!",
                provider as "provider!",
                COUNT(*) as "request_count",
                COALESCE(SUM(input_tokens), 0) + COALESCE(SUM(output_tokens), 0) as "total_tokens",
                COALESCE(SUM(cost_cents), 0) as "total_cost_cents",
                COALESCE(AVG(latency_ms), 0)::bigint as "avg_latency_ms"
            FROM ai_requests
            WHERE created_at >= $1
            GROUP BY model, provider
            ORDER BY request_count DESC
            LIMIT 10
            "#,
            since_ts
        )
        .fetch_all(pool.as_ref())
        .await?
    } else {
        sqlx::query_as!(
            ModelRow,
            r#"
            SELECT
                model as "model!",
                provider as "provider!",
                COUNT(*) as "request_count",
                COALESCE(SUM(input_tokens), 0) + COALESCE(SUM(output_tokens), 0) as "total_tokens",
                COALESCE(SUM(cost_cents), 0) as "total_cost_cents",
                COALESCE(AVG(latency_ms), 0)::bigint as "avg_latency_ms"
            FROM ai_requests
            GROUP BY model, provider
            ORDER BY request_count DESC
            LIMIT 10
            "#
        )
        .fetch_all(pool.as_ref())
        .await?
    };

    let input_tokens = totals.total_input_tokens.unwrap_or(0);
    let output_tokens = totals.total_output_tokens.unwrap_or(0);
    let total_cost_cents = totals.total_cost_cents.unwrap_or(0);

    let output = RequestStatsOutput {
        total_requests: totals.request_count.unwrap_or(0),
        total_tokens: TokenStats {
            input: input_tokens,
            output: output_tokens,
            total: input_tokens + output_tokens,
        },
        total_cost_dollars: f64::from(total_cost_cents as i32) / 1_000_000.0,
        average_latency_ms: totals.avg_latency_ms.unwrap_or(0),
        by_provider: by_provider
            .into_iter()
            .map(|r| ProviderStats {
                provider: r.provider,
                request_count: r.request_count.unwrap_or(0),
                total_tokens: r.total_tokens.unwrap_or(0),
                total_cost_dollars: f64::from(r.total_cost_cents.unwrap_or(0) as i32) / 1_000_000.0,
                avg_latency_ms: r.avg_latency_ms.unwrap_or(0),
            })
            .collect(),
        by_model: by_model
            .into_iter()
            .map(|r| ModelStats {
                model: r.model,
                provider: r.provider,
                request_count: r.request_count.unwrap_or(0),
                total_tokens: r.total_tokens.unwrap_or(0),
                total_cost_dollars: f64::from(r.total_cost_cents.unwrap_or(0) as i32) / 1_000_000.0,
                avg_latency_ms: r.avg_latency_ms.unwrap_or(0),
            })
            .collect(),
    };

    if config.is_json_output() {
        let hints = RenderingHints::default();
        let result = CommandResult::card(output)
            .with_title("AI Request Statistics")
            .with_hints(hints);
        render_result(&result);
    } else {
        render_text_output(&output);
    }

    Ok(())
}

fn render_text_output(output: &RequestStatsOutput) {
    use systemprompt_core_logging::CliService;

    CliService::section("AI Request Statistics");

    CliService::key_value("Total Requests", &output.total_requests.to_string());
    CliService::key_value("Total Cost", &format!("${:.6}", output.total_cost_dollars));
    CliService::key_value(
        "Average Latency",
        &format!("{}ms", output.average_latency_ms),
    );

    CliService::subsection("Token Usage");
    CliService::key_value("  Input Tokens", &output.total_tokens.input.to_string());
    CliService::key_value("  Output Tokens", &output.total_tokens.output.to_string());
    CliService::key_value("  Total Tokens", &output.total_tokens.total.to_string());

    if !output.by_provider.is_empty() {
        CliService::subsection("By Provider");
        for provider in &output.by_provider {
            CliService::info(&format!(
                "  {} - {} requests, {} tokens, ${:.6}, avg {}ms",
                provider.provider,
                provider.request_count,
                provider.total_tokens,
                provider.total_cost_dollars,
                provider.avg_latency_ms
            ));
        }
    }

    if !output.by_model.is_empty() {
        CliService::subsection("Top Models");
        for model in &output.by_model {
            CliService::info(&format!(
                "  {} ({}) - {} requests, ${:.6}",
                model.model, model.provider, model.request_count, model.total_cost_dollars
            ));
        }
    }
}
