//! `secret_scan`: evaluate governed input with the installation's configured
//! credential catalog.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::borrow::Cow;

use regex::Regex;
use serde_yaml::Value as YamlValue;
use systemprompt_identifiers::{PolicyId, SecretPatternId};

use super::super::governed::GovernedInput;
use super::super::registry::PolicyRegistration;
use super::super::secrets::{EntropyConfig, MAX_RECOVERY_FINDINGS, SecretFinding, SecretScanner};
use super::super::types::{GovernancePolicy, PolicyContext, SecretLocation};
use super::SECRET_SCAN_ID as ID;
use crate::authz::types::{Decision, DenyReason, MatchedBy};

#[derive(Debug)]
struct SecretScan {
    scanner: SecretScanner,
}

const ENTROPY_KEYS: [&str; 4] = ["enabled", "min_len", "threshold", "allowlist"];

pub(crate) fn entropy_from_yaml(v: &YamlValue) -> EntropyConfig {
    let defaults = EntropyConfig::default();
    let Some(block) = v.get("entropy") else {
        return defaults;
    };
    report_entropy_block_typos(block);
    let allowlist = block
        .get("allowlist")
        .and_then(YamlValue::as_sequence)
        .map(|seq| {
            seq.iter()
                .filter_map(YamlValue::as_str)
                .filter_map(|expr| match Regex::new(expr) {
                    Ok(re) => Some(re),
                    Err(error) => {
                        tracing::error!(
                            %expr,
                            %error,
                            "secret_scan: entropy.allowlist entry skipped; regex failed to compile"
                        );
                        None
                    },
                })
                .collect()
        })
        .unwrap_or(defaults.allowlist);
    EntropyConfig {
        enabled: block
            .get("enabled")
            .and_then(YamlValue::as_bool)
            .unwrap_or(defaults.enabled),
        min_len: block
            .get("min_len")
            .and_then(YamlValue::as_u64)
            .and_then(|n| usize::try_from(n).ok())
            .unwrap_or(defaults.min_len),
        threshold: block
            .get("threshold")
            .and_then(YamlValue::as_f64)
            .unwrap_or(defaults.threshold),
        allowlist,
    }
}

fn report_entropy_block_typos(block: &YamlValue) {
    let Some(map) = block.as_mapping() else {
        tracing::error!(
            "secret_scan: `entropy` is not a mapping; the block is ignored and \
             built-in defaults apply"
        );
        return;
    };
    for key in map.keys() {
        let name = key.as_str().unwrap_or("<non-string>");
        if !ENTROPY_KEYS.contains(&name) {
            tracing::error!(
                key = %name,
                "secret_scan: unknown `entropy` key ignored; valid keys are \
                 enabled, min_len, threshold, allowlist"
            );
        }
    }
    let wrong_shape = [
        (
            "enabled",
            map.get("enabled").is_some_and(|v| v.as_bool().is_none()),
        ),
        (
            "min_len",
            map.get("min_len").is_some_and(|v| v.as_u64().is_none()),
        ),
        (
            "threshold",
            map.get("threshold").is_some_and(|v| v.as_f64().is_none()),
        ),
        (
            "allowlist",
            map.get("allowlist")
                .is_some_and(|v| v.as_sequence().is_none()),
        ),
    ];
    for (name, mistyped) in wrong_shape {
        if mistyped {
            tracing::error!(
                key = %name,
                "secret_scan: entropy key has the wrong type; the built-in \
                 default is used instead"
            );
        }
    }
}

impl SecretScan {
    fn from_yaml(v: &YamlValue) -> Self {
        Self {
            scanner: SecretScanner::from_policy_yaml(v)
                .expect("secret scanner configuration was validated by GovernanceEngine"),
        }
    }
}

impl GovernancePolicy for SecretScan {
    fn id(&self) -> PolicyId {
        PolicyId::new(ID)
    }
    fn name(&self) -> &'static str {
        "Secret Scan"
    }
    fn description(&self) -> &'static str {
        "Detect configured plaintext credential signatures in tool calls and submitted prompts."
    }
    fn prompt_secret_findings(&self, input: &GovernedInput) -> Option<Vec<SecretFinding>> {
        let mut findings = self.scanner.findings(input);
        findings.retain(|finding| finding.pattern_id.as_str() != "high-entropy-token");
        findings.truncate(MAX_RECOVERY_FINDINGS + 1);
        Some(findings)
    }
    fn secret_scanner(&self) -> Option<&SecretScanner> {
        Some(&self.scanner)
    }
    fn evaluate(&self, ctx: &PolicyContext<'_>) -> Decision {
        let kind = ctx.input.location_kind();
        let hit = self.scanner.detect(ctx.input);
        if let Some(hit) = hit.as_ref().filter(|hit| !hit.observation) {
            return Decision::Deny {
                reason: DenyReason::SecretLeak {
                    pattern_id: SecretPatternId::new(hit.pattern.id.clone()),
                    pattern_name: Cow::Owned(hit.pattern.name.clone()),
                    location: SecretLocation::new(kind, hit.path.clone(), hit.redacted.clone()),
                },
            };
        }
        Decision::Allow {
            matched_by: MatchedBy::PolicyAllow {
                policy_id: PolicyId::new(ID),
                detail: hit.map_or(
                    Cow::Borrowed("No plaintext secrets detected in governed input"),
                    |hit| {
                        Cow::Owned(format!(
                            "observation: high-entropy-token at {} (unconfirmed; allow; {})",
                            hit.path, hit.redacted
                        ))
                    },
                ),
            },
        }
    }
}

inventory::submit! {
    PolicyRegistration {
        id: ID,
        factory: |v| Box::new(SecretScan::from_yaml(v)),
    }
}
