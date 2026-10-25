//! Idempotent provisioning from the CLI: `api-key issue --if-absent`,
//! `--secret-file`, and `service-account create`.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::admin::users::{self, UsersCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_database::DbPool;
use systemprompt_identifiers::UserId;
use systemprompt_test_fixtures::{
    seed_user_row, test_app_context, test_database_url, test_db_pool,
};
use uuid::Uuid;

#[derive(Debug, Parser)]
struct UsersHarness {
    #[command(subcommand)]
    cmd: UsersCommands,
}

fn parse(args: &[&str]) -> UsersCommands {
    UsersHarness::try_parse_from(std::iter::once("users").chain(args.iter().copied()))
        .unwrap_or_else(|e| panic!("parse {args:?}: {e}"))
        .cmd
}

fn ctx(pool: &DbPool) -> CommandContext {
    let url = test_database_url();
    crate::boot_ctx::with_app_boot(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
        test_app_context(pool, &url),
    )
}

async fn run(pool: &DbPool, args: &[&str]) -> anyhow::Result<()> {
    users::execute(parse(args), &ctx(pool)).await
}

async fn seeded_user(pool: &DbPool) -> String {
    let id = format!("provision-user-{}", Uuid::new_v4().simple());
    seed_user_row(pool, &UserId::new(&id), &format!("{id}@provision.invalid"))
        .await
        .expect("seed user");
    id
}

async fn key_hashes(pool: &DbPool, user: &str) -> Vec<(String, String)> {
    sqlx::query_as::<_, (String, String)>(
        "SELECT name, key_hash FROM user_api_keys WHERE user_id = $1 AND revoked_at IS NULL",
    )
    .bind(user)
    .fetch_all(pool.pool().as_ref())
    .await
    .expect("read keys")
}

#[tokio::test]
async fn issuing_if_absent_twice_leaves_one_key() {
    let pool = test_db_pool().await;
    let user = seeded_user(&pool).await;
    let args = [
        "api-key",
        "issue",
        "--user",
        &user,
        "--name",
        "deploy",
        "--if-absent",
    ];

    run(&pool, &args).await.expect("first issue");
    run(&pool, &args).await.expect("second issue is a no-op");

    assert_eq!(key_hashes(&pool, &user).await.len(), 1);
}

#[tokio::test]
async fn without_if_absent_every_issue_mints_a_new_key() {
    let pool = test_db_pool().await;
    let user = seeded_user(&pool).await;
    let args = ["api-key", "issue", "--user", &user, "--name", "deploy"];

    run(&pool, &args).await.expect("first");
    run(&pool, &args).await.expect("second");

    assert_eq!(key_hashes(&pool, &user).await.len(), 2);
}

#[tokio::test]
async fn the_secret_file_holds_the_issued_secret_with_owner_only_mode() {
    use sha2::{Digest, Sha256};

    let pool = test_db_pool().await;
    let user = seeded_user(&pool).await;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pat");
    let path_str = path.to_str().unwrap().to_owned();

    run(
        &pool,
        &[
            "api-key",
            "issue",
            "--user",
            &user,
            "--name",
            "filed",
            "--secret-file",
            &path_str,
        ],
    )
    .await
    .expect("issue");

    let secret = std::fs::read_to_string(&path).unwrap();
    assert!(secret.starts_with("sp-live-"));
    let stored = key_hashes(&pool, &user).await;
    assert_eq!(stored.len(), 1);
    assert_eq!(
        stored[0].1,
        Sha256::digest(secret.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    );

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
}

#[test]
fn renew_before_requires_if_absent() {
    let parsed = UsersHarness::try_parse_from([
        "users",
        "api-key",
        "issue",
        "--user",
        "u",
        "--name",
        "n",
        "--renew-before-days",
        "5",
    ]);
    assert!(parsed.is_err());
}

#[tokio::test]
async fn creating_a_service_account_twice_yields_one_account() {
    let pool = test_db_pool().await;
    let creator = seeded_user(&pool).await;
    let email = format!("svc-{}@provision.invalid", Uuid::new_v4().simple());
    let args = [
        "service-account",
        "create",
        "--name",
        "ci-bot",
        "--email",
        &email,
        "--created-by",
        &creator,
    ];

    run(&pool, &args).await.expect("create");
    run(&pool, &args).await.expect("re-create is a no-op");

    let (count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM users WHERE email = $1 AND account_kind = 'service'")
            .bind(&email)
            .fetch_one(pool.pool().as_ref())
            .await
            .unwrap();
    assert_eq!(count, 1);
}
