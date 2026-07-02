//! Tests for `infra services` decision logic and dispatcher gating.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::infrastructure::services::restart::format_batch_message;
use systemprompt_cli::infrastructure::services::start::{
    ServiceFlags, ServiceTarget, ServiceTargetFlags,
};
use systemprompt_cli::infrastructure::services::{self, ServicesCommands, cleanup};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_database::{DbPool, ServiceConfig};
use systemprompt_runtime::DatabaseContext;
use systemprompt_scheduler::{OrphanCleanupReport, OrphanDisposition, OrphanOutcome};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    cmd: ServicesCommands,
}

fn parse(args: &[&str]) -> ServicesCommands {
    Harness::try_parse_from(std::iter::once("services").chain(args.iter().copied()))
        .unwrap()
        .cmd
}

async fn pool() -> DbPool {
    fixture_db_pool(&fixture_database_url().unwrap())
        .await
        .unwrap()
}

fn ctx(pool: &DbPool) -> CommandContext {
    CommandContext::with_database(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
        DatabaseContext::from_pool(pool.clone()),
        fixture_database_url().unwrap(),
    )
}

fn flags(all: bool, api: bool, agents: bool, mcp: bool) -> ServiceFlags {
    ServiceFlags {
        all,
        targets: ServiceTargetFlags { api, agents, mcp },
    }
}

#[test]
fn service_target_from_flags_selects_all_when_unset_or_all() {
    let t = ServiceTarget::from_flags(flags(true, false, false, false));
    assert!(t.api && t.agents && t.mcp);

    let t = ServiceTarget::from_flags(flags(false, false, false, false));
    assert!(t.api && t.agents && t.mcp);
}

#[test]
fn service_target_from_flags_honours_specific_targets() {
    let t = ServiceTarget::from_flags(flags(false, true, false, true));
    assert!(t.api);
    assert!(!t.agents);
    assert!(t.mcp);
}

#[test]
fn format_batch_message_covers_all_outcomes() {
    assert_eq!(
        format_batch_message("agents", 0, 0, true),
        "No enabled agents found"
    );
    assert_eq!(
        format_batch_message("agents", 2, 0, true),
        "Restarted 2 agents"
    );
    let mixed = format_batch_message("MCP servers", 1, 2, true);
    assert!(mixed.contains('1') && mixed.contains('2'));
    let failed_only = format_batch_message("agents", 0, 3, false);
    assert!(failed_only.contains('3'));
}

#[test]
fn cleanup_helpers_render_reports_and_messages() {
    let report = OrphanCleanupReport {
        outcomes: vec![
            OrphanOutcome {
                name: "svc-a".to_owned(),
                pid: 123,
                port: 5001,
                disposition: OrphanDisposition::StaleEntry,
            },
            OrphanOutcome {
                name: "svc-b".to_owned(),
                pid: 456,
                port: 5002,
                disposition: OrphanDisposition::Stopped,
            },
        ],
        api_stopped: true,
        stale_entries_removed: 1,
    };
    cleanup::render_cleanup_report(&report, false);
    cleanup::render_cleanup_report(&report, true);

    assert_eq!(
        cleanup::format_cleanup_message(3, true),
        "Cleaned up 3 services"
    );
    assert_eq!(
        cleanup::format_cleanup_message(0, false),
        "No running services found"
    );

    let out = cleanup::no_services_result(false, true);
    let json = serde_json::to_value(out.artifact()).unwrap();
    assert_eq!(json["title"], "Service Cleanup");
}

#[test]
fn cleanup_dry_run_result_counts_services() {
    let services = vec![ServiceConfig {
        name: "svc-a".to_owned(),
        module_name: "mod-a".to_owned(),
        status: "running".to_owned(),
        pid: Some(4_000_000),
        port: 5001,
        binary_mtime: None,
        created_at: String::new(),
        updated_at: String::new(),
    }];
    let out = cleanup::dry_run_result(&services, Some(999), 8080, false);
    let json = serde_json::to_value(out.artifact()).unwrap();
    assert_eq!(json["title"], "Service Cleanup (Dry Run)");

    cleanup::log_service_state(&services[0]);
    cleanup::log_service_state(&ServiceConfig {
        pid: None,
        ..services[0].clone()
    });
}

#[tokio::test]
async fn start_notices_for_agents_and_mcp_only() {
    let pool = pool().await;
    let ctx = ctx(&pool);
    services::execute(
        parse(&["start", "--agents", "--mcp", "--skip-migrate"]),
        &ctx,
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn lifecycle_commands_refuse_database_scope() {
    let pool = pool().await;
    let ctx = ctx(&pool);

    for args in [
        vec!["stop", "--all"],
        vec!["restart", "--failed"],
        vec!["cleanup", "--yes"],
    ] {
        let result = services::execute(parse(&args), &ctx).await;
        assert!(result.is_err(), "{args:?}");
    }
}
