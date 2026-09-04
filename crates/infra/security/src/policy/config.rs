//! Governance-chain configuration.
//!
//! One YAML document (`governance.enabled` plus `governance.policies: [{id,
//! enabled, ...params}]`) declares whether the chain runs at all, which
//! policies it contains, in what order, and with what per-policy parameters.
//!
//! Two loaders, because startup and the request path want opposite failure
//! modes. [`GovernanceConfig::validate`] is for boot: it returns the error so
//! a misconfigured installation refuses to start.
//! [`GovernanceConfig::load`] is for the request path: it degrades to
//! [`GovernanceConfig::defaults`] and logs, because a governance deployment
//! that failed closed on a config typo would block every tool call.
//! [`GovernanceConfig::parse`] is the strict form over a string.
//!
//! Each policy also carries a [`PolicyMode`]. `enforce` is the default and
//! halts the chain on a deny; `warn` records the identical finding and lets
//! the call through, so tunables can be calibrated against real traffic
//! instead of guesses. A top-level `governance.mode` sets the default for
//! every policy that does not name its own. An unrecognised mode is a parse
//! error rather than a silent fallback: reading `mode: warnn` as `enforce`
//! would block traffic an operator believed they had unblocked, and reading it
//! as `warn` would disable enforcement nobody asked to disable.
//!
//! Note the fallback direction: defaults enable every policy, so a file that
//! cannot be read yields *more* enforcement than it declared, never less.
//! Governance cannot be disabled by deleting or breaking this file — only by
//! `governance.enabled: false` or per-policy `enabled: false`.
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
    #[error("governance config exists but could not be read: {0}")]
    Unreadable(#[from] std::io::Error),
    #[error(
        "governance config has an unknown mode `{value}` at {location}; expected `enforce` or `warn`"
    )]
    InvalidMode { location: String, value: String },
}

/// Whether a policy halts the chain on a finding or only records it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum PolicyMode {
    #[default]
    Enforce,
    Warn,
}

impl PolicyMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Enforce => "enforce",
            Self::Warn => "warn",
        }
    }

    #[must_use]
    pub const fn is_warn(self) -> bool {
        matches!(self, Self::Warn)
    }

    fn parse_str(value: &str) -> Option<Self> {
        match value {
            "enforce" => Some(Self::Enforce),
            "warn" => Some(Self::Warn),
            _ => None,
        }
    }
}

impl std::fmt::Display for PolicyMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// Why: an absent key inherits, a present-but-unreadable key is an error. Both
// are distinct from "present and valid", so the return is a three-way option
// rather than a defaulted value.
fn read_mode(
    node: Option<&YamlValue>,
    location: &str,
) -> Result<Option<PolicyMode>, GovernanceConfigError> {
    let Some(raw) = node.and_then(|n| n.get("mode")) else {
        return Ok(None);
    };
    let text = raw
        .as_str()
        .ok_or_else(|| GovernanceConfigError::InvalidMode {
            location: location.to_owned(),
            value: format!("{raw:?}"),
        })?;
    PolicyMode::parse_str(text)
        .map(Some)
        .ok_or_else(|| GovernanceConfigError::InvalidMode {
            location: location.to_owned(),
            value: text.to_owned(),
        })
}

/// One entry of the configured chain: which policy, whether it runs, and the
/// raw YAML mapping handed to the policy's factory as parameters.
#[derive(Debug, Clone)]
pub struct PolicyConfig {
    pub id: String,
    pub enabled: bool,
    pub mode: PolicyMode,
    pub params: YamlValue,
}

/// The ordered policy chain declaration.
#[derive(Debug, Clone)]
pub struct GovernanceConfig {
    pub enabled: bool,
    pub mode: PolicyMode,
    pub policies: Vec<PolicyConfig>,
}

impl GovernanceConfig {
    #[must_use]
    pub fn defaults() -> Self {
        let policies = ["secret_scan", "scope_check", "tool_blocklist", "rate_limit"]
            .into_iter()
            .map(|id| PolicyConfig {
                id: id.to_owned(),
                enabled: true,
                mode: PolicyMode::Enforce,
                params: YamlValue::Null,
            })
            .collect();
        Self {
            enabled: true,
            mode: PolicyMode::Enforce,
            policies,
        }
    }

    pub fn parse(yaml: &str) -> Result<Self, GovernanceConfigError> {
        let root: YamlValue = serde_yaml::from_str(yaml)?;
        let governance = root.get("governance");
        let enabled = governance
            .and_then(|g| g.get("enabled"))
            .and_then(YamlValue::as_bool)
            .unwrap_or(true);
        let default_mode = read_mode(governance, "governance")?.unwrap_or_default();
        let policies = governance
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
            let mode = read_mode(Some(entry), &format!("governance.policies[{index}] ({id})"))?
                .unwrap_or(default_mode);
            out.push(PolicyConfig {
                id,
                enabled,
                mode,
                params: entry.clone(),
            });
        }
        Ok(Self {
            enabled,
            mode: default_mode,
            policies: out,
        })
    }

    fn read(path: &Path) -> Result<Option<Self>, GovernanceConfigError> {
        match std::fs::read_to_string(path) {
            Ok(text) => Self::parse(&text).map(Some),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(GovernanceConfigError::Unreadable(e)),
        }
    }

    pub fn validate(path: &Path) -> Result<(), GovernanceConfigError> {
        Self::read(path).map(|_| ())
    }

    #[must_use]
    pub fn load(path: &Path) -> Self {
        match Self::read(path) {
            Ok(Some(config)) => config,
            Ok(None) => {
                tracing::warn!(
                    path = %path.display(),
                    "governance config not found; falling back to the built-in defaults, \
                     which enable every policy"
                );
                Self::defaults()
            },
            Err(error) => {
                tracing::error!(
                    path = %path.display(),
                    %error,
                    "governance config rejected; falling back to the built-in defaults, \
                     which enable every policy and may not be what this file asked for"
                );
                Self::defaults()
            },
        }
    }
}
