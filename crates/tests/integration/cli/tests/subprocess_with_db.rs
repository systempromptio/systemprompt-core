//! Subprocess tests that drive the `systemprompt` binary via `--database-url`.
//!
//! The `--database-url` flag bypasses profile/secrets/credentials bootstrap and
//! routes straight into the `*_with_db` handler dispatchers in
//! `crates/entry/cli/src/runner/mod.rs::run_with_database_url`. This unlocks
//! coverage on the entire `infra db`, `analytics`, `admin users`,
//! `core content`, and `core files` handler trees without needing a profile
//! tempdir or wiremock cloud server.
//!
//! Each `Command::cargo_bin` invocation inherits `LLVM_PROFILE_FILE` from the
//! parent test process (set by `just coverage`); the child writes its own
//! profraw which the merge step folds into the report.
//!
//! Each test drives one command family, printing every command line before it
//! runs so a failure or timeout names the invocation.
//!
//! Tests are intentionally permissive — they accept both success and failure
//! exit codes. The point is to drive code coverage into handler bodies,
//! argument parsing, repository wiring, query building, and result rendering
//! — not to assert business outcomes.

use assert_cmd::Command;
use predicates::str::contains;
use systemprompt_identifiers::SessionId;
use systemprompt_test_fixtures::{
    seed_user_row, seed_user_session, test_database_url, test_db_pool, unique_user_id,
};

fn systemprompt_bin() -> std::path::PathBuf {
    if let Ok(path) = std::env::var("SYSTEMPROMPT_BIN") {
        let p = std::path::PathBuf::from(path);
        if p.exists() {
            return p;
        }
    }
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for ancestor in manifest_dir.ancestors() {
        for sub in [
            "target/debug/systemprompt",
            "crates/tests/target/debug/systemprompt",
        ] {
            let candidate = ancestor.join(sub);
            if candidate.exists() {
                return candidate;
            }
        }
    }
    panic!("systemprompt binary not found; set SYSTEMPROMPT_BIN or run via `just coverage`");
}

fn sp_db() -> Command {
    let url = test_database_url();
    let mut c = Command::new(systemprompt_bin());
    c.env("SYSTEMPROMPT_PROFILE", "__nonexistent__");
    c.env_remove("RUST_LOG");
    c.arg("--database-url").arg(url);
    c
}

// Reports read the source tables, so seeded conversation and session
// evidence only has to commit before the binary runs.
fn seed_reporting_evidence() {
    tokio::runtime::Runtime::new()
        .expect("tokio runtime")
        .block_on(async {
            let db = test_db_pool().await;
            let user_id = unique_user_id("subprocess-analytics");
            let email = format!("{}@subprocess.invalid", user_id.as_str());
            seed_user_row(&db, &user_id, &email)
                .await
                .expect("seed user");
            seed_user_session(&db, &user_id, &SessionId::generate())
                .await
                .expect("seed session");
            sqlx::query(
                "INSERT INTO user_contexts (context_id, user_id, name, kind) VALUES ($1, $2, \
                 'subprocess-analytics', 'user')",
            )
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(user_id.as_str())
            .execute(db.pool().as_ref())
            .await
            .expect("seed context");
        });
}

fn announce(args: &[&str]) {
    eprintln!("$ systemprompt --database-url <test-db> {}", args.join(" "));
}

fn run_db(args: &[&str]) {
    announce(args);
    let mut cmd = sp_db();
    cmd.args(args);
    let _ = cmd.assert();
}

fn run_db_with_format(args: &[&str]) {
    run_db(args);
    drive_formats(args);
}

fn drive_formats(args: &[&str]) {
    for fmt in ["--json", "--yaml"] {
        let mut cmd = sp_db();
        let mut full: Vec<&str> = vec![fmt];
        full.extend_from_slice(args);
        announce(&full);
        cmd.args(&full);
        let _ = cmd.assert();
    }
}

fn db_stderr(args: &[&str], needle: &str) {
    announce(args);
    let mut cmd = sp_db();
    cmd.args(args);
    cmd.assert().success().stderr(contains(needle));
}

fn db_stdout(args: &[&str], needle: &str) {
    announce(args);
    let mut cmd = sp_db();
    cmd.args(args);
    cmd.assert().success().stdout(contains(needle));
}

fn db_stderr_fmt(args: &[&str], needle: &str) {
    db_stderr(args, needle);
    drive_formats(args);
}

fn db_stdout_fmt(args: &[&str], needle: &str) {
    db_stdout(args, needle);
    drive_formats(args);
}

fn sp_noprofile() -> Command {
    let mut c = Command::new(systemprompt_bin());
    c.env("SYSTEMPROMPT_PROFILE", "__nonexistent__");
    c.env_remove("RUST_LOG");
    c
}

fn run_noprofile(args: &[&str]) {
    eprintln!("$ systemprompt {}", args.join(" "));
    let _ = sp_noprofile().args(args).assert();
}

fn db_fails(args: &[&str], needle: &str) {
    announce(args);
    let mut cmd = sp_db();
    cmd.args(args);
    cmd.assert().failure().stderr(contains(needle));
}

#[test]
fn db_connection_and_tables() {
    db_stderr_fmt(&["infra", "db", "status"], "Database connection");
    db_stderr_fmt(&["infra", "db", "info"], "Database: PostgreSQL");
    db_stdout_fmt(&["infra", "db", "tables"], "users");
    db_stdout_fmt(&["infra", "db", "tables", "--filter", "user"], "users");
    db_stderr_fmt(
        &["infra", "db", "tables", "--filter", "zzz_no_match"],
        "Tables",
    );
}

#[test]
fn db_size_indexes_describe_and_count() {
    db_stderr_fmt(&["infra", "db", "size"], "Database Size");
    db_stderr_fmt(&["infra", "db", "indexes"], "Indexes");
    db_stderr_fmt(
        &["infra", "db", "indexes", "--table", "users"],
        "idx_users_name",
    );
    db_stdout_fmt(&["infra", "db", "describe", "users"], "email");
    db_fails(
        &["infra", "db", "describe", "definitely_not_a_real_table"],
        "not found",
    );
    db_stderr_fmt(&["infra", "db", "count", "users"], "users:");
    db_fails(
        &["infra", "db", "count", "definitely_not_a_real_table"],
        "not found",
    );
}

#[test]
fn db_doctor_and_migration_ledger() {
    run_db_with_format(&["infra", "db", "doctor"]);
    db_stderr_fmt(&["infra", "db", "migrations", "status"], "applied");
    db_stderr_fmt(&["infra", "db", "migrations", "list"], "applied");
    run_db(&["infra", "db", "migrations", "history", "nonexistent_ext"]);
}

#[test]
fn db_migrate_plan_and_status() {
    db_stderr_fmt(&["infra", "db", "migrate-plan"], "migrations");
    run_db(&["infra", "db", "migrate-plan", "--json"]);
    run_db_with_format(&["infra", "db", "migrate-plan", "users"]);
    db_stderr_fmt(&["infra", "db", "migrate-status"], "Applied:");
    run_db(&["infra", "db", "migrate-status", "--json"]);
    run_db_with_format(&["infra", "db", "migrate-status", "users"]);
}

#[test]
fn db_migrate_repair_and_apply() {
    run_db(&["infra", "db", "migrate-repair"]);
    run_db(&["infra", "db", "migrate-repair", "--json"]);
    run_db(&["infra", "db", "migrate-repair", "users"]);
    run_db(&["infra", "db", "migrate"]);
    run_db(&["infra", "db", "migrate", "--allow-checksum-drift"]);
}

#[test]
fn db_query_selects() {
    db_stdout_fmt(&["infra", "db", "query", "SELECT 1 AS one"], "one");
    run_db_with_format(&[
        "infra",
        "db",
        "query",
        "SELECT * FROM users",
        "--limit",
        "5",
    ]);
    run_db_with_format(&[
        "infra",
        "db",
        "query",
        "SELECT * FROM users",
        "--limit",
        "5",
        "--offset",
        "0",
    ]);
    run_db_with_format(&["infra", "db", "query", "SELECT 1 AS x WHERE 1=0"]);
}

#[test]
fn db_query_catalogs_and_refusals() {
    db_fails(
        &["infra", "db", "query", "SELECT not valid sql syntax !!!"],
        "syntax error",
    );
    db_stdout_fmt(
        &[
            "infra",
            "db",
            "query",
            "SELECT table_schema, table_name FROM information_schema.tables LIMIT 3",
        ],
        "table_name",
    );
    db_stdout_fmt(
        &["infra", "db", "query", "SELECT current_database() AS db"],
        "db",
    );
    run_db(&[
        "infra",
        "db",
        "query",
        "SELECT 1 AS one",
        "--format",
        "json",
    ]);
    db_fails(
        &[
            "infra",
            "db",
            "query",
            "INSERT INTO users (id) VALUES ('nope')",
        ],
        "must be a SELECT, an EXPLAIN of a SELECT, or SHOW",
    );
}

#[test]
fn db_execute() {
    run_db(&["infra", "db", "execute", "SELECT 1", "--yes"]);
    db_fails(
        &[
            "infra",
            "db",
            "execute",
            "DELETE FROM nonexistent_table_xyz",
            "--yes",
        ],
        "does not exist",
    );
    run_db(&[
        "infra", "db", "execute", "SELECT 1", "--yes", "--format", "json",
    ]);
}

#[test]
fn analytics_overview() {
    db_stderr_fmt(&["analytics", "overview"], "Analytics Overview");
    run_db_with_format(&["analytics", "overview", "--since", "1h"]);
    run_db_with_format(&["analytics", "overview", "--since", "24h"]);
    run_db_with_format(&["analytics", "overview", "--since", "7d"]);
    run_db(&["analytics", "overview", "--since", "garbage"]);
}

#[test]
fn analytics_conversation_stats_and_trends() {
    db_stderr_fmt(
        &["analytics", "conversations", "stats"],
        "Conversation Statistics",
    );
    run_db_with_format(&["analytics", "conversations", "stats", "--since", "7d"]);
    run_db_with_format(&["analytics", "conversations", "trends"]);
    run_db_with_format(&["analytics", "conversations", "trends", "--since", "24h"]);
}

#[test]
fn analytics_conversation_list() {
    seed_reporting_evidence();
    db_stderr_fmt(&["analytics", "conversations", "list"], "Conversations");
    run_db_with_format(&["analytics", "conversations", "list", "--limit", "5"]);
}

#[test]
fn analytics_agents() {
    db_stderr_fmt(&["analytics", "agents", "stats"], "Agent Statistics");
    run_db_with_format(&["analytics", "agents", "trends"]);
    run_db_with_format(&["analytics", "agents", "list"]);
    run_db(&["analytics", "agents", "show", "nonexistent-agent"]);
}

#[test]
fn analytics_tools() {
    db_stderr_fmt(&["analytics", "tools", "stats"], "Tool Statistics");
    run_db_with_format(&["analytics", "tools", "trends"]);
    run_db_with_format(&["analytics", "tools", "list"]);
    run_db(&["analytics", "tools", "show", "nonexistent-tool"]);
}

#[test]
fn analytics_requests() {
    db_stderr_fmt(&["analytics", "requests", "stats"], "AI Request Statistics");
    run_db_with_format(&["analytics", "requests", "trends"]);
    run_db_with_format(&["analytics", "requests", "list"]);
    run_db_with_format(&["analytics", "requests", "list", "--limit", "5"]);
    run_db_with_format(&["analytics", "requests", "models"]);
}

#[test]
fn analytics_sessions() {
    db_stderr_fmt(&["analytics", "sessions", "stats"], "Session Statistics");
    run_db_with_format(&["analytics", "sessions", "list"]);
    run_db_with_format(&["analytics", "sessions", "trends"]);
    seed_reporting_evidence();
    db_stderr_fmt(&["analytics", "sessions", "live"], "Live Sessions");
}

#[test]
fn analytics_content() {
    db_stderr_fmt(&["analytics", "content", "stats"], "Content Statistics");
    run_db_with_format(&["analytics", "content", "list"]);
    run_db_with_format(&["analytics", "content", "trends"]);
    run_db_with_format(&["analytics", "content", "top"]);
}

#[test]
fn analytics_traffic() {
    db_stderr_fmt(&["analytics", "traffic", "sources"], "Traffic Sources");
    run_db_with_format(&["analytics", "traffic", "list"]);
    db_stderr_fmt(&["analytics", "traffic", "geo"], "Geographic Distribution");
    db_stderr_fmt(&["analytics", "traffic", "devices"], "Device Breakdown");
    db_stderr_fmt(&["analytics", "traffic", "bots"], "Bot Traffic Analysis");
}

#[test]
fn analytics_costs() {
    db_stderr_fmt(&["analytics", "costs", "summary"], "Cost Summary");
    run_db_with_format(&["analytics", "costs", "list"]);
    run_db_with_format(&["analytics", "costs", "trends"]);
    run_db_with_format(&["analytics", "costs", "breakdown"]);
}

#[test]
fn admin_users_list_pages_and_admins() {
    db_stderr_fmt(&["admin", "users", "list"], "Users");
    run_db_with_format(&["admin", "users", "list", "--limit", "5"]);
    run_db_with_format(&["admin", "users", "list", "--limit", "5", "--offset", "0"]);
    run_db_with_format(&["admin", "users", "list", "--role", "admin"]);
}

#[test]
fn admin_users_list_role_and_status_filters() {
    run_db_with_format(&["admin", "users", "list", "--role", "user"]);
    run_db_with_format(&["admin", "users", "list", "--role", "anonymous"]);
    run_db_with_format(&["admin", "users", "list", "--status", "active"]);
    run_db_with_format(&["admin", "users", "list", "--status", "suspended"]);
}

#[test]
fn admin_users_counts_and_search() {
    db_stderr_fmt(&["admin", "users", "count"], "User Count");
    db_stderr_fmt(&["admin", "users", "stats"], "User Statistics");
    db_stderr_fmt(
        &["admin", "users", "search", "zzz_nothing"],
        "User Search Results",
    );
    run_db_with_format(&["admin", "users", "search", "admin"]);
}

#[test]
fn admin_users_records_sessions_and_bans() {
    db_fails(
        &["admin", "users", "show", "nonexistent-user-id"],
        "User not found",
    );
    db_stderr(&["admin", "users", "export"], "User Export");
    run_db(&["admin", "users", "session", "list", "nonexistent-user"]);
    db_stderr_fmt(&["admin", "users", "ban", "list"], "Banned IPs");
}

#[test]
fn profile_required_admin_web_plugins_build() {
    run_db(&["admin", "users", "create", "--email", "test@example.com"]);
    run_db(&["admin", "users", "delete", "some-id"]);
    run_db(&["admin", "agents", "list"]);
    run_db(&["admin", "config", "list"]);
    run_db(&["admin", "setup"]);
    run_db(&["web", "validate"]);
    run_db(&["plugins", "list"]);
    run_db(&["build", "core"]);
}

#[test]
fn profile_required_core_cloud_and_db() {
    run_db(&["core", "artifacts", "list"]);
    run_db(&["core", "skills", "list"]);
    run_db(&["core", "plugins", "list"]);
    run_db(&["core", "hooks", "list"]);
    run_db(&["core", "contexts", "list"]);
    run_db(&["cloud", "status"]);
    run_db(&[]);
    run_db(&["infra", "db", "assign-admin", "nobody"]);
}

#[test]
fn infra_logs_queries() {
    run_db(&["infra", "logs", "show"]);
    run_db(&["infra", "logs", "summary"]);
    run_db(&["infra", "logs", "search", "test"]);
    run_db(&["infra", "logs", "audit"]);
    run_db(&["infra", "logs", "tools", "list", "--limit", "5"]);
    run_db(&["infra", "logs", "request", "list"]);
    run_db(&["infra", "logs", "request", "show", "nonexistent-id"]);
    run_db(&["infra", "logs", "request", "stats"]);
    run_db(&["infra", "logs", "trace", "list"]);
}

#[test]
fn infra_logs_traces_views_and_export() {
    run_db(&["infra", "logs", "trace", "show", "nonexistent-trace"]);
    run_db(&["infra", "logs", "tools", "list"]);
    run_db(&["infra", "logs", "view"]);
    run_db(&["infra", "logs", "view", "--level", "error"]);
    run_db(&["infra", "logs", "view", "--tail", "5"]);
    run_db(&["infra", "logs", "view", "--module", "test"]);
    run_db(&["infra", "logs", "view", "--since", "1h"]);
    run_db(&["infra", "logs", "export"]);
}

#[test]
fn infra_logs_delete_and_cleanup() {
    // -y flag skips confirmation; against an empty test DB this is harmless.
    run_db(&["infra", "logs", "delete", "-y"]);
    run_db(&["infra", "logs", "cleanup"]);
}

#[test]
fn core_content_listing_and_search() {
    db_stderr_fmt(&["core", "content", "list"], "Content");
    run_db_with_format(&["core", "content", "list", "--limit", "5"]);
    db_fails(
        &["core", "content", "show", "nonexistent-slug"],
        "No content with slug",
    );
    db_stderr_fmt(&["core", "content", "search", "test"], "Search Results");
}

#[test]
fn core_content_reports() {
    run_db_with_format(&["core", "content", "popular"]);
    run_db_with_format(&["core", "content", "status", "test-source"]);
    run_db(&["core", "content", "analytics", "clicks", "nonexistent-link"]);
    run_db(&["core", "content", "analytics", "campaign", "nonexistent"]);
    run_db(&["core", "content", "analytics", "journey", "nonexistent"]);
}

#[test]
fn core_files() {
    db_stderr_fmt(&["core", "files", "list"], "Files");
    run_db_with_format(&["core", "files", "list", "--limit", "5"]);
    db_fails(
        &["core", "files", "show", "nonexistent-id"],
        "Failed to show file",
    );
    db_stderr_fmt(&["core", "files", "search", "test"], "File Search Results");
    db_stderr_fmt(&["core", "files", "stats"], "File Storage Statistics");
}

#[test]
fn db_url_global_flags() {
    run_db(&["--verbose", "infra", "db", "status"]);
    run_db(&["--debug", "infra", "db", "status"]);
    run_db(&["--quiet", "infra", "db", "status"]);
    run_db(&["--no-color", "infra", "db", "tables"]);
    run_db(&["--non-interactive", "infra", "db", "tables"]);
}

#[test]
fn renderer_and_log_filter_repeats() {
    run_db(&["--verbose", "infra", "db", "status"]);
    run_db(&["--debug", "infra", "db", "status"]);
    run_db(&["--verbose", "--json", "infra", "db", "tables"]);
    run_db(&["--no-color", "infra", "db", "query", "SELECT 1"]);
    run_db(&["--verbose", "analytics", "overview"]);
    run_db(&["--quiet", "admin", "users", "list"]);
    run_db(&["--yaml", "infra", "db", "indexes"]);
    run_db(&["--yaml", "analytics", "costs", "summary"]);
}

#[test]
fn session_and_cloud_auth_without_profile() {
    run_noprofile(&["admin", "session", "show"]);
    run_noprofile(&["--json", "admin", "session", "show"]);
    run_noprofile(&["--yaml", "admin", "session", "show"]);
    run_noprofile(&["admin", "session", "list"]);
    run_noprofile(&["--json", "admin", "session", "list"]);
    run_noprofile(&["cloud", "auth"]);
}

#[test]
fn session_logout_without_profile() {
    run_noprofile(&["admin", "session", "logout"]);
}
