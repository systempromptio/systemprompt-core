//! Spawn contract for locally managed MCP servers.
//!
//! An `internal` server always carries a binary and a port; an `external`
//! one never does. Every process, port and lifecycle path resolves the pair
//! through [`SpawnTarget`] so an external server reaching a spawn path is a
//! typed error rather than a zero port or an empty binary name.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::{McpDomainError, McpDomainResult};
use systemprompt_models::mcp::McpServerConfig;

/// Binary and port of a server that core spawns and supervises itself.
pub trait SpawnTarget {
    fn spawn_port(&self) -> McpDomainResult<u16>;
    fn spawn_binary(&self) -> McpDomainResult<&str>;
}

impl SpawnTarget for McpServerConfig {
    fn spawn_port(&self) -> McpDomainResult<u16> {
        self.port.ok_or_else(|| not_spawnable(&self.name, "port"))
    }

    fn spawn_binary(&self) -> McpDomainResult<&str> {
        self.binary
            .as_deref()
            .filter(|binary| !binary.trim().is_empty())
            .ok_or_else(|| not_spawnable(&self.name, "binary"))
    }
}

fn not_spawnable(name: &str, field: &str) -> McpDomainError {
    McpDomainError::Configuration(format!(
        "{name}: server declares no {field}; external MCP servers are proxied to their \
         endpoint and are never spawned locally"
    ))
}
