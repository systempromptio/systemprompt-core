use systemprompt_models::bridge::envelope::{ErrorEnvelope, ProblemDocument};

#[test]
fn envelope_and_problem_documents_decode() {
    let envelope: ErrorEnvelope = serde_json::from_str(
        r#"{"code":"unauthorized","message":"Bridge credential rejected","error_key":"invalid_credential","timestamp":"t"}"#,
    )
    .unwrap();
    assert_eq!(envelope.error_key.as_deref(), Some("invalid_credential"));
    let problem: ProblemDocument = serde_json::from_str(
        r#"{"type":"about:blank","title":"Unauthorized","status":401,"detail":"Valid enrolled-device authentication is required"}"#,
    )
    .unwrap();
    assert_eq!(problem.status, Some(401));
    assert_eq!(
        problem.summary(),
        Some("Valid enrolled-device authentication is required")
    );
}
