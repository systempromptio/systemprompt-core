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
    systemprompt_test_fixtures::refresh_reporting(&pool)
        .await
        .unwrap();
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
    systemprompt_test_fixtures::refresh_reporting(&pool)
        .await
        .unwrap();
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
    systemprompt_test_fixtures::refresh_reporting(&pool)
        .await
        .unwrap();
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

// An instance without A2A agents holds every conversation as a gateway
// session; the listing must be able to show those, alone or beside agent
// contexts, and narrow to one user.
#[tokio::test]
async fn the_conversation_listing_accepts_source_and_user_filters() {
    let pool = pool().await;
    seed_conversation(&pool).await;
    systemprompt_test_fixtures::refresh_reporting(&pool)
        .await
        .unwrap();
    let ctx = ctx(&pool);

    for source in ["agent", "gateway", "all"] {
        analytics::execute(
            parse(&["conversations", "list", "--source", source, "--limit", "5"]),
            &ctx,
        )
        .await
        .unwrap_or_else(|e| panic!("--source {source} must list: {e}"));
    }
    analytics::execute(
        parse(&[
            "conversations",
            "list",
            "--user",
            "nobody_here",
            "--since",
            "7d",
        ]),
        &ctx,
    )
    .await
    .expect("an unknown user is an empty listing, not an error");
}

#[test]
fn the_conversation_listing_refuses_a_non_positive_limit() {
    for limit in ["0", "-3"] {
        let err = Harness::try_parse_from(["analytics", "conversations", "list", "--limit", limit])
            .expect_err("a non-positive limit is a usage error");
        assert_eq!(
            err.kind(),
            clap::error::ErrorKind::ValueValidation,
            "--limit {limit}"
        );
    }
}

#[tokio::test]
async fn gateway_conversations_project_exact_owners_counts_and_global_order() {
    use systemprompt_test_fixtures::{DisposableDb, install_test_signing_key};

    systemprompt_test_fixtures::ensure_test_bootstrap();
    install_test_signing_key();
    let database = DisposableDb::installed("cli_gateway_conversation_projection")
        .await
        .expect("private gateway conversation database");
    let pool = database.pool().await.expect("private gateway pool");
    let raw = pool.pool_arc().expect("private SQL pool");
    let older_owner = unique_user_id("gwconvo_old");
    let newer_owner = unique_user_id("gwconvo_new");
    let older_session = SessionId::generate();
    let newer_session = SessionId::generate();
    for (owner, session) in [
        (&older_owner, &older_session),
        (&newer_owner, &newer_session),
    ] {
        seed_user_row(
            &pool,
            owner,
            &format!("{}@gateway-conversations.invalid", owner.as_str()),
        )
        .await
        .expect("seed gateway owner");
        seed_user_session(&pool, owner, session)
            .await
            .expect("seed gateway session");
    }
    for (owner, session, age_minutes, messages) in [
        (&older_owner, &older_session, 10_i32, 1_i32),
        (&newer_owner, &newer_session, 2_i32, 2_i32),
    ] {
        let request_id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO ai_requests (id, request_id, user_id, session_id, context_id, provider, \
             model, status, actor_kind, actor_id, created_at, completed_at) \
             VALUES ($1, $1, $2, $3, '00000000-0000-0000-0000-00000000c0de', 'fixture', \
             'fixture-model', 'completed', 'user', $2, NOW() - make_interval(mins => $4), \
             NOW() - make_interval(mins => $4))",
        )
        .bind(&request_id)
        .bind(owner.as_str())
        .bind(session.as_str())
        .bind(age_minutes)
        .execute(raw.as_ref())
        .await
        .expect("seed gateway AI request");
        for sequence in 0..messages {
            sqlx::query(
                "INSERT INTO ai_request_messages (request_id, role, content, sequence_number) \
                 VALUES ($1, 'user', $2, $3)",
            )
            .bind(&request_id)
            .bind(format!("gateway message {sequence}"))
            .bind(sequence)
            .execute(raw.as_ref())
            .await
            .expect("seed gateway request message");
        }
    }
    systemprompt_test_fixtures::refresh_reporting(&pool)
        .await
        .expect("refresh gateway reporting projection");
    let context = CommandContext::with_database(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
        DatabaseContext::from_pool(pool.clone()),
        database.url().to_owned(),
    );
    let output = tempfile::tempdir().expect("gateway CSV directory");
    let all_path = output.path().join("gateway.csv");
    analytics::execute(
        parse(&[
            "conversations",
            "list",
            "--source",
            "gateway",
            "--since",
            "1h",
            "--limit",
            "2",
            "--export",
            all_path.to_str().expect("gateway CSV path"),
        ]),
        &context,
    )
    .await
    .expect("export gateway conversations");
    let rows = std::fs::read_to_string(&all_path).expect("read gateway CSV");
    let rows = rows.lines().skip(1).collect::<Vec<_>>();
    assert_eq!(rows.len(), 2, "{rows:?}");
    fn fields(row: &str) -> Vec<&str> {
        row.split(',').collect()
    }
    let newest = fields(rows[0]);
    let oldest = fields(rows[1]);
    assert_eq!(newest[0], newer_session.as_str(), "{newest:?}");
    assert_eq!(newest[1], "gateway", "{newest:?}");
    assert_eq!(newest[2], newer_owner.as_str(), "{newest:?}");
    assert_eq!(newest[4], "0", "{newest:?}");
    assert_eq!(newest[5], "2", "{newest:?}");
    assert_eq!(oldest[0], older_session.as_str(), "{oldest:?}");
    assert_eq!(oldest[2], older_owner.as_str(), "{oldest:?}");
    assert_eq!(oldest[5], "1", "{oldest:?}");

    let filtered_path = output.path().join("owner.csv");
    analytics::execute(
        parse(&[
            "conversations",
            "list",
            "--source",
            "all",
            "--since",
            "1h",
            "--user",
            older_owner.as_str(),
            "--limit",
            "1",
            "--export",
            filtered_path.to_str().expect("owner CSV path"),
        ]),
        &context,
    )
    .await
    .expect("filter gateway conversation by owner");
    let filtered = std::fs::read_to_string(filtered_path).expect("read owner CSV");
    let filtered = filtered.lines().skip(1).collect::<Vec<_>>();
    assert_eq!(filtered.len(), 1, "{filtered:?}");
    assert_eq!(fields(filtered[0])[0], older_session.as_str());

    drop(raw);
    drop(context);
    pool.write_pool_arc().expect("write pool").close().await;
    drop(pool);
    database.drop_now().await;
}
