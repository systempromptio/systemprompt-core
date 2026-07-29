//! Governance-chain configuration.
//!
//! One YAML document (`governance.policies: [{id, enabled, ...params}]`)
//! declares which policies run, in what order, and with what per-policy
//! parameters. [`GovernanceConfig::load`] is deliberately lenient — a missing
//! or malformed file degrades to [`GovernanceConfig::defaults`] with a warning,
//! because a governance deployment that fails closed on a config typo would
//! block every tool call in the installation. [`GovernanceConfig::parse`] is
//! the strict form for callers that want the error.
//!
//! Path resolution is the caller's concern: core takes a path, extensions
//! resolve it from their profile (`<services>/governance/config.yaml`).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use serde_yaml::Value as YamlValue;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GovernanceConfigError {
    #[error("governance config is not valid YAML: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("governance config has no `governance.policies` sequence")]
    MissingPolicies,
    #[error("governance config policy entry {index} has no string `id`")]
    MissingPolicyId { index: usize },
}

/// One entry of the configured chain: which policy, whether it runs, and the
/// raw YAML mapping handed to the policy's factory as parameters.
#[derive(Debug, Clone)]
pub struct PolicyConfig {
    pub id: String,
    pub enabled: bool,
    pub params: YamlValue,
}

/// The ordered policy chain declaration.
#[derive(Debug, Clone)]
pub struct GovernanceConfig {
    pub policies: Vec<PolicyConfig>,
}

impl GovernanceConfig {
    /// The four built-in policies, enabled, with default parameters, in
    /// first-deny-wins order: cheap-and-fatal checks before stateful ones.
    #[must_use]
    pub fn defaults() -> Self {
        let policies = ["secret_scan", "scope_check", "tool_blocklist", "rate_limit"]
            .into_iter()
            .map(|id| PolicyConfig {
                id: id.to_owned(),
                enabled: true,
                params: YamlValue::Null,
            })
            .collect();
        Self { policies }
    }

    /// Strict parse of a YAML document.
    pub fn parse(yaml: &str) -> Result<Self, GovernanceConfigError> {
        let root: YamlValue = serde_yaml::from_str(yaml)?;
        let policies = root
            .get("governance")
            .and_then(|g| g.get("policies"))
            .and_then(YamlValue::as_sequence)
            .ok_or(GovernanceConfigError::MissingPolicies)?;

        let mut out = Vec::with_capacity(policies.len());
        for (index, entry) in policies.iter().enumerate() {
            let id = entry
                .get("id")
                .and_then(YamlValue::as_str)
                .ok_or(GovernanceConfigError::MissingPolicyId { index })?
                .to_owned();
            let enabled = entry
                .get("enabled")
                .and_then(YamlValue::as_bool)
                .unwrap_or(true);
            out.push(PolicyConfig {
                id,
                enabled,
                params: entry.clone(),
            });
        }
        Ok(Self { policies: out })
    }

    /// Lenient load: any failure — absent file, unreadable file, invalid
    /// YAML, missing `governance.policies` — logs and falls back to
    /// [`Self::defaults`].
    #[must_use]
    pub fn load(path: &Path) -> Self {
        let Ok(text) = std::fs::read_to_string(path) else {
            tracing::info!(
                path = %path.display(),
                "governance config not found; using built-in defaults"
            );
            return Self::defaults();
        };
        Self::parse(&text).unwrap_or_else(|error| {
            tracing::warn!(
                path = %path.display(),
                %error,
                "governance config rejected; using built-in defaults"
            );
            Self::defaults()
        })
    }
}
