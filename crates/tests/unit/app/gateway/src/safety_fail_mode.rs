//! Scanner fail mode and timeout: a scanner that overruns `timeout_ms` or
//! errors becomes a `scanner_failure` finding, and whether that finding
//! blocks is decided by the scanner's `fail_mode` and the policy's `mode`.

use std::collections::BTreeMap;
use std::time::Duration;

use systemprompt_gateway::protocol::canonical::{
    CanonicalContent, CanonicalMessage, CanonicalRequest, CanonicalResponse, Role,
};
use systemprompt_gateway::registry::SafetyScannerRegistry;
use systemprompt_gateway::service::finalize::safety::{
    request_finding_blocks, response_finding_blocks, scan_bounded,
};
use systemprompt_gateway::{
    CATEGORY_SCANNER_FAILURE, Finding, PHASE_REQUEST, PHASE_RESPONSE, SafetyConfig, SafetyMode,
    SafetyScanner, ScanError, ScannerFailMode, ScannerSettings, register_safety_scanner,
};
use systemprompt_identifiers::ModelId;

const SLEEPER: &str = "test_fail_mode_sleeper";

struct SleepingScanner;

#[async_trait::async_trait]
impl SafetyScanner for SleepingScanner {
    fn name(&self) -> &'static str {
        SLEEPER
    }

    async fn scan_request(&self, _req: &CanonicalRequest) -> Result<Vec<Finding>, ScanError> {
        tokio::time::sleep(Duration::from_secs(30)).await;
        Ok(Vec::new())
    }

    async fn scan_response_final(
        &self,
        _response: &CanonicalResponse,
    ) -> Result<Vec<Finding>, ScanError> {
        tokio::time::sleep(Duration::from_secs(30)).await;
        Ok(Vec::new())
    }
}

register_safety_scanner!(|_: &ScannerSettings| SleepingScanner, name = SLEEPER);

fn settings(fail_mode: ScannerFailMode, timeout_ms: u64) -> ScannerSettings {
    ScannerSettings {
        fail_mode,
        timeout_ms,
        config: BTreeMap::new(),
    }
}

fn policy(fail_mode: ScannerFailMode, mode: SafetyMode) -> SafetyConfig {
    SafetyConfig {
        mode,
        scanners: vec![SLEEPER.to_owned()],
        scanner_settings: BTreeMap::from([(SLEEPER.to_owned(), settings(fail_mode, 20))]),
        ..SafetyConfig::default()
    }
}

fn empty_request() -> CanonicalRequest {
    CanonicalRequest {
        model: ModelId::new("m"),
        cache_control: None,
        system: Vec::new(),
        messages: vec![CanonicalMessage {
            role: Role::User,
            content: vec![CanonicalContent::text("hi")],
        }],
        max_tokens: 1,
        temperature: None,
        top_p: None,
        top_k: None,
        stop_sequences: vec![],
        tools: vec![],
        tool_choice: None,
        stream: false,
        thinking: None,
        metadata: None,
        response_format: None,
        reasoning_effort: None,
        search: None,
        code_execution: false,
        presence_penalty: None,
        frequency_penalty: None,
        forwarded_surface: Default::default(),
    }
}

async fn timed_out_request_finding(fail_mode: ScannerFailMode) -> Finding {
    let safety = policy(fail_mode, SafetyMode::Enforce);
    let scanner = SafetyScannerRegistry::global()
        .create(SLEEPER, &safety)
        .expect("sleeper is registered");
    let request = empty_request();
    let mut findings = Vec::new();
    let started = std::time::Instant::now();
    scan_bounded(
        &mut findings,
        PHASE_REQUEST,
        scanner.name(),
        &safety.settings_for(SLEEPER),
        scanner.scan_request(&request),
    )
    .await;
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "the scan is cut off at timeout_ms, not awaited to completion"
    );
    assert_eq!(findings.len(), 1, "{findings:?}");
    findings.remove(0)
}

#[tokio::test]
async fn a_scanner_past_its_timeout_becomes_a_timed_out_failure_finding() {
    let finding = timed_out_request_finding(ScannerFailMode::Closed).await;
    assert_eq!(finding.category, CATEGORY_SCANNER_FAILURE);
    assert_eq!(finding.scanner, SLEEPER);
    let excerpt = finding.excerpt.as_deref().unwrap_or_default();
    assert!(excerpt.contains("timed out after 20 ms"), "{excerpt}");
}

#[tokio::test]
async fn a_closed_scanner_timeout_blocks_the_request() {
    let finding = timed_out_request_finding(ScannerFailMode::Closed).await;
    let safety = policy(ScannerFailMode::Closed, SafetyMode::Enforce);
    assert!(request_finding_blocks(&finding, &safety));
}

#[tokio::test]
async fn an_open_scanner_timeout_is_recorded_but_does_not_block() {
    let finding = timed_out_request_finding(ScannerFailMode::Open).await;
    let safety = policy(ScannerFailMode::Open, SafetyMode::Enforce);
    assert!(finding.is_scanner_failure());
    assert!(!request_finding_blocks(&finding, &safety));
}

#[tokio::test]
async fn warn_mode_never_blocks_a_closed_scanner_failure() {
    let finding = timed_out_request_finding(ScannerFailMode::Closed).await;
    let safety = policy(ScannerFailMode::Closed, SafetyMode::Warn);
    assert!(!request_finding_blocks(&finding, &safety));
}

#[tokio::test]
async fn a_response_scan_obeys_the_same_timeout_and_fail_mode() {
    let closed = policy(ScannerFailMode::Closed, SafetyMode::Enforce);
    let open = policy(ScannerFailMode::Open, SafetyMode::Enforce);
    let scanner = SafetyScannerRegistry::global()
        .create(SLEEPER, &closed)
        .expect("sleeper is registered");
    let response = CanonicalResponse::default();
    let mut findings = Vec::new();
    scan_bounded(
        &mut findings,
        PHASE_RESPONSE,
        scanner.name(),
        &closed.settings_for(SLEEPER),
        scanner.scan_response_final(&response),
    )
    .await;
    let finding = findings.first().expect("timed-out response scan recorded");
    assert!(finding.is_scanner_failure());
    assert!(response_finding_blocks(finding, &closed));
    assert!(!response_finding_blocks(finding, &open));
}

#[tokio::test]
async fn an_erroring_scanner_under_open_fail_mode_is_recorded_not_blocking() {
    let safety = policy(ScannerFailMode::Open, SafetyMode::Enforce);
    let mut findings = Vec::new();
    scan_bounded(
        &mut findings,
        PHASE_REQUEST,
        SLEEPER,
        &safety.settings_for(SLEEPER),
        async {
            Err(ScanError::Failed {
                scanner: SLEEPER,
                reason: "backend unreachable".to_owned(),
            })
        },
    )
    .await;
    let finding = findings.first().expect("failure recorded");
    assert!(finding.is_scanner_failure());
    assert!(!request_finding_blocks(finding, &safety));
    assert!(request_finding_blocks(
        finding,
        &policy(ScannerFailMode::Closed, SafetyMode::Enforce)
    ));
}

#[tokio::test]
async fn a_scan_inside_its_timeout_keeps_its_own_findings() {
    let safety = policy(ScannerFailMode::Closed, SafetyMode::Enforce);
    let mut findings = Vec::new();
    scan_bounded(
        &mut findings,
        PHASE_REQUEST,
        SLEEPER,
        &safety.settings_for(SLEEPER),
        async { Ok(Vec::new()) },
    )
    .await;
    assert!(findings.is_empty(), "{findings:?}");
}
