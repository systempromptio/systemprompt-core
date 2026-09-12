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
//! silently double every budget. [`GovernanceEngine::global`] is therefore the
//! way every enforcement point in a process reaches the chain: the MCP
//! governance webhook and the `/v1/messages` gateway must charge the same
//! limiter, not one each. [`GovernanceEngine::from_config`] remains available
//! for tests and for callers that genuinely want an isolated chain.
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
use std::path::PathBuf;
use std::sync::LazyLock;

use systemprompt_config::ProfileBootstrap;
use thiserror::Error;

use super::audit::ChainEntryOutcome;
use super::builtin::SECRET_SCAN_ID;
use super::config::{GovernanceConfig, PolicyConfig, PolicyMode};
use super::governed::GovernedInput;
use super::registry::{PolicyFactory, PolicyRegistration};
use super::secrets::SecretFinding;
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
    pub fn global() -> Result<&'static Self, GovernanceEngineError> {
        static ENGINE: LazyLock<Result<GovernanceEngine, GovernanceEngineError>> =
            LazyLock::new(|| {
                let config = governance_config_path()
                    .map_or_else(GovernanceConfig::defaults, |p| GovernanceConfig::load(&p));
                GovernanceEngine::from_config(&config)
            });
        ENGINE.as_ref().map_err(Clone::clone)
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
            entries.push(ChainEntry {
                config: cfg.clone(),
                instance: factory(&cfg.params),
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
            let instance = (r.factory)(&config.params);
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

    pub fn secret_pattern_exclusions(&self) -> Vec<systemprompt_identifiers::SecretPatternId> {
        self.entries
            .iter()
            .find(|entry| entry.config.id == SECRET_SCAN_ID)
            .map_or_else(Vec::new, |entry| entry.instance.secret_pattern_exclusions())
    }

    pub fn secret_entropy_config(&self) -> Option<super::secrets::EntropyConfig> {
        self.entries
            .iter()
            .find(|entry| entry.config.id == SECRET_SCAN_ID)
            .and_then(|entry| entry.instance.secret_entropy_config())
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

fn governance_config_path() -> Option<PathBuf> {
    let profile = ProfileBootstrap::get()
        .inspect_err(|e| {
            tracing::error!(
                error = %e,
                "governance profile bootstrap failed; policies fall back to built-in defaults"
            );
        })
        .ok()?;
    Some(PathBuf::from(&profile.paths.services).join("governance/config.yaml"))
}
