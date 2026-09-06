//! `infra logs audit` against a seeded AI request.
//!
//! The command has two shapes — a notice when the id matches nothing, and the
//! full audit card when it does — and `build_audit` titles the card differently
//! for a request that did not complete. All three are pinned here.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_ai::models::AiRequestRecord;
use systemprompt_ai::repository::AiRequestRepository;
use systemprompt_cli::infrastructure::logs::{self, AuditOutput, LogsCommands, build_audit};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{AiRequestId, ContextId, UserId};
use systemprompt_runtime::DatabaseContext;
use systemprompt_test_fixtures::{
    fixture_database_url, fixture_db_pool, seed_user_row, unique_user_id,
};

#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    cmd: LogsCommands,
}

fn parse(args: &[&str]) -> LogsCommands {
    Harness::try_parse_from(std::iter::once("logs").chain(args.iter().copied()))
        .unwrap_or_else(|e| panic!("parse {args:?}: {e}"))
        .cmd
}

async fn pool() -> DbPool {
    fixture_db_pool(&fixture_database_url().expect("a test database url"))
        .await
        .expect("the audit command tests need a reachable test database")
}

fn ctx(pool: &DbPool) -> CommandContext {
    CommandContext::with_database(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
        DatabaseContext::from_pool(pool.clone()),
        fixture_database_url().expect("a test database url"),
    )
}

async fn seed_request(pool: &DbPool) -> AiRequestId {
    let user: UserId = unique_user_id("logs-audit");
    seed_user_row(pool, &user, &format!("{}@audit.invalid", user.as_str()))
        .await
        .expect("seed user");
    let record = AiRequestRecord::builder(AiRequestId::generate(), user, ContextId::generate())
        .provider("anthropic")
        .model("claude-fixture-1")
        .build();
    AiRequestRepository::new(pool)
        .expect("request repo")
        .insert(&record)
        .await
        .expect("seed an ai request")
}

fn audit_output(status: &str) -> AuditOutput {
    AuditOutput {
        request_id: AiRequestId::generate(),
        status: status.to_owned(),
        error_message: None,
        provider: Some("anthropic".to_owned()),
        model: Some("claude-fixture-1".to_owned()),
        requested_model: None,
        input_tokens: 0,
        output_tokens: 0,
        cache_read_tokens: 0,
        cache_creation_tokens: 0,
        reasoning_tokens: 0,
        cost_dollars: 0.0,
        latency_ms: 0,
        task_id: None,
        trace_id: None,
        messages: vec![],
        tool_calls: vec![],
    }
}

#[tokio::test]
async fn auditing_a_seeded_request_renders_its_card() {
    let pool = pool().await;
    let id = seed_request(&pool).await;

    logs::execute(parse(&["audit", id.as_str()]), &ctx(&pool))
        .await
        .expect("a request that exists must render an audit");
}

#[tokio::test]
async fn auditing_an_unknown_id_renders_a_notice_rather_than_failing() {
    let pool = pool().await;

    logs::execute(
        parse(&["audit", "ai_req_that_does_not_exist_anywhere"]),
        &ctx(&pool),
    )
    .await
    .expect("an unknown id is a notice, not an error");
}

fn card_title(output: &AuditOutput) -> String {
    let card = serde_json::to_value(build_audit(output).artifact()).expect("artifact json");
    card.get("title")
        .and_then(|t| t.as_str())
        .unwrap_or_else(|| panic!("no title on {card}"))
        .to_owned()
}

#[test]
fn a_request_that_did_not_complete_says_so_in_the_title() {
    assert_eq!(card_title(&audit_output("completed")), "AI Request Audit");
    assert_eq!(
        card_title(&audit_output("failed")),
        "AI Request Audit — FAILED"
    );
}
