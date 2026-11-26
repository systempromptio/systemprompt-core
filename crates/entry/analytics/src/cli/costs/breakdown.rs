//! `analytics costs breakdown` command with per-dimension shares.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use clap::{Args, ValueEnum};
use std::path::PathBuf;
use systemprompt_logging::CliService;
use systemprompt_traits::usage_reports::ai::{
    AiUsageReport, CostBreakdownRow, CostUserBreakdownRow,
};

use super::{CostBreakdownItem, CostBreakdownOutput};
use crate::cli::shared::{export_to_csv, parse_time_range};
use systemprompt_extension::cli::CommandOutput;

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum BreakdownType {
    Model,
    Agent,
    Provider,
    User,
    #[value(alias = "billing_source")]
    BillingSource,
}

#[derive(Debug, Args)]
pub struct BreakdownArgs {
    #[arg(
        long,
        alias = "from",
        default_value = "24h",
        help = "Time range (e.g., '1h', '24h', '7d')"
    )]
    pub since: Option<String>,

    #[arg(long, alias = "to", help = "End time for range")]
    pub until: Option<String>,

    #[arg(
        long,
        value_enum,
        default_value = "model",
        help = "Breakdown by (model, agent, provider, user, billing-source)"
    )]
    pub by: BreakdownType,

    #[arg(long, short = 'n', default_value = "20", help = "Maximum items")]
    pub limit: i64,

    #[arg(long, help = "Export results to CSV file")]
    pub export: Option<PathBuf>,
}

pub(super) async fn execute(
    args: BreakdownArgs,
    repo: &dyn AiUsageReport,
) -> Result<CommandOutput> {
    let (start, end) = parse_time_range(args.since.as_ref(), args.until.as_ref())?;

    let rows: Vec<Share> = match args.by {
        BreakdownType::Model => repo
            .breakdown_by_model(start, end, args.limit)
            .await?
            .into_iter()
            .map(Share::from)
            .collect(),
        BreakdownType::Provider => repo
            .breakdown_by_provider(start, end, args.limit)
            .await?
            .into_iter()
            .map(Share::from)
            .collect(),
        BreakdownType::Agent => repo
            .breakdown_by_agent(start, end, args.limit)
            .await?
            .into_iter()
            .map(Share::from)
            .collect(),
        BreakdownType::BillingSource => repo
            .breakdown_by_billing_source(start, end, args.limit)
            .await?
            .into_iter()
            .map(Share::from)
            .collect(),
        BreakdownType::User => repo
            .breakdown_by_user(start, end, args.limit)
            .await?
            .into_iter()
            .map(Share::from)
            .collect(),
    };

    let total_cost: i64 = rows.iter().map(|r| r.cost).sum();

    let output = CostBreakdownOutput {
        period: format!(
            "{} to {}",
            start.format("%Y-%m-%d %H:%M"),
            end.format("%Y-%m-%d %H:%M")
        ),
        breakdown_by: breakdown_label(args.by).to_owned(),
        items: build_items(rows, total_cost),
        total_cost_microdollars: total_cost,
    };

    if let Some(ref path) = args.export {
        export_to_csv(&output.items, path)?;
        CliService::success(&format!("Exported to {}", path.display()));
        return Ok(breakdown_table(&output.items).with_skip_render());
    }

    if output.items.is_empty() {
        CliService::warning("No data found in the specified time range");
        return Ok(breakdown_table(&output.items).with_skip_render());
    }

    Ok(breakdown_table(&output.items).with_title("Cost Breakdown"))
}

const fn breakdown_label(by: BreakdownType) -> &'static str {
    match by {
        BreakdownType::Model => "model",
        BreakdownType::Agent => "agent",
        BreakdownType::Provider => "provider",
        BreakdownType::User => "user",
        BreakdownType::BillingSource => "billing_source",
    }
}

struct Share {
    name: String,
    cost: i64,
    requests: i64,
    tokens: i64,
    conversations: Option<i64>,
}

impl From<CostBreakdownRow> for Share {
    fn from(row: CostBreakdownRow) -> Self {
        Self {
            name: row.name,
            cost: row.cost,
            requests: row.requests,
            tokens: row.tokens,
            conversations: None,
        }
    }
}

impl From<CostUserBreakdownRow> for Share {
    fn from(row: CostUserBreakdownRow) -> Self {
        let name = match row.name {
            Some(display) if !display.is_empty() => format!("{} ({display})", row.user_id),
            _ => row.user_id.to_string(),
        };
        Self {
            name,
            cost: row.cost,
            requests: row.requests,
            tokens: row.tokens,
            conversations: Some(row.conversations),
        }
    }
}

fn build_items(rows: Vec<Share>, total_cost: i64) -> Vec<CostBreakdownItem> {
    rows.into_iter()
        .map(|row| {
            let percentage = if total_cost > 0 {
                (row.cost as f64 / total_cost as f64) * 100.0
            } else {
                0.0
            };

            CostBreakdownItem {
                name: row.name,
                cost_microdollars: row.cost,
                request_count: row.requests,
                tokens: row.tokens,
                conversations: row.conversations,
                percentage,
            }
        })
        .collect()
}

fn breakdown_table(items: &[CostBreakdownItem]) -> CommandOutput {
    let mut columns = vec!["name", "cost_microdollars", "request_count", "tokens"];
    if items.iter().any(|item| item.conversations.is_some()) {
        columns.push("conversations");
    }
    columns.push("percentage");
    CommandOutput::table_of(columns, items)
}
