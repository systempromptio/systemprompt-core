//! `admin users restore` and `admin users legal-hold` against an archived
//! account. An archive sets the row's status to deleted, which every listing
//! lookup skips, so these commands resolve the reference across every status.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use std::sync::Arc;
use systemprompt_cli::{CommandContext, EnvOverrides};
use systemprompt_database::DbPool;
use systemprompt_extension::cli::{CliConfig, OutputFormat};
use systemprompt_identity_entry::cli::users::UsersCommands;
use systemprompt_test_fixtures::{test_app_context, test_database_url, test_db_pool};
use systemprompt_users::{ArchiveParams, User, UserRepository, UserService};
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
    crate::boot_ctx::with_app_boot(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
        test_app_context(&test_database_url()).await,
    )
}

async fn archived_user(pool: &DbPool, prefix: &str) -> (UserService, User) {
    let service = UserService::new(Arc::new(UserRepository::new(pool)));
    let tag = Uuid::new_v4().simple().to_string();
    let user = service
        .create(
            &format!("{prefix}-{tag}"),
            &format!("{prefix}-{tag}@restore.invalid"),
            None,
            None,
        )
        .await
        .unwrap();
    service
        .archive(&user.id, ArchiveParams::default())
        .await
        .unwrap();
    assert!(
        service.find_by_id(&user.id).await.unwrap().is_none(),
        "an archived account leaves the ordinary lookups"
    );
    (service, user)
}

#[tokio::test]
async fn restore_brings_an_archived_account_back_by_id_and_by_email() {
    let pool = test_db_pool().await;
    let (service, user) = archived_user(&pool, "restore").await;

    crate::commands::identity_host::users(parse(&["restore", user.id.as_str()]), &ctx().await)
        .await
        .expect("an archived account restores by id");
    assert!(service.find_by_id(&user.id).await.unwrap().is_some());

    service
        .archive(&user.id, ArchiveParams::default())
        .await
        .unwrap();
    crate::commands::identity_host::users(parse(&["restore", user.email.as_str()]), &ctx().await)
        .await
        .expect("an archived account restores by email");
    assert!(service.find_by_id(&user.id).await.unwrap().is_some());
}

#[tokio::test]
async fn legal_hold_is_placed_and_released_on_an_archived_account() {
    let pool = test_db_pool().await;
    let (service, user) = archived_user(&pool, "hold").await;

    crate::commands::identity_host::users(parse(&["legal-hold", user.name.as_str()]), &ctx().await)
        .await
        .expect("a hold is placed by name");
    let held = service.find_archive_state(&user.id).await.unwrap().unwrap();
    assert!(held.legal_hold);
    assert!(
        service
            .purge(&user.id, &crate::boot_ctx::core_purge())
            .await
            .is_err(),
        "a held archive cannot be purged"
    );

    crate::commands::identity_host::users(
        parse(&["legal-hold", user.id.as_str(), "--release"]),
        &ctx().await,
    )
    .await
    .expect("the hold is released");
    let released = service.find_archive_state(&user.id).await.unwrap().unwrap();
    assert!(!released.legal_hold);
}

#[tokio::test]
async fn restore_of_an_unknown_reference_reports_it_not_found() {
    let reference = format!("nobody-{}@restore.invalid", Uuid::new_v4().simple());
    let err = crate::commands::identity_host::users(
        parse(&["restore", reference.as_str()]),
        &ctx().await,
    )
    .await
    .expect_err("no such user");
    assert!(err.to_string().contains("User not found"), "{err}");
}
