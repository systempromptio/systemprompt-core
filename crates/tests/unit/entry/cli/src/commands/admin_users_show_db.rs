//! App-context tests for `admin users show` detail, sessions, and activity
//! projections against seeded users.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::admin::users::{self, UsersCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_database::DbPool;
use systemprompt_identifiers::SessionId;
use systemprompt_test_fixtures::{
    fixture_app_context, fixture_database_url, fixture_db_pool, seed_user_session,
};
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

fn ctx(pool: &DbPool, json: bool) -> CommandContext {
    let url = fixture_database_url().unwrap();
    let mut cli = CliConfig::new().with_interactive(false);
    if json {
        cli = cli.with_output_format(OutputFormat::Json);
    }
    CommandContext::with_app_context(
        cli,
        EnvOverrides::default(),
        fixture_app_context(pool, &url).unwrap(),
    )
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
    let pool = pool().await;
    let service = UserService::new(&pool).unwrap();
    let (name, email) = unique("show");
    let user = service
        .create(&name, &email, Some("Full Name"), Some("Disp"))
        .await
        .unwrap();
    seed_user_session(&pool, &user.id, &SessionId::generate())
        .await
        .unwrap();

    users::execute(
        parse(&["show", &email, "--sessions", "--activity"]),
        &ctx(&pool, true),
    )
    .await
    .unwrap();

    users::execute(parse(&["show", &email]), &ctx(&pool, false))
        .await
        .unwrap();
}

#[tokio::test]
async fn show_rejects_unknown_identifier() {
    let pool = pool().await;
    let err = users::execute(
        parse(&["show", "no-such-user@show.invalid"]),
        &ctx(&pool, false),
    )
    .await
    .expect_err("unknown user");
    assert!(err.to_string().contains("User not found"), "{err}");
}
