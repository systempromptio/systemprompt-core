//! Traced first-deny-wins evaluation of the configured policy chain.
//!
//! [`GovernanceEngine`] owns the instantiated chain: policies resolved from
//! the inventory registry against a [`GovernanceConfig`], in declaration
//! order. [`GovernanceEngine::evaluate`] records a per-entry
//! [`ChainEntryOutcome`] — including disabled and skipped-after-deny entries —
//! so the audit row preserves the full evaluation order, not just the first
//! deny.
//!
//! Policies that accumulate state (the rate limiter) scope it to their
//! instance, so two engines never share buckets — a second engine would
//! silently double every budget. [`GovernanceEngine::global`] is therefore the
//! way every enforcement point in a process reaches the chain: the MCP
//! governance webhook and the `/v1/messages` gateway must charge the same
//! limiter, not one each. [`GovernanceEngine::from_config`] remains available
//! for tests and for callers that genuinely want an isolated chain.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::borrow::Cow;

use super::secrets::{MAX_RECOVERY_FINDINGS, SecretFinding};
use super::{GovernedInput, GovernedTarget};

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::LazyLock;

use systemprompt_config::ProfileBootstrap;
use systemprompt_identifiers::PolicyId;
use thiserror::Error;

use super::audit::{ChainEntryOutcome, ChainEntryResult};
use super::config::{GovernanceConfig, PolicyConfig, PolicyMode};
use super::registry::{PolicyFactory, PolicyRegistration};
use super::types::{GovernancePolicy, PolicyContext};
use crate::authz::types::{Decision, DenyReason, MatchedBy};

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

        for cfg in &config.policies {
            if !factories.contains_key(cfg.id.as_str()) {
                return Err(GovernanceEngineError::UnknownPolicyId { id: cfg.id.clone() });
            }
        }

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

        let mentioned: HashSet<&str> = entries.iter().map(|e| e.config.id.as_str()).collect();
        let unmentioned: Vec<&PolicyRegistration> = inventory::iter::<PolicyRegistration>()
            .filter(|r| !mentioned.contains(r.id))
            .collect();
        for r in unmentioned {
            let cfg = PolicyConfig {
                id: r.id.to_owned(),
                enabled: false,
                mode: PolicyMode::Enforce,
                params: serde_yaml::Value::Null,
            };
            let instance = (r.factory)(&cfg.params);
            entries.push(ChainEntry {
                config: cfg,
                instance,
            });
        }

        Ok(Self {
            enabled: config.enabled,
            entries,
        })
    }

    pub fn enforces_prompt_secrets(&self) -> bool {
        self.enabled
            && self.entries.iter().any(|entry| {
                entry.config.enabled
                    && !entry.config.mode.is_warn()
                    && entry.config.id == "secret_scan"
            })
    }

    pub fn policies(&self) -> impl Iterator<Item = (&PolicyConfig, &dyn GovernancePolicy)> {
        self.entries
            .iter()
            .map(|e| (&e.config, e.instance.as_ref()))
    }

    #[must_use]
    fn master_switch_off(&self) -> Evaluation {
        Evaluation {
            decision: Decision::Allow {
                matched_by: MatchedBy::DefaultIncluded,
            },
            chain: self
                .entries
                .iter()
                .map(|entry| {
                    chain_entry(
                        &entry.config,
                        ChainEntryResult::Disabled,
                        "Governance disabled by master switch",
                    )
                })
                .collect(),
        }
    }

    pub fn evaluate(&self, ctx: &PolicyContext<'_>) -> Evaluation {
        self.evaluate_chain(ctx, false, |_| None)
    }

    pub fn evaluate_with_prompt_recovery(
        &self,
        ctx: &PolicyContext<'_>,
        recover: impl FnMut(&[SecretFinding]) -> Option<GovernedInput>,
    ) -> Evaluation {
        self.evaluate_chain(ctx, true, recover)
    }

    fn evaluate_chain(
        &self,
        ctx: &PolicyContext<'_>,
        recover_prompts: bool,
        mut recover: impl FnMut(&[SecretFinding]) -> Option<GovernedInput>,
    ) -> Evaluation {
        if !self.enabled {
            return self.master_switch_off();
        }

        let mut chain: Vec<ChainEntryOutcome> = Vec::with_capacity(self.entries.len());
        let mut halted: Option<Decision> = None;
        let mut first_warn: Option<DenyReason> = None;
        let mut repaired_input = None;

        for entry in &self.entries {
            if !entry.config.enabled {
                chain.push(chain_entry(
                    &entry.config,
                    ChainEntryResult::Disabled,
                    "Policy disabled in governance config",
                ));
                continue;
            }
            if halted.is_some() {
                chain.push(chain_entry(
                    &entry.config,
                    ChainEntryResult::Skip,
                    "Skipped — already halted by an earlier policy",
                ));
                continue;
            }
            let started = std::time::Instant::now();
            let current = PolicyContext {
                input: repaired_input.as_ref().unwrap_or(ctx.input),
                target: ctx.target.clone(),
                agent_scope: ctx.agent_scope.clone(),
                access_scope: ctx.access_scope,
                session_id: ctx.session_id,
                user_id: ctx.user_id,
                call_id: ctx.call_id,
            };
            let mut decision = entry.instance.evaluate(&current);
            if recover_prompts
                && let Some((recovered, input)) =
                    recover_secret(entry, &current, &decision, &mut recover)
            {
                decision = recovered;
                repaired_input = Some(input);
            }
            let duration_ms = started.elapsed().as_secs_f64() * 1000.0;
            let (outcome, warn, halt) = classify(entry, &decision, duration_ms);
            chain.push(outcome);
            if let Some(reason) = warn
                && first_warn.is_none()
            {
                first_warn = Some(reason);
            }
            if halt {
                halted = Some(decision);
            }
        }

        let decision = halted.unwrap_or_else(|| {
            first_warn.map_or(
                Decision::Allow {
                    matched_by: MatchedBy::DefaultIncluded,
                },
                |reason| Decision::Warn { reason },
            )
        });
        Evaluation { decision, chain }
    }
}

fn recover_secret(
    entry: &ChainEntry,
    ctx: &PolicyContext<'_>,
    decision: &Decision,
    recover: &mut impl FnMut(&[SecretFinding]) -> Option<GovernedInput>,
) -> Option<(Decision, GovernedInput)> {
    if ctx.target != GovernedTarget::Prompt
        || entry.config.mode.is_warn()
        || entry.config.id != "secret_scan"
        || !matches!(
            decision,
            Decision::Deny {
                reason: DenyReason::SecretLeak { .. }
            }
        )
    {
        return None;
    }
    let findings = entry.instance.prompt_secret_findings(ctx.input);
    if findings.is_empty() || findings.len() > MAX_RECOVERY_FINDINGS {
        return None;
    }
    let input = recover(&findings)?;
    let verified = entry.instance.evaluate(&PolicyContext {
        input: &input,
        target: ctx.target.clone(),
        agent_scope: ctx.agent_scope.clone(),
        access_scope: ctx.access_scope,
        session_id: ctx.session_id,
        user_id: ctx.user_id,
        call_id: ctx.call_id,
    });
    if !matches!(verified, Decision::Allow { .. }) {
        return None;
    }
    let detail = findings
        .iter()
        .map(|finding| {
            format!(
                "{} at prompt.parts[{}]",
                finding.pattern_id, finding.source.part_index
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    Some((
        Decision::Warn {
            reason: DenyReason::PolicyViolation {
                policy: "secret_scan".to_owned(),
                detail: Cow::Owned(format!(
                    "Sanitized {} secret findings: {detail}",
                    findings.len()
                )),
            },
        },
        input,
    ))
}

fn classify(
    entry: &ChainEntry,
    decision: &Decision,
    duration_ms: f64,
) -> (ChainEntryOutcome, Option<DenyReason>, bool) {
    let row = |result, detail| ChainEntryOutcome {
        policy_id: entry.instance.id(),
        result,
        detail,
        duration_ms,
    };
    match decision {
        Decision::Allow { matched_by } => (
            row(ChainEntryResult::Pass, allow_detail(matched_by)),
            None,
            false,
        ),
        Decision::Deny { reason } if entry.config.mode.is_warn() => {
            tracing::warn!(
                policy = %entry.config.id,
                reason = %reason,
                "governance policy in warn mode would have denied this call; allowing it"
            );
            (
                row(ChainEntryResult::Warn, reason.to_string()),
                Some(reason.clone()),
                false,
            )
        },
        Decision::Deny { reason } => (row(ChainEntryResult::Fail, reason.to_string()), None, true),
        Decision::Warn { reason } => (
            row(ChainEntryResult::Warn, reason.to_string()),
            Some(reason.clone()),
            false,
        ),
        Decision::Pending { reason } => {
            (row(ChainEntryResult::Hold, reason.to_string()), None, true)
        },
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

fn chain_entry(cfg: &PolicyConfig, result: ChainEntryResult, detail: &str) -> ChainEntryOutcome {
    ChainEntryOutcome {
        policy_id: PolicyId::new(cfg.id.clone()),
        result,
        detail: detail.to_owned(),
        duration_ms: 0.0,
    }
}

fn allow_detail(matched_by: &MatchedBy) -> String {
    match matched_by {
        MatchedBy::PolicyAllow { detail, .. } => detail.to_string(),
        MatchedBy::UserAllow => "user allow".to_owned(),
        MatchedBy::RoleAllow { role } => format!("role allow: {role}"),
        MatchedBy::AttributeAllow { rule_type, value } => format!("{rule_type} allow: {value}"),
        MatchedBy::DefaultIncluded => "default included".to_owned(),
    }
}
