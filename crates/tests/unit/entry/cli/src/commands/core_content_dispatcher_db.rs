//! DB-backed tests for the `core content` dispatcher.
//!
//! The individual command bodies have their own tests against
//! `execute_with_pool`; the dispatcher, its render arms, and the
//! database-scope guard are only entered through `core::execute`.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::core::{self, CoreCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_content::models::CreateContentParams;
use systemprompt_content::{Content, ContentRepository};
use systemprompt_database::DbPool;
use systemprompt_identifiers::SourceId;
use systemprompt_runtime::DatabaseContext;
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    cmd: CoreCommands,
}

fn parse(args: &[&str]) -> CoreCommands {
    Harness::try_parse_from(std::iter::once("core").chain(args.iter().copied()))
        .unwrap()
        .cmd
}

async fn pool() -> DbPool {
    fixture_db_pool(&fixture_database_url().unwrap())
        .await
        .unwrap()
}

fn ctx(pool: &DbPool) -> CommandContext {
    CommandContext::with_database(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
        DatabaseContext::from_pool(pool.clone()),
        fixture_database_url().unwrap(),
    )
}

async fn seed(pool: &DbPool) -> (Content, String) {
    let source = format!("covsrc{}", uuid::Uuid::new_v4().simple());
    let slug = format!("covslug{}", uuid::Uuid::new_v4().simple());
    let repo = ContentRepository::new(pool).unwrap();
    let params = CreateContentParams::new(
        slug.clone(),
        format!("Title for {slug}"),
        "A description".to_owned(),
        "The body".to_owned(),
        SourceId::new(source.clone()),
    )
    .with_keywords("alpha,beta".to_owned())
    .with_version_hash("hash-1".to_owned());
    let content = repo.create(&params).await.unwrap();
    (content, source)
}

#[tokio::test]
async fn list_and_show_arms_render_seeded_content() {
    let pool = pool().await;
    let (content, source) = seed(&pool).await;
    let ctx = ctx(&pool);

    core::execute(parse(&["content", "list", "--source", &source]), &ctx)
        .await
        .unwrap();
    core::execute(parse(&["content", "show", content.id.as_str()]), &ctx)
        .await
        .unwrap();
}

#[tokio::test]
async fn show_arm_reports_an_unknown_identifier() {
    let pool = pool().await;
    let ctx = ctx(&pool);

    let err = core::execute(parse(&["content", "show", "cov_absent_content"]), &ctx)
        .await
        .unwrap_err();
    assert!(format!("{err:#}").contains("Failed to show content"));
}

#[tokio::test]
async fn search_status_and_popular_arms_run_against_a_seeded_source() {
    let pool = pool().await;
    let (_content, source) = seed(&pool).await;
    let ctx = ctx(&pool);

    core::execute(parse(&["content", "search", "alpha"]), &ctx)
        .await
        .unwrap();
    core::execute(parse(&["content", "status", "--source", &source]), &ctx)
        .await
        .unwrap();
    core::execute(parse(&["content", "popular", &source]), &ctx)
        .await
        .unwrap();
}

#[tokio::test]
async fn profile_only_content_commands_are_refused_under_a_database_scope() {
    let pool = pool().await;
    let (content, source) = seed(&pool).await;
    let ctx = ctx(&pool);

    for args in [
        vec!["content", "edit", content.id.as_str()],
        vec!["content", "delete", content.id.as_str()],
        vec!["content", "delete-source", &source, "--yes"],
        vec!["content", "verify", content.id.as_str()],
    ] {
        let err = core::execute(parse(&args), &ctx).await.unwrap_err();
        assert!(
            format!("{err:#}").contains("requires full profile context"),
            "{args:?}: {err:#}"
        );
    }
}

#[tokio::test]
async fn non_content_groups_are_refused_under_a_database_scope() {
    let pool = pool().await;
    let ctx = ctx(&pool);

    for args in [
        vec!["skills", "list"],
        vec!["hooks", "list"],
        vec!["contexts", "list"],
    ] {
        let err = core::execute(parse(&args), &ctx).await.unwrap_err();
        assert!(!format!("{err:#}").is_empty(), "{args:?}");
    }
}
