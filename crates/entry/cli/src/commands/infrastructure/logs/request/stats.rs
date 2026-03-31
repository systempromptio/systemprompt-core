use anyhow::Result;
use clap::Args;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use systemprompt_logging::TraceQueryService;

use crate::CliConfig;
use crate::commands::infrastructure::logs::duration::parse_since;
use crate::shared::{CommandResult, RenderingHints, render_result};

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

crate::define_pool_command!(StatsArgs => (), with_config);

async fn execute_with_pool_inner(
    args: StatsArgs,
    pool: &Arc<sqlx::PgPool>,
    config: &CliConfig,
) -> Result<()> {
    let since_timestamp = parse_since(args.since.as_ref())?;

    let service = TraceQueryService::new(Arc::clone(pool));
    let stats = service.get_ai_request_stats(since_timestamp).await?;

    let input_tokens = stats.total_input_tokens;
    let output_tokens = stats.total_output_tokens;

    let output = RequestStatsOutput {
        total_requests: stats.total_requests,
        total_tokens: TokenStats {
            input: input_tokens,
            output: output_tokens,
            total: input_tokens + output_tokens,
        },
        total_cost_dollars: f64::from(stats.total_cost_microdollars as i32) / 1_000_000.0,
        average_latency_ms: stats.avg_latency_ms,
        by_provider: stats
            .by_provider
            .into_iter()
            .map(|r| ProviderStats {
                provider: r.provider,
                request_count: r.request_count,
                total_tokens: r.total_tokens,
                total_cost_dollars: f64::from(r.total_cost_microdollars as i32) / 1_000_000.0,
                avg_latency_ms: r.avg_latency_ms,
            })
            .collect(),
        by_model: stats
            .by_model
            .into_iter()
            .map(|r| ModelStats {
                model: r.model,
                provider: r.provider,
                request_count: r.request_count,
                total_tokens: r.total_tokens,
                total_cost_dollars: f64::from(r.total_cost_microdollars as i32) / 1_000_000.0,
                avg_latency_ms: r.avg_latency_ms,
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
    use systemprompt_logging::CliService;

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
