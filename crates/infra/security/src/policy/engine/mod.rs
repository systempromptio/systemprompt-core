//! Traced first-deny-wins evaluation of the configured policy chain.
//!
//! [`GovernanceEngine`] owns the instantiated chain: policies resolved from
//! the inventory registry against a [`GovernanceConfig`], in declaration
//! order. [`GovernanceEngine::evaluate`] records a per-entry
//! [`ChainEntryOutcome`] — including disabled and skipped-after-deny entries —
//! so the audit row preserves the full evaluation order, not just the first
//! deny. The walk itself lives in `chain`.
//!
//! Policies hold no request state. Rate windows are counted in Postgres
//! (see [`super::rate_window`]): an enforcement point calls
//! [`GovernanceEngine::charge_rate_windows`] before evaluating, and every
//! replica charges the same counter. The engine is built once at the
//! composition root ([`GovernanceEngine::from_services_root`] plus
//! [`GovernanceEngine::with_rate_windows`]) and injected into every
//! enforcement point. [`GovernanceEngine::from_config`] builds a chain with no
//! counter store; its rate windows are never charged, so an enabled
//! `rate_limit` policy reports the missing count as a violation.
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
use std::sync::atomic::{AtomicU64, Ordering};

use systemprompt_identifiers::{CallId, PolicyId, SessionId, UserId};
use thiserror::Error;

use super::audit::ChainEntryOutcome;
use super::builtin::SECRET_SCAN_ID;
use super::config::{GovernanceConfig, GovernanceConfigError, PolicyConfig, PolicyMode};
use super::governed::GovernedInput;
use super::rate_window::{RateCharge, RateCounts, RateWindowRepository};
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

#[derive(Debug, Error)]
pub enum GovernanceEngineError {
    #[error(
        "governance config names policy `{id}`, but no implementation is linked into this binary"
    )]
    UnknownPolicyId { id: PolicyId },
    #[error("governance policy `{id}` has invalid configuration: {source}")]
    InvalidPolicyConfiguration {
        id: PolicyId,
        #[source]
        source: PolicyConfigurationError,
    },
    #[error("governance config at {path} was rejected: {source}")]
    ConfigRejected {
        path: String,
        #[source]
        source: GovernanceConfigError,
    },
}

struct ChainEntry {
    config: PolicyConfig,
    instance: Box<dyn GovernancePolicy>,
}

pub struct GovernanceEngine {
    enabled: bool,
    entries: Vec<ChainEntry>,
    rate_windows: Option<RateWindowRepository>,
    charges_since_prune: AtomicU64,
}

const PRUNE_EVERY_CHARGES: u64 = 1024;

#[derive(Debug, Clone, Copy)]
pub struct RateChargeSubject<'a> {
    pub session: &'a SessionId,
    pub user: &'a UserId,
    pub call: &'a CallId,
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
            .field("rate_windows", &self.rate_windows.is_some())
            .field("charges_since_prune", &self.charges_since_prune)
            .finish()
    }
}

impl GovernanceEngine {
    pub fn from_services_root(services_root: &Path) -> Result<Self, GovernanceEngineError> {
        let path = services_root.join("governance/config.yaml");
        let config = GovernanceConfig::load(&path).map_err(|source| {
            GovernanceEngineError::ConfigRejected {
                path: path.display().to_string(),
                source,
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
                id: PolicyId::new(r.id),
                enabled: false,
                mode: PolicyMode::Enforce,
                params: serde_yaml::Value::Null,
            };
            let instance = (r.factory)(&config.params).map_err(|source| {
                GovernanceEngineError::InvalidPolicyConfiguration {
                    id: config.id.clone(),
                    source,
                }
            })?;
            entries.push(ChainEntry { config, instance });
        }

        Ok(Self {
            enabled: config.enabled,
            entries,
            rate_windows: None,
            charges_since_prune: AtomicU64::new(0),
        })
    }

    #[must_use]
    pub fn with_rate_windows(mut self, repository: RateWindowRepository) -> Self {
        self.rate_windows = Some(repository);
        self
    }

    pub async fn charge_rate_windows(&self, subject: RateChargeSubject<'_>) -> RateCounts {
        let mut counts = RateCounts::empty();
        let mut charged = false;
        if !self.enabled {
            return counts;
        }
        let Some(repository) = self.rate_windows.as_ref() else {
            return counts;
        };
        let call = subject.call;
        let subject = format!("{}:{}", subject.session.as_str(), subject.user.as_str());
        for entry in self.entries.iter().filter(|e| e.config.enabled) {
            let Some(window) = entry.instance.rate_window() else {
                continue;
            };
            let charge = RateCharge {
                policy_id: &entry.config.id,
                window_secs: window.seconds,
                subject: &subject,
                call_id: call,
            };
            charged = true;
            match repository.charge(charge).await {
                Ok(position) => counts.record(entry.config.id.clone(), position),
                Err(error) => tracing::error!(
                    %error,
                    policy_id = %entry.config.id,
                    "rate window charge failed; the policy fails closed for this call"
                ),
            }
        }
        if charged
            && self
                .charges_since_prune
                .fetch_add(1, Ordering::Relaxed)
                .is_multiple_of(PRUNE_EVERY_CHARGES)
            && let Err(error) = repository.prune_expired().await
        {
            tracing::warn!(%error, "expired rate windows could not be pruned");
        }
        counts
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
            source: PolicyConfigurationError::ToothlessSecretScan,
        });
    }
    Ok(())
}
