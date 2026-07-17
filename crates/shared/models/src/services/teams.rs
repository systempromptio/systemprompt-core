//! Declarative configuration for Microsoft Teams apps.
//!
//! Each app describes one Teams bot registration: the Entra tenant it serves,
//! the Microsoft App Id used as the inbound-token audience, the secret
//! reference for its app password (client secret), the agent it routes to, and
//! the roles permitted to drive it. Secrets are never inlined — only references
//! resolved through the profile's secret source at boot. This type lives in
//! `models` (not the `teams` domain crate) so it can be embedded in
//! [`super::ServicesConfig`] without a dependency cycle, mirroring
//! `SlackAppConfig` and `McpServerSummary`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{AgentName, SecretName, TeamsTenantId};

use crate::errors::ConfigValidationError;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TeamsAppConfig {
    pub tenant_id: TeamsTenantId,
    pub app_id: String,
    pub app_password_ref: SecretName,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_agent: Option<AgentName>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub routing: BTreeMap<String, AgentName>,
    #[serde(default)]
    pub authz: TeamsAuthzConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TeamsAuthzConfig {
    #[serde(default)]
    pub allowed_roles: Vec<String>,
}

const fn default_enabled() -> bool {
    true
}

impl TeamsAppConfig {
    #[must_use]
    pub fn agent_for(&self, key: &str) -> Option<&AgentName> {
        self.routing.get(key).or(self.default_agent.as_ref())
    }

    pub fn validate(&self, name: &str) -> Result<(), ConfigValidationError> {
        if self.tenant_id.as_str().is_empty() {
            return Err(ConfigValidationError::invalid_field(format!(
                "teams app '{name}' has an empty tenant_id"
            )));
        }
        if self.app_id.is_empty() {
            return Err(ConfigValidationError::invalid_field(format!(
                "teams app '{name}' has an empty app_id"
            )));
        }
        if self.default_agent.is_none() && self.routing.is_empty() {
            return Err(ConfigValidationError::required(format!(
                "teams app '{name}' must set default_agent or at least one routing entry"
            )));
        }
        Ok(())
    }
}
