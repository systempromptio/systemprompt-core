//! Database configuration.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

use super::NodeRole;

/// Operator-tunable connection-pool sizing.
///
/// Omitted fields fall back to the engine defaults (50 connections, 30s
/// acquire, 300s idle, 1800s lifetime); set them to fit the pool to the
/// deployment's Postgres `max_connections` and replica count.
/// `statement_cache_capacity` defaults to 100 prepared statements per
/// connection. 0 is accepted but leaks: sqlx still names every statement and
/// never closes one without a cache, so each backend grows until Postgres runs
/// out of memory.
///
/// `max_connections_by_role` overrides `max_connections` for the node's
/// `server.role`, so one profile can size a horizontally scaled gateway pool
/// small and the single admin node's pool larger. A Postgres primary must
/// admit `gateway_replicas × (gateway pool + 2) + admin × (admin pool + 2)`
/// plus migration and operator sessions; the `+ 2` are the replica-identity
/// claim and the event listener sessions every node holds.
/// `session_max_connections` sizes the separate session pool a
/// `database_session_url` opens (10 by default): it holds only the claim,
/// listener, job-lock and migration sessions, so it never needs the main
/// pool's ceiling.
/// [`crate::profile::Profile::validate`] range-checks the values at bootstrap.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PoolConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_connections: Option<u32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acquire_timeout_secs: Option<u64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idle_timeout_secs: Option<u64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_lifetime_secs: Option<u64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub statement_cache_capacity: Option<usize>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_connections_by_role: Option<RoleMaxConnections>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_max_connections: Option<u32>,
}

impl PoolConfig {
    pub fn max_connections_for(&self, role: NodeRole) -> Option<u32> {
        self.max_connections_by_role
            .as_ref()
            .and_then(|by_role| by_role.for_role(role))
            .or(self.max_connections)
    }
}

/// Per-`server.role` pool ceilings; an omitted role uses
/// [`PoolConfig::max_connections`].
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RoleMaxConnections {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub all: Option<u32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gateway: Option<u32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub admin: Option<u32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runner: Option<u32>,
}

impl RoleMaxConnections {
    pub const fn for_role(&self, role: NodeRole) -> Option<u32> {
        match role {
            NodeRole::All => self.all,
            NodeRole::Gateway => self.gateway,
            NodeRole::Admin => self.admin,
            NodeRole::Runner => self.runner,
        }
    }

    pub const fn entries(&self) -> [(NodeRole, Option<u32>); 4] {
        [
            (NodeRole::All, self.all),
            (NodeRole::Gateway, self.gateway),
            (NodeRole::Admin, self.admin),
            (NodeRole::Runner, self.runner),
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DatabaseConfig {
    #[serde(rename = "type")]
    pub db_type: String,

    #[serde(default)]
    pub external_db_access: bool,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pool: Option<PoolConfig>,

    #[serde(default = "default_migrate_on_boot")]
    pub migrate_on_boot: bool,
}

const fn default_migrate_on_boot() -> bool {
    true
}
