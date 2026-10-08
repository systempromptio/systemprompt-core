//! Phrase-list heuristic safety scanner.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::ops::Range;

use async_trait::async_trait;
use systemprompt_wire::canonical::{CanonicalRequest, CanonicalResponse};

use super::{
    Finding, FindingSpan, PHASE_REQUEST, PHASE_REQUEST_HISTORY, PHASE_RESPONSE, SafetyScanner,
    ScanError, Severity,
};
use crate::policies::spec::HeuristicConfig;

const JAILBREAK_PHRASES: &[&str] = &[
    "ignore previous instructions",
    "ignore all previous",
    "disregard prior instructions",
    "forget your instructions",
    "act as dan",
    "developer mode enabled",
    "pretend you have no restrictions",
];

const EXCERPT_CAP: usize = 240;

#[derive(Debug, Clone)]
pub struct HeuristicScanner {
    phrases: Vec<String>,
}

impl Default for HeuristicScanner {
    fn default() -> Self {
        Self::new(&HeuristicConfig::default())
    }
}

impl HeuristicScanner {
    #[must_use]
    pub fn new(config: &HeuristicConfig) -> Self {
        Self {
            phrases: effective_phrases(config),
        }
    }
}

pub fn effective_phrases(config: &HeuristicConfig) -> Vec<String> {
    let base: Vec<String> = match (&config.phrases, config.disable_builtin) {
        (Some(list), _) => list.clone(),
        (None, true) => Vec::new(),
        (None, false) => JAILBREAK_PHRASES.iter().map(|p| (*p).to_owned()).collect(),
    };
    base.into_iter()
        .chain(config.extra_phrases.iter().cloned())
        .map(|p| p.to_ascii_lowercase())
        .filter(|p| !p.trim().is_empty())
        .collect()
}

#[async_trait]
impl SafetyScanner for HeuristicScanner {
    fn name(&self) -> &'static str {
        "heuristic"
    }

    async fn scan_request(&self, req: &CanonicalRequest) -> Result<Vec<Finding>, ScanError> {
        let mut findings = Vec::new();
        for (part, text) in req.safety_parts(false) {
            scan_text(
                &self.phrases,
                PHASE_REQUEST,
                Some(&part),
                &text,
                &mut findings,
            );
        }
        Ok(findings)
    }

    async fn scan_request_history(
        &self,
        req: &CanonicalRequest,
    ) -> Result<Vec<Finding>, ScanError> {
        let mut findings = Vec::new();
        for (part, unit) in req.safety_parts(true) {
            scan_text(
                &self.phrases,
                PHASE_REQUEST_HISTORY,
                Some(&part),
                &unit,
                &mut findings,
            );
        }
        Ok(findings)
    }

    async fn scan_response_final(
        &self,
        response: &CanonicalResponse,
    ) -> Result<Vec<Finding>, ScanError> {
        let mut findings = Vec::new();
        for unit in response.content_units() {
            scan_text(&self.phrases, PHASE_RESPONSE, None, &unit, &mut findings);
        }
        Ok(findings)
    }
}

fn scan_text(
    phrases: &[String],
    phase: &'static str,
    part: Option<&str>,
    text: &str,
    out: &mut Vec<Finding>,
) {
    let finding = |severity, category: &str, excerpt, ranges: Vec<Range<usize>>| Finding {
        phase,
        severity,
        category: category.to_owned(),
        excerpt,
        scanner: "heuristic",
        spans: part.map_or_else(Vec::new, |part| {
            ranges
                .into_iter()
                .map(|range| FindingSpan {
                    part: part.to_owned(),
                    range,
                })
                .collect()
        }),
        replacement: None,
    };
    if !phrases.is_empty() {
        let lower = text.to_ascii_lowercase();
        for phrase in phrases {
            let ranges: Vec<_> = lower
                .match_indices(phrase.as_str())
                .map(|(idx, _)| idx..idx + phrase.len())
                .collect();
            let Some(first) = ranges.first() else {
                continue;
            };
            let start = floor_boundary(text, first.start.saturating_sub(40));
            let end = ceil_boundary(text, first.end + 80);
            let excerpt = text[start..end]
                .chars()
                .take(EXCERPT_CAP)
                .collect::<String>();
            out.push(finding(
                Severity::Medium,
                "jailbreak",
                Some(excerpt),
                ranges,
            ));
        }
    }

    let emails = email_ranges(text);
    if !emails.is_empty() {
        out.push(finding(Severity::Low, "pii_email", None, emails));
    }
    if !text.bytes().any(|b| b.is_ascii_digit()) {
        return;
    }
    let cards = credit_card_ranges(text);
    if !cards.is_empty() {
        out.push(finding(Severity::High, "pii_credit_card", None, cards));
    }
}

const fn floor_boundary(text: &str, mut i: usize) -> usize {
    while i > 0 && !text.is_char_boundary(i) {
        i -= 1;
    }
    i
}

const fn ceil_boundary(text: &str, mut i: usize) -> usize {
    if i >= text.len() {
        return text.len();
    }
    while i < text.len() && !text.is_char_boundary(i) {
        i += 1;
    }
    i
}

fn email_ranges(text: &str) -> Vec<Range<usize>> {
    let bytes = text.as_bytes();
    let mut ranges = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'@' {
            let before = bytes[..i]
                .iter()
                .rev()
                .take_while(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'+' | b'-'))
                .count();
            let after = bytes[i + 1..]
                .iter()
                .take_while(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-'))
                .count();
            if before >= 2 && after >= 4 && bytes[i + 1..i + 1 + after].contains(&b'.') {
                ranges.push(i - before..i + 1 + after);
                i += after;
            }
        }
        i += 1;
    }
    ranges
}

const CARD_MIN_DIGITS: usize = 13;
const CARD_MAX_DIGITS: usize = 19;

fn credit_card_ranges(text: &str) -> Vec<Range<usize>> {
    let bytes = text.as_bytes();
    let mut ranges = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if !bytes[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let (digits, end) = card_candidate(bytes, i);
        if is_card(&digits) {
            ranges.push(i..end);
        }
        i = end;
    }
    ranges
}

fn card_candidate(bytes: &[u8], start: usize) -> (Vec<u8>, usize) {
    let mut digits = Vec::new();
    let mut i = start;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            digits.push(bytes[i]);
            i += 1;
            continue;
        }
        if matches!(bytes[i], b' ' | b'-') && bytes.get(i + 1).is_some_and(u8::is_ascii_digit) {
            i += 1;
            continue;
        }
        break;
    }
    (digits, i)
}

fn is_card(digits: &[u8]) -> bool {
    (CARD_MIN_DIGITS..=CARD_MAX_DIGITS).contains(&digits.len())
        && has_issuer_prefix(digits)
        && luhn(digits)
}

fn has_issuer_prefix(digits: &[u8]) -> bool {
    let Some(&second) = digits.get(1) else {
        return false;
    };
    match digits[0] {
        b'4' => true,
        b'5' => matches!(second, b'1'..=b'5'),
        b'2' => matches!(second, b'2'..=b'7'),
        b'3' => matches!(second, b'0' | b'4' | b'5' | b'6' | b'7' | b'8'),
        b'6' => digits.starts_with(b"6011") || second == b'5',
        _ => false,
    }
}

fn luhn(digits: &[u8]) -> bool {
    let mut sum = 0u32;
    for (i, b) in digits.iter().rev().enumerate() {
        let mut d = u32::from(b - b'0');
        if i % 2 == 1 {
            d *= 2;
            if d > 9 {
                d -= 9;
            }
        }
        sum += d;
    }
    sum.is_multiple_of(10)
}
