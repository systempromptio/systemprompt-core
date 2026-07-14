//! CLI command path constants - prevents hardcoded strings from getting out of
//! sync.

#[derive(Debug, Clone, Copy)]
pub struct CliPaths;

impl CliPaths {
    pub const ADMIN: &'static str = "admin";
    pub const INFRA: &'static str = "infra";
    pub const AGENTS: &'static str = "agents";
    pub const DB: &'static str = "db";
    pub const SERVICES: &'static str = "services";
    pub const RUN: &'static str = "run";
    pub const MIGRATE: &'static str = "migrate";
    pub const SERVE: &'static str = "serve";

    pub const fn agent_run_args() -> [&'static str; 3] {
        [Self::ADMIN, Self::AGENTS, Self::RUN]
    }

    pub const fn db_migrate_args() -> [&'static str; 3] {
        [Self::INFRA, Self::DB, Self::MIGRATE]
    }

    pub const fn db_migrate_cmd() -> &'static str {
        "infra db migrate"
    }

    pub const fn services_serve_cmd() -> &'static str {
        "infra services serve"
    }

    pub const fn agent_run_cmd_pattern() -> &'static str {
        "admin agents run"
    }
}
