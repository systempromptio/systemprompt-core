//! Tests for `analytics::overview` — change-percentage math, period
//! formatting, and CSV export of the rolled-up snapshot.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use chrono::{TimeZone, Utc};
use systemprompt_cli::analytics::overview::{
    AgentMetrics, ConversationMetrics, CostMetrics, OverviewOutput, RequestMetrics, SessionMetrics,
    ToolMetrics, calculate_change, export_overview_csv, format_period,
};
use tempfile::TempDir;

#[test]
fn calculate_change_reports_percentage_delta() {
    assert_eq!(calculate_change(150, 100), Some(50.0));
    assert_eq!(calculate_change(50, 100), Some(-50.0));
    assert_eq!(calculate_change(100, 100), Some(0.0));
}

#[test]
fn calculate_change_is_none_when_previous_period_empty() {
    assert_eq!(calculate_change(42, 0), None);
}

#[test]
fn format_period_renders_minute_precision_range() {
    let start = Utc.with_ymd_and_hms(2026, 7, 1, 8, 30, 59).unwrap();
    let end = Utc.with_ymd_and_hms(2026, 7, 2, 9, 0, 0).unwrap();
    assert_eq!(
        format_period(start, end),
        "2026-07-01 08:30 to 2026-07-02 09:00"
    );
}

fn sample_output() -> OverviewOutput {
    OverviewOutput {
        period: "2026-07-01 00:00 to 2026-07-02 00:00".to_owned(),
        conversations: ConversationMetrics {
            total: 12,
            change_percent: Some(25.5),
        },
        agents: AgentMetrics {
            active_count: 3,
            total_tasks: 40,
            success_rate: 97.5,
        },
        requests: RequestMetrics {
            total: 200,
            total_tokens: 12345,
            avg_latency_ms: 850,
        },
        tools: ToolMetrics {
            total_executions: 55,
            success_rate: 90.0,
        },
        sessions: SessionMetrics {
            active: 4,
            total_today: 9,
        },
        costs: CostMetrics {
            total_cost_microdollars: 1_500_000,
            change_percent: None,
        },
    }
}

#[test]
fn export_overview_csv_writes_headers_and_row() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("overview.csv");
    export_overview_csv(&sample_output(), &path).unwrap();

    let csv = std::fs::read_to_string(&path).unwrap();
    let mut lines = csv.lines();
    let header = lines.next().unwrap();
    assert!(header.starts_with("period,conversations_total"));
    assert!(header.ends_with("costs_microdollars,costs_change_pct"));

    let row = lines.next().unwrap();
    assert!(row.contains("2026-07-01 00:00 to 2026-07-02 00:00"));
    assert!(row.contains("25.50"));
    assert!(row.contains("97.50"));
    assert!(row.contains("1500000"));
    assert!(
        row.ends_with(','),
        "empty cost change renders as empty cell"
    );
}
