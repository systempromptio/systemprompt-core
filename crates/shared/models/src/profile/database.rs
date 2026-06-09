//! Database configuration.

use serde::{Deserialize, Serialize};

/// Operator-tunable connection-pool sizing.
///
/// Omitted fields fall back to the engine defaults (50 connections, 30s
/// acquire, 300s idle, 1800s lifetime); set them to fit the pool to the
/// deployment's Postgres `max_connections` and replica count.
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
}
