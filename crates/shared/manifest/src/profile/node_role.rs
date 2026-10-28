//! `server.role`: which surfaces and background workers a node runs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Deserializer, Serialize};

/// The part of the platform a node serves.
///
/// `all` is a single-node deployment: every surface, every worker, and the
/// extensions' runner services, so one node runs the whole product.
/// `gateway` serves only the AI gateway,
/// the bridge consumer surface and the OAuth/discovery routes they depend on,
/// and runs no scheduler, MCP or agent processes, so it scales horizontally.
/// `api` serves everything except the gateway, runs no scheduler and
/// supervises no MCP or agent process — it forwards that traffic to the
/// worker that owns the process — so it scales horizontally.
/// `worker` runs the scheduler and owns the MCP and agent processes; it
/// serves the probes plus the MCP and agent routes `api` nodes forward to.
/// `runner` serves only the health, readiness and metrics probes, runs no
/// scheduler, MCP or agent processes, and starts the extensions' runner
/// services, so long-running work scales on its own Deployment.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum NodeRole {
    #[default]
    All,
    Gateway,
    Api,
    Worker,
    Runner,
}

#[derive(Debug, thiserror::Error)]
pub enum NodeRoleParseError {
    #[error(
        "server.role 'admin' was split into 'api' (console, agent, MCP and content routes; \
         scales horizontally) and 'worker' (scheduler, MCP and agent processes); set one of them"
    )]
    AdminSplit,
    #[error("unknown server.role '{0}'; expected one of all, gateway, api, worker, runner")]
    Unknown(String),
}

impl std::str::FromStr for NodeRole {
    type Err = NodeRoleParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "all" => Ok(Self::All),
            "gateway" => Ok(Self::Gateway),
            "api" => Ok(Self::Api),
            "worker" => Ok(Self::Worker),
            "runner" => Ok(Self::Runner),
            "admin" => Err(NodeRoleParseError::AdminSplit),
            other => Err(NodeRoleParseError::Unknown(other.to_owned())),
        }
    }
}

impl<'de> Deserialize<'de> for NodeRole {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        raw.parse().map_err(serde::de::Error::custom)
    }
}

impl NodeRole {
    pub const ALL_ROLES: [Self; 5] = [
        Self::All,
        Self::Gateway,
        Self::Api,
        Self::Worker,
        Self::Runner,
    ];

    pub const fn serves_gateway(self) -> bool {
        matches!(self, Self::All | Self::Gateway)
    }

    pub const fn serves_api(self) -> bool {
        matches!(self, Self::All | Self::Api)
    }

    pub const fn runs_scheduler(self) -> bool {
        matches!(self, Self::All | Self::Worker)
    }

    pub const fn supervises_processes(self) -> bool {
        matches!(self, Self::All | Self::Worker)
    }

    pub const fn shares_process_ownership(self) -> bool {
        matches!(self, Self::Worker)
    }

    pub const fn writes_shared_caches(self) -> bool {
        matches!(self, Self::All | Self::Worker)
    }

    pub const fn reads_shared_services_composition(self) -> bool {
        matches!(self, Self::Api)
    }

    pub const fn runs_runner_services(self) -> bool {
        matches!(self, Self::All | Self::Runner)
    }

    pub const fn is_single_node(self) -> bool {
        matches!(self, Self::All)
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Gateway => "gateway",
            Self::Api => "api",
            Self::Worker => "worker",
            Self::Runner => "runner",
        }
    }
}

impl std::fmt::Display for NodeRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
