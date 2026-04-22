use async_trait::async_trait;
use serde_json::Value;

use super::{Finding, SafetyScanner, Severity};
use crate::services::gateway::models::AnthropicGatewayRequest;

const JAILBREAK_PHRASES: &[&str] = &[
    "ignore previous instructions",
    "ignore all previous",
    "disregard prior instructions",
    "forget your instructions",
    "you are now",
    "act as dan",
    "developer mode enabled",
    "pretend you have no restrictions",
];

const EXCERPT_CAP: usize = 240;

#[derive(Debug, Clone, Copy, Default)]
pub struct HeuristicScanner;

#[async_trait]
impl SafetyScanner for HeuristicScanner {
    fn name(&self) -> &'static str {
        "heuristic"
    }

    async fn scan_request(&self, req: &AnthropicGatewayRequest) -> Vec<Finding> {
        let mut findings = Vec::new();
        let mut text = String::new();
        if let Some(system) = &req.system {
            collect_text(system, &mut text);
        }
        for msg in &req.messages {
            collect_text(&msg.content, &mut text);
        }
        scan_text("request", &text, &mut findings);
        findings
    }

    async fn scan_response_final(&self, body: &[u8]) -> Vec<Finding> {
        let Ok(value) = serde_json::from_slice::<Value>(body) else {
            return Vec::new();
        };
        let mut text = String::new();
        if let Some(content) = value.get("content").and_then(Value::as_array) {
            for block in content {
                if block.get("type").and_then(Value::as_str) == Some("text") {
                    if let Some(t) = block.get("text").and_then(Value::as_str) {
                        text.push_str(t);
                        text.push('\n');
                    }
                }
            }
        }
        let mut findings = Vec::new();
        scan_text("response", &text, &mut findings);
        findings
    }
}

fn collect_text(v: &Value, out: &mut String) {
    match v {
        Value::String(s) => {
            out.push_str(s);
            out.push('\n');
        },
        Value::Array(a) => {
            for item in a {
                collect_text(item, out);
            }
        },
        Value::Object(obj) => {
            if let Some(Value::String(s)) = obj.get("text") {
                out.push_str(s);
                out.push('\n');
            }
            if let Some(Value::String(s)) = obj.get("content") {
                out.push_str(s);
                out.push('\n');
            }
        },
        _ => {},
    }
}

fn scan_text(phase: &'static str, text: &str, out: &mut Vec<Finding>) {
    let lower = text.to_ascii_lowercase();
    for phrase in JAILBREAK_PHRASES {
        if let Some(idx) = lower.find(phrase) {
            let end = (idx + phrase.len() + 80).min(text.len());
            let start = idx.saturating_sub(40);
            let excerpt = text[start..end]
                .chars()
                .take(EXCERPT_CAP)
                .collect::<String>();
            out.push(Finding {
                phase,
                severity: Severity::Medium,
                category: "jailbreak".to_string(),
                excerpt: Some(excerpt),
                scanner: "heuristic",
            });
        }
    }

    if detect_email(&lower) {
        out.push(Finding {
            phase,
            severity: Severity::Low,
            category: "pii_email".to_string(),
            excerpt: None,
            scanner: "heuristic",
        });
    }
    if detect_credit_card(&lower) {
        out.push(Finding {
            phase,
            severity: Severity::High,
            category: "pii_credit_card".to_string(),
            excerpt: None,
            scanner: "heuristic",
        });
    }
}

fn detect_email(text: &str) -> bool {
    let bytes = text.as_bytes();
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
            if before >= 2
                && after >= 4
                && bytes[i + 1..i + 1 + after]
                    .iter()
                    .any(|b| *b == b'.')
            {
                return true;
            }
        }
        i += 1;
    }
    false
}

fn detect_credit_card(text: &str) -> bool {
    let digits: String = text.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() < 13 {
        return false;
    }
    digits.as_bytes().windows(16).any(luhn_16)
}

fn luhn_16(window: &[u8]) -> bool {
    let mut sum = 0i32;
    for (i, b) in window.iter().rev().enumerate() {
        let mut d = (b - b'0') as i32;
        if i % 2 == 1 {
            d *= 2;
            if d > 9 {
                d -= 9;
            }
        }
        sum += d;
    }
    sum % 10 == 0
}
