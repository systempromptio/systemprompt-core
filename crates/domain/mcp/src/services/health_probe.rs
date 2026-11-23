//! How a health check reaches a hosted MCP server.
//!
//! A hosted server answers its own port only to a request carrying a hop
//! token minted for it, so the strength of a health check depends on who
//! holds the means to mint one. [`HealthProbe::Handshake`] carries a
//! [`HealthHop`] built from the deployment's signing keys, which only kernel
//! code holds (the API's orchestrator and the sealed MCP process set): it
//! performs the MCP handshake, so a hung or non-MCP listener is unhealthy.
//! [`HealthProbe::Listening`] is what member code holds (the read-only
//! observer, `plugins mcp validate`, the tool provider's health summary): it
//! checks only that the port accepts a connection.
//!
//! The health hop's subject is the fixed health principal
//! [`HEALTH_PRINCIPAL`]: an anonymous caller with no user, no permissions and
//! no roles, never a user's identity.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{McpServerId, SessionId, UserId};
use systemprompt_models::auth::{McpHopIdentity, UserType};
use systemprompt_security::jwt::mint_mcp_hop;
use systemprompt_security::keys::SigningKeys;

pub const HEALTH_PRINCIPAL: &str = "mcp-health";

const HEALTH_PURPOSE: &str = "health";

/// Mints the hop token a kernel health check presents to a hosted server.
#[derive(Clone)]
pub struct HealthHop {
    keys: SigningKeys,
    issuer: String,
}

impl std::fmt::Debug for HealthHop {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HealthHop")
            .field("issuer", &self.issuer)
            .finish_non_exhaustive()
    }
}

impl HealthHop {
    #[must_use]
    pub fn new(keys: SigningKeys, issuer: &str) -> Self {
        Self {
            keys,
            issuer: issuer.to_owned(),
        }
    }

    pub fn mint(
        &self,
        server: &McpServerId,
        mcp_session: Option<&str>,
    ) -> Result<String, jsonwebtoken::errors::Error> {
        mint_mcp_hop(
            &self.keys,
            &self.issuer,
            server,
            &health_identity(mcp_session),
        )
    }
}

#[must_use]
pub fn health_identity(mcp_session: Option<&str>) -> McpHopIdentity {
    McpHopIdentity {
        user_id: UserId::new(HEALTH_PRINCIPAL),
        user: None,
        session_id: SessionId::new(HEALTH_PRINCIPAL),
        user_type: UserType::Anon,
        client_id: None,
        act_chain: Vec::new(),
        purpose: Some(HEALTH_PURPOSE.to_owned()),
        interaction: None,
        mcp_session: mcp_session.map(str::to_owned),
    }
}

/// The strength of a health check, fixed by what its holder may mint.
#[derive(Debug, Clone)]
pub enum HealthProbe {
    Handshake(HealthHop),
    Listening,
}
