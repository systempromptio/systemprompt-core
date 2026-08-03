//! DB-backed tests for `analytics conversations` against seeded agent
//! contexts.
//!
//! `list_agent_contexts` only returns contexts that carry agent tasks, so the
//! per-row projection never ran on the empty-database suite.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_agent::models::context::ContextKind;
use systemprompt_agent::repository::ContextRepository;
use systemprompt_cli::analytics::{self, AnalyticsCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_database::DbPool;
use systemprompt_identifiers::SessionId;
use systemprompt_runtime::DatabaseContext;
use systemprompt_test_fixtures::{
    fixture_database_url, fixture_db_pool, seed_user_row, seed_user_session, unique_user_id,
};
use uuid::Uuid;

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

async fn seed_conversation(pool: &DbPool) -> String {
    let user_id = unique_user_id("cliconvos");
    let session_id = SessionId::generate();
    let email = format!("{}@cliconvos.invalid", user_id.as_str());
    seed_user_row(pool, &user_id, &email).await.unwrap();
    seed_user_session(pool, &user_id, &session_id)
        .await
        .unwrap();

    let context_id = ContextRepository::new(pool)
        .unwrap()
        .create_context(
            &user_id,
            Some(&session_id),
            "cli-analytics-conversations",
            ContextKind::User,
        )
        .await
        .unwrap()
        .to_string();

    let agent = format!("covconvo_{}", Uuid::new_v4().simple());
    for status in ["TASK_STATE_COMPLETED", "TASK_STATE_WORKING"] {
        sqlx::query(
            "INSERT INTO agent_tasks (task_id, context_id, status, agent_name, started_at, \
             user_id) VALUES ($1, $2, $3, $4, NOW() - INTERVAL '3 minutes', $5)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&context_id)
        .bind(status)
        .bind(&agent)
        .bind(user_id.as_str())
        .execute(pool.pool_arc().unwrap().as_ref())
        .await
        .unwrap();
    }

    context_id
}

#[tokio::test]
async fn the_conversation_listing_projects_seeded_contexts() {
    let pool = pool().await;
    seed_conversation(&pool).await;
    let ctx = ctx(&pool);

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("conversations.csv");
    analytics::execute(
        parse(&[
            "conversations",
            "list",
            "--limit",
            "200",
            "--export",
            path.to_str().unwrap(),
        ]),
        &ctx,
    )
    .await
    .unwrap();

    let csv = std::fs::read_to_string(&path).unwrap();
    assert!(csv.lines().count() > 1, "{csv}");
    assert!(csv.contains("task_count"), "{csv}");
}

#[tokio::test]
async fn conversation_stats_and_trends_render_with_seeded_tasks() {
    let pool = pool().await;
    seed_conversation(&pool).await;
    let ctx = ctx(&pool);

    analytics::execute(parse(&["conversations", "stats"]), &ctx)
        .await
        .unwrap();
    analytics::execute(parse(&["conversations", "trends"]), &ctx)
        .await
        .unwrap();
}

#[tokio::test]
async fn a_limit_of_one_still_renders_a_row() {
    let pool = pool().await;
    seed_conversation(&pool).await;
    let ctx = ctx(&pool);

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("one.csv");
    analytics::execute(
        parse(&[
            "conversations",
            "list",
            "--limit",
            "1",
            "--export",
            path.to_str().unwrap(),
        ]),
        &ctx,
    )
    .await
    .unwrap();

    let csv = std::fs::read_to_string(&path).unwrap();
    let rows = csv.lines().skip(1).filter(|l| !l.trim().is_empty()).count();
    assert_eq!(rows, 1, "{csv}");
}
