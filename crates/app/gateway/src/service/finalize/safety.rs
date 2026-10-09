//! Safety scanning of gateway requests and responses, and persistence of the
//! findings they produce.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::future::Future;

use crate::policies::{
    Finding, PHASE_REQUEST, PHASE_REQUEST_HISTORY, PHASE_RESPONSE, SafetyConfig, SafetyHistoryMode,
    ScanError, ScannerFailMode, ScannerSettings,
};
use systemprompt_ai::InsertSafetyFinding;
use systemprompt_ai::repository::AiSafetyFindingRepository;

pub fn blocks_at_phase(phase: &str, history: SafetyHistoryMode) -> bool {
    match phase {
        PHASE_REQUEST => true,
        PHASE_REQUEST_HISTORY => history == SafetyHistoryMode::Block,
        _ => false,
    }
}
use systemprompt_identifiers::AiRequestId;

use super::super::super::protocol::canonical::{CanonicalRequest, CanonicalResponse};
use super::super::super::registry::SafetyScannerRegistry;
use super::super::stages::recovery::redaction_marker;

pub(crate) async fn run_request_safety_scan(
    request: &CanonicalRequest,
    safety: &SafetyConfig,
) -> Vec<Finding> {
    let registry = SafetyScannerRegistry::global();
    let scan_history = safety.history != SafetyHistoryMode::Off;
    let mut findings = Vec::new();
    for name in &safety.scanners {
        if let Some(scanner) = registry.create(name, safety) {
            let settings = safety.settings_for(name);
            let scanner_name = scanner.name();
            scan_bounded(
                &mut findings,
                PHASE_REQUEST,
                scanner_name,
                &settings,
                scanner.scan_request(request),
            )
            .await;
            if scan_history {
                scan_bounded(
                    &mut findings,
                    PHASE_REQUEST_HISTORY,
                    scanner_name,
                    &settings,
                    scanner.scan_request_history(request),
                )
                .await;
            }
        } else {
            tracing::warn!(scanner = %name, "Unknown safety scanner in policy — skipped");
        }
    }
    dedupe_findings(&mut findings);
    findings
}

pub(crate) async fn persist_request_findings(
    repo: &AiSafetyFindingRepository,
    ai_request_id: &AiRequestId,
    findings: &[Finding],
    safety: &SafetyConfig,
) {
    if !findings.is_empty() {
        persist_findings(repo, ai_request_id, findings, safety, &|f: &Finding| {
            request_finding_blocks(f, safety)
        })
        .await;
    }
}

pub fn request_finding_blocks(finding: &Finding, safety: &SafetyConfig) -> bool {
    !safety.mode.is_warn()
        && (failure_blocks(finding, safety) || safety.block_categories.contains(&finding.category))
        && blocks_at_phase(finding.phase, safety.history)
}

pub fn response_finding_blocks(finding: &Finding, safety: &SafetyConfig) -> bool {
    !safety.mode.is_warn()
        && (failure_blocks(finding, safety)
            || safety.block_response_categories.contains(&finding.category))
}

fn failure_blocks(finding: &Finding, safety: &SafetyConfig) -> bool {
    finding.is_scanner_failure()
        && safety.settings_for(finding.scanner).fail_mode == ScannerFailMode::Closed
}

pub async fn scan_bounded<F>(
    findings: &mut Vec<Finding>,
    phase: &'static str,
    scanner: &'static str,
    settings: &ScannerSettings,
    scan: F,
) where
    F: Future<Output = Result<Vec<Finding>, ScanError>> + Send,
{
    let after = settings.timeout();
    let outcome = tokio::time::timeout(after, scan)
        .await
        .unwrap_or(Err(ScanError::TimedOut { scanner, after }));
    let error = match outcome {
        Ok(found) => {
            findings.extend(found);
            return;
        },
        Err(error) => error,
    };
    match settings.fail_mode {
        ScannerFailMode::Closed => tracing::error!(
            scanner,
            phase,
            error = %error,
            "Safety scanner failed closed — recorded as a blocking finding"
        ),
        ScannerFailMode::Open => tracing::warn!(
            scanner,
            phase,
            error = %error,
            "Safety scanner failed open — recorded, request proceeds"
        ),
    }
    findings.push(Finding::scanner_failure(phase, scanner, &error));
}

pub fn dedupe_findings(findings: &mut Vec<Finding>) {
    let mut kept: Vec<Finding> = Vec::with_capacity(findings.len());
    for finding in findings.drain(..) {
        match kept.iter_mut().find(|k| same_row(k, &finding)) {
            Some(existing) => {
                existing.spans.extend(finding.spans);
                if existing.replacement.is_none() {
                    existing.replacement = finding.replacement;
                }
            },
            None => kept.push(finding),
        }
    }
    *findings = kept;
}

fn same_row(a: &Finding, b: &Finding) -> bool {
    let replacements_compatible = match (&a.replacement, &b.replacement) {
        (Some(x), Some(y)) => x.part == y.part,
        _ => true,
    };
    a.phase == b.phase
        && a.category == b.category
        && a.scanner == b.scanner
        && replacements_compatible
}

pub fn persisted_excerpt(finding: &Finding, safety: &SafetyConfig) -> Option<String> {
    if safety.redacts(&finding.category) {
        return Some(redaction_marker(&finding.category));
    }
    finding.excerpt.clone()
}

pub(crate) async fn run_response_safety_scan(
    safety_repo: &AiSafetyFindingRepository,
    ai_request_id: &AiRequestId,
    response: &CanonicalResponse,
    safety: &SafetyConfig,
) -> Vec<Finding> {
    let registry = SafetyScannerRegistry::global();
    let mut findings = Vec::new();
    for name in &safety.scanners {
        if let Some(scanner) = registry.create(name, safety) {
            let settings = safety.settings_for(name);
            scan_bounded(
                &mut findings,
                PHASE_RESPONSE,
                scanner.name(),
                &settings,
                scanner.scan_response_final(response),
            )
            .await;
        } else {
            tracing::warn!(scanner = %name, "Unknown safety scanner in policy — skipped");
        }
    }
    dedupe_findings(&mut findings);
    if !findings.is_empty() {
        persist_findings(
            safety_repo,
            ai_request_id,
            &findings,
            safety,
            &|f: &Finding| response_finding_blocks(f, safety),
        )
        .await;
    }
    findings
}

async fn persist_findings(
    repo: &AiSafetyFindingRepository,
    ai_request_id: &AiRequestId,
    findings: &[Finding],
    safety: &SafetyConfig,
    blocks: &(dyn Fn(&Finding) -> bool + Sync),
) {
    let excerpts: Vec<Option<String>> = findings
        .iter()
        .map(|f| persisted_excerpt(f, safety))
        .collect();
    let rows: Vec<InsertSafetyFinding<'_>> = findings
        .iter()
        .zip(&excerpts)
        .map(|(f, excerpt)| InsertSafetyFinding {
            ai_request_id,
            phase: f.phase,
            severity: f.severity.as_str(),
            category: &f.category,
            scanner: f.scanner,
            excerpt: excerpt.as_deref(),
            blocked: blocks(f),
        })
        .collect();
    if let Err(e) = repo.insert_many(&rows).await {
        tracing::warn!(error = %e, count = rows.len(), "safety findings insert failed");
    }
}
