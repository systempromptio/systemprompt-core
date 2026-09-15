//! Traced first-deny-wins evaluation of the configured policy chain.
//!
//! [`GovernanceEngine`] owns the instantiated chain: policies resolved from
//! the inventory registry against a [`GovernanceConfig`], in declaration
//! order. [`GovernanceEngine::evaluate`] records a per-entry
//! [`ChainEntryOutcome`] — including disabled and skipped-after-deny entries —
//! so the audit row preserves the full evaluation order, not just the first
//! deny. The walk itself lives in `chain`.
//!
//! Policies that accumulate state (the rate limiter) scope it to their
//! instance, so two engines never share buckets — a second engine would
//! silently double every budget. The engine is therefore built once at the
//! composition root ([`GovernanceEngine::from_services_root`]) and injected
//! into every enforcement point: the MCP governance webhook and the
//! `/v1/messages` gateway charge the same limiter, not one each.
//! [`GovernanceEngine::from_config`] builds an isolated chain for tests.
//!
//! [`GovernanceEngine::evaluate_with_prompt_recovery`] is the opt-in variant
//! for prompt targets: when a policy denies with a located secret leak, the
//! caller is offered the findings and may hand back a sanitized input, which
//! the same policy re-verifies before the chain resumes with it. Earlier
//! policies are never re-run, so a stateful policy charges the call once.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod chain;

use std::collections::{HashMap, HashSet};
use std::path::Path;

use thiserror::Error;

use super::audit::ChainEntryOutcome;
use super::builtin::SECRET_SCAN_ID;
use super::config::{GovernanceConfig, PolicyConfig, PolicyMode};
use super::governed::GovernedInput;
use super::registry::{PolicyConfigurationError, PolicyFactory, PolicyRegistration};
use super::secrets::{SecretFinding, SecretScanner};
use super::types::{GovernancePolicy, PolicyContext};
use crate::authz::types::Decision;

/// The outcome of one traced chain run: the first-deny-wins [`Decision`] and
/// the ordered per-entry trace destined for the audit row.
#[derive(Debug)]
pub struct Evaluation {
    pub decision: Decision,
    pub chain: Vec<ChainEntryOutcome>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum GovernanceEngineError {
    #[error(
        "governance config names policy `{id}`, but no implementation is linked into this binary"
    )]
    UnknownPolicyId { id: String },
    #[error("governance policy `{id}` has invalid configuration: {source}")]
    InvalidPolicyConfiguration {
        id: String,
        #[source]
        source: PolicyConfigurationError,
    },
    #[error("governance config at {path} was rejected: {message}")]
    ConfigRejected { path: String, message: String },
}

struct ChainEntry {
    config: PolicyConfig,
    instance: Box<dyn GovernancePolicy>,
}

pub struct GovernanceEngine {
    enabled: bool,
    entries: Vec<ChainEntry>,
}

impl std::fmt::Debug for GovernanceEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GovernanceEngine")
            .field("enabled", &self.enabled)
            .field(
                "policies",
                &self
                    .entries
                    .iter()
                    .map(|e| e.config.id.as_str())
                    .collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl GovernanceEngine {
    pub fn from_services_root(services_root: &Path) -> Result<Self, GovernanceEngineError> {
        let path = services_root.join("governance/config.yaml");
        let config = GovernanceConfig::load(&path).map_err(|error| {
            GovernanceEngineError::ConfigRejected {
                path: path.display().to_string(),
                message: error.to_string(),
            }
        })?;
        Self::from_config(&config)
    }

    pub fn from_config(config: &GovernanceConfig) -> Result<Self, GovernanceEngineError> {
        if !config.enabled {
            tracing::warn!(
                "governance is DISABLED by config: no scope, secret, blocklist or rate-limit \
                 check will run on any request"
            );
        }
        let factories: HashMap<&'static str, PolicyFactory> =
            inventory::iter::<PolicyRegistration>()
                .map(|r| (r.id, r.factory))
                .collect();

        let mut entries = Vec::with_capacity(config.policies.len());
        for cfg in &config.policies {
            let factory = factories
                .get(cfg.id.as_str())
                .ok_or_else(|| GovernanceEngineError::UnknownPolicyId { id: cfg.id.clone() })?;
            let instance = factory(&cfg.params).map_err(|source| {
                GovernanceEngineError::InvalidPolicyConfiguration {
                    id: cfg.id.clone(),
                    source,
                }
            })?;
            if config.enabled {
                reject_toothless_enforcement(cfg, instance.as_ref())?;
            }
            entries.push(ChainEntry {
                config: cfg.clone(),
                instance,
            });
        }

        let mentioned: HashSet<&str> = config.policies.iter().map(|p| p.id.as_str()).collect();
        for r in inventory::iter::<PolicyRegistration>().filter(|r| !mentioned.contains(r.id)) {
            let config = PolicyConfig {
                id: r.id.to_owned(),
                enabled: false,
                mode: PolicyMode::Enforce,
                params: serde_yaml::Value::Null,
            };
            let instance = (r.factory)(&config.params).map_err(|source| {
                GovernanceEngineError::InvalidPolicyConfiguration {
                    id: r.id.to_owned(),
                    source,
                }
            })?;
            entries.push(ChainEntry { config, instance });
        }

        Ok(Self {
            enabled: config.enabled,
            entries,
        })
    }

    #[must_use]
    pub fn enforces_prompt_secrets(&self) -> bool {
        self.enabled
            && self.entries.iter().any(|entry| {
                entry.config.enabled
                    && !entry.config.mode.is_warn()
                    && entry.config.id == SECRET_SCAN_ID
            })
    }

    #[must_use]
    pub fn secret_scanner(&self) -> Option<&SecretScanner> {
        self.entries
            .iter()
            .find(|entry| entry.config.id == SECRET_SCAN_ID)
            .and_then(|entry| entry.instance.secret_scanner())
    }

    pub fn policies(&self) -> impl Iterator<Item = (&PolicyConfig, &dyn GovernancePolicy)> {
        self.entries
            .iter()
            .map(|e| (&e.config, e.instance.as_ref()))
    }

    pub fn evaluate(&self, ctx: &PolicyContext<'_>) -> Evaluation {
        self.evaluate_chain(ctx, None)
    }

    pub fn evaluate_with_prompt_recovery(
        &self,
        ctx: &PolicyContext<'_>,
        mut recover: impl FnMut(&[SecretFinding]) -> Option<GovernedInput>,
    ) -> Evaluation {
        self.evaluate_chain(ctx, Some(&mut recover))
    }
}

// Why: the warn-only defaults rely on an empty pattern catalog being legal,
// but an operator-authored `enforce` block that compiles to a scanner which
// can never deny is a silent non-enforcement — refuse it at boot. A globally
// disabled engine enforces nothing, so it is not silently non-enforcing.
fn reject_toothless_enforcement(
    cfg: &PolicyConfig,
    instance: &dyn GovernancePolicy,
) -> Result<(), GovernanceEngineError> {
    if cfg.id != SECRET_SCAN_ID || !cfg.enabled || cfg.mode.is_warn() {
        return Ok(());
    }
    let toothless = instance
        .secret_scanner()
        .is_none_or(|scanner| scanner.pattern_count() == 0);
    if toothless {
        return Err(GovernanceEngineError::InvalidPolicyConfiguration {
            id: cfg.id.clone(),
            source: PolicyConfigurationError(
                "secret_scan is in enforce mode but compiles no secret patterns; declare \
                 `patterns` or set `mode: warn`"
                    .to_owned(),
            ),
        });
    }
    Ok(())
}
