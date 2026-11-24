//! App-context tests driving `admin users ban`, `bulk`, and `export` against
//! a real database.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use std::sync::Arc;
use systemprompt_cli::{CommandContext, EnvOverrides};
use systemprompt_extension::cli::{CliConfig, OutputFormat};
use systemprompt_identity_entry::cli::users::{self, UsersCommands};
use systemprompt_test_fixtures::{test_app_context, test_database_url, test_db_pool};
use systemprompt_users::{UserRepository, UserService, UserStatus};
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
    let ctx = ctx().await;
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
        &crate::boot_ctx::identity(&ctx),
    )
    .await
    .unwrap();

    users::execute(
        parse(&["ban", "list", "--source", "cli"]),
        &crate::boot_ctx::identity(&ctx),
    )
    .await
    .unwrap();

    users::execute(
        parse(&["ban", "remove", &ip, "--yes"]),
        &crate::boot_ctx::identity(&ctx),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn ban_add_permanent_and_default_duration() {
    let ctx = ctx().await;

    let ip = unique_ip();
    users::execute(
        parse(&["ban", "add", &ip, "--reason", "perm", "--permanent"]),
        &crate::boot_ctx::identity(&ctx),
    )
    .await
    .unwrap();
    users::execute(
        parse(&["ban", "remove", &ip, "--yes"]),
        &crate::boot_ctx::identity(&ctx),
    )
    .await
    .unwrap();

    let ip2 = unique_ip();
    users::execute(
        parse(&["ban", "add", &ip2, "--reason", "default"]),
        &crate::boot_ctx::identity(&ctx),
    )
    .await
    .unwrap();
    users::execute(
        parse(&["ban", "remove", &ip2, "--yes"]),
        &crate::boot_ctx::identity(&ctx),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn ban_add_rejects_bad_duration() {
    let ctx = ctx().await;
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
        &crate::boot_ctx::identity(&ctx),
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("Invalid duration format"), "{err}");
}

#[tokio::test]
async fn ban_remove_and_cleanup_require_confirmation() {
    let ctx = ctx().await;

    let err = users::execute(
        parse(&["ban", "remove", "10.0.0.1"]),
        &crate::boot_ctx::identity(&ctx),
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("--yes to confirm"), "{err}");

    let err = users::execute(parse(&["ban", "cleanup"]), &crate::boot_ctx::identity(&ctx))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("--yes to confirm"), "{err}");
}

#[tokio::test]
async fn ban_cleanup_runs_with_confirmation() {
    let ctx = ctx().await;
    users::execute(
        parse(&["ban", "cleanup", "--yes"]),
        &crate::boot_ctx::identity(&ctx),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn bulk_delete_requires_confirmation_and_filter() {
    let ctx = ctx().await;

    let err = users::execute(
        parse(&["bulk", "delete", "--role", "x"]),
        &crate::boot_ctx::identity(&ctx),
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("--yes to confirm"), "{err}");

    let err = users::execute(
        parse(&["bulk", "delete", "--yes"]),
        &crate::boot_ctx::identity(&ctx),
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("At least one filter"), "{err}");
}

#[tokio::test]
async fn bulk_delete_dry_run_then_execute_scoped_by_role() {
    let pool = test_db_pool().await;
    let service = UserService::new(Arc::new(UserRepository::new(&pool)));
    let role = format!("covrole_{}", Uuid::new_v4().simple());
    let (n, e) = unique("bulkdel");
    let user = service.create(&n, &e, None, None).await.unwrap();
    service
        .assign_roles(&user.id, &[role.clone()])
        .await
        .unwrap();

    let ctx = ctx().await;
    users::execute(
        parse(&["bulk", "delete", "--role", &role, "--dry-run"]),
        &crate::boot_ctx::identity(&ctx),
    )
    .await
    .unwrap();
    assert!(service.find_by_id(&user.id).await.unwrap().is_some());

    users::execute(
        parse(&["bulk", "delete", "--role", &role, "--yes"]),
        &crate::boot_ctx::identity(&ctx),
    )
    .await
    .unwrap();
    assert!(service.find_by_id(&user.id).await.unwrap().is_none());
}

#[tokio::test]
async fn bulk_delete_reports_empty_match() {
    let ctx = ctx().await;
    let role = format!("norole_{}", Uuid::new_v4().simple());
    users::execute(
        parse(&["bulk", "delete", "--role", &role, "--yes"]),
        &crate::boot_ctx::identity(&ctx),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn bulk_update_validates_status_and_applies_by_role() {
    let pool = test_db_pool().await;
    let service = UserService::new(Arc::new(UserRepository::new(&pool)));
    let role = format!("covupd_{}", Uuid::new_v4().simple());
    let (n, e) = unique("bulkupd");
    let user = service.create(&n, &e, None, None).await.unwrap();
    service
        .assign_roles(&user.id, &[role.clone()])
        .await
        .unwrap();

    let ctx = ctx().await;
    assert!(
        UsersHarness::try_parse_from([
            "users",
            "bulk",
            "update",
            "--set-status",
            "frozen",
            "--role",
            &role,
            "--yes",
        ])
        .is_err(),
        "an unknown status is a usage error"
    );
    let err = users::execute(
        parse(&[
            "bulk",
            "update",
            "--set-status",
            "deleted",
            "--role",
            &role,
            "--yes",
        ]),
        &crate::boot_ctx::identity(&ctx),
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
        &crate::boot_ctx::identity(&ctx),
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
        &crate::boot_ctx::identity(&ctx),
    )
    .await
    .unwrap();

    let updated = service.find_by_id(&user.id).await.unwrap().unwrap();
    assert_eq!(updated.status, UserStatus::Suspended);

    let _ = service
        .delete(&user.id, &crate::boot_ctx::core_purge())
        .await;
}

#[tokio::test]
async fn export_writes_file_and_prints_without_path() {
    let pool = test_db_pool().await;
    let service = UserService::new(Arc::new(UserRepository::new(&pool)));
    let role = format!("covexp_{}", Uuid::new_v4().simple());
    let (n, e) = unique("export");
    let user = service.create(&n, &e, None, None).await.unwrap();
    service
        .assign_roles(&user.id, &[role.clone()])
        .await
        .unwrap();

    let ctx = ctx().await;
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
        &crate::boot_ctx::identity(&ctx),
    )
    .await
    .unwrap();

    let json = std::fs::read_to_string(&path).unwrap();
    assert!(json.contains(user.id.as_str()), "{json}");

    users::execute(
        parse(&["export", "--role", &role]),
        &crate::boot_ctx::identity(&ctx),
    )
    .await
    .unwrap();

    let _ = service
        .delete(&user.id, &crate::boot_ctx::core_purge())
        .await;
}
