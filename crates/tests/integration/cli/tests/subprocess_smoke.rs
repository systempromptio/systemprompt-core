//! Subprocess tests that invoke the `systemprompt` binary via `assert_cmd`.
//!
//! These exist so that the cli's instrumented binary actually executes under
//! `just coverage` and contributes line counts. Each `Command::cargo_bin`
//! invocation inherits `LLVM_PROFILE_FILE` from the parent test process; the
//! child writes its own profraw which the merge step folds into the report.
//!
//! The bulk of these tests walk the help tree -- every leaf subcommand is
//! exercised with `--help` so clap derive plumbing (subcommand registration,
//! arg parsing, derived `Display` impls, descriptor matching) gets
//! instrumented even without a live profile or database.

use assert_cmd::Command;
use predicates::str::contains;

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

fn sp() -> Command {
    let mut c = Command::new(systemprompt_bin());
    c.env("SYSTEMPROMPT_PROFILE", "__nonexistent__");
    c.env_remove("RUST_LOG");
    c
}

fn assert_help_ok(args: &[&str]) {
    let mut cmd = sp();
    cmd.args(args).arg("--help");
    cmd.assert().success();
}

// ---------- top-level ----------

#[test]
fn version_flag_prints_crate_version() {
    sp().arg("--version")
        .assert()
        .success()
        .stdout(contains("systemprompt"));
}

#[test]
fn version_short_flag() {
    sp().arg("-V").assert().success();
}

#[test]
fn help_flag_lists_top_level_commands() {
    sp().arg("--help")
        .assert()
        .success()
        .stdout(contains("admin"))
        .stdout(contains("cloud"))
        .stdout(contains("infra"))
        .stdout(contains("core"))
        .stdout(contains("analytics"))
        .stdout(contains("web"))
        .stdout(contains("plugins"))
        .stdout(contains("build"));
}

#[test]
fn help_short_flag() {
    sp().arg("-h").assert().success();
}

#[test]
fn unknown_subcommand_exits_nonzero() {
    sp().arg("definitely-not-a-real-command").assert().failure();
}

#[test]
fn unknown_flag_exits_nonzero() {
    sp().arg("--this-flag-does-not-exist").assert().failure();
}

#[test]
fn empty_arg_fails_gracefully() {
    sp().arg("").assert().failure();
}

// ---------- top-level help for each domain ----------

#[test]
fn admin_help() {
    assert_help_ok(&["admin"]);
}
#[test]
fn cloud_help() {
    assert_help_ok(&["cloud"]);
}
#[test]
fn infra_help() {
    assert_help_ok(&["infra"]);
}
#[test]
fn core_help() {
    assert_help_ok(&["core"]);
}
#[test]
fn analytics_help() {
    assert_help_ok(&["analytics"]);
}
#[test]
fn web_help() {
    assert_help_ok(&["web"]);
}
#[test]
fn plugins_help() {
    assert_help_ok(&["plugins"]);
}
#[test]
fn build_help() {
    assert_help_ok(&["build"]);
}

// ---------- admin ----------

#[test]
fn admin_users_help() {
    assert_help_ok(&["admin", "users"]);
}
#[test]
fn admin_users_list_help() {
    assert_help_ok(&["admin", "users", "list"]);
}
#[test]
fn admin_users_create_help() {
    assert_help_ok(&["admin", "users", "create"]);
}
#[test]
fn admin_users_delete_help() {
    assert_help_ok(&["admin", "users", "delete"]);
}
#[test]
fn admin_users_count_help() {
    assert_help_ok(&["admin", "users", "count"]);
}
#[test]
fn admin_users_ban_help() {
    assert_help_ok(&["admin", "users", "ban"]);
}
#[test]
fn admin_users_role_help() {
    assert_help_ok(&["admin", "users", "role"]);
}
#[test]
fn admin_users_export_help() {
    assert_help_ok(&["admin", "users", "export"]);
}
#[test]
fn admin_users_merge_help() {
    assert_help_ok(&["admin", "users", "merge"]);
}
#[test]
fn admin_users_bulk_help() {
    assert_help_ok(&["admin", "users", "bulk"]);
}

#[test]
fn admin_agents_help() {
    assert_help_ok(&["admin", "agents"]);
}
#[test]
fn admin_agents_list_help() {
    assert_help_ok(&["admin", "agents", "list"]);
}
#[test]
fn admin_agents_create_help() {
    assert_help_ok(&["admin", "agents", "create"]);
}
#[test]
fn admin_agents_delete_help() {
    assert_help_ok(&["admin", "agents", "delete"]);
}
#[test]
fn admin_agents_show_help() {
    assert_help_ok(&["admin", "agents", "show"]);
}
#[test]
fn admin_agents_edit_help() {
    assert_help_ok(&["admin", "agents", "edit"]);
}
#[test]
fn admin_agents_status_help() {
    assert_help_ok(&["admin", "agents", "status"]);
}
#[test]
fn admin_agents_logs_help() {
    assert_help_ok(&["admin", "agents", "logs"]);
}
#[test]
fn admin_agents_run_help() {
    assert_help_ok(&["admin", "agents", "run"]);
}
#[test]
fn admin_agents_message_help() {
    assert_help_ok(&["admin", "agents", "message"]);
}
#[test]
fn admin_agents_task_help() {
    assert_help_ok(&["admin", "agents", "task"]);
}
#[test]
fn admin_agents_tools_help() {
    assert_help_ok(&["admin", "agents", "tools"]);
}
#[test]
fn admin_agents_registry_help() {
    assert_help_ok(&["admin", "agents", "registry"]);
}
#[test]
fn admin_agents_validate_help() {
    assert_help_ok(&["admin", "agents", "validate"]);
}

#[test]
fn admin_config_help() {
    assert_help_ok(&["admin", "config"]);
}
#[test]
fn admin_config_list_help() {
    assert_help_ok(&["admin", "config", "list"]);
}
#[test]
fn admin_config_show_help() {
    assert_help_ok(&["admin", "config", "show"]);
}
#[test]
fn admin_config_validate_help() {
    assert_help_ok(&["admin", "config", "validate"]);
}
#[test]
fn admin_config_paths_help() {
    assert_help_ok(&["admin", "config", "paths"]);
}
#[test]
fn admin_config_provider_help() {
    assert_help_ok(&["admin", "config", "provider"]);
}
#[test]
fn admin_config_runtime_help() {
    assert_help_ok(&["admin", "config", "runtime"]);
}
#[test]
fn admin_config_security_help() {
    assert_help_ok(&["admin", "config", "security"]);
}
#[test]
fn admin_config_server_help() {
    assert_help_ok(&["admin", "config", "server"]);
}
#[test]
fn admin_config_rate_limits_help() {
    assert_help_ok(&["admin", "config", "rate-limits"]);
}

#[test]
fn admin_session_help() {
    assert_help_ok(&["admin", "session"]);
}
#[test]
fn admin_session_list_help() {
    assert_help_ok(&["admin", "session", "list"]);
}
#[test]
fn admin_session_login_help() {
    assert_help_ok(&["admin", "session", "login"]);
}
#[test]
fn admin_session_logout_help() {
    assert_help_ok(&["admin", "session", "logout"]);
}
#[test]
fn admin_session_show_help() {
    assert_help_ok(&["admin", "session", "show"]);
}
#[test]
fn admin_session_switch_help() {
    assert_help_ok(&["admin", "session", "switch"]);
}

#[test]
fn admin_setup_help() {
    assert_help_ok(&["admin", "setup"]);
}
#[test]
fn admin_bootstrap_help() {
    assert_help_ok(&["admin", "bootstrap"]);
}
#[test]
fn admin_bridge_help() {
    assert_help_ok(&["admin", "bridge"]);
}
#[test]
fn admin_access_control_help() {
    assert_help_ok(&["admin", "access-control"]);
}
#[test]
fn admin_keys_help() {
    assert_help_ok(&["admin", "keys"]);
}

// ---------- cloud ----------

#[test]
fn cloud_auth_help() {
    assert_help_ok(&["cloud", "auth"]);
}
#[test]
fn cloud_init_help() {
    assert_help_ok(&["cloud", "init"]);
}
#[test]
fn cloud_tenant_help() {
    assert_help_ok(&["cloud", "tenant"]);
}
#[test]
fn cloud_profile_help() {
    assert_help_ok(&["cloud", "profile"]);
}
#[test]
fn cloud_deploy_help() {
    assert_help_ok(&["cloud", "deploy"]);
}
#[test]
fn cloud_status_help() {
    assert_help_ok(&["cloud", "status"]);
}
#[test]
fn cloud_restart_help() {
    assert_help_ok(&["cloud", "restart"]);
}
#[test]
fn cloud_sync_help() {
    assert_help_ok(&["cloud", "sync"]);
}
#[test]
fn cloud_secrets_help() {
    assert_help_ok(&["cloud", "secrets"]);
}
#[test]
fn cloud_dockerfile_help() {
    assert_help_ok(&["cloud", "dockerfile"]);
}
#[test]
fn cloud_db_help() {
    assert_help_ok(&["cloud", "db"]);
}
#[test]
fn cloud_domain_help() {
    assert_help_ok(&["cloud", "domain"]);
}

// ---------- infra ----------

#[test]
fn infra_services_help() {
    assert_help_ok(&["infra", "services"]);
}
#[test]
fn infra_db_help() {
    assert_help_ok(&["infra", "db"]);
}
#[test]
fn infra_jobs_help() {
    assert_help_ok(&["infra", "jobs"]);
}
#[test]
fn infra_logs_help() {
    assert_help_ok(&["infra", "logs"]);
}

// ---------- core ----------

#[test]
fn core_artifacts_help() {
    assert_help_ok(&["core", "artifacts"]);
}
#[test]
fn core_content_help() {
    assert_help_ok(&["core", "content"]);
}
#[test]
fn core_contexts_help() {
    assert_help_ok(&["core", "contexts"]);
}
#[test]
fn core_files_help() {
    assert_help_ok(&["core", "files"]);
}
#[test]
fn core_hooks_help() {
    assert_help_ok(&["core", "hooks"]);
}
#[test]
fn core_plugins_help() {
    assert_help_ok(&["core", "plugins"]);
}
#[test]
fn core_skills_help() {
    assert_help_ok(&["core", "skills"]);
}

// ---------- analytics ----------

#[test]
fn analytics_agents_help() {
    assert_help_ok(&["analytics", "agents"]);
}
#[test]
fn analytics_content_help() {
    assert_help_ok(&["analytics", "content"]);
}
#[test]
fn analytics_conversations_help() {
    assert_help_ok(&["analytics", "conversations"]);
}
#[test]
fn analytics_costs_help() {
    assert_help_ok(&["analytics", "costs"]);
}
#[test]
fn analytics_overview_help() {
    assert_help_ok(&["analytics", "overview"]);
}
#[test]
fn analytics_requests_help() {
    assert_help_ok(&["analytics", "requests"]);
}
#[test]
fn analytics_sessions_help() {
    assert_help_ok(&["analytics", "sessions"]);
}
#[test]
fn analytics_tools_help() {
    assert_help_ok(&["analytics", "tools"]);
}
#[test]
fn analytics_traffic_help() {
    assert_help_ok(&["analytics", "traffic"]);
}

// ---------- web ----------

#[test]
fn web_assets_help() {
    assert_help_ok(&["web", "assets"]);
}
#[test]
fn web_content_types_help() {
    assert_help_ok(&["web", "content-types"]);
}
#[test]
fn web_sitemap_help() {
    assert_help_ok(&["web", "sitemap"]);
}
#[test]
fn web_templates_help() {
    assert_help_ok(&["web", "templates"]);
}
#[test]
fn web_validate_help() {
    assert_help_ok(&["web", "validate"]);
}

// ---------- plugins ----------

#[test]
fn plugins_list_help() {
    assert_help_ok(&["plugins", "list"]);
}
#[test]
fn plugins_show_help() {
    assert_help_ok(&["plugins", "show"]);
}
#[test]
fn plugins_run_help() {
    assert_help_ok(&["plugins", "run"]);
}
#[test]
fn plugins_validate_help() {
    assert_help_ok(&["plugins", "validate"]);
}
#[test]
fn plugins_config_help() {
    assert_help_ok(&["plugins", "config"]);
}
#[test]
fn plugins_capabilities_help() {
    assert_help_ok(&["plugins", "capabilities"]);
}
#[test]
fn plugins_mcp_help() {
    assert_help_ok(&["plugins", "mcp"]);
}
// ---------- build ----------

#[test]
fn build_core_help() {
    assert_help_ok(&["build", "core"]);
}
#[test]
fn build_mcp_help() {
    assert_help_ok(&["build", "mcp"]);
}

// ---------- global flags interleaved ----------

#[test]
fn json_flag_with_help() {
    sp().args(["--json", "--help"]).assert().success();
}

#[test]
fn yaml_flag_with_help() {
    sp().args(["--yaml", "--help"]).assert().success();
}

#[test]
fn verbose_flag_with_help() {
    sp().args(["--verbose", "--help"]).assert().success();
}

#[test]
fn quiet_flag_with_help() {
    sp().args(["--quiet", "--help"]).assert().success();
}

#[test]
fn debug_flag_with_help() {
    sp().args(["--debug", "--help"]).assert().success();
}

#[test]
fn no_color_flag_with_help() {
    sp().args(["--no-color", "--help"]).assert().success();
}

#[test]
fn non_interactive_flag_with_help() {
    sp().args(["--non-interactive", "--help"])
        .assert()
        .success();
}

#[test]
fn profile_flag_with_help() {
    sp().args(["--profile", "local", "--help"])
        .assert()
        .success();
}

#[test]
fn json_and_yaml_conflict() {
    // clap should reject the conflict
    sp().args(["--json", "--yaml", "admin", "users", "list"])
        .assert()
        .failure();
}

#[test]
fn verbose_and_quiet_conflict() {
    sp().args(["--verbose", "--quiet", "admin", "users", "list"])
        .assert()
        .failure();
}

// ---------- malformed subcommand paths ----------

#[test]
fn admin_unknown_subcommand_fails() {
    sp().args(["admin", "does-not-exist"]).assert().failure();
}

#[test]
fn cloud_unknown_subcommand_fails() {
    sp().args(["cloud", "does-not-exist"]).assert().failure();
}

#[test]
fn infra_unknown_subcommand_fails() {
    sp().args(["infra", "does-not-exist"]).assert().failure();
}

#[test]
fn core_unknown_subcommand_fails() {
    sp().args(["core", "does-not-exist"]).assert().failure();
}

#[test]
fn analytics_unknown_subcommand_fails() {
    sp().args(["analytics", "does-not-exist"])
        .assert()
        .failure();
}

#[test]
fn web_unknown_subcommand_fails() {
    sp().args(["web", "does-not-exist"]).assert().failure();
}

#[test]
fn plugins_unknown_subcommand_fails() {
    sp().args(["plugins", "does-not-exist"]).assert().failure();
}

#[test]
fn build_unknown_subcommand_fails() {
    sp().args(["build", "does-not-exist"]).assert().failure();
}

// ---------- deeper infra subtree ----------

#[test]
fn infra_services_start_help() {
    assert_help_ok(&["infra", "services", "start"]);
}
#[test]
fn infra_services_stop_help() {
    assert_help_ok(&["infra", "services", "stop"]);
}
#[test]
fn infra_services_status_help() {
    assert_help_ok(&["infra", "services", "status"]);
}
#[test]
fn infra_services_restart_help() {
    assert_help_ok(&["infra", "services", "restart"]);
}
#[test]
fn infra_services_serve_help() {
    assert_help_ok(&["infra", "services", "serve"]);
}
#[test]
fn infra_services_cleanup_help() {
    assert_help_ok(&["infra", "services", "cleanup"]);
}
#[test]
fn infra_db_query_help() {
    assert_help_ok(&["infra", "db", "query"]);
}
#[test]
fn infra_db_execute_help() {
    assert_help_ok(&["infra", "db", "execute"]);
}
#[test]
fn infra_db_tables_help() {
    assert_help_ok(&["infra", "db", "tables"]);
}
#[test]
fn infra_db_describe_help() {
    assert_help_ok(&["infra", "db", "describe"]);
}
#[test]
fn infra_db_info_help() {
    assert_help_ok(&["infra", "db", "info"]);
}
#[test]
fn infra_db_migrate_help() {
    assert_help_ok(&["infra", "db", "migrate"]);
}
#[test]
fn infra_db_migrate_down_help() {
    assert_help_ok(&["infra", "db", "migrate-down"]);
}
#[test]
fn infra_db_migrate_status_help() {
    assert_help_ok(&["infra", "db", "migrate-status"]);
}
#[test]
fn infra_db_migrate_plan_help() {
    assert_help_ok(&["infra", "db", "migrate-plan"]);
}
#[test]
fn infra_db_migrate_repair_help() {
    assert_help_ok(&["infra", "db", "migrate-repair"]);
}
#[test]
fn infra_db_migrations_help() {
    assert_help_ok(&["infra", "db", "migrations"]);
}
#[test]
fn infra_db_status_help() {
    assert_help_ok(&["infra", "db", "status"]);
}
#[test]
fn infra_db_validate_help() {
    assert_help_ok(&["infra", "db", "validate"]);
}
#[test]
fn infra_db_count_help() {
    assert_help_ok(&["infra", "db", "count"]);
}
#[test]
fn infra_db_indexes_help() {
    assert_help_ok(&["infra", "db", "indexes"]);
}
#[test]
fn infra_db_size_help() {
    assert_help_ok(&["infra", "db", "size"]);
}
#[test]
fn infra_db_doctor_help() {
    assert_help_ok(&["infra", "db", "doctor"]);
}

#[test]
fn infra_jobs_list_help() {
    assert_help_ok(&["infra", "jobs", "list"]);
}
#[test]
fn infra_jobs_show_help() {
    assert_help_ok(&["infra", "jobs", "show"]);
}
#[test]
fn infra_jobs_run_help() {
    assert_help_ok(&["infra", "jobs", "run"]);
}
#[test]
fn infra_jobs_history_help() {
    assert_help_ok(&["infra", "jobs", "history"]);
}
#[test]
fn infra_jobs_enable_help() {
    assert_help_ok(&["infra", "jobs", "enable"]);
}
#[test]
fn infra_jobs_disable_help() {
    assert_help_ok(&["infra", "jobs", "disable"]);
}

#[test]
fn infra_logs_show_help() {
    assert_help_ok(&["infra", "logs", "show"]);
}
#[test]
fn infra_logs_search_help() {
    assert_help_ok(&["infra", "logs", "search"]);
}
#[test]
fn infra_logs_stream_help() {
    assert_help_ok(&["infra", "logs", "stream"]);
}
#[test]
fn infra_logs_view_help() {
    assert_help_ok(&["infra", "logs", "view"]);
}
#[test]
fn infra_logs_summary_help() {
    assert_help_ok(&["infra", "logs", "summary"]);
}
#[test]
fn infra_logs_export_help() {
    assert_help_ok(&["infra", "logs", "export"]);
}
#[test]
fn infra_logs_delete_help() {
    assert_help_ok(&["infra", "logs", "delete"]);
}
#[test]
fn infra_logs_cleanup_help() {
    assert_help_ok(&["infra", "logs", "cleanup"]);
}
#[test]
fn infra_logs_request_help() {
    assert_help_ok(&["infra", "logs", "request"]);
}
#[test]
fn infra_logs_trace_help() {
    assert_help_ok(&["infra", "logs", "trace"]);
}
#[test]
fn infra_logs_tools_help() {
    assert_help_ok(&["infra", "logs", "tools"]);
}
#[test]
fn infra_logs_audit_help() {
    assert_help_ok(&["infra", "logs", "audit"]);
}

// ---------- cloud subtree ----------

#[test]
fn cloud_tenant_create_help() {
    assert_help_ok(&["cloud", "tenant", "create"]);
}
#[test]
fn cloud_tenant_delete_help() {
    assert_help_ok(&["cloud", "tenant", "delete"]);
}
#[test]
fn cloud_tenant_cancel_help() {
    assert_help_ok(&["cloud", "tenant", "cancel"]);
}
#[test]
fn cloud_tenant_rotate_credentials_help() {
    assert_help_ok(&["cloud", "tenant", "rotate-credentials"]);
}

#[test]
fn cloud_profile_create_help() {
    assert_help_ok(&["cloud", "profile", "create"]);
}
#[test]
fn cloud_profile_list_help() {
    assert_help_ok(&["cloud", "profile", "list"]);
}
#[test]
fn cloud_profile_edit_help() {
    assert_help_ok(&["cloud", "profile", "edit"]);
}

#[test]
fn cloud_auth_login_help() {
    assert_help_ok(&["cloud", "auth", "login"]);
}

// ---------- core subtree ----------

#[test]
fn core_skills_list_help() {
    assert_help_ok(&["core", "skills", "list"]);
}
#[test]
fn core_skills_show_help() {
    assert_help_ok(&["core", "skills", "show"]);
}

#[test]
fn core_files_show_help() {
    assert_help_ok(&["core", "files", "show"]);
}

#[test]
fn core_plugins_validate_help() {
    assert_help_ok(&["core", "plugins", "validate"]);
}
#[test]
fn core_plugins_generate_help() {
    assert_help_ok(&["core", "plugins", "generate"]);
}

// ---------- admin sub-subtree ----------

#[test]
fn admin_config_rate_limits_set_help() {
    assert_help_ok(&["admin", "config", "rate-limits", "set"]);
}
#[test]
fn admin_config_rate_limits_validate_help() {
    assert_help_ok(&["admin", "config", "rate-limits", "validate"]);
}

#[test]
fn admin_users_bulk_help_explicit() {
    assert_help_ok(&["admin", "users", "bulk"]);
}

// ---------- plugins mcp subtree ----------

#[test]
fn plugins_mcp_list_help() {
    assert_help_ok(&["plugins", "mcp", "list"]);
}
#[test]
fn plugins_mcp_status_help() {
    assert_help_ok(&["plugins", "mcp", "status"]);
}
#[test]
fn plugins_mcp_validate_help() {
    assert_help_ok(&["plugins", "mcp", "validate"]);
}

// ---------- analytics deep ----------

#[test]
fn analytics_sessions_live_help() {
    assert_help_ok(&["analytics", "sessions", "live"]);
}
#[test]
fn analytics_agents_show_help() {
    assert_help_ok(&["analytics", "agents", "show"]);
}
#[test]
fn analytics_tools_show_help() {
    assert_help_ok(&["analytics", "tools", "show"]);
}

// ---------- web deep ----------

#[test]
fn web_templates_create_help() {
    assert_help_ok(&["web", "templates", "create"]);
}
#[test]
fn web_templates_edit_help() {
    assert_help_ok(&["web", "templates", "edit"]);
}
#[test]
fn web_sitemap_generate_help() {
    assert_help_ok(&["web", "sitemap", "generate"]);
}
