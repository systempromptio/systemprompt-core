//! A composition admitting none of the members that own the report views:
//! the report commands answer `ReportUnavailable` naming the absent
//! capability, and the overview renders with every member section withheld.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::sync::Arc;

use clap::Parser;
use serde_json::Value;
use systemprompt_analytics_entry::cli::overview::{self, OverviewArgs, OverviewPorts};
use systemprompt_analytics_entry::cli::{self as analytics, AnalyticsCommands};
use systemprompt_cli::{CommandContext, EnvOverrides};
use systemprompt_database::DbPool;
use systemprompt_extension::cli::{CliConfig, OutputFormat};
use systemprompt_extension::prelude::Dependencies;
use systemprompt_runtime::{DatabaseContext, Kernel};
use systemprompt_test_fixtures::{test_database_url, test_db_pool};
use systemprompt_traits::BackgroundTasks;
use systemprompt_traits::usage_reports::ReportUnavailable;

#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    cmd: AnalyticsCommands,
}

fn parse(args: &[&str]) -> AnalyticsCommands {
    Harness::try_parse_from(std::iter::once("analytics").chain(args.iter().copied()))
        .unwrap()
        .cmd
}

fn portless_ctx(pool: &DbPool) -> CommandContext {
    CommandContext::with_database(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
        DatabaseContext::from_pool(pool.clone()),
        test_database_url(),
    )
    .with_kernel(Arc::new(Kernel::detached(
        BackgroundTasks::new(),
        Dependencies::new(),
    )))
}

async fn unavailable_capability(args: &[&str]) -> &'static str {
    let pool = test_db_pool().await;
    let ctx = portless_ctx(&pool);
    let err = analytics::execute(parse(args), &ctx)
        .await
        .expect_err("a report whose owning member is absent must fail");
    err.downcast_ref::<ReportUnavailable>()
        .unwrap_or_else(|| panic!("expected ReportUnavailable, got {err:#}"))
        .capability
}

#[tokio::test]
async fn agents_list_without_the_agent_member_is_unavailable() {
    assert_eq!(unavailable_capability(&["agents", "list"]).await, "agents");
}

#[tokio::test]
async fn tools_list_without_the_mcp_member_is_unavailable() {
    assert_eq!(unavailable_capability(&["tools", "list"]).await, "mcp");
}

#[tokio::test]
async fn costs_without_the_gateway_member_is_unavailable() {
    assert_eq!(unavailable_capability(&["costs"]).await, "gateway");
}

fn find_overview(value: &Value) -> Option<Value> {
    let sections = value.get("sections")?.as_array()?;
    let map: serde_json::Map<String, Value> = sections
        .iter()
        .filter_map(|section| {
            Some((
                section.get("heading")?.as_str()?.to_owned(),
                section.get("content")?.clone(),
            ))
        })
        .collect();
    (map.contains_key("period") && map.contains_key("sessions")).then_some(Value::Object(map))
}

#[tokio::test]
async fn overview_without_member_ports_withholds_every_member_section() {
    let pool = test_db_pool().await;
    let args = OverviewArgs {
        since: Some("24h".to_owned()),
        until: None,
        export: None,
    };
    let output = overview::execute_with_pool(
        args,
        &pool,
        &OverviewPorts::default(),
        &CliConfig::new().with_interactive(false),
    )
    .await
    .expect("the overview renders without member ports");

    let json = serde_json::to_value(output.into_artifact()).unwrap();
    let snapshot = find_overview(&json).unwrap_or_else(|| panic!("no overview in {json}"));
    for section in ["conversations", "agents", "requests", "tools", "costs"] {
        assert!(snapshot[section].is_null(), "{section} present: {snapshot}");
    }
    let unavailable = snapshot["unavailable"]
        .as_array()
        .unwrap_or_else(|| panic!("no unavailable list: {snapshot}"));
    assert!(!unavailable.is_empty(), "{snapshot}");
}
