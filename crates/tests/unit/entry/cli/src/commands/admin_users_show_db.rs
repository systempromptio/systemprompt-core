//! App-context tests for `admin users show` detail, sessions, and activity
//! projections against seeded users.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use std::sync::Arc;
use systemprompt_cli::{CommandContext, EnvOverrides};
use systemprompt_extension::cli::{CliConfig, OutputFormat};
use systemprompt_identifiers::SessionId;
use systemprompt_identity_entry::cli::users::UsersCommands;
use systemprompt_test_fixtures::{
    seed_user_session, test_app_context, test_database_url, test_db_pool,
};
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

async fn ctx(json: bool) -> CommandContext {
    let url = test_database_url();
    let mut cli = CliConfig::new().with_interactive(false);
    if json {
        cli = cli.with_output_format(OutputFormat::Json);
    }
    crate::boot_ctx::with_app_boot(cli, EnvOverrides::default(), test_app_context(&url).await)
}

fn unique(prefix: &str) -> (String, String) {
    let tag = Uuid::new_v4().simple().to_string();
    (
        format!("{prefix}-{tag}"),
        format!("{prefix}-{tag}@show.invalid"),
    )
}

#[tokio::test]
async fn show_finds_user_by_email_with_sessions_and_activity() {
    let pool = test_db_pool().await;
    let service = UserService::new(Arc::new(UserRepository::new(&pool)));
    let (name, email) = unique("show");
    let user = service
        .create(&name, &email, Some("Full Name"), Some("Disp"))
        .await
        .unwrap();
    seed_user_session(&pool, &user.id, &SessionId::generate())
        .await
        .unwrap();

    crate::commands::identity_host::users(
        parse(&["show", &email, "--sessions", "--activity"]),
        &ctx(true).await,
    )
    .await
    .unwrap();

    crate::commands::identity_host::users(parse(&["show", &email]), &ctx(false).await)
        .await
        .unwrap();
}

#[tokio::test]
async fn show_rejects_unknown_identifier() {
    let err = crate::commands::identity_host::users(
        parse(&["show", "no-such-user@show.invalid"]),
        &ctx(false).await,
    )
    .await
    .expect_err("unknown user");
    assert!(err.to_string().contains("User not found"), "{err}");
}
