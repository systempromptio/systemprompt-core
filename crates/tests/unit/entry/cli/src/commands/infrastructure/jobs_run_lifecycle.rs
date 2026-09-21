//! Public manual job execution reports mixed outcomes and persists both runs.

use std::io::{Read, Seek, SeekFrom};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use clap::Parser;
use serde_json::Value;
use systemprompt_cli::infrastructure::jobs::{self, JobsCommands};
use systemprompt_cli::{CliConfig, CommandContext, EnvOverrides, OutputFormat};
use systemprompt_scheduler::JobRepository;
use systemprompt_test_fixtures::{
    DisposableDb, ensure_test_bootstrap, fixture_app_context, install_test_signing_key,
};

const HELPER: &str = "commands::infrastructure::jobs_run_lifecycle::mixed_job_run_helper";
const SUCCESS_JOB: &str = "cleanup_inactive_sessions";
const MISSING_JOB: &str = "owned_missing_job";

#[derive(Debug, Parser)]
struct Harness {
    #[command(subcommand)]
    command: JobsCommands,
}

fn parse(args: &[&str]) -> JobsCommands {
    Harness::try_parse_from(std::iter::once("jobs").chain(args.iter().copied()))
        .expect("parse jobs command")
        .command
}

#[tokio::test]
#[ignore = "re-executed by mixed_manual_run_reports_rows_and_persists_each_outcome"]
async fn mixed_job_run_helper() {
    ensure_test_bootstrap();
    install_test_signing_key();
    let database = DisposableDb::installed("cli_jobs_run")
        .await
        .expect("private jobs database");
    let info_path =
        std::env::var_os("JOBS_RUN_DATABASE_INFO").expect("private database info channel");
    std::fs::write(info_path, database.url()).expect("write private database info");
    let pool = database.pool().await.expect("private jobs pool");
    let app = fixture_app_context(&pool, database.url()).expect("full fixture app context");
    let repository = JobRepository::new(&pool).expect("job repository");
    for name in [SUCCESS_JOB, MISSING_JOB] {
        repository
            .upsert_job(name, "0 0 * * * *", true)
            .await
            .expect("seed scheduled job row");
    }
    let ctx = CommandContext::with_app_context(
        CliConfig::new()
            .with_interactive(false)
            .with_output_format(OutputFormat::Json),
        EnvOverrides::default(),
        app,
    );

    println!("BEGIN_JOB_RUN");
    jobs::execute(
        parse(&[
            "run",
            SUCCESS_JOB,
            MISSING_JOB,
            "--param",
            "inactive_hours=8760",
        ]),
        &ctx,
    )
    .await
    .expect("mixed manual batch returns a structured report");
    println!("END_JOB_RUN");

    let raw = pool.pool_arc().expect("private SQL pool");
    let states: Vec<(String, Option<String>, Option<String>, i32)> = sqlx::query_as(
        "SELECT job_name, last_status, last_error, run_count FROM scheduled_jobs \
         WHERE job_name = ANY($1) ORDER BY job_name",
    )
    .bind(vec![SUCCESS_JOB, MISSING_JOB])
    .fetch_all(raw.as_ref())
    .await
    .expect("read durable job outcomes");
    println!(
        "JOB_STATES={}",
        serde_json::to_string(&states).expect("serialize job states")
    );

    drop(raw);
    drop(ctx);
    drop(repository);
    pool.write_pool_arc().expect("write pool").close().await;
    drop(pool);
    database.drop_now().await;
}

struct OwnedChild(Option<Child>);

impl Drop for OwnedChild {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn captured_helper() -> String {
    let stdout = tempfile::tempfile().expect("owned jobs stdout");
    let stderr = tempfile::tempfile().expect("owned jobs stderr");
    let database_info = tempfile::NamedTempFile::new().expect("owned database info channel");
    let child = Command::new(std::env::current_exe().expect("CLI unit test binary"))
        .args(["--exact", HELPER, "--ignored", "--nocapture"])
        .env("JOBS_RUN_DATABASE_INFO", database_info.path())
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout.try_clone().expect("clone stdout")))
        .stderr(Stdio::from(stderr.try_clone().expect("clone stderr")))
        .spawn()
        .expect("spawn mixed jobs helper");
    let mut child = OwnedChild(Some(child));
    let deadline = Instant::now() + Duration::from_secs(60);
    let status = loop {
        if let Some(status) = child
            .0
            .as_mut()
            .expect("child present")
            .try_wait()
            .expect("poll child")
        {
            break status;
        }
        assert!(
            Instant::now() < deadline,
            "jobs helper exceeded sixty seconds"
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    child.0.take();
    let read = |mut file: std::fs::File| {
        file.seek(SeekFrom::Start(0)).expect("rewind capture");
        let mut value = String::new();
        file.read_to_string(&mut value).expect("read capture");
        value
    };
    let stdout = read(stdout);
    let stderr = read(stderr);
    let database_url = std::fs::read_to_string(database_info.path()).unwrap_or_default();
    let password = url::Url::parse(&database_url)
        .ok()
        .and_then(|url| url.password().map(str::to_owned))
        .unwrap_or_default();
    let sanitize = |value: &str| {
        [database_url.as_str(), password.as_str()]
            .iter()
            .fold(value.to_owned(), |safe, secret| {
                if secret.is_empty() {
                    safe
                } else {
                    safe.replace(secret, "<REDACTED>")
                }
            })
    };
    assert!(
        status.success(),
        "mixed jobs helper failed; stdout={}; stderr={}",
        sanitize(&stdout),
        sanitize(&stderr)
    );
    sanitize(&stdout)
}

#[test]
fn mixed_manual_run_reports_rows_and_persists_each_outcome() {
    let stdout = captured_helper();
    let artifact = stdout
        .split_once("BEGIN_JOB_RUN")
        .and_then(|(_, tail)| tail.split_once("END_JOB_RUN"))
        .map(|(json, _)| json.trim())
        .unwrap_or_else(|| panic!("missing job artifact markers: {stdout}"));
    let table: Value = serde_json::from_str(artifact)
        .unwrap_or_else(|error| panic!("job artifact is not JSON: {error}: {artifact}"));
    assert_eq!(table["artifact_type"], "table");
    let column_names = table["columns"]
        .as_array()
        .expect("job columns")
        .iter()
        .map(|column| column["name"].as_str().expect("column name"))
        .collect::<Vec<_>>();
    assert_eq!(
        column_names,
        ["job_name", "status", "duration_ms", "result"]
    );
    let rows = table["items"].as_array().expect("job result rows");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["job_name"], SUCCESS_JOB);
    assert_eq!(rows[0]["status"], "success");
    assert!(rows[0]["duration_ms"].is_u64());
    assert_eq!(
        rows[0]["result"],
        serde_json::json!({
            "success": true,
            "message": null,
            "items_processed": 0,
            "items_failed": 0
        })
    );
    assert_eq!(rows[1]["job_name"], MISSING_JOB);
    assert_eq!(rows[1]["status"], "failed");
    assert!(rows[1]["duration_ms"].is_u64());
    assert_eq!(rows[1]["result"]["success"], false);
    assert_eq!(
        rows[1]["result"]["message"],
        "Job 'owned_missing_job' not found"
    );
    assert!(rows[1]["result"]["items_processed"].is_null());
    assert!(rows[1]["result"]["items_failed"].is_null());

    let states = stdout
        .lines()
        .find_map(|line| line.strip_prefix("JOB_STATES="))
        .expect("durable job state marker");
    assert_eq!(
        serde_json::from_str::<Value>(states).expect("durable job state JSON"),
        serde_json::json!([
            [SUCCESS_JOB, "success", null, 1],
            [
                MISSING_JOB,
                "failed",
                "Job 'owned_missing_job' not found",
                1
            ]
        ])
    );
}
