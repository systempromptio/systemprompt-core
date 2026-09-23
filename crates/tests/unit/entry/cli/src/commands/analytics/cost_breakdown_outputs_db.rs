//! DB-backed cost breakdown tests with assertions on exported command output.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::analytics::{self, AnalyticsCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_database::DbPool;
use systemprompt_runtime::DatabaseContext;
use systemprompt_test_fixtures::{DisposableDb, seed_user_row, unique_user_id};
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

async fn database() -> (DisposableDb, DbPool) {
    let database = DisposableDb::installed("cli_cost_breakdown")
        .await
        .expect("cost breakdown tests need an isolated installed database");
    let pool = database.pool().await.expect("disposable database pool");
    (database, pool)
}

fn ctx(pool: &DbPool, database_url: &str) -> CommandContext {
    CommandContext::with_database(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
        DatabaseContext::from_pool(pool.clone()),
        database_url.to_owned(),
    )
}

struct Seed {
    model_high: String,
    model_low: String,
    provider_high: String,
    provider_low: String,
    named_user: String,
    unnamed_user: String,
}

async fn insert_request(
    pool: &DbPool,
    user_id: &str,
    provider: &str,
    model: &str,
    context_id: &str,
    cost: i64,
    tokens: i64,
    synthetic: bool,
) {
    sqlx::query(
        "INSERT INTO ai_requests (id, request_id, user_id, context_id, provider, model, \
         tokens_used, input_tokens, output_tokens, cost_microdollars, latency_ms, status, \
         actor_kind, actor_id, synthetic, created_at, completed_at) VALUES \
         ($1, $2, $3, $4, $5, $6, $7, $7, 0, $8, 1, 'completed', 'user', $3, $9, NOW(), NOW())",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(Uuid::new_v4().to_string())
    .bind(user_id)
    .bind(context_id)
    .bind(provider)
    .bind(model)
    .bind(tokens)
    .bind(cost)
    .bind(synthetic)
    .execute(pool.pool_arc().expect("SQL pool").as_ref())
    .await
    .expect("seed AI request");
}

async fn seed(pool: &DbPool) -> Seed {
    let tag = Uuid::new_v4().simple().to_string();
    let named = unique_user_id("costbreaknamed");
    let unnamed = unique_user_id("costbreakunnamed");
    seed_user_row(pool, &named, &format!("{named}@costbreak.invalid"))
        .await
        .unwrap();
    seed_user_row(pool, &unnamed, &format!("{unnamed}@costbreak.invalid"))
        .await
        .unwrap();
    sqlx::query("UPDATE users SET name = $1 WHERE id = $2")
        .bind(format!("Cost Owner {tag}"))
        .bind(named.as_str())
        .execute(pool.pool_arc().unwrap().as_ref())
        .await
        .unwrap();
    sqlx::query("UPDATE users SET name = '' WHERE id = $1")
        .bind(unnamed.as_str())
        .execute(pool.pool_arc().unwrap().as_ref())
        .await
        .unwrap();

    let seed = Seed {
        model_high: format!("cost-model-high-{tag}"),
        model_low: format!("cost-model-low-{tag}"),
        provider_high: format!("cost-provider-high-{tag}"),
        provider_low: format!("cost-provider-low-{tag}"),
        named_user: named.to_string(),
        unnamed_user: unnamed.to_string(),
    };
    let conversation_a = Uuid::new_v4().to_string();
    let conversation_b = Uuid::new_v4().to_string();
    insert_request(
        pool,
        &seed.named_user,
        &seed.provider_high,
        &seed.model_high,
        &conversation_a,
        6_000,
        60,
        false,
    )
    .await;
    insert_request(
        pool,
        &seed.named_user,
        &seed.provider_high,
        &seed.model_high,
        &conversation_b,
        3_000,
        30,
        false,
    )
    .await;
    insert_request(
        pool,
        &seed.unnamed_user,
        &seed.provider_low,
        &seed.model_low,
        &conversation_a,
        1_000,
        10,
        false,
    )
    .await;
    insert_request(
        pool,
        &seed.named_user,
        &seed.provider_high,
        &seed.model_high,
        &conversation_a,
        90_000,
        900,
        true,
    )
    .await;
    seed
}

async fn export(ctx: &CommandContext, by: &str, limit: i64) -> String {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(format!("{by}.csv"));
    let limit = limit.to_string();
    analytics::execute(
        parse(&[
            "costs",
            "breakdown",
            "--by",
            by,
            "--limit",
            &limit,
            "--export",
            path.to_str().unwrap(),
        ]),
        ctx,
    )
    .await
    .unwrap();
    std::fs::read_to_string(path).unwrap()
}

fn row<'a>(csv: &'a str, needle: &str) -> Vec<&'a str> {
    csv.lines()
        .find(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("missing {needle} in {csv}"))
        .split(',')
        .map(str::trim)
        .collect()
}

#[tokio::test]
async fn model_and_provider_breakdowns_report_aggregates_and_exclude_synthetic_spend() {
    let (database, pool) = database().await;
    let seed = seed(&pool).await;
    let ctx = ctx(&pool, database.url());

    let models = export(&ctx, "model", 200).await;
    let high = row(&models, &seed.model_high);
    assert_eq!(&high[1..4], ["9000", "2", "90"], "{models}");
    assert_eq!(high[4].parse::<f64>().unwrap(), 90.0, "{models}");
    assert_eq!(
        row(&models, &seed.model_low)[4].parse::<f64>().unwrap(),
        10.0,
        "{models}"
    );
    assert!(
        !models.contains("99000"),
        "synthetic cost leaked into output: {models}"
    );

    let providers = export(&ctx, "provider", 200).await;
    let high = row(&providers, &seed.provider_high);
    let low = row(&providers, &seed.provider_low);
    assert_eq!(&high[1..4], ["9000", "2", "90"], "{providers}");
    assert_eq!(&low[1..4], ["1000", "1", "10"], "{providers}");
    assert_eq!(high[4].parse::<f64>().unwrap(), 90.0, "{providers}");
    assert_eq!(low[4].parse::<f64>().unwrap(), 10.0, "{providers}");

    drop(ctx);
    drop(pool);
    database.drop_now().await;
}

#[tokio::test]
async fn user_breakdown_includes_display_name_and_distinct_conversation_count() {
    let (database, pool) = database().await;
    let seed = seed(&pool).await;
    let ctx = ctx(&pool, database.url());
    let users = export(&ctx, "user", 200).await;

    let named = row(&users, &seed.named_user);
    assert!(named[0].contains("Cost Owner"), "{users}");
    assert_eq!(&named[1..5], ["9000", "2", "90", "2"], "{users}");
    let unnamed = row(&users, &seed.unnamed_user);
    assert_eq!(unnamed[0], seed.unnamed_user, "{users}");
    assert_eq!(unnamed[4], "1", "{users}");

    drop(ctx);
    drop(pool);
    database.drop_now().await;
}

#[tokio::test]
async fn agent_breakdown_accounts_for_requests_without_an_agent_task() {
    let (database, pool) = database().await;
    seed(&pool).await;
    let ctx = ctx(&pool, database.url());
    let agents = export(&ctx, "agent", 200).await;

    let unattributed = row(&agents, "unattributed");
    assert_eq!(
        &unattributed[0..4],
        ["unattributed", "10000", "3", "100"],
        "{agents}"
    );
    assert_eq!(unattributed[4].parse::<f64>().unwrap(), 100.0, "{agents}");

    drop(ctx);
    drop(pool);
    database.drop_now().await;
}
