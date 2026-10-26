//! Why a connector's machine token could not be minted.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::credential::CredentialError;

#[derive(Debug, thiserror::Error)]
pub enum ConnectorGrantError {
    #[error(
        "MCP server '{server}': connector uses the authorization_code grant, not a machine grant"
    )]
    NotMachine { server: String },

    #[error("MCP server '{server}': connector {field} is not configured")]
    MissingField { server: String, field: &'static str },

    #[error("MCP server '{server}': connector secret `{secret}` is not configured")]
    MissingSecret { server: String, secret: String },

    #[error(transparent)]
    Credential(CredentialError),
}
