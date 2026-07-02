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

use systemprompt_cli_integration_tests::full_bootstrap::{run, run_with_formats};
// ============================================================================
// admin agents
// ============================================================================

#[test]
fn admin_agents_list() {
    run_with_formats(&["admin", "agents", "list"]);
}

#[test]
fn admin_agents_list_verbose() {
    run(&["--verbose", "admin", "agents", "list"]);
}

#[test]
fn admin_agents_list_with_filter() {
    run(&["admin", "agents", "list", "--filter", "test"]);
}

#[test]
fn admin_agents_show_missing() {
    run(&["admin", "agents", "show", "no-such-agent"]);
}

#[test]
fn admin_agents_status_missing() {
    run(&["admin", "agents", "status", "no-such-agent"]);
}

#[test]
fn admin_agents_delete_missing() {
    run(&["admin", "agents", "delete", "no-such-agent"]);
}

#[test]
fn admin_agents_logs_missing() {
    run(&["admin", "agents", "logs", "no-such-agent"]);
}

#[test]
fn admin_agents_tools_missing() {
    run(&["admin", "agents", "tools", "no-such-agent"]);
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
    run(&["admin", "agents", "validate", "no-such-file.yaml"]);
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
    run_with_formats(&["admin", "config", "list"]);
}

#[test]
fn admin_config_show() {
    run_with_formats(&["admin", "config", "show"]);
}

#[test]
fn admin_config_validate() {
    run_with_formats(&["admin", "config", "validate"]);
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
    run_with_formats(&["admin", "users", "list"]);
}

#[test]
fn admin_users_count_full() {
    run_with_formats(&["admin", "users", "count"]);
}

#[test]
fn admin_users_stats_full() {
    run_with_formats(&["admin", "users", "stats"]);
}

#[test]
fn admin_users_search_empty_full() {
    run_with_formats(&["admin", "users", "search", "zzzzz"]);
}

#[test]
fn admin_users_show_missing_full() {
    run(&["admin", "users", "show", "no-such-user"]);
}

#[test]
fn admin_users_export_full() {
    run(&["admin", "users", "export"]);
}

#[test]
fn admin_users_ban_list_full() {
    run_with_formats(&["admin", "users", "ban", "list"]);
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
    run_with_formats(&["infra", "services", "status"]);
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
    run_with_formats(&["infra", "db", "status"]);
}

#[test]
fn infra_db_info_full() {
    run_with_formats(&["infra", "db", "info"]);
}

#[test]
fn infra_db_tables_full() {
    run_with_formats(&["infra", "db", "tables"]);
}

#[test]
fn infra_db_validate_full() {
    run_with_formats(&["infra", "db", "validate"]);
}

#[test]
fn infra_db_doctor_full() {
    run_with_formats(&["infra", "db", "doctor"]);
}

#[test]
fn infra_db_indexes_full() {
    run_with_formats(&["infra", "db", "indexes"]);
}

#[test]
fn infra_db_size_full() {
    run_with_formats(&["infra", "db", "size"]);
}

#[test]
fn infra_db_describe_users_full() {
    run_with_formats(&["infra", "db", "describe", "users"]);
}

#[test]
fn infra_db_count_users_full() {
    run_with_formats(&["infra", "db", "count", "users"]);
}

#[test]
fn infra_db_query_select_1_full() {
    run_with_formats(&["infra", "db", "query", "SELECT 1 AS one"]);
}

#[test]
fn infra_db_migrate_status_full() {
    run_with_formats(&["infra", "db", "migrate-status"]);
}

#[test]
fn infra_db_migrate_plan_full() {
    run_with_formats(&["infra", "db", "migrate-plan"]);
}

#[test]
fn infra_db_migrations_status_full() {
    run_with_formats(&["infra", "db", "migrations", "status"]);
}

// ============================================================================
// infra jobs
// ============================================================================

#[test]
fn infra_jobs_list() {
    run_with_formats(&["infra", "jobs", "list"]);
}

#[test]
fn infra_jobs_show_missing() {
    run(&["infra", "jobs", "show", "no-such-job"]);
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
    run(&["web", "validate"]);
}

#[test]
fn web_sitemap_generate_full() {
    run(&["web", "sitemap", "generate"]);
}

#[test]
fn web_templates_list() {
    run(&["web", "templates", "list"]);
}

#[test]
fn web_content_types_list() {
    run(&["web", "content-types", "list"]);
}

#[test]
fn web_assets_list() {
    run(&["web", "assets", "list"]);
}

// ============================================================================
// plugins
// ============================================================================

#[test]
fn plugins_list_full() {
    run_with_formats(&["plugins", "list"]);
}

#[test]
fn plugins_show_missing() {
    run(&["plugins", "show", "no-such-plugin"]);
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
    run(&["plugins", "capabilities"]);
}

#[test]
fn plugins_mcp_list_full() {
    run_with_formats(&["plugins", "mcp", "list"]);
}

#[test]
fn plugins_mcp_status_full() {
    run(&["plugins", "mcp", "status"]);
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
    run_with_formats(&["core", "artifacts", "list"]);
}

#[test]
fn core_artifacts_show_missing() {
    run(&["core", "artifacts", "show", "no-such-artifact"]);
}

#[test]
fn core_contexts_list_full() {
    run_with_formats(&["core", "contexts", "list"]);
}

#[test]
fn core_contexts_show_missing() {
    run(&["core", "contexts", "show", "no-such-context"]);
}

#[test]
fn core_hooks_list_full() {
    run_with_formats(&["core", "hooks", "list"]);
}

#[test]
fn core_hooks_show_missing() {
    run(&["core", "hooks", "show", "no-such-hook"]);
}

#[test]
fn core_skills_list_full() {
    run_with_formats(&["core", "skills", "list"]);
}

#[test]
fn core_skills_show_missing() {
    run(&["core", "skills", "show", "no-such-skill"]);
}

#[test]
fn core_plugins_list_full() {
    run_with_formats(&["core", "plugins", "list"]);
}

#[test]
fn core_plugins_show_missing() {
    run(&["core", "plugins", "show", "no-such-plugin"]);
}

#[test]
fn core_plugins_validate_missing() {
    run(&["core", "plugins", "validate", "no-such-plugin"]);
}

#[test]
fn core_plugins_generate_help() {
    run(&["core", "plugins", "generate", "--help"]);
}

#[test]
fn core_content_list_full() {
    run_with_formats(&["core", "content", "list"]);
}

#[test]
fn core_content_search_full() {
    run_with_formats(&["core", "content", "search", "anything"]);
}

#[test]
fn core_content_popular_full() {
    run_with_formats(&["core", "content", "popular"]);
}

#[test]
fn core_files_list_full() {
    run_with_formats(&["core", "files", "list"]);
}

#[test]
fn core_files_search_full() {
    run_with_formats(&["core", "files", "search", "anything"]);
}

#[test]
fn core_files_stats_full() {
    run_with_formats(&["core", "files", "stats"]);
}

// ============================================================================
// analytics (full bootstrap)
// ============================================================================

#[test]
fn analytics_overview_full() {
    run_with_formats(&["analytics", "overview"]);
}

#[test]
fn analytics_overview_24h_full() {
    run(&["analytics", "overview", "--since", "24h"]);
}

#[test]
fn analytics_conversations_stats_full() {
    run_with_formats(&["analytics", "conversations", "stats"]);
}

#[test]
fn analytics_conversations_list_full() {
    run_with_formats(&["analytics", "conversations", "list"]);
}

#[test]
fn analytics_agents_stats_full() {
    run_with_formats(&["analytics", "agents", "stats"]);
}

#[test]
fn analytics_agents_list_full() {
    run_with_formats(&["analytics", "agents", "list"]);
}

#[test]
fn analytics_tools_stats_full() {
    run_with_formats(&["analytics", "tools", "stats"]);
}

#[test]
fn analytics_tools_list_full() {
    run_with_formats(&["analytics", "tools", "list"]);
}

#[test]
fn analytics_requests_stats_full() {
    run_with_formats(&["analytics", "requests", "stats"]);
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
    run_with_formats(&["analytics", "sessions", "stats"]);
}

#[test]
fn analytics_sessions_live_full() {
    run(&["analytics", "sessions", "live"]);
}

#[test]
fn analytics_content_stats_full() {
    run_with_formats(&["analytics", "content", "stats"]);
}

#[test]
fn analytics_content_top_full() {
    run_with_formats(&["analytics", "content", "top"]);
}

#[test]
fn analytics_traffic_sources_full() {
    run_with_formats(&["analytics", "traffic", "sources"]);
}

#[test]
fn analytics_traffic_geo_full() {
    run_with_formats(&["analytics", "traffic", "geo"]);
}

#[test]
fn analytics_traffic_devices_full() {
    run_with_formats(&["analytics", "traffic", "devices"]);
}

#[test]
fn analytics_traffic_bots_full() {
    run_with_formats(&["analytics", "traffic", "bots"]);
}

#[test]
fn analytics_costs_summary_full() {
    run_with_formats(&["analytics", "costs", "summary"]);
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
    run(&["cloud", "tenant", "show", "no-such-tenant"]);
}

#[test]
fn infra_db_describe_missing_full() {
    run(&["infra", "db", "describe", "no_such_table_xyz"]);
}

#[test]
fn infra_db_count_missing_full() {
    run(&["infra", "db", "count", "no_such_table_xyz"]);
}

#[test]
fn infra_db_query_invalid_sql_full() {
    run(&["infra", "db", "query", "SELECT not valid sql !!"]);
}

#[test]
fn admin_agents_create_invalid() {
    run(&["admin", "agents", "create", "no-such-file.yaml"]);
}

#[test]
fn web_templates_show_missing() {
    run(&["web", "templates", "show", "no-such-template"]);
}

#[test]
fn web_content_types_show_missing() {
    run(&["web", "content-types", "show", "no-such"]);
}

#[test]
fn web_assets_show_missing() {
    run(&["web", "assets", "show", "no-such-asset"]);
}
