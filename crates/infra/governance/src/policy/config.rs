//! Governance-chain configuration.
//!
//! One YAML document (`governance.enabled` plus `governance.policies: [{id,
//! enabled, ...params}]`) declares whether the chain runs at all, which
//! policies it contains, in what order, and with what per-policy parameters.
//!
//! [`GovernanceConfig::parse`] is the one strict reader: the `governance`
//! config plane runs it on the file before an apply and again on the
//! document reassembled from `governance_config`, so a declaration that is
//! rejected — invalid YAML, an unknown mode, a bad secret catalog — is never
//! applied and never builds an engine; a typo can never silently downgrade
//! enforcement. No rows is the documented warn-only fallback
//! ([`GovernanceConfig::defaults`]).
//!
//! Each policy also carries an [`EnforcementMode`]. A declared policy without a
//! mode defaults to `enforce`, while the vendor-neutral fallback chain is
//! explicitly warn-only. `enforce` halts the chain on a deny; `warn` records
//! the identical finding and lets the call through, so tunables can be
//! calibrated against real traffic instead of guesses. A top-level
//! `governance.mode` sets the default for every policy that does not name its
//! own. An unrecognised mode is a parse error rather than a silent fallback:
//! reading `mode: warnn` as `enforce` would block traffic an operator believed
//! they had unblocked, and reading it as `warn` would disable enforcement
//! nobody asked to disable.
//!
//! The fallback chain runs every policy in warn mode. Its secret scanner has no
//! signatures because credential applicability belongs to the installation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde_yaml::Value as YamlValue;
use systemprompt_identifiers::PolicyId;
use systemprompt_manifest::EnforcementMode;
use systemprompt_manifest::services::UnknownEnforcementMode;
use thiserror::Error;

use super::builtin::{BUILTIN_POLICY_IDS, SECRET_SCAN_ID};
use super::secrets::{SecretPatternError, SecretScanner};

#[derive(Debug, Error)]
pub enum GovernanceConfigError {
    #[error("governance config is not valid YAML: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("governance config has no `governance.policies` sequence")]
    MissingPolicies,
    #[error("governance config policy entry {index} has no non-empty string `id`")]
    MissingPolicyId { index: usize },
    #[error(
        "governance config has an unknown mode `{value}` at {location}; expected `enforce` or `warn`"
    )]
    InvalidMode {
        location: String,
        value: String,
        #[source]
        source: Option<UnknownEnforcementMode>,
    },
    #[error("governance secret pattern catalog is invalid: {0}")]
    InvalidSecretPatterns(#[from] SecretPatternError),
    #[error("governance config declares policy `{id}` more than once")]
    DuplicatePolicyId { id: String },
    #[error("governance config could not be encoded as a plane entity: {0}")]
    Canonical(#[source] serde_json::Error),
}

fn read_mode(
    node: Option<&YamlValue>,
    location: &str,
) -> Result<Option<EnforcementMode>, GovernanceConfigError> {
    let Some(raw) = node.and_then(|n| n.get("mode")) else {
        return Ok(None);
    };
    let text = raw
        .as_str()
        .ok_or_else(|| GovernanceConfigError::InvalidMode {
            location: location.to_owned(),
            value: format!("{raw:?}"),
            source: None,
        })?;
    text.parse::<EnforcementMode>()
        .map(Some)
        .map_err(|source| GovernanceConfigError::InvalidMode {
            location: location.to_owned(),
            value: text.to_owned(),
            source: Some(source),
        })
}

/// One entry of the configured chain: which policy, whether it runs, and the
/// raw YAML mapping handed to the policy's factory as parameters.
#[derive(Debug, Clone)]
pub struct PolicyConfig {
    pub id: PolicyId,
    pub enabled: bool,
    pub mode: EnforcementMode,
    pub params: YamlValue,
}

/// The ordered policy chain declaration.
#[derive(Debug, Clone)]
pub struct GovernanceConfig {
    pub enabled: bool,
    pub mode: EnforcementMode,
    pub policies: Vec<PolicyConfig>,
}

impl GovernanceConfig {
    #[must_use]
    pub fn defaults() -> Self {
        let policies = BUILTIN_POLICY_IDS
            .iter()
            .map(|id| PolicyConfig {
                id: PolicyId::new(*id),
                enabled: true,
                mode: EnforcementMode::Warn,
                params: YamlValue::Null,
            })
            .collect();
        Self {
            enabled: true,
            mode: EnforcementMode::Warn,
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
                .and_then(|raw| PolicyId::try_new(raw).ok())
                .ok_or(GovernanceConfigError::MissingPolicyId { index })?;
            let enabled = entry
                .get("enabled")
                .and_then(YamlValue::as_bool)
                .unwrap_or(true);
            let mode = read_mode(Some(entry), &format!("governance.policies[{index}] ({id})"))?
                .unwrap_or(default_mode);
            if id == SECRET_SCAN_ID {
                SecretScanner::from_policy_yaml(entry)?;
            }
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
}
