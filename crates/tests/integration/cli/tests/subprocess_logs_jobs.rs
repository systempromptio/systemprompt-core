//! Subprocess coverage for the `infra logs` and `infra jobs` trees, plus
//! deep `analytics` flag variants that the smoke battery does not reach.
//!
//! Log queries run against whatever rows exist in the shared test database;
//! filters that match nothing still execute the full query/render path.
//! Tests accept success or failure exit codes.

use systemprompt_cli_integration_tests::full_bootstrap::{fixture, run, run_with_formats};

#[test]
fn logs_view_variants() {
    run_with_formats(&["infra", "logs", "view"]);
    run(&["infra", "logs", "view", "--limit", "5"]);
    run(&["infra", "logs", "view", "--level", "error"]);
    run(&["infra", "logs", "view", "--since", "1h"]);
}

#[test]
fn logs_search_variants() {
    run(&["infra", "logs", "search", "bootstrap"]);
    run(&["infra", "logs", "search", "zzz_no_match", "--limit", "3"]);
}

#[test]
fn logs_show_and_summary() {
    run_with_formats(&["infra", "logs", "summary"]);
    run(&["infra", "logs", "show", "no-such-log-id"]);
}

#[test]
fn logs_trace_trees() {
    run_with_formats(&["infra", "logs", "trace", "list"]);
    run(&["infra", "logs", "trace", "list", "--limit", "3", "--all"]);
    run(&["infra", "logs", "trace", "list", "--status", "failed"]);
    run(&["infra", "logs", "trace", "list", "--has-mcp"]);
    run(&["infra", "logs", "trace", "list", "--agent", "covagent"]);
    run(&["infra", "logs", "trace", "show", "no-such-trace"]);
    run(&["infra", "logs", "trace", "show", "no-such-trace", "--all"]);
}

#[test]
fn logs_request_and_tools() {
    run_with_formats(&["infra", "logs", "request", "list"]);
    run(&["infra", "logs", "request", "show", "no-such-request"]);
    run(&["infra", "logs", "request", "stats"]);
    run(&["infra", "logs", "tools", "list"]);
    run(&["infra", "logs", "tools", "list", "--limit", "3"]);
}

#[test]
fn logs_audit_missing_id() {
    run(&["infra", "logs", "audit", "no-such-id"]);
}

#[test]
fn logs_export_variants() {
    let Some(fixture) = fixture() else { return };
    let out = fixture.system_dir.join("logs_export.json");
    let out_str = out.to_string_lossy().into_owned();
    run(&["infra", "logs", "export", "--limit", "10"]);
    run(&["infra", "logs", "export", "--format", "csv", "--limit", "5"]);
    run(&[
        "infra", "logs", "export", "--format", "jsonl", "--output", &out_str,
    ]);
}

#[test]
fn logs_cleanup_dry_run() {
    run(&[
        "infra",
        "logs",
        "cleanup",
        "--older-than",
        "365d",
        "--dry-run",
        "--yes",
    ]);
}

#[test]
fn jobs_trees() {
    run_with_formats(&["infra", "jobs", "list"]);
    run(&["infra", "jobs", "show", "cleanup_inactive_sessions"]);
    run(&["infra", "jobs", "show", "no_such_job"]);
    run(&["infra", "jobs", "history"]);
    run(&["infra", "jobs", "history", "--limit", "3"]);
    run(&["infra", "jobs", "enable", "no_such_job"]);
    run(&["infra", "jobs", "disable", "no_such_job"]);
    run(&["infra", "jobs", "run", "no_such_job"]);
}

#[test]
fn analytics_flag_variants() {
    run(&["analytics", "overview", "--days", "7"]);
    run(&["analytics", "conversations", "list", "--limit", "3"]);
    run(&["analytics", "agents", "list", "--days", "7"]);
    run(&["analytics", "tools", "list", "--days", "7"]);
    run(&["analytics", "requests", "list", "--limit", "3"]);
    run(&["analytics", "sessions", "stats", "--days", "7"]);
    run(&["analytics", "content", "top", "--limit", "3"]);
    run(&["analytics", "traffic", "sources", "--days", "7"]);
    run(&["analytics", "costs", "breakdown", "--days", "7"]);
}

#[test]
fn db_query_and_describe_variants() {
    run(&["infra", "db", "query", "SELECT count(*) FROM users"]);
    run(&["infra", "db", "query", "SELECT 1", "--limit", "1"]);
    run(&["infra", "db", "describe", "logs"]);
    run(&["infra", "db", "count", "logs"]);
    run(&["infra", "db", "indexes", "--table", "users"]);
}
