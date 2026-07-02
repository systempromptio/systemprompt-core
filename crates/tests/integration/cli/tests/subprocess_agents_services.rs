//! Subprocess coverage for the `admin agents` and `infra services` trees.
//!
//! The shared full-bootstrap fixture ships one disabled agent (`covagent`)
//! and one enabled external MCP server (`fixture_mcp`), so list/show/status/
//! validate/tools and the service lifecycle commands all operate on real
//! loader entries. Lifecycle mutations (create/edit/delete) run inside a
//! single test with a dedicated agent name so ordering is self-contained.
//!
//! Tests accept success or failure exit codes; the goal is driving handler
//! bodies under the instrumented binary.

use systemprompt_cli_integration_tests::full_bootstrap::{run, run_with_formats};

#[test]
fn agents_show_fixture() {
    run_with_formats(&["admin", "agents", "show", "covagent"]);
}

#[test]
fn agents_status_fixture() {
    run_with_formats(&["admin", "agents", "status", "covagent"]);
}

#[test]
fn agents_validate_fixture() {
    run_with_formats(&["admin", "agents", "validate", "covagent"]);
}

#[test]
fn agents_validate_all() {
    run(&["admin", "agents", "validate"]);
}

#[test]
fn agents_tools_fixture() {
    run_with_formats(&["admin", "agents", "tools", "covagent"]);
}

#[test]
fn agents_logs_fixture() {
    run(&["admin", "agents", "logs", "covagent"]);
    run(&["admin", "agents", "logs", "covagent", "--limit", "5"]);
}

#[test]
fn agents_registry_variants() {
    run_with_formats(&["admin", "agents", "registry"]);
    run(&["admin", "agents", "registry", "--running"]);
    run(&["admin", "agents", "registry", "--verbose"]);
}

#[test]
fn agents_task_trees() {
    run_with_formats(&["admin", "agents", "task", "list"]);
    run(&["admin", "agents", "task", "list", "--limit", "3"]);
    run(&["admin", "agents", "task", "show", "no-such-task"]);
}

#[test]
fn agents_lifecycle_create_edit_delete() {
    run(&[
        "admin",
        "agents",
        "create",
        "--name",
        "covcli_created_agent",
        "--display-name",
        "Created Agent",
        "--description",
        "lifecycle fixture",
        "--port",
        "4778",
    ]);
    run(&["admin", "agents", "show", "covcli_created_agent"]);
    run(&[
        "admin",
        "agents",
        "edit",
        "covcli_created_agent",
        "--disable",
    ]);
    run(&[
        "admin",
        "agents",
        "edit",
        "covcli_created_agent",
        "--set",
        "description=edited",
    ]);
    run(&["admin", "agents", "delete", "covcli_created_agent", "--yes"]);
}

#[test]
fn agents_create_invalid_port() {
    run(&[
        "admin",
        "agents",
        "create",
        "--name",
        "covcli_bad_agent",
        "--port",
        "99999",
    ]);
}

#[test]
fn agents_delete_missing_with_force() {
    run(&[
        "admin",
        "agents",
        "delete",
        "no_such_agent",
        "--yes",
        "--force",
    ]);
}

#[test]
fn services_status_variants() {
    run_with_formats(&["infra", "services", "status"]);
}

#[test]
fn services_stop_agent_fixture() {
    run(&["infra", "services", "stop", "agent", "covagent"]);
    run(&["infra", "services", "stop", "agent", "covagent", "--force"]);
}

#[test]
fn services_stop_mcp_fixture() {
    run(&["infra", "services", "stop", "mcp", "fixture_mcp"]);
}

#[test]
fn services_stop_groups() {
    run(&["infra", "services", "stop", "--agents"]);
    run(&["infra", "services", "stop", "--mcp"]);
    run(&["infra", "services", "stop", "--all"]);
}

#[test]
fn services_restart_failed_only() {
    run(&["infra", "services", "restart", "--failed"]);
}

#[test]
fn services_restart_agent_fixture() {
    run(&["infra", "services", "restart", "agent", "covagent"]);
}

#[test]
fn services_restart_mcp_fixture() {
    run(&["infra", "services", "restart", "mcp", "fixture_mcp"]);
}

#[test]
fn services_cleanup_confirmed() {
    run(&["infra", "services", "cleanup", "--yes"]);
}

#[test]
fn mcp_status_and_list() {
    run_with_formats(&["plugins", "mcp", "status"]);
    run_with_formats(&["plugins", "mcp", "list"]);
}

#[test]
fn mcp_validate_fixture() {
    run_with_formats(&["plugins", "mcp", "validate", "fixture_mcp"]);
    run(&["plugins", "mcp", "validate"]);
}

#[test]
fn mcp_logs_fixture() {
    run(&["plugins", "mcp", "logs", "fixture_mcp"]);
    run(&["plugins", "mcp", "logs", "fixture_mcp", "--limit", "5"]);
}

#[test]
fn mcp_tools_and_call() {
    run(&["plugins", "mcp", "tools"]);
    run(&[
        "plugins",
        "mcp",
        "call",
        "fixture_mcp",
        "no_such_tool",
        "--args",
        "{}",
        "--timeout",
        "3",
    ]);
}

#[test]
fn mcp_list_packages() {
    run(&["plugins", "mcp", "list-packages"]);
}
