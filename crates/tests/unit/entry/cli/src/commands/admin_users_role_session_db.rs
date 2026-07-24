//! App-context tests driving `admin users role`, `delete`, `count`, and
//! `session list`/`cleanup` against a real database.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::admin::users::{self, UsersCommands};
use systemprompt_cli::session::api::{DEFAULT_CLI_SESSION_HOURS, create_local_session_row};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_database::DbPool;
use systemprompt_runtime::DatabaseContext;
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

fn db_scoped_ctx(pool: &DbPool) -> CommandContext {
    CommandContext::with_database(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
        DatabaseContext::from_pool(pool.clone()),
        fixture_database_url().unwrap(),
    )
}

fn unique(prefix: &str) -> (String, String) {
    let tag = Uuid::new_v4().simple().to_string();
    (
        format!("{prefix}-{tag}"),
        format!("{prefix}-{tag}@role.invalid"),
    )
}

#[tokio::test]
async fn role_assign_replaces_roles() {
    let pool = pool().await;
    let service = UserService::new(&pool).unwrap();
    let (n, e) = unique("assign");
    let user = service.create(&n, &e, None, None).await.unwrap();

    let ctx = ctx(&pool);
    users::execute(
        parse(&["role", "assign", user.id.as_str(), "--roles", "user,admin"]),
        &ctx,
    )
    .await
    .unwrap();

    let updated = service.find_by_id(&user.id).await.unwrap().unwrap();
    assert!(
        updated.roles.iter().any(|r| r == "admin"),
        "{:?}",
        updated.roles
    );

    let _ = service.delete(&user.id).await;
}

#[tokio::test]
async fn role_assign_unknown_user_errors() {
    let pool = pool().await;
    let ctx = ctx(&pool);
    let missing = format!("no-user-{}", Uuid::new_v4().simple());
    let err = users::execute(
        parse(&["role", "assign", &missing, "--roles", "admin"]),
        &ctx,
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("User not found"), "{err}");
}

#[tokio::test]
async fn role_promote_then_repeat_reports_already_admin() {
    let pool = pool().await;
    let service = UserService::new(&pool).unwrap();
    let (n, e) = unique("promote");
    let user = service.create(&n, &e, None, None).await.unwrap();

    let ctx = ctx(&pool);
    users::execute(parse(&["role", "promote", user.id.as_str()]), &ctx)
        .await
        .unwrap();

    let promoted = service.find_by_id(&user.id).await.unwrap().unwrap();
    assert!(
        promoted.roles.iter().any(|r| r == "admin"),
        "{:?}",
        promoted.roles
    );

    users::execute(parse(&["role", "promote", user.id.as_str()]), &ctx)
        .await
        .unwrap();

    let _ = service.delete(&user.id).await;
}

#[tokio::test]
async fn role_promote_unknown_user_errors() {
    let pool = pool().await;
    let ctx = ctx(&pool);
    let missing = format!("no-user-{}", Uuid::new_v4().simple());
    let err = users::execute(parse(&["role", "promote", &missing]), &ctx)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("User not found"), "{err}");
}

#[tokio::test]
async fn role_demote_admin_and_non_admin_paths() {
    let pool = pool().await;
    let service = UserService::new(&pool).unwrap();
    let (n, e) = unique("demote");
    let user = service.create(&n, &e, None, None).await.unwrap();

    let ctx = ctx(&pool);
    users::execute(parse(&["role", "promote", user.id.as_str()]), &ctx)
        .await
        .unwrap();
    users::execute(parse(&["role", "demote", user.id.as_str()]), &ctx)
        .await
        .unwrap();

    let demoted = service.find_by_id(&user.id).await.unwrap().unwrap();
    assert!(
        !demoted.roles.iter().any(|r| r == "admin"),
        "{:?}",
        demoted.roles
    );

    users::execute(parse(&["role", "demote", user.id.as_str()]), &ctx)
        .await
        .unwrap();

    let _ = service.delete(&user.id).await;
}

#[tokio::test]
async fn role_demote_unknown_user_errors() {
    let pool = pool().await;
    let ctx = ctx(&pool);
    let missing = format!("no-user-{}", Uuid::new_v4().simple());
    let err = users::execute(parse(&["role", "demote", &missing]), &ctx)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("User not found"), "{err}");
}

#[tokio::test]
async fn role_commands_require_full_profile_context() {
    let pool = pool().await;
    let ctx = db_scoped_ctx(&pool);
    let err = users::execute(parse(&["role", "promote", "someone"]), &ctx)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("full profile context"), "{err}");
}

#[tokio::test]
async fn delete_requires_confirmation() {
    let pool = pool().await;
    let ctx = ctx(&pool);
    let err = users::execute(parse(&["delete", "whoever"]), &ctx)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("--yes to confirm"), "{err}");
}

#[tokio::test]
async fn delete_removes_user() {
    let pool = pool().await;
    let service = UserService::new(&pool).unwrap();
    let (n, e) = unique("del");
    let user = service.create(&n, &e, None, None).await.unwrap();

    let ctx = ctx(&pool);
    users::execute(parse(&["delete", user.id.as_str(), "--yes"]), &ctx)
        .await
        .unwrap();

    assert!(service.find_by_id(&user.id).await.unwrap().is_none());
}

#[tokio::test]
async fn delete_unknown_user_errors() {
    let pool = pool().await;
    let ctx = ctx(&pool);
    let missing = format!("no-user-{}", Uuid::new_v4().simple());
    let err = users::execute(parse(&["delete", &missing, "--yes"]), &ctx)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("User not found"), "{err}");
}

#[tokio::test]
async fn count_breakdown_reports_totals() {
    let pool = pool().await;
    let service = UserService::new(&pool).unwrap();
    let (n, e) = unique("count");
    let user = service.create(&n, &e, None, None).await.unwrap();

    let ctx = ctx(&pool);
    users::execute(parse(&["count", "--breakdown"]), &ctx)
        .await
        .unwrap();
    users::execute(parse(&["count"]), &ctx).await.unwrap();

    let _ = service.delete(&user.id).await;
}

#[tokio::test]
async fn session_list_recent_and_active_for_seeded_user() {
    let pool = pool().await;
    let service = UserService::new(&pool).unwrap();
    let (n, e) = unique("seslist");
    let user = service.create(&n, &e, None, None).await.unwrap();
    create_local_session_row(
        &pool,
        &user.id,
        chrono::Duration::hours(DEFAULT_CLI_SESSION_HOURS),
    )
    .await
    .unwrap();

    let ctx = ctx(&pool);
    users::execute(parse(&["session", "list", user.id.as_str()]), &ctx)
        .await
        .unwrap();
    users::execute(
        parse(&["session", "list", user.id.as_str(), "--active"]),
        &ctx,
    )
    .await
    .unwrap();

    let _ = service.delete(&user.id).await;
}

#[tokio::test]
async fn session_list_unknown_user_errors() {
    let pool = pool().await;
    let ctx = ctx(&pool);
    let missing = format!("no-user-{}", Uuid::new_v4().simple());
    let err = users::execute(parse(&["session", "list", &missing]), &ctx)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("User not found"), "{err}");
}

#[tokio::test]
async fn session_cleanup_requires_confirmation() {
    let pool = pool().await;
    let ctx = ctx(&pool);
    let err = users::execute(parse(&["session", "cleanup"]), &ctx)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("--yes to confirm"), "{err}");
}

#[tokio::test]
async fn session_cleanup_deletes_old_anonymous_users() {
    let pool = pool().await;
    let service = UserService::new(&pool).unwrap();
    let fingerprint = format!("fp-{}", Uuid::new_v4().simple());
    let anon = service.create_anonymous(&fingerprint).await.unwrap();
    sqlx::query("UPDATE users SET created_at = NOW() - INTERVAL '90 days' WHERE id = $1")
        .bind(anon.id.as_str())
        .execute(pool.pool_arc().unwrap().as_ref())
        .await
        .unwrap();

    let ctx = ctx(&pool);
    users::execute(
        parse(&["session", "cleanup", "--days", "30", "--yes"]),
        &ctx,
    )
    .await
    .unwrap();

    assert!(service.find_by_id(&anon.id).await.unwrap().is_none());
}

#[tokio::test]
async fn session_cleanup_requires_full_profile_context() {
    let pool = pool().await;
    let ctx = db_scoped_ctx(&pool);
    let err = users::execute(parse(&["session", "cleanup", "--yes"]), &ctx)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("full profile context"), "{err}");
}
