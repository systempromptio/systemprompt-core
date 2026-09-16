//! Outbound personal-account OAuth connector settings for an MCP deployment.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::errors::ConfigValidationError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Outbound personal-account OAuth settings, separate from inbound MCP access.
///
/// `authorization_params` are appended to the authorization request verbatim;
/// only the keys in [`ConnectorConfig::ALLOWED_AUTHORIZATION_PARAMS`] are
/// accepted, because the generic flow owns every other parameter (Google's
/// `access_type=offline` is the motivating case: without it no refresh token
/// is issued). `identity: userinfo` asks the consenting extension to read the
/// account label from the issuer's OIDC `userinfo_endpoint`, which needs the
/// `openid` scope.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectorConfig {
    #[serde(default = "generic_adapter")]
    pub adapter: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default)]
    pub scopes: Vec<String>,
    #[serde(default)]
    pub authorization_origins: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub authorization_params: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity: Option<ConnectorIdentity>,
    #[serde(default)]
    pub client_id_secret: Option<String>,
    #[serde(default)]
    pub client_secret: Option<String>,
}

/// Where the consenting extension reads the connected account's label from.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConnectorIdentity {
    Userinfo,
}

impl ConnectorConfig {
    pub const ALLOWED_AUTHORIZATION_PARAMS: [&'static str; 4] =
        ["access_type", "prompt", "login_hint", "hd"];

    pub fn validate(&self, name: &str) -> Result<(), ConfigValidationError> {
        for (key, value) in &self.authorization_params {
            if !Self::ALLOWED_AUTHORIZATION_PARAMS.contains(&key.as_str()) {
                return Err(ConfigValidationError::invalid_field(format!(
                    "MCP server '{name}': connector authorization_params.{key} is not an \
                     allowed parameter (allowed: {})",
                    Self::ALLOWED_AUTHORIZATION_PARAMS.join(", ")
                )));
            }
            if value.is_empty()
                || value
                    .chars()
                    .any(|c| c.is_whitespace() || "&=#".contains(c))
            {
                return Err(ConfigValidationError::invalid_field(format!(
                    "MCP server '{name}': connector authorization_params.{key} must be a \
                     single non-empty token"
                )));
            }
        }
        if self.identity == Some(ConnectorIdentity::Userinfo)
            && !self.scopes.iter().any(|scope| scope == "openid")
        {
            return Err(ConfigValidationError::invalid_field(format!(
                "MCP server '{name}': connector identity: userinfo requires the openid scope"
            )));
        }
        Ok(())
    }
}

fn generic_adapter() -> String {
    "generic".to_owned()
}
