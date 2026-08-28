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

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::LazyLock;

use systemprompt_config::ProfileBootstrap;
use systemprompt_identifiers::PolicyId;

use super::audit::{ChainEntryOutcome, ChainEntryResult};
use super::config::{GovernanceConfig, PolicyConfig};
use super::registry::{PolicyFactory, PolicyRegistration};
use super::types::{GovernancePolicy, PolicyContext};
use crate::authz::types::{Decision, MatchedBy};

/// The outcome of one traced chain run: the first-deny-wins [`Decision`] and
/// the ordered per-entry trace destined for the audit row.
#[derive(Debug)]
pub struct Evaluation {
    pub decision: Decision,
    pub chain: Vec<ChainEntryOutcome>,
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
    pub fn global() -> &'static Self {
        static ENGINE: LazyLock<GovernanceEngine> = LazyLock::new(|| {
            let config = governance_config_path()
                .map_or_else(GovernanceConfig::defaults, |p| GovernanceConfig::load(&p));
            GovernanceEngine::from_config(&config)
        });
        &ENGINE
    }

    #[must_use]
    pub fn from_config(config: &GovernanceConfig) -> Self {
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
            let Some(factory) = factories.get(cfg.id.as_str()) else {
                tracing::warn!(
                    policy = %cfg.id,
                    "governance policy in config has no registered impl — skipping"
                );
                continue;
            };
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
                params: serde_yaml::Value::Null,
            };
            let instance = (r.factory)(&cfg.params);
            entries.push(ChainEntry {
                config: cfg,
                instance,
            });
        }

        Self {
            enabled: config.enabled,
            entries,
        }
    }

    pub fn policies(&self) -> impl Iterator<Item = (&PolicyConfig, &dyn GovernancePolicy)> {
        self.entries
            .iter()
            .map(|e| (&e.config, e.instance.as_ref()))
    }

    #[must_use]
    pub fn evaluate(&self, ctx: &PolicyContext<'_>) -> Evaluation {
        if !self.enabled {
            return Evaluation {
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
            };
        }

        let mut chain: Vec<ChainEntryOutcome> = Vec::with_capacity(self.entries.len());
        // Why: `Pending` halts the chain for the same reason `Deny` does — a
        // later policy cannot un-hold a call, and running it would charge the
        // rate limiter for a call that has not been authorised yet.
        let mut halted: Option<Decision> = None;

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
            let decision = entry.instance.evaluate(ctx);
            let duration_ms = started.elapsed().as_secs_f64() * 1000.0;
            match &decision {
                Decision::Allow { matched_by } => chain.push(ChainEntryOutcome {
                    policy_id: entry.instance.id(),
                    result: ChainEntryResult::Pass,
                    detail: allow_detail(matched_by),
                    duration_ms,
                }),
                Decision::Deny { reason } => {
                    chain.push(ChainEntryOutcome {
                        policy_id: entry.instance.id(),
                        result: ChainEntryResult::Fail,
                        detail: reason.to_string(),
                        duration_ms,
                    });
                    halted = Some(decision);
                },
                Decision::Pending { reason } => {
                    chain.push(ChainEntryOutcome {
                        policy_id: entry.instance.id(),
                        result: ChainEntryResult::Hold,
                        detail: reason.to_string(),
                        duration_ms,
                    });
                    halted = Some(decision);
                },
            }
        }

        Evaluation {
            decision: halted.unwrap_or(Decision::Allow {
                matched_by: MatchedBy::DefaultIncluded,
            }),
            chain,
        }
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
