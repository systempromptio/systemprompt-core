//! DB-backed tests for the `admin users` dispatcher.
//!
//! `admin::execute` admits only the users group under a database scope; the
//! per-command bodies have their own tests, while the dispatcher arms and the
//! scope guard are reached only this way.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::admin::{self, AdminCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_database::DbPool;
use systemprompt_identifiers::UserId;
use systemprompt_runtime::DatabaseContext;
use systemprompt_test_fixtures::{
    fixture_database_url, fixture_db_pool, seed_user_row, unique_user_id,
};

#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    cmd: AdminCommands,
}

fn parse(args: &[&str]) -> AdminCommands {
    Harness::try_parse_from(std::iter::once("admin").chain(args.iter().copied()))
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

async fn seed(pool: &DbPool) -> (UserId, String) {
    let user_id = unique_user_id("cliusersdispatch");
    let email = format!("{}@cliusersdispatch.invalid", user_id.as_str());
    seed_user_row(pool, &user_id, &email).await.unwrap();
    (user_id, email)
}

#[tokio::test]
async fn read_only_user_arms_render_against_a_seeded_user() {
    let pool = pool().await;
    let (user_id, email) = seed(&pool).await;
    let ctx = ctx(&pool);

    admin::execute(parse(&["users", "list", "--limit", "5"]), &ctx)
        .await
        .unwrap();
    admin::execute(parse(&["users", "show", user_id.as_str()]), &ctx)
        .await
        .unwrap();
    admin::execute(parse(&["users", "search", &email]), &ctx)
        .await
        .unwrap();
    admin::execute(parse(&["users", "count"]), &ctx)
        .await
        .unwrap();
    admin::execute(parse(&["users", "stats"]), &ctx)
        .await
        .unwrap();
}

#[tokio::test]
async fn export_arm_writes_the_requested_file() {
    let pool = pool().await;
    seed(&pool).await;
    let ctx = ctx(&pool);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("users.json");

    admin::execute(
        parse(&["users", "export", "--output", path.to_str().unwrap()]),
        &ctx,
    )
    .await
    .unwrap();

    assert!(path.exists());
    let exported = std::fs::read_to_string(&path).unwrap();
    assert!(
        exported.starts_with('[') || exported.starts_with('{'),
        "{exported}"
    );
}

#[tokio::test]
async fn show_arm_reports_an_unknown_user() {
    let pool = pool().await;
    let ctx = ctx(&pool);

    let err = admin::execute(parse(&["users", "show", "cov_absent_user"]), &ctx)
        .await
        .unwrap_err();
    assert!(!format!("{err:#}").is_empty());
}

#[tokio::test]
async fn session_listing_is_allowed_but_role_and_session_writes_are_not() {
    let pool = pool().await;
    let (user_id, _email) = seed(&pool).await;
    let ctx = ctx(&pool);

    admin::execute(parse(&["users", "session", "list", user_id.as_str()]), &ctx)
        .await
        .unwrap();

    let role_err = admin::execute(
        parse(&[
            "users",
            "role",
            "assign",
            user_id.as_str(),
            "--roles",
            "user",
        ]),
        &ctx,
    )
    .await
    .unwrap_err();
    assert!(format!("{role_err:#}").contains("Role management operations require full profile"));

    let session_err = admin::execute(parse(&["users", "session", "cleanup", "--yes"]), &ctx)
        .await
        .unwrap_err();
    assert!(format!("{session_err:#}").contains("Write operations require full profile"));
}

#[tokio::test]
async fn every_other_admin_group_is_refused_under_a_database_scope() {
    let pool = pool().await;
    let ctx = ctx(&pool);

    for args in [
        vec!["agents", "list"],
        vec!["config", "show"],
        vec!["keys", "generate"],
        vec!["session", "list"],
    ] {
        let err = admin::execute(parse(&args), &ctx).await.unwrap_err();
        assert!(
            format!("{err:#}").contains("requires full profile context"),
            "{args:?}: {err:#}"
        );
    }
}

#[tokio::test]
async fn user_listing_honours_its_role_and_status_filters() {
    let pool = pool().await;
    seed(&pool).await;
    let ctx = ctx(&pool);

    for args in [
        vec!["users", "list", "--role", "admin"],
        vec!["users", "list", "--role", "user"],
        vec!["users", "list", "--status", "active"],
        vec!["users", "list", "--limit", "3", "--offset", "1"],
    ] {
        admin::execute(parse(&args), &ctx).await.unwrap();
    }
}

#[tokio::test]
async fn user_write_arms_are_refused_under_a_database_scope() {
    let pool = pool().await;
    let (user_id, email) = seed(&pool).await;
    let ctx = ctx(&pool);

    for args in [
        vec!["users", "create", "--name", "covnew", "--email", &email],
        vec![
            "users",
            "update",
            user_id.as_str(),
            "--display-name",
            "covrenamed",
        ],
        vec!["users", "delete", user_id.as_str(), "--yes"],
        vec![
            "users",
            "merge",
            "--source",
            user_id.as_str(),
            "--target",
            user_id.as_str(),
        ],
    ] {
        let err = admin::execute(parse(&args), &ctx).await.unwrap_err();
        assert!(
            format!("{err:#}").contains("Write operations require full profile"),
            "{args:?}: {err:#}"
        );
    }
}
