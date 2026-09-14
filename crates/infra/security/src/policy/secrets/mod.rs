//! Installation-configured plaintext secret scanning and recovery.
//!
//! [`SecretScanner`] compiles one installation-owned signature catalog and
//! applies it consistently to governance evaluation, gateway responses, and
//! prompt recovery. Entropy detection remains an observation-only heuristic.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod entropy;
mod entropy_yaml;
mod fingerprint;
mod patterns;
mod recovery;
mod signatures;


use regex::Captures;
use systemprompt_identifiers::SecretPatternId;

use super::governed::{GovernedInput, GovernedString};
pub use entropy::{DEFAULT_MIN_LEN, DEFAULT_THRESHOLD, EntropyConfig, find_high_entropy_token};
use patterns::{CompiledSecretPattern, HIGH_ENTROPY_PATTERN_NAME, compile_patterns, field_matches};
pub use patterns::{SecretPattern, SecretPatternError};
pub use recovery::{
    MAX_RECOVERY_FINDINGS, REDACTION_MARKER, SecretFinding, SecretSource, redact_spans,
};
pub use signatures::SignatureExemptions;

#[derive(Debug, Clone)]
pub struct SecretScanner {
    patterns: Vec<CompiledSecretPattern>,
    entropy: EntropyConfig,
}

impl SecretScanner {
    pub fn from_policy_yaml(value: &serde_yaml::Value) -> Result<Self, SecretPatternError> {
        Ok(Self {
            patterns: compile_patterns(value.get("patterns"))?,
            entropy: entropy_yaml::from_yaml(value),
        })
    }

    #[must_use]
    pub const fn pattern_count(&self) -> usize {
        self.patterns.len()
    }

    #[must_use]
    pub const fn entropy(&self) -> &EntropyConfig {
        &self.entropy
    }

    #[must_use]
    pub fn detect(&self, input: &GovernedInput) -> Option<SecretHit> {
        let strings = input.strings();
        let exemptions = SignatureExemptions::from_strings(&strings);
        strings
            .iter()
            .find_map(|found| self.detect_confirmed(found))
            .or_else(|| {
                strings.iter().find_map(|found| {
                    if exemptions.exempts_entropy(&found.path) {
                        return None;
                    }
                    entropy::high_entropy_spans(found.value, &self.entropy)
                        .next()
                        .map(|(span, _)| SecretHit {
                            pattern: MatchedSecretPattern {
                                id: SecretPatternId::high_entropy(),
                                name: HIGH_ENTROPY_PATTERN_NAME.to_owned(),
                            },
                            path: found.path.clone(),
                            redacted: redacted_snippet(found.value, span.start, span.end),
                            observation: true,
                        })
                })
            })
    }

    #[must_use]
    pub fn findings(&self, input: &GovernedInput) -> Vec<SecretFinding> {
        recovery::secret_findings(self, input)
    }

    fn detect_confirmed(&self, found: &GovernedString<'_>) -> Option<SecretHit> {
        self.patterns.iter().find_map(|pattern| {
            if !field_matches(&found.path, pattern.definition.field.as_deref()) {
                return None;
            }
            pattern.regex.captures(found.value).and_then(|captures| {
                selected_match(pattern, &captures).map(|matched| SecretHit {
                    pattern: MatchedSecretPattern {
                        id: pattern.definition.id.clone(),
                        name: pattern.definition.name.clone(),
                    },
                    path: found.path.clone(),
                    redacted: redacted_snippet(found.value, matched.start(), matched.end()),
                    observation: false,
                })
            })
        })
    }
}

fn selected_match<'a>(
    pattern: &CompiledSecretPattern,
    captures: &'a Captures<'a>,
) -> Option<regex::Match<'a>> {
    pattern
        .definition
        .secret_capture
        .as_deref()
        .map_or_else(|| captures.get(0), |name| captures.name(name))
}

fn redacted_snippet(value: &str, start: usize, end: usize) -> String {
    format!(
        "fingerprint:{}...[REDACTED]",
        fingerprint::of(&value[start..end])
    )
}

#[derive(Debug)]
pub struct MatchedSecretPattern {
    pub id: SecretPatternId,
    pub name: String,
}

/// A credential or entropy observation found in governed input.
#[derive(Debug)]
pub struct SecretHit {
    pub pattern: MatchedSecretPattern,
    pub path: String,
    pub redacted: String,
    pub observation: bool,
}
