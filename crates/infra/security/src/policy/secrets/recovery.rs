//! Located secret findings for repairing provider-bound prompt text.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::ops::Range;

use systemprompt_identifiers::SecretPatternId;

use super::super::GovernedInput;
use super::{COMPILED, EntropyConfig, SignatureExemptions};

pub const REDACTION_MARKER: &str = "[REDACTED_BY_GOVERNANCE]";
pub const MAX_RECOVERY_FINDINGS: usize = 4096;

/// Index into the ordered string surfaces returned by `GovernedInput::strings`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SecretSource {
    pub part_index: usize,
}

/// Credential-free match metadata; `span` uses UTF-8 byte offsets within its
/// source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretFinding {
    pub source: SecretSource,
    pub span: Range<usize>,
    pub pattern_id: SecretPatternId,
}

pub fn secret_findings(input: &GovernedInput, entropy: &EntropyConfig) -> Vec<SecretFinding> {
    let strings = input.strings();
    let exemptions = SignatureExemptions::from_strings(&strings);
    let mut findings = Vec::new();
    for (part_index, found) in strings.iter().enumerate() {
        for (index, regex) in COMPILED.iter() {
            let pattern = &super::SECRET_PATTERNS[*index];
            for hit in regex.find_iter(found.value) {
                let span = if whole_value_pattern(pattern.id) {
                    0..found.value.len()
                } else {
                    hit.range()
                };
                findings.push(SecretFinding {
                    source: SecretSource { part_index },
                    span,
                    pattern_id: SecretPatternId::new(pattern.id),
                });
                if findings.len() > MAX_RECOVERY_FINDINGS {
                    return findings;
                }
            }
        }
        if !exemptions.exempts_entropy(&found.path) {
            for token in super::entropy::high_entropy_tokens(found.value, entropy) {
                let start = token.as_ptr() as usize - found.value.as_ptr() as usize;
                findings.push(SecretFinding {
                    source: SecretSource { part_index },
                    span: start..start + token.len(),
                    pattern_id: SecretPatternId::new("high-entropy-token"),
                });
                if findings.len() > MAX_RECOVERY_FINDINGS {
                    return findings;
                }
            }
        }
    }
    findings
}

fn whole_value_pattern(id: &str) -> bool {
    id.starts_with("pem-private-key")
        || matches!(
            id,
            "aws-secret-key"
                | "twilio-auth-token"
                | "heroku-api-key"
                | "bearer-token-jwt"
                | "jwt-raw"
        )
}

pub fn redact_spans(value: &str, spans: impl IntoIterator<Item = Range<usize>>) -> Option<String> {
    let mut spans: Vec<_> = spans.into_iter().collect();
    spans.sort_unstable_by_key(|span| (span.start, span.end));
    let mut out = String::new();
    let mut cursor = 0;
    for span in spans {
        if span.start >= span.end || value.get(span.clone()).is_none() {
            return None;
        }
        if span.end <= cursor {
            continue;
        }
        if span.start >= cursor {
            out.push_str(value.get(cursor..span.start)?);
            out.push_str(REDACTION_MARKER);
        }
        cursor = span.end;
    }
    out.push_str(value.get(cursor..)?);
    Some(out)
}
