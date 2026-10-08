//! `server.role`: which surfaces and background workers a node runs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

/// The part of the platform a node serves.
///
/// `all` is a single-node deployment. `gateway` serves only the AI gateway,
/// the bridge consumer surface and the OAuth/discovery routes they depend on,
/// and runs no scheduler, MCP or agent processes, so it scales horizontally.
/// `admin` serves everything except the gateway and runs the workers.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum NodeRole {
    #[default]
    All,
    Gateway,
    Admin,
}

impl NodeRole {
    pub const fn serves_gateway(self) -> bool {
        matches!(self, Self::All | Self::Gateway)
    }

    pub const fn serves_admin(self) -> bool {
        matches!(self, Self::All | Self::Admin)
    }

    pub const fn runs_workers(self) -> bool {
        self.serves_admin()
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Gateway => "gateway",
            Self::Admin => "admin",
        }
    }
}

impl std::fmt::Display for NodeRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
