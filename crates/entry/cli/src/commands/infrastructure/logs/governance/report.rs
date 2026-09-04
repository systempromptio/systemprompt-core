//! `infra logs governance report`: what warn mode caught, and what it would
//! have blocked.
//!
//! Two planes are reported side by side because they are configured
//! separately and fail differently: the governance chain
//! (`services/governance/config.yaml`) writes `governance_decisions`, and the
//! gateway safety scanners (`services/gateway/policies.yaml`) write
//! `ai_safety_findings`. A category that fires constantly with a zero blocked
//! count is a tunable that wants lowering; one that never fires at all is a
//! scanner that is not earning its place.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::io::Write;
use std::sync::Arc;

use anyhow::Result;
use clap::{Args, ValueEnum};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_ai::AiSafetyFindingRepository;
use systemprompt_security::authz::list_governance_warnings;

use crate::CliConfig;
use crate::commands::infrastructure::logs::duration::parse_since;
use crate::shared::{CommandOutput, render_result};

/// Which dimension of the warn rollup to collapse onto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum GroupBy {
    Policy,
    Tool,
    User,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ReportFormat {
    Table,
    Csv,
}

#[derive(Debug, Args)]
pub struct ReportArgs {
    #[arg(
        long,
        default_value = "24h",
        help = "Window to report over (e.g. '1h', '24h', '7d') or a datetime"
    )]
    pub since: String,

    #[arg(
        long,
        value_enum,
        default_value = "policy",
        help = "Dimension to group governance warnings by"
    )]
    pub group_by: GroupBy,

    #[arg(
        long,
        value_enum,
        default_value = "table",
        help = "Render as a table, or emit CSV on stdout for a spreadsheet"
    )]
    pub format: ReportFormat,

    #[arg(long, default_value = "50", help = "Maximum rows per section")]
    pub limit: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WarningGroupRow {
    pub group: String,
    pub warnings: i64,
    pub tools: i64,
    pub users: i64,
    pub last_seen: String,
    pub example_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SafetyFindingGroupRow {
    pub category: String,
    pub scanner: String,
    pub severity: String,
    pub phase: String,
    pub findings: i64,
    pub blocked: i64,
    pub last_seen: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GovernanceReportOutput {
    pub since: String,
    pub group_by: GroupBy,
    pub total_warnings: i64,
    pub warnings: Vec<WarningGroupRow>,
    pub total_findings: i64,
    pub total_blocked_findings: i64,
    pub safety_findings: Vec<SafetyFindingGroupRow>,
}

crate::define_pool_command!(ReportArgs => (), with_config);

// Why: accumulates in a `BTreeMap` rather than sorting at the end so the
// distinct tool and user counts per group are exact. Summing the SQL rollup's
// per-combination counts would double-count a tool used by two users.
#[derive(Default)]
struct GroupAccumulator {
    warnings: i64,
    tools: std::collections::BTreeSet<String>,
    users: std::collections::BTreeSet<String>,
    last_seen: Option<chrono::DateTime<chrono::Utc>>,
    example_reason: String,
}

async fn execute_with_pool_inner(
    args: ReportArgs,
    pool: &Arc<sqlx::PgPool>,
    config: &CliConfig,
) -> Result<()> {
    let since = parse_since(Some(&args.since))?;

    let rows = list_governance_warnings(pool.as_ref(), since, args.limit.max(1) * 20).await?;
    let total_warnings = rows.iter().map(|r| r.count).sum();

    let mut groups: BTreeMap<String, GroupAccumulator> = BTreeMap::new();
    for row in rows {
        let key = match args.group_by {
            GroupBy::Policy => row.policy.clone(),
            GroupBy::Tool => row.tool_name.clone(),
            GroupBy::User => row.user_id.clone(),
        };
        let acc = groups.entry(key).or_default();
        acc.warnings += row.count;
        acc.tools.insert(row.tool_name);
        acc.users.insert(row.user_id);
        if acc.last_seen.is_none_or(|seen| seen < row.last_seen) {
            acc.last_seen = Some(row.last_seen);
            acc.example_reason = row.example_reason;
        }
    }

    let mut warnings: Vec<WarningGroupRow> = groups
        .into_iter()
        .map(|(group, acc)| WarningGroupRow {
            group,
            warnings: acc.warnings,
            tools: acc.tools.len() as i64,
            users: acc.users.len() as i64,
            last_seen: acc
                .last_seen
                .map_or_else(String::new, |t| t.format("%Y-%m-%d %H:%M:%S").to_string()),
            example_reason: truncate(&acc.example_reason, 120),
        })
        .collect();
    warnings.sort_by(|a, b| b.warnings.cmp(&a.warnings).then_with(|| a.group.cmp(&b.group)));
    warnings.truncate(usize::try_from(args.limit.max(0)).unwrap_or(usize::MAX));

    let findings_repo = AiSafetyFindingRepository::from_pool(Arc::clone(pool));
    let finding_rows = findings_repo.list_rollup(since, args.limit.max(1)).await?;
    let total_findings = finding_rows.iter().map(|r| r.count).sum();
    let total_blocked_findings = finding_rows.iter().map(|r| r.blocked_count).sum();
    let safety_findings: Vec<SafetyFindingGroupRow> = finding_rows
        .into_iter()
        .map(|r| SafetyFindingGroupRow {
            category: r.category,
            scanner: r.scanner,
            severity: r.severity,
            phase: r.phase,
            findings: r.count,
            blocked: r.blocked_count,
            last_seen: r.last_seen.format("%Y-%m-%d %H:%M:%S").to_string(),
        })
        .collect();

    let output = GovernanceReportOutput {
        since: args.since.clone(),
        group_by: args.group_by,
        total_warnings,
        warnings,
        total_findings,
        total_blocked_findings,
        safety_findings,
    };

    if args.format == ReportFormat::Csv {
        let csv = format_csv(&output);
        std::io::stdout().write_all(csv.as_bytes())?;
        return Ok(());
    }

    render_result(
        &CommandOutput::table_of(
            vec![
                "group",
                "warnings",
                "tools",
                "users",
                "last_seen",
                "example_reason",
            ],
            &output.warnings,
        )
        .with_title(format!(
            "Governance warnings by {} — {} in the last {}",
            group_label(args.group_by),
            output.total_warnings,
            args.since
        )),
        config,
    );

    render_result(
        &CommandOutput::table_of(
            vec![
                "category",
                "scanner",
                "severity",
                "phase",
                "findings",
                "blocked",
                "last_seen",
            ],
            &output.safety_findings,
        )
        .with_title(format!(
            "Gateway safety findings — {} recorded, {} blocked",
            output.total_findings, output.total_blocked_findings
        )),
        config,
    );

    Ok(())
}

const fn group_label(group_by: GroupBy) -> &'static str {
    match group_by {
        GroupBy::Policy => "policy",
        GroupBy::Tool => "tool",
        GroupBy::User => "user",
    }
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let head: String = text.chars().take(max.saturating_sub(1)).collect();
    format!("{head}…")
}

// Why: two sections in one CSV, separated by a blank line and a fresh header
// row. A spreadsheet reads that as two blocks, and the alternative — two files
// or two invocations — loses the fact that both windows are the same.
fn format_csv(output: &GovernanceReportOutput) -> String {
    let mut csv = String::from("section,group,warnings,tools,users,last_seen,example_reason\n");
    for row in &output.warnings {
        csv.push_str(&format!(
            "warning,{},{},{},{},{},\"{}\"\n",
            csv_field(&row.group),
            row.warnings,
            row.tools,
            row.users,
            row.last_seen,
            row.example_reason.replace('"', "\"\"")
        ));
    }
    csv.push_str("\nsection,category,scanner,severity,phase,findings,blocked,last_seen\n");
    for row in &output.safety_findings {
        csv.push_str(&format!(
            "finding,{},{},{},{},{},{},{}\n",
            csv_field(&row.category),
            csv_field(&row.scanner),
            csv_field(&row.severity),
            csv_field(&row.phase),
            row.findings,
            row.blocked,
            row.last_seen
        ));
    }
    csv
}

fn csv_field(value: &str) -> String {
    if value.contains([',', '"', '\n']) {
        return format!("\"{}\"", value.replace('"', "\"\""));
    }
    value.to_owned()
}
