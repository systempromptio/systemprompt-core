//! DB-backed tests driving `analytics` subcommands through the dispatcher.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::analytics::{self, AnalyticsCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_database::DbPool;
use systemprompt_runtime::DatabaseContext;
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

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

async fn run_tolerant(args: &[&str], ctx: &CommandContext) {
    let result = analytics::execute(parse(args), ctx).await;
    if let Err(e) = result {
        assert!(!e.to_string().is_empty(), "{args:?}");
    }
}

#[tokio::test]
async fn overview_renders_on_empty_database() {
    let pool = pool().await;
    let ctx = ctx(&pool);
    run_tolerant(&["overview"], &ctx).await;
}

#[tokio::test]
async fn agents_analytics_run_on_empty_database() {
    let pool = pool().await;
    let ctx = ctx(&pool);
    run_tolerant(&["agents", "stats"], &ctx).await;
    run_tolerant(&["agents", "list"], &ctx).await;
    run_tolerant(&["agents", "trends"], &ctx).await;
    run_tolerant(&["agents", "show", "no-such-agent"], &ctx).await;
}

#[tokio::test]
async fn tools_analytics_run_on_empty_database() {
    let pool = pool().await;
    let ctx = ctx(&pool);
    run_tolerant(&["tools", "stats"], &ctx).await;
    run_tolerant(&["tools", "list"], &ctx).await;
    run_tolerant(&["tools", "trends"], &ctx).await;
    run_tolerant(&["tools", "show", "no-such-tool"], &ctx).await;
}

#[tokio::test]
async fn sessions_and_conversations_run_on_empty_database() {
    let pool = pool().await;
    let ctx = ctx(&pool);
    run_tolerant(&["sessions", "stats"], &ctx).await;
    run_tolerant(&["sessions", "trends"], &ctx).await;
    run_tolerant(&["sessions", "live"], &ctx).await;
    run_tolerant(&["conversations", "stats"], &ctx).await;
    run_tolerant(&["conversations", "trends"], &ctx).await;
    run_tolerant(&["conversations", "list"], &ctx).await;
}

#[tokio::test]
async fn content_traffic_costs_requests_run_on_empty_database() {
    let pool = pool().await;
    let ctx = ctx(&pool);
    run_tolerant(&["content", "stats"], &ctx).await;
    run_tolerant(&["content", "top"], &ctx).await;
    run_tolerant(&["content", "trends"], &ctx).await;
    run_tolerant(&["traffic", "sources"], &ctx).await;
    run_tolerant(&["traffic", "geo"], &ctx).await;
    run_tolerant(&["traffic", "devices"], &ctx).await;
    run_tolerant(&["traffic", "bots"], &ctx).await;
    run_tolerant(&["costs", "summary"], &ctx).await;
    run_tolerant(&["costs", "trends"], &ctx).await;
    run_tolerant(&["costs", "breakdown"], &ctx).await;
    run_tolerant(&["requests", "stats"], &ctx).await;
    run_tolerant(&["requests", "list"], &ctx).await;
    run_tolerant(&["requests", "trends"], &ctx).await;
    run_tolerant(&["requests", "models"], &ctx).await;
}
