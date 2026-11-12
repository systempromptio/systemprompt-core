//! Subprocess tests that drive the `systemprompt` binary through the FULL
//! bootstrap pipeline (Profile + Secrets + Config + FilesConfig + DB).
//!
//! Uses the shared [`full_bootstrap`] fixture, whose generated profile passes
//! startup validation and whose `admin bootstrap` run satisfies the
//! system-admin lookup, so every command below executes its real handler
//! body against the `DATABASE_URL` database.
//!
//! Each test drives one command family against one fixture, so a family pays
//! for one profile build and `admin bootstrap` run per process. Every command
//! line is printed before it runs so a failure or timeout names the invocation.
//!
//! Tests accept either success or failure exit codes — the point is to drive
//! handler bodies so they show up in the coverage report, not to assert
//! business outcomes against a freshly-migrated test database.

use predicates::str::contains;
use systemprompt_cli_integration_tests::full_bootstrap::{
    announce, cli_command, fixture_mcp_server, run, run_with_formats,
};

fn stdout_has(args: &[&str], needle: &str) {
    announce(&[], args);
    let mut cmd = cli_command();
    cmd.args(args);
    cmd.assert().success().stdout(contains(needle));
}

fn stderr_has(args: &[&str], needle: &str) {
    announce(&[], args);
    let mut cmd = cli_command();
    cmd.args(args);
    cmd.assert().success().stderr(contains(needle));
}

fn drive_formats(args: &[&str]) {
    for fmt in ["--json", "--yaml"] {
        announce(&[fmt], args);
        let mut cmd = cli_command();
        cmd.arg(fmt);
        cmd.args(args);
        let _ = cmd.assert();
    }
}

fn stdout_has_fmt(args: &[&str], needle: &str) {
    stdout_has(args, needle);
    drive_formats(args);
}

fn stderr_has_fmt(args: &[&str], needle: &str) {
    stderr_has(args, needle);
    drive_formats(args);
}

// Why: a list command over a freshly-migrated database prints its empty-state
// warning, not its table header, so asserting the header alone makes the test a
// function of whether earlier tests happened to leave rows behind. Accepting
// either still proves the handler ran to completion and produced its own
// output.
fn stderr_has_any(args: &[&str], needles: &[&str]) {
    announce(&[], args);
    let mut cmd = cli_command();
    cmd.args(args);
    let out = cmd.assert().success();
    let stderr = String::from_utf8_lossy(&out.get_output().stderr).into_owned();
    assert!(
        needles.iter().any(|n| stderr.contains(n)),
        "none of {needles:?} in stderr: {stderr}"
    );
    drive_formats(args);
}

fn fails_with(args: &[&str], needle: &str) {
    announce(&[], args);
    let mut cmd = cli_command();
    cmd.args(args);
    cmd.assert().failure().stderr(contains(needle));
}

#[test]
fn admin_agents_listing_and_lookups() {
    stdout_has_fmt(&["admin", "agents", "list"], "covagent");
    stdout_has(&["--verbose", "admin", "agents", "list"], "covagent");
    run(&["admin", "agents", "list", "--filter", "test"]);
    fails_with(
        &["admin", "agents", "show", "no-such-agent"],
        "Failed to show agent",
    );
    fails_with(
        &["admin", "agents", "status", "no-such-agent"],
        "Failed to get agent status",
    );
    fails_with(
        &["admin", "agents", "delete", "no-such-agent"],
        "Failed to delete agent",
    );
    fails_with(
        &["admin", "agents", "logs", "no-such-agent"],
        "Failed to get agent logs",
    );
    fails_with(
        &["admin", "agents", "tools", "no-such-agent"],
        "Failed to list agent tools",
    );
}

#[test]
fn admin_agents_registry_validate_and_tasks() {
    run(&["admin", "agents", "registry", "list"]);
    run(&["admin", "agents", "registry", "show", "no-such"]);
    fails_with(
        &["admin", "agents", "validate", "no-such-file.yaml"],
        "Failed to validate agents",
    );
    run(&["admin", "agents", "task", "list"]);
    run(&["admin", "agents", "task", "show", "no-such-task"]);
}

#[test]
fn admin_config_overview() {
    stderr_has_fmt(&["admin", "config", "list"], "Configuration Files");
    stdout_has_fmt(&["admin", "config", "show"], "subprocess_full");
    stderr_has_fmt(&["admin", "config", "validate"], "Validation Passed");
    run_with_formats(&["admin", "config", "paths"]);
}

#[test]
fn admin_config_sections() {
    run_with_formats(&["admin", "config", "provider"]);
    run_with_formats(&["admin", "config", "runtime"]);
    run_with_formats(&["admin", "config", "security"]);
    run_with_formats(&["admin", "config", "server"]);
}

#[test]
fn admin_config_rate_limits() {
    run_with_formats(&["admin", "config", "rate-limits"]);
    run(&["admin", "config", "rate-limits", "validate"]);
}

#[test]
fn admin_users_listing_and_counts() {
    // Scoped to admins with the page size raised well above the default 20:
    // every subprocess test shares one database and the unfiltered first page
    // fills with users other tests create (webauthn/grants/tx-oidc fixtures),
    // pushing the seeded `testadmin` row off it.
    stdout_has_fmt(
        &[
            "admin", "users", "list", "--role", "admin", "--limit", "500",
        ],
        "testadmin",
    );
    stderr_has_fmt(&["admin", "users", "count"], "User Count");
    stderr_has_fmt(&["admin", "users", "stats"], "User Statistics");
}

#[test]
fn admin_users_search_lookup_and_bans() {
    stderr_has_fmt(
        &["admin", "users", "search", "zzzzz"],
        "User Search Results",
    );
    fails_with(
        &["admin", "users", "show", "no-such-user"],
        "User not found",
    );
    stderr_has(&["admin", "users", "export"], "User Export");
    stderr_has_fmt(&["admin", "users", "ban", "list"], "Banned IPs");
}

#[test]
fn admin_access_control_and_keys() {
    run_with_formats(&["admin", "access-control", "list"]);
    run(&["admin", "access-control", "show", "no-such-rule"]);
    run(&["admin", "access-control", "validate"]);
    run_with_formats(&["admin", "keys", "list"]);
    run(&["admin", "keys", "show", "no-such-key"]);
    run(&["admin", "keys", "jwks"]);
}

#[test]
fn admin_setup_bridge_bootstrap_and_session() {
    run(&["admin", "setup", "status"]);
    run(&["--json", "admin", "setup", "status"]);
    run(&["admin", "bridge", "status"]);
    run(&["admin", "bridge", "list"]);
    run(&["admin", "bootstrap", "status"]);
    run(&["admin", "session", "show"]);
    run(&["admin", "session", "list"]);
}

#[test]
fn infra_services_and_jobs() {
    stderr_has_fmt(&["infra", "services", "status"], "Service Status");
    run(&["infra", "services", "status", "--all"]);
    stdout_has_fmt(&["infra", "jobs", "list"], "cleanup_anonymous_users");
    fails_with(&["infra", "jobs", "show", "no-such-job"], "Unknown job");
    run(&["infra", "jobs", "history"]);
}

#[test]
fn infra_db_connection_and_tables() {
    stderr_has_fmt(&["infra", "db", "status"], "Database connection");
    stderr_has_fmt(&["infra", "db", "info"], "Database: PostgreSQL");
    stdout_has_fmt(&["infra", "db", "tables"], "users");
    run_with_formats(&["infra", "db", "doctor"]);
}

#[test]
fn infra_db_objects() {
    stderr_has_fmt(&["infra", "db", "indexes"], "Indexes");
    stderr_has_fmt(&["infra", "db", "size"], "Database Size");
    stdout_has_fmt(&["infra", "db", "describe", "users"], "email");
    stderr_has_fmt(&["infra", "db", "count", "users"], "users:");
}

#[test]
fn infra_db_query_and_migrations() {
    stdout_has_fmt(&["infra", "db", "query", "SELECT 1 AS one"], "one");
    stderr_has_fmt(&["infra", "db", "migrate-status"], "Applied:");
    stderr_has_fmt(&["infra", "db", "migrate-plan"], "migration");
    stderr_has_fmt(&["infra", "db", "migrations", "status"], "applied");
}

#[test]
fn infra_logs_reads_then_cleanup() {
    run(&["infra", "logs", "show"]);
    run(&["infra", "logs", "summary"]);
    run(&["infra", "logs", "audit"]);
    run(&["infra", "logs", "search", "anything"]);
    run(&["infra", "logs", "view"]);
    run(&["infra", "logs", "request", "list"]);
    run(&["infra", "logs", "trace", "list"]);
    run(&["infra", "logs", "tools", "list"]);
    run(&["infra", "logs", "export"]);
    run(&["infra", "logs", "cleanup"]);
}

#[test]
fn cloud_commands_without_auth() {
    run(&["cloud", "auth"]);
    run(&["cloud", "status"]);
    run(&["cloud", "tenant", "list"]);
    run(&["cloud", "profile", "list"]);
    run(&["cloud", "secrets", "list"]);
    run(&["cloud", "dockerfile", "validate"]);
    run(&["cloud", "domain", "list"]);
}

#[test]
fn web_validate_and_listings() {
    stderr_has(&["web", "validate"], "Web Configuration Validation");
    stderr_has(&["web", "templates", "list"], "Templates");
    stderr_has(&["web", "content-types", "list"], "Content Types");
    stderr_has(&["web", "assets", "list"], "Assets");
}

#[test]
fn web_sitemap_generate_full() {
    run(&["web", "sitemap", "generate"]);
}

#[test]
fn plugins_and_mcp() {
    stdout_has_fmt(&["plugins", "list"], "Database");
    fails_with(
        &["plugins", "show", "no-such-plugin"],
        "Failed to show extension",
    );
    run(&["plugins", "validate", "no-such-plugin"]);
    run(&["plugins", "config", "show", "no-such-plugin"]);
    stderr_has(&["plugins", "capabilities"], "Capabilities Summary");
    stdout_has_fmt(&["plugins", "mcp", "list"], fixture_mcp_server());
    stderr_has(&["plugins", "mcp", "status"], "MCP Server Status");
    run(&["plugins", "mcp", "validate", "no-such"]);
}

#[test]
fn core_artifacts_and_contexts() {
    stderr_has_fmt(&["core", "artifacts", "list"], "Artifacts");
    fails_with(
        &["core", "artifacts", "show", "no-such-artifact"],
        "No artifact found",
    );
    stderr_has_fmt(&["core", "contexts", "list"], "Contexts");
    fails_with(
        &["core", "contexts", "show", "no-such-context"],
        "Context not found",
    );
}

#[test]
fn core_hooks_and_skills() {
    stderr_has_fmt(&["core", "hooks", "list"], "Hooks");
    run(&["core", "hooks", "show", "no-such-hook"]);
    stdout_has_fmt(&["core", "skills", "list"], "echo_skill");
    fails_with(
        &["core", "skills", "show", "no-such-skill"],
        "Failed to show skill",
    );
}

#[test]
fn core_plugins() {
    stderr_has_fmt(&["core", "plugins", "list"], "Plugins");
    fails_with(
        &["core", "plugins", "show", "no-such-plugin"],
        "Failed to show plugin",
    );
    fails_with(
        &["core", "plugins", "validate", "no-such-plugin"],
        "Failed to validate plugins",
    );
    run(&["core", "plugins", "generate", "--help"]);
}

#[test]
fn core_content() {
    stderr_has_fmt(&["core", "content", "list"], "Content");
    stderr_has_fmt(&["core", "content", "search", "anything"], "Search Results");
    run_with_formats(&["core", "content", "popular"]);
}

#[test]
fn core_files() {
    stderr_has_fmt(&["core", "files", "list"], "Files");
    stderr_has_fmt(
        &["core", "files", "search", "anything"],
        "File Search Results",
    );
    stderr_has_fmt(&["core", "files", "stats"], "File Storage Statistics");
}

#[test]
fn analytics_overview_and_conversations() {
    stderr_has_fmt(&["analytics", "overview"], "Analytics Overview");
    run(&["analytics", "overview", "--since", "24h"]);
    stderr_has_fmt(
        &["analytics", "conversations", "stats"],
        "Conversation Statistics",
    );
    stderr_has_any(
        &["analytics", "conversations", "list"],
        &["Conversations", "No conversations found"],
    );
}

#[test]
fn analytics_agents_and_tools() {
    stderr_has_fmt(&["analytics", "agents", "stats"], "Agent Statistics");
    run_with_formats(&["analytics", "agents", "list"]);
    stderr_has_fmt(&["analytics", "tools", "stats"], "Tool Statistics");
    run_with_formats(&["analytics", "tools", "list"]);
}

#[test]
fn analytics_requests_and_sessions() {
    stderr_has_fmt(&["analytics", "requests", "stats"], "AI Request Statistics");
    run_with_formats(&["analytics", "requests", "list"]);
    run_with_formats(&["analytics", "requests", "models"]);
    stderr_has_fmt(&["analytics", "sessions", "stats"], "Session Statistics");
    run(&["analytics", "sessions", "live"]);
}

#[test]
fn analytics_content_and_costs() {
    stderr_has_fmt(&["analytics", "content", "stats"], "Content Statistics");
    run_with_formats(&["analytics", "content", "top"]);
    stderr_has_fmt(&["analytics", "costs", "summary"], "Cost Summary");
    run_with_formats(&["analytics", "costs", "breakdown"]);
}

#[test]
fn analytics_traffic() {
    stderr_has_fmt(&["analytics", "traffic", "sources"], "Traffic Sources");
    stderr_has_fmt(&["analytics", "traffic", "geo"], "Geographic Distribution");
    stderr_has_fmt(&["analytics", "traffic", "devices"], "Device Breakdown");
    stderr_has_fmt(&["analytics", "traffic", "bots"], "Bot Traffic Analysis");
}

#[test]
fn global_flag_combinations() {
    run(&["--verbose", "admin", "config", "show"]);
    run(&["--debug", "admin", "config", "show"]);
    run(&["--quiet", "admin", "config", "show"]);
    run(&["--json", "infra", "db", "status"]);
    run(&["--yaml", "infra", "db", "status"]);
    run(&["--no-color", "admin", "users", "list"]);
    run(&["--non-interactive", "admin", "config", "paths"]);
}

#[test]
fn negative_paths_after_bootstrap() {
    run(&["admin", "no-such-cmd"]);
    fails_with(
        &["cloud", "tenant", "show", "no-such-tenant"],
        "Tenant not found",
    );
    fails_with(
        &["infra", "db", "describe", "no_such_table_xyz"],
        "not found",
    );
    fails_with(&["infra", "db", "count", "no_such_table_xyz"], "not found");
    fails_with(
        &["infra", "db", "query", "SELECT not valid sql !!"],
        "syntax error",
    );
    run(&["admin", "agents", "create", "no-such-file.yaml"]);
    fails_with(
        &["web", "templates", "show", "no-such-template"],
        "Failed to show template",
    );
    fails_with(
        &["web", "content-types", "show", "no-such"],
        "Failed to show content type",
    );
    fails_with(
        &["web", "assets", "show", "no-such-asset"],
        "Failed to show asset",
    );
}
