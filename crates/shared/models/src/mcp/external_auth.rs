//! Per-user bearer resolution for an `external` MCP server.
//!
//! [`ExternalAuth`] names the accessor core calls for the caller's
//! third-party bearer and the header it is injected on.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

use crate::errors::ServicesValidationError;

/// Per-user bearer resolution for an `external` MCP server.
///
/// The MCP gateway exposes no token vault of its own; instead an extension
/// banks the calling user's third-party token and serves it from
/// `token_endpoint`. At tool-call time core `GET`s that accessor with the
/// user's systemprompt JWT and injects the returned bearer onto `header` (as
/// `{scheme} {token}`), replacing the systemprompt credential so nothing
/// internal reaches the third party.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalAuth {
    pub token_endpoint: String,
    #[serde(default = "default_auth_header")]
    pub header: String,
    #[serde(default = "default_auth_scheme")]
    pub scheme: String,
}

fn default_auth_header() -> String {
    "Authorization".to_owned()
}

fn default_auth_scheme() -> String {
    "Bearer".to_owned()
}

impl ExternalAuth {
    pub fn header_value(&self, bearer: &str) -> String {
        if self.scheme.trim().is_empty() {
            bearer.to_owned()
        } else {
            format!("{} {bearer}", self.scheme)
        }
    }
}

pub(super) fn validate_external_auth(
    name: &str,
    ext: &ExternalAuth,
) -> Result<(), ServicesValidationError> {
    if ext.token_endpoint.starts_with("http://") || ext.token_endpoint.starts_with("https://") {
        return Err(ServicesValidationError::invalid_field(format!(
            "MCP server '{name}': external_auth.token_endpoint must be a relative \
                 path (e.g. /api/public/<provider>/token); the host is derived from \
                 server.api_external_url. Remove the scheme+host prefix."
        )));
    }
    if !ext.token_endpoint.starts_with('/') {
        return Err(ServicesValidationError::invalid_field(format!(
            "MCP server '{name}': external_auth.token_endpoint must be an absolute \
                 path beginning with '/'."
        )));
    }
    if ext.header.trim().is_empty() {
        return Err(ServicesValidationError::invalid_field(format!(
            "MCP server '{name}': external_auth.header must not be empty."
        )));
    }
    Ok(())
}
