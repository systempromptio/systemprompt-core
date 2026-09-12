//! Built-in plaintext secret-pattern registry and scanner.
//!
//! [`SECRET_PATTERNS`] holds the vendor-prefix ruleset (seeded from the
//! gitleaks MIT ruleset); [`find_high_entropy_token`] backstops it: a
//! credential with no recognisable vendor prefix — a random base64 blob pasted
//! into a prompt — matches no pattern but still reads as machine-generated key
//! material, and is reported under the pseudo-pattern id `high-entropy-token`.
//!
//! [`SignatureExemptions`] narrows the backstop: a provider-signed reasoning
//! blob the client must echo back verbatim is not a credential, so the entropy
//! detector is suppressed at those paths while every vendor pattern still runs.
//!
//! [`detect_secrets`] drives the `secret_scan` builtin policy;
//! [`scan_str_for_secret`] is the string-level entry point shared with gateway
//! safety scanners so every enforcement surface flags the same credentials.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod entropy;
mod fingerprint;
mod patterns;
mod recovery;
mod signatures;

use std::sync::LazyLock;

use regex::Regex;

use super::governed::GovernedInput;
pub use entropy::{DEFAULT_MIN_LEN, DEFAULT_THRESHOLD, EntropyConfig, find_high_entropy_token};
use patterns::HIGH_ENTROPY_PATTERN;
pub use patterns::{SECRET_PATTERNS, SecretPattern};
pub use recovery::{
    MAX_RECOVERY_FINDINGS, REDACTION_MARKER, SecretFinding, SecretSource, redact_spans,
    secret_findings,
};
pub use signatures::SignatureExemptions;

static DEFAULT_ENTROPY: LazyLock<EntropyConfig> = LazyLock::new(EntropyConfig::default);

static COMPILED: LazyLock<Vec<(usize, Regex)>> = LazyLock::new(|| {
    SECRET_PATTERNS
        .iter()
        .enumerate()
        .filter_map(|(i, p)| match Regex::new(p.expr) {
            Ok(re) => Some((i, re)),
            Err(e) => {
                tracing::error!(pattern_id = %p.id, error = %e, "secret pattern disabled: regex failed to compile");
                None
            },
        })
        .collect()
});

#[must_use]
pub fn compiled_pattern_count() -> usize {
    COMPILED.len()
}

fn redacted_snippet(s: &str, start: usize, end: usize) -> String {
    format!(
        "fingerprint:{}...[REDACTED]",
        fingerprint::of(&s[start..end])
    )
}

pub(super) fn aws_value_at(value: &str, path: &str) -> bool {
    path.rsplit('.')
        .next()
        .is_some_and(|key| key.eq_ignore_ascii_case("aws_secret_access_key"))
        && value.len() == 40
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/+= ".contains(&b) && b != b' ')
}

fn scan_patterns(s: &str) -> Option<(&'static SecretPattern, String)> {
    scan_patterns_excluding(s, &[])
}

fn scan_patterns_excluding(
    s: &str,
    excluded: &[systemprompt_identifiers::SecretPatternId],
) -> Option<(&'static SecretPattern, String)> {
    COMPILED
        .iter()
        .filter(|(i, _)| {
            !excluded
                .iter()
                .any(|id| id.as_str() == SECRET_PATTERNS[*i].id)
        })
        .find_map(|(i, re)| {
            re.captures(s).and_then(|caps| {
                caps.name("secret").or_else(|| caps.get(0)).map(|m| {
                    (
                        &SECRET_PATTERNS[*i],
                        redacted_snippet(s, m.start(), m.end()),
                    )
                })
            })
        })
}

fn scan_str(s: &str, entropy: &EntropyConfig) -> Option<(&'static SecretPattern, String)> {
    scan_patterns(s).or_else(|| {
        entropy::high_entropy_spans(s, entropy)
            .next()
            .map(|(span, _)| {
                (
                    &HIGH_ENTROPY_PATTERN,
                    redacted_snippet(s, span.start, span.end),
                )
            })
    })
}

#[must_use]
pub fn scan_str_for_secret(text: &str) -> Option<String> {
    scan_str(text, &DEFAULT_ENTROPY).map(|(_, redacted)| redacted)
}

/// One credential found in a governed input: the pattern that fired, the
/// dotted JSON path it fired at, and a truncated redacted snippet safe for
/// deny messages and audit rows.
#[derive(Debug)]
pub struct SecretHit {
    pub pattern: &'static SecretPattern,
    pub path: String,
    pub redacted: String,
}

#[must_use]
pub fn detect_secrets(input: &GovernedInput) -> Option<SecretHit> {
    detect_secrets_with(input, &DEFAULT_ENTROPY)
}

#[must_use]
pub fn detect_secrets_with(input: &GovernedInput, entropy: &EntropyConfig) -> Option<SecretHit> {
    detect_secrets_with_exclusions(input, entropy, &[])
}

pub fn detect_secrets_with_exclusions(
    input: &GovernedInput,
    entropy: &EntropyConfig,
    excluded: &[systemprompt_identifiers::SecretPatternId],
) -> Option<SecretHit> {
    let strings = input.strings();
    let exemptions = SignatureExemptions::from_strings(&strings);
    let hit = |s: &super::governed::GovernedString<'_>, pattern, redacted| SecretHit {
        pattern,
        path: s.path.clone(),
        redacted,
    };
    strings
        .iter()
        .find_map(|s| {
            scan_patterns_excluding(s.value, excluded)
                .map(|(pattern, redacted)| hit(s, pattern, redacted))
                .or_else(|| {
                    (aws_value_at(s.value, &s.path)
                        && !excluded.iter().any(|id| id.as_str() == "aws-secret-key"))
                    .then(|| {
                        hit(
                            s,
                            &SECRET_PATTERNS[1],
                            redacted_snippet(s.value, 0, s.value.len()),
                        )
                    })
                })
        })
        .or_else(|| {
            strings.iter().find_map(|s| {
                if exemptions.exempts_entropy(&s.path) {
                    return None;
                }
                entropy::high_entropy_spans(s.value, entropy)
                    .next()
                    .map(|(span, _)| {
                        hit(
                            s,
                            &HIGH_ENTROPY_PATTERN,
                            redacted_snippet(s.value, span.start, span.end),
                        )
                    })
            })
        })
}
