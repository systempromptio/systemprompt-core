use systemprompt_identifiers::{CallId, SessionId, UserId};
use systemprompt_security::authz::types::Decision;
use systemprompt_security::policy::secrets::{
    EntropyConfig, REDACTION_MARKER, redact_spans, secret_findings,
};
use systemprompt_security::policy::types::AccessScope;
use systemprompt_security::policy::{
    AgentScope, ChainEntryResult, GovernanceConfig, GovernanceEngine, GovernedInput,
    GovernedTarget, PolicyContext,
};

const KEY: &str = "AKIAIOSFODNN7EXAMPLE";

#[test]
fn recovery_finds_all_spans_with_utf8_offsets_and_no_credential_in_debug() {
    let text = format!("Résumé 🔒 {KEY} and {KEY} done");
    let input = GovernedInput::prompt_text(text.clone());
    let findings = secret_findings(&input, &EntropyConfig::default());
    assert_eq!(findings.len(), 2);
    assert!(findings.iter().all(|f| &text[f.span.clone()] == KEY));
    assert!(!format!("{findings:?}").contains(KEY));
    let redacted = redact_spans(&text, findings.into_iter().map(|f| f.span)).unwrap();
    assert_eq!(
        redacted,
        format!("Résumé 🔒 {REDACTION_MARKER} and {REDACTION_MARKER} done")
    );
}

#[test]
fn overlapping_findings_are_merged_and_invalid_offsets_are_rejected() {
    assert_eq!(
        redact_spans("abcdefghij", [2..7, 4..9, 2..7]).unwrap(),
        format!("ab{REDACTION_MARKER}j")
    );
    assert!(redact_spans("é", [1..2]).is_none());
    assert!(redact_spans("abc", [0..4]).is_none());
    assert!(redact_spans("abc", [1..1]).is_none());
}

#[test]
fn prefix_only_patterns_remove_the_entire_secret_bearing_value() {
    for text in [
        "-----BEGIN PRIVATE KEY-----\nthis-is-sensitive\n-----END PRIVATE KEY-----",
        "aws_secret_access_key = sensitive-value",
        "Bearer eyJhbGciOiABC.payload.signature",
    ] {
        let input = GovernedInput::prompt_text(text.to_owned());
        let findings = secret_findings(&input, &EntropyConfig::default());
        assert_eq!(
            redact_spans(text, findings.into_iter().map(|f| f.span)).unwrap(),
            REDACTION_MARKER
        );
    }
}

#[test]
fn recovery_is_explicit_reverified_and_never_applied_to_tools() {
    let config =
        GovernanceConfig::parse("governance:\n  policies:\n    - id: secret_scan\n").unwrap();
    let engine = GovernanceEngine::from_config(&config).unwrap();
    let input = GovernedInput::prompt_text(KEY.to_owned());
    let session = SessionId::generate();
    let user = UserId::new("recovery-user");
    let call = CallId::generate();
    let mut ctx = PolicyContext {
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
    assert!(matches!(
        engine.evaluate(&ctx).decision,
        Decision::Deny { .. }
    ));
    assert!(matches!(
        engine
            .evaluate_with_prompt_recovery(&ctx, |_| Some(input.clone()))
            .decision,
        Decision::Deny { .. }
    ));
    let evaluation = engine.evaluate_with_prompt_recovery(&ctx, |_| {
        Some(GovernedInput::prompt_text(REDACTION_MARKER.to_owned()))
    });
    assert!(matches!(evaluation.decision, Decision::Warn { .. }));
    assert_eq!(evaluation.chain[0].result, ChainEntryResult::Warn);
    assert!(!format!("{evaluation:?}").contains(KEY));
    ctx.target = GovernedTarget::Tool {
        tool: systemprompt_identifiers::McpToolName::new("send_email"),
    };
    assert!(matches!(
        engine
            .evaluate_with_prompt_recovery(&ctx, |_| panic!("MCP actions must remain denied"))
            .decision,
        Decision::Deny { .. }
    ));
}

static EVALUATIONS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

#[derive(Debug)]
struct CountEvaluations;

impl systemprompt_security::policy::GovernancePolicy for CountEvaluations {
    fn id(&self) -> systemprompt_identifiers::PolicyId {
        systemprompt_identifiers::PolicyId::new("recovery_count_evaluations")
    }
    fn name(&self) -> &'static str {
        "count"
    }
    fn description(&self) -> &'static str {
        "count every evaluation"
    }
    fn evaluate(&self, _: &PolicyContext<'_>) -> Decision {
        EVALUATIONS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Decision::Allow {
            matched_by: systemprompt_security::authz::types::MatchedBy::DefaultIncluded,
        }
    }
}
systemprompt_security::register_governance_policy!("recovery_count_evaluations", |_| Box::new(
    CountEvaluations
));

#[test]
fn recovery_never_reexecutes_earlier_policies_and_honors_later_denials() {
    let config = GovernanceConfig::parse("governance:\n  policies:\n    - id: recovery_count_evaluations\n    - id: secret_scan\n    - id: rate_limit\n      requests_per_window: 0\n").unwrap();
    let engine = GovernanceEngine::from_config(&config).unwrap();
    let input = GovernedInput::prompt_text(KEY.to_owned());
    let session = SessionId::generate();
    let user = UserId::new("recovery-counter-user");
    let call = CallId::generate();
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
    EVALUATIONS.store(0, std::sync::atomic::Ordering::SeqCst);
    let evaluation = engine.evaluate_with_prompt_recovery(&ctx, |_| {
        Some(GovernedInput::prompt_text(REDACTION_MARKER.to_owned()))
    });
    assert_eq!(EVALUATIONS.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert!(matches!(evaluation.decision, Decision::Deny { .. }));
    assert_eq!(evaluation.chain[1].result, ChainEntryResult::Warn);
    assert_eq!(evaluation.chain[2].result, ChainEntryResult::Fail);
}
