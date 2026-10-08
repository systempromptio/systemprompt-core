//! In-flight redaction: findings in `safety.redact_categories` rewrite their
//! spans in the forwarded body and the canonical request, a part replacement
//! wins over spans, and anything that cannot be rewritten is refused.

use bytes::Bytes;
use serde_json::{Value, json};
use systemprompt_gateway::protocol::canonical::{
    CanonicalContent, CanonicalMessage, CanonicalRequest, Role,
};
use systemprompt_gateway::protocol::outbound::PreparedBody;
use systemprompt_gateway::service::finalize::safety::{dedupe_findings, persisted_excerpt};
use systemprompt_gateway::service::stages::recovery::{
    Unredactable, redact_findings, redaction_marker,
};
use systemprompt_gateway::{
    Finding, FindingSpan, HeuristicScanner, PHASE_REQUEST, PartReplacement, SafetyConfig,
    SafetyScanner, Severity,
};
use systemprompt_identifiers::ModelId;
use systemprompt_wire::inspect::{self, SurfaceBudget};

const TEXT_PART: &str = "$.messages[0].content[0].text";
const EMAIL_TEXT: &str = "mail me at alice@example.com please";

fn prepared(text: &str) -> (CanonicalRequest, PreparedBody) {
    let wire = json!({
        "model": "m",
        "max_tokens": 16,
        "messages": [{"role": "user", "content": [{"type": "text", "text": text}]}]
    });
    let bytes = Bytes::from(serde_json::to_vec(&wire).expect("encode"));
    let mut request = CanonicalRequest {
        messages: vec![CanonicalMessage {
            role: Role::User,
            content: vec![CanonicalContent::text(text)],
        }],
        ..CanonicalRequest::new(ModelId::new("m"), Vec::new(), 16)
    };
    request.forwarded_surface = inspect::string_leaves(&bytes, SurfaceBudget::default());
    (
        request,
        PreparedBody {
            bytes,
            raw_lane: true,
        },
    )
}

fn forwarded_text(body: &PreparedBody) -> String {
    let value: Value = serde_json::from_slice(&body.bytes).expect("json body");
    value["messages"][0]["content"][0]["text"]
        .as_str()
        .expect("text leaf")
        .to_owned()
}

fn canonical_text(request: &CanonicalRequest) -> String {
    match &request.messages[0].content[0] {
        CanonicalContent::Text { text, .. } => text.clone(),
        other => panic!("expected text, got {other:?}"),
    }
}

fn finding(category: &str, spans: Vec<FindingSpan>) -> Finding {
    Finding {
        phase: PHASE_REQUEST,
        severity: Severity::Medium,
        category: category.to_owned(),
        excerpt: Some("raw excerpt".to_owned()),
        scanner: "heuristic",
        spans,
        replacement: None,
    }
}

fn span(part: &str, range: std::ops::Range<usize>) -> FindingSpan {
    FindingSpan {
        part: part.to_owned(),
        range,
    }
}

fn redact_list(categories: &[&str]) -> Vec<String> {
    categories.iter().map(|c| (*c).to_owned()).collect()
}

#[tokio::test]
async fn heuristic_spans_redact_the_body_and_the_canonical_request() {
    let (mut request, mut body) = prepared(EMAIL_TEXT);
    let findings = HeuristicScanner::default()
        .scan_request(&request)
        .await
        .expect("scan");
    let email = findings
        .iter()
        .find(|f| f.category == "pii_email")
        .expect("email detected");
    assert_eq!(email.spans, vec![span(TEXT_PART, 11..28)]);

    let report = redact_findings(
        &mut request,
        &mut body,
        &findings,
        &redact_list(&["pii_email"]),
    )
    .expect("redactable")
    .expect("something was redacted");

    let expected = "mail me at [REDACTED:pii_email] please";
    assert_eq!(forwarded_text(&body), expected);
    assert_eq!(canonical_text(&request), expected);
    assert_eq!(report.parts, 1);
    assert_eq!(report.categories, vec!["pii_email".to_owned()]);
    assert!(
        request
            .forwarded_surface
            .leaves()
            .iter()
            .all(|leaf| !leaf.value.contains("alice@example.com")),
        "the surface later stages read is the redacted one"
    );
}

#[test]
fn spans_from_several_categories_on_one_part_are_all_applied() {
    let text = "alpha SECRET beta TOKEN gamma";
    let (mut request, mut body) = prepared(text);
    let findings = vec![
        finding("cat_a", vec![span(TEXT_PART, 6..12)]),
        finding("cat_b", vec![span(TEXT_PART, 18..23)]),
    ];
    redact_findings(
        &mut request,
        &mut body,
        &findings,
        &redact_list(&["cat_a", "cat_b"]),
    )
    .expect("redactable");
    assert_eq!(
        forwarded_text(&body),
        "alpha [REDACTED:cat_a] beta [REDACTED:cat_b] gamma"
    );
}

#[test]
fn a_part_replacement_wins_over_spans_on_the_same_part() {
    let (mut request, mut body) = prepared(EMAIL_TEXT);
    let mut deidentified = finding("sdp_email", vec![span(TEXT_PART, 11..28)]);
    deidentified.replacement = Some(PartReplacement {
        part: TEXT_PART.to_owned(),
        text: "mail me at [EMAIL] please".to_owned(),
    });
    let spanned = finding("pii_email", vec![span(TEXT_PART, 0..4)]);
    redact_findings(
        &mut request,
        &mut body,
        &[spanned, deidentified],
        &redact_list(&["sdp_email", "pii_email"]),
    )
    .expect("redactable");
    assert_eq!(forwarded_text(&body), "mail me at [EMAIL] please");
    assert_eq!(canonical_text(&request), "mail me at [EMAIL] please");
}

#[test]
fn a_span_on_a_protected_key_makes_the_request_unredactable() {
    let (mut request, mut body) = prepared(EMAIL_TEXT);
    let before = body.bytes.clone();
    let findings = vec![
        finding("pii_email", vec![span(TEXT_PART, 11..28)]),
        finding("pii_email", vec![span("$.model", 0..1)]),
    ];
    let err = redact_findings(
        &mut request,
        &mut body,
        &findings,
        &redact_list(&["pii_email"]),
    )
    .expect_err("protected leaf");
    assert_eq!(
        err,
        Unredactable {
            category: "pii_email".to_owned()
        }
    );
    assert_eq!(body.bytes, before, "nothing is half-redacted");
    assert_eq!(canonical_text(&request), EMAIL_TEXT);
}

#[test]
fn a_redact_finding_without_a_location_is_refused() {
    let (mut request, mut body) = prepared(EMAIL_TEXT);
    let err = redact_findings(
        &mut request,
        &mut body,
        &[finding("vendor_pii", Vec::new())],
        &redact_list(&["vendor_pii"]),
    )
    .expect_err("no spans, no replacement");
    assert_eq!(err.category, "vendor_pii");
}

#[test]
fn a_span_outside_the_forwarded_surface_is_refused() {
    let (mut request, mut body) = prepared(EMAIL_TEXT);
    let result = redact_findings(
        &mut request,
        &mut body,
        &[finding("pii_email", vec![span("messages[0]", 11..28)])],
        &redact_list(&["pii_email"]),
    );
    assert!(result.is_err(), "{result:?}");
}

#[test]
fn findings_outside_the_redact_list_leave_the_body_untouched() {
    let (mut request, mut body) = prepared(EMAIL_TEXT);
    let before = body.bytes.clone();
    let outcome = redact_findings(
        &mut request,
        &mut body,
        &[finding("jailbreak", vec![span(TEXT_PART, 0..4)])],
        &redact_list(&["pii_email"]),
    )
    .expect("nothing to redact");
    assert!(outcome.is_none());
    assert_eq!(body.bytes, before);
}

#[test]
fn a_redacted_category_is_persisted_as_its_marker() {
    let safety = SafetyConfig {
        redact_categories: redact_list(&["pii_email"]),
        ..SafetyConfig::default()
    };
    let redacted = finding("pii_email", Vec::new());
    let kept = finding("jailbreak", Vec::new());
    assert_eq!(
        persisted_excerpt(&redacted, &safety).as_deref(),
        Some("[REDACTED:pii_email]")
    );
    assert_eq!(redaction_marker("pii_email"), "[REDACTED:pii_email]");
    assert_eq!(
        persisted_excerpt(&kept, &safety).as_deref(),
        Some("raw excerpt")
    );
}

#[test]
fn deduplication_keeps_the_spans_of_every_collapsed_finding() {
    let mut findings = vec![
        finding("pii_email", vec![span("$.system", 0..3)]),
        finding("pii_email", vec![span(TEXT_PART, 11..28)]),
    ];
    dedupe_findings(&mut findings);
    assert_eq!(findings.len(), 1);
    assert_eq!(
        findings[0].spans,
        vec![span("$.system", 0..3), span(TEXT_PART, 11..28)]
    );
}
