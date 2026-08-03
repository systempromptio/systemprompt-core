//! `infra db` migration paths that only run when the recorded migration state
//! diverges from the on-disk migrations: checksum drift reporting and repair,
//! the `migrate down` revert, and the verbose install renderer.
//!
//! Every test owns a disposable database so the destructive arms never touch
//! the shared measurement database.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::Parser;
use systemprompt_cli::infrastructure::db::{self, DbCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat, VerbosityLevel};
use systemprompt_runtime::DatabaseContext;
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

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
    admin: sqlx::PgPool,
    name: String,
    url: String,
}

impl Disposable {
    async fn create() -> Self {
        let base_url = fixture_database_url().unwrap();
        let admin = fixture_db_pool(&base_url)
            .await
            .unwrap()
            .pool_arc()
            .unwrap()
            .as_ref()
            .clone();

        let name = format!("cov_cli_drift_{}", uuid::Uuid::new_v4().simple());
        sqlx::query(sqlx::AssertSqlSafe(format!("CREATE DATABASE \"{name}\"")))
            .execute(&admin)
            .await
            .unwrap();

        let (prefix, _old) = base_url.rsplit_once('/').unwrap();
        let url = format!("{prefix}/{name}");
        Self { admin, name, url }
    }

    async fn conn(&self) -> sqlx::PgPool {
        sqlx::PgPool::connect(&self.url).await.unwrap()
    }

    async fn corrupt_checksum(&self, extension: &str, version: i32) {
        let pool = self.conn().await;
        let updated = sqlx::query(
            "UPDATE extension_migrations SET checksum = 'deadbeefdeadbeefdeadbeef' \
             WHERE extension_id = $1 AND version = $2",
        )
        .bind(extension)
        .bind(version)
        .execute(&pool)
        .await
        .unwrap()
        .rows_affected();
        pool.close().await;
        assert_eq!(
            updated, 1,
            "the drift fixture must alter exactly one recorded migration"
        );
    }

    async fn stored_checksum(&self, extension: &str, version: i32) -> String {
        let pool = self.conn().await;
        let checksum: String = sqlx::query_scalar(
            "SELECT checksum FROM extension_migrations WHERE extension_id = $1 AND version = $2",
        )
        .bind(extension)
        .bind(version)
        .fetch_one(&pool)
        .await
        .unwrap();
        pool.close().await;
        checksum
    }

    async fn applied_versions(&self, extension: &str) -> Vec<i32> {
        let pool = self.conn().await;
        let versions: Vec<i32> = sqlx::query_scalar(
            "SELECT version FROM extension_migrations WHERE extension_id = $1 ORDER BY version",
        )
        .bind(extension)
        .fetch_all(&pool)
        .await
        .unwrap();
        pool.close().await;
        versions
    }

    async fn ctx(&self, json: bool) -> CommandContext {
        let mut cli = CliConfig::new().with_interactive(false);
        if json {
            cli = cli.with_output_format(OutputFormat::Json);
        }
        let db_ctx = DatabaseContext::from_url(&self.url).await.unwrap();
        CommandContext::with_database(cli, EnvOverrides::default(), db_ctx, self.url.clone())
    }

    async fn verbose_ctx(&self) -> CommandContext {
        let cli = CliConfig::new()
            .with_interactive(false)
            .with_verbosity(VerbosityLevel::Verbose);
        let db_ctx = DatabaseContext::from_url(&self.url).await.unwrap();
        CommandContext::with_database(cli, EnvOverrides::default(), db_ctx, self.url.clone())
    }

    async fn drop(self) {
        let _ = sqlx::query(sqlx::AssertSqlSafe(format!(
            "DROP DATABASE IF EXISTS \"{}\" WITH (FORCE)",
            self.name
        )))
        .execute(&self.admin)
        .await;
    }
}

#[tokio::test]
async fn a_drifted_checksum_is_reported_by_status_and_healed_by_repair_apply() {
    let disp = Disposable::create().await;
    db::execute(parse(&["migrate"]), &disp.ctx(false).await)
        .await
        .unwrap();

    let original = disp.stored_checksum("logging", 3).await;
    disp.corrupt_checksum("logging", 3).await;
    assert_ne!(
        disp.stored_checksum("logging", 3).await,
        original,
        "the fixture must actually have introduced drift"
    );

    // Both renderers must survive a drift row: the text one truncates the two
    // checksums for display, the JSON one carries them whole.
    db::execute(parse(&["migrate-status"]), &disp.ctx(false).await)
        .await
        .unwrap();
    db::execute(parse(&["migrate-status", "--json"]), &disp.ctx(true).await)
        .await
        .unwrap();

    // A dry-run repair reports the drift without touching it.
    db::execute(parse(&["migrate-repair"]), &disp.ctx(false).await)
        .await
        .unwrap();
    assert_ne!(
        disp.stored_checksum("logging", 3).await,
        original,
        "a dry-run repair must not rewrite the stored checksum"
    );

    db::execute(
        parse(&["migrate-repair", "--apply"]),
        &disp.ctx(false).await,
    )
    .await
    .unwrap();
    assert_eq!(
        disp.stored_checksum("logging", 3).await,
        original,
        "--apply must restore the checksum to the on-disk migration's"
    );

    disp.drop().await;
}

#[tokio::test]
async fn repair_can_be_scoped_to_a_single_extension() {
    let disp = Disposable::create().await;
    db::execute(parse(&["migrate"]), &disp.ctx(false).await)
        .await
        .unwrap();

    let original = disp.stored_checksum("logging", 3).await;
    disp.corrupt_checksum("logging", 3).await;

    db::execute(
        parse(&["migrate-repair", "logging", "--apply"]),
        &disp.ctx(false).await,
    )
    .await
    .unwrap();
    assert_eq!(disp.stored_checksum("logging", 3).await, original);

    let unknown = db::execute(
        parse(&["migrate-repair", "no_such_extension"]),
        &disp.ctx(false).await,
    )
    .await;
    assert!(
        unknown.is_err(),
        "scoping repair to an unregistered extension must error, not silently repair nothing"
    );

    disp.drop().await;
}

#[tokio::test]
async fn migrate_down_refuses_an_irreversible_migration_and_leaves_the_ledger_intact() {
    let disp = Disposable::create().await;
    db::execute(parse(&["migrate"]), &disp.ctx(false).await)
        .await
        .unwrap();

    let before = disp.applied_versions("logging").await;
    assert!(
        !before.is_empty(),
        "the logging extension must have applied migrations, got {before:?}"
    );

    let err = db::execute(
        parse(&["migrate-down", "logging", "1"]),
        &disp.ctx(false).await,
    )
    .await
    .expect_err("no in-tree migration ships down SQL, so the revert must refuse");
    let message = err.to_string();
    assert!(
        message.contains("not reversible"),
        "the refusal must say why, got {message}"
    );

    assert_eq!(
        disp.applied_versions("logging").await,
        before,
        "a refused revert must not partially unwind the migration ledger"
    );

    disp.drop().await;
}

#[tokio::test]
async fn migrate_down_reports_an_unregistered_extension_before_touching_the_database() {
    let disp = Disposable::create().await;
    db::execute(parse(&["migrate"]), &disp.ctx(false).await)
        .await
        .unwrap();

    let before = disp.applied_versions("logging").await;
    let err = db::execute(
        parse(&["migrate-down", "no_such_extension", "1"]),
        &disp.ctx(true).await,
    )
    .await
    .expect_err("an unregistered extension has no migrations to revert");
    assert!(
        err.to_string().contains("no_such_extension"),
        "the error must name the extension asked for, got {err}"
    );

    assert_eq!(
        disp.applied_versions("logging").await,
        before,
        "a lookup failure must not disturb any other extension's ledger"
    );

    disp.drop().await;
}

#[tokio::test]
async fn a_verbose_install_still_installs_every_schema_extension() {
    let disp = Disposable::create().await;

    db::execute(parse(&["migrate"]), &disp.verbose_ctx().await)
        .await
        .unwrap();

    let applied = disp.applied_versions("logging").await;
    assert!(
        !applied.is_empty(),
        "the verbose renderer must not short-circuit the install"
    );

    // Re-running is idempotent, which is what makes `migrate` safe to put in a
    // boot sequence.
    db::execute(parse(&["migrate"]), &disp.verbose_ctx().await)
        .await
        .unwrap();
    assert_eq!(disp.applied_versions("logging").await, applied);

    disp.drop().await;
}

#[tokio::test]
async fn migrate_refuses_a_drifted_checksum_unless_drift_is_allowed() {
    let disp = Disposable::create().await;
    db::execute(parse(&["migrate"]), &disp.ctx(false).await)
        .await
        .unwrap();

    disp.corrupt_checksum("logging", 3).await;

    let strict = db::execute(parse(&["migrate"]), &disp.ctx(false).await).await;
    let permissive = db::execute(
        parse(&["migrate", "--allow-checksum-drift"]),
        &disp.ctx(false).await,
    )
    .await;

    assert!(
        strict.is_err(),
        "an install over a drifted checksum must fail closed"
    );
    assert!(
        permissive.is_ok(),
        "--allow-checksum-drift is the documented escape hatch: {permissive:?}"
    );

    disp.drop().await;
}
