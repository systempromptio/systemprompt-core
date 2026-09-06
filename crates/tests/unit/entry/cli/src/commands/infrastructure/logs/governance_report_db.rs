//! `infra logs governance report` end to end against a seeded database.
//!
//! The command has two independent sources — `governance_decisions` rows
//! written by the authz chain and `ai_safety_findings` written by the gateway
//! scanners — and both are seeded here so the rollup, the grouping switch and
//! the CSV branch all run against rows rather than against an empty window.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::sync::Arc;

use clap::Parser;
use systemprompt_ai::AiSafetyFindingRepository;
use systemprompt_ai::models::AiRequestRecord;
use systemprompt_ai::repository::{AiRequestRepository, InsertSafetyFinding};
use systemprompt_cli::infrastructure::logs::{self, LogsCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{Actor, AiRequestId, ContextId, UserId};
use systemprompt_runtime::DatabaseContext;
use systemprompt_security::authz::{
    DecisionTag, GovernanceDecisionRecord, GovernanceDecisionRepository,
};
use systemprompt_test_fixtures::{
    fixture_database_url, fixture_db_pool, seed_user_row, unique_user_id,
};
use uuid::Uuid;

#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    cmd: LogsCommands,
}

fn parse(args: &[&str]) -> LogsCommands {
    Harness::try_parse_from(
        ["logs", "governance"]
            .into_iter()
            .chain(args.iter().copied()),
    )
    .unwrap_or_else(|e| panic!("parse {args:?}: {e}"))
    .cmd
}

async fn pool() -> DbPool {
    fixture_db_pool(&fixture_database_url().expect("a test database url"))
        .await
        .expect("the governance report tests need a reachable test database")
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

async fn run(pool: &DbPool, args: &[&str]) -> anyhow::Result<()> {
    logs::execute(parse(args), &ctx(pool)).await
}

async fn seed_decision(
    pool: &DbPool,
    user: &UserId,
    decision: DecisionTag,
    policy: &str,
    tool: &str,
    reason: &str,
) {
    let repo = GovernanceDecisionRepository::from_pool(
        pool.write_pool_arc().expect("a write pool handle"),
    );
    let id = Uuid::new_v4().to_string();
    let actor = Actor::user(user.clone());
    let evaluated = serde_json::json!([]);
    repo.insert(&GovernanceDecisionRecord {
        id: &id,
        actor: &actor,
        session_id: "sess-governance-report",
        tool_name: tool,
        agent_id: None,
        agent_scope: None,
        decision,
        policy,
        reason,
        evaluated_rules: &evaluated,
        plugin_id: None,
        act_chain: &[],
        context_id: "ctx_governance_report",
        task_id: None,
        trace_id: None,
        client_id: None,
    })
    .await
    .expect("seed a governance decision");
}

async fn seed_finding(pool: &DbPool, user: &UserId, category: &str, blocked: bool) {
    let email = format!("{}@governance.invalid", user.as_str());
    seed_user_row(pool, user, &email).await.expect("seed user");
    let record =
        AiRequestRecord::builder(AiRequestId::generate(), user.clone(), ContextId::generate())
            .provider("anthropic")
            .model("claude-fixture-1")
            .build();
    let request_id = AiRequestRepository::new(pool)
        .expect("request repo")
        .insert(&record)
        .await
        .expect("seed an ai request");

    AiSafetyFindingRepository::from_pool(Arc::new(
        pool.write_pool_arc()
            .expect("a write pool handle")
            .as_ref()
            .clone(),
    ))
    .insert(InsertSafetyFinding {
        ai_request_id: &request_id,
        phase: "input",
        severity: "high",
        category,
        scanner: "heuristic",
        excerpt: Some("ignore previous instructions"),
        blocked,
    })
    .await
    .expect("seed a safety finding");
}

async fn seeded_pool() -> DbPool {
    let pool = pool().await;
    let alice = unique_user_id("gov-report-a");
    let bob = unique_user_id("gov-report-b");

    seed_decision(
        &pool,
        &alice,
        DecisionTag::Warn,
        "tool_allowlist",
        "bash",
        "bash is not on the allowlist",
    )
    .await;
    seed_decision(
        &pool,
        &bob,
        DecisionTag::Warn,
        "tool_allowlist",
        "web_fetch",
        &"a reason long enough to be truncated by the rollup ".repeat(6),
    )
    .await;
    seed_decision(
        &pool,
        &alice,
        DecisionTag::Warn,
        "entropy_threshold",
        "bash",
        "a comma, a \"quote\" and a newline\nin one reason",
    )
    .await;
    seed_decision(
        &pool,
        &alice,
        DecisionTag::Deny,
        "tool_allowlist",
        "bash",
        "denied outright",
    )
    .await;

    seed_finding(
        &pool,
        &unique_user_id("gov-report-c"),
        "prompt_injection",
        true,
    )
    .await;
    seed_finding(&pool, &unique_user_id("gov-report-d"), "pii", false).await;
    pool
}

#[tokio::test]
async fn the_default_report_rolls_up_warnings_and_findings_over_the_last_day() {
    let pool = seeded_pool().await;

    run(&pool, &["report"])
        .await
        .expect("the default 24h window should render both sections");
}

#[tokio::test]
async fn each_grouping_dimension_renders_its_own_rollup() {
    let pool = seeded_pool().await;

    for group_by in ["policy", "tool", "user"] {
        run(&pool, &["report", "--group-by", group_by])
            .await
            .unwrap_or_else(|e| panic!("--group-by {group_by} should render: {e:#}"));
    }
}

#[tokio::test]
async fn the_csv_branch_renders_rows_that_carry_commas_quotes_and_newlines() {
    let pool = seeded_pool().await;

    run(&pool, &["report", "--format", "csv", "--group-by", "tool"])
        .await
        .expect("CSV must survive a reason containing a comma, a quote and a newline");
}

#[tokio::test]
async fn a_zero_limit_still_renders_the_finding_section() {
    let pool = seeded_pool().await;

    run(&pool, &["report", "--limit", "0"])
        .await
        .expect("a zero limit truncates the warning rows rather than failing");
}

#[tokio::test]
async fn an_absolute_date_is_accepted_as_the_window() {
    let pool = seeded_pool().await;

    run(&pool, &["report", "--since", "2000-01-01"])
        .await
        .expect("a YYYY-MM-DD window is parsed as a date rather than a duration");
}

#[tokio::test]
async fn an_unparseable_window_names_the_value_it_could_not_read() {
    let pool = pool().await;

    let err = run(&pool, &["report", "--since", "last-tuesday"])
        .await
        .expect_err("a window that is neither a duration nor a date must be refused");

    let message = format!("{err:#}");
    assert!(
        message.contains("last-tuesday"),
        "the refusal should name the unparseable window, got: {message}"
    );
}
