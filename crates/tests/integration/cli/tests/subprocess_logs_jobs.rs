//! Subprocess coverage for the `infra logs` and `infra jobs` trees, plus
//! deep `analytics` flag variants that the smoke battery does not reach.
//!
//! Stateful contracts use isolated databases so filtering, export, cleanup, and
//! job metadata assertions do not depend on rows from another test process.

use std::time::Duration;

use assert_cmd::Command;
use serde_json::Value;
use systemprompt_cli_integration_tests::full_bootstrap::{
    FullBootstrap, TEST_MANIFEST_SIGNING_SEED, TEST_OAUTH_AT_REST_PEPPER, isolated_fixture, run,
    run_with_formats, systemprompt_bin,
};
use systemprompt_identifiers::UserId;
use systemprompt_test_fixtures::{DisposableDb, seed_user_row_with_roles};

fn isolated_command(database_url: &str) -> Command {
    let mut command = Command::new(systemprompt_bin());
    command.env_remove("RUST_LOG");
    command.env_remove("SYSTEMPROMPT_PROFILE");
    command.env("SYSTEMPROMPT_PROFILE", "__nonexistent__");
    command.args([
        "--non-interactive",
        "--no-color",
        "--json",
        "--database-url",
        database_url,
    ]);
    command.timeout(Duration::from_secs(120));
    command
}

async fn full_profile(database: &DisposableDb) -> FullBootstrap {
    let pool = database.pool().await.expect("open profile database");
    let admin_id = UserId::new(format!("cli-admin-{}", uuid::Uuid::new_v4().simple()));
    seed_user_row_with_roles(
        &pool,
        &admin_id,
        "cli-admin@example.invalid",
        &["admin".to_owned()],
    )
    .await
    .expect("seed configured profile administrator");
    let raw = pool.pool_arc().expect("raw profile database pool");
    sqlx::query("UPDATE users SET name = 'testadmin' WHERE id = $1")
        .bind(admin_id.as_str())
        .execute(raw.as_ref())
        .await
        .expect("bind administrator to configured username");

    let fixture = isolated_fixture(8080);
    let web = fixture.services_dir.join("web");
    std::fs::create_dir_all(web.join("templates")).expect("create web templates");
    std::fs::create_dir_all(web.join("assets")).expect("create web assets");
    let config_path = web.join("config.yaml");
    let config = std::fs::read_to_string(&config_path).expect("read web config");
    std::fs::write(
        config_path,
        format!(
            "paths:\n  templates: {}\n  assets: {}\n{config}",
            web.join("templates").display(),
            web.join("assets").display()
        ),
    )
    .expect("complete web paths");
    fixture
}

fn profiled_command(database_url: &str, fixture: &FullBootstrap) -> Command {
    let mut command = Command::new(systemprompt_bin());
    command.env_remove("RUST_LOG");
    command.env_remove("SYSTEMPROMPT_PROFILE");
    command.env("DATABASE_URL", database_url);
    command.env("OAUTH_AT_REST_PEPPER", TEST_OAUTH_AT_REST_PEPPER);
    command.env("MANIFEST_SIGNING_SECRET_SEED", TEST_MANIFEST_SIGNING_SEED);
    command.env("SYSTEMPROMPT_SUBPROCESS", "1");
    command.args(["--non-interactive", "--no-color", "--json", "--profile"]);
    command.arg(&fixture.profile_path);
    command.timeout(Duration::from_secs(120));
    command
}

fn profiled_json_success(database_url: &str, fixture: &FullBootstrap, args: &[&str]) -> Value {
    let mut command = profiled_command(database_url, fixture);
    command.args(args);
    let output = command
        .output()
        .expect("run bounded profile CLI subprocess");
    assert!(
        output.status.success(),
        "profile CLI command failed: stdout={} stderr={}",
        redact(&output.stdout, database_url),
        redact(&output.stderr, database_url),
    );
    let stdout = redact(&output.stdout, database_url);
    serde_json::from_str(&stdout)
        .unwrap_or_else(|error| panic!("stdout must be one JSON artifact: {error}\n{stdout}"))
}

fn sanitized_output(database_url: &str, args: &[&str]) -> std::process::Output {
    let mut command = isolated_command(database_url);
    command.args(args);
    command.output().expect("run bounded CLI subprocess")
}

fn redact(text: &[u8], database_url: &str) -> String {
    let mut sanitized = String::from_utf8_lossy(text).replace(database_url, "<database-url>");
    if let Ok(parsed) = url::Url::parse(database_url) {
        if let Some(password) = parsed.password().filter(|value| !value.is_empty()) {
            sanitized = sanitized.replace(password, "<database-password>");
        }
    }
    sanitized
}

fn json_success(database_url: &str, args: &[&str]) -> Value {
    let output = sanitized_output(database_url, args);
    assert!(
        output.status.success(),
        "CLI command failed: stdout={} stderr={}",
        redact(&output.stdout, database_url),
        redact(&output.stderr, database_url),
    );
    let stdout = redact(&output.stdout, database_url);
    serde_json::from_str(&stdout)
        .unwrap_or_else(|error| panic!("stdout must be one JSON artifact: {error}\n{stdout}"))
}

fn card_field<'a>(card: &'a Value, heading: &str) -> &'a Value {
    card["sections"]
        .as_array()
        .expect("card sections")
        .iter()
        .find(|section| section["heading"] == heading)
        .map(|section| &section["content"])
        .unwrap_or_else(|| panic!("missing card field {heading}: {card}"))
}

async fn seeded_logs_database(prefix: &str) -> (DisposableDb, sqlx::PgPool, String, String) {
    let database = DisposableDb::installed(prefix)
        .await
        .expect("install isolated log database");
    let pool = database
        .pool()
        .await
        .expect("open isolated log database")
        .pool_arc()
        .expect("raw PostgreSQL pool")
        .as_ref()
        .clone();
    let suffix = uuid::Uuid::new_v4().simple().to_string();
    let owner = format!("log_owner_{suffix}");
    let module = format!("coverage.logs.{suffix}");
    sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $1, $2)")
        .bind(&owner)
        .bind(format!("{owner}@example.invalid"))
        .execute(&pool)
        .await
        .expect("seed log owner");
    for (id, age, level, message) in [
        (
            format!("recent_{suffix}"),
            "1 hour",
            "INFO",
            format!("recent marker {suffix}"),
        ),
        (
            format!("old_{suffix}"),
            "400 days",
            "ERROR",
            format!("old marker {suffix}"),
        ),
    ] {
        sqlx::query(
            "INSERT INTO logs (id, timestamp, level, module, message, user_id, session_id, trace_id) \
             VALUES ($1, NOW() - $2::interval, $3, $4, $5, $6, $7, $8)",
        )
        .bind(&id)
        .bind(age)
        .bind(level)
        .bind(&module)
        .bind(message)
        .bind(&owner)
        .bind(format!("session_{id}"))
        .bind(format!("trace_{id}"))
        .execute(&pool)
        .await
        .expect("seed attributed log");
    }
    sqlx::query(
        "INSERT INTO logs (id, timestamp, level, module, message, user_id, session_id, trace_id) \
         VALUES ($1, NOW(), 'ERROR', $2, $3, $4, $5, $6)",
    )
    .bind(format!("control_{suffix}"))
    .bind(format!("coverage.other.{suffix}"))
    .bind(format!("old marker {suffix}"))
    .bind(&owner)
    .bind(format!("control_session_{suffix}"))
    .bind(format!("control_trace_{suffix}"))
    .execute(&pool)
    .await
    .expect("seed other-module level control");
    (database, pool, module, suffix)
}

#[tokio::test]
async fn logs_view_filters_isolated_rows_and_export_preserves_fields() {
    let (database, pool, module, suffix) = seeded_logs_database("cli_logs_output").await;

    let view = json_success(
        database.url(),
        &[
            "infra", "logs", "view", "--module", &module, "--level", "error",
        ],
    );
    assert_eq!(view["artifact_type"], "table");
    let rows = view["items"].as_array().expect("log table items");
    assert_eq!(
        rows.len(),
        1,
        "level and module filters must both apply: {view}"
    );
    assert_eq!(rows[0]["id"], format!("old_{suffix}"));
    assert_eq!(rows[0]["message"], format!("old marker {suffix}"));
    assert_eq!(rows[0]["level"], "ERROR");

    let temp = tempfile::tempdir().expect("export directory");
    let export_path = temp.path().join("logs.jsonl");
    let export_path_arg = export_path.to_string_lossy().into_owned();
    let exported = json_success(
        database.url(),
        &[
            "infra",
            "logs",
            "export",
            "--format",
            "jsonl",
            "--module",
            &module,
            "--output",
            &export_path_arg,
        ],
    );
    assert_eq!(exported["artifact_type"], "presentation_card");
    assert_eq!(exported["title"], "Logs Exported");
    assert_eq!(card_field(&exported, "exported_count"), 2);
    assert_eq!(card_field(&exported, "format"), "jsonl");
    assert_eq!(
        card_field(&exported, "file_path").as_str(),
        Some(export_path_arg.as_str())
    );
    let contents = std::fs::read_to_string(&export_path).expect("read JSONL export");
    let mut records = contents
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("valid exported JSON line"))
        .collect::<Vec<_>>();
    records.sort_by(|left, right| left["id"].as_str().cmp(&right["id"].as_str()));
    assert_eq!(records.len(), 2);
    assert!(records.iter().all(|record| record["module"] == module));
    assert!(
        records
            .iter()
            .any(|record| record["message"] == format!("recent marker {suffix}"))
    );
    assert!(
        records
            .iter()
            .any(|record| record["message"] == format!("old marker {suffix}"))
    );

    drop(pool);
    database.drop_now().await;
}

#[tokio::test]
async fn logs_cleanup_dry_run_reports_candidate_without_deleting_it() {
    let (database, pool, module, suffix) = seeded_logs_database("cli_logs_cleanup_preview").await;

    let fixture = full_profile(&database).await;
    let preview = profiled_json_success(
        database.url(),
        &fixture,
        &[
            "infra",
            "logs",
            "cleanup",
            "--older-than",
            "365d",
            "--dry-run",
            "--yes",
        ],
    );
    assert_eq!(preview["artifact_type"], "presentation_card");
    assert_eq!(preview["title"], "Cleanup Preview (Dry Run)");
    assert_eq!(card_field(&preview, "deleted_count"), 1);
    assert_eq!(card_field(&preview, "dry_run"), true);
    assert_eq!(card_field(&preview, "vacuum_performed"), false);
    let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM logs WHERE module = $1")
        .bind(&module)
        .fetch_one(&pool)
        .await
        .expect("count logs after preview");
    assert_eq!(remaining, 2, "dry-run must preserve recent and old rows");
    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM logs")
        .fetch_one(&pool)
        .await
        .expect("count every log after preview");
    assert_eq!(
        total, 3,
        "dry-run must preserve the unrelated control row too"
    );
    let old_exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM logs WHERE id = $1)")
        .bind(format!("old_{suffix}"))
        .fetch_one(&pool)
        .await
        .expect("check old row after preview");
    assert!(
        old_exists,
        "the reported cleanup candidate must remain stored"
    );

    drop(pool);
    database.drop_now().await;
}

#[tokio::test]
async fn jobs_list_and_show_expose_registered_metadata_and_missing_job_fails() {
    let _scheduler_extension = systemprompt_scheduler::SchedulerExtension;
    let database = DisposableDb::installed("cli_jobs_output")
        .await
        .expect("install isolated jobs database");

    let fixture = full_profile(&database).await;
    let listed = profiled_json_success(database.url(), &fixture, &["infra", "jobs", "list"]);
    assert_eq!(listed["artifact_type"], "table");
    let rows = listed["items"].as_array().expect("job table items");
    let cleanup = rows
        .iter()
        .find(|row| row["name"] == "cleanup_inactive_sessions")
        .expect("compiled cleanup job must be listed");
    assert!(
        cleanup["description"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
    );
    assert!(
        cleanup["schedule"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
    );
    assert!(cleanup["enabled"].is_boolean());

    let shown = profiled_json_success(
        database.url(),
        &fixture,
        &["infra", "jobs", "show", "cleanup_inactive_sessions"],
    );
    assert_eq!(shown["artifact_type"], "presentation_card");
    assert_eq!(shown["title"], "Job: cleanup_inactive_sessions");
    assert_eq!(card_field(&shown, "name"), "cleanup_inactive_sessions");
    assert_eq!(card_field(&shown, "run_count"), 0);

    let mut missing_command = profiled_command(database.url(), &fixture);
    missing_command.args(["infra", "jobs", "show", "no_such_job"]);
    let missing = missing_command
        .output()
        .expect("run missing-job subprocess");
    assert!(!missing.status.success(), "missing job must fail");
    let stderr = redact(&missing.stderr, database.url());
    assert!(stderr.contains("Unknown job: no_such_job"), "{stderr}");
    assert!(stderr.contains("jobs list"), "{stderr}");

    database.drop_now().await;
}

#[tokio::test]
async fn logs_search_returns_only_matching_module_and_message() {
    let (database, pool, module, suffix) = seeded_logs_database("cli_logs_search").await;
    let result = json_success(
        database.url(),
        &[
            "infra",
            "logs",
            "search",
            &format!("old marker {suffix}"),
            "--module",
            &module,
        ],
    );
    assert_eq!(result["artifact_type"], "table");
    let rows = result["items"].as_array().expect("search result rows");
    assert_eq!(
        rows.len(),
        1,
        "search must exclude nonmatching rows: {result}"
    );
    assert_eq!(rows[0]["id"], format!("old_{suffix}"));
    assert_eq!(rows[0]["module"], module);
    assert_eq!(rows[0]["message"], format!("old marker {suffix}"));
    drop(pool);
    database.drop_now().await;
}

#[tokio::test]
async fn logs_show_resolves_exact_entry_and_rejects_missing_id() {
    let (database, pool, _module, suffix) = seeded_logs_database("cli_logs_show").await;
    let shown = json_success(
        database.url(),
        &["infra", "logs", "show", &format!("recent_{suffix}")],
    );
    assert_eq!(shown["artifact_type"], "presentation_card");
    assert_eq!(shown["title"], "Log Entry Details");
    assert_eq!(
        card_field(&shown, "id").as_str(),
        Some(format!("recent_{suffix}").as_str())
    );
    assert_eq!(
        card_field(&shown, "message").as_str(),
        Some(format!("recent marker {suffix}").as_str())
    );

    let missing = sanitized_output(database.url(), &["infra", "logs", "show", "missing-log-id"]);
    assert!(!missing.status.success(), "missing log ID must fail");
    let stderr = redact(&missing.stderr, database.url());
    assert!(
        stderr.contains("No log entries found for ID: missing-log-id"),
        "{stderr}"
    );
    assert!(stderr.contains("logs view"), "{stderr}");
    drop(pool);
    database.drop_now().await;
}

#[tokio::test]
async fn logs_summary_reports_exact_isolated_level_and_module_counts() {
    let (database, pool, module, _suffix) = seeded_logs_database("cli_logs_summary").await;
    let summary = json_success(database.url(), &["infra", "logs", "summary"]);
    assert_eq!(summary["artifact_type"], "presentation_card");
    assert_eq!(summary["title"], "Logs Summary");
    assert_eq!(card_field(&summary, "total_logs"), 3);
    assert_eq!(card_field(&summary, "database_info")["logs_table_rows"], 3);
    assert_eq!(card_field(&summary, "by_level")["error"], 2);
    assert_eq!(card_field(&summary, "by_level")["info"], 1);
    let modules = card_field(&summary, "top_modules")
        .as_array()
        .expect("top module rows");
    assert!(
        modules
            .iter()
            .any(|row| row["module"] == module && row["count"] == 2)
    );
    drop(pool);
    database.drop_now().await;
}

#[tokio::test]
async fn jobs_history_filters_seeded_status_and_job_name() {
    let _scheduler_extension = systemprompt_scheduler::SchedulerExtension;
    let database = DisposableDb::installed("cli_jobs_history")
        .await
        .expect("install isolated jobs history database");
    let pool = database
        .pool()
        .await
        .expect("open jobs history database")
        .pool_arc()
        .expect("raw PostgreSQL pool")
        .as_ref()
        .clone();
    for (name, status, error) in [
        ("coverage_success", "success", None),
        ("coverage_failure", "failed", Some("owned failure")),
    ] {
        sqlx::query(
            "INSERT INTO scheduled_jobs \
             (id, job_name, schedule, enabled, last_run, last_status, last_error, run_count) \
             VALUES ($1, $1, '0 * * * * *', true, NOW(), $2, $3, 1)",
        )
        .bind(name)
        .bind(status)
        .bind(error)
        .execute(&pool)
        .await
        .expect("seed scheduled job history");
    }
    let fixture = full_profile(&database).await;
    let failed = profiled_json_success(
        database.url(),
        &fixture,
        &["infra", "jobs", "history", "--status", "failed"],
    );
    let rows = failed["items"].as_array().expect("history table items");
    assert_eq!(
        rows.len(),
        1,
        "status filter must exclude successful runs: {failed}"
    );
    assert_eq!(rows[0]["job_name"], "coverage_failure");
    assert_eq!(rows[0]["status"], "failed");
    assert_eq!(rows[0]["error"], "owned failure");

    let named = profiled_json_success(
        database.url(),
        &fixture,
        &["infra", "jobs", "history", "--job", "coverage_success"],
    );
    let named_rows = named["items"].as_array().expect("named history rows");
    assert_eq!(named_rows.len(), 1);
    assert_eq!(named_rows[0]["job_name"], "coverage_success");
    drop(pool);
    database.drop_now().await;
}

#[tokio::test]
async fn db_query_and_describe_return_live_schema_values() {
    let database = DisposableDb::installed("cli_db_query_output")
        .await
        .expect("install isolated query database");
    let query = json_success(
        database.url(),
        &["infra", "db", "query", "SELECT 7::int AS fixture_value"],
    );
    assert_eq!(query["artifact_type"], "table");
    assert_eq!(
        query["columns"],
        serde_json::json!([{ "name": "fixture_value", "column_type": "string" }])
    );
    assert_eq!(query["items"].as_array().expect("query rows").len(), 1);
    assert_eq!(query["items"][0]["fixture_value"], 7);

    let described = json_success(database.url(), &["infra", "db", "describe", "logs"]);
    assert_eq!(described["artifact_type"], "table");
    let columns = described["items"].as_array().expect("described columns");
    let id = columns
        .iter()
        .find(|column| column["name"] == "id")
        .expect("logs.id column");
    assert_eq!(id["nullable"], false);
    assert_eq!(id["primary_key"], true);
    assert!(columns.iter().any(|column| column["name"] == "trace_id"));
    database.drop_now().await;
}

#[test]
fn logs_trace_trees() {
    run_with_formats(&["infra", "logs", "trace", "list"]);
    run(&["infra", "logs", "trace", "list", "--limit", "3", "--all"]);
    run(&["infra", "logs", "trace", "list", "--status", "failed"]);
    run(&["infra", "logs", "trace", "list", "--has-mcp"]);
    run(&["infra", "logs", "trace", "list", "--agent", "covagent"]);
    run(&["infra", "logs", "trace", "show", "no-such-trace"]);
    run(&["infra", "logs", "trace", "show", "no-such-trace", "--all"]);
}

#[test]
fn logs_request_and_tools() {
    run_with_formats(&["infra", "logs", "request", "list"]);
    run(&["infra", "logs", "request", "show", "no-such-request"]);
    run(&["infra", "logs", "request", "stats"]);
    run(&["infra", "logs", "tools", "list"]);
    run(&["infra", "logs", "tools", "list", "--limit", "3"]);
}

#[test]
fn logs_audit_missing_id() {
    run(&["infra", "logs", "audit", "no-such-id"]);
}

#[test]
fn analytics_flag_variants() {
    run(&["analytics", "overview", "--days", "7"]);
    run(&["analytics", "conversations", "list", "--limit", "3"]);
    run(&["analytics", "agents", "list", "--days", "7"]);
    run(&["analytics", "tools", "list", "--days", "7"]);
    run(&["analytics", "requests", "list", "--limit", "3"]);
    run(&["analytics", "sessions", "stats", "--days", "7"]);
    run(&["analytics", "content", "top", "--limit", "3"]);
    run(&["analytics", "traffic", "sources", "--days", "7"]);
    run(&["analytics", "costs", "breakdown", "--days", "7"]);
}
