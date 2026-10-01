//! Safety scanning of gateway requests and responses, and persistence of the
//! findings they produce.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_ai::repository::AiSafetyFindingRepository;
use systemprompt_ai::{
    Finding, InsertSafetyFinding, PHASE_REQUEST, PHASE_REQUEST_HISTORY, PHASE_RESPONSE,
    SafetyConfig, SafetyHistoryMode, ScanError,
};

pub fn blocks_at_phase(phase: &str, history: SafetyHistoryMode) -> bool {
    match phase {
        PHASE_REQUEST => true,
        PHASE_REQUEST_HISTORY => history == SafetyHistoryMode::Block,
        _ => false,
    }
}
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
            let scanner_name = scanner.name();
            record_scan(
                &mut findings,
                PHASE_REQUEST,
                scanner_name,
                scanner.scan_request(request).await,
            );
            if scan_history {
                record_scan(
                    &mut findings,
                    PHASE_REQUEST_HISTORY,
                    scanner_name,
                    scanner.scan_request_history(request).await,
                );
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

pub(in crate::services::gateway) fn request_finding_blocks(
    finding: &Finding,
    safety: &SafetyConfig,
) -> bool {
    !safety.mode.is_warn()
        && (finding.is_scanner_failure() || safety.block_categories.contains(&finding.category))
        && blocks_at_phase(finding.phase, safety.history)
}

pub(in crate::services::gateway) fn response_finding_blocks(
    finding: &Finding,
    safety: &SafetyConfig,
) -> bool {
    !safety.mode.is_warn()
        && (finding.is_scanner_failure()
            || safety.block_response_categories.contains(&finding.category))
}

fn record_scan(
    findings: &mut Vec<Finding>,
    phase: &'static str,
    scanner: &'static str,
    outcome: Result<Vec<Finding>, ScanError>,
) {
    match outcome {
        Ok(found) => findings.extend(found),
        Err(e) => {
            tracing::error!(
                scanner,
                phase,
                error = %e,
                "Safety scanner failed — recorded as a blocking finding"
            );
            findings.push(Finding::scanner_failure(phase, scanner, &e));
        },
    }
}

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
            let scanner_name = scanner.name();
            record_scan(
                &mut findings,
                PHASE_RESPONSE,
                scanner_name,
                scanner.scan_response_final(response).await,
            );
        } else {
            tracing::warn!(scanner = %name, "Unknown safety scanner in policy — skipped");
        }
    }
    dedupe_findings(&mut findings);
    if !findings.is_empty() {
        persist_findings(safety_repo, ai_request_id, &findings, &|f: &Finding| {
            response_finding_blocks(f, safety)
        })
        .await;
    }
    findings
}

async fn persist_findings(
    repo: &AiSafetyFindingRepository,
    ai_request_id: &AiRequestId,
    findings: &[Finding],
    blocks: &(dyn Fn(&Finding) -> bool + Sync),
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
