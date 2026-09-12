//! Located secret findings for repairing provider-bound prompt text.
//!
//! [`secret_findings`] reports every credential the scanner would deny on, as
//! byte spans into the governed strings, and [`redact_spans`] applies them.
//! A finding never carries the credential itself, so the list is safe to log
//! and to render into a deny message. Collection stops one past
//! [`MAX_RECOVERY_FINDINGS`]: a prompt with more credentials than that is
//! not repaired, and the caller only needs to know the cap was exceeded.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::ops::Range;

use systemprompt_identifiers::SecretPatternId;

use super::super::GovernedInput;
use super::patterns::HIGH_ENTROPY_PATTERN;
use super::{COMPILED, EntropyConfig, SECRET_PATTERNS, SignatureExemptions};

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

#[must_use]
pub fn secret_findings(input: &GovernedInput, entropy: &EntropyConfig) -> Vec<SecretFinding> {
    let strings = input.strings();
    let exemptions = SignatureExemptions::from_strings(&strings);
    strings
        .iter()
        .enumerate()
        .flat_map(|(part_index, found)| {
            let source = SecretSource { part_index };
            let value = found.value;
            let patterns = COMPILED.iter().flat_map(move |(index, regex)| {
                let pattern = &SECRET_PATTERNS[*index];
                regex.captures_iter(value).filter_map(move |caps| {
                    let hit = caps.name("secret").or_else(|| caps.get(0))?;
                    Some(SecretFinding {
                        source,
                        span: if pattern.redact_whole_value {
                            0..value.len()
                        } else {
                            hit.range()
                        },
                        pattern_id: SecretPatternId::new(pattern.id),
                    })
                })
            });
            let tokens = (!exemptions.exempts_entropy(&found.path))
                .then(|| {
                    super::entropy::high_entropy_spans(value, entropy).map(move |(span, _)| {
                        SecretFinding {
                            source,
                            span,
                            pattern_id: SecretPatternId::new(HIGH_ENTROPY_PATTERN.id),
                        }
                    })
                })
                .into_iter()
                .flatten();
            let assigned = super::aws_value_at(value, &found.path).then(|| SecretFinding {
                source,
                span: 0..value.len(),
                pattern_id: SecretPatternId::new("aws-secret-key"),
            });
            patterns.chain(assigned).chain(tokens)
        })
        .take(MAX_RECOVERY_FINDINGS + 1)
        .collect()
}

#[must_use]
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
