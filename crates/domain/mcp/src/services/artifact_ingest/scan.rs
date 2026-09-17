//! Scanning and redaction before a body is stored.
//!
//! The installation's secret scanner runs first and rewrites every matched
//! span in place, so a stored body never contains a credential. The gateway's
//! safety scanners then run over the redacted text and report findings. A
//! scanner that fails aborts the ingest: an unscanned artifact is not stored.
//! Bodies are identified by content, so one that has already been through
//! this pass is not scanned again. A body over the ingestion ceiling is
//! scanned in full before only its header is kept for storage.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use async_trait::async_trait;
use serde_json::Value as JsonValue;
use systemprompt_models::artifacts::PayloadDigest;
use systemprompt_security::policy::GovernedInput;
use systemprompt_security::policy::secrets::{SecretScanner, redact_spans};

use super::{ArtifactIngest, IngestRequest};
use crate::error::{McpDomainError, McpDomainResult};
use crate::repository::{ArtifactFinding, PHASE_TOOL_RESULT};

pub(super) const SECRET_SCANNER_NAME: &str = "secret_scanner";
pub(super) const SECRET_CATEGORY: &str = "secret";

/// A content scanner run over the text surfaces of an artifact body.
///
/// `#[async_trait]` because scanners are registered as `Arc<dyn
/// ArtifactScanner>`.
#[async_trait]
pub trait ArtifactScanner: Send + Sync {
    fn name(&self) -> &'static str;

    async fn scan(&self, surfaces: &[(String, String)]) -> Result<Vec<ArtifactFinding>, String>;
}

#[derive(Debug)]
pub struct ScanOutcome {
    // JSON: the typed body after redaction.
    pub body: JsonValue,
    pub findings: Vec<ArtifactFinding>,
    pub secret_redactions: usize,
}

impl ScanOutcome {
    #[must_use]
    pub fn truncate(self, header: JsonValue, digest: &PayloadDigest) -> (Self, Option<JsonValue>) {
        tracing::warn!(
            bytes = digest.byte_len,
            sha256 = %digest.sha256,
            findings = self.findings.len(),
            secret_redactions = self.secret_redactions,
            "Tool result exceeded the ingestion ceiling; storing its header only"
        );
        let scanned_body = (self.secret_redactions > 0).then_some(self.body);
        (
            Self {
                body: header,
                findings: self.findings,
                secret_redactions: self.secret_redactions,
            },
            scanned_body,
        )
    }
}

pub(super) async fn scan_body(
    ingest: &ArtifactIngest,
    request: &IngestRequest,
    mut body: JsonValue,
    raw_digest: &PayloadDigest,
) -> McpDomainResult<ScanOutcome> {
    if ingest.payloads.payload_exists(&raw_digest.sha256).await? {
        return Ok(ScanOutcome {
            body,
            findings: Vec::new(),
            secret_redactions: 0,
        });
    }

    let mut findings = Vec::new();
    let secret_redactions = ingest.secrets.as_deref().map_or(0, |scanner| {
        redact_secrets(scanner, &mut body, &mut findings)
    });

    let surfaces = surfaces(&body);
    for scanner in ingest.scanners() {
        let found = scanner.scan(&surfaces).await.map_err(|e| {
            McpDomainError::Internal(format!(
                "artifact scanner {} failed for tool {}: {e}",
                scanner.name(),
                request.tool_name
            ))
        })?;
        findings.extend(found);
    }

    Ok(ScanOutcome {
        body,
        findings,
        secret_redactions,
    })
}

fn redact_secrets(
    scanner: &SecretScanner,
    body: &mut JsonValue,
    findings: &mut Vec<ArtifactFinding>,
) -> usize {
    let mut leaves = Vec::new();
    collect_leaves(body, "body", &mut leaves);
    let parts: Vec<(String, String)> = leaves
        .iter()
        .map(|(path, value)| (path.clone(), (*value).clone()))
        .collect();
    let input = GovernedInput::prompt_parts(parts);
    let found = scanner.findings(&input);
    if found.is_empty() {
        return 0;
    }

    let mut spans: BTreeMap<usize, Vec<std::ops::Range<usize>>> = BTreeMap::new();
    for finding in &found {
        spans
            .entry(finding.source.part_index)
            .or_default()
            .push(finding.span.clone());
        findings.push(ArtifactFinding {
            phase: PHASE_TOOL_RESULT,
            severity: "high".to_owned(),
            category: SECRET_CATEGORY.to_owned(),
            scanner: SECRET_SCANNER_NAME.to_owned(),
            path: leaves
                .get(finding.source.part_index)
                .map(|(p, _)| p.clone()),
            excerpt: Some(finding.pattern_id.to_string()),
            redacted: true,
        });
    }

    let mut redacted = 0;
    for (index, ranges) in spans {
        let Some((_, value)) = leaves.get_mut(index) else {
            continue;
        };
        if let Some(clean) = redact_spans(value, ranges.iter().cloned()) {
            **value = clean;
            redacted += ranges.len();
        }
    }
    redacted
}

fn surfaces(body: &JsonValue) -> Vec<(String, String)> {
    let mut out = Vec::new();
    collect_surfaces(body, "body", &mut out);
    out
}

// JSON: walks a typed body to expose its strings for scanning.
fn collect_leaves<'a>(
    value: &'a mut JsonValue,
    path: &str,
    out: &mut Vec<(String, &'a mut String)>,
) {
    match value {
        JsonValue::String(s) => out.push((path.to_owned(), s)),
        JsonValue::Array(items) => {
            for (index, item) in items.iter_mut().enumerate() {
                collect_leaves(item, &format!("{path}[{index}]"), out);
            }
        },
        JsonValue::Object(map) => {
            for (key, item) in map.iter_mut() {
                collect_leaves(item, &format!("{path}.{key}"), out);
            }
        },
        JsonValue::Null | JsonValue::Bool(_) | JsonValue::Number(_) => {},
    }
}

// JSON: read-only counterpart of `collect_leaves`.
fn collect_surfaces(value: &JsonValue, path: &str, out: &mut Vec<(String, String)>) {
    match value {
        JsonValue::String(s) => out.push((path.to_owned(), s.clone())),
        JsonValue::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                collect_surfaces(item, &format!("{path}[{index}]"), out);
            }
        },
        JsonValue::Object(map) => {
            for (key, item) in map {
                collect_surfaces(item, &format!("{path}.{key}"), out);
            }
        },
        JsonValue::Null | JsonValue::Bool(_) | JsonValue::Number(_) => {},
    }
}
