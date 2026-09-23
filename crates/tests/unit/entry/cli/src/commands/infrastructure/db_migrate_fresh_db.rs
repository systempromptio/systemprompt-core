//! End-to-end `infra db` migration tests against a disposable, freshly-created
//! database. Creating and dropping a throwaway database lets the standalone
//! migration dispatch drive the destructive apply/mark/down paths without
//! mutating the shared measurement database.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::infrastructure::db::{self, DbCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_runtime::DatabaseContext;
use systemprompt_test_fixtures::DisposableDb;

#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    cmd: DbCommands,
}

fn parse(args: &[&str]) -> DbCommands {
    Harness::try_parse_from(std::iter::once("db").chain(args.iter().copied()))
        .unwrap()
        .cmd
}

struct Disposable {
    db: DisposableDb,
    url: String,
}

impl Disposable {
    async fn create() -> Self {
        let db = DisposableDb::create("cov_cli_mig").await.unwrap();
        let url = db.url().to_owned();
        Self { db, url }
    }

    async fn untrack_logging_v3(&self) {
        let pool = sqlx::PgPool::connect(&self.url).await.unwrap();
        sqlx::query(
            "DELETE FROM extension_migrations WHERE extension_id = 'logging' AND version = 3",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool.close().await;
    }

    async fn ctx(&self, json: bool) -> CommandContext {
        let mut cli = CliConfig::new().with_interactive(false);
        if json {
            cli = cli.with_output_format(OutputFormat::Json);
        }
        let db_ctx = DatabaseContext::from_url(&self.url).await.unwrap();
        CommandContext::with_database(cli, EnvOverrides::default(), db_ctx, self.url.clone())
    }

    async fn drop(self) {
        self.db.drop_now().await;
    }
}

#[tokio::test]
async fn fresh_database_migrates_and_reports_status() {
    let disp = Disposable::create().await;

    db::execute(parse(&["migrate"]), &disp.ctx(false).await)
        .await
        .unwrap();

    db::execute(parse(&["migrate-status"]), &disp.ctx(false).await)
        .await
        .unwrap();
    db::execute(parse(&["migrate-status", "--json"]), &disp.ctx(true).await)
        .await
        .unwrap();
    db::execute(parse(&["migrate-plan"]), &disp.ctx(false).await)
        .await
        .unwrap();
    db::execute(parse(&["migrate-repair"]), &disp.ctx(false).await)
        .await
        .unwrap();
    db::execute(
        parse(&["migrate-repair", "--apply", "--json"]),
        &disp.ctx(true).await,
    )
    .await
    .unwrap();

    disp.drop().await;
}

#[tokio::test]
async fn migrations_status_reports_pending_and_history_lists_applied() {
    let disp = Disposable::create().await;

    db::execute(parse(&["migrate"]), &disp.ctx(false).await)
        .await
        .unwrap();

    db::execute(
        parse(&["migrations", "history", "logging"]),
        &disp.ctx(false).await,
    )
    .await
    .unwrap();
    db::execute(
        parse(&["migrations", "history", "logging"]),
        &disp.ctx(true).await,
    )
    .await
    .unwrap();

    let missing = db::execute(
        parse(&["migrations", "history", "no_such_extension"]),
        &disp.ctx(false).await,
    )
    .await;
    assert!(
        missing.is_err(),
        "history for an unknown extension must error"
    );

    disp.untrack_logging_v3().await;
    db::execute(parse(&["migrations", "status"]), &disp.ctx(false).await)
        .await
        .unwrap();
    db::execute(parse(&["migrations", "status"]), &disp.ctx(true).await)
        .await
        .unwrap();

    disp.drop().await;
}

#[tokio::test]
async fn fresh_database_mark_applied_and_down() {
    let disp = Disposable::create().await;

    db::execute(parse(&["migrate"]), &disp.ctx(false).await)
        .await
        .unwrap();

    disp.untrack_logging_v3().await;
    db::execute(
        parse(&[
            "migrate-mark-applied",
            "--extension",
            "logging",
            "--version",
            "3",
        ]),
        &disp.ctx(false).await,
    )
    .await
    .unwrap();

    disp.untrack_logging_v3().await;
    db::execute(
        parse(&[
            "migrate-mark-applied",
            "--extension",
            "logging",
            "--version",
            "3",
            "--json",
        ]),
        &disp.ctx(true).await,
    )
    .await
    .unwrap();

    let remark = db::execute(
        parse(&[
            "migrate-mark-applied",
            "--extension",
            "logging",
            "--version",
            "3",
        ]),
        &disp.ctx(false).await,
    )
    .await;
    assert!(
        remark.is_err(),
        "re-marking an already-tracked migration must error"
    );

    let down = db::execute(
        parse(&["migrate-down", "logging", "1"]),
        &disp.ctx(false).await,
    )
    .await;
    assert!(
        down.is_err(),
        "logging migrations declare no down SQL, so revert must error"
    );

    disp.drop().await;
}
