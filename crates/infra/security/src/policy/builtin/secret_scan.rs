//! `secret_scan`: refuse governed input containing a plaintext credential
//! matching one of the built-in patterns. That input is a tool call's
//! arguments or, for a prompt submission, the prompt itself — a key pasted
//! into the chat reaches the model exactly as one passed as a tool argument
//! would. The pattern list ships with the binary; per-deployment additions go
//! in the governance config under `policies[id=secret_scan].extra_patterns`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::borrow::Cow;

use regex::Regex;
use serde_yaml::Value as YamlValue;
use systemprompt_identifiers::{PolicyId, SecretPatternId};

use super::super::registry::PolicyRegistration;
use super::super::secrets::{EntropyConfig, detect_secrets_with};
use super::super::types::{GovernancePolicy, PolicyContext, SecretLocation};
use crate::authz::types::{Decision, DenyReason, MatchedBy};

const ID: &str = "secret_scan";

#[derive(Debug, Clone)]
struct ExtraPattern {
    id: String,
    name: String,
    prefix: String,
}

#[derive(Debug)]
struct SecretScan {
    extra_patterns: Vec<ExtraPattern>,
    entropy: EntropyConfig,
}

// Why: an absent block, an absent key, or a key of the wrong shape each fall
// back to the built-in default — a typo must not silently disable credential
// detection.
fn entropy_from_yaml(v: &YamlValue) -> EntropyConfig {
    let defaults = EntropyConfig::default();
    let Some(block) = v.get("entropy") else {
        return defaults;
    };
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

impl SecretScan {
    fn from_yaml(v: &YamlValue) -> Self {
        let extras = v
            .get("extra_patterns")
            .and_then(|s| s.as_sequence())
            .map(|seq| {
                let mut out: Vec<ExtraPattern> = Vec::new();
                for entry in seq {
                    let Some(name) = entry.get("name").and_then(|n| n.as_str()) else {
                        continue;
                    };
                    let Some(prefix) = entry.get("prefix").and_then(|n| n.as_str()) else {
                        continue;
                    };
                    let id = slugify(name);
                    if out.iter().any(|p| p.id == id) {
                        tracing::error!(
                            extra_pattern_name = %name,
                            extra_pattern_id = %id,
                            "secret_scan: duplicate extra_pattern id derived from name; \
                             keeping first occurrence and skipping the duplicate"
                        );
                        continue;
                    }
                    out.push(ExtraPattern {
                        id,
                        name: name.to_owned(),
                        prefix: prefix.to_owned(),
                    });
                }
                out
            })
            .unwrap_or_default();
        Self {
            extra_patterns: extras,
            entropy: entropy_from_yaml(v),
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
        "Block a tool call or submitted prompt containing an AWS key, GitHub PAT, \
         PEM block, connection string, or other plaintext credential pattern."
    }
    fn evaluate(&self, ctx: &PolicyContext<'_>) -> Decision {
        let kind = ctx.input.location_kind();
        if let Some(hit) = detect_secrets_with(ctx.input, &self.entropy) {
            return Decision::Deny {
                reason: DenyReason::SecretLeak {
                    pattern_id: SecretPatternId::new(hit.pattern.id),
                    pattern_name: Cow::Borrowed(hit.pattern.name),
                    location: SecretLocation::new(kind, hit.path, hit.redacted),
                },
            };
        }
        for found in ctx.input.strings() {
            for extra in &self.extra_patterns {
                if found.value.contains(extra.prefix.as_str()) {
                    return Decision::Deny {
                        reason: DenyReason::SecretLeak {
                            pattern_id: SecretPatternId::new(extra.id.clone()),
                            pattern_name: Cow::Owned(extra.name.clone()),
                            location: SecretLocation::new(kind, found.path, "custom_pattern"),
                        },
                    };
                }
            }
        }
        Decision::Allow {
            matched_by: MatchedBy::PolicyAllow {
                policy_id: PolicyId::new(ID),
                detail: Cow::Borrowed("No plaintext secrets detected in governed input"),
            },
        }
    }
}

fn slugify(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut last_was_dash = false;
    for ch in input.chars() {
        let mapped = if ch.is_ascii_alphanumeric() {
            Some(ch.to_ascii_lowercase())
        } else if ch.is_whitespace() || matches!(ch, '_' | '-' | '/' | '(' | ')' | '.') {
            Some('-')
        } else {
            None
        };
        if let Some(c) = mapped {
            if c == '-' {
                if !last_was_dash && !out.is_empty() {
                    out.push('-');
                    last_was_dash = true;
                }
            } else {
                out.push(c);
                last_was_dash = false;
            }
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

inventory::submit! {
    PolicyRegistration {
        id: ID,
        factory: |v| Box::new(SecretScan::from_yaml(v)),
    }
}
