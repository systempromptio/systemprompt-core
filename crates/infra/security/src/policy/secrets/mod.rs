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
mod patterns;
mod signatures;

use std::sync::LazyLock;

use regex::Regex;

use super::governed::GovernedInput;
pub use entropy::{DEFAULT_MIN_LEN, DEFAULT_THRESHOLD, EntropyConfig, find_high_entropy_token};
use patterns::HIGH_ENTROPY_PATTERN;
pub use patterns::{SECRET_PATTERNS, SecretPattern};
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

fn redacted_snippet(s: &str, match_start: usize) -> String {
    let mut snippet_end = (match_start + 12).min(s.len());
    while !s.is_char_boundary(snippet_end) {
        snippet_end -= 1;
    }
    format!("{}...[REDACTED]", &s[match_start..snippet_end])
}

fn scan_patterns(s: &str) -> Option<(&'static SecretPattern, String)> {
    COMPILED.iter().find_map(|(i, re)| {
        re.find(s)
            .map(|m| (&SECRET_PATTERNS[*i], redacted_snippet(s, m.start())))
    })
}

fn scan_str(s: &str, entropy: &EntropyConfig) -> Option<(&'static SecretPattern, String)> {
    scan_patterns(s).or_else(|| {
        find_high_entropy_token(s, entropy).map(|token| {
            let start = token.as_ptr() as usize - s.as_ptr() as usize;
            (&HIGH_ENTROPY_PATTERN, redacted_snippet(s, start))
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
    let strings = input.strings();
    let exemptions = SignatureExemptions::from_strings(&strings);
    strings.into_iter().find_map(|s| {
        let found = if exemptions.exempts_entropy(&s.path) {
            scan_patterns(s.value)
        } else {
            scan_str(s.value, entropy)
        };
        found.map(|(pattern, redacted)| SecretHit {
            pattern,
            path: s.path,
            redacted,
        })
    })
}
