//! App-context tests driving `admin users session end` against a real database.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use std::sync::Arc;
use systemprompt_cli::session::api::DEFAULT_CLI_SESSION_HOURS;
use systemprompt_cli::{CommandContext, EnvOverrides};
use systemprompt_extension::cli::{CliConfig, OutputFormat};
use systemprompt_identity_entry::cli::users::{self, UsersCommands};
use systemprompt_test_fixtures::{test_app_context, test_database_url, test_db_pool};
use systemprompt_users::{UserRepository, UserService};
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

async fn ctx() -> CommandContext {
    let url = test_database_url();
    crate::boot_ctx::with_app_boot(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
        test_app_context(&url).await,
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
    let pool = test_db_pool().await;
    let service = UserService::new(Arc::new(UserRepository::new(&pool)));
    let (n, e) = unique("sesend");
    let user = service.create(&n, &e, None, None).await.unwrap();
    let session_id = super::open_cli_session(
        &pool,
        &user.id,
        chrono::Duration::hours(DEFAULT_CLI_SESSION_HOURS),
    )
    .await
    .unwrap();

    let ctx = ctx().await;
    users::execute(
        parse(&["session", "end", session_id.as_str(), "--yes"]),
        &crate::boot_ctx::identity(&ctx),
    )
    .await
    .unwrap();

    let _ = service
        .delete(&user.id, &crate::boot_ctx::core_purge())
        .await;
}

#[tokio::test]
async fn end_unknown_session_reports_not_found() {
    let ctx = ctx().await;
    let missing = format!("no-sess-{}", Uuid::new_v4().simple());
    users::execute(
        parse(&["session", "end", &missing, "--yes"]),
        &crate::boot_ctx::identity(&ctx),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn end_all_sessions_for_user() {
    let pool = test_db_pool().await;
    let service = UserService::new(Arc::new(UserRepository::new(&pool)));
    let (n, e) = unique("sesall");
    let user = service.create(&n, &e, None, None).await.unwrap();
    super::open_cli_session(
        &pool,
        &user.id,
        chrono::Duration::hours(DEFAULT_CLI_SESSION_HOURS),
    )
    .await
    .unwrap();
    super::open_cli_session(
        &pool,
        &user.id,
        chrono::Duration::hours(DEFAULT_CLI_SESSION_HOURS),
    )
    .await
    .unwrap();

    let ctx = ctx().await;
    users::execute(
        parse(&[
            "session",
            "end",
            "--user",
            user.id.as_str(),
            "--all",
            "--yes",
        ]),
        &crate::boot_ctx::identity(&ctx),
    )
    .await
    .unwrap();

    let _ = service
        .delete(&user.id, &crate::boot_ctx::core_purge())
        .await;
}

#[tokio::test]
async fn end_without_confirmation_errors() {
    let ctx = ctx().await;
    let err = users::execute(
        parse(&["session", "end", "whatever"]),
        &crate::boot_ctx::identity(&ctx),
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("--yes to confirm"), "{err}");
}

#[tokio::test]
async fn end_all_without_user_errors() {
    let ctx = ctx().await;
    let err = users::execute(
        parse(&["session", "end", "--all", "--yes"]),
        &crate::boot_ctx::identity(&ctx),
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("--user is required"), "{err}");
}

#[tokio::test]
async fn end_all_missing_user_errors() {
    let ctx = ctx().await;
    let missing = format!("no-user-{}", Uuid::new_v4().simple());
    let err = users::execute(
        parse(&["session", "end", "--user", &missing, "--all", "--yes"]),
        &crate::boot_ctx::identity(&ctx),
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("User not found"), "{err}");
}

#[tokio::test]
async fn end_without_session_id_errors() {
    let ctx = ctx().await;
    let err = users::execute(
        parse(&["session", "end", "--yes"]),
        &crate::boot_ctx::identity(&ctx),
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("Session ID is required"), "{err}");
}
