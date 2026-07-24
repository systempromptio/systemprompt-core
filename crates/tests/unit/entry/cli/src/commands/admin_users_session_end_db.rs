//! App-context tests driving `admin users session end` against a real database.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::admin::users::{self, UsersCommands};
use systemprompt_cli::session::api::{DEFAULT_CLI_SESSION_HOURS, create_local_session_row};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_database::DbPool;
use systemprompt_test_fixtures::{fixture_app_context, fixture_database_url, fixture_db_pool};
use systemprompt_users::UserService;
use uuid::Uuid;

#[derive(Debug, Parser)]
struct UsersHarness {
    #[command(subcommand)]
    cmd: UsersCommands,
}

fn parse(args: &[&str]) -> UsersCommands {
    UsersHarness::try_parse_from(std::iter::once("users").chain(args.iter().copied()))
        .unwrap()
        .cmd
}

async fn pool() -> DbPool {
    fixture_db_pool(&fixture_database_url().unwrap())
        .await
        .unwrap()
}

fn ctx(pool: &DbPool) -> CommandContext {
    let url = fixture_database_url().unwrap();
    CommandContext::with_app_context(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
        fixture_app_context(pool, &url).unwrap(),
    )
}

fn unique(prefix: &str) -> (String, String) {
    let tag = Uuid::new_v4().simple().to_string();
    (
        format!("{prefix}-{tag}"),
        format!("{prefix}-{tag}@sess.invalid"),
    )
}

#[tokio::test]
async fn end_specific_session_succeeds() {
    let pool = pool().await;
    let service = UserService::new(&pool).unwrap();
    let (n, e) = unique("sesend");
    let user = service.create(&n, &e, None, None).await.unwrap();
    let session_id = create_local_session_row(
        &pool,
        &user.id,
        chrono::Duration::hours(DEFAULT_CLI_SESSION_HOURS),
    )
    .await
    .unwrap();

    let ctx = ctx(&pool);
    users::execute(
        parse(&["session", "end", session_id.as_str(), "--yes"]),
        &ctx,
    )
    .await
    .unwrap();

    let _ = service.delete(&user.id).await;
}

#[tokio::test]
async fn end_unknown_session_reports_not_found() {
    let pool = pool().await;
    let ctx = ctx(&pool);
    let missing = format!("no-sess-{}", Uuid::new_v4().simple());
    users::execute(parse(&["session", "end", &missing, "--yes"]), &ctx)
        .await
        .unwrap();
}

#[tokio::test]
async fn end_all_sessions_for_user() {
    let pool = pool().await;
    let service = UserService::new(&pool).unwrap();
    let (n, e) = unique("sesall");
    let user = service.create(&n, &e, None, None).await.unwrap();
    create_local_session_row(
        &pool,
        &user.id,
        chrono::Duration::hours(DEFAULT_CLI_SESSION_HOURS),
    )
    .await
    .unwrap();
    create_local_session_row(
        &pool,
        &user.id,
        chrono::Duration::hours(DEFAULT_CLI_SESSION_HOURS),
    )
    .await
    .unwrap();

    let ctx = ctx(&pool);
    users::execute(
        parse(&[
            "session",
            "end",
            "--user",
            user.id.as_str(),
            "--all",
            "--yes",
        ]),
        &ctx,
    )
    .await
    .unwrap();

    let _ = service.delete(&user.id).await;
}

#[tokio::test]
async fn end_without_confirmation_errors() {
    let pool = pool().await;
    let ctx = ctx(&pool);
    let err = users::execute(parse(&["session", "end", "whatever"]), &ctx)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("--yes to confirm"), "{err}");
}

#[tokio::test]
async fn end_all_without_user_errors() {
    let pool = pool().await;
    let ctx = ctx(&pool);
    let err = users::execute(parse(&["session", "end", "--all", "--yes"]), &ctx)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("--user is required"), "{err}");
}

#[tokio::test]
async fn end_all_missing_user_errors() {
    let pool = pool().await;
    let ctx = ctx(&pool);
    let missing = format!("no-user-{}", Uuid::new_v4().simple());
    let err = users::execute(
        parse(&["session", "end", "--user", &missing, "--all", "--yes"]),
        &ctx,
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("User not found"), "{err}");
}

#[tokio::test]
async fn end_without_session_id_errors() {
    let pool = pool().await;
    let ctx = ctx(&pool);
    let err = users::execute(parse(&["session", "end", "--yes"]), &ctx)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("Session ID is required"), "{err}");
}
