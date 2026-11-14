//! Spawn contract for locally managed MCP servers.
//!
//! An `internal` server always carries a port and is spawned one of two ways,
//! decided once by its manifest: with `binary:` core runs that extension
//! binary ([`SpawnProgram::Binary`]); with no `binary:` and a `toolsets:`
//! list (enforced when the manifest is validated and resolved) core re-runs
//! its own executable as `admin mcp serve --server <id>`
//! ([`SpawnProgram::Hosted`]). An `external` server is never spawned. Every
//! process, port and lifecycle path resolves the program and port through
//! [`SpawnTarget`], so an external server reaching a spawn path is a typed
//! error rather than a zero port or an empty binary name.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::{McpDomainError, McpDomainResult};
use systemprompt_models::mcp::{McpServerConfig, McpServerType};

pub const HOSTED_SERVE_ARGS: [&str; 3] = ["admin", "mcp", "serve"];
pub const HOSTED_SERVER_FLAG: &str = "--server";

/// What core executes to run one internal MCP server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpawnProgram<'a> {
    Binary(&'a str),
    Hosted,
}

/// Program and port of a server that core spawns and supervises itself.
pub trait SpawnTarget {
    fn spawn_port(&self) -> McpDomainResult<u16>;
    fn spawn_program(&self) -> McpDomainResult<SpawnProgram<'_>>;
}

impl SpawnTarget for McpServerConfig {
    fn spawn_port(&self) -> McpDomainResult<u16> {
        self.port
            .filter(|port| *port != 0)
            .ok_or_else(|| not_spawnable(&self.name, "port"))
    }

    fn spawn_program(&self) -> McpDomainResult<SpawnProgram<'_>> {
        match (self.server_type, self.binary.as_deref()) {
            (McpServerType::External, _) => Err(not_spawnable(&self.name, "binary")),
            (McpServerType::Internal, Some(binary)) if binary.trim().is_empty() => {
                Err(not_spawnable(&self.name, "binary"))
            },
            (McpServerType::Internal, Some(binary)) => Ok(SpawnProgram::Binary(binary)),
            (McpServerType::Internal, None) => Ok(SpawnProgram::Hosted),
        }
    }
}

#[must_use]
pub fn hosted_serve_args(server: &McpServerConfig) -> Vec<String> {
    HOSTED_SERVE_ARGS
        .iter()
        .map(|arg| (*arg).to_owned())
        .chain([HOSTED_SERVER_FLAG.to_owned(), server.name.clone()])
        .collect()
}

fn not_spawnable(name: &str, field: &str) -> McpDomainError {
    McpDomainError::Configuration(format!(
        "{name}: server declares no {field}; external MCP servers are proxied to their \
         endpoint and are never spawned locally"
    ))
}
