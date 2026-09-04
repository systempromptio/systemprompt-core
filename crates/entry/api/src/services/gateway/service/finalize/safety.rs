//! Safety scanning of gateway requests and responses, and persistence of the
//! findings they produce.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_ai::repository::AiSafetyFindingRepository;
use systemprompt_ai::{Finding, InsertSafetyFinding, SafetyConfig, SafetyHistoryMode};

use super::super::blocks_at_phase;
use systemprompt_identifiers::AiRequestId;

use super::super::super::protocol::canonical::CanonicalRequest;
use super::super::super::protocol::canonical_response::CanonicalResponse;
use super::super::super::registry::SafetyScannerRegistry;

pub(in crate::services::gateway) async fn run_request_safety_scan(
    safety_repo: &AiSafetyFindingRepository,
    ai_request_id: &AiRequestId,
    request: &CanonicalRequest,
    safety: &SafetyConfig,
) -> Vec<Finding> {
    let registry = SafetyScannerRegistry::global();
    let scan_history = safety.history != SafetyHistoryMode::Off;
    let mut findings = Vec::new();
    for name in &safety.scanners {
        if let Some(scanner) = registry.create(name, safety) {
            findings.extend(scanner.scan_request(request).await);
            if scan_history {
                findings.extend(scanner.scan_request_history(request).await);
            }
        } else {
            tracing::warn!(scanner = %name, "Unknown safety scanner in policy — skipped");
        }
    }
    dedupe_findings(&mut findings);
    if !findings.is_empty() {
        persist_findings(safety_repo, ai_request_id, &findings, &|f: &Finding| {
            request_finding_blocks(f, safety)
        })
        .await;
    }
    findings
}

// Why: one predicate decides both the `blocked` column and the refusal itself,
// so the report can never disagree with what the gateway actually did. It is
// false throughout under `safety.mode: warn`.
pub(in crate::services::gateway) fn request_finding_blocks(
    finding: &Finding,
    safety: &SafetyConfig,
) -> bool {
    !safety.mode.is_warn()
        && safety.block_categories.contains(&finding.category)
        && blocks_at_phase(finding.phase, safety.history)
}

#[cfg_attr(
    not(feature = "test-api"),
    expect(
        unreachable_pub,
        reason = "re-exported via `test_api` only when the feature is on"
    )
)]
pub fn dedupe_findings(findings: &mut Vec<Finding>) {
    let mut seen = std::collections::HashSet::new();
    findings.retain(|f| seen.insert((f.phase, f.category.clone(), f.scanner)));
}

pub(in crate::services::gateway) async fn run_response_safety_scan(
    safety_repo: &AiSafetyFindingRepository,
    ai_request_id: &AiRequestId,
    response: &CanonicalResponse,
    safety: &SafetyConfig,
) -> Vec<Finding> {
    let registry = SafetyScannerRegistry::global();
    let mut findings = Vec::new();
    for name in &safety.scanners {
        if let Some(scanner) = registry.create(name, safety) {
            findings.extend(scanner.scan_response_final(response).await);
        } else {
            tracing::warn!(scanner = %name, "Unknown safety scanner in policy — skipped");
        }
    }
    dedupe_findings(&mut findings);
    if !findings.is_empty() {
        persist_findings(safety_repo, ai_request_id, &findings, &|f: &Finding| {
            !safety.mode.is_warn() && safety.block_response_categories.contains(&f.category)
        })
        .await;
    }
    findings
}

async fn persist_findings(
    repo: &AiSafetyFindingRepository,
    ai_request_id: &AiRequestId,
    findings: &[Finding],
    blocks: &dyn Fn(&Finding) -> bool,
) {
    for f in findings {
        let params = InsertSafetyFinding {
            ai_request_id,
            phase: f.phase,
            severity: f.severity.as_str(),
            category: &f.category,
            scanner: f.scanner,
            excerpt: f.excerpt.as_deref(),
            blocked: blocks(f),
        };
        if let Err(e) = repo.insert(params).await {
            tracing::warn!(error = %e, "safety finding insert failed");
        }
    }
}
