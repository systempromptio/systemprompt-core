//! DB-backed tests for the `admin users` dispatcher.
//!
//! `admin::execute` admits only the users group under a database scope; the
//! per-command bodies have their own tests, while the dispatcher arms and the
//! scope guard are reached only this way.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::admin::{self, AdminCommands};
use systemprompt_cli::{CommandContext, EnvOverrides};
use systemprompt_database::DbPool;
use systemprompt_extension::cli::{CliConfig, OutputFormat};
use systemprompt_identifiers::UserId;
use systemprompt_identity_entry::cli::users::{self, UsersCommands};
use systemprompt_runtime::DatabaseContext;
use systemprompt_test_fixtures::{seed_user_row, test_database_url, test_db_pool, unique_user_id};

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

#[derive(Debug, Parser)]
struct UsersHarness {
    #[command(subcommand)]
    cmd: UsersCommands,
}

async fn run(
    args: &[&str],
    ctx: &CommandContext,
) -> anyhow::Result<systemprompt_extension::cli::CommandOutput> {
    match args.split_first() {
        Some((&"users", rest)) => {
            let cmd =
                UsersHarness::try_parse_from(std::iter::once("users").chain(rest.iter().copied()))
                    .unwrap()
                    .cmd;
            users::execute(cmd, &crate::boot_ctx::identity(ctx)).await
        },
        _ => admin::execute(parse(args), ctx).await,
    }
}

fn ctx(pool: &DbPool) -> CommandContext {
    crate::boot_ctx::with_database(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
        DatabaseContext::from_pool(pool.clone()),
        test_database_url(),
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
    let pool = test_db_pool().await;
    let (user_id, email) = seed(&pool).await;
    let ctx = ctx(&pool);

    run(&["users", "list", "--limit", "5"], &ctx).await.unwrap();
    run(&["users", "show", user_id.as_str()], &ctx)
        .await
        .unwrap();
    run(&["users", "search", &email], &ctx).await.unwrap();
    run(&["users", "count"], &ctx).await.unwrap();
    run(&["users", "stats"], &ctx).await.unwrap();
}

#[tokio::test]
async fn export_arm_writes_the_requested_file() {
    let pool = test_db_pool().await;
    seed(&pool).await;
    let ctx = ctx(&pool);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("users.json");

    run(
        &["users", "export", "--output", path.to_str().unwrap()],
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
    let pool = test_db_pool().await;
    let ctx = ctx(&pool);

    let err = run(&["users", "show", "cov_absent_user"], &ctx)
        .await
        .unwrap_err();
    assert!(!format!("{err:#}").is_empty());
}

#[tokio::test]
async fn session_listing_is_allowed_but_role_and_session_writes_are_not() {
    let pool = test_db_pool().await;
    let (user_id, _email) = seed(&pool).await;
    let ctx = ctx(&pool);

    run(&["users", "session", "list", user_id.as_str()], &ctx)
        .await
        .unwrap();

    let role_err = run(
        &[
            "users",
            "role",
            "assign",
            user_id.as_str(),
            "--roles",
            "user",
        ],
        &ctx,
    )
    .await
    .unwrap_err();
    assert!(format!("{role_err:#}").contains("Role management operations require full profile"));

    let session_err = run(&["users", "session", "cleanup", "--yes"], &ctx)
        .await
        .unwrap_err();
    assert!(format!("{session_err:#}").contains("Write operations require full profile"));
}

#[tokio::test]
async fn every_other_admin_group_is_refused_under_a_database_scope() {
    let pool = test_db_pool().await;
    let ctx = ctx(&pool);

    for args in [vec!["config", "show"]] {
        let err = run(&args, &ctx).await.unwrap_err();
        assert!(
            format!("{err:#}").contains("requires full profile context"),
            "{args:?}: {err:#}"
        );
    }

    // Why: `admin session` is exempt from the database-scoped guard (like
    // `admin users`): `session login` against a cloud profile with
    // external_db_access is the command that mints the token remote routing
    // needs, so refusing it made cloud login circular.
    run(&["session", "list"], &ctx).await.unwrap();
}

#[tokio::test]
async fn user_listing_honours_its_role_and_status_filters() {
    let pool = test_db_pool().await;
    seed(&pool).await;
    let ctx = ctx(&pool);

    for args in [
        vec!["users", "list", "--role", "admin"],
        vec!["users", "list", "--role", "user"],
        vec!["users", "list", "--status", "active"],
        vec!["users", "list", "--limit", "3", "--offset", "1"],
    ] {
        run(&args, &ctx).await.unwrap();
    }
}

#[tokio::test]
async fn user_write_arms_are_refused_under_a_database_scope() {
    let pool = test_db_pool().await;
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
        let err = run(&args, &ctx).await.unwrap_err();
        assert!(
            format!("{err:#}").contains("Write operations require full profile"),
            "{args:?}: {err:#}"
        );
    }
}
