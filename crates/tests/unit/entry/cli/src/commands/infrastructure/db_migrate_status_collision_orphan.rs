#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use clap::Parser;
use systemprompt_cli::infrastructure::db::{self, DbCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides};
use systemprompt_runtime::DatabaseContext;
use systemprompt_test_fixtures::DisposableDb;

const HELPER: &str =
    "commands::infrastructure::db_migrate_status_collision_orphan::dirty_status_helper";

#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    command: DbCommands,
}

fn parse(args: &[&str]) -> DbCommands {
    Harness::try_parse_from(std::iter::once("db").chain(args.iter().copied()))
        .expect("parse database command")
        .command
}

#[tokio::test]
#[ignore = "re-executed by status_diagnoses_collision_and_orphan_without_mutating_the_ledger"]
async fn dirty_status_helper() {
    let database = DisposableDb::installed("cli_migration_dirty_status")
        .await
        .expect("isolated installed database");
    let pool = database.pool().await.expect("isolated pool");
    let raw = pool.pool_arc().expect("raw pool");
    let collision = sqlx::query(
        "UPDATE extension_migrations SET name = 'reused_slot_fixture' \
         WHERE extension_id = 'logging' AND version = 3",
    )
    .execute(raw.as_ref())
    .await
    .expect("create a slot collision");
    assert_eq!(
        collision.rows_affected(),
        1,
        "logging v3 fixture must exist"
    );
    sqlx::query(
        "INSERT INTO extension_migrations (id, extension_id, version, name, checksum) \
         VALUES ('logging:999', 'logging', 999, 'deleted_without_tombstone', 'orphan-checksum')",
    )
    .execute(raw.as_ref())
    .await
    .expect("create an orphaned ledger row");
    let before: Vec<(i32, String, String)> = sqlx::query_as(
        "SELECT version, name, checksum FROM extension_migrations \
         WHERE extension_id = 'logging' ORDER BY version",
    )
    .fetch_all(raw.as_ref())
    .await
    .unwrap();

    let context = CommandContext::with_database(
        CliConfig::new().with_interactive(false),
        EnvOverrides::default(),
        DatabaseContext::from_pool(pool.clone()),
        database.url().to_owned(),
    );
    eprintln!("BEGIN_DIRTY_STATUS");
    db::execute(parse(&["migrate-status", "logging"]), &context)
        .await
        .expect("render dirty migration diagnosis");
    eprintln!("END_DIRTY_STATUS");
    let after: Vec<(i32, String, String)> = sqlx::query_as(
        "SELECT version, name, checksum FROM extension_migrations \
         WHERE extension_id = 'logging' ORDER BY version",
    )
    .fetch_all(raw.as_ref())
    .await
    .unwrap();
    assert_eq!(
        after, before,
        "status must not repair or rewrite the ledger"
    );

    drop(context);
    drop(raw);
    pool.write_pool_arc().unwrap().close().await;
    drop(pool);
    database.drop_now().await;
}

fn bounded_output(mut command: Command) -> Output {
    let stdout = tempfile::NamedTempFile::new().unwrap();
    let stderr = tempfile::NamedTempFile::new().unwrap();
    command
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout.reopen().unwrap()))
        .stderr(Stdio::from(stderr.reopen().unwrap()));
    let mut child = command.spawn().expect("spawn migration status helper");
    let deadline = Instant::now() + Duration::from_secs(25);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return Output {
                status,
                stdout: std::fs::read(stdout.path()).unwrap(),
                stderr: std::fs::read(stderr.path()).unwrap(),
            };
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let status = child.wait().unwrap();
            panic!(
                "migration status helper timed out ({status})\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&std::fs::read(stdout.path()).unwrap()),
                String::from_utf8_lossy(&std::fs::read(stderr.path()).unwrap())
            );
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

#[test]
fn status_diagnoses_collision_and_orphan_without_mutating_the_ledger() {
    let mut command = Command::new(std::env::current_exe().expect("unit-test binary"));
    command.args(["--exact", HELPER, "--ignored", "--nocapture"]);
    let output = bounded_output(command);
    assert!(
        output.status.success(),
        "dirty status helper failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8(output.stderr).expect("UTF-8 status output");
    let section = stderr
        .split_once("BEGIN_DIRTY_STATUS")
        .and_then(|(_, tail)| tail.split_once("END_DIRTY_STATUS"))
        .map(|(value, _)| value)
        .unwrap_or_else(|| panic!("missing status markers in {stderr}"));
    assert!(section.contains("Collisions: 1"), "{section}");
    assert!(section.contains("Orphaned: 1"), "{section}");
    assert!(
        section.contains("migration slot(s) were reused"),
        "{section}"
    );
    assert!(
        section.contains("recorded='reused_slot_fixture'"),
        "{section}"
    );
    assert!(section.contains("file='"), "{section}");
    assert!(
        section.contains("applied migration(s) are no longer declared"),
        "{section}"
    );
}
