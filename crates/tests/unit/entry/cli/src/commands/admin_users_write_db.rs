//! App-context tests driving the `admin users update` and `admin users merge`
//! write commands against a real database.

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
        format!("{prefix}-{tag}@write.invalid"),
    )
}

#[tokio::test]
async fn update_applies_every_field() {
    let pool = pool().await;
    let service = UserService::new(&pool).unwrap();
    let (name, email) = unique("upd");
    let user = service
        .create(&name, &email, Some("Old Full"), Some("Old Disp"))
        .await
        .unwrap();

    let (_new_name, new_email) = unique("upd2");
    let ctx = ctx(&pool);
    users::execute(
        parse(&[
            "update",
            user.id.as_str(),
            "--email",
            &new_email,
            "--full-name",
            "New Full",
            "--display-name",
            "New Disp",
            "--status",
            "suspended",
            "--email-verified",
            "true",
        ]),
        &ctx,
    )
    .await
    .unwrap();

    let refreshed = service
        .find_by_id(&user.id)
        .await
        .unwrap()
        .expect("user present");
    assert_eq!(refreshed.email, new_email);
    assert_eq!(refreshed.full_name.as_deref(), Some("New Full"));
    assert_eq!(refreshed.display_name.as_deref(), Some("New Disp"));

    let _ = service.delete(&user.id).await;
}

#[tokio::test]
async fn update_without_fields_errors() {
    let pool = pool().await;
    let service = UserService::new(&pool).unwrap();
    let (name, email) = unique("updn");
    let user = service.create(&name, &email, None, None).await.unwrap();

    let ctx = ctx(&pool);
    let err = users::execute(parse(&["update", user.id.as_str()]), &ctx)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("No fields to update"), "{err}");

    let _ = service.delete(&user.id).await;
}

#[tokio::test]
async fn update_missing_user_errors() {
    let pool = pool().await;
    let ctx = ctx(&pool);
    let missing = format!("no-such-{}", Uuid::new_v4().simple());
    let err = users::execute(parse(&["update", &missing, "--full-name", "x"]), &ctx)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("User not found"), "{err}");
}

#[tokio::test]
async fn merge_transfers_and_deletes_source() {
    let pool = pool().await;
    let service = UserService::new(&pool).unwrap();
    let (sn, se) = unique("mrgs");
    let (tn, te) = unique("mrgt");
    let source = service.create(&sn, &se, None, None).await.unwrap();
    let target = service.create(&tn, &te, None, None).await.unwrap();

    let ctx = ctx(&pool);
    users::execute(
        parse(&[
            "merge",
            "--source",
            source.id.as_str(),
            "--target",
            target.id.as_str(),
            "--yes",
        ]),
        &ctx,
    )
    .await
    .unwrap();

    assert!(
        service.find_by_id(&source.id).await.unwrap().is_none(),
        "source should be deleted after merge"
    );
    assert!(service.find_by_id(&target.id).await.unwrap().is_some());

    let _ = service.delete(&target.id).await;
}

#[tokio::test]
async fn merge_requires_confirmation() {
    let pool = pool().await;
    let ctx = ctx(&pool);
    let err = users::execute(parse(&["merge", "--source", "a", "--target", "b"]), &ctx)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("--yes to confirm"), "{err}");
}

#[tokio::test]
async fn merge_same_user_errors() {
    let pool = pool().await;
    let service = UserService::new(&pool).unwrap();
    let (n, e) = unique("mrgsame");
    let user = service.create(&n, &e, None, None).await.unwrap();

    let ctx = ctx(&pool);
    let err = users::execute(
        parse(&[
            "merge",
            "--source",
            user.id.as_str(),
            "--target",
            user.id.as_str(),
            "--yes",
        ]),
        &ctx,
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("cannot be the same"), "{err}");

    let _ = service.delete(&user.id).await;
}

#[tokio::test]
async fn merge_missing_users_errors() {
    let pool = pool().await;
    let service = UserService::new(&pool).unwrap();
    let (n, e) = unique("mrgok");
    let target = service.create(&n, &e, None, None).await.unwrap();

    let ctx = ctx(&pool);
    let missing = format!("no-src-{}", Uuid::new_v4().simple());
    let err = users::execute(
        parse(&[
            "merge",
            "--source",
            &missing,
            "--target",
            target.id.as_str(),
            "--yes",
        ]),
        &ctx,
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("Source user not found"), "{err}");

    let _ = service.delete(&target.id).await;
}
