//! DB-backed tests for `analytics tools list`, `analytics requests`, and
//! `analytics content top` against seeded rows.
//!
//! The empty-database suite only ever reaches the "nothing found" arms; these
//! seed real rows so the per-row mapping, sorting, and CSV export paths run.
//! Tool assertions scope server-side via `--server <unique-name>`, since the
//! listing is a global top-N.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
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

async fn seed_tool_server(pool: &DbPool) -> String {
    let server = format!("covsrv_{}", Uuid::new_v4().simple());

    for (tool, status, ms) in [
        ("cov_alpha", "success", 10),
        ("cov_alpha", "success", 30),
        ("cov_alpha", "failed", 90),
        ("cov_beta", "success", 5),
    ] {
        // `trace_id` is force-unwrapped by the global tool-listing queries, so
        // a NULL here breaks every unscoped `logs tools list` in the suite.
        sqlx::query(
            "INSERT INTO mcp_tool_executions (mcp_execution_id, tool_name, server_name, \
             started_at, execution_time_ms, input, status, user_id, trace_id, created_at) VALUES \
             ($1, $2, $3, NOW(), $4, '{}', $5, 'cli-usage-cov', $6, NOW())",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(tool)
        .bind(&server)
        .bind(ms)
        .bind(status)
        .bind(Uuid::new_v4().to_string())
        .execute(pool.pool_arc().unwrap().as_ref())
        .await
        .unwrap();
    }
    server
}

async fn seed_ai_requests(pool: &DbPool) -> String {
    let model = format!("covmodel-{}", Uuid::new_v4().simple());
    let user_id = unique_user_id("cliusagereq");
    let email = format!("{}@cliusagereq.invalid", user_id.as_str());
    seed_user_row(pool, &user_id, &email).await.unwrap();

    for (tokens, cost, latency) in [(100, 2_000, 250), (250, 5_000, 400)] {
        sqlx::query(
            "INSERT INTO ai_requests (id, request_id, user_id, provider, model, tokens_used, \
             input_tokens, output_tokens, cost_microdollars, latency_ms, status, actor_kind, \
             actor_id, created_at, completed_at) VALUES ($1, $2, $3, 'covprovider', $4, $5, $5, \
             $5, $6, $7, 'completed', 'user', $3, NOW(), NOW())",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(Uuid::new_v4().to_string())
        .bind(user_id.as_str())
        .bind(&model)
        .bind(tokens)
        .bind(cost)
        .bind(latency)
        .execute(pool.pool_arc().unwrap().as_ref())
        .await
        .unwrap();
    }
    model
}

async fn seed_engagement(pool: &DbPool) {
    let user_id = unique_user_id("cliusagecontent");
    let session_id = SessionId::generate();
    let email = format!("{}@cliusagecontent.invalid", user_id.as_str());
    seed_user_row(pool, &user_id, &email).await.unwrap();
    seed_user_session(pool, &user_id, &session_id)
        .await
        .unwrap();

    let content_id = format!("covcontent_{}", Uuid::new_v4().simple());
    for depth in [40, 90] {
        sqlx::query(
            "INSERT INTO engagement_events (session_id, user_id, page_url, content_id, \
             event_type, time_on_page_ms, max_scroll_depth, created_at) VALUES ($1, $2, $3, $4, \
             'page_exit', 45000, $5, NOW())",
        )
        .bind(session_id.as_str())
        .bind(user_id.as_str())
        .bind(format!("/cov/{content_id}"))
        .bind(&content_id)
        .bind(depth)
        .execute(pool.pool_arc().unwrap().as_ref())
        .await
        .unwrap();
    }
}

#[tokio::test]
async fn tools_list_scoped_to_a_server_renders_every_sort_order() {
    let pool = pool().await;
    let server = seed_tool_server(&pool).await;
    let ctx = ctx(&pool);

    for sort in ["execution-count", "success-rate", "avg-time"] {
        let output = analytics::execute(
            parse(&["tools", "list", "--server", &server, "--sort-by", sort]),
            &ctx,
        )
        .await;
        assert!(output.is_ok(), "sort {sort} failed");
    }
}

#[tokio::test]
async fn tools_list_export_contains_only_the_seeded_server_rows() {
    let pool = pool().await;
    let server = seed_tool_server(&pool).await;
    let ctx = ctx(&pool);

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tools.csv");
    analytics::execute(
        parse(&[
            "tools",
            "list",
            "--server",
            &server,
            "--export",
            path.to_str().unwrap(),
        ]),
        &ctx,
    )
    .await
    .unwrap();

    let csv = std::fs::read_to_string(&path).unwrap();
    let rows: Vec<&str> = csv.lines().skip(1).filter(|l| !l.is_empty()).collect();
    assert_eq!(rows.len(), 2, "{csv}");
    assert!(csv.contains("cov_alpha"), "{csv}");
    assert!(csv.contains("cov_beta"), "{csv}");
}

#[tokio::test]
async fn tools_list_limit_is_applied_server_side() {
    let pool = pool().await;
    let server = seed_tool_server(&pool).await;
    let ctx = ctx(&pool);

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tools-limited.csv");
    analytics::execute(
        parse(&[
            "tools",
            "list",
            "--server",
            &server,
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
    let rows: Vec<&str> = csv.lines().skip(1).filter(|l| !l.is_empty()).collect();
    assert_eq!(rows.len(), 1, "{csv}");
}

#[tokio::test]
async fn tools_list_for_an_unseeded_server_reports_nothing() {
    let pool = pool().await;
    let ctx = ctx(&pool);

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tools-empty.csv");
    analytics::execute(
        parse(&[
            "tools",
            "list",
            "--server",
            "covsrv_definitely_absent",
            "--export",
            path.to_str().unwrap(),
        ]),
        &ctx,
    )
    .await
    .unwrap();

    let csv = std::fs::read_to_string(&path).unwrap();
    assert!(!csv.contains("cov_alpha"), "{csv}");
}

#[tokio::test]
async fn requests_views_render_with_seeded_ai_requests() {
    let pool = pool().await;
    let model = seed_ai_requests(&pool).await;
    let ctx = ctx(&pool);

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("models.csv");
    analytics::execute(
        parse(&[
            "requests",
            "models",
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
    assert!(csv.contains(&model), "{csv}");
    assert!(csv.contains("covprovider"), "{csv}");

    for args in [
        vec!["requests", "stats"],
        vec!["requests", "list"],
        vec!["requests", "trends"],
        vec!["costs", "summary"],
        vec!["costs", "breakdown"],
        vec!["costs", "trends"],
    ] {
        analytics::execute(parse(&args), &ctx).await.unwrap();
    }
}

#[tokio::test]
async fn content_views_render_with_seeded_engagement_events() {
    let pool = pool().await;
    seed_engagement(&pool).await;
    let ctx = ctx(&pool);

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("content.csv");
    analytics::execute(
        parse(&[
            "content",
            "top",
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

    for args in [
        vec!["content", "stats"],
        vec!["content", "trends"],
        vec!["overview"],
    ] {
        analytics::execute(parse(&args), &ctx).await.unwrap();
    }
}

async fn seed_aged_ai_request(pool: &DbPool, age_days: i32, cost: i64) {
    let user_id = unique_user_id("cliusagewiden");
    let email = format!("{}@cliusagewiden.invalid", user_id.as_str());
    seed_user_row(pool, &user_id, &email).await.unwrap();

    sqlx::query(
        "INSERT INTO ai_requests (id, request_id, user_id, provider, model, tokens_used, \
         input_tokens, output_tokens, cost_microdollars, latency_ms, status, actor_kind, \
         actor_id, created_at, completed_at) VALUES ($1, $2, $3, 'covprovider', 'covwiden', 10, \
         5, 5, $4, 10, 'completed', 'user', $3, NOW() - ($5 || ' days')::interval, NOW())",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(Uuid::new_v4().to_string())
    .bind(user_id.as_str())
    .bind(cost)
    .bind(age_days.to_string())
    .execute(pool.pool_arc().unwrap().as_ref())
    .await
    .unwrap();
}

async fn costs_summary_csv(ctx: &CommandContext, extra: &[&str]) -> String {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("summary.csv");
    let mut args = vec!["costs", "summary", "--export", path.to_str().unwrap()];
    args.extend_from_slice(extra);
    analytics::execute(parse(&args), ctx).await.unwrap();
    std::fs::read_to_string(&path).unwrap()
}

#[tokio::test]
async fn costs_summary_widens_past_an_empty_default_window() {
    let pool = pool().await;
    seed_aged_ai_request(&pool, 3, 4_000).await;
    let ctx = ctx(&pool);

    let widened = costs_summary_csv(&ctx, &[]).await;
    let requests: i64 = csv_field(&widened, "total_requests").parse().unwrap();
    assert!(
        requests > 0,
        "the default window must reach a row seeded 3 days ago: {widened}"
    );

    let narrow = costs_summary_csv(&ctx, &["--since", "24h"]).await;
    assert!(
        csv_field(&narrow, "auto_widened_to").is_empty(),
        "an explicit --since must be honoured verbatim: {narrow}"
    );
}

fn csv_field(csv: &str, name: &str) -> String {
    let mut lines = csv.lines();
    let header = lines.next().unwrap();
    let row = lines.next().unwrap();
    let idx = header.split(',').position(|h| h.trim() == name).unwrap();
    row.split(',').nth(idx).unwrap().trim().to_owned()
}
