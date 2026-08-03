//! `infra jobs enable` / `disable` against a fixture pool: the registry lookup
//! that guards the write, and the persisted toggle itself.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::infrastructure::jobs::{self, JobsCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_database::DbPool;
use systemprompt_runtime::DatabaseContext;
use systemprompt_scheduler::JobRepository;
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};


#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    cmd: JobsCommands,
}

fn parse(args: &[&str]) -> JobsCommands {
    Harness::try_parse_from(std::iter::once("jobs").chain(args.iter().copied()))
        .unwrap()
        .cmd
}

async fn pool() -> DbPool {
    fixture_db_pool(&fixture_database_url().unwrap())
        .await
        .unwrap()
}

fn ctx(pool: &DbPool, json: bool) -> CommandContext {
    let mut cli = CliConfig::new().with_interactive(false);
    if json {
        cli = cli.with_output_format(OutputFormat::Json);
    }
    CommandContext::with_database(
        cli,
        EnvOverrides::default(),
        DatabaseContext::from_pool(pool.clone()),
        fixture_database_url().unwrap(),
    )
}

#[tokio::test]
async fn toggling_an_unregistered_job_is_refused_before_any_write() {
    let pool = pool().await;
    let repo = JobRepository::new(&pool).unwrap();
    let before = repo.find_job("no_such_job_at_all").await.unwrap();
    assert!(before.is_none(), "the fixture job must not already exist");

    for verb in ["enable", "disable"] {
        let err = jobs::execute(parse(&[verb, "no_such_job_at_all"]), &ctx(&pool, true))
            .await
            .expect_err("an unregistered job name must be refused");
        let message = err.to_string();
        assert!(
            message.contains("no_such_job_at_all"),
            "`jobs {verb}` must name the job it rejected, got {message}"
        );
        assert!(
            message.contains("jobs list"),
            "`jobs {verb}` must point at the command that lists valid names, got {message}"
        );
    }

    assert!(
        repo.find_job("no_such_job_at_all").await.unwrap().is_none(),
        "a rejected toggle must not create a schedule row"
    );
}

#[tokio::test]
async fn jobs_list_reports_the_inventory_registered_jobs() {
    let pool = pool().await;

    jobs::execute(parse(&["list"]), &ctx(&pool, true))
        .await
        .expect("json listing");
    jobs::execute(parse(&["list"]), &ctx(&pool, false))
        .await
        .expect("text listing");
}
