//! App-context tests driving `admin users ban`, `bulk`, and `export` against
//! a real database.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::admin::users::{self, UsersCommands};
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
        format!("{prefix}-{tag}@bulk.invalid"),
    )
}

fn unique_ip() -> String {
    let bytes = Uuid::new_v4().into_bytes();
    format!(
        "10.{}.{}.{}",
        bytes[0].clamp(1, 250),
        bytes[1].clamp(1, 250),
        bytes[2].clamp(1, 250)
    )
}

#[tokio::test]
async fn ban_add_list_and_remove_round_trip() {
    let pool = pool().await;
    let ctx = ctx(&pool);
    let ip = unique_ip();

    users::execute(
        parse(&[
            "ban",
            "add",
            &ip,
            "--reason",
            "test ban",
            "--duration",
            "24h",
        ]),
        &ctx,
    )
    .await
    .unwrap();

    users::execute(parse(&["ban", "list", "--source", "cli"]), &ctx)
        .await
        .unwrap();

    users::execute(parse(&["ban", "remove", &ip, "--yes"]), &ctx)
        .await
        .unwrap();
}

#[tokio::test]
async fn ban_add_permanent_and_default_duration() {
    let pool = pool().await;
    let ctx = ctx(&pool);

    let ip = unique_ip();
    users::execute(
        parse(&["ban", "add", &ip, "--reason", "perm", "--permanent"]),
        &ctx,
    )
    .await
    .unwrap();
    users::execute(parse(&["ban", "remove", &ip, "--yes"]), &ctx)
        .await
        .unwrap();

    let ip2 = unique_ip();
    users::execute(parse(&["ban", "add", &ip2, "--reason", "default"]), &ctx)
        .await
        .unwrap();
    users::execute(parse(&["ban", "remove", &ip2, "--yes"]), &ctx)
        .await
        .unwrap();
}

#[tokio::test]
async fn ban_add_rejects_bad_duration() {
    let pool = pool().await;
    let ctx = ctx(&pool);
    let err = users::execute(
        parse(&[
            "ban",
            "add",
            &unique_ip(),
            "--reason",
            "x",
            "--duration",
            "soon",
        ]),
        &ctx,
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("Invalid duration format"), "{err}");
}

#[tokio::test]
async fn ban_remove_and_cleanup_require_confirmation() {
    let pool = pool().await;
    let ctx = ctx(&pool);

    let err = users::execute(parse(&["ban", "remove", "10.0.0.1"]), &ctx)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("--yes to confirm"), "{err}");

    let err = users::execute(parse(&["ban", "cleanup"]), &ctx)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("--yes to confirm"), "{err}");
}

#[tokio::test]
async fn ban_cleanup_runs_with_confirmation() {
    let pool = pool().await;
    let ctx = ctx(&pool);
    users::execute(parse(&["ban", "cleanup", "--yes"]), &ctx)
        .await
        .unwrap();
}

#[tokio::test]
async fn bulk_delete_requires_confirmation_and_filter() {
    let pool = pool().await;
    let ctx = ctx(&pool);

    let err = users::execute(parse(&["bulk", "delete", "--role", "x"]), &ctx)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("--yes to confirm"), "{err}");

    let err = users::execute(parse(&["bulk", "delete", "--yes"]), &ctx)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("At least one filter"), "{err}");
}

#[tokio::test]
async fn bulk_delete_dry_run_then_execute_scoped_by_role() {
    let pool = pool().await;
    let service = UserService::new(&pool).unwrap();
    let role = format!("covrole_{}", Uuid::new_v4().simple());
    let (n, e) = unique("bulkdel");
    let user = service.create(&n, &e, None, None).await.unwrap();
    service
        .assign_roles(&user.id, &[role.clone()])
        .await
        .unwrap();

    let ctx = ctx(&pool);
    users::execute(
        parse(&["bulk", "delete", "--role", &role, "--dry-run"]),
        &ctx,
    )
    .await
    .unwrap();
    assert!(service.find_by_id(&user.id).await.unwrap().is_some());

    users::execute(parse(&["bulk", "delete", "--role", &role, "--yes"]), &ctx)
        .await
        .unwrap();
    assert!(service.find_by_id(&user.id).await.unwrap().is_none());
}

#[tokio::test]
async fn bulk_delete_reports_empty_match() {
    let pool = pool().await;
    let ctx = ctx(&pool);
    let role = format!("norole_{}", Uuid::new_v4().simple());
    users::execute(parse(&["bulk", "delete", "--role", &role, "--yes"]), &ctx)
        .await
        .unwrap();
}

#[tokio::test]
async fn bulk_update_validates_status_and_applies_by_role() {
    let pool = pool().await;
    let service = UserService::new(&pool).unwrap();
    let role = format!("covupd_{}", Uuid::new_v4().simple());
    let (n, e) = unique("bulkupd");
    let user = service.create(&n, &e, None, None).await.unwrap();
    service
        .assign_roles(&user.id, &[role.clone()])
        .await
        .unwrap();

    let ctx = ctx(&pool);
    let err = users::execute(
        parse(&[
            "bulk",
            "update",
            "--set-status",
            "frozen",
            "--role",
            &role,
            "--yes",
        ]),
        &ctx,
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("Invalid status"), "{err}");

    users::execute(
        parse(&[
            "bulk",
            "update",
            "--set-status",
            "suspended",
            "--role",
            &role,
            "--dry-run",
        ]),
        &ctx,
    )
    .await
    .unwrap();

    users::execute(
        parse(&[
            "bulk",
            "update",
            "--set-status",
            "suspended",
            "--role",
            &role,
            "--yes",
        ]),
        &ctx,
    )
    .await
    .unwrap();

    let updated = service.find_by_id(&user.id).await.unwrap().unwrap();
    assert_eq!(updated.status.as_deref(), Some("suspended"));

    let _ = service.delete(&user.id).await;
}

#[tokio::test]
async fn export_writes_file_and_prints_without_path() {
    let pool = pool().await;
    let service = UserService::new(&pool).unwrap();
    let role = format!("covexp_{}", Uuid::new_v4().simple());
    let (n, e) = unique("export");
    let user = service.create(&n, &e, None, None).await.unwrap();
    service
        .assign_roles(&user.id, &[role.clone()])
        .await
        .unwrap();

    let ctx = ctx(&pool);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("users.json");
    users::execute(
        parse(&[
            "export",
            "--role",
            &role,
            "--output",
            path.to_str().unwrap(),
        ]),
        &ctx,
    )
    .await
    .unwrap();

    let json = std::fs::read_to_string(&path).unwrap();
    assert!(json.contains(user.id.as_str()), "{json}");

    users::execute(parse(&["export", "--role", &role]), &ctx)
        .await
        .unwrap();

    let _ = service.delete(&user.id).await;
}
