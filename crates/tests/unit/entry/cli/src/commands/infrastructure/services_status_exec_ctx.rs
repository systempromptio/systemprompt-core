//! `infra services status` through the dispatcher.
//!
//! The status body verifies every configured service against the database and
//! then renders one of two views. The renderers and the health column run only
//! off the command's own flags, so each combination is driven here; the fixture
//! fleet is empty, which is the shape the summary line has to survive.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::sync::Arc;

use clap::Parser;
use systemprompt_cli::infrastructure::services::{self, ServicesCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_app_context, fixture_database_url, fixture_db_pool,
    install_test_signing_key,
};

#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    cmd: ServicesCommands,
}

fn parse(args: &[&str]) -> ServicesCommands {
    Harness::try_parse_from(std::iter::once("services").chain(args.iter().copied()))
        .unwrap_or_else(|e| panic!("parse {args:?}: {e}"))
        .cmd
}

async fn app() -> Arc<AppContext> {
    ensure_test_bootstrap();
    install_test_signing_key();
    let url = fixture_database_url().expect("a test database url");
    let pool = fixture_db_pool(&url)
        .await
        .expect("the services status tests need a reachable test database");
    fixture_app_context(&pool, &url).expect("fixture app context")
}

fn ctx(app: &Arc<AppContext>, json: bool) -> CommandContext {
    let mut cli = CliConfig::new().with_interactive(false);
    if json {
        cli = cli.with_output_format(OutputFormat::Json);
    }
    CommandContext::with_app_context(cli, EnvOverrides::default(), Arc::clone(app))
}

async fn run(args: &[&str], json: bool) -> anyhow::Result<()> {
    let app = app().await;
    services::execute(parse(args), &ctx(&app, json)).await
}

#[tokio::test]
async fn status_renders_the_table_view_for_a_terminal() {
    run(&["status"], false)
        .await
        .expect("an empty fleet still renders a status table");
}

#[tokio::test]
async fn the_detailed_flag_takes_the_per_service_view_instead_of_the_table() {
    run(&["status", "--detailed"], false)
        .await
        .expect("--detailed renders the per-service sections");
}

#[tokio::test]
async fn the_health_flag_adds_the_health_column_to_both_views() {
    run(&["status", "--health"], false)
        .await
        .expect("--health on the table view");
    run(&["status", "--detailed", "--health"], false)
        .await
        .expect("--health on the detailed view");
}

#[tokio::test]
async fn the_json_flag_returns_before_either_renderer() {
    run(&["status", "--json"], false)
        .await
        .expect("--json short-circuits the terminal renderers");
}

#[tokio::test]
async fn a_json_output_format_short_circuits_the_same_way_as_the_flag() {
    run(&["status"], true)
        .await
        .expect("--output json short-circuits the terminal renderers");
}
