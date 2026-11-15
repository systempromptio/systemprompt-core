//! Evaluate governed input against the installation's configured credential
//! catalog; registered as the `secret_scan` policy.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::borrow::Cow;
use std::sync::Arc;

use serde_yaml::Value as YamlValue;
use systemprompt_identifiers::PolicyId;

use super::super::registry::{BuiltPolicy, BuiltinPolicy, PolicyConfigurationError};
use super::super::secrets::{MAX_RECOVERY_FINDINGS, SecretScanner};
use super::SECRET_SCAN_ID as ID;
use crate::authz::types::{Decision, DenyReason, MatchedBy};
use crate::policy::{
    GovernancePolicy, GovernedInput, PolicyContext, SecretFinding, SecretLocation,
};

#[derive(Debug)]
struct SecretScan {
    scanner: SecretScanner,
}

impl SecretScan {
    fn from_yaml(v: &YamlValue) -> Result<Self, PolicyConfigurationError> {
        SecretScanner::from_policy_yaml(v)
            .map(|scanner| Self { scanner })
            .map_err(PolicyConfigurationError::SecretPatterns)
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
    fn evaluate(&self, ctx: &PolicyContext<'_>) -> Decision {
        let kind = ctx.input.location_kind();
        let hit = self.scanner.detect(ctx.input);
        if let Some(hit) = hit.as_ref().filter(|hit| !hit.observation) {
            return Decision::Deny {
                reason: DenyReason::SecretLeak {
                    pattern_id: hit.pattern.id.clone(),
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

pub(super) const POLICY: BuiltinPolicy = BuiltinPolicy {
    id: ID,
    factory: build,
};

fn build(v: &YamlValue) -> Result<BuiltPolicy, PolicyConfigurationError> {
    let policy = SecretScan::from_yaml(v)?;
    let scanner = policy.scanner.clone();
    Ok(BuiltPolicy::new(Arc::new(policy)).with_scanner(scanner))
}
