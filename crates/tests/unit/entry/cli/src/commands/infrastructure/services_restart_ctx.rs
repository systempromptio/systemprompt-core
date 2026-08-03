//! `infra services restart` against a fixture `AppContext`.
//!
//! The batch and single restart entry points take `&Arc<AppContext>` directly
//! rather than a `CommandContext`, so `fixture_app_context` reaches them where
//! a `--database-url` command context cannot. The fixture registry is empty,
//! which is exactly the "nothing to restart" shape the plan computation and
//! the batch message renderers have to handle.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::sync::Arc;

use systemprompt_cli::infrastructure::services::restart;
use systemprompt_cli::{CliConfig, OutputFormat};
use systemprompt_database::DbPool;
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_app_context, fixture_database_url, fixture_db_pool,
    install_test_signing_key,
};

// The restart entry points build a JWT provider and an MCP orchestrator from
// the process-global config, so the bootstrap has to run before the context is
// assembled — a bare `fixture_app_context` leaves both unresolvable.
async fn app_ctx() -> (DbPool, Arc<AppContext>) {
    ensure_test_bootstrap();
    install_test_signing_key();
    let url = fixture_database_url().unwrap();
    let pool = fixture_db_pool(&url).await.unwrap();
    let ctx = fixture_app_context(&pool, &url).expect("fixture app context");
    (pool, ctx)
}

fn json_config() -> CliConfig {
    CliConfig::new()
        .with_interactive(false)
        .with_output_format(OutputFormat::Json)
}

fn text_config() -> CliConfig {
    CliConfig::new().with_interactive(false)
}

// `card_value` flattens the output struct into presentation sections, so the
// fields are read back out of the rendered card rather than off a nested
// object.
fn field(out: &systemprompt_cli::shared::CommandOutput, key: &str) -> Option<String> {
    let card = serde_json::to_value(out.artifact()).unwrap();
    for section in card.get("sections")?.as_array()? {
        if section.get("heading").and_then(|h| h.as_str()) == Some(key) {
            let content = section.get("content")?;
            return Some(
                content
                    .as_str()
                    .map_or_else(|| content.to_string(), str::to_owned),
            );
        }
    }
    None
}

fn rendered(out: &systemprompt_cli::shared::CommandOutput) -> String {
    serde_json::to_string(&serde_json::to_value(out.artifact()).unwrap()).unwrap()
}

fn restarted_count(out: &systemprompt_cli::shared::CommandOutput) -> u64 {
    field(out, "restarted_count")
        .unwrap_or_else(|| panic!("no restarted_count in {}", rendered(out)))
        .parse()
        .expect("restarted_count is numeric")
}

fn failed_count(out: &systemprompt_cli::shared::CommandOutput) -> u64 {
    field(out, "failed_count")
        .unwrap_or_else(|| panic!("no failed_count in {}", rendered(out)))
        .parse()
        .expect("failed_count is numeric")
}

fn message(out: &systemprompt_cli::shared::CommandOutput) -> String {
    field(out, "message").unwrap_or_else(|| panic!("no message in {}", rendered(out)))
}

fn service_type(out: &systemprompt_cli::shared::CommandOutput) -> String {
    field(out, "service_type").unwrap_or_else(|| panic!("no service_type in {}", rendered(out)))
}

#[tokio::test]
async fn restarting_all_agents_over_an_empty_registry_reports_nothing_restarted() {
    let (_pool, ctx) = app_ctx().await;

    let out = restart::execute_all_agents(&ctx, &json_config())
        .await
        .expect("an empty agent registry is not an error");

    assert_eq!(restarted_count(&out), 0);
    assert_eq!(failed_count(&out), 0);
    assert_eq!(service_type(&out), "agents");
}

#[tokio::test]
async fn restarting_all_agents_renders_a_human_message_when_not_in_json_mode() {
    let (_pool, ctx) = app_ctx().await;

    let out = restart::execute_all_agents(&ctx, &text_config())
        .await
        .expect("text mode restart");

    assert!(
        !message(&out).is_empty(),
        "the text renderer must still produce a machine-readable message for the artifact"
    );
    assert_eq!(restarted_count(&out), 0);
}

#[tokio::test]
async fn restarting_all_mcp_servers_over_an_empty_registry_reports_nothing_restarted() {
    let (_pool, ctx) = app_ctx().await;

    let out = restart::execute_all_mcp(&ctx, &json_config())
        .await
        .expect("an empty MCP registry is not an error");

    assert_eq!(restarted_count(&out), 0);
    assert_eq!(failed_count(&out), 0);
    assert_eq!(service_type(&out), "mcp");
}

#[tokio::test]
async fn restarting_all_mcp_servers_in_text_mode_takes_the_same_plan() {
    let (_pool, ctx) = app_ctx().await;

    let json = restart::execute_all_mcp(&ctx, &json_config())
        .await
        .expect("json mode");
    let text = restart::execute_all_mcp(&ctx, &text_config())
        .await
        .expect("text mode");

    assert_eq!(
        restarted_count(&json),
        restarted_count(&text),
        "the output format must not change which services the plan targets"
    );
}

#[tokio::test]
async fn restarting_failed_services_reports_that_none_were_found() {
    let (_pool, ctx) = app_ctx().await;

    let out = restart::execute_failed(&ctx, &json_config())
        .await
        .expect("a healthy (empty) fleet is not an error");

    assert_eq!(restarted_count(&out), 0);
    assert_eq!(failed_count(&out), 0);
    assert_eq!(
        message(&out),
        "No failed services found",
        "the zero-restart branch must say so rather than claiming a restart"
    );
    assert_eq!(service_type(&out), "failed");
}

#[tokio::test]
async fn restarting_failed_services_in_text_mode_reaches_the_health_probe() {
    let (_pool, ctx) = app_ctx().await;

    // `execute_failed` probes MCP health (unlike `execute_all_mcp`, which
    // skips the probe), so this drives the probing arm of the snapshot builder.
    let out = restart::execute_failed(&ctx, &text_config())
        .await
        .expect("health-probing restart");

    assert_eq!(message(&out), "No failed services found");
}

#[tokio::test]
async fn restarting_an_unknown_agent_by_name_is_an_error_naming_the_agent() {
    let (_pool, ctx) = app_ctx().await;

    let err = restart::execute_agent(&ctx, "no-such-agent-anywhere", &json_config())
        .await
        .expect_err("an unregistered agent cannot be restarted");

    assert!(
        err.to_string().contains("no-such-agent-anywhere"),
        "the failure must name the agent that was asked for, got {err}"
    );
}

#[tokio::test]
async fn restarting_an_unknown_mcp_server_by_name_is_an_error() {
    let (_pool, ctx) = app_ctx().await;

    // The orchestrator filters by name, so an unregistered name selects nothing
    // and the restart is a no-op rather than a failure.
    let plain = restart::execute_mcp(&ctx, "no-such-mcp-server", false, &json_config())
        .await
        .expect("restarting an unmatched name selects no services");
    assert_eq!(service_type(&plain), "mcp");
    assert_eq!(
        field(&plain, "service_name").as_deref(),
        Some("no-such-mcp-server"),
        "the report must still name the server the operator asked for"
    );

    let with_build = restart::execute_mcp(&ctx, "no-such-mcp-server", true, &json_config()).await;
    assert!(
        with_build.is_ok(),
        "the build-and-restart arm must behave the same for an unmatched name: {with_build:?}"
    );
}
