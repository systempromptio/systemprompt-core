//! DB-backed tests for `analytics traffic devices/geo/sources` against seeded
//! engaged sessions.
//!
//! The empty-database suite only reaches the "no rows" arms; seeding engaged
//! sessions (non-bot, with a landing page and a request) makes the per-row
//! projection and percentage arithmetic run.

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

struct Visitor {
    device: &'static str,
    browser: &'static str,
    country: &'static str,
    referrer: &'static str,
    is_bot: bool,
}

async fn seed_traffic(pool: &DbPool) -> String {
    let user_id = unique_user_id("clitrafficmix");
    let email = format!("{}@clitrafficmix.invalid", user_id.as_str());
    seed_user_row(pool, &user_id, &email).await.unwrap();
    let prefix = format!("/covtraffic-{}", Uuid::new_v4().simple());

    let visitors = [
        Visitor {
            device: "desktop",
            browser: "Firefox",
            country: "PT",
            referrer: "google",
            is_bot: false,
        },
        Visitor {
            device: "desktop",
            browser: "Chrome",
            country: "PT",
            referrer: "direct",
            is_bot: false,
        },
        Visitor {
            device: "mobile",
            browser: "Safari",
            country: "ES",
            referrer: "github",
            is_bot: false,
        },
        Visitor {
            device: "bot",
            browser: "curl",
            country: "US",
            referrer: "direct",
            is_bot: true,
        },
    ];

    for visitor in &visitors {
        let session_id = SessionId::generate();
        seed_user_session(pool, &user_id, &session_id)
            .await
            .unwrap();
        sqlx::query(
            "UPDATE user_sessions SET landing_page = $2, device_type = $3, browser = $4, country \
             = $5, referrer_source = $6, is_bot = $7, request_count = 4, started_at = NOW() - \
             INTERVAL '2 minutes' WHERE session_id = $1",
        )
        .bind(session_id.as_str())
        .bind(format!("{prefix}/landing"))
        .bind(visitor.device)
        .bind(visitor.browser)
        .bind(visitor.country)
        .bind(visitor.referrer)
        .bind(visitor.is_bot)
        .execute(pool.pool_arc().unwrap().as_ref())
        .await
        .unwrap();
    }

    prefix
}

fn export_lines(csv: &str) -> usize {
    csv.lines().filter(|l| !l.trim().is_empty()).count()
}

#[tokio::test]
async fn device_breakdown_renders_and_exports_seeded_sessions() {
    let pool = pool().await;
    seed_traffic(&pool).await;
    let ctx = ctx(&pool);

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("devices.csv");
    analytics::execute(
        parse(&[
            "traffic",
            "devices",
            "--limit",
            "100",
            "--export",
            path.to_str().unwrap(),
        ]),
        &ctx,
    )
    .await
    .unwrap();

    let csv = std::fs::read_to_string(&path).unwrap();
    assert!(export_lines(&csv) > 1, "{csv}");
    assert!(csv.contains("device_type"), "{csv}");
}

#[tokio::test]
async fn including_bots_never_shrinks_the_device_breakdown() {
    let pool = pool().await;
    seed_traffic(&pool).await;
    let ctx = ctx(&pool);
    let dir = tempfile::tempdir().unwrap();

    let engaged = dir.path().join("engaged.csv");
    analytics::execute(
        parse(&[
            "traffic",
            "devices",
            "--limit",
            "100",
            "--export",
            engaged.to_str().unwrap(),
        ]),
        &ctx,
    )
    .await
    .unwrap();

    let all = dir.path().join("all.csv");
    analytics::execute(
        parse(&[
            "traffic",
            "devices",
            "--include-all",
            "--limit",
            "100",
            "--export",
            all.to_str().unwrap(),
        ]),
        &ctx,
    )
    .await
    .unwrap();

    let engaged_rows = export_lines(&std::fs::read_to_string(&engaged).unwrap());
    let all_rows = export_lines(&std::fs::read_to_string(&all).unwrap());
    assert!(all_rows >= engaged_rows, "{all_rows} < {engaged_rows}");
}

#[tokio::test]
async fn geo_and_source_breakdowns_render_seeded_sessions() {
    let pool = pool().await;
    seed_traffic(&pool).await;
    let ctx = ctx(&pool);
    let dir = tempfile::tempdir().unwrap();

    for (name, args) in [
        ("geo.csv", vec!["traffic", "geo"]),
        ("sources.csv", vec!["traffic", "sources"]),
    ] {
        let path = dir.path().join(name);
        let mut argv = args.clone();
        argv.extend_from_slice(&["--limit", "100", "--export"]);
        argv.push(path.to_str().unwrap());

        analytics::execute(parse(&argv), &ctx).await.unwrap();

        let csv = std::fs::read_to_string(&path).unwrap();
        assert!(export_lines(&csv) > 1, "{name}: {csv}");
    }
}

#[tokio::test]
async fn bot_traffic_view_renders_with_a_seeded_bot_session() {
    let pool = pool().await;
    seed_traffic(&pool).await;
    let ctx = ctx(&pool);

    analytics::execute(parse(&["traffic", "bots"]), &ctx)
        .await
        .unwrap();
    analytics::execute(parse(&["traffic", "pages"]), &ctx)
        .await
        .unwrap();
}
