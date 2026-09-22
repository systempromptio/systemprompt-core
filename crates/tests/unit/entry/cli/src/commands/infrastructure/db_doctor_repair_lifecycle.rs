#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use clap::Parser;
use systemprompt_cli::infrastructure::db::{self, DbCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_runtime::DatabaseContext;
use systemprompt_test_fixtures::DisposableDb;

const HELPER: &str =
    "commands::infrastructure::db_doctor_repair_lifecycle::db_doctor_repair_helper";
const TABLE: &str = "doctor_fixture_undeclared";

#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    command: DbCommands,
}

fn doctor() -> DbCommands {
    Harness::try_parse_from(["db", "doctor"]).unwrap().command
}

fn context(pool: systemprompt_database::DbPool, url: &str, output: OutputFormat) -> CommandContext {
    CommandContext::with_database(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(output),
        EnvOverrides::default(),
        DatabaseContext::from_pool(pool),
        url.to_owned(),
    )
}

#[tokio::test]
#[ignore = "re-executed by doctor_reports_and_clears_repairable_schema_drift"]
async fn db_doctor_repair_helper() {
    let database = DisposableDb::installed("cli_doctor_repair")
        .await
        .expect("installed disposable database");
    let pool = database.pool().await.expect("disposable pool");
    let raw = pool.pool_arc().expect("raw pool");
    sqlx::query("CREATE TABLE doctor_fixture_undeclared (id BIGINT PRIMARY KEY)")
        .execute(raw.as_ref())
        .await
        .expect("introduce undeclared table drift");

    // Residue is a failure, not a note: `db doctor` renders the report and then
    // exits non-zero for as long as a live table no extension declares is still
    // there. Both renderers must take that exit, or a drifted schema passes a
    // deploy gate that is there to stop it.
    println!("BEGIN_DRIFT");
    let json_drift = db::execute(
        doctor(),
        &context(pool.clone(), database.url(), OutputFormat::Json),
    )
    .await
    .expect_err("doctor exits non-zero while undeclared residue is live");
    println!("END_DRIFT");
    assert!(
        format!("{json_drift:#}").contains("undeclared table(s)"),
        "{json_drift:#}"
    );

    println!("BEGIN_DRIFT_TEXT");
    let text_drift = db::execute(
        doctor(),
        &context(pool.clone(), database.url(), OutputFormat::Table),
    )
    .await
    .expect_err("doctor exits non-zero for a terminal too");
    println!("END_DRIFT_TEXT");
    assert!(
        format!("{text_drift:#}").contains("undeclared table(s)"),
        "{text_drift:#}"
    );

    sqlx::query("DROP TABLE doctor_fixture_undeclared")
        .execute(raw.as_ref())
        .await
        .expect("repair undeclared table drift");
    let remaining: Option<String> = sqlx::query_scalar("SELECT to_regclass($1)::text")
        .bind(TABLE)
        .fetch_one(raw.as_ref())
        .await
        .expect("verify repaired schema");
    assert!(remaining.is_none());

    println!("BEGIN_REPAIRED");
    db::execute(
        doctor(),
        &context(pool.clone(), database.url(), OutputFormat::Json),
    )
    .await
    .expect("doctor exits zero once the residue is gone");
    println!("END_REPAIRED");

    drop(raw);
    pool.write_pool_arc().expect("write pool").close().await;
    database.drop_now().await;
}

fn marked(stdout: &str, marker: &str) -> serde_json::Value {
    let begin = format!("BEGIN_{marker}");
    let end = format!("END_{marker}");
    let document = stdout
        .split_once(&begin)
        .and_then(|(_, tail)| tail.split_once(&end))
        .map(|(json, _)| json.trim())
        .unwrap_or_else(|| panic!("missing {begin}/{end}: {stdout}"));
    serde_json::from_str(document)
        .unwrap_or_else(|error| panic!("invalid {marker} JSON: {error}: {document}"))
}

fn section<'a>(value: &'a serde_json::Value, heading: &str) -> &'a serde_json::Value {
    &value["sections"]
        .as_array()
        .expect("doctor card sections")
        .iter()
        .find(|section| section["heading"] == heading)
        .unwrap_or_else(|| panic!("missing {heading}: {value}"))["content"]
}

fn bounded_output(mut command: Command) -> Output {
    let stdout = tempfile::NamedTempFile::new().expect("create doctor stdout capture");
    let stderr = tempfile::NamedTempFile::new().expect("create doctor stderr capture");
    command
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout.reopen().expect("open doctor stdout")))
        .stderr(Stdio::from(stderr.reopen().expect("open doctor stderr")));
    let mut child = command.spawn().expect("run isolated doctor helper");
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(status) = child.try_wait().expect("poll doctor helper") {
            return Output {
                status,
                stdout: std::fs::read(stdout.path()).expect("read doctor stdout"),
                stderr: std::fs::read(stderr.path()).expect("read doctor stderr"),
            };
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let status = child.wait().expect("reap timed-out doctor helper");
            let output = Output {
                status,
                stdout: std::fs::read(stdout.path()).expect("read timed-out doctor stdout"),
                stderr: std::fs::read(stderr.path()).expect("read timed-out doctor stderr"),
            };
            panic!(
                "doctor helper timed out\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn doctor_reports_and_clears_repairable_schema_drift() {
    let mut command = Command::new(std::env::current_exe().expect("unit test binary"));
    command
        .args(["--exact", HELPER, "--ignored", "--nocapture"])
        .env("RUST_LOG", "off");
    let output = bounded_output(command);
    assert!(
        output.status.success(),
        "doctor helper failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("doctor stdout UTF-8");
    let drift = marked(&stdout, "DRIFT");
    assert_eq!(drift["title"], "Database Doctor", "{drift}");
    assert!(
        section(&drift, "undeclared_tables")
            .as_array()
            .expect("undeclared table list")
            .iter()
            .any(|table| table.as_str().is_some_and(|name| name.contains(TABLE))),
        "{drift}"
    );

    let terminal = String::from_utf8(output.stderr).expect("doctor terminal output UTF-8");
    assert!(
        terminal.contains("live table(s) declared by no registered extension"),
        "{terminal}"
    );
    assert!(terminal.contains(TABLE), "{terminal}");

    let repaired = marked(&stdout, "REPAIRED");
    assert!(
        section(&repaired, "undeclared_tables")
            .as_array()
            .expect("repaired undeclared table list")
            .iter()
            .all(|table| table.as_str().is_none_or(|name| !name.contains(TABLE))),
        "{repaired}"
    );
}
