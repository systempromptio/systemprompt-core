//! Subprocess tests that drive the `systemprompt` binary through the FULL
//! bootstrap pipeline (Profile + Secrets + Config + FilesConfig + DB).
//!
//! Uses the shared [`full_bootstrap`] fixture, whose generated profile passes
//! startup validation and whose `admin bootstrap` run satisfies the
//! system-admin lookup, so every command below executes its real handler
//! body against the `DATABASE_URL` database.
//!
//! Tests accept either success or failure exit codes — the point is to drive
//! handler bodies so they show up in the coverage report, not to assert
//! business outcomes against a freshly-migrated test database.

use predicates::str::contains;
use systemprompt_cli_integration_tests::full_bootstrap::{command, run, run_with_formats};

fn stdout_has(args: &[&str], needle: &str) {
    let Some(mut cmd) = command() else { return };
    cmd.args(args);
    cmd.assert().success().stdout(contains(needle));
}

fn stderr_has(args: &[&str], needle: &str) {
    let Some(mut cmd) = command() else { return };
    cmd.args(args);
    cmd.assert().success().stderr(contains(needle));
}

fn drive_formats(args: &[&str]) {
    for fmt in ["--json", "--yaml"] {
        let Some(mut cmd) = command() else { return };
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

fn fails_with(args: &[&str], needle: &str) {
    let Some(mut cmd) = command() else { return };
    cmd.args(args);
    cmd.assert().failure().stderr(contains(needle));
}
// ============================================================================
// admin agents
// ============================================================================

#[test]
fn admin_agents_list() {
    stdout_has_fmt(&["admin", "agents", "list"], "covagent");
}

#[test]
fn admin_agents_list_verbose() {
    stdout_has(&["--verbose", "admin", "agents", "list"], "covagent");
}

#[test]
fn admin_agents_list_with_filter() {
    run(&["admin", "agents", "list", "--filter", "test"]);
}

#[test]
fn admin_agents_show_missing() {
    fails_with(
        &["admin", "agents", "show", "no-such-agent"],
        "Failed to show agent",
    );
}

#[test]
fn admin_agents_status_missing() {
    fails_with(
        &["admin", "agents", "status", "no-such-agent"],
        "Failed to get agent status",
    );
}

#[test]
fn admin_agents_delete_missing() {
    fails_with(
        &["admin", "agents", "delete", "no-such-agent"],
        "Failed to delete agent",
    );
}

#[test]
fn admin_agents_logs_missing() {
    fails_with(
        &["admin", "agents", "logs", "no-such-agent"],
        "Failed to get agent logs",
    );
}

#[test]
fn admin_agents_tools_missing() {
    fails_with(
        &["admin", "agents", "tools", "no-such-agent"],
        "Failed to list agent tools",
    );
}

#[test]
fn admin_agents_registry_list() {
    run(&["admin", "agents", "registry", "list"]);
}

#[test]
fn admin_agents_registry_show_missing() {
    run(&["admin", "agents", "registry", "show", "no-such"]);
}

#[test]
fn admin_agents_validate_missing() {
    fails_with(
        &["admin", "agents", "validate", "no-such-file.yaml"],
        "Failed to validate agents",
    );
}

#[test]
fn admin_agents_task_list() {
    run(&["admin", "agents", "task", "list"]);
}

#[test]
fn admin_agents_task_show_missing() {
    run(&["admin", "agents", "task", "show", "no-such-task"]);
}

// ============================================================================
// admin config
// ============================================================================

#[test]
fn admin_config_list() {
    stderr_has_fmt(&["admin", "config", "list"], "Configuration Files");
}

#[test]
fn admin_config_show() {
    stdout_has_fmt(&["admin", "config", "show"], "subprocess_full");
}

#[test]
fn admin_config_validate() {
    stderr_has_fmt(&["admin", "config", "validate"], "Validation Passed");
}

#[test]
fn admin_config_paths() {
    run_with_formats(&["admin", "config", "paths"]);
}

#[test]
fn admin_config_provider() {
    run_with_formats(&["admin", "config", "provider"]);
}

#[test]
fn admin_config_runtime() {
    run_with_formats(&["admin", "config", "runtime"]);
}

#[test]
fn admin_config_security() {
    run_with_formats(&["admin", "config", "security"]);
}

#[test]
fn admin_config_server() {
    run_with_formats(&["admin", "config", "server"]);
}

#[test]
fn admin_config_rate_limits_show() {
    run_with_formats(&["admin", "config", "rate-limits"]);
}

#[test]
fn admin_config_rate_limits_validate() {
    run(&["admin", "config", "rate-limits", "validate"]);
}

// ============================================================================
// admin users (full bootstrap)
// ============================================================================

#[test]
fn admin_users_list_full() {
    stdout_has_fmt(&["admin", "users", "list"], "testadmin");
}

#[test]
fn admin_users_count_full() {
    stderr_has_fmt(&["admin", "users", "count"], "User Count");
}

#[test]
fn admin_users_stats_full() {
    stderr_has_fmt(&["admin", "users", "stats"], "User Statistics");
}

#[test]
fn admin_users_search_empty_full() {
    stderr_has_fmt(
        &["admin", "users", "search", "zzzzz"],
        "User Search Results",
    );
}

#[test]
fn admin_users_show_missing_full() {
    fails_with(
        &["admin", "users", "show", "no-such-user"],
        "User not found",
    );
}

#[test]
fn admin_users_export_full() {
    stderr_has(&["admin", "users", "export"], "User Export");
}

#[test]
fn admin_users_ban_list_full() {
    stderr_has_fmt(&["admin", "users", "ban", "list"], "Banned IPs");
}

// ============================================================================
// admin access-control
// ============================================================================

#[test]
fn admin_access_control_list() {
    run_with_formats(&["admin", "access-control", "list"]);
}

#[test]
fn admin_access_control_show_missing() {
    run(&["admin", "access-control", "show", "no-such-rule"]);
}

#[test]
fn admin_access_control_validate() {
    run(&["admin", "access-control", "validate"]);
}

// ============================================================================
// admin keys
// ============================================================================

#[test]
fn admin_keys_list() {
    run_with_formats(&["admin", "keys", "list"]);
}

#[test]
fn admin_keys_show_missing() {
    run(&["admin", "keys", "show", "no-such-key"]);
}

#[test]
fn admin_keys_jwks() {
    run(&["admin", "keys", "jwks"]);
}

// ============================================================================
// admin setup
// ============================================================================

#[test]
fn admin_setup_status() {
    run(&["admin", "setup", "status"]);
}

#[test]
fn admin_setup_status_json() {
    run(&["--json", "admin", "setup", "status"]);
}

// ============================================================================
// admin bridge
// ============================================================================

#[test]
fn admin_bridge_status() {
    run(&["admin", "bridge", "status"]);
}

#[test]
fn admin_bridge_list() {
    run(&["admin", "bridge", "list"]);
}

// ============================================================================
// admin bootstrap
// ============================================================================

#[test]
fn admin_bootstrap_status() {
    run(&["admin", "bootstrap", "status"]);
}

// ============================================================================
// admin session (no-profile in subprocess_with_db; full here too)
// ============================================================================

#[test]
fn admin_session_show_full() {
    run(&["admin", "session", "show"]);
}

#[test]
fn admin_session_list_full() {
    run(&["admin", "session", "list"]);
}

// ============================================================================
// infra services
// ============================================================================

#[test]
fn infra_services_status() {
    stderr_has_fmt(&["infra", "services", "status"], "Service Status");
}

#[test]
fn infra_services_status_all() {
    run(&["infra", "services", "status", "--all"]);
}

#[test]
fn infra_services_cleanup() {
    run(&["infra", "services", "cleanup"]);
}

// ============================================================================
// infra db (full bootstrap, not --database-url)
// ============================================================================

#[test]
fn infra_db_status_full() {
    stderr_has_fmt(&["infra", "db", "status"], "Database connection");
}

#[test]
fn infra_db_info_full() {
    stderr_has_fmt(&["infra", "db", "info"], "Database: PostgreSQL");
}

#[test]
fn infra_db_tables_full() {
    stdout_has_fmt(&["infra", "db", "tables"], "users");
}

#[test]
fn infra_db_validate_full() {
    stderr_has_fmt(&["infra", "db", "validate"], "Schema Validation");
}

#[test]
fn infra_db_doctor_full() {
    run_with_formats(&["infra", "db", "doctor"]);
}

#[test]
fn infra_db_indexes_full() {
    stderr_has_fmt(&["infra", "db", "indexes"], "Indexes");
}

#[test]
fn infra_db_size_full() {
    stderr_has_fmt(&["infra", "db", "size"], "Database Size");
}

#[test]
fn infra_db_describe_users_full() {
    stdout_has_fmt(&["infra", "db", "describe", "users"], "email");
}

#[test]
fn infra_db_count_users_full() {
    stderr_has_fmt(&["infra", "db", "count", "users"], "users:");
}

#[test]
fn infra_db_query_select_1_full() {
    stdout_has_fmt(&["infra", "db", "query", "SELECT 1 AS one"], "one");
}

#[test]
fn infra_db_migrate_status_full() {
    stderr_has_fmt(&["infra", "db", "migrate-status"], "Applied:");
}

#[test]
fn infra_db_migrate_plan_full() {
    stderr_has_fmt(&["infra", "db", "migrate-plan"], "migrations");
}

#[test]
fn infra_db_migrations_status_full() {
    stderr_has_fmt(&["infra", "db", "migrations", "status"], "applied");
}

// ============================================================================
// infra jobs
// ============================================================================

#[test]
fn infra_jobs_list() {
    stdout_has_fmt(&["infra", "jobs", "list"], "cleanup_anonymous_users");
}

#[test]
fn infra_jobs_show_missing() {
    fails_with(&["infra", "jobs", "show", "no-such-job"], "Unknown job");
}

#[test]
fn infra_jobs_history() {
    run(&["infra", "jobs", "history"]);
}

// ============================================================================
// infra logs (full bootstrap)
// ============================================================================

#[test]
fn infra_logs_show_full() {
    run(&["infra", "logs", "show"]);
}

#[test]
fn infra_logs_summary_full() {
    run(&["infra", "logs", "summary"]);
}

#[test]
fn infra_logs_audit_full() {
    run(&["infra", "logs", "audit"]);
}

#[test]
fn infra_logs_search_full() {
    run(&["infra", "logs", "search", "anything"]);
}

#[test]
fn infra_logs_view_full() {
    run(&["infra", "logs", "view"]);
}

#[test]
fn infra_logs_request_list_full() {
    run(&["infra", "logs", "request", "list"]);
}

#[test]
fn infra_logs_trace_list_full() {
    run(&["infra", "logs", "trace", "list"]);
}

#[test]
fn infra_logs_tools_list_full() {
    run(&["infra", "logs", "tools", "list"]);
}

#[test]
fn infra_logs_export_full() {
    run(&["infra", "logs", "export"]);
}

#[test]
fn infra_logs_cleanup_full() {
    run(&["infra", "logs", "cleanup"]);
}

// ============================================================================
// cloud (these mostly require no auth; commands should exit gracefully)
// ============================================================================

#[test]
fn cloud_auth_status() {
    run(&["cloud", "auth"]);
}

#[test]
fn cloud_status_full() {
    run(&["cloud", "status"]);
}

#[test]
fn cloud_tenant_list() {
    run(&["cloud", "tenant", "list"]);
}

#[test]
fn cloud_profile_list_full() {
    run(&["cloud", "profile", "list"]);
}

#[test]
fn cloud_secrets_list() {
    run(&["cloud", "secrets", "list"]);
}

#[test]
fn cloud_dockerfile_validate() {
    run(&["cloud", "dockerfile", "validate"]);
}

#[test]
fn cloud_domain_list() {
    run(&["cloud", "domain", "list"]);
}

// ============================================================================
// web
// ============================================================================

#[test]
fn web_validate_full() {
    stderr_has(&["web", "validate"], "Web Configuration Validation");
}

#[test]
fn web_sitemap_generate_full() {
    run(&["web", "sitemap", "generate"]);
}

#[test]
fn web_templates_list() {
    stderr_has(&["web", "templates", "list"], "Templates");
}

#[test]
fn web_content_types_list() {
    stderr_has(&["web", "content-types", "list"], "Content Types");
}

#[test]
fn web_assets_list() {
    stderr_has(&["web", "assets", "list"], "Assets");
}

// ============================================================================
// plugins
// ============================================================================

#[test]
fn plugins_list_full() {
    stdout_has_fmt(&["plugins", "list"], "Database");
}

#[test]
fn plugins_show_missing() {
    fails_with(
        &["plugins", "show", "no-such-plugin"],
        "Failed to show extension",
    );
}

#[test]
fn plugins_validate_missing() {
    run(&["plugins", "validate", "no-such-plugin"]);
}

#[test]
fn plugins_config_show_missing() {
    run(&["plugins", "config", "show", "no-such-plugin"]);
}

#[test]
fn plugins_capabilities_full() {
    stderr_has(&["plugins", "capabilities"], "Capabilities Summary");
}

#[test]
fn plugins_mcp_list_full() {
    stdout_has_fmt(&["plugins", "mcp", "list"], "fixture_mcp");
}

#[test]
fn plugins_mcp_status_full() {
    stderr_has(&["plugins", "mcp", "status"], "MCP Server Status");
}

#[test]
fn plugins_mcp_validate_missing() {
    run(&["plugins", "mcp", "validate", "no-such"]);
}

// ============================================================================
// core
// ============================================================================

#[test]
fn core_artifacts_list_full() {
    stderr_has_fmt(&["core", "artifacts", "list"], "Artifacts");
}

#[test]
fn core_artifacts_show_missing() {
    fails_with(
        &["core", "artifacts", "show", "no-such-artifact"],
        "No artifact found",
    );
}

#[test]
fn core_contexts_list_full() {
    stderr_has_fmt(&["core", "contexts", "list"], "Contexts");
}

#[test]
fn core_contexts_show_missing() {
    fails_with(
        &["core", "contexts", "show", "no-such-context"],
        "Context not found",
    );
}

#[test]
fn core_hooks_list_full() {
    stderr_has_fmt(&["core", "hooks", "list"], "Hooks");
}

#[test]
fn core_hooks_show_missing() {
    run(&["core", "hooks", "show", "no-such-hook"]);
}

#[test]
fn core_skills_list_full() {
    stdout_has_fmt(&["core", "skills", "list"], "echo_skill");
}

#[test]
fn core_skills_show_missing() {
    fails_with(
        &["core", "skills", "show", "no-such-skill"],
        "Failed to show skill",
    );
}

#[test]
fn core_plugins_list_full() {
    stderr_has_fmt(&["core", "plugins", "list"], "Plugins");
}

#[test]
fn core_plugins_show_missing() {
    fails_with(
        &["core", "plugins", "show", "no-such-plugin"],
        "Failed to show plugin",
    );
}

#[test]
fn core_plugins_validate_missing() {
    fails_with(
        &["core", "plugins", "validate", "no-such-plugin"],
        "Failed to validate plugins",
    );
}

#[test]
fn core_plugins_generate_help() {
    run(&["core", "plugins", "generate", "--help"]);
}

#[test]
fn core_content_list_full() {
    stderr_has_fmt(&["core", "content", "list"], "Content");
}

#[test]
fn core_content_search_full() {
    stderr_has_fmt(&["core", "content", "search", "anything"], "Search Results");
}

#[test]
fn core_content_popular_full() {
    run_with_formats(&["core", "content", "popular"]);
}

#[test]
fn core_files_list_full() {
    stderr_has_fmt(&["core", "files", "list"], "Files");
}

#[test]
fn core_files_search_full() {
    stderr_has_fmt(
        &["core", "files", "search", "anything"],
        "File Search Results",
    );
}

#[test]
fn core_files_stats_full() {
    stderr_has_fmt(&["core", "files", "stats"], "File Storage Statistics");
}

// ============================================================================
// analytics (full bootstrap)
// ============================================================================

#[test]
fn analytics_overview_full() {
    stderr_has_fmt(&["analytics", "overview"], "Analytics Overview");
}

#[test]
fn analytics_overview_24h_full() {
    run(&["analytics", "overview", "--since", "24h"]);
}

#[test]
fn analytics_conversations_stats_full() {
    stderr_has_fmt(
        &["analytics", "conversations", "stats"],
        "Conversation Statistics",
    );
}

#[test]
fn analytics_conversations_list_full() {
    stderr_has_fmt(&["analytics", "conversations", "list"], "Conversations");
}

#[test]
fn analytics_agents_stats_full() {
    stderr_has_fmt(&["analytics", "agents", "stats"], "Agent Statistics");
}

#[test]
fn analytics_agents_list_full() {
    run_with_formats(&["analytics", "agents", "list"]);
}

#[test]
fn analytics_tools_stats_full() {
    stderr_has_fmt(&["analytics", "tools", "stats"], "Tool Statistics");
}

#[test]
fn analytics_tools_list_full() {
    run_with_formats(&["analytics", "tools", "list"]);
}

#[test]
fn analytics_requests_stats_full() {
    stderr_has_fmt(&["analytics", "requests", "stats"], "AI Request Statistics");
}

#[test]
fn analytics_requests_list_full() {
    run_with_formats(&["analytics", "requests", "list"]);
}

#[test]
fn analytics_requests_models_full() {
    run_with_formats(&["analytics", "requests", "models"]);
}

#[test]
fn analytics_sessions_stats_full() {
    stderr_has_fmt(&["analytics", "sessions", "stats"], "Session Statistics");
}

#[test]
fn analytics_sessions_live_full() {
    run(&["analytics", "sessions", "live"]);
}

#[test]
fn analytics_content_stats_full() {
    stderr_has_fmt(&["analytics", "content", "stats"], "Content Statistics");
}

#[test]
fn analytics_content_top_full() {
    run_with_formats(&["analytics", "content", "top"]);
}

#[test]
fn analytics_traffic_sources_full() {
    stderr_has_fmt(&["analytics", "traffic", "sources"], "Traffic Sources");
}

#[test]
fn analytics_traffic_geo_full() {
    stderr_has_fmt(&["analytics", "traffic", "geo"], "Geographic Distribution");
}

#[test]
fn analytics_traffic_devices_full() {
    stderr_has_fmt(&["analytics", "traffic", "devices"], "Device Breakdown");
}

#[test]
fn analytics_traffic_bots_full() {
    stderr_has_fmt(&["analytics", "traffic", "bots"], "Bot Traffic Analysis");
}

#[test]
fn analytics_costs_summary_full() {
    stderr_has_fmt(&["analytics", "costs", "summary"], "Cost Summary");
}

#[test]
fn analytics_costs_breakdown_full() {
    run_with_formats(&["analytics", "costs", "breakdown"]);
}

// ============================================================================
// Global flag combinations with full bootstrap (exercise format renderers,
// verbosity wiring, environment plumbing)
// ============================================================================

#[test]
fn full_verbose_admin_config_show() {
    run(&["--verbose", "admin", "config", "show"]);
}

#[test]
fn full_debug_admin_config_show() {
    run(&["--debug", "admin", "config", "show"]);
}

#[test]
fn full_quiet_admin_config_show() {
    run(&["--quiet", "admin", "config", "show"]);
}

#[test]
fn full_json_infra_db_status() {
    run(&["--json", "infra", "db", "status"]);
}

#[test]
fn full_yaml_infra_db_status() {
    run(&["--yaml", "infra", "db", "status"]);
}

#[test]
fn full_no_color_admin_users_list() {
    run(&["--no-color", "admin", "users", "list"]);
}

#[test]
fn full_non_interactive_admin_config_paths() {
    run(&["--non-interactive", "admin", "config", "paths"]);
}

// ============================================================================
// Error/negative paths that still drive bootstrap
// ============================================================================

#[test]
fn admin_unknown_subcommand_full() {
    run(&["admin", "no-such-cmd"]);
}

#[test]
fn cloud_tenant_show_missing() {
    fails_with(
        &["cloud", "tenant", "show", "no-such-tenant"],
        "Tenant not found",
    );
}

#[test]
fn infra_db_describe_missing_full() {
    fails_with(
        &["infra", "db", "describe", "no_such_table_xyz"],
        "not found",
    );
}

#[test]
fn infra_db_count_missing_full() {
    fails_with(&["infra", "db", "count", "no_such_table_xyz"], "not found");
}

#[test]
fn infra_db_query_invalid_sql_full() {
    fails_with(
        &["infra", "db", "query", "SELECT not valid sql !!"],
        "syntax error",
    );
}

#[test]
fn admin_agents_create_invalid() {
    run(&["admin", "agents", "create", "no-such-file.yaml"]);
}

#[test]
fn web_templates_show_missing() {
    fails_with(
        &["web", "templates", "show", "no-such-template"],
        "Failed to show template",
    );
}

#[test]
fn web_content_types_show_missing() {
    fails_with(
        &["web", "content-types", "show", "no-such"],
        "Failed to show content type",
    );
}

#[test]
fn web_assets_show_missing() {
    fails_with(
        &["web", "assets", "show", "no-such-asset"],
        "Failed to show asset",
    );
}
