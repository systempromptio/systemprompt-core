//! YAML schema for the declarative gateway-policy baseline.
//!
//! A deployment commits a [`GatewayPolicyConfig`] at
//! `services/gateway/policies.yaml` declaring the gateway policies every
//! instance should boot with. The bootstrap loader parses this struct, hands
//! it to [`super::ingestion::GatewayPolicyIngestionService`], and the service
//! projects it into `ai_gateway_policies`.
//!
//! The contract is one-way (YAML → DB), mirroring the access-control
//! ingestion path.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

use super::error::GatewayPolicyError;
use super::spec::{GatewayPolicySpec, QuotaWindow, RateWindow, SafetyConfig};

const fn default_enabled() -> bool {
    true
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GatewayPolicyConfig {
    #[serde(default)]
    pub policies: Vec<GatewayPolicyEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GatewayPolicyEntry {
    pub name: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub priority: i32,
    #[serde(default)]
    pub spec: GatewayPolicySpec,
}

impl GatewayPolicyConfig {
    pub fn validate(&self) -> Result<(), GatewayPolicyError> {
        let mut seen = std::collections::HashSet::with_capacity(self.policies.len());
        for (idx, policy) in self.policies.iter().enumerate() {
            if policy.name.trim().is_empty() {
                return Err(GatewayPolicyError::Invalid {
                    field: format!("policies[{idx}].name"),
                    reason: "policy name must not be empty".to_owned(),
                });
            }
            if !seen.insert(policy.name.as_str()) {
                return Err(GatewayPolicyError::Invalid {
                    field: format!("policies[{idx}].name"),
                    reason: format!("duplicate policy name '{}'", policy.name),
                });
            }
            let safety = &policy.spec.safety;
            if safety.scanners.iter().any(|s| s == "heuristic")
                && crate::policies::safety::effective_phrases(&safety.heuristic).is_empty()
            {
                return Err(GatewayPolicyError::Invalid {
                    field: format!("policies[{idx}].spec.safety.heuristic"),
                    reason: "heuristic scanner is enabled but its effective phrase list is \
                             empty — set phrases/extra_phrases or remove the scanner"
                        .to_owned(),
                });
            }
            validate_safety_settings(idx, safety)?;
            validate_quota_windows(idx, &policy.spec.quota_windows)?;
            validate_rate_windows(idx, &policy.spec.rate_windows)?;
        }
        Ok(())
    }
}

fn validate_safety_settings(idx: usize, safety: &SafetyConfig) -> Result<(), GatewayPolicyError> {
    if let Some(category) = safety
        .redact_categories
        .iter()
        .find(|c| safety.block_categories.contains(c))
    {
        return Err(GatewayPolicyError::Invalid {
            field: format!("policies[{idx}].spec.safety.redact_categories"),
            reason: format!(
                "category '{category}' is in both block_categories and redact_categories; a \
                 category is either refused or redacted"
            ),
        });
    }
    for (name, settings) in &safety.scanner_settings {
        let field = format!("policies[{idx}].spec.safety.scanner_settings.{name}");
        if !safety.scanners.iter().any(|s| s == name) {
            return Err(GatewayPolicyError::Invalid {
                field,
                reason: format!(
                    "settings name scanner '{name}', which is not listed in safety.scanners"
                ),
            });
        }
        if settings.timeout_ms == 0 {
            return Err(GatewayPolicyError::Invalid {
                field: format!("{field}.timeout_ms"),
                reason: "timeout_ms must be at least 1".to_owned(),
            });
        }
    }
    Ok(())
}

fn validate_rate_windows(idx: usize, windows: &[RateWindow]) -> Result<(), GatewayPolicyError> {
    for (w, window) in windows.iter().enumerate() {
        if window.window_seconds == 0 || window.max_requests == 0 {
            return Err(GatewayPolicyError::Invalid {
                field: format!("policies[{idx}].spec.rate_windows[{w}]"),
                reason: "window_seconds and max_requests must both be at least 1".to_owned(),
            });
        }
    }
    Ok(())
}

fn validate_quota_windows(idx: usize, windows: &[QuotaWindow]) -> Result<(), GatewayPolicyError> {
    for (w, window) in windows.iter().enumerate() {
        window
            .validate_span()
            .map_err(|reason| GatewayPolicyError::Invalid {
                field: format!("policies[{idx}].spec.quota_windows[{w}]"),
                reason: reason.to_owned(),
            })?;
    }
    Ok(())
}
