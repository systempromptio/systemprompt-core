//! Located secret findings for repairing provider-bound prompt text.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::ops::Range;

use systemprompt_identifiers::SecretPatternId;

use super::super::GovernedInput;
use super::patterns::{HIGH_ENTROPY_PATTERN_ID, field_matches};
use super::{SecretScanner, SignatureExemptions, selected_match};

pub const REDACTION_MARKER: &str = "[REDACTED_BY_GOVERNANCE]";
pub const MAX_RECOVERY_FINDINGS: usize = 4096;

/// Index into the ordered string surfaces returned by `GovernedInput::strings`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SecretSource {
    pub part_index: usize,
}

/// Credential-free match metadata with UTF-8 byte offsets into its source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretFinding {
    pub source: SecretSource,
    pub span: Range<usize>,
    pub pattern_id: SecretPatternId,
}

pub(super) fn secret_findings(
    scanner: &SecretScanner,
    input: &GovernedInput,
) -> Vec<SecretFinding> {
    let strings = input.strings();
    let exemptions = SignatureExemptions::from_strings(&strings);
    strings
        .iter()
        .enumerate()
        .flat_map(|(part_index, found)| {
            let source = SecretSource { part_index };
            let value = found.value;
            let patterns = scanner.patterns.iter().flat_map(move |pattern| {
                if !field_matches(&found.path, pattern.definition.field.as_deref()) {
                    return Vec::new().into_iter();
                }
                pattern
                    .regex
                    .captures_iter(value)
                    .filter_map(move |captures| {
                        let matched = selected_match(pattern, &captures)?;
                        Some(SecretFinding {
                            source,
                            span: if pattern.definition.redact_whole_value {
                                0..value.len()
                            } else {
                                matched.range()
                            },
                            pattern_id: pattern.definition.id.clone(),
                        })
                    })
                    .collect::<Vec<_>>()
                    .into_iter()
            });
            let tokens = (!exemptions.exempts_entropy(&found.path))
                .then(|| {
                    super::entropy::high_entropy_spans(value, &scanner.entropy).map(
                        move |(span, _)| SecretFinding {
                            source,
                            span,
                            pattern_id: SecretPatternId::new(HIGH_ENTROPY_PATTERN_ID),
                        },
                    )
                })
                .into_iter()
                .flatten();
            patterns.chain(tokens)
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
