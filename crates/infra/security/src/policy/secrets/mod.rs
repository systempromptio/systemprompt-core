//! Built-in plaintext secret-pattern registry and scanner.
//!
//! [`SECRET_PATTERNS`] holds the vendor-prefix ruleset (seeded from the
//! gitleaks MIT ruleset); [`find_high_entropy_token`] backstops it: a
//! credential with no recognisable vendor prefix — a random base64 blob pasted
//! into a prompt — matches no pattern but still reads as machine-generated key
//! material, and is reported under the pseudo-pattern id `high-entropy-token`.
//!
//! [`detect_secrets`] drives the `secret_scan` builtin policy;
//! [`scan_str_for_secret`] is the string-level entry point shared with gateway
//! safety scanners so every enforcement surface flags the same credentials.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod patterns;

use std::sync::LazyLock;

use regex::Regex;

use super::governed::GovernedInput;
use patterns::HIGH_ENTROPY_PATTERN;
pub use patterns::{SECRET_PATTERNS, SecretPattern};

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

/// Number of built-in patterns whose regex compiled. Pinned equal to
/// `SECRET_PATTERNS.len()` by the test suite so a broken expression cannot
/// silently disable a pattern.
#[must_use]
pub fn compiled_pattern_count() -> usize {
    COMPILED.len()
}

const ENTROPY_MIN_LEN: usize = 32;

const ENTROPY_THRESHOLD: f64 = 4.0;

fn shannon_entropy(s: &str) -> f64 {
    let mut counts = [0u32; 256];
    let bytes = s.as_bytes();
    for &b in bytes {
        counts[usize::from(b)] += 1;
    }
    let len = bytes.len() as f64;
    counts
        .iter()
        .filter(|&&c| c > 0)
        .map(|&c| {
            let p = f64::from(c) / len;
            -p * p.log2()
        })
        .sum()
}

#[must_use]
pub fn find_high_entropy_token(text: &str) -> Option<&str> {
    text.split(|c: char| c.is_whitespace() || "\"'`()[]{}<>,;:".contains(c))
        .find(|token| {
            token.len() >= ENTROPY_MIN_LEN
                && token
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "+/=_-".contains(c))
                && token.chars().any(|c| c.is_ascii_uppercase())
                && token.chars().any(|c| c.is_ascii_lowercase())
                && token.chars().any(|c| c.is_ascii_digit())
                && shannon_entropy(token) >= ENTROPY_THRESHOLD
        })
}

fn redacted_snippet(s: &str, match_start: usize) -> String {
    let mut snippet_end = (match_start + 12).min(s.len());
    while !s.is_char_boundary(snippet_end) {
        snippet_end -= 1;
    }
    format!("{}...[REDACTED]", &s[match_start..snippet_end])
}

fn scan_str(s: &str) -> Option<(&'static SecretPattern, String)> {
    for (i, re) in COMPILED.iter() {
        if let Some(m) = re.find(s) {
            return Some((&SECRET_PATTERNS[*i], redacted_snippet(s, m.start())));
        }
    }
    find_high_entropy_token(s).map(|token| {
        let start = token.as_ptr() as usize - s.as_ptr() as usize;
        (&HIGH_ENTROPY_PATTERN, redacted_snippet(s, start))
    })
}

#[must_use]
pub fn scan_str_for_secret(text: &str) -> Option<String> {
    scan_str(text).map(|(_, redacted)| redacted)
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
    input.strings().into_iter().find_map(|s| {
        scan_str(s.value).map(|(pattern, redacted)| SecretHit {
            pattern,
            path: s.path,
            redacted,
        })
    })
}
