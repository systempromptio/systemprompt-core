//! CLI command path constants - prevents hardcoded strings from getting out of sync.

#[derive(Debug, Clone, Copy)]
pub struct CliPaths;

impl CliPaths {
    pub const ADMIN: &'static str = "admin";
    pub const INFRA: &'static str = "infra";
    pub const CORE: &'static str = "core";
    pub const PLUGINS: &'static str = "plugins";
    pub const CLOUD: &'static str = "cloud";
    pub const ANALYTICS: &'static str = "analytics";
    pub const WEB: &'static str = "web";
    pub const BUILD: &'static str = "build";

    pub const AGENTS: &'static str = "agents";
    pub const USERS: &'static str = "users";
    pub const CONFIG: &'static str = "config";
    pub const SETUP: &'static str = "setup";
    pub const SESSION: &'static str = "session";

    pub const DB: &'static str = "db";
    pub const JOBS: &'static str = "jobs";
    pub const LOGS: &'static str = "logs";
    pub const SERVICES: &'static str = "services";
    pub const SYSTEM: &'static str = "system";

    pub const CONTENT: &'static str = "content";
    pub const FILES: &'static str = "files";
    pub const CONTEXTS: &'static str = "contexts";
    pub const SKILLS: &'static str = "skills";

    pub const MCP: &'static str = "mcp";

    pub const RUN: &'static str = "run";
    pub const LIST: &'static str = "list";
    pub const SHOW: &'static str = "show";
    pub const START: &'static str = "start";
    pub const STOP: &'static str = "stop";
    pub const STATUS: &'static str = "status";
    pub const RESTART: &'static str = "restart";
    pub const MIGRATE: &'static str = "migrate";
    pub const SERVE: &'static str = "serve";

    pub fn agent_run_args() -> [&'static str; 3] {
        [Self::ADMIN, Self::AGENTS, Self::RUN]
    }

    pub fn db_migrate_args() -> [&'static str; 3] {
        [Self::INFRA, Self::DB, Self::MIGRATE]
    }

    pub fn services_serve_args() -> [&'static str; 3] {
        [Self::INFRA, Self::SERVICES, Self::SERVE]
    }

    pub fn infra_db_args(subcommand: &str) -> [&str; 3] {
        [Self::INFRA, Self::DB, subcommand]
    }

    pub fn infra_services_args(subcommand: &str) -> [&str; 3] {
        [Self::INFRA, Self::SERVICES, subcommand]
    }

    pub fn admin_agents_args(subcommand: &str) -> [&str; 3] {
        [Self::ADMIN, Self::AGENTS, subcommand]
    }

    pub fn plugins_mcp_args(subcommand: &str) -> [&str; 3] {
        [Self::PLUGINS, Self::MCP, subcommand]
    }
}
