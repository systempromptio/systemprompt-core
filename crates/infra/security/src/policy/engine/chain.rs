//! The traced chain walk behind [`GovernanceEngine::evaluate`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::borrow::Cow;
use std::time::Instant;

use systemprompt_identifiers::PolicyId;

use super::{ChainEntry, Evaluation, GovernanceEngine};
use crate::authz::types::{Decision, DenyReason, MatchedBy};
use crate::policy::audit::{ChainEntryOutcome, ChainEntryResult};
use crate::policy::config::PolicyConfig;
use crate::policy::governed::{GovernedInput, GovernedTarget};
use crate::policy::secrets::{MAX_RECOVERY_FINDINGS, SecretFinding};
use crate::policy::types::PolicyContext;

type PromptRecovery<'a> = &'a mut dyn FnMut(&[SecretFinding]) -> Option<GovernedInput>;

struct Classified {
    outcome: ChainEntryOutcome,
    warn: Option<DenyReason>,
    halt: bool,
}

impl GovernanceEngine {
    pub(super) fn evaluate_chain(
        &self,
        ctx: &PolicyContext<'_>,
        mut recover: Option<PromptRecovery<'_>>,
    ) -> Evaluation {
        if !self.enabled {
            return self.master_switch_off();
        }

        let mut chain = Vec::with_capacity(self.entries.len());
        let mut halted: Option<Decision> = None;
        let mut first_warn: Option<DenyReason> = None;
        let mut repaired_input: Option<GovernedInput> = None;

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
            let started = Instant::now();
            let current = ctx.with_input(repaired_input.as_ref().unwrap_or(ctx.input));
            let mut decision = entry.instance.evaluate(&current);
            if let Some(recover) = recover.as_deref_mut()
                && let Some((recovered, input)) =
                    recover_secret(entry, &current, &decision, recover)
            {
                decision = recovered;
                repaired_input = Some(input);
            }
            let duration_ms = started.elapsed().as_secs_f64() * 1000.0;
            let classified = classify(entry, &decision, duration_ms);
            chain.push(classified.outcome);
            if first_warn.is_none() {
                first_warn = classified.warn;
            }
            if classified.halt {
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
}

fn recover_secret(
    entry: &ChainEntry,
    ctx: &PolicyContext<'_>,
    decision: &Decision,
    recover: PromptRecovery<'_>,
) -> Option<(Decision, GovernedInput)> {
    let leaked = matches!(
        decision,
        Decision::Deny {
            reason: DenyReason::SecretLeak { .. }
        }
    );
    if ctx.target != GovernedTarget::Prompt || entry.config.mode.is_warn() || !leaked {
        return None;
    }
    let findings = entry.instance.prompt_secret_findings(ctx.input)?;
    if findings.is_empty() || findings.len() > MAX_RECOVERY_FINDINGS {
        return None;
    }
    let input = recover(&findings)?;
    let verified = entry.instance.evaluate(&ctx.with_input(&input));
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
    let reason = DenyReason::PolicyViolation {
        policy: entry.config.id.clone(),
        detail: Cow::Owned(format!(
            "Sanitized {} secret findings: {detail}",
            findings.len()
        )),
    };
    Some((Decision::Warn { reason }, input))
}

fn classify(entry: &ChainEntry, decision: &Decision, duration_ms: f64) -> Classified {
    let row = |result, detail| ChainEntryOutcome {
        policy_id: entry.instance.id(),
        result,
        detail,
        duration_ms,
    };
    let classified = |outcome, warn, halt| Classified {
        outcome,
        warn,
        halt,
    };
    match decision {
        Decision::Allow { matched_by } => classified(
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
            classified(
                row(ChainEntryResult::Warn, reason.to_string()),
                Some(reason.clone()),
                false,
            )
        },
        Decision::Deny { reason } => {
            classified(row(ChainEntryResult::Fail, reason.to_string()), None, true)
        },
        Decision::Warn { reason } => classified(
            row(ChainEntryResult::Warn, reason.to_string()),
            Some(reason.clone()),
            false,
        ),
        Decision::Pending { reason } => {
            classified(row(ChainEntryResult::Hold, reason.to_string()), None, true)
        },
    }
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
