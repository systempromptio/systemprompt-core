//! DB-backed tests for `analytics traffic pages` and `analytics traffic
//! navigation`: engaged-traffic filtering, the referrer and path-prefix
//! filters, external-link exclusion, and CSV export.

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

struct Session {
    landing_page: &'static str,
    referrer: Option<&'static str>,
    request_count: i32,
    is_bot: bool,
}

/// Seed one user with the given sessions, all started a minute ago so the
/// default 24h window covers them. Returns the unique path prefix the pages
/// carry, so assertions can ignore rows other tests left behind.
async fn seed_sessions(pool: &DbPool, sessions: &[Session]) -> String {
    let user_id = unique_user_id("clitrafficpages");
    let email = format!("{}@clitrafficpages.invalid", user_id.as_str());
    seed_user_row(pool, &user_id, &email).await.unwrap();
    let prefix = format!("/cov-{}", Uuid::new_v4().simple());

    for session in sessions {
        let session_id = SessionId::generate();
        seed_user_session(pool, &user_id, &session_id).await.unwrap();
        sqlx::query(
            "UPDATE user_sessions SET landing_page = $2, referrer_source = $3, request_count = \
             $4, is_bot = $5, started_at = NOW() - INTERVAL '1 minute' WHERE session_id = $1",
        )
        .bind(session_id.as_str())
        .bind(format!("{prefix}{}", session.landing_page))
        .bind(session.referrer)
        .bind(session.request_count)
        .bind(session.is_bot)
        .execute(pool.pool_arc().unwrap().as_ref())
        .await
        .unwrap();
    }
    prefix
}

async fn seed_link_clicks(pool: &DbPool) -> String {
    let user_id = unique_user_id("clitrafficnav");
    let email = format!("{}@clitrafficnav.invalid", user_id.as_str());
    seed_user_row(pool, &user_id, &email).await.unwrap();
    let prefix = format!("/cov-{}", Uuid::new_v4().simple());

    for (target, external, times) in [
        (format!("{prefix}/docs"), false, 3),
        (format!("{prefix}/pricing"), false, 1),
        (format!("{prefix}/elsewhere"), true, 2),
    ] {
        for _ in 0..times {
            sqlx::query(
                "INSERT INTO analytics_events (user_id, event_type, event_category, severity, \
                 endpoint, event_data, timestamp) VALUES ($1, 'link_click', 'navigation', 'info', \
                 $2, jsonb_build_object('target_url', $3::text, 'is_external', $4::text), NOW() - \
                 INTERVAL '1 minute')",
            )
            .bind(user_id.as_str())
            .bind(format!("{prefix}/home"))
            .bind(&target)
            .bind(external.to_string())
            .execute(pool.pool_arc().unwrap().as_ref())
            .await
            .unwrap();
        }
    }
    prefix
}

fn export_rows(csv: &str, prefix: &str) -> Vec<String> {
    csv.lines()
        .filter(|line| line.contains(prefix))
        .map(ToOwned::to_owned)
        .collect()
}

async fn pages_csv(ctx: &CommandContext, args: &[&str]) -> String {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pages.csv");
    let mut argv = vec!["traffic", "pages", "--export", path.to_str().unwrap()];
    argv.extend_from_slice(args);
    analytics::execute(parse(&argv), ctx).await.unwrap();
    std::fs::read_to_string(&path).unwrap()
}

#[tokio::test]
async fn pages_counts_sessions_per_landing_page() {
    let pool = pool().await;
    let prefix = seed_sessions(
        &pool,
        &[
            Session { landing_page: "/docs", referrer: Some("google"), request_count: 4, is_bot: false },
            Session { landing_page: "/docs", referrer: Some("google"), request_count: 2, is_bot: false },
            Session { landing_page: "/pricing", referrer: None, request_count: 1, is_bot: false },
        ],
    )
    .await;
    let ctx = ctx(&pool);

    let csv = pages_csv(&ctx, &[]).await;
    let rows = export_rows(&csv, &prefix);

    assert_eq!(rows.len(), 2, "one row per landing page and source: {csv}");
    let docs = rows.iter().find(|r| r.contains("/docs")).unwrap();
    assert!(docs.contains("google"), "referrer source is reported: {docs}");
    assert!(docs.contains(",2,"), "both /docs sessions counted: {docs}");
    let pricing = rows.iter().find(|r| r.contains("/pricing")).unwrap();
    assert!(
        pricing.contains("direct"),
        "a null referrer reports as direct: {pricing}"
    );
}

#[tokio::test]
async fn pages_excludes_ghost_and_bot_sessions_unless_include_all() {
    let pool = pool().await;
    let prefix = seed_sessions(
        &pool,
        &[
            Session { landing_page: "/real", referrer: None, request_count: 3, is_bot: false },
            Session { landing_page: "/ghost", referrer: None, request_count: 0, is_bot: false },
            Session { landing_page: "/bot", referrer: None, request_count: 5, is_bot: true },
        ],
    )
    .await;
    let ctx = ctx(&pool);

    let engaged = export_rows(&pages_csv(&ctx, &[]).await, &prefix);
    assert_eq!(engaged.len(), 1, "only the engaged human session: {engaged:?}");
    assert!(engaged[0].contains("/real"));

    let all = export_rows(&pages_csv(&ctx, &["--include-all"]).await, &prefix);
    assert!(
        all.len() > engaged.len(),
        "--include-all widens the set: {all:?}"
    );
}

#[tokio::test]
async fn pages_filters_by_referrer_and_path_prefix() {
    let pool = pool().await;
    let prefix = seed_sessions(
        &pool,
        &[
            Session { landing_page: "/docs/intro", referrer: Some("google"), request_count: 2, is_bot: false },
            Session { landing_page: "/blog/post", referrer: Some("reddit"), request_count: 2, is_bot: false },
        ],
    )
    .await;
    let ctx = ctx(&pool);

    let by_referrer = export_rows(&pages_csv(&ctx, &["--referrer", "reddit"]).await, &prefix);
    assert_eq!(by_referrer.len(), 1, "{by_referrer:?}");
    assert!(by_referrer[0].contains("/blog/post"));

    let by_path = export_rows(
        &pages_csv(&ctx, &["--path-prefix", &format!("{prefix}/docs")]).await,
        &prefix,
    );
    assert_eq!(by_path.len(), 1, "{by_path:?}");
    assert!(by_path[0].contains("/docs/intro"));
}

#[tokio::test]
async fn pages_renders_without_export() {
    let pool = pool().await;
    seed_sessions(
        &pool,
        &[Session { landing_page: "/docs", referrer: None, request_count: 1, is_bot: false }],
    )
    .await;
    let ctx = ctx(&pool);

    analytics::execute(parse(&["traffic", "pages", "--limit", "5"]), &ctx)
        .await
        .unwrap();
}

async fn navigation_csv(ctx: &CommandContext, args: &[&str]) -> String {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("navigation.csv");
    let mut argv = vec!["traffic", "navigation", "--export", path.to_str().unwrap()];
    argv.extend_from_slice(args);
    analytics::execute(parse(&argv), ctx).await.unwrap();
    std::fs::read_to_string(&path).unwrap()
}

#[tokio::test]
async fn navigation_counts_internal_transitions() {
    let pool = pool().await;
    let prefix = seed_link_clicks(&pool).await;
    let ctx = ctx(&pool);

    let rows = export_rows(&navigation_csv(&ctx, &[]).await, &prefix);

    assert_eq!(rows.len(), 2, "external clicks are excluded: {rows:?}");
    let docs = rows.iter().find(|r| r.contains("/docs")).unwrap();
    assert!(docs.contains(",3"), "repeat clicks are summed: {docs}");
}

#[tokio::test]
async fn navigation_include_external_adds_outbound_clicks() {
    let pool = pool().await;
    let prefix = seed_link_clicks(&pool).await;
    let ctx = ctx(&pool);

    let rows = export_rows(&navigation_csv(&ctx, &["--include-external"]).await, &prefix);

    assert_eq!(rows.len(), 3, "{rows:?}");
    assert!(rows.iter().any(|r| r.contains("/elsewhere")));
}

#[tokio::test]
async fn navigation_filters_by_path_prefix() {
    let pool = pool().await;
    let prefix = seed_link_clicks(&pool).await;
    let ctx = ctx(&pool);

    let rows = export_rows(
        &navigation_csv(&ctx, &["--path-prefix", &format!("{prefix}/pricing")]).await,
        &prefix,
    );

    assert_eq!(rows.len(), 1, "{rows:?}");
    assert!(rows[0].contains("/pricing"));
}

#[tokio::test]
async fn navigation_renders_without_export() {
    let pool = pool().await;
    seed_link_clicks(&pool).await;
    let ctx = ctx(&pool);

    analytics::execute(parse(&["traffic", "navigation", "--limit", "5"]), &ctx)
        .await
        .unwrap();
}
