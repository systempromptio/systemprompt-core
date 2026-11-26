//! CSV export of the rolled-up overview row; a section whose member is not
//! admitted exports empty cells.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;

use super::OverviewOutput;
use crate::cli::shared::CsvBuilder;

fn cell<T>(section: Option<&T>, value: impl Fn(&T) -> String) -> String {
    section.map_or_else(String::new, value)
}

fn pct(value: Option<f64>) -> String {
    value.map_or(String::new(), |v| format!("{v:.2}"))
}

pub fn export_overview_csv(output: &OverviewOutput, path: &std::path::Path) -> Result<()> {
    let mut csv = CsvBuilder::new().headers(vec![
        "period",
        "conversations_total",
        "conversations_change_pct",
        "agents_active",
        "agents_tasks",
        "agents_success_rate",
        "requests_total",
        "requests_tokens",
        "requests_avg_latency_ms",
        "tools_executions",
        "tools_success_rate",
        "sessions_currently_active",
        "sessions_created_in_period",
        "costs_microdollars",
        "costs_change_pct",
    ]);

    let conversations = output.conversations.as_ref();
    let agents = output.agents.as_ref();
    let requests = output.requests.as_ref();
    let tools = output.tools.as_ref();
    let costs = output.costs.as_ref();
    csv.add_row(vec![
        output.period.clone(),
        cell(conversations, |c| c.total.to_string()),
        cell(conversations, |c| pct(c.change_percent)),
        cell(agents, |a| a.active_count.to_string()),
        cell(agents, |a| a.total_tasks.to_string()),
        cell(agents, |a| format!("{:.2}", a.success_rate)),
        cell(requests, |r| r.total.to_string()),
        cell(requests, |r| r.total_tokens.to_string()),
        cell(requests, |r| r.avg_latency_ms.to_string()),
        cell(tools, |t| t.total_executions.to_string()),
        cell(tools, |t| format!("{:.2}", t.success_rate)),
        output.sessions.active.to_string(),
        output.sessions.total_today.to_string(),
        cell(costs, |c| c.total_cost_microdollars.to_string()),
        cell(costs, |c| pct(c.change_percent)),
    ]);

    csv.write_to_file(path)
}
