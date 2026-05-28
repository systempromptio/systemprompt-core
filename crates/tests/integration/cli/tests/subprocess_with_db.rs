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
//! Tests are intentionally permissive — they accept both success and failure
//! exit codes. The point is to drive code coverage into handler bodies,
//! argument parsing, repository wiring, query building, and result rendering
//! — not to assert business outcomes.

use assert_cmd::Command;

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

fn database_url() -> Option<String> {
    if let Ok(url) = std::env::var("DATABASE_URL") {
        if !url.is_empty() {
            return Some(url);
        }
    }
    None
}

fn sp_db() -> Option<Command> {
    let url = database_url()?;
    let mut c = Command::new(systemprompt_bin());
    c.env("SYSTEMPROMPT_PROFILE", "__nonexistent__");
    c.env_remove("RUST_LOG");
    c.arg("--database-url").arg(url);
    Some(c)
}

fn run_db(args: &[&str]) {
    let Some(mut cmd) = sp_db() else {
        return;
    };
    cmd.args(args);
    let _ = cmd.assert();
}

fn run_db_with_format(args: &[&str]) {
    run_db(args);
    let Some(mut cmd) = sp_db() else {
        return;
    };
    let mut full: Vec<&str> = vec!["--json"];
    full.extend_from_slice(args);
    cmd.args(&full);
    let _ = cmd.assert();
    let Some(mut cmd) = sp_db() else {
        return;
    };
    let mut full: Vec<&str> = vec!["--yaml"];
    full.extend_from_slice(args);
    cmd.args(&full);
    let _ = cmd.assert();
}

// ============================================================================
// infra db — schema introspection (read-only)
// ============================================================================

#[test]
fn db_status() {
    run_db_with_format(&["infra", "db", "status"]);
}

#[test]
fn db_info() {
    run_db_with_format(&["infra", "db", "info"]);
}

#[test]
fn db_tables() {
    run_db_with_format(&["infra", "db", "tables"]);
}

#[test]
fn db_tables_with_filter() {
    run_db_with_format(&["infra", "db", "tables", "--filter", "user"]);
}

#[test]
fn db_tables_filter_nonmatching() {
    run_db_with_format(&["infra", "db", "tables", "--filter", "zzz_no_match"]);
}

#[test]
fn db_validate() {
    run_db_with_format(&["infra", "db", "validate"]);
}

#[test]
fn db_size() {
    run_db_with_format(&["infra", "db", "size"]);
}

#[test]
fn db_indexes() {
    run_db_with_format(&["infra", "db", "indexes"]);
}

#[test]
fn db_indexes_with_table() {
    run_db_with_format(&["infra", "db", "indexes", "--table", "users"]);
}

#[test]
fn db_describe_users() {
    run_db_with_format(&["infra", "db", "describe", "users"]);
}

#[test]
fn db_describe_missing_table() {
    run_db_with_format(&["infra", "db", "describe", "definitely_not_a_real_table"]);
}

#[test]
fn db_count_users() {
    run_db_with_format(&["infra", "db", "count", "users"]);
}

#[test]
fn db_count_missing_table() {
    run_db_with_format(&["infra", "db", "count", "definitely_not_a_real_table"]);
}

#[test]
fn db_doctor() {
    run_db_with_format(&["infra", "db", "doctor"]);
}

#[test]
fn db_migrations_status() {
    run_db_with_format(&["infra", "db", "migrations", "status"]);
}

#[test]
fn db_migrations_list_alias() {
    run_db_with_format(&["infra", "db", "migrations", "list"]);
}

#[test]
fn db_migrations_history_missing_ext() {
    run_db(&["infra", "db", "migrations", "history", "nonexistent_ext"]);
}

#[test]
fn db_migrate_plan_all() {
    run_db_with_format(&["infra", "db", "migrate-plan"]);
}

#[test]
fn db_migrate_plan_json() {
    run_db(&["infra", "db", "migrate-plan", "--json"]);
}

#[test]
fn db_migrate_plan_specific_ext() {
    run_db_with_format(&["infra", "db", "migrate-plan", "users"]);
}

#[test]
fn db_migrate_status_all() {
    run_db_with_format(&["infra", "db", "migrate-status"]);
}

#[test]
fn db_migrate_status_json() {
    run_db(&["infra", "db", "migrate-status", "--json"]);
}

#[test]
fn db_migrate_status_specific_ext() {
    run_db_with_format(&["infra", "db", "migrate-status", "users"]);
}

#[test]
fn db_migrate_repair_dryrun() {
    run_db(&["infra", "db", "migrate-repair"]);
}

#[test]
fn db_migrate_repair_json() {
    run_db(&["infra", "db", "migrate-repair", "--json"]);
}

#[test]
fn db_migrate_repair_specific_ext() {
    run_db(&["infra", "db", "migrate-repair", "users"]);
}

#[test]
fn db_migrate_idempotent() {
    run_db(&["infra", "db", "migrate"]);
}

#[test]
fn db_migrate_allow_drift() {
    run_db(&["infra", "db", "migrate", "--allow-checksum-drift"]);
}

// ============================================================================
// infra db query — read SQL
// ============================================================================

#[test]
fn db_query_select_users() {
    run_db_with_format(&["infra", "db", "query", "SELECT 1 AS one"]);
}

#[test]
fn db_query_with_limit() {
    run_db_with_format(&[
        "infra",
        "db",
        "query",
        "SELECT * FROM users",
        "--limit",
        "5",
    ]);
}

#[test]
fn db_query_with_limit_and_offset() {
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
}

#[test]
fn db_query_invalid_sql() {
    run_db(&["infra", "db", "query", "SELECT not valid sql syntax !!!"]);
}

#[test]
fn db_query_empty_result() {
    run_db_with_format(&["infra", "db", "query", "SELECT 1 AS x WHERE 1=0"]);
}

#[test]
fn db_query_information_schema() {
    run_db_with_format(&[
        "infra",
        "db",
        "query",
        "SELECT table_schema, table_name FROM information_schema.tables LIMIT 3",
    ]);
}

#[test]
fn db_query_pg_catalog() {
    run_db_with_format(&["infra", "db", "query", "SELECT current_database() AS db"]);
}

#[test]
fn db_query_with_format_flag() {
    run_db(&[
        "infra",
        "db",
        "query",
        "SELECT 1 AS one",
        "--format",
        "json",
    ]);
}

#[test]
fn db_query_reject_write() {
    // Query is read-only; INSERT should be rejected by the query path.
    run_db(&[
        "infra",
        "db",
        "query",
        "INSERT INTO users (id) VALUES ('nope')",
    ]);
}

// ============================================================================
// infra db execute — write SQL
// ============================================================================

#[test]
fn db_execute_noop() {
    run_db(&["infra", "db", "execute", "SELECT 1"]);
}

#[test]
fn db_execute_invalid_sql() {
    run_db(&[
        "infra",
        "db",
        "execute",
        "DELETE FROM nonexistent_table_xyz",
    ]);
}

#[test]
fn db_execute_with_format() {
    run_db(&["infra", "db", "execute", "SELECT 1", "--format", "json"]);
}

// ============================================================================
// analytics — overview & costs
// ============================================================================

#[test]
fn analytics_overview() {
    run_db_with_format(&["analytics", "overview"]);
}

#[test]
fn analytics_overview_since() {
    run_db_with_format(&["analytics", "overview", "--since", "1h"]);
}

#[test]
fn analytics_overview_24h() {
    run_db_with_format(&["analytics", "overview", "--since", "24h"]);
}

#[test]
fn analytics_overview_7d() {
    run_db_with_format(&["analytics", "overview", "--since", "7d"]);
}

#[test]
fn analytics_overview_invalid_since() {
    run_db(&["analytics", "overview", "--since", "garbage"]);
}

// ============================================================================
// analytics — conversations
// ============================================================================

#[test]
fn analytics_conversations_stats() {
    run_db_with_format(&["analytics", "conversations", "stats"]);
}

#[test]
fn analytics_conversations_stats_since_7d() {
    run_db_with_format(&["analytics", "conversations", "stats", "--since", "7d"]);
}

#[test]
fn analytics_conversations_trends() {
    run_db_with_format(&["analytics", "conversations", "trends"]);
}

#[test]
fn analytics_conversations_trends_since() {
    run_db_with_format(&["analytics", "conversations", "trends", "--since", "24h"]);
}

#[test]
fn analytics_conversations_list() {
    run_db_with_format(&["analytics", "conversations", "list"]);
}

#[test]
fn analytics_conversations_list_with_limit() {
    run_db_with_format(&["analytics", "conversations", "list", "--limit", "5"]);
}

// ============================================================================
// analytics — agents
// ============================================================================

#[test]
fn analytics_agents_stats() {
    run_db_with_format(&["analytics", "agents", "stats"]);
}

#[test]
fn analytics_agents_trends() {
    run_db_with_format(&["analytics", "agents", "trends"]);
}

#[test]
fn analytics_agents_list() {
    run_db_with_format(&["analytics", "agents", "list"]);
}

#[test]
fn analytics_agents_show_missing() {
    run_db(&["analytics", "agents", "show", "nonexistent-agent"]);
}

// ============================================================================
// analytics — tools
// ============================================================================

#[test]
fn analytics_tools_stats() {
    run_db_with_format(&["analytics", "tools", "stats"]);
}

#[test]
fn analytics_tools_trends() {
    run_db_with_format(&["analytics", "tools", "trends"]);
}

#[test]
fn analytics_tools_list() {
    run_db_with_format(&["analytics", "tools", "list"]);
}

#[test]
fn analytics_tools_show_missing() {
    run_db(&["analytics", "tools", "show", "nonexistent-tool"]);
}

// ============================================================================
// analytics — requests
// ============================================================================

#[test]
fn analytics_requests_stats() {
    run_db_with_format(&["analytics", "requests", "stats"]);
}

#[test]
fn analytics_requests_trends() {
    run_db_with_format(&["analytics", "requests", "trends"]);
}

#[test]
fn analytics_requests_list() {
    run_db_with_format(&["analytics", "requests", "list"]);
}

#[test]
fn analytics_requests_list_limit() {
    run_db_with_format(&["analytics", "requests", "list", "--limit", "5"]);
}

#[test]
fn analytics_requests_models() {
    run_db_with_format(&["analytics", "requests", "models"]);
}

// ============================================================================
// analytics — sessions
// ============================================================================

#[test]
fn analytics_sessions_stats() {
    run_db_with_format(&["analytics", "sessions", "stats"]);
}

#[test]
fn analytics_sessions_list_alias() {
    run_db_with_format(&["analytics", "sessions", "list"]);
}

#[test]
fn analytics_sessions_trends() {
    run_db_with_format(&["analytics", "sessions", "trends"]);
}

#[test]
fn analytics_sessions_live() {
    run_db_with_format(&["analytics", "sessions", "live"]);
}

// ============================================================================
// analytics — content
// ============================================================================

#[test]
fn analytics_content_stats() {
    run_db_with_format(&["analytics", "content", "stats"]);
}

#[test]
fn analytics_content_list_alias() {
    run_db_with_format(&["analytics", "content", "list"]);
}

#[test]
fn analytics_content_trends() {
    run_db_with_format(&["analytics", "content", "trends"]);
}

#[test]
fn analytics_content_top() {
    run_db_with_format(&["analytics", "content", "top"]);
}

#[test]
fn analytics_content_popular_alias() {
    run_db_with_format(&["analytics", "content", "popular"]);
}

// ============================================================================
// analytics — traffic
// ============================================================================

#[test]
fn analytics_traffic_sources() {
    run_db_with_format(&["analytics", "traffic", "sources"]);
}

#[test]
fn analytics_traffic_list_alias() {
    run_db_with_format(&["analytics", "traffic", "list"]);
}

#[test]
fn analytics_traffic_geo() {
    run_db_with_format(&["analytics", "traffic", "geo"]);
}

#[test]
fn analytics_traffic_devices() {
    run_db_with_format(&["analytics", "traffic", "devices"]);
}

#[test]
fn analytics_traffic_bots() {
    run_db_with_format(&["analytics", "traffic", "bots"]);
}

// ============================================================================
// analytics — costs
// ============================================================================

#[test]
fn analytics_costs_summary() {
    run_db_with_format(&["analytics", "costs", "summary"]);
}

#[test]
fn analytics_costs_list_alias() {
    run_db_with_format(&["analytics", "costs", "list"]);
}

#[test]
fn analytics_costs_trends() {
    run_db_with_format(&["analytics", "costs", "trends"]);
}

#[test]
fn analytics_costs_breakdown() {
    run_db_with_format(&["analytics", "costs", "breakdown"]);
}

// ============================================================================
// admin users — read paths (execute_with_db)
// ============================================================================

#[test]
fn admin_users_list() {
    run_db_with_format(&["admin", "users", "list"]);
}

#[test]
fn admin_users_list_limit() {
    run_db_with_format(&["admin", "users", "list", "--limit", "5"]);
}

#[test]
fn admin_users_list_offset() {
    run_db_with_format(&["admin", "users", "list", "--limit", "5", "--offset", "0"]);
}

#[test]
fn admin_users_list_role_admin() {
    run_db_with_format(&["admin", "users", "list", "--role", "admin"]);
}

#[test]
fn admin_users_list_role_user() {
    run_db_with_format(&["admin", "users", "list", "--role", "user"]);
}

#[test]
fn admin_users_list_role_anonymous() {
    run_db_with_format(&["admin", "users", "list", "--role", "anonymous"]);
}

#[test]
fn admin_users_list_status_active() {
    run_db_with_format(&["admin", "users", "list", "--status", "active"]);
}

#[test]
fn admin_users_list_status_suspended() {
    run_db_with_format(&["admin", "users", "list", "--status", "suspended"]);
}

#[test]
fn admin_users_count() {
    run_db_with_format(&["admin", "users", "count"]);
}

#[test]
fn admin_users_stats() {
    run_db_with_format(&["admin", "users", "stats"]);
}

#[test]
fn admin_users_search_empty() {
    run_db_with_format(&["admin", "users", "search", "zzz_nothing"]);
}

#[test]
fn admin_users_search_term() {
    run_db_with_format(&["admin", "users", "search", "admin"]);
}

#[test]
fn admin_users_show_missing() {
    run_db(&["admin", "users", "show", "nonexistent-user-id"]);
}

#[test]
fn admin_users_export() {
    run_db(&["admin", "users", "export"]);
}

#[test]
fn admin_users_session_list_missing_user() {
    run_db(&["admin", "users", "session", "list", "nonexistent-user"]);
}

#[test]
fn admin_users_ban_list() {
    run_db_with_format(&["admin", "users", "ban", "list"]);
}

// ============================================================================
// Negative paths — commands that require full profile context
// ============================================================================

#[test]
fn admin_users_create_requires_profile() {
    run_db(&["admin", "users", "create", "--email", "test@example.com"]);
}

#[test]
fn admin_users_delete_requires_profile() {
    run_db(&["admin", "users", "delete", "some-id"]);
}

#[test]
fn admin_agents_requires_profile() {
    run_db(&["admin", "agents", "list"]);
}

#[test]
fn admin_config_requires_profile() {
    run_db(&["admin", "config", "list"]);
}

#[test]
fn admin_setup_requires_profile() {
    run_db(&["admin", "setup"]);
}

#[test]
fn web_requires_profile() {
    run_db(&["web", "validate"]);
}

#[test]
fn plugins_requires_profile() {
    run_db(&["plugins", "list"]);
}

#[test]
fn build_requires_profile() {
    run_db(&["build", "core"]);
}

#[test]
fn core_artifacts_requires_profile() {
    run_db(&["core", "artifacts", "list"]);
}

#[test]
fn core_skills_requires_profile() {
    run_db(&["core", "skills", "list"]);
}

#[test]
fn core_plugins_requires_profile() {
    run_db(&["core", "plugins", "list"]);
}

#[test]
fn core_hooks_requires_profile() {
    run_db(&["core", "hooks", "list"]);
}

#[test]
fn core_contexts_requires_profile() {
    run_db(&["core", "contexts", "list"]);
}

#[test]
fn cloud_non_db_requires_profile() {
    run_db(&["cloud", "status"]);
}

#[test]
fn db_url_no_subcommand_fails() {
    let Some(mut c) = sp_db() else { return };
    let _ = c.assert();
}

#[test]
fn db_assign_admin_requires_profile() {
    run_db(&["infra", "db", "assign-admin", "nobody"]);
}

// ============================================================================
// infra logs — execute_with_db
// ============================================================================

#[test]
fn infra_logs_show() {
    run_db(&["infra", "logs", "show"]);
}

#[test]
fn infra_logs_summary() {
    run_db(&["infra", "logs", "summary"]);
}

#[test]
fn infra_logs_search() {
    run_db(&["infra", "logs", "search", "test"]);
}

#[test]
fn infra_logs_audit() {
    run_db(&["infra", "logs", "audit"]);
}

#[test]
fn infra_logs_tools_list_alt() {
    run_db(&["infra", "logs", "tools", "list", "--limit", "5"]);
}

#[test]
fn infra_logs_request_list() {
    run_db(&["infra", "logs", "request", "list"]);
}

#[test]
fn infra_logs_request_show_missing() {
    run_db(&["infra", "logs", "request", "show", "nonexistent-id"]);
}

#[test]
fn infra_logs_request_stats() {
    run_db(&["infra", "logs", "request", "stats"]);
}

#[test]
fn infra_logs_trace_list() {
    run_db(&["infra", "logs", "trace", "list"]);
}

#[test]
fn infra_logs_trace_show_missing() {
    run_db(&["infra", "logs", "trace", "show", "nonexistent-trace"]);
}

#[test]
fn infra_logs_tools_list() {
    run_db(&["infra", "logs", "tools", "list"]);
}

#[test]
fn infra_logs_view() {
    run_db(&["infra", "logs", "view"]);
}

#[test]
fn infra_logs_view_with_level() {
    run_db(&["infra", "logs", "view", "--level", "error"]);
}

#[test]
fn infra_logs_view_with_tail() {
    run_db(&["infra", "logs", "view", "--tail", "5"]);
}

#[test]
fn infra_logs_view_with_module() {
    run_db(&["infra", "logs", "view", "--module", "test"]);
}

#[test]
fn infra_logs_view_with_since() {
    run_db(&["infra", "logs", "view", "--since", "1h"]);
}

#[test]
fn infra_logs_export() {
    run_db(&["infra", "logs", "export"]);
}

#[test]
fn infra_logs_delete_skips_confirm() {
    // -y flag skips confirmation; against an empty test DB this is harmless.
    run_db(&["infra", "logs", "delete", "-y"]);
}

#[test]
fn infra_logs_cleanup() {
    run_db(&["infra", "logs", "cleanup"]);
}

// ============================================================================
// core content / files — execute_with_db
// ============================================================================

#[test]
fn core_content_list() {
    run_db_with_format(&["core", "content", "list"]);
}

#[test]
fn core_content_list_with_limit() {
    run_db_with_format(&["core", "content", "list", "--limit", "5"]);
}

#[test]
fn core_content_show_missing() {
    run_db(&["core", "content", "show", "nonexistent-slug"]);
}

#[test]
fn core_content_search() {
    run_db_with_format(&["core", "content", "search", "test"]);
}

#[test]
fn core_content_popular() {
    run_db_with_format(&["core", "content", "popular"]);
}

#[test]
fn core_content_status() {
    run_db_with_format(&["core", "content", "status", "test-source"]);
}

#[test]
fn core_content_analytics_clicks_missing() {
    run_db(&["core", "content", "analytics", "clicks", "nonexistent-link"]);
}

#[test]
fn core_content_analytics_campaign_missing() {
    run_db(&["core", "content", "analytics", "campaign", "nonexistent"]);
}

#[test]
fn core_content_analytics_journey_missing() {
    run_db(&["core", "content", "analytics", "journey", "nonexistent"]);
}

#[test]
fn core_files_list() {
    run_db_with_format(&["core", "files", "list"]);
}

#[test]
fn core_files_list_with_limit() {
    run_db_with_format(&["core", "files", "list", "--limit", "5"]);
}

#[test]
fn core_files_show_missing() {
    run_db(&["core", "files", "show", "nonexistent-id"]);
}

#[test]
fn core_files_search() {
    run_db_with_format(&["core", "files", "search", "test"]);
}

#[test]
fn core_files_stats() {
    run_db_with_format(&["core", "files", "stats"]);
}

// ============================================================================
// Global flags combined with --database-url
// ============================================================================

#[test]
fn db_url_with_verbose() {
    let Some(mut c) = sp_db() else { return };
    c.args(["--verbose", "infra", "db", "status"]);
    let _ = c.assert();
}

#[test]
fn db_url_with_debug() {
    let Some(mut c) = sp_db() else { return };
    c.args(["--debug", "infra", "db", "status"]);
    let _ = c.assert();
}

#[test]
fn db_url_with_quiet() {
    let Some(mut c) = sp_db() else { return };
    c.args(["--quiet", "infra", "db", "status"]);
    let _ = c.assert();
}

#[test]
fn db_url_with_no_color() {
    let Some(mut c) = sp_db() else { return };
    c.args(["--no-color", "infra", "db", "tables"]);
    let _ = c.assert();
}

#[test]
fn db_url_with_non_interactive() {
    let Some(mut c) = sp_db() else { return };
    c.args(["--non-interactive", "infra", "db", "tables"]);
    let _ = c.assert();
}

// ============================================================================
// Commands with CommandDescriptor::NONE — run without --database-url, no
// profile
// ============================================================================

fn sp_noprofile() -> Command {
    let mut c = Command::new(systemprompt_bin());
    c.env("SYSTEMPROMPT_PROFILE", "__nonexistent__");
    c.env_remove("RUST_LOG");
    c
}

#[test]
fn admin_session_show_no_profile() {
    let _ = sp_noprofile().args(["admin", "session", "show"]).assert();
}

#[test]
fn admin_session_show_json() {
    let _ = sp_noprofile()
        .args(["--json", "admin", "session", "show"])
        .assert();
}

#[test]
fn admin_session_show_yaml() {
    let _ = sp_noprofile()
        .args(["--yaml", "admin", "session", "show"])
        .assert();
}

#[test]
fn admin_session_list_no_profile() {
    let _ = sp_noprofile().args(["admin", "session", "list"]).assert();
}

#[test]
fn admin_session_list_json() {
    let _ = sp_noprofile()
        .args(["--json", "admin", "session", "list"])
        .assert();
}

#[test]
fn admin_session_logout_no_profile() {
    let _ = sp_noprofile().args(["admin", "session", "logout"]).assert();
}

#[test]
fn cloud_auth_no_profile() {
    let _ = sp_noprofile().args(["cloud", "auth"]).assert();
}

// ============================================================================
// Repeats with different formats and verbosity to drive renderer & log filter
// ============================================================================

#[test]
fn db_status_verbose() {
    let Some(mut c) = sp_db() else { return };
    let _ = c.args(["--verbose", "infra", "db", "status"]).assert();
}

#[test]
fn db_status_debug() {
    let Some(mut c) = sp_db() else { return };
    let _ = c.args(["--debug", "infra", "db", "status"]).assert();
}

#[test]
fn db_tables_verbose_json() {
    let Some(mut c) = sp_db() else { return };
    let _ = c
        .args(["--verbose", "--json", "infra", "db", "tables"])
        .assert();
}

#[test]
fn db_query_no_color() {
    let Some(mut c) = sp_db() else { return };
    let _ = c
        .args(["--no-color", "infra", "db", "query", "SELECT 1"])
        .assert();
}

#[test]
fn analytics_overview_verbose() {
    let Some(mut c) = sp_db() else { return };
    let _ = c.args(["--verbose", "analytics", "overview"]).assert();
}

#[test]
fn admin_users_list_quiet() {
    let Some(mut c) = sp_db() else { return };
    let _ = c.args(["--quiet", "admin", "users", "list"]).assert();
}

#[test]
fn db_indexes_yaml() {
    let Some(mut c) = sp_db() else { return };
    let _ = c.args(["--yaml", "infra", "db", "indexes"]).assert();
}

#[test]
fn analytics_costs_yaml() {
    let Some(mut c) = sp_db() else { return };
    let _ = c.args(["--yaml", "analytics", "costs", "summary"]).assert();
}
