use std::borrow::Cow;

use super::types::{CliArgType, CliArgumentInfo, CliCommandInfo, ExecutionMode};


pub fn build_command_tree() -> CliCommandInfo {
    CliCommandInfo::new("systemprompt")
        .with_description("Agent orchestration and AI operations")
        .with_subcommands(vec![
            build_services_commands(),
            build_db_commands(),
            build_jobs_commands(),
            build_cloud_commands(),
            build_agents_commands(),
            build_mcp_commands(),
            build_logs_commands(),
            build_build_commands(),
            build_skills_commands(),
        ])
}

fn build_services_commands() -> CliCommandInfo {
    let path = vec![Cow::Borrowed("services")];

    CliCommandInfo::new("services")
        .with_path(path.clone())
        .with_description("Service lifecycle management (start, stop, status)")
        .with_subcommands(vec![
            CliCommandInfo::new("start")
                .with_path(extend_path(&path, "start"))
                .with_description("Start API, agents, and MCP servers")
                .with_execution_mode(ExecutionMode::Deterministic)
                .with_arguments(vec![
                    CliArgumentInfo::new("all")
                        .with_type(CliArgType::Bool)
                        .with_long("all")
                        .with_help("Start all services"),
                    CliArgumentInfo::new("api")
                        .with_type(CliArgType::Bool)
                        .with_long("api")
                        .with_help("Start API server only"),
                    CliArgumentInfo::new("agents")
                        .with_type(CliArgType::Bool)
                        .with_long("agents")
                        .with_help("Start agent services only"),
                    CliArgumentInfo::new("mcp")
                        .with_type(CliArgType::Bool)
                        .with_long("mcp")
                        .with_help("Start MCP servers only"),
                ]),
            CliCommandInfo::new("stop")
                .with_path(extend_path(&path, "stop"))
                .with_description("Stop running services gracefully")
                .with_execution_mode(ExecutionMode::Deterministic)
                .with_arguments(vec![CliArgumentInfo::new("all")
                    .with_type(CliArgType::Bool)
                    .with_long("all")
                    .with_help("Stop all services")]),
            CliCommandInfo::new("restart")
                .with_path(extend_path(&path, "restart"))
                .with_description("Restart services")
                .with_execution_mode(ExecutionMode::Deterministic),
            CliCommandInfo::new("status")
                .with_path(extend_path(&path, "status"))
                .with_description("Show service status")
                .with_execution_mode(ExecutionMode::Deterministic),
        ])
}

fn build_db_commands() -> CliCommandInfo {
    let path = vec![Cow::Borrowed("db")];

    CliCommandInfo::new("db")
        .with_path(path.clone())
        .with_description("Database operations and administration")
        .with_subcommands(vec![
            CliCommandInfo::new("migrate")
                .with_path(extend_path(&path, "migrate"))
                .with_description("Run database migrations")
                .with_execution_mode(ExecutionMode::Deterministic),
            CliCommandInfo::new("reset")
                .with_path(extend_path(&path, "reset"))
                .with_description("Reset database to clean state")
                .with_execution_mode(ExecutionMode::Deterministic),
            CliCommandInfo::new("seed")
                .with_path(extend_path(&path, "seed"))
                .with_description("Seed database with initial data")
                .with_execution_mode(ExecutionMode::Deterministic),
            CliCommandInfo::new("query")
                .with_path(extend_path(&path, "query"))
                .with_description("Execute SQL query")
                .with_execution_mode(ExecutionMode::AiAssisted)
                .with_arguments(vec![CliArgumentInfo::new("sql")
                    .with_type(CliArgType::String)
                    .with_required(true)
                    .with_help("SQL query to execute")]),
        ])
}

fn build_jobs_commands() -> CliCommandInfo {
    let path = vec![Cow::Borrowed("jobs")];

    CliCommandInfo::new("jobs")
        .with_path(path.clone())
        .with_description("Background jobs and scheduling")
        .with_subcommands(vec![
            CliCommandInfo::new("list")
                .with_path(extend_path(&path, "list"))
                .with_description("List scheduled jobs")
                .with_execution_mode(ExecutionMode::Deterministic),
            CliCommandInfo::new("run")
                .with_path(extend_path(&path, "run"))
                .with_description("Run a job immediately")
                .with_execution_mode(ExecutionMode::Deterministic)
                .with_arguments(vec![CliArgumentInfo::new("name")
                    .with_type(CliArgType::String)
                    .with_required(true)
                    .with_help("Job name to run")]),
        ])
}

fn build_cloud_commands() -> CliCommandInfo {
    let path = vec![Cow::Borrowed("cloud")];

    CliCommandInfo::new("cloud")
        .with_path(path.clone())
        .with_description("Cloud deployment, sync, and setup")
        .with_subcommands(vec![
            build_cloud_auth_commands(&path),
            build_cloud_tenant_commands(&path),
            build_cloud_profile_commands(&path),
            build_cloud_sync_commands(&path),
            CliCommandInfo::new("status")
                .with_path(extend_path(&path, "status"))
                .with_description("Show cloud connection status")
                .with_execution_mode(ExecutionMode::Deterministic),
            CliCommandInfo::new("deploy")
                .with_path(extend_path(&path, "deploy"))
                .with_description("Deploy to cloud")
                .with_execution_mode(ExecutionMode::Deterministic),
        ])
}

fn build_cloud_auth_commands(parent_path: &[Cow<'static, str>]) -> CliCommandInfo {
    let path = extend_path(parent_path, "auth");

    CliCommandInfo::new("auth")
        .with_path(path.clone())
        .with_description("Authentication commands")
        .with_subcommands(vec![
            CliCommandInfo::new("login")
                .with_path(extend_path(&path, "login"))
                .with_description("Login to cloud")
                .with_execution_mode(ExecutionMode::Deterministic),
            CliCommandInfo::new("logout")
                .with_path(extend_path(&path, "logout"))
                .with_description("Logout from cloud")
                .with_execution_mode(ExecutionMode::Deterministic),
            CliCommandInfo::new("whoami")
                .with_path(extend_path(&path, "whoami"))
                .with_description("Show current user")
                .with_execution_mode(ExecutionMode::Deterministic),
        ])
}

fn build_cloud_tenant_commands(parent_path: &[Cow<'static, str>]) -> CliCommandInfo {
    let path = extend_path(parent_path, "tenant");

    CliCommandInfo::new("tenant")
        .with_path(path.clone())
        .with_description("Tenant management")
        .with_subcommands(vec![
            CliCommandInfo::new("list")
                .with_path(extend_path(&path, "list"))
                .with_description("List tenants")
                .with_execution_mode(ExecutionMode::Deterministic),
            CliCommandInfo::new("select")
                .with_path(extend_path(&path, "select"))
                .with_description("Select active tenant")
                .with_execution_mode(ExecutionMode::Deterministic)
                .with_arguments(vec![CliArgumentInfo::new("tenant_id")
                    .with_type(CliArgType::String)
                    .with_help("Tenant ID to select")]),
        ])
}

fn build_cloud_profile_commands(parent_path: &[Cow<'static, str>]) -> CliCommandInfo {
    let path = extend_path(parent_path, "profile");

    CliCommandInfo::new("profile")
        .with_path(path.clone())
        .with_description("Profile management")
        .with_subcommands(vec![
            CliCommandInfo::new("list")
                .with_path(extend_path(&path, "list"))
                .with_description("List profiles")
                .with_execution_mode(ExecutionMode::Deterministic),
            CliCommandInfo::new("show")
                .with_path(extend_path(&path, "show"))
                .with_description("Show profile details")
                .with_execution_mode(ExecutionMode::Deterministic),
        ])
}

fn build_cloud_sync_commands(parent_path: &[Cow<'static, str>]) -> CliCommandInfo {
    let path = extend_path(parent_path, "sync");

    CliCommandInfo::new("sync")
        .with_path(path.clone())
        .with_description("Sync with cloud")
        .with_subcommands(vec![
            CliCommandInfo::new("push")
                .with_path(extend_path(&path, "push"))
                .with_description("Push local changes to cloud")
                .with_execution_mode(ExecutionMode::Deterministic),
            CliCommandInfo::new("pull")
                .with_path(extend_path(&path, "pull"))
                .with_description("Pull changes from cloud")
                .with_execution_mode(ExecutionMode::Deterministic),
        ])
}

fn build_agents_commands() -> CliCommandInfo {
    let path = vec![Cow::Borrowed("agents")];

    CliCommandInfo::new("agents")
        .with_path(path.clone())
        .with_description("Agent management")
        .with_subcommands(vec![
            CliCommandInfo::new("list")
                .with_path(extend_path(&path, "list"))
                .with_description("List all agents")
                .with_execution_mode(ExecutionMode::Deterministic),
            CliCommandInfo::new("show")
                .with_path(extend_path(&path, "show"))
                .with_description("Show agent details")
                .with_execution_mode(ExecutionMode::Deterministic)
                .with_arguments(vec![CliArgumentInfo::new("agent_id")
                    .with_type(CliArgType::String)
                    .with_required(true)
                    .with_help("Agent ID to show")]),
            CliCommandInfo::new("create")
                .with_path(extend_path(&path, "create"))
                .with_description("Create new agent")
                .with_execution_mode(ExecutionMode::AiAssisted)
                .with_arguments(vec![
                    CliArgumentInfo::new("name")
                        .with_type(CliArgType::String)
                        .with_required(true)
                        .with_help("Agent name"),
                    CliArgumentInfo::new("description")
                        .with_type(CliArgType::String)
                        .with_help("Agent description"),
                ]),
            CliCommandInfo::new("edit")
                .with_path(extend_path(&path, "edit"))
                .with_description("Edit agent configuration")
                .with_execution_mode(ExecutionMode::AiAssisted)
                .with_arguments(vec![CliArgumentInfo::new("agent_id")
                    .with_type(CliArgType::String)
                    .with_required(true)
                    .with_help("Agent ID to edit")]),
            CliCommandInfo::new("delete")
                .with_path(extend_path(&path, "delete"))
                .with_description("Delete an agent")
                .with_execution_mode(ExecutionMode::Deterministic)
                .with_arguments(vec![CliArgumentInfo::new("agent_id")
                    .with_type(CliArgType::String)
                    .with_required(true)
                    .with_help("Agent ID to delete")]),
            CliCommandInfo::new("status")
                .with_path(extend_path(&path, "status"))
                .with_description("Show agent status")
                .with_execution_mode(ExecutionMode::Deterministic),
        ])
}

fn build_mcp_commands() -> CliCommandInfo {
    let path = vec![Cow::Borrowed("mcp")];

    CliCommandInfo::new("mcp")
        .with_path(path.clone())
        .with_description("MCP server management")
        .with_subcommands(vec![
            CliCommandInfo::new("list")
                .with_path(extend_path(&path, "list"))
                .with_description("List MCP servers")
                .with_execution_mode(ExecutionMode::Deterministic),
            CliCommandInfo::new("status")
                .with_path(extend_path(&path, "status"))
                .with_description("Show MCP server status")
                .with_execution_mode(ExecutionMode::Deterministic),
            CliCommandInfo::new("validate")
                .with_path(extend_path(&path, "validate"))
                .with_description("Validate MCP configuration")
                .with_execution_mode(ExecutionMode::Deterministic),
            CliCommandInfo::new("logs")
                .with_path(extend_path(&path, "logs"))
                .with_description("View MCP server logs")
                .with_execution_mode(ExecutionMode::Deterministic)
                .with_arguments(vec![CliArgumentInfo::new("server")
                    .with_type(CliArgType::String)
                    .with_help("Server name to view logs for")]),
        ])
}

fn build_logs_commands() -> CliCommandInfo {
    let path = vec![Cow::Borrowed("logs")];

    CliCommandInfo::new("logs")
        .with_path(path.clone())
        .with_description("Log streaming and tracing")
        .with_subcommands(vec![
            CliCommandInfo::new("view")
                .with_path(extend_path(&path, "view"))
                .with_description("View log entries")
                .with_execution_mode(ExecutionMode::Deterministic)
                .with_arguments(vec![
                    CliArgumentInfo::new("level")
                        .with_type(CliArgType::String)
                        .with_long("level")
                        .with_help("Filter by log level")
                        .with_possible_values(vec![
                            Cow::Borrowed("error"),
                            Cow::Borrowed("warn"),
                            Cow::Borrowed("info"),
                            Cow::Borrowed("debug"),
                            Cow::Borrowed("trace"),
                        ]),
                    CliArgumentInfo::new("tail")
                        .with_type(CliArgType::Number)
                        .with_long("tail")
                        .with_short('n')
                        .with_default("20")
                        .with_help("Number of lines to show"),
                    CliArgumentInfo::new("since")
                        .with_type(CliArgType::String)
                        .with_long("since")
                        .with_help("Show logs since duration (e.g., 1h, 24h)"),
                ]),
            CliCommandInfo::new("search")
                .with_path(extend_path(&path, "search"))
                .with_description("Search log entries")
                .with_execution_mode(ExecutionMode::Deterministic)
                .with_arguments(vec![CliArgumentInfo::new("query")
                    .with_type(CliArgType::String)
                    .with_required(true)
                    .with_help("Search query")]),
            CliCommandInfo::new("stream")
                .with_path(extend_path(&path, "stream"))
                .with_description("Stream logs in real-time")
                .with_execution_mode(ExecutionMode::Deterministic),
            build_logs_trace_commands(&path),
        ])
}

fn build_logs_trace_commands(parent_path: &[Cow<'static, str>]) -> CliCommandInfo {
    let path = extend_path(parent_path, "trace");

    CliCommandInfo::new("trace")
        .with_path(path.clone())
        .with_description("Debug execution traces")
        .with_subcommands(vec![
            CliCommandInfo::new("list")
                .with_path(extend_path(&path, "list"))
                .with_description("List recent traces")
                .with_execution_mode(ExecutionMode::Deterministic)
                .with_arguments(vec![CliArgumentInfo::new("limit")
                    .with_type(CliArgType::Number)
                    .with_long("limit")
                    .with_default("10")
                    .with_help("Number of traces to show")]),
            CliCommandInfo::new("show")
                .with_path(extend_path(&path, "show"))
                .with_description("Show trace details")
                .with_execution_mode(ExecutionMode::Deterministic)
                .with_arguments(vec![CliArgumentInfo::new("trace_id")
                    .with_type(CliArgType::String)
                    .with_required(true)
                    .with_help("Trace ID to show")]),
        ])
}

fn build_build_commands() -> CliCommandInfo {
    let path = vec![Cow::Borrowed("build")];

    CliCommandInfo::new("build")
        .with_path(path.clone())
        .with_description("Build MCP extensions")
        .with_subcommands(vec![
            CliCommandInfo::new("core")
                .with_path(extend_path(&path, "core"))
                .with_description("Build core components")
                .with_execution_mode(ExecutionMode::Deterministic),
            CliCommandInfo::new("web")
                .with_path(extend_path(&path, "web"))
                .with_description("Build web components")
                .with_execution_mode(ExecutionMode::Deterministic),
            CliCommandInfo::new("mcp")
                .with_path(extend_path(&path, "mcp"))
                .with_description("Build MCP extensions")
                .with_execution_mode(ExecutionMode::Deterministic),
        ])
}

fn build_skills_commands() -> CliCommandInfo {
    let path = vec![Cow::Borrowed("skills")];

    CliCommandInfo::new("skills")
        .with_path(path.clone())
        .with_description("Skill management and database sync")
        .with_subcommands(vec![
            CliCommandInfo::new("list")
                .with_path(extend_path(&path, "list"))
                .with_description("List all skills")
                .with_execution_mode(ExecutionMode::Deterministic),
            CliCommandInfo::new("create")
                .with_path(extend_path(&path, "create"))
                .with_description("Create new skill")
                .with_execution_mode(ExecutionMode::AiAssisted)
                .with_arguments(vec![
                    CliArgumentInfo::new("name")
                        .with_type(CliArgType::String)
                        .with_required(true)
                        .with_help("Skill name"),
                    CliArgumentInfo::new("description")
                        .with_type(CliArgType::String)
                        .with_help("Skill description"),
                ]),
            CliCommandInfo::new("edit")
                .with_path(extend_path(&path, "edit"))
                .with_description("Edit skill")
                .with_execution_mode(ExecutionMode::AiAssisted)
                .with_arguments(vec![CliArgumentInfo::new("skill_id")
                    .with_type(CliArgType::String)
                    .with_required(true)
                    .with_help("Skill ID to edit")]),
            CliCommandInfo::new("delete")
                .with_path(extend_path(&path, "delete"))
                .with_description("Delete skill")
                .with_execution_mode(ExecutionMode::Deterministic)
                .with_arguments(vec![CliArgumentInfo::new("skill_id")
                    .with_type(CliArgType::String)
                    .with_required(true)
                    .with_help("Skill ID to delete")]),
            CliCommandInfo::new("sync")
                .with_path(extend_path(&path, "sync"))
                .with_description("Sync skills with database")
                .with_execution_mode(ExecutionMode::Deterministic),
        ])
}

fn extend_path(parent: &[Cow<'static, str>], child: &'static str) -> Vec<Cow<'static, str>> {
    let mut path = parent.to_vec();
    path.push(Cow::Borrowed(child));
    path
}

#[allow(dead_code)]
fn infer_arg_type_from_name(name: &str) -> CliArgType {
    match name {
        n if n.ends_with("_id") || n == "id" => CliArgType::String,
        n if n.ends_with("_path") || n == "path" || n == "file" => CliArgType::Path,
        n if n.starts_with("is_") || n.starts_with("has_") || n == "all" || n == "force" => {
            CliArgType::Bool
        },
        n if n == "limit" || n == "count" || n == "tail" || n == "offset" => CliArgType::Number,
        _ => CliArgType::String,
    }
}
