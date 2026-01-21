use anyhow::Result;
use clap::Args;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use systemprompt_analytics::RequestAnalyticsRepository;
use systemprompt_logging::CliService;
use systemprompt_runtime::{AppContext, DatabaseContext};

use crate::commands::analytics::shared::{
    export_to_csv, format_cost, format_duration_ms, format_tokens, parse_time_range,
};
use crate::shared::{render_result, CommandResult, RenderingHints};
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct ListArgs {
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
        help = "Maximum number of requests"
    )]
    pub limit: i64,

    #[arg(long, help = "Filter by model name")]
    pub model: Option<String>,

    #[arg(long, help = "Export results to CSV file")]
    pub export: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RequestListRowOutput {
    pub id: String,
    pub provider: String,
    pub model: String,
    pub input_tokens: i32,
    pub output_tokens: i32,
    pub cost_cents: i32,
    pub latency_ms: i32,
    pub cache_hit: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RequestListOutput {
    pub total: i64,
    pub requests: Vec<RequestListRowOutput>,
}

pub async fn execute(args: ListArgs, config: &CliConfig) -> Result<()> {
    let ctx = AppContext::new().await?;
    let repo = RequestAnalyticsRepository::new(ctx.db_pool())?;
    execute_internal(args, &repo, config).await
}

pub async fn execute_with_pool(
    args: ListArgs,
    db_ctx: &DatabaseContext,
    config: &CliConfig,
) -> Result<()> {
    let repo = RequestAnalyticsRepository::new(db_ctx.db_pool())?;
    execute_internal(args, &repo, config).await
}

async fn execute_internal(
    args: ListArgs,
    repo: &RequestAnalyticsRepository,
    config: &CliConfig,
) -> Result<()> {
    let (start, end) = parse_time_range(args.since.as_ref(), args.until.as_ref())?;
    let rows = repo
        .list_requests(start, end, args.limit, args.model.as_deref())
        .await?;

    let requests: Vec<RequestListRowOutput> = rows
        .into_iter()
        .map(|row| RequestListRowOutput {
            id: row.id.to_string(),
            provider: row.provider,
            model: row.model,
            input_tokens: row.input_tokens.unwrap_or(0),
            output_tokens: row.output_tokens.unwrap_or(0),
            cost_cents: row.cost_cents.unwrap_or(0),
            latency_ms: row.latency_ms.unwrap_or(0),
            cache_hit: row.cache_hit.unwrap_or(false),
            created_at: row.created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
        })
        .collect();

    let output = RequestListOutput {
        total: requests.len() as i64,
        requests,
    };

    if let Some(ref path) = args.export {
        export_to_csv(&output.requests, path)?;
        CliService::success(&format!("Exported to {}", path.display()));
        return Ok(());
    }

    if output.requests.is_empty() {
        CliService::warning("No requests found in the specified time range");
        return Ok(());
    }

    if config.is_json_output() {
        let hints = RenderingHints {
            columns: Some(vec![
                "id".to_string(),
                "provider".to_string(),
                "model".to_string(),
                "input_tokens".to_string(),
                "output_tokens".to_string(),
                "cost_cents".to_string(),
                "latency_ms".to_string(),
            ]),
            ..Default::default()
        };
        let result = CommandResult::table(output)
            .with_title("AI Requests")
            .with_hints(hints);
        render_result(&result);
    } else {
        render_list(&output);
    }

    Ok(())
}

fn render_list(output: &RequestListOutput) {
    CliService::section("AI Requests");

    for req in &output.requests {
        let cache_indicator = if req.cache_hit { " [cached]" } else { "" };
        let short_id = if req.id.len() > 8 {
            &req.id[..8]
        } else {
            &req.id
        };
        CliService::subsection(&format!(
            "{} {}/{}{}",
            short_id, req.provider, req.model, cache_indicator
        ));
        CliService::key_value(
            "Tokens",
            &format!(
                "{} in / {} out",
                format_tokens(i64::from(req.input_tokens)),
                format_tokens(i64::from(req.output_tokens))
            ),
        );
        CliService::key_value("Cost", &format_cost(i64::from(req.cost_cents)));
        CliService::key_value("Latency", &format_duration_ms(i64::from(req.latency_ms)));
        CliService::key_value("Time", &req.created_at);
    }

    CliService::info(&format!("Showing {} requests", output.total));
}
