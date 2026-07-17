//! Declarative configuration for Slack apps.
//!
//! Each app describes one Slack workspace: the secret references for its
//! signing secret and bot token, the agent it routes to, and the roles
//! permitted to drive it. Secrets are never inlined — only references resolved
//! through the profile's secret source at boot. This type lives in `models`
//! (not the `slack` domain crate) so it can be embedded in
//! [`super::ServicesConfig`] without a dependency cycle, mirroring
//! `AgentConfig` and `McpServerSummary`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{AgentName, SecretName, SlackWorkspaceId};

use crate::errors::ConfigValidationError;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SlackAppConfig {
    pub workspace_id: SlackWorkspaceId,
    pub signing_secret_ref: SecretName,
    pub bot_token_ref: SecretName,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_agent: Option<AgentName>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub routing: BTreeMap<String, AgentName>,
    #[serde(default)]
    pub authz: SlackAuthzConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SlackAuthzConfig {
    #[serde(default)]
    pub allowed_roles: Vec<String>,
}

const fn default_enabled() -> bool {
    true
}

impl SlackAppConfig {
    #[must_use]
    pub fn agent_for(&self, key: &str) -> Option<&AgentName> {
        self.routing.get(key).or(self.default_agent.as_ref())
    }

    pub fn validate(&self, name: &str) -> Result<(), ConfigValidationError> {
        if self.workspace_id.as_str().is_empty() {
            return Err(ConfigValidationError::invalid_field(format!(
                "slack app '{name}' has an empty workspace_id"
            )));
        }
        if self.default_agent.is_none() && self.routing.is_empty() {
            return Err(ConfigValidationError::required(format!(
                "slack app '{name}' must set default_agent or at least one routing entry"
            )));
        }
        Ok(())
    }
}
