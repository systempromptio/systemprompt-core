//! Typed `policies` statements and the deny-then-allow lockdown.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PolicyEffect {
    Allow,
    Deny,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolicyAction {
    #[serde(rename = "tool.use")]
    Tool,
    #[serde(rename = "provider.use")]
    Provider,
    #[serde(rename = "integration.use")]
    Integration,
}

/// One `policies[]` statement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Policy {
    pub effect: PolicyEffect,
    pub action: PolicyAction,
    pub resource: String,
}

impl Policy {
    #[must_use]
    pub fn allow(action: PolicyAction, resource: impl Into<String>) -> Self {
        Self {
            effect: PolicyEffect::Allow,
            action,
            resource: resource.into(),
        }
    }

    #[must_use]
    pub fn deny(action: PolicyAction, resource: impl Into<String>) -> Self {
        Self {
            effect: PolicyEffect::Deny,
            action,
            resource: resource.into(),
        }
    }
}

/// What [`lockdown_policies`] confines a client to.
#[derive(Debug, Clone, Copy)]
pub struct Lockdown<'a> {
    pub provider: &'a str,
    pub allowed_tools: &'a [String],
    pub denied_tools: &'a [String],
    pub allowed_mcp: &'a [String],
}

#[must_use]
pub fn lockdown_policies(lockdown: &Lockdown<'_>) -> Vec<Policy> {
    let mut out = vec![
        Policy::deny(PolicyAction::Provider, "*"),
        Policy::allow(PolicyAction::Provider, lockdown.provider),
        Policy::deny(PolicyAction::Integration, "mcp:*"),
        Policy::deny(PolicyAction::Integration, "plugin:*"),
    ];
    out.extend(
        lockdown
            .allowed_mcp
            .iter()
            .map(|name| Policy::allow(PolicyAction::Integration, format!("mcp:{name}"))),
    );
    if !lockdown.allowed_tools.is_empty() {
        out.push(Policy::deny(PolicyAction::Tool, "*"));
        out.extend(
            lockdown
                .allowed_tools
                .iter()
                .map(|tool| Policy::allow(PolicyAction::Tool, format!("{tool}:*"))),
        );
    }
    out.extend(
        lockdown
            .denied_tools
            .iter()
            .map(|tool| Policy::deny(PolicyAction::Tool, format!("{tool}:*"))),
    );
    out
}
