use bytes::Bytes;
use serde_json::{Value, json};
use systemprompt_api::services::gateway::protocol::canonical::{
    CanonicalContent, CanonicalMessage, CanonicalRequest, Role,
};
use systemprompt_api::services::gateway::protocol::outbound::PreparedBody;
use systemprompt_api::services::gateway::service::stages::recovery::{
    PromptRecovery, govern_prompt,
};
use systemprompt_identifiers::{CallId, SessionId, UserId};
use systemprompt_security::authz::types::Decision;
use systemprompt_security::policy::secrets::REDACTION_MARKER;
use systemprompt_security::policy::types::AccessScope;
use systemprompt_security::policy::{
    AgentScope, GovernanceConfig, GovernanceEngine, GovernedInput, GovernedTarget, PolicyContext,
};

pub(super) const KEY: &str = "AKIAIOSFODNN7EXAMPLE";
pub(super) const POLICY: &str = "governance:\n  policies:\n    - id: secret_scan\n";

pub(super) fn engine(yaml: &str) -> GovernanceEngine {
    GovernanceEngine::from_config(&GovernanceConfig::parse(yaml).unwrap()).unwrap()
}

pub(super) fn govern(
    engine: &GovernanceEngine,
    request: &mut CanonicalRequest,
    body: &mut PreparedBody,
) -> PromptRecovery {
    let session = SessionId::new("session-recovery");
    let user = UserId::new("user-recovery");
    let call = CallId::generate();
    let input = GovernedInput::prompt_parts([]);
    let ctx = PolicyContext {
        target: GovernedTarget::Prompt,
        agent_scope: AgentScope::User {
            user_id: user.clone(),
        },
        access_scope: AccessScope::User,
        session_id: &session,
        user_id: &user,
        input: &input,
        call_id: &call,
    };
    govern_prompt(engine, &ctx, request, body)
}

fn body(value: Value) -> PreparedBody {
    PreparedBody {
        bytes: Bytes::from(serde_json::to_vec(&value).unwrap()),
        raw_lane: true,
    }
}

pub(super) fn request() -> CanonicalRequest {
    CanonicalRequest {
        model: "test-model".to_owned(),
        system: Some(format!("Use this key {KEY} carefully")),
        messages: vec![CanonicalMessage {
            role: Role::User,
            content: vec![CanonicalContent::Text(format!("Inspect {KEY}"))],
        }],
        max_tokens: 64,
        ..CanonicalRequest::default()
    }
}

#[test]
fn subsequent_prompts_repair_resent_system_and_history_without_client_changes() {
    let engine = engine(POLICY);
    for followup in ["inspect", "please continue", "different question"] {
        let mut wire = body(json!({
            "model":"test-model", "system":format!("Use this key {KEY} carefully"),
            "messages":[{"role":"user", "content":format!("Inspect {KEY}")}, {"role":"user", "content":followup}]
        }));
        let mut request = request();
        let result = govern(&engine, &mut request, &mut wire);
        assert!(matches!(result.evaluation.decision, Decision::Warn { .. }));
        assert_eq!(result.recovery_count, 2);
        assert!(!String::from_utf8_lossy(&wire.bytes).contains(KEY));
        assert!(String::from_utf8_lossy(&wire.bytes).contains(followup));
        assert!(!format!("{:?}", request.flatten_parts()).contains(KEY));
        assert!(!format!("{:?}", result.evaluation).contains(KEY));
        let bytes = wire.bytes.clone();
        let again = govern(&engine, &mut request, &mut wire);
        assert!(matches!(again.evaluation.decision, Decision::Allow { .. }));
        assert_eq!(again.recovery_count, 0);
        assert_eq!(wire.bytes, bytes);
    }
}

#[test]
fn passthrough_tool_results_metadata_and_json_arguments_are_sanitized() {
    let mut wire = body(json!({
        "messages":[
            {"role":"assistant", "tool_calls":[{"id":"call-1", "type":"function", "function":{"name":"lookup", "arguments":json!({"credential":KEY}).to_string()}}]},
            {"role":"tool", "tool_call_id":"call-1", "content":KEY,
             "structuredContent":{"a.b[0]/~":KEY}, "_meta":{"note":KEY}}
        ]
    }));
    let result = govern(&engine(POLICY), &mut CanonicalRequest::default(), &mut wire);
    assert!(matches!(result.evaluation.decision, Decision::Warn { .. }));
    assert!(!String::from_utf8_lossy(&wire.bytes).contains(KEY));
    let value: Value = serde_json::from_slice(&wire.bytes).unwrap();
    assert_eq!(value["messages"][0]["tool_calls"][0]["id"], "call-1");
    assert_eq!(value["messages"][1]["tool_call_id"], "call-1");
    let args: Value = serde_json::from_str(
        value["messages"][0]["tool_calls"][0]["function"]["arguments"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(args["credential"], REDACTION_MARKER);
}

#[test]
fn protected_fields_and_secret_keys_fail_closed_without_partial_changes() {
    for extra in [
        json!({"id":KEY}),
        json!({KEY:"value"}),
        json!({"data":KEY}),
        json!({"type":"thinking", "signature":"signed", "thinking":KEY}),
    ] {
        let mut wire = body(json!({"system":KEY, "extra":extra}));
        let original = wire.bytes.clone();
        let result = govern(&engine(POLICY), &mut CanonicalRequest::default(), &mut wire);
        assert!(matches!(result.evaluation.decision, Decision::Deny { .. }));
        assert_eq!(wire.bytes, original);
        assert!(!format!("{:?}", result.evaluation).contains(KEY));
    }
}

#[test]
fn signed_reasoning_is_preserved_while_other_text_is_repaired() {
    let signature = "D8sK3mN7pQ2rT9vW4xY6zA1bC5dE0fG8hJ3kL7mP2qR9sT4uV6wX1yZ5aB0cD8";
    let mut wire = body(json!({"messages":[{"role":"assistant", "content":[
        {"type":"thinking", "thinking":"safe thought", "signature":signature},
        {"type":"text", "text":KEY}]}]}));
    let result = govern(&engine(POLICY), &mut CanonicalRequest::default(), &mut wire);
    assert!(matches!(result.evaluation.decision, Decision::Warn { .. }));
    assert!(String::from_utf8_lossy(&wire.bytes).contains(signature));
    assert!(!String::from_utf8_lossy(&wire.bytes).contains(KEY));
}

#[test]
fn disabled_and_warn_modes_leave_requests_unchanged() {
    for config in [
        "governance:\n  enabled: false\n  policies:\n    - id: secret_scan\n",
        "governance:\n  policies:\n    - id: secret_scan\n      enabled: false\n",
        "governance:\n  policies:\n    - id: secret_scan\n      mode: warn\n",
    ] {
        let mut wire = body(json!({"system":KEY}));
        let original = wire.bytes.clone();
        let result = govern(&engine(config), &mut CanonicalRequest::default(), &mut wire);
        assert!(!matches!(result.evaluation.decision, Decision::Deny { .. }));
        assert_eq!(result.recovery_count, 0);
        assert_eq!(wire.bytes, original);
    }
}

#[test]
fn custom_prefix_removes_whole_value_and_respects_configured_policy_order() {
    let engine = engine(
        "governance:\n  policies:\n    - id: rate_limit\n      requests_per_window: 1\n    - id: secret_scan\n      extra_patterns:\n        - name: Internal Credential\n          prefix: PRIVATE_\n",
    );
    let mut wire = body(json!({"system":"Use PRIVATE_sensitive here"}));
    let result = govern(&engine, &mut CanonicalRequest::default(), &mut wire);
    assert!(matches!(result.evaluation.decision, Decision::Warn { .. }));
    let value: Value = serde_json::from_slice(&wire.bytes).unwrap();
    assert_eq!(value["system"], REDACTION_MARKER);
    let mut second = body(json!({"system":"Use PRIVATE_sensitive here"}));
    let original = second.bytes.clone();
    let result = govern(&engine, &mut CanonicalRequest::default(), &mut second);
    assert!(matches!(result.evaluation.decision, Decision::Deny { .. }));
    assert_eq!(result.recovery_count, 0);
    assert_eq!(second.bytes, original);
}

#[test]
fn inspection_budget_and_ambiguous_paths_fail_closed() {
    for value in [
        json!({"system":"a".repeat(2 * 1024 * 1024 + 1)}),
        json!({"a.b":KEY,"a":{"b":KEY}}),
    ] {
        let mut wire = body(value);
        let result = govern(&engine(POLICY), &mut CanonicalRequest::default(), &mut wire);
        assert!(matches!(result.evaluation.decision, Decision::Deny { .. }));
    }
}

#[test]
fn secret_in_middle_of_large_history_is_removed_without_clipping() {
    let mut wire =
        body(json!({"system":format!("{} {KEY} {}", "a ".repeat(40000), "b ".repeat(40000))}));
    let result = govern(&engine(POLICY), &mut CanonicalRequest::default(), &mut wire);
    assert!(matches!(result.evaluation.decision, Decision::Warn { .. }));
    assert!(!String::from_utf8_lossy(&wire.bytes).contains(KEY));
}

#[test]
fn failed_reverification_does_not_commit_a_replacement() {
    let engine = engine(
        "governance:\n  policies:\n    - id: secret_scan\n      extra_patterns:\n        - name: Forbidden marker\n          prefix: REDACTED_BY_GOVERNANCE\n",
    );
    let mut wire = body(json!({"system":KEY}));
    let original = wire.bytes.clone();
    let result = govern(&engine, &mut CanonicalRequest::default(), &mut wire);
    assert!(matches!(result.evaluation.decision, Decision::Deny { .. }));
    assert_eq!(result.recovery_count, 0);
    assert_eq!(wire.bytes, original);
}

#[test]
fn gemini_signed_function_arguments_are_not_rewritten() {
    let mut wire = body(
        json!({"contents":[{"parts":[{"functionCall":{"name":"lookup", "args":{"credential":KEY}}, "thoughtSignature":"signature"}]}]}),
    );
    let original = wire.bytes.clone();
    let result = govern(&engine(POLICY), &mut CanonicalRequest::default(), &mut wire);
    assert!(matches!(result.evaluation.decision, Decision::Deny { .. }));
    assert_eq!(wire.bytes, original);
}

#[tokio::test]
async fn recovery_header_preserves_buffered_and_streaming_bodies() {
    use axum::body::Body;
    use axum::response::Response;
    use systemprompt_api::services::gateway::service::stages::recovery::attach_recovery_count;
    for streaming in [false, true] {
        let body = if streaming {
            Body::from_stream(futures_util::stream::iter([Ok::<_, std::io::Error>(
                "data: ok\n\n",
            )]))
        } else {
            Body::from("data: ok\n\n")
        };
        let mut response = Response::new(body);
        attach_recovery_count(&mut response, 2);
        assert_eq!(response.headers()["x-systemprompt-recovery-count"], "2");
        assert_eq!(
            axum::body::to_bytes(response.into_body(), 1024)
                .await
                .unwrap(),
            "data: ok\n\n"
        );
    }
    let mut response = Response::new(Body::empty());
    attach_recovery_count(&mut response, 0);
    assert!(
        !response
            .headers()
            .contains_key("x-systemprompt-recovery-count")
    );
}

#[test]
fn failed_repairs_report_safe_locations_without_leaking_secret_keys() {
    let mut wire = body(json!({"messages":[{"id":KEY}], KEY:"value"}));
    let result = govern(&engine(POLICY), &mut CanonicalRequest::default(), &mut wire);
    assert!(matches!(result.evaluation.decision, Decision::Deny { .. }));
    assert!(
        result
            .recovery_locations
            .iter()
            .any(|path| path == "forwarded.$.messages[0].id")
    );
    assert!(!format!("{result:?}").contains(KEY));
}

#[test]
fn invalid_json_and_excessive_findings_cannot_be_forwarded() {
    for bytes in [
        Bytes::from_static(b"not JSON"),
        body(json!({"system":format!("{KEY} ").repeat(5000)})).bytes,
    ] {
        let mut wire = PreparedBody {
            bytes: bytes.clone(),
            raw_lane: true,
        };
        let result = govern(&engine(POLICY), &mut CanonicalRequest::default(), &mut wire);
        assert!(matches!(result.evaluation.decision, Decision::Deny { .. }));
        assert_eq!(wire.bytes, bytes);
    }
}
